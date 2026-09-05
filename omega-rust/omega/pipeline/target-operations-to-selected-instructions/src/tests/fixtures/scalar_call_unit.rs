//! Exact attached-Unit U64 equality-call fork/join source.

use abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, AbstractResult, AbstractSuccessor,
};
use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
    OperationId, ScalarType, StructuralTypeId, ValueId,
};
use target_operations::TargetOperationPlan;
use terminal_psi::{
    SemanticFingerprint, StructuralTypeDeclaration, StructuralTypeShape, TerminalPsiIdentity,
    VocabularyMarker,
};

pub(in crate::tests) fn scalar_call_unit_fixture() -> (
    AbstractOperationPlan,
    TargetOperationPlan,
    PsiOptimizationUnit,
) {
    let caller = MachineId::new(101).unwrap();
    let callee = MachineId::new(102).unwrap();
    let attachment = StructuralTypeId::new(103).unwrap();
    let caller_block = BlockId::new(104).unwrap();
    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar_type = ScalarType::Integer(u64_type);
    let left = ValueId::new(105).unwrap();
    let right = ValueId::new(106).unwrap();
    let r1 = ValueId::new(107).unwrap();
    let r2 = ValueId::new(108).unwrap();
    let r3 = ValueId::new(109).unwrap();
    let caller_operations = vec![
        AbstractOperation::IntegerConstant {
            psi_operation: OperationId::new(110).unwrap(),
            result: left,
            scalar_type,
            value: IntegerValue::Unsigned(7),
        },
        AbstractOperation::IntegerConstant {
            psi_operation: OperationId::new(111).unwrap(),
            result: right,
            scalar_type,
            value: IntegerValue::Unsigned(9),
        },
        scalar_call(112, r1, callee, [left, right], scalar_type),
        scalar_call(113, r2, callee, [left, right], scalar_type),
        scalar_call(114, r3, callee, [r1, r2], scalar_type),
        AbstractOperation::ReturnUnit {
            psi_edge: EdgeId::new(115).unwrap(),
            cleanup_actions: Vec::new(),
        },
    ];
    let callee_entry = BlockId::new(120).unwrap();
    let true_block = BlockId::new(121).unwrap();
    let false_block = BlockId::new(122).unwrap();
    let p0 = ValueId::new(123).unwrap();
    let p1 = ValueId::new(124).unwrap();
    let condition = ValueId::new(125).unwrap();
    let true_value = ValueId::new(126).unwrap();
    let false_value = ValueId::new(127).unwrap();
    let result = ValueId::new(128).unwrap();
    let edge = |raw, target| AbstractSuccessor {
        psi_edge: EdgeId::new(raw).unwrap(),
        target,
        bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let callee_operations = vec![
        AbstractOperation::IntegerEqual {
            psi_operation: OperationId::new(129).unwrap(),
            result: condition,
            left: p0,
            right: p1,
        },
        AbstractOperation::Conditional {
            condition,
            when_true: edge(130, true_block),
            when_false: edge(131, false_block),
        },
        AbstractOperation::IntegerConstant {
            psi_operation: OperationId::new(132).unwrap(),
            result: true_value,
            scalar_type,
            value: IntegerValue::Unsigned(1),
        },
        AbstractOperation::Return {
            psi_edge: EdgeId::new(133).unwrap(),
            result,
            value: true_value,
            scalar_type,
            cleanup_actions: Vec::new(),
        },
        AbstractOperation::IntegerConstant {
            psi_operation: OperationId::new(134).unwrap(),
            result: false_value,
            scalar_type,
            value: IntegerValue::Unsigned(0),
        },
        AbstractOperation::Return {
            psi_edge: EdgeId::new(135).unwrap(),
            result,
            value: false_value,
            scalar_type,
            cleanup_actions: Vec::new(),
        },
    ];
    let abstract_plan = AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x71; 32]),
        },
        entry: caller,
        structural_types: vec![StructuralTypeDeclaration {
            id: attachment,
            identity: "test::ScalarCallUnitAttachment".into(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        }],
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![
            AbstractFunction {
                machine: caller,
                attachment: Some(attachment),
                entry: caller_block,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block: caller_block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: caller_operations,
            },
            AbstractFunction {
                machine: callee,
                attachment: None,
                entry: callee_entry,
                parameters: vec![
                    AbstractParameter {
                        value: p0,
                        scalar_type,
                    },
                    AbstractParameter {
                        value: p1,
                        scalar_type,
                    },
                ],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: result,
                    scalar_type,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![
                    AbstractBlockEntry {
                        block: callee_entry,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    },
                    AbstractBlockEntry {
                        block: true_block,
                        parameters: Vec::new(),
                        operation_offset: 2,
                    },
                    AbstractBlockEntry {
                        block: false_block,
                        parameters: Vec::new(),
                        operation_offset: 4,
                    },
                ],
                operations: callee_operations,
            },
        ],
    };
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        &abstract_plan,
        target::NativeTarget::linux_x64(),
    )
    .unwrap();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &abstract_plan,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    (abstract_plan, target, unit)
}

fn scalar_call(
    raw: u64,
    result: ValueId,
    callee: MachineId,
    arguments: [ValueId; 2],
    scalar_type: ScalarType,
) -> AbstractOperation {
    AbstractOperation::Call {
        psi_operation: OperationId::new(raw).unwrap(),
        result,
        scalar_type,
        callee,
        arguments: arguments.to_vec(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    }
}
