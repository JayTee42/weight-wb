use weight_wb::voucher::{Alignment, Builder, Spacing};

use image::io::Reader as ImageReader;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let img = ImageReader::open("/home/jaytee/logo.png")?.decode()?;

    let product = "Rinderschinken";
    let weight = 20.4;
    let price_per_kg = 2.24;
    let ingredients =
        "Rindfleisch, Gewürze, Trockenglukose, Nitritpökelsalz, kann Spuren von Sellerie enthalten";
    let additional =
        "DE0120337767, Herkunft: Schleswig-Holstein, geschlachtet: DEES199EG, zerlegt: SH00102";
    let mhd = "19.05.2023";
    let trailer = "Waldhof Wielenberg · F. Möller / K. Mau Gbr\nZum Wald 1 · 24991 Freienwill\n0151-52 42 29 84 · waldhofwielenberg@gmail.com";

    let voucher = Builder::new(690, None)
        .start_image_component(&img)
        .spacing(Spacing::horz_vert(16.0, 20.0))
        .finalize_image_component()
        .start_text_component(product)
        .spacing(Spacing::horz_vert(16.0, 16.0))
        .font_size(40.0)
        .alignment(Alignment::Center)
        .bold(true)
        .finalize_text_component()
        .start_text_component(&format!("Gewicht: {:.02} kg", weight))
        .spacing(Spacing::horz_vert(16.0, 12.0))
        .font_size(18.0)
        .finalize_text_component()
        .start_text_component(&format!("Preis: {:.02} €", weight * price_per_kg))
        .spacing(Spacing::horz_vert(16.0, 24.0))
        .font_size(30.0)
        .bold(true)
        .finalize_text_component()
        .start_text_component(&format!("Zutaten: {}", ingredients))
        .spacing(Spacing::horz_vert(16.0, 12.0))
        .font_size(18.0)
        .finalize_text_component()
        .start_text_component(&format!("{}", additional))
        .spacing(Spacing::horz_vert(16.0, 12.0))
        .font_size(18.0)
        .finalize_text_component()
        .start_text_component(&format!("Ungeöffnet mindestens haltbar bis: {}", mhd))
        .spacing(Spacing::horz_vert(16.0, 12.0))
        .font_size(18.0)
        .finalize_text_component()
        .start_text_component(&format!("{}", trailer))
        .spacing(Spacing::lrtb(8.0, 8.0, 48.0, 12.0))
        .font_size(14.0)
        .italic(true)
        .finalize_text_component()
        .build();

    voucher.save("/home/jaytee/voucher.png")?;

    Ok(())
}
