use image::GrayImage;

#[derive(Copy, Clone)]
pub struct Spacing {
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
}

impl Spacing {
    pub fn lrtb(left: f32, right: f32, top: f32, bottom: f32) -> Self {
        assert!(
            (left >= 0.0) && (right >= 0.0) && (top >= 0.0) && (bottom >= 0.0),
            "Spacing must be non-negative"
        );

        Self {
            left,
            right,
            top,
            bottom,
        }
    }

    pub fn horz_vert(horz: f32, vert: f32) -> Self {
        Self::lrtb(horz, horz, vert, vert)
    }

    pub fn all(all: f32) -> Self {
        Self::lrtb(all, all, all, all)
    }

    fn horz(&self) -> f32 {
        self.left + self.right
    }

    fn vert(&self) -> f32 {
        self.top + self.bottom
    }
}

impl Default for Spacing {
    fn default() -> Self {
        Self {
            left: 0.0,
            right: 0.0,
            top: 0.0,
            bottom: 0.0,
        }
    }
}

#[derive(Copy, Clone)]
pub enum Alignment {
    Left,
    Right,
    Center,
}

enum Component {
    Text(TextComponent),
    Image(ImageComponent),
}

impl Component {
    fn height(&self) -> u32 {
        use Component::*;

        match self {
            Text(text_component) => text_component.height(),
            Image(image_component) => image_component.height(),
        }
    }
}

pub struct Builder {
    /// The width of the voucher
    width: u32,

    /// The components that have been added
    components: Vec<Component>,

    /// A shared context for the text layout data
    text_ctx: TextContext,
}

impl Builder {
    pub fn new(width: u32) -> Self {
        Self {
            width,
            components: Vec::new(),
            text_ctx: TextContext::new(),
        }
    }

    pub fn build(mut self) -> GrayImage {
        // Accumulate the total height.
        let height = self.components.iter().map(Component::height).sum::<u32>();

        // Allocate the image and fill it with a white background.
        let mut image = GrayImage::new(self.width, height);
        image.fill(0xff);

        // Render the components.
        let mut offset_y_px = 0;

        for component in &self.components {
            use Component::*;

            match component {
                Text(comp) => comp.render(&mut image, offset_y_px, &mut self.text_ctx),
                Image(comp) => comp.render(&mut image, offset_y_px),
            }

            offset_y_px += component.height();
        }

        image
    }
}

/// Add image components to a voucher
pub mod img;

pub use img::Builder as ImageComponentBuilder;
use img::Component as ImageComponent;

/// Add text components to a voucher
pub mod text;

pub use text::Builder as TextComponentBuilder;
use text::{Component as TextComponent, Context as TextContext};

#[cfg(test)]
mod tests {
    use super::*;
    use image::{io::Reader as ImageReader, ImageFormat};

    #[test]
    fn realistic_voucher() {
        let logo = ImageReader::open("logo.png")
            .expect("Failed to load logo")
            .decode()
            .expect("Failed to decode logo");

        Builder::new(400)
            // Logo
            .start_image_component(&logo)
            .spacing(Spacing::horz_vert(20.0, 20.0))
            .finalize_image_component()
            // Product
            .start_text_component("Rinderhack")
            .spacing(Spacing::horz_vert(16.0, 16.0))
            .font_size(50.0)
            .alignment(Alignment::Center)
            .bold(true)
            .finalize_text_component()
            // Weight
            .start_text_component("Gewicht: 20 kg")
            .spacing(Spacing::horz_vert(16.0, 12.0))
            .font_size(25.0)
            .finalize_text_component()
            // Price
            .start_text_component("Preis: 30,14 €")
            .spacing(Spacing::horz_vert(16.0, 24.0))
            .font_size(40.0)
            .bold(true)
            .finalize_text_component()
            // Ingredients
            .start_text_component("Zutaten: Rind, Fleisch, Wasser, Zucker, Salz, Vitamine")
            .spacing(Spacing::horz_vert(16.0, 12.0))
            .font_size(25.0)
            .finalize_text_component()
            // Additionals
            .start_text_component("Kann Spuren von Nüssen enthalten")
            .spacing(Spacing::horz_vert(16.0, 12.0))
            .font_size(25.0)
            .finalize_text_component()
            // Storage
            .start_text_component("Kühl lagern")
            .spacing(Spacing::horz_vert(16.0, 12.0))
            .font_size(25.0)
            .finalize_text_component()
            // Trailer
            .start_text_component("... weitere Infos folgen!")
            .spacing(Spacing::lrtb(8.0, 8.0, 48.0, 8.0))
            .font_size(21.0)
            .alignment(Alignment::Center)
            .italic(true)
            .finalize_text_component()
            .build()
            .save_with_format("test.png", ImageFormat::Png)
            .expect("Failed to save test image");
    }
}
