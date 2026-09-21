use super::{TempTree, host_target_name, identity};
use compiler::{
    CheckedCompileRequest, CompileOptions, CompileRequest, compile, compile_to_checked,
};
use package_compilation::{
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};

#[test]
fn machine_satisfies_retains_the_exact_result_dispatch_overload() {
    let tree = TempTree::new();
    let root = tree.package("root");
    TempTree::write(
        root.join("main.omg"),
        r#"pub domain u64::First;
pub domain u64::Second;
pub trait Choice {
    machine choose(value: u64) -> u64 in First;
    machine choose(value: u64) -> u64 in Second;
}
boundary machine choose_first(value: u64) -> u64 in First
    satisfies Choice::choose;
"#,
    );
    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .expect("root-only overload graph should close");
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("result dispatch should select one exact requirement overload");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "choose_first")
        .expect("overload satisfier");
    let [conformance] = checked.machine_trait_conformances(machine) else {
        panic!("one exact overload edge")
    };
    let choice = checked
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Choice")
        .expect("choice trait");
    let selected = checked
        .trait_machine_signatures(choice)
        .iter()
        .find(|requirement| requirement.symbol == conformance.requirement_symbol)
        .expect("settled requirement symbol must name one Choice overload");
    assert!(
        checked
            .normalized_result_dispatch_set(selected.return_type)
            .identity()
            .contains("First")
    );
}

