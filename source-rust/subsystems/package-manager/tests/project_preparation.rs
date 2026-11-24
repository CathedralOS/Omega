//! Exercise package and provider selection through the public project API.
use build_declarations::DependencyPurpose;
use package_manager::project::open_product;
use platform_custody::temporary_directory::TemporaryDirectory;
use std::fs;
use std::path::Path;
use target::TargetProfile;

fn configure() {
    static LIBRARY: std::sync::Once = std::sync::Once::new();
    LIBRARY.call_once(|| {
        let library = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../source/library");
        package_compilation::toolchain_library::configure(&library).unwrap();
    });
}

fn write_package(root: &Path, build: &str) {
    fs::create_dir_all(root).unwrap();
    fs::write(root.join("build.omg"), build).unwrap();
    fs::write(root.join("main.omg"), "machine main() {}").unwrap();
}

#[test]
fn build_dependencies_use_the_host_and_product_dependencies_use_the_requested_target() {
    configure();
    let temp = TemporaryDirectory::create("project-targets", false).unwrap();
    let client = temp.path().join("client");
    write_package(
        &client,
        r#"machine build(builder: @mutable Build) {
        Build::application(builder, "client");
        let build_dep: DependencyDescription = dependency\from_source(Source::Path { location: "library" });
        let build_registration: DependencyRegistration = Build::build_dependency(builder, build_dep);
        let product_dep: DependencyDescription = dependency\from_source(Source::Path { location: "library" });
        let product_registration: DependencyRegistration = Build::dependency(builder, product_dep);
        Build::bind(builder, windows_x86_64\ProgramEntry, main);
        Build::bind(builder, linux_x86_64\ProgramEntry, main);
        Build::bind(builder, macos_arm64\ProgramEntry, main);
        Build::bind(builder, linux_arm64\ProgramEntry, main);
    }"#,
    );
    write_package(
        &client.join("library"),
        r#"machine build(builder: @mutable Build) { Build::package(builder, "shared"); }"#,
    );
    let host = TargetProfile::host_if_supported().unwrap();
    let deployment = if host == TargetProfile::LinuxArm64 {
        TargetProfile::LinuxX64
    } else {
        TargetProfile::LinuxArm64
    };
    let product = open_product(
        &client.join("build.omg"),
        deployment,
        true,
        &temp.path().join("out"),
        None,
    )
    .unwrap();
    let inputs = product.package_inputs().unwrap();
    let handoffs: Vec<_> = inputs
        .dependency_build_handoffs()
        .map(|row| (row.package(), row.purpose(), row.target()))
        .collect();
    assert_eq!(handoffs.len(), 2);
    assert_eq!(handoffs[0].0, handoffs[1].0);
    assert!(handoffs.contains(&(handoffs[0].0, DependencyPurpose::Build, host)));
    assert!(handoffs.contains(&(handoffs[0].0, DependencyPurpose::Product, deployment)));
}

#[test]
fn the_invoker_cannot_supply_two_backing_packages_under_one_alias() {
    use build_declarations::{BuildDependencySource, ProjectName};
    use package_manager::project::{BuildLibraryRequest, ProjectInput, open_products};

    configure();
    let temp = TemporaryDirectory::create("project-provider", false).unwrap();
    let client = temp.path().join("client");
    write_package(
        &client,
        r#"use omega\language\core\ProgramImage;
use {backing}/main.omg as image;
machine build(builder: @mutable Build) suspends {
    Build::application(builder, "client");
    suspend Build::await_provider::<ProgramImage, |image|::TestImage>(builder);
}"#,
    );
    let libraries = ["one", "two"].map(|directory| {
        write_package(
            &client.join(directory),
            &format!("machine build(builder: @mutable Build) {{ Build::package(builder, \"{directory}\"); }}"),
        );
        BuildLibraryRequest {
            alias: ProjectName::parse("backing").unwrap(),
            source: BuildDependencySource::Path(directory.into()),
        }
    });
    let result = open_products(
        ProjectInput::WithBuildLibraries { file: &client.join("build.omg"), libraries: &libraries },
        &[TargetProfile::host_if_supported().unwrap()],
        true,
        false,
        &temp.path().join("out"),
        None,
    );
    let Err(errors) = result else {
        panic!("the invocation supplied two libraries as backing")
    };
    assert!(
        errors.iter().any(|error| error
            .message
            .contains("duplicate supplied Build library alias `backing`")),
        "{errors:?}"
    );
}
