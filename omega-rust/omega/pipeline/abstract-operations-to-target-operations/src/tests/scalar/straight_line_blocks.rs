use super::*;

#[test]
fn straight_line_arrivals_follow_edges_independently_of_block_storage_order() {
    for return_first in [false, true] {
        let mut plan = parameter_return_plan(1);
        let function = &mut plan.functions[0];
        let scalar_type = function.parameters[0].scalar_type;
        let incoming = function.parameters[0].value;
        let arrival = ValueId::new(20).unwrap();
        let destination = BlockId::new(20).unwrap();
        let jump_edge = EdgeId::new(20).unwrap();
        let entry_operation = OperationId::new(20).unwrap();
        let exit_operation = OperationId::new(21).unwrap();
        let entry_constant = AbstractOperation::BooleanConstant {
            psi_operation: entry_operation,
            result: ValueId::new(30).unwrap(),
            value: true,
        };
        let exit_constant = AbstractOperation::BooleanConstant {
            psi_operation: exit_operation,
            result: ValueId::new(31).unwrap(),
            value: false,
        };
        let mut returned = function.operations[0].clone();
        let AbstractOperation::Return { value, .. } = &mut returned else {
            unreachable!();
        };
        *value = arrival;
        let jump = AbstractOperation::Jump {
            psi_edge: jump_edge,
            target: destination,
            bindings: vec![abstract_operations::ValueBinding {
                parameter: arrival,
                argument: incoming,
                scalar_type,
            }],
            structural_bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        };
        let mut entry = abstract_operations::AbstractBlockEntry {
            block: function.entry,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operation_offset: 0,
        };
        let mut exit = abstract_operations::AbstractBlockEntry {
            block: destination,
            parameters: vec![AbstractParameter {
                value: arrival,
                scalar_type,
            }],
            structural_parameters: Vec::new(),
            operation_offset: 2,
        };
        if return_first {
            entry.operation_offset = 2;
            exit.operation_offset = 0;
            function.block_entries = vec![exit, entry];
            function.operations = vec![exit_constant, returned, entry_constant, jump];
        } else {
            function.block_entries = vec![entry, exit];
            function.operations = vec![entry_constant, jump, exit_constant, returned];
        }
        let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
        let graph = &lowered.functions[0].graph;
        let returned = graph
            .blocks
            .iter()
            .find(|block| block.block == destination)
            .unwrap();
        assert!(matches!(&returned.terminator,
            target_operations::TargetControlTerminator::ReturnScalar {
                source_value,
                expression: target_operations::TargetScalarExpression::Integer {
                    expression: TargetIntegerExpression::BlockParameter(parameter), ..
                }, ..
            } if *source_value == arrival && parameter.value == arrival && parameter.block == destination));
        // Custody rosters retain source storage order even though execution
        // must bind the entry's argument before reading the return block.
        let mut expected_operations = vec![entry_operation, exit_operation];
        let mut expected_edges = vec![jump_edge, EdgeId::new(10).unwrap()];
        if return_first {
            expected_operations.reverse();
            expected_edges.reverse();
        }
        assert_eq!(
            lowered.functions[0].provenance.operations,
            expected_operations
        );
        assert_eq!(lowered.functions[0].provenance.edges, expected_edges);
    }
}
