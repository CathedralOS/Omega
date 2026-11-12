#![forbid(unsafe_code)]

/*
We execute the post-Terminal abstract-operation optimization stage (Omega 01),
transforming a `VerifiedPsiOptimizationInput` lowered from Terminal Psi into a
`ValidatedOptimizedAbstractPlan` ready for target lowering. Starting at
`optimize_abstract_operations`, we admit the input under the active work
schedule, construct a `VerifiedPsiOptimizationUnit`, run the selected pass
registries through `pass_manager` over the rules ordered by `rules`, and seal
the resulting plan and pre-physical manifest through `publication` and
`validation`.
*/

/*
We keep target-neutral post-Terminal optimization isolated in its own stage so
every rewrite over `AbstractOperationPlan` is governed by deterministic fixed
points, explicit work budgets, and independent custody replay before any target
ISA or ABI decision is introduced. When the Psi optimization selection is
empty, we short-circuit to an exact identity transform without reconstructing
unused optimizer ownership analyses, ensuring zero-pass compilation never adds
a second admission gate.
*/

/*
We consume `VerifiedPsiOptimizationInput` from `terminal-psi-to-abstract-operations`
(Omega 00) together with `OptimizationSelections` and `OptimizationWorkBudget`
from `optimization-core`, and we produce `ValidatedOptimizedAbstractPlan` and
`ValidatedPrePhysicalOptimizationManifest` for `abstract-operations-to-target-operations`
(Omega 02) and `native-compiler`.
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

#[macro_use]
mod rules;
mod analyses;
mod control_flow_analyses;
mod control_flow_cleanup;
mod countdown_analyses;
mod global_value_numbering;
mod pass_manager;
mod proof_check_elision;
mod publication;
mod ranked_cycles_validation;
mod ranked_rewrites;
mod sccp;
mod semantic_analyses;
mod specialization;
pub mod validation;
mod value_ranges;
mod verified_unit;

pub(crate) use optimization_core::codec_bytes;
pub(crate) use specialization::{
    field_value_specialization, representation_specialization, state_specialization,
};

pub(crate) mod cycles {
    pub(crate) use super::strongly_connected_components;
}

pub(crate) use publication::OptimizedAbstractProjectionError;
pub use publication::ValidatedOptimizedAbstractPlan;

pub(crate) use analyses::{
    AnalysisError, CallGraphAnalysis, EffectClass, EffectKnowledge, EffectSummaryAnalysis,
    OwnershipFrontierAnalysis, ScalarConstantAnalysis, UseDefinitionAnalysis, ValueRangeAnalysis,
};
pub(crate) use analyses::{AnalysisProduct, AnalysisRevision, compute_analysis};
pub(crate) use analyses::{
    CountdownInvariantConstantAnalysisError, CountdownInvariantConstantPlacementAnalysisError,
    CountedLoopAnalysisError, ValidatedCountdownInvariantConstantAnalysis,
    ValidatedCountdownInvariantConstantPlacementAnalysis, ValidatedCountedLoopAnalysis,
    analyze_countdown_invariant_constant_placement, analyze_countdown_invariant_constants,
    analyze_counted_loops,
};
pub(crate) use optimization_unit::ScalarConstantValue;
pub(crate) use pass_manager::PsiValidatedCandidateDeclaration;
pub(crate) use pass_manager::VerifiedPsiOptimizationSession;
pub(crate) use pass_manager::{OptimizationRun, OptimizationRunUsage, PsiOptimizationCommit};
pub(crate) use rules::registry::{
    OrderedRuleRegistry, PsiOptimizationRule, RuleAnalysisView, RuleProposalError,
    RuleRegistryError,
};

#[cfg(feature = "test-support")]
pub mod test_support {
    pub use crate::run_psi_pipeline;

    pub use crate::analyses::{
        AnalysisRevision, ValueRangeFact, ValueRangeScope, ValueRangeSupport,
        value_range_fact_identity,
    };

    pub use crate::pass_manager::{OptimizationRunUsage, PsiOptimizationCommit};

    pub use crate::rules::WrappingIntegerAddConstantsRule;
}

use optimization_core::{
    OptimizationExecutionPhase, OptimizationSelections, OptimizationWorkBudget,
    PsiOptimizationSelectionProjection,
};
use verified_optimization_input::VerifiedPsiOptimizationInput;
pub use verified_unit::{
    VerifiedPsiOptimizationUnit, VerifiedPsiOptimizationUnitBuildError,
    build_verified_psi_optimization_unit,
};

use crate::pass_manager::{OptimizationRunError, run_registries};
use crate::publication::publish_optimization_run;
use crate::rules::built_in_psi_steps_for_selections;
use psi_work::WorkSchedule;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug)]
pub enum AbstractOptimizationError {
    UnconsumedOptimizationPhase(OptimizationExecutionPhase),
    UnitBuild(VerifiedPsiOptimizationUnitBuildError),
    Run(OptimizationRunError),
    Publication(OptimizedAbstractProjectionError),
    EmptySelectionChangedPlan,
}

failed_display!(AbstractOptimizationError, "abstract optimization failed");

pub fn optimize_abstract_operations(
    input: VerifiedPsiOptimizationInput,
    selections: &OptimizationSelections,
    psi_selections: &PsiOptimizationSelectionProjection,
    budget_per_pass: OptimizationWorkBudget,
) -> Result<ValidatedOptimizedAbstractPlan, AbstractOptimizationError> {
    let phase = OptimizationExecutionPhase::AbstractOperations;
    if !selections.for_phase(phase).is_empty() {
        return Err(AbstractOptimizationError::UnconsumedOptimizationPhase(
            phase,
        ));
    }
    let verified = build_verified_psi_optimization_unit(input, WorkSchedule::CURRENT.identity())
        .map_err(AbstractOptimizationError::UnitBuild)?;
    let run =
        run_psi_pipeline_for_projection(verified, selections, psi_selections, budget_per_pass)
            .map_err(AbstractOptimizationError::Run)?;
    let published =
        publish_optimization_run(run).map_err(AbstractOptimizationError::Publication)?;
    if psi_selections.selections().is_empty()
        && published.plan() != published.verified_input().plan()
    {
        return Err(AbstractOptimizationError::EmptySelectionChangedPlan);
    }
    Ok(published)
}

#[cfg(any(test, feature = "test-support"))]
pub fn run_psi_pipeline(
    verified: VerifiedPsiOptimizationUnit,
    selections: &OptimizationSelections,
    budget_per_pass: OptimizationWorkBudget,
) -> Result<OptimizationRun, OptimizationRunError> {
    let projection = selections.project_psi();
    run_psi_pipeline_for_projection(verified, selections, &projection, budget_per_pass)
}

pub(crate) fn run_psi_pipeline_for_projection(
    verified: VerifiedPsiOptimizationUnit,
    selections: &OptimizationSelections,
    projection: &PsiOptimizationSelectionProjection,
    budget_per_pass: OptimizationWorkBudget,
) -> Result<OptimizationRun, OptimizationRunError> {
    if projection.complete_selection() != selections.identity() {
        return Err(OptimizationRunError::SelectionRegistryMismatch);
    }
    let steps = built_in_psi_steps_for_selections(projection.selections())
        .map_err(OptimizationRunError::RegistryConstruction)?;
    let session = if steps.is_empty() {
        let (input, unit) = verified.into_parts();
        VerifiedPsiOptimizationSession::from_transformed(input, unit)
    } else {
        VerifiedPsiOptimizationSession::new(verified)
    }
    .map_err(OptimizationRunError::InitialValidation)?;
    run_registries(
        session,
        selections,
        projection.selections(),
        &steps,
        budget_per_pass,
    )
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
