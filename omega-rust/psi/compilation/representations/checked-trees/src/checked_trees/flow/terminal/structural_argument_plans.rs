//! Structural argument plans: entry claims, call coordinates, scalar
//! parameters and arguments, and structural argument sources.

use crate::checked_trees::flow::terminal::{
    CheckedStructuralAccess, CheckedUnitStructuralPathSegment,
};
use crate::{CheckedScalarExpression, CheckedStorageRoot};
use language_semantics::CarryPolicy;
use symbols::SymbolHandle;
use typed_trees::type_identity::TypeIdentityId;
use typed_trees::types::PrimitiveType;

/// Physical interpretation retained by one source-checked reference capture.
/// A view begins at the source's actual projected address. Aggregate views
/// obtain field encodings from their authored structural catalog; scalar views
/// carry their codec independently of the callee's logical parameter type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum CheckedReferenceStorage {
    #[default]
    Declared,
    /// Exact factory formation over its captured initialized ordinary schema.
    /// The result carrier selects the retained placement at publication.
    Placed,
    View {
        stored_scalar: Option<semantic_vocabulary::ScalarType>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitEntryClaimPlan {
    pub claim_identity: language_semantics::PermissionClaimIdentity,
    /// Dense index into `structural_parameters`.
    pub parameter_index: u32,
    /// Stable structural path below the parameter root. Cases and dynamic
    /// indexes reject rather than retaining source handles.
    pub path: Vec<CheckedUnitStructuralPathSegment>,
    pub carry: CarryPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedUnitCallCoordinate {
    pub statement_index: u32,
    pub call_ordinal: u32,
}

/// One scalar operand of an invocation, construction, or view formation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedCallScalarArgument {
    Pure(CheckedScalarExpression),
    /// Exact node selected by the operand coordinate's checked computation root.
    Computation(crate::CheckedScalarComputationHandle),
}

/// One scalar formal of a call target or an erased scalar formal of a body:
/// its authored parameter position and primitive carrier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedStructuralScalarParameterPlan {
    pub source_position: u32,
    pub primitive_type: PrimitiveType,
}

