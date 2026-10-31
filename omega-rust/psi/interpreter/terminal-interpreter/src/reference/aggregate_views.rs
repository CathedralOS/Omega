//! Typed re-views retain a window of original storage, not corresponding fields.
//! Equal admitted layout does not imply equal nesting: a logical child may span
//! several backing siblings. Aggregate projection therefore composes byte regions
//! until a scalar endpoint can rejoin one original cell and its declared codec.
//! These coordinates carry no access authority; descriptor traversal and argument
//! binding still authenticate the loan before using them.

use semantic_vocabulary::{CanonicalStructuralPathSegment, StructuralFieldId, StructuralTypeId};
use terminal_psi::{StructuralPathSegment, StructuralTypeShape};

use crate::program::ProgramDeclarations;
use crate::values::TerminalStructuralValue;

use super::AggregateViewTip;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct AggregateViewRegion {
    pub(crate) structural_type: StructuralTypeId,
    pub(crate) byte_offset: u64,
    pub(crate) byte_length: u64,
}

#[derive(Clone)]
pub(crate) struct AggregateBacking {
    pub(crate) value: TerminalStructuralValue,
    /// A placed presentation keeps its exact RAM occurrence across borrowed
    /// call and state ingress, including an already selected field accessor.
    /// The pinned home and attenuated access travel together; its type alone
    /// cannot reconstruct either association.
    pub(crate) placed: Option<super::AccessReference>,
    /// Logical re-view geometry and physical byte storage are independent:
    /// a typed re-view may itself borrow an already captured byte region.
    pub(crate) region: Option<AggregateViewRegion>,
    pub(crate) byte_region: Option<super::ReferenceByteRegion>,
    pub(crate) collection_home: Option<crate::element_views::ElementViewHome>,
    pub(crate) array_loan: Option<crate::element_views::ElementView>,
}

impl AggregateViewRegion {
    pub(crate) fn whole(
        types: &ProgramDeclarations<'_, terminal_psi::StructuralTypeDeclaration>,
        structural_type: StructuralTypeId,
    ) -> Option<Self> {
        Some(Self {
            structural_type,
            byte_offset: 0,
            byte_length: terminal_semantics::reference_storage_size(
                types.values(),
                structural_type,
            )?,
        })
    }

    pub(crate) fn fits(
        self,
        types: &ProgramDeclarations<'_, terminal_psi::StructuralTypeDeclaration>,
        backing: StructuralTypeId,
    ) -> bool {
        terminal_semantics::reference_storage_size(types.values(), self.structural_type)
            == Some(self.byte_length)
            && terminal_semantics::reference_storage_size(types.values(), backing).is_some_and(
                |size| {
                    self.byte_offset
                        .checked_add(self.byte_length)
                        .is_some_and(|end| end <= size)
                },
            )
    }
}

