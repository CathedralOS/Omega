use super::{TempTree, compile_provider_mode_fixture, host_target_name, identity};
use crate::fixtures;
use build_declarations::DependencyPurpose;
use compiler::{
    CheckedCompileRequest, CompileOptions, CompileRequest, RequestedCompileProduct, compile,
    compile_to_checked,
};
use package_compilation::{
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use std::path::Path;

#[test]
fn public_machine_keeps_its_declared_ranking_measure_private() {
    let tree = TempTree::new();
    let root = tree.package("root");
    TempTree::write(
        root.join("main.omg"),
        r#"data Card { }
measure Card::PowerOrder(power: u64) -> u64 { power }

pub machine weaken(power: u64)
terminates by power -> Card::PowerOrder;
-> u64
{
    transition power > 0 {
        true -> weaken(power - 1)
        false -> power
    }
}
"#,
    );
    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .expect("root-only ranking fixture should validate structurally");
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("a public termination guarantee may use a same-package private ranking measure");
}

#[test]
fn public_callable_bound_requires_a_public_conformance_and_its_direct_owner() {
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
        package_inputs: Some(transitive_only.clone()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("a public callable may not publish private conformance evidence");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("private conformance")
                && diagnostic.message.contains("Primary")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    let directly_admitted = PackageCompilationInputs::new_package(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "middle", middle),
            PackageSourceBinding::new(identity(3), "leaf", leaf.clone()),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(1), "leaf", identity(3)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("direct leaf admission should validate");

    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(directly_admitted.clone()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("a direct dependency does not publish its private conformance");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("private conformance")
                && diagnostic.message.contains("Primary")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    TempTree::write(
        leaf.join("leaf.omg"),
        r#"pub trait Marker { }
pub data Good { }
pub Primary: Good satisfies Marker;
"#,
    );

    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(transitive_only),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("public conformance evidence still requires its direct owner");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("`root`")
                && diagnostic.message.contains("`leaf`")
                && diagnostic.message.contains("Primary")
                && diagnostic.message.contains("direct dependency")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(directly_admitted),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("a direct dependency may select the dependency's public conformance");
    assert!(checked.authored_declaration_selections().iter().any(|selection| {
        selection.kind()
            == language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Conformance
            && selection.exposure()
                == language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface
            && matches!(
                selection.target(),
                language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(target)
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
        package_inputs: Some(transitive_only.clone()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("root may not infer a private leaf conformance");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("private conformance")
                && diagnostic.message.contains("`GoodMarker`")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    let directly_admitted = PackageCompilationInputs::new_package(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "middle", middle),
            PackageSourceBinding::new(identity(3), "leaf", leaf.clone()),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(1), "leaf", identity(3)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("direct leaf admission should validate");

    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(directly_admitted.clone()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("direct dependency authority does not publish a private conformance");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("private conformance")
                && diagnostic.message.contains("`GoodMarker`")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    TempTree::write(
        leaf.join("leaf.omg"),
        r#"pub trait Marker { }
pub data Good { }
pub GoodMarker: Good satisfies Marker;
"#,
    );

    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(transitive_only),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("public inferred evidence still requires a direct dependency");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("`root`")
                && diagnostic.message.contains("`leaf`")
                && diagnostic.message.contains("`GoodMarker`")
                && diagnostic.message.contains("direct dependency")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(directly_admitted),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("direct dependency should admit the public inferred leaf conformance");
}

#[test]
fn same_package_private_conformance_is_implementation_only() {
    let tree = TempTree::new();
    let root = tree.package("root");
    TempTree::write(
        root.join("main.omg"),
        r#"pub trait Marker { }
pub data Good { }
Primary: Good satisfies Marker;

machine internal_accept<Element>(value: &Element)
where Element satisfies Good::Primary
{
}
"#,
    );
    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .expect("root-only conformance fixture should validate structurally");

    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("same-package private implementation may select its private conformance");
}

#[test]
fn public_interface_rejects_same_package_private_conformance() {
    let tree = TempTree::new();
    let root = tree.package("root");
    TempTree::write(
        root.join("main.omg"),
        r#"pub trait Marker { }
pub data Good { }
Primary: Good satisfies Marker;

pub machine exported_accept<Element>(value: &Element)
where Element satisfies Good::Primary
{
}
"#,
    );
    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .expect("root-only public-interface fixture should validate structurally");

    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("a public interface may not name a same-package private conformance");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("public interface selects private conformance")
                && diagnostic.message.contains("Primary")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );
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

    let inputs = PackageCompilationInputs::new_package(
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
        package_inputs: Some(inputs.clone()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("early const-generic execution may not select a transitive-only package");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("const-generic application evaluation")
                && diagnostic.message.contains("authored Call selection")
                && diagnostic.message.contains("direct dependency authority")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );

    TempTree::write(
        root.join("main.omg"),
        "use middle::middle;\ndata FixedBuffer<const N: u64> { items: [u8; N]; }\ndata Main { buffer: FixedBuffer<middle_size()>; }\n",
    );
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
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

    let inputs = PackageCompilationInputs::new_package(
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

    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("dependency code may not select root declarations without dependency authority");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("const-generic application evaluation")
                && diagnostic
                    .message
                    .contains("private constant declaration cannot be selected")
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

    let inputs = PackageCompilationInputs::new_package(
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
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("early fixed-array execution may not select a transitive-only package");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("fixed-array length")
                && diagnostic.message.contains("build-time invocation")
                && diagnostic.message.contains("direct dependency authority")
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
    .expect("direct fixed-array callee admission should validate structurally");
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(directly_admitted),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
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

    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
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
    let mut owned_entries: [FieldEntry; 64];
    owned_entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 }
    };
    Plan {
        entries: owned_entries,
        entry_count: 1,
        size_fixed: 4,
        size_is_dynamic: false,
        align: 4
    }
}
"#,
    );

    let inputs = PackageCompilationInputs::new_package(
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
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("early plan-laid execution may not select a transitive-only package");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("plan-laid value type")
                && diagnostic.message.contains("build-time invocation")
                && diagnostic.message.contains("direct dependency authority")
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
    .expect("direct policy admission should validate structurally");
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(directly_admitted),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
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
        "use middle::middle;\npub data Payload { value: u32; }\nmachine inspect(view: &Placed<LeafPlacement, Payload>) {}\n",
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
    let mut owned_entries: [FieldEntry; 64];
    owned_entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 }
    };
    let access: AccessPlan = AccessPlan::inaccessible(&schema);
    PlacementPlan {
        layout: Plan {
            entries: owned_entries,
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

    let inputs = PackageCompilationInputs::new_package(
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
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("early placed-view execution may not select a transitive-only package");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("placement policy")
                && diagnostic.message.contains("direct dependency authority")
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
    .expect("direct placement-policy admission should validate structurally");
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(directly_admitted),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
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
        r#"
machine build(builder: &mut Build) {
    builder.package("root");
    let marker: BuildPath = builder.output.resolve("build-ran.marker");
    let descriptor: i32 = builder.output.create(marker, 438);
    let closed: i32 = builder.output.close(descriptor);
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

    let checked_build = tree.0.join("checked-build");
    let checked_diagnostics = compile_to_checked(CheckedCompileRequest {
        build_dir: Some(checked_build.to_owned()),
        package_inputs: Some(inputs.clone()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
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
        let native_diagnostics = compile(
            CompileRequest::new(CompileOptions {
                root_path: root.join("main.omg"),
                build_dir: Some(native_build.clone()),
                target_name: Some(target_name.to_owned()),
            })
            .with_package_inputs(inputs),
        )
        .and_then(compiler::CompileOutcomes::into_single_report)
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
    builder.package("root");
    builder.select_provider<dep::Pair, dep::Provider>(CompositionMode::Fused);
}
"#,
    );
    TempTree::write(
        dependency.join("provider.omg"),
        r#"pub boundary trait Pair { machine first(); }
pub data Provider { first: addr; }
machine Provider::first() satisfies Pair::first via ForeignBinding::VtableField(first);
"#,
    );

    let inputs = PackageCompilationInputs::new_package(
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

    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("dependency provider should check");
    assert!(checked.authored_declaration_selections().iter().any(|selection| {
        selection.target()
            == language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Intrinsic(
                language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic::BuildProviderSelection,
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
fn independent_provider_selection_reaches_the_componentization_fence() {
    let diagnostics = compile_provider_mode_fixture(
        61,
        "independent-provider",
        "",
        "    builder.select_provider<Pair, Provider>(CompositionMode::Independent);",
    )
    .expect_err("independent mode must never fall through as fused");

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("retains independent composition")
                && diagnostic
                    .message
                    .contains("refusing to treat the edge as fused")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn provider_selection_rejects_an_authored_composition_mode_lookalike() {
    let diagnostics = compile_provider_mode_fixture(
        62,
        "lookalike-composition-mode",
        "data LocalCompositionMode [copy] { case Independent; }",
        "    builder.select_provider<Pair, Provider>(LocalCompositionMode::Independent);",
    )
    .expect_err("an authored lookalike cannot select composition mode");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("requires an exact payload-free compiler-owned CompositionMode case")),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn provider_selection_rejects_an_arbitrary_composition_expression() {
    let diagnostics = compile_provider_mode_fixture(
        63,
        "arbitrary-composition-expression",
        "",
        "    builder.select_provider<Pair, Provider>(true);",
    )
    .expect_err("an arbitrary expression cannot select composition mode");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("provider selection requires a compiler-owned CompositionMode case")),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn provider_selection_rejects_conflicting_composition_modes() {
    let diagnostics = compile_provider_mode_fixture(
        64,
        "conflicting-composition-modes",
        "",
        r#"    builder.select_provider<Pair, Provider>(CompositionMode::Fused);
    builder.select_provider<Pair, Provider>(CompositionMode::Independent);"#,
    )
    .expect_err("one slot cannot have two composition modes");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("with conflicting composition modes Fused and Independent")),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn target_provider_default_cannot_request_independent_composition() {
    let tree = TempTree::new();
    let root = tree.package("independent-target-default");
    let package = identity(65);
    TempTree::write(
        root.join("main.omg"),
        r#"pub boundary trait Pair { machine first(); }
data Provider { first: addr; }
machine Provider::first() satisfies Pair::first via ForeignBinding::VtableField(first);

data TargetProviders { }
linux_x86_64 machine TargetProviders::provider_defaults(defaults: &mut TargetProviders) {
    defaults.select_provider<Pair, Provider>(CompositionMode::Independent);
}

data Main { }
machine Main::main(&mut self) { }
"#,
    );
    TempTree::write(
        root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("independent_target_default");
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
}
"#,
    );
    let inputs = PackageCompilationInputs::new_package(
        package,
        vec![PackageSourceBinding::new(package, "root", root.clone())],
        Vec::new(),
    )
    .expect("one-package target-default graph");

    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), Some("linux_x86_64"))
    })
    .expect_err("target defaults cannot authorize a deployment cut");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("only the owner-controlled build may create that deployment cut")),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn dependency_operator_family_selection_covers_every_overload_atomically() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let dependency = tree.package("dependency");

    TempTree::write(root.join("main.omg"), "use dep::provider;\n");
    TempTree::write(
        root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.package("root");
    builder.select_provider<dep::Convert::apply, dep::ConvertProvider>();
}
"#,
    );
    TempTree::write(
        dependency.join("provider.omg"),
        r#"pub data Convert { }
pub boundary operator Convert::apply(value: i32) -> i32;
pub boundary operator Convert::apply(value: u32) -> u32;

pub data ConvertProvider { }
machine ConvertProvider::apply_i32(value: i32) -> i32
satisfies Convert::apply
{
    transition { _ -> (value) }
}
machine ConvertProvider::apply_u32(value: u32) -> u32
satisfies Convert::apply
{
    transition { _ -> (value) }
}
"#,
    );

    let inputs = PackageCompilationInputs::new_package(
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
    .expect("reconciled operator-provider graph should validate");

    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("one family selection should select both exact overload plans");
    let plans = checked.selected_provider_plans().plans();
    assert_eq!(plans.len(), 2);
    assert!(
        plans
            .iter()
            .all(|plan| plan.provider_type == "ConvertProvider")
    );
    let mut coordinates = plans
        .iter()
        .map(|plan| plan.schema.trait_name.as_str())
        .collect::<Vec<_>>();
    coordinates.sort_unstable();
    assert_eq!(
        coordinates,
        vec![
            "operator::Convert::apply(named(name(i32)))->named(name(i32))",
            "operator::Convert::apply(named(name(u32)))->named(name(u32))",
        ]
    );
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

    let inputs = PackageCompilationInputs::new_package(
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

    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
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

    let inputs = PackageCompilationInputs::new_package(
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

    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
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

    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .expect("root package input should validate");

    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
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
        r#"use omega::language::core::service;
use dep::values;
pub boundary trait Console { machine exit_process(return_code: i32); }
data ConsoleProvider { }
machine ConsoleProvider::exit_process(return_code: i32) satisfies Console::exit_process { }
data Main { console: Binding<Console>; }
machine Main::main(&mut self) reaches Console {
    transition ANSWER == 42 { true -> yes() _ -> no() }
    state yes(&mut self) { self.console.exit_process(0); }
    state no(&mut self) { self.console.exit_process(1); }
}
"#,
    );
    TempTree::write(
        root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("native_package_entrypoint");
    builder.select_provider<Console, ConsoleProvider>();
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

    let inputs = PackageCompilationInputs::new_package(
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

    let report = compile(
        CompileRequest::new(CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: Some(tree.0.join("build-output")),
            target_name: Some(target_name.to_owned()),
        })
        .with_package_inputs(inputs),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("native package compilation should use reconciled imports only");
    assert!(report.production_manifest().is_none());
}

#[test]
fn native_package_product_retains_one_canonical_production_manifest() {
    let Some(target_name) = host_target_name() else {
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("repository root")
        .join("tests/omega/pass")
        .join(fixtures::NO_SELECTION_EMPTY_ENTRY);
    let package = identity(44);
    let output = TempTree::new();
    let inputs = PackageCompilationInputs::new_package(
        package,
        vec![PackageSourceBinding::new(
            package,
            "manifest_native_fixture",
            root.clone(),
        )],
        Vec::new(),
    )
    .expect("single-package native graph");
    let report = compile(
        CompileRequest::new(CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: Some(output.0.clone()),
            target_name: Some(target_name.to_owned()),
        })
        .with_package_inputs(inputs)
        .with_requested_product(RequestedCompileProduct::NativeArtifact),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("package native fixture should compile");
    let manifest = report
        .production_manifest()
        .expect("native package production must retain one canonical manifest");
    assert!(manifest.validate());
    assert_eq!(manifest.subject().package().root(), package);
    assert!(matches!(
        manifest.artifact(),
        compiler::ProductionArtifactIdentity::Native(_)
    ));
    assert!(
        report
            .require_package_native_physical_evidence()
            .expect("an empty native entry has a complete empty physical projection")
            .children()
            .is_empty()
    );

    let standalone = compile(
        CompileRequest::new(CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: Some(output.0.join("standalone")),
            target_name: Some(target_name.to_owned()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("standalone native fixture should compile without package evidence");
    assert_eq!(
        standalone
            .require_package_native_physical_evidence()
            .expect_err("standalone production cannot issue package final evidence"),
        compiler::FinalRealizationEvidenceError::PackageProductionManifestRequired,
    );
}

#[test]
fn build_entry_imports_resolve_through_build_scope_aliases() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let host = tree.package("host-tool");

    TempTree::write(root.join("main.omg"), "const RESULT: u32 = 42;\n");
    TempTree::write(
        root.join("build.omg"),
        r#"use dep::values;
const KEPT: u32 = VALUES;
machine build(builder: &mut Build) {
    builder.package("root");
}
"#,
    );
    TempTree::write(host.join("values.omg"), "pub const VALUES: u32 = 7;\n");

    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "host_tool", host),
        ],
        vec![PackageDependencyBinding::for_purpose(
            identity(1),
            "dep",
            identity(2),
            DependencyPurpose::Build,
        )],
    )
    .expect("a root build edge keeps its target reachable");

    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("the selected build entry resolves build-scope aliases");
}

#[test]
fn product_imports_cannot_name_build_scope_aliases() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let host = tree.package("host-tool");

    TempTree::write(root.join("main.omg"), "use dep::values;\n");
    TempTree::write(
        root.join("build.omg"),
        "machine build(builder: &mut Build) {\n    builder.package(\"root\");\n}\n",
    );
    TempTree::write(host.join("values.omg"), "pub const VALUES: u32 = 7;\n");

    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "host_tool", host),
        ],
        vec![PackageDependencyBinding::for_purpose(
            identity(1),
            "dep",
            identity(2),
            DependencyPurpose::Build,
        )],
    )
    .expect("root build-edge graph should validate");

    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("a product import must not select a build-scope alias");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("names build dependency `dep`")),
        "unexpected diagnostics: {diagnostics:#?}"
    );
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("product import may only select product dependencies")),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn build_imports_cannot_name_product_scope_aliases() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let dependency = tree.package("dependency");

    TempTree::write(root.join("main.omg"), "const RESULT: u32 = 42;\n");
    TempTree::write(
        root.join("build.omg"),
        r#"use dep::values;
machine build(builder: &mut Build) {
    builder.package("root");
}
"#,
    );
    TempTree::write(
        dependency.join("values.omg"),
        "pub const VALUES: u32 = 7;\n",
    );

    let inputs = PackageCompilationInputs::new_package(
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
    .expect("product-edge graph should validate");

    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("a build import must not select a product-scope alias");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("names product dependency `dep`")),
        "unexpected diagnostics: {diagnostics:#?}"
    );
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("build import may only select build dependencies")),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn one_alias_answers_product_and_build_scopes_independently() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let product = tree.package("product-dep");
    let host = tree.package("host-tool");

    TempTree::write(
        root.join("main.omg"),
        "use shared::pv;\nconst RESULT: u32 = PV;\n",
    );
    TempTree::write(
        root.join("build.omg"),
        r#"use shared::hv;
const KEPT: u32 = HV;
machine build(builder: &mut Build) {
    builder.package("root");
}
"#,
    );
    TempTree::write(product.join("pv.omg"), "pub const PV: u32 = 1;\n");
    TempTree::write(host.join("hv.omg"), "pub const HV: u32 = 7;\n");

    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "product_dep", product),
            PackageSourceBinding::new(identity(3), "host_tool", host),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "shared", identity(2)),
            PackageDependencyBinding::for_purpose(
                identity(1),
                "shared",
                identity(3),
                DependencyPurpose::Build,
            ),
        ],
    )
    .expect("one alias may bind different packages in independent scopes");

    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("each scope resolves its own `shared` target");
}

#[test]
fn one_root_source_checks_as_separate_per_scope_instances() {
    let tree = TempTree::new();
    let root = tree.package("root");

    TempTree::write(
        root.join("main.omg"),
        "use common;\nconst RESULT: u32 = SHARED;\n",
    );
    TempTree::write(
        root.join("build.omg"),
        r#"use common;
const KEPT: u32 = SHARED;
machine build(builder: &mut Build) {
    builder.package("root");
}
"#,
    );
    TempTree::write(root.join("common.omg"), "pub const SHARED: u32 = 5;\n");

    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .expect("root-only package graph should validate");

    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("one physical root source checks once per dependency scope");
}

#[test]
fn build_scope_extends_through_root_local_helper_imports() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let host = tree.package("host-tool");

    TempTree::write(root.join("main.omg"), "const RESULT: u32 = 42;\n");
    TempTree::write(
        root.join("build.omg"),
        r#"use plan;
const KEPT: u32 = STEPS;
machine build(builder: &mut Build) {
    builder.package("root");
}
"#,
    );
    TempTree::write(
        root.join("plan.omg"),
        "use dep::values;\npub const STEPS: u32 = VALUES;\n",
    );
    TempTree::write(host.join("values.omg"), "pub const VALUES: u32 = 7;\n");

    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "host_tool", host),
        ],
        vec![PackageDependencyBinding::for_purpose(
            identity(1),
            "dep",
            identity(2),
            DependencyPurpose::Build,
        )],
    )
    .expect("root build-edge graph should validate");

    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("a root-local helper imported by the build entry inherits build scope");
}

