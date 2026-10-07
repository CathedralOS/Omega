//! Predicate membership and comparison vocabulary: case decisions, literal
//! member values, and subject/domain qualification proofs for constructed
//! field values.

use checked_trees::{CheckFacts, FlowStateFact};
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

/// `self in T::Case` lowers to `self == T::Case` (unions become `||`); the
/// fact is decided by the construction's own selected case or by live
/// `AssignedCase`/guard evidence at the value's place. Member comparisons
/// (`self.f == 0`, `self.flag`) read the literal's field value or the place's
/// scalar snapshot; `self.f in D` recurses into the field initializer or the
/// nested place. Mutation invalidation retires both evidence forms, so stale
/// establishment never survives a rewrite, and a foreign case or owner fails
/// the exact variant-symbol comparison.
pub(super) fn initializer_satisfies_predicate_domain(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    value: ExpressionHandle,
    domain_symbol: SymbolHandle,
    active: &mut Vec<SymbolHandle>,
) -> bool {
    if !domain_symbol.is_valid() || active.contains(&domain_symbol) {
        return false;
    }
    // An explicit `x as T in D` mint declares the qualification directly;
    // the staged mint fence at validation owns judging its legality.
    if let ExpressionNode::Cast(cast) = program.expression_table.expression(value)
        && cast.semantic_domain_symbol == domain_symbol
    {
        return true;
    }
    // A domain-owned operator is the sanctioned producer for its domain: the
    // selected candidate's domain qualifies the result, matching the trust
    // basis of a declared `-> T in D` call signature.
    if facts
        .operators
        .resolved_uses()
        .filter(|operator_use| operator_use.expression == value)
        .filter_map(|operator_use| facts.operators.selected_candidate(operator_use))
        .any(|candidate| candidate.domain_symbol == domain_symbol)
    {
        return true;
    }
    let Some(domain) = program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == domain_symbol)
    else {
        return false;
    };
    if !domain.predicate_body.is_present() {
        return false;
    }
    let entry_constraints = facts
        .flow
        .state_statement(state, statement_index)
        .map(|statement| statement.entry_constraints)
        .unwrap_or(state.entry_constraints);
    let contexts = facts
        .flow
        .semantic_constraint_contexts(entry_constraints)
        .collect::<Vec<_>>();
    active.push(domain_symbol);
    let satisfied = program.proof_facts(domain).iter().all(|fact| match fact {
        typed_trees::domain::ProofFact::Expression(expression) => {
            predicate_expression_holds_on_value(
                program,
                facts,
                state,
                statement_index,
                &contexts,
                value,
                *expression,
                domain,
            )
        }
        typed_trees::domain::ProofFact::Membership(membership) => {
            membership.domain_arguments.is_empty()
                && member_subject_satisfies_domain(
                    program,
                    facts,
                    state,
                    statement_index,
                    &contexts,
                    value,
                    membership.value,
                    membership.domain_symbol,
                    domain,
                    active,
                )
        }
        typed_trees::domain::ProofFact::Proposition(_) => false,
    });
    active.pop();
    satisfied
}

