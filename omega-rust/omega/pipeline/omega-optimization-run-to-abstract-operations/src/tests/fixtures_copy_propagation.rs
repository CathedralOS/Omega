//! Copy-propagation projection fixtures.

use super::*;

pub(super) fn redundant_block_parameter_verified() -> VerifiedPsiOptimizationUnit {
    let machine = MachineId::new(1_031).unwrap();
    let entry = BlockId::new(1_032).unwrap();
    let exit = BlockId::new(1_033).unwrap();
    let constant = ValueId::new(1_034).unwrap();
    let forwarded = ValueId::new(1_035).unwrap();
    let result = ValueId::new(1_036).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
    let declaration = |id| ValueDeclaration { id, scalar_type };
    verified(
        module_with_blocks(
            machine,
            entry,
            TerminalMachineResult::Scalar(declaration(result)),
            vec![
                Block {
                    id: entry,
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        id: OperationId::new(1_037).unwrap(),
                        result: OperationResult::Scalar(declaration(constant)),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(9),
                        },
                    }],
                    terminator: Terminator::Jump {
                        edge: EdgeId::new(1_038).unwrap(),
                        target: exit,
                        arguments: vec![constant],
                        trivial_affine_discards: Vec::new(),
                    },
                },
                Block {
                    id: exit,
                    parameters: vec![declaration(forwarded)],
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: EdgeId::new(1_039).unwrap(),
                        value: forwarded,
                    },
                },
            ],
        ),
        ProofBundle::default(),
    )
}

pub(super) fn call_result_block_parameter_verified() -> VerifiedPsiOptimizationUnit {
    let caller = MachineId::new(1_601).unwrap();
    let callee = MachineId::new(1_602).unwrap();
    let caller_entry = BlockId::new(1_603).unwrap();
    let caller_result = ValueId::new(1_604).unwrap();
    let call_result = ValueId::new(1_605).unwrap();
    let forwarded = ValueId::new(1_606).unwrap();
    let caller_exit = BlockId::new(1_607).unwrap();
    let callee_entry = BlockId::new(1_611).unwrap();
    let callee_value = ValueId::new(1_612).unwrap();
    let callee_result = ValueId::new(1_613).unwrap();
    let boolean = |id| ValueDeclaration {
        id,
        scalar_type: ScalarType::Boolean,
    };
    let mut module = module_with_blocks(
        caller,
        caller_entry,
        TerminalMachineResult::Scalar(boolean(caller_result)),
        vec![
            Block {
                id: caller_entry,
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(1_608).unwrap(),
                    result: OperationResult::Scalar(boolean(call_result)),
                    kind: OperationKind::Call {
                        callee,
                        arguments: Vec::new(),
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                    },
                }],
                terminator: Terminator::Jump {
                    edge: EdgeId::new(1_609).unwrap(),
                    target: caller_exit,
                    arguments: vec![call_result],
                    trivial_affine_discards: Vec::new(),
                },
            },
            Block {
                id: caller_exit,
                parameters: vec![boolean(forwarded)],
                operations: Vec::new(),
                terminator: Terminator::Return {
                    edge: EdgeId::new(1_610).unwrap(),
                    value: forwarded,
                    cleanup_actions: Vec::new(),
                },
            },
        ],
    );
    module.machines.push(TerminalMachine {
        id: callee,
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(boolean(callee_result)),
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: callee_entry,
        blocks: vec![Block {
            id: callee_entry,
            parameters: Vec::new(),
            operations: vec![Operation {
                id: OperationId::new(1_614).unwrap(),
                result: OperationResult::Scalar(boolean(callee_value)),
                kind: OperationKind::BooleanConstant { value: true },
            }],
            terminator: Terminator::Return {
                edge: EdgeId::new(1_615).unwrap(),
                value: callee_value,
                cleanup_actions: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: ContractId::new(1_616).unwrap(),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    });
    verified(module, ProofBundle::default())
}
