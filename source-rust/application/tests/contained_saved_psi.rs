//! `omega run app.psi` without `--trusted-artifact` interprets saved Psi the
//! host does not trust, contained
//! (wiki/drafts/designs/psi_containment_audit.md, "Contained runtime admission").
//! It gets the host's reviewed bindings, standard streams and the standalone
//! Product grant: the working directory read/write at `/`, replaced by any
//! explicit run directory mappings. It gets no native code, no network and no
<<<<<<< HEAD
//! native compilation. A source run gets the same default directory grant.
=======
//! native compilation.
>>>>>>> b84beb8852d955865ae5e2209793554e8aa7eedc

#![cfg(target_os = "linux")]

#[path = "support/project.rs"]
mod project;

use platform_custody::temporary_directory::TemporaryDirectory;
use project::{omega, stage};
use std::path::PathBuf;
use std::process::Output;

const PRINT: &str = "use {omega}/language/core/process_authority.omg;

boundary trait Streams {
    machine write(fd: i32, byte: @u8, count: u64) -> i64 reaches |process_authority|::StandardOutput crashes none;
}

machine main() reaches |process_authority|::StandardOutput crashes Trap {
    let letter: u8 = 72;
    let written: i64 = Streams::write(1, @letter, 1);
    transition written == 1 {
        true -> passed(),
        _ -> failed()
    }
    machine passed() {}
    machine failed() crashes Trap { crash Trap; }
}
";

const PRINT_BUILD: &str = "use {omega}/language/core/external_binding.omg as native;
use main.omg as app;

machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Application { name: \"contained_print\" };
    Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main);
    Build::bind_native::<|app|::Streams::write>(builder, linux_x86_64, |native|::ForeignBinding::Syscall { number: 1 });
}
";

// Read `/in`, read through the symbolic link `/link`, copy the first byte to a
// new `/out`, then print both bytes read. A refused open leaves its byte zero.
const FILES: &str = "use {omega}/language/core/filesystem_authority.omg as authority;
use {omega}/language/core/process_authority.omg;

public boundary trait Files {
    machine openat(dir: i32, path: @[u8; 4096], flags: i32, mode: i32) -> i32
    reaches |authority|::FilesystemContentRead + |authority|::FilesystemContentWrite + |authority|::FilesystemMetadataMutation + |authority|::FilesystemNamespaceMutation crashes none;
    machine read(descriptor: i32, bytes: @mutable [u8; 1], count: u64) -> i64
    requires count <= 1u64
    reaches |authority|::FilesystemContentRead crashes none;
    machine write(descriptor: i32, bytes: @[u8; 1], count: u64) -> i64
    requires count <= 1u64
    reaches |authority|::FilesystemContentWrite crashes none;
    machine close(fd: i32) -> i32 reaches none crashes none;
}

public boundary trait Streams {
    machine write(fd: i32, byte: @[u8; 1], count: u64) -> i64
    requires count <= 1u64
    reaches |process_authority|::StandardOutput crashes none;
}

public machine main() reaches |authority|::FilesystemContentRead + |authority|::FilesystemContentWrite + |authority|::FilesystemMetadataMutation + |authority|::FilesystemNamespaceMutation + |process_authority|::StandardOutput {
    let mutable input: [u8; 4096];
    input[0] = 47u8;
    input[1] = 105u8;
    input[2] = 110u8;
    let mutable read: [u8; 1];
    let fd: i32 = Files::openat(-100i32, @input, 0i32, 0i32);
    _ = Files::read(fd, @mutable read, 1u64);
    _ = Files::close(fd);

    let mutable link: [u8; 4096];
    link[0] = 47u8;
    link[1] = 108u8;
    link[2] = 105u8;
    link[3] = 110u8;
    link[4] = 107u8;
    let mutable escaped: [u8; 1];
    let linked: i32 = Files::openat(-100i32, @link, 0i32, 0i32);
    _ = Files::read(linked, @mutable escaped, 1u64);
    _ = Files::close(linked);

    let mutable output: [u8; 4096];
    output[0] = 47u8;
    output[1] = 111u8;
    output[2] = 117u8;
    output[3] = 116u8;
    let created: i32 = Files::openat(-100i32, @output, 65i32, 384i32);
    _ = Files::write(created, @read, 1u64);
    _ = Files::close(created);

    _ = Streams::write(1i32, @read, 1u64);
    _ = Streams::write(1i32, @escaped, 1u64);
}
";

const FILES_BUILD: &str = "use main.omg as app;
use {omega}/language/core/external_binding.omg as native;