/// Evaluate one `requires` expression with `self` bound to `value`: `&&`/`||`
/// compose, `self == T::Case` compares the subject's selected case, and
/// `self.f` predicates compare a member's scalar. Anything else is not
/// decidable here and falls back to the membership proof.
fn predicate_expression_holds_on_value(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    contexts: &[facts::FactContextHandle],
    value: ExpressionHandle,
    expression: ExpressionHandle,
    domain: &typed_trees::domain::DomainDefinition,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary)
            if binary.operator == typed_trees::expression::BinaryOperator::And =>
        {
            predicate_expression_holds_on_value(
                program,
                facts,
                state,
                statement_index,
                contexts,
                value,
                binary.left,
                domain,
            ) && predicate_expression_holds_on_value(
                program,
                facts,
                state,
                statement_index,
                contexts,
                value,
                binary.right,
                domain,
            )
        }
        ExpressionNode::Binary(binary)
            if binary.operator == typed_trees::expression::BinaryOperator::Or =>
        {
            predicate_expression_holds_on_value(
                program,
                facts,
                state,
                statement_index,
                contexts,
                value,
                binary.left,
                domain,
            ) || predicate_expression_holds_on_value(
                program,
                facts,
                state,
                statement_index,
                contexts,
                value,
                binary.right,
                domain,
            )
        }
        ExpressionNode::Binary(binary) => {
            if let Some(selected) = case_membership_decision(
                program,
                facts,
                state,
                statement_index,
                contexts,
                value,
                binary.left,
                binary.right,
                domain,
            ) {
                return match binary.operator {
                    typed_trees::expression::BinaryOperator::Equal => selected,
                    typed_trees::expression::BinaryOperator::NotEqual => !selected,
                    _ => false,
                };
            }
            member_comparison_holds_on_value(
                program,
                facts,
                state,
                statement_index,
                contexts,
                value,
                binary.operator,
                binary.left,
                binary.right,
                domain,
            )
        }
        ExpressionNode::Member(_) | ExpressionNode::Name(_) => {
            member_scalar_at_subject(
                program,
                facts,
                state,
                statement_index,
                contexts,
                value,
                expression,
                domain,
            ) == Some(facts::ScalarValue::Boolean(true))
        }
        // `pred(self)` — a byte-predicate fact over the domain subject: hold
        // when the subject's own place carries a minted per-byte class
        // implying the probe (a validating guard or `requires` boundary
        // upstream). `self` binds to `value`, so the subject's path below
        // `self` projects onto `value`'s place.
        ExpressionNode::Call(_) => {
            let Some((required, subject)) =
                typed_trees::byte_predicates::byte_predicate_call(program, expression)
            else {
                return false;
            };
            let Some(segments) =
                crate::flow::relative_place_segments_from_expression(program, subject, domain)
            else {
                return false;
            };
            let Some(mut place) = crate::flow::canonical_place_from_expression_in_state(
                program,
                state.state_symbol,
                statement_index,
                value,
            ) else {
                return false;
            };
            if !matches!(place.root, facts::PlaceRoot::Symbol(_)) {
                return false;
            }
            place.extend_segments(&segments);
            super::super::prover::byte_predicate_proven_at_place(
                program,
                &facts.semantic,
                contexts,
                &place,
                required,
            )
        }
        _ => false,
    }
}

/// When a comparison's operands split into a `self`-rooted path and a name
/// resolving to a variant symbol, decide whether the subject's selected case
/// is that variant. Returns `None` when the operands are not that shape or no
/// case evidence exists for the subject.
#[allow(clippy::too_many_arguments)]
fn case_membership_decision(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    contexts: &[facts::FactContextHandle],
    value: ExpressionHandle,
    left: ExpressionHandle,
    right: ExpressionHandle,
    domain: &typed_trees::domain::DomainDefinition,
) -> Option<bool> {
    for (subject, case_expression) in [(left, right), (right, left)] {
        let ExpressionNode::Name(path) = program.expression_table.expression(case_expression)
        else {
            continue;
        };
        let case_symbol = path.symbol;
        if !case_symbol.is_valid()
            || program.symbols.get(case_symbol).kind != symbols::SymbolKind::Variant
        {
            continue;
        }
        let segments =
            crate::flow::relative_place_segments_from_expression(program, subject, domain)?;
        return selected_case_is(
            program,
            facts,
            state,
            statement_index,
            contexts,
            value,
            &segments,
            case_symbol,
        );
    }
    None
}

