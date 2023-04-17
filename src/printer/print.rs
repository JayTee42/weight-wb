use super::{Printer, StatusError, StatusErrorFlags};

use std::fmt::Display;
use std::mem;

use image::GrayImage;
use rusb::Error as USBError;

#[derive(Debug, Clone)]
pub enum Error {
    USBError(USBError),
    StatusError(StatusError),
    StatusErrorFlags(StatusErrorFlags),
    NoMedia,
    WrongImageDimensions {
        image_width: u32,
        image_height: u32,
        label_width: u32,
        label_length: Option<u32>,
    },
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Error::*;

        match self {
            USBError(inner) => write!(f, "An USB error has occurred: {}", inner),
            StatusError(inner) => write!(f, "A status request has failed: {}", inner),
            StatusErrorFlags(flags) => {
                write!(f, "The status contains some error flags: {:?}", flags)
            }
            NoMedia => write!(f, "The printer is not loaded. Please insert media."),
            WrongImageDimensions {
                image_width,
                image_height,
                label_width,
                label_length,
            } => write!(
                f,
                "The image has the wrong dimensions (expected: {}x{} pixels, got {}x{} pixels).",
                label_width,
                label_length.map_or_else(|| String::from("???"), |l| l.to_string()),
                image_width,
                image_height
            ),
        }
    }
}

impl std::error::Error for Error {}

impl From<USBError> for Error {
    fn from(value: USBError) -> Self {
        Error::USBError(value)
    }
}

impl From<StatusError> for Error {
    fn from(value: StatusError) -> Self {
        match value {
            StatusError::USBError(inner) => Error::USBError(inner),
            other => Error::StatusError(other),
        }
    }
}

