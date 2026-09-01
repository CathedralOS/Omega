//! Canonical Terminal-Psi product construction and verification.

use psi_diagnostics::Diagnostic;

/// Produce one verified retained Terminal product from the complete checked
/// frontend result.
///
/// This owner closes the checked-to-Terminal boundary, replays the canonical
/// semantic and proof sections under the request admission profile, and binds
/// the target-owned native-realization proposal. It does not assemble a
/// compiler report or enter native realization.
pub(super) fn produce_retained_terminal_artifact(
    checked: &crate::pipeline::CheckedCompilation,
    profile: &psi_proof_admission::AdmissionProfile,
) -> Result<omega_compilation_report::RetainedTerminalArtifact, Vec<Diagnostic>> {
    let callback_placements = checked.callback_placements().to_vec();
    let entry_machine = checked
        .selected_program_entry_machine()
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "terminal-artifact production requires one exact selected program entry",
            )]
        })?
        .to_owned();
    omega_selected_dispatch::validate_selected_operator_terminal_custody(
        checked,
        checked.selected_provider_plans(),
    )?;
    let produced = psi_checked_trees_to_terminal::produce_terminal_artifact_with_callback_custody(
        checked,
        &entry_machine,
        callback_placements,
    )
    .map_err(|error| {
        vec![Diagnostic::error(format!(
            "terminal-artifact production failed: {}",
            error.error(),
        ))]
    })?;
    let (
        artifact,
        checked_boundary_operator_scope,
        callback_placements,
        source_call_occurrences,
        selected_ieee_float_fma_occurrences,
    ) = produced.into_parts_with_source_calls();
    verify_terminal_artifact(&artifact, profile)?;
    let native_realization_proposal = project_terminal_native_realization_proposal(
        checked,
        &artifact,
        checked_boundary_operator_scope,
        &callback_placements,
        &source_call_occurrences,
        &selected_ieee_float_fma_occurrences,
    )?;
    omega_compilation_report::RetainedTerminalArtifact::new_with_native_realization_proposal(
        artifact,
        callback_placements,
        native_realization_proposal,
    )
    .map_err(|message| vec![Diagnostic::error(message)])
}

