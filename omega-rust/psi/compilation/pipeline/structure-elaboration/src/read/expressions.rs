/*
The expression family of the reader: names as slots, literals, the scalar
operators and the short-circuit operators, and whether two read expressions
are the same computation. Record literals and member paths are read in
`places`.
*/

use semantic_vocabulary::{MachineId, ScalarType};
use terminal_check_tables::{Checked, Construct};
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableNamePath, UnaryOperator,
};

use super::{Program, Slot, symbol_kind};
use crate::body::{Expression, ExpressionKind, Strict, ValueType};
use crate::{SourceOrigin, Unserved, refuse};

impl<'t> Program<'t> {
    pub(super) fn slot_of(
        &self,
        id: MachineId,
        slots: &[Slot],
        path: &TableNamePath,
    ) -> Checked<usize> {
        let typed = self.typed;
        if path.reserved_result_of.is_valid() {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Expression("reserved result outside a contract term"),
            ));
        }
        let symbol = path.resolved_symbol().ok_or_else(|| {
            refuse(
                id,
                Construct::Machine,
                if path.static_application.is_valid() {
                    Unserved::Expression("applied name used as a plain value binding")
                } else {
                    Unserved::Spelling("name without a symbol")
                },
            )
        })?;
        if typed.expression_table.name_path_members(path.members).len() != 1 {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Expression("projected name path"),
            ));
        }
        // ponytail: linear over the body's slots; bodies in the subset hold a
        // handful of locals.
        slots
            .iter()
            .rposition(|slot| slot.symbol == symbol)
            .ok_or_else(|| {
                let kind = typed.symbols.get(symbol).kind;
                refuse(
                    id,
                    Construct::Machine,
                    Unserved::Expression(symbol_kind(kind)),
                )
            })
    }

    pub(super) fn node(
        &self,
        id: MachineId,
        expression: ExpressionHandle,
    ) -> Checked<&'t ExpressionNode> {
        if !expression.is_valid() {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Expression("missing expression"),
            ));
        }
        Ok(self.typed.expression_table.expression(expression))
    }

    /// Reads the term at an erased position: an `[erased]` local's
    /// initializer or the argument for an `[erased]` formal. It is a literal
    /// or a scalar binding, relevant or erased. Anything that computes has a
    /// formation condition (contracts.md, "Total arithmetic": "Exact
    /// operations discharge representability before formation") that no
    /// operation of the lowered form would own, and is refused.
    pub(super) fn erased_term(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        handle: ExpressionHandle,
        expected: Option<ScalarType>,
    ) -> Checked<Expression> {
        let node = self.node(id, handle)?;
        let source = SourceOrigin::of(self.typed.expression_table.source_span(handle));
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Expression(what)));
        let (kind, result) = match node {
            ExpressionNode::Integer(literal) => {
                let (value, integer) = self.literal(id, literal, expected)?;
                (ExpressionKind::Integer(value), ScalarType::Integer(integer))
            }
            ExpressionNode::Boolean(value) => {
                (ExpressionKind::Boolean(*value), ScalarType::Boolean)
            }
            ExpressionNode::Name(path) => {
                let slot = self.slot_of(id, slots, path)?;
                let bound = &slots[slot];
                match bound.value_type {
                    ValueType::Scalar(scalar_type) if !bound.homed => {
                        (ExpressionKind::Slot(slot), scalar_type)
                    }
                    ValueType::Scalar(_) => return unserved("erased term that reads storage"),
                    ValueType::Qualified(..) => {
                        return unserved("erased term of a policy-qualified type");
                    }
                    ValueType::Record(_) | ValueType::Rigid(_) => {
                        return unserved("erased term of a data type");
                    }
                }
            }
            _ => return unserved("erased term other than a literal or a binding"),
        };
        Ok(Expression {
            kind,
            result: Some(ValueType::Scalar(result)),
            source,
        })
    }

    /// Reads one expression with its complete contextual type. A literal
    /// needs that context for its integer carrier or its array element and
    /// extent; the caller still checks the resulting type exactly.
    pub(super) fn expression(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        handle: ExpressionHandle,
        expected: Option<ValueType>,
    ) -> Checked<Expression> {
        let typed = self.typed;
        let node = self.node(id, handle)?;
        let source = SourceOrigin::of(typed.expression_table.source_span(handle));
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Expression(what)));
        let expected_scalar = expected.and_then(ValueType::scalar);
        let (kind, result) = match node {
            ExpressionNode::Integer(literal) => {
                let (value, integer) = self.literal(id, literal, expected_scalar)?;
                (ExpressionKind::Integer(value), ScalarType::Integer(integer))
            }
            ExpressionNode::Boolean(value) => {
                (ExpressionKind::Boolean(*value), ScalarType::Boolean)
            }
            ExpressionNode::Name(path) => {
                let slot = self.slot_of(id, slots, path)?;
                // The current checker rejects this program ("erased
                // parameter ... has no runtime value"); this route does not
                // own that rejection yet, so the closure falls back to it.
                if slots[slot].erased {
                    return unserved("runtime use of an erased binding");
                }
                return Ok(Expression {
                    kind: ExpressionKind::Slot(slot),
                    result: Some(slots[slot].value_type),
                    source,
                });
            }
            ExpressionNode::Unary(unary) => {
                let operand = self.expression(id, slots, unary.operand, expected)?;
                match (unary.operator, operand.result) {
                    (UnaryOperator::LogicalNot, Some(ValueType::Scalar(ScalarType::Boolean))) => {
                        (ExpressionKind::Not(Box::new(operand)), ScalarType::Boolean)
                    }
                    (
                        UnaryOperator::BitwiseNot,
                        Some(ValueType::Scalar(integer @ ScalarType::Integer(_))),
                    ) => (ExpressionKind::BitwiseNot(Box::new(operand)), integer),
                    _ => {
                        return Err(refuse(
                            id,
                            Construct::Machine,
                            Unserved::Typing("unary operand type"),
                        ));
                    }
                }
            }
            // A condition closed type equations decide is that Boolean.
            ExpressionNode::Binary(_) if self.static_truth(handle).is_some() => {
                let truth = self.static_truth(handle).unwrap_or_default();
                (ExpressionKind::Boolean(truth), ScalarType::Boolean)
            }
            ExpressionNode::Binary(_) if self.wholly_anonymous(handle) => {
                let (value, integer) = self.anonymous_value(id, handle, expected_scalar)?;
                (ExpressionKind::Integer(value), ScalarType::Integer(integer))
            }
            ExpressionNode::Binary(binary) => {
                // A suffixed operator (`+%`, `+|`, `+!`) is another
                // operation with another result; only the bare, Exact
                // operator is in the subset
                // (`wiki/spec/language/numeric_values.md`, "Policy
                // operators").
                if binary.policy != numerics::arithmetic::ArithmeticDomain::Exact {
                    return unserved("policy operator");
                }
                // What an operand expects from outside: an arithmetic or shift
                // result has its (value) operand's type, `&&`/`||` operands
                // are Boolean, comparison operands expect nothing. An
                // wholly anonymous operand lands in its partner's type, so
                // its partner is read first; reading order is not evaluation
                // order, which lowering takes from the operand positions.
                let hint = match binary.operator {
                    BinaryOperator::And | BinaryOperator::Or => Some(ScalarType::Boolean),
                    BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
                    | BinaryOperator::Modulo
                    | BinaryOperator::BitwiseAnd
                    | BinaryOperator::BitwiseOr
                    | BinaryOperator::BitwiseXor
                    | BinaryOperator::ShiftLeft
                    | BinaryOperator::ShiftRight => expected_scalar,
                    BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
                    | BinaryOperator::CaseMembership => None,
                };
                // `subject in S::C`. Stage 01 writes the test as
                // `CaseMembership` in a guard and as `Equal` in value
                // position; in both the right operand names a case, which is
                // what makes it a case test. `k == Kind::A` on a
                // payload-free sum is the same test (data_and_literals.md:
                // "Primitive and payload-free-sum equality is intrinsic").
                if matches!(
                    binary.operator,
                    BinaryOperator::CaseMembership | BinaryOperator::Equal
                ) && let Some(case) = self.named_case(binary.right)
                {
                    return self.case_test(id, slots, binary.left, case, source);
                }
                let (left, right) = if self.wholly_anonymous(binary.left) {
                    let right =
                        self.expression(id, slots, binary.right, hint.map(ValueType::Scalar))?;
                    let partner = right.result.and_then(ValueType::scalar);
                    let left = self.expression(
                        id,
                        slots,
                        binary.left,
                        hint.or(partner).map(ValueType::Scalar),
                    )?;
                    (left, right)
                } else {
                    let left =
                        self.expression(id, slots, binary.left, hint.map(ValueType::Scalar))?;
                    let partner = left.result.and_then(ValueType::scalar);
                    let right = self.expression(
                        id,
                        slots,
                        binary.right,
                        hint.or(partner).map(ValueType::Scalar),
                    )?;
                    (left, right)
                };
                let anonymous = (
                    self.wholly_anonymous(binary.left),
                    self.wholly_anonymous(binary.right),
                );
                return binary_expression(id, binary.operator, left, right, anonymous, source);
            }
            ExpressionNode::Call(call) => return self.call_expression(id, slots, call, source),
            ExpressionNode::TypeExpression(_) => return unserved("type expression"),
            ExpressionNode::Match(matched) => {
                return self.match_expression(id, slots, matched, expected_scalar, source);
            }
            // The subject of the enclosing `match`, named by the capture
            // identity stage 03 gave both; any other saved subject is
            // outside the subset.
            ExpressionNode::SavedSubject(saved) => {
                match saved.component {
                    language_core::PatternComponent::Fields => {}
                    language_core::PatternComponent::Case => {
                        return unserved("saved subject case");
                    }
                }
                return match self
                    .captures
                    .iter()
                    .rev()
                    .find(|(capture, _)| *capture == saved.capture)
                {
                    Some((_, subject)) => Ok(Expression {
                        kind: ExpressionKind::SavedSubject(saved.capture),
                        result: subject.result,
                        source,
                    }),
                    None => unserved("saved subject"),
                };
            }
            ExpressionNode::ArrayLiteral(elements) => {
                return self.array_literal(id, slots, *elements, expected, source);
            }
            ExpressionNode::Atomic(_) => return unserved("atomic"),
            ExpressionNode::Cast(cast) => return self.cast(id, slots, cast, source),
            ExpressionNode::Float(_) => return unserved("float literal"),
            ExpressionNode::UndeterminedNaN(_) => return unserved("undetermined NaN"),
            ExpressionNode::Indexed(indexed) => return self.indexed(id, slots, indexed, source),
            ExpressionNode::Member(member) => return self.member(id, slots, member, source),
            ExpressionNode::Borrow(_) => return unserved("borrow"),
            ExpressionNode::Range(_) => return unserved("range"),
            ExpressionNode::StructLiteral(literal) => {
                return self.record_literal(id, slots, handle, literal, source);
            }
            ExpressionNode::String(_) => return unserved("string literal"),
            ExpressionNode::ZeroValue(_) => return unserved("zero value"),
        };
        Ok(Expression {
            kind,
            result: Some(ValueType::Scalar(result)),
            source,
        })
    }

    /// Value-position `match` over a sum with scalar arms. The subject must
    /// name storage (a slot or a path beneath one), so observing it in the
    /// dispatch and again in each arm's payload members evaluates nothing
    /// twice. Each arm is a case pattern or the wildcard, which must be
    /// last; an arm for a case an earlier arm selects can never run and is
    /// refused. Coverage is not decided here: lowering builds the
    /// terminator from the declaration's case table and reports a missing
    /// case there.
    fn match_expression(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        matched: &typed_trees::expression::TableMatchExpression,
        expected: Option<ScalarType>,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Expression(what));
        let subject = self.expression(id, slots, matched.subject, None)?;
        let Some(ValueType::Record(record)) = subject.result else {
            return Err(unserved("match on a scalar subject"));
        };
        if !names_storage(&subject) {
            return Err(unserved("match on a subject that is not a place"));
        }
        if !self
            .typed
            .expression_table
            .contains_value_capture(matched.capture)
            || !matches!(
                self.typed
                    .expression_table
                    .value_capture(matched.capture)
                    .owner,
                typed_trees::expression::ValueCaptureOwner::Match
            )
            || self
                .typed
                .expression_table
                .value_capture(matched.capture)
                .subject
                != matched.subject
        {
            return Err(unserved(
                "match without its exact completed subject capture",
            ));
        }
        self.captures.push((matched.capture, subject.clone()));
        let saved = Expression {
            kind: ExpressionKind::SavedSubject(matched.capture),
            result: subject.result,
            source: subject.source,
        };
        let read = self.match_arms(id, slots, matched, record, &saved, expected);
        self.captures.pop();
        let (arms, default, result) = read?;
        Ok(Expression {
            kind: ExpressionKind::Match {
                capture: Some(matched.capture),
                subject: Box::new(subject),
                arms,
                default,
            },
            result: Some(ValueType::Scalar(result)),
            source,
        })
    }

    #[allow(clippy::type_complexity)]
    fn match_arms(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        matched: &typed_trees::expression::TableMatchExpression,
        record: semantic_vocabulary::StructuralTypeId,
        subject: &Expression,
        expected: Option<ScalarType>,
    ) -> Checked<(
        Vec<(usize, Expression)>,
        Option<Box<Expression>>,
        ScalarType,
    )> {
        use typed_trees::expression::MatchPattern;
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Expression(what));
        let mut arms: Vec<(usize, Expression)> = Vec::new();
        let mut default = None;
        let mut result = expected;
        for arm in self.typed.expression_table.match_arms(matched.arms) {
            if default.is_some() {
                return Err(unserved("match arm after the wildcard"));
            }
            let case = match &arm.pattern {
                MatchPattern::Wildcard => None,
                MatchPattern::Value(_) => return Err(unserved("match arm on a value pattern")),
                MatchPattern::Destructure(pattern) => {
                    let case = self.case_pattern(id, record, pattern)?;
                    // Stage 03 keeps the arm's test as the pattern's
                    // condition, `subject in case && true`; it must read to
                    // exactly that, or the arm tests something else.
                    let condition = self.expression(
                        id,
                        slots,
                        pattern.condition,
                        Some(ValueType::Scalar(ScalarType::Boolean)),
                    )?;
                    if !same_computation(&condition, &case_condition(subject, case)) {
                        return Err(unserved("match arm whose condition is not its case test"));
                    }
                    if arms.iter().any(|(earlier, _)| *earlier == case) {
                        return Err(unserved("match arm for a case an earlier arm selects"));
                    }
                    Some(case)
                }
            };
            let value = self.expression(id, slots, arm.value, result.map(ValueType::Scalar))?;
            let value_type = match value.result {
                Some(ValueType::Scalar(scalar_type)) => scalar_type,
                Some(ValueType::Record(_)) => return Err(unserved("match with a data result")),
                Some(ValueType::Qualified(..)) => {
                    return Err(unserved("match with a policy-qualified result"));
                }
                Some(ValueType::Rigid(_)) => {
                    return Err(unserved("match with a result of a rigid type"));
                }
                None => return Err(unserved("match with a Unit arm")),
            };
            if result.is_some_and(|result| result != value_type) {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Typing("match arm value type"),
                ));
            }
            result = Some(value_type);
            match case {
                Some(case) => arms.push((case, value)),
                None => default = Some(Box::new(value)),
            }
        }
        let result = result.ok_or_else(|| unserved("match without an arm"))?;
        Ok((arms, default, result))
    }
}

