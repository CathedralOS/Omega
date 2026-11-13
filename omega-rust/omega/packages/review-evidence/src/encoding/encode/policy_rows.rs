// Total receipt-free row projection (`canonical_rows_with_limits`, `acceptance_rows_with_limits`, and runtime/restricted-build row encoders) under a shared sizing and emission budget.

use crate::record::{
    PackagePolicyBaseline, PackagePolicyRow, PackagePolicyRowKind, PackagePolicyRowLimits,
    PackagePolicyRowUsage, PackagePolicyTerminalPermissions,
};
mod assembly;
mod builder;
mod components;
mod declarations;

use super::{PackageReviewEncodingError, baseline, encoder::Encoder};
use builder::Builder;
use semantic_vocabulary::PackageKeyIdentity;

impl PackagePolicyBaseline {
    pub fn canonical_rows_with_limits(
        &self,
        limits: PackagePolicyRowLimits,
    ) -> Result<(Vec<PackagePolicyRow>, PackagePolicyRowUsage), PackageReviewEncodingError> {
        let limits = limits.bounded();
        let count = assembly::count(self)?;
        let mut builder = Builder::new(self.package, self.target, count, limits, false)?;
        assembly::project(&mut builder, self)?;
        builder.finish()
    }
    pub fn acceptance_rows_with_limits(
        &self,
        limits: PackagePolicyRowLimits,
    ) -> Result<(Vec<PackagePolicyRow>, PackagePolicyRowUsage), PackageReviewEncodingError> {
        let count = assembly::acceptance_count(self)?;
        let mut builder = Builder::new(self.package, self.target, count, limits.bounded(), true)?;
        assembly::project(&mut builder, self)?;
        builder.finish()
    }
}

impl PackagePolicyTerminalPermissions {
    pub fn acceptance_rows_with_limits(
        &self,
        limits: PackagePolicyRowLimits,
    ) -> Result<(Vec<PackagePolicyRow>, PackagePolicyRowUsage), PackageReviewEncodingError> {
        let count = self.services.iter().try_fold(0usize, |count, service| {
            count
                .checked_add(service.permissions.len())
                .ok_or_else(|| rejected("terminal permission row count overflows"))
        })?;
        let mut builder = Builder::new(self.package, self.target, count, limits.bounded(), true)?;
        components::terminal(&mut builder, self)?;
        builder.finish()
    }
}

pub fn restricted_build_request_acceptance_text(
    package: PackageKeyIdentity,
    target: target::TargetProfile,
    request: &package_compilation::build_execution::RestrictedBuildRequest,
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
