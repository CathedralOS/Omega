use crate::tests::fixtures::scalar_units::exact_add_unit;
use crate::tests::support::{refresh_identity, refresh_node_derivatives};
use crate::validate_psi_optimization_unit;
use abstract_operations::{AbstractFunctionResult, AbstractOperation as Operation};
use semantic_vocabulary::{IeeeFloatFormat, IeeeFloatValue, ScalarType};
use terminal_psi::{FloatResultPolicy, FloatSaturationRule};

fn policy_unit(format: IeeeFloatFormat, trapping: bool) -> optimization_unit::PsiOptimizationUnit {
    let mut unit = exact_add_unit();
    unit.accepted_obligation_facts.clear();
    for node in unit.functions[0].blocks[0].nodes.iter_mut().take(2) {
        let Operation::IntegerConstant {
            psi_operation,
            result,
            ..
        } = node.operation
        else {
            panic!("constant operand");
        };
        node.operation = Operation::IeeeFloatConstant {
            psi_operation,
            result,
            value: match format {
                IeeeFloatFormat::Binary32 => IeeeFloatValue::Binary32(0),
                IeeeFloatFormat::Binary64 => IeeeFloatValue::Binary64(0),
            },
        };
    }
    let Operation::IntegerArithmetic {
        psi_operation,
        result,
        left,
        right,
        ..
    } = unit.functions[0].blocks[0].nodes[2].operation
    else {
        panic!("binary fixture")
    };
    unit.functions[0].blocks[0].nodes[2].operation = Operation::FloatResultPolicy {
        psi_operation,
        result,
        format,
        policy: if trapping {
            FloatResultPolicy::Trapping { raw_result: left }
        } else {
            FloatResultPolicy::Saturating {
                left,
                right,
                raw_result: left,
                rule: FloatSaturationRule::FiniteDivisionOverflow,
            }
        },
    };
    let AbstractFunctionResult::Scalar(result) = &mut unit.functions[0].result else {
        panic!("scalar result");
    };
    result.scalar_type = ScalarType::IeeeFloat(format);
    let Operation::Return { scalar_type, .. } = &mut unit.functions[0].blocks[0].nodes[3].operation
    else {
        panic!("return")
    };
    *scalar_type = ScalarType::IeeeFloat(format);
    for node in 0..4 {
        refresh_node_derivatives(&mut unit, 0, 0, node);
    }
    unit
}

#[test]
fn float_result_policy_replay_checks_every_operand_and_derived_use() {
    for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
        for trapping in [false, true] {
            let valid = policy_unit(format, trapping);
            validate_psi_optimization_unit(&valid).expect("valid policy unit");
            let mut stale_uses = valid.clone();
            stale_uses.functions[0].blocks[0].nodes[2].uses.clear();
            refresh_identity(&mut stale_uses);
            assert!(validate_psi_optimization_unit(&stale_uses).is_err());
            for operand_index in 0..if trapping { 1 } else { 2 } {
                let mut malformed = valid.clone();
                let Operation::IeeeFloatConstant {
                    psi_operation,
                    result,
                    ..
                } = malformed.functions[0].blocks[0].nodes[operand_index].operation
                else {
                    panic!("constant")
                };
                malformed.functions[0].blocks[0].nodes[operand_index].operation =
                    Operation::BooleanConstant {
                        psi_operation,
                        result,
                        value: false,
                    };
                refresh_node_derivatives(&mut malformed, 0, 0, operand_index);
                assert!(validate_psi_optimization_unit(&malformed).is_err());
            }
            let mut wrong_format = valid;
            let Operation::FloatResultPolicy {
                format: policy_format,
                ..
            } = &mut wrong_format.functions[0].blocks[0].nodes[2].operation
            else {
                panic!("policy")
            };
            *policy_format = match format {
                IeeeFloatFormat::Binary32 => IeeeFloatFormat::Binary64,
                IeeeFloatFormat::Binary64 => IeeeFloatFormat::Binary32,
            };
            refresh_node_derivatives(&mut wrong_format, 0, 0, 2);
            assert!(validate_psi_optimization_unit(&wrong_format).is_err());
        }
    }
}
