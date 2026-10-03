//! Source and target joins for borrowed arguments in ordinary helper calls.
use crate::LegalizationError;
use abstract_operations::{AbstractOperation, AbstractOperationPlan};
use calling_conventions::{CallPlan, ValueShape};
use optimization_unit::PsiOptimizationFunction;
use target_operations::{TargetOperationPlan, TargetStructuralArgument};
use terminal_psi::{StructuralAccess, StructuralArgument};

mod byte_views;
mod exclusive;
mod local_array;

/// Rejoin one authored argument using its declaration ordinal and the shared call plan.
pub(in crate::legalization) fn argument_at(
    semantic: &StructuralArgument,
    position: usize,
    call_operation: semantic_vocabulary::OperationId,
    caller: &PsiOptimizationFunction,
    called: &PsiOptimizationFunction,
    call: &CallPlan,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    custody: &super::reference_custody::Custody,
) -> Result<TargetStructuralArgument, LegalizationError> {
    let destination_parameter = called
        .structural_parameters
        .get(position)
        .ok_or(LegalizationError::custody())?;
    let parameter_ordinal = called
        .parameters
        .len()
        .checked_add(position)
        .ok_or(LegalizationError::custody())?;
    if let Some(argument) = byte_views::argument(
        semantic,
        call_operation,
        caller,
        destination_parameter,
        call,
        parameter_ordinal,
        native,
        plan,
    )? {
        return Ok(argument);
    }
    // A reference crossing retains the exact carrier that stores its address.
    if semantic
        .path
        .iter()
        .any(|segment| matches!(segment, terminal_psi::StructuralPathSegment::Referent))
    {
        let target_caller = native
            .functions
            .iter()
            .find(|function| function.machine == caller.machine)
            .ok_or(LegalizationError::custody())?;
        return super::reference_custody::referent_argument(
            semantic,
            destination_parameter,
            call.parameters
                .get(parameter_ordinal)
                .ok_or(LegalizationError::custody())?,
            caller,
            target_caller,
            custody,
            &plan.structural_types,
        );
    }
    // An address join lends a projection of its joined referent; its carrier is
    // neither a producer home nor an entrance parameter.
    if super::address_joins::parameter(caller, semantic.place, plan).is_some() {
        return super::address_joins::call_argument(
            semantic,
            destination_parameter,
            call_operation,
            caller,
            call,
            parameter_ordinal,
            native,
            plan,
        );
    }
    if semantic.access == StructuralAccess::Owned
        || plan.structural_types.iter().any(|declaration| {
            declaration.id == destination_parameter.structural_type
                && matches!(
                    declaration.shape,
                    terminal_psi::StructuralTypeShape::Record { .. }
                        | terminal_psi::StructuralTypeShape::Sum { .. }
                        | terminal_psi::StructuralTypeShape::Mixed { .. }
                )
        })
    {
        return super::aggregate_results::call_argument(
            semantic,
            position,
            call_operation,
            caller,
            called,
            call,
            native,
            plan,
            custody,
        );
    }
    // A shared or exclusive borrow of a produced fixed-array
    // result lends the caller's dense leaf storage in place; the aggregate
    // route rejoins the producing home exactly as a projected borrow does.
    if matches!(
        semantic.access,
        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
    ) && plan.structural_types.iter().any(|declaration| {
        declaration.id == destination_parameter.structural_type
            && matches!(
                declaration.shape,
                terminal_psi::StructuralTypeShape::FixedArray { .. }
            )
    }) && super::structural_case::source_result(caller, semantic.place).is_ok()
    {
        return super::aggregate_results::call_argument(
            semantic,
            position,
            call_operation,
            caller,
            called,
            call,
            native,
            plan,
            custody,
        );
    }
    // A scalar projection of a local aggregate rejoins its producing home.
    // Dispatch by the callee's carrier: byte/element views belong to their
    // presentation routes below. Sending every projected local to aggregate
    // reconstruction creates a callback cycle for those non-aggregate views.
    if !semantic.path.is_empty()
        && super::structural_case::source_result(caller, semantic.place).is_ok()
        && super::primitive_locals::scalar(
            &plan.structural_types,
            destination_parameter.structural_type,
        )
        .is_some()
    {
        return super::aggregate_results::call_argument(
            semantic,
            position,
            call_operation,
            caller,
            called,
            call,
            native,
            plan,
            custody,
        );
    }
    if let Some(argument) = owned_byte_loan(
        semantic,
        call_operation,
        caller,
        destination_parameter,
        call,
        parameter_ordinal,
        plan,
    )? {
        return Ok(argument);
    }
    // A dominating local array lends its producing home through a
    // borrowed byte or element view: the same presentation an incoming
    // parameter's fixed-array window takes, rejoined to the producer.
    if let Some(argument) = local_array::argument(
        semantic,
        call_operation,
        caller,
        destination_parameter,
        call,
        parameter_ordinal,
        plan,
    )? {
        return Ok(argument);
    }
    // Whole borrowed descriptors have one source/placement join regardless of
    // whether the caller lends shared or exclusive access. Referent projections
    // continue through their storage-geometry owner below.
    if semantic.path.is_empty()
        && plan.structural_types.iter().any(|declaration| {
            declaration.id == destination_parameter.structural_type
                && (declaration.shape.is_borrowed_byte_view()
                    || matches!(
                        declaration.shape,
                        terminal_psi::StructuralTypeShape::ElementView { .. }
                    ))
        })
    {
        return view_argument(
            semantic,
            call_operation,
            caller,
            destination_parameter,
            call,
            parameter_ordinal,
            native,
            &plan.structural_types,
        );
    }
    if super::primitive_locals::producer(caller, semantic.place).is_some()
        || matches!(
            semantic.access,
            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
        )
        || semantic.access == StructuralAccess::SharedBorrow
            && super::primitive_locals::scalar(
                &plan.structural_types,
                destination_parameter.structural_type,
            )
            .is_some()
    {
        return primitive_argument(
            semantic,
            call_operation,
            caller,
            destination_parameter,
            call,
            parameter_ordinal,
            native,
            plan,
        );
    }
    if caller.structural_parameters.iter().any(|source| {
        terminal_semantics::fixed_byte_array_window(
            plan.structural_types.iter(),
            source,
            semantic,
            destination_parameter,
        )
        .is_some()
    }) {
        return primitive_argument(
            semantic,
            call_operation,
            caller,
            destination_parameter,
            call,
            parameter_ordinal,
            native,
            plan,
        );
    }
    // A shared argument may still project a field or element of its source:
    // nested records, stored byte views, and bounded inline byte fields all
    // reconstruct through the same exclusive-argument custody checks.
    if semantic.access == StructuralAccess::SharedBorrow
        && !semantic.path.is_empty()
        && semantic.path.iter().all(|segment| {
            matches!(
                segment,
                terminal_psi::StructuralPathSegment::Field(_)
                    | terminal_psi::StructuralPathSegment::FixedIndex(_)
            )
        })
    {
        return primitive_argument(
            semantic,
            call_operation,
            caller,
            destination_parameter,
            call,
            parameter_ordinal,
            native,
            plan,
        );
    }
    // A runtime-indexed shared borrow loans one array element: the aggregate
    // route resolves each selector's dominating operand source and the
    // element stride onto the reconstructed argument.
    if semantic.access == StructuralAccess::SharedBorrow
        && semantic.path.iter().any(|segment| {
            matches!(
                segment,
                terminal_psi::StructuralPathSegment::RuntimeIndex { .. }
            )
        })
    {
        return super::aggregate_results::call_argument(
            semantic,
            position,
            call_operation,
            caller,
            called,
            call,
            native,
            plan,
            custody,
        );
    }
    Err(LegalizationError::custody())
}

