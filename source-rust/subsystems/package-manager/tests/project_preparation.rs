//! Exercise package and provider selection through the public project API.
use build_declarations::{BuildDependencySource, DependencyPurpose, ProjectName};
use build_evaluation_result::BuildSubjectValidation;
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
        r#"use {omega}/build/dependency.omg;
use main.omg as app;
machine build(builder: @mutable Build) {
    Build::application(builder, "client");
    let product_dep: DependencyDescription = |dependency|::from_source(Source::Path { location: "library" });
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
        r#"use {omega}/build/dependency.omg;
use main.omg as app;
machine build(builder: @mutable Build) {
    Build::application(builder, "client");
    _ = Build::dependency(builder, |dependency|::from_source(Source::Path { location: "library" }));
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

/// The twin where the root owes its own recheck: a root Build with no package
/// graph yet is checked again under the owner its Build returns. The root
/// either has a dependency that read no constant, or no dependency at all.
/// `open_products` finishes the root's recheck before returning. Opened
/// alongside, the root's own recheck is left for the caller to settle.
#[test]
fn a_root_recheck_can_finish_after_its_own_product_opens() {
    configure();
    for dependency in [true, false] {
        root_recheck_finishes_after_its_own_product_opens(dependency);
    }
}

fn root_recheck_finishes_after_its_own_product_opens(dependency: bool) {
    let temp = TemporaryDirectory::create("project-root-recheck", false).unwrap();
    let client = temp.path().join("client");
    let registration = if dependency {
        r#"_ = Build::dependency(builder, |dependency|::from_source(Source::Path { location: "library" }));"#
    } else {
        ""
    };
    let import = if dependency {
        "use {omega}/build/dependency.omg;\n"
    } else {
        ""
    };
    write_package(
        &client,
        &format!(
            r#"{import}use main.omg as app;
machine build(builder: @mutable Build) {{
    Build::application(builder, "client");
    {registration}
    Build::bind(builder, windows_x86_64\ProgramEntry, |app|::main);
    Build::bind(builder, linux_x86_64\ProgramEntry, |app|::main);
    Build::bind(builder, macos_arm64\ProgramEntry, |app|::main);
    Build::bind(builder, linux_arm64\ProgramEntry, |app|::main);
}}"#
        ),
        "machine main() {}",
    );
    write_package(
        &client.join("library"),
        r#"machine build(builder: @mutable Build) { Build::package(builder, "shared"); }"#,
        "public machine helper() {}",
    );
    let target = TargetProfile::host_if_supported().unwrap();
    let entry = client.join("build.omg");
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
    let validation = |products: &package_manager::project::ProjectProducts| {
        products
            .select(target)
            .unwrap()
            .evaluated()
            .subject_validation
    };
    let settled = open(false);
    assert_eq!(
        validation(&settled),
        BuildSubjectValidation::Checked,
        "dependency: {dependency}"
    );
    assert!(!settled.settle_rechecks(target).unwrap().is_enabled());
    let budget = worker_budget::ExecutionBudget::with_workers(2).unwrap();
    budget.run(|| {
        let products = open(true);
        // The Product is released for checking before its own recheck settles.
        assert_eq!(
            validation(&products),
            BuildSubjectValidation::CheckedUnderProvisionalOwner,
            "dependency: {dependency}"
        );
        let recheck = products.settle_rechecks(target).unwrap();
        assert!(
            recheck.is_enabled(),
            "the root's own recheck was still owed (dependency: {dependency})"
        );
        assert!(!products.settle_rechecks(target).unwrap().is_enabled());
    });
}

#[test]
fn supplied_build_library_reach_depends_on_its_invoked_exports() {
    configure();
    let temp = TemporaryDirectory::create("library-reach", false).unwrap();
    let client = temp.path().join("client");
    write_package(
        &client.join("helper"),
        r#"machine build(builder: @mutable Build) { Build::package(builder, "helper"); }"#,
        r#"public boundary trait AuditEffect {}
public machine charged() reaches AuditEffect {}
public machine free() {}"#,
    );
    let libraries = [BuildLibraryRequest {
        alias: ProjectName::parse("checking").unwrap(),
        source: BuildDependencySource::Path("helper".into()),
    }];
    for export in ["free", "charged"] {
        write_package(
            &client,
            &format!(
                r#"use {{checking}}/main.omg as helper;
machine build(builder: @mutable Build) {{
    |helper|::{export}();
    Build::package(builder, "client");
}}"#
            ),
            "public machine main() {}",
        );
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
        if export == "free" {
            result.unwrap();
        } else {
            let Err(error) = result else {
                panic!("the reachable charged export exceeded its empty ceiling")
            };
            let message = error.to_string();
            for expected in ["checking", "charged", "AuditEffect"] {
                assert!(message.contains(expected), "{message}");
            }
        }
    }
}

#[test]
fn dependency_input_forwards_the_frozen_alias_to_the_product() {
    configure();
    let temp = TemporaryDirectory::create("forward-build-input", false).unwrap();
    let client = temp.path().join("client");
    write_package(
        &client,
        r#"use {omega}/build/dependency.omg as dep;
machine build(builder: @mutable Build) {
    let input: PackageBinding = |dep|::input(builder, "layout");
    let registration: DependencyRegistration = |dep|::add(builder, input);
    Build::package(builder, "client");
}"#,
        r#"use {layout}/main.omg as layout;
public machine answer() -> i32 { |layout|::answer() }"#,
    );
    write_package(
        &client.join("supplier"),
        r#"machine build(builder: @mutable Build) { Build::package(builder, "board_layout"); }"#,
        "public machine answer() -> i32 { 42 }",
    );
    let libraries = [BuildLibraryRequest {
        alias: ProjectName::parse("layout").unwrap(),
        source: BuildDependencySource::Path("supplier".into()),
    }];
    let target = TargetProfile::host_if_supported().unwrap();
    let products = open_products(
        ProjectInput::WithBuildLibraries {
            file: &client.join("build.omg"),
            libraries: &libraries,
        },
        &[target],
        true,
        false,
        &temp.path().join("out"),
        None,
    )
    .unwrap();
    let product = products.select(target).unwrap();
    let inputs = product.package_inputs().unwrap();
    let child = inputs
        .dependency_target_for_purpose(inputs.root(), DependencyPurpose::Build, "layout")
        .unwrap();
    assert_eq!(inputs.package_name(child), Some("board_layout"));
    assert_eq!(
        inputs.dependency_target_for_purpose(inputs.root(), DependencyPurpose::Product, "layout"),
        Some(child)
    );
}

#[test]
fn package_bindings_require_an_incoming_alias_and_issued_value() {
    configure();
    let temp = TemporaryDirectory::create("absent-build-input", false).unwrap();
    let root = temp.path().join("project");
    for (operation, diagnostic) in [
        (
            r#"let input: PackageBinding = |dep|::input(builder, "missing");"#,
            "no incoming package binding",
        ),
        (
            r#"let edge: DependencyRegistration = |dep|::add(builder, PackageBinding {});"#,
            "requires a PackageBinding this activation issued",
        ),
    ] {
        write_package(
            &root,
            &format!(
                r#"use {{omega}}/build/dependency.omg as dep;
machine build(builder: @mutable Build) {{ Build::package(builder, "project"); {operation} }}"#
            ),
            "public machine main() {}",
        );
        let result = open_products(
            ProjectInput::WithBuildLibraries {
                file: &root.join("build.omg"),
                libraries: &[],
            },
            &[TargetProfile::host_if_supported().unwrap()],
            true,
            false,
            &temp.path().join("out"),
            None,
        );
        let Err(error) = result else {
            panic!("invalid binding accepted")
        };
        assert!(error.to_string().contains(diagnostic), "{error}");
    }
}
