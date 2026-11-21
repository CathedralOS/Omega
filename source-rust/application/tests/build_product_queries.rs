/*

End-to-end CLI check that a Build can ask for a Product declaration by path
(`omega --check`).

`BuildProduct::entry(@builder.product, "support::setup::launch", slot)` asks
which machine a path names and hands back a description a root binding can
use (`wiki/spec/build/scoped_execution.md`, "Selecting product declarations
without executing them"). The path is read in the package of the Build
source that asked: a path through a Product dependency alias names a public
declaration of that dependency, and a Build-scope edge of the same alias
grants nothing to the Product.

The two kept programs differ only in the scope of the dependency edge their
Build registers under the alias `support`. With a Product edge the entry is
found and the program compiles. With a Build edge the alias names no Product
dependency, so the path names nothing and the Build is refused for it.

*/

mod support;

use std::process::Command;

fn check(fixture: &str) -> (bool, String) {
    let directory = support::repository_root()
        .join(fixture);
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .arg("--check")
        .arg("main.omg")
        .current_dir(&directory)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("omega runs");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn an_entry_described_through_a_product_dependency_alias_compiles() {
    let (compiled, stderr) = check("source-rust/application/tests/fixtures/build/pass/product_entry_query_product_scope_alias");
    assert!(compiled, "{stderr}");
}

#[test]
fn an_entry_asked_through_a_build_scope_alias_names_nothing() {
    let (compiled, stderr) = check("tests/fixtures/build/fail/product_entry_query_build_scope_alias");
    assert!(!compiled);
    assert!(
        stderr.contains(
            "product entry `support::setup::launch` names no machine in this build occurrence's package"
        ),
        "{stderr}"
    );
}

#[test]
fn a_resumed_build_calls_its_kit_and_queries_the_products_kit() {
    let Some(host) = target::TargetProfile::host_if_supported() else {
        return;
    };
    let project = platform_custody::temporary_directory::TemporaryDirectory::create(
        "shared-query-alias",
        false,
    )
    .unwrap();
    let root = project.path();
    for (package, source) in [
        ("tools", "public machine choose() -> i32 { 7 }"),
        ("runtime", "public machine launch() {}"),
    ] {
        let directory = root.join(package);
        std::fs::create_dir(&directory).unwrap();
        std::fs::write(directory.join("main.omg"), source).unwrap();
        std::fs::write(
            directory.join("build.omg"),
            format!(
                r#"machine build(builder: @mutable Build) {{ Build::package(builder, "{package}"); }}"#
            ),
        )
        .unwrap();
    }
    std::fs::write(root.join("main.omg"), "machine main() {}").unwrap();
    let build = r#"use kit;
machine build(builder: @mutable Build) suspends {
    Build::application(builder, "dual_alias");
    let mutable host: DependencyDescription = dependency\from_source(Source::Path { location: "tools" });
    dependency\with_alias(@mutable host, "kit");
    let h: DependencyRegistration = Build::build_dependency(builder, host);
    let mutable deployed: DependencyDescription = dependency\from_source(Source::Path { location: "runtime" });
    dependency\with_alias(@mutable deployed, "kit");
    let p: DependencyRegistration = Build::dependency(builder, deployed);
    suspend Build::prepare_dependency(builder, h);
    let seven: i32 = kit\choose();
    transition seven == 7 {
        true -> bind_entry(builder),
        _ -> wrong(builder)
    }
    machine bind_entry(builder: @mutable Build) {
        Build::bind(builder, HOST\ProgramEntry, entry(builder, "HOST::ProgramEntry"));
    }
    machine wrong(builder: @mutable Build) { Build::application(builder, "wrong"); }
}
machine entry(builder: @mutable Build, slot: @[u8]) -> ProductEntryRef {
    BuildProduct::entry(@builder.product, "kit::launch", slot)
}
"#.replace("HOST", host.target_name());
    std::fs::write(root.join("build.omg"), build).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["run", "--both", "build.omg"])
        .current_dir(root)
        .output()
        .unwrap();
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.status.success()
            && report.contains("native exit 0")
            && report.contains("interp exit: 0"),
        "{report}"
    );

    std::fs::write(root.join("runtime/main.omg"), "machine launch() {}").unwrap();
    let refused = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["--check", "build.omg"])
        .current_dir(root)
        .output()
        .unwrap();
    let diagnostic = String::from_utf8_lossy(&refused.stderr);
    assert!(!refused.status.success() && diagnostic.contains(
        "product entry `kit::launch` is not a product declaration visible from"
    ), "{diagnostic}");
}

#[test]
fn a_query_issued_by_a_generated_build_helper_keeps_its_source() {
    let Some(target) = target::TargetProfile::host_if_supported() else {
        return;
    };
    let project = platform_custody::temporary_directory::TemporaryDirectory::create(
        "generated-query-author",
        false,
    )
    .unwrap();
    let root = project.path();
    let kit = root.join("vendor");
    std::fs::create_dir(&kit).unwrap();
    let generated = r#"public machine select(builder: @mutable Build, slot: @[u8]) -> ProductEntryRef {
    BuildProduct::entry(@builder.product, "launch", slot)
}
"#;
    let fixture = support::repository_root().join("source-rust/subsystems/compiler/tests/fixtures/build/pass/captured_source_generated_constant/build.omg");
    let generator = std::fs::read_to_string(fixture)
        .unwrap()
        .replace(
            "Build::application(builder, \"captured_source_generated_constant\");",
            "Build::package(builder, \"kit\");",
        )
        .replace("    Build::artifact_only(builder);", "")
        .replace("31", &generated.len().to_string());
    std::fs::write(kit.join("build.omg"), generator).unwrap();
    std::fs::write(kit.join("asset.txt"), generated).unwrap();
    std::fs::write(kit.join("main.omg"), "public machine launch() {}").unwrap();
    std::fs::write(
        root.join("main.omg"),
        "use kit;\nmachine main() { kit\\launch(); }",
    )
    .unwrap();
    let build = r#"use kit;
machine build(builder: @mutable Build) suspends {
    Build::application(builder, "generated_query");
    let mutable request: DependencyDescription = dependency\from_source(Source::Path { location: "vendor" });
    dependency\with_alias(@mutable request, "kit");
    let registered: DependencyRegistration = Build::build_dependency(builder, request);
    let mutable product_request: DependencyDescription = dependency\from_source(Source::Path { location: "vendor" });
    dependency\with_alias(@mutable product_request, "kit");
    let for_product: DependencyRegistration = Build::dependency(builder, product_request);
    suspend Build::prepare_dependency(builder, registered);
    Build::bind(builder, HOST\ProgramEntry, kit\select(builder, "HOST::ProgramEntry"));
}
"#.replace("HOST", target.target_name());
    std::fs::write(root.join("build.omg"), build).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["run", "--both", "build.omg"])
        .current_dir(root)
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.status.success()
            && report.contains("native exit 0")
            && report.contains("interp exit: 0"),
        "{report}"
    );
}
