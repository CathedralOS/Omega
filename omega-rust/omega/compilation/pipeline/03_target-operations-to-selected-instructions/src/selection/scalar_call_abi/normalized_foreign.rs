//! Exact per-plan register row and custody replay for normalized foreign calls.
//!
//! The evaluated `BoundaryEntryPlan` carried by the legalized call is producer
//! evidence, not authority: this module re-derives the call signature from the
//! retained argument and result custody, re-validates the plan against it, and
//! instantiates the mechanism template from the plan's complete placements. A
//! substituted locator, provider execution, plan, argument, or home fails
//! closed; target templates do not enumerate source signatures.
use super::{CallSignature, LegalizedScalarFunction, ValueLocation, ValueShape, scalar_shape};
use crate::SelectedInstructionError;
use calling_conventions::validate_boundary_entry_plan;
use legalized_operations::{LegalizedNormalizedForeignCall, LegalizedScalarInstruction};
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterConstraintKey, RegisterInstructionConstraint};
use target_operations::{
    TargetBoundaryResult, TargetStructuralArgumentSource, TargetUnitScalarArgumentSource,
};

// The operand views alone cannot distinguish an import from kernel entry
// (notably a zero-argument call). Mechanism selects the family; the complete
// evaluated plan still selects and independently validates the exact row.
fn entry_family(call: &LegalizedNormalizedForeignCall) -> register_model::RegisterConstraintFamily {
    match call.binding.mechanism {
        target_operations::NormalizedForeignCallMechanism::Import { .. } => {
            register_model::RegisterConstraintFamily::Call
        }
        target_operations::NormalizedForeignCallMechanism::SystemCall(_) => {
            register_model::RegisterConstraintFamily::SystemCall
        }
    }
}

/// The native ordinals the call's parameter-destination callbacks consume as
/// private slots; field destinations mint no plan slot and never join here.
fn private_callback_ordinals(call: &LegalizedNormalizedForeignCall) -> Vec<usize> {
    let mut ordinals: Vec<usize> = call
        .callbacks
        .iter()
        .filter(|callback| {
            matches!(
                callback.destination,
                calling_conventions::NativePlace::Parameter(_)
            )
        })
        .map(|callback| callback.application.native_ordinal as usize)
        .collect();
    ordinals.sort_unstable();
    ordinals
}

/// Scalar rows retain their native ordinals; the remaining ordered slots
/// belong to the structural lane except each retained callback row's private
/// slot, which selection emits as materialization bytes rather than an
/// argument operand.
pub(crate) fn structural_parameter_positions(
    call: &LegalizedNormalizedForeignCall,
) -> impl Iterator<Item = usize> + '_ {
    let private_ordinals = private_callback_ordinals(call);
    (0..call.binding.boundary_entry_plan.call.parameters.len()).filter(move |position| {
        !private_ordinals.contains(position)
            && !call
                .scalar_arguments
                .iter()
                .any(|argument| argument.parameter_index as usize == *position)
    })
}

