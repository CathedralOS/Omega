/*
The operation family of lowering: expressions as scalar operations emitted in
evaluation order, and the `OperationSafety` obligation of each partial one.
A record-typed expression is storage and goes to `places`.
*/

use crate::body::Qualification;
use semantic_vocabulary::{ObligationId, OperationId, ScalarType, ValueId};
use terminal_check_tables::obligations::ObligationKind;
use terminal_check_tables::values::ValueOrigin;
use terminal_check_tables::{Checked, Construct, ProgramPoint};
use terminal_psi::{Operation, OperationKind, OperationResult};

use super::places::{Bound, Witness};
use super::{Evaluation, Lowering, Reached, Value, declaration};
use crate::body::{Expression, ExpressionKind, Policy, Strict, ValueType};
use crate::{SourceOrigin, Unserved, refuse};

impl Lowering<'_> {
    pub(super) fn operation_identity(&mut self, source: SourceOrigin) -> OperationId {
        let id = self.sources.operations.push(source);
        let location = self.operation_locations.push(None);
        debug_assert_eq!(id, location);
        id
    }

    /// The obligation guarding `operation`, created at the point before it.
    fn safety(&mut self, operation: OperationId, source: SourceOrigin) -> Checked<ObligationId> {
        let before = self.here()?;
        Ok(self.obligation(
            ObligationKind::OperationSafety {
                site: terminal_check_tables::obligations::ExecutionSite::Operation(operation),
            },
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
        self.emit_operation(operation, result, kind)?;
        Ok(value)
    }

    pub(super) fn emit_operation(
        &mut self,
        operation: OperationId,
        result: OperationResult,
        kind: OperationKind,
    ) -> Checked<()> {
        let block = self.current()?;
        let ordinal = self.draft()?.operations.len();
        let location = self.operation_locations.get_mut(operation).ok_or_else(|| {
            terminal_check_tables::Refusal {
                reason: terminal_check_tables::RefusalReason::ForeignIdentity,
                machine: self.id,
                concerns: Construct::Operation(operation),
                source_span: None,
                detail: "an operation has no allocated source coordinate".to_owned(),
            }
        })?;
        if location.replace((block, ordinal)).is_some() {
            return Err(self.internal("an operation was emitted twice"));
        }
        self.draft()?
            .operations
            .push(Operation::unsuspended(operation, result, kind));
        self.point = Some(ProgramPoint::AfterOperation(operation));
        Ok(())
    }

    pub(super) fn scalar(&mut self, expression: &Expression) -> Evaluation<ValueId> {
        match continuing!(self.expression(expression)) {
            Value::Scalar(value) => Ok(Reached::Value(value)),
            Value::Record(_) => Err(self.internal("a record is used as a scalar value")),
            Value::Unit => Err(self.internal("a Unit call is used as a value")),
        }
    }

    /// The term an erased position names: a literal, a relevant scalar slot's
    /// current value, or the term another erased slot names. No operation is
    /// emitted; contracts.md, "Explicit erased bindings": the binding
    /// "contributes no runtime field, address, read, or cleanup."
    pub(super) fn witness(&mut self, expression: &Expression) -> Checked<Witness> {
        match (&expression.kind, expression.result) {
            (
                ExpressionKind::Integer(value),
                Some(ValueType::Scalar(ScalarType::Integer(t), Qualification::NONE)),
            ) => Ok(Witness::Integer(t, *value)),
            (ExpressionKind::Boolean(value), _) => Ok(Witness::Boolean(*value)),
            (ExpressionKind::Slot(slot), Some(ValueType::Scalar(scalar_type, _))) => {
                match *self.slot_mut(*slot)? {
                    Some(Bound::Scalar(value)) => Ok(Witness::Value(value, scalar_type)),
                    Some(Bound::Erased(witness)) => Ok(witness),
                    Some(Bound::Home(_) | Bound::Record(_)) | None => Err(refuse(
                        self.id,
                        self.current.map_or(Construct::Machine, Construct::Block),
                        Unserved::Expression("erased term that reads storage"),
                    )),
                }
            }
            _ => Err(refuse(
                self.id,
                self.current.map_or(Construct::Machine, Construct::Block),
                Unserved::Expression("erased term other than a literal or a binding"),
            )),
        }
    }

    fn result_type(&self, expression: &Expression) -> Checked<ScalarType> {
        expression
            .result
            .and_then(ValueType::scalar)
            .ok_or_else(|| self.internal("an operand has no scalar type"))
    }

    pub(super) fn expression(&mut self, expression: &Expression) -> Evaluation<Value> {
        let evaluated = self.expression_inner(expression)?;
        if let Reached::Value(Value::Scalar(value)) = evaluated
            && let Some(ValueType::Scalar(_, qualification)) = expression.result
        {
            self.qualify(value, qualification)?;
        }
        Ok(evaluated)
    }

    fn expression_inner(&mut self, expression: &Expression) -> Evaluation<Value> {
        let source = expression.source;
        // A record-typed expression names storage; its value is a root that
        // holds it. Everything below is scalar or Unit.
        let result = match expression.result {
            Some(ValueType::Record(_, _, _)) => {
                return self
                    .record(expression)
                    .map(|reached| reached.map(Value::Record));
            }
            Some(ValueType::Scalar(scalar_type, _)) => Some(scalar_type),
            // A declared call may transport an opaque result, as a slot
            // transports its formal. Neither grants operations of its stand-in.
            Some(ValueType::Rigid(_)) => {
                if !matches!(
                    expression.kind,
                    ExpressionKind::Slot(_) | ExpressionKind::Call { .. }
                ) {
                    return Err(self.internal("an operation over a value of a rigid type"));
                }
                None
            }
            None => None,
        };
        match &expression.kind {
            ExpressionKind::Atomic(operation) => match operation {
                crate::body::AtomicOperation::Access {
                    place,
                    event,
                    operands,
                } => self.atomic_access(place, *event, operands, result, source),
                crate::body::AtomicOperation::Fence(ordering) => {
                    let operation = self.operation_identity(source);
                    self.emit(
                        operation,
                        None,
                        OperationKind::AtomicFence {
                            ordering: *ordering,
                        },
                    )
                    .map(Reached::Value)
                }
            },
            ExpressionKind::Integer(value) => {
                let operation = self.operation_identity(source);
                self.emit(
                    operation,
                    result,
                    OperationKind::IntegerConstant { value: *value },
                )
                .map(Reached::Value)
            }
            ExpressionKind::Float(value) => {
                let operation = self.operation_identity(source);
                self.emit(
                    operation,
                    result,
                    OperationKind::IeeeFloatConstant { value: *value },
                )
                .map(Reached::Value)
            }
            ExpressionKind::Boolean(value) => {
                let operation = self.operation_identity(source);
                self.emit(
                    operation,
                    result,
                    OperationKind::BooleanConstant { value: *value },
                )
                .map(Reached::Value)
            }
            ExpressionKind::Slot(slot) => {
                let bound = *self.slot_mut(*slot)?;
                match bound {
                    Some(Bound::Scalar(value)) => Ok(Reached::Value(Value::Scalar(value))),
                    Some(Bound::Home(home)) => {
                        let scalar_type = self.result_type(expression)?;
                        self.read_home(home, scalar_type, source)
                            .map(Reached::Value)
                    }
                    Some(Bound::Record(_)) => Err(self.internal("a record slot has a scalar type")),
                    // The reader refuses a runtime read of an erased
                    // binding, so reaching one here is a broken body.
                    Some(Bound::Erased(_)) => {
                        Err(self.internal("an erased binding is read as a runtime value"))
                    }
                    None => Err(self.internal("a slot is read before it is bound")),
                }
            }
            ExpressionKind::Index { base, index } => {
                let scalar_type = self.result_type(expression)?;
                self.read_index(base, index, scalar_type, source)
            }
            ExpressionKind::Referent(_) => {
                let scalar_type = self.result_type(expression)?;
                self.read_referent(expression, scalar_type)
            }
            ExpressionKind::Field { base, field } => {
                let scalar_type = self.result_type(expression)?;
                self.read_field(base, *field, scalar_type, source)
            }
            ExpressionKind::CollectionMeasure { base, measure } => {
                self.measure_collection(base, *measure, source)
            }
            ExpressionKind::ByteSequencePredicate { base, predicate } => {
                self.byte_sequence_predicate(base, *predicate, source)
            }
            ExpressionKind::ScalarApplication { application } => {
                let scalar_type = self.result_type(expression)?;
                let operation = self.operation_identity(source);
                self.emit(
                    operation,
                    Some(scalar_type),
                    OperationKind::ReadScalarApplication {
                        application: *application,
                    },
                )
                .map(Reached::Value)
            }
            ExpressionKind::ByteElement { base, index } => {
                self.read_byte_element(base, index, source)
            }
            ExpressionKind::CollectionSubslice { .. } | ExpressionKind::ArrayView { .. } => {
                Err(self.internal("a byte descriptor is used as a scalar"))
            }
            ExpressionKind::Payload { base, case, field } => {
                let scalar_type = self.result_type(expression)?;
                self.read_payload(base, *case, *field, scalar_type, source)
            }
            ExpressionKind::IsCase { subject, case } => {
                self.case_membership(subject, *case, source)
            }
            ExpressionKind::Match {
                capture,
                subject,
                arms,
                default,
            } => {
                let scalar_type = self.result_type(expression)?;
                self.match_value(
                    subject,
                    arms,
                    default.as_deref(),
                    ValueType::plain(scalar_type),
                    source,
                    *capture,
                )
            }
            ExpressionKind::SavedSubject(_)
            | ExpressionKind::ByteLiteral(_)
            | ExpressionKind::Array { .. }
            | ExpressionKind::Record { .. }
            | ExpressionKind::Case { .. } => Err(self.internal("a data literal has a scalar type")),
            ExpressionKind::Borrow { .. } => {
                Err(self.internal("a loan outside a call's arguments"))
            }
            ExpressionKind::Not(operand) => {
                let operand = continuing!(self.scalar(operand));
                let operation = self.operation_identity(source);
                self.emit(operation, result, OperationKind::BooleanNot { operand })
                    .map(Reached::Value)
            }
            ExpressionKind::BitwiseNot(operand) => {
                let operand = continuing!(self.scalar(operand));
                let operation = self.operation_identity(source);
                self.emit(
                    operation,
                    result,
                    OperationKind::IntegerBitwiseNot { operand },
                )
                .map(Reached::Value)
            }
            ExpressionKind::Cast(operand_expression) => {
                let operand = continuing!(self.scalar(operand_expression));
                let (ScalarType::Integer(from), Some(ScalarType::Integer(to))) =
                    (self.result_type(operand_expression)?, result)
                else {
                    return Err(self.internal("a cast between carriers that are not integers"));
                };
                let operation = self.operation_identity(source);
                // Widening is total. Any other conversion is exact only
                // where the value is representable, which is its obligation.
                let kind = if from.can_widen_to(to) {
                    OperationKind::IntegerWiden { operand }
                } else if from != to && from.can_exact_cast_to(to) {
                    OperationKind::IntegerExactCast {
                        operand,
                        obligation: self.safety(operation, source)?,
                    }
                } else {
                    return Err(self.internal("a cast with no exact conversion"));
                };
                self.emit(operation, result, kind).map(Reached::Value)
            }
            // The operand's value under another policy: nothing is computed.
            ExpressionKind::Requalify(operand) => {
                let scalar_type = self.result_type(operand)?;
                let operand = continuing!(self.scalar(operand));
                self.establish_predicates(
                    operand,
                    scalar_type,
                    expression
                        .result
                        .ok_or_else(|| self.internal("a cast has no result type"))?
                        .qualification(),
                    &[],
                    None,
                    source,
                )
                .map(|value| Reached::Value(Value::Scalar(value)))
            }
            ExpressionKind::Establish {
                operand,
                clauses,
                stated,
            } => {
                let scalar_type = self.result_type(operand)?;
                let operand = continuing!(self.scalar(operand));
                self.establish_predicates(
                    operand,
                    scalar_type,
                    expression
                        .result
                        .ok_or_else(|| self.internal("a cast has no result type"))?
                        .qualification(),
                    clauses,
                    Some(stated),
                    source,
                )
                .map(|value| Reached::Value(Value::Scalar(value)))
            }
            ExpressionKind::Strict {
                left: left_expression,
                right,
                ..
            } => {
                // The one strict operator over storage: `==` of two byte
                // views, which the reader forms for no other operand type.
                if matches!(left_expression.result, Some(ValueType::Record(..))) {
                    return self.byte_equal(left_expression, right, source);
                }
                let left = continuing!(self.scalar(left_expression));
                let right = continuing!(self.scalar(right));
                self.strict_expression(expression, left, right)
                    .map(Reached::Value)
            }
            ExpressionKind::ShortCircuit {
                conjunction,
                left,
                right,
            } => self.short_circuit(*conjunction, left, right),
            ExpressionKind::Call {
                callee,
                required_contract,
                static_reach_binding,
                source_call,
                arguments,
                designated,
            } => self.call(
                *callee,
                *required_contract,
                arguments,
                *designated,
                expression.result,
                (source, *source_call, *static_reach_binding),
                expression.call_context.unwrap_or_default(),
            ),
        }
    }

    /// One arithmetic or shift operation under its selected policy.
    ///
    /// numeric_values.md, "Overflow policies": "Wrapping reduces
    /// representable-range overflow modulo the selected carrier; Saturating
    /// clamps it to carrier bounds", and "An overflow policy does not invent
    /// a result for undefined operands such as an integer zero divisor". So
    /// a Wrapping or Saturating add, subtract and multiply is total and owes
    /// nothing, a divide and a remainder still owe their divisor, and a
    /// Wrapping shift reduces its count and is total. What each one owes is
    /// Terminal semantics' to say (`integer_policy_bridge`); this names the
    /// operation and mints the obligation the operation carries. A
    /// Saturating shift owes the Exact count proof under an operation this
    /// lowering does not emit, and is refused.
    fn policy_operation(
        &mut self,
        strict: Strict,
        policy: Policy,
        left: ValueId,
        right: ValueId,
        result: Option<ScalarType>,
        source: SourceOrigin,
    ) -> Checked<Value> {
        // Wrapping reduces modulo `2^N` and saturating clamps to the
        // carrier's range (`numeric_values.md`, "Width-indexed integers");
        // at a parameter width no term states either yet.
        if result
            .and_then(|scalar| scalar.as_integer())
            .is_some_and(|integer| integer.bits().is_none())
        {
            return Err(refuse(
                self.id,
                self.current.map_or(Construct::Machine, Construct::Block),
                Unserved::Expression("policy arithmetic at a parameter width"),
            ));
        }
        let operation = self.operation_identity(source);
        // The total operations are the ones a clause's term names too.
        if let Some(kind) = crate::policy::total(strict, policy, left, right) {
            return self.emit(operation, result, kind);
        }
        if policy == Policy::Trapping {
            return self.trapping(operation, strict, left, right, result, source);
        }
        let kind = match (strict, policy) {
            (Strict::Divide, Policy::Wrapping) => OperationKind::WrappingIntegerDivide {
                left,
                right,
                obligation: self.safety(operation, source)?,
            },
            (Strict::Divide, Policy::Saturating) => OperationKind::SaturatingIntegerDivide {
                left,
                right,
                obligation: self.safety(operation, source)?,
            },
            (Strict::Remainder, Policy::Wrapping) => OperationKind::WrappingIntegerRemainder {
                left,
                right,
                obligation: self.safety(operation, source)?,
            },
            (Strict::Remainder, Policy::Saturating) => OperationKind::SaturatingIntegerRemainder {
                left,
                right,
                obligation: self.safety(operation, source)?,
            },
            (Strict::ShiftLeft | Strict::ShiftRight, Policy::Saturating) => {
                return Err(refuse(
                    self.id,
                    self.current.map_or(Construct::Machine, Construct::Block),
                    Unserved::Expression("saturating shift"),
                ));
            }
            // The reader gives no other operation a policy-qualified result,
            // `policy::total` named every total one above, and a Trapping
            // operation is `trapping`'s.
            (_, Policy::Trapping)
            | (
                Strict::Add
                | Strict::Subtract
                | Strict::Multiply
                | Strict::ShiftLeft
                | Strict::ShiftRight
                | Strict::BitwiseAnd
                | Strict::BitwiseOr
                | Strict::BitwiseXor
                | Strict::Equal
                | Strict::NotEqual
                | Strict::Less
                | Strict::LessOrEqual
                | Strict::Greater
                | Strict::GreaterOrEqual,
                _,
            ) => return Err(self.internal("a policy on an operation that has none")),
        };
        self.emit(operation, result, kind)
    }

    /// One Trapping operation: its own `Trap` site.
    ///
    /// numeric_values.md, "Overflow policies": "Terminal carries it as one
    /// `TrappingInteger` operation per primitive ... that is its own `Trap`
    /// site", whose predicate is the exact primitive's ("result outside the
    /// carrier, a zero divisor or signed `MIN / -1` ..., and a shift count
    /// outside `0..width`"). The site owes coverage by the machine's
    /// published `Trap` route like a `crash Trap;` (effects.md, "Guarded
    /// crashes"); path facts that disprove the predicate discharge it with
    /// no route. What the predicate is and what the result equals on normal
    /// return are Terminal semantics' to say.
    fn trapping(
        &mut self,
        operation: OperationId,
        strict: Strict,
        left: ValueId,
        right: ValueId,
        result: Option<ScalarType>,
        source: SourceOrigin,
    ) -> Checked<Value> {
        use terminal_psi::TrappingIntegerOperation as Trapping;
        let primitive = match strict {
            Strict::Add => Trapping::Add { left, right },
            Strict::Subtract => Trapping::Subtract { left, right },
            Strict::Multiply => Trapping::Multiply { left, right },
            Strict::Divide => Trapping::Divide { left, right },
            Strict::Remainder => Trapping::Remainder { left, right },
            Strict::ShiftLeft => Trapping::ShiftLeft {
                value: left,
                count: right,
            },
            Strict::ShiftRight => Trapping::ShiftRight {
                value: left,
                count: right,
            },
            Strict::BitwiseAnd
            | Strict::BitwiseOr
            | Strict::BitwiseXor
            | Strict::Equal
            | Strict::NotEqual
            | Strict::Less
            | Strict::LessOrEqual
            | Strict::Greater
            | Strict::GreaterOrEqual => {
                return Err(self.internal("a Trapping policy on an operation that has none"));
            }
        };
        let before = self.here()?;
        self.obligation(
            ObligationKind::CrashCoverage {
                site: terminal_check_tables::obligations::CrashSite::Operation(operation),
                cause: semantic_vocabulary::CrashCause::Trap,
            },
            before,
            source,
        );
        self.emit(
            operation,
            result,
            OperationKind::TrappingInteger {
                operation: primitive,
            },
        )
    }

    /// Body expressions and ranking observations use the same selected
    /// arithmetic policy. Reconstructing an observation must not turn a
    /// wrapping or saturating operation into exact arithmetic.
    pub(super) fn strict_expression(
        &mut self,
        expression: &Expression,
        left: ValueId,
        right: ValueId,
    ) -> Checked<Value> {
        let ExpressionKind::Strict {
            operation,
            policy,
            left: operand,
            ..
        } = &expression.kind
        else {
            return Err(self.internal("a strict operation has no expression selection"));
        };
        let result = match expression.result {
            Some(ValueType::Scalar(scalar_type, _)) => Some(scalar_type),
            None => None,
            _ => return Err(self.internal("a strict operation has a non-scalar result")),
        };
        if let Some(policy) = policy {
            self.policy_operation(*operation, *policy, left, right, result, expression.source)
        } else {
            self.strict(
                *operation,
                self.result_type(operand)?,
                left,
                right,
                result,
                expression.source,
            )
        }
    }

    pub(super) fn strict(
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
