//! Quotient validation tests.

use super::{
    exact_relation_application_matches, first_forbidden_carrier_content, validate_quotients,
};
use crate::proof_contracts::quotients::carrier_fence::CarrierFenceViolation;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataField, DataMember};
use typed_trees::domain::{DomainAliasConstituent, DomainAliasDefinition, DomainDefinition};
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, QuotientOperationKind, QuotientOperationRequest,
    StaticMachineArgument, TableCallExpression, TableNamePath,
};
use typed_trees::name::Identifier;
use typed_trees::proposition::{
    PropositionApplication, PropositionBinder, PropositionBinderArgument,
    PropositionBinderArgumentKind, PropositionBinderKind, PropositionDefinition,
};
use typed_trees::types::{
    DomainConstraint, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

fn static_argument(name: &'static str) -> StaticMachineArgument {
    StaticMachineArgument {
        path: vec![Identifier::generated_static(name)].into_boxed_slice(),
        application: None,
        type_reference: Default::default(),
        const_literal: None,
        evidence_projection: None,
        symbol: SymbolHandle::invalid(),
    }
}

fn recursive_proof_carrier_with(
    contained: Option<(SymbolHandle, &'static str, language_semantics::Multiplicity)>,
) -> (TypedTrees, TypeReferenceHandle) {
    let mut program = TypedTrees::default();
    let carrier_symbol = SymbolHandle::from_arena_index(20);
    let carrier_type = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: carrier_symbol,
            name: Identifier::generated_static("Carrier"),
        });
    let mut carrier = DataDefinition {
        symbol: carrier_symbol,
        name: Identifier::generated_static("Carrier"),
        ..Default::default()
    };
    program.push_data_member(
        &mut carrier,
        DataMember::Field(DataField {
            symbol: SymbolHandle::from_arena_index(21),
            name: Identifier::generated_static("next"),
            type_reference: carrier_type,
            ..Default::default()
        }),
    );
    if let Some((symbol, name, multiplicity)) = contained {
        let contained_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol,
                name: Identifier::generated_static(name),
            });
        program.push_data_member(
            &mut carrier,
            DataMember::Field(DataField {
                symbol: SymbolHandle::from_arena_index(22),
                name: Identifier::generated_static("payload"),
                type_reference: contained_type,
                ..Default::default()
            }),
        );
        program.push_data_definition(DataDefinition {
            symbol,
            name: Identifier::generated_static(name),
            properties: typed_trees::data::DataProperties {
                multiplicity,
                carry: None,
            },
            ..Default::default()
        });
    }
    program.push_data_definition(carrier);
    (program, carrier_type)
}

fn constrained_copy_type(
    program: &mut TypedTrees,
    domain: DomainConstraint,
) -> TypeReferenceHandle {
    let copy_symbol = SymbolHandle::from_arena_index(40);
    let copy_type = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: copy_symbol,
            name: Identifier::generated_static("CopyValue"),
        });
    program.push_data_definition(DataDefinition {
        symbol: copy_symbol,
        name: Identifier::generated_static("CopyValue"),
        properties: typed_trees::data::DataProperties {
            multiplicity: language_semantics::Multiplicity::Unrestricted,
            carry: None,
        },
        ..Default::default()
    });
    let constraints = program
        .type_reference_table
        .insert_constraints([TypeConstraintNode::Domain(domain)]);
    program
        .type_reference_table
        .insert(TypeReferenceNode::Constrained {
            base_type: copy_type,
            constraints,
        })
}

fn checked_route() -> language_semantics::DomainEstablishmentRoute {
    language_semantics::DomainEstablishmentRoute::CheckedRequirement {
        trait_definition: SymbolHandle::from_arena_index(90),
        requirement: SymbolHandle::from_arena_index(91),
    }
}

#[test]
fn recursive_proof_carrier_without_runtime_content_passes_noncopy_fence() {
    let (program, carrier) = recursive_proof_carrier_with(None);
    let proof_only = typed_trees::proof_only::classify(&program);

    assert_eq!(
        first_forbidden_carrier_content(
            &program,
            &proof_only,
            carrier,
            &mut std::collections::HashSet::new(),
        ),
        None,
    );
}

