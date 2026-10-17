//! Predicate membership and comparison vocabulary: case decisions, literal
//! member values, and subject/domain qualification proofs for constructed
//! field values.

use checked_trees::{CheckFacts, FlowStateFact};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;

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
    if active.contains(&domain_symbol) {
        return false;
    }
    let Some(domain) = program.domain_definition(domain_symbol) else {
        return false;
    };
    if domain.alias.is_some() {
        return false;
    }
    // An empty conjunction owes no predicate substitution, including indexed
    // marker theories. The caller separately proves exact membership or the
    // authorized establishment route; this truth cannot mint that authority.
    if program.proof_facts(domain).is_empty() {
        return true;
    }
    if !typed_trees::domain::has_symbol_only_proof(program, domain_symbol) {
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

/// Substitute the completed value into the domain's selected proof expression.
/// Float observation and byte-class evidence keep their own semantic owners;
/// scalar and case predicates use the same ordered-value judgment as contracts.
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
    let (Some(machine), Some(state_declaration)) = (
        program.machine_by_symbol(state.machine_symbol),
        program.state_by_symbol(state.state_symbol),
    ) else {
        return false;
    };
    // Judge the complete proposition before asking specialized leaf owners.
    // An observed disjunction need not establish either alternative globally.
    if validation::completed_value_proves_domain_predicate(
        program,
        machine,
        state_declaration,
        statement_index,
        value,
        domain.symbol,
        expression,
        &[],
    ) {
        return true;
    }
    // Logical composition keeps the semantic owners of its children: a byte
    // classifier does not become an integer predicate merely by joining one.
    if let Some(binary) = program.expression_table.binary_node(expression) {
        use typed_trees::expression::BinaryOperator;
        let proves = |expression| {
            predicate_expression_holds_on_value(
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
            )
        };
        match binary.operator {
            BinaryOperator::And => return proves(binary.left) && proves(binary.right),
            BinaryOperator::Or => return proves(binary.left) || proves(binary.right),
            _ => {}
        }
    }
    if matches!(
        program.primitive_type_reference(domain.target_type),
        Some(typed_trees::types::PrimitiveType::F32 | typed_trees::types::PrimitiveType::F64)
    ) {
        return super::super::float_predicate_domains::expression_establishes_domain_predicate(
            program,
            facts,
            contexts,
            machine,
            state_declaration,
            statement_index,
            value,
            domain,
            expression,
        );
    }
    if let Some((required, subject)) =
        typed_trees::byte_predicates::byte_predicate_call(program, expression)
    {
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
        return super::super::prover::byte_predicate_proven_at_place(
            program,
            &facts.semantic,
            contexts,
            &place,
            required,
        );
    }
    false
}

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
