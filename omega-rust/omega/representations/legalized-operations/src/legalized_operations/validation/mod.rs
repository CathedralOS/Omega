//! Optimizer module role: executable entrance. Representation-owned validation for retained legalized-operation values.
mod call_source;
mod scalar_calls;
use crate::{LegalizedCallSourceError, LegalizedScalarCall};
use optimization_unit::OwnershipEvent;
pub use scalar_calls::LegalizedScalarCallShapeError;
impl LegalizedScalarCall {
    /// Check representation-owned call origin and ownership invariants.
    /// Upstream source, target and installation replay remain required.
    pub fn validate_source(
        &self,
        ownership: &[OwnershipEvent],
    ) -> Result<(), LegalizedCallSourceError> {
        call_source::validate(self, ownership)
    }
}