/// Whether the subject `value` followed by `segments` currently selects
/// `case_symbol`. A place answers from live case evidence; a construction
/// literal answers from its own selected variant and member initializers.
fn selected_case_is(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    contexts: &[facts::FactContextHandle],
    value: ExpressionHandle,
    segments: &[facts::PlaceSegment],
    case_symbol: SymbolHandle,
) -> Option<bool> {
    if let Some(selected) = literal_member_value(program, value, segments)
        .and_then(|member| construction_selected_case(program, member))
    {
        return Some(selected == case_symbol);
    }
    let place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.state_symbol,
        statement_index,
        value,
    )?;
    if !matches!(place.root, facts::PlaceRoot::Symbol(_)) {
        return None;
    }
    let mut subject = place.clone();
    subject.extend_segments(segments);
    Some(crate::flow::place_case_has_value(
        program,
        &facts.semantic,
        contexts,
        state.machine_symbol,
        state.state_symbol,
        statement_index,
        &subject,
        case_symbol,
        true,
    ))
}

/// The selected variant of a construction expression: a cased struct literal,
/// a `T::Case(...)` call, or a bare variant name. Record literals and
/// non-construction expressions have no selected case.
fn construction_selected_case(
    program: &typed_trees::TypedTrees,
    value: ExpressionHandle,
) -> Option<SymbolHandle> {
    match program.expression_table.expression(value) {
        ExpressionNode::StructLiteral(literal) => literal.case_symbol,
        ExpressionNode::Call(call) => {
            let ExpressionNode::Name(receiver) = program.expression_table.expression(call.receiver)
            else {
                return None;
            };
            let data_definition = program
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == receiver.symbol)?;
            program
                .data_members(data_definition)
                .iter()
                .find_map(|member| match member {
                    typed_trees::data::DataMember::Variant(variant)
                        if variant.name.as_str() == call.target.as_str() =>
                    {
                        Some(variant.symbol)
                    }
                    _ => None,
                })
        }
        ExpressionNode::Name(path)
            if path.symbol.is_valid()
                && program.symbols.get(path.symbol).kind == symbols::SymbolKind::Variant =>
        {
            Some(path.symbol)
        }
        _ => None,
    }
}

/// Follow `self`-relative field segments into a struct literal's member
/// initializers. An empty segment list is the literal itself; a non-field
/// segment or a non-literal member value cannot be descended into.
fn literal_member_value(
    program: &typed_trees::TypedTrees,
    value: ExpressionHandle,
    segments: &[facts::PlaceSegment],
) -> Option<ExpressionHandle> {
    let mut current = value;
    for segment in segments {
        let facts::PlaceSegment::Field { symbol } = segment else {
            return None;
        };
        let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(current)
        else {
            return None;
        };
        current = program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .find(|field| field.field_symbol == *symbol)
            .map(|field| field.value)?;
    }
    Some(current)
}

/// A `self`-rooted member path's current scalar: live assignment evidence at
/// the place's member segments, or the literal field's own constant.
#[allow(clippy::too_many_arguments)]
fn member_scalar_at_subject(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    contexts: &[facts::FactContextHandle],
    value: ExpressionHandle,
    member_path: ExpressionHandle,
    domain: &typed_trees::domain::DomainDefinition,
) -> Option<facts::ScalarValue> {
    let segments =
        crate::flow::relative_place_segments_from_expression(program, member_path, domain)?;
    if let Some(scalar) = literal_member_value(program, value, &segments)
        .and_then(|member| expression_constant_scalar(program, member))
    {
        return Some(scalar);
    }
    let place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.state_symbol,
        statement_index,
        value,
    )?;
    if !matches!(place.root, facts::PlaceRoot::Symbol(_)) {
        return None;
    }
    let mut subject = place.clone();
    subject.extend_segments(&segments);
    crate::values::scalar_value_at_place(
        program,
        &facts.semantic,
        contexts
            .iter()
            .map(|context| facts.semantic.contexts.get(*context)),
        &subject,
    )
}

