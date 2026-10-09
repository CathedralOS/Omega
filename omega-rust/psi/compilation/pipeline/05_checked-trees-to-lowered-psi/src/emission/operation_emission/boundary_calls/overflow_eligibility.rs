//! Overflow eligibility over already completed inputs, before the raw call.
//! IEEE exponent/magnitude predicates use total representation views and exact
//! integer operations. Booleans become normalized bytes for conjunction; the
//! final equality publishes Bool. No provider or source operator is invoked.

use super::super::buffer::OperationBuffer;
use crate::lowering_error::{LoweringError, unsupported};
use crate::terminal_identities::{allocate_dense, value_id};
use semantic_vocabulary::{
    IeeeFloatFormat, IntegerSign, IntegerType, IntegerValue, ScalarType, ValueId,
};
use terminal_psi::{OperationKind, OperationResult, ValueDeclaration};

pub(super) fn emit(
    arguments: &[ValueId],
    types: &[ScalarType],
    divisor: Option<usize>,
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<ValueId, LoweringError> {
    if arguments.len() != types.len()
        || divisor
            .is_some_and(|position| !matches!(types.get(position), Some(ScalarType::IeeeFloat(_))))
    {
        return unsupported("overflow eligibility lost its exact numeric operands");
    }
    let mut emitter = Emitter {
        next_value,
        operations,
    };
    let byte = IntegerType::new(IntegerSign::Unsigned, 8)
        .map_err(|_| LoweringError::Unsupported("invalid Boolean byte carrier"))?;
    let one = emitter.constant(byte, 1)?;
    let mut conjunction = one;
    for (position, (argument, kind)) in arguments.iter().zip(types).enumerate() {
        match kind {
            ScalarType::Integer(_) => {}
            ScalarType::IeeeFloat(format) => {
                let (bits, exponent_mask, magnitude_mask) = match format {
                    IeeeFloatFormat::Binary32 => (32, 0x7f80_0000, 0x7fff_ffff),
                    IeeeFloatFormat::Binary64 => (64, 0x7ff0_0000_0000_0000, 0x7fff_ffff_ffff_ffff),
                };
                let integer = IntegerType::new(IntegerSign::Unsigned, bits)
                    .map_err(|_| LoweringError::Unsupported("invalid float image carrier"))?;
                let raw = emitter.value(
                    ScalarType::Integer(integer),
                    OperationKind::ScalarRepresentationView {
                        operand: *argument,
                        landing_obligation: None,
                    },
                )?;
                let mask = emitter.constant(integer, exponent_mask)?;
                let exponent = emitter.value(
                    ScalarType::Integer(integer),
                    OperationKind::IntegerBitwiseAnd {
                        left: raw,
                        right: mask,
                    },
                )?;
                let nonfinite = emitter.value(
                    ScalarType::Boolean,
                    OperationKind::IntegerEqual {
                        left: exponent,
                        right: mask,
                    },
                )?;
                let finite = emitter.value(
                    ScalarType::Boolean,
                    OperationKind::BooleanNot { operand: nonfinite },
                )?;
                conjunction = emitter.conjoin(byte, conjunction, finite)?;
                if divisor == Some(position) {
                    let mask = emitter.constant(integer, magnitude_mask)?;
                    let magnitude = emitter.value(
                        ScalarType::Integer(integer),
                        OperationKind::IntegerBitwiseAnd {
                            left: raw,
                            right: mask,
                        },
                    )?;
                    let zero = emitter.constant(integer, 0)?;
                    let is_zero = emitter.value(
                        ScalarType::Boolean,
                        OperationKind::IntegerEqual {
                            left: magnitude,
                            right: zero,
                        },
                    )?;
                    let nonzero = emitter.value(
                        ScalarType::Boolean,
                        OperationKind::BooleanNot { operand: is_zero },
                    )?;
                    conjunction = emitter.conjoin(byte, conjunction, nonzero)?;
                }
            }
            _ => return unsupported("overflow eligibility input is not numeric"),
        }
    }
    emitter.value(
        ScalarType::Boolean,
        OperationKind::IntegerEqual {
            left: conjunction,
            right: one,
        },
    )
}
struct Emitter<'a> {
    next_value: &'a mut u64,
    operations: &'a mut OperationBuffer,
}
impl Emitter<'_> {
    fn value(
        &mut self,
        scalar_type: ScalarType,
        kind: OperationKind,
    ) -> Result<ValueId, LoweringError> {
        let id = value_id(allocate_dense(self.next_value)?);
        self.operations.push_leaf(
            OperationResult::Scalar(ValueDeclaration {
                id,
                scalar_type,
                qualifications: Default::default(),
            }),
            kind,
        );
        Ok(id)
    }
    fn constant(&mut self, integer: IntegerType, value: u64) -> Result<ValueId, LoweringError> {
        self.value(
            ScalarType::Integer(integer),
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(value.into()),
            },
        )
    }
    fn conjoin(
        &mut self,
        byte: IntegerType,
        conjunction: ValueId,
        predicate: ValueId,
    ) -> Result<ValueId, LoweringError> {
        let predicate = self.value(
            ScalarType::Integer(byte),
            OperationKind::ScalarRepresentationView {
                operand: predicate,
                landing_obligation: None,
            },
        )?;
        self.value(
            ScalarType::Integer(byte),
            OperationKind::IntegerBitwiseAnd {
                left: conjunction,
                right: predicate,
            },
        )
    }
}
