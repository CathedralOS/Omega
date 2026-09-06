use crate::checks;
use crate::facts::build_check_facts;
use crate::validation::validate_typed_program;
use checked_trees::CheckedTrees;

pub(crate) fn lower_typed_trees(
    program: typed_trees::TypedTrees,
    selected_generic_operator_providers: &[crate::SelectedGenericOperatorProviderSpecialization],
    opaque_property_receipts: &[validation::OpaqueDataPropertyReceipt],
) -> Result<CheckedTrees, Vec<diagnostics::Diagnostic>> {
    lower_typed_trees_with_policy(
        program,
        true,
        false,
        selected_generic_operator_providers,
        opaque_property_receipts,
        false,
    )
}

pub(crate) fn lower_preliminary_typed_trees(
    program: typed_trees::TypedTrees,
) -> Result<CheckedTrees, Vec<diagnostics::Diagnostic>> {
    lower_typed_trees_with_policy(program, true, true, &[], &[], true)
}

pub(crate) fn lower_package_typed_trees(
    program: typed_trees::TypedTrees,
    selected_generic_operator_providers: &[crate::SelectedGenericOperatorProviderSpecialization],
    opaque_property_receipts: &[validation::OpaqueDataPropertyReceipt],
) -> Result<CheckedTrees, Vec<diagnostics::Diagnostic>> {
    lower_typed_trees_with_policy(
        program,
        true,
        true,
        selected_generic_operator_providers,
        opaque_property_receipts,
        false,
    )
}

#[cfg(test)]
pub(crate) fn lower_typed_trees_for_crash_fact_inspection(
    program: typed_trees::TypedTrees,
) -> Result<CheckedTrees, Vec<diagnostics::Diagnostic>> {
    lower_typed_trees_with_policy(program, false, false, &[], &[], false)
}

