//! Count actual Build entries for a diamond in two consecutive compiler invocations.
use artifacts::compile_timings::counters::source_build_execution_count;
use package_compilation::declarations::DependencyPurpose;
use package_manager::project::{ProjectInput, open_products};
use platform_custody::temporary_directory::TemporaryDirectory;
use std::fs;
use std::path::Path;
use target::TargetProfile;

fn package(root: &Path, name: &str, dependencies: &[&str]) {
    let directory = root.join(name);
    fs::create_dir(&directory).unwrap();
    let mut body = format!("Build::package(builder, \"{name}\");\n");
    for dependency in dependencies {
        body.push_str(&format!(
            r#"let {dependency}_request: DependencyRegistration = Build::dependency(builder, |dependency|::from_source(Source::Path {{ location: "../{dependency}" }}));
"#
        ));
    }
    let import = if dependencies.is_empty() {
        ""
    } else {
        "use {omega}/build/dependency.omg;\n"
    };
    fs::write(
        directory.join("build.omg"),
        format!("{import}machine build(builder: @mutable Build) {{\n{body}}}"),
    )
    .unwrap();
    fs::write(
        directory.join("main.omg"),
        "public machine value() -> i32 { 7 }",
    )
    .unwrap();
}

#[test]
fn a_diamond_executes_four_builds_afresh_and_keeps_each_targets_handoffs() {
    let library = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../source/library");
    package_compilation::toolchain_library::configure(&library).unwrap();
    let scratch = TemporaryDirectory::create("diamond-builds", false).unwrap();
    let root = scratch.path();
    package(root, "common", &[]);
    package(root, "left", &["common"]);
    package(root, "right", &["common"]);
    package(root, "client", &["left", "right"]);
    let entry = root.join("client/build.omg");
    let targets = [TargetProfile::LinuxX64, TargetProfile::LinuxArm64];
    let compile = || {
        let before = source_build_execution_count();
        let products = open_products(
            ProjectInput::File(&entry),
            &targets,
            true,
            false,
            &root.join("out"),
            Default::default(),
        )
        .unwrap();
        assert_eq!(
            source_build_execution_count() - before,
            4,
            "the two parents share the same common Build"
        );
        targets.map(|target| {
            let inputs = products.select(target).unwrap().package_inputs().unwrap();
            let mut packages = Vec::new();
            for row in inputs.dependency_build_handoffs() {
                assert_eq!(row.purpose(), DependencyPurpose::Product);
                assert_eq!(row.target(), target);
                packages.push((
                    inputs.package_name(row.package()).unwrap().to_owned(),
                    row.source_consumption_commitment(),
                ));
            }
            packages.sort();
            assert_eq!(
                packages
                    .iter()
                    .map(|(name, _)| name.as_str())
                    .collect::<Vec<_>>(),
                ["common", "left", "right"]
            );
            packages
        })
    };
    let first = compile();
    let second = compile();
    assert_eq!(
        first, second,
        "fresh executions of the same graph produce identical handoffs"
    );
}
