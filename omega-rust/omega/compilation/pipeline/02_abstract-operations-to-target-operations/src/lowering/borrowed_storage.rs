//! Capture native scalar-leaf geometry beside target lowering, before its
//! inputs are consumed. This is the same shape/address owner as native calls
//! and field accesses, not a second layout implementation in a runtime adapter.
//! A record with non-scalar storage has no complete scalar snapshot recipe.

use super::structural_layout;
use abstract_operations::AbstractOperationPlan;
use runtime_abi::{NativeBorrowedStorageLayout, NativeStorageRepetition};
use semantic_vocabulary::{CanonicalStructuralPathSegment as Path, ScalarType, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    StructuralAccess, StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape,
};

pub(super) fn capture(plan: &AbstractOperationPlan) -> Vec<NativeBorrowedStorageLayout> {
    let declarations: BTreeMap<_, _> = plan
        .structural_types
        .iter()
        .map(|row| (row.id, row))
        .collect();
    let requested: BTreeSet<_> = plan
        .boundary_machines
        .iter()
        .flat_map(|boundary| &boundary.structural_parameters)
        .filter(|parameter| {
            matches!(
                parameter.access,
                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
            )
        })
        .map(|parameter| parameter.structural_type)
        .collect();
    requested
        .into_iter()
        .filter_map(|root| {
            let shape = structural_layout::structural_shape(
                root,
                &declarations,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
            )
            .ok()?;
            let mut fields = Vec::new();
            collect(
                root,
                0,
                None,
                &mut Vec::new(),
                &mut Vec::new(),
                &declarations,
                &mut fields,
            )?;
            Some(NativeBorrowedStorageLayout::new(
                root,
                shape.byte_size,
                shape.alignment,
                fields,
            ))
        })
        .collect()
}

type Field = (Vec<Path>, Vec<NativeStorageRepetition>, u32, ScalarType);

fn collect(
    root: StructuralTypeId,
    offset: u32,
    occurrence_stride: Option<u32>,
    path: &mut Vec<Path>,
    repetitions: &mut Vec<NativeStorageRepetition>,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    fields: &mut Vec<Field>,
) -> Option<()> {
    match &declarations.get(&root)?.shape {
        StructuralTypeShape::PrimitiveScalar(scalar) => {
            fields.push((path.clone(), repetitions.clone(), offset, *scalar))
        }
        StructuralTypeShape::FusedServiceBinding => {}
        StructuralTypeShape::Record {
            fields: members, ..
        } => {
            for member in members.iter().filter(|field| !field.relevance.is_erased()) {
                if matches!(member.field_type, StructuralFieldType::Erased { .. }) {
                    continue;
                }
                path.push(Path::Field(member.id));
                if matches!(member.field_type, StructuralFieldType::Atomic(_)) {
                    return None;
                }
                if let Some(scalar) = member.field_type.scalar_type() {
                    let (displacement, _) = structural_layout::direct_scalar_field_geometry(
                        root,
                        member.id,
                        scalar,
                        declarations,
                    )
                    .ok()?;
                    if matches!(
                        member.storage,
                        Some(semantic_vocabulary::AuthoredFieldStorage::Fragmented(_))
                    ) {
                        return None;
                    }
                    let stored = member
                        .storage
                        .as_ref()
                        .and_then(|storage| storage.stored_scalar())
                        .unwrap_or(scalar);
                    fields.push((
                        path.clone(),
                        repetitions.clone(),
                        offset.checked_add(displacement)?,
                        stored,
                    ));
                } else if let StructuralFieldType::Structural(child) = member.field_type {
                    let (_, _, displacement) =
                        structural_layout::resolve_structural_projection_path(
                            root,
                            &[StructuralPathSegment::Field(member.identity.clone())],
                            declarations,
                            &mut BTreeMap::new(),
                            &mut BTreeSet::new(),
                        )
                        .ok()?;
                    let stride = member
                        .storage
                        .as_ref()
                        .and_then(semantic_vocabulary::AuthoredFieldStorage::element_stride)
                        .map(u32::try_from)
                        .transpose()
                        .ok()?;
                    collect(
                        child,
                        offset.checked_add(displacement)?,
                        stride,
                        path,
                        repetitions,
                        declarations,
                        fields,
                    )?;
                } else {
                    return None;
                }
                path.pop();
            }
        }
        StructuralTypeShape::FixedArray { element, length } => {
            let (_, stride) = structural_layout::root_array_projection_metadata(
                root,
                declarations,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
            )
            .ok()?;
            let stride = occurrence_stride.or(stride)?;
            repetitions.push(NativeStorageRepetition {
                path_position: path.len(),
                length: *length,
                byte_stride: stride,
            });
            path.push(Path::FixedIndex(0));
            collect(
                *element,
                offset,
                None,
                path,
                repetitions,
                declarations,
                fields,
            )?;
            path.pop();
            repetitions.pop();
        }
        _ => return None,
    }
    Some(())
}
