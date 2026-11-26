/*

End-to-end CLI check that a Build can ask for a Product declaration by path
(`omega --check`).

`BuildDeclarations::entry(@builder.declarations, "support::setup::launch", slot)` asks
which machine a path names and hands back a description a root binding can
use (`wiki/spec/build/scoped_execution.md`, "Selecting product declarations
without executing them"). The path is read in the package of the Build
source that asked: a path through a Product dependency alias names a public
declaration of that dependency, and a Build-scope edge of the same alias
grants nothing to the Product.

The first two programs differ only in the scope of the dependency edge their
Build registers under the alias `support`. With a Product edge the entry is
found and the program compiles. With a Build edge the alias names no Product
dependency, so the path names nothing and the Build is refused for it.

The invoker can supply a different Build library under the Product alias.
A query issued by a generated helper uses its own package as its origin.
The tests run the selected entry on the native and Psi engines.

*/

#[path = "support/project.rs"]
mod project;
mod support;

use std::path::Path;

fn check(fixture: &str) -> (bool, String) {
    let directory = support::repository_root().join(fixture);
    project::status(&project::omega(&directory, &["--check", "main.omg"]))
}

#[test]
fn an_evaluated_product_locator_uses_the_alias_in_its_issuing_file() {
    let scratch = project::stage(
        "computed-product-locator",
        &[
            ("main.omg", "public data Table { value: u8; }"),
            (
                "build.omg",
                r#"use main.omg as selected;
use main.omg as unused;
machine locator() -> @[u8] { "|selected|::Table" }
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Package { name: "locator" };
    let schema: ProductTypeSchema = BuildDeclarations::schema(@builder.declarations, locator());
    _ = ProductTypeSchema::path(@schema);
}"#,
            ),
        ],
    );
    let output = project::omega(scratch.path(), &["--check", "build.omg"]);
    let report = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success()
            && report.matches("unused file import `unused`").count() == 1
            && !report.contains("unused file import `selected`"),
        "{report}"
    );
}

#[test]
fn an_entry_described_through_a_product_dependency_alias_compiles() {
    let (compiled, stderr) = check(
        "source-rust/application/tests/fixtures/build/pass/product_entry_query_product_scope_alias",
    );
    assert!(compiled, "{stderr}");
}

#[test]
fn an_entry_asked_through_a_build_scope_alias_names_nothing() {
    let (compiled, stderr) =
        check("tests/fixtures/build/fail/product_entry_query_build_scope_alias");
    assert!(!compiled);
    assert!(
        stderr.contains(
            "product entry `support::setup::launch` names no machine in this build occurrence's package"
        ),
        "{stderr}"
    );
}