#[test]
fn build_scope_imports_reject_missing_dependency_edges() {
    let tree = TempTree::new();
    let root = tree.package("root");

    TempTree::write(root.join("main.omg"), "const RESULT: u32 = 42;\n");
    TempTree::write(
        root.join("build.omg"),
        r#"use ghost::values;
machine build(builder: &mut Build) {
    builder.package("root");
}
"#,
    );

    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .expect("root-only package graph should validate");

    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("an undeclared build-scope import must not resolve");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("ghost")),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn non_root_packages_cannot_hold_build_dependencies() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");
    TempTree::write(root.join("main.omg"), "use dep::values;\n");
    TempTree::write(middle.join("values.omg"), "pub const VALUES: u32 = 7;\n");
    TempTree::write(leaf.join("values.omg"), "pub const VALUES: u32 = 9;\n");

    let errors = PackageCompilationInputs::new_package(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root),
            PackageSourceBinding::new(identity(2), "middle", middle),
            PackageSourceBinding::new(identity(3), "leaf", leaf),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "dep", identity(2)),
            PackageDependencyBinding::for_purpose(
                identity(2),
                "host",
                identity(3),
                DependencyPurpose::Build,
            ),
        ],
    )
    .expect_err("a dependency package must not carry build-scope edges");
    assert!(
        errors.iter().any(|error| matches!(
            error,
            package_compilation::PackageCompilationInputError::NonRootBuildDependency { .. }
        )),
        "unexpected errors: {errors:#?}"
    );
}
