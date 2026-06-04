// Dinamik demo veri üretici (Sektör bazlı, deterministik seed desteği).
//

use chrono::{Datelike, Months, NaiveDate};

#[derive(Debug, Clone, Copy)]
pub enum Pattern {
    Stable,   // düz seyir, hafif gürültü
    Growth,   // aylık ~%3-4 büyüme
    Crisis,   // ortada bir noktada gelir %45 düşüyor
    Seasonal, // sezon farkı abartılıyor (turizm, eğitim)
    Recovery, // önce düşüş sonra toparlanma
    Mature,   // büyük rakamlar, oturmuş firma
}

impl Pattern {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "growth" | "buyume" | "büyüme" => Self::Growth,
            "crisis" | "kriz" => Self::Crisis,
            "seasonal" | "sezonsal" | "sezonsel" => Self::Seasonal,
            "recovery" | "toparlanma" => Self::Recovery,
            "mature" | "olgun" => Self::Mature,
            _ => Self::Stable,
        }
    }
}

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        // Seed initialization
        Self(seed.max(1).wrapping_mul(0x9E3779B97F4A7C15))
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn jitter(&mut self, base: f64, pct: f64) -> f64 {
        base * (1.0 + (self.unit() * 2.0 - 1.0) * pct)
    }

    pub fn pick<'a, T>(&mut self, s: &'a [T]) -> &'a T {
        debug_assert!(!s.is_empty(), "pick: bos slice");
        &s[(self.next_u64() as usize) % s.len().max(1)]
    }

    pub fn int_in(&mut self, lo: i32, hi_inc: i32) -> i32 {
        if hi_inc <= lo {
            return lo;
        }
        let span = (hi_inc - lo + 1) as u64;
        lo + (self.next_u64() % span) as i32
    }
}

pub struct CatProfile {
    pub name: &'static str,
    pub weight: f64,
    pub entries_min: u32,
    pub entries_max: u32,
    pub descriptions: &'static [&'static str],
}

pub struct SectorProfile {
    pub id: &'static str,
    pub display: &'static str,
    pub monthly_income_tl: f64,
    pub monthly_expense_tl: f64,
    pub income_cats: &'static [CatProfile],
    pub expense_cats: &'static [CatProfile],
    pub seasonality: [f64; 12],
}

pub fn find_profile(id: &str) -> Option<&'static SectorProfile> {
    ALL_PROFILES.iter().find(|p| p.id == id)
}

pub fn all_ids() -> Vec<&'static str> {
    ALL_PROFILES.iter().map(|p| p.id).collect()
}

