/*
    Deriving claim outcomes: which claim each state result, named
    transition and call argument settles, and the rewrites that follow call
    result origins.

    Input outcomes forward the caller's held identity. Callee establishments
    instead name templates: each receiving occurrence keeps its locally
    allocated identity, while `joins` retains the complete call/return
    correspondence. A result binding is not new resource authority.
*/

mod outcome_tables {
    // A hypothesis changes each iteration. Prepare associations over its current
    // rows without caching source, absence or coverage judgments.
    use crate::checks::multiplicity::linear_obligations::CheckedClaimOutcomeEntry;
    use facts::PlaceSegment;
    use facts::places::PlaceBucketIndex;
    use std::hash::{Hash, Hasher};

    fn hash(value: impl Hash) -> u64 {
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        value.hash(&mut hash);
        hash.finish()
    }

    pub(super) fn unique_entries(
        rows: impl Iterator<Item = CheckedClaimOutcomeEntry>,
    ) -> Vec<CheckedClaimOutcomeEntry> {
        let rows = rows.collect::<Vec<_>>();
        let positions = PlaceBucketIndex::new(rows.iter().map(hash));
        let first = positions.group_equal_positions(|left, right| rows[left] == rows[right]);
        rows.into_iter()
            .enumerate()
            .filter_map(|(position, row)| (first[position] == position).then_some(row))
            .collect()
    }

    pub(super) struct Entries<'rows> {
        rows: &'rows [CheckedClaimOutcomeEntry],
        paths: PlaceBucketIndex,
        sources: PlaceBucketIndex,
    }

    impl<'rows> Entries<'rows> {
        pub(super) fn new(rows: &'rows [CheckedClaimOutcomeEntry]) -> Self {
            Self {
                rows,
                paths: PlaceBucketIndex::new(rows.iter().map(|row| hash(&row.output_path))),
                sources: PlaceBucketIndex::new(rows.iter().map(|row| hash(&row.source))),
            }
        }

        pub(super) fn rows(&self) -> &'rows [CheckedClaimOutcomeEntry] {
            self.rows
        }

        pub(super) fn at_path<'query>(
            &'query self,
            path: &'query [PlaceSegment],
        ) -> impl Iterator<Item = &'rows CheckedClaimOutcomeEntry> + 'query {
            self.paths
                .candidates(hash(path))
                .filter_map(move |position| {
                    let row = &self.rows[position];
                    (row.output_path == path).then_some(row)
                })
        }

        pub(super) fn contains(&self, row: &CheckedClaimOutcomeEntry) -> bool {
            self.at_path(&row.output_path)
                .any(|candidate| candidate == row)
        }

        pub(super) fn source_candidates_after(
            &self,
            position: usize,
        ) -> impl Iterator<Item = &'rows CheckedClaimOutcomeEntry> + '_ {
            self.sources
                .candidates(hash(&self.rows[position].source))
                .filter(move |other| *other > position)
                .map(|other| &self.rows[other])
        }
    }

    pub(super) struct InactivePaths<'paths> {
        rows: &'paths [Vec<PlaceSegment>],
        paths: PlaceBucketIndex,
    }

    impl<'paths> InactivePaths<'paths> {
        pub(super) fn new(rows: &'paths [Vec<PlaceSegment>]) -> Self {
            Self {
                rows,
                paths: PlaceBucketIndex::new(rows.iter().map(hash)),
            }
        }
        pub(super) fn rows(&self) -> &'paths [Vec<PlaceSegment>] {
            self.rows
        }
        pub(super) fn contains(&self, path: &[PlaceSegment]) -> bool {
            self.paths
                .candidates(hash(path))
                .any(|position| self.rows[position] == path)
        }
    }
    #[cfg(test)]
    mod tests {
        use super::{Entries, InactivePaths, hash, unique_entries};
        use crate::checks::multiplicity::linear_obligations::{
            CheckedClaimOutcomeEntry, CheckedClaimOutcomeSource,
        };
        use facts::{PlaceSegment, places::PlaceBucketIndex};
        use symbols::SymbolHandle;

        fn row(position: usize) -> CheckedClaimOutcomeEntry {
            CheckedClaimOutcomeEntry {
                output_path: vec![
                    PlaceSegment::Field {
                        symbol: SymbolHandle::from_parts(
                            (position % 19 + 1) as u32,
                            (position % 2 + 1) as u32,
                        ),
                    },
                    PlaceSegment::FixedIndex {
                        index: position % 11,
                    },
                ],
                source: CheckedClaimOutcomeSource::Input {
                    parameter_symbol: SymbolHandle::from_parts(
                        (position % 13 + 1) as u32,
                        (position % 3 + 1) as u32,
                    ),
                    path: vec![PlaceSegment::FixedIndex {
                        index: position % 7,
                    }],
                },
            }
        }

        #[test]
        fn outcome_row_associations_match_ordered_scans_and_rebuilds() {
            let mut rows = (0..512).map(row).collect::<Vec<_>>();
            rows.extend([rows[3].clone(), rows[0].clone()]);
            rows.push(CheckedClaimOutcomeEntry {
                output_path: rows[3].output_path.clone(),
                source: CheckedClaimOutcomeSource::Established {
                    claim_identity: language_semantics::PermissionClaimIdentity::Unknown,
                    provenance: language_semantics::PermissionProvenance::Unknown,
                },
            });
            let mut direct_unique = Vec::new();
            for row in &rows {
                if !direct_unique.contains(row) {
                    direct_unique.push(row.clone());
                }
            }
            assert_eq!(unique_entries(rows.clone().into_iter()), direct_unique);
            let entries = Entries::new(&rows);
            for (position, row) in rows.iter().enumerate() {
                assert_eq!(
                    entries.at_path(&row.output_path).collect::<Vec<_>>(),
                    rows.iter()
                        .filter(|other| other.output_path == row.output_path)
                        .collect::<Vec<_>>()
                );
                assert_eq!(
                    entries
                        .source_candidates_after(position)
                        .filter(|other| other.source == row.source)
                        .collect::<Vec<_>>(),
                    rows[position + 1..]
                        .iter()
                        .filter(|other| other.source == row.source)
                        .collect::<Vec<_>>()
                );
                assert!(entries.contains(row));
            }
            let requested = &rows[3];
            let collisions = Entries {
                rows: &rows,
                paths: PlaceBucketIndex::new(rows.iter().map(|_| hash(&requested.output_path))),
                sources: PlaceBucketIndex::new(rows.iter().map(|_| hash(&requested.source))),
            };
            assert_eq!(
                collisions
                    .at_path(&requested.output_path)
                    .collect::<Vec<_>>(),
                entries.at_path(&requested.output_path).collect::<Vec<_>>()
            );
            assert_eq!(
                collisions
                    .source_candidates_after(3)
                    .filter(|other| other.source == requested.source)
                    .collect::<Vec<_>>(),
                entries
                    .source_candidates_after(3)
                    .filter(|other| other.source == requested.source)
                    .collect::<Vec<_>>()
            );
            let paths = rows
                .iter()
                .map(|row| row.output_path.clone())
                .collect::<Vec<_>>();
            let inactive = InactivePaths::new(&paths);
            assert_eq!(inactive.rows(), paths);
            assert!(inactive.contains(&requested.output_path));
            assert!(!inactive.contains(&[PlaceSegment::FixedIndex { index: 9000 }]));
            let mut changed = rows.clone();
            changed.remove(3);
            changed[0].source = requested.source.clone();
            let rebuilt = Entries::new(&changed);
            assert_eq!(
                rebuilt.at_path(&requested.output_path).collect::<Vec<_>>(),
                changed
                    .iter()
                    .filter(|other| other.output_path == requested.output_path)
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                rebuilt
                    .source_candidates_after(0)
                    .filter(|other| other.source == changed[0].source)
                    .collect::<Vec<_>>(),
                changed[1..]
                    .iter()
                    .filter(|other| other.source == changed[0].source)
                    .collect::<Vec<_>>()
            );
        }

        #[test]
        fn outcome_path_batches_visit_only_matching_candidates() {
            let rows = (0..2048)
                .map(|index| CheckedClaimOutcomeEntry {
                    output_path: vec![PlaceSegment::FixedIndex { index }],
                    source: CheckedClaimOutcomeSource::Input {
                        parameter_symbol: SymbolHandle::from_parts(index as u32 + 1, 1),
                        path: vec![],
                    },
                })
                .collect::<Vec<_>>();
            let start = std::time::Instant::now();
            let entries = Entries::new(&rows);
            let preparation_ns = start.elapsed().as_nanos();
            let mut direct_rows = 0;
            let mut prepared_rows = 0;
            for row in &rows {
                let direct = rows
                    .iter()
                    .filter(|other| {
                        direct_rows += 1;
                        other.output_path == row.output_path
                    })
                    .collect::<Vec<_>>();
                prepared_rows += entries.paths.candidates(hash(&row.output_path)).count();
                assert_eq!(
                    entries.at_path(&row.output_path).collect::<Vec<_>>(),
                    direct
                );
            }
            assert_eq!(direct_rows, rows.len() * rows.len());
            assert_eq!(prepared_rows, rows.len());
            eprintln!(
                "outcome-path rows={} direct_rows={direct_rows} prepared_candidates={prepared_rows} preparation_ns={preparation_ns}",
                rows.len()
            );
        }
    }
}

mod joins {
    use super::{GlobalClaimEvents, OrderedOriginRewrites, PermissionEvents};
    /*
        Bind returned custody to an exact call occurrence, separately from lineage.
        A callee-local establishment needs this correspondence even with one exit:
        its static identity cannot identify two simultaneously live invocations.
        Input forwarding keeps the caller's held identity; differing exit sources
        retain every alternative rather than manufacturing an unconditional lineage.
    */
    use super::super::selected_evaluation::place_bindings::claim_origin;
    use super::{
        bind_claim_outcome_source_at_arguments, claim_paths_are_case_alternatives,
        derive_checked_claim_outcome_maps,
    };
    use crate::checks::multiplicity::ClaimConsumption;
    use crate::checks::multiplicity::ReturnedClaimSource;
    use crate::checks::multiplicity::ReturnedClaimSource::Live;
    use crate::checks::multiplicity::consumed_operand_claims;
    use crate::checks::multiplicity::current_returned_claims;
    use crate::checks::multiplicity::linear_obligations::{
        CheckedClaimOutcomeEntry, CheckedClaimOutcomeMap, CheckedClaimOutcomeSource,
    };
    use crate::shared::collections::push_unique;
    use checked_trees::{
        CheckFacts, FlowClaimJoinAlternative, FlowClaimJoinAlternativeSource, FlowClaimJoinExit,
        FlowClaimJoinExitKind, FlowClaimJoinReceipt, FlowClaimOutcomeSource, FlowOwnershipFacts,
        FlowPermissionEventFact,
    };
    use diagnostics::Diagnostic;
    use facts::{PlaceRoot, PlaceSegment};
    use language_semantics::{
        PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
        PermissionProvenance,
    };
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::statement::{StatementNode, TransitionExit, TransitionTargetNode};

    mod selected_storage {
        use super::PermissionEvents;
        /*
            Expand the complete storage frontier for one selected control decision.
            A destination name continues one of its exact arrivals; it is not a new
            lineage root. Substitute every affected field together and retain the edge
            so later result or storage expansion cannot combine mutually exclusive arms.
        */
        use super::{
            ExitFrontier, JoinedOccurrence, ReturnFrontierDiagnostics, expand_joined_frontier,
        };
        use crate::checks::multiplicity::linear_obligations::{
            CheckedClaimOutcomeMap, CheckedClaimOutcomeSource,
        };
        use crate::checks::multiplicity::owned_selection::terminal_arm_route;
        use crate::checks::ownership_occurrences::{EvaluationBranch, routes_compatible};
        use checked_trees::{FlowClaimJoinExit, FlowClaimJoinExitKind, FlowSelectionPlaceBinding};
        use symbols::SymbolHandle;

