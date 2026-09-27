//! Project what a later native realization may consume from the retained
//! Terminal artifact: exact provider-plan joins, callback occurrences and
//! thunks, IEEE float custody, and the closed external binding rows.

use crate::checked::CheckedCompilation;
use crate::terminal::application_coverage::project_terminal_boundary_application_coverage;
use crate::terminal::terminal_artifact::verification::verify_terminal_artifact;
use crate::terminal::{float_comparisons, integer_comparisons};
use diagnostics::Diagnostic;
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};

mod callback_registrars;

pub(crate) fn project_terminal_native_realization_proposal(
    checked: &CheckedCompilation,
    profile: &proof_admission::AdmissionProfile,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    checked_program_entry: terminal_psi::CheckedProgramEntryTerminalReceipt,
    checked_boundary_operator_scope: lowered_psi_to_terminal_psi::CheckedBoundaryOperatorApplicationScope,
    callback_placements: &[backend_plan::BoundNominalCallbackPlacement],
    source_call_occurrences: &[lowered_psi::LoweredSourceCallOccurrence],
    selected_ieee_float_comparison_occurrences: &[lowered_psi::LoweredSelectedIeeeFloatComparisonOccurrence],
    selected_integer_comparison_occurrences: &[lowered_psi::LoweredSelectedIntegerComparisonOccurrence],
    selections: &optimization_core::OptimizationSelections,
) -> Result<crate::report::TerminalNativeRealizationProposal, Vec<Diagnostic>> {
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
            crate::report::TerminalCompilerBuiltinProposal::new(
                proposal.requirement_identity,
                proposal.plan_index,
                proposal.execution,
            )
            .map_err(|message| vec![Diagnostic::error(message)])
        })
        .collect::<Result<Vec<_>, _>>()?;
    let callback_occurrences = project_callback_occurrences(
        checked,
        profile,
        callback_placements,
        source_call_occurrences,
    )?;
    let ieee_float_comparison_occurrences = float_comparisons::associate(
        checked,
        &terminal_module,
        checked.selected_provider_plans(),
        checked.selected_provider_provenance(),
        selected_ieee_float_comparison_occurrences,
    )?;
    let integer_comparison_occurrences = integer_comparisons::associate(
        checked,
        &terminal_module,
        checked.selected_provider_plans(),
        checked.selected_provider_provenance(),
        selected_integer_comparison_occurrences,
    )?;
    let boundary_application_coverage = project_terminal_boundary_application_coverage(
        checked,
        artifact,
        &checked_boundary_operator_scope,
    )?;
    let (
        boundary_application_demands,
        boundary_application_realizations,
        boundary_opaque_applications,
    ) = boundary_application_coverage.into_parts();
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
    crate::report::TerminalNativeRealizationProposal::new(
        artifact,
        crate::report::TerminalNativeRealizationInputs {
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
            // The proposal retains the exact selected-application commitment
            // at each by-value opaque edge while the checked signature custody
            // still exists; artifacts compared across an exchange must carry
            // the same set.
            boundary_opaque_applications,
            package_terminal_authority_permissions,
            compiler_builtins: builtin_proposals,
            callback_occurrences,
            ieee_float_comparison_occurrences,
            integer_comparison_occurrences,
            boundary_application_demands,
            boundary_application_realizations,
            checked_boundary_operator_scope,
            behavior_exclusions: build_evaluation::authored_behavior_exclusion_set_in(
                checked.behavior_exclusions(),
                &terminal_module,
                &crate::terminal::terminal_artifact::behavior_exclusions::boundary_trait_identity(
                    checked,
                ),
            ),
        },
    )
    .map_err(|message| vec![Diagnostic::error(message)])
}

