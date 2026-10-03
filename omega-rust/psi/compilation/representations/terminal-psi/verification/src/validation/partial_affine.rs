//! Reconstruct maximal residual subtrees from finite record/array move paths.

use super::{
    BTreeSet, OperationKind, PlaceId, StructuralAccess, StructuralMultiplicity,
    StructuralPathSegment, StructuralPlaceKind, StructuralTypeId, TerminalMachine, TerminalModule,
    resolve_structural_path,
};
/// Resolve custody from its real owner, independently of any projected use.
/// Result declarations alone do not establish an affine value or its producer.
pub(super) fn partial_affine_root_type(
    machine: &TerminalMachine,
    place: PlaceId,
) -> Option<StructuralTypeId> {
    if machine
        .entry_claims
        .iter()
        .any(|claim| claim.input == place)
        || machine
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == place)
    {
        return None;
    }
    if let Some(parameter) = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == place)
        .or_else(|| super::block_views::parameter(machine, place))
    {
        return (parameter.multiplicity == StructuralMultiplicity::Affine
            && parameter.access == StructuralAccess::Owned
            && parameter.qualifications.is_empty()
            && parameter.projected_qualifications.is_empty())
        .then_some(parameter.structural_type);
    }
    let declaration = machine
        .structural_places
        .iter()
        .find(|declaration| declaration.id == place)?;
    let StructuralPlaceKind::OperationResult {
        producer,
        structural_type,
    } = declaration.kind
    else {
        return None;
    };
    let operation = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == producer)?;
    if !matches!(
        operation.kind,
        OperationKind::CallStructuralWithScalarArguments { .. }
            | OperationKind::BoundaryCall { .. }
            // A constructed record reaches its partial-affine lane when one of
            // its affine children is moved out through a projected successor
            // argument (for example an owned match arm selecting `pair.first`);
            // the leftover complement is discarded as residuals on that edge.
            | OperationKind::EstablishStructuralArray { .. }
        | OperationKind::EstablishRecord { .. }
            | OperationKind::EstablishStructuralCase { .. }
    ) {
        return None;
    }
    let result = operation.result.structural()?;
    (result.place == place
        && result.structural_type == structural_type
        && result.multiplicity == StructuralMultiplicity::Affine
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty())
    .then_some(structural_type)
}

pub(super) fn is_partial_affine_path(
    module: &TerminalModule,
    root_type: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> bool {
    !path.is_empty() && resolve_structural_path(module, root_type, path).is_some()
}

/// Custody and residual disposal remain receiver judgments. Only the finite
/// declaration geometry is shared with the producer.
pub(super) fn partial_affine_residuals(
    module: &TerminalModule,
    root_type: StructuralTypeId,
    moved_paths: &BTreeSet<Vec<StructuralPathSegment>>,
    max_residuals: usize,
) -> Option<Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>> {
    terminal_semantics::partial_affine_residuals(
        &module.structural_types,
        root_type,
        moved_paths,
        max_residuals,
    )
}
