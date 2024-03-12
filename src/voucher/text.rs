use super::{Alignment, Builder as VoucherBuilder, Component as VoucherComponent, Spacing};

use std::ops::Range;

use cosmic_text::{
    Align, Attrs, AttrsList, BidiParagraphs, Family, FontSystem, LayoutLine, PhysicalGlyph,
    ShapeBuffer, ShapeLine, Shaping, Style, SwashCache as RasterCache,
    SwashContent as GlyphImageContent, Weight, Wrap,
};
use image::GrayImage;

/// Line height = LINE_HEIGHT_FACTOR * font size
const LINE_HEIGHT_FACTOR: f32 = 1.3;

pub(super) struct Context {
    font_system: FontSystem,
    scratch_buffer: ShapeBuffer,
    lines: Vec<LayoutLine>,
    glyphs: Vec<PhysicalGlyph>,
    raster_cache: RasterCache,
}

impl Context {
    pub fn new() -> Self {
        Self {
            font_system: FontSystem::new(),
            scratch_buffer: ShapeBuffer::default(),
            lines: Vec::new(),
            glyphs: Vec::new(),
            raster_cache: RasterCache::new(),
        }
    }
}

pub struct Builder<'t, 'f> {
    /// The underlying voucher builder
    voucher: VoucherBuilder,

    /// The text to render
    text: &'t str,

    /// The spacing to apply to this component
    spacing: Spacing,

    /// The alignment to apply to this component
    alignment: Alignment,

    /// The name of the font family
    font_family: Option<&'f str>,

    /// The font size (pixels)
    font_size: f32,

    /// Do we render bold text?
    bold: bool,

    /// Do we render italic text?
    italic: bool,
}

