use crate::tests::{exact_add_unit, refresh_node_derivatives};
use crate::validate_psi_optimization_unit;
use abstract_operations::{AbstractFunctionResult, AbstractOperation as Operation};
use semantic_vocabulary::{
    IeeeFloatComparisonOperation, IeeeFloatFormat, IeeeFloatValue, ScalarType,
};

fn comparison_unit() -> optimization_unit::PsiOptimizationUnit {
    let mut unit = exact_add_unit();
    unit.accepted_obligation_facts.clear();
    for node_index in 0..2 {
        let Operation::IntegerConstant {
            psi_operation,
            result,
            ..
        } = unit.functions[0].blocks[0].nodes[node_index].operation
        else {
            panic!("constant operand");
        };
        unit.functions[0].blocks[0].nodes[node_index].operation = Operation::IeeeFloatConstant {
            psi_operation,
            result,
            value: IeeeFloatValue::Binary32(if node_index == 0 {
                0x7fc0_0000
            } else {
                0x8000_0000
            }),
        };
    }
    let Operation::ExactIntegerAdd {
        psi_operation,
        result,
        left,
        right,
        ..
    } = unit.functions[0].blocks[0].nodes[2].operation
    else {
        panic!("binary fixture");
    };
    unit.functions[0].blocks[0].nodes[2].operation = Operation::IeeeFloatCompare {
        psi_operation,
        result,
        left,
        right,
        comparison: IeeeFloatComparisonOperation::Equal,
        format: IeeeFloatFormat::Binary32,
    };
    let AbstractFunctionResult::Scalar(result) = &mut unit.functions[0].result else {
        panic!("scalar result");
    };
    result.scalar_type = ScalarType::Boolean;
    let Operation::Return { scalar_type, .. } = &mut unit.functions[0].blocks[0].nodes[3].operation
    else {
        panic!("return");
    };
    *scalar_type = ScalarType::Boolean;
    for node in 0..4 {
        refresh_node_derivatives(&mut unit, 0, 0, node);
    }
    unit
}

#[test]
fn comparison_reconstructs_bool_result_and_ordered_float_operands() {
    let unit = comparison_unit();
    validate_psi_optimization_unit(&unit).expect("NaN and signed zero retain typed comparison");
    let node = &unit.functions[0].blocks[0].nodes[2];
    assert_eq!(node.definitions[0].scalar_type, ScalarType::Boolean);
    assert_eq!(node.uses.len(), 2);
    let Operation::IeeeFloatCompare { left, right, .. } = node.operation else {
        panic!("comparison");
    };
    assert_eq!(node.uses[0].value, left);
    assert_eq!(node.uses[1].value, right);
}

#[test]
fn comparison_rejects_self_consistent_wrong_format_and_nonfloat_operands() {
    for wrong_format in [false, true] {
        let mut unit = comparison_unit();
        if wrong_format {
            let Operation::IeeeFloatCompare { format, .. } =
                &mut unit.functions[0].blocks[0].nodes[2].operation
            else {
                panic!("comparison");
            };
            *format = IeeeFloatFormat::Binary64;
            refresh_node_derivatives(&mut unit, 0, 0, 2);
        } else {
            let Operation::IeeeFloatConstant {
                psi_operation,
                result,
                ..
            } = unit.functions[0].blocks[0].nodes[1].operation
            else {
                panic!("float operand");
            };
            unit.functions[0].blocks[0].nodes[1].operation = Operation::BooleanConstant {
                psi_operation,
                result,
                value: true,
            };
            refresh_node_derivatives(&mut unit, 0, 0, 1);
        }
        assert!(
            validate_psi_optimization_unit(&unit).is_err(),
            "wrong_format={wrong_format}"
        );
    }
}
