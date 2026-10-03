//! Current-CFG custody of a bounded field's live-length observation.
//! Original Terminal provenance cannot authorize a stale observation after a rewrite.
use super::structural_freshness::{structural_metadata_may_change, unchanged_since};
use crate::OptimizationUnitValidationError;
use abstract_operations::AbstractOperation as O;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{BlockId, PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    StructuralAccess, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
};

#[cfg(test)]
mod tests;

pub(super) fn validate(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> Result<(), OptimizationUnitValidationError> {
    for (block_position, block) in function.blocks.iter().enumerate() {
        for (node_position, node) in block.nodes.iter().enumerate() {
            // A byte store and a byte read both bound their index by the
            // field's live length, so both need that exact observation current.
            let (destination, path, field, length, reads) = match &node.operation {
                O::StructuralByteSequenceFieldByteStore {
                    destination,
                    path,
                    field,
                    length,
                    ..
                } => (destination, path, field, length, false),
                O::StructuralByteSequenceFieldRead {
                    source,
                    path,
                    field,
                    length,
                    ..
                } => (source, path, field, length, true),
                _ => continue,
            };
            let node = u32::try_from(node_position).map_err(|_| {
                OptimizationUnitValidationError::StructuralCatalogMismatch {
                    machine: Some(function.machine),
                }
            })?;
            let invalid = if reads {
                OptimizationUnitValidationError::InvalidByteSequenceRead {
                    machine: function.machine,
                    block: block.id,
                    node,
                }
            } else {
                OptimizationUnitValidationError::InvalidByteSequenceWrite {
                    machine: function.machine,
                    block: block.id,
                    node,
                }
            };
            let exact_path = field_path(function, types, *destination, path, *field)
                .ok_or_else(|| invalid.clone())?;
            let names = crate::unit_validation::arrival_storage::storage_names(
                function,
                block.id,
                *destination,
            );
            let (producer, producer_root) = function.blocks.iter().enumerate().find_map(|(owner, block)| {
                names.in_block(block.id).find_map(|name| {
                    block.nodes.iter().position(|candidate| matches!(&candidate.operation,
                        O::StructuralByteSequenceFieldLength { source, path: measured_path, field: measured_field, result, .. }
                        if *source == name && terminal_semantics::structural_paths_select_same_location(measured_path, path) && measured_field == field && result.value == *length
                    )).map(|position| ((owner, position), name))
                })
            }).ok_or_else(|| invalid.clone())?;
            if !unchanged_since(
                function,
                predecessors,
                producer,
                producer_root,
                (block_position, node_position),
                *destination,
                |operation, root| {
                    structural_metadata_may_change(function, types, root, &exact_path, operation)
                },
            ) {
                return Err(invalid);
            }
        }
    }
    Ok(())
}

/// A dynamic view has no declared extent. Even literal selections retain a
/// runtime-index certificate; replacing that segment by FixedIndex erases
/// evidence and cannot preserve the current operation contract.
pub(super) fn field_carrier(
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    for (position, segment) in path.iter().enumerate() {
        if matches!(segment, StructuralPathSegment::FixedIndex(_)) {
            let parent = super::super::structural_catalog::resolve_structural_path(
                types,
                root,
                &path[..position],
            )?;
            if matches!(
                types.get(&parent)?.shape,
                StructuralTypeShape::ElementView { .. }
            ) {
                return None;
            }
        }
    }
    super::super::structural_catalog::resolve_structural_path(types, root, path)
}

pub(super) fn field_path(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    root: PlaceId,
    path: &[StructuralPathSegment],
    field: semantic_vocabulary::StructuralFieldId,
) -> Option<Vec<StructuralPathSegment>> {
    let (structural_type, _) = byte_field_root(function, root)?;
    let (storage_type, storage_path) =
        if matches!(path.first(), Some(StructuralPathSegment::Referent)) {
            super::super::references::mutable_record_field_destination(function, types, root, path)?
        } else {
            (structural_type, path)
        };
    let carrier = field_carrier(types, storage_type, storage_path)?;
    let StructuralTypeShape::Record { fields } = &types.get(&carrier)?.shape else {
        return None;
    };
    let selected = fields
        .iter()
        .find(|candidate| candidate.id == field && !candidate.relevance.is_erased())?;
    let mut exact = path.to_vec();
    exact.push(StructuralPathSegment::Field(selected.identity.clone()));
    Some(exact)
}

/// Recover declaration custody, not availability or permission to read contents.
/// The catalog validates producer identity; the CFG ownership walk separately
/// requires a live, complete owner and rejects suspended loans at each use.
pub fn byte_field_root(
    function: &PsiOptimizationFunction,
    root: PlaceId,
) -> Option<(StructuralTypeId, StructuralAccess)> {
    use semantic_vocabulary::StructuralPlaceKind;
    use terminal_psi::StructuralMultiplicity;

    let place = function
        .structural_places
        .iter()
        .find(|place| place.id == root)?;
    if !matches!(
        place.kind,
        StructuralPlaceKind::Parameter { .. }
            | StructuralPlaceKind::BlockParameter { .. }
            | StructuralPlaceKind::OperationResult { .. }
    ) || function
        .entry_claim_declarations
        .iter()
        .any(|claim| claim.input == root)
        || function
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == root)
    {
        return None;
    }
    let source =
        crate::unit_validation::operation_contracts::structural_source_contract(function, root)?;
    (source.is_unqualified()
        && !source.has_result_evidence()
        && matches!(
            source.multiplicity,
            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
        ))
    .then_some((source.structural_type, source.access))
}

pub(super) fn overlaps(left: &[StructuralPathSegment], right: &[StructuralPathSegment]) -> bool {
    terminal_semantics::structural_paths_may_overlap(left, right)
}