/// Rejoin one whole descriptor and the authority its current source minted.
fn view_argument(
    semantic: &StructuralArgument,
    call_operation: semantic_vocabulary::OperationId,
    caller: &PsiOptimizationFunction,
    destination_parameter: &terminal_psi::StructuralParameterDeclaration,
    call: &CallPlan,
    parameter_ordinal: usize,
    native: &TargetOperationPlan,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<TargetStructuralArgument, LegalizationError> {
    use target_operations::TargetStructuralArgumentSource;
    let access = optimization_unit_semantics::structural_place_access(caller, semantic.place)
        .ok_or(LegalizationError::custody())?;
    if !matches!(
        semantic.access,
        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
    ) || !semantic.path.is_empty()
        || destination_parameter.access != semantic.access
        || destination_parameter.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
        || !(access == semantic.access
            || access == StructuralAccess::MutableBorrow
                && semantic.access == StructuralAccess::SharedBorrow)
    {
        return Err(LegalizationError::custody());
    }
    // Current-graph admission already replayed the exact argument's primitive
    // byte predicates. Reconstruct the original descriptor and placement here;
    // qualification rosters do not change that physical custody.
    let (structural_type, source) = if let Some((producer, structural_type)) =
        established_view(caller, call_operation, semantic.place)
    {
        let shape = &structural_types
            .iter()
            .find(|declaration| declaration.id == structural_type)
            .ok_or(LegalizationError::custody())?
            .shape;
        let source = if matches!(shape, terminal_psi::StructuralTypeShape::ElementView { .. }) {
            TargetStructuralArgumentSource::EstablishedElementView {
                psi_operation: producer,
            }
        } else if shape.is_borrowed_byte_view() {
            TargetStructuralArgumentSource::EstablishedByteView {
                psi_operation: producer,
            }
        } else {
            return Err(LegalizationError::custody());
        };
        (structural_type, source)
    } else if let Some((block, parameter)) = caller.blocks.iter().find_map(|block| {
        block
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == semantic.place)
            .map(|parameter| (block, parameter))
    }) {
        if block.id == caller.entry
            || parameter.access != access
            || parameter.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
            || parameter.is_self
            || !parameter.projected_qualifications.is_empty()
        {
            return Err(LegalizationError::custody());
        }
        (
            parameter.structural_type,
            TargetStructuralArgumentSource::BlockParameter {
                block: block.id,
                place: parameter.place,
            },
        )
    } else {
        let source = caller
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == semantic.place)
            .ok_or(LegalizationError::custody())?;
        let target_caller = native
            .functions
            .iter()
            .find(|function| function.machine == caller.machine)
            .ok_or(LegalizationError::custody())?;
        let parameters =
            super::structural_parameters(target_caller).ok_or(LegalizationError::custody())?;
        let parameter = parameters
            .iter()
            .find(|parameter| parameter.place == semantic.place)
            .ok_or(LegalizationError::custody())?;
        if semantic.place != source.place
            || source.access != access
            || source.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
            || !source.projected_qualifications.is_empty()
            || parameter.place != source.place
            || parameter.structural_type != source.structural_type
            || parameter.access != source.access
            || parameter.multiplicity != source.multiplicity
            || parameter.shape != ValueShape::borrowed_reference(16, 8)
            || !parameter.projected_qualifications.is_empty()
        {
            return Err(LegalizationError::custody());
        }
        (source.structural_type, parameter.placement.clone().into())
    };
    if structural_type != destination_parameter.structural_type
        || call
            .parameters
            .get(parameter_ordinal)
            .is_none_or(|parameter| parameter.shape != ValueShape::borrowed_reference(16, 8))
    {
        return Err(LegalizationError::custody());
    }
    Ok(TargetStructuralArgument {
        place: semantic.place,
        access: semantic.access,
        path: Vec::new(),
        root_structural_type: structural_type,
        structural_type,
        shape: ValueShape::borrowed_reference(16, 8),
        source_byte_offset: 0,
        fixed_array_length: None,
        element_stride: None,
        indices: Vec::new(),
        source,
        destination: call
            .parameters
            .get(parameter_ordinal)
            .ok_or(LegalizationError::custody())?
            .clone(),
    })
}