bitflags! {
    struct PrintInfoFlags: u8 {
        const VALIDATE_KIND = 0b0000_0010;
        const VALIDATE_WIDTH = 0b0000_0100;
        const VALIDATE_LENGTH = 0b0000_1000;
        const PREFER_QUALITY = 0b0100_0000;
        const RECOVER = 0b1000_0000;
    }

    struct PrintModeFlags: u8 {
        const AUTO_CUT = 0b0100_0000;
    }

    struct ExpandedPrintModeFlags: u8 {
        const HIGHRES = 0b0100_0000;
        const CUT_AT_END = 0b0001_0000;
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum PrintPriority {
    Quality,
    Speed,
}

pub struct PrintConfig {
    pub priority: PrintPriority,
    pub auto_cut: bool,
    pub high_res: bool,
    pub invert: bool,
}

impl Default for PrintConfig {
    fn default() -> Self {
        Self {
            priority: PrintPriority::Quality,
            auto_cut: true,
            high_res: false,
            invert: false,
        }
    }
}

struct BitWriter<'a> {
    output: &'a mut [u8],
    bit_idx: usize,
}

impl<'a> BitWriter<'a> {
    pub fn new(output: &'a mut [u8]) -> Self {
        Self { output, bit_idx: 7 }
    }

    pub fn write_bit(&mut self, bit: bool) {
        self.output[0] |= (bit as u8) << self.bit_idx;

        if self.bit_idx == 0 {
            // Borrow checker shenanigans ...
            let output = mem::take(&mut self.output);
            self.output = &mut output[1..];

            self.bit_idx = 7;
        } else {
            self.bit_idx -= 1;
        }
    }
}

impl Printer {
    pub fn print_config(&mut self) -> &mut PrintConfig {
        &mut self.print_config
    }

    pub fn print(&self, image: &GrayImage) -> Result<(), Error> {
        // Perform a status request to obtain the current label.
        let status = self.request_status()?;

        if !status.error_flags.is_empty() {
            return Err(Error::StatusErrorFlags(status.error_flags));
        }

        // If there is no label, the printer is not loaded and we cannot print.
        let label = status.label.ok_or(Error::NoMedia)?;

        // The label tells us how many dots there are to print to.
        // High resolution simply doubles the number of dots in vertical direction.
        let label_width = label.printable_dots_width;
        let label_length =
            label
                .printable_dots_length
                .map(|l| if self.print_config.high_res { 2 * l } else { l });

        // Ensure that the image dimensions exactly match the label.
        // TODO: Should we support resizing?
        // TODO: Validate minimum / maximum for continuous labels.
        if (label_width != image.width()) || label_length.map_or(false, |l| l != image.height()) {
            return Err(Error::WrongImageDimensions {
                image_width: image.width(),
                image_height: image.height(),
                label_width,
                label_length,
            });
        }

        // Turn the printer into raster mode (not all of them need this ... ?).
        self.write(&[0x1B, 0x69, 0x61, 0x01])?;

        // Assemble the print info flags.
        let mut print_info_flags = PrintInfoFlags::VALIDATE_KIND
            | PrintInfoFlags::VALIDATE_WIDTH
            | PrintInfoFlags::VALIDATE_LENGTH
            | PrintInfoFlags::RECOVER;

        if self.print_config.priority == PrintPriority::Quality {
            print_info_flags |= PrintInfoFlags::PREFER_QUALITY;
        }

        // Provide the print info.
        let (label_ty, label_width, label_length) = label.ty.as_bytes();
        let lines_count_bytes = image.height().to_le_bytes();

        self.write(&[
            0x1b,
            0x69,
            0x7a,
            print_info_flags.bits(),
            label_ty,
            label_width,
            label_length,
            lines_count_bytes[0],
            lines_count_bytes[1],
            lines_count_bytes[2],
            lines_count_bytes[3],
            0x00, // Starting page (we only support to print one at a time).
            0x00, // Reserved
        ])?;

        // Specify the modes to use. Currently, there is only auto-cut.
        let mut mode_flags = PrintModeFlags::empty();

        if self.print_config.auto_cut {
            mode_flags |= PrintModeFlags::AUTO_CUT;
        }

        self.write(&[0x1b, 0x69, 0x4d, mode_flags.bits()])?;

        // Specify the auto-cut rate if auto-cut is enabled.
        // We hardcode 1 (aka "Cut after every page") because we only print one page at all.
        if self.print_config.auto_cut {
            self.write(&[0x1b, 0x69, 0x41, 0x01])?;
        }

        // Specify the expanded (extended?) modes.
        let mut expanded_mode_flags = ExpandedPrintModeFlags::CUT_AT_END;

        if self.print_config.high_res {
            expanded_mode_flags |= ExpandedPrintModeFlags::HIGHRES;
        }

        self.write(&[0x1b, 0x69, 0x4b, expanded_mode_flags.bits()])?;

        // Specify the feed margin.
        let feed_margin_bytes = label.margin_dots_length.to_le_bytes();
        self.write(&[0x1b, 0x69, 0x64, feed_margin_bytes[0], feed_margin_bytes[1]])?;

        // Disable compression for now.
        // TODO: Maybe support it in the future?
        self.write(&[0x4d, 0x00])?;

        // Walk the raster lines.
        let mut line_command =
            vec![0x00; 3 + (self.model.line_width() as usize)].into_boxed_slice();

        line_command[0] = 0x67;
        line_command[1] = 0x00;
        line_command[2] = self.model.line_width();

        for row in image.rows() {
            // Zero the line.
            let line = &mut line_command[3..];
            line.fill(0);

            // Write the margin.
            let mut bit_writer = BitWriter::new(line);

            for _ in 0..label.margin_dots_right {
                bit_writer.write_bit(false);
            }

            // Sample the row from back to front.
            for pix in row
                .rev()
                .map(|p| (p.0[0] < 0x80) != self.print_config.invert)
            {
                bit_writer.write_bit(pix);
            }

            // Send the line to the printer.
            self.write(&line_command)?;
        }

        // Commit the print with feeding.
        self.write(&[0x1a])?;

        Ok(())
    }
}
