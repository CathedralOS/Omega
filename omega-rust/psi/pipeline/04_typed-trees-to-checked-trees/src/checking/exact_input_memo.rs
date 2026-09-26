//! Whole-program checks memoized by exact input.
//!
//! A package review checks every package of one closure in its discovery
//! pass, again in its bound pass and again for production, and each of those
//! compiles checks the same typed program at the pre-build gate and again at
//! settlement; the inputs are usually identical. `lower_typed_trees` is a pure
//! function of the typed program and the request, so an input equal to a
//! retained one returns that check's result. A program is retained once its
//! size fingerprint repeats, so a compile that checks each program once pays
//! only for computing that fingerprint; a caller that knows it will check
//! the same programs again holds [`retain_repeated_checks`] to retain them on
//! first sight.

use super::{
    CheckingMode, SelectedBoundaryFamilySpecialization,
    SelectedGenericOperatorProviderSpecialization,
};
use crate::checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};
use symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees;

/// Retained results; a review alternates between a closure's packages and
/// checks each at two checkpoints.
const RETAINED_RESULTS: usize = 4;
const REMEMBERED_FINGERPRINTS: usize = 64;

/// The request's content: what a check reads besides the program.
#[derive(Clone, PartialEq)]
pub(super) struct RequestKey {
    pub(super) mode: CheckingMode,
    pub(super) selected_generic_operator_providers:
        Vec<SelectedGenericOperatorProviderSpecialization>,
    pub(super) selected_boundary_families: Vec<SelectedBoundaryFamilySpecialization>,
    pub(super) opaque_property_receipts: Vec<crate::validation::OpaqueDataPropertyReceipt>,
}

struct Entry {
    fingerprint: u64,
    program: TypedTrees,
    request: RequestKey,
    result: Result<CheckedTrees, Vec<Diagnostic>>,
}

struct Memo {
    /// Least recently used first.
    entries: Vec<Entry>,
    fingerprints: Vec<u64>,
}

static MEMO: Mutex<Memo> = Mutex::new(Memo {
    entries: Vec::new(),
    fingerprints: Vec::new(),
});

static RETENTION_REQUESTS: AtomicUsize = AtomicUsize::new(0);

/// Retains every check on first sight while held; see
/// [`retain_repeated_checks`].
pub struct RepeatedCheckRetention(());

impl Drop for RepeatedCheckRetention {
    fn drop(&mut self) {
        RETENTION_REQUESTS.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Retain every check on first sight until the guard drops, for a caller
/// about to check the same programs again (a package review's passes).
pub fn retain_repeated_checks() -> RepeatedCheckRetention {
    RETENTION_REQUESTS.fetch_add(1, Ordering::Relaxed);
    RepeatedCheckRetention(())
}

/// `check(program)`, or the retained result of an earlier check of an equal
/// program under an equal request.
pub(super) fn checked(
    program: TypedTrees,
    request: RequestKey,
    check: impl FnOnce(TypedTrees) -> Result<CheckedTrees, Vec<Diagnostic>>,
) -> Result<CheckedTrees, Vec<Diagnostic>> {
    // This crate's own tests steer checking through thread-local knobs no
    // key can see; they always check afresh.
    if cfg!(test) {
        return check(program);
    }
    let fingerprint = fingerprint(&program, &request);
    {
        let mut memo = MEMO.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(position) = memo.entries.iter().position(|entry| {
            entry.fingerprint == fingerprint && entry.request == request && entry.program == program
        }) {
            let entry = memo.entries.remove(position);
            let result = entry.result.clone();
            memo.entries.push(entry);
            return result;
        }
        if !memo.fingerprints.contains(&fingerprint) {
            if memo.fingerprints.len() == REMEMBERED_FINGERPRINTS {
                memo.fingerprints.remove(0);
            }
            memo.fingerprints.push(fingerprint);
            if RETENTION_REQUESTS.load(Ordering::Relaxed) == 0 {
                drop(memo);
                return check(program);
            }
        }
    }
    let retained = program.clone();
    let result = check(program);
    let mut memo = MEMO.lock().unwrap_or_else(PoisonError::into_inner);
    if memo.entries.len() == RETAINED_RESULTS {
        memo.entries.remove(0);
    }
    memo.entries.push(Entry {
        fingerprint,
        program: retained,
        request,
        result: result.clone(),
    });
    result
}

/// Table sizes and boundary symbols: equal programs always agree on it, and
/// the full comparison decides a match.
fn fingerprint(program: &TypedTrees, request: &RequestKey) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let machines = program.machines();
    machines.len().hash(&mut hasher);
    machines
        .first()
        .map(|machine| machine.symbol)
        .hash(&mut hasher);
    machines
        .last()
        .map(|machine| machine.symbol)
        .hash(&mut hasher);
    program.data_definitions().len().hash(&mut hasher);
    program.traits().len().hash(&mut hasher);
    program
        .expression_table
        .expression_count()
        .hash(&mut hasher);
    program.statement_table.statement_count().hash(&mut hasher);
    request
        .selected_generic_operator_providers
        .len()
        .hash(&mut hasher);
    request.selected_boundary_families.len().hash(&mut hasher);
    request.opaque_property_receipts.len().hash(&mut hasher);
    hasher.finish()
}
