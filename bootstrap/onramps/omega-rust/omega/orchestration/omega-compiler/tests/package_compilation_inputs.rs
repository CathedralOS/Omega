use omega_compiler::{
    CompileOptions, PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
    compile_to_checked_with_packages, compile_to_checked_with_packages_in_build_dir,
    compile_with_packages,
};
use psi_core::PackageKeyIdentity;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempTree(PathBuf);

impl TempTree {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "omega-package-inputs-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create temporary package compilation tree");
        Self(path)
    }

    fn package(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir(&path).expect("create package directory");
        path
    }

    fn write(path: impl AsRef<Path>, source: &str) {
        fs::write(path, source).expect("write Omega package test source");
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn identity(marker: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([marker; 32]).expect("nonzero package identity")
}

#[test]
fn reconciled_bindings_ignore_build_dependency_discovery() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let admitted = tree.package("admitted");
    let malicious = tree.package("malicious");

    TempTree::write(
        root.join("main.omg"),
        "use dep::values;\nconst RESULT: u32 = 42;\n",
    );
    TempTree::write(
        root.join("build.omg"),
        "machine build(builder: &mut Build) {\n    builder.depend_as(\"dep\", Source::Path { location: \"../malicious\" });\n}\n",
    );
    TempTree::write(admitted.join("values.omg"), "const ANSWER: u32 = 42;\n");
    TempTree::write(malicious.join("values.omg"), "this is not Omega source\n");

    let inputs = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "admitted", admitted),
        ],
        vec![PackageDependencyBinding::new(
            identity(1),
            "dep",
            identity(2),
        )],
    )
    .expect("reconciled bindings should validate");

    compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect("trusted package binding should be the only dependency authority");
}

#[test]
fn canonical_build_dependency_vocabulary_typechecks() {
    let tree = TempTree::new();
    let root = tree.package("root");

    TempTree::write(root.join("main.omg"), "const RESULT: u32 = 42;\n");
    TempTree::write(
        root.join("build.omg"),
        r#"
machine build(builder: &mut Build) {
    builder.depend(Source::Path { location: "../ordinary" });
    builder.depend(Source::Git {
        repository: "https://github.com/CathedralOS/arithmetic-kernels.git",
        revision: "0123456789abcdef"
    });
    builder.depend_as(
        "arithmetic_kernels",
        Source::Path { location: "../colliding" }
    );
}
"#,
    );

    let inputs = PackageCompilationInputs::new(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .expect("root-only package graph should validate");

    compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect("canonical dependency vocabulary should typecheck");
}

#[test]
fn aliases_are_requester_local_and_dependency_imports_are_package_local() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");

    TempTree::write(
        root.join("main.omg"),
        "use shared::root_value;\nconst RESULT: u32 = 42;\n",
    );
    TempTree::write(
        middle.join("root_value.omg"),
        "use shared::leaf_value;\nuse local_value;\nconst ROOT_VALUE: u32 = 42;\n",
    );
    TempTree::write(
        middle.join("local_value.omg"),
        "const LOCAL_VALUE: u32 = 1;\n",
    );
    TempTree::write(leaf.join("leaf_value.omg"), "const LEAF_VALUE: u32 = 41;\n");

    let inputs = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "middle", middle),
            PackageSourceBinding::new(identity(3), "leaf", leaf),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "shared", identity(2)),
            PackageDependencyBinding::new(identity(2), "shared", identity(3)),
        ],
    )
    .expect("requester-local aliases should validate");

    compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect("requester-local and package-local imports should compile");
}

#[test]
fn authored_selection_requires_the_declaration_owner_as_a_direct_dependency() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");

    TempTree::write(
        root.join("main.omg"),
        "use middle::middle;\nmachine root_value() -> u32 { leaf_value() }\n",
    );
    TempTree::write(
        middle.join("middle.omg"),
        "use leaf::leaf;\npub machine middle_value() -> u32 { leaf_value() }\n",
    );
    TempTree::write(
        leaf.join("leaf.omg"),
        "pub machine leaf_value() -> u32 { 42 }\n",
    );

    let transitive_only = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "middle", middle.clone()),
            PackageSourceBinding::new(identity(3), "leaf", leaf.clone()),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("transitive package graph should validate structurally");

    let diagnostics =
        compile_to_checked_with_packages(&root.join("main.omg"), None, transitive_only)
            .expect_err("root may not select a transitive-only leaf declaration");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("`root`")
                && diagnostic.message.contains("`leaf`")
                && diagnostic.message.contains("direct dependency")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    let directly_admitted = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "middle", middle),
            PackageSourceBinding::new(identity(3), "leaf", leaf),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(1), "leaf", identity(3)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("direct leaf admission should validate");

    compile_to_checked_with_packages(&root.join("main.omg"), None, directly_admitted)
        .expect("direct dependency should admit the exact leaf declaration selection");
}

