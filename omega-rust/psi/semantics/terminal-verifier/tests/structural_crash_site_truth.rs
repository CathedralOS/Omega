//! A mutable field read cannot establish an invocation-entry crash predicate.

use proof_admission::AdmissionProfile;
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, MachineId, OperationId, PlaceId, Proposition, PsiSemanticId,
    ScalarTerm, ScalarType, StructuralFieldId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_psi::{
    BindingRelevance, Block, CrashCause, CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard,
    MachineContract, Operation, OperationKind, OperationResult, StructuralAccess,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, SuccessorEdge, TerminalMachine, TerminalMachineResult, TerminalModule,
    Terminator, ValueDeclaration, VocabularyMarker,
};
use terminal_verifier::{ModuleError, ProofBundle, VerificationError, verify_module};

fn id<Identity: PsiSemanticId>(raw: u64) -> Identity {
    Identity::new(raw).unwrap()
}

fn predicate(term: ScalarTerm, expected: bool) -> CrashPredicateTerm {
    let mut terms = [term, ScalarTerm::boolean(expected)];
    terms.sort();
    CrashPredicateTerm::new(Proposition::Equal(terms[0].clone(), terms[1].clone()))
}

fn field_predicate() -> CrashPredicateTerm {
    predicate(ScalarTerm::boolean_field(id(1), id(1)), true)
}

fn module(store_first: bool, entry_predicate: bool) -> TerminalModule {
    let mut operations = Vec::new();
    if store_first {
        operations.extend([
            Operation {
                id: id::<OperationId>(1),
                result: OperationResult::Scalar(ValueDeclaration {
                    id: id::<ValueId>(1),
                    scalar_type: ScalarType::Boolean,
                }),
                kind: OperationKind::BooleanConstant { value: true },
            },
            Operation {
                id: id::<OperationId>(2),
                result: OperationResult::Unit,
                kind: OperationKind::StructuralScalarFieldStore {
                    destination: id(1),
                    path: Vec::new(),
                    field: id(1),
                    value: id(1),
                },
            },
        ]);
    }
    operations.push(Operation {
        id: id::<OperationId>(3),
        result: OperationResult::Scalar(ValueDeclaration {
            id: id::<ValueId>(2),
            scalar_type: ScalarType::Boolean,
        }),
        kind: OperationKind::BooleanStructuralField {
            source: id(1),
            field: id(1),
        },
    });
    let guard = if entry_predicate {
        field_predicate()
    } else {
        predicate(ScalarTerm::value(id(2), ScalarType::Boolean), true)
    };
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: id::<MachineId>(1),
        structural_types: vec![StructuralTypeDeclaration {
            id: id::<StructuralTypeId>(1),
            identity: "test::MutableFlag".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: id::<StructuralFieldId>(1),
                    identity: "flag".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                }],
            },
        }],
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
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
        machines: vec![TerminalMachine {
            id: id::<MachineId>(1),
            attachment: Some(id(1)),
            parameters: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                place: id::<PlaceId>(1),
                position: 0,
                is_self: true,
                structural_type: id(1),
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::MutableBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }],
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: vec![StructuralPlaceDeclaration {
                id: id::<PlaceId>(1),
                kind: StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: true,
                },
            }],
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: id::<BlockId>(1),
            blocks: vec![
                Block {
                    id: id(1),
                    parameters: Vec::new(),
                    operations,
                    terminator: Terminator::Conditional {
                        condition: id(2),
                        when_true: SuccessorEdge {
                            edge: id(1),
                            target: id(2),
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            edge: id(2),
                            target: id(3),
                            arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                },
                Block {
                    id: id(2),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Crash {
                        edge: id(3),
                        cause: CrashCause::Trap,
                        site_guard: vec![guard],
                        frontier_lower_bound: Vec::new(),
                    },
                },
                Block {
                    id: id(3),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: id(4),
                        trivial_affine_discards: Vec::new(),
                    },
                },
            ],
            contract: MachineContract {
                id: id::<ContractId>(1),
                requires: Vec::new(),
                ensures: Vec::new(),
                crash_routes: vec![CrashRouteBucket {
                    cause: CrashCause::Trap,
                    alternatives: vec![if entry_predicate {
                        CrashRouteGuard::Predicate(field_predicate())
                    } else {
                        CrashRouteGuard::Truth
                    }],
                }],
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

fn verify(module: &TerminalModule) -> Result<(), VerificationError> {
    verify_module(
        module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .map(|_| ())
}

#[test]
fn current_read_branch_remains_valid_after_a_mutable_field_store() {
    verify(&module(true, false)).unwrap();
}

#[test]
fn a_shared_field_read_can_establish_the_entry_field_predicate() {
    let mut checked = module(false, true);
    checked.machines[0].structural_parameters[0].access = StructuralAccess::SharedBorrow;
    verify(&checked).unwrap();
}

#[test]
fn nonshared_field_reads_need_retained_origin_before_proving_entry_predicates() {
    for access in [StructuralAccess::MutableBorrow, StructuralAccess::Owned] {
        // No store is present, but this slice does not reconstruct nonshared
        // field origin. The current SSA branch remains independently valid.
        let mut control = module(false, false);
        control.machines[0].structural_parameters[0].access = access;
        verify(&control).unwrap();
        let mut unsupported_origin = module(false, true);
        unsupported_origin.machines[0].structural_parameters[0].access = access;
        let error =
            verify(&unsupported_origin).expect_err("nonshared field origin is not retained");
        assert!(
            matches!(error, VerificationError::Module(ModuleError::CrashSiteGuardUnproved {
                block, edge, predicate: 0,
            }) if block == id::<BlockId>(2) && edge == id::<EdgeId>(3)),
            "{access:?}: {error:?}"
        );
    }
}

#[test]
fn an_explicit_entry_requirement_establishes_the_mutable_field_entry_predicate() {
    for store_first in [false, true] {
        let mut checked = module(store_first, true);
        checked.machines[0].contract.requires = vec![field_predicate().proposition().clone()];
        verify(&checked).unwrap();
    }
}

#[test]
fn a_stored_true_field_does_not_prove_it_was_true_at_invocation_entry() {
    // An entry value of false is permitted: there is deliberately no Requires.
    // The body changes it to true, so this branch cannot certify the published
    // entry predicate merely by reading the current field value.
    let error = verify(&module(true, true)).expect_err("a current field is not an entry snapshot");
    assert!(
        matches!(error, VerificationError::Module(ModuleError::CrashSiteGuardUnproved {
        block, edge, predicate: 0,
    }) if block == id::<BlockId>(2) && edge == id::<EdgeId>(3)),
        "{error:?}"
    );
}
