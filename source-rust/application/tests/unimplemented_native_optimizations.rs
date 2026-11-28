//! A build that selects a native optimization the target has no implementation
//! of must fail and name the optimization and the target. These used to fail
//! with stage internals such as `NoExecutor`, `EffectCatalogMismatch` or
//! `TargetMismatch`, which did not say that the optimization is simply not
//! implemented there. Each case below reaches a different stage.

#[path = "support/project.rs"]
mod project;

const MAIN: &str = "machine add(a: u64, b: u64) -> u64 { a +% b }
public machine main() crashes Trap {
    let x: u64 = add(3u64, 4u64);
    transition x == 7u64 { true -> done(), false -> failed() }
    machine done() {}
    machine failed() { crash Trap; }
}
";

fn refusal(target: &str, optimization: &str) -> (Option<i32>, String) {
    let build = format!(
        "use main.omg as app;
machine build(builder: @mutable BuildPlan) {{
    builder.product = BuildProduct::Application {{ name: \"opt\" }};
    Optimizations::enable(@mutable builder.optimizations, Optimization::{optimization});
    Build::bind(builder, {target}::ProgramEntry, |app|::main);
}}
"
    );
    let scratch = project::stage(
        "unimplemented-native-optimization",
        &[("build.omg", &build), ("main.omg", MAIN)],
    );
    let output = project::omega(
        scratch.path(),
        &["--target", target, "--build-dir", "out", "build.omg"],
    );
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (output.status.code(), report)
}

#[test]
fn an_unimplemented_native_optimization_names_itself_and_the_target() {
    for (target, architecture, optimization) in [
        // A selected-lowering fold whose rewritten form clobbers flags on x86.
        (
            "linux_x86_64",
            "x86_64",
            "SelectedIncomingLiteralCopyMaterialization",
        ),
        // A post-allocation rule with no executor on any target.
        (
            "linux_x86_64",
            "x86_64",
            "Aarch64ElideSameViewCopyI64BeforeReturnV1",
        ),
        // A post-allocation rule for another architecture.
        (
            "linux_arm64",
            "arm64",
            "X86SelectXorZeroI64MaterializationV1",
        ),
        // A layout rule for another architecture.
        (
            "linux_arm64",
            "arm64",
            "X86RelaxConditionalBranchesToRel8V1",
        ),
        // A target-lowering rule for another architecture.
        ("linux_arm64", "arm64", "X86ScaledIndexedLoad"),
        // A selected-lowering rewrite whose packed register forms only x86 has.
        ("linux_arm64", "arm64", "SelectedPackedVectorPromotionV1"),
    ] {
        let (status, report) = refusal(target, optimization);
        assert_ne!(status, Some(0), "{optimization} on {target}: {report}");
        assert!(
            report.contains(&format!(
                "native optimization `{optimization}` is not implemented for {architecture} targets"
            )) && !report.contains("staging failed"),
            "{optimization} on {target}: {report}"
        );
    }
}
