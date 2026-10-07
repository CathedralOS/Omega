//! Root currency scope: a fresh state walks once whether its roots are
//! current, and binder-formed members escape that freshness.

use super::super::{Machine, State, TypedTrees};
use symbols::SymbolHandle;

/// One per-state index behind `root_is_current`: a place root is current when
/// it names the machine, a state parameter, or a local declared before the
/// read's statement index. Both verdicts otherwise rescan the parameter list
/// and the statement prefix per read. `local_first` keeps each symbol's first
/// declaration index, preserving the prefix scan's membership semantics.
pub(super) struct RootCurrencyIndex {
    parameters: std::collections::HashSet<SymbolHandle>,
    local_first: symbols::SymbolKeyMap<SymbolHandle, usize>,
}

/// One state's identity inside a program for the slot key: its declared
/// symbol plus the starts of its parameter and statement spans. Two states
/// in one program can share a symbol in malformed input, but they can never
/// share either span start — the spans are disjoint — and the program's
/// identity separates instances across passes.

/// One state's identity inside a program for the slot key: its declared
/// symbol plus the starts of its parameter and statement spans. Two states
/// in one program can share a symbol in malformed input, but they can never
/// share either span start — the spans are disjoint — and the program's
/// identity separates instances across passes.
type StateKey = (u32, u32, u32);

pub(super) fn state_key(state: &State) -> StateKey {
    (
        state.symbol.arena_index(),
        state.parameters.start().arena_index(),
        state.statement_nodes.start().arena_index(),
    )
}

/// `None` outside a [`RootCurrencyScope`]; inside one, the index built for
/// the state last consulted, keyed by that state's stable identity and the
/// program's structural fingerprint — an in-place rewrite of the state's
/// parameters or statements, a roster re-point, or a different program
/// entirely all move the key, so no state can ever inherit another's index.
/// The scope's lifetime additionally keeps the cache inside one check pass.

/// `None` outside a [`RootCurrencyScope`]; inside one, the index built for
/// the state last consulted, keyed by that state's stable identity and the
/// program's structural fingerprint — an in-place rewrite of the state's
/// parameters or statements, a roster re-point, or a different program
/// entirely all move the key, so no state can ever inherit another's index.
/// The scope's lifetime additionally keeps the cache inside one check pass.
type RootCurrencySlot = Option<Option<(u64, u64, StateKey, RootCurrencyIndex)>>;

thread_local! {
    static ROOT_CURRENCY: std::cell::RefCell<RootCurrencySlot> =
        const { std::cell::RefCell::new(None) };
}

/// The root-currency memo scope for one check pass; the cache is empty when
/// it opens and restored to the enclosing scope when it drops.

/// The root-currency memo scope for one check pass; the cache is empty when
/// it opens and restored to the enclosing scope when it drops.
pub(crate) struct RootCurrencyScope(RootCurrencySlot);

impl Drop for RootCurrencyScope {
    fn drop(&mut self) {
        ROOT_CURRENCY.with(|cell| *cell.borrow_mut() = self.0.take());
    }
}

pub(crate) fn enter_root_currency_scope() -> RootCurrencyScope {
    RootCurrencyScope(ROOT_CURRENCY.with(|cell| cell.borrow_mut().replace(None)))
}

pub(super) fn build_root_currency_index(program: &TypedTrees, state: &State) -> RootCurrencyIndex {
    let mut index = RootCurrencyIndex {
        parameters: std::collections::HashSet::new(),
        local_first: symbols::SymbolKeyMap::default(),
    };
    for parameter in program.state_parameters(state) {
        index.parameters.insert(parameter.symbol);
    }
    for (position, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        if let typed_trees::statement::StatementNode::LocalData(local) = statement {
            index.local_first.entry(local.symbol).or_insert(position);
        }
    }
    index
}

pub(super) fn with_root_currency<R>(
    program: &TypedTrees,
    state: &State,
    read: impl FnOnce(&RootCurrencyIndex) -> R,
) -> R {
    let identity = program.identity.get();
    let fingerprint = program.structural_fingerprint();
    let key = state_key(state);
    ROOT_CURRENCY.with(|cell| {
        let mut slot = cell.borrow_mut();
        let Some(scope) = &mut *slot else {
            drop(slot);
            return read(&build_root_currency_index(program, state));
        };
        let stale = match scope.as_ref() {
            Some((owner, seen, state_owner, _)) => {
                *owner != identity || *seen != fingerprint || *state_owner != key
            }
            None => true,
        };
        if stale {
            *scope = Some((
                identity,
                fingerprint,
                key,
                build_root_currency_index(program, state),
            ));
        }
        read(&scope.as_ref().unwrap().3)
    })
}

pub(in crate::checks::ranges::facts::dependencies) fn root_is_current(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    root: facts::PlaceRoot,
) -> bool {
    let facts::PlaceRoot::Symbol(symbol) = root else {
        return false;
    };
    symbol == machine.symbol
        || with_root_currency(program, state, |index| {
            index.parameters.contains(&symbol)
                || index
                    .local_first
                    .get(&symbol)
                    .is_some_and(|position| *position < statement_index)
        })
}
