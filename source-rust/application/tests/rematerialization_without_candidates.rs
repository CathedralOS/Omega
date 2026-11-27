//! A selected allocation-recovery rule whose input has nothing it can rewrite
//! must still build: the rule is recorded as run, with no action. Selecting
//! `ActiveResidentImmediateU64MultiUseRematerializationV1` used to fail every
//! program it found nothing in, with `NoAction`, `NoCommonCandidate` or
//! `FutureUseMismatch`.
//!
//! The spec rule: `wiki/spec/build/optimizations.md`, "Phase and product
//! boundaries" (the execution record names every selected pass even when its
//! input contains no eligible rewrite).

#[path = "support/project.rs"]
mod project;

/// A call keeps a value in a fixed view, outside the rule's victim choice.
const CALL: &str = "machine add(a: u64, b: u64) -> u64 { a +% b }
public machine main() crashes Trap {
    let x: u64 = add(3u64, 4u64);
    transition x == 7u64 { true -> done(), false -> failed() }
    machine done() {}
    machine failed() { crash Trap; }
}
";

/// The immediate under pressure has one future use, not the several the rule
/// rewrites.
const ONE_FUTURE_USE: &str = "public machine main() crashes Trap {
    let k: u64 = 81985529216486895u64;
    let a: u64 = k +% 3u64;
    let b: u64 = a *% k;
    let c: u64 = b +% a;
    let d: u64 = c ^ k;
    transition d == (((81985529216486895u64 +% 3u64) *% 81985529216486895u64) +% (81985529216486895u64 +% 3u64)) ^ 81985529216486895u64 { true -> done(), false -> failed() }
    machine done() {}
    machine failed() { crash Trap; }
}
";

const BUILD: &str = "use main.omg as app;
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Application { name: \"remat\" };
    Optimizations::enable(@mutable builder.optimizations, Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1);
    Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main);
    Build::bind(builder, linux_arm64::ProgramEntry, |app|::main);
}
";

#[test]
fn rematerialization_with_nothing_to_rewrite_builds_and_runs() {
    for (name, main) in [("call", CALL), ("one-future-use", ONE_FUTURE_USE)] {
        for target in ["linux_x86_64", "linux_arm64"] {
            let scratch = project::stage(
                &format!("remat-{name}"),
                &[("build.omg", BUILD), ("main.omg", main)],
            );
            let output = project::omega(
                scratch.path(),
                &["--target", target, "--build-dir", "out", "build.omg"],
            );
            let (ok, report) = project::status(&output);
            assert!(ok, "{name} on {target}: {report}");
            if cfg!(all(target_os = "linux", target_arch = "x86_64")) && target == "linux_x86_64" {
                let run = std::process::Command::new(scratch.path().join("out/omega-program"))
                    .output()
                    .unwrap();
                assert_eq!(run.status.code(), Some(0), "{name}: {run:?}");
            }
        }
    }
}
