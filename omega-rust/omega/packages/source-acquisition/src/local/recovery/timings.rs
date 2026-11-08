//! Opt-in cost observations for one historical local-source lookup.
//! Counts and clocks are absent unless the review timing flag is set. The
//! aggregate contains no source paths, digests, identities or per-entry output.

use std::time::{Duration, Instant};

pub(super) struct RecoveryTimings {
    started: Instant,
    entries_enumerated: usize,
    routing_reads: usize,
    routing_elapsed: Duration,
    matching_candidates: usize,
    direct_lookups: usize,
    direct_hits: usize,
    direct_misses: usize,
    legacy_lookups: usize,
    lock_elapsed: Duration,
    verification_elapsed: Duration,
}

impl RecoveryTimings {
    pub(super) fn requested() -> Option<Self> {
        std::env::var_os("OMEGA_REVIEW_TIMINGS")
            .is_some()
            .then(|| Self {
                started: Instant::now(),
                entries_enumerated: 0,
                routing_reads: 0,
                routing_elapsed: Duration::ZERO,
                matching_candidates: 0,
                direct_lookups: 0,
                direct_hits: 0,
                direct_misses: 0,
                legacy_lookups: 0,
                lock_elapsed: Duration::ZERO,
                verification_elapsed: Duration::ZERO,
            })
    }

    pub(super) fn entry_enumerated(&mut self) {
        self.entries_enumerated += 1;
    }

    pub(super) fn routing_started(&mut self) -> Instant {
        self.routing_reads += 1;
        Instant::now()
    }

    pub(super) fn routing_finished(&mut self, started: Instant) {
        self.routing_elapsed = self.routing_elapsed.saturating_add(started.elapsed());
    }

    pub(super) fn candidate_matched(&mut self) {
        self.matching_candidates += 1;
    }

    pub(super) fn direct_lookup_started(&mut self) {
        self.direct_lookups += 1;
    }

    pub(super) fn direct_hit(&mut self) {
        self.direct_hits += 1;
    }

    pub(super) fn direct_miss(&mut self) {
        self.direct_misses += 1;
    }

    pub(super) fn legacy_lookup_started(&mut self) {
        self.legacy_lookups += 1;
    }

    pub(super) fn lock_finished(&mut self, started: Instant) {
        // Acquisition includes opening/custody checks as well as any wait;
        // this is not a claim to measure only time blocked on the OS lock.
        self.lock_elapsed = self.lock_elapsed.saturating_add(started.elapsed());
    }

    pub(super) fn verification_finished(&mut self, started: Instant) {
        // Includes selected-tree custody/rehash and its surrounding exact
        // collection/lock identity checks, including a failing verification.
        self.verification_elapsed = self.verification_elapsed.saturating_add(started.elapsed());
    }
}

impl Drop for RecoveryTimings {
    fn drop(&mut self) {
        eprintln!(
            "omega_review_timings note=local_source_recovery direct_lookups={} direct_hits={} direct_misses={} legacy_lookups={} entries_enumerated={} routing_reads={} routing_micros={} matching_candidates={} lock_acquire_micros={} selected_verification_micros={} elapsed_micros={}",
            self.direct_lookups,
            self.direct_hits,
            self.direct_misses,
            self.legacy_lookups,
            self.entries_enumerated,
            self.routing_reads,
            self.routing_elapsed.as_micros(),
            self.matching_candidates,
            self.lock_elapsed.as_micros(),
            self.verification_elapsed.as_micros(),
            self.started.elapsed().as_micros(),
        );
    }
}
