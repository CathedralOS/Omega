use super::{
    CompilerReviewTriage, PackageTriageDecision, PackageTriageDisposition, PackageTriageReason,
};
use crate::declarations::PackageKey;
use omega_package_evidence::record::PackageReviewDangerousAuthorityClass;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TriageRenderError {
    maximum_bytes: usize,
    required_bytes: usize,
}

impl TriageRenderError {
    pub const fn maximum_bytes(self) -> usize {
        self.maximum_bytes
    }

    pub const fn required_bytes(self) -> usize {
        self.required_bytes
    }
}

impl fmt::Display for TriageRenderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "package source triage requires {} bytes, exceeding the {}-byte review-input ceiling",
            self.required_bytes, self.maximum_bytes
        )
    }
}

impl std::error::Error for TriageRenderError {}

pub(super) fn render_bounded(
    triage: &CompilerReviewTriage,
    maximum_bytes: usize,
) -> Result<String, TriageRenderError> {
    let required_bytes = required_render_bytes(triage.decisions());
    if required_bytes > maximum_bytes {
        return Err(TriageRenderError {
            maximum_bytes,
            required_bytes,
        });
    }
    let mut rendered = String::with_capacity(required_bytes);
    rendered.push_str("OMEGA_PACKAGE_SOURCE_TRIAGE_V2\n");
    for decision in triage.decisions() {
        rendered.push_str("package ");
        rendered.push_str(decision.package_name());
        rendered.push('\n');
        render_key(&mut rendered, "baseline_key", decision.baseline_key());
        render_key(&mut rendered, "candidate_key", decision.candidate_key());
        rendered.push_str("disposition ");
        rendered.push_str(disposition_token(decision.disposition()));
        rendered.push('\n');
        for reason in decision.reasons() {
            rendered.push_str("reason ");
            rendered.push_str(reason_token(*reason));
            rendered.push('\n');
        }
        rendered.push_str("end_package\n");
    }
    debug_assert_eq!(rendered.len(), required_bytes);
    Ok(rendered)
}

fn render_key(output: &mut String, label: &str, key: Option<&PackageKey>) {
    output.push_str(label);
    output.push(' ');
    match key {
        Some(key) => output.push_str(&encode_hex(&key.identity().digest())),
        None => output.push_str("none"),
    }
    output.push('\n');
}

fn required_render_bytes(decisions: &[PackageTriageDecision]) -> usize {
    let mut required = "OMEGA_PACKAGE_SOURCE_TRIAGE_V2\n".len();
    for decision in decisions {
        required = saturating_add(required, "package \n".len());
        required = saturating_add(required, decision.package_name().len());
        required = saturating_add(required, "baseline_key \n".len());
        required = saturating_add(required, key_token_length(decision.baseline_key()));
        required = saturating_add(required, "candidate_key \n".len());
        required = saturating_add(required, key_token_length(decision.candidate_key()));
        required = saturating_add(required, "disposition \n".len());
        required = saturating_add(required, disposition_token(decision.disposition()).len());
        for reason in decision.reasons() {
            required = saturating_add(required, "reason \n".len());
            required = saturating_add(required, reason_token(*reason).len());
        }
        required = saturating_add(required, "end_package\n".len());
    }
    required
}

const fn key_token_length(key: Option<&PackageKey>) -> usize {
    if key.is_some() { 64 } else { "none".len() }
}

const fn saturating_add(left: usize, right: usize) -> usize {
    match left.checked_add(right) {
        Some(sum) => sum,
        None => usize::MAX,
    }
}

const fn disposition_token(disposition: PackageTriageDisposition) -> &'static str {
    match disposition {
        PackageTriageDisposition::Admitted => "admitted",
        PackageTriageDisposition::AdmittedWithAuditRecommended => "admitted_with_audit_recommended",
        PackageTriageDisposition::BlockedMissingAdmissionBaseline => {
            "blocked_missing_admission_baseline"
        }
        PackageTriageDisposition::BlockedCapabilityChange => "blocked_capability_change",
        PackageTriageDisposition::BlockedProvenanceChange => "blocked_provenance_change",
    }
}

