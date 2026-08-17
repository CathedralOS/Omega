//! Root-level checked-to-terminal producer regressions.

use super::*;
use psi_language_semantics::{
    PermissionEventSource, SemanticDomainId,
    content::{
        ContentCaseSegment, ContentConservationEquation, ContentConservationOwnerKind,
        ContentFieldSegment,
    },
};
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_symbols::SymbolHandle;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

mod attached_unit_cases;
mod content_conservation;
mod scalar_graph;
mod structural_control_cases;
mod structural_return_cases;
mod unit_cleanup;

#[test]
fn shared_boolean_comparison_normalization_rejects_two_runtime_sides() {
    let comparison = LoweredBooleanReturnExpression::Equal {
        left: Box::new(LoweredBooleanReturnExpression::Parameter { position: 0 }),
        right: Box::new(LoweredBooleanReturnExpression::Parameter { position: 1 }),
    };
    assert!(normalize_shared_boolean_comparison_leaves(&comparison).is_none());

    let local_comparison = LoweredBooleanReturnExpression::Equal {
        left: Box::new(LoweredBooleanReturnExpression::Local { position: 1 }),
        right: Box::new(LoweredBooleanReturnExpression::Constant { value: false }),
    };
    assert!(normalize_shared_boolean_comparison_leaves(&local_comparison).is_none());
}

#[test]
fn scalar_crash_disjunction_lowers_to_canonical_terminal_propositions() {
    let values = vec![
        ValueDeclaration {
            id: value_id(2),
            scalar_type: ScalarType::Boolean,
        },
        ValueDeclaration {
            id: value_id(1),
            scalar_type: ScalarType::Boolean,
        },
    ];
    let proposition = checked_boolean_proposition(
        &CheckedBooleanExpression::Or {
            left: Box::new(CheckedBooleanExpression::Parameter { position: 0 }),
            right: Box::new(CheckedBooleanExpression::Parameter { position: 1 }),
        },
        &values,
    )
    .expect("scalar disjunction lowers");
    let Proposition::Disjunction(disjuncts) = &proposition else {
        panic!("scalar disjunction retains proposition structure")
    };
    assert_eq!(disjuncts.len(), 2);
    let keys = disjuncts
        .iter()
        .map(|disjunct| psi_terminal_codec::canonical_proposition_order_key(disjunct).unwrap())
        .collect::<Vec<_>>();
    assert!(keys[0] < keys[1]);
    PropositionContext::from_value_types(values.iter().map(|value| (value.id, value.scalar_type)))
        .unwrap()
        .validate(&proposition)
        .expect("scalar disjunction is well typed");
}

fn unit_claim_at(
    machine: SymbolHandle,
    state: SymbolHandle,
    ordinal: u32,
) -> PermissionClaimIdentity {
    PermissionClaimIdentity::Established {
        machine_symbol: machine,
        state_symbol: state,
        source: PermissionEventSource::StateEntry,
        ordinal,
    }
}

fn unit_claim(machine: SymbolHandle, state: SymbolHandle) -> PermissionClaimIdentity {
    unit_claim_at(machine, state, 0)
}

