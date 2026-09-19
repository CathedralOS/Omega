use super::{
    identity_reshuffle_module, multi_claim_structural_call_module, reflexive_content_module,
    structural_call_module,
};
use proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, PrimitiveJudgment, ProofNode, ProofRule,
    ProofSystemMarker,
};
use semantic_vocabulary::{
    BlockId, ClaimId, ContractId, EdgeId, EvidenceIdentity, IntegerSign, IntegerType, IntegerValue,
    MachineId, ObligationId, OperationId, PlaceId, Proposition, ScalarTerm, ScalarType,
    StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_psi::{
    Block, ContractClause, MachineContract, Operation, OperationKind, OperationResult,
    StructuralDomainDeclaration, StructuralFieldDeclaration, StructuralFieldType,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine, TerminalMachineResult,
    TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
};
use terminal_verifier::{
    ModuleError, ObligationEvidence, ProofBundle, reconstruct_operation_obligations,
    validate_module, verify_module,
};

#[test]
fn exact_subtract_requires_same_fixed_integer_operands_and_an_obligation() {
    let scalar_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).expect("u32"));
    let left = ValueId::new(198).expect("left");
    let right = ValueId::new(199).expect("right");
    let computed = ValueId::new(200).expect("computed");
    let result = ValueId::new(201).expect("result");
    let declaration = |id| ValueDeclaration {
        id,
        scalar_type,
        qualifications: Default::default(),
    };
    let mut module = TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(198).expect("machine"),
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
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(198).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![declaration(left), declaration(right)],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(result)),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(198).expect("block"),
            blocks: vec![Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: BlockId::new(198).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    static_reach_binding: None,
                    id: OperationId::new(198).expect("operation"),
                    result: terminal_psi::OperationResult::Scalar(declaration(computed)),
                    kind: OperationKind::ExactIntegerSubtract {
                        left,
                        right,
                        obligation: ObligationId::new(198).expect("subtract obligation"),
                    },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(198).expect("edge"),
                    value: computed,
                },
            }],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: ContractId::new(198).expect("contract"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    validate_module(&module).expect("admits proof-gated exact subtraction");

    module.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::ExactIntegerSubtractOperandTypeMismatch { .. })
    ));
}

#[test]
fn exact_multiply_requires_same_fixed_integer_operands_and_an_obligation() {
    let scalar_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).expect("u32"));
    let left = ValueId::new(202).expect("left");
    let right = ValueId::new(203).expect("right");
    let computed = ValueId::new(204).expect("computed");
    let result = ValueId::new(205).expect("result");
    let declaration = |id| ValueDeclaration {
        id,
        scalar_type,
        qualifications: Default::default(),
    };
    let mut module = TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(202).expect("machine"),
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
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(202).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![declaration(left), declaration(right)],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(result)),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(202).expect("block"),
            blocks: vec![Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: BlockId::new(202).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    static_reach_binding: None,
                    id: OperationId::new(202).expect("operation"),
                    result: terminal_psi::OperationResult::Scalar(declaration(computed)),
                    kind: OperationKind::ExactIntegerMultiply {
                        left,
                        right,
                        obligation: ObligationId::new(202).expect("multiply obligation"),
                    },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(202).expect("edge"),
                    value: computed,
                },
            }],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: ContractId::new(202).expect("contract"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    validate_module(&module).expect("admits proof-gated exact multiplication");

    let obligations = reconstruct_operation_obligations(&module)
        .expect("reconstruct total canonical multiplication goal");
    assert_eq!(obligations.len(), 1);
    assert!(
        obligations[0].canonical_certificate,
        "the last migrated arithmetic row must not retain legacy sufficient reduction",
    );

    module.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::ExactIntegerMultiplyOperandTypeMismatch { .. })
    ));
}

