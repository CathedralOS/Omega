//! Row lookups into the already-built borrow and proof fact tables.
//!
//! A state's borrow row and a call's contract row are addressed by their
//! recorded identity -- (machine, state) and the caller's statement/call
//! coordinate -- rather than by position, so a consumer never re-derives the
//! coordinate the producing pass already recorded.
//!
//! The arena tables are consulted once per call/statement across the flow
//! build; a linear rescan per site is O(states x calls) on large programs, so
//! each table's index is owned by the table itself (`BorrowFacts::state_index`,
//! `ProofFacts::call_index`) and guarded by the arena revision — an index can
//! never outlive the ledger it describes, and no per-pass scope is needed.
use arena::Handle;
use checked_trees::{BorrowFacts, BorrowStateIndex, ContractCallFact, ProofFacts, StateBorrowFact};
use symbols::SymbolHandle;

pub(crate) fn with_borrow_state_index<R>(
    borrow: &BorrowFacts,
    run: impl FnOnce(&BorrowStateIndex) -> R,
) -> R {
    borrow.with_state_index(run)
}

pub(crate) fn borrow_state_fact(
    borrow: &BorrowFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> Option<(Handle<StateBorrowFact>, &StateBorrowFact)> {
    let handle = with_borrow_state_index(borrow, |index| {
        index.bucket(machine_symbol, state_symbol).first().copied()
    })?;
    Some((handle, borrow.states.get(handle)))
}

pub(crate) fn proof_contract_call(
    proof: &ProofFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    call_ordinal: usize,
) -> Option<&ContractCallFact> {
    // The index's `insert` keeps the last row recorded under a coordinate,
    // matching the `.last()` verdict of the linear scan it replaced.
    let key = (machine_symbol, state_symbol, statement_index, call_ordinal);
    let handle = proof.with_call_index(|index| index.get(&key))?;
    Some(proof.contract_calls.get(handle))
}
