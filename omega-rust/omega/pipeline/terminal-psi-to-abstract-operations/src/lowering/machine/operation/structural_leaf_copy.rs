//! `StructuralLeafCopy` establishes one owned duplicate of an `Unrestricted`
//! leaf projected out of a live readable root. The verifier already proved
//! the read-only observation contract — copying never opens a restoration
//! window, so this leg re-checks only the static shape the verifier's
//! admission committed to: the fresh `Unrestricted` result row, a readable
//! source signature, and the spelled path landing on the exact leaf type.
//! The retained operation carries source place and path unchanged so
//! consumers replay the verifier's own resolution rather than a
//! lowering-side reconstruction.

use abstract_operations::AbstractOperation;
use semantic_vocabulary::{PlaceId, StructuralPlaceKind, StructuralTypeId};
use terminal_psi::{
    Operation, StructuralAccess, StructuralFieldType, StructuralMultiplicity,
    StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
};

use crate::lowering::LoweringError;

use super::borrowed_windows::{block_parameter, source_signature};

pub(super) fn lower(
    operation: &Operation,
    machine: &TerminalMachine,
    structural_types: &[StructuralTypeDeclaration],
    source: PlaceId,
    path: &[StructuralPathSegment],
) -> Result<AbstractOperation, LoweringError> {
    let invalid = || LoweringError::InvalidStructuralLeafCopy(operation.id);
    let result = operation.result.structural().ok_or_else(invalid)?;
    // The copy is fresh storage under Unrestricted multiplicity; anything
    // carrying custody, domain, or claim evidence rides a different contract.
    if result.multiplicity != StructuralMultiplicity::Unrestricted
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
        || !matches!(
            machine
                .structural_places
                .iter()
                .find(|place| place.id == result.place)
                .map(|place| place.kind),
            Some(StructuralPlaceKind::OperationResult {
                producer,
                structural_type,
            }) if producer == operation.id && structural_type == result.structural_type
        )
    {
        return Err(invalid());
    }
    // Observation requires readable access: a write-only borrow never opens.
    if machine
        .structural_parameters
        .iter()
        .chain(block_parameter(machine, source))
        .find(|parameter| parameter.place == source)
        .is_some_and(|parameter| parameter.access == StructuralAccess::WriteOnlyBorrow)
    {
        return Err(invalid());
    }
    let signature = source_signature(machine, source).ok_or_else(invalid)?;
    let selected_type = resolve_copyable_path(structural_types, signature.structural_type, path)
        .ok_or_else(invalid)?;
    if selected_type != result.structural_type {
        return Err(invalid());
    }
    Ok(AbstractOperation::StructuralLeafCopy {
        psi_operation: operation.id,
        result: result.clone(),
        source,
        path: path.to_vec(),
    })
}

/// Resolve the leaf's declared type beneath `root` through the spelled path,
/// mirroring the verifier's `resolve_structural_path`: field segments select
/// a non-erased record member whose structural or canonical leaf shape joins
/// the declaration roster, fixed indices step into in-bounds array elements,
/// and a runtime index steps into any fixed array's element: its bound is the
/// obligation the verifier discharged.
fn resolve_copyable_path(
    structural_types: &[StructuralTypeDeclaration],
    mut structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    for segment in path {
        let declaration = structural_types
            .iter()
            .find(|candidate| candidate.id == structural_type)?;
        structural_type = match (segment, &declaration.shape) {
            (StructuralPathSegment::Field(identity), StructuralTypeShape::Record { fields }) => {
                let field = fields.iter().find(|candidate| {
                    candidate.identity == *identity && !candidate.relevance.is_erased()
                })?;
                match &field.field_type {
                    StructuralFieldType::Structural(next) => *next,
                    leaf => {
                        let shape = leaf.canonical_leaf_shape()?;
                        structural_types
                            .iter()
                            .find(|candidate| candidate.shape == shape)
                            .map(|candidate| candidate.id)?
                    }
                }
            }
            (
                StructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray { element, length },
            ) if index < length => *element,
            (
                StructuralPathSegment::RuntimeIndex { .. },
                StructuralTypeShape::FixedArray { element, .. },
            ) => *element,
            _ => return None,
        };
    }
    Some(structural_type)
}
