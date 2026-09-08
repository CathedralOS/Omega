//! Destination-owned block values and complete ordered edge transfers.
use super::*;
use abstract_operations::ValueBinding;

fn transferred() -> AbstractOperationPlan {
    let mut plan = fixture();
    let caller = &mut plan.functions[1];
    let byte = caller.parameters[0].scalar_type;
    let unsigned = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    caller.parameters.push(AbstractParameter {
        value: value(3),
        scalar_type: unsigned,
    });
    let parameters = vec![
        AbstractParameter {
            value: value(50),
            scalar_type: byte,
        },
        AbstractParameter {
            value: value(51),
            scalar_type: ScalarType::Boolean,
        },
        AbstractParameter {
            value: value(52),
            scalar_type: unsigned,
        },
    ];
    let bindings = parameters
        .iter()
        .zip([value(1), value(2), value(3)])
        .map(|(parameter, argument)| ValueBinding {
            parameter: parameter.value,
            argument,
            scalar_type: parameter.scalar_type,
        })
        .collect::<Vec<_>>();
    caller.block_entries = vec![
        AbstractBlockEntry {
            block: block(1),
            parameters: Vec::new(),
            operation_offset: 0,
        },
        AbstractBlockEntry {
            block: block(4),
            parameters,
            operation_offset: 1,
        },
        AbstractBlockEntry {
            block: block(5),
            parameters: Vec::new(),
            operation_offset: 4,
        },
        AbstractBlockEntry {
            block: block(6),
            parameters: Vec::new(),
            operation_offset: 5,
        },
    ];
    let mut when_true = successor(1, 4);
    when_true.bindings = bindings.clone();
    let mut when_false = successor(2, 4);
    when_false.bindings = bindings;
    caller.operations = vec![
        AbstractOperation::Conditional {
            condition: value(2),
            when_true,
            when_false,
        },
        AbstractOperation::IntegerWiden {
            psi_operation: operation(10),
            result: value(10),
            source_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            target_type: IntegerType::new(IntegerSign::Signed, 32).unwrap(),
            operand: value(50),
        },
        call(11, value(10)),
        AbstractOperation::Conditional {
            condition: value(51),
            when_true: successor(3, 5),
            when_false: successor(4, 6),
        },
        AbstractOperation::ReturnUnit {
            psi_edge: edge(5),
            cleanup_actions: Vec::new(),
        },
        AbstractOperation::ReturnUnit {
            psi_edge: edge(6),
            cleanup_actions: Vec::new(),
        },
    ];
    plan
}

#[test]
fn scalar_block_transfers_preserve_both_parallel_edges_and_destination_values() {
    let plan = transferred();
    let lowered = lower(&plan).unwrap();
    let TargetOperation::UnitGraph(graph) = &lowered.functions[1].operation else {
        panic!("graph");
    };
    let target_operations::TargetUnitTerminator::Conditional {
        when_true,
        when_false,
        ..
    } = &graph.blocks[0].terminator
    else {
        panic!("conditional");
    };
    assert_eq!(when_true.bindings.len(), 3);
    assert_eq!(when_false.bindings, when_true.bindings);
    let target_operations::TargetUnitTerminator::Conditional { condition, .. } =
        &graph.blocks[1].terminator
    else {
        panic!("conditional");
    };
    assert_eq!(
        *condition,
        target_operations::TargetBooleanExpression::BlockParameter(
            target_operations::TargetScalarBlockValue {
                block: block(4),
                value: value(51),
                scalar_type: ScalarType::Boolean,
            }
        )
    );
    let target_operations::TargetUnitOperation::ScalarDefinition {
        expression:
            target_operations::TargetScalarExpression::Integer {
                expression: target_operations::TargetIntegerExpression::IntegerWiden { operand, .. },
                ..
            },
        ..
    } = &graph.blocks[1].operations[0]
    else {
        panic!("widen");
    };
    assert!(
        matches!(operand.as_ref(), target_operations::TargetIntegerExpression::BlockParameter(parameter) if parameter.block == block(4) && parameter.value == value(50))
    );
}

#[test]
fn scalar_block_transfers_reject_wrong_arity_type_identity_and_unavailable_sources() {
    for mutation in 0..6 {
        let mut plan = transferred();
        let AbstractOperation::Conditional { when_false, .. } =
            &mut plan.functions[1].operations[0]
        else {
            panic!("conditional");
        };
        match mutation {
            0 => {
                when_false.bindings.pop();
            }
            1 => when_false.bindings[0].scalar_type = ScalarType::Boolean,
            2 => when_false.bindings[0].parameter = value(99),
            3 => when_false.bindings[0].argument = value(50),
            4 => when_false.bindings.swap(0, 1),
            _ => plan.functions[1].block_entries[1].parameters[0].value = value(1),
        }
        assert!(lower(&plan).is_err(), "mutation {mutation}");
    }
}