/// Rejoin retained callback placements to their exact Terminal registrar
/// occurrences and produce each occurrence's thunk artifact. Both native
/// routes — retained product and direct program-entry — admit the same rows.
pub(crate) fn project_callback_occurrences(
    checked: &CheckedCompilation,
    profile: &proof_admission::AdmissionProfile,
    callback_placements: &[backend_plan::BoundNominalCallbackPlacement],
    source_call_occurrences: &[lowered_psi::LoweredSourceCallOccurrence],
) -> Result<Vec<crate::report::TerminalCallbackOccurrenceProposal>, Vec<Diagnostic>> {
    let callback_source_calls = callback_registrars::CallbackSourceCalls::new(
        callback_placements
            .iter()
            .map(|placement| (placement.site, placement.registration_operation)),
        source_call_occurrences,
    );
    callback_placements
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
            Ok(crate::report::TerminalCallbackOccurrenceProposal::new(
                placement_index,
                occurrence.terminal_operation,
                placement
                    .private_materialization
                    .as_ref()
                    .and_then(|materialization| {
                        materialization
                            .direct_registrar_parameter_application
                            .clone()
                            .or_else(|| materialization.root_parameter_application.clone())
                    }),
                callback_thunk_identity,
                callback_thunk_artifact,
            ))
        })
        .collect()
}

/// Rejoin target-closed callback registrar plans to their selected import
/// rows while the checked calling-policy realization and canonical Terminal
/// operation still coexist. The locator remains the target package's only
/// contribution; the registrar requirement owns its complete physical plan.
pub(crate) fn callback_closed_external_binding_rows(
    checked: &CheckedCompilation,
    terminal_module: &terminal_psi::TerminalModule,
    callback_placements: &[backend_plan::BoundNominalCallbackPlacement],
    callback_occurrences: &[crate::report::TerminalCallbackOccurrenceProposal],
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
) -> Result<crate::report::TerminalCallbackThunkArtifact, Vec<Diagnostic>> {
    // The thunk body leaves through the same production body as the program
    // entry: one lowering, the selected Psi optimization, canonical
    // publication. It carries no entry identity, so no `ProgramEntry`
    // receipt is minted; the callback placement binds its own evidence to
    // the lowering receipt instead.
    let psi_optimizations = checked.optimization_selections().project_psi();
    let mut production_timings = TerminalProductionTimings::default();
    let produced = terminal_production::TerminalProductionRequest {
        checked: checked.terminal_production_trees(),
        machine: terminal_production::TerminalMachineSelection::Symbol(placement.selected_machine),
        optimization_selections: psi_optimizations.selections().clone(),
    }
    .produce(TerminalProductionCustody {
        entry_identity: None,
        callback_custody: (),
        // The authored exclusions and the lowering receipt are judged on the
        // unoptimized composition the thunk was published from.
        retain_unoptimized: true,
        timings: &mut production_timings,
    })
    .map_err(|error| {
        vec![Diagnostic::error(format!(
            "callback thunk Terminal production failed: {error}",
        ))]
    })?;
    // The callback body is part of the admitted composition; the authored
    // exclusions apply to its unoptimized module exactly as to the program
    // entry's.
    crate::terminal::terminal_artifact::behavior_exclusions::verify_entry_behavior_exclusions(
        checked,
        produced.unoptimized(),
        placement.selected_machine,
    )?;
    let unoptimized = produced.unoptimized().ok_or_else(|| {
        vec![Diagnostic::error(
            "callback thunk production retained no unoptimized Terminal lowering to join the checked coordinate onto",
        )]
    })?;
    let receipt = checked_trees_to_lowered_psi::callback_lowering_receipt(
        checked,
        unoptimized,
        placement.selected_machine,
        placement.selected_entry,
    )
    .map_err(|error| {
        vec![Diagnostic::error(format!(
            "callback thunk Terminal lowering failed: {error}",
        ))]
    })?;
    let artifact = produced.into_artifact();
    verify_terminal_artifact(&artifact, profile)?;
    validate_callback_thunk_inbound_custody(checked, &artifact, placement)?;
    crate::report::TerminalCallbackThunkArtifact::new(
        backend_plan::canonical_callback_private_symbol(placement),
        artifact,
        receipt,
    )
    .map_err(|message| vec![Diagnostic::error(message)])
}

