//! Shared target-row custody for evaluated normalized foreign calls.
//!
//! `row` rejoins the exact target operation the lowering emitted for one
//! boundary call; `structural_argument_at` independently re-derives one
//! source-rooted borrowed flat-record argument from the boundary declaration
//! and the evaluated entry plan. Neither helper trusts the legalized row's
//! claims; both fail closed on missing, duplicated, or mismatched custody.
use super::{AbstractOperationPlan, MachineId, PsiOptimizationFunction, TargetOperationPlan};
use crate::LegalizationError;
use calling_conventions::{ValueLocation, ValuePlacement, ValueShape};
use semantic_vocabulary::OperationId;
use target_operations::{TargetStructuralArgument, TargetStructuralParameter, TargetUnitOperation};

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
