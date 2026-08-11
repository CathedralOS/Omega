//! Dev probe harness: compile a `.omg` NATIVELY, run it, and report the exit
//! code -- and (`--both`) interpret the same program and report agreement.
//! This is the one-shot loop the canary/probe workflow repeats constantly
//! (scratchpad probe -> native exit vs interp exit) without needing a
//! throwaway Rust test each time.
//!
//!   cargo run -q -p omega-compiler --bin omega-run -- path/to/main.omg
//!   cargo run -q -p omega-compiler --bin omega-run -- --both path/to/main.omg
//!
//! Exit code: the PROBE's native exit code (so shell `$?` composes), 200 on
//! compile failure, 201 on native/interp disagreement under `--both`.

use omega_compiler::{CompileOptions, compile, compile_to_checked};
use std::process::Command;

fn main() {
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    let both = args.iter().any(|a| a == "--both");
    let keep = args.iter().any(|a| a == "--keep");
    args.retain(|a| a != "--both" && a != "--keep");
    // `--target <name>` cross-compiles for a registered target (e.g.
    // uefi_x64) and reports COMPILE success/refusal without running -- the
    // M2/platform-session check loop.
    let target_name = args.iter().position(|a| a == "--target").map(|index| {
        let name = args.get(index + 1).cloned().unwrap_or_else(|| {
            eprintln!("usage: omega-run --target <name> <main.omg>");
            std::process::exit(2);
        });
        args.drain(index..=index + 1);
        name
    });
    let Some(main_path) = args.first() else {
        eprintln!("usage: omega-run [--both] [--target <name>] <main.omg>");
        std::process::exit(2);
    };
    let main_path = std::path::PathBuf::from(main_path);

    let build_dir = std::env::temp_dir().join(format!("omega-run-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);

    if let Err(diagnostics) = compile(CompileOptions {
        root_path: main_path.clone(),
        build_dir: Some(build_dir.clone()),
        target_name: target_name.clone(),
        write_output: true,
    }) {
        eprintln!("native compile FAILED:");
        for diagnostic in diagnostics {
            eprintln!("  {diagnostic}");
        }
        std::process::exit(200);
    }
    if let Some(target) = &target_name {
        // Cross-target images do not run on the host; compiling IS the check.
        eprintln!(
            "compiled for target `{target}` OK ({})",
            build_dir.display()
        );
        if !keep {
            let _ = std::fs::remove_dir_all(&build_dir);
        }
        return;
    }

    let exe = build_dir.join(if cfg!(windows) {
        "omega-program.exe"
    } else {
        "omega-program"
    });
    let output = Command::new(&exe)
        .output()
        .unwrap_or_else(|error| panic!("native run failed to spawn {}: {error}", exe.display()));
    let native_code = output.status.code().unwrap_or(-1);
    print!("{}", String::from_utf8_lossy(&output.stdout));
    eprint!("{}", String::from_utf8_lossy(&output.stderr));
    eprintln!("native exit: {native_code}");

    if both {
        match compile_to_checked(&main_path, None) {
            Ok(checked) => {
                let outcome = psi_checked_interpreter::interpret_entry(
                    &checked,
                    checked.program_entry_machine(),
                    &[],
                );
                if let Some(reason) = &outcome.error {
                    eprintln!("interp: DECLINED ({reason})");
                } else {
                    eprintln!("interp exit: {}", outcome.exit_code);
                    if outcome.exit_code != native_code {
                        eprintln!(
                            "DIVERGENCE: native {native_code} vs interp {}",
                            outcome.exit_code
                        );
                        let _ = std::fs::remove_dir_all(&build_dir);
                        std::process::exit(201);
                    }
                }
            }
            Err(diagnostics) => {
                eprintln!("interp frontend compile FAILED:");
                for diagnostic in diagnostics {
                    eprintln!("  {diagnostic}");
                }
            }
        }
    }

    if keep {
        eprintln!("build dir kept: {}", build_dir.display());
    } else {
        let _ = std::fs::remove_dir_all(&build_dir);
    }
    std::process::exit(native_code);
}
