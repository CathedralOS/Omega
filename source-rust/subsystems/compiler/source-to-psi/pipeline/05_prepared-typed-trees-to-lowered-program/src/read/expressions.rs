use crate::read::refuse_at;
/*
The expression family of the reader: names as slots, literals, the scalar
operators and the short-circuit operators, and whether two read expressions
are the same computation. Record literals and member paths are read in
`places`.
*/

use crate::body::Qualification;
use crate::source_state::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableNamePath, UnaryOperator,
};
use semantic_vocabulary::{IntegerType, MachineId, ScalarTerm, ScalarType, ValueId};
use symbols::{BuiltinTypeAtom, PrimitiveType};
use terminal_check_tables::Checked;

use super::authority::Access;
use super::{Program, Slot, symbol_kind};
use crate::body::{CaseSelector, Expression, ExpressionKind, Policy, Selector, Strict, ValueType};
use crate::{SourceOrigin, Unserved, reject};

impl<'t> Program<'t> {
    pub(super) fn authored_node(
        &self,
        id: MachineId,
        node: crate::source_state::SourceNode,
        span: source_text::SourceSpan,
    ) -> Checked<SourceOrigin> {
        let owner = self
            .symbols
            .get(id)
            .copied()
            .filter(|owner| self.typed.symbols.symbols().nodes().is_valid(*owner))
            .ok_or_else(|| {
                refuse_at(
                    id,
                    Unserved::Declaration("authored occurrence without its source machine"),
                )
            })?;
        Ok(SourceOrigin::AuthoredNode {
            source_member: id,
            owner,
            node,
            span: SourceOrigin::of(span).span(),
        })
    }

    pub(super) fn slot_of(
        &self,
        id: MachineId,
        slots: &[Slot],
        path: &TableNamePath,
    ) -> Checked<usize> {
        let typed = self.typed;
        if path.reserved_result_of.is_valid() {
            return Err(refuse_at(
                id,
                Unserved::Expression("reserved result outside a contract term"),
            ));
        }
        let symbol = path.resolved_symbol().ok_or_else(|| {
            refuse_at(
                id,
                if path.static_application.is_valid() {
                    Unserved::Expression("applied name used as a plain value binding")
                } else {
                    Unserved::Spelling("name without a symbol")
                },
            )
        })?;
        if typed.expression_table.name_path_members(path.members).len() != 1 {
            return Err(refuse_at(id, Unserved::Expression("projected name path")));
        }
        // ponytail: linear over the body's slots; bodies in the subset hold a
        // handful of locals.
        slots
            .iter()
            .rposition(|slot| slot.symbol == symbol)
            .ok_or_else(|| {
                let kind = typed.symbols.get(symbol).kind;
                refuse_at(id, Unserved::Expression(symbol_kind(kind)))
            })
    }

    fn enclosing_value_capture(
        &self,
        id: MachineId,
        path: &TableNamePath,
        source: SourceOrigin,
    ) -> Checked<()> {
        use symbols::SymbolKind;
        if path.resolved_symbol().is_some()
            || path.static_application.is_valid()
            || path.reserved_result_of.is_valid()
        {
            return Ok(());
        }
        let [name] = self.typed.expression_table.name_path_members(path.members) else {
            return Ok(());
        };
        let Some(&caller) = self.symbols.get(id) else {
            return Ok(());
        };
        for enclosing in self.typed.enclosing_machines(caller) {
            for state in self.typed.machine_states(enclosing) {
                if [SymbolKind::Parameter, SymbolKind::Local]
                    .into_iter()
                    .any(|kind| {
                        self.typed
                            .symbols
                            .find_child_by_name_and_kind(state.symbol, name.as_str(), kind)
                            .is_some()
                    })
                {
                    return Err(reject(
                        id,
                        source,
                        format!(
                            "nested machine cannot capture enclosing runtime binding `{}` from `{}`; pass it as an argument",
                            name.as_str(),
                            enclosing.name.as_str(),
                        ),
                    ));
                }
            }
        }
        Ok(())
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
        let value = self.constant_value(id, expression)?;
        Ok(self.typed.expression_table.expression(value))
    }

    /// The expression a reader interprets in place of `expression`. A use of
    /// an evaluated constant stays a name for its selected declaration, so
    /// every value and contract reader reads the value that declaration
    /// stores (its materialized initializer). Any other expression, a
    /// constant still awaiting a target observation included, reads itself.
    pub(super) fn constant_value(
        &self,
        id: MachineId,
        expression: ExpressionHandle,
    ) -> Checked<ExpressionHandle> {
        let table = &self.typed.expression_table;
        if !table.expression_is_valid(expression) {
            return Err(refuse_at(id, Unserved::Expression("missing expression")));
        }
        if let ExpressionNode::Name(path) = table.expression(expression)
            && let Some(constant) = self.typed.named_const_declaration(path)
            && constant.evaluated_value.is_some()
        {
            if !table.expression_is_valid(constant.materialized_initializer) {
                return Err(refuse_at(
                    id,
                    Unserved::Expression("evaluated constant has no materialized initializer"),
                ));
            }
            return Ok(constant.materialized_initializer);
        }
        Ok(expression)
    }