        pub(super) fn selection_routes_agree(
            program: &dyn validation::CheckingInput,
            exits: &[FlowClaimJoinExit],
            state: SymbolHandle,
            statement: usize,
            route: &[EvaluationBranch],
        ) -> Option<bool> {
            for exit in exits
                .iter()
                .filter(|exit| exit.state_symbol == state && exit.statement_index == statement)
            {
                let previous = match exit.kind {
                    FlowClaimJoinExitKind::SelectionArm { expression, arm } => {
                        terminal_arm_route(program, expression, arm)?
                    }
                    FlowClaimJoinExitKind::SelectionDecision {
                        expression,
                        decision,
                        selected,
                        ..
                    } => {
                        vec![EvaluationBranch::new(expression, decision, selected)]
                    }
                    _ => continue,
                };
                if !routes_compatible(route, &previous) {
                    return Some(false);
                }
            }
            Some(true)
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn expand_storage_frontier(
            program: &dyn validation::CheckingInput,
            facts: checked_trees::CheckFactsRef<'_>,
            state: &typed_trees::state::State,
            events: &PermissionEvents<'_>,
            maps: &[CheckedClaimOutcomeMap],
            active: &mut Vec<SymbolHandle>,
            visited: &[JoinedOccurrence],
            frontier: ExitFrontier,
            binding: &FlowSelectionPlaceBinding,
            diagnostics: &mut ReturnFrontierDiagnostics,
        ) -> Option<Vec<ExitFrontier>> {
            let ownership = &facts.flow.ownership;
            let occurrence = JoinedOccurrence::Storage {
                state: binding.state,
                statement: binding.statement_ordinal,
                evaluation: binding.evaluation,
                expression: binding.expression,
                decision: binding.decision,
            };
            if state.symbol != binding.state
                || visited.contains(&occurrence)
                || !crate::checks::multiplicity::selected_evaluation::place_bindings::claim_origin(
                    ownership,
                    binding.destination.claim_identity,
                    binding.destination.provenance,
                )
            {
                return None;
            }
            let rows = ownership
                .selection_place_bindings
                .iter()
                .filter_map(|(_, row)| {
                    (row.machine == binding.machine
                        && row.state == binding.state
                        && row.statement_ordinal == binding.statement_ordinal
                        && row.evaluation == binding.evaluation
                        && row.expression == binding.expression
                        && row.decision == binding.decision)
                        .then_some(row)
                })
                .collect::<Vec<_>>();
            let mut visited = visited.to_vec();
            visited.push(occurrence);
            let mut expanded = Vec::new();
            for selected in [false, true] {
                let route = [EvaluationBranch::new(
                    binding.expression,
                    binding.decision,
                    selected,
                )];
                if !selection_routes_agree(
                    program,
                    &frontier.exits,
                    binding.state,
                    binding.statement_ordinal,
                    &route,
                )? {
                    continue;
                }
                let mut next = frontier.clone();
                next.exits.push(FlowClaimJoinExit {
                    state_symbol: binding.state,
                    statement_index: binding.statement_ordinal,
                    kind: FlowClaimJoinExitKind::SelectionDecision {
                        evaluation: binding.evaluation,
                        expression: binding.expression,
                        decision: binding.decision,
                        selected,
                    },
                });
                for entry in &mut next.entries {
                    let CheckedClaimOutcomeSource::Established {
                        claim_identity,
                        provenance,
                    } = entry.source
                    else {
                        continue;
                    };
                    let mut candidates = rows.iter().filter(|row| {
                        row.selected != row.remaining
                            && row.destination.claim_identity == claim_identity
                            && row.destination.provenance == provenance
                    });
                    let Some(row) = candidates.next() else {
                        continue;
                    };
                    if candidates.next().is_some() {
                        return None;
                    }
                    let arrival = if selected {
                        row.selected
                    } else {
                        row.remaining
                    };
                    entry.source = super::super::claim_outcome_source_for_identity(
                        program,
                        state,
                        arrival.claim_identity,
                        arrival.provenance,
                        &ownership.segments,
                        events,
                    );
                }
                expanded.extend(expand_joined_frontier(
                    program,
                    facts,
                    state,
                    events,
                    maps,
                    active,
                    &visited,
                    next,
                    diagnostics,
                )?);
            }
            (!expanded.is_empty()).then_some(expanded)
        }
    }
    use selected_storage::expand_storage_frontier;

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct ExitFrontier {
        exits: Vec<FlowClaimJoinExit>,
        entries: Vec<CheckedClaimOutcomeEntry>,
        inactive_paths: Vec<Vec<PlaceSegment>>,
        path: Vec<PlaceSegment>,
        captures: Vec<Capture>,
        excluded_input: Option<ExcludedInput>,
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum JoinedOccurrence {
        Storage {
            state: SymbolHandle,
            statement: usize,
            evaluation: checked_trees::FlowPermissionEvaluation,
            expression: ExpressionHandle,
            decision: usize,
        },
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Alternative {
        exits: Vec<FlowClaimJoinExit>,
        captures: Vec<Capture>,
        source: Option<(CheckedClaimOutcomeSource, CheckedClaimOutcomeSource)>,
        excluded_input: Option<ExcludedInput>,
    }

    /*
        Retain the existing completed return graph as the guard authority. A
        path query never creates a fresh choice per output field: all projections
        refer to the same capture and its actual returned identity. Nested call
        exits namespace these captures at their exact invocation.
    */
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Capture {
        operand: arena::Handle<checked_trees::FlowOperandFact>,
        path: Vec<PlaceSegment>,
        identity: PermissionClaimIdentity,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct ExcludedInput {
        source: CheckedClaimOutcomeSource,
        operand: arena::Handle<checked_trees::FlowOperandFact>,
        path: Vec<PlaceSegment>,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Receipt {
        machine: SymbolHandle,
        state: SymbolHandle,
        statement: usize,
        ordinal: usize,
        target: SymbolHandle,
        expression: ExpressionHandle,
        root: PlaceRoot,
        path: Vec<PlaceSegment>,
        identity: PermissionClaimIdentity,
        alternatives: Vec<Alternative>,
    }

    /*
        Diagnostic observations stay outside every permission and receipt. Recording
        them does not turn a failed candidate into a new checking failure or a join.
    */
    #[derive(Clone)]
    struct ReturnFrontierFailure {
        exit: FlowClaimJoinExit,
        reason: &'static str,
        source: Option<CheckedClaimOutcomeSource>,
    }

    #[derive(Default)]
    struct ReturnFrontierDiagnostics {
        enabled: bool,
        failures: Vec<ReturnFrontierFailure>,
    }

    impl ReturnFrontierDiagnostics {
        fn record(&mut self, exit: FlowClaimJoinExit, reason: &'static str) {
            if self.enabled {
                self.failures.push(ReturnFrontierFailure {
                    exit,
                    reason,
                    source: None,
                });
            }
        }
    }

    struct ConsumedClaimPath {
        path: Vec<PlaceSegment>,
        source: ReturnedClaimSource,
    }

    #[derive(Clone, Copy, PartialEq, Eq, Hash)]
    enum ConsumedCapture {
        Outgoing(arena::Handle<checked_trees::FlowStateOutgoingFact>),
        Argument {
            consumer: ClaimConsumption,
            operand: arena::Handle<checked_trees::FlowOperandFact>,
        },
    }

    // Different fields can ask about the same consumed value. Check the whole
    // value once in this unchanged receipt pass, then select its fields. Keep
    // conditions inside their own arena; source binding and expansion still run
    // for each request.
    struct ConsumedCaptureReports<'snapshot> {
        program: &'snapshot dyn validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'snapshot>,
        captures: symbols::SymbolKeyMap<ConsumedCapture, Vec<ConsumedClaimPath>>,
    }

    impl<'snapshot> ConsumedCaptureReports<'snapshot> {
        fn new(
            program: &'snapshot dyn validation::CheckingInput,
            facts: checked_trees::CheckFactsRef<'snapshot>,
        ) -> Self {
            Self {
                program,
                facts,
                captures: symbols::SymbolKeyMap::default(),
            }
        }

        fn paths(
            &mut self,
            capture: ConsumedCapture,
        ) -> Result<&[ConsumedClaimPath], &'static str> {
            use std::collections::hash_map::Entry;
            match self.captures.entry(capture) {
                Entry::Occupied(entry) => Ok(entry.into_mut().as_slice()),
                Entry::Vacant(entry) => {
                    let mut conditions = crate::flow::SelectionConditions::new();
                    let claims = match capture {
                        ConsumedCapture::Outgoing(outgoing) => current_returned_claims(
                            self.program,
                            self.facts,
                            self.facts.flow.control.outgoing.get(outgoing),
                            &mut conditions,
                        ),
                        ConsumedCapture::Argument { consumer, operand } => consumed_operand_claims(
                            self.program,
                            self.facts,
                            consumer,
                            operand,
                            &mut conditions,
                        ),
                    }?;
                    Ok(entry
                        .insert(
                            claims
                                .into_iter()
                                .map(|claim| ConsumedClaimPath {
                                    path: claim.path,
                                    source: claim.source,
                                })
                                .collect(),
                        )
                        .as_slice())
                }
            }
        }
    }

    struct PreparedCalleeFrontier {
        state: SymbolHandle,
        path: Vec<PlaceSegment>,
        frontiers: Option<Vec<ExitFrontier>>,
        diagnostics: ReturnFrontierDiagnostics,
    }

    pub(crate) struct CallClaimJoinFailure {
        receiver: ReceivingClaim,
        statement: usize,
        ordinal: usize,
        target: SymbolHandle,
        reason: &'static str,
        returns: Vec<ReturnFrontierFailure>,
    }

    impl CallClaimJoinFailure {
        /* An unresolved receiving event can report only its own attempted join. */
        pub(crate) fn detail_for(
            &self,
            program: &dyn validation::CheckingInput,
            event: &FlowPermissionEventFact,
            expression: ExpressionHandle,
            segments: &arena::PagedArenaSet<PlaceSegment>,
        ) -> Option<String> {
            let receiver = &self.receiver;
            if receiver.machine != event.machine_symbol
                || receiver.state != event.state_symbol
                || receiver.source != event.source
                || receiver.expression != expression
                || receiver.root != event.root
                || receiver.identity != event.claim_identity
                || receiver.path.as_slice() != segments.span_or_empty(event.segments)
            {
                return None;
            }
            let mut detail = format!(
                "join candidate in machine `{}` state `{}` statement {} call {} to `{}`: {}",
                program.symbols.name(receiver.machine),
                program.symbols.name(receiver.state),
                self.statement,
                self.ordinal,
                program.symbols.name(self.target),
                self.reason,
            );
            for failure in &self.returns {
                detail.push_str(&format!(
                    "; return state `{}` statement {} {:?}: {}",
                    program.symbols.name(failure.exit.state_symbol),
                    failure.exit.statement_index,
                    failure.exit.kind,
                    failure.reason,
                ));
                if let Some(source) = &failure.source {
                    match source {
                        CheckedClaimOutcomeSource::Input {
                            parameter_symbol,
                            path,
                        } => {
                            let source = validation::CanonicalPlace {
                                root: PlaceRoot::Symbol(*parameter_symbol),
                                segments: path.clone(),
                            };
                            detail
                                .push_str(&format!(" (callee input `{}`)", source.label(program)));
                        }
                        CheckedClaimOutcomeSource::Established {
                            claim_identity,
                            provenance,
                        } => {
                            detail.push_str(&format!(
                                " (callee establishment {claim_identity:?}/{provenance:?})",
                            ));
                        }
                    }
                }
            }
            Some(detail)
        }
    }

    /*
        Preparation copies semantic paths, never handles into an arena that the
        next source walk rebuilds. The receiving identity alone is insufficient:
        its ordinal can change when an earlier establishment becomes inactive.
    */
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct ReceivingClaim {
        machine: SymbolHandle,
        state: SymbolHandle,
        source: PermissionEventSource,
        expression: ExpressionHandle,
        root: PlaceRoot,
        path: Vec<PlaceSegment>,
        identity: PermissionClaimIdentity,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub(crate) struct PreparedCallClaimJoin {
        receiver: ReceivingClaim,
        provenance: PermissionProvenance,
    }

    /*
        A previous iteration supplies a hypothesis for this exact completion, not
        authority. Publication rederives every receipt from the current source walk;
        changed or missing consulted bindings force another walk before its custody
        observations and cleanup decisions may escape preparation.
    */
    #[derive(Clone, Default)]
    pub(super) struct CallJoinPreparation {
        bindings: Vec<PreparedCallClaimJoin>,
        dependencies: Vec<ReceivingClaim>,
    }

    impl CallJoinPreparation {
        pub(super) fn begin(&mut self) {
            self.dependencies.clear();
        }

        /*
            A copy for one worker's run of states: the previous iteration's
            bindings, and no receivers consulted yet.
        */
        pub(super) fn fork_for_walk(&self) -> Self {
            Self {
                bindings: self.bindings.clone(),
                dependencies: Vec::new(),
            }
        }

        /*
            Hand over the receivers consulted since the last call, keeping the
            bindings.
        */
        pub(super) fn take_consulted(&mut self) -> Self {
            Self {
                bindings: Vec::new(),
                dependencies: std::mem::take(&mut self.dependencies),
            }
        }

        /*
            Whether each receiver `consulted` looked up has the same binding here
            as under `previous`.
        */
        pub(super) fn receivers_unchanged(&self, previous: &Self, consulted: &Self) -> bool {
            consulted.dependencies.iter().all(|receiver| {
                self.bindings
                    .iter()
                    .find(|binding| binding.receiver == *receiver)
                    == previous
                        .bindings
                        .iter()
                        .find(|binding| binding.receiver == *receiver)
            })
        }

        /*
            Record the receivers a worker consulted after these, keeping the
            first-consulted order a single walk over the same states would have.
        */
        pub(super) fn absorb_walk(&mut self, fork: Self) {
            for receiver in fork.dependencies {
                push_unique(&mut self.dependencies, receiver);
            }
        }

        pub(super) fn update(&mut self, bindings: Vec<PreparedCallClaimJoin>) -> bool {
            let changed = self.dependencies.iter().any(|receiver| {
                self.bindings
                    .iter()
                    .find(|binding| binding.receiver == *receiver)
                    != bindings
                        .iter()
                        .find(|binding| binding.receiver == *receiver)
            });
            self.bindings = bindings;
            changed
        }

        pub(super) fn resolve(
            &mut self,
            segments: &arena::PagedArenaSet<PlaceSegment>,
            expression: ExpressionHandle,
            event: &mut FlowPermissionEventFact,
        ) {
            if event.kind != PermissionEventKind::Establish
                || event.access != PermissionAccess::Owned
                || !event.obligation_live
                || event.provenance
                    != super::established_provenance(
                        event.machine_symbol,
                        event.state_symbol,
                        event.source,
                    )
                || !matches!(event.claim_identity,
                    PermissionClaimIdentity::Established { machine_symbol, state_symbol, source, .. }
                    if machine_symbol == event.machine_symbol && state_symbol == event.state_symbol
                        && source == event.source)
            {
                return;
            }
            let receiver = ReceivingClaim {
                machine: event.machine_symbol,
                state: event.state_symbol,
                source: event.source,
                expression,
                root: event.root,
                path: segments.span_or_empty(event.segments).to_vec(),
                identity: event.claim_identity,
            };
            if let Some(binding) = self
                .bindings
                .iter()
                .find(|binding| binding.receiver == receiver)
            {
                event.provenance = binding.provenance;
            }
            push_unique(&mut self.dependencies, receiver);
        }
    }

    fn known_source(source: &CheckedClaimOutcomeSource) -> bool {
        match source {
            CheckedClaimOutcomeSource::Input {
                parameter_symbol,
                path,
            } => {
                parameter_symbol.is_valid() && path.iter().all(|segment| {
                    matches!(segment,
                    PlaceSegment::Field { symbol } if symbol.is_valid())
                        || matches!(segment, PlaceSegment::Case { variant } if variant.is_valid())
                        || matches!(segment, PlaceSegment::FixedIndex { .. })
                })
            }
            CheckedClaimOutcomeSource::Established {
                claim_identity,
                provenance,
            } => {
                *claim_identity != PermissionClaimIdentity::Unknown
                    && *provenance != PermissionProvenance::Unknown
            }
        }
    }

    /*
        No exit may disappear through filter_map. Missing facts and unknown sources
        make the complete correspondence unavailable. Completed call results stay
        local edges; only named jumps without a returned capture expand here.
    */
    #[allow(clippy::too_many_arguments)]
    fn state_frontiers(
        program: &dyn ::validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        state: &typed_trees::state::State,
        events: &PermissionEvents<'_>,
        maps: &[CheckedClaimOutcomeMap],
        active: &mut Vec<SymbolHandle>,
        path: &[PlaceSegment],
        diagnostics: &mut ReturnFrontierDiagnostics,
        consumed_captures: &mut ConsumedCaptureReports<'_>,
    ) -> Option<Vec<ExitFrontier>> {
        if active.contains(&state.symbol) {
            return None;
        }
        active.push(state.symbol);
        let result = (|| {
            let ownership = &facts.flow.ownership;
            let flow = facts.flow.control.state_by_symbol(state.symbol)?;
            let mut frontiers = Vec::new();
            for (outgoing_handle, outgoing) in flow
                .outgoing
                .handles()
                .zip(facts.flow.control.outgoing.span_or_empty(flow.outgoing))
            {
                if outgoing.cannot_return_normally {
                    continue;
                }
                let (kind, target) = if outgoing.transition_target.is_valid() {
                    let StatementNode::Transition(transition) = program
                        .statement_table
                        .statement_at(state, outgoing.statement_index)?
                    else {
                        return None;
                    };
                    if transition.exit != TransitionExit::Ordinary {
                        continue;
                    }
                    let kind = if transition.target == outgoing.transition_target {
                        FlowClaimJoinExitKind::TransitionTarget
                    } else if transition.continuation == outgoing.transition_target {
                        FlowClaimJoinExitKind::TransitionContinuation
                    } else {
                        return None;
                    };
                    (
                        kind,
                        Some(
                            program
                                .statement_table
                                .transition_target(outgoing.transition_target),
                        ),
                    )
                } else {
                    (FlowClaimJoinExitKind::FinalExpression, None)
                };
                let edge = FlowClaimJoinExit {
                    state_symbol: state.symbol,
                    statement_index: outgoing.statement_index,
                    kind,
                };
                if let Some(TransitionTargetNode::Named {
                    path: target,
                    arguments,
                    ..
                }) = target
                {
                    let target = validation::call_target_state(program, target.symbol)?;
                    let children = state_frontiers(
                        program,
                        facts,
                        target,
                        events,
                        maps,
                        active,
                        path,
                        diagnostics,
                        consumed_captures,
                    )?;
                    for mut child in children {
                        for entry in &mut child.entries {
                            let Some(source) = bind_claim_outcome_source_at_arguments(
                                program,
                                state,
                                outgoing.statement_index,
                                program.statement_table.expression_handles(*arguments),
                                target,
                                &entry.source,
                                &ownership.segments,
                                events,
                                &mut super::ClaimOutcomeMapLookup::unrecorded(maps),
                            ) else {
                                diagnostics.record(
                                    edge,
                                    "named transition cannot bind its returned input source",
                                );
                                return None;
                            };
                            entry.source = source;
                        }
                        child.exits.insert(0, edge);
                        frontiers.push(child);
                    }
                    continue;
                }
                let claims =
                    match consumed_captures.paths(ConsumedCapture::Outgoing(outgoing_handle)) {
                        Ok(claims) => claims,
                        Err(reason) => {
                            diagnostics.record(edge, reason);
                            return None;
                        }
                    };
                let mut found = false;
                for claim in claims.iter().filter(|claim| claim.path == path) {
                    found = true;
                    let (entries, inactive_paths, identity) = match claim.source {
                        ReturnedClaimSource::Inactive => (
                            Vec::new(),
                            vec![path.to_vec()],
                            PermissionClaimIdentity::Unknown,
                        ),
                        Live(identity) => {
                            let Some(source) =
                                returned_identity_source(state, identity, ownership, events)
                            else {
                                diagnostics.record(
                                    edge,
                                    "returned live claim has no unique entry or establishment source",
                                );
                                return None;
                            };
                            (
                                vec![CheckedClaimOutcomeEntry {
                                    output_path: path.to_vec(),
                                    source,
                                }],
                                Vec::new(),
                                identity,
                            )
                        }
                    };
                    let frontier = ExitFrontier {
                        exits: vec![edge],
                        entries,
                        inactive_paths,
                        path: path.to_vec(),
                        captures: vec![Capture {
                            operand: outgoing.returned_value,
                            path: path.to_vec(),
                            identity,
                        }],
                        excluded_input: None,
                    };
                    if !complete_frontier(
                        program,
                        state.return_type,
                        path,
                        &frontier.entries,
                        &frontier.inactive_paths,
                    ) {
                        diagnostics.record(edge, "returned claim path has no complete frontier");
                        return None;
                    }
                    let failure_count = diagnostics.failures.len();
                    let Some(expanded) = expand_joined_frontier(
                        program,
                        facts,
                        state,
                        events,
                        maps,
                        active,
                        &[],
                        frontier,
                        diagnostics,
                    ) else {
                        if diagnostics.failures.len() == failure_count {
                            diagnostics
                                .record(edge, "returned claim source expansion is unavailable");
                        }
                        return None;
                    };
                    frontiers.extend(expanded);
                }
                if !found {
                    diagnostics.record(
                        edge,
                        "completed return omits the requested result claim path",
                    );
                    return None;
                }
            }
            (!frontiers.is_empty()).then_some(frontiers)
        })();
        active.pop();
        result
    }

    fn returned_identity_source(
        state: &typed_trees::state::State,
        identity: PermissionClaimIdentity,
        ownership: &FlowOwnershipFacts,
        events: &PermissionEvents<'_>,
    ) -> Option<CheckedClaimOutcomeSource> {
        let definitions = events
            .at_claim_identity(state.symbol, identity)
            .filter(|event| {
                event.state_symbol == state.symbol
                    && matches!(
                        event.kind,
                        PermissionEventKind::Establish | PermissionEventKind::Restore
                    )
                    && event.access == PermissionAccess::Owned
                    && event.obligation_live
                    && event.claim_identity == identity
            })
            .collect::<Vec<_>>();
        let entries = definitions
            .iter()
            .filter(|event| {
                event.kind == PermissionEventKind::Establish
                    && event.source == PermissionEventSource::StateEntry
            })
            .collect::<Vec<_>>();
        if let [entry] = entries.as_slice() {
            return Some(CheckedClaimOutcomeSource::Input {
                parameter_symbol: entry.root.as_symbol()?,
                path: ownership.segments.span(entry.segments)?.to_vec(),
            });
        }
        if !entries.is_empty() {
            return None;
        }
        let first = definitions.first()?;
        if definitions
            .iter()
            .any(|event| event.provenance != first.provenance)
        {
            return None;
        }
        let source = CheckedClaimOutcomeSource::Established {
            claim_identity: identity,
            provenance: first.provenance,
        };
        known_source(&source).then_some(source)
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct BoundSource {
        source: Option<CheckedClaimOutcomeSource>,
        captures: Vec<Capture>,
        excluded_input: Option<ExcludedInput>,
    }

    /*
        Bind one callee input path through the actual completed argument, retaining
        its original selection graph. Every argument path is checked completely
        before projection; missing alternatives cannot become a convenient source.
    */
    #[allow(clippy::too_many_arguments)]
    fn bound_sources_at_call(
        program: &dyn validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        machine: SymbolHandle,
        state: &typed_trees::state::State,
        statement: usize,
        ordinal: usize,
        expression: ExpressionHandle,
        call: &typed_trees::expression::TableCallExpression,
        target: &typed_trees::state::State,
        source: &CheckedClaimOutcomeSource,
        events: &PermissionEvents<'_>,
        consumed_captures: &mut ConsumedCaptureReports<'_>,
    ) -> Result<Vec<BoundSource>, &'static str> {
        let CheckedClaimOutcomeSource::Input {
            parameter_symbol,
            path,
        } = source
        else {
            return Ok(vec![BoundSource {
                source: Some(source.clone()),
                captures: Vec::new(),
                excluded_input: None,
            }]);
        };
        let flow = facts
            .flow
            .control
            .state(machine, state.symbol)
            .ok_or("caller argument binding has no owning state flow")?;
        let entered_target = validation::call_target_entry_symbol(
            program,
            validation::call_target_symbol(program, call),
        );
        let mut matching = crate::flow::call_flows(facts.flow.control, flow.calls)
            .iter()
            .filter(|row| {
                row.statement_index == statement
                    && row.call_ordinal == ordinal
                    && row.target_symbol == entered_target
                    && row.authored_expression == expression
            });
        let captured = matching
            .next()
            .ok_or("caller argument binding has no exact captured invocation")?;
        if matching.next().is_some() {
            return Err("caller argument binding has multiple captured invocations");
        }
        let parameters = program.state_parameters(target);
        let arguments = program.expression_table.expression_handles(call.arguments);
        let operands = facts
            .flow
            .control
            .operands
            .span(captured.argument_operands())
            .ok_or("caller argument binding has an invalid completed operand span")?;
        if parameters.len() != arguments.len()
            || !operands
                .iter()
                .map(|operand| operand.expression)
                .eq(arguments.iter().copied())
        {
            return Err("caller argument captures disagree with the exact callee parameter roster");
        }
        let position =
            typed_trees::signature::state_parameter_position(parameters, *parameter_symbol)
                .ok_or("returned input source is not a parameter of the captured call target")?;
        let operand = captured.argument_operands().handle_at(
            u32::try_from(position)
                .map_err(|_| "caller argument position exceeds arena coordinates")?,
        );
        let machine_owner = program
            .machine_by_symbol(machine)
            .ok_or("caller argument binding has no owning machine")?;
        let evaluation = crate::flow::call_permission_evaluation(
            program,
            machine_owner,
            state,
            statement,
            ordinal,
        );
        if evaluation == checked_trees::FlowPermissionEvaluation::Unknown {
            return Err("caller argument binding has no exact permission evaluation");
        }
        let consumer = ClaimConsumption {
            machine,
            state: state.symbol,
            statement_index: statement,
            evaluation,
            source: PermissionEventSource::Call {
                statement_index: statement,
                call_ordinal: ordinal,
                target_symbol: captured.target_symbol,
            },
        };
        let claims = consumed_captures.paths(ConsumedCapture::Argument { consumer, operand })?;
        let mut sources = Vec::new();
        for claim in claims.iter().filter(|claim| claim.path == *path) {
            let (source, identity) = match claim.source {
                Live(identity) => (
                    Some(
                        returned_identity_source(state, identity, facts.flow.ownership, events).ok_or(
                            "consumed caller argument has no unique entry or establishment source",
                        )?,
                    ),
                    identity,
                ),
                ReturnedClaimSource::Inactive => (None, PermissionClaimIdentity::Unknown),
            };
            let excluded_input = source.is_none().then(|| ExcludedInput {
                source: CheckedClaimOutcomeSource::Input {
                    parameter_symbol: *parameter_symbol,
                    path: path.clone(),
                },
                operand,
                path: path.clone(),
            });
            let bound = BoundSource {
                source,
                excluded_input,
                captures: vec![Capture {
                    operand,
                    path: path.clone(),
                    identity,
                }],
            };
            push_unique(&mut sources, bound);
        }
        if sources.is_empty() {
            Err("completed caller argument omits the exact returned input-source path")
        } else {
            Ok(sources)
        }
    }

    /*
        Expand local storage choices while preserving completed calls as edges.
        A receiving identity is local to its invocation, never a global resource id.
    */
    #[allow(clippy::too_many_arguments)]
    fn expand_joined_frontier(
        program: &dyn ::validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        state: &typed_trees::state::State,
        events: &PermissionEvents<'_>,
        maps: &[CheckedClaimOutcomeMap],
        active: &mut Vec<SymbolHandle>,
        visited: &[JoinedOccurrence],
        frontier: ExitFrontier,
        diagnostics: &mut ReturnFrontierDiagnostics,
    ) -> Option<Vec<ExitFrontier>> {
        let ownership = &facts.flow.ownership;
        if let Some(binding) = frontier.entries.iter().find_map(|entry| {
            let CheckedClaimOutcomeSource::Established {
                claim_identity,
                provenance,
            } = entry.source
            else {
                return None;
            };
            ownership
                .selection_place_bindings
                .iter()
                .find_map(|(_, binding)| {
                    (binding.selected != binding.remaining
                        && binding.destination.claim_identity == claim_identity
                        && binding.destination.provenance == provenance)
                        .then_some(binding)
                })
        }) {
            return expand_storage_frontier(
                program,
                facts,
                state,
                events,
                maps,
                active,
                visited,
                frontier.clone(),
                binding,
                diagnostics,
            );
        }
        if frontier.entries.iter().any(|entry| {
            matches!(
                entry.source,
                CheckedClaimOutcomeSource::Established {
                    provenance: PermissionProvenance::Joined {
                        source: PermissionEventSource::Statement { .. },
                        ..
                    },
                    ..
                }
            )
        }) {
            return None;
        }
        Some(vec![frontier])
    }

    pub(super) fn complete_frontier(
        program: &dyn ::validation::CheckingInput,
        result_type: typed_trees::types::TypeReferenceHandle,
        requested: &[PlaceSegment],
        entries: &[CheckedClaimOutcomeEntry],
        inactive_paths: &[Vec<PlaceSegment>],
    ) -> bool {
        validate_frontier(
            program,
            result_type,
            Some(requested),
            entries,
            inactive_paths,
        )
    }

    fn validate_frontier(
        program: &dyn ::validation::CheckingInput,
        result_type: typed_trees::types::TypeReferenceHandle,
        requested: Option<&[PlaceSegment]>,
        entries: &[CheckedClaimOutcomeEntry],
        inactive_paths: &[Vec<PlaceSegment>],
    ) -> bool {
        let declared = validation::declared_claim_frontier(program, result_type);
        complete_declared_frontier(
            &declared,
            requested,
            &super::outcome_tables::Entries::new(entries),
            &super::outcome_tables::InactivePaths::new(inactive_paths),
        )
    }

    pub(super) fn complete_declared_frontier(
        declared: &validation::DeclaredClaimFrontier,
        requested: Option<&[PlaceSegment]>,
        entries: &super::outcome_tables::Entries<'_>,
        inactive_paths: &super::outcome_tables::InactivePaths<'_>,
    ) -> bool {
        let expected = |path: &[PlaceSegment]| {
            requested.is_none_or(|requested| path == requested)
                && declared.positions(path).next().is_some()
        };
        if requested.map_or_else(
            || declared.claims().is_empty(),
            |path| declared.positions(path).next().is_none(),
        ) || entries.rows().iter().any(|entry| {
            !known_source(&entry.source)
                || !expected(&entry.output_path)
                || inactive_paths.contains(&entry.output_path)
        }) || inactive_paths.rows().iter().any(|path| !expected(path))
        {
            return false;
        }
        let complete = |claim: &validation::ClaimFrontierClaim| {
            let matching = entries.at_path(&claim.path).count();
            matching == 1 || (matching == 0 && inactive_paths.contains(&claim.path))
        };
        let complete = match requested {
            Some(path) => declared
                .positions(path)
                .all(|position| complete(&declared.claims()[position])),
            None => declared.claims().iter().all(complete),
        };
        if !complete {
            return false;
        }
        !entries.rows().iter().enumerate().any(|(index, entry)| {
            entries.source_candidates_after(index).any(|other| {
                entry.source == other.source
                    && !claim_paths_are_case_alternatives(&entry.output_path, &other.output_path)
            })
        })
    }

    /*
        Check an absent field against the same state records and statement effects
        throughout one complete value check. Recursive children share this owner;
        a later check starts from its own immutable flow and borrow snapshot.
    */
    pub(in crate::checks::multiplicity) struct InactiveClaimQueries<'snapshot> {
        program: &'snapshot dyn ::validation::CheckingInput,
        borrow: &'snapshot checked_trees::BorrowFacts,
        ownership: &'snapshot FlowOwnershipFacts,
        state: SymbolHandle,
        prepared: std::cell::OnceCell<Option<InactiveStateQueries<'snapshot>>>,
    }

    struct InactiveStateQueries<'snapshot> {
        writes: InactivePlaceWrites,
        events: super::PermissionEvents<'snapshot>,
    }

    impl<'snapshot> InactiveClaimQueries<'snapshot> {
        pub(in crate::checks::multiplicity) fn new(
            program: &'snapshot dyn ::validation::CheckingInput,
            facts: checked_trees::CheckFactsRef<'snapshot>,
            state: SymbolHandle,
        ) -> Self {
            Self {
                program,
                borrow: facts.borrow,
                ownership: facts.flow.ownership,
                state,
                prepared: std::cell::OnceCell::new(),
            }
        }

        pub(in crate::checks::multiplicity) fn preserved(
            &mut self,
            state: &typed_trees::state::State,
            statement_index: usize,
            place: &validation::CanonicalPlace,
        ) -> Option<usize> {
            if state.symbol != self.state
                || !place
                    .segments
                    .iter()
                    .any(|segment| matches!(segment, PlaceSegment::Case { .. }))
            {
                return None;
            }
            let prepared = self
                .prepared
                .get_or_init(|| {
                    let writes = InactivePlaceWrites::prepare(self.program, self.borrow, state)?;
                    // Filtering the original arena keeps its row order, including the
                    // last occurrence when two establishments have the same position.
                    let rows = self
                        .ownership
                        .permissions
                        .iter()
                        .filter(|(_, event)| event.state_symbol == state.symbol)
                        .map(|(_, event)| event.clone())
                        .collect();
                    Some(InactiveStateQueries {
                        writes,
                        events: super::PermissionEvents::owned(rows, &self.ownership.segments),
                    })
                })
                .as_ref()?;
            prepared.writes.preserved(
                self.program,
                state,
                statement_index,
                place,
                self.ownership,
                prepared.events.at_place_across_statements(
                    state.symbol,
                    place.root,
                    &place.segments,
                ),
            )
        }
    }

    /*
        Statement effects belong to the immutable source/borrow inputs, not the
        individual conditional claim. Reconstruct them once for a frontier batch.
        Unknown effects stay unknown at their original statement; a query still
        checks exactly the interval after its own establishing occurrence.
    */
    pub(super) struct InactivePlaceWrites {
        state: SymbolHandle,
        statements: Vec<Option<Vec<validation::CanonicalPlace>>>,
    }

    impl InactivePlaceWrites {
        pub(super) fn prepare(
            program: &dyn ::validation::CheckingInput,
            borrow: &checked_trees::BorrowFacts,
            state: &typed_trees::state::State,
        ) -> Option<Self> {
            let (machine, _) = validation::find_state(program, state.symbol)?;
            let frames = validation::CallFrameResolver::new(program)?;
            let (_, borrowed_state) = borrow.first_state_borrow(machine.symbol, state.symbol)?;
            let statements = crate::flow::state_statements(program, state.statement_nodes)
                .iter()
                .enumerate()
                .map(|(index, statement)| {
                    let mut writes = crate::flow::statement_storage_writes(
                        program,
                        machine.symbol,
                        state.symbol,
                        index,
                        statement,
                        Some(&frames),
                    )?;
                    writes.extend(crate::flow::frame_storage_writes(
                        program,
                        machine.symbol,
                        state.symbol,
                        index,
                        &frames.statement_value_write_frame(machine, statement),
                        Some(&frames),
                    )?);
                    if let Some(call) = statement.as_call() {
                        writes.extend(crate::flow::frame_storage_writes(
                            program,
                            machine.symbol,
                            state.symbol,
                            index,
                            &frames.may_write_frame(machine, call),
                            Some(&frames),
                        )?);
                    }
                    for call in borrow
                        .calls
                        .span_or_empty(borrowed_state.calls)
                        .iter()
                        .filter(|call| call.statement_index == index)
                    {
                        writes.extend(crate::flow::signature_ceiling_places(
                            program,
                            machine.symbol,
                            state.symbol,
                            None,
                            call,
                            Some(&frames),
                        )?);
                    }
                    Some(writes)
                })
                .collect();
            Some(Self {
                state: state.symbol,
                statements,
            })
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn preserved<'a>(
            &self,
            program: &dyn ::validation::CheckingInput,
            state: &typed_trees::state::State,
            statement_index: usize,
            place: &validation::CanonicalPlace,
            ownership: &FlowOwnershipFacts,
            events: impl IntoIterator<Item = &'a FlowPermissionEventFact>,
        ) -> Option<usize> {
            if self.state != state.symbol {
                return None;
            }
            if !place
                .segments
                .iter()
                .any(|segment| matches!(segment, PlaceSegment::Case { .. }))
            {
                return None;
            }
            let establishment = events.into_iter().filter(|event| {
            event.state_symbol == state.symbol && event.kind == PermissionEventKind::Establish
                && event.access == PermissionAccess::Owned && !event.obligation_live
                && event.claim_identity == PermissionClaimIdentity::Unknown
                && event.root == place.root
                && ownership.segments.span_or_empty(event.segments) == place.segments
                && matches!(event.source, PermissionEventSource::Statement { statement_index: index }
                    if index < statement_index)
        }).max_by_key(|event| match event.source {
            PermissionEventSource::Statement { statement_index } => statement_index,
            _ => 0,
        })?;
            let PermissionEventSource::Statement {
                statement_index: established_at,
            } = establishment.source
            else {
                return None;
            };
            for index in established_at + 1..=statement_index {
                let writes = self.statements.get(index)?.as_ref()?;
                if writes.iter().any(|write| {
                    (write.root) == (place.root)
                        && crate::flow::canonical_place_segments_may_overlap(
                            program,
                            &write.segments,
                            &place.segments,
                        )
                }) {
                    return None;
                }
            }
            Some(established_at)
        }
    }

    fn derive_receipts(
        program: &dyn ::validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        events: &[FlowPermissionEventFact],
        mut failures: Option<&mut Vec<CallClaimJoinFailure>>,
    ) -> Result<Vec<Receipt>, &'static str> {
        let borrow = &facts.borrow;
        let ownership = &facts.flow.ownership;
        let maps = derive_checked_claim_outcome_maps(program, borrow, ownership, events)?;
        let events = &PermissionEvents::prepare(events, &ownership.segments);
        let mut receipts = Vec::new();
        let mut prepared_frontiers: Vec<PreparedCalleeFrontier> = Vec::new();
        let mut consumed_captures = ConsumedCaptureReports::new(program, facts);
        for event in events.iter() {
            if event.kind != PermissionEventKind::Establish
                || event.access != PermissionAccess::Owned
                || !event.obligation_live
            {
                continue;
            }
            let Some((statement_index, expression, received_path)) =
                super::receiving_value(program, &ownership.segments, event)
            else {
                continue;
            };
            if super::call_result_origin_rewrite(
                program,
                &ownership.segments,
                event,
                events,
                &mut super::ClaimOutcomeMapLookup::unrecorded(&maps),
            )
            .is_some_and(|(_, _, provenance, identity)| {
                event.provenance == provenance && event.claim_identity == identity
            }) {
                continue;
            }
            let Some(state) = program.state_by_symbol(event.state_symbol) else {
                continue;
            };
            let Some(call) = program.expression_table.call_node(expression) else {
                continue;
            };
            let Some(target) = validation::call_target_state(program, call.target_symbol) else {
                continue;
            };
            let mut ordinal = 0;
            let exact_call = loop {
                let Some(site) = validation::find_call_site(
                    program,
                    event.machine_symbol,
                    event.state_symbol,
                    statement_index,
                    ordinal,
                ) else {
                    break false;
                };
                if matches!(site, validation::CallSite::Expression { expression: found, .. } if found == expression)
                {
                    break true;
                }
                ordinal += 1;
            };
            if !exact_call {
                continue;
            }
            let failure = |reason, returns| CallClaimJoinFailure {
                receiver: ReceivingClaim {
                    machine: event.machine_symbol,
                    state: event.state_symbol,
                    source: event.source,
                    expression,
                    root: event.root,
                    path: ownership.segments.span_or_empty(event.segments).to_vec(),
                    identity: event.claim_identity,
                },
                statement: statement_index,
                ordinal,
                target: call.target_symbol,
                reason,
                returns,
            };
            let frontier_index = match prepared_frontiers.iter().position(|prepared| {
                prepared.state == target.symbol && prepared.path.as_slice() == received_path
            }) {
                Some(index) => index,
                None => {
                    let index = prepared_frontiers.len();
                    let mut diagnostics = ReturnFrontierDiagnostics {
                        enabled: failures.is_some(),
                        ..Default::default()
                    };
                    let frontiers = state_frontiers(
                        program,
                        facts,
                        target,
                        events,
                        &maps,
                        &mut Vec::new(),
                        received_path,
                        &mut diagnostics,
                        &mut consumed_captures,
                    );
                    prepared_frontiers.push(PreparedCalleeFrontier {
                        state: target.symbol,
                        path: received_path.to_vec(),
                        frontiers,
                        diagnostics,
                    });
                    index
                }
            };
            let prepared = &prepared_frontiers[frontier_index];
            let Some(frontiers) = prepared.frontiers.as_deref() else {
                if let Some(failures) = failures.as_deref_mut() {
                    failures.push(failure(
                        "callee return frontier correspondence is unavailable",
                        prepared.diagnostics.failures.clone(),
                    ));
                }
                continue;
            };
            let path = ownership.segments.span_or_empty(event.segments);
            let mut alternatives = Vec::new();
            let mut complete = true;
            for frontier in frontiers {
                let mut entries = frontier
                    .entries
                    .iter()
                    .filter(|entry| entry.output_path == received_path);
                let Some(entry) = entries.next() else {
                    if frontier
                        .inactive_paths
                        .iter()
                        .any(|inactive| inactive == received_path)
                    {
                        alternatives.push(Alternative {
                            captures: frontier.captures.clone(),
                            exits: frontier.exits.clone(),
                            source: None,
                            excluded_input: frontier.excluded_input.clone(),
                        });
                        continue;
                    }
                    complete = false;
                    break;
                };
                if entries.next().is_some() {
                    complete = false;
                    break;
                }
                let bound_sources = match bound_sources_at_call(
                    program,
                    facts,
                    event.machine_symbol,
                    state,
                    statement_index,
                    ordinal,
                    expression,
                    call,
                    target,
                    &entry.source,
                    events,
                    &mut consumed_captures,
                ) {
                    Ok(sources) => sources,
                    Err(reason) => {
                        if let Some(failures) = failures.as_deref_mut() {
                            failures.push(failure(
                                "completed caller argument cannot bind this callee return source",
                                frontier
                                    .exits
                                    .iter()
                                    .map(|exit| ReturnFrontierFailure {
                                        exit: *exit,
                                        reason,
                                        source: Some(entry.source.clone()),
                                    })
                                    .collect(),
                            ));
                        }
                        complete = false;
                        break;
                    }
                };
                for bound in bound_sources {
                    let mut captures = frontier.captures.clone();
                    captures.extend(bound.captures);
                    alternatives.push(Alternative {
                        captures,
                        exits: frontier.exits.clone(),
                        source: bound.source.map(|bound| (entry.source.clone(), bound)),
                        excluded_input: bound
                            .excluded_input
                            .or_else(|| frontier.excluded_input.clone()),
                    });
                }
            }
            if !complete || alternatives.is_empty() {
                continue;
            }
            let PermissionClaimIdentity::Established {
                machine_symbol,
                state_symbol,
                source,
                ..
            } = event.claim_identity
            else {
                continue;
            };
            if machine_symbol != event.machine_symbol
                || state_symbol != state.symbol
                || source != event.source
            {
                continue;
            }
            receipts.push(Receipt {
                machine: event.machine_symbol,
                state: state.symbol,
                statement: statement_index,
                ordinal,
                target: target.symbol,
                expression,
                root: event.root,
                path: path.to_vec(),
                identity: event.claim_identity,
                alternatives,
            });
        }
        Ok(receipts)
    }

