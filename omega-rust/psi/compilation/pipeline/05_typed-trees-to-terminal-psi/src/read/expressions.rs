/*
The expression family of the reader: names as slots, literals, the scalar
operators and the short-circuit operators, and whether two read expressions
are the same computation. Record literals and member paths are read in
`places`.
*/

use crate::body::Qualification;
use semantic_vocabulary::{IntegerSign, IntegerType, MachineId, ScalarType};
use terminal_check_tables::{Checked, Construct};
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableNamePath, UnaryOperator,
};

use super::authority::Access;
use super::{Program, Slot, symbol_kind};
use crate::body::{Expression, ExpressionKind, Policy, Selector, Strict, ValueType};
use crate::{SourceOrigin, Unserved, refuse, reject};

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

    /// The rejection of a value path `Type::name` whose `Type` is a data
    /// declaration with no member spelled `name`.
    ///
    /// data_and_literals.md: "Case values always use their carrier-qualified
    /// member path", and "Cases, nested data and attached domains share
    /// their owner's member namespace". A path through a data declaration to
    /// a name it has no member of names nothing, in any program.
    fn undeclared_data_member(
        &self,
        id: MachineId,
        path: &TableNamePath,
        source: SourceOrigin,
    ) -> Checked<()> {
        let typed = self.typed;
        let (Some(symbol), [_, member]) = (
            path.resolved_symbol(),
            typed.expression_table.name_path_members(path.members),
        ) else {
            return Ok(());
        };
        if typed.symbols.get(symbol).kind != symbols::SymbolKind::Data
            || typed
                .symbols
                .find_child_by_name(symbol, member.as_str())
                .is_some()
        {
            return Ok(());
        }
        Err(reject(
            id,
            source,
            format!(
                "data `{}` has no member `{}`: a case value is written by its \
                 carrier-qualified path, and this path names nothing the type declares",
                typed.symbols.display_path(symbol, "::"),
                member.as_str()
            ),
        ))
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
                self.undeclared_data_member(id, path, source)?;
                let slot = self.slot_of(id, slots, path)?;
                let bound = &slots[slot];
                match bound.value_type {
                    ValueType::Scalar(scalar_type, Qualification::NONE) if !bound.homed => {
                        (ExpressionKind::Slot(slot), scalar_type)
                    }
                    ValueType::Scalar(_, Qualification::NONE) => {
                        return unserved("erased term that reads storage");
                    }
                    ValueType::Scalar(..) => {
                        return unserved("erased term of a qualified type");
                    }
                    ValueType::Record(_, _, _) | ValueType::Rigid(_) => {
                        return unserved("erased term of a data type");
                    }
                }
            }
            _ => return unserved("erased term other than a literal or a binding"),
        };
        Ok(Expression {
            call_context: None,
            kind,
            result: Some(ValueType::Scalar(result, Qualification::NONE)),
            source,
        })
    }

    /// An inferred binding keeps the complete value its initializer supplies.
    /// Place projections already carry their actual substituted regions; an
    /// earlier inferred type occurrence is not an annotation in this scope.
    /// Context-dependent forms still use their inferred carrier, so literals
    /// land as before and explicit borrows keep their normal formation checks.
    pub(super) fn inferred_initializer(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        handle: ExpressionHandle,
        inferred: typed_trees::types::TypeReferenceHandle,
    ) -> Checked<Expression> {
        let source = SourceOrigin::of(self.typed.expression_table.source_span(handle));
        let node = self.node(id, handle)?;
        if let ExpressionNode::Member(member) = node {
            return self.member(id, slots, member, source, Access::Read);
        }
        let expected = match node {
            ExpressionNode::Name(path) => {
                let slot = self.slot_of(id, slots, path)?;
                if slots[slot].borrowed.is_none() {
                    return self.expression(id, slots, handle, Some(slots[slot].value_type));
                }
                true
            }
            ExpressionNode::Integer(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::String(_)
            | ExpressionNode::Borrow(_)
            | ExpressionNode::ArrayLiteral(_)
            | ExpressionNode::Unary(_)
            | ExpressionNode::Binary(_) => true,
            _ => false,
        };
        let expected = if expected
            && inferred.is_valid()
            && !matches!(
                self.typed.type_reference_table.type_reference(inferred),
                typed_trees::types::TypeReferenceNode::Unit
            ) {
            Some(self.local_type(id, inferred)?)
        } else {
            None
        };
        self.expression(id, slots, handle, expected)
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
            // At a parameter width no literal is a value of the carrier
            // until the checker proves it lands: the literal is formed in
            // the closed carrier of its sign and converted exactly, which
            // owes `literal <= 2^N - 1` (`2^(N-1) - 1` signed). "A generic
            // body admitting `i1` cannot assume that positive one inhabits
            // its carrier" (`numeric_values.md`, "Width-indexed integers").
            ExpressionNode::Integer(literal)
                if expected_scalar
                    .and_then(|scalar| scalar.as_integer())
                    .is_some_and(|integer| integer.bits().is_none()) =>
            {
                let Some(ScalarType::Integer(integer)) = expected_scalar else {
                    unreachable!("matched a parameter-width integer");
                };
                let formed = IntegerType::new(integer.sign(), 64)
                    .map_err(|_| refuse(id, Construct::Machine, Unserved::Typing("literal")))?;
                let (value, formed) =
                    self.literal(id, literal, Some(ScalarType::Integer(formed)))?;
                let operand = Expression {
                    call_context: None,
                    kind: ExpressionKind::Integer(value),
                    result: Some(ValueType::Scalar(
                        ScalarType::Integer(formed),
                        Qualification::NONE,
                    )),
                    source,
                };
                (
                    ExpressionKind::Cast(Box::new(operand)),
                    ScalarType::Integer(integer),
                )
            }
            ExpressionNode::Integer(literal)
                if literal.landing().is_none()
                    && matches!(expected_scalar, Some(ScalarType::IeeeFloat(_))) =>
            {
                let Some(ScalarType::IeeeFloat(format)) = expected_scalar else {
                    unreachable!("float destination selected this landing");
                };
                let value = self.anonymous_float(id, handle, format)?;
                (ExpressionKind::Float(value), ScalarType::IeeeFloat(format))
            }
            ExpressionNode::Integer(literal) => {
                let (value, integer) = self.literal(id, literal, expected_scalar)?;
                (ExpressionKind::Integer(value), ScalarType::Integer(integer))
            }
            ExpressionNode::Boolean(value) => {
                (ExpressionKind::Boolean(*value), ScalarType::Boolean)
            }
            ExpressionNode::Name(path) => {
                self.undeclared_data_member(id, path, source)?;
                if let Some(constant) = path
                    .resolved_symbol()
                    .and_then(|symbol| self.typed.const_declaration_by_symbol(symbol))
                    .filter(|constant| {
                        constant.pending_target_observation && constant.evaluated_value.is_none()
                    })
                {
                    let (application, scalar_type) = self.deferred_scalar_constant(id, constant)?;
                    return Ok(Expression {
                        call_context: None,
                        kind: ExpressionKind::ScalarApplication { application },
                        result: Some(ValueType::plain(scalar_type)),
                        source,
                    });
                }
                let slot = self.slot_of(id, slots, path)?;
                // An erased binding supplies no executable value. This
                // reader does not materialize it; unsupported classification
                // still prevents artifact acceptance.
                if slots[slot].erased {
                    return unserved("runtime use of an erased binding");
                }
                if slots[slot].borrowed.is_some()
                    && let Some(expected) = expected
                    && self.reference_referent(id, expected)?.is_some()
                {
                    return self.borrow_value(id, slots, handle, expected, source);
                }
                let value = Expression {
                    call_context: None,
                    kind: ExpressionKind::Slot(slot),
                    result: Some(slots[slot].value_type),
                    source,
                };
                return self.held(id, slots, handle, value, expected, source);
            }
            ExpressionNode::Unary(unary) => {
                let operand = self.expression(id, slots, unary.operand, expected)?;
                match (unary.operator, operand.result) {
                    (
                        UnaryOperator::LogicalNot,
                        Some(ValueType::Scalar(ScalarType::Boolean, Qualification::NONE)),
                    ) => (ExpressionKind::Not(Box::new(operand)), ScalarType::Boolean),
                    (UnaryOperator::BitwiseNot, Some(value_type)) => {
                        if matches!(value_type.scalar(), Some(ScalarType::Integer(integer))
                            if integer.is_address())
                        {
                            return unserved("arithmetic on an address");
                        }
                        let result =
                            crate::policy::bitwise_not_type(value_type).ok_or_else(|| {
                                refuse(
                                    id,
                                    Construct::Machine,
                                    Unserved::Typing("unary operand type"),
                                )
                            })?;
                        return Ok(Expression {
                            call_context: None,
                            kind: ExpressionKind::BitwiseNot(Box::new(operand)),
                            result: Some(result),
                            source,
                        });
                    }
                    _ => {
                        return Err(refuse(
                            id,
                            Construct::Machine,
                            Unserved::Typing("unary operand type"),
                        ));
                    }
                }
            }
            // A condition static type propositions decide is that Boolean:
            // closed type equations, and the fields of the integer carrier
            // query (`carriers`), whose own body is never run.
            ExpressionNode::Binary(_) | ExpressionNode::Member(_)
                if self.type_truth(
                    super::carriers::Scope::Own,
                    handle,
                    super::types::Reading::Value,
                ) == Some(super::types::TypeTruth::Open) =>
            {
                return unserved(super::types::OPEN_CARRIER_FAMILY);
            }
            ExpressionNode::Binary(_) | ExpressionNode::Member(_)
                if self.static_truth(handle).is_some() =>
            {
                let truth = self.static_truth(handle).unwrap_or_default();
                (ExpressionKind::Boolean(truth), ScalarType::Boolean)
            }
            ExpressionNode::Binary(_)
                if self.wholly_anonymous(handle)
                    && matches!(expected_scalar, Some(ScalarType::IeeeFloat(_))) =>
            {
                let Some(ScalarType::IeeeFloat(format)) = expected_scalar else {
                    unreachable!("float destination selected this landing");
                };
                let value = self.anonymous_float(id, handle, format)?;
                (ExpressionKind::Float(value), ScalarType::IeeeFloat(format))
            }
            ExpressionNode::Binary(_) if self.wholly_anonymous(handle) => {
                let (value, integer) = self.anonymous_value(id, handle, expected_scalar)?;
                (ExpressionKind::Integer(value), ScalarType::Integer(integer))
            }
            ExpressionNode::Binary(binary) => {
                // A suffixed operator (`+%`, `+|`, `+!`) names its policy at
                // this one node (`wiki/spec/language/numeric_values.md`,
                // "Policy operators").
                let suffix = crate::body::Policy::of_domain(binary.policy);
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
                // A quoted literal is a byte view whatever its partner is
                // (data_and_literals.md, "Quoted bytes"), so it is read
                // with no scalar expected: which operator two such operands
                // select is decided below, not where the literal lands.
                let quoted = (
                    matches!(self.node(id, binary.left)?, ExpressionNode::String(_)),
                    matches!(self.node(id, binary.right)?, ExpressionNode::String(_)),
                );
                let operand = |quoted: bool, hint: Option<ScalarType>| {
                    hint.filter(|_| !quoted).map(ValueType::plain)
                };
                let (left, right) = if self.wholly_anonymous(binary.left) {
                    let right =
                        self.expression(id, slots, binary.right, operand(quoted.1, hint))?;
                    let partner = right.result.and_then(ValueType::scalar);
                    let left = self.expression(
                        id,
                        slots,
                        binary.left,
                        operand(quoted.0, hint.or(partner)),
                    )?;
                    (left, right)
                } else {
                    let left = self.expression(id, slots, binary.left, operand(quoted.0, hint))?;
                    let partner = left.result.and_then(ValueType::scalar);
                    let right = self.expression(
                        id,
                        slots,
                        binary.right,
                        operand(quoted.1, hint.or(partner)),
                    )?;
                    (left, right)
                };
                // conformances.md, "Core equality acquisition": text is
                // "compared by byte content". Two unqualified byte views
                // compare by the bytes they hold; `!=` is that comparison
                // negated (structural_predicates.md, "Byte-sequence equality
                // compares live lengths and live byte prefixes").
                if suffix.is_none()
                    && matches!(
                        binary.operator,
                        BinaryOperator::Equal | BinaryOperator::NotEqual
                    )
                    && self.is_plain_readable_bytes(id, left.result)?
                    && self.is_plain_readable_bytes(id, right.result)?
                {
                    let boolean = Some(ValueType::plain(ScalarType::Boolean));
                    let equal = Expression {
                        call_context: None,
                        kind: ExpressionKind::Strict {
                            operation: Strict::Equal,
                            policy: None,
                            left: Box::new(left),
                            right: Box::new(right),
                        },
                        result: boolean,
                        source,
                    };
                    return Ok(if binary.operator == BinaryOperator::NotEqual {
                        Expression {
                            call_context: None,
                            kind: ExpressionKind::Not(Box::new(equal)),
                            result: boolean,
                            source,
                        }
                    } else {
                        equal
                    });
                }
                let anonymous = (
                    self.wholly_anonymous(binary.left),
                    self.wholly_anonymous(binary.right),
                );
                // An address is compared, never computed with here: address
                // arithmetic relates counts to addresses under a fit proof
                // (counts_and_addresses.md), which this reader does not form.
                let address = |operand: &Expression| {
                    matches!(
                        operand.result.and_then(ValueType::scalar),
                        Some(ScalarType::Integer(integer)) if integer.is_address()
                    )
                };
                if (address(&left) || address(&right))
                    && !matches!(
                        binary.operator,
                        BinaryOperator::Equal
                            | BinaryOperator::NotEqual
                            | BinaryOperator::Less
                            | BinaryOperator::LessOrEqual
                            | BinaryOperator::Greater
                            | BinaryOperator::GreaterOrEqual
                    )
                {
                    return unserved("arithmetic on an address");
                }
                return binary_expression(
                    id,
                    binary.operator,
                    left,
                    right,
                    anonymous,
                    suffix,
                    source,
                );
            }
            ExpressionNode::Call(call) => {
                self.expected = expected;
                return self.call_expression(id, slots, call, handle, source);
            }
            ExpressionNode::TypeExpression(_) => return unserved("type expression"),
            ExpressionNode::Match(matched) => {
                return self.match_expression(id, slots, matched, expected, source);
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
                        call_context: None,
                        kind: ExpressionKind::SavedSubject(saved.capture),
                        result: subject.result,
                        source,
                    }),
                    None => unserved("saved subject"),
                };
            }
            ExpressionNode::ArrayLiteral(elements) => {
                let declared = typed
                    .expression_table
                    .constructed_value_type_reference(handle);
                let array_type = if declared.is_valid() {
                    Some(self.value_type(id, declared)?)
                } else {
                    expected
                };
                return self.array_literal(id, slots, *elements, array_type, source);
            }
            ExpressionNode::Atomic(atomic) => return self.atomic_load(id, slots, atomic, source),
            ExpressionNode::Cast(cast) => return self.cast(id, slots, cast, source),
            ExpressionNode::Float(literal) => {
                let value = self.float_literal(id, literal, expected_scalar)?;
                (
                    ExpressionKind::Float(value),
                    ScalarType::IeeeFloat(value.format()),
                )
            }
            ExpressionNode::UndeterminedNaN(_) => return unserved("undetermined NaN"),
            ExpressionNode::Indexed(indexed) => {
                return self.indexed(id, slots, indexed, handle, expected, Access::Read);
            }
            ExpressionNode::Member(member) => {
                let value = self.member(id, slots, member, source, Access::Read)?;
                return self.held(id, slots, handle, value, expected, source);
            }
            ExpressionNode::Borrow(_) => {
                let Some(expected) = expected else {
                    return unserved("reference formation without its retained contextual type");
                };
                return self.borrow_value(id, slots, handle, expected, source);
            }
            ExpressionNode::Range(_) => return unserved("range"),
            ExpressionNode::StructLiteral(literal) => {
                self.expected = expected;
                return self.record_literal(id, slots, handle, literal, source);
            }
            ExpressionNode::String(bytes) => return self.byte_literal(id, bytes, expected, source),
            ExpressionNode::ZeroValue(_) => return unserved("zero value"),
        };
        Ok(Expression {
            call_context: None,
            kind,
            result: Some(ValueType::Scalar(result, Qualification::NONE)),
            source,
        })
    }

    /// A named place where a value is expected. A place that holds a
    /// reference of another access than the reference expected lends its
    /// referent on with the expected access: an exclusive reference stored
    /// into a shared field is a shared reborrow through it, which freezes
    /// it (loans.md, "Reborrow lineage and access"), and the reverse asks a
    /// shared reference for access it does not hold and is rejected there.
    /// Every other place is seen as `observed` says.
    fn held(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        handle: ExpressionHandle,
        value: Expression,
        expected: Option<ValueType>,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        if let (Some(wanted @ ValueType::Record(wanted_row, _, _)), Some(carried)) =
            (expected, value.result)
            && let ValueType::Record(carried_row, _, _) = carried
            && wanted_row != carried_row
            && self.reference_referent(id, wanted)?.is_some()
            && self.reference_referent(id, carried)?.is_some()
        {
            return self.borrow_value(id, slots, handle, wanted, source);
        }
        self.observed(id, value, expected)
    }

    /// A place that holds a reference, as the context that names it sees it.
    /// A reference context transports the permission descriptor; an ordinary
    /// value context observes its exact referent, so the path crosses the
    /// descriptor there (structural_access.md: "the explicit `Referent` path
    /// segment crosses the borrowed boundary"). The rule is the place's
    /// type's, not the place's kind's: a local that holds a reference and a
    /// record field that holds one (`carrier.cell.value`) cross alike.
    pub(super) fn observed(
        &mut self,
        id: MachineId,
        value: Expression,
        expected: Option<ValueType>,
    ) -> Checked<Expression> {
        if let Some(expected) = expected
            && self.reference_referent(id, expected)?.is_some()
        {
            return Ok(value);
        }
        let Some(held) = value.result else {
            return Ok(value);
        };
        let Some((referent, _)) = self.reference_referent(id, held)? else {
            return Ok(value);
        };
        Ok(Expression {
            call_context: None,
            source: value.source,
            kind: ExpressionKind::Referent(Box::new(value)),
            result: Some(referent),
        })
    }

    /// Read a place for the access its caller needs. Each receiver of a
    /// projected write is opened for writing and crosses any reference it
    /// holds. The final destination of a replacement keeps its own type so
    /// assigning to a reference field replaces the descriptor.
    pub(super) fn place(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        target: ExpressionHandle,
        expected: Option<ValueType>,
        access: Access,
    ) -> Checked<Expression> {
        if access == Access::Read {
            return self.expression(id, slots, target, expected);
        }
        let value = match self.node(id, target)? {
            ExpressionNode::Member(member) => {
                let source = SourceOrigin::of(self.typed.expression_table.source_span(target));
                self.member(id, slots, member, source, Access::Write)?
            }
            ExpressionNode::Indexed(indexed) => {
                self.indexed(id, slots, indexed, target, expected, Access::Write)?
            }
            _ => return self.expression(id, slots, target, expected),
        };
        if access == Access::Replace {
            return Ok(value);
        }
        self.observed(id, value, expected)
    }

    /// A match retains one completed subject capture. Lowering evaluates a
    /// computed subject once and keeps its owned storage through the selected
    /// arm, just as a stored subject keeps its exact projected place. Each
    /// arm is a case pattern or the wildcard, which must be
    /// last; an arm for a case an earlier arm selects can never run and is
    /// refused. Coverage is not decided here: lowering builds the
    /// terminator from the declaration's case table and reports a missing
    /// case there.
    fn match_expression(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        matched: &typed_trees::expression::TableMatchExpression,
        expected: Option<ValueType>,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Expression(what));
        let subject = self.expression(id, slots, matched.subject, None)?;
        let Some(record) = self.opened(id, subject.result, Access::Read)? else {
            return self.scalar_match(id, slots, matched, subject, expected, source);
        };
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
            call_context: None,
            kind: ExpressionKind::SavedSubject(matched.capture),
            result: subject.result,
            source: subject.source,
        };
        let read = self.match_arms(id, slots, matched, record, &saved, expected);
        self.captures.pop();
        let (arms, default, result) = read?;
        Ok(Expression {
            call_context: None,
            kind: ExpressionKind::Match {
                capture: Some(matched.capture),
                subject: Box::new(subject),
                arms: arms
                    .into_iter()
                    .map(|(case, value)| (Selector::Case(case), value))
                    .collect(),
                default,
            },
            result: Some(result),
            source,
        })
    }

    #[allow(clippy::type_complexity)]
    fn match_arms(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        matched: &typed_trees::expression::TableMatchExpression,
        record: super::authority::Opened,
        subject: &Expression,
        expected: Option<ValueType>,
    ) -> Checked<(Vec<(usize, Expression)>, Option<Box<Expression>>, ValueType)> {
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
                MatchPattern::Value(head) => {
                    if !self.typed.expression_table.expression_is_valid(*head) {
                        return Err(unserved("match value pattern has no retained head"));
                    }
                    // A payload-free case value tests exactly that case.
                    // The shared pattern reader verifies its nominal owner
                    // and refuses a case whose payload would be omitted.
                    let application_type = match self.typed.expression_table.expression(*head) {
                        ExpressionNode::StructLiteral(literal)
                            if literal.application_type.is_valid() =>
                        {
                            let plain = matches!(self.typed.type_reference_table.type_reference(literal.application_type),
                                typed_trees::types::TypeReferenceNode::Named { symbol, .. } if *symbol == literal.type_symbol)
                                && self
                                    .typed
                                    .unique_data_definition_by_symbol(literal.type_symbol)
                                    .is_some_and(|data| {
                                        data.type_parameters.is_empty()
                                            && data.lifetime_parameters.is_empty()
                                            && data.generic_instance.is_none()
                                    });
                            if plain {
                                Default::default()
                            } else {
                                literal.application_type
                            }
                        }
                        _ => Default::default(),
                    };
                    Some(self.case_pattern(
                        id,
                        record,
                        &typed_trees::expression::TableDestructurePattern {
                            application_type,
                            head: *head,
                            ..Default::default()
                        },
                        SourceOrigin::of(arm.source_span),
                    )?)
                }
                MatchPattern::Destructure(pattern) => {
                    let case =
                        self.case_pattern(id, record, pattern, SourceOrigin::of(arm.source_span))?;
                    // Stage 03 keeps the arm's test as the pattern's
                    // condition, `subject in case && true`; it must read to
                    // exactly that, or the arm tests something else.
                    let condition = self.expression(
                        id,
                        slots,
                        pattern.condition,
                        Some(ValueType::Scalar(ScalarType::Boolean, Qualification::NONE)),
                    )?;
                    if !same_computation(&condition, &case_condition(subject, case)) {
                        return Err(unserved("match arm whose condition is not its case test"));
                    }
                    Some(case)
                }
            };
            if case.is_some_and(|case| arms.iter().any(|(earlier, _)| *earlier == case)) {
                return Err(unserved("match arm for a case an earlier arm selects"));
            }
            let value = self.arm_value(id, slots, arm.value, &mut result)?;
            self.label_arm(id, &value, &arm.pattern);
            match case {
                Some(case) => arms.push((case, value)),
                None => default = Some(Box::new(value)),
            }
        }
        let result = result.ok_or_else(|| unserved("match without an arm"))?;
        Ok((arms, default, result))
    }

    /// Each returning arm transfers the destination's exact type, including
    /// its domains and regions. Structural arrival parameters carry those
    /// memberships into the match continuation.
    fn arm_value(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        value: ExpressionHandle,
        result: &mut Option<ValueType>,
    ) -> Checked<Expression> {
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Expression(what));
        // An earlier arm that produces no value supplies no expected type
        // for a later, returning arm. Its own expression remains unchanged.
        let expected = result.filter(|value_type| !self.type_excludes_values(*value_type));
        let value = self.expression(id, slots, value, expected)?;
        if value
            .result
            .is_some_and(|value_type| self.type_excludes_values(value_type))
        {
            if result.is_none() {
                *result = value.result;
            }
            return Ok(value);
        }
        let value_type = match value.result {
            // Every returning arm supplies the same arithmetic policy.
            // Predicate-domain membership still needs its own arrival proof.
            Some(value_type @ ValueType::Scalar(_, qualification))
                if qualification.domains.is_empty() =>
            {
                value_type
            }
            Some(value_type @ ValueType::Record(..)) => value_type,
            Some(ValueType::Scalar(..)) => {
                return Err(unserved("match with a qualified result"));
            }
            Some(ValueType::Rigid(_)) => {
                return Err(unserved("match with a result of a rigid type"));
            }
            None => return Err(unserved("match with a Unit arm")),
        };
        if result.is_some_and(|result| {
            !self.type_excludes_values(result) && !self.agrees(Some(value_type), result)
        }) {
            return Err(self.mistyped(
                id,
                value.source,
                super::rejections::Destination::MatchArm,
                *result,
                value.result,
                "match arm value type",
            ));
        }
        *result = Some(value_type);
        Ok(value)
    }

    /// How a warning about one value `match` arm names it: its pattern as
    /// authored, or `_`, keyed by the span of the arm's value, which is how
    /// lowering knows the arm when it records the arm as an authored branch
    /// (`MachineNames::arms`). An arm whose value has no authored span is
    /// named "an arm".
    fn label_arm(
        &mut self,
        id: MachineId,
        value: &Expression,
        pattern: &typed_trees::expression::MatchPattern,
    ) {
        use typed_trees::expression::MatchPattern;
        let SourceOrigin::Authored(span) = value.source else {
            return;
        };
        let table = &self.typed.expression_table;
        let label = match pattern {
            MatchPattern::Wildcard => String::from("_"),
            MatchPattern::Value(head) => table.display_name(*head),
            MatchPattern::Destructure(pattern) => table.display_name(pattern.head),
        };
        if let Some(names) = self.names.get_mut(id) {
            names
                .arms
                .push((span, format!("the `{label}` arm of a match")));
        }
    }

    /// Value-position `match` over a scalar: `match code { 0 -> a, 1 -> b,
    /// _ -> c }` or `match flag { true -> a, false -> b }`.
    ///
    /// patterns.md: "Scalars use value patterns", and a dispatch "evaluates
    /// its subject once, tests alternatives in authored order, and executes
    /// only the selected arm". So the subject may be any scalar expression,
    /// evaluated once by lowering; each value arm is an equality test of that
    /// one value against a literal of the subject's carrier, and the
    /// wildcard, which must be last, is the default.
    ///
    /// Coverage ("Missing runtime coverage is a compile error"): a wildcard
    /// closes it, and so do Boolean arms naming both `true` and `false`
    /// ("Boolean coverage includes true/false"). In that case the last arm
    /// becomes the default: when every earlier test failed, the subject is
    /// the one value the earlier arms left, which is the last arm's. Any
    /// other ladder without a wildcard is rejected ("Other value/comparison
    /// ladders ... require a wildcard").
    ///
    /// A pattern that is not a literal (a named constant, a range, a
    /// destructure) and a subject that is not a plain integer or Boolean are
    /// refused by name.
    fn scalar_match(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        matched: &typed_trees::expression::TableMatchExpression,
        subject: Expression,
        expected: Option<ValueType>,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        use typed_trees::expression::MatchPattern;
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Expression(what));
        let carrier = match subject.result {
            Some(ValueType::Scalar(
                carrier @ (ScalarType::Integer(_) | ScalarType::Boolean),
                qualification,
            )) if qualification.domains.is_empty() => carrier,
            Some(ValueType::Scalar(ScalarType::IeeeFloat(_), _)) => {
                return Err(unserved("match on a float subject"));
            }
            Some(ValueType::Scalar(..)) => {
                return Err(unserved("match on a subject that carries a domain"));
            }
            Some(ValueType::Rigid(_)) => {
                return Err(unserved("match on a subject of a rigid type"));
            }
            Some(ValueType::Record(_, _, _)) | None => {
                return Err(unserved(
                    "match on a subject that is neither data nor scalar",
                ));
            }
        };
        // Stage 03 gives every match a completed subject capture, but only a
        // destructuring arm reads it, and a scalar has none: the capture is
        // not pushed, so a saved-subject read in an arm stays refused.
        let mut arms: Vec<(Selector, Expression)> = Vec::new();
        let mut default = None;
        let mut result = expected;
        for arm in self.typed.expression_table.match_arms(matched.arms) {
            if default.is_some() {
                return Err(unserved("match arm after the wildcard"));
            }
            let pattern = match &arm.pattern {
                MatchPattern::Wildcard => None,
                MatchPattern::Value(head) => {
                    if !self.typed.expression_table.expression_is_valid(*head) {
                        return Err(unserved("match value pattern has no retained head"));
                    }
                    let pattern = self.expression(
                        id,
                        slots,
                        *head,
                        Some(ValueType::Scalar(carrier, Qualification::NONE)),
                    )?;
                    let literal = matches!(
                        (&pattern.kind, carrier),
                        (ExpressionKind::Integer(_), ScalarType::Integer(_))
                            | (ExpressionKind::Boolean(_), ScalarType::Boolean)
                    );
                    if !literal
                        || !matches!(pattern.result,
                            Some(ValueType::Scalar(found, _)) if found == carrier)
                    {
                        return Err(unserved("scalar match pattern that is not a literal"));
                    }
                    Some(pattern)
                }
                MatchPattern::Destructure(_) => {
                    return Err(unserved("destructuring pattern on a scalar subject"));
                }
            };
            let value = self.arm_value(id, slots, arm.value, &mut result)?;
            self.label_arm(id, &value, &arm.pattern);
            match pattern {
                Some(pattern) => arms.push((Selector::Value(pattern), value)),
                None => default = Some(Box::new(value)),
            }
        }
        if default.is_none() {
            let named = |truth: bool| {
                arms.iter().any(|(selector, _)| {
                    matches!(selector, Selector::Value(Expression {
                        kind: ExpressionKind::Boolean(value), ..
                    }) if *value == truth)
                })
            };
            if carrier != ScalarType::Boolean || !named(true) || !named(false) {
                return Err(reject(
                    id,
                    source,
                    String::from(
                        "this match does not cover every value of its scalar subject: \
                         add a `_` arm (only `true` and `false` together cover a `bool`)",
                    ),
                ));
            }
            default = arms.pop().map(|(_, value)| Box::new(value));
        }
        let result = result.ok_or_else(|| unserved("match without an arm"))?;
        Ok(Expression {
            call_context: None,
            kind: ExpressionKind::Match {
                capture: None,
                subject: Box::new(subject),
                arms,
                default,
            },
            result: Some(result),
            source,
        })
    }
}

