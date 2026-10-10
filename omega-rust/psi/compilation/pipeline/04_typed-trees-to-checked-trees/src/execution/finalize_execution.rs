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
    crate::execution::scalar_computations::record_rank_observations(program, &mut facts);
    facts.flow.terminal_machines =
        crate::execution::terminal_scalar::build_checked_terminal_machine_selections(
            program,
            &facts.termination.rank_arrivals,
        );
    crate::execution::citation_erasure::retire_fact_only_citations(program, &mut facts);
    // This plan must be assembled only after multiplicity and carry checking:
    // their ownership events and claim policies are the authority for the
    // structural/Unit terminal slice.
    facts.flow.terminal_structural_control_cleanups =
        crate::execution::terminal_cleanup::build_checked_structural_control_cleanup_plans(
            program, &facts,
        );
    let mut cleanup_diagnostics = Vec::new();
    // The Unit candidates pass assembles each free `drop<T>` consumer's
    // complete nominal cleanup plan once and publishes that plan's `.machine`
    // as the consumer's own ordinary row; attached callers assemble below.
    let mut free_nominal_cleanups = Vec::new();
    let mut terminal_unit_effects =
        crate::execution::terminal_unit::build_checked_unit_effect_plans(
            program,
            &facts,
            crate::execution::terminal_unit::ScalarCalleeClosure,
            call_frames.as_ref(),
            &mut free_nominal_cleanups,
            &mut cleanup_diagnostics,
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
    let (mut nominal_cleanups, nominal_types) =
        crate::execution::terminal_unit::build_checked_nominal_affine_unit_cleanup_plans(
            program,
            &facts,
            &terminal_unit_effects,
            &mut cleanup_diagnostics,
        );
    nominal_cleanups
        .machines
        .splice(0..0, free_nominal_cleanups);
    // Every nominal dispatcher publishes inside the ordinary roster: lower
    // resolves the machine's plan row there rather than staging a second one.
    for published in terminal_unit_effects.machines.iter_mut() {
        if let Some(nominal) = nominal_cleanups
            .machines
            .iter()
            .find(|plan| plan.machine.machine == published.machine)
        {
            *published = nominal.machine.clone();
        }
    }
    facts.flow.terminal_nominal_affine_unit_cleanups = nominal_cleanups;
    if let Err(identity) =
        terminal_unit_effects.adopt_structural_types(partial_types.into_iter().chain(nominal_types))
    {
        cleanup_diagnostics.push(Diagnostic::error(format!(
            "Unit cleanup plans register structural type `{identity}` with a shape the ordinary Unit roster does not carry"
        )));
    }
    // A scalar computation that constructs a record (`Pair { .. }.first`)
    // names that record's shape though no signature or local declares it.
    // Each Unit body's computations carry those shapes into the one roster.
    let mut construction_shapes = Vec::new();
    for state in terminal_unit_effects
        .machines
        .iter()
        .map(|plan| plan.state)
        .chain(
            terminal_unit_effects
                .composed_machines
                .iter()
                .flat_map(|plan| plan.states.iter().map(|state| state.state)),
        )
    {
        crate::execution::terminal_scalar::constructions::retain_shapes(
            program,
            &facts.values.scalar_computations,
            &facts.values.structural_values,
            state,
            &mut construction_shapes,
        );
    }
    if let Err(identity) = terminal_unit_effects.adopt_structural_types(construction_shapes) {
        cleanup_diagnostics.push(Diagnostic::error(format!(
            "scalar computations construct structural type `{identity}` with a shape the ordinary Unit roster does not carry"
        )));
    }
    if !cleanup_diagnostics.is_empty() {
        return Err(cleanup_diagnostics);
    }
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
