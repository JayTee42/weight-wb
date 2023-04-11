use super::Layout;

bitflags! {
    /// The fonts for labels can be enriched with different styles.
    /// Those can even be combined.
    #[derive(Copy, Clone)]
    pub struct FontStyle: u8 {
        const BOLD = 0b0000_0001;
        const ITALIC = 0b0000_0010;
    }
}

pub struct Cache {
    current_glyphs: Vec<(rusttype::GlyphId, f32)>,
    glyphs: Vec<(rusttype::GlyphId, f32)>,
    lines: Vec<(usize, f32)>,
}

struct Context<'cache, 'font> {
    current_glyphs: &'cache mut Vec<(rusttype::GlyphId, f32)>,
    glyphs: &'cache mut Vec<(rusttype::GlyphId, f32)>,
    lines: &'cache mut Vec<(usize, f32)>,
    font: &'font rusttype::Font<'font>,
}

impl<'cache, 'font> Context<'cache, 'font> {
    fn new(cache: &'cache mut Cache, font: &'font rusttype::Font) -> Self {
        Self {
            current_glyphs: &mut cache.current_glyphs,
            glyphs: &mut cache.glyphs,
            lines: &mut cache.lines,
            font,
        }
    }

    fn push_char(&mut self, c: char) {}
}

pub fn append_text_component(
    layout: Layout,
    cache: &mut Cache,
    text: &str,
    font: &rusttype::Font,
    font_size: f32,
) {
    // Create a new context that leverages the cache.
    let ctx = Context::new(cache, font);

    // Initialize the font parameters.
    // We do uniform scaling right now.
    // TODO: For high-resolution prints, this would be 1:2 ...
    let scale = rusttype::Scale {
        x: font_size,
        y: font_size,
    };

    let v_metrics = font.v_metrics(scale);

    let start = rusttype::Point {
        x: 0.0,
        y: v_metrics.ascent,
    };

    // Create an iterator to break the text in runs.
    // Between runs, there are potential and mandatory line breaks.
    let runs = xi_unicode::LineBreakIterator::new(text).scan(
        0,
        |start_idx, (end_idx, is_mandatory_break)| {
            let run = &text[*start_idx..end_idx];
            *start_idx = end_idx + 1;

            Some((run, is_mandatory_break))
        },
    );

    for (run, is_mandatory_break) in runs {
        // Fetch glyphs for the run and scale them.
        let mut last_glyph: Option<rusttype::ScaledGlyph> = None;
        let mut x = 0.0;

        for glyph in font.glyphs_for(run.chars()).map(|g| g.scaled(scale)) {
            // Add the kerning distance to the horizontal offset.
            if let Some(last_glyph) = last_glyph {
                x += font.pair_kerning(scale, last_glyph.id(), glyph.id());
            }

            // Position the glyph and add its width the horizontal offset.
            cache.current_glyphs.push((glyph.id(), x));
            x += glyph.h_metrics().advance_width;

            // Remember the glyph for the next iteration so we can respect kerning.
            last_glyph = Some(glyph);
        }
    }
}
