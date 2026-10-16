//! Domain implication consumes the producer's prepared result type for places,
//! calls and casts alike. It never treats a receiving annotation as evidence.
//! Routed domains still require live provenance, indexed domains require exact
//! instance identity, and payload reads require the currently selected case.
//! Recast sources and contextual predicate proofs retain their distinct duties.

use checked_trees::{CheckFacts, FlowStateFact};
use facts::FactPayload;
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
        return value_result_type_carries_domain(program, state, value, domain, identity)
            || value_proves_domain(program, facts, state, statement_index, value, domain);
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
    if value_result_type_carries_domain(program, state, value, domain, identity) {
        return true;
    }
    // Numeric field domains use the same contextual proof as nominal call
    // arguments; requiring only an existing membership token would reject a
    // construction whose scalar value already establishes the predicate.
    if typed_trees::domain::has_symbol_only_proof(program, domain)
        && subject.as_ref().is_some_and(|subject| {
            super::super::prover::prove_domain_at_place(
                program,
                &facts.semantic,
                contexts,
                subject,
                domain,
            )
        })
    {
        return true;
    }
    value_proves_domain_in_contexts(
        program,
        facts,
        state,
        statement_index,
        value,
        domain,
        contexts,
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
    if !typed_trees::domain::has_symbol_only_proof(program, domain_symbol) {
        return false;
    }
    if value_result_type_carries_domain(
        program,
        state_flow,
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

    // A place whose DECLARED leaf type carries a ZII-admitting domain implying
    // the target domain remains a valid source even when flow invalidation has
    // discarded its transient membership fact (for example, after the enclosing
    // record crosses a mutable out-parameter call). Its zero/default value is
    // in-domain and every later write is checked by this pass, so that restricted
    // declaration is an invariant of the place rather than merely an
    // entry-context fact. Empty-violating domains still require a flow proof.
    if declared_value_domain_implies(program, state_flow, value, domain_symbol) {
        return true;
    }

    // An admitted recast view `&x as &T` re-reads the SOURCE place's bytes
    // through the let's stated type, so an `in D` annotation on that type is an
    // obligation on the viewed place `x`, not on the reference-typed
    // initializer (which owns no scalar place of its own). Validation's recast
    // judgment already proved the source's declared representation facts imply
    // the cast's stated shape before admitting the view; the residual question
    // here is the source's own establishment in `domain_symbol`: live
    // membership evidence at its place, or a declared leaf domain implying
    // `domain_symbol` that is an invariant of the place — caller- or
    // binder-established for parameters and initialized locals, and gated on
    // the zero-initialized value satisfying it for machine storage and
    // zero-initialized locals (the same soundness gate the field-domain fact
    // seeding applies, so the annotation still cannot mint qualification).
    if let Some(source) = recast_view_source_expression(program, value)
        && (recast_source_proves_domain(
            program,
            facts,
            state_flow,
            statement_index,
            source,
            contexts,
            domain_symbol,
        ) || recast_source_declared_domain_implies(program, state_flow, source, domain_symbol))
    {
        return true;
    }

    let value_label = program.expression_table.display_name(value);
    let value_place = validation::canonical_place_from_expression_in_state(
        program,
        state_flow.state_symbol,
        statement_index,
        value,
    );
    contexts.iter().any(|context_handle| {
        let context = facts.semantic.contexts.get(*context_handle);
        facts.semantic.context_view(context).facts().any(|fact| {
            let fact_domain = match fact.payload {
                facts::domain_membership_symbol_pattern!(domain_symbol) => domain_symbol,
                _ => return false,
            };
            if !typed_trees::domain::has_symbol_only_proof(program, fact_domain) {
                return false;
            }
            if !facts.semantic.domain_implies(fact_domain, domain_symbol)
                && !validation::field_domains::domain_membership_implies(
                    program,
                    fact_domain,
                    domain_symbol,
                )
            {
                return false;
            }
            // Match the fact's subject against the assigned value, by the
            // fact's place label and (for a contract membership) its declared
            // `value` expression label -- the same two-pronged match the
            // statement-transfer propagation uses.
            let Some(fact_place) = fact.place.as_place() else {
                return false;
            };
            if value_place.as_ref().is_some_and(|value_place| {
                crate::flow::canonical_place_from_semantic_place(
                    program,
                    &facts.semantic,
                    facts.semantic.places.get(fact_place),
                )
                .is_some_and(|fact_place| fact_place == *value_place)
            }) {
                return true;
            }
            let place_label = facts.semantic.place_label(program, fact_place);
            if place_label == value_label {
                return true;
            }
            match fact.payload {
                FactPayload::DomainMembership { value, .. }
                | FactPayload::ContractDomainMembership { value, .. } => {
                    value.is_valid() && program.expression_table.display_name(value) == value_label
                }
                _ => false,
            }
        })
    })
}

/// Whether `value` is a state place whose declared leaf type carries a
/// ZII-admitting domain that implies `domain_symbol`.
fn declared_value_domain_implies(
    program: &dyn ::validation::CheckingInput,
    state_flow: &FlowStateFact,
    value: ExpressionHandle,
    domain_symbol: SymbolHandle,
) -> bool {
    let Some((machine, state)) = validation::machine_state_by_symbols(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
    ) else {
        return false;
    };
    let Some(value_type) =
        validation::field_domains::assignment_target_type_reference(program, machine, state, value)
    else {
        return false;
    };
    validation::field_domains::predicate_domain_constraint_symbols(program, value_type)
        .into_iter()
        .filter(|value_domain| {
            validation::field_domains::domain_admits_zero_byte_carrier(
                program,
                *value_domain,
                value_type,
            )
        })
        .any(|value_domain| {
            validation::field_domains::domain_membership_implies(
                program,
                value_domain,
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

/// Whether the recast source's DECLARED leaf type carries a predicate domain
/// that implies `domain_symbol`. The declared domain is an invariant of the
/// place this pass enforces on every write, so the residual obligation is the
/// domain's own establishment at the place's initial value: a state parameter
/// is caller-established and an initialized local was proven at its `let`,
/// while machine storage and zero-initialized locals additionally require the
/// zero value to satisfy the domain -- the same soundness gate the
/// field-domain fact seeding applies (`semantic/field_domains.rs`). A domain
/// the zero value violates stays a write-time obligation rather than an
/// invariant, so its reads must still follow a proven write.
fn recast_source_declared_domain_implies(
    program: &dyn ::validation::CheckingInput,
    state_flow: &FlowStateFact,
    source: ExpressionHandle,
    domain_symbol: SymbolHandle,
) -> bool {
    let Some((machine, state)) = validation::machine_state_by_symbols(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
    ) else {
        return false;
    };
    // `self`-rooted storage is always zero-initialized, so the declared leaf
    // domain is a valid invariant only when the zero value provably satisfies
    // it.
    if let Some(leaf_type) =
        validation::field_domains::attached_data_field_type(program, machine, source)
    {
        return validation::field_domains::predicate_domain_constraint_symbols(program, leaf_type)
            .into_iter()
            .any(|source_domain| {
                domain_admits_zero_value(program, source_domain)
                    && validation::field_domains::domain_membership_implies(
                        program,
                        source_domain,
                        domain_symbol,
                    )
            });
    }
    let Some(leaf_type) =
        validation::field_domains::direct_state_place_type_reference(program, state, source)
    else {
        return false;
    };
    let established = state_place_is_established(program, state, source);
    validation::field_domains::predicate_domain_constraint_symbols(program, leaf_type)
        .into_iter()
        .any(|source_domain| {
            (established || domain_admits_zero_value(program, source_domain))
                && validation::field_domains::domain_membership_implies(
                    program,
                    source_domain,
                    domain_symbol,
                )
        })
}

/// Whether a non-`self` state place's declared domain was established before
/// the current statement: a parameter's declared domain is caller-established
/// (and `&`/`&mut`/`&write` re-establishment is enforced on writes), and an
/// initialized local's annotation was discharged at its own `let`. A
/// zero-initialized local declared `in D` has no such evidence -- the same
/// storage case as a machine field -- and any other root conservatively
/// reports unestablished.
fn state_place_is_established(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    source: ExpressionHandle,
) -> bool {
    let Some(symbol) = state_place_root_symbol(program, source) else {
        return false;
    };
    if program.state_parameter_by_symbol(state, symbol).is_some() {
        return true;
    }
    program
        .state_local_declaration(state, symbol)
        .is_some_and(|(_, local)| local.initial_value.is_valid())
}

/// The root symbol of a state place expression, peeling borrow shells and
/// member projections. `self` is never a state-place root (the caller resolves
/// it through `attached_data_field_type` first), so a `self`-headed name is
/// not special-cased here.
fn state_place_root_symbol(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => state_place_root_symbol(program, inner.target),
        ExpressionNode::Member(member) => state_place_root_symbol(program, member.receiver),
        ExpressionNode::Indexed(indexed) => state_place_root_symbol(program, indexed.collection),
        ExpressionNode::Name(path) => path.resolved_symbol(),
        _ => None,
    }
}

/// Whether `domain_symbol`'s `self`-facts provably hold on the ZERO value of
/// its declared carrier -- the scalar-carrier half of the ZII soundness gate
/// `domain_admits_zero_byte_carrier` covers for recognized byte predicates.
/// An integer leaf's ZII is `0` and a `bool` leaf's is `false`; `self`-relative
/// membership facts recurse because a member's ZII is again the all-zero value
/// of its own declared type. A leaf the evaluator cannot decide -- a float,
/// byte string, record, runtime projection, or a non-`self` membership subject
/// -- conservatively refuses, so an unproven domain stays a write-time
/// obligation rather than an invariant.
fn domain_admits_zero_value(
    program: &dyn ::validation::CheckingInput,
    domain_symbol: SymbolHandle,
) -> bool {
    domain_admits_zero_value_inner(program, domain_symbol, &mut Vec::new())
}

fn domain_admits_zero_value_inner(
    program: &dyn ::validation::CheckingInput,
    domain_symbol: SymbolHandle,
    active: &mut Vec<SymbolHandle>,
) -> bool {
    if active.contains(&domain_symbol) {
        return false;
    }
    let Some(domain) = program.domain_definition(domain_symbol) else {
        return false;
    };
    if validation::field_domains::domain_admits_zero_byte_carrier(
        program,
        domain_symbol,
        domain.target_type,
    ) {
        return true;
    }
    // Generic or indexed domain instances can bind their zero question to the
    // instance arguments; only closed symbol-only theories are answered here.
    if !domain.type_parameters.is_empty() || !domain.index_arguments.is_empty() {
        return false;
    }
    let facts = program.proof_facts.span_or_empty(domain.facts);
    if facts.is_empty() {
        return false;
    }
    active.push(domain_symbol);
    let admitted = facts.iter().all(|fact| match fact {
        typed_trees::domain::ProofFact::Expression(expression) => matches!(
            super::super::prover::evaluate_scalar(
                program,
                *expression,
                &mut |leaf| { zero_value_scalar_leaf(program, domain, leaf) },
                &|_, _| true
            ),
            Some(super::super::prover::ScalarValue::Boolean(true))
        ),
        typed_trees::domain::ProofFact::Membership(membership) => {
            expression_is_subject_relative(program, domain, membership.value)
                && domain_admits_zero_value_inner(program, membership.domain_symbol, active)
        }
        typed_trees::domain::ProofFact::Proposition(_) => false,
    });
    active.pop();
    admitted
}

/// The ZII scalar of a `self`-relative leaf in a domain fact: the leaf's
/// declared type resolved through its retained position, or through the
/// domain target's member path when the position is opaque, then the zero of
/// that primitive -- `0` for an integer leaf, `false` for a `bool` leaf.
/// Floats, byte carriers, records, and runtime projections have no decidable
/// scalar here, so the leaf stays unresolved and the enclosing evaluation
/// fails closed.
fn zero_value_scalar_leaf(
    program: &dyn ::validation::CheckingInput,
    domain: &typed_trees::domain::DomainDefinition,
    leaf: ExpressionHandle,
) -> Option<super::super::prover::ScalarValue> {
    let leaf_type =
        validation::expression_place_type_reference(program, leaf, &[]).or_else(|| {
            let segments =
                crate::flow::relative_place_segments_from_expression(program, leaf, domain)?;
            validation::project_type_reference_from_segments(program, domain.target_type, &segments)
        })?;
    match program.type_reference_table.primitive_type(leaf_type) {
        Some(typed_trees::types::PrimitiveType::Bool) => {
            Some(super::super::prover::ScalarValue::Boolean(false))
        }
        Some(primitive) if primitive.accepts_integer_literal() => Some(
            super::super::prover::ScalarValue::Integer(numerics::bignum::BigInt::from_i64(0)),
        ),
        _ => None,
    }
}

/// Whether an expression is a `self`-rooted member path in a domain fact --
/// `self`, `self.f`, or a deeper projection -- where `self` binds the domain's
/// target value. A membership fact over any other subject (a constant, an
/// unrelated place) is not evidence about the domain's own zero value.
fn expression_is_subject_relative(
    program: &dyn ::validation::CheckingInput,
    domain: &typed_trees::domain::DomainDefinition,
    expression: ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => {
            expression_is_subject_relative(program, domain, inner.target)
        }
        ExpressionNode::Member(member) => {
            expression_is_subject_relative(program, domain, member.receiver)
        }
        ExpressionNode::Indexed(indexed) => {
            expression_is_subject_relative(program, domain, indexed.collection)
        }
        ExpressionNode::Name(_) => {
            typed_trees::domain::domain_subject_reference(program, domain, expression)
        }
        _ => false,
    }
}

/// Consume the value producer's result, not the receiving annotation. Places,
/// selected calls and casts share this route; a cast's prepared result already
/// expands aliases, while independent source validation proves the mint.
/// Indexed theories require the complete instance identity. A symbol-only
/// caller can use ordinary implication only for symbol-only theories.
fn value_result_type_carries_domain(
    program: &dyn ::validation::CheckingInput,
    state_flow: &FlowStateFact,
    value: ExpressionHandle,
    domain_symbol: SymbolHandle,
    identity: language_semantics::SemanticDomainId,
) -> bool {
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
