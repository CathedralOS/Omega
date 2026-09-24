//! Borrowed-storage restoration windows transported into abstract custody.
//!
//! `MoveStructuralField` vacates one declared structural field beneath a
//! mutable-borrowed machine parameter; `StoreStructuralField` reseats exactly
//! that vacancy with an already-owned subtree of the declared type. The
//! Terminal verifier already proved the dynamic invariant — the move opens
//! restoration debt keyed by the hole's canonical field path, no operation,
//! edge, or case dispatch may observe the absent subtree, and every
//! non-crash exit leaves the debt empty. This leg re-checks the static shape
//! the verifier's admission committed to — root authority, declared field
//! type, and the exact result/value custody rows — and retains the complete
//! parameter declaration so no consumer reconstructs borrow authority from
//! physical ABI shape. The dynamic debt itself needs no replay here: the
//! spelled pairs reach Omega only through a verified module.

use abstract_operations::AbstractOperation;
use semantic_vocabulary::{PlaceId, StructuralFieldId, StructuralPlaceKind, StructuralTypeId};
use terminal_psi::{
    Operation, OperationKind, OperationResult, StructuralAccess, StructuralArgument,
    StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
};

use crate::lowering::LoweringError;

use super::structural_scalar_fields::{exact_parameter, has_empty_structural_custody};

pub(super) fn lower(
    operation: &Operation,
    machine: &TerminalMachine,
    structural_types: &[StructuralTypeDeclaration],
) -> Result<AbstractOperation, LoweringError> {
    match &operation.kind {
        OperationKind::MoveStructuralField {
            source,
            path,
            field,
        } => lower_move(operation, machine, structural_types, *source, path, *field),
        OperationKind::StoreStructuralField {
            destination,
            path,
            field,
            value,
        } => lower_store(
            operation,
            machine,
            structural_types,
            *destination,
            path,
            *field,
            value,
        ),
        _ => unreachable!("borrowed-window router is exhaustive"),
    }
}

fn lower_move(
    operation: &Operation,
    machine: &TerminalMachine,
    structural_types: &[StructuralTypeDeclaration],
    source: PlaceId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
) -> Result<AbstractOperation, LoweringError> {
    let invalid = || LoweringError::InvalidBorrowedStorageWindow(operation.id);
    let result = operation.result.structural().ok_or_else(invalid)?;
    // The moved subtree is a fresh operation result under exact-once custody:
    // no shared/linear multiplicity, no qualifications, and no pre-attached
    // claims can ride the extraction.
    if !matches!(
        result.multiplicity,
        StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
    ) || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
    {
        return Err(invalid());
    }
    if !matches!(
        machine
            .structural_places
            .iter()
            .find(|place| place.id == result.place)
            .map(|place| place.kind),
        Some(StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        }) if producer == operation.id && structural_type == result.structural_type
    ) {
        return Err(invalid());
    }
    let source = borrowed_window_root(machine, source).ok_or_else(invalid)?;
    if declared_window_field_type(structural_types, source.structural_type, path, field)
        != Some(result.structural_type)
    {
        return Err(invalid());
    }
    Ok(AbstractOperation::MoveStructuralField {
        psi_operation: operation.id,
        result: result.clone(),
        source,
        path: path.to_vec(),
        field,
    })
}

fn lower_store(
    operation: &Operation,
    machine: &TerminalMachine,
    structural_types: &[StructuralTypeDeclaration],
    destination: PlaceId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
    value: &StructuralArgument,
) -> Result<AbstractOperation, LoweringError> {
    let invalid = || LoweringError::InvalidBorrowedStorageWindow(operation.id);
    if operation.result != OperationResult::Unit
        || value.access != StructuralAccess::Owned
        || !value.path.is_empty()
    {
        return Err(invalid());
    }
    let destination = borrowed_window_root(machine, destination).ok_or_else(invalid)?;
    let Some(field_type) =
        declared_window_field_type(structural_types, destination.structural_type, path, field)
    else {
        return Err(invalid());
    };
    let Some(source) = source_signature(machine, value.place) else {
        return Err(invalid());
    };
    if source.structural_type != field_type || !source.has_empty_qualifications() {
        return Err(invalid());
    }
    Ok(AbstractOperation::StoreStructuralField {
        psi_operation: operation.id,
        destination,
        path: path.to_vec(),
        field,
        value: value.clone(),
    })
}

