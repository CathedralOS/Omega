//! Cleanup-sensitive Unit return validation and lowering.

use super::super::cleanup::validate_bounded_nominal_cleanup_body;
use super::super::shared::*;
use super::super::structural::exact_fully_consumed_affine_pair_root;
use super::super::structural_layout::{
    expected_maximal_residual_subtrees, is_partial_cleanup_path,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_unit_return(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    parameters: &[TargetStructuralParameter],
    operations: &mut Vec<TargetUnitOperation>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    nonreturning_boundary: bool,
    provenance: &mut TerminalPsiProvenance,
    returned: &mut bool,
) -> Result<(), LoweringError> {
    match operation {
        AbstractOperation::ReturnUnit {
            psi_edge,
            cleanup_actions,
        } => {
            if nonreturning_boundary && !cleanup_actions.is_empty() {
                return Err(LoweringError::InvalidLinuxExitGroupShape(function.machine));
            }
            let local_places = operations
                .iter()
                .filter_map(|operation| match operation {
                    TargetUnitOperation::EstablishTrivialAffineLocal { place, .. } => {
                        Some(place.id)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let structural_result_places = operations
                .iter()
                .filter_map(|operation| match operation {
                    TargetUnitOperation::StructuralResultCall { result, .. } => Some(result.place),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let consumed_whole_roots = operations
                .iter()
                .flat_map(|operation| match operation {
                    TargetUnitOperation::Call { arguments, .. }
                    | TargetUnitOperation::StructuralScalarCall { arguments, .. }
                    | TargetUnitOperation::StructuralResultCall { arguments, .. } => arguments
                        .iter()
                        .filter(|argument| {
                            argument.path.is_empty()
                                && argument.access == psi_terminal::StructuralAccess::Owned
                        })
                        .map(|argument| argument.place)
                        .collect::<Vec<_>>(),
                    _ => Vec::new(),
                })
                .collect::<BTreeSet<_>>();
            let fully_consumed_affine_pair = exact_fully_consumed_affine_pair_root(
                function,
                &parameters,
                &operations,
                structural_types,
                functions,
            );
            let expected_roots = structural_result_places
                .iter()
                .rev()
                .copied()
                .chain(
                    local_places
                        .iter()
                        .rev()
                        .filter(|place| !consumed_whole_roots.contains(place))
                        .copied(),
                )
                .chain(
                    function
                        .structural_parameters
                        .iter()
                        .rev()
                        .filter(|parameter| {
                            parameter.multiplicity == psi_terminal::StructuralMultiplicity::Affine
                                && parameter.access == psi_terminal::StructuralAccess::Owned
                                && Some(parameter.place) != fully_consumed_affine_pair
                                && !consumed_whole_roots.contains(&parameter.place)
                        })
                        .map(|parameter| parameter.place),
                )
                .collect::<Vec<_>>();
            let root_discards = cleanup_actions
                .iter()
                .filter_map(|action| match action {
                    psi_terminal::TerminalAffineCleanupAction::DiscardRoot(place) => Some(*place),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let residual_discards = cleanup_actions
                .iter()
                .filter_map(|action| match action {
                    psi_terminal::TerminalAffineCleanupAction::DiscardResidual(discard) => {
                        Some(discard)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let nominal_cleanups = cleanup_actions
                .iter()
                .filter_map(|action| match action {
                    psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                        Some(cleanup.clone())
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            if root_discards.len() + residual_discards.len() + nominal_cleanups.len()
                != cleanup_actions.len()
            {
                unreachable!("every cleanup action has one exact kind")
            }
            if residual_discards.is_empty()
                && nominal_cleanups.is_empty()
                && (root_discards != expected_roots
                    || operations.iter().any(|operation| {
                        matches!(operation,
                        TargetUnitOperation::Call { arguments, .. }
                            | TargetUnitOperation::StructuralResultCall { arguments, .. }
                            if arguments.iter().any(|argument| {
                                !argument.path.is_empty()
                                    && root_discards.contains(&argument.place)
                            }))
                    }))
            {
                return Err(LoweringError::UnsupportedOperationInUnitFunction(
                    function.machine,
                ));
            }
            if !residual_discards.is_empty() {
                let Some(residual_root) = residual_discards.first().map(|discard| discard.place)
                else {
                    unreachable!("nonempty residual cleanup has a root")
                };
                let Some(parameter) = function
                    .structural_parameters
                    .iter()
                    .find(|parameter| parameter.place == residual_root)
                else {
                    return Err(LoweringError::UnsupportedOperationInUnitFunction(
                        function.machine,
                    ));
                };
                let moved_arguments = operations
                    .iter()
                    .filter_map(|operation| match operation {
                        TargetUnitOperation::Call { arguments, .. } => Some(arguments),
                        _ => None,
                    })
                    .flatten()
                    .filter(|argument| argument.place == residual_root)
                    .collect::<Vec<_>>();
                let mut moved_subtrees = Vec::with_capacity(moved_arguments.len());
                if moved_arguments.is_empty()
                    || moved_arguments.iter().any(|argument| {
                        argument.root_structural_type != parameter.structural_type
                            || !is_partial_cleanup_path(&argument.path)
                            || moved_subtrees
                                .iter()
                                .any(|(path, _)| path == &argument.path)
                            || {
                                moved_subtrees
                                    .push((argument.path.clone(), argument.structural_type));
                                false
                            }
                    })
                {
                    return Err(LoweringError::UnsupportedOperationInUnitFunction(
                        function.machine,
                    ));
                }
                let Some(expected_residuals) = expected_maximal_residual_subtrees(
                    parameter.structural_type,
                    &moved_subtrees,
                    structural_types,
                ) else {
                    return Err(LoweringError::UnsupportedOperationInUnitFunction(
                        function.machine,
                    ));
                };
                let fixed_array_call_count = structural_types
                    .get(&parameter.structural_type)
                    .and_then(|declaration| match declaration.shape {
                        StructuralTypeShape::FixedArray { element, length: 2 }
                            if structural_types.get(&element).is_some_and(|inner| {
                                matches!(
                                    inner.shape,
                                    StructuralTypeShape::FixedArray {
                                        length: 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11,
                                        ..
                                    }
                                )
                            }) =>
                        {
                            Some(2)
                        }
                        StructuralTypeShape::FixedArray { length: 2, .. } => Some(1),
                        StructuralTypeShape::FixedArray { length: 3, .. } => {
                            Some(moved_arguments.len())
                        }
                        StructuralTypeShape::FixedArray { length: 4, .. } => Some(2),
                        _ => None,
                    });
                if parameter.multiplicity != psi_terminal::StructuralMultiplicity::Affine
                        || fixed_array_call_count.is_some_and(|expected_calls| {
                            function.structural_parameters.len() != 1
                                || !function.entry_claims.is_empty()
                                || !function.published_service_ceiling.is_empty()
                                || parameter.position != 0
                                || parameter.is_self
                                || parameter.access != psi_terminal::StructuralAccess::Owned
                                || !parameter.qualifications.is_empty()
                                || !local_places.is_empty()
                                || operations.len() != expected_calls
                                || operations.iter().any(|operation| {
                                    !matches!(operation, TargetUnitOperation::Call { .. })
                                })
                        })
                        || root_discards != local_places.iter().rev().copied().collect::<Vec<_>>()
                        || expected_roots.get(local_places.len()..) != Some(&[residual_root][..])
                        || expected_residuals.len() != residual_discards.len()
                        || cleanup_actions.get(..root_discards.len()).is_none_or(|prefix| {
                            !prefix.iter().zip(&root_discards).all(|(action, place)| {
                                matches!(action,
                                    psi_terminal::TerminalAffineCleanupAction::DiscardRoot(actual)
                                        if actual == place)
                            })
                        })
                        || cleanup_actions.get(root_discards.len()..).is_none_or(|suffix| {
                            suffix.iter().zip(&expected_residuals).any(
                                |(action, (path, structural_type))| {
                                    !matches!(action,
                                        psi_terminal::TerminalAffineCleanupAction::DiscardResidual(discard)
                                            if discard.place == residual_root
                                                && discard.path == *path
                                                && discard.structural_type == *structural_type)
                                },
                            )
                        })
                    {
                        return Err(LoweringError::UnsupportedOperationInUnitFunction(
                            function.machine,
                        ));
                    }
            }
            if !nominal_cleanups.is_empty() {
                if !local_places.is_empty()
                    || !root_discards.is_empty()
                    || !residual_discards.is_empty()
                    || nominal_cleanups.is_empty()
                    || function.structural_parameters.len() != nominal_cleanups.len()
                    || function
                        .structural_parameters
                        .iter()
                        .rev()
                        .zip(&nominal_cleanups)
                        .any(|(parameter, cleanup)| {
                            parameter.place != cleanup.place
                                || parameter.structural_type != cleanup.structural_type
                                || parameter.multiplicity
                                    != psi_terminal::StructuralMultiplicity::Affine
                        })
                {
                    return Err(LoweringError::UnsupportedOperationInUnitFunction(
                        function.machine,
                    ));
                }
                for cleanup in &nominal_cleanups {
                    let Some(cleanup_function) = functions.get(&cleanup.cleanup_machine).copied()
                    else {
                        return Err(LoweringError::UnsupportedOperationInUnitFunction(
                            function.machine,
                        ));
                    };
                    if cleanup_function.attachment != Some(cleanup.structural_type)
                        || cleanup_function.result != AbstractFunctionResult::Unit
                        || !cleanup_function.parameters.is_empty()
                        || !cleanup_function.structural_parameters.is_empty()
                        || !cleanup_function.entry_claims.is_empty()
                        || !cleanup_function.published_service_ceiling.is_empty()
                        || cleanup_function.block_entries.as_slice()
                            != [omega_abstract_operations::AbstractBlockEntry {
                                block: cleanup_function.entry,
                                parameters: Vec::new(),
                                operation_offset: 0,
                            }]
                    {
                        return Err(LoweringError::UnsupportedOperationInUnitFunction(
                            function.machine,
                        ));
                    }
                    validate_bounded_nominal_cleanup_body(
                        function.machine,
                        cleanup,
                        cleanup_function,
                        functions,
                        structural_types,
                    )?;
                }
            }
            if !nominal_cleanups.is_empty()
                && nominal_cleanups.len() + root_discards.len() + residual_discards.len()
                    != cleanup_actions.len()
            {
                return Err(LoweringError::UnsupportedOperationInUnitFunction(
                    function.machine,
                ));
            }
            operations.push(TargetUnitOperation::Return {
                psi_edge: *psi_edge,
                cleanup_actions: cleanup_actions.clone(),
            });
            provenance.edges.push(*psi_edge);
            *returned = true;
        }
        _ => unreachable!("Unit-return routing admits only Unit returns"),
    }
    Ok(())
}
