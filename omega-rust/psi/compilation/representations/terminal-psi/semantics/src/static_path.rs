//! Exact static `Case`/`Field`/`FixedIndex` path resolution shared by the semantic
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
//! identities and declaration-local case IDs, also refusing ambiguity. Neither
//! grants the place any storage, view adapter, or type
//! identity of its own.

use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IntegerSign, IntegerValue, Proposition, ScalarTerm, ScalarType,
    StructuralTypeId, ValueId,
};
use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralFieldType, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape,
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

/// Exact storage selection ignores operation-owned bound identities. Each
/// runtime selector must still be the same immutable SSA value; this relation
/// proves neither its bounds nor the current validity of a storage observation.
pub fn structural_paths_select_same_location(
    left: &[StructuralPathSegment],
    right: &[StructuralPathSegment],
) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| match (left, right) {
                (
                    StructuralPathSegment::RuntimeIndex { index: left, .. },
                    StructuralPathSegment::RuntimeIndex { index: right, .. },
                ) => left == right,
                _ => left == right,
            })
}

/// Conservative overlap of borrowed projections. Prefixes retain their whole
/// subtree; distinct range spellings are not evidence of disjoint storage.
pub fn structural_paths_may_overlap(
    left: &[StructuralPathSegment],
    right: &[StructuralPathSegment],
) -> bool {
    for (left, right) in left.iter().zip(right) {
        // Variant namespaces describe the same overlay. Even different
        // runtime selectors can alias, so stop before comparing their fields.
        if let (StructuralPathSegment::Case(left), StructuralPathSegment::Case(right)) =
            (left, right)
            && left != right
        {
            return true;
        }
        let overlaps = match (left, right) {
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
        };
        if !overlaps {
            return false;
        }
    }
    true
}
/// Whether `child` is a borrow formed strictly inside the extent `parent`
/// already loans on the same place. A receiver's own claim is the callee's
/// context, so a member borrow inside it is a subloan under the parent —
/// a frozen or suspended child under the loan parent-child table — never an
/// independent second claim. The parent's access admits the child's: a
/// mutable or owned parent freezes a shared child and suspends a mutable or
/// write-only child, while a write-only parent admits borrow children only.
/// Callers decide which argument plays the parent; containment is strict, so
/// same-extent pairs are never children, and a shared parent never admits an
/// exclusive child.
pub fn interior_member_borrow(parent: &StructuralArgument, child: &StructuralArgument) -> bool {
    let admitted = match parent.access {
        StructuralAccess::MutableBorrow | StructuralAccess::Owned => matches!(
            child.access,
            StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow
        ),
        StructuralAccess::WriteOnlyBorrow => matches!(
            child.access,
            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
        ),
        StructuralAccess::SharedBorrow => false,
    };
    admitted
        && child.place == parent.place
        && child.path.len() > parent.path.len()
        && child.path.starts_with(&parent.path)
}

/// Resolve `path` from `root` to its tip declaration, rejecting a duplicated
/// declaration or field id. This grants no access or establishment authority
/// for the root.
pub fn canonical_structural_path_tip<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
) -> Option<&'a StructuralTypeDeclaration> {
    let selected = canonical_projection_type(declarations.clone(), root, path)?;
    let mut matching = declarations.filter(|declaration| declaration.id == selected);
    let declaration = matching.next()?;
    matching.next().is_none().then_some(declaration)
}

/// Resolve a case-qualified structural leaf by declaration-local identities.
/// Each Case enters one sum payload namespace and must be followed by its
/// field. This checks static type meaning only: it does not establish that
/// the case is active, grant readable access, or authorize copying the leaf.
pub fn canonical_case_leaf_type<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
) -> Option<StructuralTypeId> {
    path.iter()
        .any(|segment| matches!(segment, CanonicalStructuralPathSegment::Case(_)))
        .then_some(())?;
    canonical_projection_type(types, root, path)
}

// Canonical and name-spelled paths use the same declaration-local namespaces.
// This walker retains only structural children; scalar endpoint policy belongs
// to the primitive/field observation owner.
fn canonical_projection_type<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    mut root: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
) -> Option<StructuralTypeId> {
    let declaration = |identity| {
        let mut matching = types.clone().filter(|candidate| candidate.id == identity);
        let selected = matching.next()?;
        matching.next().is_none().then_some(selected)
    };
    let mut case_fields: Option<&[terminal_psi::StructuralFieldDeclaration]> = None;
    for segment in path {
        let shape = &declaration(root)?.shape;
        match segment {
            CanonicalStructuralPathSegment::Case(identity) => {
                if case_fields.is_some() {
                    return None;
                }
                let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
                    shape
                else {
                    return None;
                };
                let mut matching = cases.iter().filter(|case| case.id == *identity);
                case_fields = Some(&matching.next()?.fields);
                if matching.next().is_some() {
                    return None;
                }
            }
            CanonicalStructuralPathSegment::Field(identity) => {
                let fields = match case_fields.take() {
                    Some(fields) => fields,
                    None => match shape {
                        StructuralTypeShape::Record { fields }
                        | StructuralTypeShape::Mixed { fields, .. } => fields,
                        _ => return None,
                    },
                };
                let mut matching = fields.iter().filter(|field| field.id == *identity);
                let field = matching.next()?;
                if matching.next().is_some() || field.relevance.is_erased() {
                    return None;
                }
                let StructuralFieldType::Structural(child) = field.field_type else {
                    return None;
                };
                root = child;
            }
            CanonicalStructuralPathSegment::FixedIndex(index) => {
                let StructuralTypeShape::FixedArray { element, length } = shape else {
                    return None;
                };
                if case_fields.is_some() || index >= length {
                    return None;
                }
                root = *element;
            }
        }
    }
    case_fields.is_none().then_some(declaration(root)?.id)
}
