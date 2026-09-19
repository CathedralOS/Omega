//! Build-scope sources are checked for the admitted build execution profile,
//! not the product target (`wiki/spec/build/scoped_execution.md`).

use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

/// A root whose build entry declares one target-scoped helper machine, for
/// `helper_target` only, and calls it while building.
struct HelperFixture {
    root: std::path::PathBuf,
    main: std::path::PathBuf,
}

impl HelperFixture {
    fn new(helper_target: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "omega-build-execution-profile-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).expect("create execution-profile fixture");
        let main = root.join("main.omg");
        fs::write(&main, "const ANSWER: u32 = 42;\n").expect("write main");
        fs::write(
            root.join("build.omg"),
            format!(
                "machine build(builder: &mut Build) {{\n    builder.package(\"scoped-helper\");\n    probe();\n}}\n\n{helper_target} machine probe() {{}}\n"
            ),
        )
        .expect("write build");
        Self { root, main }
    }
}

impl Drop for HelperFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// The catalogued host profile these tests select rows against, or an
/// explicit skip: a host no Omega profile describes cannot author a
/// host-named row, so host-row divergence has no meaning there.
fn host_profile() -> Option<target::TargetProfile> {
    let host = target::TargetProfile::host_if_supported();
    if host.is_none() {
        eprintln!("skipping: this host admits no catalogued Omega target profile");
    }
    host
}

/// A product target that is not the compiler host.
fn foreign_product_target(host: target::TargetProfile) -> target::TargetProfile {
    match host {
        target::TargetProfile::LinuxX64 => target::TargetProfile::MacosArm64,
        _ => target::TargetProfile::LinuxX64,
    }
}

/// A profile that is neither the host nor the foreign product target.
fn third_profile(
    host: target::TargetProfile,
    foreign_product: target::TargetProfile,
) -> target::TargetProfile {
    let taken = [host, foreign_product];
    [
        target::TargetProfile::WindowsX64,
        target::TargetProfile::LinuxArm64,
        target::TargetProfile::LinuxX64,
    ]
    .into_iter()
    .find(|profile| !taken.contains(profile))
    .expect("three hosted profiles remain")
}

#[test]
fn build_helper_selects_its_host_row_while_the_product_targets_another_profile() {
    let Some(host) = host_profile() else {
        return;
    };
    let fixture = HelperFixture::new(host.target_name());
    let product = foreign_product_target(host);
    let checked = super::compile_to_checked(super::CheckedCompileRequest::new(
        &fixture.main,
        Some(product.target_name()),
    ))
    .unwrap_or_else(|diagnostics| {
        panic!("the build helper's {host:?} row must select under a {product:?} product: {diagnostics:#?}")
    });
    assert_eq!(checked.selected_target_profile(), Some(product));
}

#[test]
fn an_admitted_execution_profile_without_a_helper_row_leaves_the_helper_inert() {
    let Some(host) = host_profile() else {
        return;
    };
    let fixture = HelperFixture::new(host.target_name());
    let foreign_product = foreign_product_target(host);
    let mut request =
        super::CheckedCompileRequest::new(&fixture.main, Some(foreign_product.target_name()));
    request.build_execution_profile = Some(third_profile(host, foreign_product));
    let diagnostics = super::compile_to_checked(request)
        .expect_err("a helper row for the host only is inert under another execution profile");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("probe")),
        "{diagnostics:#?}"
    );
}

/// A root package beside its reconciled dependencies, wired as a package
/// graph rather than through a lock: `kit` is reachable only through the
/// root's build edge, `lib` only through its product edge, and an optional
/// `tool` only through `kit`'s ordinary edge. The build entry imports `kit`
/// and calls its `kit_probe`; the product entry imports `lib`.
struct PackagedFixture {
    directory: std::path::PathBuf,
    root_main: std::path::PathBuf,
    inputs: package_compilation::PackageCompilationInputs,
    /// Every non-root package; an exact-target child receives one
    /// generated-source handoff (possibly empty) per entry.
    dependencies: Vec<semantic_vocabulary::PackageKeyIdentity>,
}

