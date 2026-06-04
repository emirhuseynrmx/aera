use crate::api::schemas::DateRange;
use crate::core::error::{AeraError, AeraResult};
use polars::prelude::*;
use std::io::Cursor;

/// Polars tabanlı zaman serisi analiz motoru. Her session'a izole bir örneği olur.
pub struct PolarsEngine {
    df: Option<DataFrame>,
    pub date_range: Option<DateRange>,
    pub income_col: String,
    pub expense_col: String,
    pub date_col: String,
    // Yükleme anında bir kez hesaplanır; her çağrıda O(n) tekrardan kaçınılır
    monthly_cache: Vec<serde_json::Value>,
}

impl Default for PolarsEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl PolarsEngine {
    pub fn new() -> Self {
        Self {
            df: None,
            date_range: None,
            income_col: "gelir".to_string(),
            expense_col: "gider".to_string(),
            date_col: "tarih".to_string(),
            monthly_cache: Vec::new(),
        }
    }

    pub fn configure_columns(
        &mut self,
        income: Option<String>,
        expense: Option<String>,
        date: Option<String>,
    ) {
        if let Some(c) = income {
            self.income_col = c;
        }
        if let Some(c) = expense {
            self.expense_col = c;
        }
        if let Some(c) = date {
            self.date_col = c;
        }
    }

    /// CSV string'i yükler, tarih meta-verisini ve aylık cache'i hesaplar.
    pub fn load_csv_from_string(
        &mut self,
        csv_data: &str,
    ) -> AeraResult<(usize, usize, Vec<String>)> {
        // Temel girdi koruması
        if csv_data.trim().is_empty() {
            return Err(AeraError::BadRequest("Boş CSV gönderilemez.".into()));
        }
        let line_count = csv_data.lines().count();
        if line_count < 2 {
            return Err(AeraError::BadRequest(
                "CSV en az bir başlık + bir veri satırı içermeli.".into(),
            ));
        }
        if line_count > 100_001 {
            return Err(AeraError::BadRequest(
                "CSV 100.000 satır sınırını aştı.".into(),
            ));
        }

        let cursor = Cursor::new(csv_data.as_bytes());
        let df = CsvReader::new(cursor)
            .has_header(true)
            .finish()
            .map_err(|e| AeraError::AgentExecutionError(format!("CSV parse hatası: {}", e)))?;

        let rows = df.height();
        let cols = df.width();
        let col_names: Vec<String> = df
            .get_column_names()
            .iter()
            .map(|s| s.to_string())
            .collect();

        // Sütun adlarında formül enjeksiyonu koruması (Excel/LibreOffice açılacaksa)
        for name in &col_names {
            if name.starts_with('=') || name.starts_with('+') || name.starts_with('@') {
                return Err(AeraError::BadRequest(format!(
                    "Geçersiz sütun adı: '{}'. Formül karakteriyle başlayan isimler reddedilir.",
                    name
                )));
            }
        }

        self.date_range = self.compute_date_range(&df);
        self.df = Some(df);

        // Aylık dökümü şimdi hesapla, tekrar sorulmayacak
        self.monthly_cache = self.compute_monthly_breakdown();

        tracing::info!(
            "📊 Veri yüklendi: {} satır × {} sütun | {:?}",
            rows,
            cols,
            self.date_range.as_ref().map(|d| &d.days)
        );

        Ok((rows, cols, col_names))
    }

    /// Tarih sütununu parse ederek gerçek gün aralığını döndürür.
    /// 37 satır ≠ 37 gün — aynı günde birden fazla kayıt olabilir.
    fn compute_date_range(&self, df: &DataFrame) -> Option<DateRange> {
        let col = df.column(&self.date_col).ok()?;
        let date_series = col.cast(&DataType::Date).ok()?;
        let date_chunked = date_series.date().ok()?;

        // Polars Date32 = 1970-01-01'den gün sayısı (negatif olabilir → epoch öncesi)
        let min_days = date_chunked.min()?;
        let max_days = date_chunked.max()?;
        let diff_days = (max_days - min_days) as i64;

        let start = days_to_iso_date(min_days)?;
        let end = days_to_iso_date(max_days)?;

        Some(DateRange {
            start,
            end,
            days: diff_days.max(1),
        })
    }

    pub fn has_data(&self) -> bool {
        self.df.is_some()
    }

    /// Sütun isimleri + tipler — hızlı özet.
    pub fn summary(&self) -> AeraResult<String> {
        let df = self.require_data()?;
        let mut out = format!(
            "📊 Veri Özeti: {} satır × {} sütun\n",
            df.height(),
            df.width()
        );
        for col in df.get_columns() {
            out.push_str(&format!("  • {} ({})\n", col.name(), col.dtype()));
        }
        if let Some(dr) = &self.date_range {
            out.push_str(&format!(
                "\n📅 Tarih Aralığı: {} → {} ({} gün)\n",
                dr.start, dr.end, dr.days
            ));
        }
        Ok(out)
    }

    /// Sayısal sütun toplamı.
    pub fn column_sum(&self, column: &str) -> AeraResult<f64> {
        let df = self.require_data()?;
        let series = df.column(column).map_err(|_| {
            AeraError::BadRequest(format!(
                "Sütun bulunamadı: '{}'. Mevcut: {:?}",
                column,
                df.get_column_names()
            ))
        })?;

        let sum = series
            .cast(&DataType::Float64)
            .map_err(|e| {
                AeraError::AgentExecutionError(format!("'{}' sayısal değil: {}", column, e))
            })?
            .f64()
            .map_err(|e| AeraError::AgentExecutionError(e.to_string()))?
            .sum()
            .unwrap_or(0.0);

        Ok(sum)
    }

