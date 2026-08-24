use crate::{
    CompilerIssuedPackageReviewSet, CompilerReviewTriage, PackageKey, PackageSourceCustody,
    PackageSourcePatch, PackageSourcePatchError, PackageSourcePatchLimits, PackageTriageDecision,
    PackageTriageDisposition, PackageTriageReason, ResolvedPackageSourceClosure, TriageRenderError,
    render_package_source_patch, triage_initial_install, triage_review_update,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

const REVIEW_INPUT_SCHEMA: &str = "OMEGA_PACKAGE_REVIEW_INPUT_V1\n";

/// Resource policy for assembling source packets. The final combined review
/// input has a separate caller-supplied ceiling at render time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackageSourceReviewLimits {
    maximum_source_patches: usize,
    source_patch: PackageSourcePatchLimits,
}

impl PackageSourceReviewLimits {
    pub const fn new(
        maximum_source_patches: usize,
        source_patch: PackageSourcePatchLimits,
    ) -> Self {
        Self {
            maximum_source_patches,
            source_patch,
        }
    }

    pub const fn maximum_source_patches(self) -> usize {
        self.maximum_source_patches
    }

    pub const fn source_patch(self) -> PackageSourcePatchLimits {
        self.source_patch
    }
}

impl Default for PackageSourceReviewLimits {
    fn default() -> Self {
        Self::new(4_096, PackageSourcePatchLimits::default())
    }
}

/// Deterministic review input before any advisory model invocation.
///
/// Compiler triage stays in its package-prose-free lane. Source patches are
/// separately framed hostile-data lanes and cannot alter deterministic
/// dispositions or mint admission evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageSourceReviewInput {
    triage: CompilerReviewTriage,
    source_patches: Vec<PackageSourcePatch>,
}

impl PackageSourceReviewInput {
    pub const fn triage(&self) -> &CompilerReviewTriage {
        &self.triage
    }

    pub fn source_patches(&self) -> &[PackageSourcePatch] {
        &self.source_patches
    }

    pub fn render_bounded(
        &self,
        maximum_bytes: usize,
    ) -> Result<String, PackageSourceReviewRenderError> {
        let triage = self
            .triage
            .render_bounded(maximum_bytes)
            .map_err(PackageSourceReviewRenderError::Triage)?;
        let required_bytes = required_review_input_bytes(&triage, &self.source_patches);
        if required_bytes > maximum_bytes {
            return Err(PackageSourceReviewRenderError::TotalExceeded {
                maximum_bytes,
                required_bytes,
            });
        }
        let mut rendered = String::with_capacity(required_bytes);
        rendered.push_str(REVIEW_INPUT_SCHEMA);
        rendered.push_str("triage_begin\n");
        rendered.push_str(&triage);
        rendered.push_str("triage_end\nsource_patch_count ");
        rendered.push_str(&self.source_patches.len().to_string());
        rendered.push('\n');
        for patch in &self.source_patches {
            rendered.push_str("source_patch_begin\n");
            rendered.push_str(patch.as_str());
            rendered.push_str("source_patch_end\n");
        }
        rendered.push_str("end_review_input\n");
        debug_assert_eq!(rendered.len(), required_bytes);
        Ok(rendered)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageSourceReviewCustodyRole {
    Baseline,
    Candidate,
}

#[derive(Debug)]
pub enum PackageSourceReviewError {
    MissingCustody {
        role: PackageSourceReviewCustodyRole,
        package: PackageKey,
    },
    UnexpectedCustody {
        role: PackageSourceReviewCustodyRole,
        package: PackageKey,
    },
    DuplicateCustody {
        role: PackageSourceReviewCustodyRole,
        package: PackageKey,
    },
    ResolutionMismatch {
        role: PackageSourceReviewCustodyRole,
        package: PackageKey,
    },
    TooManySourcePatches {
        maximum: usize,
        required: usize,
    },
    SourcePatch {
        package: PackageKey,
        error: PackageSourcePatchError,
    },
}

impl fmt::Display for PackageSourceReviewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCustody { role, package } => write!(
                formatter,
                "{} review row `{}` has no matching resolver custody",
                custody_role_token(*role),
                package.name().as_str()
            ),
            Self::UnexpectedCustody { role, package } => write!(
                formatter,
                "{} resolver custody `{}` has no matching compiler review row",
                custody_role_token(*role),
                package.name().as_str()
            ),
            Self::DuplicateCustody { role, package } => write!(
                formatter,
                "{} resolver custody repeats package `{}`",
                custody_role_token(*role),
                package.name().as_str()
            ),
            Self::ResolutionMismatch { role, package } => write!(
                formatter,
                "{} resolver custody and compiler review disagree on `{}` resolution",
                custody_role_token(*role),
                package.name().as_str()
            ),
            Self::TooManySourcePatches { maximum, required } => write!(
                formatter,
                "source review requires {required} patches, exceeding the {maximum}-patch ceiling"
            ),
            Self::SourcePatch { package, error } => write!(
                formatter,
                "cannot render source review for `{}`: {error}",
                package.name().as_str()
            ),
        }
    }
}

