/*
The ownership facts the descriptor and reference graphs read from a function's
current operations: whether a call's argument storage is closed, which storage
cannot retain a loan, and the dominators of each reachable block. Live
ownership-frontier replay and the address-range scalar ordering
(`RangeRelations`) used to live here as well; they served only the native
abstract-operations optimizer. The replay is archived with it, at this file's
path under `_recycling/`; the ordering is in git history.
*/

mod call_accesses {

    use super::storage_regions::storage_cannot_retain_loan;

    use crate::unit_validation::structural_catalog::resolve_structural_path;

    use semantic_vocabulary::StructuralTypeId;
    use std::collections::BTreeMap;
    use terminal_psi::{
        StructuralAccess, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
    };

    pub(crate) fn argument_storage_is_closed(
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        source: StructuralTypeId,
        path: &[StructuralPathSegment],
        access: StructuralAccess,
        established: &mut BTreeMap<StructuralTypeId, bool>,
    ) -> bool {
        let Some(selected) = resolve_structural_path(types, source, path) else {
            return false;
        };
        if path.is_empty() && access != StructuralAccess::Owned {
            match types.get(&selected).map(|declaration| &declaration.shape) {
                Some(StructuralTypeShape::ByteSequence(carrier)) if carrier.is_borrowed() => {
                    return true;
                }
                Some(StructuralTypeShape::ElementView { element, .. }) => {
                    return storage_cannot_retain_loan(types, *element, established);
                }
                _ => {}
            }
        }
        storage_cannot_retain_loan(types, selected, established)
    }
}
pub(crate) use call_accesses::argument_storage_is_closed;
mod storage_regions {

    use semantic_vocabulary::StructuralTypeId;
    use std::collections::BTreeMap;
    use terminal_psi::{
        StructuralFieldDeclaration, StructuralFieldType, StructuralTypeDeclaration,
        StructuralTypeShape,
    };

    pub(crate) fn storage_cannot_retain_loan(
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
            StructuralTypeShape::PrimitiveScalar(_) | StructuralTypeShape::AtomicCell(_) => true,
            StructuralTypeShape::ByteSequence(carrier) => !carrier.is_borrowed(),
            StructuralTypeShape::Record { fields, .. } => {
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
            StructuralTypeShape::TypeParameter
            | StructuralTypeShape::Opaque
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
            | StructuralFieldType::AtomicBoundedInteger(_)
            | StructuralFieldType::RefinedScalar(_)
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
}
