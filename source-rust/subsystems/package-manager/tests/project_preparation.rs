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
fn workspace_members_resolve_sibling_dependencies_from_the_catalog_capture() {
    configure();
    let temp = TemporaryDirectory::create("workspace-sibling-custody", false).unwrap();
    let catalog = temp.path().join("catalog");
    let consumer = catalog.join("consumer");
    let provider = catalog.join("provider");
    write_package(
        &consumer,
        r#"use {omega}/build/dependency.omg;
machine build(b: @mutable BuildPlan) {
    b.product = BuildProduct::Package { name: "consumer" };
    _ = Build::dependency(b, |dependency|::from_source(Source::Path { location: "../provider" }));
}"#,
        "public machine consume() {}",
    );
    write_package(
        &provider,
        r#"machine build(b: @mutable BuildPlan) { b.product = BuildProduct::Package { name: "original" }; }"#,
        "public machine serve() {}",
    );
    let entry = catalog.join("build.omg");
    fs::write(
        &entry,
        r#"use {omega}/build/workspace.omg;
machine build(b: @mutable BuildPlan) {
    let consumer: WorkspaceMembers = |workspace|::with_member(b, WorkspaceMembers {}, "consumer");
    let members: WorkspaceMembers = |workspace|::with_member(b, consumer, "provider");
    b.product = BuildProduct::Workspace { members: members };
}"#,
    )
    .unwrap();
    let targets = [TargetProfile::LinuxX64];
    let out = temp.path().join("out");
    let mut subjects = package_manager::project::open_check_subjects(
        &entry,
        &targets,
        true,
        false,
        &out,
        Default::default(),
    )
    .unwrap();
    // Both the provider's own subject and the consumer's authored request must keep
    // the declaration captured before either member Build runs.
    fs::write(provider.join("build.omg"), "invalid replacement").unwrap();
    let first = subjects.next().unwrap();
    assert_eq!(first.entry, consumer.join("build.omg"));
    let products = first.products.unwrap();
    let selected = products.select(targets[0]).unwrap();
    let inputs = selected.package_inputs().unwrap();
    let dependency = inputs
        .dependency_target_for_purpose(inputs.root(), DependencyPurpose::Product, "original")
        .unwrap();
    assert_eq!(inputs.package_name(dependency), Some("original"));
    products.settle_rechecks(targets[0]).unwrap();

    let second = subjects.next().unwrap();
    assert_eq!(second.entry, provider.join("build.omg"));
    let products = second.products.unwrap();
    let selected = products.select(targets[0]).unwrap();
    let inputs = selected.package_inputs().unwrap();
    assert_eq!(inputs.package_name(inputs.root()), Some("original"));
    products.settle_rechecks(targets[0]).unwrap();
    assert!(subjects.next().is_none());
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
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Application { name: "client" };
    let product_dep: DependencyDescription = |dependency|::from_source(Source::Path { location: "library" });
    _ = Build::dependency(builder, product_dep);
    Build::bind(builder, windows_x86_64::ProgramEntry, |app|::main);
    Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main);
    Build::bind(builder, macos_arm64::ProgramEntry, |app|::main);
    Build::bind(builder, linux_arm64::ProgramEntry, |app|::main);
}"#,
        "machine main() {}",
    );
    let library = client.join("library");
    write_package(
        &library,
        r#"use selected.omg as selection;
machine build(builder: @mutable BuildPlan) {
    transition |selection|::original() {
        true -> shared(builder), false -> alternate(builder),
    }
    machine shared(builder: @mutable BuildPlan) { builder.product = BuildProduct::Package { name: "shared" }; }
    machine alternate(builder: @mutable BuildPlan) { builder.product = BuildProduct::Package { name: "alternate" }; }
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
        max_reach: Default::default(),
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
            Default::default(),
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
machine build(builder: @mutable BuildPlan) suspends {
    builder.product = BuildProduct::Application { name: "client" };
    suspend Build::await_provider::<|image|::ProgramImage, |backing|::TestImage>(builder);
}"#,
        "machine main() {}",
    );
    let libraries = ["one", "two"].map(|directory| {
        write_package(
            &client.join(directory),
            &format!("machine build(builder: @mutable BuildPlan) {{ builder.product = BuildProduct::Package {{ name: \"{directory}\" }}; }}"),
            r#"use {omega}/language/core/program_image.omg as image;
public TestImage satisfies |image|::ProgramImage {
    machine retained_suffix_start() -> addr { 64 }
}"#,
        );
        BuildLibraryRequest {
            max_reach: Default::default(),
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
        Default::default(),
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

fn open_recheck_case(
    entry: &Path,
    target: TargetProfile,
    directory: &Path,
    alongside: bool,
) -> package_manager::project::ProjectProducts {
    let targets = [target];
    if alongside {
        let mut subjects = package_manager::project::open_check_subjects(
            entry,
            &targets,
            true,
            true,
            directory,
            Default::default(),
        )
        .unwrap();
        let product = subjects
            .next()
            .expect("the named check subject")
            .products
            .unwrap();
        assert!(subjects.next().is_none());
        product
    } else {
        open_products(
            ProjectInput::File(entry),
            &targets,
            true,
            true,
            directory,
            Default::default(),
        )
        .unwrap()
    }
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
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Application { name: "client" };
    _ = Build::dependency(builder, |dependency|::from_source(Source::Path { location: "library" }));
    Build::bind(builder, windows_x86_64::ProgramEntry, |app|::main);
    Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main);
    Build::bind(builder, macos_arm64::ProgramEntry, |app|::main);
    Build::bind(builder, linux_arm64::ProgramEntry, |app|::main);
}"#,
        "machine main() {}",
    );
    let library = client.join("library");
    write_package(
        &library,
        r#"use selected.omg as selection;
machine build(builder: @mutable BuildPlan) {
    transition |selection|::original() {
        true -> shared(builder), false -> alternate(builder),
    }
    machine shared(builder: @mutable BuildPlan) { builder.product = BuildProduct::Package { name: "shared" }; }
    machine alternate(builder: @mutable BuildPlan) { builder.product = BuildProduct::Package { name: "alternate" }; }
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
    let open = |alongside| open_recheck_case(&entry, target, &temp.path().join("out"), alongside);
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
machine build(builder: @mutable BuildPlan) {{
    builder.product = BuildProduct::Application {{ name: "client" }};
    {registration}
    Build::bind(builder, windows_x86_64::ProgramEntry, |app|::main);
    Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main);
    Build::bind(builder, macos_arm64::ProgramEntry, |app|::main);
    Build::bind(builder, linux_arm64::ProgramEntry, |app|::main);
}}"#
        ),
        "machine main() {}",
    );
    write_package(
        &client.join("library"),
        r#"machine build(builder: @mutable BuildPlan) { builder.product = BuildProduct::Package { name: "shared" }; }"#,
        "public machine helper() {}",
    );
    let target = TargetProfile::host_if_supported().unwrap();
    let entry = client.join("build.omg");
    let open = |alongside| open_recheck_case(&entry, target, &temp.path().join("out"), alongside);
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
        r#"machine build(builder: @mutable BuildPlan) { builder.product = BuildProduct::Package { name: "helper" }; }"#,
        r#"public boundary trait AuditEffect {}
public machine charged() reaches AuditEffect {}
public machine free() {}"#,
    );
    let libraries = [BuildLibraryRequest {
        max_reach: Default::default(),
        alias: ProjectName::parse("checking").unwrap(),
        source: BuildDependencySource::Path("helper".into()),
    }];
    for export in ["free", "charged"] {
        write_package(
            &client,
            &format!(
                r#"use {{checking}}/main.omg as helper;
machine build(builder: @mutable BuildPlan) {{
    |helper|::{export}();
    builder.product = BuildProduct::Package {{ name: "client" }};
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
            Default::default(),
        );
        if export == "free" {
            result.unwrap();
        } else {
            let Err(error) = result else {
                panic!("the reachable charged export exceeded its empty ceiling")
            };
            for expected in ["checking", "charged", "AuditEffect"] {
                assert!(
                    error
                        .iter()
                        .any(|diagnostic| diagnostic.message.contains(expected)),
                    "{error:#?}"
                );
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
machine build(builder: @mutable BuildPlan) {
    let input: PackageBinding = |dep|::input(builder, "layout");
    let empty: [BoundaryServiceReference; 0] = [];
    let registration: DependencyRegistration = |dep|::add(builder, input, @empty[..]);
    builder.product = BuildProduct::Package { name: "client" };
}"#,
        r#"use {layout}/main.omg as layout;
public machine answer() -> i32 { |layout|::answer() }"#,
    );
    write_package(
        &client.join("supplier"),
        r#"machine build(builder: @mutable BuildPlan) { builder.product = BuildProduct::Package { name: "board_layout" }; }"#,
        "public machine answer() -> i32 { 42 }",
    );
    let libraries = [BuildLibraryRequest {
        max_reach: Default::default(),
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
        Default::default(),
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
fn package_bindings_and_service_references_require_values_issued_by_the_build() {
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
        (
            r#"let dependency: DependencyDescription = |dep|::from_source(Source::Path { location: "unread" });
              let forged: [BoundaryServiceReference; 1] = [BoundaryServiceReference {}];
              _ = |dep|::add(builder, dependency, @forged[..]);"#,
            "requires a service reference this Build issued",
        ),
    ] {
        write_package(
            &root,
            &format!(
                r#"use {{omega}}/build/dependency.omg as dep;
machine build(builder: @mutable BuildPlan) {{ builder.product = BuildProduct::Package {{ name: "project" }}; {operation} }}"#
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
            Default::default(),
        );
        let Err(error) = result else {
            panic!("invalid binding accepted")
        };
        assert!(
            error.iter().any(|row| row.message.contains(diagnostic)),
            "{error:#?}"
        );
    }
}

#[test]
fn authored_service_ceiling_uses_exact_applications_and_normalized_parents() {
    configure();
    let temp = TemporaryDirectory::create("registered-service-applications", false).unwrap();
    let root = temp.path().join("client");
    write_package(
        &root.join("library"),
        r#"machine build(builder: @mutable BuildPlan) { builder.product = BuildProduct::Package { name: "library" }; }"#,
        r#"public boundary trait Telemetry {}
public boundary trait Sensor<const Channel: u32>: Telemetry {}
public machine free() {}"#,
    );
    write_package(
        &root,
        r#"use {omega}/build/dependency.omg as dep;
use {policy}/main.omg as policy;
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Package { name: "client" };
    let app: BoundaryServiceReference = Build::service::<|policy|::Sensor<17u32>>(builder);
    let description: DependencyDescription = |dep|::from_source(Source::Path { location: "library" });
    let services: [BoundaryServiceReference; 2] = [app, app];
    _ = |dep|::add(builder, description, @services[..]);
}"#,
        "public machine free() {}",
    );
    let target = TargetProfile::host_if_supported().unwrap();
    let products = open_products(
        ProjectInput::WithBuildLibraries {
            file: &root.join("build.omg"),
            libraries: &[BuildLibraryRequest {
                max_reach: Default::default(),
                alias: ProjectName::parse("policy").unwrap(),
                source: BuildDependencySource::Path("library".into()),
            }],
        },
        &[target],
        true,
        false,
        &temp.path().join("out"),
        Default::default(),
    )
    .unwrap();
    let inputs = products.select(target).unwrap().package_inputs().unwrap();
    let allowed = inputs
        .dependency_max_reach(inputs.root(), DependencyPurpose::Product, "library")
        .unwrap();
    let names = allowed.services().collect::<Vec<_>>();
    assert_eq!(names.len(), 2, "{names:?}");
    assert!(names.iter().any(|id| id.contains("Telemetry")), "{names:?}");
    assert!(names.iter().any(|id| id.contains("Sensor")), "{names:?}");
}

#[test]
fn forwarding_cannot_widen_an_incoming_empty_ceiling() {
    configure();
    let temp = TemporaryDirectory::create("binding-ceiling", false).unwrap();
    let root = temp.path().join("client");
    write_package(
        &root.join("library"),
        r#"machine build(builder: @mutable BuildPlan) { builder.product = BuildProduct::Package { name: "library" }; }"#,
        "public boundary trait MotorControl {}
public machine free() {}",
    );
    write_package(
        &root,
        r#"use {omega}/build/dependency.omg as dep;
use {helper}/main.omg as helper;
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Package { name: "client" };
    let app: BoundaryServiceReference = Build::service::<|helper|::MotorControl>(builder);
    let binding: PackageBinding = |dep|::input(builder,"helper");
    let services: [BoundaryServiceReference; 1] = [app];
    _ = |dep|::add(builder,binding,@services[..]);
}"#,
        "public machine free() {}",
    );
    let libraries = [BuildLibraryRequest {
        max_reach: Default::default(),
        alias: ProjectName::parse("helper").unwrap(),
        source: BuildDependencySource::Path("library".into()),
    }];
    let outcome = open_products(
        ProjectInput::WithBuildLibraries {
            file: &root.join("build.omg"),
            libraries: &libraries,
        },
        &[TargetProfile::host_if_supported().unwrap()],
        true,
        false,
        &temp.path().join("out"),
        Default::default(),
    );
    let Err(error) = outcome else {
        panic!("a forwarded package widened its incoming ceiling")
    };
    assert!(
        error
            .iter()
            .any(|row| row.message.contains("exceeds its incoming max_reach")),
        "{error:#?}"
    );
}

#[test]
fn product_checks_the_invoked_export_and_exact_service_application_on_its_edge() {
    configure();
    let temp = TemporaryDirectory::create("product-service-use", false).unwrap();
    let root = temp.path().join("client");
    let target = TargetProfile::host_if_supported().unwrap();
    let contract = target.program_entry_slot().source_contract();
    write_package(
        &root.join("library"),
        r#"machine build(builder: @mutable BuildPlan) { builder.product = BuildProduct::Package { name: "library" }; }"#,
        r#"public boundary trait Sensor<const Channel: u32> {}
public machine charged() reaches Sensor<17u32> {}
public machine free() {}"#,
    );
    let libraries = [BuildLibraryRequest {
        max_reach: Default::default(),
        alias: ProjectName::parse("policy").unwrap(),
        source: BuildDependencySource::Path("library".into()),
    }];
    for (export, allowed_channel, succeeds) in [
        ("free", None, true),
        ("charged", None, false),
        ("charged", Some(17), true),
        ("charged", Some(23), false),
    ] {
        let registration = match allowed_channel {
            None => "_ = |dep|::add(builder, dependency);".to_owned(),
            Some(channel) => format!(
                "let allowed: [BoundaryServiceReference; 1] = [Build::service::<|policy|::Sensor<{channel}u32>>(builder)];
    _ = |dep|::add(builder, dependency, @allowed[..]);"
            ),
        };
        write_package(
            &root,
            &format!(
                r#"use {{omega}}/build/dependency.omg as dep;
use {{policy}}/main.omg as policy;
use main.omg as app;
machine build(builder: @mutable BuildPlan) {{
    builder.product = BuildProduct::Application {{ name: "client" }};
    let dependency: DependencyDescription = |dep|::from_source(Source::Path {{ location: "library" }});
    {registration}
    Build::bind(builder, {}\{}, |app|::entry);
}}"#,
                contract.owner, contract.slot_name
            ),
            &format!(
                "use {{library}}/main.omg as supplier; machine entry() {{ |supplier|::{export}(); }}"
            ),
        );
        let opened = open_products(
            ProjectInput::WithBuildLibraries {
                file: &root.join("build.omg"),
                libraries: &libraries,
            },
            &[target],
            true,
            false,
            &temp.path().join("out"),
            Default::default(),
        )
        .unwrap();
        let stage = opened
            .select(target)
            .unwrap()
            .product_stage(&[target])
            .unwrap();
        let result = stage.check(target, &mut Default::default());
        if succeeds {
            result.unwrap();
        } else {
            let errors = result.expect_err("Product admitted the wrong reach on its library edge");
            assert!(
                errors.iter().any(|row| row.message.contains("library")
                    && row.message.contains("Sensor")
                    && row.message.contains("charged")),
                "{errors:#?}"
            );
        }
    }
}

#[test]
fn repeated_imports_charge_the_selected_alias_at_direct_and_transitive_calls() {
    configure();
    let temp = TemporaryDirectory::create("dependency-alias-authority", false).unwrap();
    let root = temp.path().join("client");
    let target = TargetProfile::host_if_supported().unwrap();
    let contract = target.program_entry_slot().source_contract();
    write_package(
        &root.join("library"),
        r#"machine build(builder: @mutable BuildPlan) { builder.product = BuildProduct::Package { name: "library" }; }"#,
        r#"public boundary trait Meter {}
public machine charge() reaches Meter {}
public machine relay() reaches Meter { charge(); }"#,
    );
    let build = format!(
        r#"use {{omega}}/build/dependency.omg as dep;
use {{policy}}/main.omg as policy;
use main.omg as app;
machine build(builder: @mutable BuildPlan) {{
    builder.product = BuildProduct::Application {{ name: "client" }};
    let mutable limited: DependencyDescription = |dep|::from_source(Source::Path {{ location: "library" }});
    |dep|::with_alias(@mutable limited, "narrow");
    _ = |dep|::add(builder, limited);
    let mutable enabled: DependencyDescription = |dep|::from_source(Source::Path {{ location: "library" }});
    |dep|::with_alias(@mutable enabled, "wide");
    let allowed: [BoundaryServiceReference; 1] = [Build::service::<|policy|::Meter>(builder)];
    _ = |dep|::add(builder, enabled, @allowed[..]);
    Build::bind(builder, {}\{}, |app|::entry);
}}"#,
        contract.owner, contract.slot_name
    );
    let libraries = [BuildLibraryRequest {
        max_reach: Default::default(),
        alias: ProjectName::parse("policy").unwrap(),
        source: BuildDependencySource::Path("library".into()),
    }];
    for (alias, export, permit) in [
        ("wide", "charge", true),
        ("narrow", "charge", false),
        ("wide", "relay", true),
        ("narrow", "relay", false),
    ] {
        write_package(
            &root,
            &build,
            &format!(
                "use {{wide}}/main.omg as open; use {{narrow}}/main.omg as closed; machine entry() {{ |{}|::{export}(); }}",
                if alias == "wide" { "open" } else { "closed" }
            ),
        );
        let products = open_products(
            ProjectInput::WithBuildLibraries {
                file: &root.join("build.omg"),
                libraries: &libraries,
            },
            &[target],
            true,
            false,
            &temp.path().join("out"),
            Default::default(),
        )
        .unwrap();
        let stage = products
            .select(target)
            .unwrap()
            .product_stage(&[target])
            .unwrap();
        let result = stage.check(target, &mut Default::default());
        if permit {
            result.unwrap();
        } else {
            let errors = result.expect_err("the narrow alias inherited the wide alias's ceiling");
            assert!(
                errors
                    .iter()
                    .any(|row| row.message.contains("narrow") && row.message.contains("Meter")),
                "{errors:#?}"
            );
        }
    }
}
