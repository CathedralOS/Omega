//! Ad-hoc interpreter runner for divergence hunts: `interp_run <root.omg>` reads stdin,
//! selects the host target's authored `ProgramEntry`, interprets it, forwards its
//! stdout/stderr, and exits with its exit code.

use std::io::{Read, Write};
use std::path::Path;

use omega_compiler::compile_to_checked;
use psi_checked_interpreter::interpret_entry;

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: interp_run <root.omg>");
    let mut stdin = Vec::new();
    std::io::stdin()
        .read_to_end(&mut stdin)
        .expect("read stdin");

    let target = omega_target::TargetProfile::host().target_name();
    let checked =
        compile_to_checked(Path::new(&path), Some(target)).unwrap_or_else(|diagnostics| {
            for diagnostic in &diagnostics {
                eprintln!("{diagnostic}");
            }
            std::process::exit(102);
        });

    let entry = checked.selected_program_entry_machine().unwrap_or_else(|| {
        eprintln!("build has no exact target-owned ProgramEntry binding");
        std::process::exit(103);
    });
    let outcome = interpret_entry(&checked, entry, &stdin);
    std::io::stdout()
        .write_all(&outcome.stdout)
        .expect("stdout");
    std::io::stderr()
        .write_all(&outcome.stderr)
        .expect("stderr");
    if outcome.is_error() {
        eprintln!("interpreter error: {:?}", outcome.error);
        std::process::exit(101);
    }
    std::process::exit(outcome.exit_code);
}