#[test]
fn carried_transitive_type_is_legal_and_retains_its_exact_owner() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");

    TempTree::write(
        root.join("main.omg"),
        "use middle::middle;\nmachine relay() { consume(make()); }\n",
    );
    TempTree::write(
        middle.join("middle.omg"),
        r#"use leaf::leaf;
pub machine make() -> Token { Token { value: 7u64 } }
pub machine consume(value: Token) {}
"#,
    );
    TempTree::write(leaf.join("leaf.omg"), "pub data Token { value: u64; }\n");

    let inputs = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "middle", middle),
            PackageSourceBinding::new(identity(3), "leaf", leaf),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("transitive package graph should validate structurally");

    let checked = compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect("carrying a transitive type through the direct dependency should be legal");
    let relay = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "relay")
        .expect("relay machine")
        .symbol;
    let rows = &checked.facts.flow.semantic_dependencies.rows;

    for kind in [
        psi_checked_trees::CheckedSemanticDependencyKind::NominalIdentity,
        psi_checked_trees::CheckedSemanticDependencyKind::Layout,
        psi_checked_trees::CheckedSemanticDependencyKind::OwnershipBehavior,
    ] {
        assert!(
            rows.iter().any(|row| {
                row.consumer_machine == relay
                && checked.symbols.symbol_package_identity(row.dependency) == Some(identity(3))
                && row.exposure
                    == psi_checked_trees::CheckedSemanticDependencyExposure::PrivateImplementation
                && row.kind == kind
            }),
            "missing exact leaf-owned {kind:?} dependency: {rows:#?}"
        );
    }
}

#[test]
fn statement_call_requires_the_declaration_owner_as_a_direct_dependency() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");

    TempTree::write(
        root.join("main.omg"),
        "use middle::middle;\nmachine root_effect() { leaf_effect(); }\n",
    );
    TempTree::write(
        middle.join("middle.omg"),
        "use leaf::leaf;\npub machine middle_effect() { leaf_effect(); }\n",
    );
    TempTree::write(leaf.join("leaf.omg"), "pub machine leaf_effect() { }\n");

    let transitive_only = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "middle", middle.clone()),
            PackageSourceBinding::new(identity(3), "leaf", leaf.clone()),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("transitive package graph should validate structurally");

    let diagnostics =
        compile_to_checked_with_packages(&root.join("main.omg"), None, transitive_only)
            .expect_err("root may not issue a transitive-only statement call");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("`root`")
                && diagnostic.message.contains("`leaf`")
                && diagnostic.message.contains("`leaf_effect::entry`")
                && diagnostic.message.contains("direct dependency")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    let directly_admitted = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "middle", middle),
            PackageSourceBinding::new(identity(3), "leaf", leaf),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(1), "leaf", identity(3)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("direct leaf admission should validate");

    compile_to_checked_with_packages(&root.join("main.omg"), None, directly_admitted)
        .expect("direct dependency should admit the leaf statement call");
}

#[test]
fn static_type_and_machine_arguments_require_their_declaration_owner() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");

    TempTree::write(
        root.join("main.omg"),
        "use middle::middle;\nmachine root_effect() {\n    accept<Leaf>();\n    invoke<selected>();\n}\n",
    );
    TempTree::write(
        middle.join("middle.omg"),
        r#"use leaf::leaf;
pub machine accept<Element>() { }
pub machine invoke<machine Selected>()
where machine Selected()
{
    Selected();
}
"#,
    );
    TempTree::write(
        leaf.join("leaf.omg"),
        "pub data Leaf { }\npub machine selected() { }\n",
    );

    let transitive_only = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "middle", middle.clone()),
            PackageSourceBinding::new(identity(3), "leaf", leaf.clone()),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("transitive package graph should validate structurally");

    let diagnostics =
        compile_to_checked_with_packages(&root.join("main.omg"), None, transitive_only)
            .expect_err("root may not select transitive-only static arguments");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("`Leaf`")),
        "missing static type selection diagnostic: {diagnostics:#?}"
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("`selected::entry`")),
        "missing static machine selection diagnostic: {diagnostics:#?}"
    );

    let directly_admitted = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "middle", middle),
            PackageSourceBinding::new(identity(3), "leaf", leaf),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(1), "leaf", identity(3)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("direct leaf admission should validate");

    compile_to_checked_with_packages(&root.join("main.omg"), None, directly_admitted)
        .expect("direct dependency should admit static type and machine arguments");
}

#[test]
fn public_type_selection_requires_the_declaration_owner_as_a_direct_dependency() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");

    TempTree::write(
        root.join("main.omg"),
        "use middle::middle;\npub data PublicApi { value: LeafValue; }\n",
    );
    TempTree::write(
        middle.join("middle.omg"),
        "use leaf::leaf;\npub data MiddleValue { value: LeafValue; }\n",
    );
    TempTree::write(
        leaf.join("leaf.omg"),
        "pub data LeafValue { value: u32; }\n",
    );

    let transitive_only = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "middle", middle.clone()),
            PackageSourceBinding::new(identity(3), "leaf", leaf.clone()),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("transitive package graph should validate structurally");

    let diagnostics =
        compile_to_checked_with_packages(&root.join("main.omg"), None, transitive_only)
            .expect_err("a public type may not select a transitive-only declaration");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("`root`")
                && diagnostic.message.contains("`leaf`")
                && diagnostic.message.contains("direct dependency")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    let directly_admitted = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "middle", middle),
            PackageSourceBinding::new(identity(3), "leaf", leaf),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(1), "leaf", identity(3)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("direct leaf admission should validate");

    compile_to_checked_with_packages(&root.join("main.omg"), None, directly_admitted)
        .expect("direct dependency should admit the public type selection");
}

