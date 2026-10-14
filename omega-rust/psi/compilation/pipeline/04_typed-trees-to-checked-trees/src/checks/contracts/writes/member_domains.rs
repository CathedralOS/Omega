//! Predicate membership and comparison vocabulary: case decisions, literal
//! member values, and subject/domain qualification proofs for constructed
//! field values.

use checked_trees::{CheckFacts, FlowStateFact};
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

mod contextual;

/// `self in T::Case` lowers to `self == T::Case` (unions become `||`); the
/// fact is decided by the construction's own selected case or by live
/// `AssignedCase`/guard evidence at the value's place. Member comparisons
/// (`self.f == 0`, `self.flag`) read the literal's field value or the place's
/// scalar snapshot; `self.f in D` recurses into the field initializer or the
/// nested place. Mutation invalidation retires both evidence forms, so stale
/// establishment never survives a rewrite, and a foreign case or owner fails
/// the exact variant-symbol comparison.
pub(super) fn initializer_satisfies_predicate_domain(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    value: ExpressionHandle,
    domain_symbol: SymbolHandle,
    active: &mut Vec<SymbolHandle>,
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    if !domain_symbol.is_valid() || active.contains(&domain_symbol) {
        return false;
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
    let Some(domain) = program.domain_definition(domain_symbol) else {
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
    value_satisfies_domain_predicates_in_contexts(
        program,
        facts,
        state,
        statement_index,
        value,
        domain_symbol,
        &contexts,
        active,
        facts
            .flow
            .state_statement(state, statement_index)
            .filter(|statement| facts.flow.control.operands.is_valid(statement.stored_value))
            .map(|statement| facts.flow.control.operands.get(statement.stored_value)),
        frames,
    )
}

/// Predicate truth is independent of the authority to introduce membership.
/// Result admission supplies its live exit contexts; initializers supply the
/// contexts before the destination exists. Neither may assume the promised
/// qualification. Indexed theories and aliases need their own exact instance
/// substitution and cannot be discharged by this declaration-only reader.
pub(in crate::checks::contracts) fn value_satisfies_domain_predicates_in_contexts(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    value: ExpressionHandle,
    domain_symbol: SymbolHandle,
    contexts: &[facts::FactContextHandle],
    active: &mut Vec<SymbolHandle>,
    capture: Option<&checked_trees::FlowOperandFact>,
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    if active.contains(&domain_symbol)
        || !typed_trees::domain::has_symbol_only_proof(program, domain_symbol)
    {
        return false;
    }
    let Some(domain) = program.domain_definition(domain_symbol) else {
        return false;
    };
    if domain.alias.is_some() {
        return false;
    }
    let capture = capture.filter(|capture| {
        capture.expression == value
            && capture.state_symbol == state.state_symbol
            && capture.statement_index == statement_index
    });
    active.push(domain_symbol);
    let satisfied = program.proof_facts(domain).iter().all(|fact| match fact {
        typed_trees::domain::ProofFact::Expression(expression) => {
            predicate_expression_holds_on_value(
                program,
                facts,
                state,
                statement_index,
                contexts,
                value,
                *expression,
                domain,
                capture,
                frames,
            )
        }
        typed_trees::domain::ProofFact::Membership(membership) => {
            membership.domain_arguments.is_empty()
                && member_subject_satisfies_domain(
                    program,
                    facts,
                    state,
                    statement_index,
                    contexts,
                    value,
                    membership.value,
                    membership.domain_symbol,
                    domain,
                    active,
                    capture,
                    frames,
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
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    contexts: &[facts::FactContextHandle],
    value: ExpressionHandle,
    expression: ExpressionHandle,
    domain: &typed_trees::domain::DomainDefinition,
    capture: Option<&checked_trees::FlowOperandFact>,
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    if matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Binary(_)
    ) && !validation::has_builtin_domain_decomposed_guard_meaning(program, domain, expression)
    {
        return false;
    }
    if contextual::proves(
        program,
        facts,
        state,
        statement_index,
        contexts,
        value,
        expression,
        domain,
        capture,
    ) {
        return true;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(value) => *value,
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
                capture,
                frames,
            ) && predicate_expression_holds_on_value(
                program,
                facts,
                state,
                statement_index,
                contexts,
                value,
                binary.right,
                domain,
                capture,
                frames,
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
                capture,
                frames,
            ) || predicate_expression_holds_on_value(
                program,
                facts,
                state,
                statement_index,
                contexts,
                value,
                binary.right,
                domain,
                capture,
                frames,
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
                capture,
                frames,
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
                capture,
                frames,
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
            let Some(mut place) = validation::canonical_place_from_expression_in_state(
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
    program: &dyn ::validation::CheckingInput,
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
        let Some(case_symbol) = case_value_symbol(program, case_expression).or_else(|| {
            program
                .expression_table
                .name_path_node(case_expression)
                .map(|path| path.symbol)
        }) else {
            continue;
        };
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

/// A closed payload-free case value has identity independent of a runtime
/// observation. Payload-bearing constructions cannot be compared by tag alone.
fn case_value_symbol(
    program: &dyn validation::CheckingInput,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    let literal = program.expression_table.struct_literal_node(expression)?;
    if !literal.fields.is_empty() {
        return None;
    }
    let case = literal.case_symbol?;
    let definition = program.data_definition_by_symbol(literal.type_symbol)?;
    let members = program.data_members(definition);
    // Omitted zero-valid fields still belong to the value. Authored field
    // count alone cannot establish that equality is determined by the tag.
    if members.iter().any(|member| member.as_field().is_some()) {
        return None;
    }
    members.iter().find_map(|member| {
        let variant = member.as_variant()?;
        (variant.symbol == case && program.data_payload_fields(variant).is_empty()).then_some(case)
    })
}

/// Whether the subject `value` followed by `segments` currently selects
/// `case_symbol`. A place answers from live case evidence; a construction
/// literal answers from its own selected variant and member initializers.
fn selected_case_is(
    program: &dyn ::validation::CheckingInput,
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
    let subject = member_place(program, state, statement_index, value, segments)?;
    // An unknown case is neither equality nor inequality evidence.
    [true, false].into_iter().find(|&selected| {
        crate::flow::place_case_has_value(
            program,
            &facts.semantic,
            contexts,
            state.machine_symbol,
            state.state_symbol,
            statement_index,
            &subject,
            case_symbol,
            selected,
        )
    })
}

/// The selected variant of a construction expression: a cased struct literal,
/// a `T::Case(...)` call, or a bare variant name. Record literals and
/// non-construction expressions have no selected case.
fn construction_selected_case(
    program: &dyn ::validation::CheckingInput,
    value: ExpressionHandle,
) -> Option<SymbolHandle> {
    match program.expression_table.expression(value) {
        ExpressionNode::StructLiteral(literal) => literal.case_symbol,
        ExpressionNode::Call(call) => {
            let receiver = program.expression_table.name_path_node(call.receiver)?;
            let data_definition = program.data_definition_by_symbol(receiver.symbol)?;
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
    program: &dyn ::validation::CheckingInput,
    value: ExpressionHandle,
    segments: &[facts::PlaceSegment],
) -> Option<ExpressionHandle> {
    let mut current = value;
    for segment in segments {
        let facts::PlaceSegment::Field { symbol } = segment else {
            return None;
        };
        let literal = program.expression_table.struct_literal_node(current)?;
        current = program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .find(|field| field.field_symbol == *symbol)
            .map(|field| field.value)?;
    }
    Some(current)
}

/// Bind a domain-relative member to its own observation namespace. A completed
/// constructor owns detached member snapshots; reading a child's former source
/// would replace its value with writes performed by a later sibling.
fn member_place(
    program: &dyn ::validation::CheckingInput,
    state: &FlowStateFact,
    statement_index: usize,
    value: ExpressionHandle,
    segments: &[facts::PlaceSegment],
) -> Option<validation::CanonicalPlace> {
    let mut place = validation::canonical_place_from_expression_in_state(
        program,
        state.state_symbol,
        statement_index,
        value,
    )?;
    place.extend_segments(segments);
    Some(place)
}

/// A `self`-rooted member path's current scalar: live assignment evidence at
/// the place's member segments, or the literal field's own constant.
#[allow(clippy::too_many_arguments)]
fn member_scalar_at_subject(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    contexts: &[facts::FactContextHandle],
    value: ExpressionHandle,
    member_path: ExpressionHandle,
    domain: &typed_trees::domain::DomainDefinition,
    capture: Option<&checked_trees::FlowOperandFact>,
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<facts::ScalarValue> {
    let segments =
        crate::flow::relative_place_segments_from_expression(program, member_path, domain)?;
    if let Some(scalar) = literal_member_value(program, value, &segments)
        .and_then(|member| expression_constant_scalar(program, member, state.machine_symbol))
    {
        return Some(scalar);
    }
    let subject = member_place(program, state, statement_index, value, &segments)?;
    crate::values::scalar_value_at_place(
        program,
        &facts.semantic,
        contexts
            .iter()
            .map(|context| facts.semantic.contexts.get(*context)),
        &subject,
    )
    .or_else(|| contextual::captured_scalar(program, facts, state, capture?, &segments, frames?))
}

fn expression_constant_scalar(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    owner: SymbolHandle,
) -> Option<facts::ScalarValue> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(value) => Some(facts::ScalarValue::Boolean(*value)),
        _ => program
            .closed_integer_value_in(expression, owner)
            .map(|value| facts::ScalarValue::Integer(value.value)),
    }
}

/// A comparison where one operand is a `self`-rooted member path and the
/// other is a constant: `self.a == 0`, `0 < self.b`, `self.flag`. Compares
/// the member's live scalar or literal value against the constant; when the
/// constant leads, the operator is mirrored so the member stays the subject.
#[allow(clippy::too_many_arguments)]
fn member_comparison_holds_on_value(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    contexts: &[facts::FactContextHandle],
    value: ExpressionHandle,
    operator: typed_trees::expression::BinaryOperator,
    left: ExpressionHandle,
    right: ExpressionHandle,
    domain: &typed_trees::domain::DomainDefinition,
    capture: Option<&checked_trees::FlowOperandFact>,
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    use typed_trees::expression::BinaryOperator;
    for (member, constant, operator) in
        [(left, right, operator), (right, left, operator.mirrored())]
    {
        if crate::flow::relative_place_segments_from_expression(program, member, domain).is_none() {
            continue;
        }
        let Some(expected) = expression_constant_scalar(program, constant, domain.symbol) else {
            continue;
        };
        let Some(actual) = member_scalar_at_subject(
            program,
            facts,
            state,
            statement_index,
            contexts,
            value,
            member,
            domain,
            capture,
            frames,
        ) else {
            continue;
        };
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
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    contexts: &[facts::FactContextHandle],
    value: ExpressionHandle,
    member: ExpressionHandle,
    domain_symbol: SymbolHandle,
    domain: &typed_trees::domain::DomainDefinition,
    active: &mut Vec<SymbolHandle>,
    capture: Option<&checked_trees::FlowOperandFact>,
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let Some(segments) =
        crate::flow::relative_place_segments_from_expression(program, member, domain)
    else {
        return false;
    };
    if let Some(member_value) = literal_member_value(program, value, &segments) {
        let child = capture.and_then(|capture| {
            crate::flow::captured_operand_at_path(
                program,
                &facts.flow.control,
                capture,
                capture.type_reference,
                &segments,
            )
        });
        if capture.is_some() && child.is_none() {
            return false;
        }
        let captured_contexts = child.map(|(child, reference)| {
            crate::flow::operand_contexts(
                program,
                &facts.flow,
                &facts.semantic,
                child,
                reference,
                capture.map_or(child.constraints, |capture| capture.constraints),
            )
        });
        // A later sibling may establish a qualification on the former source.
        // The installed child retains only its own captured membership.
        let contexts = captured_contexts.as_deref().unwrap_or(contexts);
        let identity = program
            .domain_definition(domain_symbol)
            .map_or(language_semantics::SemanticDomainId::NULL, |domain| {
                domain.semantic_id
            });
        // Proving the outer theory cannot establish a routed child. A child
        // carries its own exact membership, or independently proves a theory
        // that does not require authorized provenance.
        return super::domain_implications::value_proves_qualification_in_contexts(
            program,
            facts,
            state,
            statement_index,
            member_value,
            domain_symbol,
            identity,
            contexts,
        ) || (!validation::field_domains::domain_requires_provenance(
            program,
            domain_symbol,
        ) && value_satisfies_domain_predicates_in_contexts(
            program,
            facts,
            state,
            statement_index,
            member_value,
            domain_symbol,
            contexts,
            active,
            child.map(|(capture, _)| capture),
            frames,
        ));
    }
    let Some(place) = validation::canonical_place_from_expression_in_state(
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
