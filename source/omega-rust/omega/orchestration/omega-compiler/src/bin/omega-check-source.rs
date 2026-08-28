//! Checked-frontend validation for one or more ordinary Omega source roots.
//!
//! This deliberately stops after checked trees: profile canaries prove that
//! facilities unused by the compiler closure remain valid full-Omega source without paying for
//! native lowering or publishing build artifacts.

use omega_compiler::compile_to_checked;
use std::path::PathBuf;

fn main() {
    let mut target = None;
    let mut sources = Vec::new();
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == "--target" {
            let Some(value) = arguments.next() else {
                usage();
            };
            if target.replace(value).is_some() {
                usage();
            }
        } else {
            sources.push(PathBuf::from(argument));
        }
    }
    if sources.is_empty() {
        usage();
    }

    let mut failed = false;
    for source in sources {
        match compile_to_checked(&source, target.as_deref()) {
            Ok(_) => println!("checked {}", source.display()),
            Err(diagnostics) => {
                failed = true;
                eprintln!("checked-source validation failed for {}:", source.display());
                for diagnostic in diagnostics {
                    eprintln!("  {diagnostic}");
                }
            }
        }
    }
    if failed {
        std::process::exit(1);
    }
}

fn usage() -> ! {
    eprintln!("usage: omega-check-source [--target <name>] <root.omg> [<root.omg> ...]");
    std::process::exit(2);
}
