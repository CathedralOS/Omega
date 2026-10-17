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
use legalized_operations::{LegalizedNormalizedForeignCall, LegalizedValueDefinition};
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterConstraintKey, RegisterInstructionConstraint};
use semantic_vocabulary::BoundaryInvocationSite;
use target_operations::{
    TargetBoundaryResult, TargetStructuralArgumentSource, TargetUnitScalarArgumentSource,
};

// The operand views alone cannot distinguish an import from kernel entry
// (notably a zero-argument call). Mechanism selects the family; the complete
// evaluated plan still selects and independently validates the exact row.
fn entry_family(call: &LegalizedNormalizedForeignCall) -> register_model::RegisterConstraintFamily {
    match call.binding.mechanism {
        target_operations::NormalizedForeignCallMechanism::Import { .. }
        | target_operations::NormalizedForeignCallMechanism::Table { .. } => {
            register_model::RegisterConstraintFamily::Call
        }
        target_operations::NormalizedForeignCallMechanism::SystemCall(_) => {
            register_model::RegisterConstraintFamily::SystemCall
        }
    }
}

/// The transport ordinals the call's parameter-destination callbacks consume as
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
        .map(|callback| {
            callback.application.native_ordinal as usize + call.binding.dispatch_operand_count()
        })
        .collect();
    ordinals.sort_unstable();
    ordinals
}

/// Scalar rows retain their source transport ordinals; the remaining ordered slots
/// belong to the structural lane except each retained callback row's private
/// slot, which selection emits as materialization bytes rather than an
/// argument operand.
pub(crate) fn structural_parameter_positions(
    call: &LegalizedNormalizedForeignCall,
) -> impl Iterator<Item = usize> + '_ {
    let private_ordinals = private_callback_ordinals(call);
    (0..call.binding.operand_placements().len()).filter(move |position| {
        !private_ordinals.contains(position)
            && !call
                .scalar_arguments
                .iter()
                .any(|argument| argument.parameter_index as usize == *position)
    })
}

