//! Root-policy accepted terminal-authority permissions for realization.

use super::AcceptedOrdinaryClosureEvidence;
use omega_terminal_psi_to_native_artifact::{
    TerminalAuthorityPermissionPolicy, TerminalAuthorityPermissionPolicyBuildError,
    TerminalAuthorityPermissionPolicyRow, TerminalAuthorityPolicy,
    terminal_authority_permission_policy_with_rows,
};
use psi_diagnostics::Diagnostic;

/// Failure to project one accepted package closure into its exact accepted
/// permission set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptedTerminalAuthorityPermissionPolicyError {
    AllocationFailed,
    InvalidPolicy(TerminalAuthorityPermissionPolicyBuildError),
}

impl std::fmt::Display for AcceptedTerminalAuthorityPermissionPolicyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AllocationFailed => formatter
                .write_str("accepted terminal-authority permission policy allocation failed"),
            Self::InvalidPolicy(error) => write!(
                formatter,
                "accepted terminal-authority permissions do not form one exact receiving policy: {error:?}",
            ),
        }
    }
}

impl std::error::Error for AcceptedTerminalAuthorityPermissionPolicyError {}

/// Project only exact permission obligations that survived fresh root-policy
/// replay into the package's canonical accepted-permission set.
///
/// Semantic-binding candidates, package names, broad risk summaries, and raw
/// root decisions are deliberately unavailable to this projection. The
/// accepted evidence gate has already proved that every blocking permission
/// row is bijective with and accepted by the replayed root policy.
pub fn accepted_terminal_authority_permission_policy(
    evidence: &AcceptedOrdinaryClosureEvidence,
) -> Result<TerminalAuthorityPermissionPolicy, AcceptedTerminalAuthorityPermissionPolicyError> {
    let permissions = evidence
        .acceptance()
        .obligations()
        .root_open_terminal_authority_permissions();
    let mut rows = Vec::new();
    rows.try_reserve_exact(permissions.len())
        .map_err(|_| AcceptedTerminalAuthorityPermissionPolicyError::AllocationFailed)?;
    for (_, obligation) in permissions {
        let permission = obligation.permission();
        rows.push(TerminalAuthorityPermissionPolicyRow::new(
            permission.service_schema(),
            permission.requirement_identity(),
            permission.permitted().clone(),
        ));
    }
    terminal_authority_permission_policy_with_rows(rows)
        .map_err(AcceptedTerminalAuthorityPermissionPolicyError::InvalidPolicy)
}

