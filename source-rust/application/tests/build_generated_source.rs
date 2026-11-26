//! Exercise declaration and body completion after a Build includes generated Product source.

mod support;

use std::{fs, process::Command};

#[test]
fn generated_machine_bodies_go_through_product_resolution() {
    let Some(host) = target::TargetProfile::host_if_supported() else {
        return;
    };
    let project = platform_custody::temporary_directory::TemporaryDirectory::create(
        "generated-product-body",
        false,
    )
    .unwrap();
    let root = project.path();
    let fixture = support::repository_root().join(
        "source-rust/subsystems/compiler/tests/fixtures/build/pass/captured_source_generated_constant",
    );
    let asset_len = fs::read(fixture.join("asset.txt")).unwrap().len();
    let build = fs::read_to_string(fixture.join("build.omg"))
        .unwrap()
        .replace(
            "Build::artifact_only(builder);",
            &format!(
                "Build::bind(builder, {}::ProgramEntry, main);",
                host.target_name()
            ),
        );
    fs::write(root.join("build.omg"), build).unwrap();
    fs::write(root.join("main.omg"), "machine main() {}").unwrap();
    for (generated, accepted) in [
        ("machine late() -> i32 { 7 }", true),
        ("machine late() { missing(); }", false),
    ] {
        assert!(generated.len() <= asset_len);
        fs::write(root.join("asset.txt"), format!("{generated:asset_len$}")).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_omega"))
            .args(["run", "--both", "build.omg"])
            .current_dir(root)
            .output()
            .unwrap();
        let report = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.status.success(), accepted, "{report}");
        if accepted {
            assert!(
                report.contains("native exit 0") && report.contains("interp exit: 0"),
                "{report}"
            );
        } else {
            assert!(
                report.contains("no visible declaration") && report.contains("late"),
                "{report}"
            );
        }
    }
}
