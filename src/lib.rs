#[macro_use]
extern crate bitflags;

/// Connect to a Brother-QL label printer and print labels with it.
pub mod printer;

/// Generate vouchers and save them to images to be printed.
pub mod voucher;
