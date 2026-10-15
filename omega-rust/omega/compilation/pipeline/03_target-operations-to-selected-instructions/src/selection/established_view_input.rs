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
        || source.instructions()
            .any(|row| match &row.kind {
                LegalizedScalarInstructionKind::Call(call) => call.arguments.iter().any(|argument|
                    matches!(argument, LegalizedScalarArgument::Structural { semantic, .. }
                        if semantic.place == place)),
                LegalizedScalarInstructionKind::NormalizedForeignCall(call) => call
                    .structural_arguments.iter().any(|argument| argument.place == place),
                // A selected descendant needs its descriptor in the original
                // home; only whole observations can use retained SSA coordinates.
                LegalizedScalarInstructionKind::ByteSequenceLength { source, .. }
                | LegalizedScalarInstructionKind::ElementViewLength { source, .. }
                | LegalizedScalarInstructionKind::ByteSequenceSubslice { source, .. }
                | LegalizedScalarInstructionKind::ElementViewSubslice { source, .. }
                | LegalizedScalarInstructionKind::StructuralScalarFieldRead { source, .. }
                | LegalizedScalarInstructionKind::StructuralByteSequenceFieldLength { source, .. }
                | LegalizedScalarInstructionKind::StructuralByteSequenceFieldRead { source, .. } =>
                    source.place == place && !source.path.is_empty(),
                LegalizedScalarInstructionKind::PrimitiveScalarRead { source, path, .. } =>
                    *source == place && !path.is_empty(),
                // Scalar observations can use the retained view coordinates.
                // Capturing or copying its value needs the address/length
                // descriptor itself, independent of how the view was formed.
                LegalizedScalarInstructionKind::EstablishRecord { fields, .. }
                | LegalizedScalarInstructionKind::EstablishCase { fields, .. } => fields.iter().any(|field|
                    matches!(&field.value, terminal_psi::RecordFieldValue::Structural(argument)
                        if argument.place == place)),
                LegalizedScalarInstructionKind::EstablishStructuralArray { elements, .. } =>
                    elements.iter().any(|argument| argument.place == place),
                LegalizedScalarInstructionKind::StructuralByteViewFieldStore { source, .. }
                | LegalizedScalarInstructionKind::StructuralLeafCopy { source, .. } => *source == place,
                LegalizedScalarInstructionKind::StoreStructuralField { value, .. } => *value == place,
                _ => false,
            })
}