    /// Reads the term at an erased position: an `[erased]` local's
    /// initializer or the argument for an `[erased]` formal. Literals, scalar
    /// bindings and eligible declaration applications contribute terms
    /// without runtime operations. Partial computations need formation
    /// evidence and cannot be introduced by an erased position.
    pub(super) fn erased_term(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        handle: ExpressionHandle,
        expected: Option<ScalarType>,
    ) -> Checked<Expression> {
        let node = self.node(id, handle)?;
        let source = SourceOrigin::of(self.typed.expression_table.source_span(handle));
        let unserved = |what| Err(refuse_at(id, Unserved::Expression(what)));
        let (kind, result) = match node {
            ExpressionNode::MachineValue(_) => {
                return unserved("erased runtime machine construction");
            }
            ExpressionNode::Integer(literal) => {
                let (value, integer) = self.literal(id, literal, expected)?;
                (ExpressionKind::Integer(value), ScalarType::Integer(integer))
            }
            ExpressionNode::Boolean(value) => {
                (ExpressionKind::Boolean(*value), ScalarType::Boolean)
            }
            ExpressionNode::Name(path) => {
                self.enclosing_value_capture(id, path, source)?;
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
            ExpressionNode::Call(_) if let Some(scalar) = expected => {
                use super::clause_terms::ClauseUse;
                use super::contracts::{ClauseScope, ResultTerm};

                let values = slots
                    .iter()
                    .enumerate()
                    .map(|(index, slot)| {
                        if slot.homed
                            || slot.borrowed.is_some()
                            || !matches!(slot.value_type, ValueType::Scalar(_, Qualification::NONE))
                        {
                            None
                        } else {
                            ValueId::new(index as u64 + 1)
                        }
                    })
                    .collect::<Vec<_>>();
                let scope = ClauseScope {
                    slots,
                    values: &values,
                    bound: &[],
                    owner: self.lifetime_owner,
                    result: ResultTerm::Absent("result in an erased term"),
                    subject: None,
                    formal_fields: false,
                    logical: None,
                };
                let (operands, discarded) = self.with_clause_arguments(id, |program| {
                    let mut operands = Vec::new();
                    program
                        .clause_term(
                            id,
                            &scope,
                            handle,
                            ClauseUse::Operand(ValueType::plain(scalar), &mut operands),
                        )?
                        .operand(id)?;
                    Ok(operands)
                })?;
                if !discarded.is_empty() {
                    return unserved(
                        "an erased application discards an operand that needs formation",
                    );
                }
                let term = crate::body::scalar_clause_value(operands);
                if let Some(term) = &term {
                    let mut pending = vec![term];
                    while let Some(operand) = pending.pop() {
                        match operand {
                            ScalarTerm::DeclarationApplication { arguments, .. } => {
                                pending.extend(arguments)
                            }
                            ScalarTerm::Value { .. }
                            | ScalarTerm::Boolean(_)
                            | ScalarTerm::Integer { .. }
                            | ScalarTerm::IeeeFloatLiteral(_) => {}
                            _ => {
                                return unserved(
                                    "an erased application operand needs runtime formation",
                                );
                            }
                        }
                    }
                }
                let kind = match term {
                    Some(term @ ScalarTerm::DeclarationApplication { .. }) => {
                        ExpressionKind::ErasedApplication(term)
                    }
                    Some(ScalarTerm::Value { id, .. }) => {
                        ExpressionKind::Slot(id.get() as usize - 1)
                    }
                    Some(ScalarTerm::Integer { value, .. }) => ExpressionKind::Integer(value),
                    Some(ScalarTerm::Boolean(value)) => ExpressionKind::Boolean(value),
                    _ => return unserved("erased call without a denotational term"),
                };
                (kind, scalar)
            }
            // Evidence held in an erased field of data (`measured.evidence`)
            // is passed as the witness an erased local of evidence is
            // (`families::evidence_witness`); that the receiving formal
            // takes evidence, and at this index, is checked where the
            // mathematical signature reads evidence (`families`).
            ExpressionNode::Member(member) if expected == Some(ScalarType::Boolean) => {
                let base = self.place(
                    id,
                    slots,
                    member.receiver,
                    None,
                    super::authority::Access::Read,
                )?;
                let evidence = match self.opened_place(id, &base, super::authority::Access::Read)? {
                    Some(record) => self
                        .erased_member(id, record.record(), member.selection.field_symbol())?
                        .and_then(|position| {
                            self.data_type(id, record.record())
                                .ok()
                                .and_then(|data| data.erased.get(position))
                                .map(|erased| erased.evidence)
                        })
                        .unwrap_or(false),
                    None => false,
                };
                if !evidence {
                    return unserved("erased term other than a literal or a binding");
                }
                (ExpressionKind::Boolean(true), ScalarType::Boolean)
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
        inferred: crate::source_state::types::TypeReferenceHandle,
    ) -> Checked<Expression> {
        let source = SourceOrigin::of(self.typed.expression_table.source_span(handle));
        let node = self.node(id, handle)?;
        if let ExpressionNode::Member(member) = node {
            return self.member(id, slots, member, source, Access::Read);
        }
        let expected = match node {
            // A constant supplies its declared carrier through the ordinary
            // value reader. Only a local slot needs its storage or borrow type.
            ExpressionNode::Name(path) if self.typed.named_const_declaration(path).is_some() => {
                false
            }
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
                crate::source_state::types::TypeReferenceNode::Unit
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
        let mut questions = Vec::new();
        for test in self.typed.expression_table.executable_domain_tests(handle) {
            if let Some(question) = self.require_membership_carrier(id, slots, *test)? {
                questions.push(question);
            }
        }
        let value = self.read_expression(id, slots, handle, expected)?;
        self.complete_formed_operator(id, handle, &value)?;
        Ok(value.with_membership_tests(questions.into_iter()))
    }

    fn read_expression(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        handle: ExpressionHandle,
        expected: Option<ValueType>,
    ) -> Checked<Expression> {
        let typed = self.typed;
        let node = self.node(id, handle)?;
        if let ExpressionNode::Call(call) = node
            && call.runtime_call.is_none()
            && call.layout_query.is_none()
        {
            self.bound_target(id, call.target_symbol)?;
        }
        let span = typed.expression_table.source_span(handle);
        let source = match node {
            ExpressionNode::Call(_) | ExpressionNode::MachineValue(_) => self.authored_node(
                id,
                crate::source_state::SourceNode::Expression(handle),
                span,
            )?,
            _ => SourceOrigin::of(span),
        };
        let unserved = |what| Err(refuse_at(id, Unserved::Expression(what)));
        let expected_scalar = expected.and_then(ValueType::scalar);
        let type_truth = if matches!(node, ExpressionNode::Binary(_) | ExpressionNode::Member(_)) {
            let owner = self.symbols.get(id).copied().unwrap_or_default();
            self.read_static_truth(id, owner, handle, super::types::Reading::Value)?
        } else {
            None
        };
        let (kind, result) = match node {
            ExpressionNode::MachineValue(value) => return self.machine_value(id, value, source),
            ExpressionNode::Block(block) => {
                return self.block_value(id, slots, block, expected, source);
            }
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
                    .map_err(|_| refuse_at(id, Unserved::Typing("literal")))?;
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
                let (value, _) = self.anonymous_float(id, handle, format)?;
                (ExpressionKind::Float(value), ScalarType::IeeeFloat(format))
            }
            ExpressionNode::Integer(literal) => {
                // A literal landed in another carrier than its context
                // receives, as `1u16` in a `u8` pattern or argument, is the
                // program's error at the literal: no implicit scalar
                // conversion happens at a receiving type.
                if literal.landing().is_some()
                    && let Some(expected @ ScalarType::Integer(_)) = expected_scalar
                {
                    let (_, landed) = self.literal(id, literal, None)?;
                    if ScalarType::Integer(landed) != expected {
                        return Err(reject(
                            id,
                            source,
                            format!(
                                "this literal has scalar carrier `{}` but its context requires \
                                 `{}`: there is no implicit scalar conversion at a receiving type",
                                super::rejections::scalar_named(ScalarType::Integer(landed)),
                                super::rejections::scalar_named(expected),
                            ),
                        ));
                    }
                }
                let (value, integer) = self.literal(id, literal, expected_scalar)?;
                (ExpressionKind::Integer(value), ScalarType::Integer(integer))
            }
            ExpressionNode::Boolean(value) => {
                (ExpressionKind::Boolean(*value), ScalarType::Boolean)
            }
            ExpressionNode::Name(path) => {
                self.enclosing_value_capture(id, path, source)?;
                self.undeclared_data_member(id, path, source)?;
                if let Some(constant) = self.typed.named_const_declaration(path)
                    && constant.pending_target_observation
                    && constant.evaluated_value.is_none()
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
                    return Err(crate::reject(
                        id,
                        source,
                        "runtime use of an erased binding",
                    ));
                }
                if slots[slot].borrowed.is_some()
                    && let Some(expected) = expected
                    && self.reference_referent(id, expected)?.is_some()
                {
                    return self.borrow_value(id, slots, handle, expected, source);
                }
                // A borrowed formal's slot already addresses its referent,
                // so reading it whole as a scalar where a value type
                // receives it would be the dereference only `$` may write.
                // With no receiving type the context may take the place
                // itself (a join of references, a forwarding call), so the
                // read goes on; operator operands are checked where they are
                // read (`require_explicit_operand`).
                if slots[slot].borrowed.is_some()
                    && let ValueType::Scalar(referent, _) = slots[slot].value_type
                    && expected.is_some()
                {
                    return Err(super::rejections::implicit_dereference(
                        id, source, referent,
                    ));
                }
                let value = Expression {
                    call_context: None,
                    kind: ExpressionKind::Slot(slot),
                    result: Some(slots[slot].value_type),
                    source,
                };
                return self.held(id, slots, handle, value, expected, source);
            }
            ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::Dereference => {
                return self.dereference(id, slots, unary.operand, source);
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
                        let result = crate::policy::bitwise_not_type(value_type)
                            .ok_or_else(|| refuse_at(id, Unserved::Typing("unary operand type")))?;
                        return Ok(Expression {
                            call_context: None,
                            kind: ExpressionKind::BitwiseNot(Box::new(operand)),
                            result: Some(result),
                            source,
                        });
                    }
                    _ => {
                        return Err(refuse_at(id, Unserved::Typing("unary operand type")));
                    }
                }
            }
            // A condition static type propositions decide is that Boolean:
            // closed type equations, and the fields of the integer carrier
            // query (`carriers`), whose own body is never run.
            ExpressionNode::Binary(_) | ExpressionNode::Member(_)
                if type_truth == Some(super::types::TypeTruth::Open) =>
            {
                return unserved(super::types::OPEN_CARRIER_FAMILY);
            }
            ExpressionNode::Binary(_) | ExpressionNode::Member(_)
                if matches!(type_truth, Some(super::types::TypeTruth::Known(_))) =>
            {
                let Some(super::types::TypeTruth::Known(truth)) = type_truth else {
                    unreachable!("the static evaluator supplied the known truth");
                };
                (ExpressionKind::Boolean(truth), ScalarType::Boolean)
            }
            ExpressionNode::Binary(_)
                if self.wholly_anonymous(handle)
                    && matches!(expected_scalar, Some(ScalarType::IeeeFloat(_))) =>
            {
                let Some(ScalarType::IeeeFloat(format)) = expected_scalar else {
                    unreachable!("float destination selected this landing");
                };
                let (value, operators) = self.anonymous_float(id, handle, format)?;
                if !operators.is_empty() {
                    let owner = self.symbols.get(id).copied().unwrap_or_default();
                    self.complete_anonymous_operators(id, owner, &operators)?;
                }
                (ExpressionKind::Float(value), ScalarType::IeeeFloat(format))
            }
            // A decimal literal is an exact rational, so `1.0e0` lands in an
            // integer destination as `1`, like `1`, and `1.5` is refused as a
            // fractional value (`numeric_values.md`, "Decimal literals and
            // wholly anonymous arithmetic denote exact rationals").
            ExpressionNode::Float(literal)
                if literal.landing().is_none()
                    && matches!(expected_scalar, Some(ScalarType::Integer(_))) =>
            {
                let evaluated = self.anonymous_value(
                    id,
                    handle,
                    expected_scalar,
                    super::types::AnonymousLanding::Implicit,
                )?;
                (
                    ExpressionKind::Integer(evaluated.value),
                    ScalarType::Integer(evaluated.integer),
                )
            }
            ExpressionNode::Binary(_) if self.wholly_anonymous(handle) => {
                let evaluated = self.anonymous_value(
                    id,
                    handle,
                    expected_scalar,
                    super::types::AnonymousLanding::Implicit,
                )?;
                let owner = self.symbols.get(id).copied().unwrap_or_default();
                self.complete_anonymous_operators(id, owner, &evaluated.operators)?;
                (
                    ExpressionKind::Integer(evaluated.value),
                    ScalarType::Integer(evaluated.integer),
                )
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
                // Addition permits the address on either side. Its result
                // cannot choose an operand's carrier before the peer is read.
                let hint = hint.filter(|ty| {
                    binary.operator != BinaryOperator::Add
                        || !matches!(ty, ScalarType::Integer(integer) if integer.is_address())
                });
                // Membership names a case domain. Value equality instead
                // reads both operands through the ordinary comparison path,
                // including when one is a named case value.
                if binary.operator == BinaryOperator::CaseMembership
                    && let Some(case) = self.named_case(binary.right)
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
                // A shift's count has its own carrier. Neither an expected
                // result nor the value operand can reland a typed count literal.
                let shift = matches!(
                    binary.operator,
                    BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
                );
                let (left, right) = if !shift && self.wholly_anonymous(binary.left) {
                    let right =
                        self.expression(id, slots, binary.right, operand(quoted.1, hint))?;
                    let partner = right.result.and_then(ValueType::scalar);
                    let hint = arithmetic_operand_hint(id, binary.operator, partner, hint, false)?;
                    let left = self.expression(id, slots, binary.left, operand(quoted.0, hint))?;
                    (left, right)
                } else {
                    let left = self.expression(id, slots, binary.left, operand(quoted.0, hint))?;
                    let partner = left.result.and_then(ValueType::scalar);
                    // A wholly anonymous count (`x << 8`) has no carrier of
                    // its own, so it lands in the shifted value's carrier
                    // (numeric_values.md, "Policy operators": "At least one
                    // operand must already be integer-typed and the other
                    // lands to that carrier"). The count obligation is still
                    // judged on the landed value; a typed count keeps its type.
                    let count = if self.wholly_anonymous(binary.right) {
                        partner
                    } else {
                        None
                    };
                    let hint = if shift {
                        count
                    } else {
                        arithmetic_operand_hint(id, binary.operator, partner, hint, true)?
                    };
                    let right =
                        self.expression(id, slots, binary.right, operand(quoted.1, hint))?;
                    (left, right)
                };
                self.require_explicit_operand(id, slots, binary.left, &left)?;
                self.require_explicit_operand(id, slots, binary.right, &right)?;
                // Intrinsic storage equality compares byte contents or the
                // retained case identity of a payload-free sum. Selected
                // user comparisons are already calls, so they never enter
                // this raw-operator path (conformances.md, "Core equality
                // acquisition"). `!=` negates the same selected comparison.
                if suffix.is_none()
                    && matches!(
                        binary.operator,
                        BinaryOperator::Equal | BinaryOperator::NotEqual
                    )
                    && ((self.is_plain_readable_bytes(id, left.result)?
                        && self.is_plain_readable_bytes(id, right.result)?)
                        || self.intrinsic_enum_equality(id, &left, &right, source)?)
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
                let Some((_, captured)) = self
                    .captures
                    .iter()
                    .rev()
                    .find(|(capture, _)| *capture == saved.capture)
                else {
                    return unserved("saved subject");
                };
                let subject = Expression {
                    call_context: None,
                    kind: ExpressionKind::SavedSubject(saved.capture),
                    result: captured.result,
                    source,
                };
                // The predicate's own self reads the completed scalar through its
                // declared carrier. Its membership test already checked all required
                // qualifications against the same original capture.
                let table = &typed.expression_table;
                let carrier = table.constructed_value_type_reference(handle);
                if let Some(actual @ ValueType::Scalar(..)) = subject.result
                    && carrier.is_valid()
                    && table.executable_domain_tests(handle).iter().any(|test| {
                        test.subject != handle
                            && matches!(table.expression(test.subject),
                                ExpressionNode::SavedSubject(operand) if operand.capture == saved.capture)
                            && (if test.application.is_valid() {
                                typed.domain_definition_for_instance(test.domain, test.application)
                            } else {
                                typed.domain_definition(test.domain)
                            }).is_some_and(|domain| domain.target_type == carrier
                                || (typed.normalized_type_identity(carrier).is_valid()
                                    && typed.normalized_type_identity(carrier) == typed.normalized_type_identity(domain.target_type)))
                    })
                {
                    let logical = self.value_type(id, carrier)?;
                    if actual != logical {
                        self.authorized(id, actual, Access::Read)?;
                        return Ok(Expression {
                            call_context: None,
                            kind: ExpressionKind::Requalify(Box::new(subject)),
                            result: Some(logical),
                            source,
                        });
                    }
                }
                return Ok(subject);
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
            ExpressionNode::Cast(cast) => return self.cast(id, slots, handle, cast, source),
            ExpressionNode::Float(literal) => {
                let value = self.float_literal(id, literal, expected_scalar)?;
                (
                    ExpressionKind::Float(value),
                    ScalarType::IeeeFloat(value.format()),
                )
            }
            ExpressionNode::UndeterminedNaN(_) => return unserved("undetermined NaN"),
            ExpressionNode::Indexed(indexed) => {
                if let Some(expected) = expected
                    && let Some(view) =
                        self.borrowed_array_range(id, slots, handle, expected, source, None)?
                {
                    return Ok(view);
                }
                return self.indexed(
                    id,
                    slots,
                    indexed,
                    handle,
                    expected,
                    Access::Read,
                    super::arrays::ElementRead::Whole,
                );
            }
            ExpressionNode::Member(member) => {
                let value = self.member(id, slots, member, source, Access::Read)?;
                return self.held(id, slots, handle, value, expected, source);
            }
            ExpressionNode::Borrow(_) => {
                let expected = match expected {
                    Some(expected) => expected,
                    None => {
                        let reference = typed
                            .expression_table
                            .constructed_value_type_reference(handle);
                        if !typed
                            .type_reference_table
                            .contains_type_reference(reference)
                        {
                            return unserved(
                                "reference formation without its retained contextual type",
                            );
                        }
                        self.value_type(id, reference)?
                    }
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

    fn dereference(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        operand: ExpressionHandle,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let descriptor = if let ExpressionNode::Name(path) = self.node(id, operand)?
            && let Ok(slot) = self.slot_of(id, slots, path)
            && !slots[slot].erased
        {
            let value = Expression {
                source,
                call_context: None,
                kind: ExpressionKind::Slot(slot),
                result: Some(slots[slot].value_type),
            };
            // Incoming borrowed formals already address their referent.
            if slots[slot].borrowed.is_some() {
                return Ok(value);
            }
            value
        } else {
            self.place(id, slots, operand, None, Access::Replace)?
        };
        if let Some(held) = descriptor.result
            && let Some((referent, _)) = self.reference_referent(id, held)?
        {
            return Ok(Expression {
                source,
                call_context: None,
                kind: ExpressionKind::Referent(Box::new(descriptor)),
                result: Some(referent),
            });
        }
        Err(reject(id, source, "`$` requires a reference operand"))
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
        let value = self.retain_view_input(id, slots, value, expected)?;
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

    /// The implicit route reads only two identical, unqualified enum types.
    /// Payloads require the selected Equatable call; domains.md, "Operands"
    /// requires a declared operator or explicit cast for qualified values.
    fn intrinsic_enum_equality(
        &self,
        id: MachineId,
        left: &Expression,
        right: &Expression,
        source: SourceOrigin,
    ) -> Checked<bool> {
        let Some(left_record) = self.opened(id, left.result, Access::Read)? else {
            return Ok(false);
        };
        let Some(right_record) = self.opened(id, right.result, Access::Read)? else {
            return Ok(false);
        };
        if [left.result, right.result]
            .into_iter()
            .flatten()
            .any(|value| !value.qualification().is_none())
        {
            return Err(reject(
                id,
                source,
                "builtin equality requires unqualified operands",
            ));
        }
        if left.result != right.result {
            return Err(reject(id, source, "equality operand types differ"));
        }
        self.require_intrinsic_tag_equality(id, left_record)?;
        self.require_intrinsic_tag_equality(id, right_record)?;
        Ok(true)
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
        // Crossing a reference to reach a record is a place projection.
        // Crossing one to read its scalar whole where a value type receives
        // it is a dereference, which the source must write.
        if let ValueType::Scalar(scalar, _) = referent
            && expected.is_some()
        {
            return Err(super::rejections::implicit_dereference(
                id,
                value.source,
                scalar,
            ));
        }
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
        if self.constant_value(id, target)? != target {
            let source = SourceOrigin::of(self.typed.expression_table.source_span(target));
            if access != Access::Borrow {
                return Err(reject(id, source, "a constant has no writable storage"));
            }
            // Each use borrows its own materialized copy. Fields and indices
            // project from this storage and keep its lifetime.
            let value = self.expression(id, slots, target, expected)?;
            let scalar_home = value
                .result
                .and_then(ValueType::scalar)
                .map(|scalar| self.plain_scalar_storage(id, scalar))
                .transpose()?;
            return Ok(Expression {
                call_context: None,
                result: value.result,
                source,
                kind: ExpressionKind::OwnedTemporary {
                    occurrence: target,
                    value: Box::new(value),
                    scalar_home,
                },
            });
        }
        let projection = if access == Access::Replace {
            Access::Write
        } else {
            access
        };
        let value = match self.node(id, target)? {
            ExpressionNode::Member(member) => {
                let source = SourceOrigin::of(self.typed.expression_table.source_span(target));
                self.member(id, slots, member, source, projection)?
            }
            ExpressionNode::Indexed(indexed) => self.indexed(
                id,
                slots,
                indexed,
                target,
                expected,
                projection,
                super::arrays::ElementRead::Whole,
            )?,
            _ => return self.expression(id, slots, target, expected),
        };
        self.complete_formed_operator(id, target, &value)?;
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
        matched: &crate::source_state::expression::TableMatchExpression,
        expected: Option<ValueType>,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let unserved = |what| refuse_at(id, Unserved::Expression(what));
        if !self
            .typed
            .expression_table
            .expression_is_valid(matched.subject)
        {
            return self.guarded_match(id, slots, matched, expected, source);
        }
        let subject = self.expression(id, slots, matched.subject, None)?;
        if !self
            .typed
            .expression_table
            .contains_value_capture(matched.capture)
            || !matches!(
                self.typed
                    .expression_table
                    .value_capture(matched.capture)
                    .owner,
                crate::source_state::expression::ValueCaptureOwner::Match
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
        // An unconditional arm captures its subject without inspecting its
        // representation. Its value may read the capture through the ordinary
        // access checks for those operations.
        if let [arm] = self.typed.expression_table.match_arms(matched.arms)
            && matches!(
                arm.pattern,
                crate::source_state::expression::MatchPattern::Wildcard
            )
        {
            self.captures.push((matched.capture, subject.clone()));
            let value = self.arm_value(id, slots, arm.value, &mut ArmsDeliver::expected(expected));
            self.captures.pop();
            let value = value?;
            self.label_arm(id, &value, &arm.pattern);
            return Ok(Expression {
                call_context: None,
                result: value.result,
                kind: ExpressionKind::Match {
                    capture: Some(matched.capture),
                    subject: Box::new(subject),
                    arms: Vec::new(),
                    default: Some(Box::new(value)),
                },
                source,
            });
        }
        let record = self.opened(id, subject.result, Access::Read)?;
        // A scalar subject is compared with value patterns, which
        // "Explicit dereference" lists among the places no dereference is
        // inserted: `match $v { 0 -> ... }`, not `match v`.
        if record.is_none() {
            self.require_explicit_operand(id, slots, matched.subject, &subject)?;
        }
        self.captures.push((matched.capture, subject.clone()));
        let Some(record) = record else {
            let result = self.scalar_match(id, slots, matched, subject, expected, source);
            self.captures.pop();
            return result;
        };
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
            result,
            source,
        })
    }

    #[allow(clippy::type_complexity)]
    fn match_arms(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        matched: &crate::source_state::expression::TableMatchExpression,
        record: super::authority::Opened,
        subject: &Expression,
        expected: Option<ValueType>,
    ) -> Checked<(
        Vec<(CaseSelector, Expression)>,
        Option<Box<Expression>>,
        Option<ValueType>,
    )> {
        use crate::source_state::expression::MatchPattern;
        let unserved = |what| refuse_at(id, Unserved::Expression(what));
        let mut arms: Vec<(CaseSelector, Expression)> = Vec::new();
        let mut default = None;
        let mut result = ArmsDeliver::expected(expected);
        let mut wildcard = None;
        let mut full_cases = vec![None; self.data_type(id, record.record())?.cases().len()];
        for arm in self.typed.expression_table.match_arms(matched.arms) {
            let case = match &arm.pattern {
                MatchPattern::Wildcard => None,
                MatchPattern::Guard(_) => {
                    return Err(unserved("a condition arm in a match with a subject"));
                }
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
                                crate::source_state::types::TypeReferenceNode::Named { symbol, .. } if *symbol == literal.type_symbol)
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
                    Some(CaseSelector::tag(self.case_pattern(
                        id,
                        record,
                        &crate::source_state::expression::TableDestructurePattern {
                            application_type,
                            head: *head,
                            ..Default::default()
                        },
                        SourceOrigin::of(arm.source_span),
                    )?))
                }
                // A record has no case to select. A pattern that decomposes
                // it cannot fail, so its arm takes every value as `_` does.
                MatchPattern::Destructure(pattern) if full_cases.is_empty() => {
                    self.record_arm(id, record, pattern, SourceOrigin::of(arm.source_span))?;
                    None
                }
                MatchPattern::Destructure(pattern) => Some(self.case_selector(
                    id,
                    slots,
                    record,
                    pattern,
                    subject,
                    SourceOrigin::of(arm.source_span),
                )?),
            };
            let value = self.arm_value(id, slots, arm.value, &mut result)?;
            self.label_arm(id, &value, &arm.pattern);
            // Read and type every authored arm before omitting a branch
            // whose earlier unconditional pattern already selects its values.
            if let Some(earlier) =
                wildcard.or_else(|| case.as_ref().and_then(|case| full_cases[case.case]))
            {
                if let Some(names) = self.names.get_mut(id) {
                    names.shadowed_patterns.push((arm.source_span, earlier));
                }
                continue;
            }
            match case {
                Some(case) => {
                    if case.condition.is_none() {
                        full_cases[case.case] = Some(arm.source_span);
                    }
                    arms.push((case, value));
                }
                None => {
                    wildcard = Some(arm.source_span);
                    default = Some(Box::new(value));
                }
            }
        }
        let result = result
            .result()
            .ok_or_else(|| unserved("match without an arm"))?;
        Ok((arms, default, result))
    }

    /// Each returning arm transfers the destination's exact type, including
    /// its domains and regions. Structural arrival parameters carry those
    /// memberships into the match continuation.
    ///
    /// patterns.md, "Coverage and domain tests": "An intentional Unit
    /// completion is explicit" and "A result-producing context still
    /// requires a compatible result". So every arm of a `match` completes
    /// with Unit, such as a call of a Unit machine or `{}`, or every arm
    /// delivers the one value type; an arm that never returns joins either.
    fn arm_value(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        value: ExpressionHandle,
        result: &mut ArmsDeliver,
    ) -> Checked<Expression> {
        let unserved = |what| refuse_at(id, Unserved::Expression(what));
        // An earlier arm that produces no value supplies no expected type
        // for a later, returning arm. Its own expression remains unchanged.
        let expected = match *result {
            ArmsDeliver::Value(value_type) if !self.type_excludes_values(value_type) => {
                Some(value_type)
            }
            _ => None,
        };
        let value = self.expression(id, slots, value, expected)?;
        if value.diverges()
            || value
                .result
                .is_some_and(|value_type| self.type_excludes_values(value_type))
        {
            if let (ArmsDeliver::Unread, Some(value_type)) = (*result, value.result) {
                *result = ArmsDeliver::Value(value_type);
            }
            return Ok(value);
        }
        let earlier = match *result {
            ArmsDeliver::Value(value_type) if self.type_excludes_values(value_type) => {
                ArmsDeliver::Unread
            }
            earlier => earlier,
        };
        let value_type = match value.result {
            // Every returning arm supplies the one result type, policy and
            // domains included; the join's parameter carries that type.
            Some(value_type @ (ValueType::Scalar(..) | ValueType::Record(..))) => value_type,
            Some(ValueType::Rigid(_)) => {
                return Err(unserved("match with a result of a rigid type"));
            }
            None => {
                if let ArmsDeliver::Value(_) = earlier {
                    return Err(reject(
                        id,
                        delivered_at(&value),
                        String::from(
                            "this match arm completes with Unit, but the match produces a \
                             value: every arm of a match must deliver the same result",
                        ),
                    ));
                }
                *result = ArmsDeliver::Unit;
                return Ok(value);
            }
        };
        match earlier {
            ArmsDeliver::Unread => {}
            ArmsDeliver::Unit => {
                return Err(reject(
                    id,
                    delivered_at(&value),
                    String::from(
                        "this match arm produces a value, but an earlier arm completes with \
                         Unit: every arm of a match must deliver the same result",
                    ),
                ));
            }
            ArmsDeliver::Value(earlier) => {
                if !self.agrees(Some(value_type), Some(earlier)) {
                    return Err(self.mistyped(
                        id,
                        delivered_at(&value),
                        super::rejections::Destination::MatchArm,
                        Some(earlier),
                        value.result,
                        "match arm value type",
                    ));
                }
            }
        }
        *result = ArmsDeliver::Value(value_type);
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
        pattern: &crate::source_state::expression::MatchPattern,
    ) {
        use crate::source_state::expression::MatchPattern;
        let SourceOrigin::Authored(span) = value.source else {
            return;
        };
        let table = &self.typed.expression_table;
        let label = match pattern {
            MatchPattern::Wildcard => String::from("_"),
            MatchPattern::Value(head) | MatchPattern::Guard(head) => table.display_name(*head),
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
        matched: &crate::source_state::expression::TableMatchExpression,
        subject: Expression,
        expected: Option<ValueType>,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        use crate::source_state::expression::MatchPattern;
        let unserved = |what| refuse_at(id, Unserved::Expression(what));
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
        let mut arms: Vec<(Selector, Expression)> = Vec::new();
        let mut default = None;
        let mut result = ArmsDeliver::expected(expected);
        for arm in self.typed.expression_table.match_arms(matched.arms) {
            if default.is_some() {
                return Err(unserved("match arm after the wildcard"));
            }
            let pattern = match &arm.pattern {
                MatchPattern::Wildcard => None,
                MatchPattern::Guard(_) => {
                    return Err(unserved("a condition arm in a match with a subject"));
                }
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
        let result = result
            .result()
            .ok_or_else(|| unserved("match without an arm"))?;
        Ok(Expression {
            call_context: None,
            kind: ExpressionKind::Match {
                capture: Some(matched.capture),
                subject: Box::new(subject),
                arms,
                default,
            },
            result,
            source,
        })
    }
}

impl Program<'_> {
    /// `match { a > b -> a, _ -> b }`: no subject, and each arm but the last
    /// is a Boolean condition over existing values, tested in authored order
    /// (patterns.md, "Bare names in dispatch arms": "In a dispatch without a
    /// subject, an arm condition such as `ready -> 1` reads the existing
    /// value of `ready`"; "Value-position `match` | Subject once; first
    /// matching arm only").
    ///
    /// Without a wildcard, the final guard must hold on every remaining
    /// arrival. Lowering records that obligation on the edge into its arm.
    fn guarded_match(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        matched: &crate::source_state::expression::TableMatchExpression,
        expected: Option<ValueType>,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        use crate::source_state::expression::MatchPattern;
        let unserved = |what| refuse_at(id, Unserved::Expression(what));
        let boolean = ValueType::Scalar(ScalarType::Boolean, Qualification::NONE);
        let mut arms: Vec<(Selector, Expression)> = Vec::new();
        let mut default = None;
        let mut result = ArmsDeliver::expected(expected);
        for arm in self.typed.expression_table.match_arms(matched.arms) {
            if default.is_some() {
                return Err(unserved("match arm after the wildcard"));
            }
            let condition = match &arm.pattern {
                MatchPattern::Wildcard => None,
                MatchPattern::Guard(condition) => {
                    let condition = self.expression(id, slots, *condition, Some(boolean))?;
                    if !matches!(
                        condition.result,
                        Some(ValueType::Scalar(ScalarType::Boolean, _))
                    ) {
                        return Err(reject(
                            id,
                            condition.source,
                            String::from(
                                "the condition of an arm of a match without a subject must be a `bool`",
                            ),
                        ));
                    }
                    Some(condition)
                }
                MatchPattern::Value(_) | MatchPattern::Destructure(_) => {
                    return Err(unserved("a pattern in a match without a subject"));
                }
            };
            let value = self.arm_value(id, slots, arm.value, &mut result)?;
            self.label_arm(id, &value, &arm.pattern);
            match condition {
                Some(condition) => arms.push((Selector::Guard(condition), value)),
                None => default = Some(Box::new(value)),
            }
        }
        if arms.is_empty() && default.is_none() {
            return Err(reject(
                id,
                source,
                String::from(
                    "this match has no `_` arm: when every condition is false no arm applies, so add a `_ ->` arm",
                ),
            ));
        }
        let result = result
            .result()
            .ok_or_else(|| unserved("match without an arm"))?;
        Ok(Expression {
            call_context: None,
            kind: ExpressionKind::Match {
                capture: None,
                subject: Box::new(Expression {
                    call_context: None,
                    kind: ExpressionKind::Boolean(true),
                    result: Some(boolean),
                    source,
                }),
                arms,
                default,
            },
            result,
            source,
        })
    }
}

/// What the arms of a `match` read so far deliver: nothing read yet, Unit,
/// or one value type (which may be the expected type of the context).
#[derive(Clone, Copy)]
enum ArmsDeliver {
    Unread,
    Unit,
    Value(ValueType),
}

impl ArmsDeliver {
    fn expected(expected: Option<ValueType>) -> Self {
        expected.map_or(Self::Unread, Self::Value)
    }

    /// The match's result: `Some(None)` for a Unit match, `None` when no
    /// arm was read.
    fn result(self) -> Option<Option<ValueType>> {
        match self {
            Self::Unread => None,
            Self::Unit => Some(None),
            Self::Value(value_type) => Some(Some(value_type)),
        }
    }
}

/// Where an arm delivers its value: a block's final value, at any depth.
fn delivered_at(value: &Expression) -> SourceOrigin {
    match &value.kind {
        ExpressionKind::Block {
            value: Some(inner), ..
        } => delivered_at(inner),
        _ => value.source,
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

/// Address arithmetic keeps the offset's u64 carrier independent of its result.
/// The shared source signatures distinguish offsets from address distances;
/// an expected address result cannot turn an anonymous count into an address.
fn arithmetic_operand_hint(
    id: MachineId,
    operator: BinaryOperator,
    peer: Option<ScalarType>,
    expected: Option<ScalarType>,
    right_operand: bool,
) -> Checked<Option<ScalarType>> {
    let Some(ScalarType::Integer(address)) = peer else {
        return Ok(expected.or(peer));
    };
    if !matches!(operator, BinaryOperator::Add | BinaryOperator::Subtract) || !address.is_address()
    {
        return Ok(expected.or(peer));
    }
    let selected = crate::source_state::operator::integer_peer_operand_carrier(
        operator,
        PrimitiveType::Addr,
        right_operand,
    );
    match selected {
        PrimitiveType::Addr => Ok(peer),
        PrimitiveType::Integer(carrier) => {
            super::types::atom_type(BuiltinTypeAtom::Integer(carrier), id).map(Some)
        }
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 | PrimitiveType::Selector => {
            Err(refuse_at(
                id,
                Unserved::Typing("address arithmetic operand carrier"),
            ))
        }
    }
}

/// Reuse the source address/count signatures and the ordinary exact-fit
/// judgment. Equal physical widths do not identify these operand carriers.
fn address_arithmetic_result(
    id: MachineId,
    operator: BinaryOperator,
    left: ScalarType,
    right: ScalarType,
) -> Checked<(Strict, ScalarType)> {
    let unserved = || refuse_at(id, Unserved::Expression("arithmetic on an address"));
    let primitive = |scalar| {
        PrimitiveType::from_builtin_atom(super::types::scalar_atom(scalar, id)?)
            .ok_or_else(unserved)
    };
    let selected = crate::source_state::operator::builtin_integer_result_carrier(
        operator,
        [primitive(left)?, primitive(right)?],
    )
    .ok_or_else(unserved)?;
    let result = match selected {
        PrimitiveType::Addr => {
            if matches!(left, ScalarType::Integer(integer) if integer.is_address()) {
                left
            } else {
                right
            }
        }
        PrimitiveType::Integer(carrier) => {
            super::types::atom_type(BuiltinTypeAtom::Integer(carrier), id)?
        }
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 | PrimitiveType::Selector => {
            return Err(unserved());
        }
    };
    let operation = match operator {
        BinaryOperator::Add => Strict::Add,
        BinaryOperator::Subtract => Strict::Subtract,
        _ => return Err(unserved()),
    };
    let admitted = match (result, left, right) {
        (ScalarType::Integer(result), ScalarType::Integer(left), ScalarType::Integer(right)) => {
            match operator {
                BinaryOperator::Add => result.admits_exact_add_operands(left, right),
                BinaryOperator::Subtract => result.admits_exact_subtract_operands(left, right),
                _ => false,
            }
        }
        _ => false,
    };
    if !admitted {
        return Err(refuse_at(id, Unserved::Typing("operand types")));
    }
    Ok((operation, result))
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
    let address = |operand: &Expression| matches!(operand.result.and_then(ValueType::scalar), Some(ScalarType::Integer(integer)) if integer.is_address());
    let comparison = matches!(
        operator,
        BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
    );
    let (operation, policy, result) = if (address(&left) || address(&right)) && !comparison {
        let plain = |operand: &Expression| {
            operand
                .result
                .is_some_and(|value| value.qualification().is_none())
        };
        if suffix.is_some() || !plain(&left) || !plain(&right) {
            return Err(refuse_at(
                id,
                Unserved::Expression("arithmetic on an address"),
            ));
        }
        let (Some(left_type), Some(right_type)) = (
            left.result.and_then(ValueType::scalar),
            right.result.and_then(ValueType::scalar),
        ) else {
            return Err(refuse_at(id, Unserved::Typing("operand types")));
        };
        let (operation, result) = address_arithmetic_result(id, operator, left_type, right_type)?;
        (Some(operation), None, ValueType::plain(result))
    } else {
        binary_selection(
            id,
            operator,
            left.result,
            right.result,
            anonymous,
            suffix,
            (source, right.source),
        )?
    };
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
        /// Integer values, or two Booleans; the result is Boolean.
        Equality,
        /// Integer values; the result is Boolean.
        Ordering,
    }
    let unserved = |what| refuse_at(id, Unserved::Expression(what));
    if suffix.is_some() {
        let operation = crate::policy::suffixed(operator).map_err(unserved)?;
        if anonymous == (true, true) {
            return Err(reject(id, source, crate::policy::WHOLLY_ANONYMOUS));
        }
        for operand in [left, right] {
            match operand {
                Some(ValueType::Scalar(ScalarType::Integer(_), Qualification::NONE)) => {}
                Some(ValueType::Scalar(..) | ValueType::Record(..)) => {
                    return Err(reject(id, source, crate::policy::QUALIFIED_OPERAND));
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
            return Err(reject(
                id,
                right_source,
                "policy operator operand types differ",
            ));
        }
    }
    let typing = |what| refuse_at(id, Unserved::Typing(what));
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
        return Err(refuse_at(
            id,
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
            return Err(refuse_at(id, Unserved::Expression("case membership")));
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
        // Terminal Psi comparisons retain both integer carriers and relate
        // their exact values. They need no common carrier or inserted cast;
        // native lowering implements that same mixed-sign relation.
        Operands::Equality | Operands::Ordering
            if integer(left_type)
                && integer(right_type)
                && (left_type == right_type
                    || (left_policy.is_none()
                        && right_policy.is_none()
                        && suffix.is_none()
                        && left_type
                            .as_integer()
                            .is_some_and(|integer| !integer.is_address())
                        && right_type
                            .as_integer()
                            .is_some_and(|integer| !integer.is_address()))) =>
        {
            boolean
        }
        Operands::Equality
            if left_type == ScalarType::Boolean && right_type == ScalarType::Boolean =>
        {
            boolean
        }
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
            (ExpressionKind::TheoremApplication { .. }, _) => false,
            (ExpressionKind::Block { .. }, _) => false,
            (ExpressionKind::Integer(a), ExpressionKind::Integer(b)) => a == b,
            (ExpressionKind::Float(a), ExpressionKind::Float(b)) => a == b,
            (
                ExpressionKind::ScalarApplication { application: a },
                ExpressionKind::ScalarApplication { application: b },
            ) => a == b,
            (ExpressionKind::Boolean(a), ExpressionKind::Boolean(b)) => a == b,
            (
                ExpressionKind::MachineValue { contract: a, descriptor: a_selected },
                ExpressionKind::MachineValue { contract: b, descriptor: b_selected },
            ) => a == b && a_selected == b_selected,
            (ExpressionKind::ByteLiteral(a), ExpressionKind::ByteLiteral(b)) => a == b,
            (ExpressionKind::CollectionMeasure { base: a, measure: am }, ExpressionKind::CollectionMeasure { base: b, measure: bm }) => am == bm && both(a, b),
            (ExpressionKind::ByteSequencePredicate { base: a, predicate: ap }, ExpressionKind::ByteSequencePredicate { base: b, predicate: bp }) => ap == bp && both(a, b),
            (ExpressionKind::Slot(a), ExpressionKind::Slot(b)) => a == b,
            (
                ExpressionKind::TestMembership { subject: a, domain: ad, predicate: ap },
                ExpressionKind::TestMembership { subject: b, domain: bd, predicate: bp },
            ) => ad == bd && same_computation(a, b) && same_computation(ap, bp),
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
                matches!(a, crate::body::CallTarget::Machine(_))
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
            (ExpressionKind::ByteElement { base: a, index: ai, element: ae }, ExpressionKind::ByteElement { base: b, index: bi, element: be }) => ae == be && both(a, b) && both(ai, bi),
            (ExpressionKind::CollectionView { base: a, access: aa, element: ae }, ExpressionKind::CollectionView { base: b, access: ba, element: be }) => aa == ba && ae == be && both(a, b),
            (ExpressionKind::CollectionSubslice { base: a, start: ast, end: ae }, ExpressionKind::CollectionSubslice { base: b, start: bst, end: be }) => both(a,b) && match (ast,bst) { (Some(a),Some(b)) => both(a,b), (None,None) => true, _ => false } && match (ae,be) { (Some(a),Some(b)) => both(a,b), (None,None) => true, _ => false },
            (
                ExpressionKind::Record { fields: a, .. },
                ExpressionKind::Record { fields: b, .. },
            ) => {
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
                    ..
                },
                ExpressionKind::Case {
                    case: b_case,
                    fields: b,
                    ..
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
                                    (Selector::Case(a), Selector::Case(b)) => a.case == b.case
                                        && match (&a.condition, &b.condition) {
                                            (Some(a), Some(b)) => both(a,b),
                                            (None, None) => true,
                                            _ => false,
                                        },
                                    (Selector::Value(a), Selector::Value(b))
                                    | (Selector::Guard(a), Selector::Guard(b)) => both(a, b),
                                    (Selector::Case(_), _)
                                    | (Selector::Value(_), _)
                                    | (Selector::Guard(_), _) => false,
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
                | ExpressionKind::TestMembership { .. }
                | ExpressionKind::SavedSubject(_)
                | ExpressionKind::Integer(_)
                | ExpressionKind::Float(_)
                | ExpressionKind::ErasedApplication(_)
                | ExpressionKind::ScalarApplication { .. }
            | ExpressionKind::ByteLiteral(_)
        | ExpressionKind::StorageLocation { .. }
            | ExpressionKind::CollectionMeasure { .. }
                | ExpressionKind::PackedInteger { .. }
            | ExpressionKind::ByteSequencePredicate { .. }
                | ExpressionKind::ByteElement { .. }
            | ExpressionKind::CollectionView { .. }
            | ExpressionKind::CollectionSubslice { .. }
                | ExpressionKind::Zero
        | ExpressionKind::Boolean(_)
                | ExpressionKind::MachineValue { .. }
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
                // Each evaluation forms its own loan and temporary home.
                | ExpressionKind::OwnedTemporary { .. }
                | ExpressionKind::Borrow { .. }
                | ExpressionKind::Referent(_),
                _,
            ) => false,
        }
}

impl Program<'_> {
    /// An operator operand is a value: `expressions.md`, "Explicit
    /// dereference", lists operator operands among the places where Omega
    /// never inserts a dereference, so `v + 1u8` with `v: @u8` is wrong and
    /// `$v + 1u8` is right. The operand was read without a receiving type,
    /// so the reader recognizes the two implicit routes here: a borrowed
    /// formal named whole, and a held reference crossed to its scalar.
    fn require_explicit_operand(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        handle: ExpressionHandle,
        value: &Expression,
    ) -> Checked<()> {
        let Some(ValueType::Scalar(scalar, _)) = value.result else {
            return Ok(());
        };
        let crossed = matches!(value.kind, ExpressionKind::Referent(_));
        let implicit = match self.node(id, handle)? {
            ExpressionNode::Name(path) => {
                crossed
                    || self
                        .slot_of(id, slots, path)
                        .is_ok_and(|slot| slots[slot].borrowed.is_some())
            }
            ExpressionNode::Member(_) => crossed,
            _ => false,
        };
        if implicit {
            return Err(super::rejections::implicit_dereference(
                id,
                value.source,
                scalar,
            ));
        }
        Ok(())
    }

    /// Form the predicate on its declared carrier. Routed membership also
    /// needs evidence for this completed operand at the test's execution point.
    pub(super) fn require_membership_carrier(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        test: crate::source_state::expression::ExecutableDomainTest,
    ) -> Checked<Option<(Expression, crate::body::DomainInstance)>> {
        let definition = if test.application.is_valid() {
            Some(self.domain_theory(
                id,
                crate::body::DomainInstance {
                    declaration: test.domain,
                    identity: test.application,
                },
            )?)
        } else {
            self.typed.domain_definition(test.domain)
        };
        let Some(definition) = definition else {
            return Err(refuse_at(
                id,
                Unserved::Typing("membership in a domain without its declaration"),
            ));
        };
        let retain_domain = definition.requires_result_membership();
        // A checked unrestricted plan can leave an ordinary Boolean predicate.
        // Keep independent access and content evidence when the plan needs it.
        let needs_evidence = !matches!(
            definition.issuance(),
            crate::source_state::domain::DomainIssuance::Unrestricted
        ) || definition.requires_access_retention()
            || !definition.content_projections.is_empty();
        if !self.typed.domain_type_parameters(definition).is_empty()
            || !definition.index_arguments.is_empty()
        {
            if needs_evidence {
                return Err(refuse_at(
                    id,
                    Unserved::Expression("routed membership without its applied domain"),
                ));
            }
            return Ok(None);
        }
        let target_type = definition.target_type;
        let carrier = self.value_type(id, target_type)?;
        let landing = self
            .wholly_anonymous(test.subject)
            .then_some(carrier.erased());
        let subject = self.expression(id, slots, test.subject, landing)?;
        self.checked_membership_carrier(id, definition, carrier, subject.result, subject.source)?;
        let domain = crate::body::DomainInstance {
            declaration: test.domain,
            identity: if test.application.is_valid() {
                test.application
            } else {
                definition.semantic_id
            },
        };
        if retain_domain || test.application.is_valid() {
            let actual = subject.result.ok_or_else(|| {
                refuse_at(
                    id,
                    Unserved::Expression("domain membership without a value"),
                )
            })?;
            self.form_domain(
                id,
                domain,
                actual,
                subject.source,
                super::types::DomainUse::test(needs_evidence),
            )?;
        }
        Ok(needs_evidence.then_some((subject, domain)))
    }

    pub(super) fn checked_membership_carrier(
        &mut self,
        id: MachineId,
        definition: &crate::source_state::domain::DomainDefinition,
        carrier: ValueType,
        actual: Option<ValueType>,
        at: SourceOrigin,
    ) -> Checked<()> {
        if let (Some(ValueType::Scalar(found, _)), ValueType::Scalar(wanted, _)) = (actual, carrier)
            && found != wanted
        {
            return Err(super::rejections::membership_carrier(id, at, found, wanted));
        }
        let typed = self.typed;
        let mismatch = || {
            let kind = if definition.alias.is_some() {
                "domain alias"
            } else {
                "domain"
            };
            reject(
                id,
                at,
                format!(
                    "membership in {kind} `{}` requires carrier `{}`",
                    definition.name,
                    typed.display_type_reference(definition.target_type)
                ),
            )
        };
        let actual = actual.ok_or_else(mismatch)?;
        let plain = carrier.erased().qualified(crate::body::Qualification::NONE);
        let exact =
            plain.is_some() && actual.erased().qualified(crate::body::Qualification::NONE) == plain;
        if !exact && !self.represented_domain_carrier(id, actual, carrier, definition)? {
            return Err(mismatch());
        }
        let supplied = actual.qualification();
        let required = carrier.qualification();
        if let Some(missing) = required
            .domains
            .members()
            .find(|member| !supplied.domains.contains(*member))
        {
            return Err(reject(
                id,
                at,
                format!(
                    "the carrier of domain `{}` requires membership in {}",
                    definition.name,
                    self.domain_named(missing),
                ),
            ));
        }
        if let Some(policy) = required.policy
            && supplied.policy != Some(policy)
        {
            return Err(reject(
                id,
                at,
                format!(
                    "the carrier of domain `{}` requires the `{}` arithmetic policy",
                    definition.name,
                    crate::body::Policy::domain(Some(policy)).name(),
                ),
            ));
        }
        Ok(())
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

    /// Integer comparisons preserve each operand's carrier and compare exact
    /// values, including pairs with no common fixed carrier. Arithmetic still
    /// needs one carrier. Native support is judged by the native compiler.
    #[test]
    fn a_comparison_preserves_operands_of_different_integer_carriers() {
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
            ),
            Ok(boolean)
        );
        // The relation needs no common carrier or implicit cast.
        assert_eq!(
            typed(BinaryOperator::Less, u64_, i32_, neither),
            Ok(boolean)
        );
    }
}
