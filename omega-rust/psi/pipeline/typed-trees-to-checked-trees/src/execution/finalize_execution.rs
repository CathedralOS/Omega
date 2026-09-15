use checked_trees::CheckFacts;
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;

pub(crate) fn finalize_execution(
    program: &TypedTrees,
    mut facts: CheckFacts,
) -> Result<CheckFacts, Vec<Diagnostic>> {
    // Finalize the discovered graph shapes against completed ownership facts.
    crate::execution::finalize_checked_scalar_graph_plans(
        program,
        &facts.flow.ownership,
        &facts.values.scalar_computations,
        &mut facts.flow.terminal_scalar_graphs,
    );

    crate::execution::finalize_scalar_unit_operations(program, &mut facts);

    // This plan must be assembled only after multiplicity and carry checking:
    // their ownership events and claim policies are the authority for the
    // structural/Unit terminal slice.
    facts.flow.terminal_structural_control_cleanups =
        crate::execution::build_checked_structural_control_cleanup_plans(program, &facts);
    facts.flow.terminal_structural_unit_controls =
        crate::execution::build_checked_structural_unit_control_plans(program, &facts);
    facts.flow.terminal_structural_returns =
        crate::execution::build_checked_structural_return_plans(program, &facts);
    facts.flow.terminal_structural_call_returns =
        crate::execution::build_checked_structural_call_return_plans(
            program,
            &facts,
            &facts.flow.terminal_structural_returns,
        );
    let crate::execution::execution_plans::ExecutionPlans {
        boundary_returns,
        unit_effects: terminal_unit_effects,
        structural_scalar_returns,
        mut cleanup_diagnostics,
    } = crate::execution::execution_plans::build_execution_plans(program, &facts, None, &[], &[]);
    facts.flow.terminal_partial_affine_unit_cleanups =
        crate::execution::build_checked_partial_affine_unit_cleanup_plans(
            program,
            &facts,
            &terminal_unit_effects,
        );
    facts.flow.terminal_nominal_affine_unit_cleanups =
        crate::execution::build_checked_nominal_affine_unit_cleanup_plans(
            program,
            &facts,
            &terminal_unit_effects,
            &mut cleanup_diagnostics,
        );
    if !cleanup_diagnostics.is_empty() {
        return Err(cleanup_diagnostics);
    }
    facts.flow.terminal_boundary_scalar_returns = boundary_returns;
    facts.flow.terminal_structural_scalar_returns = structural_scalar_returns;
    facts.flow.terminal_unit_effects = terminal_unit_effects;

    Ok(facts)
}
