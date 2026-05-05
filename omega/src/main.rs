use std::path::PathBuf;

use omega::driver::{CompileOptions, check, compile};

fn main() {
    let mut arguments = std::env::args_os().skip(1);
    let check_only = matches!(arguments.next().as_deref(), Some(flag) if flag == "--check");
    let root_path = if check_only {
        arguments.next()
    } else {
        std::env::args_os().nth(1)
    };

    let Some(root_path) = root_path else {
        eprintln!("usage: omega [--check] <root.omg>");
        std::process::exit(2);
    };

    let options = CompileOptions {
        root_path: PathBuf::from(root_path),
    };

    if check_only {
        match check(options) {
            Ok(output) => {
                println!("{}", output.summary);
            }
            Err(diagnostics) => {
                for diagnostic in diagnostics {
                    eprintln!("{diagnostic}");
                }

                std::process::exit(1);
            }
        }
    } else {
        match compile(options) {
            Ok(output) => {
                println!("{}", output.summary);
            }
            Err(diagnostics) => {
                for diagnostic in diagnostics {
                    eprintln!("{diagnostic}");
                }

                std::process::exit(1);
            }
        }
    };
}