/// The ABI shape one scalar type occupies — fixed integers, Booleans, and
/// IEEE floats only; anything wider has no scalar-ABI custody.
fn scalar_value_shape(
    scalar_type: semantic_vocabulary::ScalarType,
) -> Option<calling_conventions::ValueShape> {
    use calling_conventions::ValueShape;
    match scalar_type {
        semantic_vocabulary::ScalarType::Boolean => Some(ValueShape::integer(1, 1)),
        semantic_vocabulary::ScalarType::IeeeFloat(
            semantic_vocabulary::IeeeFloatFormat::Binary32,
        ) => Some(ValueShape::float(4)),
        semantic_vocabulary::ScalarType::IeeeFloat(
            semantic_vocabulary::IeeeFloatFormat::Binary64,
        ) => Some(ValueShape::float(8)),
        semantic_vocabulary::ScalarType::Integer(integer) => {
            let bits = integer.bits();
            if integer.carrier() != semantic_vocabulary::IntegerCarrier::Fixed
                || !matches!(bits, 8 | 16 | 32 | 64)
            {
                return None;
            }
            let bytes = bits / 8;
            Some(ValueShape::integer(bytes, bytes))
        }
    }
}

/// Requirement, ABI, and call-custody validation for one produced callback
/// thunk artifact. This is deliberately not a body recognizer: any checked
/// callback body of any inbound signature is admissible while this custody
/// holds.
///
/// - Requirement: the placement rejoins exactly one target calling-plan
///   realization for its satisfaction row, whose owner is the placement's
///   canonical requirement overload.
/// - ABI: the realization replays to a validated entry plan that must equal
///   the retained `placement.boundary_entry_plan` with unaltered application
///   identity, and the produced entry machine's declared interface must
///   correspond to the requirement's materialized parameter/result roster —
///   scalar slots by exact ABI shape, reference and aggregate slots by
///   structural parameter presence (their physical shape is the plan's own
///   target-selected evidence).
/// - Call custody: the compiler-private channel carries no import custody, so
///   the thunk module may declare no boundary machines.
fn validate_callback_thunk_inbound_custody(
    checked: &CheckedCompilation,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    placement: &backend_plan::BoundNominalCallbackPlacement,
) -> Result<(), Vec<Diagnostic>> {
    backend_plan::validate_bound_nominal_callback_placement(placement).map_err(|error| {
        vec![Diagnostic::error(format!(
            "callback thunk placement failed boundary entry plan replay: {error}",
        ))]
    })?;
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).map_err(|error| {
        vec![Diagnostic::error(format!(
            "callback thunk custody replay could not decode canonical semantics: {error}",
        ))]
    })?;
    let Some(machine) = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
    else {
        return Err(vec![Diagnostic::error(
            "callback thunk module does not contain its entry machine",
        )]);
    };

    // Requirement join: exactly one target calling-plan realization owns this
    // satisfaction row, and the retained plan must be its exact evaluated
    // inbound plan — replayed, not fingerprint-trusted.
    let matching = checked
        .boundary_calling_plan_realizations()
        .iter()
        .filter(|realization| {
            realization.boundary_trait == placement.satisfaction_trait
                && realization.requirement_machine == placement.satisfaction_requirement
                && realization.boundary_arguments.is_empty()
        })
        .collect::<Vec<_>>();
    let [realization] = matching.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "callback thunk requirement `{}` resolves to {} calling-plan realizations; exactly one is required",
            placement.canonical_requirement_overload,
            matching.len(),
        ))]);
    };
    let materialized = realization.materialized_signature();
    if materialized.owner_requirement_identity() != placement.canonical_requirement_overload {
        return Err(vec![Diagnostic::error(format!(
            "callback thunk requirement `{}` rejoins a realization owned by `{}`",
            placement.canonical_requirement_overload,
            materialized.owner_requirement_identity(),
        ))]);
    }
    let (validated, report_fingerprint, commitment) = realization
        .replayed_validated_application()
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "callback thunk inbound entry plan does not replay against its requirement: {error}",
            ))]
        })?;
    if validated.plan() != &placement.boundary_entry_plan
        || realization.exact_boundary_entry_plan() != &placement.boundary_entry_plan
        || realization.report_fingerprint != report_fingerprint
        || realization.commitment != commitment
        || placement.boundary_calling_plan_report_fingerprint
            != validated.contract_report_fingerprint()
    {
        return Err(vec![Diagnostic::error(
            "callback thunk inbound entry plan drifted from its requirement's evaluated application",
        )]);
    }

    bind_thunk_machine_interface(machine, materialized)?;

    // Call custody: the compiler-private channel carries no import custody.
    // A thunk whose closure calls a boundary machine can never settle those
    // imports, so the module may declare none.
    if !module.boundary_machines.is_empty() {
        return Err(vec![Diagnostic::error(
            "callback thunk module carries boundary import custody the private channel cannot settle",
        )]);
    }
    Ok(())
}