fn expression_constant_scalar(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<facts::ScalarValue> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(value) => Some(facts::ScalarValue::Boolean(*value)),
        _ => program
            .expression_table
            .constant_integer_value(expression)
            .map(|value| facts::ScalarValue::Integer(numerics::bignum::BigInt::from_i64(value))),
    }
}

/// A comparison where one operand is a `self`-rooted member path and the
/// other is a constant: `self.a == 0`, `0 < self.b`, `self.flag`. Compares
/// the member's live scalar or literal value against the constant; when the
/// constant leads, the operator is mirrored so the member stays the subject.
#[allow(clippy::too_many_arguments)]
fn member_comparison_holds_on_value(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    contexts: &[facts::FactContextHandle],
    value: ExpressionHandle,
    operator: typed_trees::expression::BinaryOperator,
    left: ExpressionHandle,
    right: ExpressionHandle,
    domain: &typed_trees::domain::DomainDefinition,
) -> bool {
    use typed_trees::expression::BinaryOperator;
    for (member, constant, operator) in [
        (left, right, operator),
        (
            right,
            left,
            match operator {
                BinaryOperator::Less => BinaryOperator::Greater,
                BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
                BinaryOperator::Greater => BinaryOperator::Less,
                BinaryOperator::GreaterOrEqual => BinaryOperator::LessOrEqual,
                other => other,
            },
        ),
    ] {
        if crate::flow::relative_place_segments_from_expression(program, member, domain).is_none()
            || expression_constant_scalar(program, constant).is_none()
        {
            continue;
        }
        let Some(actual) = member_scalar_at_subject(
            program,
            facts,
            state,
            statement_index,
            contexts,
            value,
            member,
            domain,
        ) else {
            continue;
        };
        let expected = expression_constant_scalar(program, constant).expect("checked above");
        return match (&actual, &expected) {
            (facts::ScalarValue::Integer(actual), facts::ScalarValue::Integer(expected)) => {
                match operator {
                    BinaryOperator::Equal => actual == expected,
                    BinaryOperator::NotEqual => actual != expected,
                    BinaryOperator::Less => actual < expected,
                    BinaryOperator::LessOrEqual => actual <= expected,
                    BinaryOperator::Greater => actual > expected,
                    BinaryOperator::GreaterOrEqual => actual >= expected,
                    _ => false,
                }
            }
            (facts::ScalarValue::Boolean(actual), facts::ScalarValue::Boolean(expected)) => {
                match operator {
                    BinaryOperator::Equal => actual == expected,
                    BinaryOperator::NotEqual => actual != expected,
                    _ => false,
                }
            }
            _ => false,
        };
    }
    false
}

/// `self.f in D` on the initializer: a literal descends into the field's own
/// initializer and re-discharge; a place proves the nested place's domain the
/// ordinary way.
#[allow(clippy::too_many_arguments)]
fn member_subject_satisfies_domain(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    contexts: &[facts::FactContextHandle],
    value: ExpressionHandle,
    member: ExpressionHandle,
    domain_symbol: SymbolHandle,
    domain: &typed_trees::domain::DomainDefinition,
    active: &mut Vec<SymbolHandle>,
) -> bool {
    let Some(segments) =
        crate::flow::relative_place_segments_from_expression(program, member, domain)
    else {
        return false;
    };
    if let Some(member_value) = literal_member_value(program, value, &segments) {
        return initializer_satisfies_predicate_domain(
            program,
            facts,
            state,
            statement_index,
            member_value,
            domain_symbol,
            active,
        ) || super::domain_implications::value_proves_domain_in_contexts(
            program,
            facts,
            state,
            statement_index,
            member_value,
            domain_symbol,
            contexts,
        );
    }
    let Some(place) = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.state_symbol,
        statement_index,
        value,
    ) else {
        return false;
    };
    let mut subject = place.clone();
    subject.extend_segments(&segments);
    super::super::prover::prove_domain_at_place(
        program,
        &facts.semantic,
        contexts,
        &subject,
        domain_symbol,
    )
}
