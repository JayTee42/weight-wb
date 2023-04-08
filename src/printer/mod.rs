mod attach;

mod io;

mod label;
use label::Label;

mod model;
pub use model::Model;

mod status;
use status::Status;

pub struct Printer {
    handle: rusb::DeviceHandle<rusb::GlobalContext>,
    model: Model,
    in_addr: u8,
    out_addr: u8,
    serial_number: String,
}