fn project_terminal_native_realization_proposal(
    checked: &crate::pipeline::CheckedCompilation,
    artifact: &psi_terminal_codec::CanonicalTerminalArtifact,
    checked_boundary_operator_scope: psi_checked_trees_to_terminal::CheckedBoundaryOperatorApplicationScope,
    callback_placements: &[omega_backend_plan::BoundNominalCallbackPlacement],
    source_call_occurrences: &[psi_checked_trees_to_terminal::LoweredSourceCallOccurrence],
    selected_ieee_float_fma_occurrences: &[psi_checked_trees_to_terminal::LoweredSelectedIeeeFloatFmaOccurrence],
) -> Result<omega_compilation_report::TerminalNativeRealizationProposal, Vec<Diagnostic>> {
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
        psi_terminal_codec::decode_module(artifact.semantic_bytes()).map_err(|error| {
            vec![Diagnostic::error(format!(
                "Terminal native proposal could not replay canonical semantics: {error}",
            ))]
        })?;
    let demanded_intrinsics =
        super::intrinsic_settlements::demanded_boundary_identities(&terminal_module)?;
    let builtin_proposals =
        super::intrinsic_settlements::derive_compiler_intrinsic_settlement_proposals(
            checked,
            &demanded_intrinsics,
        )?
        .into_iter()
        .map(|proposal| {
            omega_compilation_report::TerminalCompilerBuiltinProposal::new(
                proposal.requirement_identity,
                proposal.plan_index,
                proposal.execution,
            )
            .map_err(|message| vec![Diagnostic::error(message)])
        })
        .collect::<Result<Vec<_>, _>>()?;
    let callback_occurrences = callback_placements
        .iter()
        .enumerate()
        .map(|(placement_index, placement)| {
            let matching = source_call_occurrences
                .iter()
                .filter(|occurrence| {
                    occurrence.source_site == Some(placement.site)
                        && occurrence.source_target == placement.registration_operation
                })
                .collect::<Vec<_>>();
            let [occurrence] = matching.as_slice() else {
                return Err(vec![Diagnostic::error(format!(
                    "callback placement {placement_index} resolves to {} Terminal registrar occurrences; exactly one is required",
                    matching.len(),
                ))]);
            };
            let callback_thunk_identity =
                omega_backend_plan::canonical_callback_thunk_identity(placement_index, placement)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(format!(
                            "callback placement {placement_index} cannot derive one valid callback-thunk identity",
                        ))]
                    })?;
            Ok(omega_compilation_report::TerminalCallbackOccurrenceProposal::new(
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
            ))
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    let ieee_float_fma_occurrences = selected_ieee_float_fma_occurrences
        .iter()
        .map(|occurrence| {
            let matching_plan_indices = checked
                .selected_provider_plans()
                .plans()
                .iter()
                .enumerate()
                .filter(|(_, plan)| {
                    plan.report_fingerprint() == occurrence.provider_plan_report_fingerprint
                        && plan.identity_digest().as_bytes()
                            == occurrence.provider_plan_commitment.as_bytes()
                })
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            let [provider_plan_index] = matching_plan_indices.as_slice() else {
                return Err(vec![Diagnostic::error(format!(
                    "Terminal nearest-FMA operation {} rejoins {} exact selected plans; expected one",
                    occurrence.terminal_operation.get(),
                    matching_plan_indices.len(),
                ))]);
            };
            let x86_admission = if native_target.architecture
                == omega_target::Architecture::X86_64
            {
                let Some(provider) = checked.x86_scalar_fma_provider() else {
                    return Err(vec![Diagnostic::error(format!(
                        "Terminal nearest-FMA operation {} lacks an admitted x86 deployment provider",
                        occurrence.terminal_operation.get(),
                    ))]);
                };
                let matching = checked
                    .x86_scalar_fma_plan_associations()
                    .iter()
                    .filter(|association| {
                        association.matches_lowered_occurrence(
                            occurrence,
                            checked.selected_provider_plans(),
                            provider,
                        )
                    })
                    .collect::<Vec<_>>();
                let [association] = matching.as_slice() else {
                    return Err(vec![Diagnostic::error(format!(
                        "Terminal nearest-FMA operation {} rejoins {} admitted x86 plan associations; expected one",
                        occurrence.terminal_operation.get(),
                        matching.len(),
                    ))]);
                };
                Some(omega_compilation_report::TerminalX86ScalarFmaAdmission::new(
                    association.slot(),
                    association.admitted_provider(),
                ))
            } else {
                None
            };
            Ok(
                omega_compilation_report::TerminalIeeeFloatFmaOccurrenceProposal::new(
                    occurrence.terminal_operation,
                    *provider_plan_index,
                    occurrence.format,
                    x86_admission,
                ),
            )
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    let boundary_application_demands = project_terminal_boundary_application_demands(
        checked,
        artifact,
        &checked_boundary_operator_scope,
    )?;
    omega_compilation_report::TerminalNativeRealizationProposal::new(
        artifact,
        target_profile,
        native_target,
        checked.subsystem(),
        program_entry,
        checked.selected_provider_plans().clone(),
        checked.external_binding_rows().to_vec(),
        builtin_proposals,
        callback_occurrences,
        ieee_float_fma_occurrences,
        boundary_application_demands,
        checked_boundary_operator_scope,
    )
    .map_err(|message| vec![Diagnostic::error(message)])
}

fn project_terminal_boundary_application_demands(
    checked: &crate::pipeline::CheckedCompilation,
    artifact: &psi_terminal_codec::CanonicalTerminalArtifact,
    checked_scope: &psi_checked_trees_to_terminal::CheckedBoundaryOperatorApplicationScope,
) -> Result<omega_boundary_applications::TerminalBoundaryApplicationDemands, Vec<Diagnostic>> {
    let mut rows = Vec::with_capacity(checked_scope.occurrences().len());
    for occurrence in checked_scope.occurrences() {
        let application = checked_scope
            .applications()
            .get(occurrence.application_index())
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "Terminal boundary application occurrence names an absent checked demand",
                )]
            })?;
        let operator = checked
            .typed
            .operators()
            .iter()
            .find(|operator| operator.symbol == application.requirement_symbol)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "Terminal boundary application demand lost its operator declaration",
                )]
            })?;
        if !operator.is_boundary {
            return Err(vec![Diagnostic::error(
                "Terminal boundary application demand names a non-boundary operator",
            )]);
        }
        let declaration = canonical_boundary_nominal_identity(
            checked,
            application.requirement_symbol,
            "operator requirement",
        )?;
        let overload = psi_typed_trees::operator::boundary_operator_requirement_identity(
            &checked.typed,
            operator,
        );
        let requirement =
            omega_boundary_applications::BoundaryOperatorRequirement::new(declaration, overload)
                .map_err(|message| vec![Diagnostic::error(message)])?;
        let projected_application = project_boundary_application(checked, application)?;
        rows.push(
            omega_boundary_applications::TerminalBoundaryApplicationDemand::new(
                occurrence.terminal_operation(),
                requirement,
                projected_application,
            ),
        );
    }
    omega_boundary_applications::TerminalBoundaryApplicationDemands::new(
        artifact.manifest().semantic(),
        rows,
    )
    .map_err(|message| vec![Diagnostic::error(message)])
}

