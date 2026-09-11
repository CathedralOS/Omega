//! Returning Unit branches join through ordinary source blocks.
use super::*;
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
    crate::lower_to_target_operations_with_provider_executions(
        plan,
        NativeTarget::linux_x64(),
        &[crate::AdmittedBoundarySettlement {
            boundary: plan.boundary_machines[0].id,
            execution: crate::AdmittedBoundaryExecution::CompilerBuiltin(
                target_operations::CompilerBuiltinExecution::HostedWriteByteI32,
            ),
            realization: target_operations::HostedWriteByteI32Realization.into(),
        }],
    )
}

#[test]
fn ordinary_unit_graph_retains_selected_edges_join_and_return() {
    let plan = fixture();
    let lowered = lower(&plan).unwrap();
    let caller = lowered
        .functions
        .iter()
        .find(|function| function.machine == plan.entry)
        .unwrap();
    let graph = &caller.graph;
    assert_eq!(graph.entry, block(1));
    assert_eq!(graph.blocks.len(), 4);
    let target_operations::TargetControlTerminator::Conditional {
        condition_source,
        when_true,
        when_false,
        ..
    } = &graph.blocks[0].terminator
    else {
        panic!("branch");
    };
    assert_eq!(*condition_source, value(2));
    assert_eq!((when_true.target, when_false.target), (block(2), block(3)));
    assert!(matches!(
        graph.blocks[3].terminator,
        target_operations::TargetControlTerminator::Return { .. }
    ));
    assert_eq!(
        caller.provenance.edges,
        vec![edge(1), edge(2), edge(3), edge(4), edge(5)]
    );
}

#[test]
fn abstract_unit_graph_lowering_preserves_canonical_entry_metadata() {
    // Abstract lowering compatibility only: optimization-unit validation still
    // requires empty entry block parameters before native admission.
    let mut plan = fixture();
    plan.functions[1].block_entries[0].parameters = plan.functions[1].parameters.clone();
    assert!(lower(&plan).is_ok());
}

#[test]
fn ordinary_unit_graph_retains_linear_call_continuations() {
    let mut plan = fixture();
    let caller = &mut plan.functions[1];
    caller.parameters[0].scalar_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    caller.parameters.truncate(1);
    caller.block_entries = [(1, 0), (4, 2)]
        .into_iter()
        .map(|(identity, operation_offset)| AbstractBlockEntry {
            structural_parameters: Vec::new(),
            block: block(identity),
            parameters: Vec::new(),
            operation_offset,
        })
        .collect();
    caller.operations = vec![
        call(11, value(1)),
        jump(3),
        call(15, value(1)),
        AbstractOperation::ReturnUnit {
            psi_edge: edge(5),
            cleanup_actions: Vec::new(),
        },
    ];
    let lowered = lower(&plan).unwrap();
    let caller = lowered
        .functions
        .iter()
        .find(|function| function.machine == plan.entry)
        .unwrap();
    let graph = &caller.graph;
    assert_eq!(graph.blocks.len(), 2);
    assert!(
        matches!(&graph.blocks[0].terminator, target_operations::TargetControlTerminator::Jump { successor } if successor.target == block(4))
    );
    assert_eq!(
        caller.provenance.operations,
        vec![operation(11), operation(15)]
    );
}

#[test]
fn ordinary_unit_graph_rejects_sibling_and_partial_join_definitions() {
    for call_position in [5, 8] {
        let mut plan = fixture();
        let AbstractOperation::CallUnit { arguments, .. } =
            &mut plan.functions[1].operations[call_position]
        else {
            panic!("call");
        };
        arguments[0] = value(10);
        assert!(lower(&plan).is_err());
    }
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
