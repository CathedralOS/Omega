//! A machine whose boolean results are compile-time decidable: selection
//! emits `CompareI64` over uniquely materialized literals followed by the
//! flag-reading `MaterializeBoolean*` — the exact surface
//! `SelectedConstantBooleanFoldV1` folds in the pre-allocation slice. A
//! second machine compares two parameters, so its flag reader declines.

use crate::tests::{
    AdmissionProfile, Block, BlockId, ContractId, EdgeId, ExplicitOptimizationRequest,
    MachineContract, MachineId, NativeTarget, Operation, OperationId, OperationKind,
    OperationResult, OptimizationSelections, OptimizationWorkBudget,
    OptimizedTargetLoweringRequest, StagedOptimizedSelectedInstructions, TerminalMachine,
    TerminalMachineResult, Terminator, ValueId, conditional_immediate_module,
    encode_fixture_sections, lower_optimized_to_target_operations, operation_proof_bundle,
    stage_optimized_instruction_selection,
};
use terminal_psi::ValueDeclaration;

pub(crate) const CONSTANT_BOOLEAN_MACHINE: u64 = 27_101;
pub(crate) const CONSTANT_BOOLEAN_ENTRY: u64 = 27_102;
pub(crate) const CONSTANT_BOOLEAN_CONTRACT: u64 = 27_103;
pub(crate) const CONSTANT_BOOLEAN_RETURN_EDGE: u64 = 27_104;
pub(crate) const PARAMETER_BOOLEAN_MACHINE: u64 = 27_201;
pub(crate) const PARAMETER_BOOLEAN_ENTRY: u64 = 27_202;
pub(crate) const PARAMETER_BOOLEAN_CONTRACT: u64 = 27_203;
pub(crate) const PARAMETER_BOOLEAN_RETURN_EDGE: u64 = 27_204;

fn u64_scalar() -> crate::tests::ScalarType {
    crate::tests::ScalarType::Integer(
        crate::tests::IntegerType::new(crate::tests::IntegerSign::Unsigned, 64).unwrap(),
    )
}

fn boolean_scalar() -> crate::tests::ScalarType {
    crate::tests::ScalarType::Boolean
}

fn declaration(id: ValueId, scalar_type: crate::tests::ScalarType) -> ValueDeclaration {
    ValueDeclaration {
        qualifications: Default::default(),
        id,
        scalar_type,
    }
}

fn scalar_operation(
    id: u64,
    result: ValueId,
    scalar_type: crate::tests::ScalarType,
    kind: OperationKind,
) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(id).unwrap(),
        result: OperationResult::Scalar(declaration(result, scalar_type)),
        kind,
    }
}

/// `left = 3`, `right = 5` feed `equal = left == right` and
/// `less_than = left < right`, so both flag readers fold to
/// `MaterializeI64`; their results then compare `0` and `1` inside
/// `both = equal == less_than`, whose own reader folds on the next sweep.
/// The parameter compare `equal_parameter` stays inadmissible, and
/// `result = both == equal_parameter` declines because one operand's
/// producer remains a `MaterializeBoolean*`, so the terminal sweep still
/// sees real candidates to decline.
pub(crate) fn constant_boolean_artifact() -> (Vec<u8>, Vec<u8>) {
    let machine = MachineId::new(CONSTANT_BOOLEAN_MACHINE).unwrap();
    let entry = BlockId::new(CONSTANT_BOOLEAN_ENTRY).unwrap();
    let parameter = ValueId::new(CONSTANT_BOOLEAN_MACHINE + 10).unwrap();
    let left = ValueId::new(CONSTANT_BOOLEAN_MACHINE + 11).unwrap();
    let right = ValueId::new(CONSTANT_BOOLEAN_MACHINE + 12).unwrap();
    let equal_constants = ValueId::new(CONSTANT_BOOLEAN_MACHINE + 13).unwrap();
    let less_than_constants = ValueId::new(CONSTANT_BOOLEAN_MACHINE + 14).unwrap();
    let equal_parameter = ValueId::new(CONSTANT_BOOLEAN_MACHINE + 15).unwrap();
    let both = ValueId::new(CONSTANT_BOOLEAN_MACHINE + 16).unwrap();
    let result = ValueId::new(CONSTANT_BOOLEAN_MACHINE + 17).unwrap();
    let output = ValueId::new(CONSTANT_BOOLEAN_MACHINE + 18).unwrap();
    let machine = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: machine,
        attachment: None,
        parameters: vec![declaration(parameter, u64_scalar())],
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(declaration(output, boolean_scalar())),
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry,
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: entry,
            parameters: Vec::new(),
            operations: vec![
                scalar_operation(
                    CONSTANT_BOOLEAN_MACHINE + 20,
                    left,
                    u64_scalar(),
                    OperationKind::IntegerConstant {
                        value: crate::tests::IntegerValue::Unsigned(3),
                    },
                ),
                scalar_operation(
                    CONSTANT_BOOLEAN_MACHINE + 21,
                    right,
                    u64_scalar(),
                    OperationKind::IntegerConstant {
                        value: crate::tests::IntegerValue::Unsigned(5),
                    },
                ),
                scalar_operation(
                    CONSTANT_BOOLEAN_MACHINE + 22,
                    equal_constants,
                    boolean_scalar(),
                    OperationKind::IntegerEqual { left, right },
                ),
                scalar_operation(
                    CONSTANT_BOOLEAN_MACHINE + 23,
                    less_than_constants,
                    boolean_scalar(),
                    OperationKind::IntegerLessThan { left, right },
                ),
                scalar_operation(
                    CONSTANT_BOOLEAN_MACHINE + 24,
                    equal_parameter,
                    boolean_scalar(),
                    OperationKind::IntegerEqual {
                        left: parameter,
                        right: left,
                    },
                ),
                scalar_operation(
                    CONSTANT_BOOLEAN_MACHINE + 25,
                    both,
                    boolean_scalar(),
                    OperationKind::BooleanEqual {
                        left: equal_constants,
                        right: less_than_constants,
                    },
                ),
                scalar_operation(
                    CONSTANT_BOOLEAN_MACHINE + 26,
                    result,
                    boolean_scalar(),
                    OperationKind::BooleanEqual {
                        left: both,
                        right: equal_parameter,
                    },
                ),
            ],
            terminator: Terminator::Return {
                edge: EdgeId::new(CONSTANT_BOOLEAN_RETURN_EDGE).unwrap(),
                value: result,
                cleanup_actions: Vec::new(),
            },
        }],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            id: ContractId::new(CONSTANT_BOOLEAN_CONTRACT).unwrap(),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    let module = conditional_immediate_module(machine.id, vec![machine]);
    let proof = operation_proof_bundle(&module);
    encode_fixture_sections(&module, &proof)
}

