// Demo CSV'lerini tek seferde okuyup health_score dağılımını yazdırır.
// Yatırımcı demosunda A/B/C/D spektrumunu görmek için.
//
// Çalıştırma: `cargo run --bin score_demos`

use aera_cfo::infrastructure::polars_engine::PolarsEngine;
use aera_cfo::infrastructure::data_generator::all_ids;
use std::fs;

fn main() {
    let mut rows: Vec<(String, i64, String, f64, f64)> = Vec::new();

    for id in all_ids() {
        let path = format!("data/demo_{}.csv", id);
        let csv = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let mut engine = PolarsEngine::new();
        if engine.load_csv_from_string(&csv).is_err() { continue; }

        let h = engine.health_score();
        let skor = h["skor"].as_i64().unwrap_or(0);
        let harf = h["harf"].as_str().unwrap_or("?").to_string();
        let rezerv = h["detay"]["rezerv_ay"].as_f64().unwrap_or(0.0);
        let ratio = h["detay"]["gelir_gider_orani"].as_f64().unwrap_or(0.0);

        rows.push((id.to_string(), skor, harf, rezerv, ratio));
    }

    // Skora göre sırala — A'dan D'ye
    rows.sort_by_key(|r| std::cmp::Reverse(r.1));

    println!("\n{:<22} {:>6} {:>4} {:>10} {:>8}", "Sektör", "Skor", "Harf", "Rezerv(ay)", "G/G");
    println!("{}", "─".repeat(56));
    let mut by_grade: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for (id, skor, harf, rezerv, ratio) in &rows {
        *by_grade.entry(harf.clone()).or_insert(0) += 1;
        let emoji = match harf.as_str() {
            "A" => "🟢",
            "B" => "🟡",
            "C" => "🟠",
            "D" => "🔴",
            _ => "⚪",
        };
        println!("{} {:<20} {:>6} {:>4} {:>10.2} {:>8.2}", emoji, id, skor, harf, rezerv, ratio);
    }
    println!("\n📊 Dağılım: {:?}", by_grade);
    println!("Toplam: {} sektör\n", rows.len());
}