/// The borrowed machine parameter a window operation may name: mutable-borrow
/// authority, a transferable multiplicity, and no qualifications or entry
/// claims, resolved through the exact parameter row so the abstract operation
/// carries the authority the verifier admitted rather than a bare place.
fn borrowed_window_root(
    machine: &TerminalMachine,
    place: PlaceId,
) -> Option<StructuralParameterDeclaration> {
    let parameter = exact_parameter(machine, place)?;
    if parameter.access != StructuralAccess::MutableBorrow
        || !matches!(
            parameter.multiplicity,
            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
        )
        || !has_empty_structural_custody(machine, place)
    {
        return None;
    }
    Some(parameter)
}

/// The declared `Structural` type of the field a window operation names,
/// resolved beneath `root` through the spelled path. `None` means the place
/// does not host a structural field there.
fn declared_window_field_type(
    structural_types: &[StructuralTypeDeclaration],
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
) -> Option<StructuralTypeId> {
    let parent_type = resolve_spelled_path(structural_types, root, path)?;
    let declaration = structural_types
        .iter()
        .find(|candidate| candidate.id == parent_type)?;
    let fields = match &declaration.shape {
        StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. } => {
            fields
        }
        _ => return None,
    };
    match fields
        .iter()
        .find(|candidate| candidate.id == field && !candidate.relevance.is_erased())?
        .field_type
    {
        StructuralFieldType::Structural(field_type) => Some(field_type),
        _ => None,
    }
}

/// Resolve one name-spelled path beneath `root`: field segments select a
/// non-erased record/mixed member that must itself be structural, fixed
/// indices step into in-bounds array elements, and anything else cannot be a
/// window carrier.
pub(super) fn resolve_spelled_path(
    structural_types: &[StructuralTypeDeclaration],
    mut structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    for segment in path {
        let declaration = structural_types
            .iter()
            .find(|candidate| candidate.id == structural_type)?;
        structural_type = match (segment, &declaration.shape) {
            (
                StructuralPathSegment::Field(identity),
                StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. },
            ) => {
                let field = fields.iter().find(|candidate| {
                    candidate.identity == *identity && !candidate.relevance.is_erased()
                })?;
                let StructuralFieldType::Structural(next) = field.field_type else {
                    return None;
                };
                next
            }
            (
                StructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray { element, length },
            ) if index < length => *element,
            _ => return None,
        };
    }
    Some(structural_type)
}

/// The declared signature a spelled structural place carries: machine
/// parameter, block parameter resolved through its place row, then operation
/// result — the same order the verifier's `source_signature` resolves.
pub(super) struct SourceSignature {
    pub(super) structural_type: StructuralTypeId,
    qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
    projected_qualifications: Vec<terminal_psi::StructuralPathQualification>,
}

impl SourceSignature {
    fn has_empty_qualifications(&self) -> bool {
        self.qualifications.is_empty() && self.projected_qualifications.is_empty()
    }
}

pub(super) fn source_signature(
    machine: &TerminalMachine,
    place: PlaceId,
) -> Option<SourceSignature> {
    if let Some(parameter) = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == place)
    {
        return Some(SourceSignature {
            structural_type: parameter.structural_type,
            qualifications: parameter.qualifications.clone(),
            projected_qualifications: parameter.projected_qualifications.clone(),
        });
    }
    if let Some(parameter) = block_parameter(machine, place) {
        return Some(SourceSignature {
            structural_type: parameter.structural_type,
            qualifications: parameter.qualifications.clone(),
            projected_qualifications: parameter.projected_qualifications.clone(),
        });
    }
    machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| {
            operation
                .result
                .structural()
                .filter(|result| result.place == place)
                .map(|result| SourceSignature {
                    structural_type: result.structural_type,
                    qualifications: result.qualifications.clone(),
                    projected_qualifications: result.projected_qualifications.clone(),
                })
        })
}

/// Resolve one block-structural parameter through its `BlockParameter` place
/// row: the row's block and position select the declaration, which must
/// still spell the same place and position and cannot be `self`.
pub(super) fn block_parameter(
    machine: &TerminalMachine,
    place: PlaceId,
) -> Option<&StructuralParameterDeclaration> {
    let declaration = machine
        .structural_places
        .iter()
        .find(|row| row.id == place)?;
    let StructuralPlaceKind::BlockParameter { block, position } = declaration.kind else {
        return None;
    };
    machine
        .blocks
        .iter()
        .find(|candidate| candidate.id == block)?
        .structural_parameters
        .get(position as usize)
        .filter(|parameter| {
            parameter.place == place && parameter.position == position && !parameter.is_self
        })
}