fn lower_typed_trees_with_policy(
    program: typed_trees::TypedTrees,
    enforce_crash_admission: bool,
    allow_unresolved_toolchain_selections: bool,
    selected_generic_operator_providers: &[crate::SelectedGenericOperatorProviderSpecialization],
    opaque_property_receipts: &[validation::OpaqueDataPropertyReceipt],
    allow_pending_opaque_copy: bool,
) -> Result<CheckedTrees, Vec<diagnostics::Diagnostic>> {
    // Stage-1 machine monomorphization MUST precede validation: a generic
    // machine whose value calls agree on one instantiation is substituted to a
    // concrete machine here. Validation permits unused template bodies but the
    // generic-value-call fence still rejects any emitted concrete caller whose
    // callee remains generic (an incomplete specialization).
    let mut program = program;
    // Projected statement receivers retain their lexical root before their
    // field endpoint is known. Resolve both call forms before specialization
    // and effect inference, not only when finalizing authored selections.
    crate::lookup::resolve_projected_receiver_calls(&mut program)?;
    // MP2b must judge the authored requirement -> selected implementation edge
    // before MP4 consumes the call-site selections and clears the template's
    // parameter list.
    // PDI3 operation/algebra authority is part of the authored generic
    // contract, so bind it before specialization captures the universal
    // template fingerprint. Normalize again afterward to cover cloned
    // expression handles and concrete substitutions.
    crate::authored_selections::bind_pre_specialization_authored_selections(&mut program)
        .map_err(|diagnostic| vec![diagnostic])?;
    crate::normalize_open_index_identities(&mut program)?;
    // Keep the authored generic provider templates immutable while ordinary
    // machine specialization closes caller binders. Selected providers may be
    // demanded only by applications copied into those newly concrete bodies,
    // and a newly selected provider may itself expose another ordinary or
    // selected generic application. Alternate the two existing elaborators to
    // a fixed point; neither open applications nor template mutation may be
    // mistaken for final D29 coverage.
    let selected_provider_templates = program.clone();
    let mut nominal_machine_uses =
        crate::specialize_static_machine_calls_with_nominal_uses(&mut program)?;
    crate::normalize_open_index_identities(&mut program)?;
    loop {
        let materialized = crate::monomorphization::specialize_selected_generic_operator_providers(
            &selected_provider_templates,
            &mut program,
            selected_generic_operator_providers,
        )?;
        if materialized == 0 {
            break;
        }
        nominal_machine_uses =
            crate::specialize_static_machine_calls_with_nominal_uses(&mut program)?;
        crate::normalize_open_index_identities(&mut program)?;
    }
    // F2b: unsuffixed float literals at declared f32/f64 destinations land
    // their format on the text carrier HERE, while the tree is still mutable
    // and before both engines fork off it -- every downstream read (native
    // and interpreter) then rounds once from the spelling.
    validation::land_float_literal_destinations(&mut program);
    // Calls through a typed `dyn Trait` receiver cannot be resolved during the
    // earlier symbol pass because local declared types are not available there.
    // Bind their declaring-trait requirement now so the ordinary result-
    // overload pass below starts in the correct trait family rather than from
    // an ambient same-named machine.
    validation::resolve_dynamic_call_targets(&mut program)?;
    // Concrete substitutions may make previously open field types selectable.
    crate::lookup::resolve_projected_receiver_calls(&mut program)?;
    // Named-machine result overloads are provisionally bound to the first
    // same-named symbol during early resolution. Rebind them now, after domain
    // normalization and destination typing, before validation/backend facts
    // consume the call identity.
    validation::resolve_named_result_overloads(&mut program)?;
    let validated = validate_typed_program(
        &program,
        opaque_property_receipts,
        allow_pending_opaque_copy,
    )?;
    let mut facts = build_check_facts(
        &program,
        &validated.proof_plan,
        validated.operational,
        &validated.validation_facts,
        nominal_machine_uses,
    )?;
    checks::initialize_checked_direct_borrow_resources(&program, &mut facts)?;

    // MP5: specialization selection happens before checked contract plans
    // exist. Bind the selected machines' normalized contract identities now,
    // validate the recorded relation, and make those identities part of the
    // instance fingerprint used by caches and artifacts.
    crate::monomorphization::bind_specialization_contract_identities(
        &mut program,
        &facts.contract_plans,
    )?;

    if enforce_crash_admission {
        checks::check_checked_facts_recording(&program, &mut facts)?;
    } else {
        #[cfg(test)]
        checks::check_checked_facts_recording_without_crash_admission(&program, &mut facts)?;
        #[cfg(not(test))]
        unreachable!("production lowering always enforces crash admission");
    }
    crate::facts::refresh_realized_contract_envelopes(&mut facts);

    // This plan must be assembled only after multiplicity and carry checking:
    // their ownership events and claim policies are the authority for the
    // structural/Unit terminal slice.
    facts.flow.terminal_structural_control_cleanups =
        crate::flow::build_checked_structural_control_cleanup_plans(&program, &facts);
    facts.flow.terminal_structural_unit_controls =
        crate::flow::build_checked_structural_unit_control_plans(&program, &facts);
    facts.flow.terminal_structural_returns =
        crate::flow::build_checked_structural_return_plans(&program, &facts);
    facts.flow.terminal_structural_call_returns =
        crate::flow::build_checked_structural_call_return_plans(
            &program,
            &facts,
            &facts.flow.terminal_structural_returns,
        );
    // Boundary-return bodies are independent of the Unit closure. Retain
    // their real plans before deciding which ordinary scalar callees exist.
    facts.flow.terminal_boundary_scalar_returns =
        crate::flow::build_checked_boundary_scalar_return_plans(&program, &facts);
    let terminal_unit_effects =
        crate::flow::build_checked_unit_effect_plans(&program, &facts, &[], &[]);
    let mut cleanup_diagnostics = Vec::new();
    facts.flow.terminal_structural_scalar_returns =
        crate::flow::build_checked_structural_scalar_return_plans(
            &program,
            &facts,
            &terminal_unit_effects,
            &[],
            &mut cleanup_diagnostics,
        );
    facts.flow.terminal_partial_affine_unit_cleanups =
        crate::flow::build_checked_partial_affine_unit_cleanup_plans(
            &program,
            &facts,
            &terminal_unit_effects,
        );
    facts.flow.terminal_nominal_affine_unit_cleanups =
        crate::flow::build_checked_nominal_affine_unit_cleanup_plans(
            &program,
            &facts,
            &terminal_unit_effects,
            &mut cleanup_diagnostics,
        );
    if !cleanup_diagnostics.is_empty() {
        return Err(cleanup_diagnostics);
    }
    facts.flow.terminal_unit_effects = terminal_unit_effects;
    facts.flow.semantic_dependencies =
        crate::flow::derive_checked_semantic_dependencies(&program, &facts);

    crate::authored_selections::bind_checked_intrinsic_call_facts(&program, &mut facts)
        .map_err(|diagnostic| vec![diagnostic])?;
    if allow_unresolved_toolchain_selections {
        crate::authored_selections::finalize_preliminary_checked_authored_selections(
            &mut program,
            &facts,
        )
    } else {
        crate::authored_selections::finalize_checked_authored_selections(&mut program, &facts)
    }
    .map_err(|diagnostic| vec![diagnostic])?;
    validation::validate_reserved_cleanup_selections(&program)?;
    validation::validate_declaration_visibility(&program)?;

    Ok(CheckedTrees::with_roots(program, facts))
}