#[test]
fn package_compilation_rejects_authored_reserved_cleanup_selection() {
    let tree = TempTree::new();
    let root = tree.package("root");

    TempTree::write(
        root.join("main.omg"),
        r#"
data Resource { value: i32; }
machine Resource::drop(&mut self) {}
machine misuse(resource: &mut Resource) {
    resource.drop();
}
"#,
    );
    TempTree::write(
        root.join("build.omg"),
        "machine build(builder: &mut Build) { builder.package(\"root\"); }\n",
    );

    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .expect("root-only package graph should validate");

    let checked_diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs.clone()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("package source may not invoke its reserved cleanup hook");
    assert!(
        checked_diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("reserved cleanup machine `Resource::drop` is compiler-selected")),
        "unexpected checked diagnostics: {checked_diagnostics:#?}"
    );

    if let Some(target_name) = host_target_name() {
        let native_diagnostics = compile(
            CompileRequest::new(CompileOptions {
                root_path: root.join("main.omg"),
                build_dir: Some(tree.0.join("native-build")),
                target_name: Some(target_name.to_owned()),
            })
            .with_package_inputs(inputs),
        )
        .and_then(compiler::CompileOutcomes::into_single_report)
        .expect_err("native package compilation must apply the same cleanup gate");
        assert!(
            native_diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("reserved cleanup machine `Resource::drop` is compiler-selected")),
            "unexpected native diagnostics: {native_diagnostics:#?}"
        );
    }
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

    let inputs = PackageCompilationInputs::new_package(
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

    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("carrying a transitive type through the direct dependency should be legal");
    let relay = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "relay")
        .expect("relay machine")
        .symbol;
    let rows = &checked.facts.flow.semantic_dependencies.rows;

    for kind in [
        checked_trees::CheckedSemanticDependencyKind::NominalIdentity,
        checked_trees::CheckedSemanticDependencyKind::Layout,
        checked_trees::CheckedSemanticDependencyKind::OwnershipBehavior,
    ] {
        assert!(
            rows.iter().any(|row| {
                row.consumer_machine == relay
                    && checked.symbols.symbol_package_identity(row.dependency) == Some(identity(3))
                    && row.exposure
                        == checked_trees::CheckedSemanticDependencyExposure::PrivateImplementation
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

    let transitive_only = PackageCompilationInputs::new_package(
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

    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(transitive_only),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
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

    let directly_admitted = PackageCompilationInputs::new_package(
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

    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(directly_admitted),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
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

    let transitive_only = PackageCompilationInputs::new_package(
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

    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(transitive_only),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
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

    let directly_admitted = PackageCompilationInputs::new_package(
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

    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(directly_admitted),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
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

    let transitive_only = PackageCompilationInputs::new_package(
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

    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(transitive_only),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("a public type may not select a transitive-only declaration");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("`root`")
                && diagnostic.message.contains("`leaf`")
                && diagnostic.message.contains("direct dependency")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    let directly_admitted = PackageCompilationInputs::new_package(
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

    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(directly_admitted),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
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

    let transitive_only = PackageCompilationInputs::new_package(
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

    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(transitive_only),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
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

    let directly_admitted = PackageCompilationInputs::new_package(
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

    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(directly_admitted),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("direct dependency should admit the public contract domain selection");
    assert!(checked.authored_declaration_selections().iter().any(|selection| {
        selection.kind()
            == language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::DomainMembership
            && selection.exposure()
                == language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface
            && matches!(
                selection.target(),
                language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(target)
                    if checked.symbols.display_path(target.selected_symbol(), "::").contains("Trusted")
            )
    }));
}

#[test]
fn public_qualification_cast_requires_the_selected_declaration_owner() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");

    TempTree::write(
        root.join("main.omg"),
        r#"use middle::middle;
pub machine root_check(value: u8)
requires (value as u64 in Trusted) == 1
{
}
"#,
    );
    TempTree::write(
        middle.join("middle.omg"),
        "use leaf::leaf;\npub machine middle_effect() { }\n",
    );
    TempTree::write(leaf.join("leaf.omg"), "pub domain u64::Trusted;\n");

    let transitive_only = PackageCompilationInputs::new_package(
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

    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(transitive_only),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("a qualification cast may not select a transitive-only domain");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("`root`")
                && diagnostic.message.contains("`leaf`")
                && diagnostic.message.contains("Trusted")
                && diagnostic.message.contains("direct dependency")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    let directly_admitted = PackageCompilationInputs::new_package(
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

    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(directly_admitted),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("direct dependency should admit the qualification-cast domain selection");
}

#[test]
fn public_zero_value_type_requires_the_selected_declaration_owner() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");

    TempTree::write(
        root.join("main.omg"),
        r#"use middle::middle;
pub proposition root_zero() =
    zero_value<LeafValue>() == zero_value<LeafValue>();
"#,
    );
    TempTree::write(
        middle.join("middle.omg"),
        "use leaf::leaf;\npub machine middle_effect() { }\n",
    );
    TempTree::write(leaf.join("leaf.omg"), "pub data LeafValue {}\n");

    let transitive_only = PackageCompilationInputs::new_package(
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

    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(transitive_only),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("a public zero-value type may not select a transitive-only declaration");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("`root`")
                && diagnostic.message.contains("`leaf`")
                && diagnostic.message.contains("LeafValue")
                && diagnostic.message.contains("direct dependency")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    let directly_admitted = PackageCompilationInputs::new_package(
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

    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(directly_admitted),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("direct dependency should admit the public zero-value type selection");
}

#[test]
fn ordinary_declaration_visibility_gates_cross_package_selection() {
    let cases = [
        (
            "data",
            "LeafValue",
            "use leaf::leaf;\nmachine inspect(value: LeafValue) { }\n",
            "data LeafValue { value: u64; }\n",
            "pub data LeafValue { value: u64; }\n",
        ),
        (
            "domain",
            "Trusted",
            "use leaf::leaf;\nmachine inspect(value: u64 in u64::Trusted) { }\n",
            "domain u64::Trusted;\n",
            "pub domain u64::Trusted;\n",
        ),
        (
            "machine",
            "leaf_action",
            "use leaf::leaf;\nmachine inspect() { leaf_action(); }\n",
            "machine leaf_action() { }\n",
            "pub machine leaf_action() { }\n",
        ),
        (
            "trait",
            "LeafService",
            "use leaf::leaf;\nmachine inspect(service: LeafService) { }\n",
            "trait LeafService { machine act(); }\n",
            "pub trait LeafService { machine act(); }\n",
        ),
    ];

    for (kind, name, root_source, private_leaf, public_leaf) in cases {
        let tree = TempTree::new();
        let root = tree.package("root");
        let leaf = tree.package("leaf");
        TempTree::write(root.join("main.omg"), root_source);
        TempTree::write(leaf.join("leaf.omg"), private_leaf);

        let inputs = || {
            PackageCompilationInputs::new_package(
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
            .expect("direct dependency graph should validate")
        };

        let diagnostics = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(inputs()),
            ..CheckedCompileRequest::new(&root.join("main.omg"), None)
        })
        .expect_err("a direct dependency does not implicitly publish declarations");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains(&format!("private {kind}"))
                    && diagnostic.message.contains(name)
            }),
            "unexpected {kind} diagnostics: {diagnostics:#?}"
        );

        TempTree::write(leaf.join("leaf.omg"), public_leaf);
        compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(inputs()),
            ..CheckedCompileRequest::new(&root.join("main.omg"), None)
        })
        .unwrap_or_else(|diagnostics| {
            panic!("public {kind} should be selectable: {diagnostics:#?}")
        });
    }
}

#[test]
fn named_conformance_visibility_gates_cross_package_selection() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let leaf = tree.package("leaf");
    TempTree::write(
        root.join("main.omg"),
        r#"use leaf::leaf;
machine inspect(left: &Card, right: &Card) -> bool {
    choose<Card, PowerOrder>(left, right)
}
"#,
    );
    let leaf_source = |visibility: &str| {
        format!(
            r#"pub data Card {{ rank: i32; }}
pub trait Ranked {{
    machine Self::before(&self, other: &Self) -> bool;
}}
{visibility}PowerOrder: Card satisfies Ranked {{
    machine before(&self, other: &Card) -> bool {{ self.rank < other.rank }}
}}
pub machine choose<Element, Order: Element satisfies Ranked>(
    left: &Element,
    right: &Element
) -> bool {{
    Order::before(left, right)
}}
"#,
        )
    };
    TempTree::write(leaf.join("leaf.omg"), &leaf_source(""));

    let inputs = || {
        PackageCompilationInputs::new_package(
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
        .expect("direct conformance dependency graph should validate")
    };

    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("a direct dependency does not publish a private conformance");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("private conformance")
                && diagnostic.message.contains("PowerOrder")
        }),
        "unexpected private-conformance diagnostics: {diagnostics:#?}"
    );

    TempTree::write(leaf.join("leaf.omg"), &leaf_source("pub "));
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .unwrap_or_else(|diagnostics| {
        panic!("public conformance should be selectable: {diagnostics:#?}")
    });
}

#[test]
fn named_conformance_visibility_gates_public_interface_citation() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let inputs = || {
        PackageCompilationInputs::new_package(
            identity(1),
            vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
            Vec::new(),
        )
        .expect("root-only conformance graph should validate")
    };
    let source = |conformance_visibility: &str, machine_visibility: &str| {
        format!(
            r#"pub data Card {{}}
pub trait Ranked {{}}
{conformance_visibility}PowerOrder: Card satisfies Ranked {{}}
{machine_visibility}machine inspect<Element>(value: &Element)
where Element satisfies Card::PowerOrder
{{}}
"#,
        )
    };

    TempTree::write(root.join("main.omg"), &source("", "pub "));
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("a public interface cannot cite its package-private conformance");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("public interface selects private conformance")
                && diagnostic.message.contains("PowerOrder")
        }),
        "unexpected public-conformance diagnostics: {diagnostics:#?}"
    );

    TempTree::write(root.join("main.omg"), &source("pub ", "pub "));
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("a public interface may cite its public conformance");

    TempTree::write(root.join("main.omg"), &source("", "boundary "));
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("a boundary interface cannot cite its package-private conformance");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("public interface selects private conformance")
                && diagnostic.message.contains("PowerOrder")
        }),
        "unexpected boundary-conformance diagnostics: {diagnostics:#?}"
    );

    TempTree::write(root.join("main.omg"), &source("", ""));
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("a private implementation may cite its package-private conformance");
}

