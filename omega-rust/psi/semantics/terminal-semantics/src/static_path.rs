//! Exact static `Field`/`FixedIndex` path resolution shared by the semantic
//! rows.
//!
//! One rule: each hop descends through the declared type catalog — a `Field`
//! segment names a non-erased record field holding a structural child, and a
//! `FixedIndex` segment stays inside a fixed array's declared extent. The
//! resolved tip is evidence, not authority: access, multiplicity,
//! qualification, custody, and claim obligations stay with each caller.
//!
//! Two entry points mirror the two segment vocabularies. The canonical form
//! resolves declaration-local field ids and rejects a duplicated declaration
//! or field id rather than picking one; the runtime form resolves field
//! identities and takes the first match, as the validated catalog admits no
//! duplicates. Neither grants the place any storage, view adapter, or type
//! identity of its own.

use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IntegerSign, IntegerValue, Proposition, ScalarTerm, ScalarType,
    StructuralTypeId, ValueId,
};
use terminal_psi::{
    StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
};

/// The bounds relation a `RuntimeIndex` segment's obligation proves for an
/// index of `index_type` selecting one element of a fixed array with `extent`
/// elements: `index < extent`, and `0 <= index` for a signed carrier. When
/// `extent` exceeds the carrier's largest value every carrier value is below
/// it and the upper conjunct is omitted, so an unsigned carrier then proves
/// `Truth`. Reconstruction (the verifier) and execution (the interpreter's
/// `runtime_index_selects`) share this one definition; an address carrier or
/// a non-integer index has no bound and is refused.
pub fn runtime_index_bound(
    index: ValueId,
    index_type: ScalarType,
    extent: u64,
) -> Option<Proposition> {
    let ScalarType::Integer(integer_type) = index_type else {
        return None;
    };
    if integer_type.is_address() {
        return None;
    }
    let subject = ScalarTerm::value(index, index_type);
    let mut conjuncts = Vec::with_capacity(2);
    if integer_type.sign() == IntegerSign::Signed {
        conjuncts.push(Proposition::LessOrEqual(
            ScalarTerm::integer(integer_type, IntegerValue::Signed(0)).ok()?,
            subject.clone(),
        ));
    }
    let extent_value = match integer_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(i128::from(extent)),
        IntegerSign::Unsigned => IntegerValue::Unsigned(u128::from(extent)),
    };
    if integer_type.admits(extent_value) {
        conjuncts.push(Proposition::LessThan(
            subject,
            ScalarTerm::integer(integer_type, extent_value).ok()?,
        ));
    }
    Some(match conjuncts.len() {
        0 => Proposition::Truth,
        1 => conjuncts.pop().expect("one conjunct"),
        _ => Proposition::Conjunction(conjuncts),
    })
}

/// Whether a runtime index value selects an element of a fixed array with
/// `extent` elements: the executable reading of `runtime_index_bound`.
pub fn runtime_index_selects(value: IntegerValue, extent: u64) -> Option<u64> {
    let selected = match value {
        IntegerValue::Unsigned(value) => u64::try_from(value).ok()?,
        IntegerValue::Signed(value) => u64::try_from(value).ok()?,
    };
    (selected < extent).then_some(selected)
}

/// Conservative overlap of borrowed projections. Prefixes retain their whole
/// subtree; distinct range spellings are not evidence of disjoint storage.
pub fn structural_paths_may_overlap(
    left: &[StructuralPathSegment],
    right: &[StructuralPathSegment],
) -> bool {
    left.iter()
        .zip(right)
        .all(|(left, right)| match (left, right) {
            // Empty loans still retain their backing: do not derive exclusive
            // alias freedom from the absence of writable bytes. Invalid ranges
            // are rejected by presentation validation, never trusted here.
            (
                StructuralPathSegment::FixedByteRange {
                    start: left_start,
                    end: left_end,
                },
                StructuralPathSegment::FixedByteRange {
                    start: right_start,
                    end: right_end,
                },
            ) => {
                left_start >= left_end
                    || right_start >= right_end
                    || (left_start < right_end && right_start < left_end)
            }
            (
                StructuralPathSegment::FixedByteRange { start, end },
                StructuralPathSegment::FixedIndex(index),
            )
            | (
                StructuralPathSegment::FixedIndex(index),
                StructuralPathSegment::FixedByteRange { start, end },
            ) => start >= end || (start <= index && index < end),
            // A runtime index may select any element inside its proven
            // bounds: it overlaps a fixed index, another runtime index, and
            // every sibling spelling the two paths cannot disprove.
            (StructuralPathSegment::RuntimeIndex { .. }, _)
            | (_, StructuralPathSegment::RuntimeIndex { .. }) => true,
            _ => left == right,
        })
}
/// Resolve `path` from `root` to its tip declaration, rejecting a duplicated
/// declaration or field id. This grants no access or establishment authority
/// for the root.
pub fn canonical_structural_path_tip<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    mut root: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
) -> Option<&'a StructuralTypeDeclaration> {
    for segment in path {
        let mut matching = declarations
            .clone()
            .filter(|declaration| declaration.id == root);
        let declaration = matching.next()?;
        if matching.next().is_some() {
            return None;
        }
        root = match (segment, &declaration.shape) {
            (
                CanonicalStructuralPathSegment::Field(identity),
                StructuralTypeShape::Record { fields },
            ) => {
                let mut matching = fields.iter().filter(|field| field.id == *identity);
                let field = matching.next()?;
                if matching.next().is_some() || field.relevance.is_erased() {
                    return None;
                }
                let StructuralFieldType::Structural(child) = field.field_type else {
                    return None;
                };
                child
            }
            (
                CanonicalStructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray { element, length },
            ) if index < length => *element,
            _ => return None,
        };
    }
    let mut matching = declarations.filter(|declaration| declaration.id == root);
    let declaration = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    Some(declaration)
}

/// Resolve `path` from `root` to its tip declaration over runtime field
/// identities, taking the first match the validated catalog presents. This
/// grants no access or establishment authority for the root.
pub fn runtime_structural_path_tip<'a>(
    mut types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    mut root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<&'a StructuralTypeDeclaration> {
    for segment in path {
        let declaration = types.clone().find(|declaration| declaration.id == root)?;
        root = match (segment, &declaration.shape) {
            (StructuralPathSegment::Field(identity), StructuralTypeShape::Record { fields }) => {
                let field = fields
                    .iter()
                    .find(|field| field.identity == *identity && !field.relevance.is_erased())?;
                let StructuralFieldType::Structural(next) = field.field_type else {
                    return None;
                };
                next
            }
            (
                StructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray { element, length },
            ) if index < length => *element,
            _ => return None,
        };
    }
    types.find(|declaration| declaration.id == root)
}
