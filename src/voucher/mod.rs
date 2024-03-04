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

    /// The height of the voucher (use `None` to grow it dynamically)
    height: Option<u32>,

    /// The components that have been added
    components: Vec<Component>,

    /// A shared context for the text layout data
    text_ctx: TextContext,
}

impl Builder {
    pub fn new(width: u32, height: Option<u32>) -> Self {
        Self {
            width,
            height,
            components: Vec::new(),
            text_ctx: TextContext::new(),
        }
    }

    pub fn build(mut self) -> GrayImage {
        // Accumulate the total height.
        let height = self
            .components
            .iter()
            .map(|c| c.height())
            .sum::<u32>()
            .min(self.height.unwrap_or(u32::MAX)); // TODO: Upper height for continuous labels?

        let mut image = GrayImage::new(self.width, height);
        image.fill(0xff);

        // Render the components.
        let mut offset_y_pix = 0;

        for component in &self.components {
            use Component::*;

            match component {
                Text(comp) => comp.render(&mut image, offset_y_pix, &mut self.text_ctx),
                Image(comp) => comp.render(&mut image, offset_y_pix),
            }

            offset_y_pix += component.height();
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
