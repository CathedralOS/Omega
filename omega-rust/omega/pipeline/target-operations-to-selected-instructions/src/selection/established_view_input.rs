//! Exact view-source joins. The validated source graph establishes descriptor
//! availability; selected SSA checks the separate physical address lifetime.
use legalized_operations::{
    LegalizedScalarArgument, LegalizedScalarFunction, LegalizedScalarInstructionKind,
};
use semantic_vocabulary::{OperationId, PlaceId, StructuralPlaceKind};
use target_operations::{TargetStructuralArgument, TargetStructuralArgumentSource};
use terminal_psi::{ByteSequenceCarrier, StructuralMultiplicity, StructuralTypeShape};

pub(super) fn requires_descriptor(source: &LegalizedScalarFunction, place: PlaceId) -> bool {
    transferred(source, place)
        || source
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

pub(super) fn transferred(source: &LegalizedScalarFunction, place: PlaceId) -> bool {
    source.blocks.iter().any(|block| {
        let successors = match &block.terminator {
            legalized_operations::LegalizedScalarTerminator::Crash { .. }
            | legalized_operations::LegalizedScalarTerminator::Return(_)
            | legalized_operations::LegalizedScalarTerminator::StructuralCase { .. } => {
                [None, None]
            }
            legalized_operations::LegalizedScalarTerminator::Jump { successor, .. } => {
                [Some(successor), None]
            }
            legalized_operations::LegalizedScalarTerminator::Conditional {
                when_true,
                when_false,
                ..
            } => [Some(when_true), Some(when_false)],
        };
        successors.into_iter().flatten().any(|successor| {
            successor
                .structural_bindings
                .iter()
                .any(|binding| binding.argument.place == place)
        })
    })
}

pub(super) fn view_type(
    source: &LegalizedScalarFunction,
    place: PlaceId,
) -> Option<semantic_vocabulary::StructuralTypeId> {
    declared_view_type(source, place).filter(|identity| {
        source.structural.as_ref().is_some_and(|signature| {
            signature.structural_types.iter().any(|declaration| {
                declaration.id == *identity
                    && declaration.shape
                        == StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView)
            })
        })
    })
}

pub(super) fn element_view_type(
    source: &LegalizedScalarFunction,
    place: PlaceId,
) -> Option<semantic_vocabulary::StructuralTypeId> {
    declared_view_type(source, place).filter(|identity| {
        source.structural.as_ref().is_some_and(|signature| {
            signature.structural_types.iter().any(|declaration| {
                declaration.id == *identity
                    && matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
            })
        })
    })
}

fn declared_view_type(
    source: &LegalizedScalarFunction,
    place: PlaceId,
) -> Option<semantic_vocabulary::StructuralTypeId> {
    let signature = source.structural.as_ref()?;
    let parameter = signature
        .parameters
        .iter()
        .map(|parameter| &parameter.semantic)
        .chain(
            source
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .find(|parameter| parameter.place == place);
    if let Some(parameter) = parameter {
        if !matches!(
            parameter.access,
            terminal_psi::StructuralAccess::SharedBorrow
                | terminal_psi::StructuralAccess::MutableBorrow
        ) || parameter.multiplicity != StructuralMultiplicity::Unrestricted
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
        {
            return None;
        }
        return Some(parameter.structural_type);
    }
    source
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|row| match &row.kind {
            LegalizedScalarInstructionKind::ByteSequenceSubslice { result, .. }
            | LegalizedScalarInstructionKind::ElementViewSubslice { result, .. }
                if result.place == place =>
            {
                Some(result.structural_type)
            }
            LegalizedScalarInstructionKind::EstablishByteSequenceLiteral {
                destination,
                structural_type,
                ..
            } if destination.id == place => Some(structural_type.id),
            LegalizedScalarInstructionKind::EstablishElementView { result, .. }
                if result.place == place =>
            {
                Some(result.structural_type)
            }
            _ => None,
        })
}

pub(super) fn accepts(
    source: &LegalizedScalarFunction,
    call: OperationId,
    target: &TargetStructuralArgument,
) -> Option<()> {
    if let TargetStructuralArgumentSource::BlockParameter { block, place } = target.source {
        let owner = source
            .blocks
            .iter()
            .find(|candidate| candidate.id == block)?;
        let parameter = owner
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == place)?;
        let signature = source.structural.as_ref()?;
        return (place == target.place
            && parameter.structural_type == target.structural_type
            && matches!(
                parameter.access,
                terminal_psi::StructuralAccess::SharedBorrow
                    | terminal_psi::StructuralAccess::MutableBorrow
            )
            && parameter.multiplicity == StructuralMultiplicity::Unrestricted
            && parameter.qualifications.is_empty()
            && parameter.projected_qualifications.is_empty()
            && signature.structural_places.iter().any(|candidate| {
                candidate.id == place
                    && candidate.kind
                        == StructuralPlaceKind::BlockParameter {
                            block,
                            position: parameter.position,
                        }
            }))
        .then_some(());
    }
    let (psi_operation, element_view_source) = match &target.source {
        TargetStructuralArgumentSource::EstablishedByteView { psi_operation } => {
            (*psi_operation, false)
        }
        TargetStructuralArgumentSource::EstablishedElementView { psi_operation } => {
            (*psi_operation, true)
        }
        _ => return None,
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
                || element_view_source
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
                || element_view_source
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
        LegalizedScalarInstructionKind::ElementViewSubslice { result, .. } => {
            if result.place != target.place
                || result.structural_type != target.structural_type
                || result.multiplicity != StructuralMultiplicity::Unrestricted
                || !result.qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
                || !result.claims.is_empty()
                || !element_view_source
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
                        && matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
                })
            {
                return None;
            }
        }
        LegalizedScalarInstructionKind::EstablishElementView {
            result,
            destination,
            element,
            ..
        } => {
            if result.place != target.place
                || result.structural_type != target.structural_type
                || *destination != target.place
                || result.multiplicity != StructuralMultiplicity::Unrestricted
                || !result.qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
                || !result.claims.is_empty()
                || !element_view_source
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
                        && matches!(declaration.shape,
                            StructuralTypeShape::ElementView { element: declared }
                                if declared == *element)
                })
            {
                return None;
            }
        }
        _ => return None,
    }
    Some(())
}
