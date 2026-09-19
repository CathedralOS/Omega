//! Shared target-row custody for evaluated normalized foreign calls.
//!
//! `row` rejoins the exact target operation the lowering emitted for one
//! boundary call; `structural_argument_at` independently re-derives one
//! source-rooted borrowed flat-record argument from the boundary declaration
//! and the evaluated entry plan. Neither helper trusts the legalized row's
//! claims; both fail closed on missing, duplicated, or mismatched custody.
use super::{AbstractOperationPlan, MachineId, PsiOptimizationFunction, TargetOperationPlan};
use crate::LegalizationError;
use calling_conventions::{BoundaryEntryPlan, ValueLocation, ValuePlacement, ValueShape};
use semantic_vocabulary::OperationId;
use target_operations::{
    TargetNativeCallbackArgument, TargetStructuralArgument, TargetStructuralParameter,
    TargetUnitOperation,
};

/// The unique normalized foreign row one boundary call emitted, if any.
/// Duplicate rows for one source operation are a custody failure, not an
/// ambiguous match.
pub(in crate::legalization) fn row(
    native: &TargetOperationPlan,
    machine: MachineId,
    operation: OperationId,
) -> Result<Option<&TargetUnitOperation>, LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let function = native
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .ok_or(invalid.clone())?;
    let mut rows = function
        .graph
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|row| {
            matches!(
                row,
                TargetUnitOperation::NormalizedForeignCall { psi_operation, .. }
                    if *psi_operation == operation
            )
        });
    let row = rows.next();
    if rows.next().is_some() {
        return Err(invalid);
    }
    Ok(row)
}

/// Replay the retained native-callback roster against the target rows.
///
/// Every admitted argument must join to one unique `NormalizedForeignCall` row
/// by its Terminal operation, and that row's evaluated binding must carry the
/// argument's exact registrar entry plan. A roster entry no call consumed, a
/// duplicated operation, and a substituted registrar plan all fail closed; an
/// empty roster is inert.
pub(in crate::legalization) fn validate_native_callback_roster(
    native: &TargetOperationPlan,
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    for (index, callback) in native.native_callback_arguments.iter().enumerate() {
        if native.native_callback_arguments[..index]
            .iter()
            .any(|prior| prior.terminal_operation == callback.terminal_operation)
        {
            return Err(invalid);
        }
        let mut rows = native
            .functions
            .iter()
            .flat_map(|function| &function.graph.blocks)
            .flat_map(|block| &block.operations)
            .filter(|row| {
                matches!(
                    row,
                    TargetUnitOperation::NormalizedForeignCall { psi_operation, .. }
                        if *psi_operation == callback.terminal_operation
                )
            });
        let Some(row) = rows.next() else {
            return Err(invalid);
        };
        let TargetUnitOperation::NormalizedForeignCall { binding, .. } = row else {
            unreachable!("filtered on NormalizedForeignCall")
        };
        if rows.next().is_some()
            || binding.boundary_entry_plan != callback.registrar_boundary_entry_plan
        {
            return Err(invalid);
        }
    }
    Ok(())
}

/// The retained native callback argument one normalized foreign row consumes,
/// or `None` when the row's evaluated plan carries no callback
/// materializations.
///
/// The roster is the only carrier of binder and demand custody for the
/// callback's private parameter slot, so the join is exact in both
/// directions: a materialized call without a retained argument, a retained
/// argument whose registrar plan differs from the row's, a claimed
/// application placement the plan does not carry at its native ordinal, and a
/// retained argument naming a row whose materializations were dropped all
/// fail closed.
pub(in crate::legalization) fn native_callback_at<'a>(
    native: &'a TargetOperationPlan,
    operation: OperationId,
    boundary_entry_plan: &BoundaryEntryPlan,
) -> Result<Option<&'a TargetNativeCallbackArgument>, LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let materialized = !boundary_entry_plan
        .call
        .callback_materializations
        .is_empty();
    let mut matching = native
        .native_callback_arguments
        .iter()
        .filter(|callback| callback.terminal_operation == operation);
    let Some(callback) = matching.next() else {
        return if materialized { Err(invalid) } else { Ok(None) };
    };
    let pointer_size = u16::try_from(native.target.pointer_size).map_err(|_| invalid.clone())?;
    let pointer_alignment =
        u16::try_from(native.target.pointer_alignment).map_err(|_| invalid.clone())?;
    let ordinal =
        usize::try_from(callback.application.native_ordinal).map_err(|_| invalid.clone())?;
    if !materialized
        || matching.next().is_some()
        || callback.registrar_boundary_entry_plan != *boundary_entry_plan
        || !callback.callback_function.is_valid()
        || callback.callback_function.callback_thunk_placement_index()
            != Some(callback.placement_index)
        || callback.application.shape != callback.application.placement.shape
        || callback.application.shape != ValueShape::integer(pointer_size, pointer_alignment)
        || boundary_entry_plan.call.parameters.get(ordinal) != Some(&callback.application.placement)
    {
        return Err(invalid);
    }
    Ok(Some(callback))
}

