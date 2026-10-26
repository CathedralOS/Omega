//! Unknown entry contents have exact value identity without an invented scalar
//! value. These query-local records never enter arithmetic premises or grant
//! access. The ordinary payload worklist alone copies, retires and meets them.
use super::super::{PayloadEntryScalarIdentity, PayloadScalarObservation};
use crate::descriptor_lifetimes::DescriptorPathSegment;
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use terminal_psi::{
    StructuralAccess, StructuralParameterDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape,
};

pub(super) fn collect(
    types: &[StructuralTypeDeclaration],
    parameters: &[StructuralParameterDeclaration],
) -> Vec<(PlaceId, Vec<PayloadScalarObservation>)> {
    parameters
        .iter()
        .filter_map(|parameter| {
            if parameter.access != StructuralAccess::Owned {
                return None;
            }
            let snapshots = for_parameter(types, parameter.place, parameter.structural_type)?;
            Some((parameter.place, snapshots))
        })
        .collect()
}

fn for_parameter(
    types: &[StructuralTypeDeclaration],
    root: PlaceId,
    structural_type: StructuralTypeId,
) -> Option<Vec<PayloadScalarObservation>> {
    let mut pending = vec![(structural_type, Vec::new(), Vec::new())];
    let mut snapshots = Vec::new();
    let mut remaining = 4096usize;
    while let Some((carrier, path, ancestors)) = pending.pop() {
        remaining = remaining.checked_sub(1)?;
        if ancestors.len() > 64 || ancestors.contains(&carrier) {
            return None;
        }
        let declaration = terminal_psi::find_structural_type(types, carrier)?;
        let mut lineage = ancestors;
        lineage.push(carrier);
        let mut append = |path: Vec<DescriptorPathSegment>, scalar_type| {
            snapshots.push(PayloadScalarObservation {
                path: path.clone(),
                value: None,
                entry: Some(PayloadEntryScalarIdentity {
                    root,
                    path,
                    scalar_type,
                }),
            });
        };
        if let StructuralTypeShape::PrimitiveScalar(scalar_type) = declaration.shape
            && super::super::payload_predicates::observation_type(types, structural_type, &path)
                == Some(scalar_type)
        {
            append(path.clone(), scalar_type);
        }
        if let Some(fields) = declaration.shape.direct_fields() {
            for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                let mut selected = path.clone();
                selected.push(DescriptorPathSegment::Field(field.id));
                if let Some(child) = field.field_type.as_structural() {
                    pending.push((child, selected, lineage.clone()));
                } else if let Some(scalar_type) = super::super::payload_predicates::observation_type(
                    types,
                    structural_type,
                    &selected,
                ) {
                    append(selected, scalar_type);
                }
            }
        }
        if let StructuralTypeShape::FixedArray { element, length } = declaration.shape {
            if length > remaining as u64 {
                return None;
            }
            for index in 0..length {
                let mut selected = path.clone();
                selected.push(DescriptorPathSegment::FixedIndex(index));
                pending.push((element, selected, lineage.clone()));
            }
        }
        // References/views are stored permissions, not owned backing values.
        // Variant-specific payloads need an independently selected case cut.
        // Neither acquires an entry value by this storage-only enumeration.
    }
    Some(snapshots)
}