/// Select a complete logical path before seeking its original cell. Aggregate
/// selections retain the backing root and a smaller window instead of inventing
/// an equivalent backing child. Complete original bit recipes compose at scalar
/// observation; gaps and ambiguous physical coverage refuse.
pub(crate) fn translate_aggregate_view_path(
    types: &ProgramDeclarations<'_, terminal_psi::StructuralTypeDeclaration>,
    view: AggregateViewRegion,
    backing: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<(Vec<StructuralPathSegment>, AggregateViewTip)> {
    if !view.fits(types, backing) {
        return None;
    }
    let selected =
        terminal_semantics::reference_storage_region(types.values(), view.structural_type, path)?;
    if selected.byte_offset.checked_add(selected.byte_length)? > view.byte_length {
        return None;
    }
    let byte_offset = view.byte_offset.checked_add(selected.byte_offset)?;
    if let Some(scalar) = selected.scalar {
        let fragments =
            scalar_storage_fragments(types, backing, byte_offset, selected.byte_length)?;
        if let [fragment] = fragments.as_slice()
            && fragment.addressable_cell
            && fragment.bit_offset == byte_offset.checked_mul(8)?
            && fragment.source_bit_offset == 0
            && fragment.bit_length == selected.byte_length.checked_mul(8)?
            && terminal_semantics::reference_storage_byte_width(fragment.scalar.stored_scalar)
                .is_some_and(|width| u64::from(width) == selected.byte_length)
        {
            return Some((
                fragment.path.clone(),
                AggregateViewTip::Scalar {
                    view: scalar,
                    backing: fragment.scalar,
                },
            ));
        }
        Some((
            Vec::new(),
            AggregateViewTip::ScalarRegion {
                view: terminal_semantics::ReferenceStorageProjection {
                    byte_offset,
                    ..scalar
                },
                byte_length: selected.byte_length,
            },
        ))
    } else {
        let structural_type =
            crate::custody::resolve_structural_path_type(types, view.structural_type, path).ok()?;
        Some((
            Vec::new(),
            AggregateViewTip::Aggregate {
                view: AggregateViewRegion {
                    structural_type,
                    byte_offset,
                    byte_length: selected.byte_length,
                },
                backing,
            },
        ))
    }
}

/// Exact original scalar cells covering a requested physical window. This is
/// geometry only; the caller retains and validates its original loan authority.
pub(crate) struct ScalarStorageFragmentGeometry {
    pub(crate) path: Vec<StructuralPathSegment>,
    pub(crate) scalar: terminal_semantics::ReferenceStorageProjection,
    pub(crate) bit_offset: u64,
    pub(crate) source_bit_offset: u64,
    pub(crate) bit_length: u64,
    pub(crate) addressable_cell: bool,
}

pub(crate) fn scalar_storage_fragments(
    types: &ProgramDeclarations<'_, terminal_psi::StructuralTypeDeclaration>,
    backing: StructuralTypeId,
    byte_offset: u64,
    byte_length: u64,
) -> Option<Vec<ScalarStorageFragmentGeometry>> {
    let start = byte_offset.checked_mul(8)?;
    let end = byte_offset.checked_add(byte_length)?.checked_mul(8)?;
    let mut fragments = Vec::new();
    collect_scalar_fragments(
        types,
        backing,
        backing,
        &mut Vec::new(),
        start,
        end,
        &mut fragments,
    )?;
    fragments.sort_by_key(|fragment| fragment.bit_offset);
    let mut covered = start;
    for fragment in &fragments {
        let first = fragment.bit_offset;
        let next = first.checked_add(fragment.bit_length)?;
        if first > covered || next <= covered {
            return None;
        }
        covered = next.min(end);
    }
    (covered == end).then_some(fragments)
}

fn collect_scalar_fragments(
    types: &ProgramDeclarations<'_, terminal_psi::StructuralTypeDeclaration>,
    root: StructuralTypeId,
    carrier: StructuralTypeId,
    path: &mut Vec<StructuralPathSegment>,
    start: u64,
    end: u64,
    fragments: &mut Vec<ScalarStorageFragmentGeometry>,
) -> Option<()> {
    if path.len() >= 128 {
        return None;
    }
    if let Some(selected) = terminal_semantics::reference_scalar_storage(types.values(), root, path)
    {
        let mut append =
            |scalar, bit_offset: u64, source_bit_offset, bit_length: u64, addressable_cell| {
                let last = bit_offset.checked_add(bit_length)?;
                if start < last && bit_offset < end {
                    fragments.push(ScalarStorageFragmentGeometry {
                        path: path.clone(),
                        scalar,
                        bit_offset,
                        source_bit_offset,
                        bit_length,
                        addressable_cell,
                    });
                }
                Some(())
            };
        match selected {
            terminal_semantics::ReferenceScalarStorage::Contiguous(scalar) => {
                append(
                    scalar,
                    scalar.byte_offset.checked_mul(8)?,
                    0,
                    u64::from(terminal_semantics::reference_storage_byte_width(
                        scalar.stored_scalar,
                    )?)
                    .checked_mul(8)?,
                    true,
                )?;
            }
            terminal_semantics::ReferenceScalarStorage::Fragmented {
                byte_offset,
                logical_scalar,
                fragments: recipe,
            } => {
                for fragment in recipe.fragments() {
                    let scalar = terminal_semantics::ReferenceStorageProjection {
                        byte_offset: byte_offset.checked_add(fragment.container_byte_offset)?,
                        logical_scalar,
                        stored_scalar: logical_scalar,
                    };
                    append(
                        scalar,
                        scalar
                            .byte_offset
                            .checked_mul(8)?
                            .checked_add(fragment.destination_lsb)?,
                        fragment.source_lsb,
                        fragment.width,
                        false,
                    )?;
                }
            }
        }
        return Some(());
    }
    let region = terminal_semantics::reference_storage_region(types.values(), root, path)?;
    let region_start = region.byte_offset.checked_mul(8)?;
    let region_end = region
        .byte_offset
        .checked_add(region.byte_length)?
        .checked_mul(8)?;
    if end <= region_start || start >= region_end {
        return Some(());
    }
    match &types.get(&carrier)?.shape {
        StructuralTypeShape::Record { fields, .. } => {
            for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                path.push(StructuralPathSegment::Field(field.identity));
                let child = field.field_type.as_structural().unwrap_or(carrier);
                collect_scalar_fragments(types, root, child, path, start, end, fragments)?;
                path.pop();
            }
        }
        StructuralTypeShape::FixedArray {
            element,
            length: terminal_psi::ArrayExtent::Literal(length),
        } if *length != 0 => {
            path.push(StructuralPathSegment::FixedIndex(0));
            let first = terminal_semantics::reference_storage_region(types.values(), root, path)?;
            let stride = if *length == 1 {
                first.byte_length
            } else {
                *path.last_mut()? = StructuralPathSegment::FixedIndex(1);
                terminal_semantics::reference_storage_region(types.values(), root, path)?
                    .byte_offset
                    .checked_sub(first.byte_offset)?
            };
            if stride == 0 || stride < first.byte_length {
                return None;
            }
            let first_index = (start / 8).saturating_sub(first.byte_offset) / stride;
            let final_index = ((end.checked_sub(1)? / 8).saturating_sub(first.byte_offset)
                / stride)
                .min(length - 1);
            for index in first_index..=final_index {
                *path.last_mut()? = StructuralPathSegment::FixedIndex(index);
                collect_scalar_fragments(types, root, *element, path, start, end, fragments)?;
            }
            path.pop();
        }
        _ => return None,
    }
    Some(())
}

