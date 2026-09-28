use super::SymbolHandle;
use crate::tests::front_end::{typed_program, typed_program_from_source_map};
use source::{
    DependencyScope, SourceFile, SourceId, SourceMap, SourceOrigin, SourceResolutionStratum,
};
use std::sync::Arc;
const COPY_OPAQUE: &str = r#"
boundary data Token [copy];
data Main {}
machine Main::main(&mut self) {}
"#;

fn data_symbol(program: &typed_trees::TypedTrees, name: &str) -> SymbolHandle {
    program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == name)
        .unwrap_or_else(|| panic!("missing data `{name}`"))
        .symbol
}

fn rendered(diagnostics: Vec<diagnostics::Diagnostic>) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn scoped_program(change_build_source: impl FnOnce(&mut SourceFile)) -> typed_trees::TypedTrees {
    let product = SourceFile {
        source_id: SourceId(0),
        path: "tests/token.omg".into(),
        package_root: "tests".into(),
        package_identity: None,
        dependency_scope: DependencyScope::Product,
        origin: SourceOrigin::User,
        resolution_stratum: SourceResolutionStratum::Base,
        source: Arc::from("boundary data Token [copy];"),
    };
    let mut build = SourceFile {
        source_id: SourceId(1),
        dependency_scope: DependencyScope::Build,
        ..product.clone()
    };
    change_build_source(&mut build);
    typed_program_from_source_map(
        SourceMap::from_files(vec![product.clone(), build.clone()]),
        &[
            (product.source_id, &product.source),
            (build.source_id, &build.source),
        ],
    )
}

#[test]
fn one_receipt_covers_exact_and_dependency_scope_instances() {
    let anonymous = typed_program(COPY_OPAQUE);
    let receipt = validation::OpaqueDataPropertyReceipt::copy(data_symbol(&anonymous, "Token"));
    crate::lower_typed_trees(
        anonymous,
        &crate::CheckingRequest::settled().with_opaque_property_receipts(&[receipt]),
    )
    .expect("a source-free exact receipt needs no inferred instances");

    let program = scoped_program(|_| {});
    let receipts = program
        .data_definitions()
        .iter()
        .map(|definition| validation::OpaqueDataPropertyReceipt::copy(definition.symbol))
        .collect::<Vec<_>>();
    assert_eq!(receipts.len(), 2);
    assert_ne!(receipts[0].declaration(), receipts[1].declaration());
    // Either scope may supply the anchor. Callers already carrying both exact
    // instances must not acquire duplicate inferred rows.
    for supplied in [&receipts[..1], &receipts[1..], &receipts[..]] {
        let checked = crate::lower_typed_trees(
            program.clone(),
            &crate::CheckingRequest::settled().with_opaque_property_receipts(supplied),
        )
        .expect("one authored opaque declaration admits both dependency instances");
        assert_eq!(checked.typed.data_definitions().len(), 2);
    }
}

#[test]
fn missing_and_stale_receipts_do_not_gain_source_instance_authority() {
    let program = scoped_program(|_| {});
    let stale = [validation::OpaqueDataPropertyReceipt::copy(
        SymbolHandle::default(),
    )];
    for receipts in [&[][..], &stale[..]] {
        let diagnostics = crate::lower_typed_trees(
            program.clone(),
            &crate::CheckingRequest::settled().with_opaque_property_receipts(receipts),
        )
        .expect_err("source matching cannot supply missing representation authority");
        let message = rendered(diagnostics);
        assert!(message.contains("without an admitted property receipt"));
        if !receipts.is_empty() {
            assert!(message.contains("receipt maps to 0 declarations"));
        }
    }
}

#[test]
fn source_instance_receipts_do_not_cross_provenance_changes() {
    let changes: &[(&str, fn(&mut SourceFile))] = &[
        ("path", |file| file.path = "tests/other.omg".into()),
        ("package", |file| {
            file.package_identity = semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32]);
        }),
        ("unidentified package root", |file| {
            file.package_root = "other".into()
        }),
        ("origin", |file| file.origin = SourceOrigin::Toolchain),
        ("activation stratum", |file| {
            file.resolution_stratum = SourceResolutionStratum::CurrentActivationExtension
        }),
        ("bytes outside the declaration", |file| {
            file.source = Arc::from("boundary data Token [copy]; // changed")
        }),
        ("declaration span", |file| {
            file.source = Arc::from(" boundary data Token [copy];")
        }),
    ];
    for (name, change) in changes {
        let program = scoped_program(change);
        let receipt = validation::OpaqueDataPropertyReceipt::copy(data_symbol(&program, "Token"));
        let diagnostics = crate::lower_typed_trees(
            program,
            &crate::CheckingRequest::settled().with_opaque_property_receipts(&[receipt]),
        )
        .expect_err(name);
        assert!(
            rendered(diagnostics).contains("without an admitted property receipt"),
            "{name}"
        );
    }
}

#[test]
fn duplicate_and_wrong_declaration_receipts_reject() {
    let duplicate = scoped_program(|_| {});
    let receipt = validation::OpaqueDataPropertyReceipt::copy(data_symbol(&duplicate, "Token"));
    let diagnostics = crate::lower_typed_trees(
        duplicate,
        &crate::CheckingRequest::settled().with_opaque_property_receipts(&[receipt, receipt]),
    )
    .expect_err("duplicate opaque property receipts must reject");
    assert!(rendered(diagnostics).contains("repeat one exact declaration"));

    let wrong = typed_program(COPY_OPAQUE);
    let receipt = validation::OpaqueDataPropertyReceipt::copy(data_symbol(&wrong, "Main"));
    let diagnostics = crate::lower_typed_trees(
        wrong,
        &crate::CheckingRequest::settled().with_opaque_property_receipts(&[receipt]),
    )
    .expect_err("a transparent declaration cannot receive an opaque property receipt");
    assert!(rendered(diagnostics).contains("targets non-opaque declaration"));
}

#[test]
fn copy_receipt_rejects_an_opaque_that_does_not_claim_copy() {
    let program = typed_program(
        r#"
boundary data Token;
data Main {}
machine Main::main(&mut self) {}
"#,
    );
    let receipt = validation::OpaqueDataPropertyReceipt::copy(data_symbol(&program, "Token"));
    let diagnostics = crate::lower_typed_trees(
        program,
        &crate::CheckingRequest::settled().with_opaque_property_receipts(&[receipt]),
    )
    .expect_err("a copy receipt must match the declaration's exact property claim");
    assert!(rendered(diagnostics).contains("does not claim `[copy]`"));
}
