//! Scalar observations consume owned byte geometry, not byte-recast permission.
//! Authored record plans provide offsets and codecs directly. Unplanned value
//! carriers delegate to the existing natural representation normalizer. Neither
//! route crosses a stored reference or invents geometry for an unavailable plan.

use crate::{RecastRuntimeIndexGeometry, RecastScalarGeometry, RecastScalarProjection};
use semantic_vocabulary::{AuthoredFieldStorage, ScalarType};
use symbols::SymbolHandle;
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StoragePathSegment {
    Field(SymbolHandle),
    Element(usize),
    RuntimeElement(checked_trees::CheckedRuntimeIndex),
}

pub(crate) fn project_scalar(
    program: &dyn crate::CheckingInput,
    root: TypeReferenceHandle,
    path: &[StoragePathSegment],
    mut natural: impl FnMut(TypeReferenceHandle, &[StoragePathSegment]) -> Option<RecastScalarGeometry>,
) -> Option<RecastScalarGeometry> {
    // Only the operation's root borrow is presentation. A reference reached
    // through an inline field is a descriptor, not another backing root.
    let root = crate::value_custody::places::assignment_value_type(program, root);
    project(program, root, path, &mut natural)
}

fn project(
    program: &dyn crate::CheckingInput,
    root: TypeReferenceHandle,
    path: &[StoragePathSegment],
    natural: &mut impl FnMut(TypeReferenceHandle, &[StoragePathSegment]) -> Option<RecastScalarGeometry>,
) -> Option<RecastScalarGeometry> {
    let root = program
        .type_reference_table
        .unconstrained_type_reference(root);
    if let Some(plan) = program.plan_laid_layout_for_type(root) {
        let geometry =
            plan.geometry_for_target(program.typed_program(), program.layout_target())?;
        let data = program.data_field_definition(root)?;
        let fields = program
            .live_data_fields(program.data_members(data))
            .collect::<Vec<_>>();
        if fields
            .iter()
            .map(|field| field.symbol)
            .ne(plan.field_symbols.iter().copied())
            || fields.len() != geometry.offsets.len()
        {
            return None;
        }
        let (StoragePathSegment::Field(symbol), remaining) = path.split_first()? else {
            return None;
        };
        let mut selected = fields
            .iter()
            .enumerate()
            .filter(|(_, field)| field.symbol == *symbol);
        let (index, field) = selected.next()?;
        if selected.next().is_some() {
            return None;
        }
        let primitive = program.primitive_type_reference(field.type_reference);
        let storage = geometry.authored_field_storage(index, primitive.is_some())?;
        let (offset, mut child) = match storage {
            AuthoredFieldStorage::Fragmented(_) => return None,
            AuthoredFieldStorage::Scalar {
                byte_offset,
                stored_scalar: Some(ScalarType::Integer(integer)),
            } => {
                if !remaining.is_empty() || integer.bits() == 0 || integer.bits() % 8 != 0 {
                    return None;
                }
                (
                    usize::try_from(byte_offset).ok()?,
                    RecastScalarGeometry {
                        projection: RecastScalarProjection {
                            offset: 0,
                            footprint: usize::from(integer.bits() / 8),
                            primitive: primitive?,
                            stored_integer: Some(integer),
                        },
                        indices: Vec::new(),
                    },
                )
            }
            AuthoredFieldStorage::Scalar {
                byte_offset,
                stored_scalar: None,
            }
            | AuthoredFieldStorage::Direct { byte_offset } => {
                let mut child = project(program, field.type_reference, remaining, natural)?;
                shift_indices(&mut child, 1)?;
                (usize::try_from(byte_offset).ok()?, child)
            }
            AuthoredFieldStorage::Repeated {
                byte_offset,
                element_stride,
            } => {
                let array = program
                    .type_reference_table
                    .unconstrained_type_reference(field.type_reference);
                let TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::Literal(extent),
                } = program.type_reference_table.type_reference(array)
                else {
                    return None;
                };
                let (selector, rest) = remaining.split_first()?;
                let (selected, runtime) = array_selector(selector, *extent)?;
                let stride = usize::try_from(element_stride).ok()?;
                let mut child = project(program, *element_type, rest, natural)?;
                if child.projection.footprint > stride {
                    return None;
                }
                shift_indices(&mut child, 2)?;
                if runtime {
                    child.indices.insert(
                        0,
                        RecastRuntimeIndexGeometry {
                            path_index: 1,
                            stride,
                            extent: *extent,
                        },
                    );
                }
                let offset = usize::try_from(byte_offset)
                    .ok()?
                    .checked_add(selected.checked_mul(stride)?)?;
                (offset, child)
            }
            _ => return None,
        };
        child.projection.offset = offset.checked_add(child.projection.offset)?;
        child.projection.footprint = geometry.size;
        return scalar_in_bounds(child);
    }
    match program.type_reference_table.type_reference(root) {
        TypeReferenceNode::Reference { .. } | TypeReferenceNode::Slice { .. } => None,
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(extent),
        } => {
            let (selector, rest) = path.split_first()?;
            let (selected, runtime) = array_selector(selector, *extent)?;
            let mut child = project(program, *element_type, rest, natural)?;
            let stride = child.projection.footprint;
            child.projection.offset = selected
                .checked_mul(stride)?
                .checked_add(child.projection.offset)?;
            child.projection.footprint = stride.checked_mul(*extent)?;
            shift_indices(&mut child, 1)?;
            if runtime {
                child.indices.insert(
                    0,
                    RecastRuntimeIndexGeometry {
                        path_index: 0,
                        stride,
                        extent: *extent,
                    },
                );
            }
            scalar_in_bounds(child)
        }
        _ => natural(root, path),
    }
}

fn array_selector(segment: &StoragePathSegment, extent: usize) -> Option<(usize, bool)> {
    match segment {
        StoragePathSegment::Element(index) if *index < extent => Some((*index, false)),
        StoragePathSegment::RuntimeElement(_) if extent != 0 => Some((0, true)),
        _ => None,
    }
}

fn shift_indices(geometry: &mut RecastScalarGeometry, amount: usize) -> Option<()> {
    for index in &mut geometry.indices {
        index.path_index = index.path_index.checked_add(amount)?;
    }
    Some(())
}

fn scalar_in_bounds(geometry: RecastScalarGeometry) -> Option<RecastScalarGeometry> {
    let width = match geometry.projection.stored_integer {
        Some(integer) if integer.bits() % 8 == 0 => usize::from(integer.bits() / 8),
        Some(_) => return None,
        None => geometry.projection.primitive.scalar_byte_size()?,
    };
    (width != 0 && geometry.projection.offset.checked_add(width)? <= geometry.projection.footprint)
        .then_some(geometry)
}