/// Borrowed block arrivals retain either a referent address or their actual
/// descriptor. Neither is an aggregate home merely because it has a block
/// coordinate; the ordinary marshaler selects presentation from that root.
fn borrowed_block_argument_root<'source>(
    source: &'source LegalizedScalarFunction,
    operation: semantic_vocabulary::OperationId,
    argument: &target_operations::TargetStructuralArgument,
) -> Option<(
    semantic_vocabulary::BlockId,
    &'source terminal_psi::StructuralParameterDeclaration,
)> {
    if let Some(join) = crate::selection::address_join_input::join(source, argument.place) {
        return Some(join);
    }
    let parameter = crate::selection::arrival_parameter_input::parameter(source, argument.place)?;
    if crate::selection::established_view_input::view_type(source, argument.place)
        != Some(parameter.structural_type)
        && crate::selection::established_view_input::element_view_type(source, argument.place)
            != Some(parameter.structural_type)
    {
        return None;
    }
    crate::selection::established_view_input::accepts(source, operation, argument)?;
    let block = source.blocks.iter().find(|block| {
        block
            .structural_parameters
            .iter()
            .any(|actual| actual == parameter)
    })?;
    Some((block.id, parameter))
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
    call: &LegalizedNormalizedForeignCall,
    invocation_site: BoundaryInvocationSite,
    result: Option<LegalizedValueDefinition>,
    key: RegisterConstraintKey,
    constraint: &RegisterInstructionConstraint,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<(), SelectedInstructionError> {
    // Expand at the failing check so diagnostic sites identify the refusal,
    // rather than pointing every failure at a shared closure definition.
    macro_rules! invalid {
        () => {
            SelectedInstructionError::unsupported_shape(function)
        };
    }
    // A completion has no physical return lane or callback registration. The
    // edge is retained as its owner, never recast as an operation identity.
    if invocation_site.operation().is_none()
        && (!matches!(call.result, TargetBoundaryResult::Unit)
            || result.is_some()
            || !call.callbacks.is_empty()
            || !call.structural_arguments.is_empty())
    {
        return Err(invalid!());
    }
    let plan = &call.binding.boundary_entry_plan;
    let operands = call.binding.operand_placements();
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
        return Err(invalid!());
    }
    let pointer_size = u16::try_from(environment.target().pointer_size).map_err(|_| invalid!())?;
    let pointer_alignment =
        u16::try_from(environment.target().pointer_alignment).map_err(|_| invalid!())?;
    // Every retained callback roster row joins exactly one private
    // materialization the registrar plan declares. Each row's native
    // ordinal, pointer-shape placement, destination custody, thunk identity,
    // and owning operation are exact; a materialized call missing a retained
    // row, a retained row without its materialization, a duplicated
    // destination or placement slot, and a substituted roster all fail
    // closed.
    if call.callbacks.len() != plan.call.callback_materializations.len() {
        return Err(invalid!());
    }
    let mut private_ordinals = Vec::new();
    for (index, callback) in call.callbacks.iter().enumerate() {
        let ordinal =
            usize::try_from(callback.application.native_ordinal).map_err(|_| invalid!())?;
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
        if Some(callback.terminal_operation) != invocation_site.operation()
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
            return Err(invalid!());
        }
        if matches!(
            callback.destination,
            calling_conventions::NativePlace::Parameter(_)
        ) {
            private_ordinals.push(ordinal + call.binding.dispatch_operand_count());
        }
    }
    if operands.len()
        != call.scalar_arguments.len() + call.structural_arguments.len() + private_ordinals.len()
    {
        return Err(invalid!());
    }
    // Each scalar argument keeps its authored plan position, its source's own
    // scalar type, and the exact placement the plan assigned.
    for (index, argument) in call.scalar_arguments.iter().enumerate() {
        if scalar_shape(argument.source.scalar_type()).is_none()
            || index.checked_sub(1).is_some_and(|previous| {
                call.scalar_arguments[previous].parameter_index >= argument.parameter_index
            })
            || private_ordinals.contains(&(argument.parameter_index as usize))
            || operands.get(argument.parameter_index as usize) != Some(&argument.placement)
            || scalar_shape(argument.source.scalar_type()) != Some(argument.placement.shape)
            || match &argument.source {
                TargetUnitScalarArgumentSource::IntegerImmediate {
                    scalar_type, value, ..
                } => semantic_vocabulary::ScalarTerm::integer(*scalar_type, *value).is_err(),
                _ => false,
            }
        {
            return Err(invalid!());
        }
    }
    // Storage geometry and foreign classification are different contracts.
    // Reconstruct both from source declarations; an owned indirect placement
    // still transfers bytes, whereas a borrow transports a referent pointer.
    if call.structural_arguments.len() != call.structural_indices.len() {
        return Err(invalid!());
    }
    let mut structural_shapes = Vec::new();
    for ((index, argument), indices) in structural_parameter_positions(call)
        .zip(&call.structural_arguments)
        .zip(&call.structural_indices)
    {
        let signature = source.structural.as_ref().ok_or_else(|| invalid!())?;
        let parameter = signature
            .parameters
            .iter()
            .find(|parameter| parameter.semantic.place == argument.place);
        let (root_type, root_access, expected_source, root, presentation_debt) =
            if let Some(parameter) = parameter {
                (
                    parameter.semantic.structural_type,
                    parameter.semantic.access,
                    TargetStructuralArgumentSource::Placement(parameter.target.placement.clone()),
                    parameter.semantic.clone(),
                    false,
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
                    terminal_psi::StructuralParameterDeclaration {
                        place: result.place,
                        position: 0,
                        structural_type: result.structural_type,
                        access: terminal_psi::StructuralAccess::Owned,
                        multiplicity: result.multiplicity,
                        qualifications: result.qualifications.clone(),
                        projected_qualifications: result.projected_qualifications.clone(),
                    },
                    !result.claims.is_empty() || !result.qualification_establishments.is_empty(),
                )
            } else if let Some((block, parameter)) = invocation_site
                .operation()
                .and_then(|operation| borrowed_block_argument_root(source, operation, argument))
            {
                (
                    parameter.structural_type,
                    parameter.access,
                    TargetStructuralArgumentSource::BlockParameter {
                        block,
                        place: parameter.place,
                    },
                    parameter.clone(),
                    false,
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
                    parameter.clone(),
                    false,
                )
            } else {
                return Err(invalid!());
            };
        let semantic = terminal_psi::StructuralArgument {
            place: argument.place,
            path: argument.path.clone(),
            access: argument.access,
        };
        let presentation = (argument.access != terminal_psi::StructuralAccess::Owned).then(||
            crate::structural_inputs::structural_reference_input::borrowed_byte_view_presentation(
                &root, &semantic, argument.structural_type, &signature.structural_types,
            )).flatten();
        let (projected_type, source_byte_offset, elements) = if let Some((offset, _, _)) =
            presentation
        {
            if presentation_debt {
                return Err(invalid!());
            }
            (argument.structural_type, offset, Vec::new())
        } else if argument
            .path
            .contains(&terminal_psi::StructuralPathSegment::Referent)
        {
            let address = crate::selection::reference_input::borrowed_argument(
                source, &semantic, argument, indices,
            )
            .ok_or_else(|| invalid!())?;
            // A thin pointer exports the original address and omits the
            // codec. The checked loan must admit only this byte footprint,
            // independently of the descriptor's source arrangement.
            if address.scalar_descriptor {
                let scalar = crate::structural_inputs::structural_reference_input::scalar_storage_type(
                    argument.structural_type, &signature.structural_types,
                ).ok_or_else(|| invalid!())?;
                crate::selection::reference_input::validate_scalar_foreign_export(
                    source, &semantic, scalar, argument.shape.byte_size,
                )?;
            }
            let projection =
                crate::structural_inputs::structural_reference_input::reference_projection(
                    argument.root_structural_type,
                    &argument.path,
                    argument.access,
                    Some(argument.structural_type),
                    &signature.structural_types,
                )
                .ok_or_else(|| invalid!())?;
            (
                argument.structural_type,
                address.referent_offset,
                projection.elements,
            )
        } else if argument.access != terminal_psi::StructuralAccess::Owned {
            crate::structural_inputs::structural_reference_input::runtime_borrowed_projection(
                argument.root_structural_type,
                &argument.path,
                argument.access,
                Some(argument.structural_type),
                &signature.structural_types,
            )
            .ok_or_else(|| invalid!())?
        } else {
            crate::structural_inputs::structural_reference_input::runtime_projection(
                argument.root_structural_type,
                &argument.path,
                &signature.structural_types,
            )
            .ok_or_else(|| invalid!())?
        };
        let projected_shape = crate::structural_inputs::structural_reference_input::shape(
            projected_type,
            &signature.structural_types,
        )
        .ok_or_else(|| invalid!())?;
        let (storage_shape, abi_shape) = match argument.access {
            terminal_psi::StructuralAccess::Owned => {
                // Whole-place owned transport still requires an owned root;
                // a projected member moves its subtree by value out of any
                // root that can surrender it — an owned root's partial
                // custody or an exclusive borrow's referent hole.
                let admitted_root = if argument.path.is_empty() {
                    root_access == terminal_psi::StructuralAccess::Owned
                } else {
                    matches!(
                        root_access,
                        terminal_psi::StructuralAccess::Owned
                            | terminal_psi::StructuralAccess::MutableBorrow
                    )
                };
                if !admitted_root {
                    return Err(invalid!());
                }
                let classified =
                    abstract_operations_to_target_operations::classified_boundary_shape(
                        projected_type,
                        &signature.structural_types,
                        plan.call.policy,
                    )
                    .map_err(|_| invalid!())?;
                (projected_shape, classified)
            }
            terminal_psi::StructuralAccess::SharedBorrow
            | terminal_psi::StructuralAccess::MutableBorrow
            | terminal_psi::StructuralAccess::WriteOnlyBorrow => {
                let descriptor = signature.structural_types.iter().any(|declaration| {
                    declaration.id == projected_type
                        && declaration.shape.is_borrowed_view_byte_sequence()
                });
                if descriptor {
                    let classified =
                        abstract_operations_to_target_operations::classified_boundary_shape(
                            projected_type,
                            &signature.structural_types,
                            plan.call.policy,
                        )
                        .map_err(|_| invalid!())?;
                    (
                        ValueShape::borrowed_reference(
                            projected_shape.byte_size,
                            projected_shape.alignment,
                        ),
                        classified,
                    )
                } else {
                    if !matches!(argument.destination.locations.as_slice(),
                    [ValueLocation::Register { value_byte_offset: 0, byte_size, .. }]
                    | [ValueLocation::Stack { value_byte_offset: 0, byte_size, .. }]
                        if *byte_size == u32::from(pointer_size))
                    {
                        return Err(invalid!());
                    }
                    (
                        ValueShape::borrowed_reference(
                            projected_shape.byte_size,
                            projected_shape.alignment,
                        ),
                        ValueShape::integer(u32::from(pointer_size), pointer_alignment),
                    )
                }
            }
        };
        if (presentation.is_none()
            && argument.path.iter().any(|segment| match segment {
                terminal_psi::StructuralPathSegment::Field(_)
                | terminal_psi::StructuralPathSegment::FixedIndex(_) => false,
                terminal_psi::StructuralPathSegment::RuntimeIndex { .. } => {
                    argument.access == terminal_psi::StructuralAccess::Owned
                }
                terminal_psi::StructuralPathSegment::Referent => {
                    argument.access == terminal_psi::StructuralAccess::Owned
                }
                _ => true,
            }))
            || argument.root_structural_type != root_type
            || argument.structural_type != projected_type
            || argument.source_byte_offset != source_byte_offset
            || argument.shape != storage_shape
            || argument.fixed_array_length != presentation.and_then(|(_, length, _)| length)
            || argument.element_stride != presentation.and_then(|(_, _, stride)| stride)
            || !crate::structural_inputs::structural_reference_input::runtime_indices_match(
                indices, &elements,
            )
            || argument.indices.len() != indices.len()
            || argument
                .indices
                .iter()
                .zip(indices)
                .any(|(target, selected)| {
                    target.operand.source_value() != selected.operand.value
                        || target.operand.scalar_type() != selected.operand.scalar_type
                        || target.stride != selected.stride
                })
            || argument.source != expected_source
            || operands.get(index) != Some(&argument.destination)
            || argument.destination.shape != abi_shape
        {
            return Err(invalid!());
        }
        structural_shapes.push((index, abi_shape));
    }
    // The scalar result home is exact value custody: the defining operation,
    // source value, scalar type, and shape must equal the legalized result,
    // and the plan must transport that complete shape directly or indirectly. A unit
    // result admits neither a home nor a plan result row.
    let result_shape = match (&call.result, &plan.call.result, result) {
        (TargetBoundaryResult::Unit, None, None) => None,
        (TargetBoundaryResult::Scalar(home), Some(placement), Some(result)) => {
            if scalar_shape(result.scalar_type).is_none()
                || Some(home.defining_operation) != invocation_site.operation()
                || home.source_value != result.value
                || home.scalar_type != result.scalar_type
                || scalar_shape(result.scalar_type) != Some(home.shape)
                || placement.shape != home.shape
            {
                return Err(invalid!());
            }
            if !placement.is_complete_scalar_result() {
                return Err(invalid!());
            }
            Some(home.shape)
        }
        (TargetBoundaryResult::Structural(home), Some(placement), None) => {
            let (producer, result) = home.operation_result().ok_or_else(|| invalid!())?;
            let signature = source.structural.as_ref().ok_or_else(|| invalid!())?;
            let storage = crate::structural_inputs::structural_reference_input::shape(
                result.structural_type,
                &signature.structural_types,
            )
            .ok_or_else(|| invalid!())?;
            let shape = abstract_operations_to_target_operations::classified_boundary_shape(
                result.structural_type,
                &signature.structural_types,
                plan.call.policy,
            )
            .map_err(|_| invalid!())?;
            if Some(producer) != invocation_site.operation()
                || crate::selection::aggregate_result_input::home(source, result.place)
                    != Some((producer, result))
                || home.layout != target_operations::TargetStructuralHomeLayout::Aggregate(storage)
                || placement.shape != shape
            {
                return Err(invalid!());
            }
            Some(shape)
        }
        _ => return Err(invalid!()),
    };
    // Re-validate the retained plan against the independently re-derived
    // signature; the canonical evaluation must equal the carried plan exactly.
    let mut parameters =
        vec![ValueShape::integer(u32::from(pointer_size), pointer_alignment); operands.len()];
    for argument in &call.scalar_arguments {
        *parameters
            .get_mut(argument.parameter_index as usize)
            .ok_or_else(|| invalid!())? =
            scalar_shape(argument.source.scalar_type()).ok_or_else(|| invalid!())?;
    }
    for (position, shape) in structural_shapes {
        *parameters.get_mut(position).ok_or_else(|| invalid!())? = shape;
    }
    let signature = CallSignature {
        parameters: parameters
            .into_iter()
            .skip(call.binding.dispatch_operand_count())
            .collect(),
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
    .map_err(|_| invalid!())?;
    if validated.plan() != plan {
        return Err(invalid!());
    }
    // Reconstruct the complete physical row, including all result fragments,
    // from the semantic plan just checked above and the target template.
    if call_key(call, environment) != Some(key)
        || call_constraint(call, environment).as_ref() != Some(constraint)
    {
        return Err(invalid!());
    }
    Ok(())
}