impl PackagedFixture {
    fn new(kit_main: &str, lib_main: &str, tool_main: Option<&str>) -> Self {
        use package_compilation::{
            PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
        };
        let directory = std::env::temp_dir().join(format!(
            "omega-build-dependency-scope-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&directory).expect("create dependency-scope fixture");
        let package = |name: &str, main: &str| {
            let root = directory.join(name);
            fs::create_dir(&root).expect("create package directory");
            fs::write(root.join("main.omg"), main).expect("write package main");
            fs::write(
                root.join("build.omg"),
                format!(
                    "machine build(builder: &mut Build) {{\n    builder.package(\"{name}\");\n}}\n"
                ),
            )
            .expect("write package build");
            root.canonicalize().expect("canonical package root")
        };
        let root_dir = package("scoped-root", "use lib::main;\n\nconst ANSWER: u32 = 42;\n");
        fs::write(
            root_dir.join("build.omg"),
            "use kit::main;\n\nmachine build(builder: &mut Build) {\n    builder.package(\"scoped-root\");\n    kit_probe();\n}\n",
        )
        .expect("write root build");
        let kit_dir = package("kit", kit_main);
        let lib_dir = package("lib", lib_main);
        let identity = |digest: u8| {
            semantic_vocabulary::PackageKeyIdentity::from_digest([digest; 32])
                .expect("nonzero package identity")
        };
        let (root, kit, lib, tool) = (identity(1), identity(2), identity(3), identity(4));
        let mut packages = vec![
            PackageSourceBinding::new(root, "scoped-root", root_dir.clone()),
            PackageSourceBinding::new(kit, "kit", kit_dir),
            PackageSourceBinding::new(lib, "lib", lib_dir),
        ];
        let mut dependencies = vec![
            PackageDependencyBinding::for_purpose(
                root,
                "kit",
                kit,
                build_declarations::DependencyPurpose::Build,
            ),
            PackageDependencyBinding::new(root, "lib", lib),
        ];
        if let Some(tool_main) = tool_main {
            packages.push(PackageSourceBinding::new(
                tool,
                "tool",
                package("tool", tool_main),
            ));
            dependencies.push(PackageDependencyBinding::new(kit, "tool", tool));
        }
        let dependency_packages = packages
            .iter()
            .map(PackageSourceBinding::identity)
            .filter(|package| *package != root)
            .collect();
        let inputs = PackageCompilationInputs::new_package(root, packages, dependencies)
            .expect("closed dependency graph");
        Self {
            root_main: root_dir.join("main.omg"),
            directory,
            inputs,
            dependencies: dependency_packages,
        }
    }

    /// Mount the generated source each dependency's build handed off for
    /// `product`: `generated` names the handoff text per package, and every
    /// other dependency receives an empty handoff. This is the
    /// `PackageCompilationInputs` route through which the compiler passes
    /// dependency build output to checked compilation; the dependency builds
    /// themselves run above this layer.
    fn with_generated_sources(
        mut self,
        product: target::TargetProfile,
        generated: &[(semantic_vocabulary::PackageKeyIdentity, &str)],
    ) -> Self {
        use package_compilation::{
            PackageGeneratedSourceBundle, PackageSourceConsumptionCommitment,
        };
        let bundles = self
            .dependencies
            .iter()
            .map(|&package| {
                let sources = match generated.iter().find(|(owner, _)| *owner == package) {
                    Some((_, text)) => {
                        let tree = build_output::replayed_single_ordinary_file(
                            b"generated.omg",
                            text.as_bytes(),
                        )
                        .expect("generated handoff should form a retained output tree");
                        build_output::select_included_sources(&tree, &[b"generated.omg".to_vec()])
                            .expect("generated handoff should be selected")
                    }
                    None => Vec::new(),
                };
                PackageGeneratedSourceBundle::from_checked(
                    package,
                    product,
                    target::TargetProfile::host_if_supported(),
                    self.inputs.dependency_closure_for(package),
                    PackageSourceConsumptionCommitment::for_test([5; 32]),
                    sources,
                )
            })
            .collect();
        // Clones share the immutable graph; only the target attachments change.
        self.inputs = self
            .inputs
            .clone()
            .with_complete_dependency_generated_sources(bundles)
            .expect("every dependency receives one handoff");
        self
    }

    fn package(&self, name: &str) -> semantic_vocabulary::PackageKeyIdentity {
        self.dependencies
            .iter()
            .copied()
            .find(|package| self.inputs.package_name(*package) == Some(name))
            .unwrap_or_else(|| panic!("fixture declares package `{name}`"))
    }

    fn request(&self, product: target::TargetProfile) -> super::CheckedCompileRequest<'static> {
        let mut request =
            super::CheckedCompileRequest::new(&self.root_main, Some(product.target_name()));
        request.package_inputs = Some(self.inputs.clone());
        request
    }
}

impl Drop for PackagedFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

/// A library exposing `Probe::run` for `helper_target` only, and a plain
/// `kit_probe` calling it.
fn host_probe_library(helper_target: &str) -> String {
    format!(
        "pub data Probe {{ }}\n\npub {helper_target} machine Probe::run() {{ }}\n\npub machine kit_probe() {{\n    Probe::run();\n}}\n"
    )
}

const PLAIN_KIT: &str = "pub machine kit_probe() { }\n";
const PLAIN_LIB: &str = "pub machine lib_value() -> u64 { 2 }\n";

#[test]
fn a_build_only_dependency_selects_its_host_row_under_a_foreign_product_target() {
    let Some(host) = host_profile() else {
        return;
    };
    let fixture = PackagedFixture::new(&host_probe_library(host.target_name()), PLAIN_LIB, None);
    let product = foreign_product_target(host);
    let checked = super::compile_to_checked(fixture.request(product)).unwrap_or_else(|diagnostics| {
        panic!("the build-only dependency's {host:?} row must select under a {product:?} product: {diagnostics:#?}")
    });
    assert_eq!(checked.selected_target_profile(), Some(product));
}

#[test]
fn a_build_dependency_ordinary_dependency_selects_its_host_row_too() {
    let Some(host) = host_profile() else {
        return;
    };
    let fixture = PackagedFixture::new(
        "use tool::main;\n\npub machine kit_probe() {\n    Probe::run();\n}\n",
        PLAIN_LIB,
        Some(&format!(
            "pub data Probe {{ }}\n\npub {} machine Probe::run() {{ }}\n",
            host.target_name()
        )),
    );
    let product = foreign_product_target(host);
    let checked = super::compile_to_checked(fixture.request(product)).unwrap_or_else(|diagnostics| {
        panic!("a build dependency's ordinary dependency is host context; its {host:?} row must select under a {product:?} product: {diagnostics:#?}")
    });
    assert_eq!(checked.selected_target_profile(), Some(product));
}

#[test]
fn a_product_dependency_host_row_stays_inert_under_a_foreign_product_target() {
    let Some(host) = host_profile() else {
        return;
    };
    let foreign_product = foreign_product_target(host);
    // A lone foreign row is filtered silently, so `Gauge::read` carries two
    // rows, neither for the product: the loud missing-implementation edge
    // fires only when the product target is what `lib` selects against.
    let fixture = PackagedFixture::new(
        PLAIN_KIT,
        &format!(
            "pub data Gauge {{ }}\n\npub {} machine Gauge::read() {{ }}\n\npub {} machine Gauge::read() {{ }}\n",
            host.target_name(),
            third_profile(host, foreign_product).target_name(),
        ),
        None,
    );
    let diagnostics = super::compile_to_checked(fixture.request(foreign_product))
        .expect_err("a product dependency's host row selects against the product target");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("Gauge::read")
                && diagnostic
                    .message
                    .contains("no implementation for the selected target")
        }),
        "{diagnostics:#?}"
    );
}

