use std::path::PathBuf;

use omega::driver::{CompileOptions, compile};

fn main() {
    let Some(root_path) = std::env::args_os().nth(1) else {
        eprintln!("usage: omega <root.omg>");
        std::process::exit(2);
    };

    let options = CompileOptions {
        root_path: PathBuf::from(root_path),
    };

    match compile(options) {
        Ok(output) => {
            println!("{}", output.summary);
        }
        Err(diagnostics) => {
            for diagnostic in diagnostics {
                eprintln!("{}", diagnostic.message);
            }

            std::process::exit(1);
        }
    }
}
