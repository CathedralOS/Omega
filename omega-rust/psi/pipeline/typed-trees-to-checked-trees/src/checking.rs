//! Typed-tree checking: specialize calls, validate contracts and ownership,
//! assemble execution plans, then publish checked trees.
//!
//! Package checkpoints select explicit checking modes; only the test-only
//! crash-inspection mode omits crash admission.

use crate::checks;
use crate::facts::build_check_facts;
use crate::validation::validate_typed_program;
use checked_trees::CheckedTrees;

/// Check a standalone program. Toolchain-owned selections may remain late-bound
/// until build-time evaluation; ordinary authored selections remain strict.
pub fn lower_typed_trees(
    program: typed_trees::TypedTrees,
) -> Result<CheckedTrees, Vec<diagnostics::Diagnostic>> {
    check_program(program, CheckingMode::SettledPackage, &[], &[])
}

fn check_program(
    program: typed_trees::TypedTrees,
    mode: CheckingMode,
    selected_generic_operator_providers: &[crate::SelectedGenericOperatorProviderSpecialization],
    opaque_property_receipts: &[validation::OpaqueDataPropertyReceipt],
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
        mode.allows_pending_opaque_copy(),
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

    match mode {
        CheckingMode::Complete
        | CheckingMode::PreliminaryPackage
        | CheckingMode::SettledPackage => {
            checks::check_checked_facts_recording(&program, &mut facts)?;
        }
        #[cfg(test)]
        CheckingMode::CrashFactInspection => {
            checks::check_checked_facts_recording_without_crash_admission(&program, &mut facts)?;
        }
    }
    crate::facts::refresh_realized_contract_envelopes(&mut facts);

    // Finalize the discovered graph shapes against completed ownership facts.
    crate::flow::finalize_checked_scalar_graph_plans(
        &program,
        &facts.flow.ownership,
        &facts.values.scalar_computations,
        &mut facts.flow.terminal_scalar_graphs,
    );

    crate::flow::finalize_scalar_unit_operations(&program, &mut facts);

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
    let boundary_returns =
        crate::flow::build_checked_boundary_scalar_return_plans(&program, &facts);
    let primitive_returns =
        crate::flow::build_checked_primitive_store_scalar_return_plans(&program, &facts);
    let scalar_callees = crate::flow::ScalarCalleePlans {
        boundary_returns: &boundary_returns,
        structural_returns: &primitive_returns,
    };
    let terminal_unit_effects =
        crate::flow::build_checked_unit_effect_plans(&program, &facts, scalar_callees, &[], &[]);
    let mut cleanup_diagnostics = Vec::new();
    let structural_scalar_returns = crate::flow::build_checked_structural_scalar_return_plans(
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
    facts.flow.terminal_boundary_scalar_returns = boundary_returns;
    facts.flow.terminal_structural_scalar_returns = structural_scalar_returns;
    facts.flow.terminal_unit_effects = terminal_unit_effects;
    facts.flow.semantic_dependencies =
        crate::flow::derive_checked_semantic_dependencies(&program, &facts);

    crate::authored_selections::bind_checked_intrinsic_call_facts(&program, &mut facts)
        .map_err(|diagnostic| vec![diagnostic])?;
    if mode.allows_unresolved_toolchain_selections() {
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

/// Lower a pre-settlement package checkpoint. Unresolved selections are
/// retained only for compiler-owned toolchain source; the caller must reject
/// unresolved ordinary-package selections before granting build authority.
pub fn lower_preliminary_typed_trees(
    program: typed_trees::TypedTrees,
) -> Result<CheckedTrees, Vec<diagnostics::Diagnostic>> {
    check_program(program, CheckingMode::PreliminaryPackage, &[], &[])
}

/// One Omega-selected generic checked body that must be specialized for the
/// exact closed applications of its boundary-operator requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedGenericOperatorProviderSpecialization {
    pub requirement_operator: symbols::SymbolHandle,
    pub realization_machine: symbols::SymbolHandle,
}

/// Lower with exact selected generic operator providers supplied by the
/// orchestration owner. Psi derives applications from authored uses and uses
/// ordinary authoritative specialization; the request carries no application
/// strings, capability assertions, or provider-selection policy.
pub fn lower_typed_trees_with_selected_generic_operator_providers(
    program: typed_trees::TypedTrees,
    selected: &[SelectedGenericOperatorProviderSpecialization],
    opaque_property_receipts: &[::validation::OpaqueDataPropertyReceipt],
) -> Result<CheckedTrees, Vec<diagnostics::Diagnostic>> {
    check_program(
        program,
        CheckingMode::SettledPackage,
        selected,
        opaque_property_receipts,
    )
}

/// Final package-aware lowering keeps ordinary package selections strict while
/// permitting unresolved compiler-owned toolchain selections to remain TCB
/// input. The compiler must run its package declaration-authority gate over
/// the result before issuing package evidence.
pub fn lower_package_typed_trees_with_selected_generic_operator_providers(
    program: typed_trees::TypedTrees,
    selected: &[SelectedGenericOperatorProviderSpecialization],
    opaque_property_receipts: &[::validation::OpaqueDataPropertyReceipt],
) -> Result<CheckedTrees, Vec<diagnostics::Diagnostic>> {
    check_program(
        program,
        CheckingMode::SettledPackage,
        selected,
        opaque_property_receipts,
    )
}

/// These are distinct checking checkpoints, not freely combinable permissions.
#[derive(Clone, Copy)]
enum CheckingMode {
    /// Strictly finalized checking retained for callers that must reject
    /// toolchain late bindings; standalone and package routes currently both
    /// settle toolchain-owned selections at build-time evaluation.
    #[allow(dead_code)]
    Complete,
    PreliminaryPackage,
    SettledPackage,
    #[cfg(test)]
    CrashFactInspection,
}

impl CheckingMode {
    fn allows_pending_opaque_copy(self) -> bool {
        matches!(self, Self::PreliminaryPackage)
    }

    fn allows_unresolved_toolchain_selections(self) -> bool {
        matches!(self, Self::PreliminaryPackage | Self::SettledPackage)
    }
}

#[cfg(test)]
pub(crate) fn lower_typed_trees_for_crash_fact_inspection(
    program: typed_trees::TypedTrees,
) -> Result<CheckedTrees, Vec<diagnostics::Diagnostic>> {
    check_program(program, CheckingMode::CrashFactInspection, &[], &[])
}

#[cfg(test)]
mod tests {
    use super::CheckingMode;

    #[test]
    fn checking_modes_preserve_package_settlement_permissions() {
        for mode in [CheckingMode::Complete, CheckingMode::CrashFactInspection] {
            assert!(!mode.allows_pending_opaque_copy());
            assert!(!mode.allows_unresolved_toolchain_selections());
        }
        assert!(CheckingMode::PreliminaryPackage.allows_pending_opaque_copy());
        assert!(CheckingMode::PreliminaryPackage.allows_unresolved_toolchain_selections());
        assert!(!CheckingMode::SettledPackage.allows_pending_opaque_copy());
        assert!(CheckingMode::SettledPackage.allows_unresolved_toolchain_selections());
    }
}
