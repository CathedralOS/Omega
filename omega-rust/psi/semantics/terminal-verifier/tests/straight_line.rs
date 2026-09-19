//! Fixtures shared by the straight-line verification tests: placed view
//! inputs and the reshuffle, content, wrapping, saturating and guard modules.

#[path = "straight_line/affine_local_frontier.rs"]
mod affine_local_frontier;
#[path = "straight_line/arithmetic_modules.rs"]
mod arithmetic_modules;
#[path = "straight_line/arithmetic_obligations.rs"]
mod arithmetic_obligations;
#[path = "straight_line/case_access.rs"]
mod case_access;
#[path = "straight_line/case_payload.rs"]
mod case_payload;
#[path = "straight_line/content_modules.rs"]
mod content_modules;
#[path = "straight_line/contract_fields.rs"]
mod contract_fields;
#[path = "straight_line/false_edge_custody.rs"]
mod false_edge_custody;
#[path = "straight_line/guards_and_partitions.rs"]
mod guards_and_partitions;
#[path = "straight_line/indexed_primitive_store.rs"]
mod indexed_primitive_store;
#[path = "straight_line/integer_axioms_and_carriers.rs"]
mod integer_axioms_and_carriers;
#[path = "straight_line/literal_byte_extent.rs"]
mod literal_byte_extent;
#[path = "straight_line/proof_and_guard_modules.rs"]
mod proof_and_guard_modules;
#[path = "straight_line/scalar_record.rs"]
mod record;
#[path = "straight_line/scalar_qualifications.rs"]
mod scalar_qualifications;
#[path = "straight_line/shared_record_loans.rs"]
mod shared_record_loans;
#[path = "straight_line/shared_scalar_loans.rs"]
mod shared_scalar_loans;
#[path = "straight_line/structural_byte_sequence_store.rs"]
mod structural_byte_sequence_store;
#[path = "straight_line/unit_returns_and_certificates.rs"]
mod unit_returns_and_certificates;
#[path = "straight_line/wrapping_and_saturating_axioms.rs"]
mod wrapping_and_saturating_axioms;

use arithmetic_modules::{
    saturating_add_module, saturating_multiply_module, saturating_subtract_module,
    wrapping_add_module, wrapping_multiply_module, wrapping_subtract_module,
};
use content_modules::{
    identity_reshuffle_module, multi_claim_structural_call_module, partition_composition_module,
    reflexive_content_module, structural_call_module,
};
use proof_and_guard_modules::{
    contract_id, multi_exit_payloadless_guard_module, payloadless_guard_module,
    proof_recursive_bundle, proof_recursive_module, unit_module,
};

use proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceError, EvidenceRoute, PrimitiveJudgment,
    ProofNode, ProofRule, ProofSystemMarker,
};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ContractId, EdgeId, EvidenceIdentity, IntegerSign, IntegerType,
    IntegerValue, MachineId, ObligationId, OperationId, PlaceId, Proposition, ScalarTerm,
    ScalarType, StructuralCaseId, StructuralCaseSubject, StructuralPlaceKind, StructuralTypeId,
    ValueId,
};
use terminal_psi::{
    Block, BoundaryMachineDeclaration, ContractClause, CrashCause, MachineContract, Operation,
    OperationKind, OperationResult, OutcomeSpecificEnsure, OutcomeSpecificGuard, StructuralAccess,
    StructuralArgument, StructuralCaseDeclaration, StructuralDomainDeclaration,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralOperationResult, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralResultDeclaration, StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge,
    TerminalMachine, TerminalMachineResult, TerminalModule, TerminalPlacedViewInput, Terminator,
    ValueDeclaration, VocabularyMarker,
};
use terminal_verifier::{
    ModuleError, ObligationEvidence, ProofBundle, VerificationError,
    reconstruct_operation_obligations, reconstruct_terminal_obligations, validate_module,
    verify_module,
};

fn placed_view_input(machine: MachineId, position: u32) -> TerminalPlacedViewInput {
    placed_view_input_at_state(machine, "inspect::entry", position)
}

fn placed_view_input_at_state(
    machine: MachineId,
    state: &str,
    position: u32,
) -> TerminalPlacedViewInput {
    let identity = |name: &str| format!("package:{}::{name}", "01".repeat(32));
    let policy_identity = identity("Uart");
    let schema_identity = identity("Registers");
    TerminalPlacedViewInput {
        machine,
        position,
        source_machine_identity: identity("inspect"),
        source_state_identity: identity(state),
        source_parameter_identity: identity(&format!("{state}::view{position}")),
        access: StructuralAccess::MutableBorrow,
        binding_is_const: false,
        binding_is_mutable: true,
        view_identity: terminal_psi::canonical_placed_view_identity(
            &policy_identity,
            &schema_identity,
        ),
        policy_identity,
        policy_plan_machine_identity: identity("Uart::plan"),
        schema_identity,
        placement_report_fingerprint: 41,
        placement_commitment: [0x5a; 32],
    }
}

