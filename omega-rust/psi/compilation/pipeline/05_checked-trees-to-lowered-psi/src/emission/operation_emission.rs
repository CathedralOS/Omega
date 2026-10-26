//! Emit an ordered scalar binding into the caller's operation buffer.
//!
//! Expressions emit leaves, calls preserve argument/control order, and the buffer
//! retains identities and source custody. Module-wide proof completion is separate.

pub(crate) mod boolean;
pub(crate) mod boundary_calls;
pub(crate) mod buffer;
pub(crate) mod calls;
pub(crate) mod dynamic_calls;
pub(crate) mod expressions;
pub(crate) mod integer;
pub(crate) mod place_declarations;
pub(crate) mod view_subslice;

use crate::emission::operation_emission::buffer::scalar_result_declaration;
use crate::emission::selected_comparison::SelectedComparisonMeaning;
use crate::lowering_error::LoweringError;
use crate::terminal_identities::value_id;
pub(crate) use boolean::emit_boolean_expression;
use buffer::OperationBuffer;
use calls::{CallEmissionContext, LoweredDirectCallBinding};
use expressions::LoweredDirectExpression;
pub(crate) use expressions::emit_direct_expression;
use lowered_psi::LoweredSelectedComparisonOperandOrder;
use semantic_vocabulary::{QualifiedScalarType, ScalarType, ValueId};
use terminal_psi::{Operation, OperationKind, ValueDeclaration};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoweredScalarBinding {
    StorageAddress(calls::PreparedStructuralArgument, ScalarType),
    SelectedComparison {
        occurrence: crate::emission::selected_comparison::SelectedComparison,
        left: LoweredDirectExpression,
        right: LoweredDirectExpression,
    },
    Expression(LoweredDirectExpression),
    DirectCall(LoweredDirectCallBinding),
    /// One nested boundary application whose operands are already complete.
    BoundaryCall(boundary_calls::LoweredBoundaryCallBinding),
    /// One selector call whose selector and data operands are complete.
    DynamicCall(dynamic_calls::LoweredDynamicCallBinding),
}

impl LoweredScalarBinding {
    pub(crate) fn value_type(
        &self,
        parameters: &[QualifiedScalarType],
    ) -> Result<QualifiedScalarType, LoweringError> {
        match self {
            Self::StorageAddress(_, scalar_type) => Ok((*scalar_type).into()),
            Self::SelectedComparison { .. } => Ok(ScalarType::Boolean.into()),
            Self::Expression(expression) => expression.value_type(parameters),
            Self::DirectCall(call) => Ok(call.result_type),
            Self::BoundaryCall(call) => Ok(call.result_type),
            Self::DynamicCall(call) => Ok(call.result_type),
        }
    }
    pub(crate) const fn scalar_type(&self) -> ScalarType {
        match self {
            Self::StorageAddress(_, scalar_type) => *scalar_type,
            Self::SelectedComparison { .. } => ScalarType::Boolean,
            Self::Expression(expression) => expression.scalar_type(),
            Self::DirectCall(call) => call.result_type.scalar_type,
            Self::BoundaryCall(call) => call.result_type.scalar_type,
            Self::DynamicCall(call) => call.result_type.scalar_type,
        }
    }
}