/// `subject in case`, the test of a case arm, as the reader reads it.
pub(super) fn case_test_of(subject: &Expression, case: usize) -> Expression {
    Expression {
        call_context: None,
        kind: ExpressionKind::IsCase {
            subject: Box::new(subject.clone()),
            case,
        },
        result: Some(ValueType::Scalar(ScalarType::Boolean, Qualification::NONE)),
        source: subject.source,
    }
}

/// `subject in case && true`: the condition stage 03 records on a case
/// pattern with no guard of its own.
pub(super) fn case_condition(subject: &Expression, case: usize) -> Expression {
    Expression {
        call_context: None,
        kind: ExpressionKind::ShortCircuit {
            conjunction: true,
            left: Box::new(case_test_of(subject, case)),
            right: Box::new(Expression {
                call_context: None,
                kind: ExpressionKind::Boolean(true),
                result: Some(ValueType::Scalar(ScalarType::Boolean, Qualification::NONE)),
                source: subject.source,
            }),
        },
        result: Some(ValueType::Scalar(ScalarType::Boolean, Qualification::NONE)),
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
    suffix: Option<Policy>,
    source: SourceOrigin,
) -> Checked<Expression> {
    let (left, right) = comparison_operands(operator, left, right);
    let (operation, policy, result) = binary_selection(
        id,
        operator,
        left.result,
        right.result,
        anonymous,
        suffix,
        (source, right.source),
    )?;
    let kind = match operation {
        Some(operation) => ExpressionKind::Strict {
            operation,
            policy,
            left: Box::new(left),
            right: Box::new(right),
        },
        None => ExpressionKind::ShortCircuit {
            conjunction: operator == BinaryOperator::And,
            left: Box::new(left),
            right: Box::new(right),
        },
    };
    Ok(Expression {
        call_context: None,
        kind,
        result: Some(result),
        source,
    })
}

/// numeric_values.md, "Integer equality and order" (`mixed-integer-comparison`):
/// a builtin comparison relates the exact integer values of its operands,
/// whatever their widths or signedness. Widening the narrower operand to a
/// fixed carrier that holds every value of both keeps each value exactly, so
/// comparing there is that relation. The common carrier is at most 64 bits:
/// native code has no widening into `i128`, so a pair that needs one (`u64`
/// with a signed partner) stays refused rather than passing the checker and
/// failing in native lowering. Operands that carry a domain or policy are
/// left unchanged for selection to judge.
fn comparison_operands(
    operator: BinaryOperator,
    left: Expression,
    right: Expression,
) -> (Expression, Expression) {
    let comparison = matches!(
        operator,
        BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
    );
    let plain_integer = |expression: &Expression| match expression.result {
        Some(ValueType::Scalar(ScalarType::Integer(integer), qualification))
            if qualification == Qualification::NONE && !integer.is_address() =>
        {
            Some(integer)
        }
        _ => None,
    };
    let (Some(left_type), Some(right_type)) = (plain_integer(&left), plain_integer(&right)) else {
        return (left, right);
    };
    if !comparison || left_type == right_type {
        return (left, right);
    }
    let common = if left_type.can_widen_to(right_type) {
        Some(right_type)
    } else if right_type.can_widen_to(left_type) {
        Some(left_type)
    } else {
        [16, 32, 64]
            .into_iter()
            .filter_map(|bits| IntegerType::new(IntegerSign::Signed, bits).ok())
            .find(|carrier| left_type.can_widen_to(*carrier) && right_type.can_widen_to(*carrier))
    };
    let Some(common) = common else {
        return (left, right);
    };
    let widen = |operand: Expression, integer: IntegerType| {
        if integer == common {
            return operand;
        }
        Expression {
            call_context: None,
            source: operand.source,
            result: Some(ValueType::Scalar(
                ScalarType::Integer(common),
                Qualification::NONE,
            )),
            kind: ExpressionKind::Cast(Box::new(operand)),
        }
    };
    (widen(left, left_type), widen(right, right_type))
}

/// A contract and an executable expression select the same operator from
/// the same operand types. The selected policy belongs to the operation;
/// this query neither builds an operation nor proves its formation condition.
pub(super) fn binary_selection(
    id: MachineId,
    operator: BinaryOperator,
    left: Option<ValueType>,
    right: Option<ValueType>,
    anonymous: (bool, bool),
    suffix: Option<Policy>,
    (source, right_source): (SourceOrigin, SourceOrigin),
) -> Checked<(Option<Strict>, Option<Policy>, ValueType)> {
    enum Operands {
        /// Same integer type; the result has it.
        Arithmetic,
        /// Same integer carrier, retaining its selected qualification.
        Bitwise,
        /// Integer value and integer count, each its own type; the result
        /// has the value's type.
        Shift,
        /// Same integer or Boolean type; the result is Boolean.
        Equality,
        /// Same integer type; the result is Boolean.
        Ordering,
    }
    let unserved = |what| refuse(id, Construct::Machine, Unserved::Expression(what));
    if suffix.is_some() {
        let operation = crate::policy::suffixed(operator).map_err(unserved)?;
        if anonymous == (true, true) {
            return Err(crate::reject(id, source, crate::policy::WHOLLY_ANONYMOUS));
        }
        for operand in [left, right] {
            match operand {
                Some(ValueType::Scalar(ScalarType::Integer(_), Qualification::NONE)) => {}
                Some(ValueType::Scalar(..) | ValueType::Record(..)) => {
                    return Err(crate::reject(id, source, crate::policy::QUALIFIED_OPERAND));
                }
                Some(ValueType::Rigid(_)) | None => {
                    return Err(unserved(crate::policy::QUALIFIED_OPERAND));
                }
            }
        }
        // A suffix chooses a carrier-specific operation, so mismatched
        // carriers are invalid source. Only a shift count keeps its own type.
        if !crate::policy::shifts(operation)
            && left.and_then(ValueType::scalar) != right.and_then(ValueType::scalar)
        {
            return Err(crate::reject(
                id,
                right_source,
                "policy operator operand types differ",
            ));
        }
    }
    let typing = |what| refuse(id, Construct::Machine, Unserved::Typing(what));
    let (Some(left_type), Some(right_type)) = (left, right) else {
        return Err(typing("Unit operand"));
    };
    // Core signatures accept plain scalars and single arithmetic policies.
    // Declared domains need their own operator declaration and arrive as calls.
    let carried = |value_type| match value_type {
        // domains.md, "Operands": builtin operators take unqualified
        // operands only. An arithmetic policy is the one qualification that
        // gives the bare operator a meaning; an operand that carries a
        // declared domain has no builtin operator.
        ValueType::Scalar(carrier, qualification) if qualification.domains.is_empty() => {
            Some((carrier, qualification.policy))
        }
        ValueType::Scalar(..) | ValueType::Record(_, _, _) | ValueType::Rigid(_) => None,
    };
    let (Some((left_type, left_policy)), Some((right_type, right_policy))) =
        (carried(left_type), carried(right_type))
    else {
        return Err(refuse(
            id,
            Construct::Machine,
            Unserved::Expression(
                "operator on an operand that is not a plain or policy-qualified scalar",
            ),
        ));
    };
    let (operation, operands) = match operator {
        BinaryOperator::And | BinaryOperator::Or => {
            if left_type != ScalarType::Boolean || right_type != ScalarType::Boolean {
                return Err(typing("short-circuit operand is not bool"));
            }
            return Ok((None, None, ValueType::plain(ScalarType::Boolean)));
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
        BinaryOperator::BitwiseAnd => (Strict::BitwiseAnd, Operands::Bitwise),
        BinaryOperator::BitwiseOr => (Strict::BitwiseOr, Operands::Bitwise),
        BinaryOperator::BitwiseXor => (Strict::BitwiseXor, Operands::Bitwise),
        BinaryOperator::ShiftLeft => (Strict::ShiftLeft, Operands::Shift),
        BinaryOperator::ShiftRight => (Strict::ShiftRight, Operands::Shift),
        BinaryOperator::Equal => (Strict::Equal, Operands::Equality),
        BinaryOperator::NotEqual => (Strict::NotEqual, Operands::Equality),
        BinaryOperator::Less => (Strict::Less, Operands::Ordering),
        BinaryOperator::LessOrEqual => (Strict::LessOrEqual, Operands::Ordering),
        BinaryOperator::Greater => (Strict::Greater, Operands::Ordering),
        BinaryOperator::GreaterOrEqual => (Strict::GreaterOrEqual, Operands::Ordering),
    };
    // A count has its own carrier and qualification. Only the shifted value
    // selects a shift policy. The other families require any two operand
    // policies to agree, regardless of where a plain operand came from.
    let operand_policy = operator
        .core_integer_policy(Policy::domain(left_policy), Policy::domain(right_policy))
        .map(Policy::of_domain)
        .ok_or_else(|| typing("operands under two arithmetic policies"))?;
    // A qualified operand and a plain operand must share a carrier. A
    // shift is exempt because its count has an independent type.
    if !operator.is_shift()
        && left_policy.is_some() != right_policy.is_some()
        && left_type != right_type
    {
        return Err(reject(
            id,
            right_source,
            "no arithmetic-policy operator takes a policy-qualified operand and a plain \
             operand of another carrier; cast one operand so both have the same carrier",
        ));
    }
    let integer = |scalar_type| matches!(scalar_type, ScalarType::Integer(_));
    let boolean = ValueType::plain(ScalarType::Boolean);
    // Only an operand's qualification carries through to the computed value.
    // A suffix selects the operation at this node and leaves the type bare.
    let computed = |carrier| {
        let qualifications = operand_policy.map_or(Qualification::NONE, Qualification::of_policy);
        ValueType::Scalar(carrier, qualifications)
    };
    let policy = suffix.or(operand_policy);
    let result = match operands {
        Operands::Arithmetic | Operands::Bitwise
            if integer(left_type) && left_type == right_type =>
        {
            computed(left_type)
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
        Operands::Arithmetic
            if left_type.as_ieee_float().is_some()
                && right_type.as_ieee_float().is_some()
                && left_type != right_type =>
        {
            let name = |scalar| match scalar {
                ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32) => "f32",
                ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64) => "f64",
                _ => unreachable!("the mismatch has two known float formats"),
            };
            return Err(reject(
                id,
                right_source,
                format!(
                    "float arithmetic operand types differ: {} and {}; this operation has no implicit format conversion",
                    name(left_type),
                    name(right_type),
                ),
            ));
        }
        Operands::Arithmetic
        | Operands::Bitwise
        | Operands::Shift
        | Operands::Equality
        | Operands::Ordering => return Err(typing("operand types")),
    };
    Ok((
        Some(operation),
        match operands {
            Operands::Arithmetic | Operands::Shift => policy,
            // Bitwise operations preserve a qualification but are total.
            Operands::Bitwise | Operands::Equality | Operands::Ordering => None,
        },
        result,
    ))
}

/// Whether two read expressions compute the same thing: equal kinds over
/// equal slots, literal values, operators and callee identities, ignoring
/// only where each came from. Nothing here is text.
pub(super) fn same_computation(left: &Expression, right: &Expression) -> bool {
    let both = |a: &Expression, b: &Expression| same_computation(a, b);
    left.result == right.result
        && match (&left.kind, &right.kind) {
            (ExpressionKind::Integer(a), ExpressionKind::Integer(b)) => a == b,
            (ExpressionKind::Float(a), ExpressionKind::Float(b)) => a == b,
            (
                ExpressionKind::ScalarApplication { application: a },
                ExpressionKind::ScalarApplication { application: b },
            ) => a == b,
            (ExpressionKind::Boolean(a), ExpressionKind::Boolean(b)) => a == b,
            (ExpressionKind::ByteLiteral(a), ExpressionKind::ByteLiteral(b)) => a == b,
            (ExpressionKind::CollectionMeasure { base: a, measure: am }, ExpressionKind::CollectionMeasure { base: b, measure: bm }) => am == bm && both(a, b),
            (ExpressionKind::ByteSequencePredicate { base: a, predicate: ap }, ExpressionKind::ByteSequencePredicate { base: b, predicate: bp }) => ap == bp && both(a, b),
            (ExpressionKind::Slot(a), ExpressionKind::Slot(b)) => a == b,
            (ExpressionKind::SavedSubject(a), ExpressionKind::SavedSubject(b)) => a == b,
            (
                ExpressionKind::Strict {
                    operation: a,
                    policy: a_policy,
                    left: a_left,
                    right: a_right,
                },
                ExpressionKind::Strict {
                    operation: b,
                    policy: b_policy,
                    left: b_left,
                    right: b_right,
                },
            ) => a == b && a_policy == b_policy && both(a_left, b_left) && both(a_right, b_right),
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
                    required_contract: a_required,
                    arguments: a_arguments,
                    designated: a_designated,
                    ..
                },
                ExpressionKind::Call {
                    callee: b,
                    required_contract: b_required,
                    arguments: b_arguments,
                    designated: b_designated,
                    ..
                },
            ) => {
                // A boundary may observe supplied state or issue a fresh
                // host ticket. Equal syntax is not one observed result.
                matches!(a, semantic_vocabulary::TargetMachineCallable::Machine(_))
                    && a == b
                    && a_required == b_required
                    && a_designated == b_designated
                    && a_arguments.len() == b_arguments.len()
                    && a_arguments.iter().zip(b_arguments).all(|(a, b)| both(a, b))
            }
            (ExpressionKind::Array { elements: a }, ExpressionKind::Array { elements: b }) => {
                a.len() == b.len() && a.iter().zip(b).all(|(a, b)| both(a, b))
            }
            (ExpressionKind::Index { base: a, index: ai }, ExpressionKind::Index { base: b, index: bi }) => both(a, b) && both(ai, bi),
            (ExpressionKind::ByteElement { base: a, index: ai }, ExpressionKind::ByteElement { base: b, index: bi }) => both(a, b) && both(ai, bi),
            (ExpressionKind::ArrayView { base: a, access: aa, element: ae }, ExpressionKind::ArrayView { base: b, access: ba, element: be }) => aa == ba && ae == be && both(a, b),
            (ExpressionKind::CollectionSubslice { base: a, start: ast, end: ae }, ExpressionKind::CollectionSubslice { base: b, start: bst, end: be }) => both(a,b) && match (ast,bst) { (Some(a),Some(b)) => both(a,b), (None,None) => true, _ => false } && match (ae,be) { (Some(a),Some(b)) => both(a,b), (None,None) => true, _ => false },
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
                        .all(|((a_case, a), (b_case, b))| {
                            both(a, b)
                                && match (a_case, b_case) {
                                    (Selector::Case(a), Selector::Case(b)) => a == b,
                                    (Selector::Value(a), Selector::Value(b)) => both(a, b),
                                    (Selector::Case(_), Selector::Value(_))
                                    | (Selector::Value(_), Selector::Case(_)) => false,
                                }
                        })
                    && match (a_default, b_default) {
                        (Some(a), Some(b)) => both(a, b),
                        (None, None) => true,
                        (Some(_), None) | (None, Some(_)) => false,
                    }
            }
            (
                ExpressionKind::Atomic(_)
                | ExpressionKind::SavedSubject(_)
                | ExpressionKind::Integer(_)
                | ExpressionKind::Float(_)
                | ExpressionKind::ScalarApplication { .. }
            | ExpressionKind::ByteLiteral(_)
        | ExpressionKind::CollectionMeasure { .. }
                | ExpressionKind::ByteSequencePredicate { .. }
                | ExpressionKind::ByteElement { .. }
            | ExpressionKind::ArrayView { .. }
            | ExpressionKind::CollectionSubslice { .. }
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
                | ExpressionKind::Establish { .. }
                | ExpressionKind::ShortCircuit { .. }
                | ExpressionKind::Call { .. }
                // A loan is formed afresh each time; two are never one.
                | ExpressionKind::Borrow { .. }
                | ExpressionKind::Referent(_),
                _,
            ) => false,
        }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{IntegerSign, IntegerType};

    fn carrier() -> ScalarType {
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap())
    }

    #[test]
    fn suffix_policy_belongs_to_the_operation_not_its_values() {
        let source = SourceOrigin::Unpositioned(crate::Unpositioned::SpanAbsent);
        let plain = ValueType::plain(carrier());
        let operand = |slot, result| Expression {
            call_context: None,
            kind: ExpressionKind::Slot(slot),
            result: Some(result),
            source,
        };
        let suffixed = |operator, policy, left, right| {
            binary_expression(
                MachineId::new(1).unwrap(),
                operator,
                operand(0, left),
                operand(1, right),
                (false, false),
                Some(policy),
                source,
            )
        };
        for operator in [
            BinaryOperator::Add,
            BinaryOperator::Subtract,
            BinaryOperator::Multiply,
            BinaryOperator::Divide,
            BinaryOperator::Modulo,
            BinaryOperator::ShiftLeft,
            BinaryOperator::ShiftRight,
        ] {
            let mut operations = Vec::new();
            for policy in [Policy::Wrapping, Policy::Saturating, Policy::Trapping] {
                let expression = suffixed(operator, policy, plain, plain).unwrap();
                assert_eq!(expression.result, Some(plain));
                let ExpressionKind::Strict {
                    policy: selected,
                    left,
                    right,
                    ..
                } = &expression.kind
                else {
                    panic!("a suffix must select a strict operation");
                };
                assert_eq!(*selected, Some(policy));
                assert_eq!(left.result, Some(plain));
                assert_eq!(right.result, Some(plain));
                assert!(matches!(left.kind, ExpressionKind::Slot(0)));
                assert!(matches!(right.kind, ExpressionKind::Slot(1)));
                assert!(same_computation(&expression, &expression.clone()));
                for previous in &operations {
                    assert!(!same_computation(previous, &expression));
                }
                operations.push(expression);

                let qualified = ValueType::Scalar(carrier(), Qualification::of_policy(policy));
                // A shift count is an operand too: no suffix removes its domain.
                for (left, right) in [(qualified, plain), (plain, qualified)] {
                    assert_eq!(
                        suffixed(operator, policy, left, right).unwrap_err().detail,
                        crate::policy::QUALIFIED_OPERAND,
                    );
                }
            }
        }
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
            call_context: None,
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
            None,
            source,
        )
        .map(|expression| expression.result.unwrap())
        .map_err(|refusal| refusal.detail)
    }

    #[test]
    fn core_integer_operator_types_follow_their_selected_signatures() {
        let plain = ValueType::plain(carrier());
        let qualified = |policy| ValueType::Scalar(carrier(), Qualification::of_policy(policy));
        let wrapping = qualified(Policy::Wrapping);
        let saturating = qualified(Policy::Saturating);
        let trapping = qualified(Policy::Trapping);
        let policies = [
            None,
            Some(Policy::Wrapping),
            Some(Policy::Saturating),
            Some(Policy::Trapping),
        ];
        let types = [plain, wrapping, saturating, trapping];
        let boolean = ValueType::plain(ScalarType::Boolean);
        let neither = (false, false);
        let id = MachineId::new(1).unwrap();
        let source = SourceOrigin::Unpositioned(crate::Unpositioned::SpanAbsent);
        let select = |operator, left, right| {
            binary_selection(
                id,
                operator,
                Some(left),
                Some(right),
                neither,
                None,
                (source, source),
            )
        };

        for operator in [
            BinaryOperator::Add,
            BinaryOperator::Subtract,
            BinaryOperator::Multiply,
            BinaryOperator::Divide,
            BinaryOperator::Modulo,
            BinaryOperator::BitwiseAnd,
            BinaryOperator::BitwiseOr,
            BinaryOperator::BitwiseXor,
            BinaryOperator::Less,
            BinaryOperator::Equal,
        ] {
            let comparison = matches!(operator, BinaryOperator::Less | BinaryOperator::Equal);
            let bitwise = matches!(
                operator,
                BinaryOperator::BitwiseAnd | BinaryOperator::BitwiseOr | BinaryOperator::BitwiseXor
            );
            for (value_type, policy) in types.into_iter().zip(policies) {
                let selected_policy = if comparison || bitwise { None } else { policy };
                let result = if comparison { boolean } else { value_type };
                for (left, right) in [
                    (value_type, value_type),
                    (value_type, plain),
                    (plain, value_type),
                ] {
                    let (_, found_policy, found_type) = select(operator, left, right).unwrap();
                    assert_eq!((found_policy, found_type), (selected_policy, result));
                }
            }
            let literal_result = if comparison { boolean } else { wrapping };
            assert_eq!(
                typed(operator, wrapping, plain, (false, true)),
                Ok(literal_result)
            );
            assert_eq!(
                typed(operator, plain, wrapping, (true, false)),
                Ok(literal_result)
            );
            assert_eq!(
                select(operator, wrapping, saturating).unwrap_err().detail,
                "typing operands under two arithmetic policies"
            );
        }

        // The count has a different sign and width. None of its four exact
        // qualifications can select the value's policy or transfer to the result.
        let count = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap());
        for operator in [BinaryOperator::ShiftLeft, BinaryOperator::ShiftRight] {
            for (value_type, value_policy) in types.into_iter().zip(policies) {
                for count_policy in policies {
                    let count_type = ValueType::Scalar(
                        count,
                        count_policy.map_or(Qualification::NONE, Qualification::of_policy),
                    );
                    let (_, found_policy, found_type) =
                        select(operator, value_type, count_type).unwrap();
                    assert_eq!((found_policy, found_type), (value_policy, value_type));
                }
            }
        }
        // A policy does not make two carriers one.
        let wider = ValueType::Scalar(
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            Qualification::of_policy(Policy::Wrapping),
        );
        assert_eq!(
            typed(BinaryOperator::Add, wrapping, wider, neither).unwrap_err(),
            "typing operand types"
        );
        // A plain operand of another carrier matches no policy operator.
        let plain_wider = ValueType::Scalar(
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            Qualification::NONE,
        );
        for (left, right) in [(wrapping, plain_wider), (plain_wider, wrapping)] {
            assert!(
                typed(BinaryOperator::Add, left, right, neither)
                    .unwrap_err()
                    .starts_with("no arithmetic-policy operator takes a policy-qualified operand"),
            );
        }
    }

    /// A comparison relates exact values across widths and signedness; the
    /// narrower operand widens to a carrier holding both. Arithmetic still
    /// needs one carrier, and a pair with no common carrier of at most 64 bits
    /// stays refused.
    #[test]
    fn a_comparison_widens_operands_of_different_integer_carriers() {
        let integer = |sign, bits| {
            ValueType::Scalar(
                ScalarType::Integer(IntegerType::new(sign, bits).unwrap()),
                Qualification::NONE,
            )
        };
        let boolean = ValueType::Scalar(ScalarType::Boolean, Qualification::NONE);
        let neither = (false, false);
        let u32_ = integer(IntegerSign::Unsigned, 32);
        let u64_ = integer(IntegerSign::Unsigned, 64);
        let i32_ = integer(IntegerSign::Signed, 32);
        for operator in [
            BinaryOperator::Less,
            BinaryOperator::Equal,
            BinaryOperator::GreaterOrEqual,
        ] {
            assert_eq!(typed(operator, u32_, u64_, neither), Ok(boolean));
            assert_eq!(typed(operator, u64_, u32_, neither), Ok(boolean));
            assert_eq!(typed(operator, i32_, u32_, neither), Ok(boolean));
        }
        assert_eq!(
            typed(BinaryOperator::Add, u32_, u64_, neither).unwrap_err(),
            "typing operand types"
        );
        assert_eq!(
            typed(
                BinaryOperator::Less,
                integer(IntegerSign::Unsigned, 128),
                integer(IntegerSign::Signed, 8),
                neither
            )
            .unwrap_err(),
            "typing operand types"
        );
        // `u64` against `i32` would need `i128`, which native code cannot widen to.
        assert_eq!(
            typed(BinaryOperator::Less, u64_, i32_, neither).unwrap_err(),
            "typing operand types"
        );
    }
}
