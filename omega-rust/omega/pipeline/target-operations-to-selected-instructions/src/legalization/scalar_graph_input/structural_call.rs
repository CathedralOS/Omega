//! Source and target joins for borrowed arguments in ordinary helper calls.
use crate::LegalizationError;
use abstract_operations::{AbstractFunction, AbstractOperation, AbstractOperationPlan};
use calling_conventions::{CallPlan, ValueShape};
use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit};
use semantic_vocabulary::MachineId;
use target_operations::{
    TargetFunction, TargetOperation, TargetOperationPlan, TargetStructuralArgument,
};
use terminal_psi::{StructuralAccess, StructuralArgument};

mod exclusive;

pub(in crate::legalization) fn validate_argument(
    argument: &StructuralArgument,
    target_argument: &TargetStructuralArgument,
    call_operation: semantic_vocabulary::OperationId,
    caller: &PsiOptimizationFunction,
    callee: MachineId,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<CallPlan, LegalizationError> {
    if self::argument(argument, call_operation, caller, callee, native, plan, unit)?
        != *target_argument
    {
        return Err(LegalizationError::SourceCustodyMismatch);
    }
    super::callee_plan(callee, native, plan, unit)
}

/// Reconstruct source storage separately from the callee's incoming pointer ABI.
pub(in crate::legalization) fn argument(
    semantic: &StructuralArgument,
    call_operation: semantic_vocabulary::OperationId,
    caller: &PsiOptimizationFunction,
    callee: MachineId,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<TargetStructuralArgument, LegalizationError> {
    use target_operations::TargetStructuralArgumentSource;
    let invalid = LegalizationError::SourceCustodyMismatch;
    let call = super::callee_plan(callee, native, plan, unit)?;
    let called = unit
        .functions
        .iter()
        .find(|function| function.machine == callee)
        .ok_or(invalid.clone())?;
    let [destination_parameter] = called.structural_parameters.as_slice() else {
        return Err(invalid);
    };
    if semantic.access == StructuralAccess::Owned {
        return super::aggregate_results::call_argument(
            semantic,
            0,
            call_operation,
            caller,
            called,
            &call,
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
            &call,
            called.parameters.len(),
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
            .get(called.parameters.len())
            .ok_or(invalid)?
            .clone(),
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

pub(super) fn validate_target(
    target: &TargetFunction,
    abstracted: &AbstractFunction,
    optimized: &PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let TargetOperation::ReturnStructuralScalarCall {
        psi_edge,
        psi_operation,
        source_value,
        scalar_type,
        callee,
        structural_types,
        call_plan,
        structural_parameters,
        arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
    } = &target.operation
    else {
        return Err(invalid);
    };
    let [
        AbstractOperation::CallStructuralScalar {
            psi_operation: operation,
            result,
            callee: source_callee,
            arguments: scalars,
            structural_arguments,
            claim_transfers: claims,
            requirement_obligations: requirements,
            crash_continuations: crashes,
        },
        AbstractOperation::Return {
            psi_edge: edge,
            result: returned_result,
            value,
            scalar_type: returned_type,
            cleanup_actions,
        },
    ] = abstracted.operations.as_slice()
    else {
        return Err(invalid);
    };
    let ([argument], [target_argument]) = (structural_arguments.as_slice(), arguments.as_slice())
    else {
        return Err(invalid);
    };
    let abi = target
        .mixed_structural_scalar_abi
        .as_ref()
        .ok_or(invalid.clone())?;
    if psi_edge != edge
        || psi_operation != operation
        || source_value != value
        || result.value != *value
        || result.scalar_type != *scalar_type
        || returned_type != scalar_type
        || abstracted.result.scalar().map(|result| result.value) != Some(*returned_result)
        || callee != source_callee
        || structural_types != &plan.structural_types
        || call_plan != &abi.call_plan
        || structural_parameters != &abi.structural_parameters
        || !scalars.is_empty()
        || !cleanup_actions.is_empty()
        || claim_transfers != claims
        || !claims.is_empty()
        || requirement_obligations != requirements
        || !requirements.is_empty()
        || crash_continuations != crashes
        || !crashes.is_empty()
    {
        return Err(invalid);
    }
    validate_argument(
        argument,
        target_argument,
        *psi_operation,
        optimized,
        *callee,
        native,
        plan,
        unit,
    )?;
    Ok(())
}
