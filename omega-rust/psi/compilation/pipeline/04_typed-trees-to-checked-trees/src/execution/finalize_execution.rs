use checked_trees::CheckFacts;
use diagnostics::Diagnostic;

pub(crate) fn finalize_execution(
    program: &dyn ::validation::CheckingInput,
    mut facts: CheckFacts,
) -> Result<CheckFacts, Vec<Diagnostic>> {
    // Execution planning runs in its own immutable window, after the
    // contract-identity mutation that ends the check pass's resolver lifetime.
    // One frame resolver answers every planner that still classifies typed
    // call frames.
    let call_frames = validation::CallFrameResolver::new(program);
    crate::execution::citation_erasure::retire_fact_only_citations(program, &mut facts);
    // Finalize the discovered graph shapes against completed ownership facts.
    crate::execution::terminal_scalar::finalize_checked_scalar_graph_plans(
        program,
        &facts.flow.ownership,
        &facts.flow.control,
        &facts.values.scalar_computations,
        &facts.values.structural_values,
        &mut facts.flow.terminal_scalar_graphs,
    );

    crate::execution::terminal_scalar::finalize_scalar_unit_operations(program, &mut facts);

    // This plan must be assembled only after multiplicity and carry checking:
    // their ownership events and claim policies are the authority for the
    // structural/Unit terminal slice.
    facts.flow.terminal_structural_control_cleanups =
        crate::execution::terminal_cleanup::build_checked_structural_control_cleanup_plans(
            program, &facts,
        );
    facts.flow.terminal_structural_returns =
        crate::execution::terminal_unit::returns::build_checked_structural_return_plans(
            program, &facts,
        );
    let crate::execution::execution_plans::ExecutionPlans {
        boundary_returns,
        unit_effects: mut terminal_unit_effects,
        structural_scalar_returns,
        mut cleanup_diagnostics,
    } = crate::execution::execution_plans::build_execution_plans(
        program,
        &facts,
        call_frames.as_ref(),
    );
    // Both cleanup lanes keep their dispatchers apart from the ordinary
    // roster, but every shape a Unit body names lives in that roster's one
    // type namespace. A body that kills a same-statement temporary on a shared
    // call continuation lowers through the ordinary lane and replays residual
    // reconstruction against these shapes, so a type referenced only by a
    // cleanup machine must still resolve there. Identities are deterministic:
    // a lane that registered a different shape under a published identity is
    // a planning defect, not a choice for lowering to make.
    let (partial_cleanups, partial_types) =
        crate::execution::terminal_unit::build_checked_partial_affine_unit_cleanup_plans(
            program,
            &facts,
            &terminal_unit_effects,
        );
    facts.flow.terminal_partial_affine_unit_cleanups = partial_cleanups;
    let (nominal_cleanups, nominal_types) =
        crate::execution::terminal_unit::build_checked_nominal_affine_unit_cleanup_plans(
            program,
            &facts,
            &terminal_unit_effects,
            &mut cleanup_diagnostics,
        );
    facts.flow.terminal_nominal_affine_unit_cleanups = nominal_cleanups;
    if let Err(identity) =
        terminal_unit_effects.adopt_structural_types(partial_types.into_iter().chain(nominal_types))
    {
        cleanup_diagnostics.push(Diagnostic::error(format!(
            "Unit cleanup plans register structural type `{identity}` with a shape the ordinary Unit roster does not carry"
        )));
    }
    if !cleanup_diagnostics.is_empty() {
        return Err(cleanup_diagnostics);
    }
    facts.flow.terminal_boundary_scalar_returns = boundary_returns;
    facts.flow.terminal_structural_scalar_returns = structural_scalar_returns;
    facts.flow.terminal_unit_effects = terminal_unit_effects;
    crate::execution::terminal_unit::cleanup::finalize_scalar_actions(program, &mut facts)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "cannot plan contextual temporary cleanup against the executable hook roster",
            )]
        })?;
    crate::execution::terminal_unit::retain_structural_value_calls(program, &mut facts).map_err(
        |identity| {
            vec![Diagnostic::error(format!(
                "structural value calls register structural type `{identity}` with a shape the ordinary Unit roster does not carry"
            ))]
        },
    )?;

    Ok(facts)
}