/// Canonical field observations use the same complete-path region join as
/// primitive operations. The result names the original record and field ID.
pub(crate) fn translate_aggregate_view_field(
    types: &ProgramDeclarations<'_, terminal_psi::StructuralTypeDeclaration>,
    view: AggregateViewRegion,
    backing: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
    field: StructuralFieldId,
) -> Option<(Vec<CanonicalStructuralPathSegment>, StructuralFieldId)> {
    let carrier =
        terminal_semantics::record_field_carrier(types.values(), view.structural_type, path)?;
    let declaration = terminal_semantics::observed_field_declaration(
        types.values(),
        view.structural_type,
        path,
        field,
    )?;
    let mut logical = carrier.path;
    logical.push(StructuralPathSegment::Field(declaration.identity));
    let (translated, _) = translate_aggregate_view_path(types, view, backing, &logical)?;
    let mut canonical = backing_scalar_path(types, backing, &translated)?;
    let CanonicalStructuralPathSegment::Field(field) = canonical.pop()? else {
        return None;
    };
    Some((canonical, field))
}

/// A scalar field cell and a primitive structural root use different catalog
/// spellings. Resolving retained storage must preserve that distinction rather
/// than require integer fields to become canonical primitive places. This query
/// creates no owned alias or permission; callers already selected the byte region.
pub(crate) fn backing_scalar_path(
    types: &ProgramDeclarations<'_, terminal_psi::StructuralTypeDeclaration>,
    backing: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<Vec<CanonicalStructuralPathSegment>> {
    if let Some((StructuralPathSegment::Field(_), prefix)) = path.split_last() {
        let field = crate::custody::inline_structural_field(types, backing, path)?;
        if matches!(
            field.field_type,
            terminal_psi::StructuralFieldType::Atomic(_)
                | terminal_psi::StructuralFieldType::AtomicBoundedInteger(_)
        ) {
            return None;
        }
        if field.field_type.scalar_type().is_some() {
            let (mut canonical, field) = terminal_semantics::static_field_projection(
                types.values(),
                backing,
                prefix,
                field.id,
            )?;
            canonical.push(CanonicalStructuralPathSegment::Field(field.id));
            return Some(canonical);
        }
    }
    terminal_semantics::canonical_primitive_path(types.values(), backing, path)
}