#[test]
fn public_contract_expression_requires_the_selected_declaration_owner() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");

    TempTree::write(
        root.join("main.omg"),
        r#"use middle::middle;
pub machine root_check(value: u64)
requires value in u64::Trusted
{
}
"#,
    );
    TempTree::write(
        middle.join("middle.omg"),
        "use leaf::leaf;\npub machine middle_effect() { }\n",
    );
    TempTree::write(leaf.join("leaf.omg"), "pub domain u64::Trusted;\n");

    let transitive_only = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "middle", middle.clone()),
            PackageSourceBinding::new(identity(3), "leaf", leaf.clone()),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("transitive package graph should validate structurally");

    let diagnostics =
        compile_to_checked_with_packages(&root.join("main.omg"), None, transitive_only)
            .expect_err("public contract may not select a transitive-only domain");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("`root`")
                && diagnostic.message.contains("`leaf`")
                && diagnostic.message.contains("Trusted")
                && diagnostic.message.contains("direct dependency")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    let directly_admitted = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "middle", middle),
            PackageSourceBinding::new(identity(3), "leaf", leaf),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(1), "leaf", identity(3)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("direct leaf admission should validate");

    let checked = compile_to_checked_with_packages(&root.join("main.omg"), None, directly_admitted)
        .expect("direct dependency should admit the public contract domain selection");
    assert!(checked.authored_declaration_selections().iter().any(|selection| {
        selection.kind()
            == psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::DomainMembership
            && selection.exposure()
                == psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface
            && matches!(
                selection.target(),
                psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(target)
                    if checked.symbols.display_path(target.selected_symbol(), "::").contains("Trusted")
            )
    }));
}

