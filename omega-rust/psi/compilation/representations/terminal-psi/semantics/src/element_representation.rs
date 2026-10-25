//! Declaration-derived correspondence between typed collection elements.
//!
//! Layout does not establish values. The source's initialized typed custody
//! supplies its declared leaf restrictions; shared observation preserves them
//! by containment, and mutable observation requires equal sets of stored bits.
//! Padding stays padding, and source/target paths remain separate identities.
use semantic_vocabulary::{BoundedIntegerType, IntegerValue, ScalarType, StructuralTypeId};
use terminal_psi::{
    StructuralAccess, StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementRepresentationLeaf {
    pub path: Vec<StructuralPathSegment>,
    pub offset: u64,
    pub scalar_type: ScalarType,
    pub bounds: Option<BoundedIntegerType>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementRepresentationLayout {
    pub size: u64,
    pub alignment: u64,
    pub leaves: Vec<ElementRepresentationLeaf>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementRepresentationRelation {
    /// Paired leaves have identical offsets and widths, but keep their own paths.
    pub source_layout: ElementRepresentationLayout,
    pub target_layout: ElementRepresentationLayout,
}

pub(crate) fn scalar_size(scalar: ScalarType) -> Option<u64> {
    match scalar {
        ScalarType::Boolean => Some(1),
        ScalarType::Integer(integer)
            if !integer.is_address() && matches!(integer.bits(), 8 | 16 | 32 | 64) =>
        {
            Some(u64::from(integer.bits() / 8))
        }
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32) => Some(4),
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64) => Some(8),
        _ => None,
    }
}
fn align(value: u64, alignment: u64) -> Option<u64> {
    value
        .checked_add(alignment.checked_sub(1)?)
        .map(|value| value / alignment * alignment)
}
fn scalar_layout(
    scalar: ScalarType,
    bounds: Option<BoundedIntegerType>,
) -> Option<ElementRepresentationLayout> {
    let size = scalar_size(scalar)?;
    Some(ElementRepresentationLayout {
        size,
        alignment: size,
        leaves: vec![ElementRepresentationLeaf {
            path: Vec::new(),
            offset: 0,
            scalar_type: scalar,
            bounds,
        }],
    })
}
/// Closed fixed layouts only. Exact-identity callers may retain their existing
/// target-dependent layout; this query never guesses pointer or nominal storage.
pub fn element_representation_layout<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    element: StructuralTypeId,
) -> Option<ElementRepresentationLayout> {
    fn visit<'a>(
        types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
        element: StructuralTypeId,
        active: &mut Vec<StructuralTypeId>,
        budget: &mut usize,
    ) -> Option<ElementRepresentationLayout> {
        *budget = budget.checked_sub(1)?;
        if active.len() >= 128 || active.contains(&element) {
            return None;
        }
        let mut rows = types.clone().filter(|row| row.id == element);
        let row = rows.next()?;
        if rows.next().is_some() {
            return None;
        }
        active.push(element);
        let mut layout = match &row.shape {
            StructuralTypeShape::PrimitiveScalar(scalar) => scalar_layout(*scalar, None)?,
            StructuralTypeShape::FixedArray { element, length } => {
                let child = visit(types.clone(), *element, active, budget)?;
                let stride = align(child.size, child.alignment)?;
                let count = usize::try_from(*length)
                    .ok()?
                    .checked_mul(child.leaves.len())?;
                *budget = budget.checked_sub(count)?;
                let mut leaves = Vec::new();
                leaves.try_reserve_exact(count).ok()?;
                for index in 0..*length {
                    for leaf in &child.leaves {
                        let mut path = vec![StructuralPathSegment::FixedIndex(index)];
                        path.extend_from_slice(&leaf.path);
                        leaves.push(ElementRepresentationLeaf {
                            path,
                            offset: stride.checked_mul(index)?.checked_add(leaf.offset)?,
                            scalar_type: leaf.scalar_type,
                            bounds: leaf.bounds,
                        });
                    }
                }
                ElementRepresentationLayout {
                    size: stride.checked_mul(*length)?,
                    alignment: child.alignment,
                    leaves,
                }
            }
            StructuralTypeShape::Record { fields, .. } => {
                let (size, alignment) = crate::reference_storage_layout(types.clone(), element)?;
                let mut leaves = Vec::new();
                for field in fields {
                    if field.relevance.is_erased() {
                        return None;
                    }
                    let child = match &field.field_type {
                        StructuralFieldType::Scalar(scalar) => scalar_layout(*scalar, None)?,
                        StructuralFieldType::BoundedInteger(bounds) | StructuralFieldType::AtomicBoundedInteger(bounds) => scalar_layout(
                            ScalarType::Integer(bounds.integer_type()),
                            Some(*bounds),
                        )?,
                        StructuralFieldType::IeeeFloat(format) => {
                            scalar_layout(ScalarType::IeeeFloat(*format), None)?
                        }
                        StructuralFieldType::Structural(child) => {
                            visit(types.clone(), *child, active, budget)?
                        }
                        _ => return None,
                    };
                    *budget = budget.checked_sub(child.leaves.len())?;
                    leaves.try_reserve(child.leaves.len()).ok()?;
                    for leaf in child.leaves {
                        let mut path = vec![StructuralPathSegment::Field(field.identity.clone())];
                        path.extend(leaf.path);
                        leaves.push(ElementRepresentationLeaf {
                            path,
                            // The complete path is resolved below; child
                            // offsets do not imply parent authored placement.
                            offset: leaf.offset,
                            scalar_type: leaf.scalar_type,
                            bounds: leaf.bounds,
                        });
                    }
                }
                ElementRepresentationLayout {
                    size,
                    alignment,
                    leaves,
                }
            }
            _ => return None,
        };
        // The logical leaf walk owns validity predicates; the common storage
        // query owns physical offsets, repeated strides, size and alignment.
        // Pending applications return None here rather than ordinary packing.
        let (size, alignment) = crate::reference_storage_layout(types.clone(), element)?;
        for leaf in &mut layout.leaves {
            let projection =
                crate::reference_storage_projection(types.clone(), element, &leaf.path)?;
            // This correspondence transports one scalar unchanged. A narrower
            // codec needs an explicit encoded/logical value relation, not a
            // semantic-width leaf silently placed over fewer bytes.
            if projection.logical_scalar != leaf.scalar_type
                || projection.stored_scalar != leaf.scalar_type
            {
                return None;
            }
            leaf.offset = projection.byte_offset;
        }
        layout.size = size;
        layout.alignment = alignment;
        layout.leaves.sort_by_key(|leaf| leaf.offset);
        active.pop();
        (layout.size != 0 && !layout.leaves.is_empty()).then_some(layout)
    }
    visit(types, element, &mut Vec::new(), &mut 16384)
}
fn bit_intervals(leaf: &ElementRepresentationLeaf) -> Option<Vec<(u64, u64)>> {
    let bits = scalar_size(leaf.scalar_type)?.checked_mul(8)?;
    let mask = if bits == 64 {
        u64::MAX
    } else {
        (1u64 << bits) - 1
    };
    let Some(bounds) = leaf.bounds else {
        return Some(vec![(
            0,
            if leaf.scalar_type == ScalarType::Boolean {
                1
            } else {
                mask
            },
        )]);
    };
    if leaf.scalar_type != ScalarType::Integer(bounds.integer_type()) {
        return None;
    }
    let mut intervals = match (bounds.minimum(), bounds.maximum()) {
        (IntegerValue::Unsigned(minimum), IntegerValue::Unsigned(maximum)) => {
            vec![(u64::try_from(minimum).ok()?, u64::try_from(maximum).ok()?)]
        }
        (IntegerValue::Signed(minimum), IntegerValue::Signed(maximum)) => {
            let minimum = i64::try_from(minimum).ok()?;
            let maximum = i64::try_from(maximum).ok()?;
            if minimum < 0 && maximum >= 0 {
                vec![(0, maximum as u64), ((minimum as u64) & mask, mask)]
            } else {
                vec![((minimum as u64) & mask, (maximum as u64) & mask)]
            }
        }
        _ => return None,
    };
    if intervals.len() == 2 && intervals[0].1.saturating_add(1) >= intervals[1].0 {
        intervals = vec![(intervals[0].0, intervals[1].1)];
    }
    Some(intervals)
}
/// This relation grants no loan, initialization or qualification authority.
/// Consumers must independently retain the exact source and target declarations.
pub fn element_representation_relation<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    source: StructuralTypeId,
    target: StructuralTypeId,
    access: StructuralAccess,
) -> Option<ElementRepresentationRelation> {
    if !matches!(access, terminal_psi::reading_borrow_pair_pattern!()) {
        return None;
    }
    let source_layout = element_representation_layout(types.clone(), source)?;
    let target_layout = element_representation_layout(types, target)?;
    if source_layout.size != target_layout.size
        || source_layout.alignment != target_layout.alignment
        || source_layout.leaves.len() != target_layout.leaves.len()
    {
        return None;
    }
    for (source, target) in source_layout.leaves.iter().zip(&target_layout.leaves) {
        if source.offset != target.offset
            || scalar_size(source.scalar_type)? != scalar_size(target.scalar_type)?
        {
            return None;
        }
        let source = bit_intervals(source)?;
        let target = bit_intervals(target)?;
        if !source.iter().all(|(low, high)| {
            target
                .iter()
                .any(|(other_low, other_high)| other_low <= low && high <= other_high)
        }) || (access == StructuralAccess::MutableBorrow && source != target)
        {
            return None;
        }
    }
    Some(ElementRepresentationRelation {
        source_layout,
        target_layout,
    })
}
