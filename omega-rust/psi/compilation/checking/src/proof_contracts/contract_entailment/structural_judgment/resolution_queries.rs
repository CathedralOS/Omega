//! Symbolic resolution retains exact inputs and the recurrence checks it reads.
//!
//! An unresolved helper can cause arithmetic-premise projection to re-enter
//! the resolver with a fresh depth. Recurrence still consumes the exact active
//! application and proof context. A completed query may reuse its result only
//! if those external membership questions still have the same answers.
//! Locally entered applications belong to that query's execution, not its
//! caller dependencies. Each read retains the caller-selected term mode; entering
//! a question never changes that mode. Cache hits replay their external reads into every
//! enclosing query. Changed immutable inputs or dependency observers decline
//! memoization; no partial read set can authorize reuse.

use super::{StructuralJudge, StructuralJudgment, StructuralTerm};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hasher};
use std::sync::{Arc, Mutex};

pub(super) struct ApplicationResolution<'program> {
    application: StructuralTerm,
    fingerprint: u64,
    depth: usize,
    context: StructuralJudge<'program>,
    previous: Option<Arc<Self>>,
}

#[derive(Default)]
pub(super) struct CompletedResolutions<'program> {
    entries: Vec<CompletedResolution<'program>>,
    buckets: HashMap<u64, Vec<usize>>,
    ages: Vec<u64>,
    clock: u64,
}

#[derive(Clone)]
struct ApplicationRead {
    application: StructuralTerm,
    fingerprint: u64,
    was_active: bool,
    specification_terms: bool,
}

pub(super) struct ResolutionObservations<'program> {
    base_active: Option<Arc<ApplicationResolution<'program>>>,
    context: StructuralJudge<'program>,
    reads: Mutex<Option<Vec<ApplicationRead>>>,
    previous: Option<Arc<Self>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StructuralQueryKind {
    Resolution,
    Predicate,
}

#[derive(Clone)]
enum StructuralQueryAnswer {
    Resolution(StructuralTerm),
    Predicate(StructuralJudgment),
}

struct CompletedResolution<'program> {
    fingerprint: u64,
    kind: StructuralQueryKind,
    term: StructuralTerm,
    context: StructuralJudge<'program>,
    reads: Vec<ApplicationRead>,
    depth: usize,
    resolution_depth: usize,
    value: StructuralQueryAnswer,
}

// Formatting supplies only a bucket filter; full authority-bearing equality
// still decides reuse, including payload omitted by semantic Debug output.
fn fingerprint(term: &StructuralTerm) -> u64 {
    struct Writer(DefaultHasher);
    impl std::fmt::Write for Writer {
        fn write_str(&mut self, text: &str) -> std::fmt::Result {
            self.0.write(text.as_bytes());
            Ok(())
        }
    }
    let mut writer = Writer(DefaultHasher::new());
    std::fmt::write(&mut writer, format_args!("{term:?}")).expect("fingerprint writer cannot fail");
    writer.0.finish()
}

impl<'program> CompletedResolutions<'program> {
    fn lookup(
        &mut self,
        judge: &StructuralJudge<'program>,
        kind: StructuralQueryKind,
        term: &StructuralTerm,
        depth: usize,
        fingerprint: u64,
    ) -> Option<(StructuralQueryAnswer, Vec<ApplicationRead>)> {
        let index = self
            .buckets
            .get(&fingerprint)?
            .iter()
            .rev()
            .copied()
            .find(|&index| {
                let entry = &self.entries[index];
                entry.kind == kind
                    && entry.depth == depth
                    && entry.resolution_depth == judge.application_resolution_depth()
                    && entry.context.same_resolution_context(judge)
                    && entry.term.same_resolution_payload(term)
                    && entry.reads.iter().all(|read| {
                        judge.application_is_active_in_mode(
                            &read.application,
                            read.fingerprint,
                            read.specification_terms,
                        ) == read.was_active
                    })
            })?;
        self.clock = self.clock.wrapping_add(1);
        self.ages[index] = self.clock;
        let entry = &self.entries[index];
        Some((entry.value.clone(), entry.reads.clone()))
    }

    fn insert(&mut self, entry: CompletedResolution<'program>) {
        self.clock = self.clock.wrapping_add(1);
        let fingerprint = entry.fingerprint;
        let position = if self.entries.len() < 8192 {
            let position = self.entries.len();
            self.entries.push(entry);
            self.ages.push(self.clock);
            position
        } else {
            let position = (0..self.ages.len())
                .min_by_key(|&position| self.ages[position])
                .expect("a full cache has an eviction candidate");
            let previous = self.entries[position].fingerprint;
            if let Some(bucket) = self.buckets.get_mut(&previous) {
                bucket.retain(|&index| index != position);
                if bucket.is_empty() {
                    self.buckets.remove(&previous);
                }
            }
            self.entries[position] = entry;
            self.ages[position] = self.clock;
            position
        };
        self.buckets.entry(fingerprint).or_default().push(position);
    }
}