#[test]
fn proposition_visibility_gates_public_and_cross_package_selection() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let leaf = tree.package("leaf");

    TempTree::write(
        root.join("main.omg"),
        "use leaf::leaf;\npub machine inspect()\nrequires leaf_ready()\n{ }\n",
    );
    TempTree::write(leaf.join("leaf.omg"), "proposition leaf_ready();\n");

    let inputs = || {
        PackageCompilationInputs::new(
            identity(1),
            vec![
                PackageSourceBinding::new(identity(1), "root", root.clone()),
                PackageSourceBinding::new(identity(2), "leaf", leaf.clone()),
            ],
            vec![PackageDependencyBinding::new(
                identity(1),
                "leaf",
                identity(2),
            )],
        )
        .expect("direct proposition dependency graph should validate")
    };

    let diagnostics = compile_to_checked_with_packages(&root.join("main.omg"), None, inputs())
        .expect_err("a direct dependency does not publish its private proposition");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("private proposition")
                && diagnostic.message.contains("leaf_ready")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    TempTree::write(leaf.join("leaf.omg"), "pub proposition leaf_ready();\n");
    compile_to_checked_with_packages(&root.join("main.omg"), None, inputs())
        .expect("an explicitly public proposition should be nameable by a direct dependent");

    TempTree::write(
        root.join("main.omg"),
        "proposition local_ready();\npub machine inspect()\nrequires local_ready()\n{ }\n",
    );
    let root_only = PackageCompilationInputs::new(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .expect("root-only package graph should validate");
    let diagnostics = compile_to_checked_with_packages(&root.join("main.omg"), None, root_only)
        .expect_err("a public interface may not expose its package-private proposition");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("public interface selects private proposition")
                && diagnostic.message.contains("local_ready")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    TempTree::write(
        root.join("main.omg"),
        "proposition local_ready();\nmachine inspect()\nrequires local_ready()\n{ }\n",
    );
    let root_only = PackageCompilationInputs::new(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .expect("root-only private implementation graph should validate");
    compile_to_checked_with_packages(&root.join("main.omg"), None, root_only)
        .expect("private implementation may select its package-private proposition");

    TempTree::write(
        root.join("main.omg"),
        "proposition local_ready();\npub proposition exposed() = local_ready();\n",
    );
    let root_only = PackageCompilationInputs::new(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .expect("root-only transparent proposition graph should validate");
    let diagnostics = compile_to_checked_with_packages(&root.join("main.omg"), None, root_only)
        .expect_err("a public transparent proposition may not hide a private endpoint");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("public interface selects private proposition")
                && diagnostic.message.contains("local_ready")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn const_visibility_gates_public_and_cross_package_selection() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let leaf = tree.package("leaf");

    TempTree::write(
        root.join("main.omg"),
        "use leaf::leaf;\npub proposition within_limit() = LEAF_LIMIT == 4;\n",
    );
    TempTree::write(leaf.join("leaf.omg"), "const LEAF_LIMIT: u64 = 4;\n");

    let inputs = || {
        PackageCompilationInputs::new(
            identity(1),
            vec![
                PackageSourceBinding::new(identity(1), "root", root.clone()),
                PackageSourceBinding::new(identity(2), "leaf", leaf.clone()),
            ],
            vec![PackageDependencyBinding::new(
                identity(1),
                "leaf",
                identity(2),
            )],
        )
        .expect("direct const dependency graph should validate")
    };

    let diagnostics = compile_to_checked_with_packages(&root.join("main.omg"), None, inputs())
        .expect_err("a direct dependency does not publish its private const");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("private const")
                && diagnostic.message.contains("LEAF_LIMIT")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    TempTree::write(leaf.join("leaf.omg"), "pub const LEAF_LIMIT: u64 = 4;\n");
    compile_to_checked_with_packages(&root.join("main.omg"), None, inputs())
        .expect("an explicitly public const should be nameable by a direct dependent");

    TempTree::write(
        root.join("main.omg"),
        "const LOCAL_LIMIT: u64 = 4;\npub proposition within_limit() = LOCAL_LIMIT == 4;\n",
    );
    let root_only = PackageCompilationInputs::new(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .expect("root-only package graph should validate");
    let diagnostics = compile_to_checked_with_packages(&root.join("main.omg"), None, root_only)
        .expect_err("a public interface may not expose its package-private const");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("public interface selects private const")
                && diagnostic.message.contains("LOCAL_LIMIT")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    TempTree::write(
        root.join("main.omg"),
        "const LOCAL_LIMIT: u64 = 4;\nmachine within_limit() -> bool { LOCAL_LIMIT == 4 }\n",
    );
    let root_only = PackageCompilationInputs::new(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .expect("root-only private implementation graph should validate");
    compile_to_checked_with_packages(&root.join("main.omg"), None, root_only)
        .expect("private implementation may select its package-private const");

    TempTree::write(
        root.join("main.omg"),
        "data LocalToken { value: u64; }\npub const TOKEN: LocalToken = LocalToken { value: 4 };\n",
    );
    let root_only = PackageCompilationInputs::new(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .expect("root-only public const graph should validate structurally");
    let diagnostics = compile_to_checked_with_packages(&root.join("main.omg"), None, root_only)
        .expect_err("a public const may not expose its package-private data type");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("public const `TOKEN`")
                && diagnostic.message.contains("private data `LocalToken`")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    TempTree::write(
        root.join("main.omg"),
        "pub data LocalToken { value: u64; }\npub const TOKEN: LocalToken = LocalToken { value: 4 };\n",
    );
    let root_only = PackageCompilationInputs::new(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .expect("root-only published public const graph should validate");
    compile_to_checked_with_packages(&root.join("main.omg"), None, root_only)
        .expect("a public const may expose an explicitly public structural data type");
}

#[test]
fn operator_visibility_gates_public_and_cross_package_selection() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let leaf = tree.package("leaf");

    TempTree::write(
        root.join("main.omg"),
        "use leaf::leaf;\nmachine inspect(value: Token) -> bool { value < value }\n",
    );
    TempTree::write(
        leaf.join("leaf.omg"),
        "pub data Token [copy] { value: u64; }\noperator < Token::less(left: Token, right: Token) -> bool;\n",
    );

    let inputs = || {
        PackageCompilationInputs::new(
            identity(1),
            vec![
                PackageSourceBinding::new(identity(1), "root", root.clone()),
                PackageSourceBinding::new(identity(2), "leaf", leaf.clone()),
            ],
            vec![PackageDependencyBinding::new(
                identity(1),
                "leaf",
                identity(2),
            )],
        )
        .expect("direct operator dependency graph should validate")
    };

    let diagnostics = compile_to_checked_with_packages(&root.join("main.omg"), None, inputs())
        .expect_err("a direct dependency does not publish its private operator");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("private operator")
                && diagnostic.message.contains("Token::less")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    TempTree::write(
        leaf.join("leaf.omg"),
        "pub data Token [copy] { value: u64; }\npub operator < Token::less(left: Token, right: Token) -> bool;\n",
    );
    compile_to_checked_with_packages(&root.join("main.omg"), None, inputs())
        .expect("an explicitly public operator should be nameable by a direct dependent");

    TempTree::write(
        root.join("main.omg"),
        "pub data Token [copy] { value: u64; }\noperator < Token::less(left: Token, right: Token) -> bool;\npub machine inspect(value: Token)\nrequires value < value\n{ }\n",
    );
    let root_only = PackageCompilationInputs::new(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .expect("root-only public operator interface should validate structurally");
    let diagnostics = compile_to_checked_with_packages(&root.join("main.omg"), None, root_only)
        .expect_err("a public interface may not select its package-private operator");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("public interface selects private operator")
                && diagnostic.message.contains("Token::less")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    TempTree::write(
        root.join("main.omg"),
        "pub data Token [copy] { value: u64; }\noperator < Token::less(left: Token, right: Token) -> bool;\nmachine inspect(value: Token) -> bool { value < value }\n",
    );
    let root_only = PackageCompilationInputs::new(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .expect("root-only private operator implementation should validate");
    compile_to_checked_with_packages(&root.join("main.omg"), None, root_only)
        .expect("private implementation may select its package-private operator");
}

#[test]
fn public_callable_bound_requires_the_named_conformance_owner() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");

    TempTree::write(
        root.join("main.omg"),
        r#"use middle::middle;
pub machine root_accept<Element>(value: &Element)
where Element satisfies Good::Primary
{
}
"#,
    );
    TempTree::write(
        middle.join("middle.omg"),
        "use leaf::leaf;\npub machine middle_effect() { }\n",
    );
    TempTree::write(
        leaf.join("leaf.omg"),
        r#"pub trait Marker { }
pub data Good { }
Primary: Good satisfies Marker;
"#,
    );

    let transitive_only = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "middle", middle.clone()),
            PackageSourceBinding::new(identity(3), "leaf", leaf.clone()),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("transitive package graph should validate structurally");

    let diagnostics =
        compile_to_checked_with_packages(&root.join("main.omg"), None, transitive_only)
            .expect_err("public callable may not select transitive-only conformance evidence");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("`root`")
                && diagnostic.message.contains("`leaf`")
                && (diagnostic.message.contains("Good") || diagnostic.message.contains("Primary"))
                && diagnostic.message.contains("direct dependency")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    let directly_admitted = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "middle", middle),
            PackageSourceBinding::new(identity(3), "leaf", leaf),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(1), "leaf", identity(3)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("direct leaf admission should validate");

    let checked = compile_to_checked_with_packages(&root.join("main.omg"), None, directly_admitted)
        .expect("direct dependency should admit the public callable bound");
    assert!(checked.authored_declaration_selections().iter().any(|selection| {
        selection.kind()
            == psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Conformance
            && selection.exposure()
                == psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface
            && matches!(
                selection.target(),
                psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(target)
                    if checked.symbols.display_path(target.selected_symbol(), "::").contains("Primary")
            )
    }));
}

#[test]
fn inferred_conformance_requires_the_declaration_owner_as_a_direct_dependency() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");

    TempTree::write(
        root.join("main.omg"),
        "use middle::middle;\nmachine root_effect() {\n    let value: Good = Good {};\n    accepts(value);\n}\n",
    );
    TempTree::write(
        middle.join("middle.omg"),
        r#"use leaf::leaf;
pub machine accepts<Element>(value: Element)
where Element satisfies Marker
{
}
"#,
    );
    TempTree::write(
        leaf.join("leaf.omg"),
        r#"pub trait Marker { }
pub data Good { }
GoodMarker: Good satisfies Marker;
"#,
    );

    let transitive_only = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "middle", middle.clone()),
            PackageSourceBinding::new(identity(3), "leaf", leaf.clone()),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("transitive package graph should validate structurally");

    let diagnostics =
        compile_to_checked_with_packages(&root.join("main.omg"), None, transitive_only)
            .expect_err("root may not infer a transitive-only leaf conformance");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("`root`")
                && diagnostic.message.contains("`leaf`")
                && diagnostic.message.contains("`GoodMarker`")
                && diagnostic.message.contains("direct dependency")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    let directly_admitted = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "middle", middle),
            PackageSourceBinding::new(identity(3), "leaf", leaf),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(1), "leaf", identity(3)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("direct leaf admission should validate");

    compile_to_checked_with_packages(&root.join("main.omg"), None, directly_admitted)
        .expect("direct dependency should admit the inferred leaf conformance");
}

#[test]
fn const_generic_evaluation_requires_direct_authority_before_execution() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");

    TempTree::write(
        root.join("main.omg"),
        "use middle::middle;\ndata FixedBuffer<const N: u64> { items: [u8; N]; }\ndata Main { buffer: FixedBuffer<leaf_size()>; }\n",
    );
    TempTree::write(
        middle.join("middle.omg"),
        "use leaf::leaf;\npub machine middle_size() -> u64 { leaf_size() + 0 }\n",
    );
    TempTree::write(
        leaf.join("leaf.omg"),
        "pub machine leaf_size() -> u64 { 4 }\n",
    );

    let inputs = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "middle", middle.clone()),
            PackageSourceBinding::new(identity(3), "leaf", leaf.clone()),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("transitive package graph should validate structurally");

    let diagnostics =
        compile_to_checked_with_packages(&root.join("main.omg"), None, inputs.clone())
            .expect_err("early const-generic execution may not select a transitive-only package");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("const-generic evaluation")
                && diagnostic.message.contains("build-time invocation")
                && diagnostic.message.contains("direct dependency authority")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    TempTree::write(
        root.join("main.omg"),
        "use middle::middle;\ndata FixedBuffer<const N: u64> { items: [u8; N]; }\ndata Main { buffer: FixedBuffer<middle_size()>; }\n",
    );
    compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect("each machine in the build-time call closure may select its own direct dependency");
}

