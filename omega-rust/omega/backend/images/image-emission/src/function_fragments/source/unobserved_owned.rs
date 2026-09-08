//! Admission over already replayed current source; this never proves an erasure.
use abstract_operations::{AbstractFunction, AbstractOperation};
use selected_instructions::{SelectedFunction, SelectedStructuralTransport, SelectedTerminator};
use target_operations::{TargetControlTerminator, TargetFunction, TargetOperation};
use terminal_psi::{StructuralAccess, StructuralMultiplicity, TerminalAffineCleanupAction};
#[cfg(test)]
mod tests;

/// The common source replay owns plain-type eligibility, complete frontier
/// conservation, and demand. This predicate bounds the image projection to
/// scalar graphs without executable uses of their owned parameters. Independently
/// established primitive locals may have storage and borrowed calls.
pub(in crate::function_fragments) fn arrivals(
    function: &AbstractFunction,
    target: &TargetFunction,
    selected: &SelectedFunction,
) -> bool {
    if function.result.scalar().is_none()
        || function.structural_parameters.is_empty()
        || function.attachment.is_some()
        || !function.entry_claims.is_empty()
        || !function.published_service_ceiling.is_empty()
        || target.mixed_structural_scalar_abi.is_none()
        || !matches!(target.operation, TargetOperation::ControlGraph(_))
        || selected.structural.is_none()
        || selected.ranked.is_some()
        || selected.memory_accesses.iter().any(|access| !primitive_local(function, access.place))
        || selected.local_storage_slots.iter().any(|slot| {
            !matches!(
                slot.id,
                selected_instructions::LocalStorageSlotId::Spill { .. }
            )
                && !matches!(slot.id, selected_instructions::LocalStorageSlotId::Structural { operation, place }
                    if function.operations.iter().any(|row| matches!(row,
                        AbstractOperation::EstablishPrimitiveLocal { psi_operation, result, .. }
                            if *psi_operation == operation && result.place == place)))
        })
        || !selected.boundary_settlements.is_empty()
        || selected.calls.iter().any(|call| {
            call.call.arguments.iter().any(|argument| {
                matches!(argument, legalized_operations::LegalizedScalarArgument::Structural { semantic, target }
                    if !primitive_local(function, semantic.place)
                        || !matches!(target.source, target_operations::TargetStructuralArgumentSource::EstablishedPrimitiveLocal { .. })
                        || semantic.access == StructuralAccess::Owned)
            })
        })
    {
        return false;
    }
    let declarations = || {
        function.structural_parameters.iter().chain(
            function
                .block_entries
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
    };
    if declarations().any(|parameter| {
        parameter.access != StructuralAccess::Owned
            || !matches!(
                parameter.multiplicity,
                StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
            )
            || parameter.is_self
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
    }) {
        return false;
    }
    let unused = |successor: &selected_instructions::SelectedSuccessor| {
        successor
            .structural_bindings
            .iter()
            .all(|binding| binding.transport == SelectedStructuralTransport::Unused)
            && successor.structural_case.is_none()
    };
    if !selected.blocks.iter().all(|block| match &block.terminator {
        SelectedTerminator::Jump { successor, .. } => unused(successor),
        SelectedTerminator::ConditionalBranch {
            when_nonzero,
            when_zero,
            ..
        } => unused(when_nonzero) && unused(when_zero),
        SelectedTerminator::ConditionalBranchU64LessThan {
            when_less,
            when_not_less,
            ..
        }
        | SelectedTerminator::ConditionalBranchI64LessThan {
            when_less,
            when_not_less,
            ..
        } => unused(when_less) && unused(when_not_less),
        SelectedTerminator::Return { .. } => true,
        SelectedTerminator::HostedExitProcess { .. } => false,
    }) {
        return false;
    }
    function.operations.iter().all(|operation| match operation {
        AbstractOperation::EstablishPrimitiveLocal { .. }
        | AbstractOperation::PrimitiveLocalStore { .. }
        | AbstractOperation::PrimitiveScalarRead { .. } => {
            super::super::structural::primitive_operation_retained(function, selected, operation)
                && match operation {
                    AbstractOperation::PrimitiveScalarRead { source, .. } => {
                        primitive_local(function, *source)
                    }
                    _ => true,
                }
        }
        AbstractOperation::CallStructuralScalar { psi_operation, .. }
        | AbstractOperation::CallUnit { psi_operation, .. } => {
            selected
                .calls
                .iter()
                .filter(|call| call.operation == *psi_operation)
                .count()
                == 1
        }
        AbstractOperation::IntegerConstant { .. }
        | AbstractOperation::BooleanConstant { .. }
        | AbstractOperation::IntegerEqual { .. }
        | AbstractOperation::IntegerLessThan { .. }
        | AbstractOperation::IntegerLessOrEqual { .. }
        | AbstractOperation::IntegerWiden { .. }
        | AbstractOperation::ExactIntegerAdd { .. }
        | AbstractOperation::ExactIntegerSubtract { .. }
        | AbstractOperation::Call { .. }
        | AbstractOperation::Jump { .. }
        | AbstractOperation::Conditional { .. } => true,
        AbstractOperation::Return {
            cleanup_actions, ..
        } => {
            scalar_cleanup_retained(operation, target)
                && cleanup_actions.iter().all(|action| {
                    let TerminalAffineCleanupAction::DiscardRoot(place) = action else {
                        return false;
                    };
                    declarations().any(|parameter| {
                        parameter.place == *place
                            && parameter.multiplicity == StructuralMultiplicity::Affine
                    })
                })
        }
        _ => false,
    })
}

fn primitive_local(function: &AbstractFunction, place: semantic_vocabulary::PlaceId) -> bool {
    !function
        .structural_parameters
        .iter()
        .chain(
            function
                .block_entries
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .any(|parameter| parameter.place == place)
        && function
            .operations
            .iter()
            .filter(|operation| {
                matches!(operation,
            AbstractOperation::EstablishPrimitiveLocal { result, .. } if result.place == place)
            })
            .count()
            == 1
}

/// Match the exact scalar return edge, value, and ordered no-code actions.
/// Complete source replay separately verifies disposal eligibility and order.
pub(in crate::function_fragments) fn scalar_cleanup_retained(
    operation: &AbstractOperation,
    target: &TargetFunction,
) -> bool {
    let AbstractOperation::Return {
        psi_edge,
        value,
        cleanup_actions,
        ..
    } = operation
    else {
        return false;
    };
    let TargetOperation::ControlGraph(graph) = &target.operation else {
        return false;
    };
    if !cleanup_actions
        .iter()
        .all(|action| matches!(action, TerminalAffineCleanupAction::DiscardRoot(_)))
    {
        return false;
    }
    let mut returns = graph
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            TargetControlTerminator::ReturnScalar {
                psi_edge: edge,
                source_value,
                cleanup_actions,
                ..
            } if edge == psi_edge => Some((source_value, cleanup_actions)),
            _ => None,
        });
    matches!(returns.next(), Some((source_value, actions)) if source_value == value && actions == cleanup_actions)
        && returns.next().is_none()
}