#[test]
fn exact_divide_requires_same_fixed_integer_operands_and_an_obligation() {
    let scalar_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).expect("u32"));
    let left = ValueId::new(212).expect("left");
    let right = ValueId::new(213).expect("right");
    let computed = ValueId::new(214).expect("computed");
    let result = ValueId::new(215).expect("result");
    let declaration = |id| ValueDeclaration {
        id,
        scalar_type,
        qualifications: Default::default(),
    };
    let mut module = TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(212).expect("machine"),
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
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(212).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![declaration(left), declaration(right)],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(result)),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(212).expect("block"),
            blocks: vec![Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: BlockId::new(212).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    static_reach_binding: None,
                    id: OperationId::new(212).expect("operation"),
                    result: terminal_psi::OperationResult::Scalar(declaration(computed)),
                    kind: OperationKind::ExactIntegerDivide {
                        left,
                        right,
                        obligation: ObligationId::new(212).expect("divide obligation"),
                    },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(212).expect("edge"),
                    value: computed,
                },
            }],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: ContractId::new(212).expect("contract"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    validate_module(&module).expect("admits proof-gated exact division");

    let without_available_route =
        reconstruct_operation_obligations(&module).expect("reconstruct canonical divide goal");
    assert_eq!(without_available_route.len(), 1);
    assert!(without_available_route[0].canonical_certificate);
    let canonical_question = without_available_route[0].obligation.proposition.clone();

    let mut module_with_alternate_route = module.clone();
    module_with_alternate_route.machines[0]
        .contract
        .requires
        .push(Proposition::LessOrEqual(
            ScalarTerm::integer(
                IntegerType::new(IntegerSign::Unsigned, 32).expect("u32"),
                IntegerValue::Unsigned(1),
            )
            .expect("u32 one"),
            ScalarTerm::value(right, scalar_type),
        ));
    let with_alternate_route = reconstruct_operation_obligations(&module_with_alternate_route)
        .expect("available proof routes do not choose the reconstructed goal");
    assert!(with_alternate_route[0].canonical_certificate);
    assert_eq!(
        with_alternate_route[0].obligation.proposition, canonical_question,
        "available evidence must not make reconstruction select another question",
    );

    module.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::ExactIntegerDivideOperandTypeMismatch { .. })
    ));
}

#[test]
fn exact_remainder_requires_same_fixed_integer_operands_and_an_obligation() {
    let scalar_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).expect("u32"));
    let left = ValueId::new(222).expect("left");
    let right = ValueId::new(223).expect("right");
    let computed = ValueId::new(224).expect("computed");
    let result = ValueId::new(225).expect("result");
    let declaration = |id| ValueDeclaration {
        id,
        scalar_type,
        qualifications: Default::default(),
    };
    let mut module = TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(222).expect("machine"),
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
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(222).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![declaration(left), declaration(right)],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(result)),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(222).expect("block"),
            blocks: vec![Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: BlockId::new(222).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    static_reach_binding: None,
                    id: OperationId::new(222).expect("operation"),
                    result: terminal_psi::OperationResult::Scalar(declaration(computed)),
                    kind: OperationKind::ExactIntegerRemainder {
                        left,
                        right,
                        obligation: ObligationId::new(222).expect("remainder obligation"),
                    },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(222).expect("edge"),
                    value: computed,
                },
            }],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: ContractId::new(222).expect("contract"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    validate_module(&module).expect("admits proof-gated exact remainder");

    module.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::ExactIntegerRemainderOperandTypeMismatch { .. })
    ));
}

