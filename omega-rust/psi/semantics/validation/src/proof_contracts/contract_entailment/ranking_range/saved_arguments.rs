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
//! whole state: immutable, never the root of an exclusive borrow, and never
//! an exclusive-reference carrier itself. A `mut` formal's or local's atom
//! still names its value at the call -- the latest write, not the incoming
//! value -- so an initializer that reads a mutable carrier produces no saved
//! observation. The distinction is what keeps `let n = k` honest when `k` is
//! reassigned before the call: `n` may claim the incoming value only while
//! nothing can write it away.
use super::super::StrictArithmeticExpressionBinding;
use super::{
    BigInt, Engine, ExpressionHandle, ExpressionNode, Machine, Polynomial, State, TypedTrees,
    lengths,
};
use symbols::SymbolHandle;
use typed_trees::statement::StatementNode;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// An actual that carries a saved caller observation: a plain name bound by
/// an immutable state-local `let`. Admission is by shape only -- the binding
/// pass still has to normalize the initializer against the caller's atoms --
/// because evaluating a stable name at the boundary cannot rewrite an
/// installed caller fact.
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
        if local.is_mutable
            || !local.initial_value.is_valid()
            || lengths::binding_is_exclusively_exposed(program, state, local.symbol)
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

/// The saved local `symbol` names in `state`: exactly one immutable `let`
/// binding carrying an initializer, never the root of an exclusive borrow.
/// A mutable local's binding would name its incoming value while a later
/// write already replaced it, so mutable locals produce no observation;
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
        && !local.is_mutable
        && !lengths::binding_is_exclusively_exposed(program, state, symbol))
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

/// Every name in `expression` must denote a carrier nothing in the state can
/// write: the saved observation reads each at its binding site, while the
/// engine's atoms denote values at the call. A carrier that can change
/// between the two would silently read the wrong value, so the observation
/// is withheld entirely rather than approximated.
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
                && !parameter.is_mutable
                && !exclusive_referent(program, parameter.type_reference)
                && !lengths::binding_is_exclusively_exposed(program, state, symbol)
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
        if member.member.as_str() != "len" {
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