#[test]
fn recursive_proof_carrier_rejects_contained_affine_runtime_type() {
    let token_symbol = SymbolHandle::from_arena_index(30);
    let (program, carrier) = recursive_proof_carrier_with(Some((
        token_symbol,
        "Token",
        language_semantics::Multiplicity::Affine,
    )));
    let proof_only = typed_trees::proof_only::classify(&program);

    assert_eq!(
        first_forbidden_carrier_content(
            &program,
            &proof_only,
            carrier,
            &mut std::collections::HashSet::new(),
        ),
        Some(CarrierFenceViolation::NonCopyType("Token".to_owned())),
    );
}

#[test]
fn recursive_proof_carrier_accepts_contained_copy_runtime_type() {
    let token_symbol = SymbolHandle::from_arena_index(31);
    let (program, carrier) = recursive_proof_carrier_with(Some((
        token_symbol,
        "CopyToken",
        language_semantics::Multiplicity::Unrestricted,
    )));
    let proof_only = typed_trees::proof_only::classify(&program);

    assert_eq!(
        first_forbidden_carrier_content(
            &program,
            &proof_only,
            carrier,
            &mut std::collections::HashSet::new(),
        ),
        None,
    );
}

#[test]
fn routed_qualification_on_copy_content_rejects() {
    let mut program = TypedTrees::default();
    let routed = constrained_copy_type(
        &mut program,
        DomainConstraint {
            name: Identifier::generated_static("Issued"),
            establishment_routes: vec![checked_route()],
            ..Default::default()
        },
    );
    let proof_only = typed_trees::proof_only::classify(&program);

    assert_eq!(
        first_forbidden_carrier_content(
            &program,
            &proof_only,
            routed,
            &mut std::collections::HashSet::new(),
        ),
        Some(CarrierFenceViolation::RoutedQualification(
            "Issued".to_owned()
        )),
    );
}

#[test]
fn predicate_only_qualification_on_copy_content_passes() {
    let mut program = TypedTrees::default();
    let predicate_only = constrained_copy_type(
        &mut program,
        DomainConstraint {
            name: Identifier::generated_static("NonZero"),
            predicate_body: language_semantics::DomainPredicateBody::Present,
            ..Default::default()
        },
    );
    let proof_only = typed_trees::proof_only::classify(&program);

    assert_eq!(
        first_forbidden_carrier_content(
            &program,
            &proof_only,
            predicate_only,
            &mut std::collections::HashSet::new(),
        ),
        None,
    );
}

#[test]
fn transparent_alias_cannot_hide_routed_qualification() {
    let mut program = TypedTrees::default();
    let routed_symbol = SymbolHandle::from_arena_index(50);
    let alias_symbol = SymbolHandle::from_arena_index(51);
    program.push_domain_definition(DomainDefinition {
        symbol: routed_symbol,
        name: Identifier::generated_static("Issued"),
        establishment_routes: vec![checked_route()],
        ..Default::default()
    });
    program.push_domain_definition(DomainDefinition {
        symbol: alias_symbol,
        name: Identifier::generated_static("Usable"),
        alias: Some(DomainAliasDefinition {
            constituents: vec![DomainAliasConstituent {
                domain_symbol: routed_symbol,
                ..Default::default()
            }],
        }),
        ..Default::default()
    });
    let aliased = constrained_copy_type(
        &mut program,
        DomainConstraint {
            name: Identifier::generated_static("Usable"),
            symbol: alias_symbol,
            ..Default::default()
        },
    );
    let proof_only = typed_trees::proof_only::classify(&program);

    assert_eq!(
        first_forbidden_carrier_content(
            &program,
            &proof_only,
            aliased,
            &mut std::collections::HashSet::new(),
        ),
        Some(CarrierFenceViolation::RoutedQualification(
            "Issued".to_owned()
        )),
    );
}

