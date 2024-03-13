use std::env;

use weight_wb::ui::App;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Pars the CLI args.
    let args: Vec<String> = env::args().collect();
    let emulated_scales = args.iter().any(|c| c == "--emulated-scales");
    let dump_voucher = args.iter().any(|c| c == "--dump-voucher");

    App::run(emulated_scales, dump_voucher)
}