impl<'program> StructuralJudge<'program> {
    pub(super) fn application_resolution_depth(&self) -> usize {
        self.active_application
            .as_ref()
            .map_or(0, |active| active.depth)
    }

    pub(super) fn completed_resolution(
        &self,
        term: StructuralTerm,
        depth: usize,
        compute: impl FnOnce(&Self, StructuralTerm) -> StructuralTerm,
    ) -> StructuralTerm {
        match self.completed_query(
            StructuralQueryKind::Resolution,
            term,
            depth,
            |judge, term| StructuralQueryAnswer::Resolution(compute(judge, term)),
        ) {
            StructuralQueryAnswer::Resolution(value) => value,
            StructuralQueryAnswer::Predicate(_) => unreachable!("resolution query kind changed"),
        }
    }

    pub(super) fn completed_predicate(
        &self,
        term: StructuralTerm,
        depth: usize,
        compute: impl FnOnce(&Self, StructuralTerm) -> StructuralJudgment,
    ) -> StructuralJudgment {
        match self.completed_query(
            StructuralQueryKind::Predicate,
            term,
            depth,
            |judge, term| StructuralQueryAnswer::Predicate(compute(judge, term)),
        ) {
            StructuralQueryAnswer::Predicate(value) => value,
            StructuralQueryAnswer::Resolution(_) => unreachable!("predicate query kind changed"),
        }
    }

    fn completed_query(
        &self,
        kind: StructuralQueryKind,
        term: StructuralTerm,
        depth: usize,
        compute: impl FnOnce(&Self, StructuralTerm) -> StructuralQueryAnswer,
    ) -> StructuralQueryAnswer {
        // Reuse inside one accumulating field trace preserves its observations.
        // Crossing to another trace cannot authorize an enclosing cache entry:
        // replaying that entry would skip the other trace's dependency recording.
        self.invalidate_changed_field_observers();
        let Some(cache) = &self.completed_resolutions else {
            return compute(self, term);
        };
        let fingerprint = fingerprint(&term);
        if let Ok(mut cache) = cache.lock()
            && let Some((value, reads)) = cache.lookup(self, kind, &term, depth, fingerprint)
        {
            drop(cache);
            for read in reads {
                self.observe_application_read(
                    &read.application,
                    read.fingerprint,
                    read.was_active,
                    read.specification_terms,
                );
            }
            phase_profile::count(
                match kind {
                    StructuralQueryKind::Resolution => "05.resolution.completed.hit",
                    StructuralQueryKind::Predicate => "05.resolution.predicate.hit",
                },
                1,
            );
            return value;
        }
        phase_profile::count(
            match kind {
                StructuralQueryKind::Resolution => "05.resolution.completed.miss",
                StructuralQueryKind::Predicate => "05.resolution.predicate.miss",
            },
            1,
        );
        let context = self.resolution_input_context();
        let observations = Arc::new(ResolutionObservations {
            base_active: self.active_application.clone(),
            context: context.clone(),
            reads: Mutex::new(Some(Vec::new())),
            previous: self.resolution_observations.clone(),
        });
        let mut computing = self.clone();
        computing.resolution_observations = Some(Arc::clone(&observations));
        let input = term.clone();
        let value = compute(&computing, term);
        drop(computing);
        let reads = observations
            .reads
            .lock()
            .ok()
            .and_then(|reads| reads.clone());
        if let Some(reads) = reads {
            phase_profile::count("05.resolution.dependencies.retained", reads.len() as u64);
            if let Ok(mut cache) = cache.lock() {
                cache.insert(CompletedResolution {
                    fingerprint,
                    kind,
                    term: input,
                    context,
                    reads,
                    depth,
                    resolution_depth: self.application_resolution_depth(),
                    value: value.clone(),
                });
            }
        } else {
            phase_profile::count("05.resolution.dependencies.unavailable", 1);
        }
        value
    }

    fn resolution_input_context(&self) -> Self {
        let mut context = self.clone();
        // Keys retain immutable inputs, never their containing cache or observer.
        context.active_application = None;
        context.completed_resolutions = None;
        context.resolution_observations = None;
        context
    }

    fn application_is_active(&self, application: &StructuralTerm, fingerprint: u64) -> bool {
        self.application_is_active_in_mode(application, fingerprint, self.specification_terms)
    }

