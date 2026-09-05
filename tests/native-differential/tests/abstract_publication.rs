//! Cross-stage abstract optimization publication and target-lowering controls.

use abstract_operations_to_abstract_operations::*;
use optimization_core::OptimizationWorkUsage;

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
