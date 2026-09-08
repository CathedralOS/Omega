//! Input-only custody for functions whose structural places are local literals.
use legalized_operations::{LegalizedScalarFunction, LegalizedScalarInstructionKind};
use semantic_vocabulary::StructuralPlaceKind;
use terminal_psi::{ByteSequenceCarrier, StructuralTypeShape};

pub(super) fn accepts(source: &LegalizedScalarFunction) -> bool {
    let Some(signature) = &source.structural else {
        return false;
    };
    let [block] = source.blocks.as_slice() else {
        return false;
    };
    if source.attachment.is_some()
        || source.ranked.is_some()
        || !signature.parameters.is_empty()
        || signature.structural_places.is_empty()
        || !signature.entry_claims.is_empty()
        || !signature.published_service_ceiling.is_empty()
        || source.parameters.len() != source.call_plan.parameters.len()
        || source
            .parameters
            .iter()
            .zip(&source.call_plan.parameters)
            .any(|(parameter, placement)| parameter.placement != *placement)
    {
        return false;
    }
    signature.structural_places.iter().enumerate().all(|(ordinal, declaration)| {
        let Some(row) = block.instructions.get(ordinal) else { return false; };
        let Ok(declaration_ordinal) = u32::try_from(ordinal) else { return false; };
        matches!(&row.kind,
            LegalizedScalarInstructionKind::EstablishByteSequenceLiteral {
                destination, structural_type, ..
            } if row.result.is_none() && destination == declaration
                && destination.kind == StructuralPlaceKind::ByteSequenceLiteral {
                    declaration_ordinal, structural_type: structural_type.id
                }
                && signature.structural_types.contains(structural_type)
                && structural_type.shape == StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView))
    }) && block.instructions.iter().filter(|row| matches!(row.kind,
        LegalizedScalarInstructionKind::EstablishByteSequenceLiteral { .. }
    )).count() == signature.structural_places.len()
}
