//! Row lookups into the already-built borrow and proof fact tables.
//!
//! A state's borrow row and a call's contract row are addressed by their
//! recorded identity -- (machine, state) and the caller's statement/call
//! coordinate -- rather than by position, so a consumer never re-derives the
//! coordinate the producing pass already recorded.
//!
//! The arena tables are consulted once per call/statement across the flow
//! build; a linear rescan per site is O(states x calls) on large programs.
//! Freshness anchors on the ledger pointer plus arena lens and the first
//! state's storage address, so stale buckets from a previously visited ledger
//! can never redirect a lookup on a different program at the same stack
//! address.
use arena::Handle;
use checked_trees::{BorrowFacts, ContractCallFact, ProofFacts, StateBorrowFact};
use symbols::SymbolHandle;

pub(crate) struct BorrowStateIndex {
    by_key: std::collections::HashMap<(SymbolHandle, SymbolHandle), Vec<Handle<StateBorrowFact>>>,
}

impl BorrowStateIndex {
    pub(crate) fn bucket(
        &self,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    ) -> &[Handle<StateBorrowFact>] {
        self.by_key
            .get(&(machine_symbol, state_symbol))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

fn borrow_facts_fingerprint(borrow: &BorrowFacts) -> usize {
    let states_len = borrow.states.len();
    let calls_len = borrow.calls.len();
    let first_state = borrow
        .states
        .iter()
        .next()
        .map(|(_, state)| state as *const StateBorrowFact as usize)
        .unwrap_or(0);
    (borrow as *const BorrowFacts) as usize
        ^ states_len.rotate_left(17)
        ^ calls_len.rotate_left(31)
        ^ first_state
}

thread_local! {
    static BORROW_STATE_INDEX: std::cell::RefCell<
        Option<(*const BorrowFacts, usize, BorrowStateIndex)>,
    > = const { std::cell::RefCell::new(None) };
}

pub(crate) fn with_borrow_state_index<R>(
    borrow: &BorrowFacts,
    run: impl FnOnce(&BorrowStateIndex) -> R,
) -> R {
    BORROW_STATE_INDEX.with(|cell| {
        let mut slot = cell.borrow_mut();
        let fingerprint = borrow_facts_fingerprint(borrow);
        let fresh = matches!(&*slot, Some((owner, seen, _))
            if std::ptr::eq(*owner, borrow as *const _) && *seen == fingerprint);
        if !fresh {
            let mut index = BorrowStateIndex {
                by_key: std::collections::HashMap::new(),
            };
            for (handle, state) in borrow.states.iter() {
                index
                    .by_key
                    .entry((state.machine_symbol, state.state_symbol))
                    .or_default()
                    .push(handle);
            }
            *slot = Some((borrow as *const _, fingerprint, index));
        }
        let Some((_, _, index)) = &*slot else {
            unreachable!();
        };
        run(index)
    })
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

struct ProofCallIndex {
    by_key: std::collections::HashMap<
        (SymbolHandle, SymbolHandle, usize, usize),
        Handle<ContractCallFact>,
    >,
}

fn proof_facts_fingerprint(proof: &ProofFacts) -> usize {
    let calls_len = proof.contract_calls.len();
    let first_call = proof
        .contract_calls
        .iter()
        .next()
        .map(|(_, call)| call as *const ContractCallFact as usize)
        .unwrap_or(0);
    (proof as *const ProofFacts) as usize ^ calls_len.rotate_left(17) ^ first_call
}

thread_local! {
    static PROOF_CALL_INDEX: std::cell::RefCell<
        Option<(*const ProofFacts, usize, ProofCallIndex)>,
    > = const { std::cell::RefCell::new(None) };
}

pub(crate) fn proof_contract_call(
    proof: &ProofFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    call_ordinal: usize,
) -> Option<&ContractCallFact> {
    let handle = PROOF_CALL_INDEX.with(|cell| {
        let mut slot = cell.borrow_mut();
        let fingerprint = proof_facts_fingerprint(proof);
        let fresh = matches!(&*slot, Some((owner, seen, _))
            if std::ptr::eq(*owner, proof as *const _) && *seen == fingerprint);
        if !fresh {
            let mut index = ProofCallIndex {
                by_key: std::collections::HashMap::new(),
            };
            for (handle, call) in proof.contract_calls.iter() {
                index.by_key.insert(
                    (
                        call.caller_machine_symbol,
                        call.caller_state_symbol,
                        call.statement_index,
                        call.call_ordinal,
                    ),
                    handle,
                );
            }
            *slot = Some((proof as *const _, fingerprint, index));
        }
        let Some((_, _, index)) = &*slot else {
            unreachable!();
        };
        index
            .by_key
            .get(&(machine_symbol, state_symbol, statement_index, call_ordinal))
            .copied()
    })?;
    Some(proof.contract_calls.get(handle))
}