/// A bounded owner formal borrows the complete owner layout, not a descriptor.
/// Shared view presentation is reconstructed by the byte-view owner above.
fn owned_byte_loan(
    semantic: &StructuralArgument,
    call_operation: semantic_vocabulary::OperationId,
    caller: &PsiOptimizationFunction,
    destination_parameter: &terminal_psi::StructuralParameterDeclaration,
    call: &CallPlan,
    parameter_ordinal: usize,
    plan: &AbstractOperationPlan,
) -> Result<Option<TargetStructuralArgument>, LegalizationError> {
    if !semantic.path.is_empty()
        || semantic.access != StructuralAccess::SharedBorrow
        || !abstract_operations::control_flow::owner_loans::is_shared_bounded_owner_loan_in(
            destination_parameter,
            &plan.structural_types,
        )
    {
        return Ok(None);
    }
    let site = |matches: &dyn Fn(&AbstractOperation) -> bool| {
        caller.blocks.iter().find_map(|block| {
            block
                .nodes
                .iter()
                .position(|node| matches(&node.operation))
                .map(|position| (block.id, position))
        })
    };
    let Some((producer, structural_type)) =
        super::byte_views::owned_byte_result(caller, semantic.place, &plan.structural_types)
    else {
        return Ok(None);
    };
    let (exact_producer, result) = super::structural_case::source_result(caller, semantic.place)?;
    let before = site(&|operation| {
        matches!(operation, AbstractOperation::EstablishByteSequenceConcatenation {
            psi_operation, ..
        } if *psi_operation == producer)
    })
    .ok_or(LegalizationError::custody())?;
    let after = site(&|operation| {
        matches!(operation,
            AbstractOperation::CallStructural { psi_operation, structural_arguments, .. }
            | AbstractOperation::BoundaryCall { psi_operation, structural_arguments, .. }
            if *psi_operation == call_operation && structural_arguments.contains(semantic))
    })
    .ok_or(LegalizationError::custody())?;
    if producer != exact_producer
        || structural_type != result.structural_type
        || result.structural_type != destination_parameter.structural_type
        || !plan.structural_types.iter().any(|declaration| {
            declaration.id == result.structural_type
                && matches!(
                    declaration.shape,
                    terminal_psi::StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                    )
                )
        })
        || result.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
        || !result.projected_qualifications.is_empty()
        || !result.qualification_establishments.is_empty()
        || !result.claims.is_empty()
        || (before.0 == after.0 && before.1 >= after.1)
        || (before.0 != after.0
            && !super::target::control_flow::sources::dominates(caller, before.0, after.0))
    {
        return Err(LegalizationError::custody());
    }
    let referent = crate::structural_inputs::structural_reference_input::shape(
        destination_parameter.structural_type,
        &plan.structural_types,
    )
    .ok_or(LegalizationError::custody())?;
    let shape = ValueShape::borrowed_reference(referent.byte_size, referent.alignment);
    let destination = call
        .parameters
        .get(parameter_ordinal)
        .ok_or(LegalizationError::custody())?
        .clone();
    if destination.shape != shape {
        return Err(LegalizationError::custody());
    }
    Ok(Some(TargetStructuralArgument {
        place: semantic.place,
        access: semantic.access,
        path: Vec::new(),
        root_structural_type: result.structural_type,
        structural_type: destination_parameter.structural_type,
        shape,
        source_byte_offset: 0,
        fixed_array_length: None,
        element_stride: None,
        indices: Vec::new(),
        source: target_operations::TargetStructuralArgumentSource::StructuralHome {
            psi_operation: producer,
        },
        destination,
    }))
}