pub(super) fn transferred(source: &LegalizedScalarFunction, place: PlaceId) -> bool {
    source.blocks.iter().any(|block| {
        let successors = match &block.terminator {
            legalized_operations::LegalizedScalarTerminator::Crash { .. }
            | legalized_operations::LegalizedScalarTerminator::ExternalCompletion { .. }
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
                .any(|binding| binding.semantic.argument.place == place)
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
        .chain(source.block_structural_parameters())
        .find(|parameter| parameter.place == place);
    if let Some(parameter) = parameter {
        // Qualification does not change descriptor geometry. Mutable stores
        // retain their separately checked membership-preservation obligations.
        if !matches!(
            parameter.access,
            terminal_psi::StructuralAccess::Owned
                | terminal_psi::StructuralAccess::SharedBorrow
                | terminal_psi::StructuralAccess::MutableBorrow
        ) || (parameter.multiplicity != StructuralMultiplicity::Unrestricted
            && !terminal_semantics::view_descriptor_parameter(
                parameter,
                signature.structural_types.iter(),
            ))
            || !terminal_semantics::view_descriptor_projected_qualifications(
                signature.structural_types.iter(),
                parameter.structural_type,
                &parameter.projected_qualifications,
            )
        {
            return None;
        }
        return Some(parameter.structural_type);
    }
    source.instructions().find_map(|row| {
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
        let owner = source.block(block)?;
        let parameter = owner.structural_parameter(place)?;
        let signature = source.structural.as_ref()?;
        // The arrival is the loan's root: a whole forward selects it, and a
        // projected element loan's geometry was rejoined by its caller.
        return (place == target.place
            && parameter.structural_type == target.root_structural_type
            && matches!(
                parameter.access,
                terminal_psi::StructuralAccess::SharedBorrow
                    | terminal_psi::StructuralAccess::MutableBorrow
            )
            && (target.access == parameter.access
                || target.access == terminal_psi::StructuralAccess::SharedBorrow
                    && parameter.access == terminal_psi::StructuralAccess::MutableBorrow)
            && terminal_semantics::view_descriptor_parameter(
                parameter,
                signature.structural_types.iter(),
            )
            && terminal_semantics::view_descriptor_projected_qualifications(
                signature.structural_types.iter(),
                parameter.structural_type,
                &parameter.projected_qualifications,
            )
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
    let producer = producer_strictly_before_use(source, psi_operation, call)?;
    if let Some(result) = producer.2.kind.get_structural_result()
        && !terminal_semantics::borrowed_referent_matches(
            signature.structural_types.iter(),
            result.structural_type,
            target.structural_type,
        )
    {
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
                || !result.qualifications.is_empty()
                || !terminal_semantics::view_descriptor_projected_qualifications(signature.structural_types.iter(), result.structural_type, &result.projected_qualifications)
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
                || !result.qualifications.is_empty()
                || !terminal_semantics::view_descriptor_projected_qualifications(signature.structural_types.iter(), result.structural_type, &result.projected_qualifications)
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
                || *destination != target.place
                || !result.qualifications.is_empty()
                || !terminal_semantics::view_descriptor_projected_qualifications(signature.structural_types.iter(), result.structural_type, &result.projected_qualifications)
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
                            matches!(declaration.shape, StructuralTypeShape::ElementView { element: declared, .. } if declared == *element)
                        } else {
                            declaration.shape.is_borrowed_byte_carrier()
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
    let producer = producer_strictly_before_use(source, psi_operation, call)?;
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
            terminal_psi::nonlinear_multiplicity_pair_pattern!()
        )
        && result.qualifications.is_empty()
        && terminal_semantics::view_descriptor_projected_qualifications(
            signature.structural_types.iter(),
            result.structural_type,
            &result.projected_qualifications,
        )
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
pub(super) fn readable_byte_type(
    source: &LegalizedScalarFunction,
    place: PlaceId,
) -> Option<semantic_vocabulary::StructuralTypeId> {
    view_type(source, place)
        .or_else(|| super::byte_concatenation_input::owner_layout_type(source, place))
}

/// Exact completed descriptor geometry, independent of the operation which
/// filled the home. Access and backing lifetime remain separate source-chain
/// judgments; finding this result does not establish either permission.
pub(super) fn completed_descriptor(
    source: &LegalizedScalarFunction,
    place: PlaceId,
) -> Option<(OperationId, &terminal_psi::StructuralOperationResult)> {
    let (operation, result) = super::aggregate_result_input::operation_result(source, place)?;
    let signature = source.structural.as_ref()?;
    (matches!(
        result.multiplicity,
        terminal_psi::nonlinear_multiplicity_pair_pattern!()
    ) && terminal_semantics::view_descriptor_projected_qualifications(
        signature.structural_types.iter(),
        result.structural_type,
        &result.projected_qualifications,
    ) && result.qualification_establishments.is_empty()
        && result.claims.is_empty()
        && terminal_semantics::descriptor_results::is_descriptor(
            signature.structural_types.iter(),
            result.structural_type,
        ))
    .then_some((operation, result))
}

/// A declared descriptor local's home retains the permission supplied by its
/// independently checked whole-slot stores.
pub(in crate::selection) fn descriptor_home(
    source: &LegalizedScalarFunction,
    place: PlaceId,
) -> Option<(OperationId, &terminal_psi::StructuralOperationResult)> {
    let (operation, result) = completed_descriptor(source, place)?;
    let row = source
        .instructions()
        .find(|row| row.operation == operation)?;
    matches!(
        &row.kind,
        LegalizedScalarInstructionKind::EstablishStructuralLocal { result: established, .. }
            if established == result
    )
    .then_some((operation, result))
}

/// A completed descriptor call owns only its captured descriptor payload.
/// Its original backing is the independently replayed nonempty call roster.
pub(super) fn descriptor_result(
    source: &LegalizedScalarFunction,
    place: PlaceId,
) -> Option<(OperationId, &terminal_psi::StructuralOperationResult)> {
    let (operation, result) = completed_descriptor(source, place)?;
    let row = source
        .instructions()
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

/// The `psi_operation` producer row ordered strictly before the `call` row's
/// instruction position, or `None` when either is absent, the producer
/// yields a register, or it does not precede the use in its block.
fn producer_strictly_before_use<'source>(
    source: &'source LegalizedScalarFunction,
    psi_operation: OperationId,
    call: OperationId,
) -> Option<(
    semantic_vocabulary::BlockId,
    usize,
    &'source legalized_operations::LegalizedScalarInstruction,
)> {
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
    Some(producer)
}
