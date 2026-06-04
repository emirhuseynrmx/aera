// Türkiye KOBİ teşvik veritabanı + IDF-ağırlıklı retrieval.
//
// Veri: data/incentives.json (compile-time embed, deterministik build).
// Genişletme: JSON dosyasına program eklemek yeterli; kod değişikliği gerekmez.
//
// Skorlama: corpus genelinde nadir geçen anahtar kelimeler daha yüksek puan alır
// (klasik TF-IDF mantığı). Alan ağırlıkları:
//   - etiket eşleşmesi  → 3.0 × IDF
//   - isim eşleşmesi    → 2.0 × IDF
//   - kurum eşleşmesi   → 1.5 × IDF
//   - açıklama eşleşmesi→ 1.0 × IDF
// "kosgeb" gibi 9 programda geçen yaygın terim, "patent" gibi tek programda geçen
// nadir terimin ağırlığını yememeli — IDF tam olarak bunu sağlıyor.

use serde::Deserialize;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

#[derive(Debug, Deserialize)]
struct IncentiveProgram {
    isim: String,
    kurum: String,
    tutar: String,
    tip: String,
    basvuru_url: String,
    basvuru_durumu: String,
    aciklama: String,
    etiketler: Vec<String>,
}

// JSON'u binary'ye gömüyoruz — runtime'da dosya okumaya gerek yok, container'da
// data/ dizini eksik olsa bile çalışır.
const INCENTIVES_JSON: &str = include_str!("../../data/incentives.json");

fn programs() -> &'static Vec<IncentiveProgram> {
    static CELL: OnceLock<Vec<IncentiveProgram>> = OnceLock::new();
    CELL.get_or_init(|| {
        serde_json::from_str(INCENTIVES_JSON)
            .expect("incentives.json parse edilemedi — JSON formatını kontrol et")
    })
}

// Corpus IDF tablosu — ilk sorguda hesaplanır, sonra sabit kalır.
// Smooth IDF formülü: ln((N+1)/(DF+1)) + 1.0  → 0'a inmez, nadir terim tepe yapar.
fn idf_table() -> &'static HashMap<String, f64> {
    static CELL: OnceLock<HashMap<String, f64>> = OnceLock::new();
    CELL.get_or_init(|| {
        let progs = programs();
        let n = progs.len() as f64;
        let mut df: HashMap<String, usize> = HashMap::new();

        for p in progs {
            let mut terms: HashSet<String> = HashSet::new();
            // Etiketler komple token (multi-word olabilir: "ar-ge", "yapay zeka")
            for tag in &p.etiketler {
                terms.insert(tag.to_lowercase());
            }
            // Diğer alanlar whitespace ile parçalanır
            for w in tokens(&p.isim)
                .chain(tokens(&p.aciklama))
                .chain(tokens(&p.kurum))
            {
                terms.insert(w);
            }
            for t in terms {
                *df.entry(t).or_insert(0) += 1;
            }
        }

        df.into_iter()
            .map(|(term, df_count)| {
                let idf = ((n + 1.0) / (df_count as f64 + 1.0)).ln() + 1.0;
                (term, idf)
            })
            .collect()
    })
}

// Basit Türkçe-uyumlu tokenizer: lowercase + whitespace + 2+ karakter filtresi.
// Noktalama hâlâ token'a yapışık kalabilir, ama IDF tablosu hem ham hem temiz
// formu içerir (alt-string match yine yakalar).
fn tokens(s: &str) -> impl Iterator<Item = String> + '_ {
    s.to_lowercase()
        .split(|c: char| c.is_whitespace() || matches!(c, '.' | ',' | ';' | ':' | '(' | ')' | '/'))
        .filter(|w| w.len() >= 2)
        .map(|w| w.trim_matches('-').to_string())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .into_iter()
}

fn idf_of(term: &str) -> f64 {
    // Tanınmayan kelime → ortalama IDF (1.5) ver, sıfır skorla cezalandırma.
    idf_table().get(term).copied().unwrap_or(1.5)
}