machine build(plan: @mutable BuildPlan) {
    plan.product = BuildProduct::Application { name: \"contained_files\" };
    Build::bind(plan, linux_x86_64::ProgramEntry, |app|::main);
    Build::bind_native::<|app|::Files::openat>(plan, linux_x86_64, |native|::ForeignBinding::Syscall { number: 257 });
    Build::bind_native::<|app|::Files::read>(plan, linux_x86_64, |native|::ForeignBinding::Syscall { number: 0 });
    Build::bind_native::<|app|::Files::write>(plan, linux_x86_64, |native|::ForeignBinding::Syscall { number: 1 });
    Build::bind_native::<|app|::Files::close>(plan, linux_x86_64, |native|::ForeignBinding::Syscall { number: 3 });
    Build::bind_native::<|app|::Streams::write>(plan, linux_x86_64, |native|::ForeignBinding::Syscall { number: 1 });
}
";

// A socket is a live network endpoint, which a contained run never gets.
const SOCKET: &str = "use {omega}/language/core/network_authority.omg as network;

boundary trait Net {
    machine socket(domain: i32, kind: i32, protocol: i32) -> i32 reaches |network|::NetworkInitiateContact crashes none;
}

machine main() reaches |network|::NetworkInitiateContact {
    _ = Net::socket(2i32, 1i32, 0i32);
}
";

const SOCKET_BUILD: &str = "use {omega}/language/core/external_binding.omg as native;
use main.omg as app;

machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Application { name: \"contained_socket\" };
    Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main);
    Build::bind_native::<|app|::Net::socket>(builder, linux_x86_64, |native|::ForeignBinding::Syscall { number: 41 });
}
";

// `fork` starts a host process, which a contained run never gets.
const LAUNCH: &str = "use {omega}/language/core/program_launch.omg as launch;

boundary trait Process {
    machine fork() -> i32 reaches |launch|::ProgramLaunch crashes none;
}

machine main() reaches |launch|::ProgramLaunch {
    _ = Process::fork();
}
";

const LAUNCH_BUILD: &str = "use {omega}/language/core/external_binding.omg as native;
use main.omg as app;

machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Application { name: \"contained_launch\" };
    Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main);
    Build::bind_native::<|app|::Process::fork>(builder, linux_x86_64, |native|::ForeignBinding::Syscall { number: 57 });
}
";

const TARGET: [&str; 2] = ["--target", "linux_x86_64"];

