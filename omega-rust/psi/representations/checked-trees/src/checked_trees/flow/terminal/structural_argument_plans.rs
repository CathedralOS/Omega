//! Structural argument plans: entry claims, call coordinates, scalar
//! arguments and structural argument sources.

use crate::CheckedScalarExpression;
use crate::checked_trees::flow::terminal::{
    CheckedStructuralAccess, CheckedUnitStructuralPathSegment,
};
use language_semantics::CarryPolicy;
use symbols::SymbolHandle;

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

/// One scalar operand of an invocation or array construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedCallScalarArgument {
    Pure(CheckedScalarExpression),
    /// Exact node selected by the operand coordinate's checked computation root.
    Computation(crate::CheckedScalarComputationHandle),
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
    /// Exact initialized mutable primitive storage in the caller state.
    PrimitiveLocal { symbol: SymbolHandle },
    /// Exact immutable structural declaration; the operation sequence owns its slot.
    StructuralLocal { symbol: SymbolHandle },
    /// Dense declaration ordinal in the caller's checked trivial-affine-local
    /// table. This source is always the exact whole local.
    TrivialAffineLocal { declaration_ordinal: u32 },
    /// Exact binding ordinal of an earlier whole structural call result.
    StructuralResult { binding_ordinal: u32 },
    /// Exact byte sequence passed directly to a bodyless boundary.
    ByteSequenceLiteral { bytes: Vec<u8> },
    /// Exclusive range over one whole immutable byte-view parameter. Missing
    /// endpoints mean zero and the source length, not absent runtime values.
    /// Present endpoints copy the unique source-bound scalar facts under
    /// ByteSequenceSubsliceStart/End at the enclosing call coordinate and
    /// dense structural argument ordinal. Parameter indices are state-local.
    ByteSequenceSubslice {
        parameter_index: u32,
        expression: typed_trees::expression::ExpressionHandle,
        start: Option<CheckedScalarExpression>,
        end: Option<CheckedScalarExpression>,
    },
    /// Exclusive element range over one whole immutable `&[T]` view
    /// parameter, T != u8. Endpoints share the byte subslice's
    /// source-bound scalar roles; the view's stored extent names its
    /// element count, not a byte length.
    ElementViewSubslice {
        parameter_index: u32,
        expression: typed_trees::expression::ExpressionHandle,
        start: Option<CheckedScalarExpression>,
        end: Option<CheckedScalarExpression>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedUnitStructuralArgumentPlan {
    pub source: CheckedUnitStructuralArgumentSourcePlan,
    /// Empty names the complete source. Accepted parameter projections retain
    /// exactly one literal fixed-array index or a finite nonempty record field
    /// path; trivial locals and literals require an empty path.
    pub path: Vec<CheckedUnitStructuralPathSegment>,
    pub type_identity: String,
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
            type_identity: String::new(),
            access: CheckedStructuralAccess::Owned,
        }
    }
}

impl CheckedUnitStructuralArgumentPlan {
    pub fn source_parameter_index(&self) -> Option<u32> {
        match self.source {
            CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
                Some(parameter_index)
            }
            CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { .. }
            | CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. }
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
            CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { .. }
            | CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. }
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
            CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { .. }
            | CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. }
            | CheckedUnitStructuralArgumentSourcePlan::Parameter { .. }
            | CheckedUnitStructuralArgumentSourcePlan::TrivialAffineLocal { .. }
            | CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { .. }
            | CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice { .. }
            | CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice { .. } => None,
        }
    }

    pub fn byte_sequence_literal(&self) -> Option<&[u8]> {
        match &self.source {
            CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { bytes } => Some(bytes),
            CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { .. }
            | CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. }
            | CheckedUnitStructuralArgumentSourcePlan::Parameter { .. }
            | CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice { .. }
            | CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice { .. }
            | CheckedUnitStructuralArgumentSourcePlan::TrivialAffineLocal { .. }
            | CheckedUnitStructuralArgumentSourcePlan::StructuralResult { .. } => None,
        }
    }
}
