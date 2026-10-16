//! Publish checked formation judgments on their fresh structural producer.
//! Explicit construction and zero provisioning use the same join in Unit and
//! scalar schedules. This transports exact checked memberships; it does not
//! qualify an existing home, a parameter or a callee's ABI result by annotation.

use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::lowering_error::LoweringError;
use semantic_vocabulary::{PlaceId, StructuralDomainId, StructuralTypeId};

pub(crate) fn publish(
    operations: &mut OperationBuffer,
    place: PlaceId,
    carrier: StructuralTypeId,
    domains: Vec<StructuralDomainId>,
) -> Result<(), LoweringError> {
    let result = operations
        .operations
        .iter_mut()
        .find_map(|operation| match &mut operation.result {
            terminal_psi::OperationResult::Structural(result) if result.place == place => {
                Some(result)
            }
            _ => None,
        })
        .ok_or(LoweringError::Unsupported(
            "fresh construction has no producer",
        ))?;
    if result.structural_type != carrier {
        return Err(LoweringError::Unsupported(
            "formation membership carrier differs from its producer",
        ));
    }
    result.qualifications.extend(domains);
    result.qualifications.sort();
    result.qualifications.dedup();
    Ok(())
}
