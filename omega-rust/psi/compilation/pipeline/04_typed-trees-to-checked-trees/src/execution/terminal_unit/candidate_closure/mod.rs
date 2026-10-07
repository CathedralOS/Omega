//! Close the fixed candidate roster over exact call dependencies, without
//! rebuilding bodies or rechecking unaffected callers after each removal.
use super::{
    CheckFacts, CheckedComposedUnitControlMachinePlan, CheckedUnitEffectMachinePlan,
    CheckedUnitEffectOperationPlan, SymbolHandle, TypedTrees,
};
use crate::execution::terminal_unit::ScalarCalleePlans;
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
    // dense and private to this invocation. Only initially unique live entries
    // survive: the old first round removed every competing definition together.
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
        let mut roster = entries
            .enumerate()
            .map(|(candidate_index, (machine, state))| (machine, state, candidate_index))
            .collect::<Vec<_>>();
        let mut retained = vec![false; roster.len()];
        let mut causes = vec![
            Some((
                CheckedUnitPlanGapReason::Unsupported("two plan routes describe this body"),
                GapSite::Machine,
            ));
            roster.len()
        ];
        roster.sort_unstable_by_key(|(machine, _, _)| symbol_key(*machine));
        for group in roster.chunk_by(|left, right| left.0 == right.0) {
            if let [(machine, state, candidate_index)] = group
                && machine.is_valid()
                && state.is_valid()
            {
                retained[*candidate_index] = true;
                causes[*candidate_index] = None;
            }
        }
        roster.retain(|(_, _, candidate_index)| retained[*candidate_index]);
        Self {
            entries: roster,
            retained,
            causes,
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
    caller_index: usize,
    target: SymbolHandle,
    verdict: Option<scalar_targets::AvailableScalarTarget>,
    site: GapSite,
) {
    match verdict {
        Some(scalar_targets::AvailableScalarTarget::Registered) => {}
        Some(scalar_targets::AvailableScalarTarget::OrdinaryBody(target_index)) => {
            closure.depend(target_index, caller_index, site);
        }
        Some(scalar_targets::AvailableScalarTarget::ComposedBody(composed_index)) => {
            closure.depend(candidate_count + composed_index, caller_index, site);
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

fn symbol_key(symbol: SymbolHandle) -> (u32, u32) {
    (symbol.arena_index(), symbol.generation())
}

/// Prune both catalogs to the bodies whose every call dependency is available,
/// returning one gap per dropped machine that names the direct dependency
/// that failed at its call site.
/// `nominal_cleanup_edges` carries each seeded nominal consuming machine to the
/// exact owner-attached `::drop` hook its return edge will invoke. The hook is
/// not an operation in the synthetic body, so the closure needs the edge named
/// here: a dropped hook body drops the consuming machine with it.
pub(super) fn retain_available(
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleePlans<'_>,
    boundary_symbols: &[SymbolHandle],
    candidates: &mut Vec<CheckedUnitEffectMachinePlan>,
    composed_machines: &mut Vec<CheckedComposedUnitControlMachinePlan>,
    nominal_cleanup_edges: &[(SymbolHandle, SymbolHandle)],
) -> Vec<CheckedUnitPlanGap> {
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
            })),
    );
    let mut boundaries = boundary_symbols
        .iter()
        .copied()
        .map(symbol_key)
        .collect::<Vec<_>>();
    boundaries.sort_unstable();
    boundaries.dedup();
    for (caller_index, plan) in candidates.iter().enumerate() {
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
                    caller_index,
                    call,
                    || {
                        scalar_targets::available_target(
                            program,
                            facts,
                            scalar_callees,
                            candidates,
                            composed_machines,
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
                | CheckedUnitEffectOperationPlan::EstablishScalarArray { .. }
                | CheckedUnitEffectOperationPlan::EstablishReference { .. }
                | CheckedUnitEffectOperationPlan::ReleaseReference { .. }
                | CheckedUnitEffectOperationPlan::EstablishByteSequence { .. }
                | CheckedUnitEffectOperationPlan::EstablishViewSubslice { .. }
                | CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. }
                | CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
                | CheckedUnitEffectOperationPlan::AtomicAccess(_)
                | CheckedUnitEffectOperationPlan::ByteSequenceStore(_)
                | CheckedUnitEffectOperationPlan::ByteSequenceWrite(_)
                | CheckedUnitEffectOperationPlan::ElementViewWrite(_)
                | CheckedUnitEffectOperationPlan::RepresentationViewStore(_)
                | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_)
                | CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
                | CheckedUnitEffectOperationPlan::StructuralCaseFieldStore(_)
                | CheckedUnitEffectOperationPlan::StructuralCaseLiteralFieldStore(_)
                | CheckedUnitEffectOperationPlan::MoveStructuralField { .. }
                | CheckedUnitEffectOperationPlan::StoreStructuralField { .. }
                | CheckedUnitEffectOperationPlan::RebindStructuralLocal { .. }
                | CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal { .. }
                | CheckedUnitEffectOperationPlan::EstablishStructuralLocal { .. }
                | CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { .. }
                | CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. }
                | CheckedUnitEffectOperationPlan::Complete { .. } => {}
                CheckedUnitEffectOperationPlan::ContinuationCleanup {
                    boundary,
                    cleanup_actions,
                } => {
                    for cleanup in cleanup_actions
                        .iter()
                        .filter_map(checked_trees::CheckedNormalCleanupAction::nominal)
                    {
                        closure.require_entry(
                            caller_index,
                            cleanup.cleanup_machine,
                            cleanup.cleanup_state,
                            cleanup_site(plan.state, *boundary),
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
        for (_, hook_machine) in nominal_cleanup_edges
            .iter()
            .filter(|(caller, _)| *caller == plan.machine)
        {
            let hook_state = crate::lookup::machine_by_symbol(program, *hook_machine)
                .and_then(|hook| program.machine_states(hook).first())
                .map(|state| state.symbol);
            match hook_state {
                Some(state) => {
                    closure.require_entry(caller_index, *hook_machine, state, GapSite::Machine)
                }
                None => closure.drop_candidate(
                    caller_index,
                    CheckedUnitPlanGapReason::Callee(*hook_machine),
                    GapSite::Machine,
                ),
            }
        }
    }
    for (composed_index, plan) in composed_machines.iter().enumerate() {
        let caller_index = candidates.len() + composed_index;
        if !closure.retained[caller_index] {
            continue;
        }
        for state in &plan.states {
            for action in state
                .normal_exit_cleanups
                .iter()
                .flat_map(|row| &row.actions)
            {
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
                    CheckedUnitEffectOperationPlan::Call(call) => match (call.route(), call.result()) {
                        (CheckedUnitCallRoute::Boundary, _) => {
                            if boundaries
                                .binary_search(&symbol_key(call.target.machine))
                                .is_err()
                            {
                                closure.drop_candidate(
                                    caller_index,
                                    CheckedUnitPlanGapReason::BoundaryTarget(call.target.machine),
                                    call_site(call.coordinate),
                                );
                            }
                        }
                        (
                            CheckedUnitCallRoute::Module { .. },
                            CheckedUnitCallResult::Unit | CheckedUnitCallResult::Structural { .. },
                        ) => closure.require_entry(
                            caller_index,
                            call.target.machine,
                            call.target.state,
                            call_site(call.coordinate),
                        ),
                        (CheckedUnitCallRoute::Module { .. }, CheckedUnitCallResult::Scalar(_)) => {
                        apply_scalar_target_verdict(
                            &mut closure,
                            candidates.len(),
                            caller_index,
                            call.target.machine,
                            scalar_targets::ScalarCallSite::of_composed_operation(
                                plan.machine,
                                state,
                                operation,
                            )
                            .and_then(|site| {
                                scalar_targets::available_target(
                                    program,
                                    facts,
                                    scalar_callees,
                                    candidates,
                                    composed_machines,
                                    &site,
                                    operation,
                                )
                            }),
                            call_site(call.coordinate),
                        )
                        }
                        (CheckedUnitCallRoute::Dynamic { .. }, _) => {}
                    },
                    CheckedUnitEffectOperationPlan::ByteSequenceStore(_)
                    | CheckedUnitEffectOperationPlan::ByteSequenceWrite(_)
                    | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_)
                    // A write through a borrowed view names no callee: the
                    // destination is a loan inside this frame and the index
                    // and element scalars are dense, exactly as the byte-
                    // sequence and field-store siblings above.
                    | CheckedUnitEffectOperationPlan::ElementViewWrite(_)
                    // A store through a recast loan local names no callee
                    // either: the destination rejoins a loan inside this
                    // frame and the authored value is a dense scalar.
                    | CheckedUnitEffectOperationPlan::RepresentationViewStore(_)
                    | CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
                    // Complete case replacement has no callee dependency;
                    // its exact construction/copy and repair stay in-frame.
                    | CheckedUnitEffectOperationPlan::StructuralCaseFieldStore(_)
                    | CheckedUnitEffectOperationPlan::StructuralCaseLiteralFieldStore(_)
                    | CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
                    | CheckedUnitEffectOperationPlan::AtomicAccess(_)
                    | CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. }
                    // A borrowed reference binds storage already owned by the
                    // frame; establish/release carry no callee dependency.
                    | CheckedUnitEffectOperationPlan::EstablishReference { .. }
                    | CheckedUnitEffectOperationPlan::ReleaseReference { .. }
                    // A view-subslice local narrows a view the frame already
                    // holds; it names no callee.
                    | CheckedUnitEffectOperationPlan::EstablishByteSequence { .. }
                    | CheckedUnitEffectOperationPlan::EstablishViewSubslice { .. }
                    | CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. }
                    // Fixed-array, mutable-primitive, and trivial-affine
                    // establishments mint frame-local storage at the authored
                    // statement; they name no callee either.
                    | CheckedUnitEffectOperationPlan::EstablishScalarArray { .. }
                    | CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { .. }
                    | CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal { .. }
                    // A declared structural local's establishment mints
                    // frame-local storage at the authored statement as well;
                    // member fills name no callee.
                    | CheckedUnitEffectOperationPlan::EstablishStructuralLocal { .. }
                    // A borrowed-window move/restore pair carries no callee:
                    // the move's result binding and the restore's stored value
                    // are already custody rows inside this same state.
                    | CheckedUnitEffectOperationPlan::MoveStructuralField { .. }
                    | CheckedUnitEffectOperationPlan::StoreStructuralField { .. }
                    | CheckedUnitEffectOperationPlan::RebindStructuralLocal { .. }
                    => {}
                    CheckedUnitEffectOperationPlan::ContinuationCleanup { boundary, cleanup_actions } => {
                        for cleanup in cleanup_actions.iter().filter_map(checked_trees::CheckedNormalCleanupAction::nominal) {
                            closure.require_entry(
                                caller_index,
                                cleanup.cleanup_machine,
                                cleanup.cleanup_state,
                                cleanup_site(state.state, *boundary),
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
    gaps
}

/// The authored statement a continuation cleanup runs at, or its state when
/// the cleanup follows a call by coordinate.
fn cleanup_site(state: SymbolHandle, boundary: CheckedUnitCleanupBoundary) -> GapSite {
    let index = match boundary {
        CheckedUnitCleanupBoundary::Call(coordinate) => coordinate.statement_index,
        CheckedUnitCleanupBoundary::Replacement { statement_index }
        | CheckedUnitCleanupBoundary::StatementEntry { statement_index } => statement_index,
    };
    GapSite::Statement { state, index }
}
