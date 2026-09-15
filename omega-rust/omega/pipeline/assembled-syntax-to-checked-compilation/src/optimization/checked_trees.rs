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

#[cfg(test)]
mod tests {
    use super::execute;
    use crate::checking::phase_transitions::CheckedProgramSurface;
    use checked_trees::{CheckFacts, CheckedTrees};
    use language_semantics::MachineSupplyMode;
    use optimization_core::{Optimization, OptimizationSelections};
    use std::sync::Arc;
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;
    use typed_trees::machine::Machine;

    fn machine(supply_mode: MachineSupplyMode, arena_index: u32) -> Machine {
        Machine {
            symbol: SymbolHandle::from_arena_index(arena_index),
            supply_mode,
            body_is_present: false,
            ..Default::default()
        }
    }

    fn surface(typed: TypedTrees) -> CheckedProgramSurface {
        let accepted_template_classifications =
            trust_model::AcceptedTemplateClassifications::capture(&typed);
        CheckedProgramSurface {
            program: Arc::new(CheckedTrees::with_roots(typed, CheckFacts::default())),
            selected_provider_plan_facts: effects::SelectedProviderPlanFacts::default(),
            selected_provider_grants: Vec::new(),
            callback_placements: Vec::new(),
            accepted_template_classifications,
            contract_entailment_stand_downs: Vec::new(),
        }
    }

    fn product_pruning_selection() -> OptimizationSelections {
        OptimizationSelections::new([Optimization::CheckedTreeProductPruning])
            .expect("the checked-tree product-pruning selection is canonical")
    }

    #[test]
    fn an_unselected_phase_is_an_identity_boundary_with_no_product_selection() {
        let mut typed = TypedTrees::default();
        typed.push_machine(machine(MachineSupplyMode::CheckedBody, 1));
        let surface = surface(typed);
        let program = Arc::clone(&surface.program);

        let (surface, selection) = execute(surface, &OptimizationSelections::default(), Vec::new())
            .expect("an empty projection runs no checked-tree optimization");

        // Disabled is the identity boundary: the surface passes through
        // untouched and no selection evidence is fabricated.
        assert!(selection.is_none());
        assert!(Arc::ptr_eq(&program, &surface.program));
        assert_eq!(surface.program.machines().len(), 1);
    }

    #[test]
    fn a_selection_for_another_phase_never_routes_here() {
        let mut typed = TypedTrees::default();
        typed.push_machine(machine(MachineSupplyMode::CheckedBody, 1));
        let selections =
            OptimizationSelections::new([Optimization::ControlFlowCleanup]).expect("selections");
        let surface = surface(typed);
        let program = Arc::clone(&surface.program);

        let (surface, selection) = execute(surface, &selections, Vec::new())
            .expect("a Psi-phase selection never reaches the checked-tree phase");

        assert!(selection.is_none());
        assert!(Arc::ptr_eq(&program, &surface.program));
        assert_eq!(surface.program.machines().len(), 1);
    }

    #[test]
    fn selected_product_pruning_requires_a_bound_product_root() {
        let Err(diagnostics) = execute(
            surface(TypedTrees::default()),
            &product_pruning_selection(),
            Vec::new(),
        ) else {
            panic!("an unbound root set must reject before any mutation");
        };
        assert!(diagnostics.iter().any(|diagnostic| {
            format!("{diagnostic:?}").contains("requires at least one bound product root")
        }));
    }

    #[test]
    fn selected_product_pruning_propagates_the_unknown_root_rejection() {
        let mut typed = TypedTrees::default();
        typed.push_machine(machine(MachineSupplyMode::CheckedBody, 1));

        let Err(diagnostics) = execute(
            surface(typed),
            &product_pruning_selection(),
            vec![SymbolHandle::from_arena_index(4_242)],
        ) else {
            panic!("a root that names no declared machine must reject");
        };
        assert!(diagnostics.iter().any(|diagnostic| {
            format!("{diagnostic:?}").contains("does not name a declared machine")
        }));
    }

    #[test]
    fn selected_product_pruning_prunes_unreachable_bodies_and_publishes_the_selection() {
        let boundary = SymbolHandle::from_arena_index(1);
        let detached = SymbolHandle::from_arena_index(2);
        let mut typed = TypedTrees::default();
        typed.push_machine(machine(MachineSupplyMode::Boundary, 1));
        typed.push_machine(machine(MachineSupplyMode::CheckedBody, 2));

        let (surface, selection) =
            execute(surface(typed), &product_pruning_selection(), vec![boundary])
                .expect("pruning a detached checked body succeeds");
        let selection = selection.expect("a selected pruning publishes selection evidence");

        assert_eq!(selection.roots().machines(), &[boundary]);
        assert_eq!(selection.retained_machines(), &[boundary]);
        assert_eq!(selection.pruned_machines(), &[detached]);
        assert_eq!(surface.program.machines().len(), 1);
        assert_eq!(surface.program.machines()[0].symbol, boundary);
    }
}
