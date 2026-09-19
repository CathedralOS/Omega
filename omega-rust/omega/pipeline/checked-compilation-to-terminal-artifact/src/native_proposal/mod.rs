//! Project what a later native realization may consume from the retained
//! Terminal artifact: exact provider-plan joins, callback occurrences and
//! thunks, IEEE float custody, and the closed external binding rows.

use crate::application_coverage::project_terminal_boundary_application_coverage;
use crate::float_comparisons;
use crate::terminal_artifact::verification::verify_terminal_artifact;
use assembled_syntax_to_checked_compilation::CheckedCompilation;
use diagnostics::Diagnostic;

mod callback_registrars;

pub(crate) fn project_terminal_native_realization_proposal(
    checked: &CheckedCompilation,
    profile: &proof_admission::AdmissionProfile,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    checked_program_entry: terminal_psi::CheckedProgramEntryTerminalReceipt,
    checked_boundary_operator_scope: lowered_psi_to_terminal_psi::CheckedBoundaryOperatorApplicationScope,
    callback_placements: &[backend_plan::BoundNominalCallbackPlacement],
    source_call_occurrences: &[lowered_psi::LoweredSourceCallOccurrence],
    selected_ieee_float_fma_occurrences: &[lowered_psi::LoweredSelectedIeeeFloatFmaOccurrence],
    selected_ieee_float_comparison_occurrences: &[lowered_psi::LoweredSelectedIeeeFloatComparisonOccurrence],
    selections: &optimization_core::OptimizationSelections,
) -> Result<compilation_report::TerminalNativeRealizationProposal, Vec<Diagnostic>> {
    let target_profile = checked.selected_target_profile().ok_or_else(|| {
        vec![Diagnostic::error(
            "Terminal native proposal requires one selected target profile",
        )]
    })?;
    let native_target = checked.selected_native_target().ok_or_else(|| {
        vec![Diagnostic::error(
            "Terminal native proposal requires one selected native target",
        )]
    })?;
    let program_entry = checked.selected_program_entry().cloned().ok_or_else(|| {
        vec![Diagnostic::error(
            "Terminal native proposal requires one exact selected ProgramEntry",
        )]
    })?;
    let terminal_module =
        terminal_codec::decode_module(artifact.semantic_bytes()).map_err(|error| {
            vec![Diagnostic::error(format!(
                "Terminal native proposal could not replay canonical semantics: {error}",
            ))]
        })?;
    let demanded_intrinsics =
        provider_planning::compiler_intrinsics::demanded_boundary_identities(&terminal_module)?;
    let builtin_proposals =
        provider_planning::compiler_intrinsics::derive_selected_intrinsic_settlement_proposals(
            checked.selected_provider_plans().plans(),
            checked.selected_provider_provenance(),
            &demanded_intrinsics,
        )?
        .into_iter()
        .map(|proposal| {
            compilation_report::TerminalCompilerBuiltinProposal::new(
                proposal.requirement_identity,
                proposal.plan_index,
                proposal.execution,
            )
            .map_err(|message| vec![Diagnostic::error(message)])
        })
        .collect::<Result<Vec<_>, _>>()?;
    let callback_source_calls = callback_registrars::CallbackSourceCalls::new(
        callback_placements
            .iter()
            .map(|placement| (placement.site, placement.registration_operation)),
        source_call_occurrences,
    );
    let callback_occurrences = callback_placements
        .iter()
        .enumerate()
        .map(|(placement_index, placement)| {
            let occurrence = callback_source_calls
                .find(placement.site, placement.registration_operation)
                .map_err(|count| vec![Diagnostic::error(format!(
                    "callback placement {placement_index} resolves to {count} Terminal registrar occurrences; exactly one is required",
                ))])?;
            let callback_thunk_identity =
                backend_plan::canonical_callback_thunk_identity(placement_index, placement)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(format!(
                            "callback placement {placement_index} cannot derive one valid callback-thunk identity",
                        ))]
                    })?;
            let callback_thunk_artifact =
                produce_callback_thunk_artifact(checked, profile, placement)?;
            Ok(compilation_report::TerminalCallbackOccurrenceProposal::new(
                placement_index,
                occurrence.terminal_operation,
                placement
                    .private_materialization
                    .as_ref()
                    .and_then(|materialization| {
                        materialization
                            .direct_registrar_parameter_application
                            .clone()
                    }),
                callback_thunk_identity,
                callback_thunk_artifact,
            ))
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    let ieee_float_fma_occurrences =
        crate::float_fma::associate(checked, native_target, selected_ieee_float_fma_occurrences)?;
    let ieee_float_comparison_occurrences = float_comparisons::associate(
        checked,
        &terminal_module,
        checked.selected_provider_plans(),
        checked.selected_provider_provenance(),
        selected_ieee_float_comparison_occurrences,
    )?;
    let boundary_application_coverage = project_terminal_boundary_application_coverage(
        checked,
        artifact,
        &checked_boundary_operator_scope,
    )?;
    let (boundary_application_demands, boundary_application_realizations) =
        boundary_application_coverage.into_parts();
    let external_binding_rows = callback_closed_external_binding_rows(
        checked,
        &terminal_module,
        callback_placements,
        &callback_occurrences,
    )?;
    let package_terminal_authority_permissions = checked
        .resolved_semantic_bindings()
        .flat_map(|binding| binding.terminal_authority_permissions())
        .cloned()
        .collect();
    compilation_report::TerminalNativeRealizationProposal::new(
        artifact,
        compilation_report::TerminalNativeRealizationInputs {
            target_profile,
            native_target,
            subsystem: checked.subsystem(),
            application_intent: checked.application_intent(),
            application_identifier: checked.application_identifier().cloned(),
            application_name: checked
                .application_name()
                .map(|name| name.as_str().to_owned()),
            post_terminal_optimizations: selections.project_post_terminal(),
            program_entry,
            checked_program_entry,
            selected_provider_plans: checked.selected_provider_plans().clone(),
            external_binding_rows,
            package_terminal_authority_permissions,
            compiler_builtins: builtin_proposals,
            callback_occurrences,
            ieee_float_fma_occurrences,
            ieee_float_comparison_occurrences,
            boundary_application_demands,
            boundary_application_realizations,
            checked_boundary_operator_scope,
            behavior_exclusions: build_evaluation::authored_behavior_exclusion_set_in(
                checked.behavior_exclusions(),
                &terminal_module,
                &crate::terminal_artifact::behavior_exclusions::boundary_trait_identity(checked),
            ),
        },
    )
    .map_err(|message| vec![Diagnostic::error(message)])
}