pub(crate) fn emit_scalar_binding(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &checked_trees::CheckedTrees,
    binding: &LoweredScalarBinding,
    parameters: &[ValueDeclaration],
    frontier: &lowered_psi::LoweredSourceFrontierBindings,
    caller_erased_formals: &[ValueDeclaration],
    next_value_identity: &mut u64,
    operations: &mut OperationBuffer,
    call_emission: &mut CallEmissionContext<'_>,
) -> Result<ValueId, LoweringError> {
    let result = match binding {
        LoweredScalarBinding::StorageAddress(argument, scalar_type) => {
            let source =
                argument.emit(parameters, call_emission, next_value_identity, operations)?;
            let id = value_id(*next_value_identity);
            *next_value_identity =
                next_value_identity
                    .checked_add(1)
                    .ok_or(LoweringError::Unsupported(
                        "address result identity overflow",
                    ))?;
            let operation = operations.allocate();
            operations.push(Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: operation,
                result: buffer::scalar_result_declaration(id, *scalar_type),
                kind: OperationKind::StorageAddress { source },
            });
            Ok(id)
        }

        LoweredScalarBinding::SelectedComparison {
            occurrence,
            left,
            right,
        } => {
            let left = emit_direct_expression(left, parameters, next_value_identity, operations);
            let right = emit_direct_expression(right, parameters, next_value_identity, operations);
            let id = value_id(*next_value_identity);
            *next_value_identity =
                next_value_identity
                    .checked_add(1)
                    .ok_or(LoweringError::Unsupported(
                        "comparison value identity overflow",
                    ))?;
            let operation = operations.allocate();
            // Operator-use and application-site identities retain the authored
            // owner; this coordinate names the machine receiving the operation.
            // A fused sibling need not have a standalone Terminal machine.
            let terminal_machine = call_emission.terminal_machine;
            // The occurrence row is recorded beside the exact operation it
            // names: a selected use that reached emission without one would
            // be indistinguishable from a builtin comparison downstream.
            let (kind, negated) = match occurrence.meaning {
                SelectedComparisonMeaning::IeeeFloat { comparison, .. } => {
                    operations.selected_comparisons.push(
                        lowered_psi::LoweredSelectedComparisonOccurrence {
                            operator_use: occurrence.operator_use,
                            application_site: occurrence.application_site,
                            requirement_operator: occurrence.requirement_operator,
                            terminal_machine,
                            terminal_operation: operation,
                            // `IeeeFloatCompare` carries the authored comparison
                            // itself, so the emitted operation always reads the
                            // authored operand order.
                            operand_order: LoweredSelectedComparisonOperandOrder::Authored,
                        },
                    );
                    (
                        OperationKind::IeeeFloatCompare {
                            comparison,
                            left,
                            right,
                        },
                        false,
                    )
                }
                SelectedComparisonMeaning::Integer {
                    comparison,
                    operand_order,
                    negated,
                    ..
                } => {
                    operations.selected_comparisons.push(
                        lowered_psi::LoweredSelectedComparisonOccurrence {
                            operator_use: occurrence.operator_use,
                            application_site: occurrence.application_site,
                            requirement_operator: occurrence.requirement_operator,
                            terminal_machine,
                            terminal_operation: operation,
                            operand_order,
                        },
                    );
                    // Both operands are already completed above in authored
                    // evaluation order; only the emitted operation's
                    // positional roster follows the recorded mapping, exactly
                    // as the checked stage normalizes a builtin `>` or `>=`.
                    let (first, second) = match operand_order {
                        LoweredSelectedComparisonOperandOrder::Authored => (left, right),
                        LoweredSelectedComparisonOperandOrder::Swapped => (right, left),
                    };
                    (comparison.operation(first, second), negated)
                }
            };
            operations.push(Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: operation,
                result: scalar_result_declaration(id, ScalarType::Boolean),
                kind,
            });
            if !negated {
                return Ok(id);
            }
            // `!=` completes as the emitted equality's negation. The crash
            // contract stays on the comparison the occurrence names, which is
            // the operation owning the scalar operands the formal telescope
            // binds; this negation only carries its Boolean result forward.
            let negated_id = value_id(*next_value_identity);
            *next_value_identity =
                next_value_identity
                    .checked_add(1)
                    .ok_or(LoweringError::Unsupported(
                        "comparison negation value identity overflow",
                    ))?;
            let negation = operations.allocate();
            operations.push(Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: negation,
                result: scalar_result_declaration(negated_id, ScalarType::Boolean),
                kind: OperationKind::BooleanNot { operand: id },
            });
            Ok(negated_id)
        }
        LoweredScalarBinding::Expression(expression) => Ok(emit_direct_expression(
            expression,
            parameters,
            next_value_identity,
            operations,
        )),
        LoweredScalarBinding::DirectCall(call) => calls::emit_scalar_call_binding(
            identities,
            checked,
            call,
            parameters,
            frontier,
            caller_erased_formals,
            next_value_identity,
            operations,
            call_emission,
        ),
        LoweredScalarBinding::BoundaryCall(call) => boundary_calls::emit_boundary_call_binding(
            call,
            parameters,
            frontier,
            next_value_identity,
            operations,
            call_emission,
        ),
        LoweredScalarBinding::DynamicCall(call) => dynamic_calls::emit_dynamic_call_binding(
            call,
            parameters,
            frontier,
            next_value_identity,
            operations,
            call_emission,
        ),
    }?;
    let completion = match binding {
        LoweredScalarBinding::DirectCall(call) => {
            Some((call.source_coordinate, &call.loan_completions))
        }
        LoweredScalarBinding::BoundaryCall(call) => {
            Some((call.source_coordinate, &call.loan_completions))
        }
        LoweredScalarBinding::DynamicCall(call) => {
            Some((call.source_coordinate, &call.loan_completions))
        }
        _ => None,
    };
    if let Some((coordinate, completions)) = completion {
        crate::emission::loan_completions::emit(
            completions,
            coordinate,
            operations,
            call_emission,
        )?;
    }
    Ok(result)
}
