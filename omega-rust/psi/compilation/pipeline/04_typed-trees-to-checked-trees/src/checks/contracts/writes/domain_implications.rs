//! Domain implication routes: declared-type carries, value-call returns,
//! recast sources, state-place establishment, and zero-value admission.

use checked_trees::{CheckFacts, FlowStateFact};
use facts::FactPayload;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

/// Routed theories require the exact live instance at the initializer's actual
/// place. Read the last completed operand-call context when calls occur in the
/// statement, otherwise its entry context; never read the destination binding's
/// post-statement facts or equate places by their display names.
pub(super) fn value_proves_qualification(
    program: &typed_trees::TypedTrees,
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
    if !crate::facts::field_domain::domain_requires_provenance(program, domain) {
        return value_proves_domain(program, facts, state, statement_index, value, domain);
    }
    let Some(subject) = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.state_symbol,
        statement_index,
        value,
    ) else {
        return false;
    };
    let contexts = statement_semantic_contexts(facts, state, statement_index);
    super::super::exits::exact_scalar_membership(
        program, facts, &contexts, &subject, domain, identity,
    )
}

/// The semantic contexts whose membership evidence applies at one statement:
/// the exit contexts of the call that owns it, or the statement's own entry
/// contexts when no call does.
pub(super) fn statement_semantic_contexts(
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
) -> Vec<facts::FactContextHandle> {
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

/// Discharge a `requires` predicate domain against the initializer itself.
/// can consume its field facts. Use the same completed-operand contexts as
/// routed membership, never an initializer or a post-assignment destination.
/// An extracted payload needs its current tag before any qualification rule
/// can consume its field facts. Use the same completed-operand contexts as
/// routed membership, never an initializer or a post-assignment destination.
pub(super) fn value_case_path_is_selected(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    value: ExpressionHandle,
) -> bool {
    let Some(subject) = crate::flow::canonical_place_from_expression_in_state(
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
    let contexts = statement_semantic_contexts(facts, state, statement_index);
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
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    statement_index: usize,
    value: ExpressionHandle,
    domain_symbol: SymbolHandle,
) -> bool {
    // IDENTITY. A value whose DECLARED TYPE already carries this domain is in
    // it by declaration: a place declared `in D` establishes D at every write,
    // so reading it yields a value in D and no separate fact is owed. Reading
    // `self.scalar: i32 in Small` into `let x: i32 in Small` is the whole
    // obligation, and it is discharged by the declaration that created it.
    if value_declared_type_carries_domain(program, state_flow, value, domain_symbol) {
        return true;
    }
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

pub(super) fn construction_value_proves_qualification(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    value: ExpressionHandle,
    domain: SymbolHandle,
    identity: language_semantics::SemanticDomainId,
    contexts: &[facts::FactContextHandle],
) -> bool {
    let subject = crate::flow::canonical_place_from_expression_in_state(
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
    if crate::facts::field_domain::domain_requires_provenance(program, domain) {
        return subject.as_ref().is_some_and(|subject| {
            super::super::exits::exact_scalar_membership(
                program, facts, contexts, subject, domain, identity,
            )
        });
    }
    // Numeric field domains use the same contextual proof as nominal call
    // arguments; requiring only an existing membership token would reject a
    // construction whose scalar value already establishes the predicate.
    if typed_trees::domain::supports_symbol_only_proof(program, domain)
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
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    statement_index: usize,
    value: ExpressionHandle,
    domain_symbol: SymbolHandle,
    contexts: &[facts::FactContextHandle],
) -> bool {
    if !typed_trees::domain::supports_symbol_only_proof(program, domain_symbol) {
        return false;
    }
    if crate::facts::field_domain::string_literal_expression_grants_domain(
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
    if let ExpressionNode::Binary(binary) = program.expression_table.expression(value)
        && binary.operator == typed_trees::expression::BinaryOperator::Add
        && crate::facts::field_domain::domain_is_concat_preserving(program, domain_symbol)
    {
        let (left, right) = (binary.left, binary.right);
        if construction_value_proves_qualification(
            program,
            facts,
            state_flow,
            statement_index,
            left,
            domain_symbol,
            language_semantics::SemanticDomainId::NULL,
            contexts,
        ) && construction_value_proves_qualification(
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

    // A value-position call whose target's DECLARED return type carries a domain
    // implying `domain_symbol` is accepted on the same trust basis as a declared
    // param domain at a call site: the signature's domain is trusted at use
    // sites, with its establishment enforced separately (params at call sites;
    // scalar results at ordinary exits in exits/result_domains.rs). This is how a
    // `-> &[u8] in Utf8` value-call result flows into a `&[u8] in Utf8` field.
    if value_call_return_domain_implies(program, value, domain_symbol) {
        return true;
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
    let value_place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state_flow.state_symbol,
        statement_index,
        value,
    );
    contexts.iter().any(|context_handle| {
        let context = facts.semantic.contexts.get(*context_handle);
        facts.semantic.context_view(context).facts().any(|fact| {
            let fact_domain = match fact.payload {
                FactPayload::DomainMembership { domain_symbol, .. }
                | FactPayload::ContractDomainMembership { domain_symbol, .. } => domain_symbol,
                _ => return false,
            };
            if !typed_trees::domain::supports_symbol_only_proof(program, fact_domain) {
                return false;
            }
            if !facts.semantic.domain_implies(fact_domain, domain_symbol)
                && !crate::facts::field_domain::domain_membership_implies(
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
            let facts::FactPlace::Place(fact_place) = fact.place else {
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
    program: &typed_trees::TypedTrees,
    state_flow: &FlowStateFact,
    value: ExpressionHandle,
    domain_symbol: SymbolHandle,
) -> bool {
    let Some(machine) = crate::lookup::machine_by_symbol(program, state_flow.machine_symbol) else {
        return false;
    };
    let Some(state) = crate::semantic::calls::find_state_in_machine(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
    ) else {
        return false;
    };
    let Some(value_type) = crate::facts::field_domain::assignment_target_type_reference(
        program, machine, state, value,
    ) else {
        return false;
    };
    crate::facts::field_domain::predicate_domain_constraint_symbols(program, value_type)
        .into_iter()
        .filter(|value_domain| {
            crate::facts::field_domain::domain_admits_empty_byte_sequence(program, *value_domain)
        })
        .any(|value_domain| {
            crate::facts::field_domain::domain_membership_implies(
                program,
                value_domain,
                domain_symbol,
            )
        })
}

/// Whether `value` is a value-position call whose resolved target state declares
/// a return type carrying a domain that implies `domain_symbol`.
fn value_call_return_domain_implies(
    program: &typed_trees::TypedTrees,
    value: ExpressionHandle,
    domain_symbol: SymbolHandle,
) -> bool {
    let ExpressionNode::Call(call) = program.expression_table.expression(value) else {
        return false;
    };
    let Some(target) = crate::semantic::calls::find_state(program, call.target_symbol) else {
        return false;
    };
    if !target.return_type.is_valid() {
        return false;
    }
    crate::facts::field_domain::predicate_domain_constraint_symbols(program, target.return_type)
        .into_iter()
        .any(|return_domain| {
            crate::facts::field_domain::domain_membership_implies(
                program,
                return_domain,
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
    program: &typed_trees::TypedTrees,
    value: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let mut current = value;
    loop {
        match program.expression_table.expression(current) {
            ExpressionNode::Borrow(inner) => current = inner.target,
            ExpressionNode::Cast(cast) if cast.form.is_recast() => {
                let mut source = cast.value;
                while let ExpressionNode::Borrow(inner) =
                    program.expression_table.expression(source)
                {
                    source = inner.target;
                }
                return Some(source);
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
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    statement_index: usize,
    source: ExpressionHandle,
    contexts: &[facts::FactContextHandle],
    domain_symbol: SymbolHandle,
) -> bool {
    let Some(subject) = crate::flow::canonical_place_from_expression_in_state(
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
    program: &typed_trees::TypedTrees,
    state_flow: &FlowStateFact,
    source: ExpressionHandle,
    domain_symbol: SymbolHandle,
) -> bool {
    let Some(machine) = crate::lookup::machine_by_symbol(program, state_flow.machine_symbol) else {
        return false;
    };
    let Some(state) = crate::semantic::calls::find_state_in_machine(
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
        crate::facts::field_domain::attached_data_field_type(program, machine, source)
    {
        return crate::facts::field_domain::predicate_domain_constraint_symbols(program, leaf_type)
            .into_iter()
            .any(|source_domain| {
                domain_admits_zero_value(program, source_domain)
                    && crate::facts::field_domain::domain_membership_implies(
                        program,
                        source_domain,
                        domain_symbol,
                    )
            });
    }
    let Some(leaf_type) =
        crate::facts::field_domain::direct_state_place_type_reference(program, state, source)
    else {
        return false;
    };
    let established = state_place_is_established(program, state, source);
    crate::facts::field_domain::predicate_domain_constraint_symbols(program, leaf_type)
        .into_iter()
        .any(|source_domain| {
            (established || domain_admits_zero_value(program, source_domain))
                && crate::facts::field_domain::domain_membership_implies(
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
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    source: ExpressionHandle,
) -> bool {
    let Some(symbol) = state_place_root_symbol(program, source) else {
        return false;
    };
    if program
        .state_parameters(state)
        .iter()
        .any(|parameter| parameter.symbol == symbol)
    {
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
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => state_place_root_symbol(program, inner.target),
        ExpressionNode::Member(member) => state_place_root_symbol(program, member.receiver),
        ExpressionNode::Indexed(indexed) => state_place_root_symbol(program, indexed.collection),
        ExpressionNode::Name(path) => path
            .head_symbol
            .is_valid()
            .then_some(path.head_symbol)
            .or_else(|| path.symbol.is_valid().then_some(path.symbol)),
        _ => None,
    }
}

/// Whether `domain_symbol`'s `self`-facts provably hold on the ZERO value of
/// its declared carrier -- the scalar-carrier half of the ZII soundness gate
/// `domain_admits_empty_byte_sequence` covers for recognized byte predicates.
/// An integer leaf's ZII is `0` and a `bool` leaf's is `false`; `self`-relative
/// membership facts recurse because a member's ZII is again the all-zero value
/// of its own declared type. A leaf the evaluator cannot decide -- a float,
/// byte string, record, runtime projection, or a non-`self` membership subject
/// -- conservatively refuses, so an unproven domain stays a write-time
/// obligation rather than an invariant.
fn domain_admits_zero_value(
    program: &typed_trees::TypedTrees,
    domain_symbol: SymbolHandle,
) -> bool {
    domain_admits_zero_value_inner(program, domain_symbol, &mut Vec::new())
}

fn domain_admits_zero_value_inner(
    program: &typed_trees::TypedTrees,
    domain_symbol: SymbolHandle,
    active: &mut Vec<SymbolHandle>,
) -> bool {
    if crate::facts::field_domain::domain_admits_empty_byte_sequence(program, domain_symbol) {
        return true;
    }
    if active.contains(&domain_symbol) {
        return false;
    }
    let Some(domain) = program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == domain_symbol)
    else {
        return false;
    };
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
            super::super::prover::evaluate_scalar(program, *expression, &mut |leaf| {
                zero_value_scalar_leaf(program, domain, leaf)
            }),
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
    program: &typed_trees::TypedTrees,
    domain: &typed_trees::domain::DomainDefinition,
    leaf: ExpressionHandle,
) -> Option<super::super::prover::ScalarValue> {
    let leaf_type =
        crate::flow::expression_place_type_reference(program, leaf, &[]).or_else(|| {
            let segments =
                crate::flow::relative_place_segments_from_expression(program, leaf, domain)?;
            crate::flow::project_type_reference_from_segments(
                program,
                domain.target_type,
                &segments,
            )
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
    program: &typed_trees::TypedTrees,
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

/// Whether the value's own declared type carries `domain_symbol`.
///
/// This is the identity case of membership, not an inference: the type is the
/// declaration the place was created under, so a read of it is already a value
/// of that domain. A value with no resolvable declared type, or one carrying a
/// different domain, answers `false` and leaves every other route to decide.
fn value_declared_type_carries_domain(
    program: &typed_trees::TypedTrees,
    state_flow: &FlowStateFact,
    value: ExpressionHandle,
    domain_symbol: SymbolHandle,
) -> bool {
    let Some(machine) = crate::lookup::machine_by_symbol(program, state_flow.machine_symbol) else {
        return false;
    };
    let Some(state) = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_flow.state_symbol)
    else {
        return false;
    };
    let Some(type_reference) =
        validation::declared_place_type_raw(program, machine, Some(state), value)
    else {
        return false;
    };
    crate::facts::field_domain::domain_constraint_symbols(program, type_reference)
        .contains(&domain_symbol)
}
