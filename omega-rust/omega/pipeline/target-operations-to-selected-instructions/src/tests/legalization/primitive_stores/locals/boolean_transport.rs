//! Scalar-result graphs preserve Boolean reads through ordinary edge bindings.
use super::*;
use abstract_operations::{
    AbstractBlockEntry, AbstractFunctionResult, AbstractSuccessor, ValueBinding,
};
use semantic_vocabulary::{BlockId, EdgeId};
use target_operations::{TargetBooleanExpression, TargetControlTerminator};

#[test]
fn scalar_result_boolean_transport_rejects_binding_and_predicate_substitution() {
    let mut source = fixture(ScalarType::Boolean);
    let function = &mut source.functions[0];
    let scalar = integer(IntegerSign::Unsigned, 64);
    let result = ValueId::new(99).unwrap();
    function.result = AbstractFunctionResult::Scalar(AbstractResult {
        value: result,
        scalar_type: scalar,
    });
    function.operations.pop().unwrap();
    function.operations.push(AbstractOperation::Jump {
        psi_edge: EdgeId::new(1).unwrap(),
        target: BlockId::new(2).unwrap(),
        bindings: vec![ValueBinding {
            parameter: ValueId::new(3).unwrap(),
            argument: ValueId::new(2).unwrap(),
            scalar_type: ScalarType::Boolean,
        }],
        structural_bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    });
    function.block_entries.push(AbstractBlockEntry {
        block: BlockId::new(2).unwrap(),
        operation_offset: function.operations.len(),
        parameters: vec![AbstractParameter {
            value: ValueId::new(3).unwrap(),
            scalar_type: ScalarType::Boolean,
        }],
        structural_parameters: Vec::new(),
    });
    let successor = |ordinal| AbstractSuccessor {
        psi_edge: EdgeId::new(ordinal - 1).unwrap(),
        target: BlockId::new(ordinal).unwrap(),
        bindings: Vec::new(),
        structural_bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    function.operations.push(AbstractOperation::Conditional {
        condition: ValueId::new(3).unwrap(),
        when_true: successor(3),
        when_false: successor(4),
    });
    for ordinal in [3, 4] {
        function.block_entries.push(AbstractBlockEntry {
            block: BlockId::new(ordinal).unwrap(),
            operation_offset: function.operations.len(),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
        });
        let returned = ValueId::new(ordinal + 1).unwrap();
        function.operations.extend([
            AbstractOperation::IntegerConstant {
                psi_operation: OperationId::new(ordinal + 2).unwrap(),
                result: returned,
                scalar_type: scalar,
                value: IntegerValue::Unsigned(u128::from(4 - ordinal)),
            },
            AbstractOperation::Return {
                psi_edge: EdgeId::new(ordinal + 1).unwrap(),
                result,
                value: returned,
                scalar_type: scalar,
                cleanup_actions: Vec::new(),
            },
        ]);
    }
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let target =
            abstract_operations_to_target_operations::lower_to_target_operations(&source, native)
                .unwrap();
        let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
            &source,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
        for mutation in 0..5 {
            let mut changed = target.clone();
            let graph = &mut changed.functions[0].graph;
            match mutation {
                0 => {
                    let TargetControlTerminator::Jump { successor } =
                        &mut graph.blocks[0].terminator
                    else {
                        panic!("read transfer");
                    };
                    successor.bindings[0].argument = ValueId::new(1).unwrap();
                }
                1 => graph.blocks[1].parameters[0].scalar_type = integer(IntegerSign::Unsigned, 8),
                _ => {
                    let TargetControlTerminator::Conditional {
                        condition_source,
                        condition: TargetBooleanExpression::BlockParameter(parameter),
                        ..
                    } = &mut graph.blocks[1].terminator
                    else {
                        panic!("transported Boolean predicate");
                    };
                    match mutation {
                        2 => *condition_source = ValueId::new(2).unwrap(),
                        3 => parameter.block = BlockId::new(3).unwrap(),
                        _ => parameter.value = ValueId::new(2).unwrap(),
                    }
                }
            }
            assert!(legalize_target_operations(&changed, &source, &unit).is_err());
            assert!(
                validate_legalized_operations(&changed, &source, &unit, legalized.plan().clone())
                    .is_err(),
                "Boolean transfer mutation {mutation}"
            );
        }
    }
}
