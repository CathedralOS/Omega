//! Storage membership consumes live facts, never a declaration or initializer.
//! Computed calls and casts retain their independently checked result contracts.
//! Routed domains still require live provenance, indexed domains require exact
//! instance identity, and payload reads require the currently selected case.
//! Recast sources and contextual predicate proofs retain their distinct duties.

use checked_trees::{CheckFacts, FlowStateFact};
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

/// Routed theories require the exact live instance at the initializer's actual
/// place. Read the completed operand snapshot, never the destination binding's
/// post-statement facts or places equated by their display names.
pub(super) fn value_proves_qualification(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    value: ExpressionHandle,
    domain: SymbolHandle,
    identity: language_semantics::SemanticDomainId,
) -> bool {
    if !value_case_path_is_selected(program, facts, state, statement_index, value) {
        return false;
    }
    if !validation::field_domains::domain_requires_provenance(program, domain) {
        return value_result_type_carries_domain(
            program,
            state,
            statement_index,
            value,
            domain,
            identity,
        ) || value_proves_domain(program, facts, state, statement_index, value, domain);
    }
    let Some(subject) = validation::canonical_place_from_expression_in_state(
        program,
        state.state_symbol,
        statement_index,
        value,
    ) else {
        return false;
    };
    let contexts = value_semantic_contexts(facts, state, statement_index, value);
    super::super::exits::exact_scalar_membership(
        program, facts, &contexts, &subject, domain, identity,
    )
}

/// Use the value's retained evaluation epoch, including constructed children
/// before later sibling effects. Statements without a stored operand retain
/// the existing completed-call or entry context.
fn value_semantic_contexts(
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    value: ExpressionHandle,
) -> Vec<facts::FactContextHandle> {
    if let Some(statement) = facts.flow.state_statement(state, statement_index) {
        match completed_operand(facts, statement.stored_value, value) {
            Ok(Some(operand)) => {
                return facts
                    .flow
                    .semantic_constraint_contexts(operand.constraints)
                    .collect();
            }
            // A shared syntax handle cannot choose between distinct captured
            // evaluations. An entry or last-call fallback cannot resolve that
            // ambiguity either, so it supplies no membership premise.
            Err(()) => return Vec::new(),
            Ok(None) => {}
        }
    }
    let contexts = facts
        .flow
        .control
        .calls
        .span_or_empty(state.calls)
        .iter()
        .rfind(|call| call.statement_index == statement_index)
        .map(|call| call.exit_semantic_contexts)
        .unwrap_or_else(|| {
            facts
                .flow
                .state_statement(state, statement_index)
                .map_or(state.entry_semantic_contexts, |statement| {
                    statement.entry_semantic_contexts
                })
        });
    facts
        .flow
        .contexts
        .semantic_context_refs
        .span_or_empty(contexts)
        .iter()
        .map(|reference| reference.context)
        .collect::<Vec<_>>()
}

/// Construction children are already evaluated observations, not syntax replay.
fn completed_operand(
    facts: &CheckFacts,
    handle: arena::Handle<checked_trees::FlowOperandFact>,
    expression: ExpressionHandle,
) -> Result<Option<&checked_trees::FlowOperandFact>, ()> {
    if !facts.flow.control.operands.is_valid(handle) {
        return Ok(None);
    }
    fn select(
        facts: &CheckFacts,
        handle: arena::Handle<checked_trees::FlowOperandFact>,
        expression: ExpressionHandle,
        selected: &mut arena::Handle<checked_trees::FlowOperandFact>,
    ) -> Result<(), ()> {
        if !facts.flow.control.operands.is_valid(handle) {
            return Err(());
        }
        let operand = facts.flow.control.operands.get(handle);
        if operand.expression == expression {
            if selected.is_valid() && *selected != handle {
                return Err(());
            }
            *selected = handle;
        }
        for child in facts
            .flow
            .control
            .operand_children
            .span_or_empty(operand.children)
        {
            select(facts, child.value, expression, selected)?;
        }
        Ok(())
    }
    let mut selected = arena::Handle::invalid();
    select(facts, handle, expression, &mut selected)?;
    Ok(selected
        .is_valid()
        .then(|| facts.flow.control.operands.get(selected)))
}

/// An extracted payload needs its current tag before any qualification rule
/// can consume its field facts. Use the same completed-operand contexts as
/// routed membership, never an initializer or a post-assignment destination.
pub(super) fn value_case_path_is_selected(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    value: ExpressionHandle,
) -> bool {
    let Some(subject) = validation::canonical_place_from_expression_in_state(
        program,
        state.state_symbol,
        statement_index,
        value,
    ) else {
        return true;
    };
    if !subject
        .segments
        .iter()
        .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. }))
    {
        return true;
    }
    let contexts = value_semantic_contexts(facts, state, statement_index, value);
    crate::flow::place_cases_are_selected(
        program,
        &facts.semantic,
        &contexts,
        state.machine_symbol,
        state.state_symbol,
        statement_index,
        &subject,
    )
}

