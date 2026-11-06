//! Structural roles of the authenticated empty-run release operation.
//!
//! Core field order identifies the roles, not field-ID ordinals or layout
//! offsets. Execution still checks zero occupancy and transfers the actual
//! backing. A matching record shape alone never authorizes this operation.

use semantic_vocabulary::{IntegerSign, StructuralFieldId};
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryStructuralResultDeclaration,
    CheckedBoundaryExecution, StructuralAccess, StructuralFieldType, StructuralMultiplicity,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalIdentityIndex,
};

pub struct EmptyRunReleaseContract<'module> {
    pub storage: TerminalIdentityIndex,
    pub count: StructuralFieldId,
    pub length: StructuralFieldId,
    pub result: &'module BoundaryStructuralResultDeclaration,
}

pub fn empty_run_release_contract<'module>(
    types: &'module [StructuralTypeDeclaration],
    boundary: &'module BoundaryMachineDeclaration,
) -> Result<EmptyRunReleaseContract<'module>, &'static str> {
    if boundary.declaration.as_ref().map(|row| row.execution)
        != Some(CheckedBoundaryExecution::EmptyRunRelease)
        || !boundary.scalar_parameters.is_empty()
    {
        return Err("empty-run release has no authenticated core execution");
    }
    let [parameter] = boundary.structural_parameters.as_slice() else {
        return Err("empty-run release requires one owned run");
    };
    let BoundaryMachineResult::Structural(result) = &boundary.result else {
        return Err("empty-run release must return the existing extent");
    };
    if parameter.access != StructuralAccess::Owned
        || parameter.multiplicity != StructuralMultiplicity::Linear
        || result.multiplicity != StructuralMultiplicity::Linear
    {
        return Err("empty-run release changed its linear custody contract");
    }
    let run = terminal_psi::find_structural_type(types, parameter.structural_type)
        .ok_or("empty-run release lost its run carrier")?;
    let StructuralTypeShape::Record { fields, .. } = &run.shape else {
        return Err("empty-run release has no record descriptor");
    };
    let [storage, count] = fields.as_slice() else {
        return Err("empty-run release changed its descriptor fields");
    };
    if storage.field_type.as_structural() != Some(result.structural_type)
        || storage.relevance.is_erased()
        || count.relevance.is_erased()
        || !unsigned_count(&count.field_type)
    {
        return Err("empty-run release changed its backing or count carrier");
    }
    let extent = terminal_psi::find_structural_type(types, result.structural_type)
        .ok_or("empty-run release lost its extent carrier")?;
    let StructuralTypeShape::Record { fields, .. } = &extent.shape else {
        return Err("empty-run release has no extent descriptor");
    };
    let [_base, length] = fields.as_slice() else {
        return Err("empty-run release changed its extent fields");
    };
    if length.relevance.is_erased() || !unsigned_count(&length.field_type) {
        return Err("empty-run release changed its extent length carrier");
    }
    Ok(EmptyRunReleaseContract {
        storage: storage.identity,
        count: count.id,
        length: length.id,
        result,
    })
}

fn unsigned_count(field: &StructuralFieldType) -> bool {
    field
        .scalar_type()
        .and_then(|scalar| scalar.as_integer())
        .is_some_and(|integer| integer.sign() == IntegerSign::Unsigned && integer.bits() == 64)
}
