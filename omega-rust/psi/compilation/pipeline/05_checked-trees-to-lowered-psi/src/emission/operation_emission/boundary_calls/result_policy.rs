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
use language_core::operator_spelling::OperatorSpelling;
use numerics::float_semantics::FloatFormat;
use semantic_vocabulary::{IeeeFloatFormat, ScalarType, ValueId};
use symbols::SymbolHandle;
use terminal_psi::{
    FloatResultPolicy, FloatSaturationRule, Operation, OperationKind, OperationResult,
    ValueDeclaration,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BoundaryResultPolicy {
    None,
    Trapping {
        format: IeeeFloatFormat,
    },
    Saturating {
        format: IeeeFloatFormat,
        rule: FloatSaturationRule,
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
        let mut selections = checked
            .facts
            .operators
            .resolved_expression_selections()
            .filter(|(candidate, origin, _, _)| {
                *candidate == expression && origin_matches(checked, *origin, machine, coordinate)
            });
        let Some((_, _, selected, adapter)) = selections.next() else {
            if typed_trees::operator::declaration_by_symbol(&checked.typed, target).is_some() {
                return unsupported("boundary result policy lost its checked selection");
            }
            return Ok(Self::None);
        };
        if selections.next().is_some() || selected != target {
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
                // The division exception is part of the selected operation's
                // meaning. The authored expression may be a named call.
                let rule = match declaration.spelling {
                    Some(OperatorSpelling::Divide) => FloatSaturationRule::FiniteDivisionOverflow,
                    Some(
                        OperatorSpelling::Add
                        | OperatorSpelling::Subtract
                        | OperatorSpelling::Multiply,
                    ) => FloatSaturationRule::FiniteOperandOverflow,
                    _ => {
                        return unsupported(
                            "unimplemented: saturation needs exact overflow semantics for this boundary",
                        );
                    }
                };
                Ok(Self::Saturating { format, rule })
            }
        }
    }

    pub(crate) fn bind(
        self,
        raw: ValueDeclaration,
        arguments: &[ValueId],
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
            Self::Saturating { rule, .. } => {
                let [left, right] = arguments else {
                    return unsupported(
                        "unimplemented: saturation needs two exact scalar call operands",
                    );
                };
                FloatResultPolicy::Saturating {
                    raw_result: raw.id,
                    left: *left,
                    right: *right,
                    rule,
                }
            }
            Self::None => return Ok(None),
        }))
    }
}

fn origin_matches(
    checked: &CheckedTrees,
    mut origin: CheckedValueOrigin,
    machine: SymbolHandle,
    coordinate: SourceCallCoordinate,
) -> bool {
    let mut parents = Vec::new();
    while let CheckedValueOrigin::NestedExpression { parent } = origin {
        if parents.contains(&parent) {
            return false;
        }
        parents.push(parent);
        let mut origins = checked
            .facts
            .values
            .expression_values(parent)
            .map(|(_, value)| value.origin);
        let Some(parent_origin) = origins.next() else {
            return false;
        };
        if origins.any(|candidate| candidate != parent_origin) {
            return false;
        }
        origin = parent_origin;
    }
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
    let id = operations.allocate();
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id,
        result: OperationResult::Scalar(result),
        kind: OperationKind::FloatResultPolicy { policy },
    });
    Ok(result)
}
