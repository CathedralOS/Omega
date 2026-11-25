//! Exercise package and provider selection through the public project API.
use build_declarations::{BuildDependencySource, DependencyPurpose, ProjectName};
use package_manager::project::{BuildLibraryRequest, ProjectInput, open_products};
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

fn write_package(root: &Path, build: &str, main: &str) {
    fs::create_dir_all(root).unwrap();
    fs::write(root.join("build.omg"), build).unwrap();
    fs::write(root.join("main.omg"), main).unwrap();
}

#[test]
fn build_dependencies_use_the_host_and_product_dependencies_use_the_requested_target() {
    configure();
    let temp = TemporaryDirectory::create("project-targets", false).unwrap();
    let client = temp.path().join("client");
    write_package(
        &client,
        r#"use main.omg as app;
machine build(builder: @mutable Build) {
    Build::application(builder, "client");
    let product_dep: DependencyDescription = dependency\from_source(Source::Path { location: "library" });
    _ = Build::dependency(builder, product_dep);
    Build::bind(builder, windows_x86_64\ProgramEntry, |app|::main);
    Build::bind(builder, linux_x86_64\ProgramEntry, |app|::main);
    Build::bind(builder, macos_arm64\ProgramEntry, |app|::main);
    Build::bind(builder, linux_arm64\ProgramEntry, |app|::main);
}"#,
        "machine main() {}",
    );
    let library = client.join("library");
    write_package(
        &library,
        r#"use selected.omg as selection;
machine build(builder: @mutable Build) {
    transition |selection|::original() {
        true -> shared(builder), false -> alternate(builder),
    }
    machine shared(builder: @mutable Build) { Build::package(builder, "shared"); }
    machine alternate(builder: @mutable Build) { Build::package(builder, "alternate"); }
}"#,
        "public machine helper() {}",
    );
    let host = TargetProfile::host_if_supported().unwrap();
    let deployment = if host == TargetProfile::LinuxArm64 {
        TargetProfile::LinuxX64
    } else {
        TargetProfile::LinuxArm64
    };
    let libraries = [BuildLibraryRequest {
        alias: ProjectName::parse("host_lib").unwrap(),
        source: BuildDependencySource::Path("library".into()),
    }];
    // A generator supplies an ordinary file in the selected input view. Replacing
    // it gives the same dependency a different declaration on the next invocation.
    for (answer, name) in [(true, "shared"), (false, "alternate")] {
        fs::write(
            library.join("selected.omg"),
            format!("public machine original() -> bool {{ {answer} }}"),
        )
        .unwrap();
        let products = open_products(
            ProjectInput::WithBuildLibraries {
                file: &client.join("build.omg"),
                libraries: &libraries,
            },
            &[deployment],
            true,
            false,
            &temp.path().join("out"),
            None,
        )
        .unwrap();
        let product = products.select(deployment).unwrap();
        let inputs = product.package_inputs().unwrap();
        let dependency =
            |purpose, alias| inputs.dependency_target_for_purpose(inputs.root(), purpose, alias);
        let package = dependency(DependencyPurpose::Build, "host_lib").unwrap();
        assert_eq!(inputs.package_name(package), Some(name));
        assert_eq!(dependency(DependencyPurpose::Product, name), Some(package));
        assert_eq!(dependency(DependencyPurpose::Product, "host_lib"), None);
        assert_eq!(dependency(DependencyPurpose::Build, name), None);

        let handoffs: Vec<_> = inputs.dependency_build_handoffs().collect();
        assert_eq!(handoffs.len(), 2);
        for (purpose, target) in [
            (DependencyPurpose::Build, host),
            (DependencyPurpose::Product, deployment),
        ] {
            let handoff = handoffs
                .iter()
                .find(|row| row.purpose() == purpose)
                .unwrap();
            assert_eq!(handoff.package(), package);
            assert_eq!(handoff.target(), target);
            assert_eq!(handoff.build_execution_profile(), Some(host));
        }
    }
}

#[test]
fn the_invoker_cannot_supply_two_backing_packages_under_one_alias() {
    configure();
    let temp = TemporaryDirectory::create("project-provider", false).unwrap();
    let client = temp.path().join("client");
    write_package(
        &client,
        r#"use {omega}/language/core/program_image.omg as image;
use {backing}/main.omg as backing;
machine build(builder: @mutable Build) suspends {
    Build::application(builder, "client");
    suspend Build::await_provider::<|image|::ProgramImage, |backing|::TestImage>(builder);
}"#,
        "machine main() {}",
    );
    let libraries = ["one", "two"].map(|directory| {
        write_package(
            &client.join(directory),
            &format!("machine build(builder: @mutable Build) {{ Build::package(builder, \"{directory}\"); }}"),
            r#"use {omega}/language/core/program_image.omg as image;
public TestImage satisfies |image|::ProgramImage {
    machine retained_suffix_start() -> addr { 64 }
}"#,
        );
        BuildLibraryRequest {
            alias: ProjectName::parse("backing").unwrap(),
            source: BuildDependencySource::Path(directory.into()),
        }
    });
    let result = open_products(
        ProjectInput::WithBuildLibraries {
            file: &client.join("build.omg"),
            libraries: &libraries,
        },
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
