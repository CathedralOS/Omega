//! Saved caller observations: an immutable state-local `let` binding denotes
//! the exact value its initializer produced, and that value is stable caller
//! evidence at every later call boundary in the state. Binding the local to
//! its initializer's polynomial lets a callee's requires row discharge from
//! the saved fact -- `let n = items.len` then `inner(items, n)` proves
//! `rest.len <= capacity` as `items.len <= items.len` -- rather than from a
//! literal actual alone.
//!
//! A saved value is observed at its binding site, so a local joins the roster
//! only while every carrier its initializer references is stable for the
//! whole state: never written anywhere in it, never the root of an exclusive
//! borrow, and never an exclusive-reference carrier itself. A `mut` formal's
//! or local's atom names its entry value, so an initializer that reads a
//! mutable carrier still saves an observation -- but only while the state
//! never writes the carrier; any assignment target or mutable-receiver call
//! rooted at it makes the read unstable. The distinction is what keeps
//! `let n = k` honest when `k` is reassigned before the call: `n` may claim
//! the incoming value only while nothing can write it away.
use super::super::StrictArithmeticExpressionBinding;
use super::{
    BigInt, Engine, ExpressionHandle, ExpressionNode, Machine, Polynomial, State, TypedTrees,
    lengths, meanings,
};
use language_semantics::declaration_selection::CollectionMeasure;
use symbols::SymbolHandle;
use typed_trees::statement::StatementNode;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// An actual that carries a saved caller observation: a plain name bound by
/// a state-local `let` nothing writes. Admission is by shape only -- the
/// binding pass still has to normalize the initializer against the caller's
/// atoms -- because evaluating a stable name at the boundary cannot rewrite
/// an installed caller fact.
pub(super) fn actual(
    program: &TypedTrees,
    state: &State,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    let symbol = plain_name(program, expression)?;
    saved_local(program, state, symbol)?;
    Some(symbol)
}

/// Bind each saved local of `state` to the polynomial its initializer
/// denotes, so an actual naming it substitutes the saved value. Locals enter
/// in statement order: `let n = a + 1` after `let a = items.len` reads the
/// already-bound `a`. A local whose initializer leaves the strict arithmetic
/// language, or reads a carrier that can change, simply never binds.
pub(super) fn install(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    source_lengths: &[(SymbolHandle, String)],
    engine: &mut Engine<'_>,
) {
    for statement in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
    {
        let StatementNode::LocalData(local) = statement else {
            continue;
        };
        if !local.initial_value.is_valid()
            || lengths::binding_is_exclusively_exposed(program, state, local.symbol)
            || lengths::binding_is_written(program, state, local.symbol)
            || !stable_references(program, state, local.initial_value)
        {
            continue;
        }
        // `.len` members inside the initializer name the borrowed-collection
        // observation: a slice formal's extent binds through the shared
        // length atoms, a state-local collection's extent is a produced
        // constant.
        let _ = lengths::install(
            program,
            machine,
            state,
            state,
            source_lengths,
            engine,
            &[local.initial_value],
        );
        bind_local_lengths(program, machine, state, local.initial_value, engine);
        // The saved initializer may carry a runtime quotient, remainder, or
        // exact shift: mint each non-polynomial operand-pair atom the same
        // way the range owner does before binding, so the observation saves
        // the exact operation instead of dropping the local. A failed bind
        // still just leaves the local unobserved, as before.
        let _ = meanings::install_nonpolynomial_terms(
            program,
            machine,
            state,
            engine,
            local.initial_value,
            0,
        );
        let _ = engine.bind_strict_arguments(&[StrictArithmeticExpressionBinding {
            symbol: local.symbol,
            expression: local.initial_value,
        }]);
    }
}

/// A plain name occurrence and the symbol it resolves to; member chains and
/// path heads are deliberately not plain names.
fn plain_name(program: &TypedTrees, expression: ExpressionHandle) -> Option<SymbolHandle> {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    (path.symbol.is_valid()
        && path.head_symbol == path.symbol
        && program
            .expression_table
            .name_path_members(path.members)
            .len()
            == 1)
        .then_some(path.symbol)
}

/// The saved local `symbol` names in `state`: exactly one `let` binding
/// carrying an initializer, never written, never the root of an exclusive
/// borrow. A mutable local's binding names its incoming value only while no
/// later write replaces it, so written locals produce no observation;
/// neither do ambiguous or formal names.
fn saved_local<'a>(
    program: &'a TypedTrees,
    state: &State,
    symbol: SymbolHandle,
) -> Option<&'a typed_trees::statement::TableLocalData> {
    let locals = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .filter_map(|statement| match statement {
            StatementNode::LocalData(local) if local.symbol == symbol => Some(local),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [local] = locals.as_slice() else {
        return None;
    };
    (local.initial_value.is_valid()
        && !lengths::binding_is_exclusively_exposed(program, state, symbol)
        && !lengths::binding_is_written(program, state, symbol))
    .then_some(*local)
}

/// Whether `type_reference` resolves to an exclusive (`&mut`/`&out`) borrow:
/// writes through it replace the referent without touching the binding, so a
/// saved read of the carrier would go stale silently.
fn exclusive_referent(program: &TypedTrees, mut reference: TypeReferenceHandle) -> bool {
    let mut visited = Vec::new();
    while reference.is_valid() && !visited.contains(&reference) {
        visited.push(reference);
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { access, .. } => return access.is_exclusive(),
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            _ => return false,
        }
    }
    false
}

/// Every name in `expression` must denote a carrier nothing in the state
/// writes: the saved observation reads each at its binding site, while the
/// engine's atoms denote the entry value a written carrier has already left.
/// A carrier that can change between the two would silently read the wrong
/// value, so the observation is withheld entirely rather than approximated.
fn stable_references(program: &TypedTrees, state: &State, expression: ExpressionHandle) -> bool {
    let mut nodes = Vec::new();
    crate::value_custody::expression_types::collect_expression_nodes(
        program, expression, &mut nodes,
    );
    nodes.iter().all(|node| {
        let ExpressionNode::Name(path) = program.expression_table.expression(*node) else {
            return true;
        };
        let symbol = path.head_symbol;
        if let Some(parameter) = program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.symbol == symbol)
        {
            !parameter.is_self
                && !exclusive_referent(program, parameter.type_reference)
                && !lengths::binding_is_exclusively_exposed(program, state, symbol)
                && !lengths::binding_is_written(program, state, symbol)
        } else {
            saved_local(program, state, symbol)
                .is_some_and(|local| !exclusive_referent(program, local.type_reference))
        }
    })
}

/// Constant extent for a `.len` member whose receiver is itself a saved
/// local collection: `let s = &[..]; let n = s.len`. Formal receivers bind
/// through `lengths::install`; only a local receiver needs this pass.
fn bind_local_lengths(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    engine: &mut Engine<'_>,
) {
    let mut nodes = Vec::new();
    crate::value_custody::expression_types::collect_expression_nodes(
        program, expression, &mut nodes,
    );
    for node in nodes {
        let ExpressionNode::Member(member) = program.expression_table.expression(node) else {
            continue;
        };
        if CollectionMeasure::from_authored_spelling(member.member.as_str())
            != Some(CollectionMeasure::Length)
        {
            continue;
        }
        let Some(receiver) = plain_name(program, member.receiver) else {
            continue;
        };
        if saved_local(program, state, receiver).is_none() {
            continue;
        }
        let Some(length) = lengths::produced_length(program, machine, state, member.receiver, 0)
        else {
            continue;
        };
        let _ = engine.bind_strict_projection(node, Polynomial::constant(BigInt::from_u64(length)));
    }
}
