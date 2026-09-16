//! The roots one Unit function's affine cleanup must discard, derived from
//! its locals, parameter homes, calls and settlements.

use machine_code::BoundaryResultRecord;
use semantic_vocabulary::PlaceId;

use super::CleanupInputs;

/// The roots a cleanup must discard, as the actions that discard them, the
/// owned affine parameters among them, and the operations establishing
/// its locals.
pub(super) struct CleanupRoots {
    /// The owned affine parameters to discard, last first.
    pub(super) expected_parameter_suffix: Vec<PlaceId>,
    /// The operations that establish the locals.
    pub(super) local_operations: std::collections::BTreeSet<semantic_vocabulary::OperationId>,
    /// The actions discarding every root: results, locals, then parameters.
    pub(super) expected_root_actions: Vec<terminal_psi::TerminalAffineCleanupAction>,
    /// The actions discarding the results and locals only.
    pub(super) expected_local_actions: Vec<terminal_psi::TerminalAffineCleanupAction>,
}

/// The roots the cleanup must discard, in cleanup order: the affine
/// structural results of the function's calls and settlements (latest
/// first) that are neither inspected nor discarded by a continuation, then
/// the trivial affine locals not transferred to a callee, then the owned
/// affine parameters not transferred, consumed or discarded by a
/// continuation; and the actions that discard exactly those roots.
pub(super) fn cleanup_roots(inputs: &CleanupInputs<'_>) -> CleanupRoots {
    let CleanupInputs {
        parameter_homes,
        internal_unit_calls,
        boundary_settlements,
        cleanup,
        fully_consumed_affine_parameter,
        continuation_discards,
        ..
    } = *inputs;
    let local_places = cleanup
        .locals
        .iter()
        .map(|(_, place, _)| place.id)
        .collect::<Vec<_>>();
    let transferred_roots = internal_unit_calls
        .iter()
        .flat_map(|call| &call.arguments)
        .filter(|argument| argument.path.is_empty())
        .map(|argument| argument.place)
        .collect::<std::collections::BTreeSet<_>>();
    let inspected_roots =
        crate::object_artifact::replay::boundary::runtime_scalar_custody::inspected_hosted_read_byte_roots(boundary_settlements);
    let expected_local_prefix = local_places
        .iter()
        .rev()
        .filter(|place| !transferred_roots.contains(place))
        .copied()
        .collect::<Vec<_>>();
    let expected_parameter_suffix = parameter_homes
        .iter()
        .rev()
        .filter(|home| {
            home.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                && home.access == terminal_psi::StructuralAccess::Owned
                && !transferred_roots.contains(&home.place)
                && !fully_consumed_affine_parameter
                && !continuation_discards.contains(&home.place)
        })
        .map(|home| home.place)
        .collect::<Vec<_>>();
    let mut structural_results = internal_unit_calls
        .iter()
        .filter_map(|call| match call.structural_result.as_ref() {
            Some(result)
                if result.operation_result.multiplicity
                    == terminal_psi::StructuralMultiplicity::Affine
                    && result.operation_result.claims.is_empty()
                    && result.returned_claim_transfers.is_empty()
                    && result.returned_claims.is_empty() =>
            {
                Some((call.operation_ordinal, result.operation_result.place))
            }
            _ => None,
        })
        .chain(boundary_settlements.iter().filter_map(|settlement| {
            let BoundaryResultRecord::Structural(result) = &settlement.native_result else {
                return None;
            };
            (result.result.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                && result.result.claims.is_empty())
            .then_some((settlement.operation_ordinal, result.result.place))
        }))
        .collect::<Vec<_>>();
    structural_results.sort_by_key(|(operation_ordinal, _)| std::cmp::Reverse(*operation_ordinal));
    let structural_result_prefix = structural_results
        .into_iter()
        .map(|(_, place)| place)
        .filter(|place| !inspected_roots.contains(place))
        .filter(|place| !continuation_discards.contains(place))
        .collect::<Vec<_>>();
    let local_operations = cleanup
        .locals
        .iter()
        .map(|(operation, _, _)| *operation)
        .collect::<std::collections::BTreeSet<_>>();
    let expected_root_actions = structural_result_prefix
        .iter()
        .copied()
        .chain(expected_local_prefix.iter().copied())
        .chain(expected_parameter_suffix.iter().copied())
        .map(terminal_psi::TerminalAffineCleanupAction::DiscardRoot)
        .collect::<Vec<_>>();
    let expected_local_actions = structural_result_prefix
        .iter()
        .copied()
        .chain(expected_local_prefix.iter().copied())
        .map(terminal_psi::TerminalAffineCleanupAction::DiscardRoot)
        .collect::<Vec<_>>();
    CleanupRoots {
        expected_parameter_suffix,
        local_operations,
        expected_root_actions,
        expected_local_actions,
    }
}