/// Prove an assigned value from live membership or an independently checked
/// predicate. A declared field alone cannot license an inactive payload read.
pub(super) fn value_proves_domain(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    statement_index: usize,
    value: ExpressionHandle,
    domain_symbol: SymbolHandle,
) -> bool {
    let entry_constraints = facts
        .flow
        .state_statement(state_flow, statement_index)
        .map(|statement| statement.entry_constraints)
        .unwrap_or(state_flow.entry_constraints);
    let contexts = facts
        .flow
        .semantic_constraint_contexts(entry_constraints)
        .collect::<Vec<_>>();
    if !value_case_path_is_selected(program, facts, state_flow, statement_index, value) {
        return false;
    }
    value_proves_domain_in_contexts(
        program,
        facts,
        state_flow,
        statement_index,
        value,
        domain_symbol,
        &contexts,
    )
}

pub(in crate::checks::contracts) fn value_proves_qualification_in_contexts(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    value: ExpressionHandle,
    domain: SymbolHandle,
    identity: language_semantics::SemanticDomainId,
    contexts: &[facts::FactContextHandle],
) -> bool {
    let subject = validation::canonical_place_from_expression_in_state(
        program,
        state.state_symbol,
        statement_index,
        value,
    );
    if subject.as_ref().is_some_and(|subject| {
        !crate::flow::place_cases_are_selected(
            program,
            &facts.semantic,
            contexts,
            state.machine_symbol,
            state.state_symbol,
            statement_index,
            subject,
        )
    }) {
        return false;
    }
    if validation::field_domains::domain_requires_provenance(program, domain) {
        return subject.as_ref().is_some_and(|subject| {
            super::super::exits::exact_scalar_membership(
                program, facts, contexts, subject, domain, identity,
            )
        });
    }
    if value_result_type_carries_domain(program, state, statement_index, value, domain, identity) {
        return true;
    }
    if value_proves_domain_in_contexts(
        program,
        facts,
        state,
        statement_index,
        value,
        domain,
        contexts,
    ) {
        return true;
    }
    // Only a missing qualification needs predicate reconstruction. Consume
    // existing occurrence evidence first, without replaying the same prefix.
    // The provenance fence above still precedes this truth judgment.
    typed_trees::domain::has_symbol_only_proof(program, domain)
        && super::member_domains::value_satisfies_domain_predicates_in_contexts(
            program,
            facts,
            state,
            statement_index,
            value,
            domain,
            contexts,
            &mut Vec::new(),
            None,
            None,
        )
}

pub(super) fn value_proves_domain_in_contexts(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    statement_index: usize,
    value: ExpressionHandle,
    domain_symbol: SymbolHandle,
    contexts: &[facts::FactContextHandle],
) -> bool {
    if validation::field_domains::domain_requires_provenance(program, domain_symbol)
        || !typed_trees::domain::has_symbol_only_proof(program, domain_symbol)
    {
        return false;
    }
    if value_result_type_carries_domain(
        program,
        state_flow,
        statement_index,
        value,
        domain_symbol,
        language_semantics::SemanticDomainId::NULL,
    ) {
        return true;
    }
    if validation::field_domains::string_literal_expression_grants_domain(
        program,
        value,
        domain_symbol,
    ) {
        return true;
    }

    // Concat preserves a byte-predicate domain: a `left + right` whose two
    // operands are each provably in `domain_symbol` is itself in the domain, for
    // the recognized concat-preserving byte-predicates (valid_utf8/no_nul/
    // ascii_only/non_empty). This is the DOMAIN half of the rung-2 growth bound;
    // the capacity half -- that the result fits an owned `[u8; N]` carrier -- is
    // the separate length-fits check at the write site, so admitting the domain
    // here is not on its own a license to overflow the target.
    if let Some(binary) = program.expression_table.binary_node(value)
        && binary.operator == typed_trees::expression::BinaryOperator::Add
        && validation::field_domains::domain_is_concat_preserving(program, domain_symbol)
    {
        let (left, right) = (binary.left, binary.right);
        if value_proves_qualification_in_contexts(
            program,
            facts,
            state_flow,
            statement_index,
            left,
            domain_symbol,
            language_semantics::SemanticDomainId::NULL,
            contexts,
        ) && value_proves_qualification_in_contexts(
            program,
            facts,
            state_flow,
            statement_index,
            right,
            domain_symbol,
            language_semantics::SemanticDomainId::NULL,
            contexts,
        ) {
            return true;
        }
    }

    // A recast observes the source's current bytes. An initializer, a zero-valid
    // domain or a prior parameter annotation cannot revive invalidated facts.
    // Borrowed field/default-domain restoration is enforced independently and
    // contributes its actual surviving facts through this same live proof.
    if let Some(source) = recast_view_source_expression(program, value)
        && recast_source_proves_domain(
            program,
            facts,
            state_flow,
            statement_index,
            source,
            contexts,
            domain_symbol,
        )
    {
        return true;
    }

    let subject = validation::canonical_place_from_expression_in_state(
        program,
        state_flow.state_symbol,
        statement_index,
        value,
    );
    // Initializers, constructor children and writes consume the same live
    // judgment. A retained AssignedValue can establish a byte or scalar
    // predicate without a separate membership token; mutation retires that
    // evidence. Neither a declaration nor its former initializer is replayed.
    subject.as_ref().is_some_and(|subject| {
        super::super::prover::prove_domain_at_place(
            program,
            &facts.semantic,
            contexts,
            subject,
            domain_symbol,
        )
    })
}