fn project_boundary_application(
    checked: &crate::pipeline::CheckedCompilation,
    application: &psi_checked_trees::CheckedBoundaryOperatorApplicationDemand,
) -> Result<omega_boundary_applications::BoundaryApplication, Vec<Diagnostic>> {
    if application.arguments.is_empty() {
        return Ok(omega_boundary_applications::BoundaryApplication::Empty);
    }
    let mut arguments = Vec::with_capacity(application.arguments.len());
    for (ordinal, argument) in application.arguments.iter().enumerate() {
        let expected_ordinal = u32::try_from(ordinal).map_err(|_| {
            vec![Diagnostic::error(
                "Terminal boundary application exceeds the supported ordinal range",
            )]
        })?;
        match argument {
            psi_checked_trees::CheckedBoundaryOperatorApplicationArgument::Type {
                binder_owner,
                binder_ordinal,
                type_reference,
                ..
            } if *binder_owner == application.requirement_symbol
                && *binder_ordinal == expected_ordinal =>
            {
                arguments.push(
                    omega_boundary_applications::BoundaryApplicationArgument::type_argument(
                        *binder_ordinal,
                        canonical_boundary_type_identity(checked, *type_reference)?,
                    ),
                );
            }
            psi_checked_trees::CheckedBoundaryOperatorApplicationArgument::Const {
                binder_owner,
                binder_ordinal,
                declared_carrier,
                value,
                ..
            } if *binder_owner == application.requirement_symbol
                && *binder_ordinal == expected_ordinal =>
            {
                psi_validation::validate_exact_const_value_encoding(
                    &checked.typed,
                    *declared_carrier,
                    value.encoding.as_str(),
                )
                .map_err(|reason| {
                    vec![Diagnostic::error(format!(
                        "Terminal boundary const application has invalid canonical encoding: {reason}",
                    ))]
                })?;
                arguments.push(
                    omega_boundary_applications::BoundaryApplicationArgument::const_argument(
                        *binder_ordinal,
                        canonical_boundary_type_identity(checked, *declared_carrier)?,
                        value.type_name.clone(),
                        value.encoding.clone(),
                    )
                    .map_err(|message| vec![Diagnostic::error(message)])?,
                );
            }
            _ => {
                return Err(vec![Diagnostic::error(
                    "Terminal boundary application does not rejoin its binder owner, category, and ordinal",
                )]);
            }
        }
    }
    omega_boundary_applications::BoundaryApplication::exact(arguments)
        .map_err(|message| vec![Diagnostic::error(message)])
}

