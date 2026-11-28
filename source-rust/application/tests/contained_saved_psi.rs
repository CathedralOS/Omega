//! `omega run app.psi` without `--trusted-artifact` interprets saved Psi the
//! host does not trust, contained
//! (wiki/drafts/designs/psi_containment_audit.md, "Contained runtime admission"). It gets the host's
//! reviewed bindings and standard streams, but no native code, no live
//! filesystem yet and no native compilation.

#![cfg(target_os = "linux")]

#[path = "support/project.rs"]
mod project;

use project::{omega, stage};

const MAIN: &str = "use {omega}/language/core/process_authority.omg;

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

const BUILD: &str = "use {omega}/language/core/external_binding.omg as native;
use main.omg as app;

machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Application { name: \"contained_print\" };
    Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main);
    Build::bind_native::<|app|::Streams::write>(builder, linux_x86_64, |native|::ForeignBinding::Syscall { number: 1 });
}
";

fn report(output: &std::process::Output) -> String {
    format!(
        "{:?}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn saved_psi_without_producer_trust_runs_contained() {
    let project = stage(
        "contained-saved-psi",
        &[("main.omg", MAIN), ("build.omg", BUILD)],
    );
    let root = project.path();
    let target = ["--target", "linux_x86_64"];
    let emitted = omega(
        root,
        &[&["--emit-psi", "app.psi"][..], &target, &["build.omg"]].concat(),
    );
    assert!(emitted.status.success(), "{}", report(&emitted));
    let review = omega(
        root,
        &[
            &["packages", "update", "--project", ".", "--offline"][..],
            &target,
            &["--build-input", "build.omg", "--build-input", "main.omg"],
            &["--dangerously-accept-all"],
        ]
        .concat(),
    );
    assert!(review.status.success(), "{}", report(&review));
    let lock = root.join("approved.lock");
    std::fs::rename(root.join("omega.lock"), &lock).unwrap();
    let approval = {
        use sha2::{Digest, Sha256};
        format!("{:x}", Sha256::digest(std::fs::read(&lock).unwrap()))
    };
    let consumer = root.join("consumer");
    std::fs::create_dir(&consumer).unwrap();
    std::fs::rename(root.join("app.psi"), consumer.join("app.psi")).unwrap();
    let saved = |arguments: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_omega"))
            .env("OMEGA_LIBRARY_ROOT", consumer.join("absent-sdk"))
            .env("OMEGA_HOST_PACKAGE_LOCK", &lock)
            .env("OMEGA_HOST_PACKAGE_LOCK_SHA256", &approval)
            .args(arguments)
            .current_dir(&consumer)
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap()
    };

    let run = saved(&[&["run"][..], &target, &["app.psi"]].concat());
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
    let recorded = saved(&[&["run", "--outputs", "recorded"][..], &target, &["app.psi"]].concat());
    assert!(recorded.status.success(), "{}", report(&recorded));
    assert_eq!(
        std::fs::read(consumer.join("recorded/stdout")).unwrap(),
        b"H"
    );
    assert_eq!(
        std::fs::read_to_string(consumer.join("recorded/termination.tsv")).unwrap(),
        "psi\treturn\n"
    );

    // No filesystem binding is audited for contained runs yet, so a requested
    // directory is refused rather than granted.
    std::fs::create_dir(consumer.join("data")).unwrap();
    let mapped = saved(
        &[
            &["run", "--run-directories-readwrite", "data::/data"][..],
            &target,
            &["app.psi"],
        ]
        .concat(),
    );
    assert!(!mapped.status.success(), "{}", report(&mapped));
    assert!(
        String::from_utf8_lossy(&mapped.stderr).contains("no filesystem binding yet"),
        "{}",
        report(&mapped)
    );

    // Native compilation still needs the producer's trust.
    let compiled = saved(&[&target[..], &["--build-dir", "image", "app.psi"]].concat());
    assert!(!compiled.status.success(), "{}", report(&compiled));
    assert!(
        String::from_utf8_lossy(&compiled.stderr).contains("requires --trusted-artifact"),
        "{}",
        report(&compiled)
    );
    assert!(!consumer.join("image").exists());
}
