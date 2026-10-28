//! Result binding plans for scalar and structural results and the byte
//! values a local or store receives.

use crate::checked_trees::CheckedMemberIdentity;
use crate::checked_trees::flow::terminal::{
    CheckedStructuralAccess, CheckedUnitStructuralPathSegment,
};
use language_semantics::Multiplicity;
use symbols::SymbolHandle;
use typed_trees::type_identity::TypeIdentityId;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

/// Exact state-local coordinate receiving one primitive boundary result in a
/// Unit-effect body. The local has no structural place or cleanup action; its
/// dense binding ordinal is the scalar value namespace used by later checked
/// call arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedUnitScalarResultBindingPlan {
    pub statement_index: u32,
    pub binding_ordinal: u32,
    pub primitive_type: PrimitiveType,
}

/// Exact whole structural operation result in the shared dense value namespace.
/// Its statement may bind an immutable local or directly supply completion.
/// Claim-free construction carries neither a fabricated claim nor a projected path.
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedUnitStructuralResultBindingPlan {
    pub statement_index: u32,
    pub binding_ordinal: u32,
    pub type_identity: TypeIdentityId,
    pub multiplicity: Multiplicity,
    /// The `let` this result initializes, recorded where planning pairs the
    /// result with its home. The zero value marks an anonymous result.
    pub local: CheckedInitializedLocal,
}

/// The `let` local whose home a structural result establishes: later reads,
/// loans, claims and edge disposal of that local name this result. The zero
/// value (an invalid `symbol`) marks an anonymous result -- a returned
/// value, a computed dispatch subject, a nested operand, an explicitly
/// discarded call, a call whose result a store consumes, or the call result
/// a descriptor local's first store reads after its home is established.
#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedInitializedLocal {
    pub symbol: SymbolHandle,
    /// The local's declared type. Claim paths, closed-sum subjects and byte
    /// carriers resolve against the declaration, not the stored identity.
    pub declared_type: TypeReferenceHandle,
    /// The custody the local's binding holds over its carrier: a shared
    /// borrow for `&T` over a record, primitive or slice view, an exclusive
    /// borrow for `&mut T`, otherwise owned. Constraint shells do not change
    /// custody (`&[u8] in Utf8` is a shared view), and an element-declared
    /// view (`let v: u8 = bytes[a..b]`) classifies the view it binds.
    pub access: CheckedStructuralAccess,
}

impl CheckedInitializedLocal {
    pub fn is_named(&self) -> bool {
        self.symbol.is_valid()
    }
}

impl Default for CheckedInitializedLocal {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            declared_type: TypeReferenceHandle::invalid(),
            access: CheckedStructuralAccess::Owned,
        }
    }
}

/// Pure byte values retain authored evaluation order through arena children.
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedByteSequenceValuePlan {
    pub nodes: arena::Arena<CheckedByteSequenceValue>,
    pub root: arena::Handle<CheckedByteSequenceValue>,
}

impl CheckedByteSequenceValuePlan {
    pub fn literal(bytes: Vec<u8>) -> Self {
        let mut plan = Self::default();
        plan.root = plan.nodes.insert(CheckedByteSequenceValue::Literal(bytes));
        plan
    }

    /// The completed structural call this value observes. Source correspondence
    /// separately authenticates the expression and call occurrence; this query
    /// lets the ordinary schedule treat descriptor and owned-byte consumers alike.
    pub fn completed_call_binding_ordinal(&self) -> Option<u32> {
        let value = self
            .nodes
            .iter()
            .find_map(|(identity, value)| (identity == self.root).then_some(value))?;
        match value {
            CheckedByteSequenceValue::CallResult {
                binding_ordinal, ..
            } => Some(*binding_ordinal),
            CheckedByteSequenceValue::Descriptor { source, .. } => {
                source.source_structural_result_binding_ordinal()
            }
            _ => None,
        }
    }

    /// Only a literal root can lend image storage without an owned construction.
    pub fn literal_bytes(&self) -> Option<&[u8]> {
        self.nodes.iter().find_map(|(identity, value)| match value {
            CheckedByteSequenceValue::Literal(bytes) if identity == self.root => {
                Some(bytes.as_slice())
            }
            _ => None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub enum CheckedByteSequenceValue {
    Literal(Vec<u8>),
    /// A completed descriptor value, distinct from a byte-content snapshot.
    /// No fabricated capacity or owned copy can represent its backing loan.
    Descriptor {
        expression: typed_trees::expression::ExpressionHandle,
        source: crate::CheckedUnitStructuralArgumentPlan,
    },
    /// An already completed call, rejoined to its exact authored occurrence
    /// and structural result binding before the store observes its live length.
    CallResult {
        state: symbols::SymbolHandle,
        statement_index: u32,
        binding_ordinal: u32,
        capacity: u64,
    },
    /// Read the current live bytes of a readable parameter or established local.
    /// Capacity is its carrier bound, not the observed content length.
    Whole {
        root: crate::CheckedStoreRoot,
        capacity: u64,
    },
    Field {
        /// Exact readable parameter or established local home. The source is
        /// observed at the store, not reconstructed from its initializer.
        root: crate::CheckedStoreRoot,
        carrier_path: Vec<CheckedUnitStructuralPathSegment>,
        field_identity: CheckedMemberIdentity,
        capacity: u64,
    },
    Concatenation {
        left: arena::Handle<CheckedByteSequenceValue>,
        right: arena::Handle<CheckedByteSequenceValue>,
        capacity: u64,
    },
}

impl Default for CheckedByteSequenceValue {
    fn default() -> Self {
        Self::Literal(Vec::new())
    }
}
