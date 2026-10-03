//! Exact view-source joins. The validated source graph establishes descriptor
//! availability; selected SSA checks the separate physical address lifetime.
use legalized_operations::{
    LegalizedScalarArgument, LegalizedScalarFunction, LegalizedScalarInstructionKind,
    LegalizedScalarTerminator,
};
use semantic_vocabulary::{OperationId, PlaceId, StructuralPlaceKind};
use target_operations::{TargetStructuralArgument, TargetStructuralArgumentSource};
use terminal_psi::{StructuralMultiplicity, StructuralTypeShape};

mod access;
pub(super) use access::view_access;

pub(super) fn requires_descriptor(source: &LegalizedScalarFunction, place: PlaceId) -> bool {
    transferred(source, place)
        || source.blocks.iter().any(|block| matches!(&block.terminator,
            LegalizedScalarTerminator::Return(returned) | LegalizedScalarTerminator::ReturnCleanup { returned, .. } if matches!(&returned.value,
                legalized_operations::LegalizedScalarReturnValue::SharedDescriptor { place: returned } if *returned == place)
                || matches!(&returned.value,
                legalized_operations::LegalizedScalarReturnValue::Structural { source: legalized_operations::LegalizedStructuralCaseSource::OperationResult { result, .. } } if result.place == place)))
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
            | legalized_operations::LegalizedScalarTerminator::ReturnCleanup { .. }
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
                declaration.id == *identity && declaration.shape.is_borrowed_byte_carrier()
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
        // A declared-domain shared view (`&[u8] in Utf8`) carries the same
        // descriptor; its qualifications constrain contents it cannot change.
        // A mutable view stays unqualified: its stores would have to preserve them.
        if !matches!(
            parameter.access,
            terminal_psi::StructuralAccess::Owned
                | terminal_psi::StructuralAccess::SharedBorrow
                | terminal_psi::StructuralAccess::MutableBorrow
        ) || parameter.multiplicity != StructuralMultiplicity::Unrestricted
            || (!parameter.qualifications.is_empty()
                && parameter.access != terminal_psi::StructuralAccess::SharedBorrow)
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
        .find_map(|row| {
            if let Some(result) = row.kind.get_structural_result()
                && result.place == place
            {
                return Some(result.structural_type);
            }
            match &row.kind {
                LegalizedScalarInstructionKind::EstablishByteSequenceLiteral {
                    destination,
                    structural_type,
                    ..
                } if destination.id == place => Some(structural_type.id),
                _ => None,
            }
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
            && (target.access == parameter.access
                || target.access == terminal_psi::StructuralAccess::SharedBorrow
                    && parameter.access == terminal_psi::StructuralAccess::MutableBorrow)
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
    {
        let actual = view_access(source, target.place)?;
        if target.access != actual
            && !(target.access == terminal_psi::StructuralAccess::SharedBorrow
                && actual == terminal_psi::StructuralAccess::MutableBorrow)
        {
            return None;
        }
    }
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
                || !structural_type.shape.is_borrowed_byte_view()
                || !matches!(destination.kind, StructuralPlaceKind::ByteSequenceLiteral { structural_type: identity, .. } if identity == structural_type.id)
            {
                return None;
            }
        }
        LegalizedScalarInstructionKind::ByteSequenceSubslice { result, .. } => {
            if result.place != target.place
                || result.structural_type != target.structural_type
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
                        && declaration.shape.is_borrowed_byte_view()
                })
            {
                return None;
            }
        }
        LegalizedScalarInstructionKind::ElementViewSubslice { result, .. } => {
            if result.place != target.place
                || result.structural_type != target.structural_type
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
                        && if element_view_source {
                            matches!(declaration.shape, StructuralTypeShape::ElementView { element: declared } if declared == *element)
                        } else {
                            declaration.shape.is_borrowed_byte_view()
                        }
                })
            {
                return None;
            }
        }
        _ => return None,
    }
    Some(())
}

