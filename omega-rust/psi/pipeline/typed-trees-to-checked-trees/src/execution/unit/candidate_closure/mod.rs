//! Close the fixed candidate roster over exact call dependencies, without
//! rebuilding bodies or rechecking unaffected callers after each removal.
use super::{
    CheckFacts, CheckedComposedUnitControlMachinePlan, CheckedUnitEffectMachinePlan,
    CheckedUnitEffectOperationPlan, SymbolHandle, TypedTrees,
};
use crate::execution::terminal_unit::ScalarCalleePlans;
use crate::execution::terminal_unit::scalar_targets;
use checked_trees::{CheckedUnitPlanOmission, CheckedUnitPlanOmissionStage};

#[cfg(test)]
mod tests;

struct CandidateClosure {
    // Symbols are sparse across the complete program; candidate ordinals are
    // dense and private to this invocation. Only initially unique live entries
    // survive: the old first round removed every competing definition together.
    entries: Vec<(SymbolHandle, SymbolHandle, usize)>,
    retained: Vec<bool>,
    // The first reason each candidate left the roster, kept beside `retained`
    // so the omission evidence names the direct dependency that failed.
    causes: Vec<Option<CheckedUnitPlanOmissionStage>>,
    dependents: Vec<(usize, usize)>,
}

impl CandidateClosure {
    fn new(entries: impl Iterator<Item = (SymbolHandle, SymbolHandle)>) -> Self {
        let mut roster = entries
            .enumerate()
            .map(|(candidate_index, (machine, state))| (machine, state, candidate_index))
            .collect::<Vec<_>>();
        let mut retained = vec![false; roster.len()];
        let mut causes =
            vec![Some(CheckedUnitPlanOmissionStage::CompetingCandidates); roster.len()];
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
        }
    }

    fn drop_candidate(&mut self, caller_index: usize, stage: CheckedUnitPlanOmissionStage) {
        if self.retained[caller_index] {
            self.retained[caller_index] = false;
            self.causes[caller_index] = Some(stage);
        }
    }

    fn require_entry(&mut self, caller_index: usize, machine: SymbolHandle, state: SymbolHandle) {
        match self
            .entries
            .binary_search_by_key(&symbol_key(machine), |(candidate, _, _)| {
                symbol_key(*candidate)
            }) {
            Ok(entry_index) if self.entries[entry_index].1 == state => {
                let target_index = self.entries[entry_index].2;
                self.dependents.push((target_index, caller_index));
            }
            _ => self.drop_candidate(
                caller_index,
                CheckedUnitPlanOmissionStage::UnavailableCallee { target: machine },
            ),
        }
    }

    fn close(mut self) -> (Vec<bool>, Vec<Option<CheckedUnitPlanOmissionStage>>) {
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
        self.dependents.sort_unstable();
        self.dependents.dedup();
        let mut queue_position = 0;
        while let Some(&target_index) = unavailable.get(queue_position) {
            queue_position += 1;
            let first = self
                .dependents
                .partition_point(|(target, _)| *target < target_index);
            let target = self
                .entries
                .iter()
                .find(|(_, _, candidate_index)| *candidate_index == target_index)
                .map(|(machine, _, _)| *machine)
                .unwrap_or_else(SymbolHandle::invalid);
            for &(_, caller_index) in self.dependents[first..]
                .iter()
                .take_while(|(target, _)| *target == target_index)
            {
                if self.retained[caller_index] {
                    self.retained[caller_index] = false;
                    self.causes[caller_index] =
                        Some(CheckedUnitPlanOmissionStage::UnavailableCallee { target });
                    unavailable.push(caller_index);
                }
            }
        }
        (self.retained, self.causes)
    }
}

fn symbol_key(symbol: SymbolHandle) -> (u32, u32) {
    (symbol.arena_index(), symbol.generation())
}