/// Consume one package-aware retained Terminal report only after joining it to
/// the exact package/source subject that produced fresh accepted evidence.
///
/// The accepted permission set is projected here, where callers cannot
/// substitute a freely constructed policy for package admission. The
/// receiving policy remains a separate deployment input and may contain
/// unrelated rows. This entrypoint deliberately requires the complete
/// compiler report: extracting the retained artifact first would discard the
/// production manifest that binds it to the accepted source closure.
pub fn realize_accepted_terminal_artifact_with_source_evaluated_imports_and_policy(
    report: omega_compiler::CompileReport,
    evidence: &AcceptedOrdinaryClosureEvidence,
    profile: &psi_proof_admission::AdmissionProfile,
    optimization_selections: &omega_optimization_core::OptimizationSelections,
    terminal_authority_policy: TerminalAuthorityPolicy,
    receiving_terminal_authority_permission_policy: TerminalAuthorityPermissionPolicy,
    imports: &[omega_compiler::SourceEvaluatedImportSettlement<'_>],
) -> Result<omega_compiler::RetainedNativeArtifact, Vec<Diagnostic>> {
    validate_accepted_terminal_production_subject(&report, evidence)?;
    let accepted_permission_policy = accepted_terminal_authority_permission_policy(evidence)
        .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
    let retained = report.into_retained_terminal_artifact().ok_or_else(|| {
        diagnostics("accepted Terminal realization requires one retained Terminal artifact")
    })?;
    omega_compiler::realize_retained_terminal_artifact_with_source_evaluated_imports_and_policy(
        retained,
        profile,
        optimization_selections,
        terminal_authority_policy,
        accepted_permission_policy,
        receiving_terminal_authority_permission_policy,
        imports,
    )
}

/// Consume the exact checked root retained by final package review and realize
/// it only after joining its Terminal production subject to accepted evidence.
///
/// This route never reloads package source, reruns `build.omg`, or reopens
/// dependency discovery. The review set stays owned for the duration of the
/// handoff and the resulting native artifact remains unpublished.
pub fn realize_accepted_reviewed_package_candidate_with_source_evaluated_imports_and_policy(
    candidate: crate::review::ReviewedPackageProductionCandidate,
    evidence: &AcceptedOrdinaryClosureEvidence,
    profile: &psi_proof_admission::AdmissionProfile,
    optimization_selections: &omega_optimization_core::OptimizationSelections,
    terminal_authority_policy: TerminalAuthorityPolicy,
    receiving_terminal_authority_permission_policy: TerminalAuthorityPermissionPolicy,
    imports: &[omega_compiler::SourceEvaluatedImportSettlement<'_>],
) -> Result<omega_compiler::RetainedNativeArtifact, Vec<Diagnostic>> {
    let (_reviews, root_path, checked_root) = candidate.into_production_parts();
    let report = omega_compiler::retained_terminal_report_from_checked_package(
        root_path,
        checked_root,
        profile.clone(),
    )?;
    realize_accepted_terminal_artifact_with_source_evaluated_imports_and_policy(
        report,
        evidence,
        profile,
        optimization_selections,
        terminal_authority_policy,
        receiving_terminal_authority_permission_policy,
        imports,
    )
}

fn validate_accepted_terminal_production_subject(
    report: &omega_compiler::CompileReport,
    evidence: &AcceptedOrdinaryClosureEvidence,
) -> Result<(), Vec<Diagnostic>> {
    if report.output_kind() != omega_compiler::CompileOutputKind::TerminalArtifact {
        return Err(diagnostics(
            "accepted Terminal realization requires a retained Terminal report",
        ));
    }
    let artifact = report.artifact().ok_or_else(|| {
        diagnostics("accepted Terminal realization report has no Terminal artifact")
    })?;
    let manifest = report.production_manifest().ok_or_else(|| {
        diagnostics("accepted Terminal realization requires package production custody")
    })?;
    if !manifest.validate() || !manifest.matches_terminal_artifact(artifact) {
        return Err(diagnostics(
            "accepted Terminal realization report has inconsistent production custody",
        ));
    }

    let subject = manifest.subject();
    let package_subject = subject.package();
    let retained_proposal = report
        .terminal_native_realization_proposal()
        .ok_or_else(|| {
            diagnostics("accepted Terminal realization report has no native proposal")
        })?;
    if retained_proposal.target_profile() != subject.target_profile()
        || retained_proposal.native_target() != subject.native_target()
    {
        return Err(diagnostics(
            "retained Terminal proposal target differs from package production custody",
        ));
    }
    let accepted_root = evidence
        .packages()
        .iter()
        .find(|package| package.package().identity() == package_subject.root())
        .ok_or_else(|| {
            diagnostics(
                "accepted Terminal realization package root is absent from accepted evidence",
            )
        })?;
    if evidence
        .acceptance()
        .obligations()
        .question()
        .source_closure()
        .root()
        .selected()
        .key()
        .identity()
        != package_subject.root()
    {
        return Err(diagnostics(
            "retained Terminal production root differs from accepted package evidence",
        ));
    }
    if accepted_root.generated_sources().target() != subject.target_profile() {
        return Err(diagnostics(
            "retained Terminal production target differs from accepted package evidence",
        ));
    }
    if accepted_root.generated_sources().dependency_closure()
        != package_subject.dependency_closure()
    {
        return Err(diagnostics(
            "retained Terminal production dependency closure differs from accepted package evidence",
        ));
    }
    if accepted_root.source_consumption() != package_subject.source_consumption_commitment() {
        return Err(diagnostics(
            "retained Terminal production source consumption differs from accepted package evidence",
        ));
    }
    if accepted_root.selected_build_machine_identity() != subject.selected_build_machine_identity()
    {
        return Err(diagnostics(
            "retained Terminal production build machine differs from accepted package evidence",
        ));
    }
    if !accepted_root
        .build_evaluation_usage()
        .is_some_and(|accepted| {
            accepted.has_same_invocation_usage(subject.build_evaluation_usage())
        })
    {
        return Err(diagnostics(
            "retained Terminal production invocation usage differs from accepted package evidence",
        ));
    }
    if accepted_root
        .build_observation()
        .map(omega_build_evaluation::BuildObservationSummary::identity)
        != Some(subject.build_observation_identity())
    {
        return Err(diagnostics(
            "retained Terminal production build observation differs from accepted package evidence",
        ));
    }
    Ok(())
}

fn diagnostics(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::error(message)]
}
