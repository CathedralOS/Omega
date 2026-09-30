//! Materialize typed Boolean values before later operations can replace condition state.
//! Equal 64-bit address models share native comparison instructions, retaining
//! their source types and unsigned ordering. Mixed address/count comparisons
//! remain invalid even when both values occupy the same register width.
use super::{
    IntegerSign, LegalizedScalarInstructionKind, ScalarType, SelectedInstructionKind,
    SelectedInstructionProvenance, VirtualRegisterId,
};
use crate::SelectedInstructionError;
use crate::selection::validation::scalar_graph::Replay;
use legalized_operations::LegalizedScalarComparison;

pub(super) fn validate(
    operation: &legalized_operations::LegalizedScalarInstruction,
    state: &mut Replay<'_>,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let result = operation
        .result
        .ok_or_else(|| SelectedInstructionError::custody())?;
    if result.scalar_type != ScalarType::Boolean {
        return Err(SelectedInstructionError::custody());
    }
    let (comparison, comparison_key, operands, values, materialize) = match operation.kind {
        LegalizedScalarInstructionKind::Compare {
            predicate,
            operand_type,
            right_operand_type,
            left,
            right,
        } => {
            let (_, left_register, _, left_type) = state
                .resolve(left)
                .ok_or_else(|| SelectedInstructionError::custody())?;
            let (_, right_register, _, right_type) = state
                .resolve(right)
                .ok_or_else(|| SelectedInstructionError::custody())?;
            if left_type != operand_type || right_type != right_operand_type {
                return Err(SelectedInstructionError::custody());
            }
            if left_type != right_type {
                return mixed_integer(
                    operation,
                    state,
                    left_register,
                    right_register,
                    left_type,
                    right_type,
                    predicate,
                );
            }
            let signed = match operand_type {
                ScalarType::Boolean if predicate == LegalizedScalarComparison::Equal => false,
                ScalarType::Integer(integer)
                    if (integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
                        && matches!(integer.bits(), 8 | 16 | 32 | 64))
                        || (integer.is_address() && integer.bits() == 64) =>
                {
                    integer.sign() == IntegerSign::Signed
                }
                _ => return Err(SelectedInstructionError::custody()),
            };
            let materialize = match (predicate, signed) {
                (LegalizedScalarComparison::Equal, _) => {
                    SelectedInstructionKind::MaterializeBooleanEqual
                }
                (LegalizedScalarComparison::LessThan, false) => {
                    SelectedInstructionKind::MaterializeBooleanU64LessThan
                }
                (LegalizedScalarComparison::LessThan, true) => {
                    SelectedInstructionKind::MaterializeBooleanI64LessThan
                }
                (LegalizedScalarComparison::LessOrEqual, false) => {
                    SelectedInstructionKind::MaterializeBooleanU64LessOrEqual
                }
                (LegalizedScalarComparison::LessOrEqual, true) => {
                    SelectedInstructionKind::MaterializeBooleanI64LessOrEqual
                }
            };
            (
                SelectedInstructionKind::CompareI64,
                state.constraints.keys.compare_i64,
                vec![left_register, right_register],
                vec![left, right, result.value],
                materialize,
            )
        }
        LegalizedScalarInstructionKind::BooleanNot { operand } => {
            let (_, input, _, carrier) = state
                .resolve(operand)
                .ok_or_else(|| SelectedInstructionError::custody())?;
            if carrier != ScalarType::Boolean {
                return Err(SelectedInstructionError::custody());
            }
            (
                SelectedInstructionKind::CompareI64Zero,
                state.constraints.keys.compare_i64_zero,
                vec![input],
                vec![operand, result.value],
                SelectedInstructionKind::MaterializeBooleanEqual,
            )
        }
        _ => return Err(SelectedInstructionError::custody()),
    };
    // The source operation and logical fuel occur once, at the comparison.
    state.check_instruction(
        comparison,
        comparison_key,
        &operands,
        &SelectedInstructionProvenance {
            operations: vec![operation.operation],
            values,
            fuel: operation.fuel.clone(),
            ..Default::default()
        },
    )?;
    let output =
        state.result_register(result.value, result.definition_site, ScalarType::Boolean)?;
    state.check_instruction(
        materialize,
        state.constraints.keys.materialize_boolean,
        &[output],
        &SelectedInstructionProvenance {
            values: vec![result.value],
            ..Default::default()
        },
    )?;
    Ok(output)
}