/// Rejoin target-closed callback registrar plans to their selected import
/// rows while the checked calling-policy realization and canonical Terminal
/// operation still coexist. The locator remains the target package's only
/// contribution; the registrar requirement owns its complete physical plan.
fn callback_closed_external_binding_rows(
    checked: &CheckedCompilation,
    terminal_module: &terminal_psi::TerminalModule,
    callback_placements: &[backend_plan::BoundNominalCallbackPlacement],
    callback_occurrences: &[compilation_report::TerminalCallbackOccurrenceProposal],
) -> Result<Vec<calling_conventions::ExternalBindingRow>, Vec<Diagnostic>> {
    let mut rows = checked.external_binding_rows().to_vec();
    if callback_occurrences.is_empty() {
        return Ok(rows);
    }
    let operations = callback_registrars::CallbackTerminalOperations::new(
        callback_occurrences
            .iter()
            .map(|occurrence| occurrence.terminal_operation()),
        terminal_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations),
    );
    let declarations = terminal_module
        .boundary_machines
        .iter()
        .map(|declaration| (declaration.id, declaration.identity.as_str()))
        .collect::<std::collections::BTreeMap<_, _>>();

    for occurrence in callback_occurrences {
        let placement = callback_placements
            .get(occurrence.placement_index())
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "callback registrar import closure names an absent retained placement",
                )]
            })?;
        let materialization = placement.private_materialization.as_ref().ok_or_else(|| {
            vec![Diagnostic::error(
                "callback registrar import closure requires one private materialization",
            )]
        })?;
        let operation = operations.find(occurrence.terminal_operation()).map_err(|count| {
            vec![Diagnostic::error(format!(
                "callback registrar import closure resolves Terminal operation {} to {count} operations",
                occurrence.terminal_operation().get(),
            ))]
        })?;
        let terminal_psi::OperationKind::BoundaryCall { boundary, .. } = &operation.kind else {
            return Err(vec![Diagnostic::error(
                "callback registrar import closure names a non-boundary Terminal operation",
            )]);
        };
        let requirement = declarations.get(boundary).copied().ok_or_else(|| {
            vec![Diagnostic::error(
                "callback registrar import closure names an absent Terminal boundary",
            )]
        })?;

        let matching_realizations = checked
            .boundary_calling_plan_realizations()
            .iter()
            .filter(|realization| {
                realization
                    .materialized_signature()
                    .owner_requirement_identity()
                    == requirement
                    && realization.callback_context_closed
                    && realization.exact_boundary_entry_plan()
                        == &materialization.registrar_boundary_entry_plan
            })
            .collect::<Vec<_>>();
        let [realization] = matching_realizations.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "callback registrar `{requirement}` rejoins {} exact target-closed calling-plan realizations",
                matching_realizations.len(),
            ))]);
        };
        let validated = realization.replayed_validated_plan().map_err(|error| {
            vec![Diagnostic::error(format!(
                "callback registrar `{requirement}` target-closed plan failed replay: {error}",
            ))]
        })?;
        if validated.plan() != &materialization.registrar_boundary_entry_plan {
            return Err(vec![Diagnostic::error(format!(
                "callback registrar `{requirement}` target-closed plan changed during replay",
            ))]);
        }

        let matching_rows = rows
            .iter_mut()
            .filter(|row| {
                row.requirement_identity == requirement
                    && matches!(
                        row.binding,
                        calling_conventions::ExternalBindingKind::Import { .. }
                    )
            })
            .collect::<Vec<_>>();
        if matching_rows.is_empty() {
            // Check-only Terminal custody does not require a target package to
            // supply a normalized import. Native source-import re-entry will
            // independently require and settle the exact row.
            continue;
        }
        if matching_rows.len() != 1 {
            return Err(vec![Diagnostic::error(format!(
                "callback registrar `{requirement}` rejoins {} retained external import rows",
                matching_rows.len(),
            ))]);
        }
        let row = matching_rows
            .into_iter()
            .next()
            .expect("one exact callback external import row");
        row.boundary_entry_plan = Some(materialization.registrar_boundary_entry_plan.clone());
    }

    Ok(rows)
}