/// An element loan rooted at an established element view: the argument's
/// referent is the projected element, so the producer check names the view's
/// own type as the root — unlike `accepts`, which replays whole-view forwards
/// where the argument type IS the view type. A mutable view establishes an
/// affine result, so multiplicity admits Affine beside Unrestricted.
pub(super) fn accepts_view_loan(
    source: &LegalizedScalarFunction,
    call: OperationId,
    target: &TargetStructuralArgument,
) -> Option<()> {
    let TargetStructuralArgumentSource::EstablishedElementView { psi_operation } = target.source
    else {
        return None;
    };
    let access = view_access(source, target.place)?;
    if target.access != access
        && !(access == terminal_psi::StructuralAccess::MutableBorrow
            && matches!(
                target.access,
                terminal_psi::StructuralAccess::SharedBorrow
                    | terminal_psi::StructuralAccess::WriteOnlyBorrow
            ))
    {
        return None;
    }
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
    let (result, destination) = match &producer.2.kind {
        LegalizedScalarInstructionKind::EstablishElementView {
            result,
            destination,
            ..
        } => (result, Some(*destination)),
        LegalizedScalarInstructionKind::ElementViewSubslice { result, .. } => (result, None),
        _ => return None,
    };
    (destination.is_none_or(|destination| destination == target.place)
        && result.place == target.place
        && result.structural_type == target.root_structural_type
        && matches!(
            result.multiplicity,
            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
        )
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty()
        && signature.structural_places.iter().any(|place| {
            place.id == result.place
                && place.kind
                    == StructuralPlaceKind::OperationResult {
                        producer: psi_operation,
                        structural_type: result.structural_type,
                    }
        })
        && signature.structural_types.iter().any(|declaration| {
            declaration.id == result.structural_type
                && matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
        }))
    .then_some(())
}

/// Completed owned concatenations are readable byte storage, without granting
/// the borrowed descriptor ABI or mutable view authority to their owner.
/// The declared borrowed-view type of an established literal place. A view
/// field store relocates only this descriptor: its verified source is a
/// literal, never a parameter view, subslice or owned concatenation.
pub(super) fn literal_view_type(
    source: &LegalizedScalarFunction,
    place: PlaceId,
) -> Option<semantic_vocabulary::StructuralTypeId> {
    source
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|row| match &row.kind {
            LegalizedScalarInstructionKind::EstablishByteSequenceLiteral {
                destination,
                structural_type,
                ..
            } if destination.id == place && structural_type.shape.is_borrowed_byte_view() => {
                Some(structural_type.id)
            }
            _ => None,
        })
}

pub(super) fn readable_byte_type(
    source: &LegalizedScalarFunction,
    place: PlaceId,
) -> Option<semantic_vocabulary::StructuralTypeId> {
    view_type(source, place)
        .or_else(|| super::byte_concatenation_input::owner_layout_type(source, place))
}

/// A completed descriptor call owns only its captured descriptor payload.
/// Its original backing is the independently replayed nonempty call roster.
pub(super) fn descriptor_result(
    source: &LegalizedScalarFunction,
    place: PlaceId,
) -> Option<(OperationId, &terminal_psi::StructuralOperationResult)> {
    let (operation, result) = super::aggregate_result_input::operation_result(source, place)?;
    let signature = source.structural.as_ref()?;
    if result.multiplicity != StructuralMultiplicity::Unrestricted
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.qualification_establishments.is_empty()
        || !result.claims.is_empty()
        || !terminal_semantics::descriptor_results::is_shared_descriptor(
            signature.structural_types.iter(),
            result.structural_type,
        )
    {
        return None;
    }
    let row = source
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find(|row| row.operation == operation)?;
    let LegalizedScalarInstructionKind::Call(call) = &row.kind else {
        return None;
    };
    if call.descriptor_origins.is_empty()
        || call.structural_result.as_ref() != Some(result)
        || super::aggregate_result_input::call_result(source, call).map(|(result, _)| result)
            != Some(result)
    {
        return None;
    }
    Some((operation, result))
}
