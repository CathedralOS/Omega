//! Reference shells: the borrow a type spells, read once for every access.
//!
//! A shared `&T`, an exclusive `&mut T` and a write-only `&write T` are one
//! type shape — a `Reference` node, possibly wrapped in constraint shells —
//! that differs only in the access the node records. Stages that ask what a
//! borrow lends classify the shell here and filter on its access, rather
//! than re-walking the shells once per access: `reference_shell` answers the
//! borrow and its referee, `slice_view_shell` the `[T]` slice a borrowed view
//! lends. Referee and element restrictions (a named referent, a non-byte
//! element) stay with the caller whose carrier needs them.

use language_semantics::ReferenceAccess;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// The first borrow beneath a type's constraint shells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReferenceShell {
    /// The access the borrow grants.
    pub access: ReferenceAccess,
    /// The shell's own referee, before any constraint shells it carries.
    pub referee: TypeReferenceHandle,
}

/// A borrowed `[T]` view: exactly one reference shell over a slice, with
/// constraint shells and bound type parameters looked through on both sides
/// of the borrow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SliceViewShell {
    /// The access the view's borrow grants.
    pub access: ReferenceAccess,
    /// The `[T]` slice node the borrow lends.
    pub slice: TypeReferenceHandle,
    /// The slice's element type.
    pub element: TypeReferenceHandle,
}

/// The borrow `reference` spells beneath its constraint shells. `None` when
/// the first node beneath them is not a `Reference`.
pub fn reference_shell(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<ReferenceShell> {
    let shell = peel_carrier_shells(program, reference, &[])?;
    let TypeReferenceNode::Reference {
        referee, access, ..
    } = program.type_reference_table.type_reference(shell)
    else {
        return None;
    };
    Some(ReferenceShell {
        access: *access,
        referee: *referee,
    })
}

/// The slice a borrowed `[T]` view lends. Constraint shells qualify the
/// carrier without changing it, and a type parameter bound in
/// `substitutions` stands for its actual, so both are looked through before
/// and after the one reference shell; a second shell is a reference to a
/// reference, not a view.
pub fn slice_view_shell(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> Option<SliceViewShell> {
    let shell = peel_carrier_shells(program, reference, substitutions)?;
    let TypeReferenceNode::Reference {
        referee, access, ..
    } = program.type_reference_table.type_reference(shell)
    else {
        return None;
    };
    let slice = peel_carrier_shells(program, *referee, substitutions)?;
    let TypeReferenceNode::Slice { element_type } =
        program.type_reference_table.type_reference(slice)
    else {
        return None;
    };
    Some(SliceViewShell {
        access: *access,
        slice,
        element: *element_type,
    })
}

/// Follow constraint shells and bound type parameters down to the first
/// node that is neither. A parameter bound to itself has no actual to stand
/// for, and a walk longer than the table has nodes has revisited one and
/// would never end; both answer `None`.
fn peel_carrier_shells(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> Option<TypeReferenceHandle> {
    for _ in 0..=program.type_reference_table.type_reference_count() {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Named { symbol, .. } => {
                let Some((_, replacement)) = substitutions
                    .iter()
                    .rev()
                    .find(|(parameter, _)| parameter == symbol)
                else {
                    return Some(reference);
                };
                if *replacement == reference {
                    return None;
                }
                reference = *replacement;
            }
            _ => return Some(reference),
        }
    }
    None
}