/// `left == right` over two parameters publishes a flag state no literal
/// proves, so the plan's `MaterializeBooleanEqual` declines the fold.
pub(crate) fn parameter_boolean_artifact() -> (Vec<u8>, Vec<u8>) {
    let machine = MachineId::new(PARAMETER_BOOLEAN_MACHINE).unwrap();
    let entry = BlockId::new(PARAMETER_BOOLEAN_ENTRY).unwrap();
    let left = ValueId::new(PARAMETER_BOOLEAN_MACHINE + 10).unwrap();
    let right = ValueId::new(PARAMETER_BOOLEAN_MACHINE + 11).unwrap();
    let result = ValueId::new(PARAMETER_BOOLEAN_MACHINE + 12).unwrap();
    let output = ValueId::new(PARAMETER_BOOLEAN_MACHINE + 13).unwrap();
    let machine = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: machine,
        attachment: None,
        parameters: vec![
            declaration(left, u64_scalar()),
            declaration(right, u64_scalar()),
        ],
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(declaration(output, boolean_scalar())),
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry,
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: entry,
            parameters: Vec::new(),
            operations: vec![scalar_operation(
                PARAMETER_BOOLEAN_MACHINE + 20,
                result,
                boolean_scalar(),
                OperationKind::IntegerEqual { left, right },
            )],
            terminator: Terminator::Return {
                edge: EdgeId::new(PARAMETER_BOOLEAN_RETURN_EDGE).unwrap(),
                value: result,
                cleanup_actions: Vec::new(),
            },
        }],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            id: ContractId::new(PARAMETER_BOOLEAN_CONTRACT).unwrap(),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    let module = conditional_immediate_module(machine.id, vec![machine]);
    let proof = operation_proof_bundle(&module);
    encode_fixture_sections(&module, &proof)
}

fn staged_with(
    artifact: (Vec<u8>, Vec<u8>),
    target: NativeTarget,
    selections: OptimizationSelections,
    budget: OptimizationWorkBudget,
) -> StagedOptimizedSelectedInstructions {
    let (semantic, proof) = artifact;
    let optimized = crate::tests::optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections, budget).unwrap(),
    )
    .unwrap();
    let target = lower_optimized_to_target_operations(
        optimized,
        OptimizedTargetLoweringRequest::new(target),
    )
    .unwrap();
    stage_optimized_instruction_selection(target).unwrap()
}

/// The constant-compare artifact under an authored suite, lowered and
/// selected for `target` with `budget` as the per-pass work budget.
pub(crate) fn staged_constant_boolean_with_selections(
    target: NativeTarget,
    selections: OptimizationSelections,
    budget: OptimizationWorkBudget,
) -> StagedOptimizedSelectedInstructions {
    staged_with(constant_boolean_artifact(), target, selections, budget)
}

/// The parameter-compare artifact under an authored suite, lowered and
/// selected for `target` with `budget` as the per-pass work budget.
pub(crate) fn staged_parameter_boolean_with_selections(
    target: NativeTarget,
    selections: OptimizationSelections,
    budget: OptimizationWorkBudget,
) -> StagedOptimizedSelectedInstructions {
    staged_with(parameter_boolean_artifact(), target, selections, budget)
}