#[test]
fn retained_sealed_request_is_not_executable_admission() {
    let mut program = TypedTrees::default();
    let arguments = program
        .expression_table
        .insert_expression_handles(std::iter::empty());
    program
        .expression_table
        .insert(ExpressionNode::Call(TableCallExpression {
            receiver: ExpressionHandle::invalid(),
            target_symbol: SymbolHandle::invalid(),
            target: Identifier::generated_static("lift"),
            static_machine_parameter: symbols::SymbolHandle::invalid(),
            static_requirement_dispatch: None,
            machine_arguments: Box::default(),
            quotient_operation: Some(QuotientOperationRequest {
                kind: QuotientOperationKind::Lift,
                representative_operation: static_argument("representative"),
                theorem_evidence: vec![typed_trees::expression::QuotientTheoremSelection {
                    role: typed_trees::expression::QuotientTheoremRole::Congruence,
                    application: static_argument("ExactRespect"),
                }]
                .into_boxed_slice(),
            }),
            private_layout_operation: None,
            arguments,
            evidence_arguments: Box::default(),
            operational_acknowledgement: Default::default(),
        }));
    let proof_only = typed_trees::proof_only::classify(&program);
    let mut diagnostics = Vec::new();

    validate_quotients(&program, &proof_only, &mut diagnostics);

    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0]
            .message
            .contains("executable quotient operations are not admitted")
    );
}

#[test]
fn quotient_relation_application_rejects_swapped_generic_binders() {
    let mut program = TypedTrees::default();
    let relation_symbol = SymbolHandle::from_arena_index(1);
    let left_symbol = SymbolHandle::from_arena_index(2);
    let right_symbol = SymbolHandle::from_arena_index(3);
    let left_binder = SymbolHandle::from_arena_index(4);
    let right_binder = SymbolHandle::from_arena_index(5);
    let family_symbol = SymbolHandle::from_arena_index(6);

    let mut relation = PropositionDefinition {
        symbol: relation_symbol,
        name: Identifier::generated_static("Related"),
        ..Default::default()
    };
    for (symbol, name) in [(left_binder, "L"), (right_binder, "R")] {
        program.push_proposition_binder(
            &mut relation,
            PropositionBinder {
                symbol,
                name: Identifier::generated_static(name),
                kind: PropositionBinderKind::Machine,
                ..Default::default()
            },
        );
    }
    program.push_proposition(relation);

    let left_argument = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: left_binder,
            name: Identifier::generated_static("L"),
        });
    let right_argument = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: right_binder,
            name: Identifier::generated_static("R"),
        });
    let left_arguments = program
        .type_reference_table
        .insert_type_reference_handles([left_argument]);
    let right_arguments = program
        .type_reference_table
        .insert_type_reference_handles([right_argument]);
    let left_type = program
        .type_reference_table
        .insert(TypeReferenceNode::Generic {
            base_symbol: family_symbol,
            base_name: Identifier::generated_static("Carrier"),
            lifetime_arguments: Vec::new(),
            arguments: left_arguments,
        });
    let right_type = program
        .type_reference_table
        .insert(TypeReferenceNode::Generic {
            base_symbol: family_symbol,
            base_name: Identifier::generated_static("Carrier"),
            lifetime_arguments: Vec::new(),
            arguments: right_arguments,
        });

    let left = program
        .expression_table
        .insert(ExpressionNode::Name(TableNamePath {
            head_symbol: left_symbol,
            symbol: left_symbol,
            ..Default::default()
        }));
    let right = program
        .expression_table
        .insert(ExpressionNode::Name(TableNamePath {
            head_symbol: right_symbol,
            symbol: right_symbol,
            ..Default::default()
        }));
    let arguments = program
        .expression_table
        .insert_expression_handles([left, right]);
    let binder_argument = |symbol| PropositionBinderArgument {
        kind: PropositionBinderArgumentKind::Machine,
        path: Box::default(),
        const_literal: None,
        evidence_projection: None,
        symbol,
    };
    let application = |binders: [SymbolHandle; 2]| PropositionApplication {
        proposition: relation_symbol,
        name: Identifier::generated_static("Related"),
        binder_arguments: binders.map(binder_argument).into(),
        arguments,
    };

    assert!(exact_relation_application_matches(
        &program,
        &application([left_binder, right_binder]),
        relation_symbol,
        left_symbol,
        right_symbol,
        left_type,
        right_type,
    ));
    assert!(!exact_relation_application_matches(
        &program,
        &application([right_binder, left_binder]),
        relation_symbol,
        left_symbol,
        right_symbol,
        left_type,
        right_type,
    ));
}