/// Reconstruct a primitive borrow at its actual ordered call ABI position.
pub(super) fn primitive_argument(
    semantic: &StructuralArgument,
    call_operation: semantic_vocabulary::OperationId,
    caller: &PsiOptimizationFunction,
    destination_parameter: &terminal_psi::StructuralParameterDeclaration,
    call: &CallPlan,
    parameter_ordinal: usize,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
) -> Result<TargetStructuralArgument, LegalizationError> {
    use target_operations::TargetStructuralArgumentSource;
    if let Some((producer, result, value)) =
        super::primitive_locals::producer(caller, semantic.place)
    {
        if !super::primitive_locals::valid_result(caller, producer, result)
            || !semantic.path.is_empty()
            || semantic.access == StructuralAccess::Owned
            || destination_parameter.access != semantic.access
            || destination_parameter.structural_type != result.structural_type
            || destination_parameter.multiplicity
                != terminal_psi::StructuralMultiplicity::Unrestricted
            || !destination_parameter.qualifications.is_empty()
            || !destination_parameter.projected_qualifications.is_empty()
            || super::primitive_locals::scalar(&plan.structural_types, result.structural_type)
                != Some(value.scalar_type)
        {
            return Err(LegalizationError::custody());
        }
        let referent = super::scalar_shape(value.scalar_type).ok_or(LegalizationError::custody())?;
        let shape = ValueShape::borrowed_reference(referent.byte_size, referent.alignment);
        let destination = call
            .parameters
            .get(parameter_ordinal)
            .ok_or(LegalizationError::custody())?
            .clone();
        if destination.shape != shape {
            return Err(LegalizationError::custody());
        }
        return Ok(TargetStructuralArgument {
            place: semantic.place,
            access: semantic.access,
            path: Vec::new(),
            root_structural_type: result.structural_type,
            structural_type: result.structural_type,
            shape,
            source_byte_offset: 0,
            fixed_array_length: None,
            element_stride: None,
            indices: Vec::new(),
            source: TargetStructuralArgumentSource::EstablishedPrimitiveLocal {
                psi_operation: producer,
            },
            destination,
        });
    }
    exclusive::argument(
        semantic,
        call_operation,
        caller,
        destination_parameter,
        call,
        parameter_ordinal,
        native,
        plan,
    )
}

