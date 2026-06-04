// Demo CSV regenerator
//
// Tüm 23 sektör için statik anchor CSV'leri data/ klasörüne yeniden üretir.
// Her sektör için sabit seed kullanılır — build'ler arası determinizm için.
//
// Çalıştırma: `cargo run --bin regenerate_demos`
//
// Pattern seçimi sektör doğasına göre özelleştirilmiş (turizm seasonal,
// startup growth, vb.). Anchor demolar yatırımcı sunumlarında tekrarlanabilir
// olsun diye sabittir; canlı /api/demo?generate=true farklı seed kullanır.

use aera_cfo::infrastructure::data_generator::{all_ids, find_profile, generate, Pattern};
use chrono::NaiveDate;
use std::fs;
use std::path::Path;

// Sektör başına özel pattern + ay sayısı + seed
// Seed sayıları rastgele seçildi ama sabit — anchor demoların stabil olması için
fn config_for(id: &str) -> (Pattern, usize, u64) {
    match id {
        "teknoloji_startup" => (Pattern::Growth, 18, 4242),
        "yazilim_ajans" => (Pattern::Growth, 15, 4343),
        "e_ticaret" => (Pattern::Seasonal, 18, 4444),
        "turizm" => (Pattern::Seasonal, 24, 4545),
        "egitim_kursu" => (Pattern::Seasonal, 18, 4646),
        "perakende" => (Pattern::Seasonal, 15, 4747),
        "insaat" => (Pattern::Seasonal, 18, 4848),
        "muhasebe_buro" => (Pattern::Seasonal, 15, 4949),
        "otomotiv_servis" => (Pattern::Seasonal, 15, 5050),
        "eczane" => (Pattern::Stable, 18, 5151),
        "kuafor_guzellik" => (Pattern::Stable, 15, 5252),
        "restoran" => (Pattern::Stable, 18, 5353),
        "cafe" => (Pattern::Stable, 15, 5454),
        "imalat" => (Pattern::Mature, 18, 5555),
        "ihracat" => (Pattern::Mature, 18, 5656),
        "lojistik" => (Pattern::Mature, 18, 5757),
        "gida_uretim" => (Pattern::Stable, 15, 5858),
        "tekstil" => (Pattern::Seasonal, 18, 5959),
        "emlak" => (Pattern::Seasonal, 15, 6060),
        "medikal" => (Pattern::Stable, 15, 6161),
        "saglik_klinik" => (Pattern::Stable, 15, 6262),
        "danismanlik" => (Pattern::Stable, 15, 6363),
        "kobi" => (Pattern::Stable, 15, 6464),
        _ => (Pattern::Stable, 15, 1234),
    }
}

fn main() {
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        fs::create_dir_all(data_dir).expect("data/ oluşturulamadı");
    }

    // Her sektörün 24 ay'lık geçmiş alanı içinde son N ayı kapsasın.
    // Başlangıç: 2024-01-01 — Holt projeksiyonu için yeterli geçmiş.
    let start = NaiveDate::from_ymd_opt(2024, 1, 1).expect("Geçerli tarih");

    let mut total_rows = 0usize;
    for id in all_ids() {
        let profile = find_profile(id).expect("profile bulunamadı");
        let (pattern, months, seed) = config_for(id);
        let csv = generate(profile, pattern, months, start, seed);
        let path = data_dir.join(format!("demo_{}.csv", id));
        let row_count = csv.lines().count().saturating_sub(1); // başlık hariç
        total_rows += row_count;
        fs::write(&path, &csv).expect("CSV yazılamadı");
        println!(
            "✅ {:30} → {:>4} satır, {} ay, {:?}",
            profile.display, row_count, months, pattern,
        );
    }
    println!(
        "\n🎯 23 sektör tamamlandı. Toplam {} işlem satırı.",
        total_rows
    );
}
