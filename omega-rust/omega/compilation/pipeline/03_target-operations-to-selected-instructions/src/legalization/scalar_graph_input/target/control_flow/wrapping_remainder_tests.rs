//! Correspondence controls do not supply the nonzero-divisor proof required by legalization.
use crate::LegalizationError;
use abstract_operations::{
    AbstractBlockEntry, AbstractParameter, AbstractResult, IntegerArithmetic,
    IntegerArithmeticOperator, IntegerOverflowPolicy,
};

use super::super::{BlockId, ValueId};
use super::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractOperationPlan,
    PsiOptimizationUnit, ScalarType, TargetOperationPlan, TargetScalarExpression,
    TargetUnitOperation,
};
use crate::legalization::scalar_graph_input::supports_wrapping_division;
use crate::legalization::scalar_graph_input::target::Expression;
use semantic_vocabulary::{
    EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, ObligationId, OperationId,
};
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

fn value(ordinal: u64) -> ValueId {
    ValueId::new(ordinal).unwrap()
}

fn fixture(
    integer: IntegerType,
) -> (
    AbstractOperationPlan,
    TargetOperationPlan,
    PsiOptimizationUnit,
) {
    let scalar_type = ScalarType::Integer(integer);
    let machine = MachineId::new(1).unwrap();
    let entry = BlockId::new(1).unwrap();
    let body = BlockId::new(2).unwrap();
    let function = AbstractFunction {
        machine,
        attachment: None,
        entry,
        parameters: [value(1), value(2)]
            .map(|value| AbstractParameter { value, scalar_type })
            .to_vec(),
        structural_parameters: Vec::new(),
        result: AbstractFunctionResult::Scalar(AbstractResult {
            value: value(5),
            scalar_type,
        }),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        block_entries: [(entry, 0), (body, 1)]
            .map(|(block, operation_offset)| AbstractBlockEntry {
                block,
                operation_offset,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
            })
            .to_vec(),
        operations: vec![
            AbstractOperation::Jump {
                psi_edge: EdgeId::new(1).unwrap(),
                target: body,
                bindings: Vec::new(),
                structural_bindings: Vec::new(),
                trivial_affine_discards: Vec::new(),
                residual_affine_discards: Vec::new(),
            },
            AbstractOperation::IntegerArithmetic {
                psi_operation: OperationId::new(1).unwrap(),
                result: value(3),
                scalar_type: integer,
                arithmetic: IntegerArithmetic::new(
                    IntegerOverflowPolicy::Wrapping,
                    IntegerArithmeticOperator::Remainder,
                    Some(ObligationId::new(1).unwrap()),
                )
                .expect("admitted integer arithmetic"),
                left: value(1),
                right: value(2),
            },
            AbstractOperation::IntegerArithmetic {
                psi_operation: OperationId::new(2).unwrap(),
                result: value(4),
                scalar_type: integer,
                arithmetic: IntegerArithmetic::new(
                    IntegerOverflowPolicy::Wrapping,
                    IntegerArithmeticOperator::Remainder,
                    Some(ObligationId::new(2).unwrap()),
                )
                .expect("admitted integer arithmetic"),
                left: value(3),
                right: value(2),
            },
            AbstractOperation::Return {
                psi_edge: EdgeId::new(2).unwrap(),
                result: value(5),
                value: value(4),
                scalar_type,
                cleanup_actions: Vec::new(),
            },
        ],
    };
    let plan = AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([107; 32]),
        },
        entry: machine,
        structural_types: Vec::new().into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![function],
    };
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        &plan,
        abstract_operations_to_target_operations::TargetLoweringRequest::new(
            ::target::NativeTarget::macos_arm64(),
        ),
    )
    .unwrap();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    (plan, target, unit)
}

fn check(
    plan: &AbstractOperationPlan,
    target: &TargetOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    super::super::validate_target(
        &target.functions[0],
        &plan.functions[0],
        &unit.functions[0],
        target,
        plan,
        unit,
    )
    .map(|_| ())
}

