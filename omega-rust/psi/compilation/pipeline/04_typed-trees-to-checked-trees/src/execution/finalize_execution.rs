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
    phase_profile::scope("05.d.8_10.a.1.rank_and_terminal_machines", || {
        crate::execution::scalar_computations::record_rank_observations(program, &mut facts);
        facts.flow.terminal_machines =
            crate::execution::terminal_scalar::build_checked_terminal_machine_selections(
                program,
                &facts.termination.rank_arrivals,
            );
        crate::execution::citation_erasure::retire_fact_only_citations(program, &mut facts);
    });
    // This plan must be assembled only after multiplicity and carry checking:
    // their ownership events and claim policies are the authority for the
    // structural/Unit terminal slice.
    facts.flow.terminal_structural_control_cleanups =
        phase_profile::scope("05.d.8_10.a.2.structural_control_cleanups", || {
            crate::execution::terminal_cleanup::build_checked_structural_control_cleanup_plans(
                program, &facts,
            )
        });
    let mut cleanup_diagnostics = Vec::new();
    // Contracts are declaration-owned, not a summary of whichever calls
    // happen to exist. Intern their complete target-neutral reach once before
    // immutable planning so whole-provider defaults retain unused methods.
    let boundary_signature_reaches = program
        .traits()
        .iter()
        .filter(|owner| owner.is_boundary)
        .flat_map(|owner| program.trait_machine_instances(owner))
        .filter_map(|signature| {
            validation::boundary_signature_service_reach(program, signature.symbol).map(
                |services| {
                    (
                        signature.symbol,
                        facts.service_reaches.rows.intern(services),
                    )
                },
            )
        })
        .collect::<Vec<_>>();
    let (mut terminal_unit_effects, partial_cleanups, prepared_value_calls) =
        phase_profile::scope("05.d.8_10.a.3.unit_effect_plans", || {
            crate::execution::terminal_unit::build_checked_unit_effect_plans(
                program,
                &mut facts,
                &boundary_signature_reaches,
                crate::execution::terminal_unit::ScalarCalleeClosure,
                call_frames.as_ref(),
            )
        });
    // The partial-affine roster is built beside the ordinary one, from the
    // same body plans, so its shapes already live in the roster's one type
    // namespace. Nominal metadata retains caller prerequisites, while its
    // executable body and normal-exit actions remain in the ordinary roster.
    // A body that kills a same-statement temporary on a shared
    // call continuation lowers through the ordinary lane and replays residual
    // reconstruction against these shapes, so a type referenced only by a
    // cleanup machine must still resolve there. Identities are deterministic:
    // a lane that registered a different shape under a published identity is
    // a planning defect, not a choice for lowering to make.
    facts.flow.terminal_partial_affine_unit_cleanups = partial_cleanups;
    let (nominal_cleanups, nominal_types) =
        phase_profile::scope("05.d.8_10.a.4.nominal_affine_cleanups", || {
            crate::execution::terminal_unit::build_checked_nominal_affine_unit_cleanup_plans(
                program,
                &facts,
                &terminal_unit_effects,
                &mut cleanup_diagnostics,
            )
        });
    facts.flow.terminal_nominal_affine_unit_cleanups = nominal_cleanups;
    if let Err(identity) = terminal_unit_effects.adopt_structural_types(nominal_types) {
        cleanup_diagnostics.push(Diagnostic::error(format!(
            "Unit cleanup plans register structural type `{}` with a shape the ordinary Unit roster does not carry",
            program.type_identity_text(identity).unwrap_or_default()
        )));
    }
    // A scalar computation that constructs a record (`Pair { .. }.first`)
    // names that record's shape though no signature or local declares it.
    // Each Unit body's computations carry those shapes into the one roster.
    let mut construction_shapes = Vec::new();
    for state in terminal_unit_effects
        .machines
        .iter()
        .chain(
            facts
                .flow
                .terminal_partial_affine_unit_cleanups
                .machines
                .iter()
                .map(|plan| &plan.machine),
        )
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
        )
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "cannot retain constructed value storage shapes for state `{}` of machine `{}`",
                program.symbols.name(state),
                validation::find_state(program, state)
                    .map(|(machine, _)| machine.name.as_str())
                    .unwrap_or("<unknown>"),
            ))]
        })?;
    }
    if let Err(identity) = terminal_unit_effects.adopt_structural_types(construction_shapes) {
        cleanup_diagnostics.push(Diagnostic::error(format!(
            "scalar computations construct structural type `{}` with a shape the ordinary Unit roster does not carry",
            program.type_identity_text(identity).unwrap_or_default()
        )));
    }
    if !cleanup_diagnostics.is_empty() {
        return Err(cleanup_diagnostics);
    }
    facts.flow.terminal_unit_effects = terminal_unit_effects;
    crate::execution::terminal_unit::retain_structural_value_calls(
        &mut facts,
        prepared_value_calls,
    )
    .map_err(|reason| {
        vec![Diagnostic::error(format!(
            "cannot retain structural value calls: {reason}"
        ))]
    })?;

    // Publish all value-call schedules before selecting their normal-return
    // cleanup. A later schedule must not retain provisional disposal seeds.
    crate::execution::terminal_unit::cleanup::finalize_completion_actions(program, &mut facts)
        .map_err(|reason| {
            vec![Diagnostic::error(format!(
                "cannot plan contextual call cleanup against the executable hook roster: {reason}"
            ))]
        })?;

    Ok(facts)
}