    fn application_is_active_in_mode(
        &self,
        application: &StructuralTerm,
        fingerprint: u64,
        specification_terms: bool,
    ) -> bool {
        let mut active = self.active_application.as_deref();
        while let Some(frame) = active {
            if frame.fingerprint == fingerprint
                && frame.application.same_resolution_payload(application)
                && frame.context.specification_terms == specification_terms
                && frame.context.same_resolution_inputs(self)
            {
                return true;
            }
            active = frame.previous.as_deref();
        }
        false
    }

    fn invalidate_changed_field_observers(&self) {
        let mut observation = self.resolution_observations.as_deref();
        while let Some(current) = observation {
            if !current.context.same_returned_field_trace(self)
                && let Ok(mut reads) = current.reads.lock()
            {
                *reads = None;
            }
            observation = current.previous.as_deref();
        }
    }

    /// Whether this question matches an application entered inside the query.
    /// An absent base marker means the lineage changed and cannot be reused.
    fn locally_active_application(
        &self,
        application: &StructuralTerm,
        fingerprint: u64,
        base: Option<&Arc<ApplicationResolution<'program>>>,
        specification_terms: bool,
    ) -> Option<bool> {
        let mut active = self.active_application.as_ref();
        let mut local_match = false;
        loop {
            match (active, base) {
                (None, None) => return Some(local_match),
                (Some(frame), Some(base)) if Arc::ptr_eq(frame, base) => {
                    return Some(local_match);
                }
                (Some(frame), _) => {
                    local_match |= frame.fingerprint == fingerprint
                        && frame.application.same_resolution_payload(application)
                        && frame.context.specification_terms == specification_terms
                        && frame.context.same_resolution_inputs(self);
                    active = frame.previous.as_ref();
                }
                (None, Some(_)) => return None,
            }
        }
    }

    fn observe_application_read(
        &self,
        application: &StructuralTerm,
        fingerprint: u64,
        was_active: bool,
        specification_terms: bool,
    ) {
        let mut observation = self.resolution_observations.as_deref();
        while let Some(current) = observation {
            let local = self.locally_active_application(
                application,
                fingerprint,
                current.base_active.as_ref(),
                specification_terms,
            );
            if let Ok(mut recorded) = current.reads.lock() {
                if !current.context.same_resolution_inputs(self) || local.is_none() {
                    *recorded = None;
                } else if local == Some(false)
                    && let Some(reads) = recorded.as_mut()
                {
                    if let Some(previous) = reads.iter().find(|read| {
                        read.fingerprint == fingerprint
                            && read.specification_terms == specification_terms
                            && read.application.same_resolution_payload(application)
                    }) {
                        if previous.was_active != was_active {
                            *recorded = None;
                        }
                    } else if reads.len() >= 256 {
                        *recorded = None;
                    } else {
                        reads.push(ApplicationRead {
                            application: application.clone(),
                            fingerprint,
                            was_active,
                            specification_terms,
                        });
                    }
                }
            }
            observation = current.previous.as_deref();
        }
    }

    pub(super) fn resolving_application(
        &self,
        application: &StructuralTerm,
        depth: usize,
    ) -> Option<Self> {
        let fingerprint = fingerprint(application);
        let active = self.application_is_active(application, fingerprint);
        self.observe_application_read(application, fingerprint, active, self.specification_terms);
        if active {
            return None;
        }
        let context = self.resolution_input_context();
        let mut resolving = self.clone();
        resolving.active_application = Some(Arc::new(ApplicationResolution {
            application: application.clone(),
            fingerprint,
            depth: depth.max(self.application_resolution_depth()),
            context,
            previous: self.active_application.clone(),
        }));
        Some(resolving)
    }

    fn same_resolution_context(&self, other: &Self) -> bool {
        self.specification_terms == other.specification_terms
            && self.same_returned_field_trace(other)
            && self.same_resolution_inputs(other)
    }

    fn same_returned_field_trace(&self, other: &Self) -> bool {
        match (&self.returned_field_trace, &other.returned_field_trace) {
            (None, None) => true,
            (Some(left), Some(right)) => Arc::ptr_eq(left, right),
            _ => false,
        }
    }

