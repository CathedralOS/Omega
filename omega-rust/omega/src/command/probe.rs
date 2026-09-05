//! Dev probe harness: compile a `.omg` NATIVELY, run it, and report the exit
//! code -- and (`--both`) interpret the same program and report agreement.
//! This is the one-shot loop the canary/probe workflow repeats constantly
//! (scratchpad probe -> native exit vs interp exit) without needing a
//! throwaway Rust test each time.
//!
//!   omega run path/to/main.omg
//!   omega run --both path/to/main.omg
//!   omega run --keep path/to/main.omg
//!
//! Ordinary probes emit only the executable because their temporary build
//! directory is deleted immediately. `--keep` retains the full compiler report
//! and visualization set for inspection.
//!
//! Exit code: the PROBE's native exit code (so shell `$?` composes), 200 on
//! compile failure, 201 on native/interp disagreement under `--both`.

use compiler::{
    ArtifactEmissionPolicy, CompileOptions, CompileRequest, compile, compile_to_checked,
};
use std::process::Command;

pub(super) fn run(arguments: impl Iterator<Item = std::ffi::OsString>) -> ! {
    let ProbeArguments {
        both,
        keep,
        target_name,
        main_path,
    } = parse_arguments(arguments).unwrap_or_else(|error| {
        eprintln!("{error}");
        eprintln!("usage: omega run [--both] [--keep] [--target <name>] <main.omg>");
        std::process::exit(2);
    });

    let build_dir = std::env::temp_dir().join(format!("omega-probe-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&build_dir);

    let artifact_policy = probe_artifact_policy(keep);
    let report = match compile(
        CompileRequest::new(CompileOptions {
            root_path: main_path.clone(),
            build_dir: Some(build_dir.clone()),
            target_name: target_name.clone(),
        })
        .with_requested_product(compiler::RequestedCompileProduct::NativeArtifact)
        .with_artifact_policy(artifact_policy),
    ) {
        Ok(report) => report,
        Err(diagnostics) => {
            eprintln!("native compile FAILED:");
            for diagnostic in diagnostics {
                eprintln!("  {diagnostic}");
            }
            std::process::exit(200);
        }
    };
    let exe = match super::output::publish_native_artifact(report, &build_dir) {
        Ok((_published, path)) => path,
        Err(error) => {
            eprintln!("native publication FAILED: {error}");
            std::process::exit(200);
        }
    };
    if let Some(target) = &target_name {
        // Cross-target images do not run on the host; compiling IS the check.
        eprintln!(
            "compiled for target `{target}` OK ({})",
            build_dir.display()
        );
        if !keep {
            let _ = std::fs::remove_dir_all(&build_dir);
        }
        std::process::exit(0);
    }

    let output = Command::new(&exe)
        .output()
        .unwrap_or_else(|error| panic!("native run failed to spawn {}: {error}", exe.display()));
    let native_code = output.status.code().unwrap_or(-1);
    print!("{}", String::from_utf8_lossy(&output.stdout));
    eprint!("{}", String::from_utf8_lossy(&output.stderr));
    eprintln!("native exit: {native_code}");

    if both {
        let selected_target = target_name
            .as_deref()
            .unwrap_or_else(|| target::TargetProfile::host().target_name());
        match compile_to_checked(&main_path, Some(selected_target)) {
            Ok(checked) => {
                let Some(entry) = checked.selected_program_entry_machine() else {
                    eprintln!(
                        "interp: DECLINED (build has no exact target-owned ProgramEntry binding)"
                    );
                    if !keep {
                        let _ = std::fs::remove_dir_all(&build_dir);
                    }
                    std::process::exit(201);
                };
                let outcome = checked_interpreter::interpret_entry(&checked, entry, &[]);
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

struct ProbeArguments {
    both: bool,
    keep: bool,
    target_name: Option<String>,
    main_path: std::path::PathBuf,
}

fn parse_arguments(
    arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Result<ProbeArguments, String> {
    let mut arguments = arguments
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    // Probes bypass package preparation. Reject this flag before extracting
    // values or positionals so it cannot disappear among ignored arguments.
    if arguments.iter().any(|argument| argument == "--offline") {
        return Err("omega run does not support --offline".to_owned());
    }
    let both = arguments.iter().any(|argument| argument == "--both");
    let keep = arguments.iter().any(|argument| argument == "--keep");
    arguments.retain(|argument| argument != "--both" && argument != "--keep");
    let target_name =
        if let Some(index) = arguments.iter().position(|argument| argument == "--target") {
            let name = arguments
                .get(index + 1)
                .cloned()
                .ok_or_else(|| "--target requires a name".to_owned())?;
            arguments.drain(index..=index + 1);
            Some(name)
        } else {
            None
        };
    let main_path = arguments
        .first()
        .map(std::path::PathBuf::from)
        .ok_or_else(|| "missing root Omega source path".to_owned())?;
    Ok(ProbeArguments {
        both,
        keep,
        target_name,
        main_path,
    })
}

fn probe_artifact_policy(keep: bool) -> ArtifactEmissionPolicy {
    if keep {
        ArtifactEmissionPolicy::Full
    } else {
        ArtifactEmissionPolicy::OutputOnly
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_rejects_offline_including_after_the_source_or_as_a_value() {
        for arguments in [
            vec!["--offline", "main.omg"],
            vec!["main.omg", "--offline"],
            vec!["--both", "main.omg", "--offline"],
            vec!["--target", "--offline", "main.omg"],
        ] {
            let result = parse_arguments(arguments.iter().map(std::ffi::OsString::from));
            assert!(matches!(result, Err(error) if error.contains("does not support --offline")));
        }
    }

    #[test]
    fn probe_preserves_ordinary_options() {
        let arguments = parse_arguments(
            ["--both", "main.omg", "--keep", "--target", "linux_x64"]
                .into_iter()
                .map(std::ffi::OsString::from),
        )
        .unwrap();
        assert!(arguments.both);
        assert!(arguments.keep);
        assert_eq!(arguments.main_path, std::path::PathBuf::from("main.omg"));
        assert_eq!(arguments.target_name.as_deref(), Some("linux_x64"));
    }

    #[test]
    fn disposable_probe_skips_auxiliary_artifacts() {
        assert_eq!(
            probe_artifact_policy(false),
            ArtifactEmissionPolicy::OutputOnly
        );
    }

    #[test]
    fn kept_probe_retains_full_artifacts() {
        assert_eq!(probe_artifact_policy(true), ArtifactEmissionPolicy::Full);
    }
}