impl<'t, 'f> Builder<'t, 'f> {
    fn new(voucher: VoucherBuilder, text: &'t str) -> Self {
        Self {
            voucher,
            text,
            spacing: Default::default(),
            alignment: Alignment::Left,
            font_family: None,
            font_size: 12.0,
            bold: false,
            italic: false,
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

    pub fn font_family(mut self, font_family: &'f str) -> Self {
        self.font_family = Some(font_family);
        self
    }

    pub fn default_font_family(mut self) -> Self {
        self.font_family = None;
        self
    }

    pub fn font_size(mut self, font_size: f32) -> Self {
        assert!(font_size >= 0.0, "Font size must be non-negative.");
        self.font_size = font_size;

        self
    }

    pub fn bold(mut self, bold: bool) -> Self {
        self.bold = bold;
        self
    }

    pub fn italic(mut self, italic: bool) -> Self {
        self.italic = italic;
        self
    }

    pub fn finalize_text_component(mut self) -> VoucherBuilder {
        // Obtain the context.
        let ctx = &mut self.voucher.text_ctx;

        // Calculate the available line width and line height.
        // If one of them is degenerated, we return early.
        let line_width = (self.voucher.width as f32) - self.spacing.horz();
        let line_height = LINE_HEIGHT_FACTOR * self.font_size;

        if (line_width <= 0.0) || (line_height <= 0.0) {
            return self.voucher;
        }

        // Build the attributes.
        let attrs_list = {
            let family = self.font_family.map_or(Family::SansSerif, Family::Name);

            let weight = if self.bold {
                Weight::BOLD
            } else {
                Weight::NORMAL
            };

            let style = if self.italic {
                Style::Italic
            } else {
                Style::Normal
            };

            let attrs = Attrs::new().family(family).weight(weight).style(style);
            AttrsList::new(attrs)
        };

        // Break the text into bidi paragraphs.
        let old_lines_count = ctx.lines.len();

        for text_line in BidiParagraphs::new(self.text) {
            // Shape the line.
            let shape_line = ShapeLine::new_in_buffer(
                &mut ctx.scratch_buffer,
                &mut ctx.font_system,
                text_line,
                &attrs_list,
                Shaping::Advanced,
            );

            // Perform layouting.
            shape_line.layout_to_buffer(
                &mut ctx.scratch_buffer,
                self.font_size,
                line_width,
                Wrap::WordOrGlyph,
                Some(Align::Left),
                &mut ctx.lines,
                None,
            );
        }

        // Count the layout lines we have just added.
        // If there is not a single line we can fit, we should bail out.
        let lines_range = old_lines_count..ctx.lines.len();

        if lines_range.is_empty() {
            return self.voucher;
        }

        // Walk the lines to check their widths.
        for line in ctx.lines[lines_range.clone()].iter_mut() {
            // The line *can* exceed our maximum width at this point:
            // - Word wrapping might have failed (e.g. no spaces).
            // - A single glyph might be wide enough to overshoot.
            // In that case, we simply truncate the line until it fits.
            // TODO: It would be nice to ellipsize :)
            while line.w > line_width {
                // If we fail here, the line is exceeded.
                let Some(last_glyph) = line.glyphs.pop() else {
                    break;
                };

                // Adapt the line width.
                line.w -= last_glyph.w;
            }
        }

        // Calculate the total height of the component in pixels.
        let height_pix =
            (self.spacing.vert() + ((lines_range.len() as f32) * line_height)).ceil() as u32;

        // Push the text component to the builder.
        // It contains all info to render the lines.
        let component = Component {
            height_pix,
            lines_range,
            offset_x: self.spacing.left,
            offset_y: self.spacing.top,
            line_width,
            line_height,
            alignment: self.alignment,
        };

        self.voucher
            .components
            .push(VoucherComponent::Text(component));

        self.voucher
    }

    pub fn cancel_text_component(self) -> VoucherBuilder {
        self.voucher
    }
}

impl VoucherBuilder {
    pub fn start_text_component(self, text: &str) -> Builder {
        Builder::new(self, text)
    }
}

pub struct Component {
    /// The total height of the component in pixels
    height_pix: u32,

    /// The range in the vector of layout lines
    lines_range: Range<usize>,

    /// The X offset of all lines to render (aka `spacing.left`)
    offset_x: f32,

    /// The Y offset of the first line to render (aka `spacing.top`)
    offset_y: f32,

    /// The width of a line (aka `voucher.width - spacing.horz()`)
    line_width: f32,

    /// The height of a line (aka `LINE_HEIGHT_FACTOR * font_size`)
    line_height: f32,

    /// The alignment
    alignment: Alignment,
}

impl Component {
    pub fn height(&self) -> u32 {
        self.height_pix
    }

    pub(super) fn render(&self, image: &mut GrayImage, offset_y_pix: u32, ctx: &mut Context) {
        use Alignment::*;
        use GlyphImageContent::*;

        // First, we pre-calculate some stuff that is used in the loops.
        // Combine our vertical component offset and spacing.
        let total_offset_y = (offset_y_pix as f32) + self.offset_y;

        // The alignment factor moves a line in horizontal direction.
        let align_factor = match self.alignment {
            Left => 0.0,
            Center => 0.5,
            Right => 1.0,
        };

        // This rect defines the valid component area we can draw into.
        let comp_left_pix = 0;
        let comp_right_pix = comp_left_pix + (image.width() as i32);
        let comp_top_pix = offset_y_pix as i32;
        let comp_bottom_pix = comp_top_pix + (self.height_pix as i32);

        // This closure sets pixels in the image.
        let mut set_pixel = |x_pix, y_pix, glyph_a| {
            // Perform manual alpha blending. We blend A over B.
            // - `alpha_a` is color.a().
            // - `luma_a` is always 0xff (as our base color is black).
            // - `alpha_b` is always 0xff (as our background is opaque).
            // - `luma_b` is the existing pixel in the image.
            // Now, the blend equation simplifies to (1 - alpha_a) * luma_b.
            let pix = image.get_pixel_mut(x_pix, y_pix);
            let luma_b = (pix[0] as f32) / 255.0;
            let alpha_a = (glyph_a as f32) / 255.0;
            let new_luma = (1.0 - alpha_a) * luma_b;

            pix[0] = (new_luma * 255.0).round() as u8;
        };

        // Walk the lines.
        for (idx, line) in ctx.lines[self.lines_range.clone()].iter().enumerate() {
            // Calculate the glyph origin (= the leftmost point on the baseline).
            let glyph_origin_x = self.offset_x + (align_factor * (self.line_width - line.w));

            let glyph_origin_y = total_offset_y
                + ((idx as f32) * self.line_height)
                + ((self.line_height + line.max_ascent - line.max_descent) / 2.0);

            // Calculate the pixel positions of the line glyphs.
            ctx.glyphs.clear();

            ctx.glyphs.extend(
                line.glyphs
                    .iter()
                    .map(|g| g.physical((glyph_origin_x, glyph_origin_y), 1.0)),
            );

            // Walk the glyphs.
            for glyph in &ctx.glyphs {
                // Get the glyph image.
                let Some(glyph_image) = ctx
                    .raster_cache
                    .get_image(&mut ctx.font_system, glyph.cache_key)
                else {
                    eprintln!("Failed to rasterize image for glyph: {:?}", glyph);
                    continue;
                };

                // Compute a glyph image rect with upper-left origin and correct size.
                let glyph_image_width_pix = glyph_image.placement.width as usize;
                let glyph_image_height_pix = glyph_image.placement.height as usize;

                let glyph_image_left_pix = glyph.x + glyph_image.placement.left;
                let glyph_image_right_pix = glyph_image_left_pix + (glyph_image_width_pix as i32);
                let glyph_image_top_pix = glyph.y - glyph_image.placement.top;
                let glyph_image_bottom_pix = glyph_image_top_pix + (glyph_image_height_pix as i32);

                // Clip the glyph image against the component box.
                // Ideally, it should be fully contained, but there might be fonts
                // that don't respect their bounding box.
                let left_pix = glyph_image_left_pix.max(comp_left_pix);
                let right_pix = glyph_image_right_pix.min(comp_right_pix);
                let top_pix = glyph_image_top_pix.max(comp_top_pix);
                let bottom_pix = glyph_image_bottom_pix.min(comp_bottom_pix);

                // If the image is empty, we can bail out.
                if (left_pix >= right_pix) || (top_pix >= bottom_pix) {
                    continue;
                }

                // Calculate the initial row offset for the source.
                let mut glyph_row_offset_pix = (((top_pix - glyph_image_top_pix) as usize)
                    * glyph_image_width_pix)
                    + ((left_pix - glyph_image_left_pix) as usize);

                // Draw the image.
                match glyph_image.content {
                    Mask => {
                        for y_pix in top_pix..bottom_pix {
                            let glyph_row = &glyph_image.data[glyph_row_offset_pix..];

                            for (x_pix, &glyph_a) in (left_pix..right_pix).zip(glyph_row) {
                                set_pixel(x_pix as u32, y_pix as u32, glyph_a);
                            }

                            glyph_row_offset_pix += glyph_image_width_pix;
                        }
                    }

                    Color => {
                        for y_pix in top_pix..bottom_pix {
                            let glyph_row = &glyph_image.data[(glyph_row_offset_pix * 4)..];

                            for (x_pix, glyph_a) in (left_pix..right_pix)
                                .zip(glyph_row.chunks_exact(4).map(|pix| pix[3]))
                            {
                                set_pixel(x_pix as u32, y_pix as u32, glyph_a);
                            }

                            glyph_row_offset_pix += glyph_image_width_pix;
                        }
                    }

                    // Since we ordered `GlyphFormat::Alpha` via the renderer,
                    // we should never encounter anything else (e.g. subpixel antialiasing) here.
                    _ => unreachable!("Invalid glyph image content (expected mask or color)"),
                }
            }
        }
    }
}