#[test]
fn wrapping_divide_requires_same_fixed_integer_operands_and_an_obligation() {
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).expect("i32"));
    let left = ValueId::new(232).expect("left");
    let right = ValueId::new(233).expect("right");
    let computed = ValueId::new(234).expect("computed");
    let result = ValueId::new(235).expect("result");
    let declaration = |id| ValueDeclaration {
        id,
        scalar_type,
        qualifications: Default::default(),
    };
    let mut module = TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(232).expect("machine"),
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
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(232).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![declaration(left), declaration(right)],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(result)),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(232).expect("block"),
            blocks: vec![Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: BlockId::new(232).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    static_reach_binding: None,
                    id: OperationId::new(232).expect("operation"),
                    result: terminal_psi::OperationResult::Scalar(declaration(computed)),
                    kind: OperationKind::WrappingIntegerDivide {
                        left,
                        right,
                        obligation: ObligationId::new(232).expect("divide obligation"),
                    },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(232).expect("edge"),
                    value: computed,
                },
            }],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: ContractId::new(232).expect("contract"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    validate_module(&module).expect("admits proof-gated wrapping division");

    module.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::WrappingIntegerDivideOperandTypeMismatch { .. })
    ));
}

#[test]
fn wrapping_remainder_requires_same_fixed_integer_operands_and_an_obligation() {
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).expect("i32"));
    let left = ValueId::new(242).expect("left");
    let right = ValueId::new(243).expect("right");
    let computed = ValueId::new(244).expect("computed");
    let result = ValueId::new(245).expect("result");
    let declaration = |id| ValueDeclaration {
        id,
        scalar_type,
        qualifications: Default::default(),
    };
    let mut module = TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(242).expect("machine"),
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
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(242).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![declaration(left), declaration(right)],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(result)),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(242).expect("block"),
            blocks: vec![Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: BlockId::new(242).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    static_reach_binding: None,
                    id: OperationId::new(242).expect("operation"),
                    result: terminal_psi::OperationResult::Scalar(declaration(computed)),
                    kind: OperationKind::WrappingIntegerRemainder {
                        left,
                        right,
                        obligation: ObligationId::new(242).expect("remainder obligation"),
                    },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(242).expect("edge"),
                    value: computed,
                },
            }],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: ContractId::new(242).expect("contract"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    validate_module(&module).expect("admits proof-gated wrapping remainder");

    module.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::WrappingIntegerRemainderOperandTypeMismatch { .. })
    ));
}

#[test]
fn saturating_divide_requires_same_fixed_integer_operands_and_an_obligation() {
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).expect("i32"));
    let left = ValueId::new(252).expect("left");
    let right = ValueId::new(253).expect("right");
    let computed = ValueId::new(254).expect("computed");
    let result = ValueId::new(255).expect("result");
    let declaration = |id| ValueDeclaration {
        id,
        scalar_type,
        qualifications: Default::default(),
    };
    let mut module = TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(252).expect("machine"),
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
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(252).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![declaration(left), declaration(right)],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(result)),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(252).expect("block"),
            blocks: vec![Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: BlockId::new(252).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    static_reach_binding: None,
                    id: OperationId::new(252).expect("operation"),
                    result: terminal_psi::OperationResult::Scalar(declaration(computed)),
                    kind: OperationKind::SaturatingIntegerDivide {
                        left,
                        right,
                        obligation: ObligationId::new(252).expect("divide obligation"),
                    },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(252).expect("edge"),
                    value: computed,
                },
            }],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: ContractId::new(252).expect("contract"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    validate_module(&module).expect("admits proof-gated saturating division");

    module.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::SaturatingIntegerDivideOperandTypeMismatch { .. })
    ));
}

#[test]
fn saturating_remainder_requires_same_fixed_integer_operands_and_an_obligation() {
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).expect("i32"));
    let left = ValueId::new(256).expect("left");
    let right = ValueId::new(257).expect("right");
    let computed = ValueId::new(258).expect("computed");
    let result = ValueId::new(259).expect("result");
    let declaration = |id| ValueDeclaration {
        id,
        scalar_type,
        qualifications: Default::default(),
    };
    let mut module = TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(256).expect("machine"),
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
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(256).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![declaration(left), declaration(right)],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(result)),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(256).expect("block"),
            blocks: vec![Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: BlockId::new(256).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    static_reach_binding: None,
                    id: OperationId::new(256).expect("operation"),
                    result: terminal_psi::OperationResult::Scalar(declaration(computed)),
                    kind: OperationKind::SaturatingIntegerRemainder {
                        left,
                        right,
                        obligation: ObligationId::new(256).expect("remainder obligation"),
                    },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(256).expect("edge"),
                    value: computed,
                },
            }],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: ContractId::new(256).expect("contract"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    validate_module(&module).expect("admits proof-gated saturating remainder");

    module.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::SaturatingIntegerRemainderOperandTypeMismatch { .. })
    ));
}

