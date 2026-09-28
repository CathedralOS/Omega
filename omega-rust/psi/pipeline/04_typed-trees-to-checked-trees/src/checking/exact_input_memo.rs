//! Whole-program checks memoized by exact input.
//!
//! A package review checks every package of one closure in its discovery
//! pass, again in its bound pass and again for production, and each of those
//! compiles checks the same typed program at the pre-build gate and again at
//! settlement; the inputs are usually identical. The language judgment is a
//! pure function of the typed program and the retained request fields, so an
//! equal input returns that judgment's result. Invocation-owned package authority
//! is never cached here: `lower_typed_trees` admits it after every lookup.
//! A program is retained once its
//! size fingerprint repeats, so a compile that checks each program once pays
//! only for computing that fingerprint; a caller that knows it will check
//! the same programs again holds [`retain_repeated_checks`] to retain them on
//! first sight.

use super::{
    CheckingMode, SelectedBoundaryFamilySpecialization,
    SelectedGenericOperatorProviderSpecialization,
};
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};
use typed_trees::TypedTrees;

/// Retained results. A review pass checks each package of its closure at two
/// checkpoints, in the same order every pass, so a table smaller than one
/// pass's checks evicts each result just before it is asked for again. This
/// covers closures of up to eight packages.
const RETAINED_RESULTS: usize = 16;
const REMEMBERED_FINGERPRINTS: usize = 64;

/// The request's content: what a check reads besides the program.
#[derive(Clone, PartialEq)]
pub(super) struct RequestKey {
    pub(super) mode: CheckingMode,
    pub(super) selected_generic_operator_providers:
        Vec<SelectedGenericOperatorProviderSpecialization>,
    pub(super) selected_boundary_families: Vec<SelectedBoundaryFamilySpecialization>,
    pub(super) opaque_property_receipts: Vec<validation::OpaqueDataPropertyReceipt>,
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
static EXECUTED_PRELIMINARY_CHECKS: AtomicU64 = AtomicU64::new(0);
static EXECUTED_SETTLED_CHECKS: AtomicU64 = AtomicU64::new(0);

/// Retains every check on first sight while held; see
/// [`retain_repeated_checks`].
pub struct RepeatedCheckRetention(());

impl RepeatedCheckRetention {
    /// Monotonic count of pre-build declaration-authority checks that
    /// actually executed. A request this memo serves from a retained result
    /// is still a request — the caller's `lower_typed_trees` accounting counts
    /// those — but is not an execution, so the delta between a caller's
    /// request total and this count names the memo-served repeats.
    pub fn executed_preliminary_check_count() -> u64 {
        EXECUTED_PRELIMINARY_CHECKS.load(Ordering::Relaxed)
    }

