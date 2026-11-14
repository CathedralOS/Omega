//! Show one source program to the lowered-form checker ("the route") and
//! print its verdict, without running the program's Build and without the
//! old checker (Psi 05, validation's judgments, Psi 06).
//!
//! An ordinary compilation checks, lowers and executes the project's Build
//! program before its product reaches the route, on stages that are being
//! removed; when they are gone, `omega --check` stops at the Build until the
//! route checks and publishes Build programs. This probe does not depend on
//! any of that. It types the sources as a compilation does, binds the named
//! machine to the target's `ProgramEntry` in the Build's place
//! (`CompilationSession::typed_frontier_without_build`), stops at Psi 04's
//! frozen typed trees, and runs `structure_elaboration` and
//! `lowered_discharge` on them. Nothing else a Build would select is present,
//! so a program that needs its Build's providers or representations is judged
//! without them.
//!
//! A program that imports another package needs the dependency edge its
//! Build registers, stated here with `--dependency`. Most programs print and
//! exit through the standard library's `Console`:
//!
//!     --dependency omega_language_std=source/library/std
//!
//! Run:
//!
//!     mbx run -p compiler --example route_probe -- <main.omg>... [--entry NAME] [--target NAME] [--dependency ALIAS=DIRECTORY]...
//!
//! Without `--entry`, `main` is bound when the source declares it and a source
//! without one is judged with no entry. `--target` defaults to `linux_x86_64`.
//! Each `--dependency` is a Product edge of every listed source's directory.
//! For each source one line per judged part (the entry closure first, then each generic
//! check form), then `<path>\troute=<verdict>` where the verdict is
//! `accepted`, `rejected`, `incomplete`, `not-supported` or `no-typed-trees`.
//! The exit code is 0 only when every source was accepted.

use std::path::{Path, PathBuf};

#[path = "../tests/support/unbuilt_dependencies.rs"]
mod unbuilt_dependencies;

use lowered_discharge::{CompilationRefusal, RouteVerdict};

fn brief(text: impl std::fmt::Debug) -> String {
    format!("{text:?}")
        .replace(['\t', '\n'], " ")
        .chars()
        .take(600)
        .collect()
}

/// The whole compilation's verdict, by the router's rule: not supported when
/// any part is, otherwise rejected when any part is, otherwise accepted.
fn probe(
    main: &Path,
    entry: Option<&str>,
    target: &str,
    dependencies: &[(String, PathBuf)],
) -> &'static str {
    let mut session = compiler::CompilationSession::default();
    let mut request = compiler::CheckedCompileRequest::new(main, Some(target));
    if !dependencies.is_empty() {
        let program = main.parent().unwrap_or_else(|| Path::new("."));
        match unbuilt_dependencies::graph(program, dependencies) {
            Ok(graph) => request.package_inputs = Some(graph),
            Err(reason) => {
                println!("  stopped: {reason}");
                return "no-typed-trees";
            }
        }
    }
    let frontier = match session.typed_frontier_without_build(request, entry) {
        Ok(frontier) => frontier,
        Err(failure) => {
            for diagnostic in failure.into_diagnostics() {
                println!("  stopped: {}", brief(&diagnostic.message));
            }
            return "no-typed-trees";
        }
    };
    let verdicts = lowered_discharge::judge_compilation(
        &frontier.typed,
        structure_elaboration::Compilation {
            declaration_metadata: &[],
            entries: &frontier.entries,
            subjects: &[frontier.subject],
            purpose: source::DependencyScope::Product,
            selected: &frontier.selected,
            bindings: &frontier.bindings,
            supplies: &[],
        },
    );
    let mut whole = "accepted";
    let mut worse = |verdict: &'static str| {
        let rank = |name| match name {
            "accepted" => 0,
            "rejected" => 1,
            "incomplete" => 2,
            _ => 3,
        };
        if rank(verdict) > rank(whole) {
            whole = verdict;
        }
    };
    for (part, verdict) in verdicts.iter().enumerate() {
        match verdict {
            Ok(RouteVerdict::Accepted(_)) => {
                println!("  part {part}: accepted");
                worse("accepted");
            }
            Ok(RouteVerdict::Rejected {
                formations,
                unproved,
                unestablished,
                conflicts,
                exceeded,
            }) => {
                println!("  part {part}: rejected");
                for diagnostic in formations
                    .iter()
                    .map(|entry| entry.reject_diagnostic())
                    .chain(unproved.iter().map(|entry| entry.reject_diagnostic()))
                    .chain(unestablished.iter().map(|entry| entry.reject_diagnostic()))
                    .chain(conflicts.iter().map(|entry| entry.reject_diagnostic()))
                    .chain(exceeded.iter().map(|entry| entry.reject_diagnostic()))
                {
                    println!("    {}", brief(&diagnostic.message));
                }
                worse("rejected");
            }
            Ok(RouteVerdict::Incomplete(unproved)) => {
                println!("  part {part}: incomplete (no rule for a goal form)");
                for entry in unproved {
                    println!("    {}", brief(&entry.reject_diagnostic().message));
                }
                worse("incomplete");
            }
            Ok(RouteVerdict::FallBack(refusal)) => {
                println!(
                    "  part {part}: not supported yet: {:?}: {}",
                    refusal.reason, refusal.detail
                );
                worse("not-supported");
            }
            Err(CompilationRefusal::Elaboration(refusal)) => {
                println!(
                    "  part {part}: not supported yet: {:?}: {}",
                    refusal.reason, refusal.detail
                );
                worse("not-supported");
            }
            Err(CompilationRefusal::Binding(defect)) => {
                println!("  part {part}: route defect: {}", brief(defect));
                worse("not-supported");
            }
        }
    }
    whole
}

fn main() {
    let mut sources = Vec::new();
    let mut entry: Option<String> = None;
    let mut target = "linux_x86_64".to_owned();
    let mut dependencies = Vec::new();
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--entry" => {
                entry = Some(arguments.next().expect("--entry takes a machine name"));
            }
            "--target" => target = arguments.next().expect("--target takes a target name"),
            "--dependency" => {
                let edge = arguments
                    .next()
                    .expect("--dependency takes ALIAS=DIRECTORY");
                let (alias, path) = edge
                    .split_once('=')
                    .expect("--dependency takes ALIAS=DIRECTORY");
                dependencies.push((alias.to_owned(), PathBuf::from(path)));
            }
            _ => sources.push(argument),
        }
    }
    if sources.is_empty() {
        eprintln!(
            "usage: route_probe <main.omg>... [--entry NAME] [--target NAME] [--dependency ALIAS=DIRECTORY]..."
        );
        std::process::exit(2);
    }
    let mut all_accepted = true;
    for source in &sources {
        println!("{source}");
        let verdict = probe(Path::new(source), entry.as_deref(), &target, &dependencies);
        println!("{source}\troute={verdict}");
        all_accepted &= verdict == "accepted";
    }
    std::process::exit(if all_accepted { 0 } else { 1 });
}