    fn same_resolution_inputs(&self, other: &Self) -> bool {
        // Each active frame keeps these immutable owners alive. Pointer
        // equality therefore identifies the same premises/storage epoch, not
        // a recycled address. COW fact changes remain independent questions.
        // Each membership read supplies its exact term mode separately. Other
        // inputs stay exact; completed-result keys also compare the caller mode.
        // Program-pure memo cells and query observers supply no premises.
        std::ptr::eq(self.program, other.program)
            && Arc::ptr_eq(&self.entry_machines, &other.entry_machines)
            && self.machine_symbol == other.machine_symbol
            && self.declaration_state == other.declaration_state
            && Arc::ptr_eq(&self.declaration_parameters, &other.declaration_parameters)
            && std::ptr::eq(self.body_type_parameters, other.body_type_parameters)
            && self.premise_facts == other.premise_facts
            && Arc::ptr_eq(&self.normal_results, &other.normal_results)
            && Arc::ptr_eq(
                &self.completed_nominal_captures,
                &other.completed_nominal_captures,
            )
            && Arc::ptr_eq(&self.returned_field_owners, &other.returned_field_owners)
            && Arc::ptr_eq(&self.body_invocations, &other.body_invocations)
            && Arc::ptr_eq(&self.normal_memberships, &other.normal_memberships)
            && Arc::ptr_eq(
                &self.normal_content_guarantees,
                &other.normal_content_guarantees,
            )
            && Arc::ptr_eq(&self.invalidated_storage, &other.invalidated_storage)
            && Arc::ptr_eq(&self.storage_exposures, &other.storage_exposures)
            && Arc::ptr_eq(&self.stored_values, &other.stored_values)
            && self.capture_sequence == other.capture_sequence
            && self.storage_write_sequence == other.storage_write_sequence
            && Arc::ptr_eq(&self.normal_guarantees, &other.normal_guarantees)
            && Arc::ptr_eq(
                &self.captured_result_equations,
                &other.captured_result_equations,
            )
            && self.resolve_applications == other.resolve_applications
            && self.require_closed_computation == other.require_closed_computation
            && self.runtime_body_values == other.runtime_body_values
            && self.case_facts_only == other.case_facts_only
            && Arc::ptr_eq(&self.substitutions, &other.substitutions)
            && Arc::ptr_eq(&self.rewrites, &other.rewrites)
            && Arc::ptr_eq(&self.disequalities, &other.disequalities)
            && Arc::ptr_eq(&self.case_facts, &other.case_facts)
            && Arc::ptr_eq(&self.case_substitutions, &other.case_substitutions)
            && self.hypotheses_contradictory == other.hypotheses_contradictory
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use typed_trees::types::TypeReferenceHandle;

    #[test]
    fn completed_queries_keep_full_authority_depth_and_premise_context() {
        let program = typed_trees::TypedTrees::default();
        let judge = StructuralJudge::from_declaration_requires(
            &program,
            symbols::SymbolHandle::invalid(),
            &[],
            &[],
            true,
        );
        let calls = Cell::new(0);
        let compute = |_: &StructuralJudge<'_>, term| {
            calls.set(calls.get() + 1);
            term
        };
        let term = StructuralTerm::ObservedValue {
            observed_type: TypeReferenceHandle::from_arena_index(1),
            value: Box::new(StructuralTerm::Variable("input".to_owned())),
            descriptor: None,
        };
        assert!(judge.completed_resolution(term.clone(), 0, compute) == term);
        assert!(judge.clone().completed_resolution(term.clone(), 0, compute) == term);
        assert_eq!(calls.get(), 1);
        judge.completed_resolution(term.clone(), 1, compute);
        let mut changed = judge.clone();
        changed.capture_sequence += 1;
        changed.completed_resolution(term.clone(), 0, compute);
        let mut authority = term.clone();
        if let StructuralTerm::ObservedValue { observed_type, .. } = &mut authority {
            *observed_type = TypeReferenceHandle::from_arena_index(2);
        }
        assert!(authority == term);
        assert_eq!(fingerprint(&authority), fingerprint(&term));
        judge.completed_resolution(authority, 0, compute);
        assert_eq!(calls.get(), 4);
    }

    #[test]
    fn recurrence_reads_allow_unrelated_frames_but_reject_changed_cycle_answers() {
        let program = typed_trees::TypedTrees::default();
        let mut judge = StructuralJudge::from_declaration_requires(
            &program,
            symbols::SymbolHandle::invalid(),
            &[],
            &[],
            true,
        );
        judge.specification_terms = true;
        let question = StructuralTerm::Variable("question".to_owned());
        let unrelated = StructuralTerm::Variable("unrelated".to_owned());
        let goal = StructuralTerm::Variable("goal".to_owned());
        let active = judge.resolving_application(&question, 0).unwrap();
        let value = active.completed_resolution(goal.clone(), 0, |judge, _| {
            assert!(judge.resolving_application(&question, 0).is_none());
            StructuralTerm::Opaque("blocked".to_owned())
        });
        assert!(value == StructuralTerm::Opaque("blocked".to_owned()));
        let equivalent = active.resolving_application(&unrelated, 0).unwrap();
        let reused = equivalent.completed_resolution(goal.clone(), 0, |_, _| {
            panic!("the additional frame did not change the observed recurrence answer")
        });
        assert!(reused == value);
        drop(equivalent);
        let outside = judge.completed_resolution(goal, 0, |judge, _| {
            assert!(judge.resolving_application(&question, 0).is_some());
            StructuralTerm::Opaque("open".to_owned())
        });
        assert!(outside != value);
        let cache = Arc::downgrade(judge.completed_resolutions.as_ref().unwrap());
        drop(active);
        drop(judge);
        assert!(
            cache.upgrade().is_none(),
            "keys must not retain their cache or observers"
        );
    }

