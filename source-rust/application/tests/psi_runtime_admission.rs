#[path = "support/project.rs"]
mod project;
#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "linux", target_arch = "x86_64"),
))]
mod support;

const BUILD: &str = r#"use main.omg as app;
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Application { name: "structural_domain_issuer" };
    Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main);
    Build::bind(builder, linux_arm64::ProgramEntry, |app|::main);
    Build::bind(builder, macos_arm64::ProgramEntry, |app|::main);
    Build::bind(builder, windows_x86_64::ProgramEntry, |app|::main);
}
"#;

fn assert_psi_runs(name: &str, source: &str) {
    let scratch = project::stage(name, &[("main.omg", source), ("build.omg", BUILD)]);
    let output = project::omega(scratch.path(), &["run", "build.omg"]);
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(output.status.success(), "{report}");
    assert!(report.lines().any(|line| line == "Psi exit 0"), "{report}");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn interpreted_and_built_programs_replay_eof_for_the_darwin_console_descriptor() {
    use std::process::{Command, Stdio};

    let library = support::repository_root().join("source/library");
    let build = format!(
        r#"use main.omg as app;
use {{omega}}/build/dependency.omg;
machine build(builder: @mutable BuildPlan) {{
    builder.product = BuildProduct::Application { name: "console_descriptor_eof" };
    let registration: DependencyRegistration = Build::dependency(builder, |dependency|::from_source(Source::Path {{ location: "{}" }}));
    Build::bind(builder, macos_arm64::ProgramEntry, |app|::main);
}}
"#,
        library.join("system").display()
    );
    let scratch = project::stage(
        "darwin-descriptor-input",
        &[
            (
                "main.omg",
                r#"
use {omega}/language/core/process_exit.omg;
use {omega}/language/core/process_authority.omg;
use {system}/console.omg;
machine main() reaches |process_exit|::ProcessExit + |process_authority|::StandardInput {
    let read: |console|::ByteRead = block |console|::Console::read_byte();
    transition read { |console|::ByteRead::Eof -> done(), _ -> unexpected() }
    machine done() { |process_exit|::ProcessExit::exit_process(0); }
    machine unexpected() { |process_exit|::ProcessExit::exit_process(1); }
}
"#,
            ),
            ("build.omg", &build),
        ],
    );
    for command in ["run", "build"] {
        let output = project::observe_command(
            Command::new(env!("CARGO_BIN_EXE_omega"))
                .args([command, "build.omg"])
                .current_dir(scratch.path())
                .env("OMEGA_LIBRARY_ROOT", &library)
                .stdin(Stdio::null()),
        );
        let report = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{report}");
        assert!(output.stdout.is_empty(), "{output:?}");
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn failed_standard_input_returns_the_written_prefix() {
    use std::io::Write;
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;
    use std::process::{Command, Stdio};

    let library = support::repository_root().join("source/library");
    let build = format!(
        r#"use main.omg as app;
use {{omega}}/build/dependency.omg;
machine build(builder: @mutable BuildPlan) {{
    builder.product = BuildProduct::Application {{ name: "partial_read_error" }};
    let registration: DependencyRegistration = Build::dependency(builder, |dependency|::from_source(Source::Path {{ location: "{}" }}));
    Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main);
}}
"#,
        library.join("system").display()
    );
    let scratch = project::stage(
        "partial-read-error",
        &[
            (
                "main.omg",
                r#"
use {omega}/language/core/process_exit.omg;
use {omega}/language/core/process_authority.omg;
use {system}/console.omg;
machine main() reaches |process_authority|::StandardInput + |process_exit|::ProcessExit crashes none {
    let mutable bytes: [u8; 3];
    bytes[2] = 99;
    let result: |console|::LineReadResult = block |console|::Console::read_line(@mutable bytes[..]);
    transition result {
        |console|::LineReadResult::Error { count } -> inspect(count, bytes[0], bytes[1], bytes[2]),
        _ -> wrong()
    }
    machine inspect(count: u64, first: u8, second: u8, last: u8) {
        transition count == 2 && first == 65 && second == 66 && last == 99 {
            true -> ok(), false -> wrong()
        }
    }
    machine ok() { |process_exit|::ProcessExit::exit_process(0); }
    machine wrong() { |process_exit|::ProcessExit::exit_process(1); }
}
"#,
            ),
            ("build.omg", &build),
        ],
    );
    let output = scratch.path().join("native");
    let build = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["--target", "linux_x86_64", "--build-dir"])
        .arg(&output)
        .arg("build.omg")
        .current_dir(scratch.path())
        .env("OMEGA_LIBRARY_ROOT", library)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );

    // The third read observes EAGAIN while the writer stays open.
    let (input, mut writer) = UnixStream::pair().unwrap();
    input.set_nonblocking(true).unwrap();
    writer.write_all(b"AB").unwrap();
    let result = Command::new(output.join("omega-program"))
        .stdin(Stdio::from(OwnedFd::from(input)))
        .status()
        .unwrap();
    assert_eq!(result.code(), Some(0));
}

#[test]
fn psi_runs_an_owned_record_sequence_literal() {
    assert_psi_runs(
        "owned-sequence",
        "data Ticket { number: u32; }
machine main() {
    let tickets: [Ticket] = [Ticket { number: 11u32 }];
}
",
    );
}

#[test]
fn psi_moves_a_returned_owned_sequence_into_a_by_value_argument() {
    assert_psi_runs(
        "move-owned-sequence",
        "data Ticket { number: u32; }
machine main() {
    consume(make(11u32));
}
machine make(number: u32) -> [Ticket] {
    [Ticket { number: number }]
}
machine consume(tickets: [Ticket]) {}
",
    );
}

#[test]
fn psi_runs_the_bundled_atomic_fence_in_its_module() {
    assert_psi_runs(
        "atomic-fence",
        r#"use {omega}/language/core/Atomic/atomic.omg;
use {omega}/language/core/fence_ordering.omg;
machine main() {
    let ordering: |fence_ordering|::FenceOrdering = |fence_ordering|::FenceOrdering::Receive;
    let alias: |fence_ordering|::FenceOrdering = ordering;
    |atomic|::fence(alias);
    |atomic|::fence(|fence_ordering|::FenceOrdering::Publish);
    |atomic|::fence(|fence_ordering|::FenceOrdering::ReceivePublish);
}
"#,
    );
}

#[test]
fn psi_runs_a_named_sum_constant_with_its_case_payload() {
    assert_psi_runs(
        "sum-constant",
        r#"data Policy [copy] {
    case Read(number: u64);
    case Write(number: u64);
}
const selected: Policy = Policy::Read { number: 7u64 };
machine main() crashes Trap {
    let result: u64 = match selected {
        Policy::Read { number } -> number,
        _ -> 99u64
    };
    transition result == 7u64 {
        true -> done(),
        false -> bad()
    }
    machine done() {}
    machine bad() { crash Trap; }
}
"#,
    );
}
