//! Close the fixed candidate roster over exact call dependencies, without
//! rebuilding bodies or rechecking unaffected callers after each removal.
use super::{
    CheckFacts, CheckedComposedUnitControlMachinePlan, CheckedPartialAffineUnitCleanupMachinePlan,
    CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan, SymbolHandle, symbol_key,
};
use crate::execution::terminal_unit::plan_gaps::{GapSite, plan_gap};
use crate::execution::terminal_unit::scalar_targets;
use checked_trees::{
    CheckedUnitCallResult, CheckedUnitCallRoute, CheckedUnitCleanupBoundary, CheckedUnitPlanGap,
    CheckedUnitPlanGapReason,
};
use std::collections::BTreeSet;

/// Why one candidate left the roster, and where its body names the reason.
type Cause = (CheckedUnitPlanGapReason, GapSite);

struct CandidateClosure {
    // Symbols are sparse across the complete program; candidate ordinals are
    // dense and private to this invocation. Each machine has at most one body:
    // its shape selected one plan route.
    entries: Vec<(SymbolHandle, SymbolHandle, usize)>,
    retained: Vec<bool>,
    // The first reason each candidate left the roster, kept beside `retained`
    // so the gap names the direct dependency that failed and its call site.
    causes: Vec<Option<Cause>>,
    // (target, caller, index into `sites`): the caller's body depends on the
    // target's at that call site.
    dependents: Vec<(usize, usize, usize)>,
    sites: Vec<GapSite>,
}

impl CandidateClosure {
    fn new(entries: impl Iterator<Item = (SymbolHandle, SymbolHandle)>) -> Self {
        let mut entries = entries
            .enumerate()
            .map(|(candidate_index, (machine, state))| (machine, state, candidate_index))
            .collect::<Vec<_>>();
        entries.sort_unstable_by_key(|(machine, _, _)| symbol_key(*machine));
        Self {
            retained: vec![true; entries.len()],
            causes: vec![None; entries.len()],
            entries,
            dependents: Vec::new(),
            sites: Vec::new(),
        }
    }

    fn drop_candidate(
        &mut self,
        caller_index: usize,
        reason: CheckedUnitPlanGapReason,
        site: GapSite,
    ) {
        if self.retained[caller_index] {
            self.retained[caller_index] = false;
            self.causes[caller_index] = Some((reason, site));
        }
    }

    fn depend(&mut self, target_index: usize, caller_index: usize, site: GapSite) {
        self.dependents
            .push((target_index, caller_index, self.sites.len()));
        self.sites.push(site);
    }

    fn require_entry(
        &mut self,
        caller_index: usize,
        machine: SymbolHandle,
        state: SymbolHandle,
        site: GapSite,
    ) {
        match self
            .entries
            .binary_search_by_key(&symbol_key(machine), |(candidate, _, _)| {
                symbol_key(*candidate)
            }) {
            Ok(entry_index) if self.entries[entry_index].1 == state => {
                let target_index = self.entries[entry_index].2;
                self.depend(target_index, caller_index, site);
            }
            _ => self.drop_candidate(
                caller_index,
                CheckedUnitPlanGapReason::Callee(machine),
                site,
            ),
        }
    }

    fn close(mut self) -> (Vec<bool>, Vec<Option<Cause>>) {
        // Flat reverse edges avoid allocating a child vector for each body.
        // Each unavailable body enters the queue once, including broken cycles.
        let mut unavailable = self
            .retained
            .iter()
            .enumerate()
            .filter_map(|(candidate_index, retained)| (!retained).then_some(candidate_index))
            .collect::<Vec<_>>();
        if unavailable.is_empty() {
            return (self.retained, self.causes);
        }
        // One edge per caller and target, at the first call site that named it.
        self.dependents.sort_unstable();
        self.dependents
            .dedup_by_key(|(target, caller, _)| (*target, *caller));
        let mut queue_position = 0;
        while let Some(&target_index) = unavailable.get(queue_position) {
            queue_position += 1;
            let first = self
                .dependents
                .partition_point(|(target, _, _)| *target < target_index);
            let target = self
                .entries
                .iter()
                .find(|(_, _, candidate_index)| *candidate_index == target_index)
                .map(|(machine, _, _)| *machine)
                .unwrap_or_else(SymbolHandle::invalid);
            for &(_, caller_index, site) in self.dependents[first..]
                .iter()
                .take_while(|(target, _, _)| *target == target_index)
            {
                if self.retained[caller_index] {
                    self.retained[caller_index] = false;
                    self.causes[caller_index] =
                        Some((CheckedUnitPlanGapReason::Callee(target), self.sites[site]));
                    unavailable.push(caller_index);
                }
            }
        }
        (self.retained, self.causes)
    }
}