    #[test]
    fn cached_child_reads_reach_the_parent_without_exporting_local_cycles() {
        let program = typed_trees::TypedTrees::default();
        let mut judge = StructuralJudge::from_declaration_requires(
            &program,
            symbols::SymbolHandle::invalid(),
            &[],
            &[],
            true,
        );
        judge.specification_terms = true;
        let local = StructuralTerm::Variable("local".to_owned());
        let external = StructuralTerm::Variable("external".to_owned());
        let child_calls = Cell::new(0);
        let child = |judge: &StructuralJudge<'_>| {
            judge.completed_resolution(
                StructuralTerm::Variable("child".to_owned()),
                1,
                |judge, _| {
                    child_calls.set(child_calls.get() + 1);
                    assert!(judge.resolving_application(&local, 0).is_none());
                    StructuralTerm::Variable(
                        if judge.resolving_application(&external, 0).is_some() {
                            "open".to_owned()
                        } else {
                            "blocked".to_owned()
                        },
                    )
                },
            )
        };
        let primed = judge.resolving_application(&local, 0).unwrap();
        assert!(child(&primed) == StructuralTerm::Variable("open".to_owned()));
        let root_calls = Cell::new(0);
        let root = |judge: &StructuralJudge<'_>| {
            judge.completed_resolution(
                StructuralTerm::Variable("root".to_owned()),
                0,
                |judge, _| {
                    root_calls.set(root_calls.get() + 1);
                    child(&judge.resolving_application(&local, 0).unwrap())
                },
            )
        };
        assert!(root(&judge) == StructuralTerm::Variable("open".to_owned()));
        assert_eq!(
            child_calls.get(),
            1,
            "the parent consumed the primed child result"
        );
        assert!(root(&judge) == StructuralTerm::Variable("open".to_owned()));
        assert_eq!(
            root_calls.get(),
            1,
            "the child's local cycle is not an external dependency"
        );
        let changed = judge.resolving_application(&external, 0).unwrap();
        assert!(root(&changed) == StructuralTerm::Variable("blocked".to_owned()));
        assert_eq!(
            root_calls.get(),
            2,
            "the cached child's external read reaches its parent"
        );
        assert_eq!(child_calls.get(), 2);
    }

    #[test]
    fn changed_query_inputs_decline_dependency_reuse() {
        let program = typed_trees::TypedTrees::default();
        let judge = StructuralJudge::from_declaration_requires(
            &program,
            symbols::SymbolHandle::invalid(),
            &[],
            &[],
            true,
        );
        let calls = Cell::new(0);
        for _ in 0..2 {
            judge.completed_resolution(
                StructuralTerm::Variable("goal".to_owned()),
                0,
                |judge, term| {
                    calls.set(calls.get() + 1);
                    let mut changed = judge.clone();
                    changed.capture_sequence += 1;
                    changed.resolving_application(&term, 0);
                    term
                },
            );
        }
        assert_eq!(calls.get(), 2);
    }

    #[test]
    fn conditional_reuse_matches_uncached_queries_for_every_three_question_stack() {
        fn question(name: &str) -> StructuralTerm {
            StructuralTerm::Variable(name.to_owned())
        }
        fn stacked<'program>(
            base: &StructuralJudge<'program>,
            names: &[&str],
        ) -> StructuralJudge<'program> {
            let mut judge = base.clone();
            for name in names {
                judge = judge.resolving_application(&question(name), 0).unwrap();
            }
            judge
        }
        fn evaluate(judge: &StructuralJudge<'_>, calls: &Cell<usize>) -> StructuralTerm {
            judge.completed_resolution(question("root"), 0, |judge, _| {
                calls.set(calls.get() + 1);
                let Some(entered) = judge.resolving_application(&question("a"), 0) else {
                    return question("a-blocked");
                };
                entered.completed_resolution(question("child"), 1, |judge, _| {
                    if let Some(entered) = judge.resolving_application(&question("b"), 0) {
                        question(
                            if entered.resolving_application(&question("c"), 0).is_some() {
                                "c-open"
                            } else {
                                "c-blocked"
                            },
                        )
                    } else {
                        question("b-blocked")
                    }
                })
            })
        }
        let program = typed_trees::TypedTrees::default();
        let mut judge = StructuralJudge::from_declaration_requires(
            &program,
            symbols::SymbolHandle::invalid(),
            &[],
            &[],
            true,
        );
        judge.specification_terms = true;
        let mut reference = judge.clone();
        reference.completed_resolutions = None;
        let mut stacks = vec![Vec::new()];
        let mut frontier = vec![Vec::new()];
        for _ in 0..3 {
            let mut next = Vec::new();
            for prefix in frontier {
                for name in ["a", "b", "c"] {
                    if !prefix.contains(&name) {
                        let mut extended = prefix.clone();
                        extended.push(name);
                        next.push(extended);
                    }
                }
            }
            stacks.extend(next.iter().cloned());
            frontier = next;
        }
        assert_eq!(stacks.len(), 16);
        let cached_calls = Cell::new(0);
        let reference_calls = Cell::new(0);
        for stack in stacks {
            assert!(
                evaluate(&stacked(&judge, &stack), &cached_calls).same_resolution_payload(
                    &evaluate(&stacked(&reference, &stack), &reference_calls)
                ),
                "stack {stack:?} changed the result"
            );
        }
        assert!(cached_calls.get() < reference_calls.get());
    }

    #[test]
    fn field_dependency_recording_reuses_only_the_same_accumulating_trace() {
        let program = typed_trees::TypedTrees::default();
        let mut judge = StructuralJudge::from_declaration_requires(
            &program,
            symbols::SymbolHandle::invalid(),
            &[],
            &[],
            true,
        );
        judge.returned_field_trace = Some(Arc::new(std::sync::Mutex::new(
            super::super::returned_fields::ReturnedFieldTrace::default(),
        )));
        let calls = Cell::new(0);
        let dependency = symbols::SymbolHandle::invalid();
        let compute = |judge: &StructuralJudge<'_>, term| {
            calls.set(calls.get() + 1);
            if let Some(trace) = &judge.returned_field_trace {
                let mut trace = trace.lock().unwrap();
                trace.visited_callees.push(dependency);
                trace.min_cycle_owner_index = 0;
            }
            term
        };
        let query = |judge: &StructuralJudge<'_>| {
            judge.completed_resolution(StructuralTerm::Variable("value".to_owned()), 0, compute)
        };
        query(&judge);
        query(&judge);
        assert_eq!(calls.get(), 1);
        let trace = judge.returned_field_trace.as_ref().unwrap().lock().unwrap();
        assert_eq!(trace.visited_callees, vec![dependency]);
        assert_eq!(trace.min_cycle_owner_index, 0);
        drop(trace);

        let mut other = judge.clone();
        other.returned_field_trace = Some(Arc::new(Mutex::new(
            super::super::returned_fields::ReturnedFieldTrace::default(),
        )));
        query(&other);
        query(&other);
        assert_eq!(calls.get(), 2);
        assert_eq!(
            other
                .returned_field_trace
                .as_ref()
                .unwrap()
                .lock()
                .unwrap()
                .visited_callees,
            vec![dependency]
        );
        other.returned_field_trace = None;
        query(&other);
        query(&other);
        assert_eq!(calls.get(), 3);
        other.returned_field_trace = judge.returned_field_trace.clone();
        Arc::make_mut(&mut other.returned_field_owners).push(dependency);
        query(&other);
        assert_eq!(
            calls.get(),
            4,
            "a changed recursion owner chain must recompute"
        );
    }

    #[test]
    fn changing_field_observers_invalidates_enclosing_queries_not_recurrence() {
        let program = typed_trees::TypedTrees::default();
        let judge = StructuralJudge::from_declaration_requires(
            &program,
            symbols::SymbolHandle::invalid(),
            &[],
            &[],
            true,
        );
        let trace = Arc::new(Mutex::new(
            super::super::returned_fields::ReturnedFieldTrace::default(),
        ));
        let calls = Cell::new(0);
        let term = StructuralTerm::Variable("outer".to_owned());
        for _ in 0..2 {
            judge.completed_resolution(term.clone(), 0, |judge, term| {
                calls.set(calls.get() + 1);
                let mut changed = judge.resolving_application(&term, 0).unwrap();
                changed.returned_field_trace = Some(Arc::clone(&trace));
                assert!(
                    changed.resolving_application(&term, 0).is_none(),
                    "a dependency recorder is not a different recurrence context"
                );
                changed.completed_resolution(
                    StructuralTerm::Variable("inner".to_owned()),
                    0,
                    |_, term| term,
                );
                term
            });
        }
        assert_eq!(
            calls.get(),
            2,
            "the outer query cannot skip another recorder"
        );
    }
}