pub fn generate(
    profile: &SectorProfile,
    pattern: Pattern,
    months: usize,
    start_date: NaiveDate,
    seed: u64,
) -> String {
    let mut rng = Rng::new(seed);
    let months = months.clamp(1, 36);

    let mut rows: Vec<(NaiveDate, String, f64, f64, &'static str)> =
        Vec::with_capacity(months * 24);

    for m_idx in 0..months {
        // Takvim ayı bazlı tarih artışı
        let current_first = start_date
            .checked_add_months(Months::new(m_idx as u32))
            .unwrap_or(start_date);
        let month_of_year = current_first.month0() as usize;

        // Sezonsallık limitleri (alt sınır koruması)
        let seasonality_raw = profile.seasonality[month_of_year];
        let seasonality_mult = match pattern {
            Pattern::Seasonal => (1.0 + (seasonality_raw - 1.0) * 1.5).max(0.25),
            _ => seasonality_raw.max(0.35),
        };

        let (inc_mult, exp_mult) = pattern_mults(pattern, m_idx, months);
        let last_day = days_in_month(current_first) as i32;

        emit_category_rows(
            &mut rng,
            &mut rows,
            profile.income_cats,
            profile.monthly_income_tl * seasonality_mult * inc_mult,
            true,
            current_first.year(),
            current_first.month(),
            last_day,
            0.20,
        );

        emit_category_rows(
            &mut rng,
            &mut rows,
            profile.expense_cats,
            profile.monthly_expense_tl * seasonality_mult * exp_mult,
            false,
            current_first.year(),
            current_first.month(),
            last_day,
            0.15,
        );
    }

    rows.sort_by_key(|r| r.0);

    let mut out = String::with_capacity(rows.len() * 80);
    out.push_str("tarih,aciklama,gelir,gider,kategori\n");
    for (date, desc, gelir, gider, cat) in rows {
        // CSV escape
        let safe_desc = if desc.contains(',') || desc.contains('"') {
            format!("\"{}\"", desc.replace('"', "\"\""))
        } else {
            desc
        };
        out.push_str(&format!(
            "{},{},{:.0},{:.0},{}\n",
            date.format("%Y-%m-%d"),
            safe_desc,
            gelir,
            gider,
            cat,
        ));
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn emit_category_rows(
    rng: &mut Rng,
    rows: &mut Vec<(NaiveDate, String, f64, f64, &'static str)>,
    cats: &'static [CatProfile],
    total_budget: f64,
    is_income: bool,
    year: i32,
    month: u32,
    last_day: i32,
    jitter_pct: f64,
) {
    for cat in cats {
        let cat_budget = total_budget * cat.weight;
        let n = rng.int_in(cat.entries_min as i32, cat.entries_max as i32) as usize;
        if n == 0 || cat.descriptions.is_empty() {
            continue;
        }
        let per_entry = cat_budget / n as f64;

        for _ in 0..n {
            let amount = rng.jitter(per_entry, jitter_pct).max(0.0).round();
            if amount < 1.0 {
                continue;
            }
            let day = rng.int_in(1, last_day) as u32;
            let date = NaiveDate::from_ymd_opt(year, month, day)
                .unwrap_or_else(|| NaiveDate::from_ymd_opt(year, month, 1).unwrap_or_default());
            let desc = rng.pick(cat.descriptions);
            if is_income {
                rows.push((date, desc.to_string(), amount, 0.0, cat.name));
            } else {
                rows.push((date, desc.to_string(), 0.0, amount, cat.name));
            }
        }
    }
}

fn pattern_mults(p: Pattern, m: usize, total: usize) -> (f64, f64) {
    let t = m as f64;
    let n = total as f64;
    match p {
        Pattern::Stable => (1.0, 1.0),
        Pattern::Growth => (1.0 + 0.035 * t, 1.0 + 0.022 * t),
        Pattern::Crisis => {
            let break_at = (n * 0.4) as usize;
            if m >= break_at {
                let since = (m - break_at) as f64;
                // Sabit giderlerin kriz anında korunması
                (0.55 + 0.025 * since, 1.05 + 0.008 * since)
            } else {
                (1.0, 1.0)
            }
        }
        Pattern::Seasonal => (1.0, 1.0),
        Pattern::Recovery => {
            let trough = (n * 0.4) as usize;
            if m <= trough {
                (1.0 - 0.07 * t, 1.0 + 0.015 * t)
            } else {
                let r = (m - trough) as f64;
                (0.45 + 0.10 * r, 1.05)
            }
        }
        Pattern::Mature => (2.4, 2.2),
    }
}

fn days_in_month(d: NaiveDate) -> u32 {
    let (y, m) = (d.year(), d.month());
    let next_first = if m == 12 {
        NaiveDate::from_ymd_opt(y + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(y, m + 1, 1)
    };
    next_first
        .and_then(|nf| nf.pred_opt())
        .map(|last| last.day())
        .unwrap_or(28)
}

// Sektör Profilleri (2025 TL Bazlı)

const ALL_PROFILES: &[SectorProfile] = &[
    RESTORAN,
    CAFE,
    OTOMOTIV_SERVIS,
    PERAKENDE,
    E_TICARET,
    IHRACAT,
    TEKNOLOJI_STARTUP,
    YAZILIM_AJANS,
    DANISMANLIK,
    MUHASEBE_BURO,
    IMALAT,
    INSAAT,
    EMLAK,
    TEKSTIL,
    TURIZM,
    EGITIM_KURSU,
    LOJISTIK,
    MEDIKAL,
    SAGLIK_KLINIK,
    GIDA_URETIM,
    KOBI,
    KUAFOR_GUZELLIK,
    ECZANE,
];

// Düşük marjlı sektör profili
const RESTORAN: SectorProfile = SectorProfile {
    id: "restoran",
    display: "Restoran",
    monthly_income_tl: 345_000.0,
    monthly_expense_tl: 320_000.0,
    seasonality: [
        0.85, 0.9, 1.05, 1.1, 1.2, 1.25, 1.3, 1.2, 1.05, 1.05, 1.1, 1.35,
    ],
    income_cats: &[
        CatProfile {
            name: "Satış",
            weight: 0.92,
            entries_min: 16,
            entries_max: 24,
            descriptions: &[
                "Öğle yemeği servisi",
                "Akşam servisi geliri",
                "Hafta sonu yoğunluk",
                "Catering siparişi",
                "İş yemeği rezervasyonu",
                "Doğum günü organizasyonu",
                "Online sipariş geliri",
                "Kahvaltı servisi",
                "Ramazan iftar geliri",
                "Yılbaşı menü geliri",
                "Bayram menüsü satışı",
            ],
        },
        CatProfile {
            name: "Diğer Gelir",
            weight: 0.08,
            entries_min: 1,
            entries_max: 3,
            descriptions: &[
                "Yemeksepeti komisyon iadesi",
                "Sponsorluk geliri",
                "Mekan kiralama geliri",
            ],
        },
    ],
    expense_cats: &[
        CatProfile {
            name: "Tedarik",
            weight: 0.38,
            entries_min: 5,
            entries_max: 9,
            descriptions: &[
                "Et ve tavuk alımı",
                "Sebze meyve toptan",
                "Süt ve mandıra ürünleri",
                "Kuru gıda toptan alım",
                "Deniz ürünleri tedarik",
                "İçecek toptan satın alma",
                "Baharat ve sos tedarik",
            ],
        },
        CatProfile {
            name: "Maaş",
            weight: 0.28,
            entries_min: 1,
            entries_max: 2,
            descriptions: &[
                "Şef ve mutfak personeli maaşları",
                "Garson ve servis maaşları",
                "Bulaşıkçı ve yardımcı personel",
            ],
        },
        CatProfile {
            name: "Kira",
            weight: 0.10,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["Aylık dükkân kirası", "İşyeri kira ödemesi"],
        },
        CatProfile {
            name: "Fatura",
            weight: 0.08,
            entries_min: 2,
            entries_max: 4,
            descriptions: &[
                "Elektrik faturası",
                "Doğal gaz faturası",
                "Su faturası",
                "İnternet aboneliği",
            ],
        },
        CatProfile {
            name: "Pazarlama",
            weight: 0.06,
            entries_min: 1,
            entries_max: 3,
            descriptions: &[
                "Instagram reklam harcaması",
                "Yemeksepeti komisyonu",
                "Getir komisyonu",
                "Google Maps öne çıkarma",
                "Influencer iş birliği",
            ],
        },
        CatProfile {
            name: "Vergi",
            weight: 0.05,
            entries_min: 1,
            entries_max: 1,
            descriptions: &[
                "KDV ödemesi",
                "Stopaj vergi ödemesi",
                "Geçici vergi taksiti",
            ],
        },
        CatProfile {
            name: "Sarf",
            weight: 0.05,
            entries_min: 1,
            entries_max: 3,
            descriptions: &[
                "Temizlik malzemeleri",
                "Servis ve mutfak ekipmanı",
                "Tek kullanımlık ambalaj",
            ],
        },
    ],
};

// İçecek odaklı düşük maliyet profili
const CAFE: SectorProfile = SectorProfile {
    id: "cafe",
    display: "Kafe",
    monthly_income_tl: 138_000.0,
    monthly_expense_tl: 118_000.0,
    seasonality: [
        0.9, 0.95, 1.05, 1.1, 1.15, 1.1, 1.05, 1.05, 1.1, 1.15, 1.1, 1.2,
    ],
    income_cats: &[
        CatProfile {
            name: "Satış",
            weight: 0.95,
            entries_min: 14,
            entries_max: 22,
            descriptions: &[
                "Kahve ve içecek satışı",
                "Tatlı ve pasta geliri",
                "Kahvaltı menüsü satışı",
                "Brunch hafta sonu geliri",
                "Sıcak içecek satışı",
                "Soğuk kahve serisi",
                "Mevsim limonata satışı",
            ],
        },
        CatProfile {
            name: "Diğer Gelir",
            weight: 0.05,
            entries_min: 0,
            entries_max: 2,
            descriptions: &["Mug ve aksesuar satışı", "Mekan etkinlik kiralama"],
        },
    ],
    expense_cats: &[
        CatProfile {
            name: "Tedarik",
            weight: 0.35,
            entries_min: 4,
            entries_max: 8,
            descriptions: &[
                "Kahve çekirdeği alımı",
                "Süt ve süt ürünleri tedarik",
                "Pastane ürünleri alımı",
                "Şurup ve sos toptan alım",
                "Tatlı malzemesi tedariği",
            ],
        },
        CatProfile {
            name: "Maaş",
            weight: 0.32,
            entries_min: 1,
            entries_max: 2,
            descriptions: &["Barista maaşları", "Servis personeli maaşı"],
        },
        CatProfile {
            name: "Kira",
            weight: 0.14,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["Aylık kafe kirası"],
        },
        CatProfile {
            name: "Fatura",
            weight: 0.08,
            entries_min: 2,
            entries_max: 3,
            descriptions: &["Elektrik faturası", "Su faturası", "İnternet aboneliği"],
        },
        CatProfile {
            name: "Pazarlama",
            weight: 0.05,
            entries_min: 0,
            entries_max: 2,
            descriptions: &["Instagram reklam", "Google Maps reklamı"],
        },
        CatProfile {
            name: "Vergi",
            weight: 0.04,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["KDV ödemesi", "Stopaj vergisi"],
        },
        CatProfile {
            name: "Sarf",
            weight: 0.02,
            entries_min: 1,
            entries_max: 2,
            descriptions: &["Karton bardak ve ambalaj", "Temizlik malzemeleri"],
        },
    ],
};

const OTOMOTIV_SERVIS: SectorProfile = SectorProfile {
    id: "otomotiv_servis",
    display: "Otomotiv Servis",
    monthly_income_tl: 280_000.0,
    monthly_expense_tl: 235_000.0,
    seasonality: [
        1.05, 1.0, 1.05, 1.25, 1.3, 1.0, 0.85, 0.9, 1.05, 1.3, 1.25, 1.0,
    ],
    income_cats: &[
        CatProfile {
            name: "Satış",
            weight: 0.95,
            entries_min: 10,
            entries_max: 18,
            descriptions: &[
                "Periyodik bakım geliri",
                "Motor revizyonu",
                "Fren sistemi onarımı",
                "Lastik değişim ve balans",
                "Akü değişim",
                "Egzoz ve emisyon kontrol",
                "Boya ve kaporta işleri",
                "Klima bakımı",
                "Şanzıman onarımı",
                "Elektrik ve elektronik arıza",
            ],
        },
        CatProfile {
            name: "Diğer Gelir",
            weight: 0.05,
            entries_min: 0,
            entries_max: 2,
            descriptions: &["İkinci el parça satışı", "Yağ ve sıvı satışı"],
        },
    ],
    expense_cats: &[
        CatProfile {
            name: "Tedarik",
            weight: 0.38,
            entries_min: 3,
            entries_max: 7,
            descriptions: &[
                "Yedek parça toptan alımı",
                "Motor yağı ve filtre stoğu",
                "Lastik tedariği",
                "Akü tedariği",
                "Boya ve kimyasal alımı",
            ],
        },
        CatProfile {
            name: "Maaş",
            weight: 0.30,
            entries_min: 1,
            entries_max: 2,
            descriptions: &["Usta ve teknisyen maaşları", "Yardımcı personel maaşı"],
        },
        CatProfile {
            name: "Kira",
            weight: 0.10,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["Servis atölye kirası"],
        },
        CatProfile {
            name: "Fatura",
            weight: 0.06,
            entries_min: 2,
            entries_max: 3,
            descriptions: &["Elektrik faturası", "Su faturası", "İnternet aboneliği"],
        },
        CatProfile {
            name: "Üretim",
            weight: 0.07,
            entries_min: 1,
            entries_max: 3,
            descriptions: &[
                "Kompresör bakımı",
                "Lift ve ekipman bakım",
                "Atölye sarf malzeme",
            ],
        },
        CatProfile {
            name: "Sigorta",
            weight: 0.04,
            entries_min: 0,
            entries_max: 1,
            descriptions: &["İşyeri sigorta poliçesi", "Sorumluluk sigortası"],
        },
        CatProfile {
            name: "Vergi",
            weight: 0.05,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["KDV ödemesi", "Gelir vergisi taksiti"],
        },
    ],
};

const PERAKENDE: SectorProfile = SectorProfile {
    id: "perakende",
    display: "Perakende Mağaza",
    monthly_income_tl: 420_000.0,
    monthly_expense_tl: 340_000.0,
    seasonality: [
        0.85, 0.9, 1.0, 1.0, 1.05, 1.0, 0.95, 1.0, 1.1, 1.15, 1.45, 1.5,
    ],
    income_cats: &[
        CatProfile {
            name: "Satış",
            weight: 0.97,
            entries_min: 15,
            entries_max: 25,
            descriptions: &[
                "Mağaza günlük satış",
                "POS ile kart satışı",
                "Nakit tahsilat",
                "Hafta sonu satış geliri",
                "Sezonluk kampanya satışı",
                "İndirim dönemi geliri",
                "Yılbaşı satış yoğunluğu",
                "Bayram öncesi satış",
            ],
        },
        CatProfile {
            name: "Diğer Gelir",
            weight: 0.03,
            entries_min: 0,
            entries_max: 2,
            descriptions: &["Vitrin reklam geliri", "Tedarikçi ciro primi"],
        },
    ],
    expense_cats: &[
        CatProfile {
            name: "Tedarik",
            weight: 0.55,
            entries_min: 4,
            entries_max: 8,
            descriptions: &[
                "Toptan stok alımı",
                "Sezonluk koleksiyon alımı",
                "Tedarikçi A ödemesi",
                "İthalat sipariş ödemesi",
                "Marka distribütör ödemesi",
            ],
        },
        CatProfile {
            name: "Maaş",
            weight: 0.20,
            entries_min: 1,
            entries_max: 2,
            descriptions: &["Mağaza personeli maaşları", "Mağaza müdürü maaşı"],
        },
        CatProfile {
            name: "Kira",
            weight: 0.10,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["AVM mağaza kirası", "Cadde dükkan kirası"],
        },
        CatProfile {
            name: "Fatura",
            weight: 0.04,
            entries_min: 2,
            entries_max: 3,
            descriptions: &[
                "Elektrik faturası",
                "AVM aidat ödemesi",
                "İnternet aboneliği",
            ],
        },
        CatProfile {
            name: "Pazarlama",
            weight: 0.05,
            entries_min: 1,
            entries_max: 3,
            descriptions: &[
                "İndirim kampanya tasarımı",
                "Sosyal medya reklamı",
                "Vitrin düzenleme",
            ],
        },
        CatProfile {
            name: "Vergi",
            weight: 0.04,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["KDV ödemesi", "Geçici vergi taksiti"],
        },
        CatProfile {
            name: "Sarf",
            weight: 0.02,
            entries_min: 1,
            entries_max: 2,
            descriptions: &["Poşet ve ambalaj", "Temizlik malzemeleri"],
        },
    ],
};

const E_TICARET: SectorProfile = SectorProfile {
    id: "e_ticaret",
    display: "E-Ticaret",
    monthly_income_tl: 510_000.0,
    monthly_expense_tl: 420_000.0,
    seasonality: [
        0.85, 0.9, 1.0, 1.0, 1.05, 1.0, 0.95, 1.0, 1.1, 1.2, 1.75, 1.55,
    ],
    income_cats: &[
        CatProfile {
            name: "Satış",
            weight: 0.96,
            entries_min: 18,
            entries_max: 28,
            descriptions: &[
                "Trendyol satış geliri",
                "Hepsiburada sipariş",
                "Amazon satış",
                "Kendi site ödemeleri",
                "Instagram shopping satış",
                "WhatsApp sipariş tahsilat",
                "Black Friday satış patlaması",
                "Yılbaşı kampanya satışı",
            ],
        },
        CatProfile {
            name: "Diğer Gelir",
            weight: 0.04,
            entries_min: 0,
            entries_max: 2,
            descriptions: &["Pazaryeri iade düzeltmesi", "Affiliate komisyon geliri"],
        },
    ],
    expense_cats: &[
        CatProfile {
            name: "Tedarik",
            weight: 0.45,
            entries_min: 4,
            entries_max: 8,
            descriptions: &[
                "Stok alımı toptancı",
                "İthalat sipariş ödemesi",
                "Yerli üretici sipariş",
                "Tedarikçi avans ödemesi",
            ],
        },
        CatProfile {
            name: "Lojistik",
            weight: 0.13,
            entries_min: 3,
            entries_max: 6,
            descriptions: &[
                "Kargo ödeme Aras",
                "Yurtiçi kargo faturası",
                "MNG kargo ödemesi",
                "Depo lojistik ücreti",
                "Paketleme malzemeleri",
            ],
        },
        CatProfile {
            name: "Pazarlama",
            weight: 0.13,
            entries_min: 3,
            entries_max: 6,
            descriptions: &[
                "Google Ads kampanyası",
                "Meta reklam harcaması",
                "TikTok ads ödeme",
                "Influencer iş birliği",
                "SEO ajans hizmet bedeli",
            ],
        },
        CatProfile {
            name: "Maaş",
            weight: 0.13,
            entries_min: 1,
            entries_max: 2,
            descriptions: &[
                "Depo personeli maaşları",
                "Müşteri hizmetleri maaşı",
                "Pazarlama uzmanı maaşı",
            ],
        },
        CatProfile {
            name: "Teknoloji",
            weight: 0.06,
            entries_min: 1,
            entries_max: 3,
            descriptions: &[
                "Shopify abonelik",
                "ERP yazılım ücreti",
                "AWS bulut faturası",
                "E-fatura entegrasyon ücreti",
            ],
        },
        CatProfile {
            name: "Kira",
            weight: 0.05,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["Depo kira ödemesi", "Ofis kirası"],
        },
        CatProfile {
            name: "Vergi",
            weight: 0.05,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["KDV ödemesi", "Stopaj vergisi", "Geçici vergi"],
        },
    ],
};

// İhracat
const IHRACAT: SectorProfile = SectorProfile {
    id: "ihracat",
    display: "İhracat Firması",
    monthly_income_tl: 1_250_000.0,
    monthly_expense_tl: 980_000.0,
    seasonality: [
        0.95, 1.0, 1.15, 1.15, 1.1, 1.0, 0.9, 0.85, 1.1, 1.2, 1.15, 1.0,
    ],
    income_cats: &[
        CatProfile {
            name: "Satış",
            weight: 0.95,
            entries_min: 4,
            entries_max: 9,
            descriptions: &[
                "Almanya ihracat tahsilatı (EUR)",
                "ABD müşteri ödemesi (USD)",
                "Hollanda konteyner ödemesi",
                "Orta Doğu ihracat geliri",
                "Avrupa toptan satış tahsilatı",
                "Avustralya sipariş ödemesi",
                "İngiltere ihracat (GBP) tahsilatı",
            ],
        },
        CatProfile {
            name: "Devlet Desteği",
            weight: 0.05,
            entries_min: 0,
            entries_max: 1,
            descriptions: &["Pazara giriş desteği geri ödemesi", "İhracat hibe ödemesi"],
        },
    ],
    expense_cats: &[
        CatProfile {
            name: "Tedarik",
            weight: 0.48,
            entries_min: 3,
            entries_max: 6,
            descriptions: &[
                "Yerli üretici tedarik ödemesi",
                "Hammadde alımı",
                "Yarı mamul tedariği",
            ],
        },
        CatProfile {
            name: "Lojistik",
            weight: 0.18,
            entries_min: 2,
            entries_max: 5,
            descriptions: &[
                "Konteyner navlun ödemesi",
                "Gümrük müşavir ücreti",
                "Liman elleçleme",
                "Uluslararası nakliye sigortası",
            ],
        },
        CatProfile {
            name: "Maaş",
            weight: 0.13,
            entries_min: 1,
            entries_max: 2,
            descriptions: &["İhracat departmanı maaşları", "Operasyon ekibi maaşları"],
        },
        CatProfile {
            name: "Vergi",
            weight: 0.05,
            entries_min: 1,
            entries_max: 2,
            descriptions: &["KDV iade öncesi ödeme", "Gümrük vergisi"],
        },
        CatProfile {
            name: "Pazarlama",
            weight: 0.06,
            entries_min: 0,
            entries_max: 2,
            descriptions: &[
                "Uluslararası fuar katılımı",
                "B2B platform üyelik",
                "Yurt dışı tanıtım",
            ],
        },
        CatProfile {
            name: "Kira",
            weight: 0.04,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["Ofis kirası", "Depo kira"],
        },
        CatProfile {
            name: "Danışmanlık",
            weight: 0.06,
            entries_min: 0,
            entries_max: 2,
            descriptions: &["İhracat danışmanlık hizmeti", "Mali müşavir ücreti"],
        },
    ],
};

// Teknoloji Startup
// Yüksek nakit yakımı (Burn-rate odaklı profil)
const TEKNOLOJI_STARTUP: SectorProfile = SectorProfile {
    id: "teknoloji_startup",
    display: "Teknoloji Startup",
    monthly_income_tl: 145_000.0,
    monthly_expense_tl: 285_000.0,
    seasonality: [
        1.0, 1.0, 1.05, 1.05, 1.05, 1.0, 0.95, 0.95, 1.05, 1.1, 1.1, 1.05,
    ],
    income_cats: &[
        CatProfile {
            name: "Satış",
            weight: 0.55,
            entries_min: 4,
            entries_max: 9,
            descriptions: &[
                "SaaS aylık abonelik (Pro paket)",
                "Kurumsal müşteri ödemesi",
                "Yıllık abonelik tahsilatı",
                "Enterprise lisans satışı",
                "API kullanım faturası",
            ],
        },
        CatProfile {
            name: "Yatırım",
            weight: 0.40,
            entries_min: 0,
            entries_max: 1,
            descriptions: &[
                "Seed round melek yatırımcı transferi",
                "Pre-seed sermaye girişi",
                "Convertible note ödemesi",
            ],
        },
        CatProfile {
            name: "Devlet Desteği",
            weight: 0.05,
            entries_min: 0,
            entries_max: 1,
            descriptions: &["TÜBİTAK 1507 hibe taksiti", "KOSGEB Ar-Ge hibe"],
        },
    ],
    expense_cats: &[
        CatProfile {
            name: "Maaş",
            weight: 0.55,
            entries_min: 1,
            entries_max: 2,
            descriptions: &[
                "Yazılım geliştirici maaşları (x5)",
                "Ürün ve tasarım ekibi",
                "Founder maaşları",
                "Pazarlama uzmanı maaşı",
            ],
        },
        CatProfile {
            name: "Teknoloji",
            weight: 0.12,
            entries_min: 2,
            entries_max: 4,
            descriptions: &[
                "AWS bulut altyapısı",
                "OpenAI API kullanımı",
                "Vercel + GitHub abonelik",
                "Datadog monitoring",
                "Mixpanel analytics",
            ],
        },
        CatProfile {
            name: "Kira",
            weight: 0.08,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["Coworking ofis kirası", "Levent ofis kirası"],
        },
        CatProfile {
            name: "Pazarlama",
            weight: 0.10,
            entries_min: 1,
            entries_max: 3,
            descriptions: &[
                "LinkedIn Ads",
                "Google Ads kampanya",
                "Content marketing ajans",
                "PR ajans aylık ödeme",
            ],
        },
        CatProfile {
            name: "Danışmanlık",
            weight: 0.06,
            entries_min: 0,
            entries_max: 2,
            descriptions: &[
                "Hukuk danışmanlık",
                "Mali müşavir ücreti",
                "Stratejik danışmanlık",
            ],
        },
        CatProfile {
            name: "Vergi",
            weight: 0.04,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["Stopaj vergi ödemesi", "Geçici vergi taksiti"],
        },
        CatProfile {
            name: "Fatura",
            weight: 0.05,
            entries_min: 1,
            entries_max: 2,
            descriptions: &["Ofis elektrik", "İnternet aboneliği", "Telefon hatları"],
        },
    ],
};

// Yazılım Ajans
const YAZILIM_AJANS: SectorProfile = SectorProfile {
    id: "yazilim_ajans",
    display: "Yazılım Ajansı",
    monthly_income_tl: 320_000.0,
    monthly_expense_tl: 265_000.0,
    seasonality: [
        1.0, 1.05, 1.1, 1.05, 1.0, 0.95, 0.85, 0.85, 1.05, 1.15, 1.1, 1.0,
    ],
    income_cats: &[
        CatProfile {
            name: "Satış",
            weight: 0.92,
            entries_min: 3,
            entries_max: 7,
            descriptions: &[
                "Müşteri A — proje milestone ödemesi",
                "Web sitesi geliştirme tahsilatı",
                "Mobil uygulama proje ödemesi",
                "Bakım sözleşmesi aylık tahsilat",
                "E-ticaret entegrasyon projesi",
                "Kurumsal yazılım geliştirme",
            ],
        },
        CatProfile {
            name: "Diğer Gelir",
            weight: 0.08,
            entries_min: 0,
            entries_max: 2,
            descriptions: &["Reseller komisyon geliri", "Eğitim/workshop geliri"],
        },
    ],
    expense_cats: &[
        CatProfile {
            name: "Maaş",
            weight: 0.62,
            entries_min: 1,
            entries_max: 2,
            descriptions: &[
                "Geliştirici ekibi maaşları",
                "Tasarımcı ve PM maaşları",
                "Stajyer ödemesi",
            ],
        },
        CatProfile {
            name: "Teknoloji",
            weight: 0.10,
            entries_min: 2,
            entries_max: 4,
            descriptions: &[
                "Yazılım lisans ödemeleri",
                "GitHub Enterprise",
                "Figma takım üyeliği",
                "Hosting ve domain",
            ],
        },
        CatProfile {
            name: "Kira",
            weight: 0.10,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["Ofis kirası"],
        },
        CatProfile {
            name: "Pazarlama",
            weight: 0.06,
            entries_min: 0,
            entries_max: 2,
            descriptions: &["LinkedIn premium", "Web reklamı"],
        },
        CatProfile {
            name: "Vergi",
            weight: 0.06,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["KDV ödemesi", "Stopaj vergi"],
        },
        CatProfile {
            name: "Danışmanlık",
            weight: 0.04,
            entries_min: 0,
            entries_max: 1,
            descriptions: &["Mali müşavir ücreti", "Hukuk danışmanlık"],
        },
        CatProfile {
            name: "Fatura",
            weight: 0.02,
            entries_min: 1,
            entries_max: 2,
            descriptions: &["Elektrik ve internet", "Telefon faturası"],
        },
    ],
};

// Danışmanlık
// Yüksek marjlı hizmet profili
const DANISMANLIK: SectorProfile = SectorProfile {
    id: "danismanlik",
    display: "Danışmanlık",
    monthly_income_tl: 305_000.0,
    monthly_expense_tl: 215_000.0,
    seasonality: [
        0.9, 0.95, 1.1, 1.1, 1.05, 1.0, 0.85, 0.85, 1.1, 1.15, 1.1, 1.2,
    ],
    income_cats: &[
        CatProfile {
            name: "Satış",
            weight: 0.94,
            entries_min: 3,
            entries_max: 7,
            descriptions: &[
                "Yönetim danışmanlığı tahsilatı",
                "Süreç iyileştirme proje ödemesi",
                "ISO sertifikasyon danışmanlığı",
                "Eğitim ve workshop geliri",
                "Stratejik plan danışmanlığı",
                "Audit ve due diligence ücreti",
            ],
        },
        CatProfile {
            name: "Diğer Gelir",
            weight: 0.06,
            entries_min: 0,
            entries_max: 2,
            descriptions: &["Konuşmacı ücreti", "Kitap telif gelirleri"],
        },
    ],
    expense_cats: &[
        CatProfile {
            name: "Maaş",
            weight: 0.58,
            entries_min: 1,
            entries_max: 2,
            descriptions: &[
                "Kıdemli danışman maaşları",
                "Junior danışman maaşı",
                "Asistan maaşı",
            ],
        },
        CatProfile {
            name: "Kira",
            weight: 0.12,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["Ofis kirası", "Levent ofis kira"],
        },
        CatProfile {
            name: "Lojistik",
            weight: 0.07,
            entries_min: 1,
            entries_max: 4,
            descriptions: &[
                "Şehir dışı müşteri ziyaret",
                "Uçak bileti",
                "Konaklama gideri",
            ],
        },
        CatProfile {
            name: "Teknoloji",
            weight: 0.06,
            entries_min: 1,
            entries_max: 3,
            descriptions: &["CRM yazılım abonelik", "Office 365", "Veri analiz araçları"],
        },
        CatProfile {
            name: "Pazarlama",
            weight: 0.05,
            entries_min: 0,
            entries_max: 2,
            descriptions: &[
                "LinkedIn Ads",
                "Web site bakımı",
                "Konferans katılım ücreti",
            ],
        },
        CatProfile {
            name: "Vergi",
            weight: 0.08,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["KDV ödemesi", "Geçici vergi"],
        },
        CatProfile {
            name: "Fatura",
            weight: 0.04,
            entries_min: 1,
            entries_max: 2,
            descriptions: &["Elektrik", "Telefon ve internet"],
        },
    ],
};

// Muhasebe Bürosu
const MUHASEBE_BURO: SectorProfile = SectorProfile {
    id: "muhasebe_buro",
    display: "Muhasebe Bürosu",
    monthly_income_tl: 175_000.0,
    monthly_expense_tl: 130_000.0,
    seasonality: [
        1.05, 1.1, 1.55, 1.45, 1.05, 0.95, 0.9, 0.95, 1.2, 1.05, 1.05, 1.1,
    ],
    income_cats: &[
        CatProfile {
            name: "Satış",
            weight: 0.96,
            entries_min: 8,
            entries_max: 16,
            descriptions: &[
                "KOBİ aylık muhasebe ücreti",
                "Yıllık beyanname hazırlık ücreti",
                "Vergi denetim danışmanlık",
                "Bordro hizmet bedeli",
                "Şirket kuruluş danışmanlığı",
                "E-fatura entegrasyon ücreti",
            ],
        },
        CatProfile {
            name: "Diğer Gelir",
            weight: 0.04,
            entries_min: 0,
            entries_max: 2,
            descriptions: &["Eğitim semineri geliri", "Bilirkişilik ücreti"],
        },
    ],
    expense_cats: &[
        CatProfile {
            name: "Maaş",
            weight: 0.62,
            entries_min: 1,
            entries_max: 2,
            descriptions: &[
                "Mali müşavir maaşı",
                "Muhasebe personeli maaşları",
                "Stajyer ücreti",
            ],
        },
        CatProfile {
            name: "Kira",
            weight: 0.13,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["Ofis kirası"],
        },
        CatProfile {
            name: "Teknoloji",
            weight: 0.08,
            entries_min: 1,
            entries_max: 3,
            descriptions: &[
                "Logo muhasebe yazılım abonelik",
                "Mikro yazılım lisans",
                "E-fatura portalı ücreti",
            ],
        },
        CatProfile {
            name: "Vergi",
            weight: 0.06,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["KDV ödemesi", "Stopaj"],
        },
        CatProfile {
            name: "Fatura",
            weight: 0.05,
            entries_min: 1,
            entries_max: 3,
            descriptions: &["Elektrik", "İnternet", "Telefon"],
        },
        CatProfile {
            name: "Sarf",
            weight: 0.04,
            entries_min: 1,
            entries_max: 2,
            descriptions: &["Kırtasiye ve toner", "Ofis sarf malzemesi"],
        },
        CatProfile {
            name: "Danışmanlık",
            weight: 0.02,
            entries_min: 0,
            entries_max: 1,
            descriptions: &["TÜRMOB üyelik aidatı", "Meslek eğitim ücreti"],
        },
    ],
};

// İmalat
const IMALAT: SectorProfile = SectorProfile {
    id: "imalat",
    display: "İmalat",
    monthly_income_tl: 920_000.0,
    monthly_expense_tl: 780_000.0,
    seasonality: [
        0.95, 1.0, 1.05, 1.1, 1.1, 1.0, 0.85, 0.85, 1.1, 1.15, 1.1, 1.0,
    ],
    income_cats: &[
        CatProfile {
            name: "Satış",
            weight: 0.96,
            entries_min: 5,
            entries_max: 10,
            descriptions: &[
                "Toptan müşteri sevkiyat ödemesi",
                "Bayi sipariş tahsilatı",
                "Kurumsal sipariş ödemesi",
                "Yurt içi distribütör tahsilatı",
                "Sözleşmeli üretim geliri",
            ],
        },
        CatProfile {
            name: "Diğer Gelir",
            weight: 0.04,
            entries_min: 0,
            entries_max: 2,
            descriptions: &["Hurda satışı", "Devlet hibe ödemesi"],
        },
    ],
    expense_cats: &[
        CatProfile {
            name: "Tedarik",
            weight: 0.42,
            entries_min: 4,
            entries_max: 8,
            descriptions: &[
                "Hammadde alımı (çelik)",
                "Kimyasal tedarik",
                "Plastik granül alımı",
                "Yarı mamul tedarik",
                "İthal makine parça",
            ],
        },
        CatProfile {
            name: "Maaş",
            weight: 0.28,
            entries_min: 1,
            entries_max: 2,
            descriptions: &[
                "Üretim işçileri maaşları",
                "Mühendis maaşları",
                "Ofis personeli maaşı",
            ],
        },
        CatProfile {
            name: "Üretim",
            weight: 0.12,
            entries_min: 2,
            entries_max: 5,
            descriptions: &[
                "Makine bakım ve revizyon",
                "Kalıp ve aparat alımı",
                "Üretim sarf malzeme",
                "Kalite kontrol cihaz",
            ],
        },
        CatProfile {
            name: "Kira",
            weight: 0.05,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["Fabrika kira ödemesi", "Depo kira"],
        },
        CatProfile {
            name: "Fatura",
            weight: 0.06,
            entries_min: 2,
            entries_max: 4,
            descriptions: &[
                "Elektrik faturası (yüksek tüketim)",
                "Doğal gaz",
                "Su faturası",
            ],
        },
        CatProfile {
            name: "Lojistik",
            weight: 0.04,
            entries_min: 1,
            entries_max: 3,
            descriptions: &["Nakliye ücreti", "Yurt içi taşıma", "Forklift yakıt"],
        },
        CatProfile {
            name: "Vergi",
            weight: 0.03,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["KDV ödemesi", "ÖTV ödemesi"],
        },
    ],
};

// İnşaat
const INSAAT: SectorProfile = SectorProfile {
    id: "insaat",
    display: "İnşaat",
    monthly_income_tl: 1_650_000.0,
    monthly_expense_tl: 1_380_000.0,
    seasonality: [
        0.65, 0.7, 0.9, 1.1, 1.3, 1.35, 1.4, 1.35, 1.25, 1.15, 0.85, 0.65,
    ],
    income_cats: &[
        CatProfile {
            name: "Satış",
            weight: 0.95,
            entries_min: 2,
            entries_max: 5,
            descriptions: &[
                "Hakediş tahsilatı (Şantiye A)",
                "Daire satış tahsilatı",
                "Müteahhit hakediş ödemesi",
                "Proje milestone ödemesi",
                "Kamu ihale hakedişi",
            ],
        },
        CatProfile {
            name: "Diğer Gelir",
            weight: 0.05,
            entries_min: 0,
            entries_max: 1,
            descriptions: &["Hurda demir satışı", "Ekipman kiralama geliri"],
        },
    ],
    expense_cats: &[
        CatProfile {
            name: "Tedarik",
            weight: 0.45,
            entries_min: 4,
            entries_max: 9,
            descriptions: &[
                "Çimento ve demir alımı",
                "Hazır beton sipariş",
                "İnşaat malzemeleri toptan",
                "Elektrik tesisat malzeme",
                "Sıhhi tesisat tedarik",
            ],
        },
        CatProfile {
            name: "Maaş",
            weight: 0.27,
            entries_min: 1,
            entries_max: 2,
            descriptions: &[
                "İşçi yevmiyeleri",
                "Ustabaşı ve teknik kadro maaşları",
                "Şantiye şefi maaşı",
            ],
        },
        CatProfile {
            name: "Üretim",
            weight: 0.12,
            entries_min: 2,
            entries_max: 4,
            descriptions: &[
                "İş makinesi kirası",
                "Kalıp ve iskele kiralama",
                "Vinç kira",
                "Yakıt ve mazot ödemesi",
            ],
        },
        CatProfile {
            name: "Lojistik",
            weight: 0.06,
            entries_min: 1,
            entries_max: 3,
            descriptions: &["Nakliye ve hafriyat", "Kamyon taşıma"],
        },
        CatProfile {
            name: "Sigorta",
            weight: 0.04,
            entries_min: 0,
            entries_max: 1,
            descriptions: &["All-risk inşaat sigortası", "İşçi sigorta ödemesi"],
        },
        CatProfile {
            name: "Vergi",
            weight: 0.04,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["KDV ödemesi", "Stopaj"],
        },
        CatProfile {
            name: "Danışmanlık",
            weight: 0.02,
            entries_min: 0,
            entries_max: 1,
            descriptions: &["Mimar ücret ödemesi", "Statik mühendis ücreti"],
        },
    ],
};

// Emlak
const EMLAK: SectorProfile = SectorProfile {
    id: "emlak",
    display: "Emlak Ofisi",
    monthly_income_tl: 240_000.0,
    monthly_expense_tl: 175_000.0,
    seasonality: [
        0.85, 0.95, 1.15, 1.35, 1.4, 1.2, 1.0, 1.0, 1.15, 1.15, 1.0, 0.85,
    ],
    income_cats: &[
        CatProfile {
            name: "Satış",
            weight: 0.93,
            entries_min: 2,
            entries_max: 6,
            descriptions: &[
                "Satış komisyon tahsilatı",
                "Kiralık komisyon",
                "Aylık portföy yönetim",
                "Yatırımcıya konut tahsis komisyonu",
                "Ticari gayrimenkul komisyonu",
            ],
        },
        CatProfile {
            name: "Diğer Gelir",
            weight: 0.07,
            entries_min: 0,
            entries_max: 2,
            descriptions: &["Değerleme raporu ücreti", "Tapu işlem danışmanlık"],
        },
    ],
    expense_cats: &[
        CatProfile {
            name: "Maaş",
            weight: 0.40,
            entries_min: 1,
            entries_max: 2,
            descriptions: &["Emlak danışmanı sabit maaşı", "Asistan maaşı"],
        },
        CatProfile {
            name: "Pazarlama",
            weight: 0.25,
            entries_min: 2,
            entries_max: 5,
            descriptions: &[
                "Sahibinden.com ilan paketi",
                "Hepsiemlak ilan ücreti",
                "Google Ads kampanya",
                "Instagram reklam",
            ],
        },
        CatProfile {
            name: "Kira",
            weight: 0.18,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["Ofis kirası (cadde dükkan)"],
        },
        CatProfile {
            name: "Fatura",
            weight: 0.05,
            entries_min: 1,
            entries_max: 3,
            descriptions: &["Elektrik", "Telefon", "İnternet"],
        },
        CatProfile {
            name: "Vergi",
            weight: 0.06,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["KDV ödemesi", "Stopaj"],
        },
        CatProfile {
            name: "Lojistik",
            weight: 0.04,
            entries_min: 1,
            entries_max: 3,
            descriptions: &["Araç yakıt", "Müşteri ziyaret giderleri"],
        },
        CatProfile {
            name: "Teknoloji",
            weight: 0.02,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["CRM yazılım abonelik"],
        },
    ],
};

// Tekstil
const TEKSTIL: SectorProfile = SectorProfile {
    id: "tekstil",
    display: "Tekstil",
    monthly_income_tl: 680_000.0,
    monthly_expense_tl: 570_000.0,
    seasonality: [
        0.85, 0.9, 1.0, 1.15, 1.25, 1.05, 0.85, 0.95, 1.25, 1.2, 1.05, 0.95,
    ],
    income_cats: &[
        CatProfile {
            name: "Satış",
            weight: 0.96,
            entries_min: 4,
            entries_max: 9,
            descriptions: &[
                "Toptan yazlık koleksiyon satış",
                "Kışlık sezon sipariş tahsilatı",
                "Mağaza zinciri toptan",
                "Markaya fason üretim ödemesi",
                "İhracat sipariş tahsilatı",
            ],
        },
        CatProfile {
            name: "Diğer Gelir",
            weight: 0.04,
            entries_min: 0,
            entries_max: 2,
            descriptions: &["Kumaş hurda satışı"],
        },
    ],
    expense_cats: &[
        CatProfile {
            name: "Tedarik",
            weight: 0.40,
            entries_min: 3,
            entries_max: 7,
            descriptions: &[
                "Kumaş toptan alımı",
                "Aksesuar (fermuar, düğme) tedarik",
                "Boyalı kumaş alımı",
                "İplik ve dokuma malzeme",
            ],
        },
        CatProfile {
            name: "Maaş",
            weight: 0.30,
            entries_min: 1,
            entries_max: 2,
            descriptions: &[
                "Atölye işçi maaşları",
                "Modelist ve tasarımcı maaşı",
                "Kalite kontrol maaşı",
            ],
        },
        CatProfile {
            name: "Üretim",
            weight: 0.12,
            entries_min: 2,
            entries_max: 5,
            descriptions: &[
                "Dikim makinesi bakım",
                "İğne ve sarf malzeme",
                "Ütü ve presleme",
                "Etiket ve paketleme",
            ],
        },
        CatProfile {
            name: "Kira",
            weight: 0.06,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["Atölye kira", "Showroom kirası"],
        },
        CatProfile {
            name: "Fatura",
            weight: 0.05,
            entries_min: 2,
            entries_max: 3,
            descriptions: &["Elektrik", "Su", "Doğal gaz"],
        },
        CatProfile {
            name: "Lojistik",
            weight: 0.04,
            entries_min: 1,
            entries_max: 3,
            descriptions: &["Nakliye ödemesi", "Kargo gönderim"],
        },
        CatProfile {
            name: "Vergi",
            weight: 0.03,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["KDV", "Stopaj"],
        },
    ],
};

// Turizm
const TURIZM: SectorProfile = SectorProfile {
    id: "turizm",
    display: "Turizm/Otel",
    monthly_income_tl: 540_000.0,
    monthly_expense_tl: 410_000.0,
    seasonality: [
        0.4, 0.45, 0.65, 0.95, 1.35, 1.85, 2.2, 2.1, 1.4, 1.0, 0.55, 0.5,
    ],
    income_cats: &[
        CatProfile {
            name: "Satış",
            weight: 0.93,
            entries_min: 8,
            entries_max: 18,
            descriptions: &[
                "Otel konaklama geliri",
                "Restoran ve bar geliri",
                "Online rezervasyon tahsilatı (Booking)",
                "Tur ve transfer geliri",
                "Toplantı ve etkinlik salonu geliri",
                "Spa ve havuz hizmet geliri",
            ],
        },
        CatProfile {
            name: "Diğer Gelir",
            weight: 0.07,
            entries_min: 0,
            entries_max: 2,
            descriptions: &["Hediyelik eşya satışı", "Sponsor anlaşma geliri"],
        },
    ],
    expense_cats: &[
        CatProfile {
            name: "Maaş",
            weight: 0.32,
            entries_min: 1,
            entries_max: 2,
            descriptions: &[
                "Resepsiyon ve kat hizmetleri maaşları",
                "Mutfak personeli maaşları",
                "Sezonluk personel ücretleri",
                "Yönetici kadro maaşları",
            ],
        },
        CatProfile {
            name: "Tedarik",
            weight: 0.22,
            entries_min: 4,
            entries_max: 8,
            descriptions: &[
                "Gıda tedariği",
                "İçecek toptan alımı",
                "Temizlik kimyasal tedarik",
                "Çamaşır ve tekstil",
            ],
        },
        CatProfile {
            name: "Pazarlama",
            weight: 0.10,
            entries_min: 2,
            entries_max: 4,
            descriptions: &[
                "Booking komisyonu",
                "Expedia komisyonu",
                "Google Ads",
                "Tur operatörü komisyonu",
            ],
        },
        CatProfile {
            name: "Fatura",
            weight: 0.10,
            entries_min: 2,
            entries_max: 4,
            descriptions: &[
                "Elektrik (yüksek tüketim)",
                "Su faturası",
                "Doğal gaz",
                "İnternet",
            ],
        },
        CatProfile {
            name: "Kira",
            weight: 0.08,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["Otel binası kira", "Yer kirası"],
        },
        CatProfile {
            name: "Vergi",
            weight: 0.05,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["KDV", "Konaklama vergisi"],
        },
        CatProfile {
            name: "Sigorta",
            weight: 0.04,
            entries_min: 0,
            entries_max: 1,
            descriptions: &["Otel sigorta poliçesi", "Müşteri sorumluluk sigortası"],
        },
        CatProfile {
            name: "Üretim",
            weight: 0.04,
            entries_min: 1,
            entries_max: 3,
            descriptions: &["Tesis bakım ve onarım", "Klima bakımı"],
        },
        CatProfile {
            name: "Lojistik",
            weight: 0.05,
            entries_min: 1,
            entries_max: 3,
            descriptions: &["Transfer ve servis araç yakıt", "Lojistik ödemeleri"],
        },
    ],
};

// Eğitim Kursu
const EGITIM_KURSU: SectorProfile = SectorProfile {
    id: "egitim_kursu",
    display: "Eğitim Kursu",
    monthly_income_tl: 220_000.0,
    monthly_expense_tl: 175_000.0,
    seasonality: [
        1.1, 1.05, 1.05, 1.0, 0.85, 0.55, 0.4, 0.5, 1.95, 1.7, 1.2, 1.05,
    ],
    income_cats: &[
        CatProfile {
            name: "Satış",
            weight: 0.95,
            entries_min: 6,
            entries_max: 14,
            descriptions: &[
                "Aylık kurs ücreti tahsilatı",
                "Yıllık kayıt ücreti",
                "YKS hazırlık paketi",
                "LGS deneme sınavı geliri",
                "Online kurs satışı",
                "Hızlı okuma eğitimi geliri",
                "Yaz okulu kaydı (düşük dönem)",
            ],
        },
        CatProfile {
            name: "Devlet Desteği",
            weight: 0.05,
            entries_min: 0,
            entries_max: 1,
            descriptions: &["MEB protokol ödemesi"],
        },
    ],
    expense_cats: &[
        CatProfile {
            name: "Maaş",
            weight: 0.55,
            entries_min: 1,
            entries_max: 2,
            descriptions: &[
                "Öğretmen maaşları",
                "Eğitim koordinatörü maaşı",
                "İdari personel maaşı",
            ],
        },
        CatProfile {
            name: "Kira",
            weight: 0.18,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["Kurs binası kirası"],
        },
        CatProfile {
            name: "Pazarlama",
            weight: 0.10,
            entries_min: 1,
            entries_max: 4,
            descriptions: &[
                "Google Ads",
                "Instagram reklam",
                "Tabela ve broşür",
                "Veli toplantı organizasyonu",
            ],
        },
        CatProfile {
            name: "Tedarik",
            weight: 0.06,
            entries_min: 1,
            entries_max: 3,
            descriptions: &["Kitap ve yayın alımı", "Kırtasiye toptan"],
        },
        CatProfile {
            name: "Fatura",
            weight: 0.05,
            entries_min: 2,
            entries_max: 3,
            descriptions: &["Elektrik", "İnternet", "Telefon"],
        },
        CatProfile {
            name: "Vergi",
            weight: 0.04,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["KDV", "Stopaj"],
        },
        CatProfile {
            name: "Teknoloji",
            weight: 0.02,
            entries_min: 0,
            entries_max: 2,
            descriptions: &["Eğitim yazılımı abonelik", "Zoom kurumsal lisans"],
        },
    ],
};

// Lojistik
// Düşük marjlı / yüksek operasyonel giderli profil
const LOJISTIK: SectorProfile = SectorProfile {
    id: "lojistik",
    display: "Lojistik / Kargo",
    monthly_income_tl: 645_000.0,
    monthly_expense_tl: 600_000.0,
    seasonality: [0.9, 0.9, 1.0, 1.0, 1.0, 1.0, 1.0, 1.05, 1.1, 1.2, 1.45, 1.4],
    income_cats: &[
        CatProfile {
            name: "Satış",
            weight: 0.96,
            entries_min: 8,
            entries_max: 16,
            descriptions: &[
                "Aylık taşıma sözleşmesi",
                "Spot konteyner tahsilatı",
                "E-ticaret kargo geliri",
                "Soğuk zincir taşıma",
                "Şehirler arası nakliye ücreti",
                "Depolama hizmet geliri",
            ],
        },
        CatProfile {
            name: "Diğer Gelir",
            weight: 0.04,
            entries_min: 0,
            entries_max: 2,
            descriptions: &["Sigorta tazminat", "Hurda araç satış"],
        },
    ],
    expense_cats: &[
        CatProfile {
            name: "Tedarik",
            weight: 0.28,
            entries_min: 5,
            entries_max: 12,
            descriptions: &[
                "Akaryakıt ve mazot alımı",
                "Yedek parça stok",
                "Lastik alımı",
                "Adblue tedarik",
            ],
        },
        CatProfile {
            name: "Maaş",
            weight: 0.32,
            entries_min: 1,
            entries_max: 2,
            descriptions: &[
                "Şoför maaşları (15 kişi)",
                "Depo personeli maaşı",
                "Dispatcher maaşı",
            ],
        },
        CatProfile {
            name: "Üretim",
            weight: 0.13,
            entries_min: 2,
            entries_max: 5,
            descriptions: &["Araç bakım ve onarım", "Yağ değişimi", "Periyodik bakım"],
        },
        CatProfile {
            name: "Sigorta",
            weight: 0.08,
            entries_min: 1,
            entries_max: 2,
            descriptions: &["Kasko sigortası", "Trafik sigortası", "Yük sigortası"],
        },
        CatProfile {
            name: "Vergi",
            weight: 0.06,
            entries_min: 1,
            entries_max: 2,
            descriptions: &["MTV ödemesi", "KDV", "ÖTV"],
        },
        CatProfile {
            name: "Kira",
            weight: 0.05,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["Depo kira", "Filo park kirası"],
        },
        CatProfile {
            name: "Fatura",
            weight: 0.05,
            entries_min: 1,
            entries_max: 3,
            descriptions: &["Elektrik", "İnternet"],
        },
        CatProfile {
            name: "Teknoloji",
            weight: 0.03,
            entries_min: 1,
            entries_max: 2,
            descriptions: &["Filo takip sistemi", "TMS yazılım abonelik"],
        },
    ],
};

// Medikal
const MEDIKAL: SectorProfile = SectorProfile {
    id: "medikal",
    display: "Medikal Ürün",
    monthly_income_tl: 460_000.0,
    monthly_expense_tl: 360_000.0,
    seasonality: [
        1.05, 1.05, 1.1, 1.05, 1.0, 0.95, 0.9, 0.95, 1.05, 1.05, 1.05, 1.1,
    ],
    income_cats: &[
        CatProfile {
            name: "Satış",
            weight: 0.95,
            entries_min: 5,
            entries_max: 11,
            descriptions: &[
                "Hastane sipariş tahsilatı",
                "SGK ödeme tahsilatı",
                "Bayi sipariş ödemesi",
                "Eczane toptan satış",
                "Özel klinik sipariş geliri",
            ],
        },
        CatProfile {
            name: "Diğer Gelir",
            weight: 0.05,
            entries_min: 0,
            entries_max: 2,
            descriptions: &["Eğitim ve demo geliri"],
        },
    ],
    expense_cats: &[
        CatProfile {
            name: "Tedarik",
            weight: 0.45,
            entries_min: 3,
            entries_max: 7,
            descriptions: &[
                "İthalat medikal cihaz",
                "Sarf malzeme alımı",
                "Yedek parça stoğu",
            ],
        },
        CatProfile {
            name: "Maaş",
            weight: 0.25,
            entries_min: 1,
            entries_max: 2,
            descriptions: &[
                "Satış temsilcisi maaşları",
                "Teknik servis maaşı",
                "Ofis personeli",
            ],
        },
        CatProfile {
            name: "Lojistik",
            weight: 0.08,
            entries_min: 1,
            entries_max: 4,
            descriptions: &[
                "Kargo gönderim ücreti",
                "Soğuk zincir taşıma",
                "Saha ziyaret yakıt",
            ],
        },
        CatProfile {
            name: "Kira",
            weight: 0.06,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["Ofis ve depo kira"],
        },
        CatProfile {
            name: "Vergi",
            weight: 0.06,
            entries_min: 1,
            entries_max: 2,
            descriptions: &["KDV", "Gümrük vergisi", "Stopaj"],
        },
        CatProfile {
            name: "Sigorta",
            weight: 0.04,
            entries_min: 0,
            entries_max: 1,
            descriptions: &["Ürün sorumluluk sigortası"],
        },
        CatProfile {
            name: "Pazarlama",
            weight: 0.04,
            entries_min: 0,
            entries_max: 2,
            descriptions: &["Kongre ve fuar katılımı", "Doktor eğitim semineri"],
        },
        CatProfile {
            name: "Fatura",
            weight: 0.02,
            entries_min: 1,
            entries_max: 2,
            descriptions: &["Elektrik", "İnternet"],
        },
    ],
};

// Sağlık Klinik
// Orta-yüksek marjlı hizmet profili
const SAGLIK_KLINIK: SectorProfile = SectorProfile {
    id: "saglik_klinik",
    display: "Sağlık Kliniği",
    monthly_income_tl: 600_000.0,
    monthly_expense_tl: 470_000.0,
    seasonality: [
        1.05, 1.0, 1.05, 1.05, 1.05, 0.95, 0.85, 0.9, 1.05, 1.1, 1.1, 1.0,
    ],
    income_cats: &[
        CatProfile {
            name: "Satış",
            weight: 0.95,
            entries_min: 12,
            entries_max: 22,
            descriptions: &[
                "Muayene ücreti tahsilatı",
                "Tedavi paket ödemesi",
                "Laboratuvar test geliri",
                "Estetik uygulama tahsilatı",
                "Diş tedavi tahsilatı",
                "Görüntüleme geliri",
                "Özel sigorta tahsilatı",
            ],
        },
        CatProfile {
            name: "Diğer Gelir",
            weight: 0.05,
            entries_min: 0,
            entries_max: 2,
            descriptions: &["Sağlık raporu ücreti", "İşyeri hekimi geliri"],
        },
    ],
    expense_cats: &[
        CatProfile {
            name: "Maaş",
            weight: 0.42,
            entries_min: 1,
            entries_max: 2,
            descriptions: &[
                "Doktor maaşları",
                "Hemşire ve teknisyen maaşları",
                "Resepsiyon ve sekreter maaşı",
            ],
        },
        CatProfile {
            name: "Tedarik",
            weight: 0.22,
            entries_min: 3,
            entries_max: 7,
            descriptions: &[
                "Tıbbi sarf malzeme",
                "İlaç stoğu",
                "Tek kullanımlık ürünler",
                "Laboratuvar reaktifi",
            ],
        },
        CatProfile {
            name: "Kira",
            weight: 0.10,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["Klinik kira ödemesi"],
        },
        CatProfile {
            name: "Üretim",
            weight: 0.08,
            entries_min: 1,
            entries_max: 3,
            descriptions: &["Cihaz bakım ve kalibrasyon", "Sterilizasyon ekipmanı"],
        },
        CatProfile {
            name: "Fatura",
            weight: 0.06,
            entries_min: 2,
            entries_max: 3,
            descriptions: &["Elektrik", "Su", "Doğal gaz"],
        },
        CatProfile {
            name: "Sigorta",
            weight: 0.05,
            entries_min: 0,
            entries_max: 1,
            descriptions: &["Mesleki sorumluluk sigortası", "Klinik sigorta"],
        },
        CatProfile {
            name: "Vergi",
            weight: 0.04,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["KDV", "Stopaj"],
        },
        CatProfile {
            name: "Pazarlama",
            weight: 0.03,
            entries_min: 0,
            entries_max: 2,
            descriptions: &["Google Ads sağlık", "Sosyal medya tanıtım"],
        },
    ],
};

// Gıda Üretim
const GIDA_URETIM: SectorProfile = SectorProfile {
    id: "gida_uretim",
    display: "Gıda Üretim",
    monthly_income_tl: 620_000.0,
    monthly_expense_tl: 525_000.0,
    seasonality: [
        1.0, 1.0, 1.05, 1.0, 1.0, 1.05, 0.95, 1.0, 1.05, 1.1, 1.1, 1.2,
    ],
    income_cats: &[
        CatProfile {
            name: "Satış",
            weight: 0.95,
            entries_min: 6,
            entries_max: 13,
            descriptions: &[
                "Zincir market tahsilatı",
                "Bayi sipariş ödemesi",
                "Yerel bakkal toptan satış",
                "Restoran toptan tedarik geliri",
                "İhracat tahsilatı",
                "Online satış geliri",
            ],
        },
        CatProfile {
            name: "Diğer Gelir",
            weight: 0.05,
            entries_min: 0,
            entries_max: 2,
            descriptions: &["Devlet teşvik ödemesi", "Hurda ambalaj satışı"],
        },
    ],
    expense_cats: &[
        CatProfile {
            name: "Tedarik",
            weight: 0.48,
            entries_min: 4,
            entries_max: 9,
            descriptions: &[
                "Süt tedariği (çiftlik)",
                "Un ve şeker alımı",
                "Yağ ve katkı maddesi",
                "Meyve ve sebze hammadde",
                "Ambalaj malzemesi",
            ],
        },
        CatProfile {
            name: "Maaş",
            weight: 0.22,
            entries_min: 1,
            entries_max: 2,
            descriptions: &[
                "Üretim işçileri maaşları",
                "Kalite kontrol maaşı",
                "Lojistik ekibi",
            ],
        },
        CatProfile {
            name: "Üretim",
            weight: 0.10,
            entries_min: 2,
            entries_max: 4,
            descriptions: &["Makine bakımı", "Soğutma ünitesi bakımı", "Sterilizasyon"],
        },
        CatProfile {
            name: "Lojistik",
            weight: 0.07,
            entries_min: 2,
            entries_max: 4,
            descriptions: &["Soğuk zincir nakliye", "Şehir içi dağıtım yakıt"],
        },
        CatProfile {
            name: "Fatura",
            weight: 0.05,
            entries_min: 2,
            entries_max: 3,
            descriptions: &["Elektrik (soğutma yüksek)", "Doğal gaz", "Su"],
        },
        CatProfile {
            name: "Kira",
            weight: 0.04,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["Üretim tesisi kirası"],
        },
        CatProfile {
            name: "Vergi",
            weight: 0.04,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["KDV", "Stopaj"],
        },
    ],
};

// Generic KOBİ
const KOBI: SectorProfile = SectorProfile {
    id: "kobi",
    display: "Genel KOBİ",
    monthly_income_tl: 240_000.0,
    monthly_expense_tl: 200_000.0,
    seasonality: [
        0.95, 1.0, 1.05, 1.05, 1.05, 1.0, 0.9, 0.9, 1.05, 1.1, 1.05, 1.1,
    ],
    income_cats: &[
        CatProfile {
            name: "Satış",
            weight: 0.95,
            entries_min: 8,
            entries_max: 16,
            descriptions: &[
                "Müşteri A fatura tahsilatı",
                "Müşteri B sipariş ödemesi",
                "Toptan satış tahsilatı",
                "Perakende günlük geliri",
                "Sözleşmeli hizmet bedeli",
            ],
        },
        CatProfile {
            name: "Diğer Gelir",
            weight: 0.05,
            entries_min: 0,
            entries_max: 2,
            descriptions: &["Faiz geliri", "Devlet hibe"],
        },
    ],
    expense_cats: &[
        CatProfile {
            name: "Maaş",
            weight: 0.36,
            entries_min: 1,
            entries_max: 2,
            descriptions: &["Personel maaşları", "SGK ve stopaj"],
        },
        CatProfile {
            name: "Tedarik",
            weight: 0.25,
            entries_min: 3,
            entries_max: 6,
            descriptions: &["Stok alımı", "Tedarikçi ödemesi", "Hammadde alımı"],
        },
        CatProfile {
            name: "Kira",
            weight: 0.10,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["Ofis kira", "Dükkan kirası"],
        },
        CatProfile {
            name: "Fatura",
            weight: 0.08,
            entries_min: 2,
            entries_max: 4,
            descriptions: &["Elektrik", "Su", "İnternet", "Telefon"],
        },
        CatProfile {
            name: "Pazarlama",
            weight: 0.07,
            entries_min: 1,
            entries_max: 3,
            descriptions: &["Reklam ödemesi", "Web sitesi bakım"],
        },
        CatProfile {
            name: "Vergi",
            weight: 0.07,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["KDV ödemesi", "Geçici vergi"],
        },
        CatProfile {
            name: "Sarf",
            weight: 0.04,
            entries_min: 1,
            entries_max: 2,
            descriptions: &["Ofis malzemeleri", "Sarf gider"],
        },
        CatProfile {
            name: "Sigorta",
            weight: 0.03,
            entries_min: 0,
            entries_max: 1,
            descriptions: &["İşyeri sigortası"],
        },
    ],
};

// Kuaför / Güzellik (YENİ)
const KUAFOR_GUZELLIK: SectorProfile = SectorProfile {
    id: "kuafor_guzellik",
    display: "Kuaför / Güzellik Salonu",
    monthly_income_tl: 165_000.0,
    monthly_expense_tl: 128_000.0,
    seasonality: [
        0.95, 0.95, 1.15, 1.1, 1.2, 1.25, 1.05, 1.15, 1.1, 1.05, 1.05, 1.3,
    ],
    income_cats: &[
        CatProfile {
            name: "Satış",
            weight: 0.94,
            entries_min: 15,
            entries_max: 26,
            descriptions: &[
                "Saç kesim hizmet geliri",
                "Saç boyama ve röfle",
                "Manikür ve pedikür",
                "Cilt bakım uygulaması",
                "Düğün ve özel gün makyaj",
                "Keratin ve fön bakımı",
                "Erkek tıraş ve sakal şekillendirme",
                "Lazer epilasyon seans",
                "Yılbaşı paket geliri",
            ],
        },
        CatProfile {
            name: "Diğer Gelir",
            weight: 0.06,
            entries_min: 1,
            entries_max: 3,
            descriptions: &["Saç bakım ürünü satışı", "Kozmetik ürün satışı"],
        },
    ],
    expense_cats: &[
        CatProfile {
            name: "Maaş",
            weight: 0.42,
            entries_min: 1,
            entries_max: 2,
            descriptions: &[
                "Kuaför ve berber maaşları",
                "Güzellik uzmanı maaşı",
                "Resepsiyon personeli maaşı",
            ],
        },
        CatProfile {
            name: "Tedarik",
            weight: 0.20,
            entries_min: 2,
            entries_max: 5,
            descriptions: &[
                "Boya ve saç bakım ürünü",
                "Şampuan toptan alımı",
                "Manikür/pedikür malzemesi",
                "Cilt bakım kozmetik",
            ],
        },
        CatProfile {
            name: "Kira",
            weight: 0.18,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["Salon kira ödemesi"],
        },
        CatProfile {
            name: "Fatura",
            weight: 0.07,
            entries_min: 2,
            entries_max: 4,
            descriptions: &["Elektrik", "Su", "İnternet"],
        },
        CatProfile {
            name: "Pazarlama",
            weight: 0.06,
            entries_min: 1,
            entries_max: 3,
            descriptions: &[
                "Instagram reklamı",
                "Booksy randevu platform ücreti",
                "Influencer iş birliği",
            ],
        },
        CatProfile {
            name: "Vergi",
            weight: 0.04,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["KDV", "Stopaj"],
        },
        CatProfile {
            name: "Sarf",
            weight: 0.03,
            entries_min: 1,
            entries_max: 2,
            descriptions: &["Havlu ve tek kullanımlık ürün", "Temizlik malzemeleri"],
        },
    ],
};

// Eczane (YENİ)
// Regülatif dar marjlı profil
const ECZANE: SectorProfile = SectorProfile {
    id: "eczane",
    display: "Eczane",
    monthly_income_tl: 442_000.0,
    monthly_expense_tl: 410_000.0,
    seasonality: [
        1.25, 1.2, 1.1, 1.0, 0.95, 0.95, 0.9, 0.95, 1.05, 1.1, 1.15, 1.3,
    ],
    income_cats: &[
        CatProfile {
            name: "Satış",
            weight: 0.96,
            entries_min: 18,
            entries_max: 28,
            descriptions: &[
                "Reçeteli ilaç satışı",
                "SGK reçete tahsilatı",
                "OTC ilaç satışı",
                "Dermokozmetik satış",
                "Bebek bakım ürünleri",
                "Medikal ürün satışı",
                "Reçetesiz ürün geliri",
            ],
        },
        CatProfile {
            name: "Diğer Gelir",
            weight: 0.04,
            entries_min: 0,
            entries_max: 2,
            descriptions: &[
                "Ölçüm hizmet ücreti (tansiyon, kan şekeri)",
                "SGK ödeme farkı",
            ],
        },
    ],
    expense_cats: &[
        CatProfile {
            name: "Tedarik",
            weight: 0.68,
            entries_min: 6,
            entries_max: 12,
            descriptions: &[
                "İlaç toptancı sipariş (depo)",
                "Dermokozmetik tedarik",
                "Bebek ürünleri stoğu",
                "Medikal sarf tedariği",
                "Reçetesiz ürün stoğu",
            ],
        },
        CatProfile {
            name: "Maaş",
            weight: 0.16,
            entries_min: 1,
            entries_max: 2,
            descriptions: &["Eczacı kalfası maaşı", "Yardımcı personel maaşı"],
        },
        CatProfile {
            name: "Kira",
            weight: 0.07,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["Eczane kira ödemesi"],
        },
        CatProfile {
            name: "Vergi",
            weight: 0.04,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["KDV", "Stopaj"],
        },
        CatProfile {
            name: "Fatura",
            weight: 0.03,
            entries_min: 2,
            entries_max: 3,
            descriptions: &["Elektrik", "İnternet"],
        },
        CatProfile {
            name: "Teknoloji",
            weight: 0.01,
            entries_min: 1,
            entries_max: 1,
            descriptions: &["Eczane otomasyon yazılım abonelik"],
        },
        CatProfile {
            name: "Sarf",
            weight: 0.01,
            entries_min: 1,
            entries_max: 2,
            descriptions: &["Poşet ve ambalaj", "Kırtasiye"],
        },
    ],
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_profiles_loadable() {
        assert_eq!(ALL_PROFILES.len(), 23, "23 sektör profili olmalı");
        for p in ALL_PROFILES {
            assert!(!p.id.is_empty());
            assert!(p.monthly_income_tl > 0.0);
            assert!(p.monthly_expense_tl > 0.0);
            assert!(!p.income_cats.is_empty(), "{} income_cats boş", p.id);
            assert!(!p.expense_cats.is_empty(), "{} expense_cats boş", p.id);

            // Seasonality 12 ay olmalı ve pozitif
            assert!(
                p.seasonality.iter().all(|&v| v > 0.0),
                "{} negatif seasonality",
                p.id
            );

            // Weight'ler ~1.0'a yaklaşmalı (esnek 0.8-1.2 aralığı)
            let inc_w: f64 = p.income_cats.iter().map(|c| c.weight).sum();
            let exp_w: f64 = p.expense_cats.iter().map(|c| c.weight).sum();
            assert!(
                (0.85..=1.15).contains(&inc_w),
                "{} income weight={}",
                p.id,
                inc_w
            );
            assert!(
                (0.85..=1.15).contains(&exp_w),
                "{} expense weight={}",
                p.id,
                exp_w
            );

            // Her kategori en az bir açıklama içermeli
            for cat in p.income_cats.iter().chain(p.expense_cats.iter()) {
                assert!(
                    !cat.descriptions.is_empty(),
                    "{}::{} açıklamasız",
                    p.id,
                    cat.name
                );
                assert!(
                    cat.entries_max >= cat.entries_min,
                    "{}::{} entries ters",
                    p.id,
                    cat.name
                );
            }
        }
    }

    #[test]
    fn test_generator_produces_csv_with_header() {
        let p = find_profile("restoran").unwrap();
        let csv = generate(
            p,
            Pattern::Stable,
            12,
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            42,
        );
        assert!(csv.starts_with("tarih,aciklama,gelir,gider,kategori"));
        let line_count = csv.lines().count();
        assert!(
            line_count > 50,
            "12 ayda 50+ satır beklenir, geldi: {}",
            line_count
        );
    }

    #[test]
    fn test_generator_deterministic_with_seed() {
        let p = find_profile("teknoloji_startup").unwrap();
        let a = generate(
            p,
            Pattern::Growth,
            18,
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            1234,
        );
        let b = generate(
            p,
            Pattern::Growth,
            18,
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            1234,
        );
        assert_eq!(a, b, "Aynı seed deterministik olmalı");
    }

    #[test]
    fn test_growth_pattern_increases_income() {
        // Eczane: düşük varyans, stabil gelir dağılımı (false negative riski az)
        let p = find_profile("eczane").unwrap();
        let csv = generate(
            p,
            Pattern::Growth,
            12,
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            7,
        );

        let month_sum = |needle: &str| -> f64 {
            csv.lines()
                .skip(1)
                .filter(|l| l.contains(needle))
                .filter_map(|l| l.split(',').nth(2).and_then(|s| s.parse::<f64>().ok()))
                .sum()
        };
        // 3 aylık moving sum karşılaştırma — tek aydan daha sağlam
        let early = month_sum("2024-01-") + month_sum("2024-02-") + month_sum("2024-03-");
        let late = month_sum("2024-10-") + month_sum("2024-11-") + month_sum("2024-12-");
        assert!(
            late > early * 1.2,
            "Growth pattern son 3 ay > ilk 3 ay × 1.2 olmalı (ilk={:.0}, son={:.0})",
            early,
            late
        );
    }

    #[test]
    fn test_polars_can_parse_generated_csv() {
        use crate::infrastructure::polars_engine::PolarsEngine;
        let p = find_profile("eczane").unwrap();
        let csv = generate(
            p,
            Pattern::Stable,
            12,
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            99,
        );

        let mut engine = PolarsEngine::new();
        let (rows, _cols, _names) = engine
            .load_csv_from_string(&csv)
            .expect("Generated CSV polars tarafından parse edilebilir olmalı");
        assert!(rows > 50);

        let monthly = engine.monthly_breakdown();
        assert!(monthly.len() >= 10, "12 ayda 10+ ay olmalı");
    }
}
