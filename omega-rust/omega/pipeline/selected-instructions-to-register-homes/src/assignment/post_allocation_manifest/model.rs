use super::PostAllocationOptimizationManifest;

/// Validated authority receipt for the durable manifest record. The transform
/// owns admission; the record itself lives in `register_homes`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPostAllocationOptimizationManifest {
    pub(super) record: PostAllocationOptimizationManifest,
}

impl ValidatedPostAllocationOptimizationManifest {
    pub const fn record(&self) -> &PostAllocationOptimizationManifest {
        &self.record
    }
}