fn report(output: &Output) -> String {
    format!(
        "{:?}\nstdout: {:?}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// A saved program in its own consumer directory, with the host's reviewed
/// package lock, after its source is out of reach.
struct Saved {
    _project: TemporaryDirectory,
    consumer: PathBuf,
    lock: PathBuf,
    approval: String,
}

impl Saved {
    fn publish(name: &str, main: &str, build: &str) -> Self {
        let project = stage(
            name,
            &[("source/main.omg", main), ("source/build.omg", build)],
        );
        let source = project.path().join("source");
        let emitted = omega(
            &source,
            &[&["--emit-psi", "app.psi"][..], &TARGET, &["build.omg"]].concat(),
        );
        assert!(emitted.status.success(), "{}", report(&emitted));
        let review = omega(
            &source,
            &[
                &["packages", "update", "--project", ".", "--offline"][..],
                &TARGET,
                &["--build-input", "build.omg", "--build-input", "main.omg"],
                &["--dangerously-accept-all"],
            ]
            .concat(),
        );
        assert!(review.status.success(), "{}", report(&review));
        let lock = project.path().join("approved.lock");
        std::fs::rename(source.join("omega.lock"), &lock).unwrap();
        let approval = {
            use sha2::{Digest, Sha256};
            format!("{:x}", Sha256::digest(std::fs::read(&lock).unwrap()))
        };
        let consumer = project.path().join("consumer");
        std::fs::create_dir(&consumer).unwrap();
        std::fs::rename(source.join("app.psi"), consumer.join("app.psi")).unwrap();
        Self {
            _project: project,
            consumer,
            lock,
            approval,
        }
    }

    fn omega(&self, arguments: &[&str]) -> Output {
        std::process::Command::new(env!("CARGO_BIN_EXE_omega"))
            .env("OMEGA_LIBRARY_ROOT", self.consumer.join("absent-sdk"))
            .env("OMEGA_HOST_PACKAGE_LOCK", &self.lock)
            .env("OMEGA_HOST_PACKAGE_LOCK_SHA256", &self.approval)
            .args(arguments)
            .current_dir(&self.consumer)
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap()
    }

    fn run(&self, options: &[&str]) -> Output {
        self.omega(&[&["run"][..], options, &TARGET, &["app.psi"]].concat())
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.consumer.join(relative)
    }
}

#[test]
fn saved_psi_without_producer_trust_runs_contained() {
    let saved = Saved::publish("contained-saved-psi", PRINT, PRINT_BUILD);
    let run = saved.run(&[]);
    assert!(run.status.success(), "{}", report(&run));
    assert_eq!(run.stdout, b"H", "{}", report(&run));
    assert!(
        String::from_utf8_lossy(&run.stderr)
            .lines()
            .any(|line| line == "Psi exit 0"),
        "{}",
        report(&run)
    );

    // A recorded run keeps the guest's output out of the caller's streams.
    let recorded = saved.run(&["--outputs", "recorded"]);
    assert!(recorded.status.success(), "{}", report(&recorded));
    assert_eq!(std::fs::read(saved.path("recorded/stdout")).unwrap(), b"H");
    assert_eq!(
        std::fs::read_to_string(saved.path("recorded/termination.tsv")).unwrap(),
        "psi\treturn\n"
    );

    // Native compilation still needs the producer's trust.
    let compiled = saved.omega(&[&TARGET[..], &["--build-dir", "image", "app.psi"]].concat());
    assert!(!compiled.status.success(), "{}", report(&compiled));
    assert!(
        String::from_utf8_lossy(&compiled.stderr).contains("requires --trusted-artifact"),
        "{}",
        report(&compiled)
    );
    assert!(!saved.path("image").exists());
}

#[test]
fn a_contained_run_gets_its_working_directory_or_the_explicit_mappings() {
    let saved = Saved::publish("contained-saved-files", FILES, FILES_BUILD);
    let outside = saved.consumer.parent().unwrap().join("outside");
    std::fs::write(&outside, b"X").unwrap();
    std::fs::write(saved.path("in"), b"R").unwrap();
    std::os::unix::fs::symlink(&outside, saved.path("link")).unwrap();

    // By default the working directory is mounted read/write at `/`. A
    // symbolic link out of it is refused, so its byte stays zero.
    let default = saved.run(&[]);
    assert!(default.status.success(), "{}", report(&default));
    assert_eq!(default.stdout, b"R\0", "{}", report(&default));
    assert_eq!(std::fs::read(saved.path("out")).unwrap(), b"R");
    assert_eq!(std::fs::read(&outside).unwrap(), b"X");

    // Explicit mappings replace that mount: the guest's `/` is now a
    // read-only directory of its own, and the working directory is gone.
    std::fs::remove_file(saved.path("out")).unwrap();
    let data = saved.path("data");
    std::fs::create_dir(&data).unwrap();
    std::fs::write(data.join("in"), b"S").unwrap();
    let mapped = saved.run(&["--run-directories-readonly", "data::/"]);
    assert!(mapped.status.success(), "{}", report(&mapped));
    assert_eq!(mapped.stdout, b"S\0", "{}", report(&mapped));
    assert!(!data.join("out").exists());
    assert!(!saved.path("out").exists());
}

<<<<<<< HEAD
// A source run gets the same default grant: the caller's working directory
// at `/`, not the host namespace, when the interpreter performs its
// filesystem calls itself.
#[test]
fn a_source_run_gets_its_working_directory_at_the_root() {
    let project = stage(
        "source-run-working-directory",
        &[("tool/main.omg", FILES), ("tool/build.omg", FILES_BUILD)],
    );
    let root = project.path();
    let outside = root.parent().unwrap().join(format!(
        "{}-outside",
        root.file_name().unwrap().to_string_lossy()
    ));
    std::fs::write(&outside, b"X").unwrap();
    std::fs::write(root.join("in"), b"R").unwrap();
    std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();
    // The package lives elsewhere; the run sees the directory it starts in.
    let run = omega(root, &[&["run"][..], &TARGET, &["tool/build.omg"]].concat());
    std::fs::remove_file(&outside).ok();
    assert!(run.status.success(), "{}", report(&run));
    assert_eq!(run.stdout, b"R\0", "{}", report(&run));
    assert_eq!(std::fs::read(root.join("out")).unwrap(), b"R");
}

=======
>>>>>>> b84beb8852d955865ae5e2209793554e8aa7eedc
#[test]
fn a_contained_run_gets_no_network() {
    let saved = Saved::publish("contained-saved-socket", SOCKET, SOCKET_BUILD);
    let run = saved.run(&[]);
    assert!(!run.status.success(), "{}", report(&run));
    assert!(run.stdout.is_empty(), "{}", report(&run));
    assert!(
        String::from_utf8_lossy(&run.stderr).contains("no trusted host implementation matches"),
        "{}",
        report(&run)
    );
}

#[test]
fn a_contained_run_launches_no_program() {
    let saved = Saved::publish("contained-saved-launch", LAUNCH, LAUNCH_BUILD);
    let run = saved.run(&[]);
    assert!(!run.status.success(), "{}", report(&run));
    assert!(
        String::from_utf8_lossy(&run.stderr).contains("no trusted host implementation matches"),
        "{}",
        report(&run)
    );
}
