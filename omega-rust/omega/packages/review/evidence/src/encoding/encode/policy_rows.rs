//! Total receipt-free row projection under one sizing and emission budget.

mod assembly;
mod builder;
mod components;
mod declarations;

use super::{PackageReviewEncodingError, encoder::Encoder};
use crate::record::*;
use builder::Builder;

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

fn rejected(message: &'static str) -> PackageReviewEncodingError {
    PackageReviewEncodingError::new(message)
}
