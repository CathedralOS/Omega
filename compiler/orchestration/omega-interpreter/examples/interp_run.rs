//! Ad-hoc interpreter runner for divergence hunts: `interp_run <root.omg>` reads stdin,
//! interprets the program, forwards its stdout/stderr, and exits with its exit code.

use std::io::{Read, Write};
use std::path::Path;

use omega_compiler::compile_to_checked;
use omega_interpreter::interpret;

fn main() {
    let path = std::env::args().nth(1).expect("usage: interp_run <root.omg>");
    let mut stdin = Vec::new();
    std::io::stdin().read_to_end(&mut stdin).expect("read stdin");

    let checked = compile_to_checked(Path::new(&path), None).unwrap_or_else(|diagnostics| {
        for diagnostic in &diagnostics {
            eprintln!("{diagnostic}");
        }
        std::process::exit(102);
    });

    let outcome = interpret(&checked, &stdin);
    std::io::stdout().write_all(&outcome.stdout).expect("stdout");
    std::io::stderr().write_all(&outcome.stderr).expect("stderr");
    if outcome.is_error() {
        eprintln!("interpreter error: {:?}", outcome.error);
        std::process::exit(101);
    }
    std::process::exit(outcome.exit_code);
}