impl CheckedCallScalarArgument {
    pub fn as_pure(&self) -> Option<&CheckedScalarExpression> {
        match self {
            Self::Pure(expression) => Some(expression),
            Self::Computation(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedUnitStructuralArgumentSourcePlan {
    /// Dense index into the caller's structural parameter list.
    Parameter { parameter_index: u32 },
    /// Dense scalar ingress whose original storage is established before lending.
    ScalarParameter { parameter_index: u32 },
    /// Exact initialized mutable primitive storage in the caller state.
    PrimitiveLocal { symbol: SymbolHandle },
    /// Exact immutable structural declaration; the operation sequence owns its slot.
    StructuralLocal { symbol: SymbolHandle },
    /// A loan whose path starts at the checked logical referent of a local.
    /// Publication may retain a reference descriptor or present its referent
    /// directly. Lowering joins this exact root identity to the published
    /// carrier before interpreting the unchanged projection suffix. This is
    /// distinct from a physical stored-value path, not an initializer mode.
    StructuralLocalBorrow {
        symbol: SymbolHandle,
        root_type_identity: TypeIdentityId,
    },
    /// An already evaluated structural expression. The enclosing operation
    /// sequence establishes its storage before a consumer observes it; this
    /// identity never authorizes evaluating the expression again.
    StructuralExpression {
        expression: typed_trees::expression::ExpressionHandle,
    },
    /// Dense declaration ordinal in the caller's checked trivial-affine-local
    /// table. This source is always the exact whole local.
    TrivialAffineLocal { declaration_ordinal: u32 },
    /// Exact binding ordinal of an earlier whole structural call result.
    StructuralResult { binding_ordinal: u32 },
    /// Exact byte sequence passed directly to a bodyless boundary.
    ByteSequenceLiteral { bytes: Vec<u8> },
    /// Exclusive range over one established immutable byte view: a whole
    /// byte-view parameter (dense, state-local structural index) or an
    /// earlier byte-view local. Missing endpoints mean zero and the source
    /// length, not absent runtime values. Present endpoints retain the unique
    /// scalar operand under `SubsliceStart`/`SubsliceEnd` at the enclosing
    /// statement and this range's `CheckedSubsliceSite`. Calls remain computed
    /// operands, evaluated once in start/end order before narrowing the view.
    ByteSequenceSubslice {
        root: CheckedStorageRoot,
        expression: typed_trees::expression::ExpressionHandle,
        start: Option<CheckedCallScalarArgument>,
        end: Option<CheckedCallScalarArgument>,
        /// The authored range is `start..=end`. Lowering normalizes the end
        /// only after capturing its value, so the endpoint keeps its identity.
        end_inclusive: bool,
        /// The occurrence whose `SubsliceStart`/`SubsliceEnd` facts `start`
        /// and `end` were taken from: a call argument, a `let`, a returned
        /// expression or a transition argument.
        site: crate::CheckedSubsliceSite,
        /// Exact Terminal type identity of the collection the range reads:
        /// the peeled carrier of the projected backing, already enrolled in
        /// the checked structural type table. Naming it keeps the storage
        /// declaration rooted even when nothing else in the body spells the
        /// backing type (a whole-extent binding produces no subslice
        /// computation operands to root it incidentally).
        storage_type_identity: TypeIdentityId,
    },
    /// Exclusive element range over one established immutable `&[T]` view,
    /// T != u8, rooted exactly as a byte range is. Endpoints share the same
    /// source-bound scalar roles; the view's stored extent names its element
    /// count, not a byte length. A view local's range over a fixed-array
    /// field (`self.items[a..b]`) roots at the parameter owning the field,
    /// and the enclosing plan's `path` names the record-field projection to
    /// the array, which lowering views whole before narrowing it.
    ElementViewSubslice {
        root: CheckedStorageRoot,
        expression: typed_trees::expression::ExpressionHandle,
        start: Option<CheckedCallScalarArgument>,
        end: Option<CheckedCallScalarArgument>,
        end_inclusive: bool,
        site: crate::CheckedSubsliceSite,
        /// Exact Terminal type identity of the collection the range reads.
        storage_type_identity: TypeIdentityId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitStructuralArgumentPlan {
    pub source: CheckedUnitStructuralArgumentSourcePlan,
    /// Ordered declaration-resolved projection of the original source. A
    /// range retains its collection path here; its completed descriptor enters
    /// the callee whole through `call_projection`. Other arguments retain their
    /// exact field and fixed or captured-index path into the call.
    pub path: Vec<CheckedUnitStructuralPathSegment>,
    pub type_identity: TypeIdentityId,
    /// Explicit access presented at this call site. This is independently
    /// checked against both the source carrier and target parameter.
    pub access: CheckedStructuralAccess,
}

impl Default for CheckedUnitStructuralArgumentPlan {
    fn default() -> Self {
        Self {
            source: CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal {
                symbol: SymbolHandle::invalid(),
            },
            path: Vec::new(),
            type_identity: TypeIdentityId::default(),
            access: CheckedStructuralAccess::Owned,
        }
    }
}

impl CheckedUnitStructuralArgumentPlan {
    /// Projection applied when the completed argument enters its callee.
    /// A range's path selects its original collection during range production;
    /// the call instead receives the whole derived descriptor. Keeping these
    /// coordinates separate prevents replaying its selectors after endpoints
    /// or later arguments have changed the source bindings.
    pub fn call_projection(&self) -> &[super::CheckedUnitStructuralPathSegment] {
        match self.source {
            CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice { .. }
            | CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice { .. } => &[],
            _ => &self.path,
        }
    }

    /// Address operands the call schedule must capture, in projection order.
    /// Parameter and local SSA indices already name captured values; computed
    /// indices and stored-field observations require evaluation at this slot.
    pub fn call_selector_segments(
        &self,
    ) -> impl Iterator<Item = (usize, &super::CheckedUnitStructuralPathSegment)> {
        self.call_projection()
            .iter()
            .enumerate()
            .filter(|(_, segment)| {
                matches!(
                    segment,
                    super::CheckedUnitStructuralPathSegment::ComputedIndex(_)
                        | super::CheckedUnitStructuralPathSegment::RuntimeIndex(
                            super::CheckedRuntimeIndex::StructuralField(_)
                                | super::CheckedRuntimeIndex::StorageRead { .. }
                        )
                )
            })
    }

    /// Complete-source structural-result argument: an empty projection path.
    pub fn structural_result(
        binding_ordinal: u32,
        type_identity: TypeIdentityId,
        access: CheckedStructuralAccess,
    ) -> Self {
        Self {
            source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal },
            path: Vec::new(),
            type_identity,
            access,
        }
    }

    /// Evaluated scalar dependencies of this source, in authored order.
    /// Omitted range bounds obtain defaults from the source descriptor.
    pub fn scalar_operands(&self) -> impl Iterator<Item = &CheckedCallScalarArgument> {
        let endpoints = match &self.source {
            CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
                start, end, ..
            }
            | CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice { start, end, .. } => {
                [start.as_ref(), end.as_ref()]
            }
            _ => [None, None],
        };
        endpoints.into_iter().flatten()
    }

    /// Executable children of this argument, in its retained source order.
    /// Range endpoints and projected selectors share the same call-occurrence
    /// and dependency traversal instead of replaying their source expressions.
    pub fn computation_operands(
        &self,
    ) -> impl Iterator<Item = crate::CheckedScalarComputationHandle> + '_ {
        self.path
            .iter()
            .filter_map(|segment| match segment {
                super::CheckedUnitStructuralPathSegment::ComputedIndex(root) => Some(*root),
                _ => None,
            })
            .chain(self.scalar_operands().filter_map(|operand| match operand {
                CheckedCallScalarArgument::Computation(root) => Some(*root),
                CheckedCallScalarArgument::Pure(_) => None,
            }))
    }

    /// Source identity for the common established structural storage roster.
    /// Result ordinals remain the operation sequence's independent namespace.
    pub fn structural_storage_root(&self) -> Option<facts::PlaceRoot> {
        match self.source {
            CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol }
            | CheckedUnitStructuralArgumentSourcePlan::StructuralLocalBorrow { symbol, .. } => {
                Some(facts::PlaceRoot::Symbol(symbol))
            }
            CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { expression } => {
                Some(facts::PlaceRoot::Expression(expression))
            }
            _ => None,
        }
    }

    pub fn source_parameter_index(&self) -> Option<u32> {
        match self.source {
            CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
                Some(parameter_index)
            }
            CheckedUnitStructuralArgumentSourcePlan::ScalarParameter { .. }
            | CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { .. }
            | CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. }
            | CheckedUnitStructuralArgumentSourcePlan::StructuralLocalBorrow { .. }
            | CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { .. }
            | CheckedUnitStructuralArgumentSourcePlan::TrivialAffineLocal { .. }
            | CheckedUnitStructuralArgumentSourcePlan::StructuralResult { .. }
            | CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { .. }
            | CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice { .. }
            | CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice { .. } => None,
        }
    }

