//! Optimizer module role: executable entrance. Checked-tree optimization phase.
//!
//! The `CheckedTrees` execution phase runs after every authored declaration has
//! been parsed, resolved, typed, and checked, and before selected-execution
//! settlement derives sidecars from the product. An empty phase projection is
//! the identity boundary; each named member must route through its owning
//! transform here rather than selecting a different pipeline.

use crate::checking::phase_transitions::CheckedProgramSurface;
use diagnostics::Diagnostic;
use optimization_core::{Optimization, OptimizationExecutionPhase, OptimizationSelections};
use std::sync::Arc;
use typed_trees_to_checked_trees::{
    CheckedTreeProductPruning, CheckedTreeProductRoots, CheckedTreeProductSelection,
};

/// Execute each optimization the effective build selection assigns to the
/// checked-tree phase, in canonical selection order.
///
/// `product_root_machines` is the coordinator's exact root translation: the
/// validated program-entry binding for a selected target, or every authored
/// root binding for a targetless shared product. The phase never infers roots
/// itself. Returns the surface — transformed only by named members — and the
/// identity-bearing product selection evidence; `None` when no checked-tree
/// optimization ran.
pub(crate) fn execute(
    mut checked: CheckedProgramSurface,
    effective: &OptimizationSelections,
    product_root_machines: Vec<symbols::SymbolHandle>,
) -> Result<(CheckedProgramSurface, Option<CheckedTreeProductSelection>), Vec<Diagnostic>> {
    let phase = effective.project_phase(OptimizationExecutionPhase::CheckedTrees);
    let mut product_selection = None;
    for optimization in phase.selections().as_slice() {
        match optimization {
            Optimization::CheckedTreeProductPruning => {
                let roots = CheckedTreeProductRoots::new(product_root_machines.iter().copied())
                    .map_err(|error| {
                        vec![Diagnostic::error(format!(
                            "checked-tree product root selection is not canonical: {error:?}"
                        ))]
                    })?;
                if roots.machines().is_empty() {
                    return Err(vec![Diagnostic::error(
                        "CheckedTreeProductPruning requires at least one bound product root; this compilation's build bound none",
                    )]);
                }
                let plan = CheckedTreeProductPruning::new(roots);
                let program =
                    Arc::try_unwrap(checked.program).unwrap_or_else(|shared| (*shared).clone());
                let outcome =
                    typed_trees_to_checked_trees::prune_checked_tree_product(program, &plan)?;
                checked.program = Arc::new(outcome.checked);
                product_selection = Some(outcome.selection);
            }
            other => {
                return Err(vec![Diagnostic::error(format!(
                    "the checked-tree optimization phase has no route for `{}`",
                    other.build_case_name()
                ))]);
            }
        }
    }
    Ok((checked, product_selection))
}
