use image::io::Reader as ImageReader;
use weight_wb::printer::Printer;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let img = ImageReader::open("/Users/jaytee/Desktop/1.png")?
        .decode()?
        .to_luma8();

    let printer = Printer::attach(None)?;
    printer.print(&img)?;

    Ok(())
}
