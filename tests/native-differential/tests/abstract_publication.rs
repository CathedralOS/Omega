//! Cross-stage abstract optimization publication and target-lowering controls.

use omega_abstract_operations_optimizer::*;
use omega_optimization_core::OptimizationWorkUsage;

#[path = "abstract_publication/mod.rs"]
mod tests;

const fn work_usage(usage: OptimizationRunUsage) -> OptimizationWorkUsage {
    OptimizationWorkUsage {
        rule_evaluations: usage.rule_evaluations,
        candidates: usage.candidates,
        validation_steps: usage.validation_steps,
        commits: usage.commits,
        iterations: usage.iterations,
    }
}