fn produce_callback_thunk_artifact(
    checked: &CheckedCompilation,
    profile: &proof_admission::AdmissionProfile,
    placement: &backend_plan::BoundNominalCallbackPlacement,
) -> Result<compilation_report::TerminalCallbackThunkArtifact, Vec<Diagnostic>> {
    let matching = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .filter(|selection| selection.machine == placement.selected_machine)
        .collect::<Vec<_>>();
    let [_selection] = matching.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "callback selection resolves to {} Terminal-lowerable machines; exactly one is required",
            matching.len(),
        ))]);
    };
    let lowered = checked_trees_to_lowered_psi::lower_bounded_callback_identity_machine(
        checked,
        placement.selected_machine,
        placement.selected_entry,
    )
    .map_err(|error| {
        vec![Diagnostic::error(format!(
            "callback thunk Terminal lowering failed: {error}",
        ))]
    })?;
    // The callback body is part of the admitted composition; the authored
    // exclusions apply to its unoptimized module exactly as to the program
    // entry's.
    let provenance =
        crate::terminal_artifact::behavior_exclusions::MachineProvenance::from_lowering(
            checked,
            &lowered.terminal,
            placement.selected_machine,
        );
    crate::terminal_artifact::behavior_exclusions::verify_module_behavior_exclusions(
        checked,
        &lowered.terminal.semantic_module,
        &provenance,
    )?;
    let psi_optimizations = checked.optimization_selections().project_psi();
    let optimized = lowered_psi_to_lowered_psi::run_psi_optimization(
        lowered.terminal,
        psi_optimizations.selections().clone(),
    );
    let optimized = optimized.map_err(|error| {
        vec![Diagnostic::error(format!(
            "callback thunk Psi optimization failed: {error}",
        ))]
    })?;
    let artifact =
        lowered_psi_to_terminal_psi::finalize_terminal_artifact(&optimized).map_err(|error| {
            vec![Diagnostic::error(format!(
                "callback thunk canonicalization failed: {error}",
            ))]
        })?;
    verify_terminal_artifact(&artifact, profile)?;
    validate_direct_callback_thunk_shape(&artifact, placement)?;
    compilation_report::TerminalCallbackThunkArtifact::new(
        backend_plan::canonical_callback_private_symbol(placement),
        artifact,
        lowered.receipt,
    )
    .map_err(|message| vec![Diagnostic::error(message)])
}

