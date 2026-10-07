//! A callee signature is a declaration, independently of implementation storage.
//!
//! Requirement slots own the same argument geometry as machine declarations.
//! Geometry readers predating this projection consume a declaration adapter
//! anchored to the actual caller place; they never read its place as a callee
//! identity. Source custody stays in the original argument and caller frontier.

use crate::LegalizationError;
use abstract_operations::{AbstractCallSignature, AbstractCallTarget, AbstractOperationPlan};

pub(in crate::legalization) fn signature(
    target: impl Into<AbstractCallTarget>,
    plan: &AbstractOperationPlan,
) -> Result<AbstractCallSignature<'_>, LegalizationError> {
    match target.into() {
        AbstractCallTarget::Machine(machine) => plan
            .functions
            .iter()
            .find(|function| function.machine == machine)
            .map(AbstractCallSignature::Machine)
            .ok_or_else(LegalizationError::custody),
        AbstractCallTarget::Dynamic {
            interface, slot, ..
        } => {
            let interface = plan
                .dynamic_dispatch
                .interface(interface)
                .ok_or_else(LegalizationError::custody)?;
            let row = interface
                .slots
                .get(slot as usize)
                .filter(|row| row.slot == slot)
                .ok_or_else(LegalizationError::custody)?;
            Ok(AbstractCallSignature::Requirement(row))
        }
    }
}

pub(in crate::legalization) fn native_target(
    target: impl Into<AbstractCallTarget>,
) -> target_operations::NativeCallTarget {
    match target.into() {
        AbstractCallTarget::Machine(machine) => {
            target_operations::NativeCallTarget::Machine(machine)
        }
        AbstractCallTarget::Dynamic {
            selector,
            interface,
            slot,
        } => target_operations::NativeCallTarget::Dynamic {
            selector,
            interface,
            slot,
        },
    }
}

pub(in crate::legalization) fn parameter(
    signature: AbstractCallSignature<'_>,
    position: usize,
    place: semantic_vocabulary::PlaceId,
) -> Result<terminal_psi::StructuralParameterDeclaration, LegalizationError> {
    let formal = signature
        .structural_parameter(position)
        .ok_or_else(LegalizationError::custody)?;
    Ok(terminal_psi::StructuralParameterDeclaration {
        place,
        position: formal.position,
        structural_type: formal.structural_type,
        multiplicity: formal.multiplicity,
        access: formal.access,
        qualifications: formal.qualifications.to_vec(),
        projected_qualifications: formal.projected_qualifications.to_vec(),
    })
}
