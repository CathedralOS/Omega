//! Source and target joins for borrowed arguments in ordinary helper calls.
use crate::LegalizationError;
use abstract_operations::{AbstractOperation, AbstractOperationPlan};
use calling_conventions::{CallPlan, ValueShape};
use optimization_unit::PsiOptimizationFunction;
use target_operations::{TargetOperationPlan, TargetStructuralArgument};
use terminal_psi::{StructuralAccess, StructuralArgument};

mod exclusive;

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
) -> Result<TargetStructuralArgument, LegalizationError> {
    use target_operations::TargetStructuralArgumentSource;
    let invalid = LegalizationError::SourceCustodyMismatch;
    let destination_parameter = called
        .structural_parameters
        .get(position)
        .ok_or(invalid.clone())?;
    let parameter_ordinal = called
        .parameters
        .len()
        .checked_add(position)
        .ok_or(invalid.clone())?;
    if semantic.access == StructuralAccess::Owned {
        return super::aggregate_results::call_argument(
            semantic,
            position,
            call_operation,
            caller,
            called,
            call,
            native,
            plan,
        );
    }
    if semantic.access == StructuralAccess::SharedBorrow
        && crate::structural_reference_input::plain_record_shape(
            destination_parameter.structural_type,
            &plan.structural_types,
        )
        .is_some()
    {
        return record_argument(
            semantic,
            caller,
            destination_parameter,
            call,
            parameter_ordinal,
            native,
            plan,
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
            caller,
            destination_parameter,
            call,
            parameter_ordinal,
            native,
            plan,
        );
    }
    if semantic.access != StructuralAccess::SharedBorrow || !semantic.path.is_empty() {
        return Err(invalid);
    }
    let (structural_type, source) = if let Some((producer, structural_type)) =
        established_view(caller, call_operation, semantic.place)
    {
        (
            structural_type,
            TargetStructuralArgumentSource::EstablishedByteView {
                psi_operation: producer,
            },
        )
    } else if let Some((block, parameter)) = caller.blocks.iter().find_map(|block| {
        block
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == semantic.place)
            .map(|parameter| (block, parameter))
    }) {
        if block.id == caller.entry
            || parameter.access != StructuralAccess::SharedBorrow
            || parameter.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
            || parameter.is_self
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
        {
            return Err(invalid);
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
            .ok_or(invalid.clone())?;
        let target_caller = native
            .functions
            .iter()
            .find(|function| function.machine == caller.machine)
            .ok_or(invalid.clone())?;
        let parameters = super::structural_parameters(target_caller).ok_or(invalid.clone())?;
        let parameter = parameters
            .iter()
            .find(|parameter| parameter.place == semantic.place)
            .ok_or(invalid.clone())?;
        if semantic.place != source.place
            || source.access != StructuralAccess::SharedBorrow
            || source.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
            || !source.qualifications.is_empty()
            || !source.projected_qualifications.is_empty()
            || parameter.place != source.place
            || parameter.structural_type != source.structural_type
            || parameter.access != source.access
            || parameter.multiplicity != source.multiplicity
            || parameter.shape != ValueShape::borrowed_reference(16, 8)
            || !parameter.projected_qualifications.is_empty()
        {
            return Err(invalid);
        }
        (source.structural_type, parameter.placement.clone().into())
    };
    if structural_type != destination_parameter.structural_type {
        return Err(invalid);
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
        source,
        destination: call
            .parameters
            .get(parameter_ordinal)
            .ok_or(invalid)?
            .clone(),
    })
}