/// IDF-ağırlıklı anahtar kelime retrieval ile teşvik arar.
pub fn search_incentives(query: &str) -> serde_json::Value {
    let q = query.to_lowercase();
    let q_words: Vec<String> = tokens(&q).collect();

    if q_words.is_empty() {
        return all_programs_response();
    }

    let progs = programs();
    let mut scored: Vec<(f64, &IncentiveProgram)> = progs
        .iter()
        .map(|p| {
            let mut score = 0.0_f64;
            for qw in &q_words {
                let w_idf = idf_of(qw);
                // Etiket: nadir terimi yakalarsa büyük ödül
                for tag in &p.etiketler {
                    let tag_lc = tag.to_lowercase();
                    if tag_lc.contains(qw) || qw.contains(&tag_lc) {
                        score += 3.0 * w_idf;
                    }
                }
                if p.isim.to_lowercase().contains(qw) {
                    score += 2.0 * w_idf;
                }
                if p.aciklama.to_lowercase().contains(qw) {
                    score += 1.0 * w_idf;
                }
                if p.kurum.to_lowercase().contains(qw) {
                    score += 1.5 * w_idf;
                }
            }
            (score, p)
        })
        .collect();

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    // Genel sorgular ("teşvik", "destek", "kobi", "hibe"...) tüm programları döndürür.
    let is_general = [
        "teşvik", "destek", "kob", "devlet", "hibe", "genel", "uygun", "program", "tümü",
    ]
    .iter()
    .any(|kw| q.contains(kw));

    let results: Vec<serde_json::Value> = if is_general {
        progs.iter().map(program_to_json).collect()
    } else {
        scored
            .iter()
            .filter(|(s, _)| *s > 0.0)
            .take(8)
            .map(|(score, p)| {
                let mut obj = program_to_json(p);
                if let Some(o) = obj.as_object_mut() {
                    o.insert("relevans_skoru".to_string(), json!(round2(*score)));
                }
                obj
            })
            .collect()
    };

    if results.is_empty() {
        json!({
            "sorgu": query,
            "sonuc_sayisi": 0,
            "mesaj": format!("'{}' için doğrudan eşleşme bulunamadı. Genel terimler deneyin: 'teknoloji', 'ihracat', 'ar-ge', 'nakit destek'.", query),
            "onerilen_kategoriler": ["teknoloji", "ihracat", "ar-ge", "enerji", "girişimcilik", "istihdam", "dijital"],
            "toplam_program_sayisi": progs.len()
        })
    } else {
        json!({
            "sorgu": query,
            "sonuc_sayisi": results.len(),
            "tesvik_programlari": results,
            "toplam_program_sayisi": progs.len(),
            "skorlama": "IDF-ağırlıklı retrieval (etiket×3, isim×2, kurum×1.5, açıklama×1.0)",
            "kaynak": "AeraCFO Teşvik Veritabanı v3.1 — data/incentives.json (KOSGEB, TÜBİTAK, Ticaret Bakanlığı, İŞKUR, SGK)"
        })
    }
}

fn all_programs_response() -> serde_json::Value {
    let progs = programs();
    let all: Vec<serde_json::Value> = progs.iter().map(program_to_json).collect();
    json!({
        "sorgu": "tümü",
        "sonuc_sayisi": all.len(),
        "tesvik_programlari": all,
        "toplam_program_sayisi": progs.len(),
        "kaynak": "AeraCFO Teşvik Veritabanı v3.1 — data/incentives.json"
    })
}

fn program_to_json(p: &IncentiveProgram) -> serde_json::Value {
    json!({
        "isim": p.isim,
        "kurum": p.kurum,
        "tutar": p.tutar,
        "tip": p.tip,
        "basvuru": p.basvuru_url,
        "basvuru_durumu": p.basvuru_durumu,
        "aciklama": p.aciklama,
        "etiketler": p.etiketler
    })
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_teknoloji() {
        let r = search_incentives("teknoloji yazılım");
        let n = r["sonuc_sayisi"].as_u64().unwrap_or(0);
        assert!(n > 0, "Teknoloji sorgusu sonuç dönmeli: {}", n);
    }

    #[test]
    fn test_search_nakit_destek() {
        let r = search_incentives("nakit destek acil KOBİ");
        let n = r["sonuc_sayisi"].as_u64().unwrap_or(0);
        assert!(n > 0, "Nakit destek sorgusu sonuç dönmeli");
    }

    #[test]
    fn test_search_ihracat() {
        let r = search_incentives("ihracat pazar yurt dışı");
        let n = r["sonuc_sayisi"].as_u64().unwrap_or(0);
        assert!(n > 0, "İhracat sorgusu sonuç dönmeli");
    }

    #[test]
    fn test_search_genel() {
        let r = search_incentives("teşvik");
        let n = r["sonuc_sayisi"].as_u64().unwrap_or(0);
        assert!(n >= 20, "Genel sorguda tüm programlar dönmeli: {}", n);
    }

    #[test]
    fn test_search_empty() {
        let r = search_incentives("");
        assert!(r["sonuc_sayisi"].as_u64().unwrap_or(0) > 0);
    }

    #[test]
    fn test_all_programs_have_url() {
        for p in programs() {
            assert!(
                p.basvuru_url.starts_with("https://"),
                "URL https ile başlamalı: {}",
                p.isim
            );
        }
    }

    #[test]
    fn test_idf_rare_term_outranks_common() {
        // "patent" tek programda, "kosgeb" 9 programda → patent IDF > kosgeb IDF.
        // Smoke test: nadir terim corpus'ta daha yüksek IDF puanı almalı.
        let rare = idf_of("patent");
        let common = idf_of("kosgeb");
        assert!(
            rare > common,
            "Nadir terim daha yüksek IDF almalı: patent={} kosgeb={}",
            rare,
            common
        );
    }

    #[test]
    fn test_specific_query_returns_relevant_first() {
        // "patent" sorgusunda TÜBİTAK Patent Programı ilk sırada olmalı.
        let r = search_incentives("patent buluş");
        let progs = r["tesvik_programlari"].as_array().unwrap();
        let first = progs.first().unwrap();
        assert!(
            first["isim"]
                .as_str()
                .unwrap()
                .to_lowercase()
                .contains("patent"),
            "Patent sorgusunda ilk sonuç patent programı olmalı: {}",
            first["isim"]
        );
    }
}
