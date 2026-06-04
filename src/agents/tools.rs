use crate::infrastructure::gemini::FunctionDeclaration;
use serde_json::json;

/// Gemini'nin çağırabileceği tüm araç (tool) tanımlarını döndürür.
/// Bu fonksiyonlar, CFO ajanının otonom karar almasını sağlayan "elleri"dir.
pub fn get_cfo_tools() -> Vec<FunctionDeclaration> {
    vec![
        FunctionDeclaration {
            name: "analyze_cash_flow".to_string(),
            description: "KOBİ'nin nakit akışını analiz eder. Gelir ve gider sütunlarını toplayarak net nakit pozisyonunu, burn rate'i, kalan süreyi ve risk seviyesini hesaplar.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "income_column": {
                        "type": "string",
                        "description": "Gelir sütununun adı (örn: 'gelir')"
                    },
                    "expense_column": {
                        "type": "string",
                        "description": "Gider sütununun adı (örn: 'gider')"
                    }
                },
                "required": ["income_column", "expense_column"]
            }),
        },
        FunctionDeclaration {
            name: "get_data_summary".to_string(),
            description: "Yüklenen mali verinin genel istatistiksel özetini döndürür. Sütun adları, veri tipleri, satır/sütun sayıları.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
        },
        FunctionDeclaration {
            name: "search_incentives".to_string(),
            description: "KOSGEB, TÜBİTAK ve diğer devlet teşviklerini arar. KOBİ'nin durumuna uygun teşvik programlarını listeler.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Aranacak teşvik konusu (örn: 'dijital dönüşüm', 'ihracat desteği', 'nakit destek')"
                    }
                },
                "required": ["query"]
            }),
        },
        FunctionDeclaration {
            name: "predict_cashflow".to_string(),
            description: "Geçmiş mali verilere dayanarak önümüzdeki aylara ait nakit akışı projeksiyonu yapar. 'Gelecek ay', '3 ay sonra', 'bütçe tahmini' gibi sorularda kullan.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "months_ahead": {
                        "type": "integer",
                        "description": "Kaç ay ilerisi için projeksiyon yapılsın (1-6 arası)"
                    }
                },
                "required": ["months_ahead"]
            }),
        },
        // Finansal Sağlık Skoru
        FunctionDeclaration {
            name: "get_health_score".to_string(),
            description: "AeraCFO Finansal Sağlık Skoru'nu hesaplar. 0-100 arası composite skor: nakit güvenliği (40p), gelir/gider oranı (35p), istikrar (25p). Harf notu: A/B/C/D. Her analizde çağır.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
        },
        // Türkiye Sektör Benchmark
        FunctionDeclaration {
            name: "compare_sector_benchmark".to_string(),
            description: "Firmanın finansal metriklerini Türkiye KOBİ sektör ortalamasıyla karşılaştırır. 8 sektör desteklenir: teknoloji, perakende, üretim, hizmet, inşaat, gıda, danışmanlık, genel.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "sektor": {
                        "type": "string",
                        "description": "Firma sektörü (örn: 'teknoloji', 'perakende', 'üretim', 'hizmet', 'inşaat', 'gıda')"
                    }
                },
                "required": ["sektor"]
            }),
        },
        // Kategori bazlı gider analizi
        FunctionDeclaration {
            name: "analyze_expense_categories".to_string(),
            description: "Harcamaları kategori bazında analiz eder: maaş, kira, SGK, teknoloji oranları. CSV'de 'kategori' sütunu varsa kullan.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
        },
        // Z-score anomali tespiti
        FunctionDeclaration {
            name: "detect_anomalies".to_string(),
            description: "Harcama ve gelirlerde istatistiksel anomali (anormal artış/düşüş) tespit eder. Z-score ile son ayı önceki aylara kıyaslar. Beklenmedik değişiklik veya sıradışı ay sorulduğunda kullan.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
        },
        // What-If senaryo simülasyonu
        FunctionDeclaration {
            name: "simulate_scenario".to_string(),
            description: "What-If senaryo simülasyonu: ek gelir veya gider parametreleriyle gelecek nakit akışını tahmin eder. '2 kişi işe alsam', 'kira %20 artsa', 'fiyatları yükselsem' gibi sorularda kullan.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "extra_monthly_income": {
                        "type": "number",
                        "description": "Aylık ek gelir miktarı TL (negatif olabilir)"
                    },
                    "extra_monthly_expense": {
                        "type": "number",
                        "description": "Aylık ek gider miktarı TL (negatif olabilir, gider azalması için)"
                    },
                    "months": {
                        "type": "integer",
                        "description": "Kaç aylık projeksiyon (1-6)"
                    }
                },
                "required": ["extra_monthly_income", "extra_monthly_expense", "months"]
            }),
        },
        // Nakit sıkışıklığı tespiti
        FunctionDeclaration {
            name: "detect_cash_crunch".to_string(),
            description: "Ardışık negatif nakit akışı aylarını tespit eder. Geciken alacaklar, nakit sıkışıklığı riski veya kümülatif zarar durumunda kullan.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
        },
    ]
}
