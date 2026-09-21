//! Representation-specialization verified fixtures: source-produced machines
//! whose `StructuralCaseMembership` observations the unit itself proves —
//! an `EstablishScalarCase` producer on the observed operation-result place —
//! and a near-miss machine whose parameter membership no basis can prove.

use super::super::VerifiedPsiOptimizationUnit;
use super::admission::verified_unit;

/// Two memberships observe one established place: `c in Choice::Some` folds
/// to `true` and `c in Choice::Empty` folds to `false` inside a single
/// candidate covering the place, so the pass commits exactly once.
const ESTABLISHED_MEMBERSHIPS_SOURCE: &str = r#"
    data Choice { case Empty; case Some(value: u32); }
    machine probe() -> bool {
        let c: Choice = Choice::Some { value: 37 };
        (c in Choice::Some) == (c in Choice::Empty)
    }
"#;

/// A membership on a machine parameter place over a multi-case roster carries
/// no establishment proof and no sole-case roster: the parameter arrives with
/// whatever case the caller supplied, so the whole selected registry must
/// decline it.
const PARAMETER_MEMBERSHIP_SOURCE: &str = r#"
    data Choice { case Empty; case Some(value: u32); }
    machine probe(c: Choice) -> bool {
        c in Choice::Some
    }
"#;

fn lowered_module(
    source: &str,
    label: &str,
    entry: &str,
) -> (terminal_psi::TerminalModule, terminal_verifier::ProofBundle) {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap_or_else(|error| panic!("tokenize {label}: {error:?}"));
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens)
        .unwrap_or_else(|error| panic!("parse {label}: {error:?}"));
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap_or_else(|error| panic!("resolve {label}: {error:?}"));
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .unwrap_or_else(|error| panic!("type {label}: {error:?}"));
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .unwrap_or_else(|error| panic!("check {label}: {error:?}"));
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, entry)
        .unwrap_or_else(|error| panic!("lower {label}: {error:?}"));
    (lowered.semantic_module, lowered.proof_bundle)
}

/// The established-place positive: two memberships on `c`'s place fold in one
/// candidate whose producer is the `EstablishScalarCase` that fixed `Some`.
pub(in crate::pass_manager::tests) fn verified_representation_specialization_unit()
-> VerifiedPsiOptimizationUnit {
    let (module, proof) = lowered_module(
        ESTABLISHED_MEMBERSHIPS_SOURCE,
        "established memberships",
        "probe",
    );
    verified_unit(&module, &proof)
}

/// The boundary decline: a real membership the unit cannot prove, so the
/// selection's admission edge commits nothing.
pub(in crate::pass_manager::tests) fn verified_membership_decline_unit()
-> VerifiedPsiOptimizationUnit {
    let (module, proof) = lowered_module(
        PARAMETER_MEMBERSHIP_SOURCE,
        "parameter membership decline",
        "probe",
    );
    verified_unit(&module, &proof)
}

/// The cyclic freeze: a roster-proven membership inside a machine carrying an
/// authenticated cyclic component must stay byte-exact under the selection.
/// Source cannot express this shape — multi-state machines refuse structural
/// formals, and the verifier's unranked-cycle fence admits only
/// parameter-sourced structural work — so the Terminal module is built
/// directly: an unranked self-loop header observes its owned `Token`
/// parameter, and `Token`'s sole-case roster proves the verdict.
pub(in crate::pass_manager::tests) fn verified_cyclic_membership_unit()
-> VerifiedPsiOptimizationUnit {
    use semantic_vocabulary::{
        BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
        OperationId, PlaceId, ScalarType, StructuralCaseId, StructuralFieldId, StructuralPlaceKind,
        StructuralTypeId, ValueId,
    };
    use terminal_psi::{
        Block, MachineContract, Operation, OperationKind, OperationResult, StructuralAccess,
        StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralFieldType,
        StructuralMultiplicity, StructuralParameterDeclaration, StructuralPlaceDeclaration,
        StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge, TerminalMachine,
        TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
    };

    let value = |raw| ValueId::new(raw).unwrap();
    let edge = |raw| EdgeId::new(raw).unwrap();
    let block = |raw| BlockId::new(raw).unwrap();
    let successor = |edge, target, arguments| SuccessorEdge {
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        edge,
        target,
        arguments,
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let unsigned_32 = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    let token = StructuralTypeId::new(601).unwrap();
    let only = StructuralCaseId::new(602).unwrap();
    let token_place = PlaceId::new(520).unwrap();

    let module = TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(501).unwrap(),
        structural_types: vec![StructuralTypeDeclaration {
            id: token,
            identity: "Token".into(),
            shape: StructuralTypeShape::Sum {
                cases: vec![StructuralCaseDeclaration {
                    id: only,
                    identity: "Only".into(),
                    fields: vec![StructuralFieldDeclaration {
                        id: StructuralFieldId::new(603).unwrap(),
                        identity: "value".into(),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(ScalarType::Integer(unsigned_32)),
                    }],
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
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(501).unwrap(),
            attachment: None,
            parameters: vec![ValueDeclaration {
                qualifications: Default::default(),
                id: value(502),
                scalar_type: ScalarType::Boolean,
            }],
            structural_parameters: vec![StructuralParameterDeclaration {
                place: token_place,
                position: 0,
                is_self: false,
                structural_type: token,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::Owned,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }],
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: vec![StructuralPlaceDeclaration {
                id: token_place,
                kind: StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            }],
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block(503),
            blocks: vec![
                Block {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: block(503),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Jump {
                        erased_arguments: Vec::new(),
                        erased_proof_arguments: Vec::new(),
                        edge: edge(504),
                        target: block(505),
                        arguments: vec![value(502)],
                        structural_arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                        residual_affine_discards: Vec::new(),
                    },
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: block(505),
                    parameters: vec![ValueDeclaration {
                        qualifications: Default::default(),
                        id: value(506),
                        scalar_type: ScalarType::Boolean,
                    }],
                    operations: vec![
                        Operation {
                            static_reach_binding: None,
                            suspension_crossing: None,
                            id: OperationId::new(507).unwrap(),
                            result: OperationResult::Scalar(ValueDeclaration {
                                qualifications: Default::default(),
                                id: value(508),
                                scalar_type: ScalarType::Integer(unsigned_32),
                            }),
                            kind: OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(7),
                            },
                        },
                        Operation {
                            static_reach_binding: None,
                            suspension_crossing: None,
                            id: OperationId::new(521).unwrap(),
                            result: OperationResult::Scalar(ValueDeclaration {
                                qualifications: Default::default(),
                                id: value(522),
                                scalar_type: ScalarType::Boolean,
                            }),
                            kind: OperationKind::StructuralCaseMembership {
                                source: token_place,
                                path: Vec::new(),
                                case: only,
                            },
                        },
                    ],
                    terminator: Terminator::Conditional {
                        condition: value(506),
                        when_true: successor(edge(509), block(505), vec![value(506)]),
                        when_false: successor(edge(510), block(511), Vec::new()),
                    },
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: block(511),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: edge(512),
                        trivial_affine_discards: Vec::new(),
                    },
                },
            ],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                id: ContractId::new(513).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let semantic = terminal_codec::encode_module(&module).unwrap();
    let proof =
        terminal_codec::encode_proof_section(&module, &terminal_verifier::ProofBundle::default())
            .unwrap();
    let input = terminal_psi_to_abstract_operations::lower_artifact_for_optimization(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: None,
        },
        &proof_admission::AdmissionProfile::default(),
    )
    .and_then(|admitted| admitted.try_into_optimization_input())
    .unwrap();
    terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap()
}
