use super::{Alignment, Builder as VoucherBuilder, Component as VoucherComponent, Spacing};

use std::ops::Range;

use cosmic_text::{
    Attrs, AttrsList, BufferLine, Color, Family, FontSystem, Style, SwashCache as RasterizerCache,
    Weight, Wrap,
};
use image::GrayImage;

pub(super) struct Store {
    font_system: FontSystem,
    text_lines: Vec<String>,
    buffer_lines: Vec<BufferLine>,
    rasterizer_cache: RasterizerCache,
}

impl Store {
    pub fn new() -> Self {
        Self {
            font_system: FontSystem::new(),
            text_lines: Vec::new(),
            buffer_lines: Vec::new(),
            rasterizer_cache: RasterizerCache::new(),
        }
    }
}

pub struct Builder {
    /// The underlying voucher builder
    voucher: VoucherBuilder,

    /// The spacing to apply to this component
    spacing: Spacing,

    /// The alignment to apply to this component
    alignment: Alignment,

    /// The name of the font family
    font_family: Option<String>,

    /// The font size (pixels)
    font_size: f32,

    /// The line height, as defined in CSS (aka a factor that is multiplied on top of the font size).
    /// Typical values are between 1.0 and 1.3.
    line_height: f32,

    /// Do we render bold text?
    bold: bool,

    /// Do we render italic text?
    italic: bool,
}

impl Builder {
    fn new(mut voucher: VoucherBuilder, text: &str) -> Self {
        // Break the text into lines and store them temporarily.
        // We do not render from this data!
        // The vector in the cache just prevents some additional memory allocations.
        voucher.text_store.text_lines.clear();

        voucher
            .text_store
            .text_lines
            .extend(text.lines().map(String::from));

        Self {
            voucher,
            spacing: Default::default(),
            alignment: Alignment::Left,
            font_family: None,
            font_size: 12.0,
            line_height: 1.3,
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

    pub fn font_family<S: ToString>(mut self, font_family: S) -> Self {
        self.font_family = Some(font_family.to_string());
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

    pub fn line_height(mut self, line_height: f32) -> Self {
        assert!(line_height >= 0.0, "Line height must be non-negative.");
        self.line_height = line_height;

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
        // Return early if there is no text.
        if self.voucher.text_store.text_lines.is_empty() {
            return self.voucher;
        }

        // Calculate the available line width and line height.
        // If one of them is degenerated, we return early.
        let line_width = (self.voucher.width as f32) - self.spacing.horz();
        let line_height = self.line_height * self.font_size;

        if (line_width <= 0.0) || (line_height <= 0.0) {
            return self.voucher;
        }

        // Build the attributes.
        let attrs = Attrs::new()
            .family(if let Some(font_family) = self.font_family.as_ref() {
                Family::Name(font_family)
            } else {
                Family::SansSerif
            })
            .weight(if self.bold {
                Weight::BOLD
            } else {
                Weight::NORMAL
            })
            .style(if self.italic {
                Style::Italic
            } else {
                Style::Normal
            });

        // Walk the lines of the component.
        let buffer_lines_offset = self.voucher.text_store.buffer_lines.len();
        let buffer_lines_count = self.voucher.text_store.text_lines.len();
        let mut layout_lines_count = 0;

        for line in self.voucher.text_store.text_lines.drain(..) {
            // Perform the shaping and layout process on the line.
            // Count how many layout lines we receive.
            // They are stored inside a vector that is owned by the buffer line.
            // Therefore, we don't have to layout again when it's rendering time.
            let mut buffer_line = BufferLine::new(line, AttrsList::new(attrs));

            layout_lines_count += buffer_line
                .layout(
                    &mut self.voucher.text_store.font_system,
                    self.font_size,
                    line_width,
                    Wrap::Word,
                )
                .len();

            self.voucher.text_store.buffer_lines.push(buffer_line);
        }

        // Push the text component to the builder.
        // It contains all info to render the lines.
        let component = Component {
            buffer_lines_range: buffer_lines_offset..(buffer_lines_offset + buffer_lines_count),
            layout_lines_count,
            offset_x: self.spacing.left,
            offset_y: self.spacing.top + self.font_size,
            line_width,
            line_height,
            leading: (self.line_height - 1.0) * self.font_size,
            vert_spacing: self.spacing.vert(),
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
    /// The range in the vector of buffer lines
    buffer_lines_range: Range<usize>,

    /// The number of layout lines that is produced by the buffer lines
    layout_lines_count: usize,

    /// The X offset of all lines to render (aka `spacing.left`)
    offset_x: f32,

    /// The Y offset of the first line to render (aka `spacing.top` + `font_size`)
    offset_y: f32,

    /// The width of a line (aka `voucher.width` - `spacing.horz()`)
    line_width: f32,

    /// The height of a line (aka `line_height` * `font_size`)
    line_height: f32,

    /// The leading (aka `line_height - 1.0` * `font_size`)
    leading: f32,

    /// The vertical spacing
    vert_spacing: f32,

    /// The alignment
    alignment: Alignment,
}

impl Component {
    pub fn height(&self) -> u32 {
        // WIP: Account for Cosmic-Text #123 by incorporating one half of a line height.
        // This will hopefully be fixed in the future!
        // See here:
        // https://github.com/pop-os/cosmic-text/issues/123
        let fix_me_cosmic_text = 0.5 * self.line_height;

        (self.vert_spacing + ((self.layout_lines_count as f32) * self.line_height) - self.leading
            + fix_me_cosmic_text)
            .ceil() as _
    }

    pub(super) fn render(&self, image: &mut GrayImage, offset_y_pix: u32, store: &mut Store) {
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
                        .rasterizer_cache
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

                store.rasterizer_cache.with_pixels(
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