/// Whether a data-typed expression names storage that exists before it is
/// read: a slot, or a field or payload path beneath one. Reading it again
/// names the same place and evaluates nothing.
pub(super) fn names_storage(expression: &Expression) -> bool {
    match &expression.kind {
        ExpressionKind::Slot(_) | ExpressionKind::SavedSubject(_) => true,
        ExpressionKind::Field { base, .. }
        | ExpressionKind::Payload { base, .. }
        | ExpressionKind::Index { base, .. } => names_storage(base),
        ExpressionKind::Borrow { .. }
        | ExpressionKind::Integer(_)
        | ExpressionKind::Boolean(_)
        | ExpressionKind::Array { .. }
        | ExpressionKind::Record { .. }
        | ExpressionKind::Case { .. }
        | ExpressionKind::IsCase { .. }
        | ExpressionKind::Match { .. }
        | ExpressionKind::Strict { .. }
        | ExpressionKind::Not(_)
        | ExpressionKind::BitwiseNot(_)
        | ExpressionKind::Cast(_)
        | ExpressionKind::Requalify(_)
        | ExpressionKind::ShortCircuit { .. }
        | ExpressionKind::Call { .. } => false,
    }
}

/// `subject in case`, the test of a case arm, as the reader reads it.
pub(super) fn case_test_of(subject: &Expression, case: usize) -> Expression {
    Expression {
        kind: ExpressionKind::IsCase {
            subject: Box::new(subject.clone()),
            case,
        },
        result: Some(ValueType::Scalar(ScalarType::Boolean)),
        source: subject.source,
    }
}