    /// Monotonic count of settled whole-program checks that actually
    /// executed, excluding requests served from a retained result.
    pub fn executed_settled_check_count() -> u64 {
        EXECUTED_SETTLED_CHECKS.load(Ordering::Relaxed)
    }
}

/// Record that `lower_typed_trees` is about to run a real check rather than
/// serve a retained result. Modes other than the two package checkpoints only
/// exist for this crate's own tests, which take the fresh-check return below
/// before reaching here.
fn account_executed_check(mode: CheckingMode) {
    let counter = match mode {
        CheckingMode::Preliminary => &EXECUTED_PRELIMINARY_CHECKS,
        CheckingMode::Settled => &EXECUTED_SETTLED_CHECKS,
        #[cfg(test)]
        CheckingMode::CrashFactInspection => &EXECUTED_SETTLED_CHECKS,
    };
    counter.fetch_add(1, Ordering::Relaxed);
}

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
    // OMEGA_LOWER_MEMO=1 reports each check's fingerprint, mode, and hit or
    // miss — the only way to observe which checkpoint a check served from.
    let probe = std::env::var_os("OMEGA_LOWER_MEMO").is_some();
    let fingerprint = fingerprint(&program, &request);
    {
        let mut memo = MEMO.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(position) = memo.entries.iter().position(|entry| {
            entry.fingerprint == fingerprint && entry.request == request && entry.program == program
        }) {
            let entry = memo.entries.remove(position);
            let result = entry.result.clone();
            memo.entries.push(entry);
            if probe {
                eprintln!(
                    "[lower-memo] HIT mode={:?} fp={fingerprint:016x}",
                    request.mode,
                );
            }
            return result;
        }
        // A package compile checks the same frozen program at the preliminary
        // checkpoint and again at settlement. The checkpoint only changes how
        // the check treats pending opaque-copy admission. Const completeness
        // was already checked before memo lookup. With no pending opaque-copy
        // claims the program checks identically under both modes, and a settled
        // request on such a program may serve the preliminary check's result.
        if request.mode == CheckingMode::Settled && preliminary_mode_is_vacuous(&program) {
            let preliminary_request = RequestKey {
                mode: CheckingMode::Preliminary,
                ..request.clone()
            };
            if let Some(position) = memo.entries.iter().position(|entry| {
                entry.fingerprint == fingerprint
                    && entry.request == preliminary_request
                    && programs_equal_for_check(&entry.program, &program)
            }) {
                let entry = memo.entries.remove(position);
                // The masked field is return cargo: the result embeds the
                // preliminary program, so the identities settlement stamped
                // on this program must come back along for post-lower pruning.
                let mut result = entry.result.clone();
                if let Ok(checked) = &mut result {
                    checked.typed.boundary_calling_plans = program.boundary_calling_plans.clone();
                }
                memo.entries.push(entry);
                if probe {
                    eprintln!(
                        "[lower-memo] HIT-PRELIMINARY mode={:?} fp={fingerprint:016x}",
                        request.mode,
                    );
                }
                return result;
            }
        }
        if !memo.fingerprints.contains(&fingerprint) {
            if memo.fingerprints.len() == REMEMBERED_FINGERPRINTS {
                memo.fingerprints.remove(0);
            }
            memo.fingerprints.push(fingerprint);
            // Preliminary results are also retained on first sight: the
            // settled checkpoint of the same compile asks for them next.
            if RETENTION_REQUESTS.load(Ordering::Relaxed) == 0
                && request.mode != CheckingMode::Preliminary
            {
                drop(memo);
                account_executed_check(request.mode);
                return check(program);
            }
        }
    }
    let retained = program.clone();
    let started = std::time::Instant::now();
    account_executed_check(request.mode);
    let result = check(program);
    if probe {
        eprintln!(
            "[lower-memo] MISS mode={:?} fp={fingerprint:016x} elapsed={:?}",
            request.mode,
            started.elapsed(),
        );
    }
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

/// Whether the checking mode reads any program state a preliminary check
/// tolerates differently than a settled one after input completeness has
/// been checked by `lower_typed_trees`. Opaque `[copy]` claims admit pending
/// selection only in the preliminary window; the toolchain-selection policy
/// arm is identical for both modes.
fn preliminary_mode_is_vacuous(program: &TypedTrees) -> bool {
    !program.data_definitions().iter().any(|definition| {
        definition.supply_mode == language_semantics::DataSupplyMode::BoundaryOpaque
            && definition.properties.multiplicity == language_semantics::Multiplicity::Unrestricted
    })
}

/// Equality over the state the check consumes. Settlement stamps derived
/// boundary calling-policy identities on the program between the two
/// checkpoints; `lower_typed_trees` never reads them (their consumer is
/// post-lower product pruning), so they are masked out of the comparison
/// while every other field — including any added later — still compares
/// exactly.
fn programs_equal_for_check(retained: &TypedTrees, incoming: &TypedTrees) -> bool {
    if retained == incoming {
        return true;
    }
    let mut retained = retained.clone();
    let mut incoming = incoming.clone();
    retained.boundary_calling_plans.clear();
    incoming.boundary_calling_plans.clear();
    retained == incoming
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
