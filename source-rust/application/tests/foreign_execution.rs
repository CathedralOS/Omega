//! A declared foreign service follows the same Product and dependency gates
//! as a core native operation. The accepted Linux program reads its process id.
#![cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]

#[path = "support/project.rs"]
mod project;
mod support;

use std::process::Command;

const PROGRAM: &str = r#"use {renderer}/main.omg as renderer;
machine main() crashes Trap {
    transition |renderer|::current() > 0i32 { true -> done(), _ -> fail() }
    machine done() {}
    machine fail() { crash Trap; }
}"#;

const PROVIDER: &str = r#"use {omega}/language/core/unsafe.omg as trust;
public boundary trait ProcessIdentity {
    machine read() -> i32 reaches |trust|::ForeignCodeExecution crashes none;
}
public machine current() -> i32 { ProcessIdentity::read() }
"#;

const PROVIDER_BUILD: &str = r#"use {omega}/language/core/unsafe.omg as trust;
use {omega}/language/core/external_binding.omg as native;
use main.omg as app;
machine build(plan: @mutable BuildPlan) {
    plan.product = BuildProduct::Package { name: "identity" };
    Build::enable_unsafe::<|trust|::ForeignCodeExecution>(plan);
    Build::bind_native::<|app|::ProcessIdentity::read>(plan, linux_x86_64,
        |native|::ForeignBinding::DllImport { import: |native|::DllImport::ElfVersioned {
            object: "libc.so.6", symbol: "getpid", version: "GLIBC_2.2.5"
        } });
}
"#;

const MIDDLE: &str = r#"use {identity}/main.omg as identity;
public machine current() -> i32 { |identity|::current() }"#;

fn consumer(name: &str, child: &str, grant: bool, delegate: bool) -> String {
    let enabled = if grant {
        "Build::enable_unsafe::<|trust|::ForeignCodeExecution>(plan);"
    } else {
        ""
    };
    let edge = if delegate {
        "let services: [BoundaryServiceReference; 1] = [Build::service::<|trust|::ForeignCodeExecution>(plan)];\n    _ = |dependency|::add(plan, package, @services[..]);"
    } else {
        "_ = Build::dependency(plan, package);"
    };
    let (kind, import, entry) = if name == "client" {
        (
            "Application",
            "use main.omg as app;",
            "Build::bind(plan, linux_x86_64::ProgramEntry, |app|::main);",
        )
    } else {
        ("Package", "", "")
    };
    format!(
        r#"use {{omega}}/language/core/unsafe.omg as trust;
use {{omega}}/build/dependency.omg as dependency;
{import}
machine build(plan: @mutable BuildPlan) {{
    plan.product = BuildProduct::{kind} {{ name: "{name}" }};
    {enabled}
    let package: DependencyDescription = |dependency|::from_source(Source::Path {{ location: "{child}" }});
    {edge}
    {entry}
}}
"#
    )
}

#[test]
fn a_foreign_call_names_its_origin_and_runs_only_with_every_product_and_edge_admission() {
    let project = project::stage(
        "foreign-process-id",
        &[
            ("main.omg", PROGRAM),
            ("renderer/main.omg", MIDDLE),
            ("renderer/identity/main.omg", PROVIDER),
        ],
    );
    let command = |action| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_omega"));
        command
            .args([action, "build.omg"])
            .current_dir(project.path())
            .env(
                "OMEGA_LIBRARY_ROOT",
                support::repository_root().join("source/library"),
            );
        command
    };
    let enabled_leaf = "Build::enable_unsafe::<|trust|::ForeignCodeExecution>(plan);";
    let absent = PROVIDER_BUILD.replace(enabled_leaf, "");
    for (missing, refusal) in [
        ("none", None),
        ("client", Some("Build of the root Product did not enable")),
        (
            "client -> renderer",
            Some("dependency edge `renderer` did not admit"),
        ),
        (
            "renderer",
            Some("Build of dependency `renderer` did not enable"),
        ),
        (
            "renderer -> identity",
            Some("dependency edge `identity` did not admit"),
        ),
        (
            "identity",
            Some("Build of dependency `identity` did not enable"),
        ),
    ] {
        project::write(
            project.path(),
            &[
                (
                    "build.omg",
                    &consumer(
                        "client",
                        "renderer",
                        missing != "client",
                        missing != "client -> renderer",
                    ),
                ),
                (
                    "renderer/build.omg",
                    &consumer(
                        "renderer",
                        "identity",
                        missing != "renderer",
                        missing != "renderer -> identity",
                    ),
                ),
                (
                    "renderer/identity/build.omg",
                    if missing == "identity" {
                        &absent
                    } else {
                        PROVIDER_BUILD
                    },
                ),
            ],
        );
        if let Some(reason) = refusal {
            let checked = command("--check").output().unwrap();
            let text = String::from_utf8_lossy(&checked.stderr);
            assert!(!checked.status.success(), "{text}");
            for (kind, detail) in [
                ("error", format!("{reason} Unsafe + ForeignCodeExecution")),
                ("note", "dependency path: client -> renderer -> identity".into()),
                ("note", "call path: main.omg::main -> main.omg::current -> main.omg::current -> main.omg::ProcessIdentity::read".into()),
                ("note", "operation: main.omg::ProcessIdentity::read".into()),
            ] {
                let expected = format!("{kind}: {detail}");
                assert!(text.lines().any(|line| line.trim() == expected), "{text}");
            }
        } else {
            let built = project::build_and_run_command(&mut command("build"));
            assert!(
                built.compiler.status.success(),
                "{}",
                String::from_utf8_lossy(&built.compiler.stderr)
            );
            let native = built.execution.expect("published native executable");
            assert_eq!(native.status.code(), Some(0), "{native:?}");
            assert!(
                native.stdout.is_empty() && native.stderr.is_empty(),
                "{native:?}"
            );
        }
    }
}