#[test]
fn quotient_formation_retains_selected_evidence_as_private_package_custody() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let leaf = tree.package("leaf");
    TempTree::write(
        root.join("main.omg"),
        r#"use omega::language::core::relation;
use leaf::leaf;

pub data EquivalenceClass = Representative % equivalent
where equivalent satisfies
    Equivalence<Representative, equivalent>
    as Evidence;
"#,
    );
    let leaf_source = |visibility: &str| {
        format!(
            r#"use omega::language::core::relation;

pub data Representative {{
    case Zero;
    case Next(previous: Representative);
}}
pub proposition equivalent(a: Representative, b: Representative) = a == b;
machine equivalent_reflexive(a: Representative) ensures a == a {{}}
machine equivalent_symmetric(a: Representative, b: Representative)
requires a == b
ensures b == a
{{}}
machine equivalent_transitive(a: Representative, b: Representative, c: Representative)
requires
    a == b
    b == c
ensures a == c
{{}}
{visibility}Evidence: satisfies Equivalence<Representative, equivalent> {{
    Reflexive::reflexive = equivalent_reflexive;
    Symmetric::symmetric = equivalent_symmetric;
    Transitive::transitive = equivalent_transitive;
}}
"#,
        )
    };
    TempTree::write(leaf.join("leaf.omg"), &leaf_source(""));
    let inputs = || {
        PackageCompilationInputs::new_package(
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
        .expect("direct quotient dependency graph should validate")
    };

    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("quotient formation may not consume a dependency's private proof evidence");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("private conformance")
                && diagnostic.message.contains("Evidence")
        }),
        "unexpected quotient evidence diagnostics: {diagnostics:#?}"
    );

    TempTree::write(leaf.join("leaf.omg"), &leaf_source("pub "));
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .unwrap_or_else(|diagnostics| {
        panic!("public quotient proof evidence should be selectable: {diagnostics:#?}")
    });
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure as Exposure, AuthoredDeclarationSelectionKind as Kind,
        AuthoredDeclarationSelectionTarget as Target,
    };
    let root_selections = checked
        .authored_declaration_selections()
        .iter()
        .filter(|selection| {
            checked
                .symbols
                .source_file(selection.source_span())
                .is_some_and(|source| source.package_identity == Some(identity(1)))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        root_selections
            .iter()
            .filter(|selection| {
                selection.kind() == Kind::StaticPathSegment
                    && selection.exposure() == Exposure::PublicInterface
                    && matches!(selection.target(), Target::Resolved(target) if checked.symbols.display_path(target.selected_symbol(), "::").contains("equivalent"))
            })
            .count(),
        2,
        "the quotient relation and repeated equivalence subject need exact public path rows",
    );
    assert!(root_selections.iter().any(|selection| {
        selection.kind() == Kind::TypeReference
            && selection.exposure() == Exposure::PublicInterface
            && matches!(selection.target(), Target::Resolved(target) if checked.symbols.display_path(target.selected_symbol(), "::").contains("Equivalence"))
    }));
    assert!(root_selections.iter().any(|selection| {
        selection.kind() == Kind::Conformance
            && selection.exposure() == Exposure::PrivateImplementation
            && matches!(selection.target(), Target::Resolved(target) if checked.symbols.display_path(target.selected_symbol(), "::").contains("Evidence"))
    }));
}

