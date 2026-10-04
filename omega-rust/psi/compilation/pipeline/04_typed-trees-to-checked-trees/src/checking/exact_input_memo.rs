//! Whole-program checks memoized by exact input.
//!
//! A package review checks every package of one closure in its discovery
//! pass, again in its bound pass and again for production, and each of those
//! compiles checks the same typed program at the pre-build gate and again at
//! settlement; the inputs are usually identical. The language judgment is a
//! pure function of the typed program and the retained request fields, so an
//! equal input returns that judgment's result. Equal means the whole typed
//! program: this memo masks no field out of the comparison and rewrites no
//! field of a judgment it serves, so a served judgment is the judgment that
//! was checked rather than a copy of it carrying some other checkpoint's
//! derived identities. Invocation-owned package authority
//! is never cached here: `lower_typed_trees` admits it after every lookup.
//! A program is retained once its
//! size fingerprint repeats, so a compile that checks each program once pays
//! only for computing that fingerprint; a caller that knows it will check
//! the same programs again holds [`retain_repeated_checks`] to retain them on
//! first sight.

use super::CheckingMode;
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use typed_trees::TypedTrees;

/// Retained results. A review pass checks each package of its closure at two
/// checkpoints, in the same order every pass, so a table smaller than one
/// pass's checks evicts each result just before it is asked for again. This
/// covers closures of up to eight packages.
const RETAINED_RESULTS: usize = 16;
const REMEMBERED_FINGERPRINTS: usize = 64;

/// The request's content: what a check reads besides the program.
#[derive(PartialEq)]
pub(super) struct RequestKey {
    pub(super) mode: CheckingMode,
    pub(super) opaque_property_receipts: Vec<validation::OpaqueDataPropertyReceipt>,
}

impl RequestKey {
    /// Whether this retained key is `incoming`'s request taken at the
    /// preliminary checkpoint: the mode is that checkpoint's and every other
    /// field agrees. The pattern is exhaustive, so a field added to
    /// `RequestKey` later fails to compile here instead of silently dropping
    /// out of the comparison.
    fn is_preliminary_form_of(&self, incoming: &Self) -> bool {
        let Self {
            mode,
            opaque_property_receipts,
        } = self;
        *mode == CheckingMode::Preliminary
            && *opaque_property_receipts == incoming.opaque_property_receipts
    }
}

/// One retained judgment. Both the program that keyed it and the judgment
/// itself are held as shared values: a lookup, a retention and a served
/// request each take a reference count, and the only copy any of them makes
/// is the one its caller's owned return demands.
struct Entry {
    fingerprint: u64,
    program: Arc<TypedTrees>,
    request: RequestKey,
    result: Arc<Result<CheckedTrees, Vec<Diagnostic>>>,
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

/// Hand back a share of the judgment the entry at `position` holds and make
/// that entry the most recently used. Every serve path goes through here, so
/// no path can produce a judgment that differs from the one retained.
fn serve(memo: &mut Memo, position: usize) -> Arc<Result<CheckedTrees, Vec<Diagnostic>>> {
    let entry = memo.entries.remove(position);
    let result = Arc::clone(&entry.result);
    memo.entries.push(entry);
    result
}

/// `check(program)`, or the retained result of an earlier check of an equal
/// program under an equal request.
///
/// The judgment comes back shared. Every caller needs an owned `CheckedTrees`
/// it may rewrite, so exactly one copy of a served judgment is made, and it is
/// made where that ownership is taken rather than here: a request served from
/// a retained entry and a request that ran the check both hand back a share of
/// the one instance this memo holds.
pub(super) fn checked(
    program: TypedTrees,
    request: RequestKey,
    check: impl FnOnce(Arc<TypedTrees>) -> Result<CheckedTrees, Vec<Diagnostic>>,
) -> Arc<Result<CheckedTrees, Vec<Diagnostic>>> {
    let program = Arc::new(program);
    // This crate's own tests steer checking through thread-local knobs no
    // key can see; they always check afresh.
    if cfg!(test) {
        return Arc::new(check(program));
    }
    // OMEGA_LOWER_MEMO=1 reports each check's fingerprint, mode, and hit or
    // miss — the only way to observe which checkpoint a check served from.
    let probe = std::env::var_os("OMEGA_LOWER_MEMO").is_some();
    let fingerprint = fingerprint(&program, &request);
    {
        let mut memo = MEMO.lock().unwrap_or_else(PoisonError::into_inner);
        // One notion of program equality for both serve paths: the whole
        // typed program, every field, by its own `PartialEq`. The carrier tag
        // is the only exemption and it is declared with the tag itself, since
        // `ProgramIdentity`'s equality is vacuous by construction. A field
        // added to `TypedTrees` later joins this comparison with no decision
        // to make, so no field can leave it silently.
        let serves_program =
            |entry: &Entry| entry.fingerprint == fingerprint && *entry.program == *program;
        if let Some(position) = memo
            .entries
            .iter()
            .position(|entry| serves_program(entry) && entry.request == request)
        {
            if probe {
                eprintln!(
                    "[lower-memo] HIT mode={:?} fp={fingerprint:016x}",
                    request.mode,
                );
            }
            return serve(&mut memo, position);
        }
        // A package compile checks the same frozen program at the preliminary
        // checkpoint and again at settlement. The checkpoint only changes how
        // the check treats pending opaque-copy admission. Const completeness
        // was already checked before memo lookup. With no pending opaque-copy
        // claims the program checks identically under both modes, and a settled
        // request on such a program may serve the preliminary check's result.
        if request.mode == CheckingMode::Settled
            && preliminary_mode_is_vacuous(&program)
            && let Some(position) = memo.entries.iter().position(|entry| {
                serves_program(entry) && entry.request.is_preliminary_form_of(&request)
            })
        {
            if probe {
                eprintln!(
                    "[lower-memo] HIT-PRELIMINARY mode={:?} fp={fingerprint:016x}",
                    request.mode,
                );
            }
            return serve(&mut memo, position);
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
                // Nothing retains this program, so the check takes sole
                // custody of it and copies nothing.
                return Arc::new(check(program));
            }
        }
    }
    let retained = Arc::clone(&program);
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
    let result = Arc::new(result);
    let mut memo = MEMO.lock().unwrap_or_else(PoisonError::into_inner);
    if memo.entries.len() == RETAINED_RESULTS {
        memo.entries.remove(0);
    }
    memo.entries.push(Entry {
        fingerprint,
        program: retained,
        request,
        result: Arc::clone(&result),
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

/// Table sizes and boundary symbols: equal programs always agree on it, and
/// the full comparison decides a match.
fn fingerprint(program: &TypedTrees, request: &RequestKey) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    program
        .excluded_authored_selection_occurrences
        .hash(&mut hasher);
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
    request.opaque_property_receipts.len().hash(&mut hasher);
    hasher.finish()
}
