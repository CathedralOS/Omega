//! Shared target-row custody for evaluated normalized foreign calls.
//!
//! `row` rejoins the exact target operation the lowering emitted for one
//! boundary call; `structural_argument_at` independently re-derives one
//! source-rooted argument from the boundary declaration and the
//! evaluated entry plan. Neither helper trusts the legalized row's
//! claims; both fail closed on missing, duplicated, or mismatched custody.
use super::{AbstractOperationPlan, MachineId, PsiOptimizationFunction, TargetOperationPlan};
use crate::LegalizationError;
use calling_conventions::{BoundaryEntryPlan, ValueLocation, ValuePlacement, ValueShape};
use semantic_vocabulary::OperationId;
use target_operations::{
    TargetNativeCallbackArgument, TargetStructuralArgument, TargetStructuralParameter,
    TargetUnitOperation,
};

/// Reconstruct completion custody from the boundary declaration and source
/// operation, never from the target row's proposed home. ABI classification
/// differs from storage geometry: both are required for a foreign result.
pub(in crate::legalization) fn result_at(
    result: &abstract_operations::AbstractBoundaryResult,
    declaration: &terminal_psi::BoundaryMachineResult,
    operation: OperationId,
    optimized: &PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
    policy: calling_conventions::CallingPolicy,
) -> Result<(target_operations::TargetBoundaryResult, Option<ValueShape>), LegalizationError> {
    use target_operations::TargetBoundaryResult;
    match (result, declaration) {
        (
            abstract_operations::AbstractBoundaryResult::Unit,
            terminal_psi::BoundaryMachineResult::Unit,
        ) => Ok((TargetBoundaryResult::Unit, None)),
        (
            abstract_operations::AbstractBoundaryResult::Scalar(result),
            terminal_psi::BoundaryMachineResult::Scalar(declared),
        ) => {
            if result.scalar_type != *declared
                || super::value_type(optimized, result.value) != Some(result.scalar_type)
            {
                return Err(LegalizationError::custody());
            }
            let shape =
                super::scalar_shape(result.scalar_type).ok_or(LegalizationError::custody())?;
            Ok((
                TargetBoundaryResult::Scalar(target_operations::TargetUnitScalarHomeRequirement {
                    defining_operation: operation,
                    source_value: result.value,
                    scalar_type: result.scalar_type,
                    shape,
                }),
                Some(shape),
            ))
        }
        (
            abstract_operations::AbstractBoundaryResult::Structural(result),
            terminal_psi::BoundaryMachineResult::Structural(declared),
        ) => {
            if result.structural_type != declared.structural_type
                || result.multiplicity != declared.multiplicity
                || result.qualifications != declared.qualifications
                || result.projected_qualifications != declared.projected_qualifications
                || !result.projected_qualifications.is_empty()
                || !result.qualification_establishments.is_empty()
                || !result.claims.is_empty()
                || result.multiplicity == terminal_psi::StructuralMultiplicity::Linear
            {
                return Err(LegalizationError::custody());
            }
            let storage = crate::structural_inputs::structural_reference_input::shape(
                result.structural_type,
                &plan.structural_types,
            )
            .ok_or(LegalizationError::custody())?;
            let shape = abstract_operations_to_target_operations::classified_boundary_shape(
                result.structural_type,
                &plan.structural_types,
                policy,
            )
            .map_err(|_| LegalizationError::custody())?;
            Ok((
                TargetBoundaryResult::Structural(
                    target_operations::TargetStructuralHomeRequirement {
                        origin: target_operations::TargetStructuralHomeOrigin::OperationResult {
                            operation,
                            result: result.clone(),
                        },
                        layout: target_operations::TargetStructuralHomeLayout::Aggregate(storage),
                    },
                ),
                Some(shape),
            ))
        }
        _ => Err(LegalizationError::custody()),
    }
}

/// The unique normalized foreign row one boundary call emitted, if any.
/// Duplicate rows for one source operation are a custody failure, not an
/// ambiguous match.
pub(in crate::legalization) fn row(
    native: &TargetOperationPlan,
    machine: MachineId,
    operation: OperationId,
) -> Result<Option<&TargetUnitOperation>, LegalizationError> {
    let function = native
        .function(machine)
        .ok_or(LegalizationError::custody())?;
    let mut rows = function.graph.operations().filter(|row| {
        matches!(
            row,
            TargetUnitOperation::NormalizedForeignCall { psi_operation, .. }
                if *psi_operation == operation
        )
    });
    let row = rows.next();
    if rows.next().is_some() {
        return Err(LegalizationError::custody());
    }
    Ok(row)
}

