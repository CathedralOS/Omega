//! Deterministic source and provenance triage dispositions.

use crate::review::comparison::changed_review_risk;
use crate::review::records::PackageReviewEvidence;
use crate::review::{CompilerIssuedPackageReview, CompilerIssuedPackageReviewSet};
use omega_package_review::{
    PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk,
    PackageReviewDangerousAuthorityClass,
};
use omega_package_source::PackageKey;
use std::collections::{BTreeMap, BTreeSet};

mod render;

pub use render::TriageRenderError;

/// Deterministic package-manager disposition for source review.
///
/// This is review-only orchestration state. It is neither accepted lock
/// evidence nor proof that a human or model audited source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageTriageDisposition {
    Admitted,
    AdmittedWithAuditRecommended,
    BlockedMissingAdmissionBaseline,
    BlockedCapabilityChange,
    BlockedProvenanceChange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageTriageReason {
    InitialAdmission,
    NewTransitivePackage,
    RemovedPackage,
    SourceChanged,
    BaselineSourceUnavailable,
    MissingAdmissionBaseline,
    CapabilityOrApiChanged,
    SourceLineageChanged,
    BuildObservationChanged,
    RepresentationTcbIntroducedOrChanged,
    AcceptedClaimRequiresResolution,
    RetainedDangerousAuthority(PackageReviewDangerousAuthorityClass),
    DangerousAuthoritySlack(PackageReviewDangerousAuthorityClass),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageTriageDecision {
    package_name: String,
    baseline_key: Option<PackageKey>,
    candidate_key: Option<PackageKey>,
    disposition: PackageTriageDisposition,
    reasons: Vec<PackageTriageReason>,
}

impl PackageTriageDecision {
    pub fn package_name(&self) -> &str {
        &self.package_name
    }

    pub fn baseline_key(&self) -> Option<&PackageKey> {
        self.baseline_key.as_ref()
    }

    pub fn candidate_key(&self) -> Option<&PackageKey> {
        self.candidate_key.as_ref()
    }

    pub const fn disposition(&self) -> PackageTriageDisposition {
        self.disposition
    }

    pub fn reasons(&self) -> &[PackageTriageReason] {
        &self.reasons
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerReviewTriage {
    decisions: Vec<PackageTriageDecision>,
}

impl CompilerReviewTriage {
    pub fn decisions(&self) -> &[PackageTriageDecision] {
        &self.decisions
    }

    pub fn disposition(&self) -> PackageTriageDisposition {
        self.decisions
            .iter()
            .map(PackageTriageDecision::disposition)
            .max()
            .unwrap_or(PackageTriageDisposition::Admitted)
    }

    /// Render bounded, fixed-vocabulary evidence suitable for an advisory
    /// reviewer prompt. No package README, commit message, source comment, or
    /// reviewer-authored string enters this representation.
    pub fn render_bounded(&self, maximum_bytes: usize) -> Result<String, TriageRenderError> {
        render::render_bounded(self, maximum_bytes)
    }
}

/// Initial installation is comparison against an empty baseline. Dangerous
/// authority and opaque representation-TCB rows recommend audit immediately;
/// the absence of an old package is not itself a conflict.
pub fn triage_initial_install(candidate: &CompilerIssuedPackageReviewSet) -> CompilerReviewTriage {
    CompilerReviewTriage {
        decisions: candidate
            .reviews()
            .iter()
            .map(|review| {
                let mut reasons = vec![PackageTriageReason::InitialAdmission];
                let blocking = append_candidate_blocking_reasons(review, &mut reasons);
                let recommendation = append_candidate_audit_reasons(review, true, &mut reasons);
                PackageTriageDecision {
                    package_name: review.key().name().as_str().to_owned(),
                    baseline_key: None,
                    candidate_key: Some(review.key().clone()),
                    disposition: if blocking {
                        PackageTriageDisposition::BlockedCapabilityChange
                    } else if recommendation {
                        PackageTriageDisposition::AdmittedWithAuditRecommended
                    } else {
                        PackageTriageDisposition::Admitted
                    },
                    reasons,
                }
            })
            .collect(),
    }
}

/// Fail closed when an update has no normalized accepted admission baseline.
///
/// This is distinct from initial installation, which deliberately admits a
/// complete graph against an empty baseline, and from unavailable old source,
/// where retained accepted rows still govern capability comparison. A caller
/// may leave this state only by starting the explicit full-graph fresh-
/// admission flow; it may not reinterpret the update as unchanged.
pub fn triage_update_without_admission_baseline(
    candidate: &CompilerIssuedPackageReviewSet,
) -> CompilerReviewTriage {
    CompilerReviewTriage {
        decisions: candidate
            .reviews()
            .iter()
            .map(|review| {
                let mut reasons = vec![PackageTriageReason::MissingAdmissionBaseline];
                append_candidate_blocking_reasons(review, &mut reasons);
                append_candidate_audit_reasons(review, true, &mut reasons);
                PackageTriageDecision {
                    package_name: review.key().name().as_str().to_owned(),
                    baseline_key: None,
                    candidate_key: Some(review.key().clone()),
                    disposition: PackageTriageDisposition::BlockedMissingAdmissionBaseline,
                    reasons,
                }
            })
            .collect(),
    }
}

/// Compare compiler-issued candidate evidence with a valid accepted baseline.
///
/// `unavailable_baseline_sources` affects source-audit guidance only: the
/// canonical baseline rows still govern capability comparison. A package-key
/// change is a replacement/provenance conflict; exact-key source revisions are
/// ordinary updates. Advisory model output is deliberately not accepted here.
pub fn triage_review_update(
    baseline: &CompilerIssuedPackageReviewSet,
    candidate: &CompilerIssuedPackageReviewSet,
    unavailable_baseline_sources: &BTreeSet<PackageKey>,
) -> CompilerReviewTriage {
    triage_review_update_records(baseline.reviews(), candidate, unavailable_baseline_sources)
}

pub(crate) fn triage_review_update_records<B: PackageReviewEvidence>(
    baseline: &[B],
    candidate: &CompilerIssuedPackageReviewSet,
    unavailable_baseline_sources: &BTreeSet<PackageKey>,
) -> CompilerReviewTriage {
    let baseline_by_name = reviews_by_name(baseline);
    let candidate_by_name = reviews_by_name(candidate.reviews());
    let names = baseline_by_name
        .keys()
        .chain(candidate_by_name.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    let mut decisions = Vec::new();
    for name in names {
        append_name_decisions(
            name,
            baseline_by_name.get(name).map(Vec::as_slice).unwrap_or(&[]),
            candidate_by_name
                .get(name)
                .map(Vec::as_slice)
                .unwrap_or(&[]),
            unavailable_baseline_sources,
            &mut decisions,
        );
    }
    CompilerReviewTriage { decisions }
}

fn reviews_by_name<R: PackageReviewEvidence>(reviews: &[R]) -> BTreeMap<&str, Vec<&R>> {
    let mut by_name = BTreeMap::<_, Vec<_>>::new();
    for review in reviews {
        by_name
            .entry(review.key().name().as_str())
            .or_default()
            .push(review);
    }
    for group in by_name.values_mut() {
        group.sort_by_key(|review| review.key());
    }
    by_name
}

fn append_name_decisions<B: PackageReviewEvidence>(
    package_name: &str,
    baseline: &[&B],
    candidate: &[&CompilerIssuedPackageReview],
    unavailable_baseline_sources: &BTreeSet<PackageKey>,
    decisions: &mut Vec<PackageTriageDecision>,
) {
    let mut unmatched_baseline = Vec::new();
    let mut unmatched_candidate = candidate.to_vec();
    for baseline_review in baseline {
        if let Some(index) = unmatched_candidate
            .iter()
            .position(|candidate_review| candidate_review.key() == baseline_review.key())
        {
            let candidate_review = unmatched_candidate.remove(index);
            decisions.push(decide_update(
                package_name,
                Some(*baseline_review),
                Some(candidate_review),
                unavailable_baseline_sources,
            ));
        } else {
            unmatched_baseline.push(*baseline_review);
        }
    }
    let replacement_count = unmatched_baseline.len().min(unmatched_candidate.len());
    for (baseline_review, candidate_review) in unmatched_baseline
        .drain(..replacement_count)
        .zip(unmatched_candidate.drain(..replacement_count))
    {
        decisions.push(decide_update(
            package_name,
            Some(baseline_review),
            Some(candidate_review),
            unavailable_baseline_sources,
        ));
    }
    decisions.extend(unmatched_baseline.into_iter().map(|baseline_review| {
        decide_update::<B>(
            package_name,
            Some(baseline_review),
            None,
            unavailable_baseline_sources,
        )
    }));
    decisions.extend(unmatched_candidate.into_iter().map(|candidate_review| {
        decide_update::<B>(
            package_name,
            None,
            Some(candidate_review),
            unavailable_baseline_sources,
        )
    }));
}

fn decide_update<B: PackageReviewEvidence>(
    package_name: &str,
    baseline: Option<&B>,
    candidate: Option<&CompilerIssuedPackageReview>,
    unavailable_baseline_sources: &BTreeSet<PackageKey>,
) -> PackageTriageDecision {
    let mut reasons = Vec::new();
    let disposition = match (baseline, candidate) {
        (Some(baseline), Some(candidate)) if baseline.key() != candidate.key() => {
            reasons.push(PackageTriageReason::SourceLineageChanged);
            if unavailable_baseline_sources.contains(baseline.key()) {
                reasons.push(PackageTriageReason::BaselineSourceUnavailable);
            }
            append_candidate_blocking_reasons(candidate, &mut reasons);
            append_candidate_audit_reasons(candidate, true, &mut reasons);
            PackageTriageDisposition::BlockedProvenanceChange
        }
        (Some(baseline), Some(candidate)) => {
            let mut disposition = PackageTriageDisposition::Admitted;
            if baseline.whole_review_commitment()
                != PackageReviewEvidence::whole_review_commitment(candidate)
            {
                reasons.push(PackageTriageReason::CapabilityOrApiChanged);
                disposition = match changed_review_risk(baseline, candidate) {
                    Some(PackageReviewCanonicalRowRisk::AuditRecommended) => {
                        PackageTriageDisposition::AdmittedWithAuditRecommended
                    }
                    Some(
                        PackageReviewCanonicalRowRisk::Blocking
                        | PackageReviewCanonicalRowRisk::OpaqueBlocking,
                    )
                    | None => PackageTriageDisposition::BlockedCapabilityChange,
                };
            }
            if baseline.resolution() != candidate.resolution()
                || baseline.source_consumption_commitment()
                    != PackageReviewEvidence::source_consumption_commitment(candidate)
            {
                reasons.push(PackageTriageReason::SourceChanged);
            }
            if baseline.build_observation_commitment()
                != PackageReviewEvidence::build_observation_commitment(candidate)
            {
                reasons.push(PackageTriageReason::BuildObservationChanged);
                disposition =
                    disposition.max(PackageTriageDisposition::AdmittedWithAuditRecommended);
            }
            if unavailable_baseline_sources.contains(baseline.key()) {
                reasons.push(PackageTriageReason::BaselineSourceUnavailable);
                disposition =
                    disposition.max(PackageTriageDisposition::AdmittedWithAuditRecommended);
            }
            let representation_changed = row_family_changed(
                baseline,
                candidate,
                PackageReviewCanonicalRowKind::RepresentationTcb,
            );
            if append_candidate_audit_reasons(candidate, representation_changed, &mut reasons) {
                disposition =
                    disposition.max(PackageTriageDisposition::AdmittedWithAuditRecommended);
            }
            disposition
        }
        (None, Some(candidate)) => {
            reasons.push(PackageTriageReason::NewTransitivePackage);
            let blocking = append_candidate_blocking_reasons(candidate, &mut reasons);
            let recommendation = append_candidate_audit_reasons(candidate, true, &mut reasons);
            if blocking {
                PackageTriageDisposition::BlockedCapabilityChange
            } else if recommendation {
                PackageTriageDisposition::AdmittedWithAuditRecommended
            } else {
                PackageTriageDisposition::Admitted
            }
        }
        (Some(_), None) => {
            reasons.push(PackageTriageReason::RemovedPackage);
            PackageTriageDisposition::BlockedCapabilityChange
        }
        (None, None) => unreachable!("name union contains at least one package review"),
    };
    PackageTriageDecision {
        package_name: package_name.to_owned(),
        baseline_key: baseline.map(|review| review.key().clone()),
        candidate_key: candidate.map(|review| review.key().clone()),
        disposition,
        reasons,
    }
}

fn row_family_changed(
    baseline: &impl PackageReviewEvidence,
    candidate: &impl PackageReviewEvidence,
    kind: PackageReviewCanonicalRowKind,
) -> bool {
    let mut baseline_rows = baseline
        .canonical_rows()
        .iter()
        .filter(|row| row.kind() == kind);
    let mut candidate_rows = candidate
        .canonical_rows()
        .iter()
        .filter(|row| row.kind() == kind);
    loop {
        match (baseline_rows.next(), candidate_rows.next()) {
            (Some(left), Some(right)) if left.canonical_bytes() == right.canonical_bytes() => {}
            (None, None) => return false,
            _ => return true,
        }
    }
}

fn append_candidate_blocking_reasons(
    candidate: &CompilerIssuedPackageReview,
    reasons: &mut Vec<PackageTriageReason>,
) -> bool {
    if candidate
        .canonical_rows()
        .iter()
        .any(|row| row.kind() == PackageReviewCanonicalRowKind::AcceptedClaim)
    {
        reasons.push(PackageTriageReason::AcceptedClaimRequiresResolution);
        true
    } else {
        false
    }
}

fn append_candidate_audit_reasons(
    candidate: &CompilerIssuedPackageReview,
    representation_changed: bool,
    reasons: &mut Vec<PackageTriageReason>,
) -> bool {
    let mut recommend = false;
    if representation_changed && !candidate.projection().representation_tcb().is_empty() {
        reasons.push(PackageTriageReason::RepresentationTcbIntroducedOrChanged);
        recommend = true;
    }
    for class in candidate
        .projection()
        .dangerous_authorities()
        .iter()
        .map(|authority| authority.class())
        .collect::<BTreeSet<_>>()
    {
        reasons.push(PackageTriageReason::RetainedDangerousAuthority(class));
        recommend = true;
    }
    for class in candidate
        .projection()
        .dangerous_authority_slack()
        .iter()
        .map(|slack| slack.class())
        .collect::<BTreeSet<_>>()
    {
        reasons.push(PackageTriageReason::DangerousAuthoritySlack(class));
        recommend = true;
    }
    recommend
}

#[cfg(test)]
mod tests;
