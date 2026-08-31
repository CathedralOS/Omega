use super::request::ValidatedCompileRequest;
use super::{
    CompileReport, CompileRequest, NativeCompilationWithCheckedReceipt, RequestedCompileProduct,
    TrustAdmissionSettlement,
};
use psi_diagnostics::Diagnostic;

/// Drive the one production route and stop at the requested product.
///
/// Check, Terminal Psi, and retained native artifacts share one checked-Psi
/// frontend and differ only in how far the result proceeds.
pub(super) fn compile(request: CompileRequest) -> Result<CompileReport, Vec<Diagnostic>> {
    if request.requested_product() == RequestedCompileProduct::NativeArtifact {
        return compile_native_with_checked_receipt(request)
            .map(NativeCompilationWithCheckedReceipt::into_report);
    }
    let request = request.validate_for_execution()?;
    let (checked, trust_settlement) = compile_checked_with_observations(&request)?;
    let report = match request.requested_product() {
        RequestedCompileProduct::Check => checked_report(request, &checked),
        RequestedCompileProduct::TerminalArtifact => terminal_report(request, checked),
        RequestedCompileProduct::NativeArtifact => unreachable!(
            "native requests enter the checked-receipt route before frontend compilation"
        ),
    }?;
    Ok(report.with_trust_admission_settlement(trust_settlement))
}

pub(super) fn compile_native_with_checked_receipt(
    request: CompileRequest,
) -> Result<NativeCompilationWithCheckedReceipt, Vec<Diagnostic>> {
    let request = request.validate_for_native_execution()?;
    let (checked, trust_settlement) = compile_checked_with_observations(&request)?;
    let report = super::optimization::native_report(request, &checked)?
        .with_trust_admission_settlement(trust_settlement);
    NativeCompilationWithCheckedReceipt::new(checked, report)
        .map_err(|message| vec![Diagnostic::error(message)])
}

fn compile_checked_with_observations(
    request: &ValidatedCompileRequest,
) -> Result<
    (
        crate::pipeline::CheckedCompilation,
        TrustAdmissionSettlement,
    ),
    Vec<Diagnostic>,
> {
    let checked = crate::pipeline::checked_entry::compile_to_checked_for_terminal(
        request.options(),
        request.package_inputs(),
    )?;
    let trust_settlement = crate::pipeline::reporting::report_checked_observations(
        crate::pipeline::reporting::CheckedObservationInput {
            options: request.options(),
            artifact_policy: request.artifact_policy(),
            accepted_trust_admissions: request.accepted_trust_admissions(),
            checked: &checked,
        },
    )?;
    Ok((checked, trust_settlement))
}

fn checked_report(
    request: ValidatedCompileRequest,
    checked: &crate::pipeline::CheckedCompilation,
) -> Result<CompileReport, Vec<Diagnostic>> {
    let request = request.into_inner();
    CompileReport::checked(
        request.options.root_path,
        checked.source_file_count(),
        false,
        super::CompileOutputKind::CheckOnly,
        None,
        None,
    )
    .map_err(|message| vec![Diagnostic::error(message)])
}

fn terminal_report(
    request: ValidatedCompileRequest,
    checked: crate::pipeline::CheckedCompilation,
) -> Result<CompileReport, Vec<Diagnostic>> {
    let request = request.into_inner();
    let production_subject = crate::pipeline::reporting::project_production_subject(&checked)?;
    let source_file_count = checked.source_file_count();
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
        &checked,
        checked.selected_provider_plans(),
    )?;
    let produced = psi_checked_trees_to_terminal::produce_terminal_artifact_with_callback_custody(
        &checked,
        &entry_machine,
        callback_placements,
    )
    .map_err(|error| {
        vec![Diagnostic::error(format!(
            "terminal-artifact production failed: {}",
            error.error(),
        ))]
    })?;
    let (artifact, callback_placements, source_call_occurrences) =
        produced.into_parts_with_source_calls();
    verify_terminal_artifact(&artifact, &request.terminal_admission_profile)?;
    let native_realization_proposal = project_terminal_native_realization_proposal(
        &checked,
        &artifact,
        &callback_placements,
        &source_call_occurrences,
    )?;
    let artifact =
        omega_compilation_report::RetainedTerminalArtifact::new_with_native_realization_proposal(
            artifact,
            callback_placements,
            native_realization_proposal,
        )
        .map_err(|message| vec![Diagnostic::error(message)])?;
    CompileReport::from_retained_terminal_artifact(
        request.options.root_path,
        source_file_count,
        artifact,
        production_subject,
    )
    .map_err(|message| vec![Diagnostic::error(message)])
}

fn project_terminal_native_realization_proposal(
    checked: &crate::pipeline::CheckedCompilation,
    artifact: &psi_terminal_codec::CanonicalTerminalArtifact,
    callback_placements: &[omega_backend_plan::BoundNominalCallbackPlacement],
    source_call_occurrences: &[psi_checked_trees_to_terminal::LoweredSourceCallOccurrence],
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
            Ok(omega_compilation_report::TerminalCallbackOccurrenceProposal::new(
                placement_index,
                occurrence.terminal_operation,
            ))
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
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
    )
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
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            proof_output_calls: Vec::new(),
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
