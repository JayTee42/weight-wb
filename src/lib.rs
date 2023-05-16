#[macro_use]
extern crate bitflags;

/// Connect to the scales via serial port.
pub mod weight;

/// Connect to a Brother-QL label printer and print labels with it.
pub mod printer;

/// Generate vouchers and save them to images to be printed.
pub mod voucher;

/// Access the product database.
pub mod db;

/// Render the UI.
pub mod ui;