    fn join_provenance(receipt: &Receipt) -> Option<PermissionProvenance> {
        let PermissionClaimIdentity::Established { ordinal, .. } = receipt.identity else {
            return None;
        };
        Some(PermissionProvenance::Joined {
            machine_symbol: receipt.machine,
            state_symbol: receipt.state,
            source: PermissionEventSource::Call {
                statement_index: receipt.statement,
                call_ordinal: receipt.ordinal,
                target_symbol: receipt.target,
            },
            ordinal,
        })
    }

    fn store_source(
        ownership: &mut FlowOwnershipFacts,
        source: CheckedClaimOutcomeSource,
    ) -> FlowClaimOutcomeSource {
        match source {
            CheckedClaimOutcomeSource::Input {
                parameter_symbol,
                path,
            } => FlowClaimOutcomeSource::Input {
                parameter_symbol,
                segments: ownership.segments.insert_many(path),
            },
            CheckedClaimOutcomeSource::Established {
                claim_identity,
                provenance,
            } => FlowClaimOutcomeSource::Established {
                claim_identity,
                provenance,
            },
        }
    }

    fn claim_join_rewrites(
        receipts: &[Receipt],
        events: &[FlowPermissionEventFact],
    ) -> Vec<super::ClaimOriginRewrite> {
        let mut targets = receipts
            .iter()
            .filter_map(|receipt| join_provenance(receipt).map(|provenance| (receipt, provenance)))
            .peekable();
        if targets.peek().is_none() {
            return Vec::new();
        }
        // These identities can cross states. State-scoped origin queries cannot
        // replace the original global scan, and keep their own narrower indexes.
        let events = GlobalClaimEvents::prepare(events);
        let mut rewrites = OrderedOriginRewrites::default();
        for (receipt, provenance) in targets {
            for event in events.at_identity(receipt.identity) {
                if event.claim_identity == receipt.identity && event.provenance != provenance {
                    rewrites.push_unique((
                        event.provenance,
                        event.claim_identity,
                        provenance,
                        receipt.identity,
                    ));
                }
            }
        }
        rewrites.into_rows()
    }

