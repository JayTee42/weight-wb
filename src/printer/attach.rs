use super::{model::Model, Printer};

use std::fmt::Display;

/// USB Vendor ID for Brother QL printers
const VENDOR_ID: u16 = 0x04f9;

/// Some printers can be put into mass storage mode.
/// This means they get a different USB product ID and we cannot use them.
const MASS_STORAGE_PRODUCT_IDS: &[u16] = &[0x2049];

#[derive(Debug, Copy, Clone)]
pub enum Error {
    USBError(rusb::Error),
    NoPrinter,
    NoInterface,
    NoInterfaceDescriptor,
    NoInEndpoint,
    NoOutEndpoint,
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Error::*;

        match self {
            USBError(inner) => write!(f, "An USB error has occurred: {}", inner),
            NoPrinter => write!(f, "No printer has been found."),
            NoInterface => write!(f, "The USB device for the printer offers no interace."),
            NoInterfaceDescriptor => {
                write!(f, "The USB interface for the printer offers no descriptor.")
            }
            NoInEndpoint => write!(
                f,
                "The USB interface for the printer offers no bulk input endpoint."
            ),
            NoOutEndpoint => write!(
                f,
                "The USB interface for the printer offers no bulk output endpoint."
            ),
        }
    }
}

impl std::error::Error for Error {}

impl From<rusb::Error> for Error {
    fn from(err: rusb::Error) -> Self {
        Error::USBError(err)
    }
}

fn select_device<F>(
    mut f: F,
) -> Result<
    Option<(
        Model,
        rusb::Device<rusb::GlobalContext>,
        rusb::DeviceDescriptor,
    )>,
    rusb::Error,
>
where
    F: FnMut(Model, &rusb::DeviceDescriptor) -> bool,
{
    Ok(rusb::DeviceList::new()?
        .iter()
        .find_map(|device| {
            // Obtain the device descriptor.
            // Skip devices where this step fails.
            let device_desc = device.device_descriptor().ok()?;

            // Skip devices that don't match the vendor ID.
            if device_desc.vendor_id() != VENDOR_ID {
                return None;
            }

            // Try to select a model from the product ID.
            // Skip the printer if that fails, but log it to the console.
            let product_id = device_desc.product_id();

            let Ok(model) = Model::try_from(product_id) else {
                // Some printers allow to select mass storage modes that we don't support.
                if MASS_STORAGE_PRODUCT_IDS.contains(&product_id) {
                    eprintln!("Found a Brother QL printer in mass storage mode. Please switch modes to select it.");
                } else {
                    eprintln!("Found a Brother QL printer with an unknown product ID: {:#06x}", product_id);
                }

                return None;
            };

            // Evaluate the user-defined predicate.
            f(model, &device_desc).then_some((model, device, device_desc))
        }))
}

fn select_interface(device: &rusb::Device<rusb::GlobalContext>) -> Result<(u8, u8, u8), Error> {
    // Query the interface from the device. There should be exactly one.
    let config_desc = device.active_config_descriptor()?;
    let interface = config_desc.interfaces().next().ok_or(Error::NoInterface)?;

    // That interface should again have exactly one descriptor.
    let interface_desc = interface
        .descriptors()
        .next()
        .ok_or(Error::NoInterfaceDescriptor)?;

    // Walk the endpoints provided by the interface to find input and output.
    // We are only interested in bulk endpoints.
    let (mut in_addr, mut out_addr) = (None, None);

    for endpoint_desc in interface_desc
        .endpoint_descriptors()
        .filter(|desc| desc.transfer_type() == rusb::TransferType::Bulk)
    {
        match endpoint_desc.direction() {
            rusb::Direction::In => in_addr = Some(endpoint_desc.address()),
            rusb::Direction::Out => out_addr = Some(endpoint_desc.address()),
        }
    }

    // Return the interface number and the endpoint addresses if found.
    Ok((
        interface.number(),
        in_addr.ok_or(Error::NoInEndpoint)?,
        out_addr.ok_or(Error::NoOutEndpoint)?,
    ))
}

impl Printer {
    /// Try to find and attach a Brother QL printer.
    /// If `filter_model` is given, we search for the first printer of this model.
    /// Otherwise, the first printer at all is returned.
    pub fn attach(model_filter: Option<Model>) -> Result<Self, Error> {
        // Try to select a device.
        let (model, device, device_desc) =
            select_device(|m1, _| model_filter.map_or(true, |m2| m1 == m2))?
                .ok_or(Error::NoPrinter)?;

        // Try to open the USB device, giving us a handle.
        // Ensure that a potential kernel driver is automatically detached and later reattached.
        let mut handle = device.open()?;
        handle.set_auto_detach_kernel_driver(true)?;

        // Select the correct interface for the printer.
        let (interface_number, in_addr, out_addr) = select_interface(&device)?;

        // Claim the interface.
        handle.claim_interface(interface_number)?;

        // Read some meta info from the device descriptor.
        let serial_number = handle.read_serial_number_string_ascii(&device_desc)?;

        // Populate the printer struct.
        let printer = Printer {
            handle,
            model,
            in_addr,
            out_addr,
            serial_number,
        };

        // Clear outstanding jobs by sending a bunch of "invalid" commands.
        // Then initialize the printer.
        printer.write(&[0x00; 350])?;
        printer.write(&[0x1b, 0x40])?;

        Ok(printer)
    }
}