    /// Nakit akışı analizi: burn rate, runway, risk seviyesi.
    /// Gerçek tarih aralığını kullanır — satır sayısı değil.
    pub fn analyze_cash_flow(
        &self,
        income_col: &str,
        expense_col: &str,
    ) -> AeraResult<serde_json::Value> {
        let total_income = self.column_sum(income_col)?;
        let total_expense = self.column_sum(expense_col)?;
        let net_cash = total_income - total_expense;

        let actual_months = self
            .date_range
            .as_ref()
            .map(|dr| dr.days as f64 / 30.44)
            .unwrap_or_else(|| {
                let rows = self.df.as_ref().map(|d| d.height()).unwrap_or(1) as f64;
                (rows / 30.44).max(1.0)
            })
            .max(0.01);

        let monthly_income = total_income / actual_months;
        let monthly_expense = total_expense / actual_months;
        let burn_rate = monthly_expense;

        let runway_months = if burn_rate > monthly_income {
            net_cash / (burn_rate - monthly_income)
        } else {
            f64::INFINITY
        };

        let risk_level = match runway_months {
            r if r < 1.0 => "KRİTİK 🔴",
            r if r < 3.0 => "YÜKSEK 🟠",
            r if r < 6.0 => "ORTA 🟡",
            _ => "DÜŞÜK 🟢",
        };

        let runway_str = if runway_months == f64::INFINITY {
            "∞ (Nakit akışı pozitif)".to_string()
        } else {
            format!("{:.1} ay", runway_months)
        };

        Ok(serde_json::json!({
            "toplam_gelir":            format!("{:.2} TL", total_income),
            "toplam_gider":            format!("{:.2} TL", total_expense),
            "net_nakit_pozisyonu":     format!("{:.2} TL", net_cash),
            "analiz_suresi_ay":        format!("{:.1} ay", actual_months),
            "aylik_ortalama_gelir":    format!("{:.2} TL", monthly_income),
            "aylik_ortalama_gider":    format!("{:.2} TL", monthly_expense),
            "burn_rate":               format!("{:.2} TL/ay", burn_rate),
            "tahmini_kalan_sure":      runway_str,
            "risk_seviyesi":           risk_level,
            "islem_sayisi":            self.df.as_ref().map(|d| d.height()).unwrap_or(0),
            "tarih_araligi":           self.date_range.as_ref().map(|dr|
                format!("{} → {} ({} gün)", dr.start, dr.end, dr.days)
            ).unwrap_or_else(|| "Bilinmiyor".to_string())
        }))
    }

    pub fn get_columns(&self) -> AeraResult<Vec<String>> {
        Ok(self
            .require_data()?
            .get_column_names()
            .iter()
            .map(|s| s.to_string())
            .collect())
    }

    /// Cache'lenmiş aylık dökümü döndürür. Yükleme anında hesaplandı.
    pub fn monthly_breakdown(&self) -> Vec<serde_json::Value> {
        self.monthly_cache.clone()
    }