/// A generated handoff declaring `Owner::read` for the host and a third
/// profile only, neither the product. A lone foreign row is filtered
/// silently; two rows without a match reach the loud missing-implementation
/// edge, so the row set exposes which target the handoff's owner selects
/// against.
fn two_row_handoff(
    owner: &str,
    host: target::TargetProfile,
    foreign_product: target::TargetProfile,
) -> String {
    format!(
        "pub data {owner} {{ }}\n\npub {} machine {owner}::read() {{ }}\n\npub {} machine {owner}::read() {{ }}\n",
        host.target_name(),
        third_profile(host, foreign_product).target_name(),
    )
}

#[test]
fn a_build_only_dependency_generated_source_selects_its_host_row_under_a_foreign_product_target() {
    let Some(host) = host_profile() else {
        return;
    };
    let product = foreign_product_target(host);
    let fixture = PackagedFixture::new(PLAIN_KIT, PLAIN_LIB, None);
    let kit = fixture.package("kit");
    let fixture =
        fixture.with_generated_sources(product, &[(kit, &two_row_handoff("Probe", host, product))]);
    let checked = super::compile_to_checked(fixture.request(product)).unwrap_or_else(|diagnostics| {
        panic!("a build-only dependency's generated source is host context; its {host:?} row must select under a {product:?} product: {diagnostics:#?}")
    });
    assert_eq!(checked.selected_target_profile(), Some(product));
}

#[test]
fn a_product_dependency_generated_source_host_row_stays_inert_under_a_foreign_product_target() {
    let Some(host) = host_profile() else {
        return;
    };
    let product = foreign_product_target(host);
    let fixture = PackagedFixture::new(PLAIN_KIT, PLAIN_LIB, None);
    let lib = fixture.package("lib");
    let fixture =
        fixture.with_generated_sources(product, &[(lib, &two_row_handoff("Gauge", host, product))]);
    let diagnostics = super::compile_to_checked(fixture.request(product))
        .expect_err("a product dependency's generated source selects against the product target");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("Gauge::read")
                && diagnostic
                    .message
                    .contains("no implementation for the selected target")
        }),
        "{diagnostics:#?}"
    );
}
