use super::{Label, LabelType, Printer};

use std::fmt::Display;

use rusb::Error as USBError;

#[derive(Debug, Clone)]
pub enum Error {
    USBError(USBError),
    WrongResponseSizeUSB(usize),
    WrongPrintHeadMark(u8),
    WrongResponseSizeHeader(u8),
    InvalidLabel(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Error::*;

        match self {
            USBError(inner) => write!(f, "An USB error has occurred: {}", inner),
            WrongResponseSizeUSB(len) => write!(f, "The status response from the printer has the wrong size at USB level ({} instead of 32 bytes).", len),
            WrongPrintHeadMark(mark) => write!(f, "The status response from the printer has the wrong print head mark ({:#04x} instead of 0x80).", mark),
            WrongResponseSizeHeader(len) => write!(f, "The status response from the printer has the wrong size at header level ({} instead of 32 bytes).", len),
            InvalidLabel(inner) => write!(f, "The label is invalid: {}", inner),
        }
    }
}

impl std::error::Error for Error {}

impl From<USBError> for Error {
    fn from(value: USBError) -> Self {
        Error::USBError(value)
    }
}

bitflags! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub struct ErrorFlags: u16 {
        // Error info 1 (LSB)
        const NO_MEDIA = 0b0000_0000_0000_0001;
        const END_OF_MEDIA = 0b0000_0000_0000_0010;
        const TAPE_CUTTER_JAM = 0b0000_0000_0000_0100;
        const MAIN_UNIT_IN_USE = 0b0000_0000_0001_0000;
        const TURNED_OFF = 0b0000_0000_0010_0000;
        const HIGH_VOLTAGE_ADAPTER = 0b0000_0000_0100_0000;
        const FAN_NOT_WORKING = 0b0000_0000_1000_0000;

        // Error info 2 (MSB)
        const REPLACE_MEDIA_ERROR = 0b0000_0001_0000_0000;
        const EXPANSION_BUFFER_FULL = 0b0000_0010_0000_0000;
        const TRANSMISSION_ERROR = 0b0000_0100_0000_0000;
        const COMMUNICATION_BUFFER_FULL = 0b0000_1000_0000_0000;
        const COVER_OPEN = 0b0001_0000_0000_0000;
        const CANCEL_KEY = 0b0010_0000_0000_0000;
        const CANNOT_FEED = 0b0100_0000_0000_0000;
        const SYSTEM_ERROR = 0b1000_0000_0000_0000;
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub(super) enum StatusType {
    StatusReply,
    PrintingCompleted,
    ErrorOccurred,
    Notification,
    PhaseChange,
    Unknown(u8),
}

impl From<u8> for StatusType {
    fn from(value: u8) -> Self {
        use StatusType::*;

        match value {
            0x00 => StatusReply,
            0x01 => PrintingCompleted,
            0x02 => ErrorOccurred,
            0x05 => Notification,
            0x06 => PhaseChange,
            other => Unknown(other),
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub(super) enum PhaseType {
    Waiting,
    Printing,
    Unknown(u8),
}

impl From<u8> for PhaseType {
    fn from(value: u8) -> Self {
        use PhaseType::*;

        match value {
            0x00 => Waiting,
            0x01 => Printing,
            other => Unknown(other),
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub(super) enum Notification {
    CoolingStart,
    CoolingFinish,
    Unknown(u8),
}

impl Notification {
    fn from_byte(value: u8) -> Option<Self> {
        use Notification::*;

        match value {
            0x00 => None,
            0x03 => Some(CoolingStart),
            0x04 => Some(CoolingFinish),
            other => Some(Unknown(other)),
        }
    }
}

#[allow(dead_code)]
pub(super) struct Status {
    pub error_flags: ErrorFlags,
    pub label: Option<Label>,
    pub status_type: StatusType,
    pub phase_type: PhaseType,
    pub notification: Option<Notification>,
}

impl Printer {
    pub(super) fn request_status(&self) -> Result<Status, Error> {
        self.write(&[0x1b, 0x69, 0x53])?;
        self.read_status_response()
    }

    pub(super) fn read_status_response(&self) -> Result<Status, Error> {
        // Read the status data. It has always 32 bytes.
        let mut data = [0u8; 32];
        let read_bytes = self.read(&mut data)?;

        if read_bytes != 32 {
            return Err(Error::WrongResponseSizeUSB(read_bytes));
        }

        // Check head mark and length.
        if data[0] != 0x80 {
            return Err(Error::WrongPrintHeadMark(data[0]));
        }

        if data[1] != 0x20 {
            return Err(Error::WrongResponseSizeHeader(data[1]));
        }

        // Extract the label type and map it to a label.
        let label = if let Some(ty) = LabelType::from_bytes(data[11], data[10], data[17]) {
            Some(Label::try_from((self.model, ty)).map_err(Error::InvalidLabel)?)
        } else {
            None
        };

        // Assemble the status.
        Ok(Status {
            error_flags: ErrorFlags::from_bits_truncate(u16::from_le_bytes([data[8], data[9]])),
            label,
            status_type: StatusType::from(data[18]),
            phase_type: PhaseType::from(data[19]),
            notification: Notification::from_byte(data[22]),
        })
    }
}
