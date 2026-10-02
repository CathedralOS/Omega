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
    StructuralAccess, StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
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

/// One mutation inventory serves whole-root join pins and projected child
/// freezes. A field id stays attached until its declaring record is resolved;
/// losing that last step would freeze unrelated sibling fields.
struct MutationRegion<'a> {
    place: PlaceId,
    path: &'a [terminal_psi::StructuralPathSegment],
    field: Option<semantic_vocabulary::StructuralFieldId>,
}

fn mutation_regions(operation: &O) -> Vec<MutationRegion<'_>> {
    fn exclusive(arguments: &[terminal_psi::StructuralArgument]) -> Vec<MutationRegion<'_>> {
        arguments
            .iter()
            .filter(|argument| {
                matches!(
                    argument.access,
                    StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                )
            })
            .map(|argument| MutationRegion {
                place: argument.place,
                path: argument.path.as_slice(),
                field: None,
            })
            .collect::<Vec<_>>()
    }
    let region = |place, path, field| vec![MutationRegion { place, path, field }];
    match operation {
        O::CallStructural {
            structural_arguments,
            ..
        }
        | O::BoundaryCall {
            structural_arguments,
            ..
        } => exclusive(structural_arguments),
        O::CallDynamic {
            dispatch: abstract_operations::AbstractDynamicDispatch::Rebound(dispatch),
            ..
        } => exclusive(std::slice::from_ref(&dispatch.rebound.source)),
        O::EstablishReference { source, .. } | O::EstablishElementView { source, .. } => {
            exclusive(std::slice::from_ref(source))
        }
        O::PrimitiveLocalStore { destination, .. }
        | O::ByteSequenceWrite { destination, .. }
        | O::ElementViewWrite { destination, .. }
        | O::ByteSequenceStore { destination, .. } => region(*destination, &[], None),
        O::StructuralByteSequenceFieldByteStore {
            destination,
            path,
            field,
            ..
        }
        | O::StructuralByteSequenceFieldStore {
            destination,
            path,
            field,
            ..
        }
        | O::StructuralByteViewFieldStore {
            destination,
            path,
            field,
            ..
        } => region(*destination, path, Some(*field)),
        O::WriteOnlyPrimitiveStore {
            destination, path, ..
        } => region(destination.place(), path, None),
        O::StructuralScalarFieldStore {
            destination,
            path,
            field,
            ..
        } => region(destination.place(), path, Some(*field)),
        O::StoreStructuralField {
            destination,
            path,
            field,
            ..
        } => region(destination.place(), path, *field),
        O::MoveStructuralField {
            source,
            path,
            field,
            ..
        } => region(source.place(), path, *field),
        _ => Vec::new(),
    }
}

/// Resolve each mutation to the known projection it may disturb. A dynamic
/// selector or unresolved leaf freezes its known enclosing region, never an
/// invented disjoint sibling. Edge lending is checked per selected successor.
pub(super) fn written_or_exclusive_regions(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    operation: &O,
) -> Vec<(PlaceId, terminal_semantics::StorageRegion)> {
    mutation_regions(operation)
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