struct Fixture {
    module: TerminalModule,
    integer: IntegerType,
    constant: ValueId,
    forwarded: ValueId,
    result: ValueId,
    obligation: ObligationId,
}

impl Fixture {
    fn new() -> Self {
        let integer = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
        let scalar_type = ScalarType::Integer(integer);
        let constant = ValueId::new(1).expect("constant value");
        let forwarded = ValueId::new(2).expect("forwarded value");
        let result = ValueId::new(3).expect("result value");
        let obligation = ObligationId::new(1).expect("ensures obligation");
        let seven = ScalarTerm::integer(integer, IntegerValue::Signed(7)).expect("seven");
        let goal = Proposition::Equal(ScalarTerm::value(result, scalar_type), seven);

        let machine = TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(1).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: result,
                scalar_type,
            }),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(1).expect("entry block"),
            blocks: vec![
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: BlockId::new(1).expect("entry block"),
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        static_reach_binding: None,
                        id: OperationId::new(1).expect("constant operation"),
                        result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                            qualifications: Default::default(),
                            id: constant,
                            scalar_type,
                        }),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Signed(7),
                        },
                    }],
                    terminator: Terminator::Jump {
                        structural_arguments: Vec::new(),
                        edge: EdgeId::new(1).expect("jump edge"),
                        target: BlockId::new(2).expect("exit block"),
                        arguments: vec![constant],
                        erased_arguments: Vec::new(),
                        residual_affine_discards: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: BlockId::new(2).expect("exit block"),
                    parameters: vec![ValueDeclaration {
                        qualifications: Default::default(),
                        id: forwarded,
                        scalar_type,
                    }],
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: EdgeId::new(2).expect("return edge"),
                        value: forwarded,
                    },
                },
            ],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: ContractId::new(1).expect("contract"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: vec![ContractClause {
                    obligation,
                    proposition: goal,
                }],
                outcome_specific_ensures: Vec::new(),
            },
        };
        Self {
            module: TerminalModule {
                scalar_qualifications: Default::default(),
                scalar_block_invariants: Vec::new(),
                operation_crash_contracts: Vec::new(),
                vocabulary_marker: VocabularyMarker::CURRENT,
                entry: machine.id,
                structural_types: Vec::new(),
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
                machines: vec![machine],
            },
            integer,
            constant,
            forwarded,
            result,
            obligation,
        }
    }

    fn proof_bundle(&self) -> ProofBundle {
        let scalar_type = ScalarType::Integer(self.integer);
        let term = |id| ScalarTerm::value(id, scalar_type);
        let seven = ScalarTerm::integer(self.integer, IntegerValue::Signed(7)).expect("seven");
        let constant_fact = Proposition::Equal(term(self.constant), seven.clone());
        let forwarding_fact = Proposition::Equal(term(self.forwarded), term(self.constant));
        let return_fact = Proposition::Equal(term(self.result), term(self.forwarded));
        let forwarded_is_seven = Proposition::Equal(term(self.forwarded), seven.clone());
        let goal = Proposition::Equal(term(self.result), seven);
        let proof = ProofNode {
            conclusion: goal,
            rule: ProofRule::EqualityTransitivity {
                left_equals_middle: Box::new(ProofNode {
                    conclusion: return_fact,
                    rule: ProofRule::SemanticAxiom { index: 2 },
                }),
                middle_equals_right: Box::new(ProofNode {
                    conclusion: forwarded_is_seven,
                    rule: ProofRule::EqualityTransitivity {
                        left_equals_middle: Box::new(ProofNode {
                            conclusion: forwarding_fact,
                            rule: ProofRule::SemanticAxiom { index: 1 },
                        }),
                        middle_equals_right: Box::new(ProofNode {
                            conclusion: constant_fact,
                            rule: ProofRule::SemanticAxiom { index: 0 },
                        }),
                    },
                }),
            },
        };
        ProofBundle {
            recursive_components: Vec::new(),
            control_cycles: Vec::new(),
            evidence_producers: Vec::new(),
            evidence: vec![ObligationEvidence {
                obligation: self.obligation,
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: EvidenceIdentity::new(1).expect("certificate"),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof,
                }),
            }],
        }
    }
}