/// Whole-unit custody has already checked exact CFG dominance and producer
/// metadata. Rejoin that producer, not a declaration or flattened predecessor.
fn established_view(
    caller: &PsiOptimizationFunction,
    call: semantic_vocabulary::OperationId,
    place: semantic_vocabulary::PlaceId,
) -> Option<(
    semantic_vocabulary::OperationId,
    semantic_vocabulary::StructuralTypeId,
)> {
    if let Some(producer) = super::literals::producer(caller, call, place) {
        return Some(producer);
    }
    if !matches!(
        optimization_unit_semantics::structural_place_access(caller, place),
        Some(StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow)
    ) || !caller.blocks.iter().flat_map(|block| &block.nodes).any(|node| {
        matches!(&node.operation, AbstractOperation::CallStructural { psi_operation, structural_arguments, .. }
            | AbstractOperation::BoundaryCall { psi_operation, structural_arguments, .. }
            if *psi_operation == call && structural_arguments.iter().any(|argument| argument.place == place))
    }) {
        return None;
    }
    caller
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| {
            let (producer, result) = match &node.operation {
                AbstractOperation::ByteSequenceSubslice {
                    psi_operation,
                    result,
                    ..
                }
                | AbstractOperation::ElementViewSubslice {
                    psi_operation,
                    result,
                    ..
                } => (*psi_operation, result),
                AbstractOperation::EstablishElementView {
                    psi_operation,
                    result,
                    destination,
                    ..
                } if *destination == place => (*psi_operation, result),
                _ => return None,
            };
            (result.place == place
                && result.qualifications.is_empty()
                && result.projected_qualifications.is_empty()
                && result.claims.is_empty())
            .then_some((producer, result.structural_type))
        })
}