#[test]
fn public_dynamic_return_may_carry_private_producer_selected_evidence() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let leaf = tree.package("leaf");
    TempTree::write(
        root.join("main.omg"),
        r#"use leaf::leaf;
machine inspect<'item>(item: &'item Item) -> i32 {
    let value: &'item dyn Shape = erased(item);
    value.code()
}
"#,
    );
    TempTree::write(
        leaf.join("leaf.omg"),
        r#"pub trait Shape {
    machine Self::code(&self) -> i32;
}
pub data Item { code: i32; }
Primary: Item satisfies Shape {
    machine code(&self) -> i32 { self.code }
}
pub machine erased<'item>(item: &'item Item) -> &'item dyn Shape {
    let value: &'item dyn Shape = item as &dyn Item::Primary;
    value
}
"#,
    );
    let inputs = || {
        PackageCompilationInputs::new_package(
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
        .expect("direct dynamic producer dependency graph should validate")
    };

    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .unwrap_or_else(|diagnostics| {
        panic!("a public bare-dynamic return may carry private producer evidence: {diagnostics:#?}")
    });

    TempTree::write(
        root.join("main.omg"),
        r#"use leaf::leaf;
machine inspect<'item>(item: &'item Item) -> i32 {
    let erased: &'item dyn Item::Primary = erased(item);
    erased.code()
}
"#,
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("the receiver may not name the producer's private conformance");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("private conformance")
                && diagnostic.message.contains("Primary")
        }),
        "unexpected private dynamic-evidence diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn public_conformance_header_cannot_hide_private_carrier_or_trait() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let inputs = || {
        PackageCompilationInputs::new_package(
            identity(1),
            vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
            Vec::new(),
        )
        .expect("root-only public-conformance graph should validate")
    };

    TempTree::write(
        root.join("main.omg"),
        "data Card {} pub trait Ranked {} pub PowerOrder: Card satisfies Ranked {}",
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("a public conformance cannot hide its private carrier");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("public interface selects private data `Card`")),
        "unexpected private-carrier diagnostics: {diagnostics:#?}"
    );

    TempTree::write(
        root.join("main.omg"),
        "pub data Card {} trait Ranked {} pub PowerOrder: Card satisfies Ranked {}",
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("a public conformance cannot hide its private trait");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("public interface selects private trait `Ranked`")),
        "unexpected private-trait diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn explicit_conformance_member_reference_obeys_package_visibility() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let leaf = tree.package("leaf");
    TempTree::write(
        root.join("main.omg"),
        r#"use leaf::leaf;
pub PowerOrder: Card satisfies Ranked {
    Ranked::before = Card::external_before;
}
"#,
    );
    let leaf_source = |visibility: &str| {
        format!(
            r#"pub data Card {{ rank: i32; }}
pub trait Ranked {{
    machine Self::before(&self, other: &Self) -> bool;
}}
{visibility}machine Card::external_before(&self, other: &Card) -> bool {{
    self.rank < other.rank
}}
"#,
        )
    };
    TempTree::write(leaf.join("leaf.omg"), &leaf_source(""));
    let inputs = || {
        PackageCompilationInputs::new_package(
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
        .expect("direct realization dependency graph should validate")
    };

    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("a conformance row cannot reference a dependency's private machine");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("private machine")
                && diagnostic.message.contains("external_before")
        }),
        "unexpected private-realization diagnostics: {diagnostics:#?}"
    );

    TempTree::write(leaf.join("leaf.omg"), &leaf_source("pub "));
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .unwrap_or_else(|diagnostics| {
        panic!("public referenced realization should validate: {diagnostics:#?}")
    });
}

