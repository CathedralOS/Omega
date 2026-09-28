//! Calling policies use the shared representation target invariant.

pub(super) fn validate(
    target: crate::record::PackageReviewRepresentationTarget,
) -> Result<(), &'static str> {
    target.validate_canonical_structure()
}