impl std::error::Error for PackageSourceReviewError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageSourceReviewRenderError {
    Triage(TriageRenderError),
    TotalExceeded {
        maximum_bytes: usize,
        required_bytes: usize,
    },
}

impl fmt::Display for PackageSourceReviewRenderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Triage(error) => error.fmt(formatter),
            Self::TotalExceeded {
                maximum_bytes,
                required_bytes,
            } => write!(
                formatter,
                "package review input requires {required_bytes} bytes, exceeding the {maximum_bytes}-byte ceiling"
            ),
        }
    }
}

impl std::error::Error for PackageSourceReviewRenderError {}

/// Assemble initial-install review input. Pure candidates remain represented
/// in deterministic triage but receive source packets only when compiler facts
/// already recommend audit.
pub fn assemble_initial_source_review(
    candidate_reviews: &CompilerIssuedPackageReviewSet,
    candidate_sources: &ResolvedPackageSourceClosure,
    limits: PackageSourceReviewLimits,
) -> Result<PackageSourceReviewInput, PackageSourceReviewError> {
    validate_complete_custody(
        candidate_reviews,
        candidate_sources,
        PackageSourceReviewCustodyRole::Candidate,
    )?;
    let triage = triage_initial_install(candidate_reviews);
    assemble_source_patches(triage, &BTreeMap::new(), candidate_sources, limits, true)
}

/// Assemble update review input from compiler-issued baseline/candidate rows,
/// every recovered old custody, and the complete candidate closure.
///
/// Missing old custody is derived here and cannot erase the accepted compiler
/// baseline. It selects standalone candidate review for that exact package.
pub fn assemble_update_source_review(
    baseline_reviews: &CompilerIssuedPackageReviewSet,
    candidate_reviews: &CompilerIssuedPackageReviewSet,
    recovered_baseline_sources: &[PackageSourceCustody],
    candidate_sources: &ResolvedPackageSourceClosure,
    limits: PackageSourceReviewLimits,
) -> Result<PackageSourceReviewInput, PackageSourceReviewError> {
    validate_complete_custody(
        candidate_reviews,
        candidate_sources,
        PackageSourceReviewCustodyRole::Candidate,
    )?;
    let baseline_sources = validate_partial_custody(
        baseline_reviews,
        recovered_baseline_sources,
        PackageSourceReviewCustodyRole::Baseline,
    )?;
    let unavailable = baseline_reviews
        .reviews()
        .iter()
        .filter(|review| !baseline_sources.contains_key(review.key()))
        .map(|review| review.key().clone())
        .collect::<BTreeSet<_>>();
    let triage = triage_review_update(baseline_reviews, candidate_reviews, &unavailable);
    assemble_source_patches(triage, &baseline_sources, candidate_sources, limits, false)
}

