//! Saturating arithmetic legalizes every fixed 8/16/32/64-bit carrier to a
//! kind naming that carrier; a non-fixed carrier reports the unsupported
//! family instead of a custody mismatch, and replay rejects a kind that names
//! a different carrier than the source operation.
use abstract_operations::{
    AbstractFunctionResult, AbstractOperation, AbstractOperationPlan, AbstractParameter,
    AbstractResult,
};
use legalized_operations::LegalizedScalarInstructionKind;
use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::{
    EdgeId, FuelScheduleIdentity, IntegerType, OperationId, ScalarType, ValueId,
};
use target::NativeTarget;
use target_operations::TargetOperationPlan;

use crate::{legalize_target_operations, validate_legalized_operations};
use legalized_operations::SaturatingCarrier;

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
    let operands = (value(1), value(2));
    binary_inputs(
        integer,
        native,
        if adds {
            AbstractOperation::SaturatingIntegerAdd {
                psi_operation: OperationId::new(1).unwrap(),
                result: value(3),
                scalar_type: integer,
                left: operands.0,
                right: operands.1,
            }
        } else {
            AbstractOperation::SaturatingIntegerSubtract {
                psi_operation: OperationId::new(1).unwrap(),
                result: value(3),
                scalar_type: integer,
                left: operands.0,
                right: operands.1,
            }
        },
    )
}

/// One two-parameter integer operation producing `value(3)`, returned directly.
fn binary_inputs(
    integer: IntegerType,
    native: NativeTarget,
    operation: AbstractOperation,
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
        operation,
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
fn every_fixed_carrier_saturating_add_and_subtract_legalize_to_its_carrier_kind() {
    for native in hosted_targets() {
        for carrier in SaturatingCarrier::ALL {
            for adds in [true, false] {
                let integer = carrier.integer_type();
                let (source, target, unit) = saturating_binary_inputs(integer, native, adds);
                let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
                let row = &legalized.plan().scalar_functions[0].blocks[0].instructions[0];
                let expected = if adds {
                    LegalizedScalarInstructionKind::SaturatingAdd {
                        carrier,
                        left: value(1),
                        right: value(2),
                    }
                } else {
                    LegalizedScalarInstructionKind::SaturatingSubtract {
                        carrier,
                        left: value(1),
                        right: value(2),
                    }
                };
                assert_eq!(row.kind, expected, "{native:?} {carrier:?} adds={adds}");
                assert_eq!(
                    row.result.as_ref().unwrap().scalar_type,
                    ScalarType::Integer(integer)
                );
                validate_legalized_operations(&target, &source, &unit, legalized.plan().clone())
                    .unwrap();
            }
        }
    }
}

/// A legalized kind naming the wrong carrier would clamp to the wrong bounds
/// while every register-level check still passes; replay must reject every
/// other carrier, the sibling operation, swapped operands, and a wrapping
/// substitute.
#[test]
fn saturating_replay_rejects_a_kind_naming_a_different_carrier() {
    for native in hosted_targets() {
        for carrier in SaturatingCarrier::ALL {
            for adds in [true, false] {
                let (source, target, unit) =
                    saturating_binary_inputs(carrier.integer_type(), native, adds);
                let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
                let (left, right) = (value(1), value(2));
                let accepted = legalized.plan().scalar_functions[0].blocks[0].instructions[0]
                    .kind
                    .clone();
                let mut substitutes = vec![
                    LegalizedScalarInstructionKind::WrappingAdd { left, right },
                    LegalizedScalarInstructionKind::SaturatingAdd {
                        carrier,
                        left: right,
                        right: left,
                    },
                    LegalizedScalarInstructionKind::SaturatingSubtract {
                        carrier,
                        left: right,
                        right: left,
                    },
                ];
                for other in SaturatingCarrier::ALL {
                    substitutes.push(LegalizedScalarInstructionKind::SaturatingAdd {
                        carrier: other,
                        left,
                        right,
                    });
                    substitutes.push(LegalizedScalarInstructionKind::SaturatingSubtract {
                        carrier: other,
                        left,
                        right,
                    });
                }
                for substitute in substitutes {
                    if substitute == accepted {
                        continue;
                    }
                    let mut proposed = legalized.plan().clone();
                    proposed.scalar_functions[0].blocks[0].instructions[0].kind =
                        substitute.clone();
                    assert!(
                        validate_legalized_operations(&target, &source, &unit, proposed).is_err(),
                        "{native:?} {carrier:?} adds={adds} accepted {substitute:?}"
                    );
                }
            }
        }
    }
}

/// Every native fixed width is now a saturating carrier, so the
/// `UnsupportedScalarOperation` route is no longer reachable through this
/// fixture: a non-native carrier (an address carrier or a 128-bit width)
/// is rejected by target lowering as `UnitFunctionHasScalarParameters`
/// before legalization admits any node. What remains pinned is the
/// admission predicate itself, which is what node admission consults.
#[test]
fn non_native_carriers_are_not_saturating_carriers() {
    assert_eq!(
        SaturatingCarrier::from_integer(IntegerType::address(64).unwrap()),
        None
    );
    assert_eq!(
        SaturatingCarrier::from_integer(
            IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 128).unwrap()
        ),
        None
    );
    for carrier in SaturatingCarrier::ALL {
        assert_eq!(
            SaturatingCarrier::from_integer(carrier.integer_type()),
            Some(carrier)
        );
    }
}

/// Saturating multiplication is lowered to target operations but has no
/// legalized scalar kind at any carrier, so it is the reachable witness the
/// `UnsupportedScalarOperation` classification exists for. Node admission must
/// refuse it by name — returning the rejected operation and its machine for the
/// compile diagnostic — rather than panicking, silently dropping the row, or
/// collapsing into the `SourceCustodyMismatch` producer-defect spelling.
#[test]
fn saturating_multiply_reports_the_unsupported_family_with_its_operation() {
    for native in hosted_targets() {
        for carrier in SaturatingCarrier::ALL {
            let integer = carrier.integer_type();
            let (source, target, unit) = binary_inputs(
                integer,
                native,
                AbstractOperation::SaturatingIntegerMultiply {
                    psi_operation: OperationId::new(1).unwrap(),
                    result: value(3),
                    scalar_type: integer,
                    left: value(1),
                    right: value(2),
                },
            );
            let error = legalize_target_operations(&target, &source, &unit)
                .expect_err("saturating multiply has no legalized scalar kind");
            let crate::LegalizationError::UnsupportedScalarOperation { machine, operation } =
                &error
            else {
                panic!("{native:?} {carrier:?} reported {error:?}");
            };
            assert_eq!(*machine, source.functions[0].machine);
            assert!(
                matches!(
                    operation,
                    AbstractOperation::SaturatingIntegerMultiply { .. }
                ),
                "{native:?} {carrier:?} retained {operation:?}"
            );
            // The retained operation reaches the compile diagnostic through
            // this rendering; an abort would never produce a message at all.
            assert!(
                format!("{error}").contains("no legal scalar instruction"),
                "{native:?} {carrier:?} rendered {error}"
            );
        }
    }
}
