//! Rebuild checked execution plans after provider settlement.
//!
//! Settlement changes no checked body: a call or operator application keeps
//! naming the requirement it resolved to, and Omega installs the selected
//! provider. What settlement does add is the dispatch rows a top-level
//! requirement call's Unit plan consumes, so `settle_checked_execution`
//! rebuilds the Terminal plan lanes once against the settled facts.

use crate::checked_trees::CheckedTrees;
use crate::execution::execution_plans::{ExecutionPlans, build_execution_plans};

/// Rebuild the three Terminal plan lanes of a checked program whose facts
/// provider settlement extended. The lanes are published together; a failure
/// publishes nothing.
pub fn settle_checked_execution(
    mut checked: CheckedTrees,
) -> Result<CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let call_frames = crate::validation::CallFrameResolver::new(&checked.typed);
    let ExecutionPlans {
        boundary_returns,
        unit_effects,
        structural_scalar_returns,
        cleanup_diagnostics,
    } = build_execution_plans(&checked.typed, &checked.facts, call_frames.as_ref());
    if !cleanup_diagnostics.is_empty() {
        return Err(cleanup_diagnostics);
    }
    checked.facts.flow.terminal_boundary_scalar_returns = boundary_returns;
    checked.facts.flow.terminal_unit_effects = unit_effects;
    checked.facts.flow.terminal_structural_scalar_returns = structural_scalar_returns;
    Ok(checked)
}
