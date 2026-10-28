//! Silent place loans: an immutable `let r: &T = &place` or
//! `let r: &mut T = &mut place` that is neither a reference capture nor a
//! value root. The binding cannot be reseated, so it denotes one place for its
//! whole scope and plans no operation; each read or store through it is an
//! ordinary access of that place. The row records the captured place so
//! lowering rejoins uses to it without re-reading the initializer.

use super::{CheckedStructuralAccess, CheckedUnitStructuralPathSegment};
use symbols::SymbolHandle;
use typed_trees::types::TypeReferenceHandle;

/// One silent place loan of a state, in the plan that owns the state's
/// structural parameter roster.
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedPlaceLoanPlan {
    /// The `let` that declares the loan local.
    pub statement_index: u32,
    pub symbol: SymbolHandle,
    /// `SharedBorrow` or `MutableBorrow`; only an exclusive loan may store.
    pub access: CheckedStructuralAccess,
    /// The referee the local's reference type names.
    pub referee: TypeReferenceHandle,
    pub root: CheckedPlaceLoanRoot,
    /// The root's declared type with its own reference shells removed.
    pub root_type: TypeReferenceHandle,
    /// The authored projection from the root to the loaned place: field
    /// identities, literal indexes (an immutable local bound to a constant
    /// folds to its literal), runtime selectors at their checked coordinates
    /// and every stored-reference crossing.
    pub path: Vec<CheckedUnitStructuralPathSegment>,
    /// For a recast (`&place as &View`), the loaned place's own declared type,
    /// which the view reinterprets; the zero handle for a plain loan.
    pub recast_source: TypeReferenceHandle,
    /// `true` when the recast admission at formation proved the byte window:
    /// `index + view footprint <= region length` for an indexed `[u8; N]`
    /// source. Byte-view stores through the loan then discharge their emitted
    /// runtime obligations (`base + byte_offset` representability and the
    /// index's `index < extent` requirement) from that admission rather than
    /// a fresh runtime check.
    pub decided_byte_view_store: bool,
}

/// The storage a silent place loan names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum CheckedPlaceLoanRoot {
    /// Dense index into the state's structural parameter roster.
    StructuralParameter { parameter_index: u32 },
    /// A runtime scalar parameter at its authored position. A shared loan may
    /// alias it; the parameter has no structural place to project.
    ScalarParameter { position: u32 },
    /// An earlier local of the same state, structural or primitive.
    Local { symbol: SymbolHandle },
}
