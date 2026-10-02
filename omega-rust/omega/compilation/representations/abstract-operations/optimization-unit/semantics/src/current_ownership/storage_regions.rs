//! Rejoin current-IR source contracts before resolving shared storage geometry.
//! The immutable Terminal snapshot cannot supply a rewritten root's type.

use crate::unit_validation::operation_contracts::structural_source_contract;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, PlaceId, StructuralFieldId, StructuralTypeId,
};
use std::collections::BTreeMap;
use terminal_psi::{StructuralPathSegment, StructuralTypeDeclaration};

pub(super) fn projected(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    place: PlaceId,
    path: &[StructuralPathSegment],
    field: Option<StructuralFieldId>,
) -> terminal_semantics::StorageRegion {
    structural_source_contract(function, place, false).map_or_else(
        || terminal_semantics::StorageRegion {
            path: Vec::new(),
            indirect: true,
        },
        |root| {
            terminal_semantics::projected_storage_region(
                types.values().copied(),
                root.structural_type,
                path,
                field,
            )
        },
    )
}

pub(super) fn canonical_field(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    place: PlaceId,
    path: &[CanonicalStructuralPathSegment],
    field: StructuralFieldId,
) -> Vec<StructuralPathSegment> {
    structural_source_contract(function, place, false).map_or_else(Vec::new, |root| {
        terminal_semantics::canonical_field_storage_region(
            types.values().copied(),
            root.structural_type,
            path,
            field,
        )
    })
}