    pub(crate) fn publish_call_claim_joins(
        program: &dyn ::validation::CheckingInput,
        facts: &mut checked_trees::CheckFactsWalk<'_>,
    ) -> Result<Vec<PreparedCallClaimJoin>, &'static str> {
        let mut events = facts
            .flow
            .ownership
            .permissions
            .iter()
            .map(|(_, event)| event.clone())
            .collect::<Vec<_>>();
        let mut converged = false;
        for _ in 0..=events.len() {
            let receipts = derive_receipts(program, facts.view(), &events, None)?;
            let rewrites = claim_join_rewrites(&receipts, &events);
            if !super::apply_claim_origin_rewrites(&mut events, facts.flow.ownership, &rewrites) {
                converged = true;
                break;
            }
        }
        if !converged {
            return Err("call-result claim joins did not converge");
        }
        let receipts = derive_receipts(program, facts.view(), &events, None)?;
        let ownership = &mut facts.flow.ownership;
        ownership.permissions_by_state.clear();
        ownership.permissions = Default::default();
        ownership.permissions.insert_many(events);
        ownership.claim_join_receipts = Default::default();
        ownership.claim_join_alternatives = Default::default();
        ownership.claim_join_exits = Default::default();
        let mut prepared = Vec::new();
        ownership.claim_join_captures = Default::default();
        for receipt in receipts {
            let PermissionClaimIdentity::Established { source, .. } = receipt.identity else {
                return Err("call-result join lacks an exact receiving identity");
            };
            let provenance = join_provenance(&receipt)
                .ok_or("call-result join lacks exact invocation provenance")?;
            prepared.push(PreparedCallClaimJoin {
                receiver: ReceivingClaim {
                    machine: receipt.machine,
                    state: receipt.state,
                    source,
                    expression: receipt.expression,
                    root: receipt.root,
                    path: receipt.path.clone(),
                    identity: receipt.identity,
                },
                provenance,
            });
            let mut alternatives = Vec::new();
            for alternative in receipt.alternatives {
                let exits = ownership.claim_join_exits.insert_many(alternative.exits);
                let source = match (alternative.source, alternative.excluded_input) {
                    (None, None) => FlowClaimJoinAlternativeSource::Inactive,
                    (_, Some(excluded)) => FlowClaimJoinAlternativeSource::ExcludedInput {
                        source: store_source(ownership, excluded.source),
                        operand: excluded.operand,
                        segments: ownership.segments.insert_many(excluded.path),
                    },
                    (Some((source, bound_source)), None) => FlowClaimJoinAlternativeSource::Claim {
                        source: store_source(ownership, source),
                        bound_source: store_source(ownership, bound_source),
                    },
                };
                let captures = alternative
                    .captures
                    .into_iter()
                    .map(|capture| checked_trees::FlowClaimJoinCapture {
                        operand: capture.operand,
                        path: ownership.segments.insert_many(capture.path),
                        identity: capture.identity,
                    })
                    .collect::<Vec<_>>();
                let captures = ownership.claim_join_captures.insert_many(captures);
                alternatives.push(FlowClaimJoinAlternative {
                    exits,
                    source,
                    captures,
                });
            }
            let alternatives = ownership.claim_join_alternatives.insert_many(alternatives);
            let result_segments = ownership.segments.insert_many(receipt.path);
            ownership.claim_join_receipts.insert(FlowClaimJoinReceipt {
                machine_symbol: receipt.machine,
                state_symbol: receipt.state,
                statement_index: receipt.statement,
                call_ordinal: receipt.ordinal,
                target_symbol: receipt.target,
                expression: receipt.expression,
                result_root: receipt.root,
                result_segments,
                claim_identity: receipt.identity,
                alternatives,
            });
        }
        Ok(prepared)
    }

    fn read_source(
        ownership: &FlowOwnershipFacts,
        source: FlowClaimOutcomeSource,
    ) -> Option<CheckedClaimOutcomeSource> {
        Some(match source {
            FlowClaimOutcomeSource::Unknown => return None,
            FlowClaimOutcomeSource::Input {
                parameter_symbol,
                segments,
            } => CheckedClaimOutcomeSource::Input {
                parameter_symbol,
                path: ownership.segments.span(segments)?.to_vec(),
            },
            FlowClaimOutcomeSource::Established {
                claim_identity,
                provenance,
            } => CheckedClaimOutcomeSource::Established {
                claim_identity,
                provenance,
            },
        })
    }

    pub(crate) fn validate_call_claim_joins(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Vec<CallClaimJoinFailure> {
        let ownership = &facts.flow.ownership;
        let events = ownership
            .permissions
            .iter()
            .map(|(_, event)| event.clone())
            .collect::<Vec<_>>();
        let mut failures = Vec::new();
        let expected = match derive_receipts(program, facts.view(), &events, Some(&mut failures)) {
            Ok(receipts) => receipts,
            Err(message) => {
                diagnostics.push(Diagnostic::error(message));
                return failures;
            }
        };
        let recorded = ownership
            .claim_join_receipts
            .iter()
            .map(|(_, receipt)| {
                let alternatives = ownership
                    .claim_join_alternatives
                    .span(receipt.alternatives)?
                    .iter()
                    .map(|alternative| {
                        Some(Alternative {
                            captures: ownership
                                .claim_join_captures
                                .span(alternative.captures)?
                                .iter()
                                .map(|capture| {
                                    Some(Capture {
                                        operand: capture.operand,
                                        path: ownership.segments.span(capture.path)?.to_vec(),
                                        identity: capture.identity,
                                    })
                                })
                                .collect::<Option<Vec<_>>>()?,
                            exits: ownership.claim_join_exits.span(alternative.exits)?.to_vec(),
                            source: match alternative.source {
                                FlowClaimJoinAlternativeSource::Inactive
                                | FlowClaimJoinAlternativeSource::ExcludedInput { .. } => None,
                                FlowClaimJoinAlternativeSource::Claim {
                                    source,
                                    bound_source,
                                } => Some((
                                    read_source(ownership, source)?,
                                    read_source(ownership, bound_source)?,
                                )),
                            },
                            excluded_input: match alternative.source {
                                FlowClaimJoinAlternativeSource::ExcludedInput {
                                    source,
                                    operand,
                                    segments,
                                } => Some(ExcludedInput {
                                    source: read_source(ownership, source)?,
                                    operand,
                                    path: ownership.segments.span(segments)?.to_vec(),
                                }),
                                _ => None,
                            },
                        })
                    })
                    .collect::<Option<Vec<_>>>()?;
                Some(Receipt {
                    machine: receipt.machine_symbol,
                    state: receipt.state_symbol,
                    statement: receipt.statement_index,
                    ordinal: receipt.call_ordinal,
                    target: receipt.target_symbol,
                    expression: receipt.expression,
                    root: receipt.result_root,
                    path: ownership.segments.span(receipt.result_segments)?.to_vec(),
                    identity: receipt.claim_identity,
                    alternatives,
                })
            })
            .collect::<Option<Vec<_>>>();
        if recorded.as_ref() != Some(&expected) {
            let detail = if let Some(recorded) = &recorded {
                (0..recorded.len().max(expected.len()))
                    .find(|&position| recorded.get(position) != expected.get(position))
                    .and_then(|position| expected.get(position).or_else(|| recorded.get(position)))
                    .map(|receipt| format!(
                        "in machine `{}` state `{}` statement {} call {} to `{}`: result receipt differs",
                        program.symbols.name(receipt.machine),
                        program.symbols.name(receipt.state),
                        receipt.statement,
                        receipt.ordinal,
                        program.symbols.name(receipt.target),
                    ))
                    .unwrap_or_else(|| "receipt roster differs".to_owned())
            } else {
                "stored receipt contains an invalid source or span".to_owned()
            };
            diagnostics.push(Diagnostic::error(format!(
                "call claim joins differ from exact source exit and call replay: {detail}"
            )));
        }
        let invalid_event = events.iter().find(|event| {
            let valid = if matches!(
                event.provenance,
                PermissionProvenance::Joined {
                    source: PermissionEventSource::Statement { .. },
                    ..
                }
            ) {
                claim_origin(ownership, event.claim_identity, event.provenance)
            } else if matches!(event.provenance, PermissionProvenance::Joined { .. }) {
                expected.iter().any(|receipt| {
                    receipt.identity == event.claim_identity
                        && join_provenance(receipt) == Some(event.provenance)
                })
            } else {
                !expected
                    .iter()
                    .any(|receipt| receipt.identity == event.claim_identity)
            };
            !valid
        });
        if let Some(event) = invalid_event {
            diagnostics.push(Diagnostic::error(format!(
                "call claim joins differ from exact source exit and call replay in machine `{}` state `{}` at {:?}: {:?} has no matching claim origin",
                program.symbols.name(event.machine_symbol),
                program.symbols.name(event.state_symbol),
                event.source,
                event.kind,
            )));
        }
        failures
    }
}
mod event_queries {
    //! A return summary should search the permission rows for its actual argument,
    //! not every event in unrelated states. A batch borrows one immutable event and
    //! segment snapshot; its sorted associations retain original row ordinals.
    //! Candidate outcome maps remain separate and are read afresh by the caller.
    //!
    //! Recording asks once before appending the next event, so it borrows the
    //! current rows directly. Reconciliation and return discovery prepare repeated
    //! queries once, then drop the associations before rewriting those rows.

    use super::permission_event_statement_index;
    use checked_trees::FlowPermissionEventFact;
    use language_semantics::{PermissionClaimIdentity, PermissionProvenance};
    use std::hash::{DefaultHasher, Hash, Hasher};
    use symbols::SymbolHandle;

    struct SortedRows {
        rows: Vec<(u64, usize)>,
    }

    impl SortedRows {
        fn new(rows: impl Iterator<Item = (u64, usize)>) -> Self {
            let mut rows = rows.collect::<Vec<_>>();
            // The ordinal breaks ties, including hash collisions. A query therefore
            // sees the same order as filtering the original complete event slice.
            rows.sort_unstable();
            Self { rows }
        }

        fn matching(&self, key: u64) -> &[(u64, usize)] {
            let first = self.rows.partition_point(|row| row.0 < key);
            let count = self.rows[first..].partition_point(|row| row.0 == key);
            &self.rows[first..first + count]
        }
    }

    fn key_hash(value: impl Hash) -> u64 {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    // A join update follows one identity across every state. Keep this view
    // separate from state-scoped queries so neither search visits unrelated rows.
    pub(super) struct GlobalClaimEvents<'events> {
        events: &'events [FlowPermissionEventFact],
        identities: SortedRows,
    }

    impl<'events> GlobalClaimEvents<'events> {
        pub(super) fn prepare(events: &'events [FlowPermissionEventFact]) -> Self {
            Self {
                events,
                identities: SortedRows::new(
                    events
                        .iter()
                        .enumerate()
                        .map(|(ordinal, event)| (key_hash(event.claim_identity), ordinal)),
                ),
            }
        }

        pub(super) fn at_identity(&self, identity: PermissionClaimIdentity) -> EventCandidates<'_> {
            EventCandidates {
                events: self.events,
                rows: Some(self.identities.matching(key_hash(identity))),
                position: 0,
            }
        }
    }

    pub(super) type ClaimOriginRewrite = (
        PermissionProvenance,
        PermissionClaimIdentity,
        PermissionProvenance,
        PermissionClaimIdentity,
    );

    #[derive(Default)]
    pub(super) struct OrderedOriginRewrites {
        rows: Vec<ClaimOriginRewrite>,
        seen: symbols::SymbolKeyMap<ClaimOriginRewrite, ()>,
    }

    impl OrderedOriginRewrites {
        pub(super) fn push_unique(&mut self, rewrite: ClaimOriginRewrite) {
            if let std::collections::hash_map::Entry::Vacant(entry) = self.seen.entry(rewrite) {
                entry.insert(());
                self.rows.push(rewrite);
            }
        }

        pub(super) fn into_rows(self) -> Vec<ClaimOriginRewrite> {
            self.rows
        }
    }

