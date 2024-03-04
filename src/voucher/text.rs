use super::{Alignment, Builder as VoucherBuilder, Component as VoucherComponent, Spacing};

use std::ops::Range;

use cosmic_text::{
    Align, Attrs, AttrsList, BidiParagraphs, Color, Family, FontSystem, LayoutLine, ShapeBuffer,
    ShapeLine, Shaping, Style, SwashCache as RasterCache, Weight, Wrap,
};
use image::GrayImage;

/// Line height = LINE_HEIGHT_FACTOR * font size
const LINE_HEIGHT_FACTOR: f32 = 1.3;

pub(super) struct Context {
    font_system: FontSystem,
    scratch_buffer: ShapeBuffer,
    lines: Vec<LayoutLine>,
    raster_cache: RasterCache,
}

impl Context {
    pub fn new() -> Self {
        Self {
            font_system: FontSystem::new(),
            scratch_buffer: ShapeBuffer::default(),
            lines: Vec::new(),
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
    fn new(mut voucher: VoucherBuilder, text: &'t str) -> Self {
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
        let max_line_width = (self.voucher.width as f32) - self.spacing.horz();
        let line_height = LINE_HEIGHT_FACTOR * self.font_size;

        if (max_line_width <= 0.0) || (line_height <= 0.0) {
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
                max_line_width,
                Wrap::Word,
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

        // Walk the lines to determine their maximum width.
        let mut line_width = 0.0f32;

        for line in ctx.lines[lines_range.clone()].iter_mut() {
            // The line *can* exceed our maximum width at this point:
            // - Word wrapping might have failed (e.g. no spaces).
            // - A single glyph might be wide enough to overshoot.
            // In that case, we simply truncate the line until it fits.
            // TODO: It would be nice to ellipsize :)
            while line.w > max_line_width {
                // If we fail here, the line is exceeded.
                let Some(last_glyph) = line.glyphs.pop() else {
                    break;
                };

                // Adapt the line width.
                line.w -= last_glyph.w;
            }

            line_width = line_width.max(line.w);
        }

        // Calculate the total height of the component in pixels.
        let height =
            (self.spacing.vert() + ((lines_range.len() as f32) * line_height)).ceil() as u32;

        // Push the text component to the builder.
        // It contains all info to render the lines.
        let component = Component {
            height,
            lines_range,
            offset_x: self.spacing.left,
            offset_y: self.spacing.top + self.font_size,
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
    height: u32,

    /// The range in the vector of layout lines
    lines_range: Range<usize>,

    /// The X offset of all lines to render (aka `spacing.left`)
    offset_x: f32,

    /// The Y offset of the first line to render (aka `spacing.top` + `font_size`)
    offset_y: f32,

    /// The width of a line (aka `voucher.width` - `spacing.horz()`)
    line_width: f32,

    /// The height of a line (aka `line_height` * `font_size`)
    line_height: f32,

    /// The alignment
    alignment: Alignment,
}

impl Component {
    pub fn height(&self) -> u32 {
        self.height
    }

    pub(super) fn render(&self, image: &mut GrayImage, offset_y_pix: u32, ctx: &mut Context) {
        // Obtain the bounds of the image.
        let image_width_pix = image.width() as i32;
        let image_height_pix = image.height() as i32;

        // Add the given Y offset in pixels to our calculated offset.
        let mut offset_y = (offset_y_pix as f32) + self.offset_y;

        // Walk the layout lines inside the buffer lines.
        // They must be present because we have already performed layouting.
        let layout_lines = store.buffer_lines[self.buffer_lines_range.clone()]
            .iter()
            .flat_map(|buffer_line| {
                buffer_line
                    .layout_opt()
                    .as_ref()
                    .expect("Missing layout (evicted from cache?)")
            });

        for layout_line in layout_lines {
            // Use the calculated width of the line to determine its X offset.
            // This is influenced by the alignment.
            let empty_width = self.line_width - layout_line.w.min(self.line_width);

            let offset_x_pix = (self.offset_x
                + match self.alignment {
                    Alignment::Left => 0.0,
                    Alignment::Right => empty_width,
                    Alignment::Center => empty_width / 2.0,
                })
            .round() as i32;

            // Determine the Y offset of the line.
            // That's pretty easy because line heights are constant.
            let offset_y_pix = offset_y.round() as i32;

            // Walk all the glyphs to rasterize them.
            for glyph in layout_line.glyphs.iter() {
                // Rasterize the glyph and obtain its placement.
                // This can fail if none of the swash sources we requested is available.
                // But then it probably fails for every glyph ...
                let Some(placement) = store
                    .raster_cache
                    .get_image(&mut store.font_system, glyph.cache_key)
                    .as_ref()
                    .map(|image| image.placement)
                else {
                    continue;
                };

                // Build the glyph bounding box and ensure that it is completely contained in the image.
                // This validation step allows us to omit bounds checks from the hot inner loop.
                let left_pix = offset_x_pix + glyph.x_int + placement.left;
                let right_pix = left_pix + ((placement.width as i32) - 1);
                let top_pix = offset_y_pix + glyph.y_int - placement.top;
                let bottom_pix = top_pix + ((placement.height as i32) - 1);

                // If at least one glyph does not fit, stop rendering the whole line.
                if (left_pix < 0)
                    || (right_pix >= image_width_pix)
                    || (top_pix < 0)
                    || (bottom_pix >= image_height_pix)
                {
                    return;
                }
            }

            // If the line fits, walk the glyphs again and copy their pixels into the image.
            for glyph in layout_line.glyphs.iter() {
                // Walk the pixels of the glyph.
                let base_left_pix = offset_x_pix + glyph.x_int;
                let base_bottom_pix = offset_y_pix + glyph.y_int;

                store.raster_cache.with_pixels(
                    &mut store.font_system,
                    glyph.cache_key,
                    Color::rgb(0x00, 0x00, 0x00),
                    |x, y, color| {
                        // Determine the final position of the pixel.
                        let x_pix = base_left_pix + x;
                        let y_pix = base_bottom_pix + y;

                        // Perform manual alpha blending. We blend A over B.
                        // - `alpha_a` is color.a().
                        // - `luma_a` is always 0xff (as our base color is black).
                        // - `alpha_b` is always 0xff (as our background is opaque).
                        // - `luma_b` is the existing pixel in the image.
                        // Now, the blend equation simplifies to (1 - alpha_a) * luma_b.
                        let pix = &mut image.get_pixel_mut(x_pix as u32, y_pix as u32).0;
                        let luma_b = (pix[0] as f32) / 255.0;
                        let alpha_a = (color.a() as f32) / 255.0;
                        let new_luma = (1.0 - alpha_a) * luma_b;

                        pix[0] = (new_luma * 255.0).round() as u8;
                    },
                );
            }

            // Increment the Y offset by one line height.
            offset_y += self.line_height;
        }
    }
}
