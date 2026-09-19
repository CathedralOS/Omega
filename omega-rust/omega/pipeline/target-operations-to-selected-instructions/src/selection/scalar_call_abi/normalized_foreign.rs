//! Exact per-plan register row and custody replay for normalized foreign calls.
//!
//! The evaluated `BoundaryEntryPlan` carried by the legalized call is producer
//! evidence, not authority: this module re-derives the call signature from the
//! retained argument and result custody, re-validates the plan against it, and
//! selects the unique register-model row whose explicit operands match the
//! plan's register-resident parameter banks and optional scalar result. A
//! substituted locator, provider execution, plan, argument, or home fails
//! closed; a missing or duplicated catalog row is not a match.
use super::{
    CallSignature, CallingPolicy, LegalizedScalarFunction, RegisterOperandAccess, ValueLocation,
    ValueShape, scalar_shape,
};
use crate::SelectedInstructionError;
use calling_conventions::{EntryControl, validate_boundary_entry_plan};
use legalized_operations::{LegalizedNormalizedForeignCall, LegalizedScalarInstruction};
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterConstraintKey, RegisterInstructionConstraint};
use semantic_vocabulary::ScalarType;
use target_operations::{TargetStructuralArgumentSource, TargetUnitScalarArgumentSource};

/// The exact operand roster one evaluated plan requires: register-resident
/// parameter banks in authored order, then the scalar result definition.
fn plan_operand_views(
    call: &LegalizedNormalizedForeignCall,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Option<Vec<(register_model::RegisterViewId, RegisterOperandAccess)>> {
    let plan = &call.binding.boundary_entry_plan.call;
    let mut views = Vec::new();
    for placement in &plan.parameters {
        match placement.locations.as_slice() {
            [
                ValueLocation::Register {
                    register,
                    value_byte_offset: 0,
                    byte_size,
                },
            ] if *byte_size == placement.shape.byte_size => {
                views.push((
                    environment.fixed_register_view(*register)?,
                    RegisterOperandAccess::Use,
                ));
            }
            [
                ValueLocation::Stack {
                    value_byte_offset: 0,
                    byte_size,
                    ..
                },
            ] if *byte_size == placement.shape.byte_size => {}
            _ => return None,
        }
    }
    if let Some(result) = &plan.result {
        let [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] = result.locations.as_slice()
        else {
            return None;
        };
        if *byte_size != result.shape.byte_size {
            return None;
        }
        views.push((
            environment.fixed_register_view(*register)?,
            RegisterOperandAccess::Def,
        ));
    }
    Some(views)
}

/// The unique selected foreign-call row for this exact evaluated plan. Zero
/// or multiple matching rows are both custody failures: the catalog must hold
/// exactly one row per (integer bank, register arity, scalar result) plan.
pub(crate) fn call_key(
    call: &LegalizedNormalizedForeignCall,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Option<RegisterConstraintKey> {
    let views = plan_operand_views(call, environment)?;
    let keys = environment.selected_keys();
    let mut matches = keys.call_normalized_foreign.iter().filter(|key| {
        environment.constraint(**key).is_some_and(|row| {
            row.operands.len() == views.len()
                && row
                    .operands
                    .iter()
                    .zip(&views)
                    .all(|(operand, (view, access))| {
                        operand.access == *access && operand.fixed_view == Some(*view)
                    })
        })
    });
    let result = *matches.next()?;
    matches.next().is_none().then_some(result)
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
    let invalid = || SelectedInstructionError::UnsupportedSourceShape { function };
    let legalized_operations::LegalizedScalarInstructionKind::NormalizedForeignCall(call) =
        &instruction.kind
    else {
        return Err(invalid());
    };
    let plan = &call.binding.boundary_entry_plan;
    // Target applicability, calling policy, and entry control are exact
    // coordinates; the locator's sealed target must name this environment.
    if call.binding.locator.target().native_target() != environment.target()
        || plan.call.policy != CallingPolicy::native_for_target(environment.target())
        || plan.call.entry_control != EntryControl::CallReturn
        || !plan.call.callback_materializations.is_empty()
        || call
            .binding
            .same_stack_contribution
            .provider_plan_report_identity()
            != call
                .provider_execution
                .provider_plan_report_identity()
                .get()
    {
        return Err(invalid());
    }
    // Scalar and structural argument lanes never mix in the evaluated plan:
    // the structural lane already transports pointer words for every plan
    // parameter.
    if !call.scalar_arguments.is_empty() && !call.structural_arguments.is_empty() {
        return Err(invalid());
    }
    if plan.call.parameters.len() != call.scalar_arguments.len() + call.structural_arguments.len() {
        return Err(invalid());
    }
    let pointer_size = u16::try_from(environment.target().pointer_size).map_err(|_| invalid())?;
    let pointer_alignment =
        u16::try_from(environment.target().pointer_alignment).map_err(|_| invalid())?;
    // Each scalar argument keeps its authored plan position, its source's own
    // fixed-integer scalar type, and the exact placement the plan assigned.
    for (index, argument) in call.scalar_arguments.iter().enumerate() {
        let ScalarType::Integer(integer) = argument.source.scalar_type() else {
            return Err(invalid());
        };
        if integer.carrier() != semantic_vocabulary::IntegerCarrier::Fixed
            || !matches!(integer.bits(), 8 | 16 | 32 | 64)
            || argument.parameter_index != index as u32
            || plan.call.parameters.get(index) != Some(&argument.placement)
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
    // Each structural argument re-derives its projected type, source offset,
    // and borrowed-view shape from the caller's own structural signature; the
    // source must be the incoming parameter's declared placement, and the
    // destination is the exact pointer word the plan assigned at that index.
    for (index, argument) in call.structural_arguments.iter().enumerate() {
        let signature = source.structural.as_ref().ok_or_else(invalid)?;
        let parameter = signature
            .parameters
            .iter()
            .find(|parameter| parameter.semantic.place == argument.place)
            .ok_or_else(invalid)?;
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
        if argument.path.is_empty()
            || argument
                .path
                .iter()
                .any(|segment| !matches!(segment, terminal_psi::StructuralPathSegment::Field(_)))
            || !matches!(
                argument.access,
                terminal_psi::StructuralAccess::SharedBorrow
                    | terminal_psi::StructuralAccess::MutableBorrow
                    | terminal_psi::StructuralAccess::WriteOnlyBorrow
            )
            || argument.root_structural_type != parameter.semantic.structural_type
            || argument.structural_type != projected_type
            || argument.source_byte_offset != source_byte_offset
            || argument.shape
                != ValueShape::borrowed_reference(
                    projected_shape.byte_size,
                    projected_shape.alignment,
                )
            || argument.fixed_array_length.is_some()
            || argument.element_stride.is_some()
            || argument.source
                != TargetStructuralArgumentSource::Placement(parameter.target.placement.clone())
            || plan.call.parameters.get(index) != Some(&argument.destination)
            || argument.destination.shape != ValueShape::integer(pointer_size, pointer_alignment)
        {
            return Err(invalid());
        }
        match argument.destination.locations.as_slice() {
            [
                ValueLocation::Register {
                    value_byte_offset: 0,
                    byte_size,
                    ..
                },
            ]
            | [
                ValueLocation::Stack {
                    value_byte_offset: 0,
                    byte_size,
                    ..
                },
            ] if *byte_size == pointer_size => {}
            _ => return Err(invalid()),
        }
    }
    // The scalar result home is exact value custody: the defining operation,
    // source value, scalar type, and shape must equal the legalized result,
    // and the plan's result must be one register of that exact shape. A unit
    // result admits neither a home nor a plan result row.
    match (&call.result_home, &plan.call.result, instruction.result) {
        (None, None, None) => {}
        (Some(home), Some(placement), Some(result)) => {
            let ScalarType::Integer(integer) = result.scalar_type else {
                return Err(invalid());
            };
            if integer.carrier() != semantic_vocabulary::IntegerCarrier::Fixed
                || !matches!(integer.bits(), 8 | 16 | 32 | 64)
                || home.defining_operation != instruction.operation
                || home.source_value != result.value
                || home.scalar_type != result.scalar_type
                || scalar_shape(result.scalar_type) != Some(home.shape)
                || placement.shape != home.shape
                || source.attachment.is_none()
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
        }
        _ => return Err(invalid()),
    }
    // Re-validate the retained plan against the independently re-derived
    // signature; the canonical evaluation must equal the carried plan exactly.
    let parameters = if call.structural_arguments.is_empty() {
        call.scalar_arguments
            .iter()
            .map(|argument| scalar_shape(argument.source.scalar_type()))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(invalid)?
    } else {
        vec![ValueShape::integer(pointer_size, pointer_alignment); call.structural_arguments.len()]
    };
    let signature = CallSignature {
        parameters,
        result: call.result_home.as_ref().map(|home| home.shape),
    };
    let Ok(validated) = validate_boundary_entry_plan(plan.clone(), &signature) else {
        return Err(invalid());
    };
    if validated.plan() != plan {
        return Err(invalid());
    }
    // The selected row is the unique catalog row for this plan, carried by
    // the environment's foreign roster and equal to the claimed constraint.
    let views = plan_operand_views(call, environment).ok_or_else(invalid)?;
    if call_key(call, environment) != Some(key)
        || environment.constraint(key) != Some(constraint)
        || constraint.key != key
        || constraint.operands.len() != views.len()
        || constraint
            .operands
            .iter()
            .zip(&views)
            .any(|(operand, (view, access))| {
                operand.access != *access || operand.fixed_view != Some(*view)
            })
    {
        return Err(invalid());
    }
    Ok(())
}
