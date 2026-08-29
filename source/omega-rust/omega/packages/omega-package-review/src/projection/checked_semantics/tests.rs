use super::super::providers::selection::validate_selected_provider_declaration_owner;
use super::declarations::{nominal_owner_from_symbols, toolchain_source_identity};
use super::types::{
    validate_package_type_identity_input, validate_package_type_identity_input_inner,
};
use crate::evidence::{
    PackageReviewNominalIdentity, PackageReviewNominalOwner, PackageReviewToolchainSourceIdentity,
};
use psi_core::PackageKeyIdentity;
use psi_source::{SourceFile, SourceId, SourceMap, SourceOrigin, SourceSpan, Span};
use psi_symbols::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTable, SymbolTableBuilder};
use std::path::PathBuf;
use std::sync::Arc;

#[test]
fn selected_provider_declaration_ownership_is_exact_and_fail_closed() {
    let package = PackageKeyIdentity::from_digest([1; 32]).expect("package identity");
    let other_package = PackageKeyIdentity::from_digest([2; 32]).expect("other package identity");
    let package_declaration = PackageReviewNominalIdentity {
        owner: PackageReviewNominalOwner::Package(package),
        path: "service::requirement".to_owned(),
    };
    let toolchain_declaration = PackageReviewNominalIdentity {
        owner: PackageReviewNominalOwner::ToolchainSource(PackageReviewToolchainSourceIdentity {
            digest: [3; 32],
        }),
        path: "service::requirement".to_owned(),
    };
    let unresolved_declaration = PackageReviewNominalIdentity {
        owner: PackageReviewNominalOwner::Unresolved,
        path: "service::requirement".to_owned(),
    };

    validate_selected_provider_declaration_owner(
        &package_declaration,
        Some(package),
        "plan",
        "row requirement",
    )
    .expect("an exact package owner must pass");
    validate_selected_provider_declaration_owner(
        &toolchain_declaration,
        None,
        "plan",
        "row requirement",
    )
    .expect("an exact authored toolchain source must pass");

    for (declaration, expected_package) in [
        (&package_declaration, Some(other_package)),
        (&package_declaration, None),
        (&toolchain_declaration, Some(package)),
        (&unresolved_declaration, Some(package)),
        (&unresolved_declaration, None),
    ] {
        let error = validate_selected_provider_declaration_owner(
            declaration,
            expected_package,
            "plan",
            "row requirement",
        )
        .expect_err("mismatched or unresolved ownership must reject");
        assert!(
            error[0]
                .message
                .contains("exact package/toolchain ownership")
        );
    }
}

#[test]
fn package_type_identity_rejects_textual_and_unselected_fallbacks() {
    use psi_typed_trees::expression::{BinaryOperator, ExpressionNode, TableBinaryExpression};
    use psi_typed_trees::name::Identifier;
    use psi_typed_trees::types::{
        DomainConstraint, DomainConstraintSubject, FixedArrayLength, TypeConstraintNode,
        TypeReferenceNode,
    };

    let mut program = psi_typed_trees::TypedTrees::default();
    let element_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let residual = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::ConstCall {
                name: Identifier::generated("length"),
                source_span: SourceSpan::default(),
            },
        });
    let error = validate_package_type_identity_input(&program, residual, &[])
        .expect_err("residual const call must reject package evidence");
    assert!(error[0].message.contains("unevaluated const call"));

    let textual = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("source_spelling"),
        });
    let error = validate_package_type_identity_input(&program, textual, &[])
        .expect_err("unresolved source spelling must reject package evidence");
    assert!(error[0].message.contains("without exact semantic identity"));

    let misplaced_const = psi_language_semantics::const_value::CanonicalConstValue::new(
        "u32",
        "integer3:u321:7",
        "7",
    );
    let misplaced_const = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated(misplaced_const.atom()),
        });
    let error = validate_package_type_identity_input(&program, misplaced_const, &[])
        .expect_err("canonical const outside a declared const slot must reject");
    assert!(error[0].message.contains("without exact semantic identity"));

    let unresolved_binder = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::ConstParameter {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated("N"),
            },
        });
    let error = validate_package_type_identity_input(&program, unresolved_binder, &[])
        .expect_err("unreconciled const binder must reject package evidence");
    assert!(error[0].message.contains("exact telescope identity"));

    let left = program.expression_table.insert(ExpressionNode::Integer(
        psi_numerics::literals::IntegerLiteral::zero(),
    ));
    let right = program.expression_table.insert(ExpressionNode::Integer(
        psi_numerics::literals::IntegerLiteral::zero(),
    ));
    let binary = program
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left,
            operator: BinaryOperator::Add,
            right,
        }));
    let open_index = program
        .type_reference_table
        .insert(TypeReferenceNode::ConstExpression(binary));
    let error = validate_package_type_identity_input_inner(&program, open_index, &[], true)
        .expect_err("unselected open index operation must reject package evidence");
    assert!(error[0].message.contains("without exact checked selection"));

    let unsupported = program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let unsupported = program
        .type_reference_table
        .insert(TypeReferenceNode::ConstExpression(unsupported));
    let error = validate_package_type_identity_input_inner(&program, unsupported, &[], true)
        .expect_err("unsupported index shape must reject package evidence");
    assert!(error[0].message.contains("unsupported structural index"));

    let legacy_layout =
        program
            .type_reference_table
            .insert_constraints([TypeConstraintNode::Domain(DomainConstraint {
                name: Identifier::generated("OmegaLayout<Save>"),
                ..DomainConstraint::default()
            })]);
    let legacy_layout = program
        .type_reference_table
        .insert(TypeReferenceNode::Constrained {
            base_type: element_type,
            constraints: legacy_layout,
        });
    let error = validate_package_type_identity_input(&program, legacy_layout, &[])
        .expect_err("flattened layout spelling must reject package evidence");
    assert!(error[0].message.contains("legacy flattened OmegaLayout"));

    let malformed_carry =
        program
            .type_reference_table
            .insert_constraints([TypeConstraintNode::Domain(DomainConstraint {
                name: Identifier::generated("diagnostic-only"),
                arguments: vec![element_type],
                subject: DomainConstraintSubject::Carry(
                    psi_language_semantics::CarryPermission::AnyCpu,
                ),
                ..DomainConstraint::default()
            })]);
    let malformed_carry = program
        .type_reference_table
        .insert(TypeReferenceNode::Constrained {
            base_type: element_type,
            constraints: malformed_carry,
        });
    let error = validate_package_type_identity_input(&program, malformed_carry, &[])
        .expect_err("malformed closed domain must reject package evidence");
    assert!(error[0].message.contains("malformed compiler-owned scalar"));
}