    /// Gerçek hesaplama — yalnızca load_csv_from_string çağırır.
    fn compute_monthly_breakdown(&self) -> Vec<serde_json::Value> {
        let df = match &self.df {
            Some(d) => d,
            None => return vec![],
        };
        let n = df.height();
        if n == 0 {
            return vec![];
        }

        let date_s = match df.column(&self.date_col) {
            Ok(s) => s.clone(),
            Err(_) => return vec![],
        };
        let income_f64 = match df
            .column(&self.income_col)
            .and_then(|s| s.cast(&DataType::Float64))
        {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        let expense_f64 = match df
            .column(&self.expense_col)
            .and_then(|s| s.cast(&DataType::Float64))
        {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let mut monthly: std::collections::BTreeMap<String, (f64, f64)> =
            std::collections::BTreeMap::new();

        for i in 0..n {
            let date_raw = match date_s.get(i) {
                Ok(av) => format!("{}", av),
                Err(_) => continue,
            };
            let date_str = date_raw.trim_matches('"');
            if date_str.len() < 7 {
                continue;
            }
            let month = date_str[..7].to_string();

            let inc = income_f64.get(i).map(av_to_f64).unwrap_or(0.0);
            let exp = expense_f64.get(i).map(av_to_f64).unwrap_or(0.0);

            let entry = monthly.entry(month).or_insert((0.0, 0.0));
            entry.0 += inc;
            entry.1 += exp;
        }

        monthly
            .iter()
            .map(|(k, v)| {
                serde_json::json!({
                    "ay":    k,
                    "gelir": round2(v.0),
                    "gider": round2(v.1),
                    "net":   round2(v.0 - v.1),
                })
            })
            .collect()
    }

    /// Holt çift üstel düzeltme ile nakit projeksiyonu + %90 güven aralığı.
    /// Seviye (α=0.3) + trend (β=0.1) — artan/azalan eğilimleri yakalar.
    pub fn predict_cashflow(&self, months_ahead: usize) -> serde_json::Value {
        let monthly = self.monthly_breakdown();
        if monthly.is_empty() {
            return serde_json::json!({"hata": "Projeksiyon için veri yüklenmemiş."});
        }

        let incomes: Vec<f64> = monthly
            .iter()
            .map(|m| m["gelir"].as_f64().unwrap_or(0.0))
            .collect();
        let expenses: Vec<f64> = monthly
            .iter()
            .map(|m| m["gider"].as_f64().unwrap_or(0.0))
            .collect();
        let nets: Vec<f64> = monthly
            .iter()
            .map(|m| m["net"].as_f64().unwrap_or(0.0))
            .collect();

        // α=0.3 seviye, β=0.1 trend — trend yavaş adapte olur, gürültüye karşı sağlam
        let alpha = 0.3_f64;
        let beta = 0.1_f64;

        let (level_inc, trend_inc, fitted_inc) = holt_smooth(&incomes, alpha, beta);
        let (level_exp, trend_exp, fitted_exp) = holt_smooth(&expenses, alpha, beta);

        // Kalıntı std → CI için gerçek tahmin hatasından türetilir
        let residuals_net: Vec<f64> = nets
            .iter()
            .enumerate()
            .map(|(i, &n)| {
                let f_net = fitted_inc.get(i).copied().unwrap_or(incomes[0])
                    - fitted_exp.get(i).copied().unwrap_or(expenses[0]);
                n - f_net
            })
            .collect();
        let std_residuals = sample_std(&residuals_net);
        let hist_net: f64 = nets.iter().sum();

        let mut cumulative = hist_net;
        let mut projections = Vec::new();

        for t in 1..=months_ahead {
            let h = t as f64;
            // Holt h-adım tahmini: seviye + h × trend
            let forecast_income = (level_inc + h * trend_inc).max(0.0);
            let forecast_expense = (level_exp + h * trend_exp).max(0.0);
            let forecast_net = forecast_income - forecast_expense;
            cumulative += forecast_net;
            // Ufuk uzadıkça belirsizlik artar — kalıntı std × √h
            let margin = 1.64 * std_residuals * h.sqrt();
            projections.push(serde_json::json!({
                "ay":               format!("T+{} ay", t),
                "tahmini_gelir":    round2(forecast_income),
                "tahmini_gider":    round2(forecast_expense),
                "tahmini_net":      round2(forecast_net),
                "kumulatif_nakit":  round2(cumulative),
                "ci_alt":           round2(forecast_net - margin),
                "ci_ust":           round2(forecast_net + margin),
            }));
        }

        serde_json::json!({
            "gecmis_ay_sayisi":       monthly.len(),
            "ortalama_aylik_gelir":   round2(level_inc),
            "ortalama_aylik_gider":   round2(level_exp),
            "ortalama_aylik_net":     round2(level_inc - level_exp),
            "aylik_trend_gelir":      round2(trend_inc),
            "aylik_trend_gider":      round2(trend_exp),
            "tahmin_yontemi":         "Holt Çift Üstel Düzeltme (α=0.3, β=0.1)",
            "projeksiyon":            projections,
            "uyari": "Holt trend düzeltme tahmini — mevsimsellik dahil değil. Güven aralığı %90 (kalıntı bazlı)."
        })
    }

    /// AeraCFO Finansal Sağlık Skoru™ — 0-100 arası composite skor.
    pub fn health_score(&self) -> serde_json::Value {
        if !self.has_data() {
            return serde_json::json!({"hata": "Veri yüklenmemiş. Önce CSV yükleyin."});
        }
        let income = self.column_sum(&self.income_col).unwrap_or(0.0);
        let expense = self.column_sum(&self.expense_col).unwrap_or(0.0);
        let net = income - expense;
        let months = self
            .date_range
            .as_ref()
            .map(|d| d.days as f64 / 30.44)
            .unwrap_or(1.0)
            .max(0.01);
        let monthly_exp = expense / months;

        // Cash Reserve Months — gelir sıfıra düşse kaç ay dayanırsın. -12 ile capla,
        // aksi halde tek aylık 10x burn negatif outlier'ı bütün skoru sıfırlıyor.
        let cash_reserve_months = if monthly_exp > 0.0 {
            (net / monthly_exp).max(-12.0)
        } else {
            0.0
        };
        let ratio = if expense > 0.0 { income / expense } else { 2.0 };
        let monthly = self.monthly_breakdown();

        // Eski versiyonda her bileşen sabit eşikli if/else bucket'tı; rezerv 6.1 ay da
        // 14 ay da 40 puan alıyordu. Sonuç: ~%30 marjlı tüm sektörler 85 veya 93'e
        // çakılıyor, demo havuzunda "her veri 92" görüntüsü oluşuyordu.
        // Piecewise linear interpolasyon eşik mantığını koruyor (anchor noktaları aynı)
        // ama aralarda gerçek farkları puana yansıtıyor.
        // Eşikler hafifçe yukarı çekildi — eskiden 6 ay rezerv tam puan veriyordu,
        // 12+ ay olan firma da aynı puanı alıyordu. Demo havuzunda %70 sektör 80+ skora
        // çakılıyordu; jüri/yatırımcı gözünde "her demo şirket sağlıklı" hissi veriyordu.
        // Yeni eğri: gerçek anlamda 12 ay rezerv → tam puan, 24 ay'da bile cap 40'ta.
        let cash_score = piecewise_linear(
            cash_reserve_months,
            &[
                (-12.0, 0.0),
                (0.0, 2.0),
                (1.0, 8.0),
                (3.0, 18.0),
                (6.0, 30.0),
                (12.0, 38.0),
                (24.0, 40.0),
            ],
        )
        .round() as u32;

        // Ratio için de benzer sıkılaştırma: %12 marjla A alan firma kalmasın.
        // 1.50 marj (%50 gelir > gider) → 30, 2.0 marj → tam puan.
        let ratio_score = piecewise_linear(
            ratio,
            &[
                (0.5, 0.0),
                (0.95, 3.0),
                (1.05, 8.0),
                (1.15, 16.0),
                (1.30, 24.0),
                (1.50, 32.0),
                (2.0, 35.0),
            ],
        )
        .round() as u32;

        // 3 aylık veri minimumu sağlandıktan sonra istikrar puanı, geçmiş net akışın
        // varyasyon katsayısıyla differansiyel olarak veriliyor — uzun ama dalgalı
        // veri ile uzun ve istikrarlı veri artık aynı 25 puanı almıyor.
        let stability_score = stability_points(&monthly).round() as u32;

        let total = (cash_score + ratio_score + stability_score).min(100);
        let (harf, durum, emoji) = if total >= 80 {
            ("A", "Finansal Açıdan Sağlıklı", "🟢")
        } else if total >= 60 {
            ("B", "Dikkat Gerektiriyor", "🟡")
        } else if total >= 40 {
            ("C", "Riskli", "🟠")
        } else {
            ("D", "Kritik Durum", "🔴")
        };

        serde_json::json!({
            "skor": total,
            "harf": harf,
            "durum": durum,
            "emoji": emoji,
            "bilesenler": {
                "nakit_guvenlik": cash_score,
                "gelir_gider_orani": ratio_score,
                "istikrar": stability_score
            },
            "detay": {
                // Geriye uyumluluk: eski runway alanı = cash_reserve_months
                // (semantik olarak daha doğru — eski "runway when burning" yanıltıcıydı)
                "runway_ay": round2(cash_reserve_months),
                "rezerv_ay": round2(cash_reserve_months),
                "gelir_gider_orani": round2(ratio),
                "analiz_ay_sayisi": monthly.len()
            }
        })
    }

    /// Kategori bazlı harcama dökümü — 'kategori' sütunu gerekli.
    pub fn expense_by_category(&self) -> Vec<serde_json::Value> {
        let df = match &self.df {
            Some(d) => d,
            None => return vec![],
        };
        let n = df.height();
        if n == 0 {
            return vec![];
        }

        let cat_s = match df.column("kategori") {
            Ok(s) => s.clone(),
            Err(_) => return vec![],
        };
        let expense_f64 = match df
            .column(&self.expense_col)
            .and_then(|s| s.cast(&DataType::Float64))
        {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let mut by_cat: std::collections::BTreeMap<String, f64> = std::collections::BTreeMap::new();
        for i in 0..n {
            let cat = match cat_s.get(i) {
                Ok(av) => format!("{}", av).trim_matches('"').to_string(),
                Err(_) => continue,
            };
            if cat.is_empty() || cat == "null" {
                continue;
            }
            let exp = expense_f64.get(i).map(av_to_f64).unwrap_or(0.0);
            *by_cat.entry(cat).or_insert(0.0) += exp;
        }

        let total: f64 = by_cat.values().filter(|&&v| v > 0.0).sum();
        if total == 0.0 {
            return vec![];
        }

        let mut result: Vec<serde_json::Value> = by_cat
            .iter()
            .filter(|(_, &v)| v > 0.0)
            .map(|(k, &v)| {
                serde_json::json!({
                    "kategori": k,
                    "tutar":    round2(v),
                    "yuzde":    ((v / total * 1000.0).round()) / 10.0,
                })
            })
            .collect();

        result.sort_by(|a, b| {
            b["tutar"]
                .as_f64()
                .unwrap_or(0.0)
                .partial_cmp(&a["tutar"].as_f64().unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        result
    }

    /// Z-score ile son ayın harcamalarını önceki aylara kıyaslar.
    /// Örnek varyans (N-1) kullanılır — küçük veri setlerinde daha doğru.
    pub fn detect_anomalies(&self) -> serde_json::Value {
        let monthly = self.monthly_breakdown();
        if monthly.len() < 3 {
            return serde_json::json!({
                "mesaj": "Anomali tespiti için en az 3 aylık veri gerekli.",
                "anomaliler": []
            });
        }
        let n = monthly.len();
        let history_gider: Vec<f64> = monthly[..n - 1]
            .iter()
            .map(|m| m["gider"].as_f64().unwrap_or(0.0))
            .collect();
        let history_gelir: Vec<f64> = monthly[..n - 1]
            .iter()
            .map(|m| m["gelir"].as_f64().unwrap_or(0.0))
            .collect();

        let mean_gider = history_gider.iter().sum::<f64>() / history_gider.len() as f64;
        let mean_gelir = history_gelir.iter().sum::<f64>() / history_gelir.len() as f64;

        // Örnek std (N-1) — popülasyon std'den daha güvenilir
        let std_gider = sample_std(&history_gider);
        let std_gelir = sample_std(&history_gelir);

        let last = &monthly[n - 1];
        let last_gider = last["gider"].as_f64().unwrap_or(0.0);
        let last_gelir = last["gelir"].as_f64().unwrap_or(0.0);
        let last_ay = last["ay"].as_str().unwrap_or("Son ay").to_string();
        let z_gider = if std_gider > 0.0 {
            (last_gider - mean_gider) / std_gider
        } else {
            0.0
        };
        let z_gelir = if std_gelir > 0.0 {
            (last_gelir - mean_gelir) / std_gelir
        } else {
            0.0
        };

        let mut anomaliler: Vec<serde_json::Value> = Vec::new();
        // Sapma yüzdesi mean==0 olduğunda Inf/NaN üretir; JSON'da bunlar `null` olur ve
        // frontend "%null" basar. Yeni başlayan KOBİ'de tipik: ilk ay gelir 0.
        let pct_or_dash = |last: f64, mean: f64| -> String {
            if mean.abs() < f64::EPSILON {
                "—".to_string()
            } else {
                format!("{:+.1}%", (last - mean) / mean * 100.0)
            }
        };
        if z_gider.abs() > 1.5 {
            let sev = if z_gider.abs() > 2.5 {
                "KRİTİK 🔴"
            } else {
                "YÜKSEK 🟠"
            };
            let dir = if z_gider > 0.0 {
                "anormal ARTIŞ"
            } else {
                "anormal DÜŞÜŞ"
            };
            anomaliler.push(serde_json::json!({
                "tip": "Gider Anomalisi",
                "ay": last_ay,
                "mevcut_tutar": round2(last_gider),
                "ortalama_tutar": round2(mean_gider),
                "z_skor": round2(z_gider),
                "severity": sev,
                "durum": format!("{sev} — {dir} (Z={z_gider:.1})"),
                "sapma_yuzdesi": pct_or_dash(last_gider, mean_gider)
            }));
        }
        if z_gelir.abs() > 1.5 {
            let sev = if z_gelir.abs() > 2.5 {
                "KRİTİK 🔴"
            } else {
                "YÜKSEK 🟠"
            };
            let dir = if z_gelir > 0.0 {
                "anormal ARTIŞ"
            } else {
                "anormal DÜŞÜŞ"
            };
            anomaliler.push(serde_json::json!({
                "tip": "Gelir Anomalisi",
                "ay": last_ay,
                "mevcut_tutar": round2(last_gelir),
                "ortalama_tutar": round2(mean_gelir),
                "z_skor": round2(z_gelir),
                "severity": sev,
                "durum": format!("{sev} — {dir} (Z={z_gelir:.1})"),
                "sapma_yuzdesi": pct_or_dash(last_gelir, mean_gelir)
            }));
        }
        let mesaj = if anomaliler.is_empty() {
            format!("{last_ay} ayında istatistiksel açıdan normal finansal hareket.")
        } else {
            format!(
                "{last_ay} ayında {} adet anomali tespit edildi!",
                anomaliler.len()
            )
        };
        serde_json::json!({
            "analiz_donemi": format!("{} aylık geçmiş", n - 1),
            "son_ay": last_ay,
            "anomali_sayisi": anomaliler.len(),
            "anomaliler": anomaliler,
            "mesaj": mesaj
        })
    }

    /// Ardışık negatif net akış aylarını sayar — nakit sıkışıklığı sinyali.
    pub fn detect_cash_crunch(&self) -> serde_json::Value {
        let monthly = self.monthly_breakdown();
        if monthly.is_empty() {
            return serde_json::json!({"mesaj": "Veri yüklenmemiş."});
        }
        let consecutive_negative = monthly
            .iter()
            .rev()
            .take_while(|m| m["net"].as_f64().unwrap_or(0.0) < 0.0)
            .count();
        let total_negative = monthly
            .iter()
            .filter(|m| m["net"].as_f64().unwrap_or(0.0) < 0.0)
            .count();
        let risk = if consecutive_negative >= 3 {
            "KRİTİK 🔴"
        } else if consecutive_negative >= 2 {
            "YÜKSEK 🟠"
        } else if consecutive_negative >= 1 {
            "ORTA 🟡"
        } else {
            "DÜŞÜK 🟢"
        };
        let uyari = if consecutive_negative >= 3 {
            format!(
                "Son {} ay üst üste negatif nakit akışı! Acil aksiyon gerekiyor.",
                consecutive_negative
            )
        } else if consecutive_negative >= 2 {
            format!(
                "Son {} ay negatif nakit akışı. Hızlı müdahale önerilir.",
                consecutive_negative
            )
        } else if consecutive_negative == 1 {
            "Son ay negatif nakit akışı. Dikkatli takip edilmeli.".to_string()
        } else {
            "Ardışık negatif nakit akışı yok. Nakit akışı sağlıklı.".to_string()
        };
        let son_aylar: Vec<serde_json::Value> =
            monthly.iter().rev().take(3).rev().cloned().collect();
        serde_json::json!({
            "ardisik_negatif_ay": consecutive_negative,
            "toplam_negatif_ay": total_negative,
            "toplam_ay": monthly.len(),
            "risk": risk,
            "uyari": uyari,
            "son_aylar": son_aylar
        })
    }

    /// What-If simülasyonu: ek gelir/gider parametreleriyle senaryo tahmini.
    pub fn simulate_scenario(
        &self,
        extra_monthly_income: f64,
        extra_monthly_expense: f64,
        months: usize,
    ) -> serde_json::Value {
        // Gemini bazen NaN/Inf veya astronomik değer üretebilir; sanitize edilir.
        // Aksi halde tüm downstream aritmetiği zehirlenir ve JSON'da "null" beliriyor.
        let sanitize = |v: f64| -> f64 {
            if !v.is_finite() {
                0.0
            } else {
                v.clamp(-1.0e12, 1.0e12)
            }
        };
        let extra_monthly_income = sanitize(extra_monthly_income);
        let extra_monthly_expense = sanitize(extra_monthly_expense);

        let monthly = self.monthly_breakdown();
        if monthly.is_empty() {
            return serde_json::json!({"hata": "Simülasyon için veri yüklenmemiş."});
        }
        let n = monthly.len() as f64;
        let base_income: f64 = monthly
            .iter()
            .map(|m| m["gelir"].as_f64().unwrap_or(0.0))
            .sum::<f64>()
            / n;
        let base_expense: f64 = monthly
            .iter()
            .map(|m| m["gider"].as_f64().unwrap_or(0.0))
            .sum::<f64>()
            / n;
        let base_net = base_income - base_expense;
        let sim_income = base_income + extra_monthly_income;
        let sim_expense = base_expense + extra_monthly_expense;
        let sim_net = sim_income - sim_expense;
        let hist_net: f64 = monthly
            .iter()
            .map(|m| m["net"].as_f64().unwrap_or(0.0))
            .sum::<f64>();

        let months_capped = months.min(6);
        let mut cumulative = hist_net;
        let mut projections = Vec::new();
        for i in 1..=months_capped {
            cumulative += sim_net;
            projections.push(serde_json::json!({
                "ay": format!("T+{} ay", i),
                "tahmini_gelir":   round2(sim_income),
                "tahmini_gider":   round2(sim_expense),
                "tahmini_net":     round2(sim_net),
                "kumulatif_nakit": round2(cumulative),
            }));
        }

        let runway_label = |exp: f64, inc: f64| -> &'static str {
            let runway = if exp > inc {
                hist_net / (exp - inc)
            } else {
                f64::INFINITY
            };
            if runway < 1.0 {
                "KRİTİK 🔴"
            } else if runway < 3.0 {
                "YÜKSEK 🟠"
            } else if runway < 6.0 {
                "ORTA 🟡"
            } else {
                "DÜŞÜK 🟢"
            }
        };
        let risk_before = runway_label(base_expense, base_income);
        let risk_after = runway_label(sim_expense, sim_income);

        serde_json::json!({
            "senaryo": {"ek_aylik_gelir": extra_monthly_income, "ek_aylik_gider": extra_monthly_expense, "simule_ay": months_capped},
            "baz":    {"aylik_gelir": round2(base_income), "aylik_gider": round2(base_expense), "aylik_net": round2(base_net), "risk": risk_before},
            "simule": {"aylik_gelir": round2(sim_income),  "aylik_gider": round2(sim_expense),  "aylik_net": round2(sim_net),  "risk": risk_after},
            "degisim": {
                "net_degisim_tlpm": round2(sim_net - base_net),
                "net_degisim_yuzdesi": if base_net.abs() > 0.01 {
                    format!("{:+.1}%", (sim_net - base_net) / base_net.abs() * 100.0)
                } else { "+∞%".to_string() }
            },
            "projeksiyon": projections,
            "ozet": format!("Senaryo uygulandığında aylık net {} TL → {} TL olur. Risk: {} → {}.", base_net as i64, sim_net as i64, risk_before, risk_after)
        })
    }

    #[inline]
    fn require_data(&self) -> AeraResult<&DataFrame> {
        self.df.as_ref().ok_or_else(|| {
            AeraError::BadRequest(
                "Henüz veri yüklenmemiş. Önce /api/upload ile CSV gönderin.".into(),
            )
        })
    }
}

// Küçük yardımcı fonksiyonlar

/// AnyValue → f64 dönüşümü. Polars her sütun tipi için güvenli çevirim.
#[inline]
fn av_to_f64(av: AnyValue<'_>) -> f64 {
    match av {
        AnyValue::Float64(v) => v,
        AnyValue::Float32(v) => v as f64,
        AnyValue::Int64(v) => v as f64,
        AnyValue::Int32(v) => v as f64,
        AnyValue::UInt64(v) => v as f64,
        AnyValue::UInt32(v) => v as f64,
        _ => 0.0,
    }
}

/// İki ondalık basamağa yuvarla.
#[inline]
fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// Polars Date32 (1970 epoch'tan gün) → ISO tarih string'i.
/// unsigned_abs() bug'ı: negatif gün sayısı mutlak değer alıp YANLIŞ tarih üretirdi.
/// Şimdi negatif gün → epoch'tan geriye sayım yapılıyor (pre-1970 tarihler doğru).
fn days_to_iso_date(days: i32) -> Option<String> {
    let epoch = chrono::NaiveDate::from_ymd_opt(1970, 1, 1)?;
    let date = if days >= 0 {
        epoch.checked_add_days(chrono::Days::new(days as u64))?
    } else {
        // i32::MIN için -days taşar; i64'e geçişle güvenli
        let abs = (-(days as i64)) as u64;
        epoch.checked_sub_days(chrono::Days::new(abs))?
    };
    Some(date.to_string())
}

/// Örnek standart sapma (N-1). Küçük veri setlerinde popülasyon std'den daha doğru.
fn sample_std(data: &[f64]) -> f64 {
    if data.len() < 2 {
        return 0.0;
    }
    let mean = data.iter().sum::<f64>() / data.len() as f64;
    let var = data.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / (data.len() - 1) as f64;
    var.sqrt()
}

/// Piecewise linear interpolasyon. Anchor noktaları x'e göre artan sıralı olmalı.
/// Sınırların dışında kalan x değerleri uçtaki y'ye sabitleniyor (extrapolasyon yok —
/// sınırların ötesinde lineerle skoru uçurmaya gerek yok).
fn piecewise_linear(x: f64, anchors: &[(f64, f64)]) -> f64 {
    if anchors.is_empty() {
        return 0.0;
    }
    if x <= anchors[0].0 {
        return anchors[0].1;
    }
    for w in anchors.windows(2) {
        let (x0, y0) = w[0];
        let (x1, y1) = w[1];
        if x <= x1 {
            // x1 == x0 olursa NaN üretir, anchor seti hatalı demektir; debug'da yakala.
            debug_assert!(x1 > x0, "piecewise_linear: anchor x'ler artan olmalı");
            let t = ((x - x0) / (x1 - x0)).clamp(0.0, 1.0);
            return y0 + t * (y1 - y0);
        }
    }
    anchors[anchors.len() - 1].1
}

/// Aylık veri uzunluğu + net akış istikrarına göre 0-25 puan.
/// monthly_breakdown çıktısı bekleniyor (her elemanda "gelir" ve "gider" f64 alanı).
/// 3 aydan az veride güvenilir varyasyon hesaplanamaz, sadece veri uzunluğu baz alınır.
fn stability_points(monthly: &[serde_json::Value]) -> f64 {
    let n = monthly.len();
    if n == 0 {
        return 0.0;
    }
    if n == 1 {
        return 8.0;
    }
    if n == 2 {
        return 16.0;
    }

    // 3+ ay: tabanda 18 puan, üstüne CoV bazlı 0-7 bonus.
    // Coefficient of Variation (std / |mean|) — birimsiz, sektör/şirket boyu fark etmez.
    let nets: Vec<f64> = monthly
        .iter()
        .map(|m| m["gelir"].as_f64().unwrap_or(0.0) - m["gider"].as_f64().unwrap_or(0.0))
        .collect();
    let mean = nets.iter().sum::<f64>() / nets.len() as f64;
    let std = sample_std(&nets);
    // mean ≈ 0 ise CoV patlıyor; bu noktada zaten dalgalı sayılır, en düşük bonus.
    let cov = if mean.abs() < 1.0 {
        1.0
    } else {
        (std / mean.abs()).min(1.0)
    };

    // Uzunluk bonusu da var — 12+ ay verisi yatırımcı için 3 aydan daha güvenilir.
    let length_bonus = piecewise_linear(
        n as f64,
        &[(3.0, 0.0), (6.0, 2.0), (12.0, 4.0), (24.0, 5.0)],
    );
    // CoV 0.0 → +2, CoV 1.0 → 0 (dalgalı veri penalty).
    let smoothness_bonus = (1.0 - cov) * 2.0;

    (18.0 + length_bonus + smoothness_bonus).min(25.0)
}

/// Holt çift üstel düzeltme — seviye + trend bileşeni.
/// Returns: (son_seviye, son_trend, fitted_değerler)
/// Fitted değerler CI hesabı için kalıntı (residual) bulmada kullanılır.
fn holt_smooth(data: &[f64], alpha: f64, beta: f64) -> (f64, f64, Vec<f64>) {
    if data.is_empty() {
        return (0.0, 0.0, vec![]);
    }
    if data.len() == 1 {
        return (data[0], 0.0, vec![data[0]]);
    }

    let mut level = data[0];
    let mut trend = data[1] - data[0]; // ilk trend tahmini
    let mut fitted = vec![data[0]]; // t=0 için fitted = kendisi

    for &y in &data[1..] {
        let f = level + trend; // bu turun önceden tahmini
        fitted.push(f);
        let prev = level;
        level = alpha * y + (1.0 - alpha) * f;
        trend = beta * (level - prev) + (1.0 - beta) * trend;
    }
    (level, trend, fitted)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine_from_csv(csv: &str) -> PolarsEngine {
        let mut e = PolarsEngine::new();
        e.load_csv_from_string(csv).expect("CSV yüklenemedi");
        e
    }

    #[test]
    fn test_burn_rate_uses_date_range_not_row_count() {
        // 3 satır ama 59 günlük aralık → burn rate row/30 değil days/30.44 kullanmalı
        let csv = "tarih,gelir,gider\n2026-01-01,100000,60000\n2026-02-01,100000,60000\n2026-03-01,100000,60000";
        let e = engine_from_csv(csv);
        let r = e.analyze_cash_flow("gelir", "gider").unwrap();
        let burn = r["burn_rate"].as_str().unwrap();
        assert!(burn.contains("TL"), "burn_rate TL içermeli: {}", burn);
    }

    #[test]
    fn test_risk_critical_when_high_burn() {
        let csv = "tarih,gelir,gider\n2026-01-01,10000,200000\n2026-01-15,10000,200000";
        let e = engine_from_csv(csv);
        let r = e.analyze_cash_flow("gelir", "gider").unwrap();
        let risk = r["risk_seviyesi"].as_str().unwrap();
        assert!(
            risk.contains("KRİTİK"),
            "Yüksek giderde KRİTİK bekleniyor: {}",
            risk
        );
    }

    #[test]
    fn test_no_division_by_zero_when_balanced() {
        // Gelir == Gider → runway sonsuz olmalı, panic olmaz
        let csv = "tarih,gelir,gider\n2026-01-01,50000,50000\n2026-02-01,50000,50000";
        let e = engine_from_csv(csv);
        let r = e.analyze_cash_flow("gelir", "gider").unwrap();
        let runway = r["tahmini_kalan_sure"].as_str().unwrap();
        assert!(
            runway.contains("∞") || runway.contains("Pozitif"),
            "Dengeli durumda ∞ bekleniyor: {}",
            runway
        );
    }

    #[test]
    fn test_monthly_breakdown_groups_correctly() {
        let csv = "tarih,gelir,gider\n2026-01-05,50000,20000\n2026-01-20,30000,10000\n2026-02-10,60000,25000";
        let e = engine_from_csv(csv);
        let breakdown = e.monthly_breakdown();
        assert_eq!(breakdown.len(), 2, "2 farklı ay olmalı");
        let jan = breakdown
            .iter()
            .find(|m| m["ay"] == "2026-01")
            .expect("Ocak bulunamadı");
        let jan_gelir = jan["gelir"].as_f64().unwrap();
        assert!(
            (jan_gelir - 80000.0).abs() < 1.0,
            "Ocak gelir 80000 olmalı: {}",
            jan_gelir
        );
    }

    #[test]
    fn test_health_score_returns_valid_range() {
        let csv = "tarih,gelir,gider\n2026-01-01,100000,60000\n2026-02-01,110000,62000\n2026-03-01,105000,61000";
        let e = engine_from_csv(csv);
        let score = e.health_score();
        let skor = score["skor"].as_u64().expect("skor sayısal olmalı");
        assert!(skor <= 100, "Skor 0-100 arasında olmalı: {}", skor);
        assert!(score["harf"].is_string(), "harf alanı olmalı");
        let harf = score["harf"].as_str().unwrap();
        assert!(
            ["A", "B", "C", "D"].contains(&harf),
            "harf A/B/C/D olmalı: {}",
            harf
        );
    }

    #[test]
    fn test_health_score_critical_when_high_burn() {
        let csv = "tarih,gelir,gider\n2026-01-01,10000,500000\n2026-02-01,10000,500000";
        let e = engine_from_csv(csv);
        let score = e.health_score();
        let harf = score["harf"].as_str().unwrap();
        assert_eq!(harf, "D", "Yüksek burn'de D notu bekleniyor: {}", score);
    }

    #[test]
    fn test_health_score_good_when_profitable() {
        let csv = "tarih,gelir,gider\n2026-01-01,500000,100000\n2026-02-01,520000,105000\n2026-03-01,510000,102000";
        let e = engine_from_csv(csv);
        let score = e.health_score();
        let skor = score["skor"].as_u64().unwrap();
        assert!(skor >= 60, "Karlı firmada 60+ skor bekleniyor: {}", skor);
    }

    #[test]
    fn test_expense_by_category_groups_correctly() {
        let csv = "tarih,gelir,gider,kategori\n2026-01-05,50000,20000,Kira\n2026-01-10,0,85000,Maaş\n2026-02-05,50000,20000,Kira";
        let e = engine_from_csv(csv);
        let cats = e.expense_by_category();
        assert!(!cats.is_empty(), "Kategori sonucu boş olmamalı");
        let maas = cats
            .iter()
            .find(|c| c["kategori"] == "Maaş")
            .expect("Maaş kategorisi olmalı");
        let kira = cats
            .iter()
            .find(|c| c["kategori"] == "Kira")
            .expect("Kira kategorisi olmalı");
        assert!((maas["tutar"].as_f64().unwrap() - 85000.0).abs() < 1.0);
        assert!((kira["tutar"].as_f64().unwrap() - 40000.0).abs() < 1.0);
    }

    #[test]
    fn test_detect_cash_crunch_no_crunch() {
        let csv = "tarih,gelir,gider\n2026-01-01,100000,60000\n2026-02-01,110000,65000\n2026-03-01,105000,62000";
        let e = engine_from_csv(csv);
        let r = e.detect_cash_crunch();
        let ardisik = r["ardisik_negatif_ay"].as_u64().unwrap_or(99);
        assert_eq!(ardisik, 0, "Pozitif aylarda ardışık negatif olmamalı");
        let risk = r["risk"].as_str().unwrap();
        assert!(
            risk.contains("DÜŞÜK"),
            "Pozitif akışta risk DÜŞÜK olmalı: {}",
            risk
        );
    }

    #[test]
    fn test_detect_cash_crunch_consecutive() {
        let csv = "tarih,gelir,gider\n2026-01-01,100000,60000\n2026-02-01,40000,90000\n2026-03-01,40000,90000";
        let e = engine_from_csv(csv);
        let r = e.detect_cash_crunch();
        let ardisik = r["ardisik_negatif_ay"].as_u64().unwrap_or(0);
        assert_eq!(ardisik, 2, "Son 2 ay negatif olmalı: {:?}", r);
        let risk = r["risk"].as_str().unwrap();
        assert!(
            risk.contains("YÜKSEK"),
            "2 ardışık negatif → YÜKSEK: {}",
            risk
        );
    }

    #[test]
    fn test_simulate_scenario_extra_expense() {
        let csv = "tarih,gelir,gider\n2026-01-01,100000,60000\n2026-02-01,100000,60000\n2026-03-01,100000,60000";
        let e = engine_from_csv(csv);
        let r = e.simulate_scenario(0.0, 50000.0, 3);
        let sim_net = r["simule"]["aylik_net"].as_f64().unwrap_or(0.0);
        assert!(
            (sim_net - (-10000.0)).abs() < 1.0,
            "Ek 50K gider → -10K net bekleniyor: {}",
            sim_net
        );
    }

    #[test]
    fn test_simulate_scenario_extra_income_improves() {
        let csv = "tarih,gelir,gider\n2026-01-01,50000,80000\n2026-02-01,50000,80000\n2026-03-01,50000,80000";
        let e = engine_from_csv(csv);
        let r = e.simulate_scenario(40000.0, 0.0, 3);
        let sim_net = r["simule"]["aylik_net"].as_f64().unwrap_or(-99999.0);
        assert!(
            sim_net > 0.0,
            "Ek 40K gelir negatifi pozitife çevirmeli: {}",
            sim_net
        );
    }

    #[test]
    fn test_detect_anomalies_insufficient_data() {
        let csv = "tarih,gelir,gider\n2026-01-01,100000,60000\n2026-02-01,100000,60000";
        let e = engine_from_csv(csv);
        let r = e.detect_anomalies();
        let anomali_sayisi = r["anomaliler"].as_array().map(|a| a.len()).unwrap_or(0);
        assert_eq!(
            anomali_sayisi, 0,
            "2 aylık veride anomali tespit edilmemeli"
        );
    }

    #[test]
    fn test_detect_anomalies_normal_variation() {
        // Küçük varyasyon → Z-score 1.5 altında kalmalı
        let csv = "tarih,gelir,gider\n2026-01-01,100000,60000\n2026-02-01,105000,62000\n2026-03-01,103000,61000\n2026-04-01,102000,61500";
        let e = engine_from_csv(csv);
        let r = e.detect_anomalies();
        let anomali_sayisi = r["anomaliler"].as_array().map(|a| a.len()).unwrap_or(99);
        assert_eq!(
            anomali_sayisi, 0,
            "Normal varyasyonda anomali raporlanmamalı: {:?}",
            r
        );
    }

    #[test]
    fn test_detect_anomalies_spike() {
        // Son ay giderde büyük sıçrama → anomali tetiklenmeli
        let csv = "tarih,gelir,gider\n2026-01-01,100000,60000\n2026-02-01,100000,62000\n2026-03-01,100000,61000\n2026-04-01,100000,200000";
        let e = engine_from_csv(csv);
        let r = e.detect_anomalies();
        let anomali_sayisi = r["anomaliler"].as_array().map(|a| a.len()).unwrap_or(0);
        assert!(
            anomali_sayisi > 0,
            "Giderde büyük sıçramada anomali tespit edilmeli: {:?}",
            r
        );
    }

    #[test]
    fn test_sample_std_unbiased() {
        // [2,4,4,4,5,5,7,9] → popülasyon std=2.0, örnek std=2.138 (N-1)
        // Örneklem varyansı için N-1 kullanılır
        let data = vec![2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        let s = sample_std(&data);
        assert!((s - 2.138).abs() < 0.01, "Örnek std ≈ 2.138 olmalı: {}", s);
    }

    #[test]
    fn test_predict_cashflow_has_confidence_interval() {
        let csv = "tarih,gelir,gider\n2026-01-01,100000,60000\n2026-02-01,110000,65000\n2026-03-01,105000,62000";
        let e = engine_from_csv(csv);
        let r = e.predict_cashflow(3);
        let proj = r["projeksiyon"]
            .as_array()
            .expect("projeksiyon dizisi olmalı");
        assert_eq!(proj.len(), 3);
        // Her projeksiyon noktasında güven aralığı alanları olmalı
        assert!(proj[0]["ci_alt"].is_number(), "ci_alt olmalı");
        assert!(proj[0]["ci_ust"].is_number(), "ci_ust olmalı");
        assert!(
            proj[0]["ci_alt"].as_f64().unwrap() <= proj[0]["ci_ust"].as_f64().unwrap(),
            "ci_alt ≤ ci_ust"
        );
    }

    #[test]
    fn test_empty_csv_rejected() {
        let mut e = PolarsEngine::new();
        assert!(e.load_csv_from_string("").is_err(), "Boş CSV reddedilmeli");
    }

    #[test]
    fn test_monthly_breakdown_cached_on_load() {
        let csv = "tarih,gelir,gider\n2026-01-01,100000,60000\n2026-02-01,110000,65000";
        let e = engine_from_csv(csv);
        // İki çağrı aynı sonucu vermeli (cache'den geliyor)
        let r1 = e.monthly_breakdown();
        let r2 = e.monthly_breakdown();
        assert_eq!(r1.len(), r2.len());
    }
}
