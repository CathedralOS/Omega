//! Customers of the bundled system vector plan storage using the selected target's array stride.
#[path = "support/project.rs"]
mod project;
mod support;

use std::process::{Command, Stdio};

const BUILD: &str = r#"use main.omg as app;
use {omega}/build/dependency.omg;
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Application { name: "vector_capacity" };
    let system: DependencyRegistration = Build::dependency(builder,
        |dependency|::from_source(Source::Bundled { name: "system" }));
    Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main);
    Build::bind(builder, linux_arm64::ProgramEntry, |app|::main);
    Build::bind(builder, macos_arm64::ProgramEntry, |app|::main);
    Build::bind(builder, windows_x86_64::ProgramEntry, |app|::main);
}
"#;

const QUEUE: &str = r#"use {system}/vec.omg as vec;
data Message [linear] { code: u8; sequence: u64; }
data Ticket [linear] {}
machine main() crashes Trap {
    let bytes: u64 = |vec|::capacity_for::<u8>(31);
    let messages: u64 = |vec|::capacity_for::<Message>(31);
    let tickets: u64 = |vec|::capacity_for::<Ticket>(0);
    transition bytes == 31 && messages == 1 && tickets == 18446744073709551615 {
        true -> done(), false -> wrong()
    }
    machine done() {}
    machine wrong() { crash Trap; }
}
"#;

#[test]
fn storage_for_a_message_queue_accounts_for_padding_and_zero_byte_tickets() {
    for (source, returns) in [
        (QUEUE.to_owned(), true),
        (QUEUE.replace("messages == 1", "messages == 2"), false),
        (
            QUEUE.replace("tickets == 18446744073709551615", "tickets == 0"),
            false,
        ),
    ] {
        let dir = project::stage(
            "system-vec-capacity",
            &[("main.omg", &source), ("build.omg", BUILD)],
        );
        let command = |action| {
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_omega"));
            cmd.args([action, "build.omg"])
                .current_dir(dir.path())
                .env(
                    "OMEGA_LIBRARY_ROOT",
                    support::repository_root().join("source/library"),
                )
                .stdin(Stdio::null());
            cmd
        };
        let interpreted = command("run").output().unwrap();
        let report = String::from_utf8_lossy(&interpreted.stderr);
        if returns {
            assert!(interpreted.status.success(), "{report}");
        } else {
            assert!(report.contains("Psi crash: Trap"), "{report}");
        }
        let native = project::build_and_run_command(&mut command("build"));
        assert!(native.compiler.status.success(), "{:?}", native.compiler);
        let guest = native.execution.unwrap();
        assert_eq!(guest.status.success(), returns, "{guest:?}");
        assert!(guest.stdout.is_empty());
    }
}
