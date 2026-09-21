//! Total receipt-free row projection under one sizing and emission budget.

mod assembly;
mod builder;
mod components;
mod declarations;

use super::{PackageReviewEncodingError, baseline, encoder::Encoder};
use crate::record::*;
use builder::Builder;
use semantic_vocabulary::PackageKeyIdentity;

impl PackagePolicyBaseline {
    pub fn canonical_rows_with_limits(
        &self,
        limits: PackagePolicyRowLimits,
    ) -> Result<(Vec<PackagePolicyRow>, PackagePolicyRowUsage), PackageReviewEncodingError> {
        // Public construction and recovery already validate the private typed
        // fields. Do not repeat unbudgeted association scans before row sizing.
        let limits = limits.bounded();
        let count = assembly::count(self)?;
        let mut builder = Builder::new(self, count, limits, false)?;
        assembly::project(&mut builder, self)?;
        builder.finish()
    }
    /// Project only explicit acceptance obligations. Rebuildable API, provider,
    /// representation and dependency descriptions remain in the fresh audit.
    pub fn acceptance_rows_with_limits(
        &self,
        limits: PackagePolicyRowLimits,
    ) -> Result<(Vec<PackagePolicyRow>, PackagePolicyRowUsage), PackageReviewEncodingError> {
        let count = assembly::acceptance_count(self)?;
        let mut builder = Builder::new(self, count, limits.bounded(), true)?;
        assembly::project(&mut builder, self)?;
        builder.finish()
    }
}

/// Render one restricted build-host request as its acceptance row's canonical
/// text for the given checked occurrence — the same retained meaning a
/// whole-policy projection emits under kind `restricted_build_request`.
///
/// The row's ordinal joins its coordinate key, not this text: the retained
/// meaning compares request content only, so a grant issued under the
/// accepted policy matches the identical request wherever issue order lands.
/// The join this serves — an occurrence's own build effect waiting on
/// retained consent — is granted or refused by this one meaning.
pub fn restricted_build_request_acceptance_text(
    package: PackageKeyIdentity,
    target: target::TargetProfile,
    request: &build_evaluation::RestrictedBuildRequest,
) -> Result<String, PackageReviewEncodingError> {
    let limits = PackagePolicyRowLimits::default();
    let request = crate::record::PackagePolicyRestrictedBuildRequest::from(request);
    let frame = |encoder: &mut Encoder| -> Result<(), PackageReviewEncodingError> {
        builder::frame_row(
            encoder,
            package,
            target,
            PackagePolicyRowKind::RestrictedBuildRequest,
            true,
            true,
            &|encoder| baseline::restricted_build::request(encoder, &request),
        )
    };
    let mut measure = Encoder::row_measure(
        limits.maximum_canonical_bytes,
        Some(limits.maximum_text_bytes),
        limits.maximum_sequence_elements,
        limits.maximum_depth,
    );
    frame(&mut measure)?;
    let (binary_length, text_length, remaining) = measure.row_metrics()?;
    let mut output = Encoder::row_output(
        binary_length,
        Some(text_length),
        remaining,
        limits.maximum_depth,
    )?;
    frame(&mut output)?;
    let (binary, text, _) = output.row_metrics()?;
    if (binary, text) != (binary_length, text_length) {
        return Err(rejected(
            "package policy row sizing disagrees with emission",
        ));
    }
    let (_, canonical_text) = output.finish_row()?;
    Ok(canonical_text)
}

fn rejected(message: &'static str) -> PackageReviewEncodingError {
    PackageReviewEncodingError::new(message)
}