#[test]
fn build_time_call_closure_rejects_internal_undeclared_package_selection() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let middle = tree.package("middle");

    TempTree::write(
        root.join("main.omg"),
        "use middle::middle;\nconst ROOT_SIZE: u64 = 4;\ndata FixedBuffer<const N: u64> { items: [u8; N]; }\ndata Main { buffer: FixedBuffer<middle_size()>; }\n",
    );
    TempTree::write(
        middle.join("middle.omg"),
        "pub machine middle_size() -> u64 { ROOT_SIZE }\n",
    );

    let inputs = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "middle", middle),
        ],
        vec![PackageDependencyBinding::new(
            identity(1),
            "middle",
            identity(2),
        )],
    )
    .expect("one-way direct dependency should validate structurally");

    let diagnostics = compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect_err(
            "dependency code may not select root declarations without dependency authority",
        );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("const-generic evaluation")
                && diagnostic
                    .message
                    .contains("authored StaticPathSegment selection")
                && diagnostic.message.contains("direct dependency authority")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn fixed_array_evaluation_requires_direct_authority_before_execution() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");

    TempTree::write(
        root.join("main.omg"),
        "use middle::middle;\ndata Main { items: [u8; leaf_size()]; }\n",
    );
    TempTree::write(
        middle.join("middle.omg"),
        "use leaf::leaf;\npub machine middle_size() -> u64 { leaf_size() }\n",
    );
    TempTree::write(
        leaf.join("leaf.omg"),
        "pub machine leaf_size() -> u64 { 4 }\n",
    );

    let inputs = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "middle", middle.clone()),
            PackageSourceBinding::new(identity(3), "leaf", leaf.clone()),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("transitive package graph should validate structurally");

    let diagnostics = compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect_err("early fixed-array execution may not select a transitive-only package");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("fixed-array length")
                && diagnostic.message.contains("build-time invocation")
                && diagnostic.message.contains("direct dependency authority")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    let directly_admitted = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "middle", middle),
            PackageSourceBinding::new(identity(3), "leaf", leaf),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(1), "leaf", identity(3)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("direct fixed-array callee admission should validate structurally");
    compile_to_checked_with_packages(&root.join("main.omg"), None, directly_admitted)
        .expect("direct package authority should admit fixed-array evaluation");
}