pub(super) const fn reason_token(reason: PackageTriageReason) -> &'static str {
    match reason {
        PackageTriageReason::InitialAdmission => "initial_admission",
        PackageTriageReason::NewTransitivePackage => "new_transitive_package",
        PackageTriageReason::RemovedPackage => "removed_package",
        PackageTriageReason::SourceChanged => "source_changed",
        PackageTriageReason::BaselineSourceUnavailable => "baseline_source_unavailable",
        PackageTriageReason::MissingAdmissionBaseline => "missing_admission_baseline",
        PackageTriageReason::CapabilityOrApiChanged => "capability_or_api_changed",
        PackageTriageReason::SourceLineageChanged => "source_lineage_changed",
        PackageTriageReason::BuildObservationChanged => "build_observation_changed",
        PackageTriageReason::RootLostDependencyCompatibility => {
            "root_lost_dependency_compatibility"
        }
        PackageTriageReason::RootLostApplicationActivation => "root_lost_application_activation",
        PackageTriageReason::RepresentationTcbIntroducedOrChanged => {
            "representation_tcb_introduced_or_changed"
        }
        PackageTriageReason::AcceptedClaimRequiresResolution => {
            "accepted_claim_requires_resolution"
        }
        PackageTriageReason::RetainedDangerousAuthority(
            PackageReviewDangerousAuthorityClass::Filesystem,
        ) => "retained_dangerous_authority_filesystem",
        PackageTriageReason::RetainedDangerousAuthority(
            PackageReviewDangerousAuthorityClass::MachineControl,
        ) => "retained_dangerous_authority_machine_control",
        PackageTriageReason::RetainedDangerousAuthority(
            PackageReviewDangerousAuthorityClass::PortIo,
        ) => "retained_dangerous_authority_port_io",
        PackageTriageReason::RetainedDangerousAuthority(
            PackageReviewDangerousAuthorityClass::InterruptControl,
        ) => "retained_dangerous_authority_interrupt_control",
        PackageTriageReason::RetainedDangerousAuthority(
            PackageReviewDangerousAuthorityClass::InterruptEntry,
        ) => "retained_dangerous_authority_interrupt_entry",
        PackageTriageReason::RetainedDangerousAuthority(
            PackageReviewDangerousAuthorityClass::RootMemory,
        ) => "retained_dangerous_authority_root_memory",
        PackageTriageReason::RetainedDangerousAuthority(
            PackageReviewDangerousAuthorityClass::Process,
        ) => "retained_dangerous_authority_process",
        PackageTriageReason::DangerousAuthoritySlack(
            PackageReviewDangerousAuthorityClass::Filesystem,
        ) => "dangerous_authority_slack_filesystem",
        PackageTriageReason::DangerousAuthoritySlack(
            PackageReviewDangerousAuthorityClass::MachineControl,
        ) => "dangerous_authority_slack_machine_control",
        PackageTriageReason::DangerousAuthoritySlack(
            PackageReviewDangerousAuthorityClass::PortIo,
        ) => "dangerous_authority_slack_port_io",
        PackageTriageReason::DangerousAuthoritySlack(
            PackageReviewDangerousAuthorityClass::InterruptControl,
        ) => "dangerous_authority_slack_interrupt_control",
        PackageTriageReason::DangerousAuthoritySlack(
            PackageReviewDangerousAuthorityClass::InterruptEntry,
        ) => "dangerous_authority_slack_interrupt_entry",
        PackageTriageReason::DangerousAuthoritySlack(
            PackageReviewDangerousAuthorityClass::RootMemory,
        ) => "dangerous_authority_slack_root_memory",
        PackageTriageReason::DangerousAuthoritySlack(
            PackageReviewDangerousAuthorityClass::Process,
        ) => "dangerous_authority_slack_process",
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(DIGITS[usize::from(byte >> 4)]));
        encoded.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    encoded
}