    // One immutable rewrite list supplies every bounded chain walk. Candidate
    // ordinals retain its first-match rule, including conflicting replacements.
    pub(super) struct OriginRewrites<'rewrites> {
        rows: &'rewrites [ClaimOriginRewrite],
        sources: SortedRows,
    }

    impl<'rewrites> OriginRewrites<'rewrites> {
        pub(super) fn prepare(rows: &'rewrites [ClaimOriginRewrite]) -> Self {
            Self {
                rows,
                sources: SortedRows::new(
                    rows.iter()
                        .enumerate()
                        .map(|(ordinal, row)| (key_hash((row.0, row.1)), ordinal)),
                ),
            }
        }

        pub(super) fn len(&self) -> usize {
            self.rows.len()
        }

        pub(super) fn first_for_source(
            &self,
            provenance: PermissionProvenance,
            identity: PermissionClaimIdentity,
        ) -> Option<&ClaimOriginRewrite> {
            self.sources
                .matching(key_hash((provenance, identity)))
                .iter()
                .map(|(_, ordinal)| &self.rows[*ordinal])
                .find(|(source_provenance, source_identity, _, _)| {
                    *source_provenance == provenance && *source_identity == identity
                })
        }
    }

    struct EventAssociations {
        statements: SortedRows,
        origins: SortedRows,
        claim_identities: SortedRows,
        places: SortedRows,
        places_across_statements: SortedRows,
    }

    pub(super) struct PermissionEvents<'events> {
        events: std::borrow::Cow<'events, [FlowPermissionEventFact]>,
        associations: Option<EventAssociations>,
    }

    impl<'events> PermissionEvents<'events> {
        pub(super) fn recorded(events: &'events [FlowPermissionEventFact]) -> Self {
            Self {
                events: std::borrow::Cow::Borrowed(events),
                associations: None,
            }
        }

        pub(super) fn prepare(
            events: &'events [FlowPermissionEventFact],
            segments: &arena::PagedArenaSet<facts::PlaceSegment>,
        ) -> Self {
            Self::with_rows(std::borrow::Cow::Borrowed(events), segments)
        }

        pub(super) fn owned(
            events: Vec<FlowPermissionEventFact>,
            segments: &arena::PagedArenaSet<facts::PlaceSegment>,
        ) -> Self {
            Self::with_rows(std::borrow::Cow::Owned(events), segments)
        }

        fn with_rows(
            events: std::borrow::Cow<'events, [FlowPermissionEventFact]>,
            segments: &arena::PagedArenaSet<facts::PlaceSegment>,
        ) -> Self {
            let statements =
                SortedRows::new(events.iter().enumerate().filter_map(|(ordinal, event)| {
                    let statement = permission_event_statement_index(event.source)?;
                    Some((key_hash((event.state_symbol, statement)), ordinal))
                }));
            let origins = SortedRows::new(events.iter().enumerate().map(|(ordinal, event)| {
                (
                    key_hash((event.state_symbol, event.claim_identity, event.provenance)),
                    ordinal,
                )
            }));
            let claim_identities =
                SortedRows::new(events.iter().enumerate().map(|(ordinal, event)| {
                    (
                        key_hash((event.state_symbol, event.claim_identity)),
                        ordinal,
                    )
                }));
            let places = SortedRows::new(events.iter().enumerate().map(|(ordinal, event)| {
                let statement = permission_event_statement_index(event.source);
                (
                    key_hash((
                        event.state_symbol,
                        statement,
                        event.root,
                        segments.span_or_empty(event.segments),
                    )),
                    ordinal,
                )
            }));
            let places_across_statements =
                SortedRows::new(events.iter().enumerate().map(|(ordinal, event)| {
                    (
                        key_hash((
                            event.state_symbol,
                            event.root,
                            segments.span_or_empty(event.segments),
                        )),
                        ordinal,
                    )
                }));
            Self {
                events,
                associations: Some(EventAssociations {
                    statements,
                    origins,
                    claim_identities,
                    places,
                    places_across_statements,
                }),
            }
        }

        pub(super) fn iter(&self) -> std::slice::Iter<'_, FlowPermissionEventFact> {
            self.events.iter()
        }

        pub(super) fn at_statement(
            &self,
            state: SymbolHandle,
            statement: usize,
        ) -> EventCandidates<'_> {
            self.candidates(
                self.associations
                    .as_ref()
                    .map(|rows| rows.statements.matching(key_hash((state, statement)))),
            )
        }

        pub(super) fn at_origin(
            &self,
            state: SymbolHandle,
            identity: PermissionClaimIdentity,
            provenance: PermissionProvenance,
        ) -> EventCandidates<'_> {
            self.candidates(self.associations.as_ref().map(|rows| {
                rows.origins
                    .matching(key_hash((state, identity, provenance)))
            }))
        }

        // An origin search needs every provenance for this exact claim. Keep it
        // separate so queries with a known provenance retain their narrow key.
        pub(super) fn at_claim_identity(
            &self,
            state: SymbolHandle,
            identity: PermissionClaimIdentity,
        ) -> EventCandidates<'_> {
            self.candidates(
                self.associations
                    .as_ref()
                    .map(|rows| rows.claim_identities.matching(key_hash((state, identity)))),
            )
        }

        pub(super) fn at_place(
            &self,
            state: SymbolHandle,
            statement: Option<usize>,
            root: facts::PlaceRoot,
            path: &[facts::PlaceSegment],
        ) -> EventCandidates<'_> {
            self.candidates(self.associations.as_ref().map(|rows| {
                rows.places
                    .matching(key_hash((state, statement, root, path)))
            }))
        }

        // Preservation must consider earlier statements. Its broader candidates
        // must not make exact argument queries scan the history of the same place.
        pub(super) fn at_place_across_statements(
            &self,
            state: SymbolHandle,
            root: facts::PlaceRoot,
            path: &[facts::PlaceSegment],
        ) -> EventCandidates<'_> {
            self.candidates(self.associations.as_ref().map(|rows| {
                rows.places_across_statements
                    .matching(key_hash((state, root, path)))
            }))
        }

        fn candidates<'query>(
            &'query self,
            rows: Option<&'query [(u64, usize)]>,
        ) -> EventCandidates<'query> {
            EventCandidates {
                events: &self.events,
                rows,
                position: 0,
            }
        }
    }

    /// Hashes select candidates only. Every caller retains its full original typed
    /// equality, permission, liveness and uniqueness predicates, including when a
    /// colliding row is offered by one of these associations.
    pub(super) struct EventCandidates<'query> {
        events: &'query [FlowPermissionEventFact],
        rows: Option<&'query [(u64, usize)]>,
        position: usize,
    }

    impl<'query> Iterator for EventCandidates<'query> {
        type Item = &'query FlowPermissionEventFact;

        fn next(&mut self) -> Option<Self::Item> {
            let ordinal = match self.rows {
                Some(rows) => rows.get(self.position)?.1,
                None => self.position,
            };
            let event = self.events.get(ordinal)?;
            self.position += 1;
            Some(event)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{
            ClaimOriginRewrite, GlobalClaimEvents, OrderedOriginRewrites, OriginRewrites,
            PermissionEvents, SortedRows, key_hash, permission_event_statement_index,
        };
        use checked_trees::FlowPermissionEventFact;
        use language_semantics::{
            PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
            PermissionProvenance,
        };
        use std::cell::Cell;
        use symbols::SymbolHandle;

        fn rows(
            count: usize,
        ) -> (
            Vec<FlowPermissionEventFact>,
            arena::PagedArenaSet<facts::PlaceSegment>,
        ) {
            let mut segments = arena::PagedArenaSet::default();
            let events = (0..count)
                .map(|ordinal| {
                    let state = SymbolHandle::from_parts(
                        (ordinal % 8 + 1) as u32,
                        (ordinal / 8 % 2 + 1) as u32,
                    );
                    let source = if ordinal % 3 == 0 {
                        PermissionEventSource::StateEntry
                    } else {
                        PermissionEventSource::Statement {
                            statement_index: ordinal / 16 % 4,
                        }
                    };
                    FlowPermissionEventFact {
                        state_symbol: state,
                        source,
                        expression: typed_trees::expression::ExpressionHandle::from_parts(
                            (ordinal + 1) as u32,
                            1,
                        ),
                        kind: if ordinal % 3 == 0 {
                            PermissionEventKind::Establish
                        } else {
                            PermissionEventKind::Transfer
                        },
                        access: if ordinal % 5 == 0 {
                            PermissionAccess::Shared
                        } else {
                            PermissionAccess::Owned
                        },
                        claim_identity: PermissionClaimIdentity::Established {
                            machine_symbol: SymbolHandle::from_parts(1, 1),
                            state_symbol: state,
                            source: PermissionEventSource::StateEntry,
                            ordinal: (ordinal % 13) as u32,
                        },
                        provenance: PermissionProvenance::Established {
                            machine_symbol: SymbolHandle::from_parts(1, 1),
                            state_symbol: state,
                            source,
                        },
                        root: facts::PlaceRoot::Symbol(SymbolHandle::from_parts(
                            45,
                            (ordinal % 2 + 1) as u32,
                        )),
                        segments: segments.insert_many([
                            facts::PlaceSegment::Field {
                                symbol: SymbolHandle::from_parts(22, 1),
                            },
                            facts::PlaceSegment::FixedIndex { index: ordinal % 7 },
                        ]),
                        obligation_live: ordinal % 11 != 0,
                        ..FlowPermissionEventFact::default()
                    }
                })
                .collect();
            (events, segments)
        }

        fn claim_definitions<'a>(
            events: impl Iterator<Item = &'a FlowPermissionEventFact>,
            state: SymbolHandle,
            identity: PermissionClaimIdentity,
        ) -> Vec<&'a FlowPermissionEventFact> {
            events
                .filter(|event| {
                    event.state_symbol == state
                        && matches!(
                            event.kind,
                            PermissionEventKind::Establish | PermissionEventKind::Restore
                        )
                        && event.access == PermissionAccess::Owned
                        && event.obligation_live
                        && event.claim_identity == identity
                })
                .collect()
        }

        fn compare_batches(
            events: &[FlowPermissionEventFact],
            segments: &arena::PagedArenaSet<facts::PlaceSegment>,
        ) -> (usize, usize, usize) {
            let recorded = PermissionEvents::recorded(events);
            let preparation_start = std::time::Instant::now();
            let prepared = PermissionEvents::prepare(events, segments);
            let preparation_elapsed = preparation_start.elapsed();
            let owned = PermissionEvents::owned(events.to_vec(), segments);
            assert_eq!(
                owned.iter().collect::<Vec<_>>(),
                events.iter().collect::<Vec<_>>()
            );
            let owned_absence_scans = Cell::new(0);
            let indexed_rows = prepared.associations.as_ref().map_or(0, |rows| {
                rows.statements.rows.len()
                    + rows.origins.rows.len()
                    + rows.claim_identities.rows.len()
                    + rows.places.rows.len()
                    + rows.places_across_statements.rows.len()
            });
            eprintln!(
                "permission event preparation: {} source rows across five passes; {} indexed rows; {:?}",
                events.len(),
                indexed_rows,
                preparation_elapsed
            );
            let direct_scans = Cell::new(0);
            let prepared_scans = Cell::new(0);
            let matched = Cell::new(0);
            let identity_direct = Cell::new(0);
            let identity_prepared = Cell::new(0);
            let identity_matches = Cell::new(0);
            for event in events.iter().step_by(31) {
                let statement = permission_event_statement_index(event.source);
                let path = segments.span_or_empty(event.segments);
                let same_place = |candidate: &&FlowPermissionEventFact| {
                    candidate.state_symbol == event.state_symbol
                        && permission_event_statement_index(candidate.source) == statement
                        && candidate.root == event.root
                        && segments.span_or_empty(candidate.segments) == path
                        && candidate.kind == event.kind
                        && candidate.access == PermissionAccess::Owned
                        && candidate.obligation_live
                };
                let direct = recorded
                    .at_place(event.state_symbol, statement, event.root, path)
                    .inspect(|_| direct_scans.set(direct_scans.get() + 1))
                    .filter(same_place)
                    .collect::<Vec<_>>();
                let indexed = prepared
                    .at_place(event.state_symbol, statement, event.root, path)
                    .inspect(|_| prepared_scans.set(prepared_scans.get() + 1))
                    .filter(same_place)
                    .collect::<Vec<_>>();
                assert_eq!(indexed, direct);
                matched.set(matched.get() + direct.len());
                let direct = claim_definitions(
                    recorded
                        .at_claim_identity(event.state_symbol, event.claim_identity)
                        .inspect(|_| {
                            direct_scans.set(direct_scans.get() + 1);
                            identity_direct.set(identity_direct.get() + 1);
                        }),
                    event.state_symbol,
                    event.claim_identity,
                );
                let indexed = claim_definitions(
                    prepared
                        .at_claim_identity(event.state_symbol, event.claim_identity)
                        .inspect(|_| {
                            prepared_scans.set(prepared_scans.get() + 1);
                            identity_prepared.set(identity_prepared.get() + 1);
                        }),
                    event.state_symbol,
                    event.claim_identity,
                );
                assert_eq!(indexed, direct);
                matched.set(matched.get() + direct.len());
                identity_matches.set(identity_matches.get() + direct.len());
                // Absence searches all earlier establishments, not one exact
                // statement. The last equal-position row must still win.
                let inactive = |candidate: &&FlowPermissionEventFact| {
                    candidate.state_symbol == event.state_symbol
                        && candidate.kind == PermissionEventKind::Establish
                        && candidate.access == PermissionAccess::Owned
                        && !candidate.obligation_live
                        && candidate.claim_identity == PermissionClaimIdentity::Unknown
                        && candidate.root == event.root
                        && segments.span_or_empty(candidate.segments) == path
                        && matches!(candidate.source,
                            PermissionEventSource::Statement { statement_index } if statement_index < 5)
                };
                let latest = |candidate: &&FlowPermissionEventFact| {
                    permission_event_statement_index(candidate.source).unwrap_or(0)
                };
                let direct = recorded
                    .at_place_across_statements(event.state_symbol, event.root, path)
                    .inspect(|_| direct_scans.set(direct_scans.get() + 1))
                    .filter(inactive)
                    .max_by_key(latest);
                let indexed = prepared
                    .at_place_across_statements(event.state_symbol, event.root, path)
                    .inspect(|_| prepared_scans.set(prepared_scans.get() + 1))
                    .filter(inactive)
                    .max_by_key(latest);
                assert_eq!(indexed, direct);
                let from_owned = owned
                    .at_place_across_statements(event.state_symbol, event.root, path)
                    .inspect(|_| owned_absence_scans.set(owned_absence_scans.get() + 1))
                    .filter(inactive)
                    .max_by_key(latest);
                assert_eq!(from_owned, direct);
                matched.set(matched.get() + usize::from(direct.is_some()));
                let same_origin = |candidate: &&FlowPermissionEventFact| {
                    candidate.state_symbol == event.state_symbol
                        && candidate.claim_identity == event.claim_identity
                        && candidate.provenance == event.provenance
                        && candidate.kind == PermissionEventKind::Establish
                        && candidate.access == PermissionAccess::Owned
                        && candidate.obligation_live
                };
                let direct = recorded
                    .at_origin(event.state_symbol, event.claim_identity, event.provenance)
                    .inspect(|_| direct_scans.set(direct_scans.get() + 1))
                    .filter(same_origin)
                    .collect::<Vec<_>>();
                let indexed = prepared
                    .at_origin(event.state_symbol, event.claim_identity, event.provenance)
                    .inspect(|_| prepared_scans.set(prepared_scans.get() + 1))
                    .filter(same_origin)
                    .collect::<Vec<_>>();
                assert_eq!(indexed, direct);
                matched.set(matched.get() + direct.len());
                // State-entry source lookup stops on its first exact match. Compare
                // rows actually consumed by find, not a full-scan estimate.
                let same_entry = |candidate: &&FlowPermissionEventFact| {
                    candidate.state_symbol == event.state_symbol
                        && candidate.claim_identity == event.claim_identity
                        && candidate.provenance == event.provenance
                        && candidate.source == PermissionEventSource::StateEntry
                        && candidate.kind == PermissionEventKind::Establish
                        && candidate.access == PermissionAccess::Owned
                };
                let direct = recorded
                    .at_origin(event.state_symbol, event.claim_identity, event.provenance)
                    .inspect(|_| direct_scans.set(direct_scans.get() + 1))
                    .find(same_entry);
                let indexed = prepared
                    .at_origin(event.state_symbol, event.claim_identity, event.provenance)
                    .inspect(|_| prepared_scans.set(prepared_scans.get() + 1))
                    .find(same_entry);
                assert_eq!(indexed, direct);
                matched.set(matched.get() + usize::from(direct.is_some()));
                if let Some(statement) = statement {
                    let same_statement = |candidate: &&FlowPermissionEventFact| {
                        candidate.state_symbol == event.state_symbol
                            && permission_event_statement_index(candidate.source) == Some(statement)
                            && candidate.kind == PermissionEventKind::Transfer
                            && candidate.access == PermissionAccess::Owned
                            && candidate.obligation_live
                    };
                    let direct = recorded
                        .at_statement(event.state_symbol, statement)
                        .inspect(|_| direct_scans.set(direct_scans.get() + 1))
                        .filter(same_statement)
                        .collect::<Vec<_>>();
                    let indexed = prepared
                        .at_statement(event.state_symbol, statement)
                        .inspect(|_| prepared_scans.set(prepared_scans.get() + 1))
                        .filter(same_statement)
                        .collect::<Vec<_>>();
                    assert_eq!(indexed, direct);
                    matched.set(matched.get() + direct.len());
                }
            }
            eprintln!(
                "claim identity table: direct {} visited rows; prepared {} visited rows; {} matched rows",
                identity_direct.get(),
                identity_prepared.get(),
                identity_matches.get()
            );
            eprintln!(
                "owned absence snapshot: {} candidate rows across the same query batch",
                owned_absence_scans.get()
            );
            (direct_scans.get(), prepared_scans.get(), matched.get())
        }

        #[test]
        fn immutable_event_associations_preserve_query_batches_and_rebuilds() {
            let (mut events, mut segments) = rows(4096);
            // Equal path values in separately allocated ranges are interchangeable
            // for the old slice comparison; range-handle hashing would lose rows.
            assert_ne!(events[1].segments, events[1345].segments);
            assert_eq!(
                segments.span_or_empty(events[1].segments),
                segments.span_or_empty(events[1345].segments)
            );
            let mut inactive = events[0].clone();
            inactive.segments = segments.insert_many([
                facts::PlaceSegment::Field {
                    symbol: SymbolHandle::from_parts(22, 1),
                },
                facts::PlaceSegment::Case {
                    variant: SymbolHandle::from_parts(83, 1),
                },
            ]);
            events[0].segments = inactive.segments;
            inactive.kind = PermissionEventKind::Establish;
            inactive.access = PermissionAccess::Owned;
            inactive.claim_identity = PermissionClaimIdentity::Unknown;
            inactive.obligation_live = false;
            inactive.source = PermissionEventSource::Statement { statement_index: 1 };
            events.push(inactive.clone());
            inactive.source = PermissionEventSource::Statement { statement_index: 3 };
            events.push(inactive.clone());
            // A distinct segment range with the same values must participate in
            // the same latest-position tie, using its actual original ordinal.
            inactive.segments =
                segments.insert_many(segments.span_or_empty(inactive.segments).to_vec());
            events.push(inactive.clone());
            let last_tie = events.len() - 1;
            let mut excluded = inactive.clone();
            excluded.source = PermissionEventSource::Statement { statement_index: 5 };
            events.push(excluded);
            let mut excluded = inactive.clone();
            excluded.obligation_live = true;
            events.push(excluded);
            let mut excluded = inactive.clone();
            excluded.claim_identity = events[1].claim_identity;
            events.push(excluded);
            let mut excluded = inactive.clone();
            excluded.kind = PermissionEventKind::Transfer;
            events.push(excluded);
            let mut excluded = inactive.clone();
            excluded.access = PermissionAccess::Shared;
            events.push(excluded);
            let mut excluded = inactive.clone();
            excluded.state_symbol = SymbolHandle::from_parts(
                inactive.state_symbol.arena_index(),
                inactive.state_symbol.generation() + 1,
            );
            events.push(excluded);
            let mut excluded = inactive.clone();
            excluded.root = facts::PlaceRoot::Symbol(SymbolHandle::from_parts(45, 9));
            events.push(excluded);
            let mut excluded = inactive.clone();
            excluded.segments = segments.insert_many([
                facts::PlaceSegment::Field {
                    symbol: SymbolHandle::from_parts(22, 1),
                },
                facts::PlaceSegment::Case {
                    variant: SymbolHandle::from_parts(83, 2),
                },
            ]);
            events.push(excluded);
            let selected = |events: &[FlowPermissionEventFact]| {
                let prepared = PermissionEvents::prepare(events, &segments);
                prepared
                    .at_place_across_statements(
                        inactive.state_symbol,
                        inactive.root,
                        segments.span_or_empty(inactive.segments),
                    )
                    .filter(|event| {
                        event.state_symbol == inactive.state_symbol
                            && event.root == inactive.root
                            && segments.span_or_empty(event.segments)
                                == segments.span_or_empty(inactive.segments)
                            && event.kind == PermissionEventKind::Establish
                            && event.access == PermissionAccess::Owned
                            && !event.obligation_live
                            && event.claim_identity == PermissionClaimIdentity::Unknown
                            && matches!(event.source,
                                PermissionEventSource::Statement { statement_index } if statement_index < 5)
                    })
                    .max_by_key(|event| permission_event_statement_index(event.source))
                    .map(|event| {
                        events.iter().position(|row| std::ptr::eq(row, event))
                            .expect("candidate belongs to the borrowed event slice")
                    })
            };
            assert_eq!(selected(&events), Some(last_tie));
            let start = std::time::Instant::now();
            let (direct_scans, prepared_scans, matched) = compare_batches(&events, &segments);
            eprintln!(
                "permission event table: {} prepared source rows; direct {} visited rows; prepared {} visited rows; {} matched rows; comparison {:?}",
                events.len(),
                direct_scans,
                prepared_scans,
                matched,
                start.elapsed()
            );
            assert!(prepared_scans < direct_scans / 8);
            events[last_tie].obligation_live = true;
            assert_eq!(selected(&events), Some(last_tie - 1));
            // A new immutable snapshot sees changed keys and filtering fields. No
            // association can remain borrowed across these actual mutations.
            for event in events.iter_mut().step_by(5) {
                event.state_symbol = SymbolHandle::from_parts(
                    event.state_symbol.arena_index(),
                    event.state_symbol.generation() + 1,
                );
                event.provenance = PermissionProvenance::Unknown;
                event.obligation_live = !event.obligation_live;
            }
            compare_batches(&events, &segments);
            compare_batches(&[], &segments);

            // Replacing the same local at many statements must not broaden an
            // exact-statement query to the complete history of that storage.
            let (mut repeated, repeated_segments) = rows(1024);
            let place = repeated[1].clone();
            for (statement_index, event) in repeated.iter_mut().enumerate() {
                event.state_symbol = place.state_symbol;
                event.root = place.root;
                event.segments = place.segments;
                event.source = PermissionEventSource::Statement { statement_index };
            }
            let start = std::time::Instant::now();
            let prepared = PermissionEvents::prepare(&repeated, &repeated_segments);
            let preparation_elapsed = start.elapsed();
            let recorded = PermissionEvents::recorded(&repeated);
            let mut direct_scans = 0;
            let mut prepared_scans = 0;
            let path = repeated_segments.span_or_empty(place.segments);
            for statement in 0..repeated.len() {
                let exact = |candidate: &&FlowPermissionEventFact| {
                    candidate.state_symbol == place.state_symbol
                        && permission_event_statement_index(candidate.source) == Some(statement)
                        && candidate.root == place.root
                        && repeated_segments.span_or_empty(candidate.segments) == path
                };
                let direct = recorded
                    .at_place(place.state_symbol, Some(statement), place.root, path)
                    .inspect(|_| direct_scans += 1)
                    .filter(exact)
                    .collect::<Vec<_>>();
                let indexed = prepared
                    .at_place(place.state_symbol, Some(statement), place.root, path)
                    .inspect(|_| prepared_scans += 1)
                    .filter(exact)
                    .collect::<Vec<_>>();
                assert_eq!(indexed, direct);
                assert_eq!(indexed.len(), 1);
            }
            assert_eq!(direct_scans, repeated.len() * repeated.len());
            assert_eq!(prepared_scans, repeated.len());
            eprintln!(
                "repeated place table: {} source rows across five preparation passes; {} exact-statement queries; direct {} visited rows; prepared {} visited rows; preparation {:?}",
                repeated.len(),
                repeated.len(),
                direct_scans,
                prepared_scans,
                preparation_elapsed,
            );

            // Duplicate entries and differing provenances must reach the origin
            // consumer unchanged; selecting candidates must not settle ambiguity.
            let (mut definitions, definition_segments) = rows(64);
            let mut entry = definitions[1].clone();
            entry.state_symbol = SymbolHandle::from_parts(90001, 7);
            entry.claim_identity = PermissionClaimIdentity::Established {
                machine_symbol: SymbolHandle::from_parts(90000, 1),
                state_symbol: entry.state_symbol,
                source: PermissionEventSource::StateEntry,
                ordinal: 44,
            };
            entry.source = PermissionEventSource::StateEntry;
            entry.kind = PermissionEventKind::Establish;
            entry.access = PermissionAccess::Owned;
            entry.obligation_live = true;
            entry.provenance = PermissionProvenance::Unknown;
            let identity = entry.claim_identity;
            let entry_position = definitions.len();
            definitions.push(entry.clone());
            definitions.push(entry.clone());
            let mut restored = entry.clone();
            restored.source = PermissionEventSource::Statement { statement_index: 9 };
            restored.kind = PermissionEventKind::Restore;
            restored.provenance = definitions[2].provenance;
            definitions.push(restored.clone());
            let mut excluded = restored.clone();
            excluded.kind = PermissionEventKind::Transfer;
            definitions.push(excluded);
            let mut excluded = restored.clone();
            excluded.access = PermissionAccess::Shared;
            definitions.push(excluded);
            let mut excluded = restored.clone();
            excluded.obligation_live = false;
            definitions.push(excluded);
            let other_state = SymbolHandle::from_parts(90001, 8);
            let mut excluded = restored.clone();
            excluded.state_symbol = other_state;
            definitions.push(excluded);
            let other_identity = PermissionClaimIdentity::Established {
                machine_symbol: SymbolHandle::from_parts(90000, 2),
                state_symbol: entry.state_symbol,
                source: PermissionEventSource::StateEntry,
                ordinal: 44,
            };
            let mut excluded = restored.clone();
            excluded.claim_identity = other_identity;
            definitions.push(excluded);
            restored.claim_identity = PermissionClaimIdentity::Unknown;
            definitions.push(restored);
            let inspect = |events: &[FlowPermissionEventFact]| {
                let prepared = PermissionEvents::prepare(events, &definition_segments);
                for (state, claim) in [
                    (entry.state_symbol, identity),
                    (entry.state_symbol, PermissionClaimIdentity::Unknown),
                    (other_state, identity),
                    (entry.state_symbol, other_identity),
                ] {
                    assert_eq!(
                        claim_definitions(prepared.at_claim_identity(state, claim), state, claim),
                        claim_definitions(events.iter(), state, claim),
                    );
                }
                let selected = claim_definitions(
                    prepared.at_claim_identity(entry.state_symbol, identity),
                    entry.state_symbol,
                    identity,
                );
                assert!(
                    selected
                        .iter()
                        .any(|event| event.provenance == PermissionProvenance::Unknown)
                );
                assert!(
                    selected
                        .iter()
                        .any(|event| event.provenance != PermissionProvenance::Unknown)
                );
                (
                    selected.len(),
                    selected
                        .iter()
                        .filter(|event| event.source == PermissionEventSource::StateEntry)
                        .count(),
                    claim_definitions(
                        prepared.at_claim_identity(
                            entry.state_symbol,
                            PermissionClaimIdentity::Unknown,
                        ),
                        entry.state_symbol,
                        PermissionClaimIdentity::Unknown,
                    )
                    .len(),
                )
            };
            assert_eq!(inspect(&definitions), (3, 2, 1));
            definitions[entry_position].claim_identity = PermissionClaimIdentity::Unknown;
            assert_eq!(inspect(&definitions), (2, 1, 2));
        }

        #[test]
        fn global_identity_updates_keep_order_and_bounded_first_match_chains() {
            let (mut events, _) = rows(4096);
            let identity = |ordinal| PermissionClaimIdentity::Established {
                machine_symbol: SymbolHandle::from_parts(1, 1),
                state_symbol: SymbolHandle::from_parts(2, 1),
                source: PermissionEventSource::StateEntry,
                ordinal,
            };
            for (ordinal, event) in events.iter_mut().enumerate() {
                event.claim_identity = identity((ordinal % 512) as u32);
                // An identity deliberately survives across event states, kinds,
                // access modes and live flags. Collection excludes none of them.
                event.state_symbol = SymbolHandle::from_parts((ordinal / 512 + 1) as u32, 2);
            }
            let started = std::time::Instant::now();
            let indexed = GlobalClaimEvents::prepare(&events);
            let preparation = started.elapsed();
            let mut direct_rows = 0;
            let mut indexed_rows = 0;
            for ordinal in 0..512 {
                let wanted = identity(ordinal);
                let direct = events
                    .iter()
                    .filter(|event| {
                        direct_rows += 1;
                        event.claim_identity == wanted
                    })
                    .map(|event| event.expression)
                    .collect::<Vec<_>>();
                let prepared = indexed
                    .at_identity(wanted)
                    .filter(|event| {
                        indexed_rows += 1;
                        event.claim_identity == wanted
                    })
                    .map(|event| event.expression)
                    .collect::<Vec<_>>();
                assert_eq!(direct.len(), 8);
                assert_eq!(prepared, direct);
            }
            assert_eq!(direct_rows, 4096 * 512);
            assert!(indexed_rows < direct_rows / 8);
            let wanted = identity(7);
            let colliding = GlobalClaimEvents {
                events: &events,
                identities: SortedRows::new(
                    (0..events.len()).map(|ordinal| (key_hash(wanted), ordinal)),
                ),
            };
            assert_eq!(
                colliding
                    .at_identity(wanted)
                    .filter(|event| event.claim_identity == wanted)
                    .map(|event| event.expression)
                    .collect::<Vec<_>>(),
                indexed
                    .at_identity(wanted)
                    .filter(|event| event.claim_identity == wanted)
                    .map(|event| event.expression)
                    .collect::<Vec<_>>(),
            );
            let mut changed = events.clone();
            changed[7].claim_identity = PermissionClaimIdentity::Unknown;
            let next = GlobalClaimEvents::prepare(&changed);
            assert_eq!(
                next.at_identity(wanted)
                    .filter(|event| event.claim_identity == wanted)
                    .count(),
                7
            );
            assert_eq!(
                indexed
                    .at_identity(wanted)
                    .filter(|event| event.claim_identity == wanted)
                    .count(),
                8
            );
            eprintln!(
                "global identity updates: {} rows prepared in {:?}; direct {} visited rows; prepared {} visited rows",
                events.len(),
                preparation,
                direct_rows,
                indexed_rows
            );

            let a = PermissionProvenance::Established {
                machine_symbol: SymbolHandle::from_parts(1, 1),
                state_symbol: SymbolHandle::from_parts(2, 1),
                source: PermissionEventSource::StateEntry,
            };
            let b = PermissionProvenance::Established {
                state_symbol: SymbolHandle::from_parts(2, 2),
                machine_symbol: SymbolHandle::from_parts(1, 1),
                source: PermissionEventSource::StateEntry,
            };
            let c = PermissionProvenance::Unknown;
            let ia = identity(1);
            let ib = identity(2);
            let ic = identity(3);
            let first = (a, ia, b, ib);
            let conflict = (a, ia, c, ic);
            let tail = (b, ib, c, ic);
            let mut ordered = OrderedOriginRewrites::default();
            let mut direct = Vec::new();
            for row in [first, first, conflict, tail, conflict, tail] {
                ordered.push_unique(row);
                if !direct.contains(&row) {
                    direct.push(row);
                }
            }
            let ordered = ordered.into_rows();
            assert_eq!(ordered, direct);
            assert_eq!(ordered, [first, conflict, tail]);

            fn direct_apply(
                mut provenance: PermissionProvenance,
                mut identity: PermissionClaimIdentity,
                rows: &[ClaimOriginRewrite],
            ) -> (PermissionProvenance, PermissionClaimIdentity, bool) {
                let previous = (provenance, identity);
                for _ in 0..rows.len() {
                    let Some((_, _, next_provenance, next_identity)) = rows
                        .iter()
                        .find(|row| row.0 == provenance && row.1 == identity)
                    else {
                        break;
                    };
                    if *next_provenance == provenance && *next_identity == identity {
                        break;
                    }
                    provenance = *next_provenance;
                    identity = *next_identity;
                }
                (provenance, identity, previous != (provenance, identity))
            }
            let cases: Vec<Vec<ClaimOriginRewrite>> = vec![
                vec![],
                ordered.clone(),
                vec![conflict, first, tail],
                vec![(a, ia, a, ia), first],
                vec![first, (b, ib, a, ia)],
                // The third conflicting row changes the original chain bound:
                // three steps end at b, not a; distinct sources must not be counted.
                vec![first, (b, ib, a, ia), conflict],
                vec![(b, ia, c, ic), (a, ib, c, ic), first],
            ];
            for rows in cases {
                let prepared = OriginRewrites::prepare(&rows);
                let collided = OriginRewrites {
                    rows: &rows,
                    sources: SortedRows::new(
                        (0..rows.len()).map(|ordinal| (key_hash((a, ia)), ordinal)),
                    ),
                };
                assert_eq!(prepared.len(), rows.len());
                assert_eq!(
                    prepared.first_for_source(a, ia),
                    rows.iter().find(|row| row.0 == a && row.1 == ia)
                );
                assert_eq!(
                    collided.first_for_source(a, ia),
                    prepared.first_for_source(a, ia)
                );
                for (provenance, identity) in [(a, ia), (b, ib), (c, ic)] {
                    let expected = direct_apply(provenance, identity, &rows);
                    let (mut actual_provenance, mut actual_identity) = (provenance, identity);
                    let changed = super::super::rewrite_claim_origin(
                        &mut actual_provenance,
                        &mut actual_identity,
                        &prepared,
                    );
                    assert_eq!((actual_provenance, actual_identity, changed), expected);
                }
            }
            let current_rows = [conflict];
            let current = OriginRewrites::prepare(&current_rows);
            assert_eq!(current.first_for_source(a, ia), Some(&conflict));
        }

        #[test]
        fn colliding_event_keys_keep_original_order_and_exact_filtering() {
            let (mut events, segments) = rows(80);
            let mut duplicate = events[18].clone();
            duplicate.expression = typed_trees::expression::ExpressionHandle::from_parts(90001, 7);
            events.push(duplicate);
            let mut prepared = PermissionEvents::prepare(&events, &segments);
            let selected = &events[17];
            let statement =
                permission_event_statement_index(selected.source).expect("statement fixture");
            // All rows deliberately occupy the requested hash bucket. The full
            // original predicate still distinguishes their states and statements.
            prepared
                .associations
                .as_mut()
                .expect("prepared fixture")
                .statements = SortedRows::new(
                (0..events.len())
                    .rev()
                    .map(|ordinal| (key_hash((selected.state_symbol, statement)), ordinal)),
            );
            let actual = prepared
                .at_statement(selected.state_symbol, statement)
                .filter(|event| {
                    event.state_symbol == selected.state_symbol
                        && permission_event_statement_index(event.source) == Some(statement)
                })
                .collect::<Vec<_>>();
            let expected = events
                .iter()
                .filter(|event| {
                    event.state_symbol == selected.state_symbol
                        && permission_event_statement_index(event.source) == Some(statement)
                })
                .collect::<Vec<_>>();
            assert_eq!(actual, expected);
            assert_eq!(actual.first().copied(), expected.first().copied());
            let path = segments.span_or_empty(selected.segments);
            prepared
                .associations
                .as_mut()
                .expect("prepared fixture")
                .places_across_statements =
                SortedRows::new((0..events.len()).rev().map(|ordinal| {
                    (
                        key_hash((selected.state_symbol, selected.root, path)),
                        ordinal,
                    )
                }));
            let same_place = |event: &&FlowPermissionEventFact| {
                event.state_symbol == selected.state_symbol
                    && event.root == selected.root
                    && segments.span_or_empty(event.segments) == path
                    && permission_event_statement_index(event.source) == Some(statement)
            };
            let actual = prepared
                .at_place_across_statements(selected.state_symbol, selected.root, path)
                .filter(same_place)
                .collect::<Vec<_>>();
            let expected = events.iter().filter(same_place).collect::<Vec<_>>();
            assert_eq!(actual, expected);
            let selected = &events[18];
            prepared
                .associations
                .as_mut()
                .expect("prepared fixture")
                .claim_identities = SortedRows::new((0..events.len()).rev().map(|ordinal| {
                (
                    key_hash((selected.state_symbol, selected.claim_identity)),
                    ordinal,
                )
            }));
            let actual = claim_definitions(
                prepared.at_claim_identity(selected.state_symbol, selected.claim_identity),
                selected.state_symbol,
                selected.claim_identity,
            );
            let expected = claim_definitions(
                events.iter(),
                selected.state_symbol,
                selected.claim_identity,
            );
            assert_eq!(actual, expected);
            assert_eq!(actual.len(), 2);
        }
    }
}
use event_queries::{
    ClaimOriginRewrite, GlobalClaimEvents, OrderedOriginRewrites, OriginRewrites, PermissionEvents,
};