#[test]
fn wrapping_shift_axioms_preserve_the_count_type() {
    let value_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 value type");
    let count_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16 count type");
    let value_scalar = ScalarType::Integer(value_type);
    let count_scalar = ScalarType::Integer(count_type);
    let value = ValueId::new(70).expect("value");
    let count = ValueId::new(71).expect("count");
    let computed = ValueId::new(72).expect("computed");
    let result = ValueId::new(73).expect("result");
    let value_term = |id| ScalarTerm::value(id, value_scalar);
    let count_term = |id| ScalarTerm::value(id, count_scalar);

    for kind in 0_u8..2 {
        let operation = if kind == 0 {
            OperationKind::WrappingIntegerShiftLeft { value, count }
        } else {
            OperationKind::WrappingIntegerShiftRight { value, count }
        };
        let term = if kind == 0 {
            ScalarTerm::wrapping_integer_shift_left(
                value_type,
                count_type,
                value_term(value),
                count_term(count),
            )
        } else {
            ScalarTerm::wrapping_integer_shift_right(
                value_type,
                count_type,
                value_term(value),
                count_term(count),
            )
        }
        .expect("independently typed shift operands");
        let goal = Proposition::Equal(value_term(result), term.clone());
        let obligation = ObligationId::new(70 + u64::from(kind)).expect("obligation");
        let module = TerminalModule {
            scalar_qualifications: Default::default(),
            scalar_block_invariants: Vec::new(),
            operation_crash_contracts: Vec::new(),
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: MachineId::new(70).expect("machine"),
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
            machines: vec![TerminalMachine {
                closed_reach_application: None,
                declared_service_reach: Vec::new(),
                id: MachineId::new(70).expect("machine"),
                attachment: None,
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters: vec![
                    ValueDeclaration {
                        qualifications: Default::default(),
                        id: value,
                        scalar_type: value_scalar,
                    },
                    ValueDeclaration {
                        qualifications: Default::default(),
                        id: count,
                        scalar_type: count_scalar,
                    },
                ],
                ranked_scc: None,
                result: TerminalMachineResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: result,
                    scalar_type: value_scalar,
                }),
                structural_places: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: BlockId::new(70).expect("block"),
                blocks: vec![Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: BlockId::new(70).expect("block"),
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        static_reach_binding: None,
                        id: OperationId::new(70).expect("operation"),
                        result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                            qualifications: Default::default(),
                            id: computed,
                            scalar_type: value_scalar,
                        }),
                        kind: operation,
                    }],
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: EdgeId::new(70).expect("edge"),
                        value: computed,
                    },
                }],
                contract: MachineContract {
                    erased_scalar_formals: Vec::new(),
                    id: ContractId::new(70).expect("contract"),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: vec![ContractClause {
                        obligation,
                        proposition: goal.clone(),
                    }],
                    outcome_specific_ensures: Vec::new(),
                },
            }],
        };
        let bundle = ProofBundle {
            recursive_components: Vec::new(),
            control_cycles: Vec::new(),
            evidence_producers: Vec::new(),
            evidence: vec![ObligationEvidence {
                obligation,
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: EvidenceIdentity::new(70 + u64::from(kind)).expect("certificate"),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof: ProofNode {
                        conclusion: goal,
                        rule: ProofRule::EqualityTransitivity {
                            left_equals_middle: Box::new(ProofNode {
                                conclusion: Proposition::Equal(
                                    value_term(result),
                                    value_term(computed),
                                ),
                                rule: ProofRule::SemanticAxiom { index: 1 },
                            }),
                            middle_equals_right: Box::new(ProofNode {
                                conclusion: Proposition::Equal(value_term(computed), term),
                                rule: ProofRule::SemanticAxiom { index: 0 },
                            }),
                        },
                    },
                }),
            }],
        };
        verify_module(&module, &bundle, &AdmissionProfile::default())
            .expect("reconstructs the exact wrapping-shift result axiom");

        if kind == 0 {
            let mut wrong_count = module.clone();
            wrong_count.machines[0].contract.ensures.clear();
            wrong_count.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
            assert!(matches!(
                validate_module(&wrong_count),
                Err(ModuleError::WrappingIntegerShiftOperandTypeMismatch { .. })
            ));

            let mut wrong_result = module.clone();
            wrong_result.machines[0].contract.ensures.clear();
            wrong_result.machines[0].blocks[0].operations[0]
                .result
                .scalar_mut()
                .unwrap()
                .scalar_type = ScalarType::Boolean;
            assert_eq!(
                validate_module(&wrong_result)
                    .expect_err("wrapping shift requires an integer result"),
                ModuleError::WrappingIntegerShiftRequiresIntegerResult(
                    OperationId::new(70).expect("operation")
                )
            );
        }
    }
}

