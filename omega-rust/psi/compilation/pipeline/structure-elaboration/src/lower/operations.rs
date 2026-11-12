/*
The operation family of lowering: expressions as scalar operations emitted in
evaluation order, and the `OperationSafety` obligation of each partial one.
A record-typed expression is storage and goes to `places`.
*/

use semantic_vocabulary::{ObligationId, OperationId, ScalarType, ValueId};
use terminal_check_tables::obligations::ObligationKind;
use terminal_check_tables::values::ValueOrigin;
use terminal_check_tables::{Checked, ProgramPoint};
use terminal_psi::{Operation, OperationKind, OperationResult};

use super::places::Bound;
use super::{Lowering, Value, declaration};
use crate::SourceOrigin;
use crate::body::{Expression, ExpressionKind, Strict, ValueType};

impl Lowering<'_> {
    pub(super) fn operation_identity(&mut self, source: SourceOrigin) -> OperationId {
        self.sources.operations.push(source)
    }

    /// The obligation guarding `operation`, created at the point before it.
    fn safety(&mut self, operation: OperationId, source: SourceOrigin) -> Checked<ObligationId> {
        let before = self.here()?;
        Ok(self.obligation(
            ObligationKind::OperationSafety { operation },
            before,
            source,
        ))
    }

    pub(super) fn emit(
        &mut self,
        operation: OperationId,
        result: Option<ScalarType>,
        kind: OperationKind,
    ) -> Checked<Value> {
        let (result, value) = match result {
            Some(scalar_type) => {
                let value = self.value(scalar_type, ValueOrigin::Operation(operation));
                (
                    OperationResult::Scalar(declaration(value, scalar_type)),
                    Value::Scalar(value),
                )
            }
            None => (OperationResult::Unit, Value::Unit),
        };
        self.draft()?
            .operations
            .push(Operation::unsuspended(operation, result, kind));
        self.point = Some(ProgramPoint::AfterOperation(operation));
        Ok(value)
    }

    pub(super) fn scalar(&mut self, expression: &Expression) -> Checked<ValueId> {
        match self.expression(expression)? {
            Value::Scalar(value) => Ok(value),
            Value::Record(_) => Err(self.internal("a record is used as a scalar value")),
            Value::Unit => Err(self.internal("a Unit call is used as a value")),
        }
    }

    fn result_type(&self, expression: &Expression) -> Checked<ScalarType> {
        expression
            .result
            .and_then(ValueType::scalar)
            .ok_or_else(|| self.internal("an operand has no scalar type"))
    }

    pub(super) fn expression(&mut self, expression: &Expression) -> Checked<Value> {
        let source = expression.source;
        // A record-typed expression names storage; its value is a root that
        // holds it. Everything below is scalar or Unit.
        let result = match expression.result {
            Some(ValueType::Record(_)) => return self.record(expression).map(Value::Record),
            Some(ValueType::Scalar(scalar_type)) => Some(scalar_type),
            // A value of a rigid type is only ever named: no operation
            // produces one, so no result row is minted for it here.
            Some(ValueType::Rigid(_)) => {
                let ExpressionKind::Slot(slot) = &expression.kind else {
                    return Err(self.internal("an operation over a value of a rigid type"));
                };
                return match *self.slot_mut(*slot)? {
                    Some(Bound::Scalar(value)) => Ok(Value::Scalar(value)),
                    _ => Err(self.internal("a rigid slot is not bound to a value")),
                };
            }
            None => None,
        };
        match &expression.kind {
            ExpressionKind::Integer(value) => {
                let operation = self.operation_identity(source);
                self.emit(
                    operation,
                    result,
                    OperationKind::IntegerConstant { value: *value },
                )
            }
            ExpressionKind::Boolean(value) => {
                let operation = self.operation_identity(source);
                self.emit(
                    operation,
                    result,
                    OperationKind::BooleanConstant { value: *value },
                )
            }
            ExpressionKind::Slot(slot) => {
                let bound = *self.slot_mut(*slot)?;
                match bound {
                    Some(Bound::Scalar(value)) => Ok(Value::Scalar(value)),
                    Some(Bound::Home(home)) => {
                        let scalar_type = self.result_type(expression)?;
                        self.read_home(home, scalar_type, source)
                    }
                    Some(Bound::Record(_)) => Err(self.internal("a record slot has a scalar type")),
                    None => Err(self.internal("a slot is read before it is bound")),
                }
            }
            ExpressionKind::Field { base, field } => {
                let scalar_type = self.result_type(expression)?;
                self.read_field(base, *field, scalar_type, source)
            }
            ExpressionKind::Payload { base, case, field } => {
                let scalar_type = self.result_type(expression)?;
                self.read_payload(base, *case, *field, scalar_type, source)
            }
            ExpressionKind::IsCase { subject, case } => {
                self.case_membership(subject, *case, source)
            }
            ExpressionKind::Match {
                subject,
                arms,
                default,
            } => {
                let scalar_type = self.result_type(expression)?;
                self.match_value(subject, arms, default.as_deref(), scalar_type, source)
            }
            ExpressionKind::Record { .. } | ExpressionKind::Case { .. } => {
                Err(self.internal("a data literal has a scalar type"))
            }
            ExpressionKind::Not(operand) => {
                let operand = self.scalar(operand)?;
                let operation = self.operation_identity(source);
                self.emit(operation, result, OperationKind::BooleanNot { operand })
            }
            ExpressionKind::BitwiseNot(operand) => {
                let operand = self.scalar(operand)?;
                let operation = self.operation_identity(source);
                self.emit(
                    operation,
                    result,
                    OperationKind::IntegerBitwiseNot { operand },
                )
            }
            ExpressionKind::Strict {
                operation,
                left: left_expression,
                right,
            } => {
                let left = self.scalar(left_expression)?;
                let right = self.scalar(right)?;
                let operand_type = self.result_type(left_expression)?;
                self.strict(*operation, operand_type, left, right, result, source)
            }
            ExpressionKind::ShortCircuit {
                conjunction,
                left,
                right,
            } => self.short_circuit(*conjunction, left, right),
            ExpressionKind::Call { callee, arguments } => {
                self.call(*callee, arguments, expression.result, source)
            }
        }
    }

    fn strict(
        &mut self,
        strict: Strict,
        operand_type: ScalarType,
        left: ValueId,
        right: ValueId,
        result: Option<ScalarType>,
        source: SourceOrigin,
    ) -> Checked<Value> {
        let operation = self.operation_identity(source);
        let kind = match strict {
            Strict::Add => OperationKind::ExactIntegerAdd {
                left,
                right,
                obligation: self.safety(operation, source)?,
            },
            Strict::Subtract => OperationKind::ExactIntegerSubtract {
                left,
                right,
                obligation: self.safety(operation, source)?,
            },
            Strict::Multiply => OperationKind::ExactIntegerMultiply {
                left,
                right,
                obligation: self.safety(operation, source)?,
            },
            Strict::Divide => OperationKind::ExactIntegerDivide {
                left,
                right,
                obligation: self.safety(operation, source)?,
            },
            Strict::Remainder => OperationKind::ExactIntegerRemainder {
                left,
                right,
                obligation: self.safety(operation, source)?,
            },
            Strict::ShiftLeft => OperationKind::ExactIntegerShiftLeft {
                value: left,
                count: right,
                obligation: self.safety(operation, source)?,
            },
            Strict::ShiftRight => OperationKind::ExactIntegerShiftRight {
                value: left,
                count: right,
                obligation: self.safety(operation, source)?,
            },
            Strict::BitwiseAnd => OperationKind::IntegerBitwiseAnd { left, right },
            Strict::BitwiseOr => OperationKind::IntegerBitwiseOr { left, right },
            Strict::BitwiseXor => OperationKind::IntegerBitwiseXor { left, right },
            Strict::Equal | Strict::NotEqual => match operand_type {
                ScalarType::Boolean => OperationKind::BooleanEqual { left, right },
                ScalarType::Integer(_) => OperationKind::IntegerEqual { left, right },
                ScalarType::IeeeFloat(_) => {
                    return Err(self.internal("a float equality reached lowering"));
                }
            },
            Strict::Less => OperationKind::IntegerLessThan { left, right },
            Strict::LessOrEqual => OperationKind::IntegerLessOrEqual { left, right },
            // `a > b` is `b < a`: both operands are already evaluated in
            // authored order, so swapping the operand identities changes no
            // evaluation.
            Strict::Greater => OperationKind::IntegerLessThan {
                left: right,
                right: left,
            },
            Strict::GreaterOrEqual => OperationKind::IntegerLessOrEqual {
                left: right,
                right: left,
            },
        };
        if strict != Strict::NotEqual {
            return self.emit(operation, result, kind);
        }
        // `!=` is the negation of the equality: Terminal Psi has no
        // inequality operation.
        let Value::Scalar(equal) = self.emit(operation, Some(ScalarType::Boolean), kind)? else {
            return Err(self.internal("an equality produced no value"));
        };
        let negation = self.operation_identity(source);
        self.emit(
            negation,
            result,
            OperationKind::BooleanNot { operand: equal },
        )
    }
}