fn canonical_boundary_nominal_identity(
    checked: &crate::pipeline::CheckedCompilation,
    symbol: psi_symbols::SymbolHandle,
    role: &str,
) -> Result<omega_boundary_applications::BoundaryNominalIdentity, Vec<Diagnostic>> {
    let identity = checked
        .package_qualified_nominal_identity_with_toolchain_sources(
            symbol,
            checked.exact_toolchain_sources(),
        )
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "Terminal boundary {role} has no exact package or toolchain owner",
            ))]
        })?;
    omega_boundary_applications::BoundaryNominalIdentity::new(identity.into_string())
        .map_err(|message| vec![Diagnostic::error(message)])
}

fn canonical_boundary_type_identity(
    checked: &crate::pipeline::CheckedCompilation,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
) -> Result<omega_boundary_applications::BoundaryTypeIdentity, Vec<Diagnostic>> {
    let identity = checked
        .package_qualified_type_identity_with_binders_and_toolchain_sources(
            type_reference,
            &[],
            checked.exact_toolchain_sources(),
        )
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "Terminal boundary application type has no exact package or toolchain owner",
            )]
        })?;
    omega_boundary_applications::BoundaryTypeIdentity::new(identity.into_string())
        .map_err(|message| vec![Diagnostic::error(message)])
}

fn verify_terminal_artifact(
    artifact: &psi_terminal_codec::CanonicalTerminalArtifact,
    profile: &psi_proof_admission::AdmissionProfile,
) -> Result<(), Vec<Diagnostic>> {
    let module = psi_terminal_codec::decode_module(artifact.semantic_bytes()).map_err(|error| {
        vec![Diagnostic::error(format!(
            "terminal-artifact verification could not decode canonical semantics: {error}"
        ))]
    })?;
    let proof =
        psi_terminal_codec::decode_proof_bundle(artifact.proof_bytes()).map_err(|error| {
            vec![Diagnostic::error(format!(
                "terminal-artifact verification could not decode canonical proof: {error}"
            ))]
        })?;
    psi_terminal_verifier::verify_module(&module, &proof, profile)
        .map(|_| ())
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "terminal-artifact verification failed: {error}"
            ))]
        })
}

#[cfg(test)]
mod tests {
    use super::verify_terminal_artifact;
    use psi_core::{BlockId, ContractId, EdgeId, MachineId, ObligationId, Proposition};
    use psi_terminal::{
        Block, ContractClause, MachineContract, TerminalMachine, TerminalMachineResult,
        TerminalModule, Terminator, VocabularyMarker,
    };
    use psi_terminal_verifier::ProofBundle;

    #[test]
    fn terminal_product_verification_rejects_a_canonical_unproved_contract() {
        let machine = MachineId::new(900).expect("machine");
        let block = BlockId::new(900).expect("block");
        let obligation = ObligationId::new(900).expect("obligation");
        let module = TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine,
            structural_types: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            placed_view_inputs: Vec::new(),
            reborrow_root_handoffs: Vec::new(),
            reborrow_restored_call_uses: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            proof_output_calls: Vec::new(),
            proof_recursive_components: Vec::new(),
            closed_conformance_applications: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines: vec![TerminalMachine {
                id: machine,
                attachment: None,
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters: Vec::new(),
                ranked_scc: None,
                result: TerminalMachineResult::Unit,
                structural_places: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block,
                blocks: vec![Block {
                    id: block,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: EdgeId::new(900).expect("edge"),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: MachineContract {
                    id: ContractId::new(900).expect("contract"),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: vec![ContractClause {
                        obligation,
                        proposition: Proposition::Truth,
                    }],
                    outcome_specific_ensures: Vec::new(),
                },
            }],
        };
        let artifact = psi_terminal_codec::CanonicalTerminalArtifact::from_parts(
            &module,
            &ProofBundle::default(),
            None,
        )
        .expect("canonical framing does not prove contract evidence");

        let diagnostics =
            verify_terminal_artifact(&artifact, &psi_proof_admission::AdmissionProfile::default())
                .expect_err("Terminal product verification must reconstruct proof obligations");
        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0]
                .message
                .contains("terminal-artifact verification failed: MissingEvidence"),
            "unexpected diagnostic: {}",
            diagnostics[0].message
        );
    }
}
