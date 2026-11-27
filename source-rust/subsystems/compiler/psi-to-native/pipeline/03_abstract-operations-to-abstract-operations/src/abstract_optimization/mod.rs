#![forbid(unsafe_code)]

/*
Native stage 03 turns the `VerifiedPsiOptimizationInput` that stage 02 lowered
from Terminal Psi into the `ValidatedOptimizedAbstractPlan` that target lowering
reads. It builds the `VerifiedPsiOptimizationUnit`, admits its runtime evidence
and ranked cycles, and seals the plan with its projection and pre-physical
manifest.

The Psi optimization phase runs before Terminal publication, in source stage 06
(`wiki/spec/terminal-psi/product.md#optimization-ordering`), and no
optimization is assigned to the abstract-operations phase. Native lowering
receives only post-Terminal selections, so stage 03 always records the identity
run (`pass_manager::identity_run`) and publishes the input plan unchanged. The
rule engine that used to live here is parked in `_recycling/` at this crate's
mirrored path until its rules migrate to stage 06.
*/

macro_rules! failed_display {
    ($type:ty, $message:literal) => {
        impl std::fmt::Display for $type {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, concat!($message, ": {:?}"), self)
            }
        }

        impl std::error::Error for $type {}
    };
}

mod pass_manager;
mod publication;
mod ranked_cycles_validation;
pub mod validation;
mod verified_unit;

pub(crate) use ::wire::codec_bytes;

pub(crate) mod cycles {
    pub(crate) use super::strongly_connected_components;
}

pub(crate) use pass_manager::OptimizationRun;
pub(crate) use pass_manager::VerifiedPsiOptimizationSession;
pub(crate) use publication::OptimizedAbstractProjectionError;
pub use publication::ValidatedOptimizedAbstractPlan;

use optimization_core::{
    OptimizationExecutionPhase, OptimizationSelections, OptimizationWorkBudget,
};
use verified_optimization_input::VerifiedPsiOptimizationInput;
pub use verified_unit::{
    VerifiedPsiOptimizationUnitBuildError, build_verified_psi_optimization_unit,
};

use crate::abstract_optimization::pass_manager::{OptimizationRunError, identity_run};
use crate::abstract_optimization::publication::publish_optimization_run;
use std::collections::{BTreeMap, BTreeSet};
use terminal_work::WorkSchedule;

#[derive(Debug)]
pub enum AbstractOptimizationError {
    UnconsumedOptimizationPhase(OptimizationExecutionPhase),
    UnitBuild(VerifiedPsiOptimizationUnitBuildError),
    Run(OptimizationRunError),
    Publication(OptimizedAbstractProjectionError),
}

failed_display!(AbstractOptimizationError, "abstract optimization failed");

pub fn optimize_abstract_operations(
    input: VerifiedPsiOptimizationInput,
    selections: &OptimizationSelections,
    budget_per_pass: OptimizationWorkBudget,
) -> Result<ValidatedOptimizedAbstractPlan, AbstractOptimizationError> {
    for phase in [
        OptimizationExecutionPhase::Psi,
        OptimizationExecutionPhase::AbstractOperations,
    ] {
        if !selections.for_phase(phase).is_empty() {
            return Err(AbstractOptimizationError::UnconsumedOptimizationPhase(
                phase,
            ));
        }
    }
    let verified = build_verified_psi_optimization_unit(input, WorkSchedule::CURRENT.identity())
        .map_err(AbstractOptimizationError::UnitBuild)?;
    let initial = validation::InitialUnitRecord::of(verified.unit()).map_err(|error| {
        AbstractOptimizationError::Publication(
            OptimizedAbstractProjectionError::PrePhysicalManifest(error),
        )
    })?;
    let (input, unit) = verified.into_parts();
    let session = VerifiedPsiOptimizationSession::admit(input, unit).map_err(|error| {
        AbstractOptimizationError::Run(OptimizationRunError::InitialValidation(error))
    })?;
    let run = identity_run(session, selections, budget_per_pass)
        .map_err(AbstractOptimizationError::Run)?;
    publish_optimization_run(run, initial).map_err(AbstractOptimizationError::Publication)
}

pub(crate) fn strongly_connected_components<T>(graph: &BTreeMap<T, Vec<T>>) -> Vec<Vec<T>>
where
    T: Copy + Ord,
{
    struct Tarjan<T> {
        next: usize,
        indices: BTreeMap<T, usize>,
        lowlinks: BTreeMap<T, usize>,
        stack: Vec<T>,
        on_stack: BTreeSet<T>,
        components: Vec<Vec<T>>,
    }

    fn visit<T>(node: T, graph: &BTreeMap<T, Vec<T>>, state: &mut Tarjan<T>)
    where
        T: Copy + Ord,
    {
        let index = state.next;
        state.next += 1;
        state.indices.insert(node, index);
        state.lowlinks.insert(node, index);
        state.stack.push(node);
        state.on_stack.insert(node);
        for successor in graph.get(&node).into_iter().flatten().copied() {
            if !graph.contains_key(&successor) {
                continue;
            }
            if !state.indices.contains_key(&successor) {
                visit(successor, graph, state);
                state
                    .lowlinks
                    .insert(node, state.lowlinks[&node].min(state.lowlinks[&successor]));
            } else if state.on_stack.contains(&successor) {
                state
                    .lowlinks
                    .insert(node, state.lowlinks[&node].min(state.indices[&successor]));
            }
        }
        if state.lowlinks[&node] == state.indices[&node] {
            let mut component = Vec::new();
            loop {
                let member = state.stack.pop().expect("SCC root remains on stack");
                state.on_stack.remove(&member);
                component.push(member);
                if member == node {
                    break;
                }
            }
            component.sort_unstable();
            state.components.push(component);
        }
    }

    let mut state = Tarjan {
        next: 0,
        indices: BTreeMap::new(),
        lowlinks: BTreeMap::new(),
        stack: Vec::new(),
        on_stack: BTreeSet::new(),
        components: Vec::new(),
    };
    for node in graph.keys().copied() {
        if !state.indices.contains_key(&node) {
            visit(node, graph, &mut state);
        }
    }
    state.components.sort();
    state.components
}
