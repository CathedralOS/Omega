//! Checked store plans: one authored store `place = value` as one operation
//! over its exact destination place and the value it receives.
//!
//! The destination's leaf representation decides the emitted Terminal
//! operation: plain primitive storage, a declared scalar field, an indexed
//! byte or element-view element, a representation view's re-covered leaf,
//! byte contents, a byte-view descriptor slot, a whole sum field, the hole of
//! an open borrowed window, or a whole owned local. Every
//! judgment the store consumes — a declared field's receiving range, an
//! indexed element's bound and byte-predicate preservation, a view's
//! footprint width — is data on that leaf, not a separate store family.

use symbols::SymbolHandle;
use typed_trees::types::PrimitiveType;

use crate::{
    CheckedByteSequenceValuePlan, CheckedScalarCaseFieldPlan, CheckedScalarExpression,
    CheckedScalarExpressionRole, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralParameterPlan,
};

use super::CheckedUnitStructuralPathSegment;
use crate::checked_trees::CheckedMemberIdentity;
use typed_trees::type_identity::TypeIdentityId;

/// One authored store: the exact place it replaces and the value it writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStorePlan {
    pub statement_index: u32,
    pub destination: CheckedStoreDestination,
    pub value: CheckedStoreValue,
}

/// The exact place one store replaces: its root, the checked path below it,
/// and the representation of the replaced leaf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStoreDestination {
    pub root: CheckedStoreRoot,
    /// Exact checked path from the root to the replaced place. An indexed
    /// element's own selection is its leaf's `index`, so the path of a
    /// `ByteElement` or `ViewElement` leaf ends at the indexed carrier.
    pub path: Vec<CheckedUnitStructuralPathSegment>,
    pub leaf: CheckedStoreLeaf,
}

impl CheckedStorePlan {
    /// The whole owned binding a window repair or local rebind installs.
    pub fn installed_value(&self) -> Option<&CheckedUnitStructuralArgumentPlan> {
        match (&self.destination.leaf, &self.value) {
            (
                CheckedStoreLeaf::StructuralStore { .. } | CheckedStoreLeaf::OwnedRebind { .. },
                CheckedStoreValue::Place(value),
            ) => Some(value),
            _ => None,
        }
    }

    pub fn installed_value_mut(&mut self) -> Option<&mut CheckedUnitStructuralArgumentPlan> {
        match (&self.destination.leaf, &mut self.value) {
            (
                CheckedStoreLeaf::StructuralStore { .. } | CheckedStoreLeaf::OwnedRebind { .. },
                CheckedStoreValue::Place(value),
            ) => Some(value),
            _ => None,
        }
    }

    /// The window ledger's spelling of a window repair's or local rebind's
    /// destination: a parameter root as its index in the dense
    /// `structural_parameters` roster the ledger and cleanup actions name,
    /// or the local itself.
    pub fn window_place(
        &self,
        structural_parameters: &[CheckedUnitStructuralParameterPlan],
    ) -> Option<CheckedUnitStructuralArgumentPlan> {
        let (CheckedStoreLeaf::StructuralStore { type_identity, .. }
        | CheckedStoreLeaf::OwnedRebind { type_identity }) = &self.destination.leaf
        else {
            return None;
        };
        let source = match self.destination.root {
            CheckedStoreRoot::Parameter { position } => {
                CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index: u32::try_from(
                        structural_parameters
                            .iter()
                            .position(|parameter| parameter.position == position)?,
                    )
                    .ok()?,
                }
            }
            CheckedStoreRoot::Local { symbol } => {
                CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol }
            }
        };
        Some(CheckedUnitStructuralArgumentPlan {
            source,
            path: self.destination.path.clone(),
            type_identity: type_identity.clone(),
            access: crate::CheckedStructuralAccess::Owned,
        })
    }
}

impl CheckedStoreDestination {
    /// The carrier path and identity of a leaf that is a record field: the
    /// path's final `Field` segment.
    pub fn field(&self) -> Option<(&[CheckedUnitStructuralPathSegment], &CheckedMemberIdentity)> {
        match self.path.split_last()? {
            (CheckedUnitStructuralPathSegment::Field(identity), carrier) => {
                Some((carrier, identity))
            }
            _ => None,
        }
    }
}

/// The declaration-selected interval and the completed judgment of this exact
/// authored assignment. This is source proof custody, not a Terminal storage
/// refinement: primitive type identity deliberately looks through qualification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPrimitiveReceivingRange {
    pub primitive: PrimitiveType,
    pub integer: semantic_vocabulary::BoundedIntegerType,
    pub decision: arena::Handle<crate::CheckedIntegerAssignmentDecision>,
}