#[test]
fn content_conservation_accepts_a_replaceable_certificate() {
    let (module, goal, obligation) = reflexive_content_module();
    let bundle = ProofBundle {
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(80).expect("certificate"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: goal,
                    rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
                },
            }),
        }],
    };

    let verified = verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("content proposition and certificate");
    assert_eq!(verified.accepted_facts().len(), 1);
}

#[test]
fn identity_reshuffle_reconstructs_content_equality_as_a_semantic_axiom() {
    let (module, goal, obligation) = identity_reshuffle_module();
    let bundle = ProofBundle {
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(90).expect("certificate"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: goal,
                    rule: ProofRule::SemanticAxiom { index: 0 },
                },
            }),
        }],
    };

    let verified = verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("an exact identity reshuffle should establish its content equality");
    assert_eq!(verified.accepted_facts().len(), 1);
}

#[test]
fn structural_return_rejects_inexact_custody_and_scalar_content_carriers() {
    let (module, _, _) = identity_reshuffle_module();

    let mut missing_structural_claim = module.clone();
    missing_structural_claim.machines[0].entry_claims.clear();
    assert!(matches!(
        validate_module(&missing_structural_claim),
        Err(ModuleError::LinearParameterHasNoEntryClaim { .. })
    ));

    let mut wrong_claim = module.clone();
    let Terminator::ReturnStructural {
        returned_claims, ..
    } = &mut wrong_claim.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    *returned_claims = vec![ClaimId::new(2).unwrap()];
    assert!(matches!(
        validate_module(&wrong_claim),
        Err(ModuleError::StructuralReturnClaimSetMismatch { .. })
    ));

    let mut wrong_source = module.clone();
    let result_place = wrong_source.machines[0].result.structural().unwrap().place;
    let Terminator::ReturnStructural { source, .. } =
        &mut wrong_source.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    *source = result_place;
    assert!(matches!(
        validate_module(&wrong_source),
        Err(ModuleError::StructuralReturnRequiresParameterSource { .. })
    ));

    let mut wrong_root = module.clone();
    wrong_root.machines[0].content_identity_reshuffles[0]
        .output
        .root = PlaceId::new(999).unwrap();
    assert!(matches!(
        validate_module(&wrong_root),
        Err(ModuleError::ContentIdentityReshuffleRequiresCurrentResult(
            _
        ))
    ));

    let mut wrong_signature = module.clone();
    let result = wrong_signature.machines[0]
        .result
        .structural()
        .unwrap()
        .clone();
    wrong_signature
        .structural_domains
        .push(terminal_psi::StructuralDomainDeclaration {
            id: semantic_vocabulary::StructuralDomainId::new(91).unwrap(),
            semantic_domain: semantic_vocabulary::DomainSemanticId::new(91).unwrap(),
            identity: "Other".into(),
            carrier: result.structural_type,
            content_projection: None,
        });
    let TerminalMachineResult::Structural(result) = &mut wrong_signature.machines[0].result else {
        unreachable!()
    };
    result
        .qualifications
        .push(semantic_vocabulary::StructuralDomainId::new(91).unwrap());
    assert!(matches!(
        validate_module(&wrong_signature),
        Err(ModuleError::StructuralReturnSignatureMismatch { .. })
    ));

    let mut scalar_carrier = module;
    scalar_carrier.machines[0].result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: ValueId::new(999).unwrap(),
        scalar_type: ScalarType::Boolean,
    });
    assert!(matches!(
        validate_module(&scalar_carrier),
        Err(ModuleError::ScalarMachineHasResultStructuralPlace { .. })
    ));

    scalar_carrier.machines[0].result = TerminalMachineResult::Unit;
    assert!(matches!(
        validate_module(&scalar_carrier),
        Err(ModuleError::UnitMachineHasResultStructuralPlace { .. })
    ));
}