/// The target supplies one canonical mechanism template. The evaluated plan,
/// not a catalog permutation, supplies its explicit operands and callback uses.
pub(crate) fn call_key(
    call: &LegalizedNormalizedForeignCall,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Option<RegisterConstraintKey> {
    let family = entry_family(call);
    environment
        .selected_keys()
        .call_normalized_foreign
        .into_iter()
        .find(|key| {
            key.family == family
                && environment
                    .constraint(*key)
                    .is_some_and(|row| row.operands.is_empty())
        })
}

pub(crate) fn call_constraint(
    call: &LegalizedNormalizedForeignCall,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Option<RegisterInstructionConstraint> {
    let key = call_key(call, environment)?;
    selected_instructions::calls::constraints::instantiate_foreign_call_constraint(
        call,
        environment.constraint(key)?,
        environment.physical(),
        |register| environment.fixed_register_view(register),
    )
}

/// Replay the complete evaluated custody a normalized foreign call claims.
/// The boundary declaration is not retained at this stage, so every check
/// here is re-derived from the caller's own structural signature, the scalar
/// source types, the admitted locator's target applicability, and the sealed
/// provider/same-stack coordinates the call carries.
#[allow(clippy::too_many_arguments)]
pub(crate) fn validate(
    function: usize,
    source: &LegalizedScalarFunction,
    instruction: &LegalizedScalarInstruction,
    key: RegisterConstraintKey,
    constraint: &RegisterInstructionConstraint,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<(), SelectedInstructionError> {
    let invalid = || SelectedInstructionError::unsupported_shape(function);
    let legalized_operations::LegalizedScalarInstructionKind::NormalizedForeignCall(call) =
        &instruction.kind
    else {
        return Err(invalid());
    };
    let plan = &call.binding.boundary_entry_plan;
    // Target applicability, calling policy, and entry control are exact
    // coordinates; the entry mechanism's sealed target must name this environment.
    if call.binding.native_target() != Some(environment.target())
        || !call.binding.has_valid_entry_mechanism()
        || call.binding.provider_plan_report_identity()
            != call
                .provider_execution
                .provider_plan_report_identity()
                .get()
    {
        return Err(invalid());
    }
    let pointer_size = u16::try_from(environment.target().pointer_size).map_err(|_| invalid())?;
    let pointer_alignment =
        u16::try_from(environment.target().pointer_alignment).map_err(|_| invalid())?;
    // Every retained callback roster row joins exactly one private
    // materialization the registrar plan declares. Each row's native
    // ordinal, pointer-shape placement, destination custody, thunk identity,
    // and owning operation are exact; a materialized call missing a retained
    // row, a retained row without its materialization, a duplicated
    // destination or placement slot, and a substituted roster all fail
    // closed.
    if call.callbacks.len() != plan.call.callback_materializations.len() {
        return Err(invalid());
    }
    let mut private_ordinals = Vec::new();
    for (index, callback) in call.callbacks.iter().enumerate() {
        let ordinal =
            usize::try_from(callback.application.native_ordinal).map_err(|_| invalid())?;
        let placed = plan.call.parameters.get(ordinal) == Some(&callback.application.placement);
        let destination_admitted = match &callback.destination {
            calling_conventions::NativePlace::Parameter(parameter) => {
                *parameter == callback.application.parameter
                    && callback.field_byte_offset.is_none()
                    && placed
            }
            calling_conventions::NativePlace::Field { parameter, .. } => {
                *parameter == callback.application.parameter
                    && callback.field_byte_offset.is_some()
                    && placed
                    && matches!(
                        callback.application.placement.locations.as_slice(),
                        [ValueLocation::Register { value_byte_offset: 0, byte_size, .. }]
                            if *byte_size == u32::from(pointer_size)
                    )
            }
        };
        if callback.terminal_operation != instruction.operation
            || callback.registrar_boundary_entry_plan != *plan
            || callback.registrar_context != call.callbacks[0].registrar_context
            || !callback.callback_function.is_valid()
            || callback.callback_function.callback_thunk_placement_index()
                != Some(callback.placement_index)
            || callback.application.shape != callback.application.placement.shape
            || callback.application.shape
                != ValueShape::integer(u32::from(pointer_size), pointer_alignment)
            || !destination_admitted
            || !plan
                .call
                .callback_materializations
                .iter()
                .any(|materialization| materialization.destination == callback.destination)
            || call.callbacks[..index].iter().any(|prior| {
                prior.placement_index == callback.placement_index
                    || prior.destination == callback.destination
            })
        {
            return Err(invalid());
        }
        if matches!(
            callback.destination,
            calling_conventions::NativePlace::Parameter(_)
        ) {
            private_ordinals.push(ordinal);
        }
    }
    if plan.call.parameters.len()
        != call.scalar_arguments.len() + call.structural_arguments.len() + private_ordinals.len()
    {
        return Err(invalid());
    }
    // Each scalar argument keeps its authored plan position, its source's own
    // scalar type, and the exact placement the plan assigned.
    for (index, argument) in call.scalar_arguments.iter().enumerate() {
        if scalar_shape(argument.source.scalar_type()).is_none()
            || index.checked_sub(1).is_some_and(|previous| {
                call.scalar_arguments[previous].parameter_index >= argument.parameter_index
            })
            || private_ordinals.contains(&(argument.parameter_index as usize))
            || plan.call.parameters.get(argument.parameter_index as usize)
                != Some(&argument.placement)
            || scalar_shape(argument.source.scalar_type()) != Some(argument.placement.shape)
            || match &argument.source {
                TargetUnitScalarArgumentSource::IntegerImmediate {
                    scalar_type, value, ..
                } => semantic_vocabulary::ScalarTerm::integer(*scalar_type, *value).is_err(),
                _ => false,
            }
        {
            return Err(invalid());
        }
    }
    // Storage geometry and foreign classification are different contracts.
    // Reconstruct both from source declarations; an owned indirect placement
    // still transfers bytes, whereas a borrow transports a referent pointer.
    let mut structural_shapes = Vec::new();
    for (index, argument) in structural_parameter_positions(call).zip(&call.structural_arguments) {
        let signature = source.structural.as_ref().ok_or_else(invalid)?;
        let parameter = signature
            .parameters
            .iter()
            .find(|parameter| parameter.semantic.place == argument.place);
        let (root_type, root_access, expected_source) = if let Some(parameter) = parameter {
            (
                parameter.semantic.structural_type,
                parameter.semantic.access,
                TargetStructuralArgumentSource::Placement(parameter.target.placement.clone()),
            )
        } else if let Some((producer, result)) =
            crate::selection::aggregate_result_input::home(source, argument.place)
        {
            (
                result.structural_type,
                terminal_psi::StructuralAccess::Owned,
                TargetStructuralArgumentSource::StructuralHome {
                    psi_operation: producer,
                },
            )
        } else if let Some((block, parameter)) =
            crate::selection::aggregate_result_input::block_home(source, argument.place)
        {
            (
                parameter.structural_type,
                parameter.access,
                TargetStructuralArgumentSource::BlockParameter {
                    block,
                    place: parameter.place,
                },
            )
        } else {
            return Err(invalid());
        };
        let (projected_type, source_byte_offset) =
            crate::structural_inputs::structural_reference_input::project(
                argument.root_structural_type,
                &argument.path,
                &signature.structural_types,
            )
            .ok_or_else(invalid)?;
        let projected_shape = crate::structural_inputs::structural_reference_input::shape(
            projected_type,
            &signature.structural_types,
        )
        .ok_or_else(invalid)?;
        let (storage_shape, abi_shape) = match argument.access {
            terminal_psi::StructuralAccess::Owned => {
                if root_access != terminal_psi::StructuralAccess::Owned || !argument.path.is_empty()
                {
                    return Err(invalid());
                }
                let classified =
                    abstract_operations_to_target_operations::classified_boundary_shape(
                        projected_type,
                        &signature.structural_types,
                        plan.call.policy,
                    )
                    .map_err(|_| invalid())?;
                (projected_shape, classified)
            }
            terminal_psi::StructuralAccess::SharedBorrow
            | terminal_psi::StructuralAccess::MutableBorrow
            | terminal_psi::StructuralAccess::WriteOnlyBorrow => {
                if !matches!(argument.destination.locations.as_slice(),
                    [ValueLocation::Register { value_byte_offset: 0, byte_size, .. }]
                    | [ValueLocation::Stack { value_byte_offset: 0, byte_size, .. }]
                        if *byte_size == u32::from(pointer_size))
                {
                    return Err(invalid());
                }
                (
                    ValueShape::borrowed_reference(
                        projected_shape.byte_size,
                        projected_shape.alignment,
                    ),
                    ValueShape::integer(u32::from(pointer_size), pointer_alignment),
                )
            }
        };
        if argument.path.iter().any(|segment| {
            !matches!(
                segment,
                terminal_psi::StructuralPathSegment::Field(_)
                    | terminal_psi::StructuralPathSegment::FixedIndex(_)
            )
        }) || argument.root_structural_type != root_type
            || argument.structural_type != projected_type
            || argument.source_byte_offset != source_byte_offset
            || argument.shape != storage_shape
            || argument.fixed_array_length.is_some()
            || argument.element_stride.is_some()
            || !argument.indices.is_empty()
            || argument.source != expected_source
            || plan.call.parameters.get(index) != Some(&argument.destination)
            || argument.destination.shape != abi_shape
        {
            return Err(invalid());
        }
        structural_shapes.push((index, abi_shape));
    }
    // The scalar result home is exact value custody: the defining operation,
    // source value, scalar type, and shape must equal the legalized result,
    // and the plan's result must be one register of that exact shape. A unit
    // result admits neither a home nor a plan result row.
    let result_shape = match (&call.result, &plan.call.result, instruction.result) {
        (TargetBoundaryResult::Unit, None, None) => None,
        (TargetBoundaryResult::Scalar(home), Some(placement), Some(result)) => {
            if scalar_shape(result.scalar_type).is_none()
                || home.defining_operation != instruction.operation
                || home.source_value != result.value
                || home.scalar_type != result.scalar_type
                || scalar_shape(result.scalar_type) != Some(home.shape)
                || placement.shape != home.shape
            {
                return Err(invalid());
            }
            let [
                ValueLocation::Register {
                    value_byte_offset: 0,
                    byte_size,
                    ..
                },
            ] = placement.locations.as_slice()
            else {
                return Err(invalid());
            };
            if *byte_size != home.shape.byte_size {
                return Err(invalid());
            }
            Some(home.shape)
        }
        (TargetBoundaryResult::Structural(home), Some(placement), None) => {
            let (producer, result) = home.operation_result().ok_or_else(invalid)?;
            let signature = source.structural.as_ref().ok_or_else(invalid)?;
            let storage = crate::structural_inputs::structural_reference_input::shape(
                result.structural_type,
                &signature.structural_types,
            )
            .ok_or_else(invalid)?;
            let shape = abstract_operations_to_target_operations::classified_boundary_shape(
                result.structural_type,
                &signature.structural_types,
                plan.call.policy,
            )
            .map_err(|_| invalid())?;
            if producer != instruction.operation
                || crate::selection::aggregate_result_input::home(source, result.place)
                    != Some((producer, result))
                || home.layout != target_operations::TargetStructuralHomeLayout::Aggregate(storage)
                || placement.shape != shape
            {
                return Err(invalid());
            }
            Some(shape)
        }
        _ => return Err(invalid()),
    };
    // Re-validate the retained plan against the independently re-derived
    // signature; the canonical evaluation must equal the carried plan exactly.
    let mut parameters = vec![
        ValueShape::integer(u32::from(pointer_size), pointer_alignment);
        plan.call.parameters.len()
    ];
    for argument in &call.scalar_arguments {
        *parameters
            .get_mut(argument.parameter_index as usize)
            .ok_or_else(invalid)? =
            scalar_shape(argument.source.scalar_type()).ok_or_else(invalid)?;
    }
    for (position, shape) in structural_shapes {
        *parameters.get_mut(position).ok_or_else(invalid)? = shape;
    }
    let signature = CallSignature {
        parameters,
        result: result_shape,
    };
    let validated = match call.callbacks.first() {
        Some(callback) => {
            calling_conventions::validate_boundary_entry_plan_with_callback_materializations(
                plan.clone(),
                &signature,
                &callback.registrar_context,
            )
        }
        None => validate_boundary_entry_plan(plan.clone(), &signature),
    }
    .map_err(|_| invalid())?;
    if validated.plan() != plan {
        return Err(invalid());
    }
    // Reconstruct the complete physical row, including all result fragments,
    // from the semantic plan just checked above and the target template.
    if call_key(call, environment) != Some(key)
        || call_constraint(call, environment).as_ref() != Some(constraint)
    {
        return Err(invalid());
    }
    Ok(())
}
