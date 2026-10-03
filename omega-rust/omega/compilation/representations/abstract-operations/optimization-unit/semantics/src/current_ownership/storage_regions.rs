//! Rejoin current-IR source contracts before resolving shared storage geometry.
//! The immutable Terminal snapshot cannot supply a rewritten root's type.

use crate::unit_validation::operation_contracts::structural_source_contract;
use abstract_operations::AbstractOperation as O;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, PlaceId, StructuralFieldId, StructuralTypeId,
};
use std::collections::BTreeMap;
use terminal_psi::{
    StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape,
};

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
            let mut region = terminal_semantics::projected_storage_region(
                types.values().copied(),
                root.structural_type,
                path,
                field,
            );
            // Keep the first dynamic selector for current-graph range replay.
            // Later suffixes remain conservative; no selector grants authority.
            if !region.indirect
                && let Some(position) = path.iter().position(|segment| {
                    matches!(segment, StructuralPathSegment::RuntimeIndex { .. })
                })
            {
                region.path = path[..=position].to_vec();
            }
            region
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

/// Resolve each mutation to the known projection it may disturb. A dynamic
/// selector or unresolved leaf freezes its known enclosing region, never an
/// invented disjoint sibling. Edge lending is checked per selected successor.
pub(super) fn written_or_exclusive_regions(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    operation: &O,
) -> Vec<(PlaceId, terminal_semantics::StorageRegion)> {
    operation
        .storage_mutations()
        .into_iter()
        .map(|region| {
            (
                region.place,
                projected(function, types, region.place, region.path, region.field),
            )
        })
        .collect()
}

/// This is a storage property, not a constructor whitelist. Every case and
/// nested element must be closed owned data; missing or recursive declarations
/// cannot prove absence of an alias. Mark an in-progress declaration false so
/// cycles fail conservatively. Reuse the result across this function's calls;
/// storage declarations do not change while the descriptor flow is built.
pub(super) fn storage_cannot_retain_loan(
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    structural_type: StructuralTypeId,
    established: &mut BTreeMap<StructuralTypeId, bool>,
) -> bool {
    if let Some(result) = established.get(&structural_type) {
        return *result;
    }
    established.insert(structural_type, false);
    let Some(declaration) = types.get(&structural_type) else {
        return false;
    };
    let result = match &declaration.shape {
        StructuralTypeShape::PrimitiveScalar(_) => true,
        StructuralTypeShape::ByteSequence(carrier) => !carrier.is_borrowed(),
        StructuralTypeShape::Record { fields } => {
            fields_cannot_retain_loan(types, fields, established)
        }
        StructuralTypeShape::FixedArray { element, .. } => {
            storage_cannot_retain_loan(types, *element, established)
        }
        StructuralTypeShape::Sum { cases } => cases
            .iter()
            .all(|case| fields_cannot_retain_loan(types, &case.fields, established)),
        StructuralTypeShape::Mixed { fields, cases } => {
            fields_cannot_retain_loan(types, fields, established)
                && cases
                    .iter()
                    .all(|case| fields_cannot_retain_loan(types, &case.fields, established))
        }
        StructuralTypeShape::FusedServiceBinding
        | StructuralTypeShape::Reference { .. }
        | StructuralTypeShape::ElementView { .. } => false,
    };
    established.insert(structural_type, result);
    result
}

fn fields_cannot_retain_loan(
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    fields: &[StructuralFieldDeclaration],
    established: &mut BTreeMap<StructuralTypeId, bool>,
) -> bool {
    fields.iter().all(|field| match &field.field_type {
        StructuralFieldType::Scalar(_)
        | StructuralFieldType::Atomic(_)
        | StructuralFieldType::BoundedInteger(_)
        | StructuralFieldType::IeeeFloat(_) => true,
        StructuralFieldType::ByteSequence(carrier)
        | StructuralFieldType::PredicateByteSequence { carrier, .. } => !carrier.is_borrowed(),
        StructuralFieldType::Structural(child) => {
            storage_cannot_retain_loan(types, *child, established)
        }
        StructuralFieldType::Erased { .. } => field.relevance.is_erased(),
    })
}