/// The representation of the place one store replaces, carrying the checked
/// judgments that representation's store consumes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedStoreLeaf {
    /// Primitive storage with no declared field contract: a whole mutable
    /// primitive local or borrowed primitive, or the primitive array element
    /// or plain IEEE field a primitive projection reaches.
    Primitive {
        /// Exact indexed write's completed byte-predicate preservation judgment.
        preserves_byte_memberships: bool,
        /// Scalar qualification constrains this source write without changing
        /// primitive storage identity. Lowering consumes its completed receiving
        /// judgment before emitting the ordinary physical store.
        receiving_range: Option<CheckedPrimitiveReceivingRange>,
    },
    /// A declared primitive field, the path's final segment. The receiving
    /// range judgment is the exact completed source check; zero for a
    /// deferred or unjudged store.
    ScalarField {
        primitive_type: PrimitiveType,
        range_decision: arena::Handle<crate::CheckedIntegerAssignmentDecision>,
    },
    /// One live byte of the byte carrier the path names — a whole mutable
    /// view or bounded owner (an empty path), or a byte field — selected by
    /// `index` and stored as a `u8`.
    ByteElement {
        index: CheckedScalarExpression,
        decisions: CheckedIndexedWriteDecisions,
    },
    /// One element of the mutable `&mut [T]` view at the root, selected by
    /// `index`, then `element_path` within that element (empty for a scalar
    /// element). The live-length observation composes with the write; the
    /// element's receiving range is the separate `range_decision`.
    ViewElement {
        index: CheckedScalarExpression,
        element_path: Vec<CheckedUnitStructuralPathSegment>,
        element: PrimitiveType,
        range_decision: arena::Handle<crate::CheckedIntegerAssignmentDecision>,
        decisions: CheckedIndexedWriteDecisions,
    },
    /// The primitive leaf a `&mut`/`&write` representation view re-covers:
    /// a whole primitive field of the view's width, or one `u8` element
    /// below a bounded byte field. `view` is the referee's declared
    /// primitive, the value's carrier; a view wider than its leaf lowers to
    /// one store per little-endian footprint byte.
    RepresentationView { view: PrimitiveType },
    /// The live contents of a bounded-owned byte carrier: the whole root (an
    /// empty path) or a byte field.
    ByteContents,
    /// One shared borrowed-view descriptor slot: a whole declared descriptor
    /// local (an empty path) or a record's view field. No bytes are copied;
    /// loan replay proves the stored descriptor's source outlives the slot.
    ByteDescriptor,
    /// A whole sum-typed field, the path's final segment, replaced through
    /// its borrowed window: the stored value is a whole owned place copied
    /// in, or a case literal established in place.
    SumField,
    /// Install a completed whole value at its captured structural destination.
    /// An extraction binding identifies restoration debt. Without one, owned
    /// storage has its checked old-value disposition, while intact borrowed
    /// storage requires unrestricted contents with no cleanup obligation.
    StructuralStore {
        type_identity: TypeIdentityId,
        /// Custody of the completed incoming value, including a selected child.
        multiplicity: language_semantics::Multiplicity,
        /// Result coordinate of the extraction that opened this hole. The
        /// source checker establishes storage equality; lowering must not
        /// infer it from selector evaluation coordinates. None denotes an
        /// store with no extraction debt: owned old contents were disposed in
        /// place, or unrestricted borrowed contents need no displacement.
        opening_binding: Option<u32>,
    },
    /// A whole owned binding (local or formal, with an empty path) rebound to a whole owned value
    /// after its prior occurrence's disposal. Unlike a window repair, the
    /// binding's value identity changes and no restoration debt opens at
    /// another owner's home.
    OwnedRebind { type_identity: TypeIdentityId },
}

/// The value one store writes, in the shape its leaf receives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedStoreValue {
    /// A scalar evaluated at `role`: the assignment's `AssignmentValue`, or
    /// one row-major element of a closed array literal stored element-wise.
    Scalar {
        role: CheckedScalarExpressionRole,
        value: CheckedScalarStoreValue,
    },
    /// A byte value replacing contents or a descriptor slot.
    Bytes(CheckedByteSequenceValuePlan),
    /// A whole place: for a sum field, an owned or shared place of its
    /// declared unrestricted sum type, copied whole without moving; for a
    /// window repair or local rebind, the whole owned binding installed.
    Place(CheckedUnitStructuralArgumentPlan),
    /// An authored case literal the store establishes into the leaf's storage.
    CaseLiteral(CheckedCaseLiteralValuePlan),
}