use crate::flow::literal_expression_projections;
use crate::shared::collections::push_unique;
use crate::shared::collections::unique_in_order;
pub(super) use joins::InactiveClaimQueries;
pub(crate) use joins::{CallClaimJoinFailure, publish_call_claim_joins, validate_call_claim_joins};

use crate::checks::multiplicity::linear_obligations::{
    CheckedClaimOutcomeEntry, CheckedClaimOutcomeMap, CheckedClaimOutcomeSource,
};
use crate::checks::multiplicity::linear_validation::{
    established_provenance, permission_event_statement_index,
};
use crate::checks::multiplicity::type_multiplicity::data_field_name;
use crate::flow::expressions;
use crate::published::places::place_at;
use arena::HandleSpan;
use checked_trees::{
    FlowClaimOutcomeEntryFact, FlowClaimOutcomeMapFact, FlowClaimOutcomeSource,
    FlowPermissionEventFact,
};
use language_semantics::{
    PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
    PermissionProvenance,
};
use symbols::SymbolHandle;
use typed_trees::statement::StatementNode;

/*
    Invocation correspondence is an input to permission preparation, not a
    correction to custody after cleanup has already observed it. Only a changed
    summary actually read by recording schedules another preparation iteration.
*/
#[derive(Clone, Default)]
pub(crate) struct ClaimOutcomePreparation {
    /* The walk reads these only through `ClaimOutcomeMapLookup`. */
    maps: Vec<CheckedClaimOutcomeMap>,
    dependencies: Vec<SymbolHandle>,
    /*
        Every map the current state's walk looked up, directly or beneath
        `call_result_origin_rewrite`, in first-read order.
    */
    map_reads: Vec<SymbolHandle>,
    joins: joins::CallJoinPreparation,
    revisions: usize,
    revision_limit: usize,
}

impl ClaimOutcomePreparation {
    pub(crate) fn begin(&mut self) {
        self.dependencies.clear();
        self.map_reads.clear();
        self.joins.begin();
    }

    /*
        A copy for one worker's run of states. `resolve` reads only the
        previous iteration's maps and join bindings and appends what it
        consulted, so the copy carries those and starts with nothing consulted.
    */
    pub(crate) fn fork_for_walk(&self) -> Self {
        Self {
            maps: self.maps.clone(),
            dependencies: Vec::new(),
            map_reads: Vec::new(),
            joins: self.joins.fork_for_walk(),
            revisions: self.revisions,
            revision_limit: self.revision_limit,
        }
    }

    /*
        Hand over what this worker consulted since the last call, as a value
        `absorb_walk` takes, and keep the summaries for its next state.
    */
    pub(crate) fn take_consulted(&mut self) -> Self {
        Self {
            maps: Vec::new(),
            dependencies: std::mem::take(&mut self.dependencies),
            map_reads: std::mem::take(&mut self.map_reads),
            joins: self.joins.take_consulted(),
            revisions: self.revisions,
            revision_limit: self.revision_limit,
        }
    }

    /*
        Whether every summary one state's walk read under `previous` reads
        the same here: each claim-outcome map and the join binding of each
        receiver in `consulted` (what `take_consulted` handed over after that
        walk). A missing map or binding compares like any other value.
    */
    pub(crate) fn reads_unchanged(&self, previous: &Self, consulted: &Self) -> bool {
        consulted.map_reads.iter().all(|state| {
            self.maps.iter().find(|map| map.state_symbol == *state)
                == previous.maps.iter().find(|map| map.state_symbol == *state)
        }) && self
            .joins
            .receivers_unchanged(&previous.joins, &consulted.joins)
    }

    /*
        Record what a worker consulted after what is recorded here, in the
        first-consulted order a single walk over the same states would keep.
    */
    pub(crate) fn absorb_walk(&mut self, fork: Self) {
        let Self {
            maps: _,
            dependencies,
            map_reads: _,
            joins,
            revisions: _,
            revision_limit: _,
        } = fork;
        for dependency in dependencies {
            push_unique(&mut self.dependencies, dependency);
        }
        self.joins.absorb_walk(joins);
    }

    pub(crate) fn update(
        &mut self,
        maps: Vec<CheckedClaimOutcomeMap>,
        joins: Vec<joins::PreparedCallClaimJoin>,
    ) -> Result<bool, &'static str> {
        let dependency_count = maps.len().saturating_add(joins.len());
        self.revision_limit = self
            .revision_limit
            .max(dependency_count.saturating_mul(2).saturating_add(2));
        let maps_changed = self.dependencies.iter().any(|state| {
            self.maps.iter().find(|map| map.state_symbol == *state)
                != maps.iter().find(|map| map.state_symbol == *state)
        });
        let joins_changed = self.joins.update(joins);
        let changed = maps_changed || joins_changed;
        if changed {
            self.revisions += 1;
            if self.revisions > self.revision_limit {
                return Err("call-result custody preparation did not converge");
            }
        }
        self.maps = maps;
        Ok(changed)
    }

    pub(crate) fn resolve(
        &mut self,
        program: &dyn validation::CheckingInput,
        segments: &arena::PagedArenaSet<facts::PlaceSegment>,
        event: &mut FlowPermissionEventFact,
        permission_events: &[FlowPermissionEventFact],
    ) {
        let Some((_, expression, path)) = receiving_value(program, segments, event) else {
            return;
        };
        let Some(call) = program.expression_table.call_node(expression) else {
            return;
        };
        let Some(target) = validation::call_target_state(program, call.target_symbol) else {
            return;
        };
        push_unique(&mut self.dependencies, target.symbol);
        let mut maps = ClaimOutcomeMapLookup::recorded(&self.maps, &mut self.map_reads);
        if let Some(map) = maps.find(target.symbol)
            && !map.entries.iter().any(|entry| entry.output_path == path)
        {
            event.obligation_live = false;
            event.claim_identity = PermissionClaimIdentity::Unknown;
            return;
        }
        let permission_events = PermissionEvents::recorded(permission_events);
        if let Some((_, _, provenance, identity)) =
            call_result_origin_rewrite(program, segments, event, &permission_events, &mut maps)
        {
            event.provenance = provenance;
            event.claim_identity = identity;
            return;
        }
        self.joins.resolve(segments, expression, event);
    }
}

/*
    The claim-outcome maps one lookup path may read, and where it notes each
    map it reads. `ClaimOutcomePreparation::resolve` notes into the walking
    worker's own preparation: a state may skip its next walk only if none of
    the maps it read changed, and the source resolution beneath `resolve` also
    looks up maps of other calls in the same state. Deriving the maps
    themselves reads them unrecorded. Every lookup goes through `find`, so no
    read on a recorded path goes unnoted.
*/
pub(crate) struct ClaimOutcomeMapLookup<'maps, 'reads> {
    maps: &'maps [CheckedClaimOutcomeMap],
    reads: Option<&'reads mut Vec<SymbolHandle>>,
}

impl<'maps, 'reads> ClaimOutcomeMapLookup<'maps, 'reads> {
    pub(crate) fn unrecorded(maps: &'maps [CheckedClaimOutcomeMap]) -> Self {
        Self { maps, reads: None }
    }

    fn recorded(
        maps: &'maps [CheckedClaimOutcomeMap],
        reads: &'reads mut Vec<SymbolHandle>,
    ) -> Self {
        Self {
            maps,
            reads: Some(reads),
        }
    }

    fn find(&mut self, state: SymbolHandle) -> Option<&'maps CheckedClaimOutcomeMap> {
        if let Some(reads) = self.reads.as_deref_mut() {
            push_unique(reads, state);
        }
        self.maps.iter().find(|map| map.state_symbol == state)
    }
}

pub(crate) fn derive_checked_claim_outcome_maps(
    program: &dyn ::validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    ownership: &checked_trees::FlowOwnershipFacts,
    permission_events: &[FlowPermissionEventFact],
) -> Result<Vec<CheckedClaimOutcomeMap>, &'static str> {
    let candidate_states = program
        .machines()
        .iter()
        .flat_map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .filter_map(move |state| {
                    let declared = validation::declared_claim_frontier(program, state.return_type);
                    (!declared.claims().is_empty()).then_some((machine.symbol, state, declared))
                })
        })
        .collect::<Vec<_>>();
    if candidate_states.is_empty() {
        return Ok(Vec::new());
    }
    let permission_events = &PermissionEvents::prepare(permission_events, &ownership.segments);
    let state_count = candidate_states.len();
    let derive = |maps: &[CheckedClaimOutcomeMap]| {
        candidate_states
            .iter()
            .filter_map(|(machine_symbol, state, declared)| {
                derive_claim_outcome_hypothesis(
                    program,
                    borrow,
                    *machine_symbol,
                    state,
                    declared,
                    ownership,
                    permission_events,
                    maps,
                )
            })
            .collect::<Vec<_>>()
    };
    let mut maps = Vec::new();
    for _ in 0..=state_count {
        let mut next = derive(&maps)
            .into_iter()
            .map(|hypothesis| hypothesis.map)
            .collect::<Vec<_>>();
        for previous in &maps {
            if !next
                .iter()
                .any(|map| map.state_symbol == previous.state_symbol)
            {
                next.push(previous.clone());
            }
        }
        next.sort_by_key(|map| {
            candidate_states
                .iter()
                .position(|(_, state, _)| state.symbol == map.state_symbol)
        });
        if next == maps {
            loop {
                let supported = derive(&maps)
                    .into_iter()
                    .filter(|hypothesis| {
                        hypothesis.all_returns_covered && maps.contains(&hypothesis.map)
                    })
                    .map(|hypothesis| hypothesis.map)
                    .collect::<Vec<_>>();
                if supported.len() == maps.len() {
                    return Ok(supported);
                }
                maps = supported;
            }
        }
        maps = next;
    }
    Err("claim outcome equations did not converge")
}