#[test]
fn carrier_qualified_declarations_own_visibility_independently() {
    let tree = TempTree::new();
    let root = tree.package("root");

    TempTree::write(
        root.join("main.omg"),
        "data Token [copy] { value: u64; }\npub domain Token::Trusted;\n",
    );
    let inputs = || {
        PackageCompilationInputs::new_package(
            identity(1),
            vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
            Vec::new(),
        )
        .expect("root-only package graph should validate")
    };
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("a public domain may not hide a private carrier");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("public interface selects private data `Token`")
        }),
        "unexpected public-domain diagnostics: {diagnostics:#?}"
    );

    TempTree::write(
        root.join("main.omg"),
        "data Token [copy] { value: u64; }\npub operator < Token::less(left: Token, right: Token) -> bool;\n",
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("a public operator may not hide a private carrier");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("public interface selects private data `Token`")
        }),
        "unexpected public-operator diagnostics: {diagnostics:#?}"
    );
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
        PackageCompilationInputs::new_package(
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

    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("a direct dependency does not publish its private proposition");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("private proposition")
                && diagnostic.message.contains("leaf_ready")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    TempTree::write(leaf.join("leaf.omg"), "pub proposition leaf_ready();\n");
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("an explicitly public proposition should be nameable by a direct dependent");

    TempTree::write(
        root.join("main.omg"),
        "proposition local_ready();\npub machine inspect()\nrequires local_ready()\n{ }\n",
    );
    let root_only = PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .expect("root-only package graph should validate");
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(root_only),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
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
    let root_only = PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .expect("root-only private implementation graph should validate");
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(root_only),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("private implementation may select its package-private proposition");

    TempTree::write(
        root.join("main.omg"),
        "proposition local_ready();\npub proposition exposed() = local_ready();\n",
    );
    let root_only = PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .expect("root-only transparent proposition graph should validate");
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(root_only),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
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
        PackageCompilationInputs::new_package(
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

    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("a direct dependency does not publish its private const");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("private const")
                && diagnostic.message.contains("LEAF_LIMIT")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    TempTree::write(leaf.join("leaf.omg"), "pub const LEAF_LIMIT: u64 = 4;\n");
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("an explicitly public const should be nameable by a direct dependent");

    TempTree::write(
        root.join("main.omg"),
        "const LOCAL_LIMIT: u64 = 4;\npub proposition within_limit() = LOCAL_LIMIT == 4;\n",
    );
    let root_only = PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .expect("root-only package graph should validate");
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(root_only),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
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
    let root_only = PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .expect("root-only private implementation graph should validate");
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(root_only),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("private implementation may select its package-private const");

    TempTree::write(
        root.join("main.omg"),
        "data LocalToken [copy] { value: u64; }\npub const TOKEN: LocalToken = LocalToken { value: 4 };\n",
    );
    let root_only = PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .expect("root-only public const graph should validate structurally");
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(root_only),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
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
        "pub data LocalToken [copy] { value: u64; }\npub const TOKEN: LocalToken = LocalToken { value: 4 };\n",
    );
    let root_only = PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .expect("root-only published public const graph should validate");
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(root_only),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
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
        PackageCompilationInputs::new_package(
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

    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
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
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("an explicitly public operator should be nameable by a direct dependent");

    TempTree::write(
        root.join("main.omg"),
        "pub data Token [copy] { value: u64; }\noperator < Token::less(left: Token, right: Token) -> bool;\npub machine inspect(value: Token)\nrequires value < value\n{ }\n",
    );
    let root_only = PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .expect("root-only public operator interface should validate structurally");
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(root_only),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
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
    let root_only = PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .expect("root-only private operator implementation should validate");
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(root_only),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("private implementation may select its package-private operator");
}

#[test]
fn leaf_computed_constants_select_their_exact_declaration_owner() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let leaf = tree.package("leaf");

    TempTree::write(
        root.join("main.omg"),
        "use leaf::leaf;\nmachine limits() -> u64 { LIMIT + HALF }\n",
    );
    // Computed leaf declarations: an arithmetic initializer and a constant
    // that itself selects another constant in the same leaf package.
    TempTree::write(
        leaf.join("leaf.omg"),
        "pub const LIMIT: u64 = 6 * 7;\npub const HALF: u64 = LIMIT / 2;\n",
    );

    let inputs = PackageCompilationInputs::new_package(
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
    .expect("direct computed-const dependency graph should validate");

    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("a computed leaf constant should be nameable by a direct dependent");

    let selected = checked
        .authored_declaration_selections()
        .iter()
        .filter_map(|selection| {
            let language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(target) = selection.target() else {
                return None;
            };
            let symbol = target.selected_symbol();
            let path = checked.symbols.display_path(symbol, "::");
            matches!(path.as_str(), "LIMIT" | "HALF")
                .then_some((path, checked.symbols.symbol_package_identity(symbol)))
        })
        .collect::<Vec<_>>();
    assert!(
        selected
            .iter()
            .all(|(_, owner)| *owner == Some(identity(2))),
        "every computed-constant selection must name the exact leaf package: {selected:?}"
    );
    assert_eq!(
        selected.iter().filter(|(path, _)| path == "HALF").count(),
        1,
        "the consumer selects HALF once: {selected:?}"
    );
    // LIMIT is selected by the consumer and inside the leaf's own
    // `HALF = LIMIT / 2` initializer — both keep the leaf owner.
    assert!(
        selected.iter().filter(|(path, _)| path == "LIMIT").count() >= 2,
        "LIMIT selection should cover consumer use and the leaf initializer: {selected:?}"
    );
}
