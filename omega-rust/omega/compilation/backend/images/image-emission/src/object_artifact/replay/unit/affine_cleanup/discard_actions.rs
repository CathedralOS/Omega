//! The discard shapes an affine cleanup may take: every root discarded
//! whole, or the locals discarded whole and one parameter discarded by
//! residual paths.

use super::super::super::structural::partial_cleanup_partition::exact_partial_cleanup_partition;
use super::is_partial_cleanup_path;
use machine_code::UnitAffineCleanupRecord;

use super::CleanupInputs;
use super::cleanup_roots::CleanupRoots;

/// Whether the cleanup's root discards name the same place twice.
pub(super) fn root_discards_are_duplicated(cleanup: &UnitAffineCleanupRecord) -> bool {
    cleanup
        .actions
        .iter()
        .filter_map(|action| match action {
            terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place) => Some(*place),
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        != cleanup.actions.len()
}

/// Whether the residual discards after the local discards are malformed:
/// they must all target the single remaining affine parameter along
/// distinct partial paths that, with the paths moved into callees, exactly
/// partition the parameter's type.
pub(super) fn residual_discards_are_malformed(
    inputs: &CleanupInputs<'_>,
    roots: &CleanupRoots,
) -> bool {
    let CleanupInputs {
        parameter_homes,
        internal_unit_calls,
        cleanup,
        partially_consumed_affine_parameter,
        ..
    } = *inputs;
    let expected_parameter_suffix = &roots.expected_parameter_suffix;
    let expected_local_actions = &roots.expected_local_actions;
    let residual_actions = &cleanup.actions[expected_local_actions.len()..];
    let residuals = residual_actions
        .iter()
        .filter_map(|action| match action {
            terminal_psi::TerminalAffineCleanupAction::DiscardResidual(residual) => Some(residual),
            _ => None,
        })
        .collect::<Vec<_>>();
    let residual_root = residuals.first().map(|residual| residual.place);
    let parameter_type = residual_root.and_then(|place| {
        parameter_homes
            .iter()
            .find(|parameter| parameter.place == place)
            .map(|parameter| parameter.structural_type)
    });
    let moved = internal_unit_calls
        .iter()
        .flat_map(|call| &call.arguments)
        .filter(|argument| {
            Some(argument.place) == residual_root
                && Some(argument.root_structural_type) == parameter_type
        })
        .map(|argument| (argument.path.as_slice(), argument.structural_type))
        .collect::<Vec<_>>();
    cleanup.actions[..expected_local_actions.len()] != *expected_local_actions
        || residuals.len() != residual_actions.len()
        || residuals.is_empty()
        || residual_root.is_none_or(|root| expected_parameter_suffix.as_slice() != [root])
        || parameter_type.is_none()
        || (moved.iter().any(|(path, _)| {
            path.iter().any(|segment| {
                matches!(segment, terminal_psi::StructuralPathSegment::FixedIndex(_))
            })
        }) && !partially_consumed_affine_parameter)
        || residuals.iter().any(|residual| {
            Some(residual.place) != residual_root
                || residual.path.is_empty()
                || !is_partial_cleanup_path(&residual.path)
                || parameter_type == Some(residual.structural_type)
        })
        || residuals.iter().enumerate().any(|(index, residual)| {
            residuals[..index].iter().any(|earlier| {
                residual.path.starts_with(&earlier.path) || earlier.path.starts_with(&residual.path)
            })
        })
        || moved.is_empty()
        || moved.iter().any(|(path, _)| {
            path.is_empty()
                || !is_partial_cleanup_path(path)
                || residuals.iter().any(|residual| {
                    path.starts_with(&residual.path) || residual.path.starts_with(path)
                })
        })
        || moved.iter().enumerate().any(|(index, (path, _))| {
            moved[..index]
                .iter()
                .any(|(earlier, _)| path.starts_with(earlier) || earlier.starts_with(path))
        })
        || parameter_type.is_none_or(|root_type| {
            !exact_partial_cleanup_partition(
                &cleanup.structural_types,
                root_type,
                &moved,
                &residuals,
            )
        })
}