/// Re-derive one source-rooted borrowed flat-record argument for the boundary
/// declaration's `position`-th structural parameter. The evaluated plan's
/// parameter row at that ordinal must place the referent pointer as one
/// pointer-width word; owned transport fails closed until an aggregate ABI
/// contract owns it.
pub(in crate::legalization) fn structural_argument_at(
    semantic: &terminal_psi::StructuralArgument,
    position: usize,
    declaration_parameter: &terminal_psi::StructuralParameterDeclaration,
    plan_row: Option<&ValuePlacement>,
    parameters: &[TargetStructuralParameter],
    optimized: &PsiOptimizationFunction,
    native: ::target::NativeTarget,
    plan: &AbstractOperationPlan,
) -> Result<TargetStructuralArgument, LegalizationError> {
    use terminal_psi::{StructuralAccess, StructuralPathSegment};
    let invalid = LegalizationError::SourceCustodyMismatch;
    let declarations = plan.structural_types.as_slice();
    let caller_parameter = parameters
        .iter()
        .find(|parameter| parameter.place == semantic.place)
        .ok_or(invalid.clone())?;
    let semantic_parameter = optimized
        .structural_parameters
        .iter()
        .chain(
            optimized
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .find(|parameter| parameter.place == semantic.place)
        .ok_or(invalid.clone())?;
    let pointer_size = u16::try_from(native.pointer_size).map_err(|_| invalid.clone())?;
    let pointer_alignment = u16::try_from(native.pointer_alignment).map_err(|_| invalid.clone())?;
    let (projected_type, source_byte_offset) =
        crate::structural_inputs::structural_reference_input::project(
            caller_parameter.structural_type,
            &semantic.path,
            declarations,
        )
        .ok_or(invalid.clone())?;
    let root_shape = crate::structural_inputs::structural_reference_input::shape(
        caller_parameter.structural_type,
        declarations,
    )
    .ok_or(invalid.clone())?;
    let projected_shape =
        crate::structural_inputs::structural_reference_input::shape(projected_type, declarations)
            .ok_or(invalid.clone())?;
    let destination = plan_row.ok_or(invalid.clone())?;
    let placed_pointer_word = match destination.locations.as_slice() {
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
        ] => *byte_size,
        _ => return Err(invalid),
    };
    if semantic.path.is_empty()
        || semantic
            .path
            .iter()
            .any(|segment| !matches!(segment, StructuralPathSegment::Field(_)))
        || usize::try_from(declaration_parameter.position).ok() != Some(position)
        || projected_type != declaration_parameter.structural_type
        || semantic.access != declaration_parameter.access
        || !matches!(
            semantic.access,
            StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow
        )
        || declaration_parameter.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
        || !declaration_parameter.qualifications.is_empty()
        || !declaration_parameter.projected_qualifications.is_empty()
        || caller_parameter.structural_type != semantic_parameter.structural_type
        || caller_parameter.access != semantic_parameter.access
        || caller_parameter.multiplicity != semantic_parameter.multiplicity
        || caller_parameter.projected_qualifications != semantic_parameter.projected_qualifications
        || caller_parameter.shape
            != crate::structural_inputs::structural_reference_input::parameter_shape(
                semantic_parameter,
                declarations,
            )
            .ok_or(invalid.clone())?
        || u32::from(projected_shape.byte_size)
            .checked_add(source_byte_offset)
            .is_none_or(|end| end > u32::from(root_shape.byte_size))
        || destination.shape != ValueShape::integer(pointer_size, pointer_alignment)
        || placed_pointer_word != pointer_size
    {
        return Err(invalid);
    }
    Ok(TargetStructuralArgument {
        place: semantic.place,
        access: semantic.access,
        path: semantic.path.clone(),
        root_structural_type: caller_parameter.structural_type,
        structural_type: projected_type,
        shape: ValueShape::borrowed_reference(projected_shape.byte_size, projected_shape.alignment),
        source_byte_offset,
        fixed_array_length: None,
        element_stride: None,
        source: caller_parameter.placement.clone().into(),
        destination: destination.clone(),
    })
}
