//! Exact producer joins; selected SSA validation independently checks dominance.
use legalized_operations::{
    LegalizedScalarArgument, LegalizedScalarFunction, LegalizedScalarInstructionKind,
};
use semantic_vocabulary::{OperationId, PlaceId, StructuralPlaceKind};
use target_operations::{TargetStructuralArgument, TargetStructuralArgumentSource};
use terminal_psi::{ByteSequenceCarrier, StructuralMultiplicity, StructuralTypeShape};

pub(super) fn called(source: &LegalizedScalarFunction, place: PlaceId) -> bool {
    source
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .any(|row| {
            matches!(&row.kind, LegalizedScalarInstructionKind::Call(call)
            if call.arguments.iter().any(|argument|
                matches!(argument, LegalizedScalarArgument::Structural { semantic, .. }
                    if semantic.place == place)))
        })
}

pub(super) fn accepts(
    source: &LegalizedScalarFunction,
    call: OperationId,
    target: &TargetStructuralArgument,
) -> Option<()> {
    let TargetStructuralArgumentSource::EstablishedByteView { psi_operation } = target.source
    else {
        return None;
    };
    let signature = source.structural.as_ref()?;
    let mut occurrences = source.blocks.iter().flat_map(|block| {
        block
            .instructions
            .iter()
            .enumerate()
            .map(move |(position, row)| (block.id, position, row))
    });
    let producer = occurrences
        .clone()
        .find(|(_, _, row)| row.operation == psi_operation)?;
    let used = occurrences.find(|(_, _, row)| row.operation == call)?;
    if producer.2.result.is_some() || producer.0 == used.0 && producer.1 >= used.1 {
        return None;
    }
    // Across blocks, the descriptor's actual defining register must dominate its
    // use. The existing full selected def/use validator checks that relation.
    match &producer.2.kind {
        LegalizedScalarInstructionKind::EstablishByteSequenceLiteral {
            destination,
            structural_type,
            ..
        } => {
            if destination.id != target.place
                || !signature.structural_places.contains(destination)
                || !signature.structural_types.contains(structural_type)
                || structural_type.id != target.structural_type
                || structural_type.shape
                    != StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView)
                || !matches!(destination.kind, StructuralPlaceKind::ByteSequenceLiteral { structural_type: identity, .. } if identity == structural_type.id)
            {
                return None;
            }
        }
        LegalizedScalarInstructionKind::ByteSequenceSubslice { result, .. } => {
            if result.place != target.place
                || result.structural_type != target.structural_type
                || result.multiplicity != StructuralMultiplicity::Unrestricted
                || !result.qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
                || !result.claims.is_empty()
                || !signature.structural_places.iter().any(|place| {
                    place.id == result.place
                        && place.kind
                            == StructuralPlaceKind::OperationResult {
                                producer: psi_operation,
                                structural_type: result.structural_type,
                            }
                })
                || !signature.structural_types.iter().any(|declaration| {
                    declaration.id == result.structural_type
                        && declaration.shape
                            == StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView)
                })
            {
                return None;
            }
        }
        _ => return None,
    }
    Some(())
}