fn validate_complete_custody(
    reviews: &CompilerIssuedPackageReviewSet,
    sources: &ResolvedPackageSourceClosure,
    role: PackageSourceReviewCustodyRole,
) -> Result<(), PackageSourceReviewError> {
    for review in reviews.reviews() {
        let custody = sources.custody(review.key()).ok_or_else(|| {
            PackageSourceReviewError::MissingCustody {
                role,
                package: review.key().clone(),
            }
        })?;
        if custody.resolution() != review.resolution() {
            return Err(PackageSourceReviewError::ResolutionMismatch {
                role,
                package: review.key().clone(),
            });
        }
    }
    for custody in sources.custodies() {
        if reviews.review(custody.key()).is_none() {
            return Err(PackageSourceReviewError::UnexpectedCustody {
                role,
                package: custody.key().clone(),
            });
        }
    }
    Ok(())
}

fn validate_partial_custody<'source>(
    reviews: &CompilerIssuedPackageReviewSet,
    sources: &'source [PackageSourceCustody],
    role: PackageSourceReviewCustodyRole,
) -> Result<BTreeMap<PackageKey, &'source PackageSourceCustody>, PackageSourceReviewError> {
    let mut validated = BTreeMap::new();
    for custody in sources {
        let review = reviews.review(custody.key()).ok_or_else(|| {
            PackageSourceReviewError::UnexpectedCustody {
                role,
                package: custody.key().clone(),
            }
        })?;
        if custody.resolution() != review.resolution() {
            return Err(PackageSourceReviewError::ResolutionMismatch {
                role,
                package: custody.key().clone(),
            });
        }
        if validated.insert(custody.key().clone(), custody).is_some() {
            return Err(PackageSourceReviewError::DuplicateCustody {
                role,
                package: custody.key().clone(),
            });
        }
    }
    Ok(validated)
}

fn assemble_source_patches(
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
        return decision.disposition() == PackageTriageDisposition::AdmittedWithAuditRecommended;
    }
    decision.reasons().iter().any(|reason| {
        matches!(
            reason,
            PackageTriageReason::SourceChanged
                | PackageTriageReason::BaselineSourceUnavailable
                | PackageTriageReason::SourceLineageChanged
        ) || (matches!(reason, PackageTriageReason::NewTransitivePackage)
            && decision.disposition() == PackageTriageDisposition::AdmittedWithAuditRecommended)
    })
}

fn required_review_input_bytes(triage: &str, patches: &[PackageSourcePatch]) -> usize {
    let mut required = REVIEW_INPUT_SCHEMA.len();
    required = required.saturating_add("triage_begin\n".len());
    required = required.saturating_add(triage.len());
    required = required.saturating_add("triage_end\nsource_patch_count \n".len());
    required = required.saturating_add(patches.len().to_string().len());
    for patch in patches {
        required = required.saturating_add("source_patch_begin\n".len());
        required = required.saturating_add(patch.as_str().len());
        required = required.saturating_add("source_patch_end\n".len());
    }
    required.saturating_add("end_review_input\n".len())
}

const fn custody_role_token(role: PackageSourceReviewCustodyRole) -> &'static str {
    match role {
        PackageSourceReviewCustodyRole::Baseline => "baseline",
        PackageSourceReviewCustodyRole::Candidate => "candidate",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combined_render_size_accounting_rejects_without_truncating() {
        let triage = "OMEGA_PACKAGE_SOURCE_TRIAGE_V1\n";
        let required = required_review_input_bytes(triage, &[]);
        assert!(required > triage.len());
        assert_eq!(
            required,
            REVIEW_INPUT_SCHEMA.len()
                + "triage_begin\n".len()
                + triage.len()
                + "triage_end\nsource_patch_count \n".len()
                + 1
                + "end_review_input\n".len()
        );
    }
}