fn validate_direct_callback_thunk_shape(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    placement: &backend_plan::BoundNominalCallbackPlacement,
) -> Result<(), Vec<Diagnostic>> {
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).map_err(|error| {
        vec![Diagnostic::error(format!(
            "callback thunk shape replay could not decode canonical semantics: {error}",
        ))]
    })?;
    let [machine] = module.machines.as_slice() else {
        return Err(vec![Diagnostic::error(
            "direct callback thunk currently requires exactly one Terminal machine",
        )]);
    };
    let ([parameter], [block]) = (machine.parameters.as_slice(), machine.blocks.as_slice()) else {
        return Err(vec![Diagnostic::error(
            "direct callback thunk currently requires one scalar parameter and one block",
        )]);
    };
    let expected_type =
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
            .expect("u64 is a valid fixed integer type");
    // The two admitted leaf cohorts mirror the two checked callback bodies the
    // bounded lowering accepts: a `u64 -> u64` identity return, and a
    // `u64 -> Unit` body that completes without producing a value. The
    // evaluated boundary entry plan retains which of them the requirement
    // signature declared, so the Terminal shape and the replayed signature
    // must agree exactly.
    let (exact_body, signature_result) = match (&machine.result, &block.terminator) {
        (
            terminal_psi::TerminalMachineResult::Scalar(result),
            terminal_psi::Terminator::Return {
                value,
                cleanup_actions,
                ..
            },
        ) => (
            result.scalar_type == parameter.scalar_type
                && *value == parameter.id
                && cleanup_actions.is_empty(),
            Some(calling_conventions::ValueShape::integer(8, 8)),
        ),
        (
            terminal_psi::TerminalMachineResult::Unit,
            terminal_psi::Terminator::ReturnUnit {
                trivial_affine_discards,
                ..
            },
        ) => (trivial_affine_discards.is_empty(), None),
        _ => (false, None),
    };
    let is_exact_leaf = module.entry == machine.id
        && machine.entry == block.id
        && machine.structural_parameters.is_empty()
        && machine.structural_places.is_empty()
        && machine.ranked_scc.is_none()
        && block.parameters.is_empty()
        && block.operations.is_empty()
        && parameter.scalar_type == semantic_vocabulary::ScalarType::Integer(expected_type)
        && exact_body;
    if !is_exact_leaf {
        return Err(vec![Diagnostic::error(
            "direct callback thunk currently admits only the exact u64 identity or u64-to-Unit leaf",
        )]);
    }
    let signature = calling_conventions::CallSignature {
        parameters: vec![calling_conventions::ValueShape::integer(8, 8)],
        result: signature_result,
    };
    let validated = calling_conventions::validate_boundary_entry_plan(
        placement.boundary_entry_plan.clone(),
        &signature,
    )
    .map_err(|error| {
        vec![Diagnostic::error(format!(
            "callback thunk boundary entry plan does not match its Terminal body: {error}",
        ))]
    })?;
    if validated.plan() != &placement.boundary_entry_plan {
        return Err(vec![Diagnostic::error(
            "callback thunk boundary entry plan changed during canonical validation",
        )]);
    }
    Ok(())
}
