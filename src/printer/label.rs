use super::{Model, Printer, StatusError};

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum LabelType {
    Continuous { width: u8 },
    DieCut { width: u8, length: u8 },
}

impl LabelType {
    pub(super) fn from_bytes(ty: u8, width: u8, length: u8) -> Option<Self> {
        use LabelType::*;

        match ty {
            0x0a => {
                if length != 0 {
                    eprintln!(
                        "Length for continuous label should be 0 mm, but is {} mm.",
                        length
                    );
                }

                Some(Continuous { width })
            }

            0x0b => Some(DieCut { width, length }),

            other => {
                if other != 0x00 {
                    eprintln!("Unknown label type: {:#04x}", other);
                }

                None
            }
        }
    }

    pub(super) fn as_bytes(&self) -> (u8, u8, u8) {
        use LabelType::*;

        match *self {
            Continuous { width } => (0x0a, width, 0x00),
            DieCut { width, length } => (0x0b, width, length),
        }
    }
}

#[derive(Copy, Clone)]
pub struct Label {
    pub ty: LabelType,
    pub printable_dots_width: u32,
    pub printable_dots_length: Option<u32>,
    pub(super) margin_dots_right: u32,
    pub(super) margin_dots_length: u16,
}

impl TryFrom<(Model, LabelType)> for Label {
    type Error = String;

    fn try_from((model, ty): (Model, LabelType)) -> Result<Self, Self::Error> {
        use LabelType::*;

        // Printable dots: Columns 3 and 4 from tables in section 3.2.2.
        // Margin dots: Columns "Number of Pins for Right Margin" from tables in section 3.2.5.
        // Margin dots: Table in section 3.2.3. (aka feed margin)
        let (printable_dots_width, printable_dots_length);
        let margin_dots_right;
        let margin_dots_length;

        // "Wide" printers have more pins and therefore require different margins.
        let is_wide = [Model::BrotherQL1050, Model::BrotherQL1060N].contains(&model);

        match ty {
            Continuous { width } => {
                printable_dots_length = None;
                margin_dots_length = 35;

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
                margin_dots_length = 0;

                // TODO: Dia (no idea about that, hehe ...)
                (
                    printable_dots_width,
                    printable_dots_length,
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
            ty,
            printable_dots_width,
            printable_dots_length,
            margin_dots_right,
            margin_dots_length,
        })
    }
}

impl Printer {
    pub fn current_label(&self) -> Result<Option<Label>, StatusError> {
        Ok(self.request_status()?.label)
    }
}