/// Fold a scalar-target availability verdict into dependency edges or a
/// missing-target drop.
fn apply_scalar_target_verdict(
    closure: &mut CandidateClosure,
    candidate_count: usize,
    partial_base: usize,
    caller_index: usize,
    target: SymbolHandle,
    verdict: Option<scalar_targets::AvailableScalarTarget>,
    site: GapSite,
) {
    match verdict {
        Some(scalar_targets::AvailableScalarTarget::Ordinary(target_index)) => {
            closure.depend(target_index, caller_index, site);
        }
        Some(scalar_targets::AvailableScalarTarget::Composed(composed_index)) => {
            closure.depend(candidate_count + composed_index, caller_index, site);
        }
        Some(scalar_targets::AvailableScalarTarget::PartialAffine(partial_index)) => {
            closure.depend(partial_base + partial_index, caller_index, site);
        }
        None => {
            closure.drop_candidate(caller_index, CheckedUnitPlanGapReason::Callee(target), site)
        }
    }
}

/// Admit a call operation: boundary coverage, a module entry requirement, or
/// the scalar target's availability verdict.
fn admit_call_target(
    closure: &mut CandidateClosure,
    boundaries: &[(u32, u32)],
    candidate_count: usize,
    partial_base: usize,
    caller_index: usize,
    call: &checked_trees::CheckedUnitCallPlan,
    scalar_verdict: impl FnOnce() -> Option<scalar_targets::AvailableScalarTarget>,
    site: GapSite,
) {
    match (call.route(), call.result()) {
        (CheckedUnitCallRoute::Boundary, _) => {
            if boundaries
                .binary_search(&symbol_key(call.target.machine))
                .is_err()
            {
                closure.drop_candidate(
                    caller_index,
                    CheckedUnitPlanGapReason::BoundaryTarget(call.target.machine),
                    site,
                );
            }
        }
        (
            CheckedUnitCallRoute::Module { .. },
            CheckedUnitCallResult::Unit | CheckedUnitCallResult::Structural { .. },
        ) => {
            // Call admission consumes this roster's exact body,
            // not a different return catalog's coverage promise.
            closure.require_entry(caller_index, call.target.machine, call.target.state, site)
        }
        (CheckedUnitCallRoute::Module { .. }, CheckedUnitCallResult::Scalar(_)) => {
            apply_scalar_target_verdict(
                closure,
                candidate_count,
                partial_base,
                caller_index,
                call.target.machine,
                scalar_verdict(),
                site,
            )
        }
        // A selector call enters no single body. Each selected
        // implementation's rows are the dispatch catalog's dependencies,
        // which lowering resolves when it assembles the tables.
        (CheckedUnitCallRoute::Dynamic { .. }, _) => {}
    }
}