/// `subject in case && true`: the condition stage 03 records on a case
/// pattern with no guard of its own.
pub(super) fn case_condition(subject: &Expression, case: usize) -> Expression {
    Expression {
        kind: ExpressionKind::ShortCircuit {
            conjunction: true,
            left: Box::new(case_test_of(subject, case)),
            right: Box::new(Expression {
                kind: ExpressionKind::Boolean(true),
                result: Some(ValueType::Scalar(ScalarType::Boolean)),
                source: subject.source,
            }),
        },
        result: Some(ValueType::Scalar(ScalarType::Boolean)),
        source: subject.source,
    }
}

/// Builtin operator selection: which operation a binary operator names, and
/// its operand and result types. Builtin meaning is all the subset has: an
/// operator bound to a declared machine reaches the typed trees as a call.
/// `anonymous` says which operands are wholly anonymous in the source: a
/// literal that landed in its partner's carrier.
fn binary_expression(
    id: MachineId,
    operator: BinaryOperator,
    left: Expression,
    right: Expression,
    anonymous: (bool, bool),
    source: SourceOrigin,
) -> Checked<Expression> {
    enum Operands {
        /// Same integer type; the result has it.
        Arithmetic,
        /// Integer value and integer count, each its own type; the result
        /// has the value's type.
        Shift,
        /// Same integer or Boolean type; the result is Boolean.
        Equality,
        /// Same integer type; the result is Boolean.
        Ordering,
    }
    let typing = |what| refuse(id, Construct::Machine, Unserved::Typing(what));
    let (Some(left_type), Some(right_type)) = (left.result, right.result) else {
        return Err(typing("Unit operand"));
    };
    // Builtin operators are over scalars. Equality of two records needs
    // synthesized structural comparison evidence
    // (`wiki/spec/terminal-psi/structural_predicates.md`) and is refused.
    // A policy-qualified operand is its carrier under that policy
    // (numeric_values.md, "Policy domains": core declares each unsuffixed
    // arithmetic operator "for two operands carrying the domain and for one
    // such operand with a plain operand of the same carrier, so
    // `counter + 1` needs no cast", and "its result carries the same
    // domain"). Two different policies select no operator ("incompatible
    // policies do not mix implicitly").
    //
    // The plain partner is served when it is a literal, the case that
    // sentence names. Whether a plain *binding* is one too is open
    // (`OWNER_QUESTIONS.md`, decision `policy-operator-plain-operand`):
    // the sentence reads that way, and the current checker rejects it as
    // mixed arithmetic domains.
    let carried = |value_type| match value_type {
        ValueType::Scalar(scalar_type) => Some((scalar_type, None)),
        ValueType::Qualified(carrier, policy) => Some((carrier, Some(policy))),
        ValueType::Record(_) | ValueType::Rigid(_) => None,
    };
    let (Some((left_type, left_policy)), Some((right_type, right_policy))) =
        (carried(left_type), carried(right_type))
    else {
        return Err(refuse(
            id,
            Construct::Machine,
            Unserved::Expression("operator on a record"),
        ));
    };
    let policy = match (left_policy, right_policy) {
        (Some(left), Some(right)) if left != right => {
            return Err(typing("operands under two arithmetic policies"));
        }
        (left, right) => left.or(right),
    };
    // A shift's count is the exception: it "keeps its own" type and is
    // never the operand a policy is matched against.
    let shift = matches!(
        operator,
        BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
    );
    let plain_binding = match (left_policy, right_policy) {
        (Some(_), None) => !anonymous.1 && !shift,
        (None, Some(_)) => !anonymous.0,
        (Some(_), Some(_)) | (None, None) => false,
    };
    if plain_binding {
        return Err(refuse(
            id,
            Construct::Machine,
            Unserved::Expression(
                "operator between a policy-qualified operand and a plain operand that is not a literal",
            ),
        ));
    }
    let (operation, operands) = match operator {
        BinaryOperator::And | BinaryOperator::Or => {
            if left_type != ScalarType::Boolean || right_type != ScalarType::Boolean {
                return Err(typing("short-circuit operand is not bool"));
            }
            return Ok(Expression {
                kind: ExpressionKind::ShortCircuit {
                    conjunction: operator == BinaryOperator::And,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                result: Some(ValueType::Scalar(ScalarType::Boolean)),
                source,
            });
        }
        BinaryOperator::CaseMembership => {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Expression("case membership"),
            ));
        }
        BinaryOperator::Add => (Strict::Add, Operands::Arithmetic),
        BinaryOperator::Subtract => (Strict::Subtract, Operands::Arithmetic),
        BinaryOperator::Multiply => (Strict::Multiply, Operands::Arithmetic),
        BinaryOperator::Divide => (Strict::Divide, Operands::Arithmetic),
        BinaryOperator::Modulo => (Strict::Remainder, Operands::Arithmetic),
        BinaryOperator::BitwiseAnd => (Strict::BitwiseAnd, Operands::Arithmetic),
        BinaryOperator::BitwiseOr => (Strict::BitwiseOr, Operands::Arithmetic),
        BinaryOperator::BitwiseXor => (Strict::BitwiseXor, Operands::Arithmetic),
        BinaryOperator::ShiftLeft => (Strict::ShiftLeft, Operands::Shift),
        BinaryOperator::ShiftRight => (Strict::ShiftRight, Operands::Shift),
        BinaryOperator::Equal => (Strict::Equal, Operands::Equality),
        BinaryOperator::NotEqual => (Strict::NotEqual, Operands::Equality),
        BinaryOperator::Less => (Strict::Less, Operands::Ordering),
        BinaryOperator::LessOrEqual => (Strict::LessOrEqual, Operands::Ordering),
        BinaryOperator::Greater => (Strict::Greater, Operands::Ordering),
        BinaryOperator::GreaterOrEqual => (Strict::GreaterOrEqual, Operands::Ordering),
    };
    let unserved = |what| refuse(id, Construct::Machine, Unserved::Expression(what));
    let integer = |scalar_type| matches!(scalar_type, ScalarType::Integer(_));
    let boolean = ValueType::Scalar(ScalarType::Boolean);
    // The type an arithmetic or shift result has: its value operand's
    // carrier, under the operation's policy when it has one.
    let computed = |carrier| match policy {
        Some(policy) => ValueType::Qualified(carrier, policy),
        None => ValueType::Scalar(carrier),
    };
    let result = match operands {
        // The policy catalog has no bitwise row: `&`, `|` and `^` cannot
        // overflow, and whether core declares them for a policy-qualified
        // operand the specification's "each unsuffixed arithmetic
        // operator" does not say.
        Operands::Arithmetic
            if policy.is_some()
                && matches!(
                    operation,
                    Strict::BitwiseAnd | Strict::BitwiseOr | Strict::BitwiseXor
                ) =>
        {
            return Err(unserved("bitwise operator on a policy-qualified operand"));
        }
        Operands::Arithmetic if integer(left_type) && left_type == right_type => {
            computed(left_type)
        }
        // A shift's count keeps its own type and chooses nothing: the
        // value operand's policy is the operation's.
        Operands::Shift if right_policy.is_some() => {
            return Err(unserved("shift by a policy-qualified count"));
        }
        Operands::Shift if integer(left_type) && integer(right_type) => computed(left_type),
        // A comparison compares the stored values, whatever policy either
        // carries ("they compare the stored values and return `bool`").
        Operands::Equality
            if left_type == right_type
                && (integer(left_type) || left_type == ScalarType::Boolean) =>
        {
            boolean
        }
        Operands::Ordering if integer(left_type) && left_type == right_type => boolean,
        Operands::Arithmetic | Operands::Shift | Operands::Equality | Operands::Ordering => {
            return Err(typing("operand types"));
        }
    };
    Ok(Expression {
        kind: ExpressionKind::Strict {
            operation,
            left: Box::new(left),
            right: Box::new(right),
        },
        result: Some(result),
        source,
    })
}