impl CheckedStoreValue {
    /// The scalar an authored assignment's right-hand side supplies.
    pub fn assigned(value: CheckedScalarStoreValue) -> Self {
        Self::Scalar {
            role: CheckedScalarExpressionRole::AssignmentValue,
            value,
        }
    }

    /// The scalar a scalar leaf stores, with its evaluation role.
    pub fn scalar(&self) -> Option<(CheckedScalarExpressionRole, &CheckedScalarStoreValue)> {
        match self {
            Self::Scalar { role, value } => Some((*role, value)),
            Self::Bytes(_) | Self::Place(_) | Self::CaseLiteral(_) => None,
        }
    }

    /// The byte value a contents or descriptor leaf stores.
    pub fn bytes(&self) -> Option<&CheckedByteSequenceValuePlan> {
        match self {
            Self::Bytes(value) => Some(value),
            Self::Scalar { .. } | Self::Place(_) | Self::CaseLiteral(_) => None,
        }
    }

    pub fn bytes_mut(&mut self) -> Option<&mut CheckedByteSequenceValuePlan> {
        match self {
            Self::Bytes(value) => Some(value),
            Self::Scalar { .. } | Self::Place(_) | Self::CaseLiteral(_) => None,
        }
    }
}

/// `place.field = Sum::Case { .. }`: no held place supplies the stored value,
/// so the plan retains the authored construction and its scalar operands.
/// Each operand was bound at the statement under
/// `StructuralValueField { expression: literal, .. }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedCaseLiteralValuePlan {
    /// The authored case literal — the namespace subject every scalar operand
    /// bound against under `StructuralValueField`.
    pub literal: typed_trees::expression::ExpressionHandle,
    pub case_identity: CheckedMemberIdentity,
    /// Exact type identity of the literal's normalized type, which admission
    /// proved equals the field's declared sum type.
    pub type_identity: TypeIdentityId,
    /// Scalar operands in authored order, covering payload and named common
    /// fields; each `field_identity` matches its terminal declaration.
    pub fields: Vec<CheckedScalarCaseFieldPlan>,
}

/// What checking already proved about one runtime-indexed write, recorded
/// against the authored indexed target so lowering never repeats it. The two
/// judgments stay independent: an in-bounds index supplies the count
/// conversion but cannot authorize a byte that violates the owner's byte
/// predicates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CheckedIndexedWriteDecisions {
    /// The index is proven within the destination's live extent.
    pub index_in_bounds: bool,
    /// The stored byte is proven to preserve the owner's byte predicates.
    /// Element-view writes keep this `false`; their receiving range is the
    /// separate `range_decision`.
    pub predicates_preserved: bool,
}

/// Evaluated source of one scalar store. Every scalar leaf shares this one
/// vocabulary. A representation view lowers no scalar computation; its
/// producer refuses `Computation`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedScalarStoreValue {
    /// A bound pure authored expression at the store's value role.
    Pure(CheckedScalarExpression),
    /// A scalar computation rooted at the store's own role coordinate.
    Computation(crate::CheckedScalarComputationHandle),
    /// The already-defined SSA result of the scalar call this same statement
    /// performs, named by its dense position in the consuming plan's scalar
    /// namespace -- the namespace `CheckedScalarExpression::Local` indexes,
    /// scalar parameters first and then each established scalar result in
    /// order. The authored source has no local binding to name, so no
    /// `AssignmentValue` scalar-expression row exists for it; the receiving
    /// lowerer reconstructs the value from the call operation it already
    /// emitted for this statement instead of from an authored expression.
    ScalarResult { position: u32 },
}

impl CheckedScalarStoreValue {
    pub fn as_pure(&self) -> Option<&CheckedScalarExpression> {
        match self {
            Self::Pure(expression) => Some(expression),
            Self::Computation(_) | Self::ScalarResult { .. } => None,
        }
    }
}

/// Exact source root of one store. A parameter position is the authored
/// state position, never a dense structural-parameter index; a consumer
/// holding a dense roster maps the position through the checked structural
/// parameters, whose own `position` is authored too. A local keeps symbol
/// identity so ordered lowering can resolve its current home after copies or
/// moves. Byte-field source reads use that same home identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedStoreRoot {
    Parameter { position: u32 },
    Local { symbol: SymbolHandle },
}

impl CheckedStoreRoot {
    /// Parameter-only consumers reject local storage instead of inventing an index.
    pub fn parameter_position(self) -> Option<u32> {
        match self {
            Self::Parameter { position } => Some(position),
            Self::Local { .. } => None,
        }
    }
}