fn toolchain_source(relative_path: &str, source: &str) -> SourceFile {
    namespaced_toolchain_source("std", relative_path, source)
}

fn namespaced_toolchain_source(namespace: &str, relative_path: &str, source: &str) -> SourceFile {
    let package_root = PathBuf::from("toolchain").join(namespace);
    SourceFile {
        source_id: SourceId(0),
        path: package_root.join(relative_path),
        package_root,
        package_identity: None,
        origin: SourceOrigin::Toolchain,
        source: Arc::from(source),
    }
}

fn virtual_toolchain_source(path: &str, source: &str) -> SourceFile {
    SourceFile {
        source_id: SourceId(0),
        path: PathBuf::from(path),
        package_root: PathBuf::from("toolchain/std"),
        package_identity: None,
        origin: SourceOrigin::Toolchain,
        source: Arc::from(source),
    }
}

fn generated_symbol_owner(
    origin: SourceOrigin,
    package_identity: Option<PackageKeyIdentity>,
) -> PackageReviewNominalOwner {
    let mut sources = SourceMap::default();
    let source_id = sources
        .add_with_metadata(
            PathBuf::from("toolchain/std/origin.omg"),
            String::from("origin"),
            PathBuf::from("toolchain/std"),
            package_identity,
            origin,
        )
        .source_id;
    let mut builder = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let authored = SymbolTableBuilder::child_handles(builder.insert_children(
        root,
        [(
            SymbolKind::Machine,
            SymbolNameRef::Source(SourceSpan::new(source_id, Span::new(0, 6))),
        )],
    ))
    .next()
    .expect("authored derivation origin");
    let mut symbols = builder.finish();
    let generated =
        symbols.insert_generated_root_from(authored, SymbolKind::Machine, "generated_origin");
    nominal_owner_from_symbols(&symbols, generated).expect("generated nominal owner")
}

#[test]
fn toolchain_source_identity_is_framed_over_path_and_exact_bytes() {
    let first = toolchain_source_identity(&toolchain_source("service.omg", "trait Host {}"))
        .expect("canonical toolchain source identity");
    let repeated = toolchain_source_identity(&toolchain_source("service.omg", "trait Host {}"))
        .expect("repeated canonical toolchain source identity");
    let changed_path = toolchain_source_identity(&toolchain_source("other.omg", "trait Host {}"))
        .expect("changed-path toolchain source identity");
    let changed_source =
        toolchain_source_identity(&toolchain_source("service.omg", "trait Host { }"))
            .expect("changed-source toolchain source identity");

    assert_eq!(first, repeated);
    assert_ne!(first, changed_path);
    assert_ne!(first, changed_source);
    assert_ne!(
        first,
        toolchain_source_identity(&namespaced_toolchain_source(
            "core",
            "service.omg",
            "trait Host {}",
        ))
        .expect("changed-namespace toolchain source identity")
    );
    assert_ne!(first.digest(), [0; 32]);
}

#[test]
fn toolchain_source_identity_accepts_only_canonical_virtual_coordinates() {
    let virtual_source =
        toolchain_source_identity(&virtual_toolchain_source("<build-prelude>", "data Build"))
            .expect("canonical virtual toolchain source identity");
    assert_ne!(virtual_source.digest(), [0; 32]);

    let error = toolchain_source_identity(&virtual_toolchain_source(
        "virtual/<build-prelude>",
        "data Build",
    ))
    .expect_err("nested virtual path outside the toolchain root must reject");
    assert!(error[0].message.contains("outside its canonical root"));
}

#[test]
fn generated_nominals_follow_exact_derivation_ownership() {
    assert!(matches!(
        generated_symbol_owner(SourceOrigin::Toolchain, None),
        PackageReviewNominalOwner::ToolchainSource(_)
    ));

    let package_identity =
        PackageKeyIdentity::from_digest([41; 32]).expect("nonzero package identity");
    assert_eq!(
        generated_symbol_owner(SourceOrigin::User, Some(package_identity)),
        PackageReviewNominalOwner::Package(package_identity)
    );

    let mut builder = SymbolTableBuilder::new();
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let source_free = SymbolTableBuilder::child_handles(builder.insert_children(
        root,
        [(SymbolKind::Machine, SymbolNameRef::Static("source_free"))],
    ))
    .next()
    .expect("source-free symbol");
    let symbols: SymbolTable = builder.finish();
    assert_eq!(
        nominal_owner_from_symbols(&symbols, source_free).expect("source-free nominal owner"),
        PackageReviewNominalOwner::Unresolved
    );
}