#[test]
fn signed_wrapping_remainder_preserves_width_and_prior_result_homes() {
    for bits in [8, 16, 32, 64] {
        let (plan, target, unit) = fixture(IntegerType::new(IntegerSign::Signed, bits).unwrap());
        check(&plan, &target, &unit).unwrap();
        let TargetUnitOperation::ScalarDefinition {
            expression:
                TargetScalarExpression::Integer {
                    expression:
                        Expression::Arithmetic {
                            arithmetic, left, ..
                        },
                    ..
                },
            ..
        } = &target.functions[0].graph.blocks[1].operations[1]
        else {
            panic!("remainder");
        };
        assert!(arithmetic.is(
            IntegerOverflowPolicy::Wrapping,
            IntegerArithmeticOperator::Remainder
        ));
        assert!(
            matches!(left.as_ref(), Expression::ScalarHome(home) if home.source_value == value(3))
        );
        assert!(
            super::super::super::match_input(
                &target.functions[0],
                &plan.functions[0],
                &unit.functions[0],
                &target,
                &plan,
                &unit
            )
            .is_err(),
            "correspondence alone must not invent nonzero-divisor evidence"
        );
    }
}

#[test]
fn wrapping_remainder_replay_rejects_policy_evidence_and_snapshot_drift() {
    let (plan, target, unit) = fixture(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    for mutation in [
        "operation",
        "obligation",
        "operand",
        "width",
        "home",
        "policy",
        "reevaluation",
    ] {
        let mut changed = target.clone();
        let first_expression = match &changed.functions[0].graph.blocks[1].operations[0] {
            TargetUnitOperation::ScalarDefinition {
                expression: TargetScalarExpression::Integer { expression, .. },
                ..
            } => expression.clone(),
            _ => panic!("first remainder"),
        };
        let TargetUnitOperation::ScalarDefinition {
            result_home,
            expression:
                TargetScalarExpression::Integer {
                    scalar_type,
                    expression,
                },
        } = &mut changed.functions[0].graph.blocks[1].operations[1]
        else {
            panic!("remainder");
        };
        let Expression::Arithmetic {
            psi_operation,
            arithmetic,
            left,
            right,
        } = expression
        else {
            panic!("wrapping policy");
        };
        assert!(arithmetic.is(
            IntegerOverflowPolicy::Wrapping,
            IntegerArithmeticOperator::Remainder
        ));
        match mutation {
            "operation" => *psi_operation = OperationId::new(99).unwrap(),
            "obligation" => {
                *arithmetic = IntegerArithmetic::new(
                    arithmetic.policy(),
                    arithmetic.operator(),
                    Some(ObligationId::new(99).unwrap()),
                )
                .expect("admitted integer arithmetic")
            }
            "operand" => std::mem::swap(left, right),
            "width" => *scalar_type = IntegerType::new(IntegerSign::Signed, 64).unwrap(),
            "home" => result_home.source_value = value(3),
            "reevaluation" => **left = first_expression,
            _ => {
                *expression = Expression::Arithmetic {
                    psi_operation: *psi_operation,
                    arithmetic: IntegerArithmetic::new(
                        IntegerOverflowPolicy::Exact,
                        IntegerArithmeticOperator::Remainder,
                        arithmetic.obligation(),
                    )
                    .expect("admitted integer arithmetic"),
                    left: left.clone(),
                    right: right.clone(),
                }
            }
        }
        assert!(
            check(&plan, &changed, &unit).is_err(),
            "accepted {mutation}"
        );
    }
}

#[test]
fn wrapping_division_admits_every_native_width_and_rejects_the_rest() {
    for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
        for bits in [8, 16, 32, 64] {
            assert!(supports_wrapping_division(
                IntegerType::new(sign, bits).unwrap()
            ));
        }
        for bits in [1, 7, 24, 128] {
            assert!(!supports_wrapping_division(
                IntegerType::new(sign, bits).unwrap()
            ));
        }
    }
    assert!(!supports_wrapping_division(
        IntegerType::address(64).unwrap()
    ));
}