#[cfg(test)]
mod work_domain_tests {
    use super::*;
    #[test]
    fn bucket_eviction_keeps_recent_queries_and_removes_old_indices() {
        let program = typed_trees::TypedTrees::default();
        let judge = StructuralJudge::from_declaration_requires(
            &program,
            symbols::SymbolHandle::invalid(),
            &[],
            &[],
            true,
        );
        let calls = std::cell::Cell::new(0);
        let compute = |_: &StructuralJudge<'_>, term| {
            calls.set(calls.get() + 1);
            term
        };
        let term = |position| StructuralTerm::Variable(format!("question-{position}"));
        for position in 0..8192 {
            judge.completed_resolution(term(position), 0, compute);
        }
        judge.completed_resolution(term(0), 0, compute);
        judge.completed_resolution(term(8192), 0, compute);
        judge.completed_resolution(term(0), 0, compute);
        assert_eq!(calls.get(), 8193);
        judge.completed_resolution(term(1), 0, compute);
        assert_eq!(calls.get(), 8194);
        let cache = judge
            .completed_resolutions
            .as_ref()
            .unwrap()
            .lock()
            .unwrap();
        assert_eq!(cache.entries.len(), 8192);
        for (fingerprint, indices) in &cache.buckets {
            for &index in indices {
                assert_eq!(cache.entries[index].fingerprint, *fingerprint);
            }
        }
        assert_eq!(cache.buckets.values().map(Vec::len).sum::<usize>(), 8192);
    }

    #[test]
    fn cheap_leaf_queries_do_not_evict_completed_applications() {
        let program = typed_trees::TypedTrees::default();
        let judge = StructuralJudge::from_declaration_requires(
            &program,
            symbols::SymbolHandle::invalid(),
            &[],
            &[],
            true,
        );
        let expensive = StructuralTerm::Variable("retained application witness".to_owned());
        judge.completed_resolution(expensive.clone(), 0, |_, term| term);
        for _ in 0..1024 {
            judge.resolve(StructuralTerm::Variable("cheap leaf".to_owned()));
        }
        let entries = judge
            .completed_resolutions
            .as_ref()
            .unwrap()
            .lock()
            .unwrap();
        assert_eq!(entries.entries.len(), 1);
        assert!(entries.entries[0].term.same_resolution_payload(&expensive));
    }
}

