//! Root-policy accepted terminal-authority permissions for realization.

use super::AcceptedOrdinaryClosureEvidence;
use diagnostics::Diagnostic;
use native_realization::{
    TerminalAuthorityPermissionPolicy, TerminalAuthorityPermissionPolicyBuildError,
    TerminalAuthorityPermissionPolicyRow, TerminalAuthorityPolicy,
    terminal_authority_permission_policy_with_rows,
};

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

/// Project only exact permission obligations that survived fresh project-policy
/// comparison into the package's canonical accepted-permission set.
///
/// Semantic-binding candidates, package names, broad risk summaries, and raw
/// root decisions are deliberately unavailable to this projection. The
/// accepted evidence gate has already proved that every blocking permission
/// row is bijective with fresh obligations from the same compiler reviews whose
/// complete policy was compared with the project's recorded acceptance.
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

/// The retained boundary available for an accepted package's native production.
///
/// A Terminal report already fixes its exact post-Terminal selection. A reviewed
/// candidate instead retains the authored build selections against which a
/// subtractive release rollback must be settled before consuming checked custody.
pub enum AcceptedNativeInput<'input> {
    Terminal {
        report: compiler::CompileReport,
        optimization_selections: &'input optimization_core::PostTerminalOptimizationSelections,
    },
    Reviewed {
        // Keep the large reviewed owner from inflating every Terminal request's
        // stack footprint while preserving its consumed custody boundary.
        candidate: Box<crate::review::ReviewedPackageProductionCandidate>,
        optimization_rollback: &'input compiler::OptimizationRollback,
    },
}

/// Explicit admission evidence and receiving inputs for accepted native production.
/// Freely constructed receiving policies never substitute for package acceptance.
pub struct AcceptedNativeRealizationRequest<'request> {
    pub evidence: &'request AcceptedOrdinaryClosureEvidence,
    pub profile: &'request proof_admission::AdmissionProfile,
    pub terminal_authority_policy: TerminalAuthorityPolicy,
    pub receiving_terminal_authority_permission_policy: TerminalAuthorityPermissionPolicy,
    pub imports: &'request [compiler::SourceEvaluatedImportSettlement<'request>],
}

/// Consume accepted package custody into one unpublished retained-native report.
///
/// Both input forms join the complete Terminal production subject to fresh
/// accepted evidence before extracting its artifact or projecting permissions.
/// Reviewed input consumes the exact checked root without rerunning its build or
/// source discovery. The resulting native manifest retains the same production
/// subject and applicable rollback receipt. Terminal input also retains its
/// existing trust settlement; reviewed input leaves that outcome to its caller.
/// Artifact-only consumers explicitly extract their custody from this report.
pub fn realize_accepted_native_report(
    input: AcceptedNativeInput<'_>,
    request: AcceptedNativeRealizationRequest<'_>,
) -> Result<compiler::CompileReport, Vec<Diagnostic>> {
    let AcceptedNativeRealizationRequest {
        evidence,
        profile,
        terminal_authority_policy,
        receiving_terminal_authority_permission_policy,
        imports,
    } = request;
    // Keep the reviewed input's source/review custody alive through realization.
    let (report, optimization_selections, rollback_receipt, trust_settlement, _reviews) =
        match input {
            AcceptedNativeInput::Terminal {
                report,
                optimization_selections,
            } => {
                let rollback_receipt = report.optimization_rollback_receipt().cloned();
                let trust_settlement = report.trust_admission_settlement().clone();
                (
                    report,
                    optimization_selections.clone(),
                    rollback_receipt,
                    trust_settlement,
                    None,
                )
            }
            AcceptedNativeInput::Reviewed {
                candidate,
                optimization_rollback,
            } => {
                let build_selected = candidate.checked_root().optimization_selections().clone();
                let rollback_receipt = optimization_rollback.reconcile(&build_selected);
                let effective_optimizations = rollback_receipt
                    .as_ref()
                    .map_or(build_selected, |receipt| receipt.effective().clone());
                let post_terminal_optimizations = effective_optimizations.project_post_terminal();
                let (reviews, root_path, checked_root) = candidate.into_production_parts();
                let report = compiler::retained_terminal_report_from_checked_package(
                    root_path,
                    checked_root,
                    profile.clone(),
                )?;
                (
                    report,
                    post_terminal_optimizations.selections().clone(),
                    rollback_receipt,
                    compiler::TrustAdmissionSettlement::default(),
                    Some(reviews),
                )
            }
        };
    validate_accepted_terminal_production_subject(&report, evidence)?;
    let root_path = report.root_path().to_path_buf();
    let source_file_count = report.source_file_count;
    let production_subject = report
        .production_manifest()
        .map(|manifest| manifest.subject().clone());
    let accepted_permission_policy = accepted_terminal_authority_permission_policy(evidence)
        .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
    let retained = report.into_retained_terminal_artifact().ok_or_else(|| {
        diagnostics("accepted Terminal realization requires one retained Terminal artifact")
    })?;
    let subsystem = retained
        .native_realization_proposal()
        .ok_or_else(|| {
            diagnostics("retained Terminal product: source-evaluated import realization requires one native proposal")
        })?
        .subsystem();
    let artifact = compiler::realize_retained_native_artifact(
        retained,
        compiler::RetainedNativeRealizationRequest {
            profile,
            optimization_selections: &optimization_selections,
            terminal_authority_policy,
            accepted_package_terminal_authority_permission_policy: accepted_permission_policy,
            terminal_authority_permission_policy: receiving_terminal_authority_permission_policy,
            image_request: native_realization::ExecutableImageEmissionRequest::direct(subsystem),
            imports,
        },
    )
    .map_err(|(_, diagnostics)| diagnostics)?;
    let native_realization::RequestedNativeArtifact::Direct(artifact) = artifact else {
        return Err(diagnostics(
            "accepted Terminal realization requires direct image custody",
        ));
    };
    compiler::CompileReport::from_retained_native_artifact(
        root_path,
        source_file_count,
        artifact,
        rollback_receipt,
        production_subject,
    )
    .map(|report| report.with_trust_admission_settlement(trust_settlement))
    .map_err(|message| vec![Diagnostic::error(message)])
}

fn validate_accepted_terminal_production_subject(
    report: &compiler::CompileReport,
    evidence: &AcceptedOrdinaryClosureEvidence,
) -> Result<(), Vec<Diagnostic>> {
    if report.output_kind() != compiler::CompileOutputKind::TerminalArtifact {
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
        .map(build_evaluation::BuildObservationSummary::identity)
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
