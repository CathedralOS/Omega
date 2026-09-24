//! Wrapping divide and remainder legalize at every fixed native width of
//! either sign. Wrapping defines MIN / -1 and MIN % -1 but not division by
//! zero, so each row carries the exact accepted nonzero-divisor fact, and
//! replay rejects the sibling division family, swapped operands, and any
//! other obligation or fact.
use abstract_operations::AbstractOperation;
use legalized_operations::{LegalizedExactIntegerOperator, LegalizedScalarInstructionKind};
use optimization_core::AcceptedObligationFactIdentity;
use optimization_unit::{AcceptedObligationFact, attach_accepted_obligation_facts};
use semantic_vocabulary::{IntegerSign, IntegerType, ObligationId, OperationId, ScalarType};

use super::saturating_arithmetic::{binary_inputs, hosted_targets, value};
use crate::{legalize_target_operations, validate_legalized_operations};

fn division(divides: bool, integer: IntegerType) -> AbstractOperation {
    let (psi_operation, obligation) = (OperationId::new(1).unwrap(), ObligationId::new(1).unwrap());
    if divides {
        AbstractOperation::WrappingIntegerDivide {
            psi_operation,
            obligation,
            result: value(3),
            scalar_type: integer,
            left: value(1),
            right: value(2),
        }
    } else {
        AbstractOperation::WrappingIntegerRemainder {
            psi_operation,
            obligation,
            result: value(3),
            scalar_type: integer,
            left: value(1),
            right: value(2),
        }
    }
}

fn division_kind(
    divides: bool,
    left: semantic_vocabulary::ValueId,
    right: semantic_vocabulary::ValueId,
    obligation: ObligationId,
    accepted_fact: AcceptedObligationFactIdentity,
) -> LegalizedScalarInstructionKind {
    if divides {
        LegalizedScalarInstructionKind::WrappingDivide {
            left,
            right,
            obligation,
            accepted_fact,
        }
    } else {
        LegalizedScalarInstructionKind::WrappingRemainder {
            left,
            right,
            obligation,
            accepted_fact,
        }
    }
}

#[test]
fn wrapping_division_legalizes_every_native_width_with_its_nonzero_divisor_fact() {
    for native in hosted_targets() {
        for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
            for bits in [8, 16, 32, 64] {
                for divides in [true, false] {
                    let integer = IntegerType::new(sign, bits).unwrap();
                    let (source, target, unit) =
                        binary_inputs(integer, native, division(divides, integer));
                    // Correspondence alone must not invent the divisor evidence.
                    assert!(
                        legalize_target_operations(&target, &source, &unit).is_err(),
                        "{native:?} {sign:?} {bits} divides={divides} without a fact"
                    );
                    let fact = AcceptedObligationFact::new(
                        unit.psi,
                        [4; 32],
                        source.functions[0].machine,
                        OperationId::new(1).unwrap(),
                        ObligationId::new(1).unwrap(),
                        vec![1, 2, 3],
                    );
                    let accepted_fact = fact.identity;
                    let unit = attach_accepted_obligation_facts(unit, vec![fact]).unwrap();
                    let legalized = legalize_target_operations(&target, &source, &unit)
                        .unwrap_or_else(|error| {
                            panic!("{native:?} {sign:?} {bits} divides={divides}: {error:?}")
                        });
                    let row = &legalized.plan().scalar_functions[0].blocks[0].instructions[0];
                    let (left, right, obligation) =
                        (value(1), value(2), ObligationId::new(1).unwrap());
                    assert_eq!(
                        row.kind,
                        division_kind(divides, left, right, obligation, accepted_fact),
                        "{native:?} {sign:?} {bits} divides={divides}"
                    );
                    assert_eq!(
                        row.result.as_ref().unwrap().scalar_type,
                        ScalarType::Integer(integer)
                    );
                    validate_legalized_operations(
                        &target,
                        &source,
                        &unit,
                        legalized.plan().clone(),
                    )
                    .unwrap();
                    for substitute in [
                        division_kind(!divides, left, right, obligation, accepted_fact),
                        division_kind(divides, right, left, obligation, accepted_fact),
                        division_kind(
                            divides,
                            left,
                            right,
                            ObligationId::new(2).unwrap(),
                            accepted_fact,
                        ),
                        division_kind(
                            divides,
                            left,
                            right,
                            obligation,
                            AcceptedObligationFactIdentity::from_bytes([9; 32]),
                        ),
                        LegalizedScalarInstructionKind::ExactBinary {
                            operator: if divides {
                                LegalizedExactIntegerOperator::Divide
                            } else {
                                LegalizedExactIntegerOperator::Remainder
                            },
                            left,
                            right,
                            obligation,
                            accepted_fact,
                        },
                    ] {
                        let mut proposed = legalized.plan().clone();
                        proposed.scalar_functions[0].blocks[0].instructions[0].kind =
                            substitute.clone();
                        assert!(
                            validate_legalized_operations(&target, &source, &unit, proposed)
                                .is_err(),
                            "{native:?} {sign:?} {bits} divides={divides} accepted {substitute:?}"
                        );
                    }
                }
            }
        }
    }
}