/*
    Private fixed-point hypotheses are not permission or lineage evidence.
    Normal-return coverage is checked again after removing unsupported maps.
*/
struct ClaimOutcomeHypothesis {
    map: CheckedClaimOutcomeMap,
    all_returns_covered: bool,
}

#[allow(clippy::too_many_arguments)]
fn derive_claim_outcome_hypothesis(
    program: &dyn ::validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    machine_symbol: SymbolHandle,
    state: &typed_trees::state::State,
    declared: &validation::DeclaredClaimFrontier,
    ownership: &checked_trees::FlowOwnershipFacts,
    permission_events: &PermissionEvents<'_>,
    known_maps: &[CheckedClaimOutcomeMap],
) -> Option<ClaimOutcomeHypothesis> {
    let segments = &ownership.segments;
    if declared.claims().is_empty() {
        return None;
    }
    let absence = ClaimAbsence {
        program,
        state,
        ownership,
        events: permission_events,
        maps: known_maps,
        writes: joins::InactivePlaceWrites::prepare(program, borrow, state),
    };
    let statements = typed_trees::statement::reachable_state_statements(
        program.statement_table.statements(state.statement_nodes),
    );
    let result_expressions = state_result_expressions(program, statements);
    let named_transitions = state_result_named_transitions(program, statements);
    let return_entries = result_expressions
        .iter()
        .copied()
        .map(|(statement_index, expression)| {
            claim_outcomes_for_expression(
                program,
                state,
                statement_index,
                expression,
                ownership,
                permission_events,
                known_maps,
                &[],
            )
        })
        .chain(named_transitions.iter().copied().map(
            |(statement_index, target_symbol, arguments)| {
                claim_outcomes_for_named_transition(
                    program,
                    state,
                    statement_index,
                    target_symbol,
                    arguments,
                    segments,
                    permission_events,
                    known_maps,
                )
            },
        ))
        .collect::<Vec<_>>();
    let mut entries = outcome_tables::unique_entries(return_entries.iter().flatten().cloned());
    entries.retain(|entry| declared.positions(&entry.output_path).next().is_some());
    {
        let current = outcome_tables::Entries::new(&entries);
        for claim in declared.claims() {
            let path = &claim.path;
            let mut sources = current.at_path(path).map(|entry| &entry.source);
            match sources.next() {
                Some(first) if sources.any(|other| other != first) => return None,
                Some(_) => {}
                None => {
                    if !claim_path_is_statically_inactive(
                        &absence,
                        path,
                        &result_expressions,
                        &named_transitions,
                    ) {
                        return None;
                    }
                }
            }
        }
    }
    entries.retain(|entry| declared.positions(&entry.output_path).count() == 1);
    entries.sort_by_key(|entry| {
        declared
            .positions(&entry.output_path)
            .next()
            .unwrap_or(usize::MAX)
    });
    let current = outcome_tables::Entries::new(&entries);
    let inactive_paths = declared
        .claims()
        .iter()
        .filter(|claim| current.at_path(&claim.path).next().is_none())
        .map(|claim| claim.path.clone())
        .collect::<Vec<_>>();
    if !joins::complete_declared_frontier(
        declared,
        None,
        &current,
        &outcome_tables::InactivePaths::new(&inactive_paths),
    ) {
        return None;
    }
    let all_returns_covered = !return_entries.is_empty()
        && return_entries
            .iter()
            .enumerate()
            .all(|(return_index, returned)| {
                let (expressions, transitions) = if return_index < result_expressions.len() {
                    (
                        &result_expressions[return_index..return_index + 1],
                        &named_transitions[0..0],
                    )
                } else {
                    let transition_index = return_index - result_expressions.len();
                    (
                        &result_expressions[0..0],
                        &named_transitions[transition_index..transition_index + 1],
                    )
                };
                let returned = outcome_tables::Entries::new(returned);
                let absent = declared
                    .claims()
                    .iter()
                    .filter(|claim| {
                        returned.at_path(&claim.path).next().is_none()
                            && claim_path_is_statically_inactive(
                                &absence,
                                &claim.path,
                                expressions,
                                transitions,
                            )
                    })
                    .map(|claim| claim.path.clone())
                    .collect::<Vec<_>>();
                joins::complete_declared_frontier(
                    declared,
                    None,
                    &returned,
                    &outcome_tables::InactivePaths::new(&absent),
                ) && returned.rows().iter().all(|entry| current.contains(entry))
            });
    Some(ClaimOutcomeHypothesis {
        map: CheckedClaimOutcomeMap {
            machine_symbol,
            state_symbol: state.symbol,
            entries,
        },
        all_returns_covered,
    })
}

pub(crate) fn claim_paths_are_case_alternatives(
    left: &[facts::PlaceSegment],
    right: &[facts::PlaceSegment],
) -> bool {
    left.iter()
        .zip(right)
        .find_map(|(left, right)| {
            if left == right {
                return None;
            }
            Some(matches!(
                (left, right),
                (
                    facts::PlaceSegment::Case {
                        variant: left_variant
                    },
                    facts::PlaceSegment::Case {
                        variant: right_variant
                    }
                ) if left_variant != right_variant
            ))
        })
        .unwrap_or(false)
}

fn claim_path_is_statically_inactive(
    absence: &ClaimAbsence<'_>,
    path: &[facts::PlaceSegment],
    result_expressions: &[(usize, typed_trees::expression::ExpressionHandle)],
    named_transitions: &[(
        usize,
        SymbolHandle,
        HandleSpan<typed_trees::expression::ExpressionHandle>,
    )],
) -> bool {
    if (result_expressions.is_empty() && named_transitions.is_empty())
        || !path
            .iter()
            .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. }))
    {
        return false;
    }
    result_expressions.iter().all(|(statement, expression)| {
        absence.expression(*statement, *expression, path, absence.state.return_type)
    }) && named_transitions
        .iter()
        .all(|(statement, target, arguments)| {
            absence.arguments(
                *statement,
                *target,
                absence
                    .program
                    .statement_table
                    .expression_handles(*arguments),
                path,
            )
        })
}

/*
    Source-owned exclusion of a conditional claim. Complete outcome maps and
    preserved inactive storage are evidence; a failed identity lookup is not.
*/
struct ClaimAbsence<'a> {
    program: &'a dyn ::validation::CheckingInput,
    state: &'a typed_trees::state::State,
    ownership: &'a checked_trees::FlowOwnershipFacts,
    events: &'a PermissionEvents<'a>,
    maps: &'a [CheckedClaimOutcomeMap],
    writes: Option<joins::InactivePlaceWrites>,
}

impl ClaimAbsence<'_> {
    fn expression(
        &self,
        statement: usize,
        expression: typed_trees::expression::ExpressionHandle,
        path: &[facts::PlaceSegment],
        expected: typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        if let Some(mut place) = place_at(self.program, self.state.symbol, statement, expression) {
            place.segments.extend_from_slice(path);
            if self
                .writes
                .as_ref()
                .and_then(|writes| {
                    writes.preserved(
                        self.program,
                        self.state,
                        statement,
                        &place,
                        self.ownership,
                        self.events.at_place_across_statements(
                            self.state.symbol,
                            place.root,
                            &place.segments,
                        ),
                    )
                })
                .is_some()
            {
                return true;
            }
        }
        match self.program.expression_table.expression(expression) {
            typed_trees::expression::ExpressionNode::StructLiteral(literal) => {
                let mut remaining = path;
                if literal.case_name.is_some() {
                    let Some(facts::PlaceSegment::Case { variant }) = remaining.first() else {
                        return false;
                    };
                    let Some(selected) = literal.case_symbol.and_then(|selected| {
                        self.program
                            .closed_data_constructor_case(expected, selected)
                    }) else {
                        return false;
                    };
                    if self
                        .program
                        .closed_data_constructor_case(expected, *variant)
                        .is_none()
                    {
                        return false;
                    }
                    if selected.symbol != *variant {
                        return true;
                    }
                    remaining = &remaining[1..];
                }
                let Some(facts::PlaceSegment::Field { symbol }) = remaining.first() else {
                    return false;
                };
                let Some(field_name) = data_field_name(self.program, *symbol) else {
                    return false;
                };
                expressions(self.program)
                    .struct_fields(literal.fields)
                    .iter()
                    .find(|field| field.name.as_str() == field_name)
                    .is_some_and(|field| {
                        let consumed = path.len() - remaining.len() + 1;
                        output_path_type(
                            self.program,
                            self.state,
                            statement,
                            expected,
                            &path[..consumed],
                        )
                        .is_some_and(|expected| {
                            self.expression(statement, field.value, &remaining[1..], expected)
                        })
                    })
            }
            typed_trees::expression::ExpressionNode::ArrayLiteral(values) => {
                let Some(facts::PlaceSegment::FixedIndex { index }) = path.first() else {
                    return false;
                };
                expressions(self.program)
                    .expression_handles(*values)
                    .get(*index)
                    .is_some_and(|value| {
                        output_path_type(self.program, self.state, statement, expected, &path[..1])
                            .is_some_and(|expected| {
                                self.expression(statement, *value, &path[1..], expected)
                            })
                    })
            }
            typed_trees::expression::ExpressionNode::Call(call) => self.arguments(
                statement,
                call.target_symbol,
                expressions(self.program).expression_handles(call.arguments),
                path,
            ),
            _ => false,
        }
    }

    fn arguments(
        &self,
        statement: usize,
        target_symbol: SymbolHandle,
        arguments: &[typed_trees::expression::ExpressionHandle],
        path: &[facts::PlaceSegment],
    ) -> bool {
        let Some(target) = validation::call_target_state(self.program, target_symbol) else {
            return false;
        };
        let mut maps = self
            .maps
            .iter()
            .filter(|map| map.state_symbol == target.symbol);
        let Some(map) = maps.next() else {
            return false;
        };
        if maps.next().is_some() {
            return false;
        }
        let mut entries = map.entries.iter().filter(|entry| entry.output_path == path);
        let Some(entry) = entries.next() else {
            return path
                .iter()
                .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. }));
        };
        if entries.next().is_some() {
            return false;
        }
        let CheckedClaimOutcomeSource::Input {
            parameter_symbol,
            path: input_path,
        } = &entry.source
        else {
            return false;
        };
        let Some(argument) =
            argument_for_parameter(self.program, arguments, target, *parameter_symbol)
        else {
            return false;
        };
        let Some(parameter) = parameter_for_symbol(self.program, target, *parameter_symbol) else {
            return false;
        };
        self.expression(statement, argument, input_path, parameter.type_reference)
    }
}

fn state_result_expressions(
    program: &dyn ::validation::CheckingInput,
    statements: &[StatementNode],
) -> Vec<(usize, typed_trees::expression::ExpressionHandle)> {
    statements
        .iter()
        .enumerate()
        .flat_map(|(statement_index, statement)| match statement {
            StatementNode::Expression(expression) if statement_index + 1 == statements.len() => {
                vec![(statement_index, *expression)]
            }
            StatementNode::Transition(transition) => [transition.target, transition.continuation]
                .into_iter()
                .filter(|handle| handle.is_valid())
                .filter_map(|handle| {
                    let typed_trees::statement::TransitionTargetNode::Value(expression) =
                        program.statement_table.transition_target(handle)
                    else {
                        return None;
                    };
                    Some((statement_index, *expression))
                })
                .collect(),
            _ => Vec::new(),
        })
        .collect()
}