/// Bind one produced thunk entry machine's declared interface to its
/// requirement's materialized signature. The authored parameter roots order
/// the walk: a scalar-classed root consumes one scalar machine parameter whose
/// ABI shape must re-derive exactly; a reference or aggregate root consumes
/// one structural machine parameter — its physical shape is the plan's own
/// target-selected evidence, already replayed above. Either roster left over
/// means the produced machine drifted from its requirement signature.
fn bind_thunk_machine_interface(
    machine: &terminal_psi::TerminalMachine,
    materialized: &provider_planning::calling_policy_plans::MaterializedBoundarySignature,
) -> Result<(), Vec<Diagnostic>> {
    use provider_planning::calling_policy_plans::BoundaryValueClass as Class;
    let mut scalar_parameters = machine.parameters.iter();
    let mut structural_parameters = machine.structural_parameters.iter();
    for (ordinal, root) in materialized.parameters().iter().copied().enumerate() {
        let Some(shape) = materialized.shapes().get(usize::from(root)) else {
            return Err(vec![Diagnostic::error(format!(
                "callback thunk requirement parameter {ordinal} names an absent shape root",
            ))]);
        };
        match shape.class() {
            Class::Integer | Class::Float => {
                let Some(parameter) = scalar_parameters.next() else {
                    return Err(vec![Diagnostic::error(format!(
                        "callback thunk machine declares too few scalar parameters for requirement parameter {ordinal}",
                    ))]);
                };
                let expected = calling_conventions::ValueShape {
                    class: match shape.class() {
                        Class::Float => calling_conventions::ValueClass::Float,
                        _ => calling_conventions::ValueClass::Integer,
                    },
                    byte_size: shape.byte_size(),
                    alignment: shape.alignment(),
                };
                if scalar_value_shape(parameter.scalar_type) != Some(expected) {
                    return Err(vec![Diagnostic::error(format!(
                        "callback thunk scalar parameter {ordinal} does not carry its requirement's ABI shape",
                    ))]);
                }
            }
            Class::Reference | Class::FixedArray { .. } | Class::Record { .. } => {
                if structural_parameters.next().is_none() {
                    return Err(vec![Diagnostic::error(format!(
                        "callback thunk machine declares too few structural parameters for requirement parameter {ordinal}",
                    ))]);
                }
            }
        }
    }
    if scalar_parameters.next().is_some() || structural_parameters.next().is_some() {
        return Err(vec![Diagnostic::error(
            "callback thunk machine declares parameters beyond its requirement signature",
        )]);
    }
    match materialized.result() {
        Some(root) => {
            let Some(shape) = materialized.shapes().get(usize::from(root)) else {
                return Err(vec![Diagnostic::error(
                    "callback thunk requirement result names an absent shape root",
                )]);
            };
            match shape.class() {
                Class::Integer | Class::Float => {
                    let terminal_psi::TerminalMachineResult::Scalar(result) = &machine.result
                    else {
                        return Err(vec![Diagnostic::error(
                            "callback thunk requirement result is scalar but the machine declares none",
                        )]);
                    };
                    let expected = calling_conventions::ValueShape {
                        class: match shape.class() {
                            Class::Float => calling_conventions::ValueClass::Float,
                            _ => calling_conventions::ValueClass::Integer,
                        },
                        byte_size: shape.byte_size(),
                        alignment: shape.alignment(),
                    };
                    if scalar_value_shape(result.scalar_type) != Some(expected) {
                        return Err(vec![Diagnostic::error(
                            "callback thunk scalar result does not carry its requirement's ABI shape",
                        )]);
                    }
                }
                Class::Reference | Class::FixedArray { .. } | Class::Record { .. } => {
                    if !matches!(
                        machine.result,
                        terminal_psi::TerminalMachineResult::Structural(_)
                    ) {
                        return Err(vec![Diagnostic::error(
                            "callback thunk requirement result is structural but the machine declares none",
                        )]);
                    }
                }
            }
        }
        None => {
            if !matches!(machine.result, terminal_psi::TerminalMachineResult::Unit) {
                return Err(vec![Diagnostic::error(
                    "callback thunk requirement has no result but the machine declares one",
                )]);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scalar_type(bits: u16) -> semantic_vocabulary::ScalarType {
        semantic_vocabulary::ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, bits)
                .expect("fixed-width integer type"),
        )
    }

    fn scalar_parameter(id: u64, bits: u16) -> terminal_psi::ValueDeclaration {
        terminal_psi::ValueDeclaration {
            id: semantic_vocabulary::ValueId::new(id).expect("nonzero value id"),
            scalar_type: scalar_type(bits),
            qualifications: semantic_vocabulary::ScalarQualificationSetId::ZERO,
        }
    }

    fn machine(
        parameters: Vec<terminal_psi::ValueDeclaration>,
        structural_parameters: Vec<terminal_psi::StructuralParameterDeclaration>,
        result: terminal_psi::TerminalMachineResult,
    ) -> terminal_psi::TerminalMachine {
        terminal_psi::TerminalMachine {
            id: semantic_vocabulary::MachineId::new(1).expect("nonzero machine id"),
            attachment: None,
            parameters,
            structural_parameters,
            ranked_scc: None,
            result,
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            declared_service_reach: Vec::new(),
            closed_reach_application: None,
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: semantic_vocabulary::BlockId::new(1).expect("nonzero block id"),
            blocks: Vec::new(),
            contract: terminal_psi::MachineContract {
                id: semantic_vocabulary::ContractId::new(1).expect("nonzero contract id"),
                crash_routes: Vec::new(),
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }
    }

    fn materialized(
        signature: calling_conventions::CallSignature,
    ) -> provider_planning::calling_policy_plans::MaterializedBoundarySignature {
        provider_planning::calling_policy_plans::materialized_boundary_signature_from_abi(
            &signature,
        )
        .expect("materialized signature")
    }

    #[test]
    fn thunk_interface_binds_a_multi_parameter_scalar_signature() {
        let signature = calling_conventions::CallSignature {
            parameters: vec![
                calling_conventions::ValueShape::integer(8, 8),
                calling_conventions::ValueShape::integer(1, 1),
            ],
            result: Some(calling_conventions::ValueShape::integer(2, 2)),
        };
        let machine = machine(
            vec![scalar_parameter(1, 64), scalar_parameter(2, 8)],
            Vec::new(),
            terminal_psi::TerminalMachineResult::Scalar(scalar_parameter(3, 16)),
        );
        bind_thunk_machine_interface(&machine, &materialized(signature))
            .expect("multi-parameter scalar signature binds");
    }

    #[test]
    fn thunk_interface_binds_a_parameterless_unit_signature() {
        let signature = calling_conventions::CallSignature {
            parameters: Vec::new(),
            result: None,
        };
        let machine = machine(
            Vec::new(),
            Vec::new(),
            terminal_psi::TerminalMachineResult::Unit,
        );
        bind_thunk_machine_interface(&machine, &materialized(signature))
            .expect("parameterless Unit signature binds");
    }

    #[test]
    fn thunk_interface_rejects_a_drifted_scalar_shape() {
        let signature = calling_conventions::CallSignature {
            parameters: vec![calling_conventions::ValueShape::integer(8, 8)],
            result: None,
        };
        let machine = machine(
            vec![scalar_parameter(1, 32)],
            Vec::new(),
            terminal_psi::TerminalMachineResult::Unit,
        );
        assert!(bind_thunk_machine_interface(&machine, &materialized(signature)).is_err());
    }

    #[test]
    fn thunk_interface_rejects_extra_and_missing_machine_parameters() {
        let signature = || {
            materialized(calling_conventions::CallSignature {
                parameters: vec![calling_conventions::ValueShape::integer(8, 8)],
                result: None,
            })
        };
        let extra = machine(
            vec![scalar_parameter(1, 64), scalar_parameter(2, 64)],
            Vec::new(),
            terminal_psi::TerminalMachineResult::Unit,
        );
        assert!(bind_thunk_machine_interface(&extra, &signature()).is_err());
        let missing = machine(
            Vec::new(),
            Vec::new(),
            terminal_psi::TerminalMachineResult::Unit,
        );
        assert!(bind_thunk_machine_interface(&missing, &signature()).is_err());
    }

    #[test]
    fn thunk_interface_binds_float_and_boolean_shapes() {
        let signature = calling_conventions::CallSignature {
            parameters: vec![
                calling_conventions::ValueShape::float(8),
                calling_conventions::ValueShape::integer(1, 1),
            ],
            result: Some(calling_conventions::ValueShape::float(4)),
        };
        let machine = machine(
            vec![
                terminal_psi::ValueDeclaration {
                    id: semantic_vocabulary::ValueId::new(1).expect("nonzero value id"),
                    scalar_type: semantic_vocabulary::ScalarType::IeeeFloat(
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ),
                    qualifications: semantic_vocabulary::ScalarQualificationSetId::ZERO,
                },
                terminal_psi::ValueDeclaration {
                    id: semantic_vocabulary::ValueId::new(2).expect("nonzero value id"),
                    scalar_type: semantic_vocabulary::ScalarType::Boolean,
                    qualifications: semantic_vocabulary::ScalarQualificationSetId::ZERO,
                },
            ],
            Vec::new(),
            terminal_psi::TerminalMachineResult::Scalar(terminal_psi::ValueDeclaration {
                id: semantic_vocabulary::ValueId::new(3).expect("nonzero value id"),
                scalar_type: semantic_vocabulary::ScalarType::IeeeFloat(
                    semantic_vocabulary::IeeeFloatFormat::Binary32,
                ),
                qualifications: semantic_vocabulary::ScalarQualificationSetId::ZERO,
            }),
        );
        bind_thunk_machine_interface(&machine, &materialized(signature))
            .expect("float and boolean shapes bind");
    }

    #[test]
    fn thunk_interface_rejects_result_presence_drift() {
        let unit_requirement = materialized(calling_conventions::CallSignature {
            parameters: Vec::new(),
            result: None,
        });
        let scalar_machine = machine(
            Vec::new(),
            Vec::new(),
            terminal_psi::TerminalMachineResult::Scalar(scalar_parameter(1, 64)),
        );
        assert!(bind_thunk_machine_interface(&scalar_machine, &unit_requirement).is_err());

        let scalar_requirement = materialized(calling_conventions::CallSignature {
            parameters: Vec::new(),
            result: Some(calling_conventions::ValueShape::integer(8, 8)),
        });
        let unit_machine = machine(
            Vec::new(),
            Vec::new(),
            terminal_psi::TerminalMachineResult::Unit,
        );
        assert!(bind_thunk_machine_interface(&unit_machine, &scalar_requirement).is_err());
    }
}