#[cfg(test)]
mod predicate_tests {
    use super::*;
    #[test]
    fn value_resolution_and_predicate_answers_remain_distinct() {
        let program = typed_trees::TypedTrees::default();
        let judge = StructuralJudge::from_declaration_requires(
            &program,
            symbols::SymbolHandle::invalid(),
            &[],
            &[],
            true,
        );
        let question = StructuralTerm::Variable("question".to_owned());
        assert_eq!(
            judge.completed_resolution(question.clone(), 0, |_, term| term),
            question
        );
        assert!(matches!(
            judge.completed_predicate(question.clone(), 0, |_, _| StructuralJudgment::Refuted),
            StructuralJudgment::Refuted
        ));
        assert!(matches!(
            judge.completed_predicate(question.clone(), 0, |_, _| panic!(
                "completed predicate was recomputed"
            )),
            StructuralJudgment::Refuted
        ));
        assert_eq!(
            judge.completed_resolution(question.clone(), 0, |_, _| panic!(
                "value answer was displaced"
            )),
            question
        );
    }
}

#[cfg(test)]
mod depth_tests {
    use super::*;

    #[test]
    fn nested_arithmetic_queries_keep_the_active_resolution_budget() {
        let program = typed_trees::TypedTrees::default();
        let judge = StructuralJudge::from_declaration_requires(
            &program,
            symbols::SymbolHandle::invalid(),
            &[],
            &[],
            true,
        );
        let question = StructuralTerm::Variable("outer".to_owned());
        let nested = judge.resolving_application(&question, 31).unwrap();
        let inner = nested
            .resolving_application(&StructuralTerm::Variable("inner".to_owned()), 32)
            .unwrap();
        assert_eq!(judge.application_resolution_depth(), 0);
        assert_eq!(nested.clone().application_resolution_depth(), 31);
        assert_eq!(inner.application_resolution_depth(), 32);
        let mut root = judge.clone();
        Arc::make_mut(&mut root.substitutions).push((
            "value".to_owned(),
            StructuralTerm::Variable("resolved".to_owned()),
        ));
        let value = StructuralTerm::Variable("value".to_owned());
        assert_eq!(
            root.resolve(value.clone()),
            StructuralTerm::Variable("resolved".to_owned())
        );
        root.active_application = inner.active_application.clone();
        assert_eq!(root.resolve(value.clone()), value);
        assert_eq!(root.resolve_at(value.clone(), 0), value);
        assert!(nested.resolving_application(&question, 31).is_none());
    }
}

