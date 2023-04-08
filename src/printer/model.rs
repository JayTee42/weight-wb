use std::fmt::Display;

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Model {
    BrotherQL500,
    BrotherQL550,
    BrotherQL560,
    BrotherQL570,
    BrotherQL580N,
    BrotherQL600,
    BrotherQL650TD,
    BrotherQL700,
    BrotherQL1050,
    BrotherQL1060N,
}

impl Display for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Model::*;

        let model_nr = match self {
            BrotherQL500 => "500",
            BrotherQL550 => "550",
            BrotherQL560 => "560",
            BrotherQL570 => "570",
            BrotherQL580N => "580N",
            BrotherQL600 => "600",
            BrotherQL650TD => "650TD",
            BrotherQL700 => "700",
            BrotherQL1050 => "1050",
            BrotherQL1060N => "1060N",
        };

        write!(f, "Brother QL-{}", model_nr)
    }
}

impl TryFrom<u16> for Model {
    type Error = String;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        use Model::*;

        Ok(match value {
            0x2015 => BrotherQL500,
            0x2016 => BrotherQL550,
            0x2027 => BrotherQL560,
            0x2028 => BrotherQL570,
            0x2029 => BrotherQL580N,
            0x20c0 => BrotherQL600,
            0x201b => BrotherQL650TD,
            0x2042 => BrotherQL700,
            0x2020 => BrotherQL1050,
            0x202a => BrotherQL1060N,
            _ => return Err(format!("Unknown product ID: {:#06x}", value)),
        })
    }
}
