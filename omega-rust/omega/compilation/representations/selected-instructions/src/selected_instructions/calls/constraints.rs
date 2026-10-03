//! Instantiate call operands from retained evaluated placement, not signature catalogs.
//!
//! The caller supplies the independently validated, zero-operand target/mechanism
//! row and target-owned register resolver. This query validates physical plan
//! structure and callback joins; it does not establish source argument ownership,
//! result type identity, provider admission, or the authority of the supplied base.
//! Those remain obligations of the stage consuming the retained call contract.

use calling_conventions::{
    CallSignature, IndirectPointerLocation, MachineRegister, NativePlace, ValueClass,
    ValueLocation, ValueShape,
};
use legalized_operations::LegalizedNormalizedForeignCall;
use register_model::{
    RegisterConstraintFamily, RegisterInstructionConstraint, RegisterOperandAccess,
    RegisterOperandConstraint, RegisterView, RegisterViewId, ValidatedPhysicalRegisterModel,
};
use target_operations::NormalizedForeignCallMechanism;

use crate::{
    SelectedFunction, SelectedInstruction, SelectedInstructionKind, SelectedNormalizedForeignCall,
};

/// Find the unique foreign-call roster record named by this instruction.
///
/// An ordinal cannot authorize another instruction's contract, and duplicate
/// records for the same instruction cannot choose their own preferred ordinal.
pub fn foreign_call_record<'function>(
    function: &'function SelectedFunction,
    instruction: &SelectedInstruction,
) -> Option<&'function SelectedNormalizedForeignCall> {
    let SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } = instruction.kind
    else {
        return None;
    };
    let record = function
        .normalized_foreign_calls
        .get(usize::try_from(ordinal).ok()?)?;
    if record.instruction != instruction.id || record.call.boundary != boundary {
        return None;
    }
    let mut matching = function
        .normalized_foreign_calls
        .iter()
        .filter(|candidate| candidate.instruction == instruction.id);
    let unique = matching.next()?;
    (matching.next().is_none() && unique == record).then_some(record)
}

