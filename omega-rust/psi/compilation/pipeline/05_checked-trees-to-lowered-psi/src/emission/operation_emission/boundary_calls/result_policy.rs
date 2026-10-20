//! Checked call-result adaptation shared by every scalar boundary emitter.
//!
//! Selection belongs to the exact checked occurrence, not to the destination
//! type or to an expression recognizer. Prepare it while source custody is in
//! scope, bind the actual call operands once, and publish the adapted value to
//! the ordinary consumer. The raw call keeps its provider and source identity.

use super::super::buffer::{OperationBuffer, SourceCallCoordinate};
use crate::lowering_error::{LoweringError, unsupported};
use crate::terminal_identities::{allocate_dense, value_id};
use checked_trees::{
    CheckedArithmeticPolicyAdapter, CheckedTrees, CheckedValueOrigin, NominalMachineUseSite,
};
use numerics::float_semantics::FloatFormat;
use semantic_vocabulary::{IeeeFloatFormat, ScalarType, ValueId};
use symbols::SymbolHandle;
use terminal_psi::{FloatResultPolicy, OperationKind, OperationResult, ValueDeclaration};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BoundaryResultPolicy {
    None,
    Trapping {
        format: IeeeFloatFormat,
    },
    Saturating {
        format: IeeeFloatFormat,
        divisor_position: Option<usize>,
    },
}

impl BoundaryResultPolicy {
    pub(crate) fn prepare(
        checked: &CheckedTrees,
        machine: SymbolHandle,
        coordinate: SourceCallCoordinate,
        site: Option<NominalMachineUseSite>,
        target: SymbolHandle,
    ) -> Result<Self, LoweringError> {
        let Some(NominalMachineUseSite::Expression(expression)) = site else {
            return Ok(Self::None);
        };
        let mut first_selection = None;
        let mut multiple_selections = false;
        checked
            .facts
            .operators
            .for_each_resolved_expression_selection(expression, |origin, selected, adapter, _| {
                if origin_matches(checked, origin, machine, coordinate) {
                    if first_selection.is_none() {
                        first_selection = Some((selected, adapter));
                    } else {
                        multiple_selections = true;
                    }
                }
            });
        let Some((selected, adapter)) = first_selection else {
            if typed_trees::operator::declaration_by_symbol(&checked.typed, target).is_some() {
                return unsupported("boundary result policy lost its checked selection");
            }
            return Ok(Self::None);
        };
        if multiple_selections || selected != target {
            return unsupported("boundary result policy has no unique exact checked selection");
        }
        let Some(format) = adapter.float_format() else {
            return Ok(Self::None);
        };
        let format = match format {
            FloatFormat::BINARY32 => IeeeFloatFormat::Binary32,
            FloatFormat::BINARY64 => IeeeFloatFormat::Binary64,
            _ => return unsupported("boundary result policy has an unsupported float format"),
        };
        match adapter {
            CheckedArithmeticPolicyAdapter::None => Ok(Self::None),
            CheckedArithmeticPolicyAdapter::FloatTrappingNonFinite { .. } => {
                Ok(Self::Trapping { format })
            }
            CheckedArithmeticPolicyAdapter::FloatSaturatingOverflowOnly { .. } => {
                let declaration =
                    typed_trees::operator::declaration_by_symbol(&checked.typed, target).ok_or(
                        LoweringError::Unsupported(
                            "saturating boundary lost its selected declaration",
                        ),
                    )?;
                let divisor =
                    validation::float_saturation_divisor_parameter(&checked.typed, declaration)
                        .ok_or(LoweringError::Unsupported(
                            "saturating boundary lost its raw semantic equation",
                        ))?;
                Ok(Self::Saturating {
                    format,
                    divisor_position: divisor,
                })
            }
        }
    }

    pub(crate) fn bind(
        self,
        raw: ValueDeclaration,
        arguments: &[ValueId],
        argument_types: &[ScalarType],
        next_value: &mut u64,
        operations: &mut OperationBuffer,
    ) -> Result<Option<FloatResultPolicy>, LoweringError> {
        let format = match self {
            Self::None => return Ok(None),
            Self::Trapping { format } | Self::Saturating { format, .. } => format,
        };
        if raw.scalar_type != ScalarType::IeeeFloat(format) || !raw.qualifications.is_empty() {
            return unsupported("boundary result policy disagrees with its raw result format");
        }
        Ok(Some(match self {
            Self::Trapping { .. } => FloatResultPolicy::Trapping { raw_result: raw.id },
            Self::Saturating {
                divisor_position, ..
            } => {
                let eligible = super::overflow_eligibility::emit(
                    arguments,
                    argument_types,
                    divisor_position,
                    next_value,
                    operations,
                )?;
                FloatResultPolicy::Saturating {
                    raw_result: raw.id,
                    eligible,
                }
            }
            Self::None => return Ok(None),
        }))
    }
}

fn origin_matches(
    checked: &CheckedTrees,
    origin: CheckedValueOrigin,
    machine: SymbolHandle,
    coordinate: SourceCallCoordinate,
) -> bool {
    let Some(origin) = checked.consistent_terminal_origin(origin) else {
        return false;
    };
    matches!(origin, CheckedValueOrigin::StateStatement { machine_symbol, state_symbol, statement_index, .. }
        if machine_symbol == machine && state_symbol == coordinate.state && statement_index == coordinate.statement_index)
}

pub(crate) fn emit(
    policy: Option<FloatResultPolicy>,
    raw: ValueDeclaration,
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<ValueDeclaration, LoweringError> {
    let Some(policy) = policy else {
        return Ok(raw);
    };
    let result = ValueDeclaration {
        id: value_id(allocate_dense(next_value)?),
        ..raw
    };
    operations.push_leaf(
        OperationResult::Scalar(result),
        OperationKind::FloatResultPolicy { policy },
    );
    Ok(result)
}