/// Bind a shared pointer to the exact established home or incoming referent.
fn record_argument(
    semantic: &StructuralArgument,
    caller: &PsiOptimizationFunction,
    destination: &terminal_psi::StructuralParameterDeclaration,
    call: &CallPlan,
    parameter_ordinal: usize,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
) -> Result<TargetStructuralArgument, LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    if semantic.access != StructuralAccess::SharedBorrow
        || destination.access != StructuralAccess::SharedBorrow
        || destination.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
        || !destination.qualifications.is_empty()
        || !destination.projected_qualifications.is_empty()
    {
        return Err(invalid);
    }
    let (structural_type, source) = if let Some(parameter) = caller
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == semantic.place)
    {
        if parameter.access != StructuralAccess::SharedBorrow
            || parameter.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
        {
            return Err(invalid);
        }
        let target = native
            .functions
            .iter()
            .find(|function| function.machine == caller.machine)
            .and_then(super::structural_parameters)
            .and_then(|parameters| {
                parameters
                    .iter()
                    .find(|target| target.place == semantic.place)
            })
            .ok_or(invalid.clone())?;
        (parameter.structural_type, target.placement.clone().into())
    } else {
        let home = super::aggregate_results::result_home(caller, semantic.place, plan)?;
        let (producer, result) = home.operation_result().ok_or(invalid.clone())?;
        if result.multiplicity == terminal_psi::StructuralMultiplicity::Linear
            || !result.claims.is_empty()
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !caller
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .any(|node| {
                    matches!(&node.operation,
                AbstractOperation::EstablishScalarRecord { psi_operation, result: retained, .. }
                | AbstractOperation::CallStructural { psi_operation, result: retained, .. }
                    if *psi_operation == producer && retained == result)
                })
        {
            return Err(invalid);
        }
        (
            result.structural_type,
            target_operations::TargetStructuralArgumentSource::StructuralHome {
                psi_operation: producer,
            },
        )
    };
    crate::structural_reference_input::plain_record_shape(structural_type, &plan.structural_types)
        .ok_or(invalid.clone())?;
    let (referent_type, offset) = crate::structural_reference_input::project(
        structural_type,
        &semantic.path,
        &plan.structural_types,
    )
    .ok_or(invalid.clone())?;
    let referent = crate::structural_reference_input::plain_record_shape(
        referent_type,
        &plan.structural_types,
    )
    .ok_or(invalid.clone())?;
    let shape = ValueShape::borrowed_reference(referent.byte_size, referent.alignment);
    let placement = call
        .parameters
        .get(parameter_ordinal)
        .ok_or(invalid.clone())?;
    if referent_type != destination.structural_type || placement.shape != shape {
        return Err(invalid);
    }
    Ok(TargetStructuralArgument {
        place: semantic.place,
        access: semantic.access,
        path: semantic.path.clone(),
        root_structural_type: structural_type,
        structural_type: referent_type,
        shape,
        source_byte_offset: offset,
        fixed_array_length: None,
        element_stride: None,
        source,
        destination: placement.clone(),
    })
}

/// Reconstruct a primitive borrow at its actual ordered call ABI position.
pub(super) fn primitive_argument(
    semantic: &StructuralArgument,
    caller: &PsiOptimizationFunction,
    destination_parameter: &terminal_psi::StructuralParameterDeclaration,
    call: &CallPlan,
    parameter_ordinal: usize,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
) -> Result<TargetStructuralArgument, LegalizationError> {
    use target_operations::TargetStructuralArgumentSource;
    let invalid = LegalizationError::SourceCustodyMismatch;
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
            return Err(invalid);
        }
        let referent = super::scalar_shape(value.scalar_type).ok_or(invalid.clone())?;
        let shape = ValueShape::borrowed_reference(referent.byte_size, referent.alignment);
        let destination = call
            .parameters
            .get(parameter_ordinal)
            .ok_or(invalid.clone())?
            .clone();
        if destination.shape != shape {
            return Err(invalid);
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
            source: TargetStructuralArgumentSource::EstablishedPrimitiveLocal {
                psi_operation: producer,
            },
            destination,
        });
    }
    exclusive::argument(
        semantic,
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
        return super::literals::roster(caller).then_some(producer);
    }
    if !caller.blocks.iter().flat_map(|block| &block.nodes).any(|node| {
        matches!(&node.operation, AbstractOperation::CallStructuralScalar { psi_operation, structural_arguments, .. }
            | AbstractOperation::CallUnit { psi_operation, structural_arguments, .. }
            if *psi_operation == call && structural_arguments.iter().any(|argument| argument.place == place))
    }) {
        return None;
    }
    caller
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| match &node.operation {
            AbstractOperation::ByteSequenceSubslice {
                psi_operation,
                result,
                ..
            } if result.place == place
                && result.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                && result.qualifications.is_empty()
                && result.projected_qualifications.is_empty()
                && result.claims.is_empty() =>
            {
                Some((*psi_operation, result.structural_type))
            }
            _ => None,
        })
}