/// Replay the retained native-callback roster against the target rows.
///
/// Every admitted argument must join to one unique `NormalizedForeignCall` row
/// by its Terminal operation, and that row's evaluated binding must carry the
/// argument's exact registrar entry plan. A roster entry no call consumed, a
/// duplicated (operation, placement) pair, and a substituted registrar plan
/// all fail closed; an empty roster is inert.
pub(in crate::legalization) fn validate_native_callback_roster(
    native: &TargetOperationPlan,
) -> Result<(), LegalizationError> {
    for (index, callback) in native.native_callback_arguments.iter().enumerate() {
        if native.native_callback_arguments[..index]
            .iter()
            .any(|prior| {
                prior.terminal_operation == callback.terminal_operation
                    && prior.placement_index == callback.placement_index
            })
        {
            return Err(LegalizationError::custody());
        }
        let mut rows = native
            .functions
            .iter()
            .flat_map(|function| function.graph.operations())
            .filter(|row| {
                matches!(
                    row,
                    TargetUnitOperation::NormalizedForeignCall { psi_operation, .. }
                        if *psi_operation == callback.terminal_operation
                )
            });
        let Some(row) = rows.next() else {
            return Err(LegalizationError::custody());
        };
        let TargetUnitOperation::NormalizedForeignCall { binding, .. } = row else {
            unreachable!("filtered on NormalizedForeignCall")
        };
        if rows.next().is_some()
            || binding.boundary_entry_plan != callback.registrar_boundary_entry_plan
        {
            return Err(LegalizationError::custody());
        }
    }
    Ok(())
}

/// The retained native callback arguments one normalized foreign row
/// consumes, in roster order, or an empty roster when the row's evaluated
/// plan carries no callback materializations.
///
/// The roster is the only carrier of binder and demand custody for the
/// callbacks' private slots, so the join is exact in both directions: every
/// plan materialization must name one retained argument's destination, every
/// retained argument must name one materialization, and no two may share a
/// destination, placement slot, or thunk identity. A materialized call
/// without the full retained roster, a retained argument whose registrar
/// plan differs from the row's, a claimed application placement the plan
/// does not carry, and a substituted materialization roster all fail closed.
pub(in crate::legalization) fn native_callbacks_at<'a>(
    native: &'a TargetOperationPlan,
    operation: OperationId,
    boundary_entry_plan: &BoundaryEntryPlan,
) -> Result<Vec<&'a TargetNativeCallbackArgument>, LegalizationError> {
    let materializations = &boundary_entry_plan.call.callback_materializations;
    let matching: Vec<&'a TargetNativeCallbackArgument> = native
        .native_callback_arguments
        .iter()
        .filter(|callback| callback.terminal_operation == operation)
        .collect();
    if materializations.is_empty() {
        return if matching.is_empty() {
            Ok(Vec::new())
        } else {
            Err(LegalizationError::custody())
        };
    }
    if matching.len() != materializations.len() {
        return Err(LegalizationError::custody());
    }
    let pointer_size =
        u16::try_from(native.target.pointer_size).map_err(|_| LegalizationError::custody())?;
    let pointer_alignment =
        u16::try_from(native.target.pointer_alignment).map_err(|_| LegalizationError::custody())?;
    for (index, callback) in matching.iter().enumerate() {
        let ordinal = usize::try_from(callback.application.native_ordinal)
            .map_err(|_| LegalizationError::custody())?;
        let placed_root = boundary_entry_plan.call.parameters.get(ordinal)
            == Some(&callback.application.placement);
        let destination_admitted = match &callback.destination {
            calling_conventions::NativePlace::Parameter(parameter) => {
                *parameter == callback.application.parameter
                    && callback.field_byte_offset.is_none()
                    && placed_root
            }
            calling_conventions::NativePlace::Field { parameter, .. } => {
                *parameter == callback.application.parameter
                    && callback.field_byte_offset.is_some()
                    && placed_root
            }
        };
        if callback.registrar_boundary_entry_plan != *boundary_entry_plan
            || !callback.callback_function.is_valid()
            || callback.callback_function.callback_thunk_placement_index()
                != Some(callback.placement_index)
            || callback.application.shape != callback.application.placement.shape
            || callback.application.shape
                != ValueShape::integer(u32::from(pointer_size), pointer_alignment)
            || !destination_admitted
            || !materializations
                .iter()
                .any(|materialization| materialization.destination == callback.destination)
            || matching[..index].iter().any(|prior| {
                prior.placement_index == callback.placement_index
                    || prior.destination == callback.destination
            })
        {
            return Err(LegalizationError::custody());
        }
    }
    if matching
        .iter()
        .filter(|callback| {
            matches!(
                callback.destination,
                calling_conventions::NativePlace::Parameter(_)
            )
        })
        .count()
        > 1
    {
        // One private parameter slot per call is the admitted bound; the
        // constraint catalog carries only single-removal callback rows.
        return Err(LegalizationError::custody());
    }
    Ok(matching)
}

