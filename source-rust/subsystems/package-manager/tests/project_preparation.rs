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

/// A dependency whose Build read a constant is checked again on its Product's
/// sources. Opened alongside that check, the consuming Product is the same one
/// `open_products` returns once every check has finished, and the check is
/// settled only when the caller asks for it.
#[test]
fn a_dependency_recheck_can_finish_after_the_consuming_product_opens() {
    configure();
    let temp = TemporaryDirectory::create("project-recheck", false).unwrap();
    let client = temp.path().join("client");
    write_package(
        &client,
        r#"use main.omg as app;
machine build(builder: @mutable Build) {
    Build::application(builder, "client");
    _ = Build::dependency(builder, dependency\from_source(Source::Path { location: "library" }));
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
    fs::write(
        library.join("selected.omg"),
        "public machine original() -> bool { true }",
    )
    .unwrap();
    let target = TargetProfile::host_if_supported().unwrap();
    let entry = client.join("build.omg");
    let handoffs = |products: &package_manager::project::ProjectProducts| {
        let inputs = products.select(target).unwrap().package_inputs().unwrap();
        let package = inputs
            .dependency_target_for_purpose(inputs.root(), DependencyPurpose::Product, "shared")
            .unwrap();
        assert_eq!(inputs.package_name(package), Some("shared"));
        inputs
            .dependency_build_handoffs()
            .map(|row| {
                (
                    inputs.package_name(row.package()).unwrap().to_owned(),
                    row.source_consumption_commitment(),
                )
            })
            .collect::<Vec<_>>()
    };
    let open = |alongside| {
        let open = if alongside {
            package_manager::project::open_products_alongside_rechecks
        } else {
            open_products
        };
        open(
            ProjectInput::File(&entry),
            &[target],
            true,
            true,
            &temp.path().join("out"),
            None,
        )
        .unwrap()
    };
    let settled = open(false);
    // `open_products` finished every recheck, so none is left to settle.
    assert!(!settled.settle_rechecks(target).unwrap().is_enabled());
    // Two workers let the recheck run while this thread reads the Product.
    let budget = worker_budget::ExecutionBudget::with_workers(2).unwrap();
    let alongside = budget.run(|| {
        let products = open(true);
        let opened = handoffs(&products);
        let recheck = products.settle_rechecks(target).unwrap();
        assert!(
            recheck.is_enabled(),
            "the dependency's recheck was still owed"
        );
        assert!(!products.settle_rechecks(target).unwrap().is_enabled());
        opened
    });
    assert_eq!(alongside, handoffs(&settled));
}
