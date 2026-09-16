//! Saturating arithmetic legalizes only the families this stage realizes (the
//! u64 and signed i32 carriers); every other width reports the unsupported
//! family instead of a custody mismatch, and replay rejects a kind that names
//! a different carrier than the source operation.
use abstract_operations::{
    AbstractFunctionResult, AbstractOperation, AbstractOperationPlan, AbstractParameter,
    AbstractResult,
};
use legalized_operations::LegalizedScalarInstructionKind;
use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::{
    EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, OperationId, ScalarType,
    ValueId,
};
use target::NativeTarget;
use target_operations::TargetOperationPlan;

use crate::{LegalizationError, legalize_target_operations, validate_legalized_operations};

fn value(ordinal: u64) -> ValueId {
    ValueId::new(ordinal).unwrap()
}

fn hosted_targets() -> [NativeTarget; 4] {
    [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ]
}

/// One saturating addition of two parameters, returned directly.
fn saturating_add_inputs(
    integer: IntegerType,
    native: NativeTarget,
) -> (
    AbstractOperationPlan,
    TargetOperationPlan,
    PsiOptimizationUnit,
) {
    saturating_binary_inputs(integer, native, true)
}

/// One saturating addition or subtraction of two parameters, returned directly.
fn saturating_binary_inputs(
    integer: IntegerType,
    native: NativeTarget,
    adds: bool,
) -> (
    AbstractOperationPlan,
    TargetOperationPlan,
    PsiOptimizationUnit,
) {
    let scalar_type = ScalarType::Integer(integer);
    let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
    let function = &mut source.functions[0];
    function.parameters = [value(1), value(2)]
        .map(|value| AbstractParameter { value, scalar_type })
        .to_vec();
    function.result = AbstractFunctionResult::Scalar(AbstractResult {
        value: value(4),
        scalar_type,
    });
    function.operations = vec![
        if adds {
            AbstractOperation::SaturatingIntegerAdd {
                psi_operation: OperationId::new(1).unwrap(),
                result: value(3),
                scalar_type: integer,
                left: value(1),
                right: value(2),
            }
        } else {
            AbstractOperation::SaturatingIntegerSubtract {
                psi_operation: OperationId::new(1).unwrap(),
                result: value(3),
                scalar_type: integer,
                left: value(1),
                right: value(2),
            }
        },
        AbstractOperation::Return {
            psi_edge: EdgeId::new(1).unwrap(),
            result: value(4),
            value: value(3),
            scalar_type,
            cleanup_actions: Vec::new(),
        },
    ];
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        &source,
        abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
    )
    .unwrap();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
    (source, target, unit)
}

#[test]
fn unsigned_64_bit_saturating_add_legalizes_to_its_u64_kind() {
    for native in hosted_targets() {
        let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let (source, target, unit) = saturating_add_inputs(integer, native);
        let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
        let row = &legalized.plan().scalar_functions[0].blocks[0].instructions[0];
        assert_eq!(
            row.kind,
            LegalizedScalarInstructionKind::SaturatingAddU64 {
                left: value(1),
                right: value(2),
            },
            "{native:?}"
        );
    }
}

#[test]
fn signed_32_bit_saturating_add_and_subtract_legalize_to_their_i32_kinds() {
    for native in hosted_targets() {
        for adds in [true, false] {
            let integer = IntegerType::new(IntegerSign::Signed, 32).unwrap();
            let (source, target, unit) = saturating_binary_inputs(integer, native, adds);
            let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
            let row = &legalized.plan().scalar_functions[0].blocks[0].instructions[0];
            let expected = if adds {
                LegalizedScalarInstructionKind::SaturatingAddI32 {
                    left: value(1),
                    right: value(2),
                }
            } else {
                LegalizedScalarInstructionKind::SaturatingSubtractI32 {
                    left: value(1),
                    right: value(2),
                }
            };
            assert_eq!(row.kind, expected, "{native:?} adds={adds}");
            assert_eq!(
                row.result.as_ref().unwrap().scalar_type,
                ScalarType::Integer(integer)
            );
        }
    }
}

/// A legalized kind naming the wrong carrier would clamp an i32 value to the
/// u64 bounds (or a u64 value to the i32 bounds); replay must reject both
/// directions, along with swapped operands and the sibling operation.
#[test]
fn saturating_replay_rejects_a_kind_naming_a_different_carrier() {
    for native in hosted_targets() {
        for (integer, adds) in [
            (IntegerType::new(IntegerSign::Signed, 32).unwrap(), true),
            (IntegerType::new(IntegerSign::Signed, 32).unwrap(), false),
            (IntegerType::new(IntegerSign::Unsigned, 64).unwrap(), true),
            (IntegerType::new(IntegerSign::Unsigned, 64).unwrap(), false),
        ] {
            let (source, target, unit) = saturating_binary_inputs(integer, native, adds);
            let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
            validate_legalized_operations(&target, &source, &unit, legalized.plan().clone())
                .unwrap();
            let (left, right) = (value(1), value(2));
            let substitutes = [
                LegalizedScalarInstructionKind::SaturatingAddU64 { left, right },
                LegalizedScalarInstructionKind::SaturatingSubtractU64 { left, right },
                LegalizedScalarInstructionKind::SaturatingAddI32 { left, right },
                LegalizedScalarInstructionKind::SaturatingSubtractI32 { left, right },
                LegalizedScalarInstructionKind::WrappingAdd { left, right },
                LegalizedScalarInstructionKind::SaturatingAddI32 {
                    left: right,
                    right: left,
                },
                LegalizedScalarInstructionKind::SaturatingAddU64 {
                    left: right,
                    right: left,
                },
            ];
            let accepted = legalized.plan().scalar_functions[0].blocks[0].instructions[0]
                .kind
                .clone();
            for substitute in substitutes {
                if substitute == accepted {
                    continue;
                }
                let mut proposed = legalized.plan().clone();
                proposed.scalar_functions[0].blocks[0].instructions[0].kind = substitute.clone();
                assert!(
                    validate_legalized_operations(&target, &source, &unit, proposed).is_err(),
                    "{native:?} {integer:?} adds={adds} accepted {substitute:?}"
                );
            }
        }
    }
}

#[test]
fn signed_16_bit_saturating_add_reports_the_unsupported_family() {
    for native in hosted_targets() {
        let integer = IntegerType::new(IntegerSign::Signed, 16).unwrap();
        let (source, target, unit) = saturating_add_inputs(integer, native);
        let error = legalize_target_operations(&target, &source, &unit).unwrap_err();
        assert_eq!(
            error,
            LegalizationError::UnsupportedScalarOperation {
                machine: MachineId::new(1).unwrap(),
                operation: source.functions[0].operations[0].clone(),
            },
            "{native:?}"
        );
        assert!(
            error.to_string().contains("SaturatingIntegerAdd")
                && error.to_string().contains("bits: 16"),
            "{error}"
        );
    }
}
