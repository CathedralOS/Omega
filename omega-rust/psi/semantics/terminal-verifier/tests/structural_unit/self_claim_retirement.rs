//! A claim established on a consumed owned receiver retires exactly once, on
//! the terminal completion edge, and only claims rooted at that receiver.
//!
//! A by-value `self` parameter on linear attached data is a terminal consumer:
//! the checker records no ordinary body permission event for it, so the claim
//! minted for the receiver at entry is discharged when the machine completes.
//! The retirement is exact — claims rooted at any other place remain live,
//! receiver claims cannot be re-rooted or doubled, and crash exits still name
//! the claim in their frontier lower bound.

use super::*;
use terminal_psi::{CrashCause, CrashRouteBucket, CrashRouteGuard};

fn self_consumer_machine() -> TerminalMachine {
    TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: machine_id(1),
        attachment: Some(structural_type_id(1)),
        parameters: Vec::new(),
        structural_parameters: vec![StructuralParameterDeclaration {
            place: place_id(1),
            position: 0,
            is_self: true,
            structural_type: structural_type_id(1),
            multiplicity: StructuralMultiplicity::Linear,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }],
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: vec![StructuralPlaceDeclaration {
            id: place_id(1),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: true,
            },
        }],
        entry_claims: vec![EntryClaim {
            claim: claim_id(1),
            input: place_id(1),
            path: Vec::new(),
        }],
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(1),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(1),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: edge_id(1),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: empty_contract(contract_id(1)),
    }
}

fn self_consumer_module() -> TerminalModule {
    TerminalModule {
        entry: machine_id(1),
        structural_types: vec![
            StructuralTypeDeclaration {
                id: structural_type_id(1),
                identity: "Task".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![StructuralFieldDeclaration {
                        id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                        identity: "payload".into(),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(structural_type_id(2)),
                    }],
                },
            },
            StructuralTypeDeclaration {
                id: structural_type_id(2),
                identity: "Payload".into(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
        ],
        machines: vec![self_consumer_machine()],
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: terminal_psi::TerminalRootServiceReach {
            concrete: Vec::new(),
            installation_dependencies: Vec::new(),
        },
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
    }
}

fn second_linear_parameter() -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: place_id(2),
        position: 1,
        is_self: false,
        structural_type: structural_type_id(1),
        multiplicity: StructuralMultiplicity::Linear,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}

fn second_parameter_place() -> StructuralPlaceDeclaration {
    StructuralPlaceDeclaration {
        id: place_id(2),
        kind: StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    }
}

#[test]
fn an_owned_self_receiver_claim_retires_at_unit_return() {
    validate_module(&self_consumer_module()).expect("consumed receiver claim retires");
}

#[test]
fn claims_rooted_outside_the_consumed_receiver_stay_live() {
    let mut module = self_consumer_module();
    let machine = &mut module.machines[0];
    machine
        .structural_parameters
        .push(second_linear_parameter());
    machine.structural_places.push(second_parameter_place());
    machine.entry_claims.push(EntryClaim {
        claim: claim_id(2),
        input: place_id(2),
        path: Vec::new(),
    });

    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::LiveLinearClaimAtUnitReturn {
            machine: machine_id(1),
            block: block_id(1),
            claim: claim_id(2),
        }
    );
}

#[test]
fn a_subclaim_rooted_within_the_receiver_retires_with_it() {
    let mut module = self_consumer_module();
    module.machines[0].entry_claims[0].path = vec![StructuralPathSegment::Field("payload".into())];

    validate_module(&module).expect("receiver sub-claim retires with its root");
}

#[test]
fn a_borrowed_receiver_claim_cannot_retire() {
    let mut module = self_consumer_module();
    module.machines[0].structural_parameters[0].access = StructuralAccess::MutableBorrow;

    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::LiveLinearClaimAtUnitReturn {
            machine: machine_id(1),
            block: block_id(1),
            claim: claim_id(1),
        }
    );
}

#[test]
fn receiver_claims_cannot_be_re_rooted_to_another_parameter() {
    let mut module = self_consumer_module();
    let machine = &mut module.machines[0];
    machine
        .structural_parameters
        .push(second_linear_parameter());
    machine.structural_places.push(second_parameter_place());
    // Re-rooting the receiver claim at the other parameter satisfies that
    // parameter's entry requirement but leaves the receiver unclaimed.
    machine.entry_claims[0].input = place_id(2);

    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::LinearParameterHasNoEntryClaim {
            machine: machine_id(1),
            place: place_id(1),
        }
    );
}

#[test]
fn a_second_claim_rooted_within_the_receiver_overlaps() {
    let mut module = self_consumer_module();
    module.machines[0].entry_claims.push(EntryClaim {
        claim: claim_id(2),
        input: place_id(1),
        path: vec![StructuralPathSegment::Field("payload".into())],
    });

    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::OverlappingEntryClaimInput {
            first: claim_id(1),
            second: claim_id(2),
        }
    );
}

#[test]
fn an_owned_self_receiver_claim_retires_at_scalar_return() {
    let mut module = self_consumer_module();
    let machine = &mut module.machines[0];
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(21),
        scalar_type: ScalarType::Boolean,
    });
    machine.blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: operation_id(1),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(20),
            scalar_type: ScalarType::Boolean,
        }),
        kind: OperationKind::BooleanConstant { value: true },
    });
    machine.blocks[0].terminator = Terminator::Return {
        edge: edge_id(1),
        value: value_id(20),
        cleanup_actions: Vec::new(),
    };

    validate_module(&module).expect("the receiver claim retires while a scalar result returns");
}

#[test]
fn a_returned_value_does_not_drain_the_receiver_claim() {
    let mut module = self_consumer_module();
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(3),
        identity: "Token".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let machine = &mut module.machines[0];
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: place_id(2),
            position: 1,
            is_self: false,
            structural_type: structural_type_id(3),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    machine.structural_places.push(second_parameter_place());
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(3),
        kind: StructuralPlaceKind::Result,
    });
    machine.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        place: place_id(3),
        structural_type: structural_type_id(3),
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        reference_sources: Vec::new(),
    });
    machine.blocks[0].terminator = Terminator::ReturnStructural {
        edge: edge_id(1),
        source: place_id(2),
        returned_claims: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };

    validate_module(&module)
        .expect("the receiver claim retires while an unrestricted result returns");
}

#[test]
fn a_crash_names_the_receiver_claim_in_its_frontier() {
    let mut module = self_consumer_module();
    let machine = &mut module.machines[0];
    machine.contract.crash_routes = vec![CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Truth],
    }];
    machine.blocks[0].terminator = Terminator::Crash {
        edge: edge_id(1),
        cause: CrashCause::Trap,
        site_guard: Vec::new(),
        frontier_lower_bound: vec![claim_id(1)],
    };

    validate_module(&module).expect("crash lower bound names the receiver claim");
}

#[test]
fn a_crash_cannot_release_the_receiver_claim_implicitly() {
    let mut module = self_consumer_module();
    let machine = &mut module.machines[0];
    machine.contract.crash_routes = vec![CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Truth],
    }];
    machine.blocks[0].terminator = Terminator::Crash {
        edge: edge_id(1),
        cause: CrashCause::Trap,
        site_guard: Vec::new(),
        frontier_lower_bound: Vec::new(),
    };

    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::CrashFrontierMismatch { block: block_id(1) }
    );
}