fn hard_root_checked_fixture() -> CheckedTrees {
    let root = SymbolHandle::from_arena_index(1);
    let helper = SymbolHandle::from_arena_index(2);
    let boundary = SymbolHandle::from_arena_index(3);
    let root_state = SymbolHandle::from_arena_index(11);
    let helper_state = SymbolHandle::from_arena_index(12);
    let boundary_state = SymbolHandle::from_arena_index(13);
    let port_service_symbol = SymbolHandle::from_arena_index(20);
    let domain = SemanticDomainId(9);

    let mut checked = CheckedTrees::default();
    let port_service = checked
        .facts
        .service_reaches
        .services
        .intern(port_service_symbol, "PortIo");
    let empty_reach = checked.facts.service_reaches.rows.intern(Vec::new());
    assert_eq!(
        empty_reach,
        psi_language_semantics::ServiceReachRowTable::EMPTY_ROW
    );
    let port_reach = checked
        .facts
        .service_reaches
        .rows
        .intern(vec![port_service]);
    let reach = ServiceReachSummary {
        direct: port_reach,
        transitive: port_reach,
    };
    let contract_reach = ServiceReachPlan {
        interface: ServiceReachInterface::PublishedCeiling(port_reach),
        checked_inferred: port_reach,
    };
    checked.facts.flow.terminal_machines = psi_checked_trees::CheckedTerminalMachineSelections {
        machines: vec![
            CheckedTerminalMachineSelection {
                machine: root,
                name: "example::Root::enter".to_owned(),
                signature: CheckedTerminalSignatureEligibility::Attached,
            },
            CheckedTerminalMachineSelection {
                machine: helper,
                name: "example::Helper::run".to_owned(),
                signature: CheckedTerminalSignatureEligibility::Attached,
            },
            CheckedTerminalMachineSelection {
                machine: boundary,
                name: "example::Acknowledgement::settle".to_owned(),
                signature: CheckedTerminalSignatureEligibility::Attached,
            },
        ],
    };
    let structural_parameter = |position| psi_checked_trees::CheckedUnitStructuralParameterPlan {
        position,
        is_self: false,
        type_identity: "example::Acknowledgement".to_owned(),
        multiplicity: Multiplicity::Linear,
        qualifications: vec![domain],
    };
    let entry_claim = |machine, state| psi_checked_trees::CheckedUnitEntryClaimPlan {
        claim_identity: unit_claim(machine, state),
        parameter_index: 0,
        path: Vec::new(),
        carry: CarryPolicy::STRICT,
    };
    checked.facts.flow.terminal_unit_effects = psi_checked_trees::CheckedUnitEffectPlans {
        structural_types: vec![
            psi_checked_trees::CheckedUnitStructuralTypePlan {
                identity: "example::Acknowledgement".to_owned(),
                shape: CheckedUnitStructuralTypeShape::Record {
                    fields: vec![
                        psi_checked_trees::CheckedUnitStructuralFieldPlan {
                            identity: "sequence".to_owned(),
                            relevance: psi_terminal::BindingRelevance::Relevant,
                            field_type: CheckedUnitStructuralFieldType::Scalar(PrimitiveType::U64),
                        },
                        psi_checked_trees::CheckedUnitStructuralFieldPlan {
                            identity: "proof".to_owned(),
                            relevance: psi_terminal::BindingRelevance::Erased,
                            field_type: CheckedUnitStructuralFieldType::Erased {
                                type_identity: "named(name(example::Evidence))".to_owned(),
                            },
                        },
                    ],
                },
            },
            psi_checked_trees::CheckedUnitStructuralTypePlan {
                identity: "example::Helper".to_owned(),
                shape: CheckedUnitStructuralTypeShape::Record { fields: Vec::new() },
            },
            psi_checked_trees::CheckedUnitStructuralTypePlan {
                identity: "example::Root".to_owned(),
                shape: CheckedUnitStructuralTypeShape::Record { fields: Vec::new() },
            },
        ],
        structural_domains: vec![psi_checked_trees::CheckedUnitStructuralDomainPlan {
            domain,
            identity: "example::Acknowledgement::Pending".to_owned(),
            carrier_type_identity: "example::Acknowledgement".to_owned(),
        }],
        boundary_machines: vec![CheckedBoundaryMachinePlan {
            machine: boundary,
            state: boundary_state,
            attachment_type_identity: Some("example::Acknowledgement".to_owned()),
            structural_parameters: vec![psi_checked_trees::CheckedUnitStructuralParameterPlan {
                is_self: true,
                ..structural_parameter(0)
            }],
            result_type: None,
            domain_requirements: vec![
                psi_checked_trees::CheckedUnitStructuralDomainRequirementPlan {
                    argument_index: 0,
                    domain,
                },
            ],
            contract_fingerprint: 0x303,
            contract_service_reach: contract_reach,
            service_reach: reach,
        }],
        machines: vec![
            CheckedUnitEffectMachinePlan {
                machine: root,
                state: root_state,
                attachment_type_identity: "example::Root".to_owned(),
                structural_parameters: vec![structural_parameter(7)],
                trivial_affine_locals: Vec::new(),
                entry_claims: vec![entry_claim(root, root_state)],
                body_qualifications: vec![domain],
                contract_fingerprint: 0x101,
                contract_service_reach: contract_reach,
                service_reach: reach,
                operations: vec![
                    CheckedUnitEffectOperationPlan::CallUnit {
                        coordinate: psi_checked_trees::CheckedUnitCallCoordinate {
                            statement_index: 0,
                            call_ordinal: 0,
                        },
                        target_machine: helper,
                        target_state: helper_state,
                        target_contract_fingerprint: 0x202,
                        service_reach: reach,
                        structural_arguments: vec![
                            psi_checked_trees::CheckedUnitStructuralArgumentPlan {
                                source_parameter_index: 0,
                                type_identity: "example::Acknowledgement".to_owned(),
                                path: Vec::new(),
                            },
                        ],
                        claim_transfers: vec![psi_checked_trees::CheckedUnitClaimTransferPlan {
                            claim_identity: unit_claim(root, root_state),
                            argument_index: 0,
                        }],
                    },
                    CheckedUnitEffectOperationPlan::ReturnUnit {
                        statement_index: 1,
                        trivial_affine_local_discard_ordinals: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                ],
            },
            CheckedUnitEffectMachinePlan {
                machine: helper,
                state: helper_state,
                attachment_type_identity: "example::Helper".to_owned(),
                structural_parameters: vec![structural_parameter(3)],
                trivial_affine_locals: Vec::new(),
                entry_claims: vec![entry_claim(helper, helper_state)],
                body_qualifications: vec![domain],
                contract_fingerprint: 0x202,
                contract_service_reach: contract_reach,
                service_reach: reach,
                operations: vec![
                    CheckedUnitEffectOperationPlan::PortWrite {
                        coordinate: psi_checked_trees::CheckedUnitCallCoordinate {
                            statement_index: 0,
                            call_ordinal: 0,
                        },
                        port: 0x3f8,
                        value: 0x5a,
                        service_reach: reach,
                    },
                    CheckedUnitEffectOperationPlan::BoundaryCall {
                        coordinate: psi_checked_trees::CheckedUnitCallCoordinate {
                            statement_index: 1,
                            call_ordinal: 0,
                        },
                        target_machine: boundary,
                        target_state: boundary_state,
                        target_contract_fingerprint: 0x303,
                        service_reach: reach,
                        structural_arguments: vec![
                            psi_checked_trees::CheckedUnitStructuralArgumentPlan {
                                source_parameter_index: 0,
                                type_identity: "example::Acknowledgement".to_owned(),
                                path: Vec::new(),
                            },
                        ],
                        completion_receipts: vec![
                            psi_checked_trees::CheckedUnitClaimTransferPlan {
                                claim_identity: unit_claim(helper, helper_state),
                                argument_index: 0,
                            },
                        ],
                    },
                    CheckedUnitEffectOperationPlan::ReturnUnit {
                        statement_index: 2,
                        trivial_affine_local_discard_ordinals: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                ],
            },
        ],
    };
    checked
}

fn source_projection(
    version: CheckedContentPlaceVersion,
    root: CheckedContentPlaceRoot,
    fields: &[(&str, u32)],
    semantic_domain: SemanticDomainId,
) -> CheckedContentConservationTerm {
    CheckedContentConservationTerm::Projection {
        domain: SymbolHandle::from_arena_index(70),
        semantic_domain,
        projection_machine: SymbolHandle::from_arena_index(71),
        projection_fingerprint: 0xfeed,
        subject: CheckedContentStructuralPlace {
            version,
            root,
            segments: fields
                .iter()
                .map(|(name, symbol)| {
                    CheckedContentPlaceSegment::Field(ContentFieldSegment {
                        symbol: SymbolHandle::from_arena_index(*symbol),
                        name: (*name).to_owned(),
                    })
                })
                .collect(),
        },
    }
}
