use std::path::PathBuf;

use omega_driver::{CompileOptions, check, compile};

fn main() {
    let Some(arguments) = parse_arguments() else {
        eprintln!("usage: omega [--check] [--build-dir <dir>] <root.omg>");
        std::process::exit(2);
    };

    let options = CompileOptions {
        build_dir: arguments.build_dir,
        root_path: arguments.root_path,
    };

    if arguments.check_only {
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

struct CliArguments {
    build_dir: Option<PathBuf>,
    check_only: bool,
    root_path: PathBuf,
}

fn parse_arguments() -> Option<CliArguments> {
    let mut build_dir = None;
    let mut check_only = false;
    let mut root_path = None;
    let mut arguments = std::env::args_os().skip(1);

    while let Some(argument) = arguments.next() {
        if argument == "--check" {
            check_only = true;
            continue;
        }

        if argument == "--build-dir" {
            build_dir = arguments.next().map(PathBuf::from);
            build_dir.as_ref()?;
            continue;
        }

        if root_path.is_some() {
            return None;
        }

        root_path = Some(PathBuf::from(argument));
    }

    Some(CliArguments {
        build_dir,
        check_only,
        root_path: root_path?,
    })
}
