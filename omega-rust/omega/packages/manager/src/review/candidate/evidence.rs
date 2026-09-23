//! Common package-review evidence interface and live compiler adaptation.

use crate::declarations::PackageKey;
use crate::review::CompilerIssuedPackageReview;
#[cfg(test)]
use crate::review::SemanticBindingReview;
use package_evidence::record::PackageReviewCanonicalRow;
use package_source::ImmutableSourceResolution;

use super::rows::ReviewOnlySourceConsumptionCommitment;
use super::{build_observation_commitment, whole_review_commitment};

/// The package-manager-facing view of compiler review facts used by comparison
/// and source-triage operations.
///
/// This trait is deliberately private. Implementing it does not issue accepted
/// evidence or permit construction of a package instance.
pub(crate) trait PackageReviewEvidence {
    fn key(&self) -> &PackageKey;
    fn resolution(&self) -> &ImmutableSourceResolution;
    fn projection_identity_matches(&self) -> bool;
    fn target_name(&self) -> &str;
    fn source_consumption_commitment(&self) -> ReviewOnlySourceConsumptionCommitment;
    fn build_observation_commitment(&self) -> Option<[u8; 32]>;
    fn whole_review_commitment(&self) -> [u8; 32];
    // Borrow the issued rows: row bytes are not admission authority, and a
    // second owned representation would not establish any additional check.
    fn canonical_rows(&self) -> &[PackageReviewCanonicalRow];
}

impl PackageReviewEvidence for CompilerIssuedPackageReview {
    fn key(&self) -> &PackageKey {
        CompilerIssuedPackageReview::key(self)
    }

    fn resolution(&self) -> &ImmutableSourceResolution {
        CompilerIssuedPackageReview::resolution(self)
    }

    fn projection_identity_matches(&self) -> bool {
        self.projection().package() == self.key().identity()
    }

    fn target_name(&self) -> &str {
        self.projection().target().target_name()
    }

    fn source_consumption_commitment(&self) -> ReviewOnlySourceConsumptionCommitment {
        CompilerIssuedPackageReview::source_consumption_commitment(self).into()
    }

    fn build_observation_commitment(&self) -> Option<[u8; 32]> {
        self.build_observation_summary()
            .map(build_observation_commitment)
    }

    fn whole_review_commitment(&self) -> [u8; 32] {
        whole_review_commitment(self.canonical_review_bytes())
    }

    fn canonical_rows(&self) -> &[PackageReviewCanonicalRow] {
        CompilerIssuedPackageReview::canonical_rows(self)
    }
}

#[cfg(test)]
mod tests {
    use super::{PackageReviewEvidence, SemanticBindingReview};
    use crate::resolution::graph::{
        PackageSourceClosureLimits, resolve_external_local_package_closure_from_hardened_base,
    };
    use crate::review::compile_resolved_package_reviews;
    use package_source::{ExternalSourceContext, LocalSourceLimits};

    #[test]
    fn comparison_borrows_issued_rows_and_whole_review_equality_keeps_source_locations() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temporary = std::env::temp_dir().join(format!(
            "omega-borrowed-review-{}-{stamp}",
            std::process::id()
        ));
        let root = temporary.join("source");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("build.omg"),
            "machine build(builder: &mut Build) { builder.package(\"borrowed_review\"); }\n",
        )
        .unwrap();
        let mut reviews = Vec::new();
        for prefix in ["", "\n\n"] {
            std::fs::write(
                root.join("main.omg"),
                format!("{prefix}pub const VALUE: u64 = 17;\n"),
            )
            .unwrap();
            let closure = resolve_external_local_package_closure_from_hardened_base(
                &root,
                ExternalSourceContext::derive(b"borrowed-review-rows"),
                temporary.join("cache"),
                LocalSourceLimits::default(),
                PackageSourceClosureLimits::default(),
            )
            .unwrap();
            let issued = compile_resolved_package_reviews(
                &closure.for_exact_target(target::TargetProfile::WindowsX64),
                &temporary.join("build"),
                SemanticBindingReview::Explicit(&[]),
            )
            .unwrap();
            let review = &issued.reviews()[0];
            let compared = PackageReviewEvidence::canonical_rows(review);
            assert!(!compared.is_empty());
            assert!(std::ptr::eq(compared, review.canonical_rows()));
            assert_eq!(review, &review.clone());
            reviews.push(review.clone());
        }
        let mut changed_source = reviews[0].clone();
        changed_source.canonical_rows = reviews[1].canonical_rows.clone();
        assert_eq!(reviews[0].canonical_rows(), changed_source.canonical_rows());
        assert_ne!(
            reviews[0], changed_source,
            "whole-review equality must retain diagnostic source identity"
        );
        let _ = std::fs::remove_dir_all(temporary);
    }
}