/// Prune both catalogs to the bodies whose every call dependency is available,
/// returning one gap per dropped machine that names the direct dependency
/// that failed at its call site.
/// Completion and continuation actions publish their hook dependencies in the
/// same body vocabulary as authored calls; no synthetic dispatcher edge exists.
pub(super) fn retain_available(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    boundary_symbols: &[SymbolHandle],
    cleanup_control: &checked_trees::CheckedCleanupControlPlans,
    candidates: &mut Vec<CheckedUnitEffectMachinePlan>,
    composed_machines: &mut Vec<CheckedComposedUnitControlMachinePlan>,
    partial_affine_machines: &mut Vec<CheckedPartialAffineUnitCleanupMachinePlan>,
) -> Vec<CheckedUnitPlanGap> {
    // Candidate ordinals: ordinary bodies, then composed bodies, then the
    // partial-affine bodies, which are ordinary bodies with a residual
    // return edge and admit their calls exactly as ordinary bodies do.
    let partial_base = candidates.len() + composed_machines.len();
    let mut closure = CandidateClosure::new(
        candidates
            .iter()
            .map(|plan| (plan.machine, plan.state))
            .chain(composed_machines.iter().map(|plan| {
                (
                    plan.machine,
                    plan.states
                        .first()
                        .map_or(SymbolHandle::invalid(), |state| state.state),
                )
            }))
            .chain(
                partial_affine_machines
                    .iter()
                    .map(|plan| (plan.machine.machine, plan.machine.state)),
            ),
    );
    let mut boundaries = boundary_symbols
        .iter()
        .copied()
        .map(symbol_key)
        .collect::<Vec<_>>();
    boundaries.sort_unstable();
    boundaries.dedup();
    let ordinary_bodies = candidates.iter().enumerate().chain(
        partial_affine_machines
            .iter()
            .enumerate()
            .map(|(index, plan)| (partial_base + index, &plan.machine)),
    );
    for (caller_index, plan) in ordinary_bodies {
        if !closure.retained[caller_index] {
            continue;
        }
        for operation in plan
            .operations
            .iter()
            .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
        {
            match operation {
                CheckedUnitEffectOperationPlan::Call(call) => admit_call_target(
                    &mut closure,
                    &boundaries,
                    candidates.len(),
                    partial_base,
                    caller_index,
                    call,
                    || {
                        scalar_targets::available_target(
                            program,
                            facts,
                            candidates,
                            composed_machines,
                            partial_affine_machines,
                            &scalar_targets::ScalarCallSite::of_plan(plan),
                            operation,
                        )
                    },
                    GapSite::Statement {
                        state: plan.state,
                        index: call.coordinate.statement_index,
                    },
                ),
                // Selected execution already joined exact realization custody.
                CheckedUnitEffectOperationPlan::PortWrite { .. }
                | CheckedUnitEffectOperationPlan::RegisterAccess { .. }
                | CheckedUnitEffectOperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::ScalarArray { .. },
                )
                | checked_trees::reference_hold_pattern!()
                | checked_trees::byte_sequence_establishment_pattern!()
                | CheckedUnitEffectOperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { .. },
                )
                | CheckedUnitEffectOperationPlan::Store(_)
                | CheckedUnitEffectOperationPlan::AtomicFence { .. }
                | CheckedUnitEffectOperationPlan::AtomicAccess(_)
                | checked_trees::structural_field_write_pattern!()
                | CheckedUnitEffectOperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::TrivialAffine { .. },
                )
                | CheckedUnitEffectOperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::Structural { .. },
                )
                | CheckedUnitEffectOperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::Primitive { .. },
                )
                | CheckedUnitEffectOperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::Scalar { .. },
                ) => {}
                CheckedUnitEffectOperationPlan::Complete { .. } => {
                    for cleanup in cleanup_control
                        .normal_actions(plan.normal_exit_cleanups)
                        .filter_map(checked_trees::CheckedNormalCleanupAction::nominal)
                    {
                        closure.require_entry(
                            caller_index,
                            cleanup.cleanup_machine,
                            cleanup.cleanup_state,
                            GapSite::State(plan.state),
                        );
                    }
                }
                CheckedUnitEffectOperationPlan::ContinuationCleanup {
                    boundary,
                    cleanup_actions,
                    ..
                } => {
                    for cleanup in cleanup_actions
                        .iter()
                        .filter_map(checked_trees::CheckedNormalCleanupAction::nominal)
                    {
                        closure.require_entry(
                            caller_index,
                            cleanup.cleanup_machine,
                            cleanup.cleanup_state,
                            cleanup_site(plan.state, boundary),
                        );
                    }
                }
            }
            if !closure.retained[caller_index] {
                break;
            }
        }
        if !closure.retained[caller_index] {
            continue;
        }
    }
    for (composed_index, plan) in composed_machines.iter().enumerate() {
        let caller_index = candidates.len() + composed_index;
        if !closure.retained[caller_index] {
            continue;
        }
        for state in &plan.states {
            for action in cleanup_control.normal_actions(state.normal_exit_cleanups) {
                if let checked_trees::CheckedNormalCleanupAction::InvokeNominal(cleanup) = action {
                    closure.require_entry(
                        caller_index,
                        cleanup.cleanup_machine,
                        cleanup.cleanup_state,
                        GapSite::State(state.state),
                    );
                }
            }
            for operation in state
                .operation_dependencies()
                .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
            {
                let call_site =
                    |coordinate: checked_trees::CheckedUnitCallCoordinate| GapSite::Statement {
                        state: state.state,
                        index: coordinate.statement_index,
                    };
                // Composed bodies have a narrower operation vocabulary and no
                // ordinary structural-return fallback. Keep that admission explicit.
                match operation {
                    CheckedUnitEffectOperationPlan::Call(call) => admit_call_target(
                        &mut closure,
                        &boundaries,
                        candidates.len(),
                        partial_base,
                        caller_index,
                        call,
                        || {
                            scalar_targets::ScalarCallSite::of_composed_operation(
                                plan.machine,
                                state,
                                operation,
                            )
                            .and_then(|site| {
                                scalar_targets::available_target(
                                    program,
                                    facts,
                                    candidates,
                                    composed_machines,
                                    partial_affine_machines,
                                    &site,
                                    operation,
                                )
                            })
                        },
                        call_site(call.coordinate),
                    ),
                    // A store names no callee: its destination is storage or
                    // a loan inside this frame, its index and scalar value
                    // are dense, and a sum field's copy or construction and
                    // its window repair stay in-frame. A port write is a
                    // checked physical event with exact operands, not a call.
                    CheckedUnitEffectOperationPlan::Store(_)
                    | CheckedUnitEffectOperationPlan::AtomicFence { .. }
                    | CheckedUnitEffectOperationPlan::AtomicAccess(_)
                    | CheckedUnitEffectOperationPlan::PortWrite { .. }
                    | CheckedUnitEffectOperationPlan::RegisterAccess { .. }
                    | CheckedUnitEffectOperationPlan::EstablishLocal(checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { .. })
                    // A borrowed reference binds storage already owned by the
                    // frame; establish/release carry no callee dependency.
                    | checked_trees::reference_hold_pattern!()
                    // A view-subslice local narrows a view the frame already
                    // holds; it names no callee.
                    | checked_trees::byte_sequence_establishment_pattern!()
                    | CheckedUnitEffectOperationPlan::EstablishLocal(checked_trees::CheckedUnitLocalBindingPlan::Scalar { .. })
                    // Fixed-array, mutable-primitive, and trivial-affine
                    // establishments mint frame-local storage at the authored
                    // statement; they name no callee either.
                    | CheckedUnitEffectOperationPlan::EstablishLocal(checked_trees::CheckedUnitLocalBindingPlan::ScalarArray { .. })
                    | CheckedUnitEffectOperationPlan::EstablishLocal(checked_trees::CheckedUnitLocalBindingPlan::Primitive { .. })
                    | CheckedUnitEffectOperationPlan::EstablishLocal(checked_trees::CheckedUnitLocalBindingPlan::TrivialAffine { .. })
                    // A declared structural local's establishment mints
                    // frame-local storage at the authored statement as well;
                    // member fills name no callee.
                    | CheckedUnitEffectOperationPlan::EstablishLocal(checked_trees::CheckedUnitLocalBindingPlan::Structural { .. })
                    // A borrowed-window move carries no callee: its result
                    // binding, like a repair store's installed value, is
                    // already a custody row inside this same state.
                    | checked_trees::structural_field_write_pattern!()
                    => {}
                    CheckedUnitEffectOperationPlan::ContinuationCleanup { boundary, cleanup_actions, .. } => {
                        for cleanup in cleanup_actions.iter().filter_map(checked_trees::CheckedNormalCleanupAction::nominal) {
                            closure.require_entry(
                                caller_index,
                                cleanup.cleanup_machine,
                                cleanup.cleanup_state,
                                cleanup_site(state.state, boundary),
                            );
                        }
                    }
                    _ => closure.drop_candidate(
                        caller_index,
                        CheckedUnitPlanGapReason::Unsupported("composed control vocabulary"),
                        GapSite::State(state.state),
                    ),
                }
                if !closure.retained[caller_index] {
                    break;
                }
            }
            if !closure.retained[caller_index] {
                break;
            }
        }
    }
    let machines = candidates
        .iter()
        .map(|plan| plan.machine)
        .chain(composed_machines.iter().map(|plan| plan.machine))
        .chain(
            partial_affine_machines
                .iter()
                .map(|plan| plan.machine.machine),
        )
        .collect::<Vec<_>>();
    let (retained, causes) = closure.close();
    // A machine leaves only when none of its bodies survived; it keeps the
    // first dropped body's cause.
    let surviving = machines
        .iter()
        .zip(&retained)
        .filter(|(_, retained)| **retained)
        .map(|(machine, _)| symbol_key(*machine))
        .collect::<BTreeSet<_>>();
    let mut reported = BTreeSet::new();
    let gaps = machines
        .iter()
        .zip(&causes)
        .filter_map(|(machine, cause)| {
            let (reason, site) = (*cause)?;
            let key = symbol_key(*machine);
            (!surviving.contains(&key) && reported.insert(key))
                .then(|| plan_gap(program, *machine, site, reason))
        })
        .collect();
    let mut retained = retained.into_iter();
    candidates.retain(|_| retained.next().unwrap_or(false));
    composed_machines.retain(|_| retained.next().unwrap_or(false));
    partial_affine_machines.retain(|_| retained.next().unwrap_or(false));
    gaps
}

/// The authored statement a continuation cleanup runs at, or its state when
/// the cleanup follows a call by coordinate.
fn cleanup_site(state: SymbolHandle, boundary: &CheckedUnitCleanupBoundary) -> GapSite {
    let index = match boundary {
        CheckedUnitCleanupBoundary::Call(coordinate) => coordinate.statement_index,
        CheckedUnitCleanupBoundary::Replacement {
            statement_index, ..
        }
        | CheckedUnitCleanupBoundary::StatementEntry { statement_index } => *statement_index,
    };
    GapSite::Statement { state, index }
}
