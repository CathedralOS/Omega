//! Result binding plans for scalar and structural results and the
//! byte-sequence field stores that receive them.

use crate::CheckedScalarExpression;
use crate::checked_trees::flow::terminal::CheckedUnitStructuralPathSegment;
use language_semantics::Multiplicity;
use typed_trees::types::PrimitiveType;

/// Exact state-local coordinate receiving one primitive boundary result in a
/// Unit-effect body. The local has no structural place or cleanup action; its
/// dense binding ordinal is the scalar value namespace used by later checked
/// call arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedUnitScalarResultBindingPlan {
    pub statement_index: u32,
    pub binding_ordinal: u32,
    pub primitive_type: PrimitiveType,
}

/// Exact whole structural operation result in the shared dense value namespace.
/// Its statement may bind an immutable local or directly supply completion.
/// Claim-free construction carries neither a fabricated claim nor a projected path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitStructuralResultBindingPlan {
    pub statement_index: u32,
    pub binding_ordinal: u32,
    pub type_identity: String,
    pub multiplicity: Multiplicity,
}

/// Exact source occurrence of bounded byte-owner replacement. The root is
/// always an exclusive borrowed parameter: a local-rooted whole owner or byte
/// field has no lowering and is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedByteSequenceStorePlan {
    pub statement_index: u32,
    pub root: crate::CheckedStoreRoot,
    /// The replaced place below `root`: the whole owner or one byte field.
    pub destination: CheckedByteSequenceStoreDestination,
    pub value: CheckedByteSequenceValuePlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedByteSequenceStoreDestination {
    Whole,
    Field {
        carrier_path: Vec<CheckedUnitStructuralPathSegment>,
        field_identity: String,
    },
    /// One shared borrowed-view field whose descriptor the value replaces;
    /// no bytes are copied. A descriptor value retains its actual source;
    /// independent loan replay proves that source outlives every later use.
    ViewField {
        carrier_path: Vec<CheckedUnitStructuralPathSegment>,
        field_identity: String,
    },
}

/// Pure byte values retain authored evaluation order through arena children.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
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
        field_identity: String,
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

/// Exact source operands of a scalar byte replacement within the live prefix.
/// The stored byte is a pure value or the statement's own call result; byte
/// leaves lower no scalar computation, so `Computation` is refused. The root is
/// a parameter; a local-rooted byte-field element is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralByteSequenceFieldByteStorePlan {
    pub statement_index: u32,
    pub destination: crate::CheckedStoreRoot,
    pub carrier_path: Vec<CheckedUnitStructuralPathSegment>,
    pub field_identity: String,
    pub index: CheckedScalarExpression,
    pub value: crate::CheckedStoreValue,
}

/// Exact authored operands of a write through a whole mutable borrowed byte
/// parameter or an exactly established mutable bounded-owned local. The stored
/// byte is refused as a `Computation`, exactly as a byte replacement's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedByteSequenceWritePlan {
    pub statement_index: u32,
    pub destination: crate::CheckedStoreRoot,
    pub index: CheckedScalarExpression,
    pub value: crate::CheckedStoreValue,
}

/// Exact authored operands of a runtime-indexed element write through a
/// mutable `&mut [T]` view. The destination names the view's root — a
/// borrowed slice parameter or a locally bound `&mut` view — the index its
/// runtime selector, and the value the element-typed scalar stored. The
/// element's primitive type is retained so lowering validates the stored
/// scalar against the view's declared element, never the destination type
/// of another leaf family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedElementViewWritePlan {
    pub statement_index: u32,
    pub destination: crate::CheckedStoreRoot,
    pub index: CheckedScalarExpression,
    /// Projection within the selected element; empty for a scalar element.
    pub path: Vec<CheckedUnitStructuralPathSegment>,
    pub element: PrimitiveType,
    pub value: crate::CheckedStoreValue,
}

/// Exact authored operands of a scalar store through a representation view:
/// `view = v` where `view` is an immutable `&mut`/`&write` local bound by
/// `&mut place as &mut T`. `destination` and `path` rejoin the view's
/// loaned place so `path` ends at the leaf the view re-covers — a whole
/// primitive field for a same-width view, or one `u8` element selected by
/// `FixedIndex`/`RuntimeIndex` below a bounded byte field for a wider one.
/// `element` is that leaf's declared primitive and `view` the referee's
/// declared primitive. One authored statement mints one row: lowering
/// expands a view wider than its leaf into one primitive store per
/// little-endian footprint byte, each byte's index and value re-proving
/// their own obligations against the carrier's extent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedRepresentationViewStorePlan {
    pub statement_index: u32,
    pub destination: crate::CheckedStoreRoot,
    pub path: Vec<CheckedUnitStructuralPathSegment>,
    pub element: PrimitiveType,
    pub view: PrimitiveType,
    pub value: crate::CheckedStoreValue,
}