fn state_result_named_transitions(
    program: &dyn ::validation::CheckingInput,
    statements: &[StatementNode],
) -> Vec<(
    usize,
    SymbolHandle,
    HandleSpan<typed_trees::expression::ExpressionHandle>,
)> {
    statements
        .iter()
        .enumerate()
        .flat_map(|(statement_index, statement)| {
            let Some(transition) = statement.as_transition() else {
                return Vec::new();
            };
            [transition.target, transition.continuation]
                .into_iter()
                .filter(|handle| handle.is_valid())
                .filter_map(|handle| {
                    let typed_trees::statement::TransitionTargetNode::Named {
                        path, arguments, ..
                    } = program.statement_table.transition_target(handle)
                    else {
                        return None;
                    };
                    if validation::call_target_state(program, path.symbol).is_some_and(|target| {
                        validation::normal_completion_possible(program, target) == Some(false)
                    }) {
                        return None;
                    }
                    Some((statement_index, path.symbol, *arguments))
                })
                .collect()
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn claim_outcomes_for_named_transition(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    target_symbol: SymbolHandle,
    arguments: HandleSpan<typed_trees::expression::ExpressionHandle>,
    segments: &arena::PagedArenaSet<facts::PlaceSegment>,
    permission_events: &PermissionEvents<'_>,
    known_maps: &[CheckedClaimOutcomeMap],
) -> Vec<CheckedClaimOutcomeEntry> {
    let Some(target_state) = validation::call_target_state(program, target_symbol) else {
        return Vec::new();
    };
    let Some(target_map) = known_maps
        .iter()
        .find(|map| map.state_symbol == target_state.symbol)
    else {
        return Vec::new();
    };
    let arguments = program.statement_table.expression_handles(arguments);
    target_map
        .entries
        .iter()
        .filter_map(|entry| {
            let source = bind_claim_outcome_source_at_arguments(
                program,
                state,
                statement_index,
                arguments,
                target_state,
                &entry.source,
                segments,
                permission_events,
                &mut ClaimOutcomeMapLookup::unrecorded(known_maps),
            )?;
            Some(CheckedClaimOutcomeEntry {
                output_path: entry.output_path.clone(),
                source,
            })
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn claim_outcomes_for_expression(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
    ownership: &checked_trees::FlowOwnershipFacts,
    permission_events: &PermissionEvents<'_>,
    known_maps: &[CheckedClaimOutcomeMap],
    output_prefix: &[facts::PlaceSegment],
) -> Vec<CheckedClaimOutcomeEntry> {
    macro_rules! claim_child {
        ($expression:expr, $output_prefix:expr $(,)?) => {
            claim_outcomes_for_expression(
                program,
                state,
                statement_index,
                $expression,
                ownership,
                permission_events,
                known_maps,
                $output_prefix,
            )
        };
    }
    let segments = &ownership.segments;
    // The selection retains each arm's consumed frontier. Read that evidence
    // before the joined home, whose provenance can hide distinct arm origins.
    if ownership
        .owned_selection_for_expression(state.symbol, statement_index as u32, expression)
        .is_some()
    {
        return claim_outcomes_for_owned_selection(
            program,
            state,
            statement_index,
            expression,
            ownership,
            permission_events,
            output_prefix,
        );
    }
    if !matches!(
        program.expression_table.expression(expression),
        typed_trees::expression::ExpressionNode::Call(_)
    ) && let Some(place) = place_at(program, state.symbol, statement_index, expression)
    {
        let found = permission_events
            .at_statement(state.symbol, statement_index)
            .filter(|event| {
                event.state_symbol == state.symbol
                    && permission_event_statement_index(event.source) == Some(statement_index)
                    && event.kind == PermissionEventKind::Transfer
                    && event.access == PermissionAccess::Owned
                    && event.obligation_live
                    && event.root == place.root
            })
            .filter_map(|event| {
                let event_path = segments.span_or_empty(event.segments);
                if !event_path.starts_with(place.segments.as_slice()) {
                    return None;
                }
                let mut output_path = output_prefix.to_vec();
                output_path.extend_from_slice(&event_path[place.segments.len()..]);
                Some(CheckedClaimOutcomeEntry {
                    output_path,
                    source: resolve_claim_outcome_source(
                        program,
                        state,
                        claim_outcome_source_for_event(
                            program,
                            state,
                            event,
                            segments,
                            permission_events,
                        ),
                        segments,
                        permission_events,
                        &mut ClaimOutcomeMapLookup::unrecorded(known_maps),
                    )?,
                })
            })
            .collect::<Vec<_>>();
        if !found.is_empty() {
            return found;
        }
    }

    match program.expression_table.expression(expression) {
        typed_trees::expression::ExpressionNode::ArrayLiteral(values) => expressions(program)
            .expression_handles(*values)
            .iter()
            .enumerate()
            .flat_map(|(index, value)| {
                let mut element_prefix = output_prefix.to_vec();
                element_prefix.push(facts::PlaceSegment::FixedIndex { index });
                claim_child!(*value, &element_prefix)
            })
            .collect(),
        typed_trees::expression::ExpressionNode::StructLiteral(literal)
            if literal.case_name.is_some() =>
        {
            let Some(variant) = output_path_type(
                program,
                state,
                statement_index,
                state.return_type,
                output_prefix,
            )
            .and_then(|reference| {
                program.closed_data_constructor_case(reference, literal.case_symbol?)
            }) else {
                return Vec::new();
            };
            let mut case_prefix = output_prefix.to_vec();
            case_prefix.push(facts::PlaceSegment::Case {
                variant: variant.symbol,
            });
            program
                .data_payload_fields(variant)
                .iter()
                .filter_map(|field| {
                    let value = expressions(program)
                        .struct_fields(literal.fields)
                        .iter()
                        .find(|literal_field| literal_field.name == field.name)?
                        .value;
                    let mut field_prefix = case_prefix.clone();
                    field_prefix.push(facts::PlaceSegment::Field {
                        symbol: field.symbol,
                    });
                    Some(claim_child!(value, &field_prefix))
                })
                .flatten()
                .collect()
        }
        typed_trees::expression::ExpressionNode::StructLiteral(literal)
            if literal.case_name.is_none() =>
        {
            let Some(definition) = output_path_type(
                program,
                state,
                statement_index,
                state.return_type,
                output_prefix,
            )
            .and_then(|reference| {
                program.closed_data_constructor_definition(reference, literal.type_symbol)
            }) else {
                return Vec::new();
            };
            expressions(program)
                .struct_fields(literal.fields)
                .iter()
                .flat_map(|literal_field| {
                    let Some(field) =
                        program
                            .data_members(definition)
                            .iter()
                            .find_map(|member| match member {
                                typed_trees::data::DataMember::Field(field)
                                    if field.name.as_str() == literal_field.name.as_str() =>
                                {
                                    Some(field)
                                }
                                _ => None,
                            })
                    else {
                        return Vec::new();
                    };
                    let mut field_prefix = output_prefix.to_vec();
                    field_prefix.push(facts::PlaceSegment::Field {
                        symbol: field.symbol,
                    });
                    claim_child!(literal_field.value, &field_prefix)
                })
                .collect()
        }
        typed_trees::expression::ExpressionNode::Call(call) => {
            let Some(target_state) = validation::call_target_state(program, call.target_symbol)
            else {
                return Vec::new();
            };
            let Some(target_map) = known_maps
                .iter()
                .find(|map| map.state_symbol == target_state.symbol)
            else {
                return Vec::new();
            };
            target_map
                .entries
                .iter()
                .filter_map(|entry| {
                    let source = bind_claim_outcome_source_at_call(
                        program,
                        state,
                        statement_index,
                        call,
                        target_state,
                        &entry.source,
                        segments,
                        permission_events,
                        &mut ClaimOutcomeMapLookup::unrecorded(known_maps),
                    )?;
                    let mut output_path = output_prefix.to_vec();
                    output_path.extend_from_slice(&entry.output_path);
                    Some(CheckedClaimOutcomeEntry {
                        output_path,
                        source,
                    })
                })
                .collect()
        }
        typed_trees::expression::ExpressionNode::Cast(cast)
            if validation::value_preserving_cast_operand(
                program,
                state.symbol,
                statement_index,
                expression,
            )
            .is_some() =>
        {
            claim_child!(cast.value, output_prefix)
        }
        _ => Vec::new(),
    }
}

/*
    Resolve the receiving namespace before interpreting a constructor. A shared
    generic body retains template selectors while each result owns its closed
    case and field identities; display names cannot join those namespaces.
*/
fn output_path_type(
    program: &dyn validation::CheckingInput,
    state: &typed_trees::state::State,
    statement: usize,
    root_type: typed_trees::types::TypeReferenceHandle,
    path: &[facts::PlaceSegment],
) -> Option<typed_trees::types::TypeReferenceHandle> {
    validation::canonical_place_type_reference(
        program,
        state.symbol,
        statement,
        &validation::CanonicalPlace {
            root: facts::PlaceRoot::TypeReference(root_type),
            segments: path.to_vec(),
        },
    )
}

/*
    A receipt'd selection produces its result claims at the join edge: each
    transfer names the consumed source claims that become the result's claims
    on that edge, and uniform linear consumption means every edge maps the
    result to the same source places. A whole affine carrier contributes one
    entry per frontier claim, landing at the claim's position below the moved
    leaf. A parameter source binds the caller's claim at its exact input path;
    a local source publishes the established identity the roster recorded. A
    computed owner has no named storage source, but its checked completion still
    records each actual claim. Keep those origins too: omitting a fresh arm
    could make another arm's input appear to be the common source.
*/
fn claim_outcomes_for_owned_selection(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
    ownership: &checked_trees::FlowOwnershipFacts,
    permission_events: &PermissionEvents<'_>,
    output_prefix: &[facts::PlaceSegment],
) -> Vec<CheckedClaimOutcomeEntry> {
    let Some((_, receipt)) =
        ownership.owned_selection_for_expression(state.symbol, statement_index as u32, expression)
    else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    for transfer in ownership
        .selection_transfers
        .span_or_empty(receipt.transfers)
    {
        let leaf_path = ownership.segments.span_or_empty(transfer.path);
        for claim in ownership
            .selection_transfer_claims
            .span_or_empty(transfer.claims)
        {
            if claim.claim_identity == PermissionClaimIdentity::Unknown
                || claim.provenance == PermissionProvenance::Unknown
            {
                // Every active arm participates in the common-origin judgment.
                // An unresolved arm cannot disappear behind another arm's input.
                return Vec::new();
            }
            let claim_path = ownership.segments.span_or_empty(claim.path);
            let Some(suffix) = claim_path.strip_prefix(leaf_path) else {
                return Vec::new();
            };
            let mut output_path = output_prefix.to_vec();
            output_path
                .extend_from_slice(ownership.segments.span_or_empty(transfer.destination_path));
            output_path.extend_from_slice(suffix);
            let source = claim_outcome_source_for_identity(
                program,
                state,
                claim.claim_identity,
                claim.provenance,
                &ownership.segments,
                permission_events,
            );
            let entry = CheckedClaimOutcomeEntry {
                output_path,
                source,
            };
            push_unique(&mut entries, entry);
        }
    }
    if entries.is_empty() {
        entries.push(CheckedClaimOutcomeEntry {
            output_path: output_prefix.to_vec(),
            source: CheckedClaimOutcomeSource::Established {
                claim_identity: PermissionClaimIdentity::Unknown,
                provenance: PermissionProvenance::Unknown,
            },
        });
    }
    entries
}

fn claim_outcome_source_for_event(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    event: &FlowPermissionEventFact,
    segments: &arena::PagedArenaSet<facts::PlaceSegment>,
    permission_events: &PermissionEvents<'_>,
) -> CheckedClaimOutcomeSource {
    claim_outcome_source_for_identity(
        program,
        state,
        event.claim_identity,
        event.provenance,
        segments,
        permission_events,
    )
}

fn claim_outcome_source_for_identity(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    claim_identity: PermissionClaimIdentity,
    provenance: PermissionProvenance,
    segments: &arena::PagedArenaSet<facts::PlaceSegment>,
    permission_events: &PermissionEvents<'_>,
) -> CheckedClaimOutcomeSource {
    if claim_identity != PermissionClaimIdentity::Unknown
        && provenance != PermissionProvenance::Unknown
        && let Some(entry) = permission_events
            .at_origin(state.symbol, claim_identity, provenance)
            .find(|candidate| {
                candidate.state_symbol == state.symbol
                    && candidate.source == PermissionEventSource::StateEntry
                    && candidate.kind == PermissionEventKind::Establish
                    && candidate.access == PermissionAccess::Owned
                    && candidate.claim_identity == claim_identity
                    && candidate.provenance == provenance
            })
        && let Some(parameter_symbol) = entry.root.as_symbol()
        && parameter_for_symbol(program, state, parameter_symbol).is_some()
    {
        return CheckedClaimOutcomeSource::Input {
            parameter_symbol,
            path: segments.span_or_empty(entry.segments).to_vec(),
        };
    }
    CheckedClaimOutcomeSource::Established {
        claim_identity,
        provenance,
    }
}

fn claim_origin_for_source(
    state: &typed_trees::state::State,
    source: &CheckedClaimOutcomeSource,
    segments: &arena::PagedArenaSet<facts::PlaceSegment>,
    permission_events: &PermissionEvents<'_>,
) -> Option<(PermissionProvenance, PermissionClaimIdentity)> {
    match source {
        CheckedClaimOutcomeSource::Established {
            claim_identity,
            provenance,
        } => (*claim_identity != PermissionClaimIdentity::Unknown
            && *provenance != PermissionProvenance::Unknown)
            .then_some((*provenance, *claim_identity)),
        CheckedClaimOutcomeSource::Input {
            parameter_symbol,
            path,
        } => {
            let origins = unique_in_order(
                permission_events
                    .at_place(
                        state.symbol,
                        None,
                        facts::PlaceRoot::Symbol(*parameter_symbol),
                        path,
                    )
                    .filter(|event| {
                        event.state_symbol == state.symbol
                            && event.source == PermissionEventSource::StateEntry
                            && event.kind == PermissionEventKind::Establish
                            && event.access == PermissionAccess::Owned
                            && event.obligation_live
                            && event.root == facts::PlaceRoot::Symbol(*parameter_symbol)
                            && segments.span_or_empty(event.segments) == path
                            && event.claim_identity != PermissionClaimIdentity::Unknown
                            && event.provenance != PermissionProvenance::Unknown
                    })
                    .map(|event| (event.provenance, event.claim_identity)),
            );
            let [origin] = origins.as_slice() else {
                return None;
            };
            Some(*origin)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn bind_claim_outcome_source_at_call(
    program: &dyn ::validation::CheckingInput,
    caller_state: &typed_trees::state::State,
    statement_index: usize,
    call: &typed_trees::expression::TableCallExpression,
    target_state: &typed_trees::state::State,
    source: &CheckedClaimOutcomeSource,
    segments: &arena::PagedArenaSet<facts::PlaceSegment>,
    permission_events: &PermissionEvents<'_>,
    maps: &mut ClaimOutcomeMapLookup<'_, '_>,
) -> Option<CheckedClaimOutcomeSource> {
    bind_claim_outcome_source_at_arguments(
        program,
        caller_state,
        statement_index,
        program.expression_table.expression_handles(call.arguments),
        target_state,
        source,
        segments,
        permission_events,
        maps,
    )
}

#[allow(clippy::too_many_arguments)]
fn bind_claim_outcome_source_at_arguments(
    program: &dyn ::validation::CheckingInput,
    caller_state: &typed_trees::state::State,
    statement_index: usize,
    arguments: &[typed_trees::expression::ExpressionHandle],
    target_state: &typed_trees::state::State,
    source: &CheckedClaimOutcomeSource,
    segments: &arena::PagedArenaSet<facts::PlaceSegment>,
    permission_events: &PermissionEvents<'_>,
    maps: &mut ClaimOutcomeMapLookup<'_, '_>,
) -> Option<CheckedClaimOutcomeSource> {
    let source = bind_claim_outcome_source_at_arguments_raw(
        program,
        caller_state,
        statement_index,
        arguments,
        target_state,
        source,
        segments,
        permission_events,
    )?;
    resolve_claim_outcome_source(
        program,
        caller_state,
        source,
        segments,
        permission_events,
        maps,
    )
}

/*
    A completed helper may forward an input before permission preparation has
    rewritten its receiving identity. Solve that correspondence from the same
    candidate maps as the surrounding return, without modifying observed events.
    Every candidate is still checked against all returns before publication.
*/
fn resolve_claim_outcome_source(
    program: &dyn validation::CheckingInput,
    state: &typed_trees::state::State,
    mut source: CheckedClaimOutcomeSource,
    segments: &arena::PagedArenaSet<facts::PlaceSegment>,
    events: &PermissionEvents<'_>,
    maps: &mut ClaimOutcomeMapLookup<'_, '_>,
) -> Option<CheckedClaimOutcomeSource> {
    let mut visited = Vec::new();
    loop {
        let CheckedClaimOutcomeSource::Established {
            claim_identity,
            provenance,
        } = source
        else {
            return Some(source);
        };
        if visited.contains(&(claim_identity, provenance)) {
            return None;
        }
        visited.push((claim_identity, provenance));
        let mut forwarded = Vec::new();
        for event in events
            .at_origin(state.symbol, claim_identity, provenance)
            .filter(|event| {
                event.state_symbol == state.symbol
                    && event.kind == PermissionEventKind::Establish
                    && event.access == PermissionAccess::Owned
                    && event.obligation_live
                    && event.claim_identity == claim_identity
                    && event.provenance == provenance
            })
        {
            let Some((statement, expression, path)) = receiving_value(program, segments, event)
            else {
                continue;
            };
            let Some(call) = program.expression_table.call_node(expression) else {
                continue;
            };
            let Some(target) = validation::call_target_state(program, call.target_symbol) else {
                continue;
            };
            let Some(map) = maps.find(target.symbol) else {
                continue;
            };
            for entry in map.entries.iter().filter(|entry| {
                entry.output_path == path
                    && matches!(entry.source, CheckedClaimOutcomeSource::Input { .. })
            }) {
                let bound = bind_claim_outcome_source_at_arguments_raw(
                    program,
                    state,
                    statement,
                    program.expression_table.expression_handles(call.arguments),
                    target,
                    &entry.source,
                    segments,
                    events,
                )?;
                if bound == source {
                    continue;
                }
                push_unique(&mut forwarded, bound);
            }
        }
        match forwarded.as_slice() {
            [] => return Some(source),
            [next] => source = next.clone(),
            _ => return None,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn bind_claim_outcome_source_at_arguments_raw(
    program: &dyn ::validation::CheckingInput,
    caller_state: &typed_trees::state::State,
    statement_index: usize,
    arguments: &[typed_trees::expression::ExpressionHandle],
    target_state: &typed_trees::state::State,
    source: &CheckedClaimOutcomeSource,
    segments: &arena::PagedArenaSet<facts::PlaceSegment>,
    permission_events: &PermissionEvents<'_>,
) -> Option<CheckedClaimOutcomeSource> {
    let CheckedClaimOutcomeSource::Input {
        parameter_symbol,
        path,
    } = source
    else {
        return Some(source.clone());
    };
    let argument = argument_for_parameter(program, arguments, target_state, *parameter_symbol)?;
    let parameter = parameter_for_symbol(program, target_state, *parameter_symbol)?;
    let mut projections =
        literal_expression_projections(program, argument, parameter.type_reference, path, false)?;
    let projection = projections.pop()?;
    if !projections.is_empty() {
        return None;
    }
    let mut place = place_at(
        program,
        caller_state.symbol,
        statement_index,
        projection.expression,
    )?;
    place.segments.extend_from_slice(&projection.remaining);
    let sources = unique_in_order(
        permission_events
            .at_place(
                caller_state.symbol,
                Some(statement_index),
                place.root,
                &place.segments,
            )
            .filter(|event| {
                event.state_symbol == caller_state.symbol
                    && permission_event_statement_index(event.source) == Some(statement_index)
                    && event.kind == PermissionEventKind::Transfer
                    && event.access == PermissionAccess::Owned
                    && event.obligation_live
                    && event.root == place.root
                    && segments.span_or_empty(event.segments) == place.segments
            })
            .map(|event| {
                claim_outcome_source_for_event(
                    program,
                    caller_state,
                    event,
                    segments,
                    permission_events,
                )
            }),
    );
    let [source] = sources.as_slice() else {
        return None;
    };
    Some(source.clone())
}

/* Resolve `parameter_symbol` to its authored state parameter. */
fn parameter_for_symbol<'a>(
    program: &'a dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    parameter_symbol: SymbolHandle,
) -> Option<&'a typed_trees::signature::StateParameter> {
    let parameters = program.state_parameters(state);
    parameters
        .iter()
        .find(|parameter| parameter.symbol == parameter_symbol)
}

fn argument_for_parameter(
    program: &dyn ::validation::CheckingInput,
    arguments: &[typed_trees::expression::ExpressionHandle],
    target_state: &typed_trees::state::State,
    parameter_symbol: SymbolHandle,
) -> Option<typed_trees::expression::ExpressionHandle> {
    let parameters = program.state_parameters(target_state);
    let matched = parameter_for_symbol(program, target_state, parameter_symbol)?;
    let position = typed_trees::signature::state_parameter_position(parameters, matched.symbol)?;
    arguments.get(position).copied()
}

pub(crate) use validation::receiving_value;

pub(crate) fn call_result_origin_rewrites(
    program: &dyn ::validation::CheckingInput,
    segments: &arena::PagedArenaSet<facts::PlaceSegment>,
    permission_events: &[FlowPermissionEventFact],
    maps: &[CheckedClaimOutcomeMap],
) -> Vec<(
    PermissionProvenance,
    PermissionClaimIdentity,
    PermissionProvenance,
    PermissionClaimIdentity,
)> {
    if maps.is_empty() {
        return Vec::new();
    }
    let permission_events = &PermissionEvents::prepare(permission_events, segments);
    permission_events
        .iter()
        .filter_map(|event| {
            call_result_origin_rewrite(
                program,
                segments,
                event,
                permission_events,
                &mut ClaimOutcomeMapLookup::unrecorded(maps),
            )
        })
        .collect()
}

fn call_result_origin_rewrite(
    program: &dyn validation::CheckingInput,
    segments: &arena::PagedArenaSet<facts::PlaceSegment>,
    event: &FlowPermissionEventFact,
    permission_events: &PermissionEvents<'_>,
    maps: &mut ClaimOutcomeMapLookup<'_, '_>,
) -> Option<(
    PermissionProvenance,
    PermissionClaimIdentity,
    PermissionProvenance,
    PermissionClaimIdentity,
)> {
    if event.kind != PermissionEventKind::Establish
        || event.access != PermissionAccess::Owned
        || !event.obligation_live
    {
        return None;
    }
    let (statement_index, result_expression, receiving_path) =
        receiving_value(program, segments, event)?;
    let locally_minted =
        established_provenance(event.machine_symbol, event.state_symbol, event.source);
    if event.provenance != locally_minted
        || event.claim_identity == PermissionClaimIdentity::Unknown
    {
        return None;
    }
    let state = program.state_by_symbol(event.state_symbol)?;
    let typed_trees::expression::ExpressionNode::Call(call) =
        program.expression_table.expression(result_expression)
    else {
        return None;
    };
    let target_state = validation::call_target_state(program, call.target_symbol)?;
    let map = maps.find(target_state.symbol)?;
    let matching = unique_in_order(
        map.entries
            .iter()
            .filter(|entry| entry.output_path == receiving_path)
            .filter(|entry| matches!(entry.source, CheckedClaimOutcomeSource::Input { .. }))
            .filter(|entry| {
                !map.entries.iter().any(|other| {
                    other.source == entry.source && other.output_path != entry.output_path
                })
            })
            .filter_map(|entry| {
                bind_claim_outcome_source_at_call(
                    program,
                    state,
                    statement_index,
                    call,
                    target_state,
                    &entry.source,
                    segments,
                    permission_events,
                    maps,
                )
            }),
    );
    let [source] = matching.as_slice() else {
        return None;
    };
    let (provenance, claim_identity) =
        claim_origin_for_source(state, source, segments, permission_events)?;
    Some((
        locally_minted,
        event.claim_identity,
        provenance,
        claim_identity,
    ))
}

/*
    Canonical origin substitution updates every retained use of the same
    claim pair. A selected completion is an actual consumer too; leaving its
    transfer roster behind would revive pre-join provenance during replay.
*/
pub(crate) fn apply_claim_origin_rewrites(
    permission_events: &mut [FlowPermissionEventFact],
    ownership: &mut checked_trees::FlowOwnershipFacts,
    rewrites: &[(
        PermissionProvenance,
        PermissionClaimIdentity,
        PermissionProvenance,
        PermissionClaimIdentity,
    )],
) -> bool {
    let rewrites = &OriginRewrites::prepare(rewrites);
    let mut changed = false;
    for event in permission_events
        .iter_mut()
        .filter(|event| event.access == PermissionAccess::Owned)
    {
        changed |= rewrite_claim_origin(&mut event.provenance, &mut event.claim_identity, rewrites);
    }
    ownership.selection_transfers.for_each_mut(|_, transfer| {
        changed |= rewrite_claim_origin(
            &mut transfer.provenance,
            &mut transfer.claim_identity,
            rewrites,
        );
    });
    ownership
        .selection_transfer_claims
        .for_each_mut(|_, claim| {
            changed |=
                rewrite_claim_origin(&mut claim.provenance, &mut claim.claim_identity, rewrites);
        });
    changed
}

fn rewrite_claim_origin(
    provenance: &mut PermissionProvenance,
    claim_identity: &mut PermissionClaimIdentity,
    rewrites: &OriginRewrites<'_>,
) -> bool {
    let previous = (*provenance, *claim_identity);
    for _ in 0..rewrites.len() {
        let Some((_, _, replacement_provenance, replacement_identity)) =
            rewrites.first_for_source(*provenance, *claim_identity)
        else {
            break;
        };
        if *replacement_provenance == *provenance && *replacement_identity == *claim_identity {
            break;
        }
        *provenance = *replacement_provenance;
        *claim_identity = *replacement_identity;
    }
    previous != (*provenance, *claim_identity)
}

pub(crate) fn publish_claim_outcome_maps(
    facts: &mut checked_trees::CheckFactsWalk<'_>,
    maps: Vec<CheckedClaimOutcomeMap>,
) {
    facts.flow.ownership.claim_outcome_entries = Default::default();
    facts.flow.ownership.claim_outcome_maps = Default::default();
    for map in maps {
        let mut entries = Vec::new();
        for entry in map.entries {
            let output_segments = facts.flow.ownership.segments.insert_many(entry.output_path);
            let source = match entry.source {
                CheckedClaimOutcomeSource::Input {
                    parameter_symbol,
                    path,
                } => FlowClaimOutcomeSource::Input {
                    parameter_symbol,
                    segments: facts.flow.ownership.segments.insert_many(path),
                },
                CheckedClaimOutcomeSource::Established {
                    claim_identity,
                    provenance,
                } => FlowClaimOutcomeSource::Established {
                    claim_identity,
                    provenance,
                },
            };
            entries.push(FlowClaimOutcomeEntryFact {
                output_segments,
                source,
            });
        }
        let entries = facts
            .flow
            .ownership
            .claim_outcome_entries
            .insert_many(entries);
        facts
            .flow
            .ownership
            .claim_outcome_maps
            .insert(FlowClaimOutcomeMapFact {
                machine_symbol: map.machine_symbol,
                state_symbol: map.state_symbol,
                entries,
            });
    }
}