/// The viewed place expression behind an admitted recast view: `&x as &T` is a
/// `Cast` whose `form` is a borrow re-view spelling (`RecastShared` or
/// `RecastMutable`), and the `&mut x as &mut T` pun wraps that cast in the
/// unary `&mut`. Borrow shells peel in both directions so either spelling
/// reduces to the source place `x`. Validation only admits a recast in the
/// blessed `let` initializer position, so a non-recast value -- a conversion
/// `x as T`, a place read, or any other shape -- yields `None` and keeps the
/// ordinary discharge paths.
fn recast_view_source_expression(
    program: &dyn ::validation::CheckingInput,
    value: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let mut current = value;
    loop {
        match program.expression_table.expression(current) {
            ExpressionNode::Borrow(inner) => current = inner.target,
            ExpressionNode::Cast(cast) if cast.form.is_recast() => {
                return Some(
                    program
                        .expression_table
                        .peeled_expression_handle(cast.value),
                );
            }
            _ => return None,
        }
    }
}

/// Whether the recast source place carries LIVE membership evidence in
/// `domain_symbol` -- an assigned value whose scalar satisfies the predicate,
/// or a carried `DomainMembership`/`ContractDomainMembership` fact at the
/// place (a minted local, a contract-proven parameter, a checked prior write).
fn recast_source_proves_domain(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    statement_index: usize,
    source: ExpressionHandle,
    contexts: &[facts::FactContextHandle],
    domain_symbol: SymbolHandle,
) -> bool {
    let Some(subject) = validation::canonical_place_from_expression_in_state(
        program,
        state_flow.state_symbol,
        statement_index,
        source,
    ) else {
        return false;
    };
    super::super::prover::prove_domain_at_place(
        program,
        &facts.semantic,
        contexts,
        &subject,
        domain_symbol,
    )
}

/// Consume a computed producer's result, never a storage declaration. Selected
/// calls and casts retain their completed contracts; a cast's prepared result
/// expands aliases, while independent source validation proves the mint.
/// Indexed theories require the complete instance identity. A symbol-only
/// caller can use ordinary implication only for symbol-only theories.
fn value_result_type_carries_domain(
    program: &dyn ::validation::CheckingInput,
    state_flow: &FlowStateFact,
    statement_index: usize,
    value: ExpressionHandle,
    domain_symbol: SymbolHandle,
    identity: language_semantics::SemanticDomainId,
) -> bool {
    // Mutable storage declarations state obligations, not facts that survived
    // mutation. Only a frozen chain with stable selected contents preserves
    // its declared invariants; all other reads use live occurrence evidence.
    if validation::canonical_place_from_expression_in_state(
        program,
        state_flow.state_symbol,
        statement_index,
        value,
    )
    .is_some_and(|place| place.root.as_symbol().is_some())
        && !crate::values::scalar::structural_fields::storage_observation_is_frozen(
            program,
            state_flow.state_symbol,
            statement_index,
            value,
        )
    {
        return false;
    }
    let Some((machine, state)) =
        program.state_flow_site(state_flow.machine_symbol, state_flow.state_symbol)
    else {
        return false;
    };
    let Some(type_reference) =
        validation::expression_result_type_reference(program, machine, state, value)
    else {
        return false;
    };
    validation::field_domains::domain_constraint_identities(program, type_reference)
        .into_iter()
        .any(|(source, source_identity)| {
            if !typed_trees::domain::has_symbol_only_proof(program, source)
                || !typed_trees::domain::has_symbol_only_proof(program, domain_symbol)
            {
                return identity.is_valid()
                    && source == domain_symbol
                    && source_identity == identity;
            }
            validation::field_domains::domain_membership_implies(program, source, domain_symbol)
        })
}
