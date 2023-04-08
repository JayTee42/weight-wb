use super::{status::Media, Model};

pub struct Label {
    printable_dots_width: u32,
    printable_dots_height: Option<u32>,
    margin_dots_right: u32,
    feed_dots_height: u32,
}

impl TryFrom<(Model, Media)> for Label {
    type Error = String;

    fn try_from((model, media): (Model, Media)) -> Result<Self, Self::Error> {
        use Media::*;

        // Printable dots: Columns 3 and 4 from tables in section 3.2.2.
        // Margin dots: Columns "Number of Pins for Right Margin" from tables in section 3.2.5.
        // Feed dots: Table in section 3.2.3.
        let (printable_dots_width, printable_dots_height);
        let margin_dots_right;
        let feed_dots_height;

        // "Wide" printers have more pins and therefore require different margins.
        let is_wide = [Model::BrotherQL1050, Model::BrotherQL1060N].contains(&model);

        match media {
            Continuous { width } => {
                printable_dots_height = None;
                feed_dots_height = 35;

                (printable_dots_width, margin_dots_right) = match width {
                    12 => (106, if is_wide { 74 } else { 29 }),
                    29 => (306, if is_wide { 50 } else { 6 }),
                    38 => (413, if is_wide { 56 } else { 12 }),
                    50 => (554, if is_wide { 56 } else { 12 }),
                    54 => (590, if is_wide { 44 } else { 0 }),
                    62 => (696, if is_wide { 56 } else { 12 }),
                    102 => (1164, 56), // Only wide

                    _ => {
                        return Err(format!(
                            "Unknown continuous label type (width: {} mm)",
                            width
                        ))
                    }
                };
            }

            DieCut { width, length } => {
                feed_dots_height = 0;

                // TODO: Dia (no idea about that, hehe ...)
                (
                    printable_dots_width,
                    printable_dots_height,
                    margin_dots_right,
                ) = match (width, length) {
                    (17, 54) => (165, Some(566), if is_wide { 44 } else { 0 }),
                    (17, 87) => (165, Some(956), if is_wide { 44 } else { 0 }),
                    (23, 23) => (236, Some(202), if is_wide { 84 } else { 42 }),
                    (29, 90) => (306, Some(991), if is_wide { 50 } else { 6 }),
                    (38, 90) => (413, Some(991), if is_wide { 56 } else { 12 }),
                    (39, 48) => (425, Some(495), if is_wide { 50 } else { 6 }),
                    (52, 29) => (578, Some(271), if is_wide { 44 } else { 0 }),
                    (62, 29) => (696, Some(271), if is_wide { 56 } else { 12 }),
                    (62, 100) => (696, Some(1109), if is_wide { 56 } else { 12 }),
                    (102, 51) => (1164, Some(526), 56), // Only wide
                    (102, 152) => (1164, Some(1660), 56), // Only wide

                    _ => {
                        return Err(format!(
                            "Unknown die-cut label type (dimensions: {}x{} mm)",
                            width, length
                        ))
                    }
                }
            }
        }

        Ok(Label {
            printable_dots_width,
            printable_dots_height,
            margin_dots_right,
            feed_dots_height,
        })
    }
}