/// Re-derive storage, access and ABI classification independently. Owned
/// arguments transfer a whole occurrence, even when the ABI passes a private
/// copy indirectly. Borrowed arguments instead transport a referent pointer;
/// equal storage size cannot turn either contract into the other.
pub(in crate::legalization) fn structural_argument_at(
    semantic: &terminal_psi::StructuralArgument,
    position: usize,
    declaration_parameter: &terminal_psi::StructuralParameterDeclaration,
    plan_row: Option<&ValuePlacement>,
    parameters: &[TargetStructuralParameter],
    optimized: &PsiOptimizationFunction,
    native: ::target::NativeTarget,
    policy: calling_conventions::CallingPolicy,
    plan: &AbstractOperationPlan,
    call_operation: OperationId,
    target_plan: &TargetOperationPlan,
) -> Result<TargetStructuralArgument, LegalizationError> {
    use terminal_psi::{StructuralAccess, StructuralPathSegment};
    let declarations = plan.structural_types.as_slice();
    let (root_type, root_access, root_multiplicity, source, root, presentation_debt) =
        if let Some(caller_parameter) = parameters
            .iter()
            .find(|parameter| parameter.place == semantic.place)
        {
            let semantic_parameter = optimized
                .structural_parameters
                .iter()
                .chain(optimized.block_structural_parameters())
                .find(|parameter| parameter.place == semantic.place)
                .ok_or(LegalizationError::custody())?;
            if caller_parameter.structural_type != semantic_parameter.structural_type
                || caller_parameter.access != semantic_parameter.access
                || caller_parameter.multiplicity != semantic_parameter.multiplicity
                || caller_parameter.projected_qualifications
                    != semantic_parameter.projected_qualifications
                || caller_parameter.shape
                    != crate::structural_inputs::structural_reference_input::parameter_shape(
                        semantic_parameter,
                        declarations,
                    )
                    .ok_or(LegalizationError::custody())?
            {
                return Err(LegalizationError::custody());
            }
            (
                caller_parameter.structural_type,
                semantic_parameter.access,
                semantic_parameter.multiplicity,
                caller_parameter.placement.clone().into(),
                semantic_parameter.clone(),
                false,
            )
        } else if let Some((block, parameter)) =
            super::aggregate_results::borrowed_block_arrival(optimized, semantic.place, plan)
        {
            let call_block = super::aggregate_results::node_site(optimized, |operation| {
                operation.psi_operation() == Some(call_operation)
            })
            .ok_or(LegalizationError::custody())?
            .0;
            let permitted_access = match parameter.access {
                StructuralAccess::SharedBorrow => semantic.access == StructuralAccess::SharedBorrow,
                StructuralAccess::MutableBorrow => {
                    semantic.access != StructuralAccess::Owned || !semantic.path.is_empty()
                }
                StructuralAccess::WriteOnlyBorrow => {
                    semantic.access == StructuralAccess::WriteOnlyBorrow
                }
                StructuralAccess::Owned => false,
            };
            if !permitted_access
                || !super::target::control_flow::sources::dominates(optimized, block, call_block)
            {
                return Err(LegalizationError::custody());
            }
            (
                parameter.structural_type,
                parameter.access,
                parameter.multiplicity,
                target_operations::TargetStructuralArgumentSource::BlockParameter {
                    block,
                    place: parameter.place,
                },
                parameter.clone(),
                false,
            )
        } else {
            // Reconstruct local storage from the optimized source's exact owner,
            // shared with ordinary aggregate calls. A retained target row cannot
            // grant backing or move an establishment across its consuming call.
            // Claims are custody the call's completion receipts end, not bytes
            // the foreign leaf transports.
            let home = super::aggregate_results::result_home(optimized, semantic.place, plan)?;
            if home.access() != terminal_psi::StructuralAccess::Owned
                || !home.qualifications().is_empty()
                || !home.projected_qualifications().is_empty()
            {
                return Err(LegalizationError::custody());
            }
            let site = |operation| {
                optimized.blocks.iter().find_map(|block| {
                    block
                        .nodes
                        .iter()
                        .position(|node| node.operation.psi_operation() == Some(operation))
                        .map(|position| (block.id, position))
                })
            };
            let after = site(call_operation).ok_or(LegalizationError::custody())?;
            let (before, source) = match home.origin {
                target_operations::TargetStructuralHomeOrigin::OperationResult {
                    operation,
                    ..
                } => (
                    site(operation).ok_or(LegalizationError::custody())?,
                    target_operations::TargetStructuralArgumentSource::StructuralHome {
                        psi_operation: operation,
                    },
                ),
                target_operations::TargetStructuralHomeOrigin::BlockParameter {
                    block,
                    ref declaration,
                } => (
                    (block, 0),
                    target_operations::TargetStructuralArgumentSource::BlockParameter {
                        block,
                        place: declaration.place,
                    },
                ),
            };
            if (before.0 == after.0 && home.operation_result().is_some() && before.1 >= after.1)
                || (before.0 != after.0
                    && !super::target::control_flow::sources::dominates(
                        optimized, before.0, after.0,
                    ))
            {
                return Err(LegalizationError::custody());
            }
            (
                home.structural_type(),
                home.access(),
                home.multiplicity(),
                source,
                terminal_psi::StructuralParameterDeclaration {
                    place: home.place(),
                    position: declaration_parameter.position,
                    structural_type: home.structural_type(),
                    access: home.access(),
                    multiplicity: home.multiplicity(),
                    qualifications: home.qualifications().to_vec(),
                    projected_qualifications: home.projected_qualifications().to_vec(),
                },
                home.has_claims()
                    || home
                        .operation_result()
                        .is_some_and(|(_, result)| !result.qualification_establishments.is_empty()),
            )
        };
    let pointer_size =
        u16::try_from(native.pointer_size).map_err(|_| LegalizationError::custody())?;
    let pointer_alignment =
        u16::try_from(native.pointer_alignment).map_err(|_| LegalizationError::custody())?;
    let presentation = (semantic.access != StructuralAccess::Owned)
        .then(|| {
            crate::structural_inputs::structural_reference_input::borrowed_byte_view_presentation(
                &root,
                semantic,
                declaration_parameter.structural_type,
                declarations,
            )
        })
        .flatten();
    let (projected_type, source_byte_offset, elements) = if let Some((offset, _, _)) = presentation
    {
        if presentation_debt {
            return Err(LegalizationError::custody());
        }
        (declaration_parameter.structural_type, offset, Vec::new())
    } else if semantic.path.contains(&StructuralPathSegment::Referent) {
        let projection =
            crate::structural_inputs::structural_reference_input::reference_projection(
                root_type,
                &semantic.path,
                semantic.access,
                Some(declaration_parameter.structural_type),
                declarations,
            )
            .ok_or(LegalizationError::custody())?;
        // The foreign ABI receives the referent address, never the codec
        // descriptor. Admit a scalar only when discarding that codec cannot
        // expose bytes outside any backing admitted by its checked loan.
        if projection.scalar_descriptor {
            let shape = crate::structural_inputs::structural_reference_input::shape(
                projection.selected,
                declarations,
            )
            .ok_or(LegalizationError::custody())?;
            let exported_width =
                crate::structural_inputs::structural_reference_input::scalar_reference_export_width(
                    projection.selected,
                    semantic.access,
                    declarations,
                );
            if exported_width != Some(shape.byte_size) {
                return Err(LegalizationError::custody());
            }
        }
        (
            projection.selected,
            projection.referent_offset,
            projection.elements,
        )
    } else if semantic.access != StructuralAccess::Owned {
        crate::structural_inputs::structural_reference_input::runtime_borrowed_projection(
            root_type,
            &semantic.path,
            semantic.access,
            Some(declaration_parameter.structural_type),
            declarations,
        )
        .ok_or(LegalizationError::custody())?
    } else {
        crate::structural_inputs::structural_reference_input::runtime_projection(
            root_type,
            &semantic.path,
            declarations,
        )
        .ok_or(LegalizationError::custody())?
    };
    let root_shape =
        crate::structural_inputs::structural_reference_input::shape(root_type, declarations)
            .ok_or(LegalizationError::custody())?;
    let projected_shape =
        crate::structural_inputs::structural_reference_input::shape(projected_type, declarations)
            .ok_or(LegalizationError::custody())?;
    let destination = plan_row.ok_or(LegalizationError::custody())?;
    let (shape, destination_shape, multiplicity) = match semantic.access {
        StructuralAccess::Owned => {
            // Whole-place owned transport still names the root itself under
            // an owned source; a projected member moves its subtree by value
            // out of any root that can surrender it — an owned root's
            // partial custody or an exclusive borrow's referent hole — and
            // presents the moved occurrence's own custody: used at most once
            // while the ownership frontier tracks the hole it leaves.
            let admitted_root = if semantic.path.is_empty() {
                root_access == StructuralAccess::Owned
            } else {
                matches!(root_access, terminal_psi::mutating_access_pair_pattern!())
            };
            if !admitted_root {
                return Err(LegalizationError::custody());
            }
            let classified = abstract_operations_to_target_operations::classified_boundary_shape(
                projected_type,
                declarations,
                policy,
            )
            .map_err(|_| LegalizationError::custody())?;
            // A projected member surrenders its own custody: affine, or
            // linear when the member carries its own claim. Ownership replay
            // already placed every completed claim at this owned argument.
            let multiplicity = if semantic.path.is_empty() {
                root_multiplicity
            } else if declaration_parameter.multiplicity
                == terminal_psi::StructuralMultiplicity::Linear
            {
                terminal_psi::StructuralMultiplicity::Linear
            } else {
                terminal_psi::StructuralMultiplicity::Affine
            };
            (projected_shape, classified, multiplicity)
        }
        StructuralAccess::SharedBorrow
        | StructuralAccess::MutableBorrow
        | StructuralAccess::WriteOnlyBorrow => {
            let descriptor = declarations.iter().any(|declaration| {
                declaration.id == projected_type
                    && declaration.shape.is_borrowed_view_byte_sequence()
            });
            let destination_shape = if descriptor {
                // A view's loan still has referent storage custody, but its
                // boundary operand is the descriptor value, not its address.
                abstract_operations_to_target_operations::classified_boundary_shape(
                    projected_type,
                    declarations,
                    policy,
                )
                .map_err(|_| LegalizationError::custody())?
            } else {
                if !matches!(destination.locations.as_slice(),
                [ValueLocation::Register { value_byte_offset: 0, byte_size, .. }]
                | [ValueLocation::Stack { value_byte_offset: 0, byte_size, .. }]
                    if *byte_size == u32::from(pointer_size))
                {
                    return Err(LegalizationError::custody());
                }
                ValueShape::integer(u32::from(pointer_size), pointer_alignment)
            };
            (
                ValueShape::borrowed_reference(
                    projected_shape.byte_size,
                    projected_shape.alignment,
                ),
                destination_shape,
                terminal_psi::StructuralMultiplicity::Unrestricted,
            )
        }
    };
    let projects_view_backing = semantic.access != StructuralAccess::Owned
        && !semantic.path.is_empty()
        && declarations.iter().any(|row| {
            row.id == root_type
                && (row.shape.is_borrowed_byte_carrier()
                    || matches!(
                        row.shape,
                        terminal_psi::StructuralTypeShape::ElementView { .. }
                    ))
        });
    // Dynamic selectors retain their accepted bound and use the same backing
    // projection as ordinary borrowed calls; no descriptor extent is invented.
    if (presentation.is_none()
        && semantic.path.iter().any(|segment| match segment {
            StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_) => false,
            StructuralPathSegment::RuntimeIndex { .. } => {
                semantic.access == StructuralAccess::Owned
            }
            StructuralPathSegment::Referent => semantic.access == StructuralAccess::Owned,
            _ => true,
        }))
        || usize::try_from(declaration_parameter.position).ok() != Some(position)
        || projected_type != declaration_parameter.structural_type
        || semantic.access != declaration_parameter.access
        || declaration_parameter.multiplicity != multiplicity
        || !declaration_parameter.qualifications.is_empty()
        || !declaration_parameter.projected_qualifications.is_empty()
        || (presentation.is_none()
            && !projects_view_backing
            && !semantic.path.contains(&StructuralPathSegment::Referent)
            && projected_shape
                .byte_size
                .checked_add(source_byte_offset)
                .is_none_or(|end| end > root_shape.byte_size))
        || destination.shape != destination_shape
    {
        return Err(LegalizationError::custody());
    }
    Ok(TargetStructuralArgument {
        place: semantic.place,
        access: semantic.access,
        path: semantic.path.clone(),
        root_structural_type: root_type,
        structural_type: projected_type,
        shape,
        source_byte_offset,
        fixed_array_length: presentation.and_then(|(_, length, _)| length),
        element_stride: presentation.and_then(|(_, _, stride)| stride),
        indices: super::target::control_flow::sources::call_indices(
            target_plan,
            optimized,
            target_operations::CallSiteOwner::Operation(call_operation),
            &elements,
        )?,
        source,
        destination: destination.clone(),
    })
}
