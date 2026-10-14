//! Optimizer module role: executable entrance. The post-allocation machine
//! phase over the current plan.
//!
//! The phase receives the complete build selection, projects its own
//! post-allocation slice, and executes every selected rule in catalog order
//! against the freshly constructed plan. A rule rewrites instructions onto
//! catalog alternatives the legality rule never chooses
//! (`MachineAlternativeApplicability::OptimizationSelected`), so every
//! optimizer form is declared once by the ISA beside its baseline forms and
//! the rule consumes that declared entry. The run records the phase
//! projection it executed, the plan identities before and after, every
//! action, and the work consumed, which realization binds against the
//! build selection. An empty projection is the phase's identity execution:
//! the same entrance, no actions, equal identities.
//!
//! A selected rule the stage cannot execute, or whose required architecture
//! is not the plan's, rejects by name. Declaring a rule in the catalog is not
//! evidence of support; the rule table here is.

mod rules;
mod unit_liveness;

use optimization_core::{
    Optimization, OptimizationExecutionPhase, OptimizationSelectionIdentity,
    OptimizationSelections, OptimizationWorkBudget, OptimizationWorkUsage,
};
use physical_instructions::{
    PostAllocationMachineIdentity, PostAllocationMachinePlan, post_allocation_machine_identity,
};
use selected_instructions::{
    MachineAlternativeKey, SelectedInstructionId, SelectedInstructionPlan,
};
use selected_instructions_to_register_homes::ValidatedPreAllocationMachineEffects;
use target::Architecture;

/// Execute the post-allocation phase projection of `selections` over `plan`.
/// The plan identity is recomputed afterwards so a rewritten plan is bound by
/// its own content and an untouched plan keeps its source identity.
pub(crate) fn execute_post_allocation_machine_optimizations(
    plan: &mut PostAllocationMachinePlan,
    selected: &SelectedInstructionPlan,
    effects: &ValidatedPreAllocationMachineEffects,
    selections: &OptimizationSelections,
    budget: OptimizationWorkBudget,
) -> Result<PostAllocationMachineOptimizationRun, PostAllocationMachineOptimizationError> {
    let phase = OptimizationExecutionPhase::PostAllocationMachine;
    let projection = selections.project_phase(phase);
    let source = plan.identity;
    let successors = rules::successor_table(selected)?;
    let mut usage = OptimizationWorkUsage {
        iterations: 1,
        ..OptimizationWorkUsage::default()
    };
    let mut executions = Vec::new();
    for optimization in projection.selections().as_slice().iter().copied() {
        let executor = rules::executor(optimization)
            .ok_or(PostAllocationMachineOptimizationError::NoExecutor { optimization })?;
        if plan.target.architecture != executor.required_architecture {
            return Err(PostAllocationMachineOptimizationError::TargetMismatch {
                optimization,
                required: executor.required_architecture,
                actual: plan.target.architecture,
            });
        }
        usage.rule_evaluations = usage.rule_evaluations.saturating_add(1);
        let mut context = rules::RuleContext {
            plan,
            effects,
            successors: &successors,
            usage: &mut usage,
        };
        executions.push((executor.run)(optimization, &mut context)?);
    }
    let extent = plan
        .instructions()
        .try_fold(0_u64, |count, _| count.checked_add(1))
        .ok_or(PostAllocationMachineOptimizationError::CountOverflow)?;
    if !usage.within(budget.for_extent(extent)) {
        return Err(PostAllocationMachineOptimizationError::BudgetExhausted { phase });
    }
    plan.identity = post_allocation_machine_identity(plan);
    Ok(PostAllocationMachineOptimizationRun {
        complete_selection: projection.complete_selection(),
        phase_selections: projection.selections().identity(),
        source,
        result: plan.identity,
        executions,
        usage,
    })
}

/// Evidence of one post-allocation phase execution: which projection ran,
/// over which plan, what it produced, and every action per rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostAllocationMachineOptimizationRun {
    complete_selection: OptimizationSelectionIdentity,
    phase_selections: OptimizationSelectionIdentity,
    source: PostAllocationMachineIdentity,
    result: PostAllocationMachineIdentity,
    executions: Vec<PostAllocationMachineRuleExecution>,
    usage: OptimizationWorkUsage,
}

impl PostAllocationMachineOptimizationRun {
    pub const fn phase(&self) -> OptimizationExecutionPhase {
        OptimizationExecutionPhase::PostAllocationMachine
    }

    /// The identity of the complete build selection the phase was projected from.
    pub const fn complete_selection(&self) -> OptimizationSelectionIdentity {
        self.complete_selection
    }

    /// The identity of the exact post-allocation projection that executed.
    pub const fn phase_selections(&self) -> OptimizationSelectionIdentity {
        self.phase_selections
    }

    /// The plan identity before any rule ran.
    pub const fn source(&self) -> PostAllocationMachineIdentity {
        self.source
    }

    /// The plan identity after every rule ran; equal to `source` when no
    /// rule acted.
    pub const fn result(&self) -> PostAllocationMachineIdentity {
        self.result
    }

    pub fn executions(&self) -> &[PostAllocationMachineRuleExecution] {
        &self.executions
    }

    pub fn action_count(&self) -> usize {
        self.executions
            .iter()
            .map(|execution| execution.actions.len())
            .sum()
    }

    pub const fn usage(&self) -> OptimizationWorkUsage {
        self.usage
    }
}

/// One selected rule's execution: the candidates it examined and the
/// instructions it rewrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostAllocationMachineRuleExecution {
    optimization: Optimization,
    candidates: u64,
    actions: Vec<PostAllocationMachineAction>,
}

impl PostAllocationMachineRuleExecution {
    pub const fn optimization(&self) -> Optimization {
        self.optimization
    }

    pub const fn candidates(&self) -> u64 {
        self.candidates
    }

    pub fn actions(&self) -> &[PostAllocationMachineAction] {
        &self.actions
    }
}

/// One instruction rewritten from its legality-chosen alternative onto the
/// rule's declared optimizer form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PostAllocationMachineAction {
    pub function: usize,
    pub block: usize,
    pub instruction: SelectedInstructionId,
    pub baseline: MachineAlternativeKey,
    pub selected: MachineAlternativeKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostAllocationMachineOptimizationError {
    /// The build selected a post-allocation rule this stage has no executor
    /// for. Catalog presence alone does not execute a rule.
    NoExecutor {
        optimization: Optimization,
    },
    /// The rule is target-specific and the plan realizes another architecture.
    TargetMismatch {
        optimization: Optimization,
        required: Architecture,
        actual: Architecture,
    },
    /// The selected plan, its machine effects and the post-allocation plan
    /// disagree on a function or block roster.
    RosterMismatch {
        function: usize,
        block: usize,
    },
    /// A rule found a candidate whose instruction declares no alternative
    /// tagged with that rule, so the ISA catalog does not support the rewrite.
    MissingDeclaredForm {
        optimization: Optimization,
        function: usize,
        instruction: u32,
    },
    BudgetExhausted {
        phase: OptimizationExecutionPhase,
    },
    CountOverflow,
}

impl std::fmt::Display for PostAllocationMachineOptimizationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "post-allocation machine optimization failed: {self:?}"
        )
    }
}

impl std::error::Error for PostAllocationMachineOptimizationError {}

#[cfg(test)]
mod tests;