#[test]
fn const_domain_evaluation_requires_direct_authority_before_execution() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");

    TempTree::write(
        root.join("main.omg"),
        r#"use middle::middle;
domain u64::BufferSize
requires
    leaf_accepts(self);
data FixedBuffer<const N: u64>
where
    N in BufferSize,
{
    items: [u8; N];
}
data Main { buffer: FixedBuffer<4>; }
"#,
    );
    TempTree::write(
        middle.join("middle.omg"),
        "use leaf::leaf;\npub machine middle_accepts(value: u64) -> bool { leaf_accepts(value) }\n",
    );
    TempTree::write(
        leaf.join("leaf.omg"),
        "pub machine leaf_accepts(value: u64) -> bool { true }\n",
    );

    let inputs = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "middle", middle),
            PackageSourceBinding::new(identity(3), "leaf", leaf),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("transitive package graph should validate structurally");

    let diagnostics = compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect_err("early const-domain execution may not select a transitive-only package");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("const domain fact evaluation")
                && diagnostic.message.contains("build-time invocation")
                && diagnostic.message.contains("direct dependency authority")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn plan_laid_evaluation_requires_direct_authority_before_execution() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");

    TempTree::write(
        root.join("main.omg"),
        "use middle::middle;\ndata Payload { value: u32; }\ndata Main { payload: LeafLayout<Payload>; }\n",
    );
    TempTree::write(middle.join("middle.omg"), "use leaf::leaf;\n");
    TempTree::write(
        leaf.join("leaf.omg"),
        r#"use omega::language::core::layout;
pub data LeafLayout { entries: [FieldEntry; 64]; }
pub machine LeafLayout::plan(&mut self, schema: Schema) -> Plan {
    self.entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 }
    };
    Plan {
        entries: self.entries,
        entry_count: 1,
        size_fixed: 4,
        size_is_dynamic: false,
        align: 4
    }
}
"#,
    );

    let inputs = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "middle", middle.clone()),
            PackageSourceBinding::new(identity(3), "leaf", leaf.clone()),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("transitive package graph should validate structurally");

    let diagnostics = compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect_err("early plan-laid execution may not select a transitive-only package");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("plan-laid value type")
                && diagnostic.message.contains("build-time invocation")
                && diagnostic.message.contains("direct dependency authority")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    let directly_admitted = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "middle", middle),
            PackageSourceBinding::new(identity(3), "leaf", leaf),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(1), "leaf", identity(3)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("direct policy admission should validate structurally");
    compile_to_checked_with_packages(&root.join("main.omg"), None, directly_admitted)
        .expect("direct package authority should admit plan-laid policy execution");
}

