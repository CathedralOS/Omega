//! Build prelude operations resolve to the compiler-owned machines. The selected
//! program must satisfy the exclusions its Build actually runs.

use super::project;

fn execute(name: &str, build: &str, program: &str, args: &[&str]) -> (bool, String) {
    let scratch = project::stage(name, &[("main.omg", program), ("build.omg", build)]);
    let output = project::observe(scratch.path(), args);
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
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Application { name: "excluding" };
    Build::bind(builder, HOST::ProgramEntry, |app|::main);
    exclude(builder, CrashCause::Trap);
}
machine exclude(builder: @mutable BuildPlan, cause: CrashCause) {
    Build::exclude_crash(builder, cause);
}
"#
    .replace("HOST", host.target_name());
    let program = r#"
machine main() crashes Trap { selected(); }
machine selected() crashes Trap { BEHAVIOR }
"#;
    for command in ["run", "build"] {
        let (passed, report) = execute(
            "excluded-clean",
            &build,
            &program.replace("BEHAVIOR", ""),
            &[command, "build.omg"],
        );
        assert!(passed, "{report}");

        let (passed, report) = execute(
            "excluded-trap",
            &build,
            &program.replace("BEHAVIOR", "crash Trap;"),
            &[command, "build.omg"],
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
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Application { name: "excluding" };
    Build::bind(builder, HOST::ProgramEntry, |app|::main);
    let authority: PhysicalAuthorityClass = PhysicalAuthorityClass::ProcessOutput;
    Build::exclude_physical_authority(builder, authority);
    Build::exclude_physical_authority(builder, PhysicalAuthorityClass::HostNetworkStack);
    Build::exclude_physical_authority(builder, PhysicalAuthorityClass::OffMachineNetworkTransmit);
}
"#
    .replace("HOST", host.target_name());
    for command in ["run", "build"] {
        let (passed, report) = execute(
            "excluded-output",
            &build,
            "machine main() {}",
            &[command, "build.omg"],
        );
        assert!(passed, "{report}");
    }
}

/// Opening an unconnected internet datagram socket uses the host stack. The
/// receipt must account for the family, type and protocol of every call.
#[test]
fn socket_exclusions_follow_the_checked_calls_on_each_posix_target() {
    let darwin =
        r#"MachODylibSymbol { install_name: "/usr/lib/libSystem.B.dylib", symbol: "_socket" }"#;
    let linux = |version: &str| {
        format!(r#"ElfVersioned {{ object: "libc.so.6", symbol: "socket", version: "{version}" }}"#)
    };
    for (target, import, ipv6, other_family, object) in [
        (
            "macos_arm64",
            darwin.to_owned(),
            "30i32",
            "10i32",
            "libSystem.B.dylib",
        ),
        (
            "linux_x86_64",
            linux("GLIBC_2.2.5"),
            "10i32",
            "30i32",
            "libc.so.6",
        ),
        (
            "linux_arm64",
            linux("GLIBC_2.17"),
            "10i32",
            "30i32",
            "libc.so.6",
        ),
    ] {
        checked_datagram_calls(target, &import, ipv6, other_family, object);
    }
}

fn checked_datagram_calls(
    target: &str,
    import: &str,
    ipv6: &str,
    other_family: &str,
    object: &str,
) {
    let build = r#"use {omega}/language/core/external_binding.omg as native;
use {omega}/language/core/unsafe.omg as trust;
use main.omg as app;
machine build(b: @mutable BuildPlan) {
    b.product = BuildProduct::Application { name: "socket_review" };
    Build::enable_unsafe::<|trust|::ForeignCodeExecution>(b);
    Build::bind(b, TARGET::ProgramEntry, |app|::main);
    Build::bind_native::<|app|::Transport::open>(b, TARGET, |native|::ForeignBinding::DllImport {
        import: |native|::DllImport::LOCATOR
    });
    Build::exclude_physical_authority(b, PhysicalAuthorityClass::EXCLUSION);
}"#
    .replace("TARGET", target)
    .replace("LOCATOR", import);
    let program = r#"use {omega}/language/core/unsafe.omg as trust;
boundary trait Transport {
    machine open(family: i32, kind: i32, protocol: i32) -> i32 reaches |trust|::ForeignCodeExecution crashes none;
}
machine main() reaches |trust|::ForeignCodeExecution invokes Transport {
    _ = Transport::open(2i32, 2i32, 0i32);
    _ = Transport::open(FAMILY, KIND, PROTOCOL);
}"#;
    let check = |class: &str, family: &str, kind: &str, protocol: &str| {
        execute(
            "checked-udp-socket",
            &build.replace("EXCLUSION", class),
            &program
                .replace("FAMILY", family)
                .replace("KIND", kind)
                .replace("PROTOCOL", protocol),
            &["--target", target, "build.omg"],
        )
    };
    let (passed, report) = check("OffMachineNetworkTransmit", ipv6, "2i32", "17i32");
    assert!(passed, "{report}");
    let (passed, report) = check("HostNetworkStack", ipv6, "2i32", "17i32");
    assert!(
        !passed && report.contains("exercises excluded physical authority class HostNetworkStack"),
        "{report}"
    );
    for (family, kind, protocol) in [
        ("1i32", "2i32", "17i32"),
        (other_family, "2i32", "17i32"),
        ("2i32", "1i32", "0i32"),
        (ipv6, "2i32", "6i32"),
    ] {
        let (passed, report) = check("OffMachineNetworkTransmit", family, kind, protocol);
        assert!(
            !passed && report.contains("no physical authority classification"),
            "{report}"
        );
    }
    let exclude_remote = build.replace("EXCLUSION", "OffMachineNetworkTransmit");
    let two_sockets = program
        .replace("FAMILY", ipv6)
        .replace("KIND", "2i32")
        .replace("PROTOCOL", "17i32");
    for (description, build, source) in [
        (
            "different-library",
            exclude_remote.replace(object, "other.so"),
            two_sockets.clone(),
        ),
        (
            "different-abi",
            exclude_remote,
            two_sockets.replace("i32", "u32"),
        ),
    ] {
        let (passed, report) = execute(
            description,
            &build,
            &source,
            &["--target", target, "build.omg"],
        );
        assert!(
            !passed && report.contains("no physical authority classification"),
            "{description}: {report}"
        );
    }
}

/// Zero descriptors make poll a timer. Every call to the selected import
/// must support that conclusion before Build can certify a physical exclusion.
#[test]
fn darwin_poll_exclusions_check_each_invocation() {
    let build = r#"use {omega}/language/core/external_binding.omg as native;
use main.omg as app;
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Application { name: "poll_counts" };
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
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Application { name: \"plain\" };
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
        "machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Application { name: \"querying\" };
    Build::bind(builder, linux_x86_64::ProgramEntry, entry(builder, \"linux_x86_64::ProgramEntry\"));
}
machine entry(builder: @mutable BuildPlan, slot: @[u8]) -> ProductEntryRef {
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
