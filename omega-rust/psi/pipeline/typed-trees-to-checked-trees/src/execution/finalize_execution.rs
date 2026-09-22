use checked_trees::CheckFacts;
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;

/// The provider-settled execution the Terminal plan lanes plan against:
/// empty for ordinary checking, the exact selected applications when the
/// orchestration owner settles execution.
#[derive(Clone, Copy, Default)]
pub(crate) struct SelectedExecution<'a> {
    pub(crate) operator_applications: &'a [crate::SelectedOperatorApplication],
    pub(crate) ieee_float_fma_unit_applications: &'a [crate::SelectedIeeeFloatFmaUnitApplication],
}

pub(crate) fn finalize_execution(
    program: &TypedTrees,
    mut facts: CheckFacts,
    selected: SelectedExecution<'_>,
) -> Result<CheckFacts, Vec<Diagnostic>> {
    // Execution planning runs in its own immutable window, after the
    // contract-identity mutation that ends the check pass's resolver lifetime.
    // One frame resolver answers every planner that still classifies typed
    // call frames.
    let call_frames = validation::CallFrameResolver::new(program);
    // Finalize the discovered graph shapes against completed ownership facts.
    crate::execution::terminal_scalar::finalize_checked_scalar_graph_plans_with_call_frames(
        program,
        &facts.flow.ownership,
        &facts.values.scalar_computations,
        &mut facts.flow.terminal_scalar_graphs,
        call_frames.as_ref(),
    );

    crate::execution::terminal_scalar::finalize_scalar_unit_operations(program, &mut facts);

    // This plan must be assembled only after multiplicity and carry checking:
    // their ownership events and claim policies are the authority for the
    // structural/Unit terminal slice.
    facts.flow.terminal_structural_control_cleanups =
        crate::execution::terminal_cleanup::build_checked_structural_control_cleanup_plans(
            program, &facts,
        );
    facts.flow.terminal_structural_unit_controls =
        crate::execution::terminal_unit::control::build_checked_structural_unit_control_plans(
            program,
            &facts,
            call_frames.as_ref(),
        );
    facts.flow.terminal_structural_returns =
        crate::execution::terminal_unit::returns::build_checked_structural_return_plans(
            program, &facts,
        );
    facts.flow.terminal_structural_call_returns =
        crate::execution::terminal_unit::returns::build_checked_structural_call_return_plans(
            program,
            &facts,
            &facts.flow.terminal_structural_returns,
        );
    let crate::execution::execution_plans::ExecutionPlans {
        boundary_returns,
        unit_effects: terminal_unit_effects,
        structural_scalar_returns,
        mut cleanup_diagnostics,
    } = crate::execution::execution_plans::build_execution_plans(
        program,
        &facts,
        selected.operator_applications,
        selected.ieee_float_fma_unit_applications,
        call_frames.as_ref(),
    );
    facts.flow.terminal_partial_affine_unit_cleanups =
        crate::execution::terminal_unit::build_checked_partial_affine_unit_cleanup_plans(
            program,
            &facts,
            &terminal_unit_effects,
        );
    facts.flow.terminal_nominal_affine_unit_cleanups =
        crate::execution::terminal_unit::build_checked_nominal_affine_unit_cleanup_plans(
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