#[test]
fn structural_call_rebinds_one_exact_whole_root_claim_and_rejects_tampering() {
    let module = structural_call_module();
    validate_module(&module).expect("one exact whole-root structural call");

    let mut wrong_source_claim = module.clone();
    let OperationKind::CallStructural {
        returned_claim_transfers,
        ..
    } = &mut wrong_source_claim.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    returned_claim_transfers[0].callee_claim = ClaimId::new(2).unwrap();
    assert_eq!(
        validate_module(&wrong_source_claim).unwrap_err(),
        ModuleError::StructuralCallClaimInterfaceMismatch(OperationId::new(1).unwrap())
    );

    let mut wrong_producer = module.clone();
    let StructuralPlaceKind::OperationResult { producer, .. } =
        &mut wrong_producer.machines[0].structural_places[2].kind
    else {
        unreachable!()
    };
    *producer = OperationId::new(2).unwrap();
    assert_eq!(
        validate_module(&wrong_producer).unwrap_err(),
        ModuleError::StructuralCallResultPlaceMismatch(OperationId::new(2).unwrap())
    );

    let mut duplicated_claim = module.clone();
    let result = duplicated_claim.machines[0].blocks[0].operations[0]
        .result
        .structural()
        .unwrap()
        .clone();
    let OperationResult::Structural(result_mut) =
        &mut duplicated_claim.machines[0].blocks[0].operations[0].result
    else {
        unreachable!()
    };
    result_mut.claims.push(result.claims[0].clone());
    assert_eq!(
        validate_module(&duplicated_claim).unwrap_err(),
        ModuleError::NonCanonicalStructuralOperationResult(OperationId::new(1).unwrap())
    );
}