#[cfg(test)]
mod guard_depth_tests {
    use super::*;
    #[test]
    fn completed_guards_keep_the_active_resolution_depth() {
        let program = typed_trees::TypedTrees::default();
        let judge = StructuralJudge::from_declaration_requires(
            &program,
            symbols::SymbolHandle::invalid(),
            &[],
            &[],
            true,
        );
        let question = StructuralTerm::Variable("guard".to_owned());
        assert!(matches!(
            judge.completed_predicate(question.clone(), 0, |_, _| StructuralJudgment::Proven),
            StructuralJudgment::Proven
        ));
        let nested = judge
            .resolving_application(&StructuralTerm::Variable("active".to_owned()), 31)
            .unwrap();
        assert!(matches!(
            nested.completed_predicate(question, 0, |_, _| StructuralJudgment::Unknown),
            StructuralJudgment::Unknown
        ));
    }
}

#[cfg(test)]
mod term_mode_tests {
    use super::*;

    #[test]
    fn entering_a_question_preserves_its_caller_term_mode() {
        let program = typed_trees::TypedTrees::default();
        let mut judge = StructuralJudge::from_declaration_requires(
            &program,
            symbols::SymbolHandle::invalid(),
            &[],
            &[],
            true,
        );
        let question = StructuralTerm::Variable("question".to_owned());
        for mode in [false, true] {
            judge.specification_terms = mode;
            let entered = judge.resolving_application(&question, 0).unwrap();
            assert_eq!(entered.specification_terms, mode);
            assert!(entered.resolving_application(&question, 0).is_none());
            let mut other_mode = entered;
            other_mode.specification_terms = !mode;
            assert!(other_mode.resolving_application(&question, 0).is_some());
        }
    }

    fn read_specification_question(
        judge: &StructuralJudge<'_>,
        _: StructuralTerm,
    ) -> StructuralTerm {
        let mut specification = judge.clone();
        specification.specification_terms = true;
        StructuralTerm::Integer(numerics::bignum::BigInt::from_i64(i64::from(
            specification
                .resolving_application(&StructuralTerm::Variable("external".to_owned()), 0)
                .is_none(),
        )))
    }

    #[test]
    fn completed_reads_keep_the_observed_mode_across_caller_mode_changes() {
        let program = typed_trees::TypedTrees::default();
        let judge = StructuralJudge::from_declaration_requires(
            &program,
            symbols::SymbolHandle::invalid(),
            &[],
            &[],
            true,
        );
        let question = StructuralTerm::Variable("external".to_owned());
        let task = StructuralTerm::Variable("query".to_owned());
        let integer = |value| StructuralTerm::Integer(numerics::bignum::BigInt::from_i64(value));
        assert_eq!(
            judge.completed_resolution(task.clone(), 0, read_specification_question),
            integer(0)
        );
        let mut specification = judge.clone();
        specification.specification_terms = true;
        let mut active = specification.resolving_application(&question, 0).unwrap();
        active.specification_terms = false;
        assert_eq!(
            active.completed_resolution(task.clone(), 0, read_specification_question),
            integer(1)
        );
        let ordinary = judge.resolving_application(&question, 0).unwrap();
        assert_eq!(
            ordinary.completed_resolution(task.clone(), 0, read_specification_question),
            integer(0)
        );
        assert_eq!(
            active.completed_resolution(task, 0, read_specification_question),
            integer(1)
        );
    }

    #[test]
    fn cached_child_reads_replay_their_original_mode_into_the_parent() {
        let program = typed_trees::TypedTrees::default();
        let judge = StructuralJudge::from_declaration_requires(
            &program,
            symbols::SymbolHandle::invalid(),
            &[],
            &[],
            true,
        );
        let child = StructuralTerm::Variable("child".to_owned());
        let parent = StructuralTerm::Variable("parent".to_owned());
        let integer = |value| StructuralTerm::Integer(numerics::bignum::BigInt::from_i64(value));
        judge.completed_resolution(child.clone(), 0, read_specification_question);
        let compute_parent = |scope: &StructuralJudge<'_>, _| {
            scope.completed_resolution(child.clone(), 0, read_specification_question)
        };
        assert_eq!(
            judge.completed_resolution(parent.clone(), 0, compute_parent),
            integer(0)
        );
        let mut specification = judge.clone();
        specification.specification_terms = true;
        let mut active = specification
            .resolving_application(&StructuralTerm::Variable("external".to_owned()), 0)
            .unwrap();
        active.specification_terms = false;
        assert_eq!(
            active.completed_resolution(parent, 0, compute_parent),
            integer(1)
        );
    }
}
