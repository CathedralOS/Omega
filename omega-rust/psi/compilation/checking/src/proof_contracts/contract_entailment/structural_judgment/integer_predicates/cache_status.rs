//! Temporary status census under the existing phase-profile flag.
//! Legacy-key recurrence is diagnostic only, not complete question equality.
//! Relaxed checkpoints are aggregate attempts, not final totals or CPU time.
//! The bounded stack sees same-thread recursion only; cross-thread misses are
//! not classified as pending. No proof decision reads this observer.

use super::{PreparedIntegerPremisesCacheKey, StructuralJudge};
use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use symbols::SymbolHandle;

const MAX_PENDING: usize = 64; // Observer storage only, never a proof budget.

#[derive(Clone, Copy, Debug)]
pub(super) struct Request {
    cache: usize,
    legacy_key: Option<PreparedIntegerPremisesCacheKey>,
    machine: SymbolHandle,
    state: SymbolHandle,
    body_frames: usize,
    returned_field_frames: usize,
    active_application: bool,
}

thread_local! {
    static PENDING: RefCell<Vec<Request>> = const { RefCell::new(Vec::new()) };
}

#[derive(Clone, Copy, Debug)]
enum Event {
    HitSome,
    HitNone,
    Miss,
    SameThreadPendingLegacyKey,
    Eviction,
    CompletionSome,
    CompletionNone,
    ContradictionFallback,
    GoalFirst,
    PendingUntracked,
}

static COUNTERS: [AtomicU64; 10] = [const { AtomicU64::new(0) }; 10];
static CALLBACKS: AtomicU64 = AtomicU64::new(0);

pub(super) fn request(
    judge: &StructuralJudge<'_>,
    legacy_key: Option<PreparedIntegerPremisesCacheKey>,
) -> Option<Request> {
    phase_profile::enabled().then(|| Request {
        cache: std::sync::Arc::as_ptr(&judge.prepared_integer_premises_cache) as usize,
        legacy_key,
        machine: judge.machine_symbol,
        state: judge.declaration_state,
        body_frames: judge.body_invocations.len(),
        returned_field_frames: judge.returned_field_owners.len(),
        active_application: judge.active_application.is_some(),
    })
}

fn note(event: Event, request: Option<Request>, pending_depth: Option<usize>) {
    let _ =
        COUNTERS[event as usize].try_update(Relaxed, Relaxed, |old| Some(old.saturating_add(1)));
    let prior = CALLBACKS
        .try_update(Relaxed, Relaxed, |old| Some(old.saturating_add(1)))
        .unwrap_or_else(|old| old);
    let count = prior.saturating_add(1);
    if count < 64 || !count.is_power_of_two() {
        return;
    }
    let values = COUNTERS.each_ref().map(|counter| counter.load(Relaxed));
    // Explicit field reads keep these exact handles/counts in diagnostic output.
    let context = request.map(|r| {
        (
            r.cache,
            r.legacy_key,
            r.machine,
            r.state,
            r.body_frames,
            r.returned_field_frames,
            r.active_application,
        )
    });
    eprintln!(
        "integer-premise-cache-status callbacks={count} hit_some={} hit_none={} miss={} same_thread_pending_legacy_key={} evictions={} completion_some={} completion_none={} contradiction_fallback={} goal_first={} pending_untracked={} event={event:?} context(cache,key,machine,state,body_frames,returned_field_frames,active_application)={context:?} tracked_pending_depth={pending_depth:?} structural_depth=not_carried",
        values[0],
        values[1],
        values[2],
        values[3],
        values[4],
        values[5],
        values[6],
        values[7],
        values[8],
        values[9],
    );
}

pub(super) fn hit(request: Option<Request>, some: bool) {
    if let Some(request) = request {
        note(
            if some { Event::HitSome } else { Event::HitNone },
            Some(request),
            None,
        );
    }
}

pub(super) struct Preparation {
    request: Request,
}

pub(super) fn preparing(request: Option<Request>) -> Option<Preparation> {
    let request = request?;
    note(Event::Miss, Some(request), None);
    PENDING.with(|pending| {
        let Ok(mut stack) = pending.try_borrow_mut() else {
            note(Event::PendingUntracked, Some(request), None);
            return None;
        };
        let depth = stack.len();
        if stack
            .iter()
            .any(|active| active.cache == request.cache && active.legacy_key == request.legacy_key)
        {
            note(
                Event::SameThreadPendingLegacyKey,
                Some(request),
                Some(depth),
            );
        }
        if depth >= MAX_PENDING || stack.try_reserve(1).is_err() {
            note(Event::PendingUntracked, Some(request), Some(depth));
            return None;
        }
        stack.push(request);
        Some(Preparation { request })
    })
}

impl Drop for Preparation {
    fn drop(&mut self) {
        PENDING.with(|pending| {
            let Ok(mut stack) = pending.try_borrow_mut() else {
                note(Event::PendingUntracked, Some(self.request), None);
                return;
            };
            // Each tracked frame owns one entry. Untracked recursion cannot
            // pop another frame; unwind follows this same cleanup path.
            if let Some(position) = stack.iter().rposition(|active| {
                active.cache == self.request.cache && active.legacy_key == self.request.legacy_key
            }) {
                stack.remove(position);
            }
        });
    }
}

pub(super) fn completion(request: Option<Request>, some: bool) {
    if let Some(request) = request {
        note(
            if some {
                Event::CompletionSome
            } else {
                Event::CompletionNone
            },
            Some(request),
            None,
        );
    }
}

pub(super) fn eviction() {
    if phase_profile::enabled() {
        // A preparation can be untracked at the observer cap. Do not label
        // its eviction with some tracked ancestor's request context.
        note(Event::Eviction, None, None);
    }
}

pub(super) fn contradiction_fallback(judge: &StructuralJudge<'_>) {
    if let Some(request) = request(judge, None) {
        note(Event::ContradictionFallback, Some(request), None);
    }
}

pub(super) fn goal_first(judge: &StructuralJudge<'_>) {
    if let Some(request) = request(judge, None) {
        note(Event::GoalFirst, Some(request), None);
    }
}
