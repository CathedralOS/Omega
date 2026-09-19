//! Emit an ordered scalar binding into the caller's operation buffer.
//!
//! Expressions emit leaves, calls preserve argument/control order, and the buffer
//! retains identities and source custody. Module-wide proof completion is separate.

pub(crate) mod boolean;
pub(crate) mod buffer;
pub(crate) mod calls;
pub(crate) mod expressions;
pub(crate) mod integer;

use crate::emission::expression_validation::direct_expression_contains_short_circuit;
use crate::emission::selected_comparison::SelectedComparisonMeaning;
use crate::lowering_error::LoweringError;
use crate::lowering_error::unsupported;
use crate::terminal_identities::value_id;
pub(crate) use boolean::emit_boolean_expression;
use buffer::OperationBuffer;
pub(crate) use calls::emit_staged_scalar_call_binding;
use calls::{CallEmissionContext, LoweredDirectCallBinding};
use expressions::LoweredDirectExpression;
pub(crate) use expressions::{emit_byte_length, emit_direct_expression};
use lowered_psi::{
    LoweredSelectedIntegerComparisonOperandOrder, LoweredSelectedIntegerComparisonOperation,
};
use semantic_vocabulary::{QualifiedScalarType, ScalarType, ValueId};
use terminal_psi::{
    Operation, OperationKind, OperationResult, StructuralMultiplicity, StructuralOperationResult,
    ValueDeclaration,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoweredScalarBinding {
    SelectedComparison {
        occurrence: crate::emission::selected_comparison::SelectedComparison,
        source_machine: symbols::SymbolHandle,
        left: LoweredDirectExpression,
        right: LoweredDirectExpression,
    },
    Expression(LoweredDirectExpression),
    DirectCall(LoweredDirectCallBinding),
    /// Retain the evaluated RHS while committing its separate Unit store effect.
    StoredValue {
        value: LoweredDirectExpression,
        destination: crate::emission::store_destination::StoreDestination,
    },
}

impl LoweredScalarBinding {
    pub(crate) fn value_type(
        &self,
        parameters: &[QualifiedScalarType],
    ) -> Result<QualifiedScalarType, LoweringError> {
        match self {
            Self::SelectedComparison { .. } => Ok(ScalarType::Boolean.into()),
            Self::Expression(expression)
            | Self::StoredValue {
                value: expression, ..
            } => expression.value_type(parameters),
            Self::DirectCall(call) => Ok(call.result_type),
        }
    }
    pub(crate) const fn scalar_type(&self) -> ScalarType {
        match self {
            Self::SelectedComparison { .. } => ScalarType::Boolean,
            Self::Expression(expression) => expression.scalar_type(),
            Self::DirectCall(call) => call.result_type.scalar_type,
            Self::StoredValue { value, .. } => value.scalar_type(),
        }
    }
}

pub(crate) fn emit_scalar_binding(
    binding: &LoweredScalarBinding,
    parameters: &[ValueDeclaration],
    caller_erased_formals: &[ValueDeclaration],
    next_value_identity: &mut u64,
    operations: &mut OperationBuffer,
    call_emission: &mut CallEmissionContext<'_>,
) -> Result<ValueId, LoweringError> {
    match binding {
        LoweredScalarBinding::SelectedComparison {
            occurrence,
            source_machine,
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
            let terminal_machine = call_emission
                .machine_ids
                .iter()
                .find_map(|(source, machine)| (*source == *source_machine).then_some(*machine))
                .ok_or(LoweringError::Unsupported(
                    "comparison source machine has no Terminal identity",
                ))?;
            // The occurrence row is recorded beside the exact operation it
            // names: a selected use that reached emission without one would
            // be indistinguishable from a builtin comparison downstream.
            let (kind, negated) = match occurrence.meaning {
                SelectedComparisonMeaning::IeeeFloat { comparison, format } => {
                    operations.selected_ieee_float_comparisons.push(
                        lowered_psi::LoweredSelectedIeeeFloatComparisonOccurrence {
                            operator_use: occurrence.operator_use,
                            application_site: occurrence.application_site,
                            requirement_operator: occurrence.requirement_operator,
                            provider_plan_report_fingerprint: occurrence
                                .provider_plan_report_fingerprint,
                            provider_plan_commitment: occurrence.provider_plan_commitment,
                            comparison,
                            format,
                            terminal_machine,
                            terminal_operation: operation,
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
                    integer_type,
                } => {
                    operations.selected_integer_comparisons.push(
                        lowered_psi::LoweredSelectedIntegerComparisonOccurrence {
                            operator_use: occurrence.operator_use,
                            application_site: occurrence.application_site,
                            requirement_operator: occurrence.requirement_operator,
                            provider_plan_report_fingerprint: occurrence
                                .provider_plan_report_fingerprint,
                            provider_plan_commitment: occurrence.provider_plan_commitment,
                            comparison,
                            operand_order,
                            negated,
                            integer_type,
                            terminal_machine,
                            terminal_operation: operation,
                        },
                    );
                    // Both operands are already completed above in authored
                    // evaluation order; only the emitted operation's
                    // positional roster follows the recorded mapping, exactly
                    // as the checked stage normalizes a builtin `>` or `>=`.
                    let (first, second) = match operand_order {
                        LoweredSelectedIntegerComparisonOperandOrder::Authored => (left, right),
                        LoweredSelectedIntegerComparisonOperandOrder::Swapped => (right, left),
                    };
                    let kind = match comparison {
                        LoweredSelectedIntegerComparisonOperation::Equal => {
                            OperationKind::IntegerEqual {
                                left: first,
                                right: second,
                            }
                        }
                        LoweredSelectedIntegerComparisonOperation::LessThan => {
                            OperationKind::IntegerLessThan {
                                left: first,
                                right: second,
                            }
                        }
                        LoweredSelectedIntegerComparisonOperation::LessOrEqual => {
                            OperationKind::IntegerLessOrEqual {
                                left: first,
                                right: second,
                            }
                        }
                    };
                    (kind, negated)
                }
            };
            operations.push(Operation {
                static_reach_binding: None,
                id: operation,
                result: OperationResult::Scalar(ValueDeclaration {
                    id,
                    scalar_type: ScalarType::Boolean,
                    qualifications: Default::default(),
                }),
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
                id: negation,
                result: OperationResult::Scalar(ValueDeclaration {
                    id: negated_id,
                    scalar_type: ScalarType::Boolean,
                    qualifications: Default::default(),
                }),
                kind: OperationKind::BooleanNot { operand: id },
            });
            Ok(negated_id)
        }
        LoweredScalarBinding::StoredValue { value, destination } => {
            use crate::emission::store_destination::StoreDestination;
            if direct_expression_contains_short_circuit(value) {
                return unsupported("primitive store requires a completed scalar value");
            }
            let value = emit_direct_expression(value, parameters, next_value_identity, operations);
            let producer = operations.allocate();
            let (result, kind) = match *destination {
                StoreDestination::Initialize {
                    place,
                    structural_type,
                } => (
                    OperationResult::Structural(StructuralOperationResult {
                        place,
                        structural_type,
                        multiplicity: StructuralMultiplicity::Unrestricted,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        claims: Vec::new(),
                    }),
                    OperationKind::EstablishPrimitiveLocal { value },
                ),
                StoreDestination::Assign { place } => (
                    OperationResult::Unit,
                    OperationKind::WriteOnlyPrimitiveStore {
                        destination: place,
                        path: Vec::new(),
                        value,
                    },
                ),
            };
            operations.push(Operation {
                static_reach_binding: None,
                id: producer,
                result,
                kind,
            });
            // The store produces no scalar. Its already evaluated RHS remains
            // available to private continuation plumbing without another read.
            Ok(value)
        }
        LoweredScalarBinding::Expression(expression) => Ok(emit_direct_expression(
            expression,
            parameters,
            next_value_identity,
            operations,
        )),
        LoweredScalarBinding::DirectCall(call) => calls::emit_scalar_call_binding(
            call,
            parameters,
            caller_erased_formals,
            next_value_identity,
            operations,
            call_emission,
        ),
    }
}
