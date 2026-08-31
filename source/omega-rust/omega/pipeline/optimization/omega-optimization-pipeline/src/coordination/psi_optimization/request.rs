use omega_optimization_core::{OptimizationSelections, OptimizationWorkBudget};

/// Exact optimizer inputs chosen by the compiler coordinator.
///
/// Construction rejects the empty selection so compatibility builds cannot
/// accidentally enter the optimizer or manufacture optimizer work records.
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

/// Compiler-owned bounded baseline for the experimental optimized lane.
/// Every value is a per-pass-group ceiling; this is not a source-visible
/// optimization level or an intensity preset.
pub fn compiler_baseline_request_v1(
    selections: &OptimizationSelections,
) -> Result<ExplicitOptimizationRequest, EmptyOptimizationSelections> {
    ExplicitOptimizationRequest::new(selections.clone(), compiler_baseline_budget_v1())
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