#[test]
fn placed_view_evaluation_requires_direct_authority_before_execution() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");

    TempTree::write(
        root.join("main.omg"),
        "use middle::middle;\ndata Payload { value: u32; }\nmachine inspect(view: &Placed<LeafPlacement, Payload>) {}\n",
    );
    TempTree::write(middle.join("middle.omg"), "use leaf::leaf;\n");
    TempTree::write(
        leaf.join("leaf.omg"),
        r#"use omega::language::core::layout;
pub data LeafPlacement {
    entries: [FieldEntry; 64];
    services: [u64; 32];
}
pub machine LeafPlacement::plan(&mut self, schema: Schema) -> PlacementPlan {
    self.entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 }
    };
    let access: AccessPlan = AccessPlan::inaccessible(schema);
    PlacementPlan {
        layout: Plan {
            entries: self.entries,
            entry_count: 1,
            size_fixed: 4,
            size_is_dynamic: false,
            align: 4
        },
        access: access,
        reach: BoundaryReach {
            services: self.services,
            service_count: 0
        }
    }
}
"#,
    );

    let inputs = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "middle", middle.clone()),
            PackageSourceBinding::new(identity(3), "leaf", leaf.clone()),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("transitive package graph should validate structurally");

    let diagnostics = compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect_err("early placed-view execution may not select a transitive-only package");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("placed view")
                && diagnostic.message.contains("build-time invocation")
                && diagnostic.message.contains("direct dependency authority")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    let directly_admitted = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "middle", middle),
            PackageSourceBinding::new(identity(3), "leaf", leaf),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(1), "leaf", identity(3)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("direct placement-policy admission should validate structurally");
    compile_to_checked_with_packages(&root.join("main.omg"), None, directly_admitted)
        .expect("direct package authority should admit placed-view policy execution");
}

#[test]
fn package_selection_admission_precedes_build_machine_side_effects() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");

    TempTree::write(
        root.join("main.omg"),
        "use middle::middle;\nconst RESULT: u32 = 42;\n",
    );
    TempTree::write(
        root.join("build.omg"),
        r#"use omega::language::std::filesystem_host;

target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }

machine build(builder: &mut Build)
reaches FilesystemHost
invokes FilesystemHost;
{
    let marker: &[u8] in Path = builder.output.resolve("build-ran.marker");
    let descriptor: i32 = builder.filesystem.create(marker, 438);
    let closed: i32 = builder.filesystem.close(descriptor);
    let transitive: u32 = leaf_value();
}
"#,
    );
    TempTree::write(
        middle.join("middle.omg"),
        "use leaf::leaf;\npub machine middle_value() -> u32 { leaf_value() }\n",
    );
    TempTree::write(
        leaf.join("leaf.omg"),
        "pub machine leaf_value() -> u32 { 42 }\n",
    );

    let inputs = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "middle", middle),
            PackageSourceBinding::new(identity(3), "leaf", leaf),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("transitive package graph should validate structurally");

    let checked_build = tree.0.join("checked-build");
    let checked_diagnostics = compile_to_checked_with_packages_in_build_dir(
        &root.join("main.omg"),
        &checked_build,
        None,
        inputs.clone(),
    )
    .expect_err("checked package compilation must reject the transitive selection");
    assert!(
        checked_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("direct dependency")),
        "unexpected checked diagnostics: {checked_diagnostics:#?}"
    );
    assert!(
        !checked_build.join("build-ran.marker").exists(),
        "checked package admission must reject before build execution"
    );

    if let Some(target_name) = host_target_name() {
        let native_build = tree.0.join("native-build");
        let native_diagnostics = compile_with_packages(
            CompileOptions {
                root_path: root.join("main.omg"),
                build_dir: Some(native_build.clone()),
                target_name: Some(target_name.to_owned()),
                write_output: false,
            },
            inputs,
        )
        .expect_err("native package compilation must reject the transitive selection");
        assert!(
            native_diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("direct dependency")),
            "unexpected native diagnostics: {native_diagnostics:#?}"
        );
        assert!(
            !native_build.join("build-ran.marker").exists(),
            "native package admission must reject before build execution"
        );
    }
}

