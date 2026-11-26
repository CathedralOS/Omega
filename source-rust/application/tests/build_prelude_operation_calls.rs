//! Build prelude operations resolve to the compiler-owned machines. The selected
//! program must satisfy the exclusions its Build actually runs.

#[path = "support/project.rs"]
mod project;

fn execute(name: &str, build: &str, program: &str, args: &[&str]) -> (bool, String) {
    let scratch = project::stage(name, &[("main.omg", program), ("build.omg", build)]);
    let output = project::omega(scratch.path(), args);
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (output.status.success(), report)
}

/// The Build ran: nothing was refused while its source was discovered, and
/// neither of the two refusals a replaced prelude operation used to meet is
/// present.
fn build_ran(stderr: &str) {
    for refusal in [
        "project source discovery failed",
        "rewritten call contradicts its original selection record",
        "cannot call a private machine of another package",
    ] {
        assert!(!stderr.contains(refusal), "{stderr}");
    }
}

// A Build may pass the evaluated cause through an ordinary helper. The two
// programs differ only in whether the selected call can reach a Trap.
#[test]
fn an_executed_exclusion_checks_the_selected_callee() {
    let Some(host) = target::TargetProfile::host_if_supported() else {
        return;
    };
    let build = r#"use main.omg as app;
machine build(builder: @mutable Build) {
    Build::application(builder, "excluding");
    Build::bind(builder, HOST::ProgramEntry, |app|::main);
    exclude(builder, CrashCause::Trap);
}
machine exclude(builder: @mutable Build, cause: CrashCause) {
    Build::exclude_crash(builder, cause);
}
"#
    .replace("HOST", host.target_name());
    let program = r#"
machine main() crashes Trap { selected(); }
machine selected() crashes Trap { BEHAVIOR }
"#;
    for (engine, expected) in [("native", "native exit 0"), ("psi", "Psi exit 0")] {
        let (passed, report) = execute(
            "excluded-clean",
            &build,
            &program.replace("BEHAVIOR", ""),
            &["run", "--engine", engine, "build.omg"],
        );
        assert!(
            passed && report.lines().any(|line| line == expected),
            "{report}"
        );

        let (passed, report) = execute(
            "excluded-trap",
            &build,
            &program.replace("BEHAVIOR", "crash Trap;"),
            &["run", "--engine", engine, "build.omg"],
        );
        assert!(
            !passed
                && report.contains("behavior exclusion violated: crash cause Trap is reachable"),
            "{report}"
        );
    }
}

#[test]
fn a_direct_build_exclusion_checks_an_evaluated_physical_class() {
    let Some(host) = target::TargetProfile::host_if_supported() else {
        return;
    };
    let build = r#"use main.omg as app;
machine build(builder: @mutable Build) {
    Build::application(builder, "excluding");
    Build::bind(builder, HOST::ProgramEntry, |app|::main);
    let authority: PhysicalAuthorityClass = PhysicalAuthorityClass::ProcessOutput;
    Build::exclude_physical_authority(builder, authority);
}
"#
    .replace("HOST", host.target_name());
    for (engine, expected) in [("native", "native exit 0"), ("psi", "Psi exit 0")] {
        let (passed, report) = execute(
            "excluded-output",
            &build,
            "machine main() {}",
            &["run", "--engine", engine, "build.omg"],
        );
        assert!(
            passed && report.lines().any(|line| line == expected),
            "{report}"
        );
    }
}

/// Zero descriptors make poll a timer. Every call to the selected import
/// must support that conclusion before Build can certify a physical exclusion.
#[test]
fn darwin_poll_exclusions_check_each_invocation() {
    let build = r#"use {omega}/language/core/external_binding.omg as native;
use main.omg as app;
machine build(builder: @mutable Build) {
    Build::application(builder, "poll_counts");
    Build::bind(builder, macos_arm64::ProgramEntry, |app|::main);
    Build::bind_native::<|app|::Wait::poll>(builder, macos_arm64, |native|::ForeignBinding::DllImport {
        import: |native|::DllImport::MachODylibSymbol {
            install_name: "/usr/lib/libSystem.B.dylib", symbol: "_poll"
        }
    });
    Build::exclude_physical_authority(builder, PhysicalAuthorityClass::ProcessOutput);
}
"#;
    let program = r#"
boundary trait Wait {
    machine poll(descriptors: u64, count: u32, milliseconds: i32) -> i32
    reaches none crashes none;
}
machine main() reaches none invokes Wait {
    let first: i32 = Wait::poll(0u64, 0u32, 0i32);
    let second: i32 = Wait::poll(0u64, COUNT, 0i32);
}
"#;
    let args = ["--target", "macos_arm64", "build.omg"];
    let (passed, report) = execute(
        "descriptor-free",
        build,
        &program.replace("COUNT", "0u32"),
        &args,
    );
    assert!(passed, "{report}");

    let (passed, report) = execute(
        "retired-native-binding-name",
        &build.replace("Build::bind_native::<", "Build::bind::<"),
        &program.replace("COUNT", "0u32"),
        &args,
    );
    assert!(
        !passed && report.contains("this call has no visible declaration in its source scope"),
        "{report}"
    );

    let (passed, report) = execute(
        "descriptor-bearing",
        build,
        &program.replace("COUNT", "1u32"),
        &args,
    );
    assert!(
        !passed && report.contains("no physical authority classification"),
        "{report}"
    );
}

/// A Build that excludes nothing has nothing for that admission to refuse.
#[test]
fn a_build_without_exclusions_is_not_refused_for_one() {
    let (_, stderr) = execute(
        "no-exclusion",
        "use main.omg as app;
machine build(builder: @mutable Build) {
    Build::application(builder, \"plain\");
    Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main);
}
",
        "machine main() {}",
        &["--check", "main.omg"],
    );
    build_ran(&stderr);
    assert!(
        !stderr.contains("Build behavior exclusion admission"),
        "{stderr}"
    );
}

/// A Product query in a value position: the entry a root binding is given.
#[test]
fn a_build_that_queries_a_product_entry_is_not_refused_for_its_selection() {
    let (_, stderr) = execute(
        "query-entry",
        "machine build(builder: @mutable Build) {
    Build::application(builder, \"querying\");
    Build::bind(builder, linux_x86_64::ProgramEntry, entry(builder, \"linux_x86_64::ProgramEntry\"));
}
machine entry(builder: @mutable Build, slot: @[u8]) -> ProductEntryRef {
    BuildDeclarations::entry(@builder.declarations, \"main\", slot)
}
",
        "machine main() {}",
        &["--check", "main.omg"],
    );
    build_ran(&stderr);
    assert!(
        !stderr.contains("rewritten call contradicts its original selection record"),
        "{stderr}"
    );
}
