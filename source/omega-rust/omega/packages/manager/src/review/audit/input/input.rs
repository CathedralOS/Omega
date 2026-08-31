use crate::review::{
    CompilerReviewTriage, PackageSourcePatch, PackageSourcePatchLimits, PackageTriageDecision,
    PackageTriageDisposition, PackageTriageReason,
};

use super::error::PackageSourceReviewRenderError;

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

/// Deterministic source-review input for optional human or automated tooling.
///
/// Compiler triage stays in its package-prose-free lane. Source patches are
/// separately framed hostile-data lanes and cannot alter deterministic
/// dispositions or mint admission evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageSourceReviewInput {
    pub(super) triage: CompilerReviewTriage,
    pub(super) source_patches: Vec<PackageSourcePatch>,
}

impl PackageSourceReviewInput {
    pub const fn triage(&self) -> &CompilerReviewTriage {
        &self.triage
    }

    pub fn source_patches(&self) -> &[PackageSourcePatch] {
        &self.source_patches
    }

    /// Whether compiler-owned policy already recommends an audit independently
    /// of any optional review tooling.
    pub fn deterministic_audit_recommended(&self) -> bool {
        self.triage
            .decisions()
            .iter()
            .any(deterministic_decision_recommends_audit)
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

fn deterministic_decision_recommends_audit(decision: &PackageTriageDecision) -> bool {
    decision.disposition() == PackageTriageDisposition::NoReviewBlockerWithAuditRecommended
        || decision.reasons().iter().any(|reason| {
            matches!(
                reason,
                PackageTriageReason::BaselineSourceUnavailable
                    | PackageTriageReason::BuildObservationChanged
                    | PackageTriageReason::RepresentationTcbIntroducedOrChanged
                    | PackageTriageReason::ExternalExecutableSupplyRequiresResolution
                    | PackageTriageReason::RetainedDangerousAuthority(_)
                    | PackageTriageReason::DangerousAuthoritySlack(_)
            )
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

#[cfg(test)]
mod tests {
    use super::{REVIEW_INPUT_SCHEMA, required_review_input_bytes};

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
