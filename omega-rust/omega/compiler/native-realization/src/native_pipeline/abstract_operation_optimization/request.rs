use optimization_core::{
    OptimizationSelections, OptimizationWorkBudget, PsiOptimizationSelectionProjection,
};

/// Canonical input to the post-Terminal abstract-operation optimization phase.
///
/// Empty selections are ordinary identity execution. The phase still admits
/// and validates its input, publishes empty transformation custody, and hands
/// the unchanged representation to the next stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizationPipelineRequest {
    selections: OptimizationSelections,
    psi: PsiOptimizationSelectionProjection,
    budget_per_pass: OptimizationWorkBudget,
}

impl OptimizationPipelineRequest {
    pub fn new(
        selections: OptimizationSelections,
        budget_per_pass: OptimizationWorkBudget,
    ) -> Self {
        let psi = selections.project_psi();
        Self {
            selections,
            psi,
            budget_per_pass,
        }
    }

    pub const fn selections(&self) -> &OptimizationSelections {
        &self.selections
    }

    pub const fn budget_per_pass(&self) -> OptimizationWorkBudget {
        self.budget_per_pass
    }

    pub const fn psi_projection(&self) -> &PsiOptimizationSelectionProjection {
        &self.psi
    }
}

/// Legacy nonempty-only adapter for callers that still use optimization
/// presence to choose a pipeline branch.
///
/// New callers use [`OptimizationPipelineRequest`]. Construction rejects the
/// empty selection so compatibility callers preserve their old branch while
/// they migrate to canonical identity execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplicitOptimizationRequest {
    selections: OptimizationSelections,
    budget_per_pass: OptimizationWorkBudget,
}

impl ExplicitOptimizationRequest {
    pub fn new(
        selections: OptimizationSelections,
        budget_per_pass: OptimizationWorkBudget,
    ) -> Result<Self, EmptyOptimizationSelections> {
        if selections.is_empty() {
            return Err(EmptyOptimizationSelections);
        }
        Ok(Self {
            selections,
            budget_per_pass,
        })
    }

    pub const fn selections(&self) -> &OptimizationSelections {
        &self.selections
    }

    pub const fn budget_per_pass(&self) -> OptimizationWorkBudget {
        self.budget_per_pass
    }
}

impl From<ExplicitOptimizationRequest> for OptimizationPipelineRequest {
    fn from(request: ExplicitOptimizationRequest) -> Self {
        Self::new(request.selections, request.budget_per_pass)
    }
}

/// Compiler-owned bounded baseline for the experimental optimized lane.
/// Every value is a per-pass-group ceiling; this is not a source-visible
/// optimization level or an intensity preset.
pub fn compiler_baseline_request_v1(
    selections: &OptimizationSelections,
) -> OptimizationPipelineRequest {
    OptimizationPipelineRequest::new(selections.clone(), compiler_baseline_budget_v1())
}

fn compiler_baseline_budget_v1() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(1_000_000, 100_000, 100_000, 100_000, 10_000)
        .expect("compiler baseline optimizer ceilings are nonzero")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmptyOptimizationSelections;

impl std::fmt::Display for EmptyOptimizationSelections {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("the explicit optimizer pipeline requires at least one named selection")
    }
}

impl std::error::Error for EmptyOptimizationSelections {}