    pub fn source_local_declaration_ordinal(&self) -> Option<u32> {
        match self.source {
            CheckedUnitStructuralArgumentSourcePlan::TrivialAffineLocal {
                declaration_ordinal,
            } => Some(declaration_ordinal),
            CheckedUnitStructuralArgumentSourcePlan::ScalarParameter { .. }
            | CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { .. }
            | CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. }
            | CheckedUnitStructuralArgumentSourcePlan::StructuralLocalBorrow { .. }
            | CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { .. }
            | CheckedUnitStructuralArgumentSourcePlan::Parameter { .. }
            | CheckedUnitStructuralArgumentSourcePlan::StructuralResult { .. }
            | CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { .. }
            | CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice { .. }
            | CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice { .. } => None,
        }
    }

    pub fn source_structural_result_binding_ordinal(&self) -> Option<u32> {
        match self.source {
            CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } => {
                Some(binding_ordinal)
            }
            CheckedUnitStructuralArgumentSourcePlan::ScalarParameter { .. }
            | CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { .. }
            | CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. }
            | CheckedUnitStructuralArgumentSourcePlan::StructuralLocalBorrow { .. }
            | CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { .. }
            | CheckedUnitStructuralArgumentSourcePlan::Parameter { .. }
            | CheckedUnitStructuralArgumentSourcePlan::TrivialAffineLocal { .. }
            | CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { .. }
            | CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice { .. }
            | CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice { .. } => None,
        }
    }

    /// The exact Terminal type identity of the collection a subslice source
    /// reads; `None` for sources that carry no range.
    pub fn subslice_storage_type_identity(&self) -> Option<TypeIdentityId> {
        match &self.source {
            CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
                storage_type_identity,
                ..
            }
            | CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
                storage_type_identity,
                ..
            } => storage_type_identity
                .is_valid()
                .then_some(*storage_type_identity),
            _ => None,
        }
    }

    /// The logical root before a borrowed local's projection, not its final
    /// argument type or the possibly reference-shaped published home.
    pub fn borrowed_root_type_identity(&self) -> Option<TypeIdentityId> {
        match &self.source {
            CheckedUnitStructuralArgumentSourcePlan::StructuralLocalBorrow {
                root_type_identity,
                ..
            } => Some(*root_type_identity),
            _ => None,
        }
    }

    pub fn byte_sequence_literal(&self) -> Option<&[u8]> {
        match &self.source {
            CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { bytes } => Some(bytes),
            CheckedUnitStructuralArgumentSourcePlan::ScalarParameter { .. }
            | CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { .. }
            | CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. }
            | CheckedUnitStructuralArgumentSourcePlan::StructuralLocalBorrow { .. }
            | CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { .. }
            | CheckedUnitStructuralArgumentSourcePlan::Parameter { .. }
            | CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice { .. }
            | CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice { .. }
            | CheckedUnitStructuralArgumentSourcePlan::TrivialAffineLocal { .. }
            | CheckedUnitStructuralArgumentSourcePlan::StructuralResult { .. } => None,
        }
    }
}