/// Whether two read expressions compute the same thing: equal kinds over
/// equal slots, literal values, operators and callee identities, ignoring
/// only where each came from. Nothing here is text.
pub(super) fn same_computation(left: &Expression, right: &Expression) -> bool {
    let both = |a: &Expression, b: &Expression| same_computation(a, b);
    left.result == right.result
        && match (&left.kind, &right.kind) {
            (ExpressionKind::Integer(a), ExpressionKind::Integer(b)) => a == b,
            (ExpressionKind::Boolean(a), ExpressionKind::Boolean(b)) => a == b,
            (ExpressionKind::Slot(a), ExpressionKind::Slot(b)) => a == b,
            (ExpressionKind::SavedSubject(a), ExpressionKind::SavedSubject(b)) => a == b,
            (
                ExpressionKind::Strict {
                    operation: a,
                    left: a_left,
                    right: a_right,
                },
                ExpressionKind::Strict {
                    operation: b,
                    left: b_left,
                    right: b_right,
                },
            ) => a == b && both(a_left, b_left) && both(a_right, b_right),
            (ExpressionKind::Not(a), ExpressionKind::Not(b))
            | (ExpressionKind::BitwiseNot(a), ExpressionKind::BitwiseNot(b)) => both(a, b),
            // The same operand converted to the same carrier.
            (ExpressionKind::Cast(a), ExpressionKind::Cast(b)) => both(a, b),
            (
                ExpressionKind::ShortCircuit {
                    conjunction: a,
                    left: a_left,
                    right: a_right,
                },
                ExpressionKind::ShortCircuit {
                    conjunction: b,
                    left: b_left,
                    right: b_right,
                },
            ) => a == b && both(a_left, b_left) && both(a_right, b_right),
            (
                ExpressionKind::Call {
                    callee: a,
                    arguments: a_arguments,
                },
                ExpressionKind::Call {
                    callee: b,
                    arguments: b_arguments,
                },
            ) => {
                a == b
                    && a_arguments.len() == b_arguments.len()
                    && a_arguments.iter().zip(b_arguments).all(|(a, b)| both(a, b))
            }
            (ExpressionKind::Array { elements: a }, ExpressionKind::Array { elements: b }) => {
                a.len() == b.len() && a.iter().zip(b).all(|(a, b)| both(a, b))
            }
            (ExpressionKind::Index { base: a, index: ai }, ExpressionKind::Index { base: b, index: bi }) => both(a, b) && both(ai, bi),
            (ExpressionKind::Record { fields: a }, ExpressionKind::Record { fields: b }) => {
                a.len() == b.len()
                    && a.iter()
                        .zip(b)
                        .all(|((a_field, a), (b_field, b))| a_field == b_field && both(a, b))
            }
            (
                ExpressionKind::Field {
                    base: a,
                    field: a_field,
                },
                ExpressionKind::Field {
                    base: b,
                    field: b_field,
                },
            ) => a_field == b_field && both(a, b),
            (
                ExpressionKind::Case {
                    case: a_case,
                    fields: a,
                },
                ExpressionKind::Case {
                    case: b_case,
                    fields: b,
                },
            ) => {
                a_case == b_case
                    && a.len() == b.len()
                    && a.iter()
                        .zip(b)
                        .all(|((a_field, a), (b_field, b))| a_field == b_field && both(a, b))
            }
            (
                ExpressionKind::Payload {
                    base: a,
                    case: a_case,
                    field: a_field,
                },
                ExpressionKind::Payload {
                    base: b,
                    case: b_case,
                    field: b_field,
                },
            ) => a_case == b_case && a_field == b_field && both(a, b),
            (
                ExpressionKind::IsCase {
                    subject: a,
                    case: a_case,
                },
                ExpressionKind::IsCase {
                    subject: b,
                    case: b_case,
                },
            ) => a_case == b_case && both(a, b),
            (
                ExpressionKind::Match {
                    capture: a_capture,
                    subject: a,
                    arms: a_arms,
                    default: a_default,
                },
                ExpressionKind::Match {
                    capture: b_capture,
                    subject: b,
                    arms: b_arms,
                    default: b_default,
                },
            ) => {
                a_capture == b_capture && both(a, b)
                    && a_arms.len() == b_arms.len()
                    && a_arms
                        .iter()
                        .zip(b_arms)
                        .all(|((a_case, a), (b_case, b))| a_case == b_case && both(a, b))
                    && match (a_default, b_default) {
                        (Some(a), Some(b)) => both(a, b),
                        (None, None) => true,
                        (Some(_), None) | (None, Some(_)) => false,
                    }
            }
            (
                ExpressionKind::SavedSubject(_)
                | ExpressionKind::Integer(_)
                | ExpressionKind::Boolean(_)
                | ExpressionKind::Slot(_)
                | ExpressionKind::Array { .. }
                | ExpressionKind::Index { .. }
                | ExpressionKind::Record { .. }
                | ExpressionKind::Case { .. }
                | ExpressionKind::Payload { .. }
                | ExpressionKind::IsCase { .. }
                | ExpressionKind::Match { .. }
                | ExpressionKind::Field { .. }
                | ExpressionKind::Strict { .. }
                | ExpressionKind::Not(_)
                | ExpressionKind::BitwiseNot(_)
                | ExpressionKind::Cast(_)
            | ExpressionKind::Requalify(_)
                | ExpressionKind::ShortCircuit { .. }
                | ExpressionKind::Call { .. }
                // A loan is formed afresh each time; two are never one.
                | ExpressionKind::Borrow { .. },
                _,
            ) => false,
        }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::Policy;
    use semantic_vocabulary::{IntegerSign, IntegerType};

    fn carrier() -> ScalarType {
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap())
    }

    /// The type `operator` gives two operands of these types, or why it is
    /// refused. `anonymous` says which operand is a literal in the source.
    fn typed(
        operator: BinaryOperator,
        left: ValueType,
        right: ValueType,
        anonymous: (bool, bool),
    ) -> Result<ValueType, String> {
        let source = SourceOrigin::Unpositioned(crate::Unpositioned::SpanAbsent);
        let operand = |slot, value_type| Expression {
            kind: ExpressionKind::Slot(slot),
            result: Some(value_type),
            source,
        };
        binary_expression(
            MachineId::new(1).unwrap(),
            operator,
            operand(0, left),
            operand(1, right),
            anonymous,
            source,
        )
        .map(|expression| expression.result.unwrap())
        .map_err(|refusal| refusal.detail)
    }

    /// An arithmetic operator over a policy-qualified operand is that
    /// policy's and its result carries the policy; a comparison yields
    /// `bool`. The partner is an operand of the same policy or a literal.
    /// Two policies, a plain binding, a bitwise operator, a
    /// policy-qualified shift count and two carriers are each refused by
    /// name, and two plain operands are untouched.
    #[test]
    fn an_operator_takes_its_policy_from_its_operands_and_never_from_a_plain_binding() {
        let plain = ValueType::Scalar(carrier());
        let wrapping = ValueType::Qualified(carrier(), Policy::Wrapping);
        let saturating = ValueType::Qualified(carrier(), Policy::Saturating);
        let boolean = ValueType::Scalar(ScalarType::Boolean);
        let neither = (false, false);
        for operator in [
            BinaryOperator::Add,
            BinaryOperator::Subtract,
            BinaryOperator::Multiply,
            BinaryOperator::Divide,
            BinaryOperator::Modulo,
        ] {
            assert_eq!(typed(operator, plain, plain, neither), Ok(plain));
            assert_eq!(typed(operator, wrapping, wrapping, neither), Ok(wrapping));
            assert_eq!(
                typed(operator, saturating, saturating, neither),
                Ok(saturating)
            );
            // `w + 1` and `1 + w`: the literal landed in the carrier.
            assert_eq!(
                typed(operator, wrapping, plain, (false, true)),
                Ok(wrapping)
            );
            assert_eq!(
                typed(operator, plain, wrapping, (true, false)),
                Ok(wrapping)
            );
            for (left, right) in [(wrapping, plain), (plain, wrapping)] {
                assert_eq!(
                    typed(operator, left, right, neither).unwrap_err(),
                    "expression operator between a policy-qualified operand and a plain operand that is not a literal"
                );
            }
            assert_eq!(
                typed(operator, wrapping, saturating, neither).unwrap_err(),
                "typing operands under two arithmetic policies"
            );
        }
        for operator in [BinaryOperator::Less, BinaryOperator::Equal] {
            assert_eq!(typed(operator, wrapping, wrapping, neither), Ok(boolean));
            assert_eq!(typed(operator, wrapping, plain, (false, true)), Ok(boolean));
            assert_eq!(
                typed(operator, wrapping, plain, neither).unwrap_err(),
                "expression operator between a policy-qualified operand and a plain operand that is not a literal"
            );
        }
        // A shift's count keeps its own plain type, literal or binding.
        assert_eq!(
            typed(BinaryOperator::ShiftLeft, wrapping, plain, neither),
            Ok(wrapping)
        );
        assert_eq!(
            typed(BinaryOperator::ShiftRight, wrapping, wrapping, neither).unwrap_err(),
            "expression shift by a policy-qualified count"
        );
        assert_eq!(
            typed(BinaryOperator::BitwiseAnd, wrapping, wrapping, neither).unwrap_err(),
            "expression bitwise operator on a policy-qualified operand"
        );
        assert_eq!(
            typed(BinaryOperator::BitwiseAnd, plain, plain, neither),
            Ok(plain)
        );
        // A policy does not make two carriers one.
        let wider = ValueType::Qualified(
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            Policy::Wrapping,
        );
        assert_eq!(
            typed(BinaryOperator::Add, wrapping, wider, neither).unwrap_err(),
            "typing operand types"
        );
    }
}
