//! Closed physical footprint of a caller-activation establishment. The value
//! prefix retains ordinary ABI geometry; its private backing never transports.
use calling_conventions::ValueShape;
use legalized_operations::{LegalizedScalarFunction, LegalizedScalarInstructionKind as K};
use selected_instructions::LocalStorageSlotId;

pub(super) fn footprint(ordinary: ValueShape, bound: u64) -> Option<ValueShape> {
    let offset = ordinary.byte_size.checked_add(7)? & !7;
    Some(ValueShape {
        byte_size: offset
            .checked_add(8)?
            .checked_add(u32::try_from(bound).ok()?)?,
        alignment: ordinary.alignment.max(8),
        ..ordinary
    })
}

pub(super) fn local_shape(
    source: &LegalizedScalarFunction,
    slot: LocalStorageSlotId,
    ordinary: ValueShape,
) -> Option<ValueShape> {
    let LocalStorageSlotId::Structural { operation, place } = slot else {
        return Some(ordinary);
    };
    let mut producers = source
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter(|row| row.operation == operation);
    let row = producers.next()?;
    if producers.next().is_some() {
        return None;
    }
    let K::EstablishActivationClaim {
        result,
        bound,
        backing_type,
        layout,
        ..
    } = &row.kind
    else {
        return Some(ordinary);
    };
    if result.place != place || layout.shape != ordinary {
        return None;
    }
    let types = &source.structural.as_ref()?.structural_types;
    let backing = terminal_psi::find_structural_type(types, *backing_type)?;
    if backing.owns_linear_claim
        || backing.descriptor_cleanup
        || !backing.no_code_affine_cleanup
        || backing.shape
            != terminal_psi::StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity: *bound },
            )
    {
        return None;
    }
    footprint(ordinary, *bound)
}