/// Compare independently normalized carriers without changing either source
/// definition. Negative signed values sort before every unsigned value, even
/// where their register bits equal an unsigned value (for example -1/u64::MAX).
fn mixed_integer(
    operation: &legalized_operations::LegalizedScalarInstruction,
    state: &mut Replay<'_>,
    left_register: VirtualRegisterId,
    right_register: VirtualRegisterId,
    left_type: ScalarType,
    right_type: ScalarType,
    predicate: LegalizedScalarComparison,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let invalid = SelectedInstructionError::custody;
    let (ScalarType::Integer(left_type), ScalarType::Integer(right_type)) = (left_type, right_type)
    else {
        return Err(invalid());
    };
    if left_type.is_address()
        || right_type.is_address()
        || !matches!(left_type.bits(), 8 | 16 | 32 | 64)
        || !matches!(right_type.bits(), 8 | 16 | 32 | 64)
    {
        return Err(invalid());
    }
    let result = operation.result.ok_or_else(invalid)?;
    let LegalizedScalarInstructionKind::Compare { left, right, .. } = operation.kind else {
        return Err(invalid());
    };
    let mut sequence = IntegerComparisonSequence {
        state,
        value: result.value,
        site: result.definition_site,
        provenance: SelectedInstructionProvenance {
            operations: vec![operation.operation],
            values: vec![left, right, result.value],
            fuel: operation.fuel.clone(),
            ..Default::default()
        },
    };
    let left = sequence.normalize(left_register, left_type)?;
    let right = sequence.normalize(right_register, right_type)?;
    if left_type.sign() == right_type.sign() {
        return sequence.compare(
            left,
            right,
            predicate,
            left_type.sign() == IntegerSign::Signed,
        );
    }
    let zero = sequence.zero()?;
    let signed_left = left_type.sign() == IntegerSign::Signed;
    let signed = if signed_left { left } else { right };
    let signed_left_order = signed_left && predicate != LegalizedScalarComparison::Equal;
    let sign_condition = if signed_left_order {
        sequence.compare(zero, signed, LegalizedScalarComparison::LessOrEqual, true)?
    } else {
        sequence.compare(signed, zero, LegalizedScalarComparison::LessThan, true)?
    };
    let raw = sequence.compare(left, right, predicate, false)?;
    // Booleans are canonical 0/1. (!negative)<=raw is negative OR raw;
    // negative<raw is !negative AND raw. This preserves equality at both
    // signed/unsigned extremes without an unchecked integer cast.
    sequence.compare(
        sign_condition,
        raw,
        if signed_left_order {
            LegalizedScalarComparison::LessOrEqual
        } else {
            LegalizedScalarComparison::LessThan
        },
        false,
    )
}

struct IntegerComparisonSequence<'state, 'catalog> {
    state: &'state mut Replay<'catalog>,
    value: semantic_vocabulary::ValueId,
    site: optimization_unit::ValueDefinitionSite,
    provenance: SelectedInstructionProvenance,
}

impl IntegerComparisonSequence<'_, '_> {
    fn provenance(&mut self) -> SelectedInstructionProvenance {
        let provenance = self.provenance.clone();
        self.provenance.fuel.clear();
        provenance
    }

    fn normalize(
        &mut self,
        input: VirtualRegisterId,
        carrier: semantic_vocabulary::IntegerType,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let kind = match (carrier.sign(), carrier.bits()) {
            (_, 64) => return Ok(input),
            (IntegerSign::Signed, 8) => SelectedInstructionKind::SignExtendI8,
            (IntegerSign::Signed, 16) => SelectedInstructionKind::SignExtendI16,
            (IntegerSign::Signed, 32) => SelectedInstructionKind::SignExtendI32,
            (IntegerSign::Unsigned, 8) => SelectedInstructionKind::ZeroExtendU8,
            (IntegerSign::Unsigned, 16) => SelectedInstructionKind::ZeroExtendU16,
            (IntegerSign::Unsigned, 32) => SelectedInstructionKind::ZeroExtendU32,
            _ => return Err(SelectedInstructionError::custody()),
        };
        let carrier = semantic_vocabulary::IntegerType::new(carrier.sign(), 64)
            .map_err(|_| SelectedInstructionError::custody())?;
        let output =
            self.state
                .result_register(self.value, self.site, ScalarType::Integer(carrier))?;
        let provenance = self.provenance();
        self.state.check_instruction(
            kind,
            self.state.constraints.keys.copy_i64,
            &[input, output],
            &provenance,
        )?;
        Ok(output)
    }

    fn zero(&mut self) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let carrier = semantic_vocabulary::IntegerType::new(IntegerSign::Signed, 64)
            .map_err(|_| SelectedInstructionError::custody())?;
        let output =
            self.state
                .result_register(self.value, self.site, ScalarType::Integer(carrier))?;
        let provenance = self.provenance();
        self.state.check_instruction(
            SelectedInstructionKind::MaterializeI64 {
                value: semantic_vocabulary::IntegerValue::Signed(0),
            },
            self.state.constraints.keys.materialize_i64,
            &[output],
            &provenance,
        )?;
        Ok(output)
    }

    fn compare(
        &mut self,
        left: VirtualRegisterId,
        right: VirtualRegisterId,
        predicate: LegalizedScalarComparison,
        signed: bool,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let provenance = self.provenance();
        self.state.check_instruction(
            SelectedInstructionKind::CompareI64,
            self.state.constraints.keys.compare_i64,
            &[left, right],
            &provenance,
        )?;
        let materialize = match (predicate, signed) {
            (LegalizedScalarComparison::Equal, _) => {
                SelectedInstructionKind::MaterializeBooleanEqual
            }
            (LegalizedScalarComparison::LessThan, true) => {
                SelectedInstructionKind::MaterializeBooleanI64LessThan
            }
            (LegalizedScalarComparison::LessThan, false) => {
                SelectedInstructionKind::MaterializeBooleanU64LessThan
            }
            (LegalizedScalarComparison::LessOrEqual, true) => {
                SelectedInstructionKind::MaterializeBooleanI64LessOrEqual
            }
            (LegalizedScalarComparison::LessOrEqual, false) => {
                SelectedInstructionKind::MaterializeBooleanU64LessOrEqual
            }
        };
        let output = self
            .state
            .result_register(self.value, self.site, ScalarType::Boolean)?;
        let provenance = self.provenance();
        self.state.check_instruction(
            materialize,
            self.state.constraints.keys.materialize_boolean,
            &[output],
            &provenance,
        )?;
        Ok(output)
    }
}
