use rusb::{DeviceHandle, GlobalContext};

/// There are different printer models with variable parameters.
mod model;
pub use model::Model;

/// Brother printer labels are standardized. To properly print them, we need layout parameters (margins etc.).
mod label;
pub use label::{Label, LabelType};

/// Search the list of available USB devices, find a Brother thermal printer, attach it and perform IO.
mod usb;
pub use usb::Error as AttachError;

/// The status response is the basic feedback method from the printer to the host.
mod status;
pub use status::{Error as StatusError, ErrorFlags as StatusErrorFlags};

/// Printing requires separate commands and the conversion of the input picture into raster lines.
mod print;
pub use print::{Error as PrintError, PrintConfig, PrintPriority};

pub struct Printer {
    handle: DeviceHandle<GlobalContext>,
    model: Model,
    in_addr: u8,
    out_addr: u8,
    serial_number: String,
    print_config: PrintConfig,
}