#[test]
fn dependency_provider_plan_retains_exact_dependency_package_provenance() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let dependency = tree.package("dependency");

    TempTree::write(root.join("main.omg"), "use dep::provider;\n");
    TempTree::write(
        root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.select_provider<Pair, Provider>();
}
"#,
    );
    TempTree::write(
        dependency.join("provider.omg"),
        r#"boundary trait Pair { machine first(); }
data Provider { }
machine Provider::first() satisfies Pair::first via Binding::VtableSlot(1);
"#,
    );

    let inputs = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "dependency", dependency),
        ],
        vec![PackageDependencyBinding::new(
            identity(1),
            "dep",
            identity(2),
        )],
    )
    .expect("reconciled provider graph should validate");

    let checked = compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect("dependency provider should check");
    assert!(checked.authored_declaration_selections().iter().any(|selection| {
        selection.target()
            == psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Intrinsic(
                psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic::BuildProviderSelection,
            )
    }));
    let [plan] = checked.selected_provider_plans().plans() else {
        panic!("one selected dependency provider plan")
    };
    assert_eq!(plan.origin_package_identity, Some(identity(2)));
    assert_eq!(plan.provider_type_package_identity, Some(identity(2)));
    assert_eq!(plan.schema.trait_package_identity, Some(identity(2)));
    assert_eq!(
        plan.schema.methods[0].requirement_owner_package_identity,
        Some(identity(2))
    );
    assert_eq!(plan.origin_package, "");
}

#[test]
fn dependency_build_files_cannot_join_the_program() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let dependency = tree.package("dependency");
    TempTree::write(root.join("main.omg"), "use dep::build;\n");
    TempTree::write(
        dependency.join("build.omg"),
        "machine build(builder: &mut Build) { }\n",
    );

    let inputs = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "dependency", dependency),
        ],
        vec![PackageDependencyBinding::new(
            identity(1),
            "dep",
            identity(2),
        )],
    )
    .expect("reconciled bindings should validate");

    let diagnostics = compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect_err("dependency build file import must reject");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("may not load dependency build file")),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[cfg(unix)]
#[test]
fn dependency_import_symlink_escape_rejects() {
    use std::os::unix::fs::symlink;

    let tree = TempTree::new();
    let root = tree.package("root");
    let dependency = tree.package("dependency");
    let outside = tree.package("outside");
    TempTree::write(root.join("main.omg"), "use dep::escape;\n");
    TempTree::write(outside.join("secret.omg"), "const SECRET: u32 = 42;\n");
    symlink(outside.join("secret.omg"), dependency.join("escape.omg"))
        .expect("create escaping import symlink");

    let inputs = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "dependency", dependency),
        ],
        vec![PackageDependencyBinding::new(
            identity(1),
            "dep",
            identity(2),
        )],
    )
    .expect("reconciled bindings should validate");

    let diagnostics = compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect_err("symlink import escape must reject");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("escapes expected source root")),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[cfg(unix)]
#[test]
fn root_build_companion_symlink_escape_rejects_before_loading() {
    use std::os::unix::fs::symlink;

    let tree = TempTree::new();
    let root = tree.package("root");
    let outside = tree.package("outside");
    TempTree::write(root.join("main.omg"), "const RESULT: u32 = 42;\n");
    TempTree::write(
        outside.join("hostile-build.omg"),
        "machine build(builder: &mut Build) { }\n",
    );
    symlink(outside.join("hostile-build.omg"), root.join("build.omg"))
        .expect("create escaping build companion symlink");

    let inputs = PackageCompilationInputs::new(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .expect("root package input should validate");

    let diagnostics = compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect_err("root build companion escape must reject");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("escapes every reconciled source root")),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn native_package_entrypoint_uses_the_same_reconciled_binding_mode() {
    let Some(target_name) = host_target_name() else {
        return;
    };
    let tree = TempTree::new();
    let root = tree.package("root");
    let admitted = tree.package("admitted");
    let malicious = tree.package("malicious");

    TempTree::write(
        root.join("main.omg"),
        r#"use dep::values;
boundary trait Console { machine exit_process(return_code: i32); }
data Main { console: Console; }
machine Main::main(&mut self) {
    transition ANSWER == 42 { true -> yes() _ -> no() }
    state yes(&mut self) { self.console.exit_process(0); }
    state no(&mut self) { self.console.exit_process(1); }
}
"#,
    );
    TempTree::write(
        root.join("build.omg"),
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) {
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
    builder.roots.bind(linux_arm64::ProgramEntry, Main::main);
    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);
    builder.depend_as("dep", Source::Path { location: "../malicious" });
}
"#,
    );
    TempTree::write(admitted.join("values.omg"), "pub const ANSWER: u32 = 42;\n");
    TempTree::write(malicious.join("values.omg"), "this is not Omega source\n");

    let inputs = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "admitted", admitted),
        ],
        vec![PackageDependencyBinding::new(
            identity(1),
            "dep",
            identity(2),
        )],
    )
    .expect("reconciled package graph should validate");

    compile_with_packages(
        CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: Some(tree.0.join("build-output")),
            target_name: Some(target_name.to_owned()),
            write_output: false,
        },
        inputs,
    )
    .expect("native package compilation should use reconciled imports only");
}

fn host_target_name() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Some("windows_x64"),
        ("linux", "x86_64") => Some("linux_x64"),
        ("linux", "aarch64") => Some("linux_arm64"),
        ("macos", "aarch64") => Some("macos_arm64"),
        _ => None,
    }
}
