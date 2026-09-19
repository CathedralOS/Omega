//! Wrapping and saturating arithmetic fixtures.

use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, MachineId, ObligationId, OperationId,
    Proposition, ScalarTerm, ScalarType, ValueId,
};
use terminal_psi::{
    Block, ContractClause, MachineContract, Operation, OperationKind, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
};

pub(super) fn wrapping_add_module() -> (TerminalModule, Proposition, ObligationId) {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let left = ValueId::new(20).expect("left parameter");
    let right = ValueId::new(21).expect("right parameter");
    let sum = ValueId::new(22).expect("sum result");
    let result = ValueId::new(23).expect("machine result");
    let obligation = ObligationId::new(20).expect("obligation");
    let term = |id| ScalarTerm::value(id, scalar_type);
    let goal = Proposition::Equal(
        term(result),
        ScalarTerm::wrapping_integer_add(integer, term(left), term(right)).unwrap(),
    );
    let machine = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: MachineId::new(20).expect("machine"),
        attachment: None,
        structural_parameters: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters: vec![
            ValueDeclaration {
                qualifications: Default::default(),
                id: left,
                scalar_type,
            },
            ValueDeclaration {
                qualifications: Default::default(),
                id: right,
                scalar_type,
            },
        ],
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
        entry: BlockId::new(20).expect("block"),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: BlockId::new(20).expect("block"),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: OperationId::new(20).expect("add operation"),
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: sum,
                    scalar_type,
                }),
                kind: OperationKind::WrappingIntegerAdd { left, right },
            }],
            terminator: Terminator::Return {
                cleanup_actions: Vec::new(),
                edge: EdgeId::new(20).expect("return edge"),
                value: sum,
            },
        }],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            id: ContractId::new(20).expect("contract"),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: vec![ContractClause {
                obligation,
                proposition: goal.clone(),
            }],
            outcome_specific_ensures: Vec::new(),
        },
    };
    (
        TerminalModule {
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
        goal,
        obligation,
    )
}

pub(super) fn saturating_add_module() -> (TerminalModule, Proposition, ObligationId) {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let left = ValueId::new(30).expect("left parameter");
    let right = ValueId::new(31).expect("right parameter");
    let sum = ValueId::new(32).expect("sum result");
    let result = ValueId::new(33).expect("machine result");
    let obligation = ObligationId::new(30).expect("obligation");
    let term = |id| ScalarTerm::value(id, scalar_type);
    let goal = Proposition::Equal(
        term(result),
        ScalarTerm::saturating_integer_add(integer, term(left), term(right)).unwrap(),
    );
    let machine = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: MachineId::new(30).expect("machine"),
        attachment: None,
        structural_parameters: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters: vec![
            ValueDeclaration {
                qualifications: Default::default(),
                id: left,
                scalar_type,
            },
            ValueDeclaration {
                qualifications: Default::default(),
                id: right,
                scalar_type,
            },
        ],
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
        entry: BlockId::new(30).expect("block"),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: BlockId::new(30).expect("block"),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: OperationId::new(30).expect("add operation"),
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: sum,
                    scalar_type,
                }),
                kind: OperationKind::SaturatingIntegerAdd { left, right },
            }],
            terminator: Terminator::Return {
                cleanup_actions: Vec::new(),
                edge: EdgeId::new(30).expect("return edge"),
                value: sum,
            },
        }],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            id: ContractId::new(30).expect("contract"),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: vec![ContractClause {
                obligation,
                proposition: goal.clone(),
            }],
            outcome_specific_ensures: Vec::new(),
        },
    };
    (
        TerminalModule {
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
        goal,
        obligation,
    )
}

pub(super) fn wrapping_subtract_module() -> (TerminalModule, Proposition, ObligationId) {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let left = ValueId::new(40).expect("left parameter");
    let right = ValueId::new(41).expect("right parameter");
    let difference = ValueId::new(42).expect("difference result");
    let result = ValueId::new(43).expect("machine result");
    let obligation = ObligationId::new(40).expect("obligation");
    let term = |id| ScalarTerm::value(id, scalar_type);
    let goal = Proposition::Equal(
        term(result),
        ScalarTerm::wrapping_integer_subtract(integer, term(left), term(right)).unwrap(),
    );
    let machine = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: MachineId::new(40).expect("machine"),
        attachment: None,
        structural_parameters: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters: vec![
            ValueDeclaration {
                qualifications: Default::default(),
                id: left,
                scalar_type,
            },
            ValueDeclaration {
                qualifications: Default::default(),
                id: right,
                scalar_type,
            },
        ],
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
        entry: BlockId::new(40).expect("block"),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: BlockId::new(40).expect("block"),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: OperationId::new(40).expect("subtract operation"),
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: difference,
                    scalar_type,
                }),
                kind: OperationKind::WrappingIntegerSubtract { left, right },
            }],
            terminator: Terminator::Return {
                cleanup_actions: Vec::new(),
                edge: EdgeId::new(40).expect("return edge"),
                value: difference,
            },
        }],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            id: ContractId::new(40).expect("contract"),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: vec![ContractClause {
                obligation,
                proposition: goal.clone(),
            }],
            outcome_specific_ensures: Vec::new(),
        },
    };
    (
        TerminalModule {
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
        goal,
        obligation,
    )
}