/// Prune both catalogs to the bodies whose every call dependency is available,
/// returning one omission row per dropped body that names the stage and the
/// direct dependency that failed.
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
) -> Vec<CheckedUnitPlanOmission> {
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
                CheckedUnitEffectOperationPlan::CallUnit {
                    target_machine,
                    target_state,
                    ..
                } => closure.require_entry(caller_index, *target_machine, *target_state),
                CheckedUnitEffectOperationPlan::StructuralCall {
                    target_machine,
                    target_state,
                    ..
                } => {
                    if facts
                        .flow
                        .terminal_structural_returns
                        .claim_free_affine_for_machine(*target_machine)
                        .is_none()
                    {
                        closure.require_entry(caller_index, *target_machine, *target_state);
                    }
                }
                CheckedUnitEffectOperationPlan::BoundaryCall { target_machine, .. }
                | CheckedUnitEffectOperationPlan::BoundaryScalarCall { target_machine, .. }
                | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    target_machine, ..
                } => {
                    if boundaries
                        .binary_search(&symbol_key(*target_machine))
                        .is_err()
                    {
                        closure.drop_candidate(
                            caller_index,
                            CheckedUnitPlanOmissionStage::MissingBoundaryTarget {
                                target: *target_machine,
                            },
                        );
                    }
                }
                CheckedUnitEffectOperationPlan::ScalarCall { target_machine, .. } => {
                    match scalar_targets::available_target(
                        program,
                        facts,
                        scalar_callees,
                        candidates,
                        plan,
                        operation,
                    ) {
                        Some(scalar_targets::AvailableScalarTarget::Registered) => {}
                        Some(scalar_targets::AvailableScalarTarget::OrdinaryBody(target_index)) => {
                            closure.dependents.push((target_index, caller_index));
                        }
                        None => closure.drop_candidate(
                            caller_index,
                            CheckedUnitPlanOmissionStage::UnavailableScalarTarget {
                                target: *target_machine,
                            },
                        ),
                    }
                }
                // Selected execution already joined exact realization custody.
                CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall { .. }
                | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall { .. }
                | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralCall { .. }
                | CheckedUnitEffectOperationPlan::SelectedIeeeFloatFusedMultiplyAdd { .. }
                | CheckedUnitEffectOperationPlan::PortWrite { .. }
                | CheckedUnitEffectOperationPlan::EstablishScalarArray { .. }
                | CheckedUnitEffectOperationPlan::EstablishReference { .. }
                | CheckedUnitEffectOperationPlan::ReleaseReference { .. }
                | CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. }
                | CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
                | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(_)
                | CheckedUnitEffectOperationPlan::ByteSequenceWrite(_)
                | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_)
                | CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
                | CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal { .. }
                | CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { .. }
                | CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. }
                | CheckedUnitEffectOperationPlan::CallContinuationCleanup { .. }
                | CheckedUnitEffectOperationPlan::Complete { .. } => {}
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
            let hook_state = program
                .machines()
                .iter()
                .find(|candidate| candidate.symbol == *hook_machine)
                .and_then(|hook| program.machine_states(hook).first())
                .map(|state| state.symbol);
            match hook_state {
                Some(state) => closure.require_entry(caller_index, *hook_machine, state),
                None => closure.drop_candidate(
                    caller_index,
                    CheckedUnitPlanOmissionStage::UnavailableCallee {
                        target: *hook_machine,
                    },
                ),
            }
        }
    }
    for (composed_index, plan) in composed_machines.iter().enumerate() {
        let caller_index = candidates.len() + composed_index;
        if !closure.retained[caller_index] {
            continue;
        }
        for operation in plan
            .states
            .iter()
            .flat_map(|state| state.operation_dependencies())
            .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
        {
            // Composed bodies have a narrower operation vocabulary and no
            // ordinary structural-return fallback. Keep that admission explicit.
            match operation {
                CheckedUnitEffectOperationPlan::CallUnit {
                    target_machine,
                    target_state,
                    ..
                }
                | CheckedUnitEffectOperationPlan::StructuralCall {
                    target_machine,
                    target_state,
                    ..
                } => closure.require_entry(caller_index, *target_machine, *target_state),
                CheckedUnitEffectOperationPlan::BoundaryCall { target_machine, .. }
                | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    target_machine, ..
                } => {
                    if boundaries
                        .binary_search(&symbol_key(*target_machine))
                        .is_err()
                    {
                        closure.drop_candidate(
                            caller_index,
                            CheckedUnitPlanOmissionStage::MissingBoundaryTarget {
                                target: *target_machine,
                            },
                        );
                    }
                }
                CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(_)
                | CheckedUnitEffectOperationPlan::ByteSequenceWrite(_)
                | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_)
                | CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
                | CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
                | CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. }
                | CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. }
                // The paired boundary call carries the callee dependency;
                // its cleanup continuation only disposes the discarded result.
                | CheckedUnitEffectOperationPlan::CallContinuationCleanup { .. } => {}
                _ => closure.drop_candidate(
                    caller_index,
                    CheckedUnitPlanOmissionStage::ComposedVocabulary,
                ),
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
    let omissions = machines
        .iter()
        .zip(&causes)
        .filter_map(|(machine, cause)| {
            cause.map(|stage| CheckedUnitPlanOmission {
                machine: *machine,
                stage,
            })
        })
        .collect();
    let mut retained = retained.into_iter();
    candidates.retain(|_| retained.next().unwrap_or(false));
    composed_machines.retain(|_| retained.next().unwrap_or(false));
    omissions
}