/// Derive exact explicit operands and implicit effects for one retained call.
///
/// Register fragments preserve placement order within each authored parameter;
/// parameter operands retain the existing integer-before-floating bank order.
/// Stack fragments are transported by separate instructions. Private callback
/// parameters are materialized inside the call encoding and therefore contribute
/// implicit uses instead of explicit operands. Result definitions remove only
/// their architectural write footprint from the canonical base clobbers.
pub fn instantiate_foreign_call_constraint(
    call: &LegalizedNormalizedForeignCall,
    base: &RegisterInstructionConstraint,
    physical: &ValidatedPhysicalRegisterModel,
    fixed_register_view: impl Fn(MachineRegister) -> Option<RegisterViewId>,
) -> Option<RegisterInstructionConstraint> {
    let target = call.binding.native_target()?;
    let expected_family = match call.binding.mechanism {
        NormalizedForeignCallMechanism::Import { .. } => RegisterConstraintFamily::Call,
        NormalizedForeignCallMechanism::SystemCall(_) => RegisterConstraintFamily::SystemCall,
    };
    if !base.operands.is_empty()
        || base.key.family != expected_family
        || physical.model().architecture != target.architecture
        || !call.binding.has_valid_entry_mechanism()
    {
        return None;
    }
    let plan = &call.binding.boundary_entry_plan;
    let signature = CallSignature {
        parameters: plan
            .call
            .parameters
            .iter()
            .map(|placement| placement.shape)
            .collect(),
        result: plan.call.result.as_ref().map(|placement| placement.shape),
    };
    let validated = match call.callbacks.first() {
        Some(callback) => {
            calling_conventions::validate_boundary_entry_plan_with_callback_materializations(
                plan.clone(),
                &signature,
                &callback.registrar_context,
            )
        }
        None => calling_conventions::validate_boundary_entry_plan(plan.clone(), &signature),
    }
    .ok()?;
    if validated.plan() != plan || call.callbacks.len() != plan.call.callback_materializations.len()
    {
        return None;
    }

    let mut row = base.clone();
    let mut private_parameters = Vec::new();
    let mut field_destination = false;
    let pointer_shape = ValueShape::integer(
        u32::try_from(target.pointer_size).ok()?,
        u16::try_from(target.pointer_alignment).ok()?,
    );
    for (callback_index, callback) in call.callbacks.iter().enumerate() {
        let parameter = usize::try_from(callback.application.native_ordinal).ok()?;
        if callback.registrar_boundary_entry_plan != *plan
            || callback.registrar_context != call.callbacks.first()?.registrar_context
            || callback.application.shape != pointer_shape
            || callback.application.placement.shape != callback.application.shape
            || plan.call.parameters.get(parameter) != Some(&callback.application.placement)
            || !plan
                .call
                .callback_materializations
                .iter()
                .any(|materialization| materialization.destination == callback.destination)
            || call.callbacks[..callback_index].iter().any(|previous| {
                previous.destination == callback.destination
                    || previous.placement_index == callback.placement_index
            })
        {
            return None;
        }
        match callback.destination {
            NativePlace::Parameter(identity) => {
                if identity != callback.application.parameter
                    || callback.field_byte_offset.is_some()
                    || private_parameters.contains(&parameter)
                {
                    return None;
                }
                private_parameters.push(parameter);
                match callback.application.placement.locations.as_slice() {
                    [ValueLocation::Register { register, .. }] => {
                        row.implicit_uses.extend(
                            register_view(physical, *register, &fixed_register_view)?
                                .units
                                .iter()
                                .copied(),
                        );
                    }
                    [ValueLocation::Stack { .. }] => {}
                    _ => return None,
                }
            }
            NativePlace::Field {
                parameter: identity,
                ..
            } => {
                if identity != callback.application.parameter
                    || callback.field_byte_offset.is_none()
                    || !matches!(
                        callback.application.placement.locations.as_slice(),
                        [ValueLocation::Register { .. }]
                    )
                {
                    return None;
                }
                field_destination = true;
            }
        }
    }
    if field_destination {
        row.implicit_uses.extend(
            register_view(
                physical,
                plan.call.policy.callback_field_scratch(),
                &fixed_register_view,
            )?
            .units
            .iter()
            .copied(),
        );
    }
    row.implicit_uses.sort_unstable();
    row.implicit_uses.dedup();

    let mut parameters = plan
        .call
        .parameters
        .iter()
        .enumerate()
        .filter(|(parameter, _)| !private_parameters.contains(parameter))
        .collect::<Vec<_>>();
    parameters.sort_by_key(|(_, placement)| placement.shape.class == ValueClass::Float);
    for (_, placement) in parameters {
        for location in &placement.locations {
            let register = match location {
                ValueLocation::Register { register, .. }
                | ValueLocation::Indirect {
                    pointer: IndirectPointerLocation::Register(register),
                    ..
                } => *register,
                ValueLocation::Stack { .. }
                | ValueLocation::Indirect {
                    pointer: IndirectPointerLocation::Stack { .. },
                    ..
                } => continue,
            };
            append_operand(
                &mut row,
                register_view(physical, register, &fixed_register_view)?,
                RegisterOperandAccess::Use,
            )?;
        }
    }
    if let Some(result) = &plan.call.result {
        for location in &result.locations {
            let (register, access) = match location {
                ValueLocation::Register { register, .. } => (*register, RegisterOperandAccess::Def),
                ValueLocation::Indirect {
                    pointer: IndirectPointerLocation::Register(register),
                    ..
                } => (*register, RegisterOperandAccess::Use),
                _ => return None,
            };
            append_operand(
                &mut row,
                register_view(physical, register, &fixed_register_view)?,
                access,
            )?;
        }
    }
    Some(row)
}

fn register_view<'physical>(
    physical: &'physical ValidatedPhysicalRegisterModel,
    register: MachineRegister,
    fixed_register_view: &impl Fn(MachineRegister) -> Option<RegisterViewId>,
) -> Option<&'physical RegisterView> {
    let identity = fixed_register_view(register)?;
    physical
        .model()
        .views
        .iter()
        .find(|view| view.id == identity)
}

fn append_operand(
    row: &mut RegisterInstructionConstraint,
    view: &RegisterView,
    access: RegisterOperandAccess,
) -> Option<()> {
    row.operands.push(RegisterOperandConstraint {
        operand: u16::try_from(row.operands.len()).ok()?,
        access,
        class: view.class,
        fixed_view: Some(view.id),
        tied_to: None,
        early_clobber: false,
    });
    if access == RegisterOperandAccess::Def {
        row.clobbers.retain(|unit| !view.write_units.contains(unit));
    }
    Some(())
}
