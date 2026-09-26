//! Memos over one frozen program. Between validation and execution
//! finalization `lower_typed_trees` checks a program no pass restructures,
//! and many consumers there derive the same program-pure tables: every
//! check-pass consumer builds its own call-frame resolver, two dozen sites
//! classify proof-only data, and every structural judge rescans the trait
//! laws. Inside a scope they share one copy.

use crate::machine_calls::calls::CallFrameCaches;
use crate::proof_contracts::contract_entailment::structural_judgment::{
    EntryMachines, LicenseCandidates,
};
use std::cell::RefCell;
use std::sync::{Arc, OnceLock};
use typed_trees::TypedTrees;
use typed_trees::proof_only::ProofOnlyClassification;

thread_local! {
    static FROZEN_PROGRAM: RefCell<Option<FrozenProgramSlot>> = const { RefCell::new(None) };
}

struct FrozenProgramSlot {
    program: usize,
    identity: u64,
    memos: Arc<FrozenProgramMemos>,
}

#[derive(Default)]
pub(crate) struct FrozenProgramMemos {
    pub(crate) call_frames: Arc<CallFrameCaches>,
    proof_only: OnceLock<Arc<ProofOnlyClassification>>,
    pub(crate) license_candidates: OnceLock<Arc<LicenseCandidates>>,
    pub(crate) entry_machines: OnceLock<Arc<EntryMachines>>,
}

/// Restores the enclosing scope, if any, when dropped.
pub struct FrozenProgramScope {
    enclosing: Option<FrozenProgramSlot>,
}

impl Drop for FrozenProgramScope {
    fn drop(&mut self) {
        FROZEN_PROGRAM.with(|slot| *slot.borrow_mut() = self.enclosing.take());
    }
}

/// Share program-pure memos for `program` on this thread until the guard
/// drops. The caller keeps `program` in place and leaves everything the memos
/// read unchanged while the guard lives. Any other program, including a clone
/// at another address, keeps private memos.
pub fn enter_frozen_program_scope(program: &TypedTrees) -> FrozenProgramScope {
    let slot = FrozenProgramSlot {
        program: std::ptr::from_ref(program).addr(),
        identity: program.identity.get(),
        memos: Arc::default(),
    };
    FrozenProgramScope {
        enclosing: FROZEN_PROGRAM.with(|scope| scope.borrow_mut().replace(slot)),
    }
}

/// The open scope's memos when `program` is its frozen program.
pub(crate) fn frozen_program_memos(program: &TypedTrees) -> Option<Arc<FrozenProgramMemos>> {
    FROZEN_PROGRAM.with(|scope| {
        scope
            .borrow()
            .as_ref()
            .filter(|slot| {
                slot.program == std::ptr::from_ref(program).addr()
                    && slot.identity == program.identity.get()
            })
            .map(|slot| slot.memos.clone())
    })
}

/// `compute`, once per frozen program in the memo `slot` selects; afresh for
/// any other program.
pub(crate) fn frozen_memo<T>(
    program: &TypedTrees,
    slot: impl FnOnce(&FrozenProgramMemos) -> &OnceLock<Arc<T>>,
    compute: impl FnOnce() -> T,
) -> Arc<T> {
    match frozen_program_memos(program) {
        Some(memos) => slot(&memos).get_or_init(|| Arc::new(compute())).clone(),
        None => Arc::new(compute()),
    }
}

/// `typed_trees::proof_only::classify`, computed once per frozen program.
pub fn proof_only_classification(program: &TypedTrees) -> Arc<ProofOnlyClassification> {
    frozen_memo(
        program,
        |memos| &memos.proof_only,
        || typed_trees::proof_only::classify(program),
    )
}
