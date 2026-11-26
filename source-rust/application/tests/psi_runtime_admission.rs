#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "linux", target_arch = "x86_64"),
))]
mod support;

use std::path::PathBuf;
use std::process::{Command, Stdio};

fn stage_project(name: &str, source: &str, build: &str) -> PathBuf {
    let project =
        std::env::temp_dir().join(format!("omega-psi-runtime-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&project).expect("staging directory");
    std::fs::write(project.join("main.omg"), source).expect("staged program");
    std::fs::write(project.join("build.omg"), build).expect("staged Build");
    project
}

fn assert_psi_runs(name: &str, source: &str) {
    let project = stage_project(
        name,
        source,
        r#"machine build(builder: @mutable Build) {
    Build::application(builder, "structural_domain_issuer");
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
    Build::bind(builder, linux_arm64::ProgramEntry, main);
    Build::bind(builder, macos_arm64::ProgramEntry, main);
    Build::bind(builder, windows_x86_64::ProgramEntry, main);
}
"#,
    );
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["run", "--engine", "psi", "build.omg"])
        .current_dir(&project)
        .stdin(Stdio::null())
        .output()
        .expect("omega runs on the hosted target");
    std::fs::remove_dir_all(&project).expect("clean staged project");

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
fn both_engines_replay_eof_for_the_darwin_console_descriptor() {
    let library = support::repository_root().join("source/library");
    let project = stage_project(
        "darwin-descriptor-input",
        r#"
use omega\language\core\ProcessExit;
use omega\language\core\StandardInput;
use system\ByteRead;
use system\Console;
machine main() reaches ProcessExit + StandardInput {
    let read: ByteRead = block Console::read_byte();
    transition read { ByteRead::Eof -> done(), _ -> unexpected() }
    machine done() { ProcessExit::exit_process(0); }
    machine unexpected() { ProcessExit::exit_process(1); }
}
"#,
        &format!(
            r#"
use {{omega}}/build/dependency.omg;
machine build(builder: @mutable Build) {{
    Build::application(builder, "console_descriptor_eof");
    let registration: DependencyRegistration = Build::dependency(builder, |dependency|::from_source(Source::Path {{ location: "{}" }}));
    Build::bind(builder, macos_arm64::ProgramEntry, main);
}}
"#,
            library.join("system").display()
        ),
    );
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["run", "--both", "build.omg"])
        .current_dir(&project)
        .env("OMEGA_LIBRARY_ROOT", library)
        .stdin(Stdio::null())
        .output()
        .expect("run the same finite input in both engines");
    std::fs::remove_dir_all(&project).expect("clean staged project");

    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(output.status.success(), "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn failed_standard_input_returns_the_written_prefix() {
    use std::io::Write;
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;

    let library = support::repository_root().join("source/library");
    let project = stage_project(
        "partial-read-error",
        r#"
use omega\language\core\ProcessExit;
use omega\language\core\StandardInput;
use system\Console;
use system\LineReadResult;
machine main() reaches StandardInput + ProcessExit crashes none {
    let mutable bytes: [u8; 3];
    bytes[2] = 99;
    let result: LineReadResult = block Console::read_line(@mutable bytes[..]);
    transition result {
        LineReadResult::Error { count } -> inspect(count, bytes[0], bytes[1], bytes[2]),
        _ -> wrong()
    }
    machine inspect(count: u64, first: u8, second: u8, last: u8) {
        transition count == 2 && first == 65 && second == 66 && last == 99 {
            true -> ok(), false -> wrong()
        }
    }
    machine ok() { ProcessExit::exit_process(0); }
    machine wrong() { ProcessExit::exit_process(1); }
}
"#,
        &format!(
            r#"use {{omega}}/build/dependency.omg;
machine build(builder: @mutable Build) {{
    Build::application(builder, "partial_read_error");
    let registration: DependencyRegistration = Build::dependency(builder, |dependency|::from_source(Source::Path {{ location: "{}" }}));
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
}}
"#,
            library.join("system").display()
        ),
    );
    let output = project.join("native");
    let build = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["--target", "linux_x86_64", "--build-dir"])
        .arg(&output)
        .arg("build.omg")
        .current_dir(&project)
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
    std::fs::remove_dir_all(project).unwrap();
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
        r#"use omega\language\core\Atomic;
use omega\language\core\Atomic\fence;
use omega\language\core\FenceOrdering;
machine main() {
    let ordering: FenceOrdering = FenceOrdering::Receive;
    let alias: FenceOrdering = ordering;
    Atomic\fence(alias);
    fence(FenceOrdering::Publish);
    Atomic\fence(FenceOrdering::ReceivePublish);
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