pub(super) fn saturating_subtract_module() -> (TerminalModule, Proposition, ObligationId) {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let left = ValueId::new(50).expect("left parameter");
    let right = ValueId::new(51).expect("right parameter");
    let difference = ValueId::new(52).expect("difference result");
    let result = ValueId::new(53).expect("machine result");
    let obligation = ObligationId::new(50).expect("obligation");
    let term = |id| ScalarTerm::value(id, scalar_type);
    let goal = Proposition::Equal(
        term(result),
        ScalarTerm::saturating_integer_subtract(integer, term(left), term(right)).unwrap(),
    );
    let machine = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: MachineId::new(50).expect("machine"),
        attachment: None,
        structural_parameters: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters: vec![
            ValueDeclaration {
                qualifications: Default::default(),
                id: left,
                scalar_type,
            },
            ValueDeclaration {
                qualifications: Default::default(),
                id: right,
                scalar_type,
            },
        ],
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
        entry: BlockId::new(50).expect("block"),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: BlockId::new(50).expect("block"),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: OperationId::new(50).expect("subtract operation"),
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: difference,
                    scalar_type,
                }),
                kind: OperationKind::SaturatingIntegerSubtract { left, right },
            }],
            terminator: Terminator::Return {
                cleanup_actions: Vec::new(),
                edge: EdgeId::new(50).expect("return edge"),
                value: difference,
            },
        }],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            id: ContractId::new(50).expect("contract"),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: vec![ContractClause {
                obligation,
                proposition: goal.clone(),
            }],
            outcome_specific_ensures: Vec::new(),
        },
    };
    (
        TerminalModule {
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
        goal,
        obligation,
    )
}

pub(super) fn wrapping_multiply_module() -> (TerminalModule, Proposition, ObligationId) {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let left = ValueId::new(60).expect("left parameter");
    let right = ValueId::new(61).expect("right parameter");
    let product = ValueId::new(62).expect("product result");
    let result = ValueId::new(63).expect("machine result");
    let obligation = ObligationId::new(60).expect("obligation");
    let term = |id| ScalarTerm::value(id, scalar_type);
    let goal = Proposition::Equal(
        term(result),
        ScalarTerm::wrapping_integer_multiply(integer, term(left), term(right)).unwrap(),
    );
    let machine = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: MachineId::new(60).expect("machine"),
        attachment: None,
        structural_parameters: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters: vec![
            ValueDeclaration {
                qualifications: Default::default(),
                id: left,
                scalar_type,
            },
            ValueDeclaration {
                qualifications: Default::default(),
                id: right,
                scalar_type,
            },
        ],
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
        entry: BlockId::new(60).expect("block"),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: BlockId::new(60).expect("block"),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: OperationId::new(60).expect("multiply operation"),
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: product,
                    scalar_type,
                }),
                kind: OperationKind::WrappingIntegerMultiply { left, right },
            }],
            terminator: Terminator::Return {
                cleanup_actions: Vec::new(),
                edge: EdgeId::new(60).expect("return edge"),
                value: product,
            },
        }],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            id: ContractId::new(60).expect("contract"),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: vec![ContractClause {
                obligation,
                proposition: goal.clone(),
            }],
            outcome_specific_ensures: Vec::new(),
        },
    };
    (
        TerminalModule {
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
        goal,
        obligation,
    )
}

pub(super) fn saturating_multiply_module() -> (TerminalModule, Proposition, ObligationId) {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let left = ValueId::new(70).expect("left parameter");
    let right = ValueId::new(71).expect("right parameter");
    let product = ValueId::new(72).expect("product result");
    let result = ValueId::new(73).expect("machine result");
    let obligation = ObligationId::new(70).expect("obligation");
    let term = |id| ScalarTerm::value(id, scalar_type);
    let goal = Proposition::Equal(
        term(result),
        ScalarTerm::saturating_integer_multiply(integer, term(left), term(right)).unwrap(),
    );
    let machine = TerminalMachine {
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
                id: left,
                scalar_type,
            },
            ValueDeclaration {
                qualifications: Default::default(),
                id: right,
                scalar_type,
            },
        ],
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
        entry: BlockId::new(70).expect("block"),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: BlockId::new(70).expect("block"),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: OperationId::new(70).expect("multiply operation"),
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: product,
                    scalar_type,
                }),
                kind: OperationKind::SaturatingIntegerMultiply { left, right },
            }],
            terminator: Terminator::Return {
                cleanup_actions: Vec::new(),
                edge: EdgeId::new(70).expect("return edge"),
                value: product,
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
    };
    (
        TerminalModule {
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
        goal,
        obligation,
    )
}