#[test]
fn structural_call_and_return_copy_one_exact_projected_result_roster() {
    let mut module = structural_call_module();
    let root = module.structural_types[0].id;
    let leaf = StructuralTypeId::new(90).unwrap();
    let domain = semantic_vocabulary::StructuralDomainId::new(90).unwrap();
    module.structural_types[0].shape = StructuralTypeShape::Record {
        fields: vec![StructuralFieldDeclaration {
            id: semantic_vocabulary::StructuralFieldId::new(90).unwrap(),
            identity: "payload".into(),
            relevance: terminal_psi::BindingRelevance::Relevant,
            field_type: StructuralFieldType::Structural(leaf),
        }],
    };
    module.structural_types.push(StructuralTypeDeclaration {
        id: leaf,
        identity: "ReceiptPayload".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    module.structural_domains.push(StructuralDomainDeclaration {
        id: domain,
        semantic_domain: semantic_vocabulary::DomainSemanticId::new(90).unwrap(),
        identity: "ReceiptPayload::Ready".into(),
        carrier: leaf,
        content_projection: None,
    });
    let row = terminal_psi::StructuralPathQualification {
        path: vec![terminal_psi::StructuralPathSegment::Field("payload".into())],
        domain,
    };
    for machine in &mut module.machines {
        machine.structural_parameters[0].projected_qualifications = vec![row.clone()];
        let TerminalMachineResult::Structural(result) = &mut machine.result else {
            panic!("fixture machines return structural values")
        };
        assert_eq!(result.structural_type, root);
        result.projected_qualifications = vec![row.clone()];
    }
    let OperationResult::Structural(call_result) =
        &mut module.machines[0].blocks[0].operations[0].result
    else {
        panic!("fixture call returns a structural value")
    };
    call_result.projected_qualifications = vec![row];
    validate_module(&module)
        .expect("callee result, call result, and both return edges retain the exact roster");

    let mut missing_call_row = module.clone();
    let OperationResult::Structural(call_result) =
        &mut missing_call_row.machines[0].blocks[0].operations[0].result
    else {
        unreachable!()
    };
    call_result.projected_qualifications.clear();
    assert!(matches!(
        validate_module(&missing_call_row),
        Err(ModuleError::StructuralCallTargetMismatch { .. })
    ));

    let mut missing_return_row = module;
    let TerminalMachineResult::Structural(result) = &mut missing_return_row.machines[0].result
    else {
        unreachable!()
    };
    result.projected_qualifications.clear();
    assert!(matches!(
        validate_module(&missing_return_row),
        Err(ModuleError::StructuralReturnSignatureMismatch { .. })
    ));
}

#[test]
fn structural_call_rebinds_an_exact_multi_claim_whole_root_map() {
    let module = multi_claim_structural_call_module();
    validate_module(&module).expect("two exact projected claims cross a whole-root call");

    let mut swapped_paths = module.clone();
    let OperationKind::CallStructural {
        returned_claim_transfers,
        ..
    } = &mut swapped_paths.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let first_caller = returned_claim_transfers[0].caller_claim;
    returned_claim_transfers[0].caller_claim = returned_claim_transfers[1].caller_claim;
    returned_claim_transfers[1].caller_claim = first_caller;
    assert_eq!(
        validate_module(&swapped_paths).unwrap_err(),
        ModuleError::StructuralCallClaimInterfaceMismatch(OperationId::new(1).unwrap())
    );

    let mut missing_return = module.clone();
    let OperationKind::CallStructural {
        returned_claim_transfers,
        ..
    } = &mut missing_return.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    returned_claim_transfers.pop();
    assert_eq!(
        validate_module(&missing_return).unwrap_err(),
        ModuleError::StructuralCallClaimInterfaceMismatch(OperationId::new(1).unwrap())
    );

    let mut duplicate_caller = module.clone();
    let OperationKind::CallStructural {
        returned_claim_transfers,
        ..
    } = &mut duplicate_caller.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    returned_claim_transfers[1].caller_claim = ClaimId::new(1).unwrap();
    assert_eq!(
        validate_module(&duplicate_caller).unwrap_err(),
        ModuleError::StructuralCallClaimInterfaceMismatch(OperationId::new(1).unwrap())
    );

    let mut callee_returns_a_subset = module;
    let Terminator::ReturnStructural {
        returned_claims, ..
    } = &mut callee_returns_a_subset.machines[1].blocks[0].terminator
    else {
        unreachable!()
    };
    returned_claims.pop();
    assert_eq!(
        validate_module(&callee_returns_a_subset).unwrap_err(),
        ModuleError::StructuralCallClaimInterfaceMismatch(OperationId::new(1).unwrap())
    );
}
