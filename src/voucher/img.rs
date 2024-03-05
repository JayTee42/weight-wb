use super::{Alignment, Builder as VoucherBuilder, Component as VoucherComponent, Spacing};

use image::{imageops::FilterType, DynamicImage, GrayImage};

pub struct Builder {
    /// The underlying voucher builder
    voucher: VoucherBuilder,

    /// The image that shall be rendered
    image: GrayImage,

    /// The spacing to apply to this component
    spacing: Spacing,

    /// The alignment to apply to this component
    alignment: Alignment,
}

impl Builder {
    fn new(voucher: VoucherBuilder, image: &DynamicImage) -> Self {
        // Convert the image to grayscale.
        Self {
            voucher,
            image: image.to_luma8(),
            spacing: Default::default(),
            alignment: Alignment::Center,
        }
    }

    pub fn spacing(mut self, spacing: Spacing) -> Self {
        self.spacing = spacing;
        self
    }

    pub fn alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }

    pub fn finalize_image_component(mut self) -> VoucherBuilder {
        // Calculate the available line width.
        // If it is degenerated, we return early.
        let width_pix = ((self.voucher.width as f32) - self.spacing.horz()).floor() as u32;

        if width_pix == 0 {
            return self.voucher;
        }

        // Downscale the image to the given width if it is above it.
        // This method keeps its aspect ratio.
        if width_pix < self.image.width() {
            self.image = DynamicImage::from(self.image)
                .resize(width_pix, u32::MAX, FilterType::CatmullRom)
                .to_luma8();
        }

        // Determine the X offset of the image.
        let empty_width = width_pix - self.image.width();

        let offset_x_pix = self.spacing.left.round() as u32
            + match self.alignment {
                Alignment::Left => 0,
                Alignment::Right => empty_width,
                Alignment::Center => empty_width / 2,
            };

        // Push the image component to the builder.
        // It contains all info to render the image.
        let component = Component {
            image: self.image,
            offset_x_pix,
            offset_y_pix: self.spacing.top.round() as u32,
            vert_spacing_pix: self.spacing.vert().round() as u32,
        };

        self.voucher
            .components
            .push(VoucherComponent::Image(component));

        self.voucher
    }

    pub fn cancel_image_component(self) -> VoucherBuilder {
        self.voucher
    }
}

impl VoucherBuilder {
    pub fn start_image_component(self, image: &DynamicImage) -> Builder {
        Builder::new(self, image)
    }
}

pub struct Component {
    /// The converted and resized image
    image: GrayImage,

    /// The X pixel offset to render the image to (aka `spacing.left` + potential alignment)
    offset_x_pix: u32,

    /// The Y pixel offset to render the image to (aka `spacing.top`)
    offset_y_pix: u32,

    /// The vertical spacing in pixels
    vert_spacing_pix: u32,
}

impl Component {
    pub fn height(&self) -> u32 {
        self.vert_spacing_pix + self.image.height()
    }

    pub(super) fn render(&self, image: &mut GrayImage, offset_y_pix: u32) {
        // Combine our vertical component offset and spacing.
        let total_offset_y = offset_y_pix + self.offset_y_pix;

        // Walk the pixels.
        for y in 0..self.image.height() {
            for x in 0..self.image.width() {
                let pix = *self.image.get_pixel(x, y);
                let x_pix = self.offset_x_pix + x;
                let y_pix = total_offset_y + y;

                image.put_pixel(x_pix, y_pix, pix);
            }
        }
    }
}
