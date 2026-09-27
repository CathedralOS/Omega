//! Returning Unit branches join through ordinary source blocks.
use super::super::{
    AbstractBlockEntry, AbstractOperation, AbstractOperationPlan, AbstractParameter, BlockId,
    EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, NativeTarget, OperationId,
    ScalarType, ValueId,
};

mod cycles;
mod scalar_arrays;
mod structural_cases;
mod transfers;
use abstract_operations::AbstractSuccessor;

fn block(value: u64) -> BlockId {
    BlockId::new(value).unwrap()
}
fn edge(value: u64) -> EdgeId {
    EdgeId::new(value).unwrap()
}
fn value(value: u64) -> ValueId {
    ValueId::new(value).unwrap()
}
fn operation(value: u64) -> OperationId {
    OperationId::new(value).unwrap()
}
fn successor(identity: u64, target: u64) -> AbstractSuccessor {
    AbstractSuccessor {
        structural_bindings: Vec::new(),
        psi_edge: edge(identity),
        target: block(target),
        bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
    }
}
fn call(identity: u64, argument: ValueId) -> AbstractOperation {
    AbstractOperation::CallUnit {
        psi_operation: operation(identity),
        callee: MachineId::new(901).unwrap(),
        arguments: vec![argument],
        structural_arguments: Vec::new(),
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    }
}
fn jump(identity: u64) -> AbstractOperation {
    AbstractOperation::Jump {
        structural_bindings: Vec::new(),
        psi_edge: edge(identity),
        target: block(4),
        bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    }
}
fn fixture() -> AbstractOperationPlan {
    let mut plan = super::returning_byte_parameter::fixture();
    let mut caller = plan.functions[0].clone();
    caller.machine = MachineId::new(100).unwrap();
    caller.entry = block(1);
    let byte_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let integer_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    caller.parameters = vec![
        AbstractParameter {
            value: value(1),
            scalar_type: ScalarType::Integer(byte_type),
        },
        AbstractParameter {
            value: value(2),
            scalar_type: ScalarType::Boolean,
        },
    ];
    caller.block_entries = [(1, 0), (2, 1), (3, 4), (4, 7)]
        .into_iter()
        .map(|(identity, operation_offset)| AbstractBlockEntry {
            structural_parameters: Vec::new(),
            block: block(identity),
            parameters: Vec::new(),
            operation_offset,
        })
        .collect();
    caller.operations = vec![
        AbstractOperation::Conditional {
            condition: value(2),
            when_true: successor(1, 2),
            when_false: successor(2, 3),
        },
        AbstractOperation::IntegerWiden {
            psi_operation: operation(10),
            result: value(10),
            source_type: byte_type,
            target_type: integer_type,
            operand: value(1),
        },
        call(11, value(10)),
        jump(3),
        AbstractOperation::IntegerConstant {
            psi_operation: operation(12),
            result: value(12),
            scalar_type: ScalarType::Integer(integer_type),
            value: IntegerValue::Signed(63),
        },
        call(13, value(12)),
        jump(4),
        AbstractOperation::IntegerConstant {
            psi_operation: operation(14),
            result: value(14),
            scalar_type: ScalarType::Integer(integer_type),
            value: IntegerValue::Signed(33),
        },
        call(15, value(14)),
        AbstractOperation::ReturnUnit {
            psi_edge: edge(5),
            cleanup_actions: Vec::new(),
        },
    ];
    plan.entry = caller.machine;
    plan.functions.push(caller);
    plan
}
fn lower(
    plan: &AbstractOperationPlan,
) -> Result<target_operations::TargetOperationPlan, crate::LoweringError> {
    crate::lower_to_target_operations(
        plan,
        crate::TargetLoweringRequest {
            target: NativeTarget::linux_x64(),
            settlements: &[crate::AdmittedBoundarySettlement {
                boundary: plan.boundary_machines[0].id,
                execution: crate::AdmittedBoundaryExecution::CompilerBuiltin(
                    target_operations::CompilerBuiltinExecution::HostedWriteByteI32,
                ),
                realization: target_operations::HostedWriteByteI32Realization.into(),
            }],
            installation: None,
            native_callbacks: &[],
        },
    )
}

#[test]
fn ordinary_unit_graph_rejects_cycles_unknown_targets_and_parameter_transfers() {
    for mutation in 0..3 {
        let mut plan = fixture();
        match mutation {
            0 | 1 => {
                let AbstractOperation::Jump { target, .. } = &mut plan.functions[1].operations[3]
                else {
                    panic!("jump");
                };
                *target = block(if mutation == 0 { 1 } else { 99 });
            }
            _ => plan.functions[1].block_entries[3]
                .parameters
                .push(AbstractParameter {
                    value: value(99),
                    scalar_type: ScalarType::Boolean,
                }),
        }
        assert!(lower(&plan).is_err());
    }
}