/// Supply the external Build library at invocation. Run the resulting Product
/// artifact through the CLI so both engines exercise the entry the Build bound.
fn run_with_kit(root: &Path, library: &str, target: target::TargetProfile) -> Result<(), String> {
    use build_declarations::{BuildDependencySource, ProjectName};
    use compiler::{Artifact, CompileRequest};
    use package_manager::project::{BuildLibraryRequest, ProjectInput, open_products};
    use product_publication::CompletedProduct;

    package_compilation::toolchain_library::configure(
        &support::repository_root().join("source/library"),
    )?;
    let output = platform_custody::temporary_directory::TemporaryDirectory::create(
        "supplied-query-product",
        false,
    )
    .unwrap();
    let libraries = [BuildLibraryRequest {
        max_reach: Default::default(),
        alias: ProjectName::parse("kit").unwrap(),
        source: BuildDependencySource::Path(library.into()),
    }];
    let products = open_products(
        ProjectInput::WithBuildLibraries {
            file: &root.join("build.omg"),
            libraries: &libraries,
        },
        &[target],
        true,
        false,
        &output.path().join("build"),
        Default::default(),
    )
    .map_err(|errors| format!("{errors:?}"))?;
    let product = products
        .select(target)
        .map_err(|errors| format!("{errors:?}"))?;
    let stage = product
        .product_stage(&[target])
        .map_err(|errors| format!("{errors:?}"))?;
    let compiled = compiler::compile(CompileRequest::Psi(stage.source()), Default::default())
        .map_err(|errors| format!("{errors:?}"))?;
    let Artifact::Psi(artifact) = compiled.artifact else {
        panic!("requested Psi");
    };
    let mut published = CompletedProduct::from_published_psi(artifact, None)?;
    let path = output.path().join("program.psi");
    CompletedProduct::save_shared_psi(&mut [&mut published], &path)?;
    let manifest = published
        .artifact()
        .unwrap()
        .manifest()
        .identity()
        .to_string();
    let path = path.to_str().unwrap();
    for (engine, expected) in [("native", "native exit 0"), ("psi", "Psi exit 0")] {
        let arguments = [
            "run",
            "--engine",
            engine,
            "--trusted-artifact",
            &manifest,
            path,
        ];
        let result = project::omega(root, &arguments);
        let report = format!(
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        if !result.status.success() || !report.lines().any(|line| line == expected) {
            return Err(format!("{engine}: {report}"));
        }
    }
    Ok(())
}

#[test]
fn a_build_calls_its_supplied_kit_and_queries_the_products_kit() {
    let Some(host) = target::TargetProfile::host_if_supported() else {
        return;
    };
    let build = r#"use {kit}/main.omg as kit;
use {omega}/build/dependency.omg;
machine build(builder: @mutable Build) suspends {
    Build::application(builder, "dual_alias");
    let mutable deployed: DependencyDescription = |dependency|::from_source(Source::Path { location: "runtime" });
    |dependency|::with_alias(@mutable deployed, "kit");
    let p: DependencyRegistration = Build::dependency(builder, deployed);
    suspend Build::prepare_dependency(builder, p);
    let seven: i32 = |kit|::choose();
    transition seven == 7 {
        true -> bind_entry(builder),
        _ -> wrong(builder)
    }
    machine bind_entry(builder: @mutable Build) {
        Build::bind(builder, HOST::ProgramEntry, entry(builder, "HOST::ProgramEntry"));
    }
    machine wrong(builder: @mutable Build) { Build::application(builder, "wrong"); }
}
machine entry(builder: @mutable Build, slot: @[u8]) -> ProductEntryRef {
    BuildDeclarations::entry(@builder.declarations, "kit::launch", slot)
}
"#.replace("HOST", host.target_name());
    let scratch = project::stage(
        "shared-query-alias",
        &[
            ("tools/main.omg", "public machine choose() -> i32 { 7 }"),
            (
                "tools/build.omg",
                r#"machine build(builder: @mutable Build) { Build::package(builder, "tools"); }"#,
            ),
            ("runtime/main.omg", "public machine launch() {}"),
            (
                "runtime/build.omg",
                r#"machine build(builder: @mutable Build) { Build::package(builder, "runtime"); }"#,
            ),
            ("main.omg", "machine main() {}"),
            ("build.omg", &build),
        ],
    );
    let root = scratch.path();
    run_with_kit(root, "tools", host).unwrap();

    let tools = root.join("tools/main.omg");
    std::fs::write(&tools, "public machine choose() -> i32 { 8 }").unwrap();
    let diagnostic = run_with_kit(root, "tools", host).unwrap_err();
    assert!(diagnostic.contains("project role"), "{diagnostic}");

    std::fs::write(&tools, "public machine choose() -> i32 { 7 }").unwrap();
    std::fs::write(root.join("runtime/main.omg"), "machine launch() {}").unwrap();
    let diagnostic = run_with_kit(root, "tools", host).unwrap_err();
    assert!(
        diagnostic
            .contains("product entry `kit::launch` is not a product declaration visible from"),
        "{diagnostic}"
    );
}

#[test]
fn a_query_issued_by_a_generated_build_helper_keeps_its_source() {
    let Some(target) = target::TargetProfile::host_if_supported() else {
        return;
    };
    let build = r#"use {kit}/generated.omg as selector;
use {omega}/build/dependency.omg;
machine build(builder: @mutable BuildPlan) suspends {
    builder.product = BuildProduct::Application { name: "generated_query" };
    let mutable product_request: DependencyDescription = |dependency|::from_source(Source::Path { location: "vendor" });
    |dependency|::with_alias(@mutable product_request, "kit");
    let for_product: DependencyRegistration = |dependency|::add(builder, product_request);
    suspend Build::prepare_dependency(builder, for_product);
    Build::bind(builder, HOST::ProgramEntry, |selector|::select(builder, "HOST::ProgramEntry"));
}
"#.replace("HOST", target.target_name());
    let scratch = project::stage(
        "generated-query-author",
        &[
            (
                "build.omg",
                r#"use {omega}/build/dependency.omg;
machine build(plan: @mutable BuildPlan) {
    let mutable kit: DependencyDescription = |dependency|::from_source(Source::Path { location: "vendor" });
    |dependency|::with_alias(@mutable kit, "kit");
    _ = |dependency|::add(plan, kit);
    plan.product = BuildProduct::InterpretedBuild { entry: "configure.omg" };
}"#,
            ),
            (
                "vendor/asset.txt",
                r#"public machine select(builder: @mutable BuildPlan, slot: @[u8]) -> ProductEntryRef {
    BuildDeclarations::entry(@builder.declarations, "launch", slot)
}"#,
            ),
            (
                "vendor/finish.template",
                r#"machine build(plan: @mutable BuildPlan) {
    plan.product = BuildProduct::Package { name: "kit" };
}"#,
            ),
            ("vendor/main.omg", "public machine launch() {}"),
            (
                "main.omg",
                r"use {kit}/main.omg as kit;
machine main() { |kit|::launch(); }
machine launch() crashes Trap { crash Trap; }",
            ),
            ("configure.omg", &build),
        ],
    );
    let root = scratch.path();
    let generator = support::repository_root().join("source-rust/subsystems/compiler/tests/fixtures/build/pass/captured_source_generated_constant/build.omg");
    std::fs::copy(generator, root.join("vendor/build.omg")).unwrap();
    for engine in ["native", "psi"] {
        let output = project::omega(
            root,
            &["run", "--engine", engine, "--outputs", engine, "build.omg"],
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let observations = root.join(engine);
        let result = std::fs::read_to_string(observations.join("termination.tsv")).unwrap();
        if engine == "psi" {
            assert_eq!(result, "psi\treturn\n");
        } else {
            assert!(result.starts_with("native\texit\t0\t"), "{result}");
        }
    }
}
