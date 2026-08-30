use crate::resolution::graph::ResolvedPackageSourceClosure;
use crate::manifest::PackageKey;
use crate::review::{
    CompilerReviewTriage, PackageTriageDecision, PackageTriageDisposition, PackageTriageReason,
    render_package_source_patch,
};
use crate::resolution::source::PackageSourceCustody;
use std::collections::BTreeMap;

use super::super::error::PackageSourceReviewError;
use super::super::input::{PackageSourceReviewInput, PackageSourceReviewLimits};

pub(super) fn assemble_source_patches(
    triage: CompilerReviewTriage,
    baseline_sources: &BTreeMap<PackageKey, &PackageSourceCustody>,
    candidate_sources: &ResolvedPackageSourceClosure,
    limits: PackageSourceReviewLimits,
    initial: bool,
) -> Result<PackageSourceReviewInput, PackageSourceReviewError> {
    let selected = triage
        .decisions()
        .iter()
        .filter(|decision| source_patch_required(decision, initial))
        .collect::<Vec<_>>();
    if selected.len() > limits.maximum_source_patches() {
        return Err(PackageSourceReviewError::TooManySourcePatches {
            maximum: limits.maximum_source_patches(),
            required: selected.len(),
        });
    }
    let mut source_patches = Vec::new();
    source_patches
        .try_reserve_exact(selected.len())
        .map_err(|_| PackageSourceReviewError::TooManySourcePatches {
            maximum: limits.maximum_source_patches(),
            required: selected.len(),
        })?;
    for decision in selected {
        let candidate_key = decision
            .candidate_key()
            .expect("source review selection always retains a candidate");
        let candidate = candidate_sources
            .custody(candidate_key)
            .expect("complete candidate custody was validated");
        let baseline = if decision
            .reasons()
            .iter()
            .any(|reason| matches!(reason, PackageTriageReason::SourceLineageChanged))
        {
            None
        } else {
            decision
                .baseline_key()
                .and_then(|key| baseline_sources.get(key).copied())
        };
        let patch = render_package_source_patch(baseline, candidate, limits.source_patch())
            .map_err(|error| PackageSourceReviewError::SourcePatch {
                package: candidate_key.clone(),
                error,
            })?;
        source_patches.push(patch);
    }
    Ok(PackageSourceReviewInput {
        triage,
        source_patches,
    })
}

fn source_patch_required(decision: &PackageTriageDecision, initial: bool) -> bool {
    if decision.candidate_key().is_none() {
        return false;
    }
    if initial {
        return decision.disposition() != PackageTriageDisposition::Admitted;
    }
    decision.reasons().iter().any(|reason| {
        matches!(
            reason,
            PackageTriageReason::SourceChanged
                | PackageTriageReason::BaselineSourceUnavailable
                | PackageTriageReason::SourceLineageChanged
        ) || (matches!(reason, PackageTriageReason::NewTransitivePackage)
            && decision.disposition() != PackageTriageDisposition::Admitted)
    })
}
