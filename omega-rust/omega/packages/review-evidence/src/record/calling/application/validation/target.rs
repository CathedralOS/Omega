// Calling-policy target validation delegating to `PackageReviewRepresentationTarget`.

pub(super) fn validate(
    target: crate::record::PackageReviewRepresentationTarget,
) -> Result<(), &'static str> {
    target.validate_canonical_structure()
}
