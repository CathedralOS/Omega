//! Optimizer module role: executable entrance. Representation-owned validation for retained legalized-operation values.
mod call_source;
mod read_byte;
mod scalar_calls;
use crate::legalized_operations::{LegalizedCallSourceError, LegalizedScalarCall};
pub use scalar_calls::{LegalizedDynamicParameterCallShapeError, LegalizedScalarCallShapeError};
use terminal_psi_to_abstract_operations::optimization_unit::OwnershipEvent;
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
