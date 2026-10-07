//! Checked store plans shared by the Unit-effect and scalar-graph planners:
//! the evaluated value and exact root of one scalar store, and the field
//! replacement plans built on them.

use symbols::SymbolHandle;

use crate::{
    CheckedScalarCaseFieldPlan, CheckedScalarExpression, CheckedUnitStructuralArgumentPlan,
};

use super::CheckedUnitStructuralPathSegment;

/// Evaluated source of one scalar store: a primitive field replacement, an
/// element-view write, a byte-view write, or a byte replacement inside a
/// bounded byte field. Every leaf shares this one vocabulary. The two byte
/// leaves lower no scalar computation, so their producer, replay and
/// lowering each refuse `Computation` explicitly; an element-view write may
/// carry one, and its replay and lowering refuse it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedStoreValue {
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

impl CheckedStoreValue {
    pub fn as_pure(&self) -> Option<&CheckedScalarExpression> {
        match self {
            Self::Pure(expression) => Some(expression),
            Self::Computation(_) | Self::ScalarResult { .. } => None,
        }
    }
}

/// Exact source root of one store: a primitive, field, case-field, element,
/// byte-view, byte-field or whole-byte-owner write. A parameter position is the
/// authored state position, never a dense structural-parameter index; a
/// consumer holding a dense roster maps the position through the checked
/// structural parameters, whose own `position` is authored too. A local keeps
/// symbol identity so ordered lowering can resolve its current home after
/// copies or moves. Byte-field source reads use that same home identity;
/// destination leaves without local-rooted lowering still refuse `Local`
/// explicitly in their producer, replay and lowering.
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

/// Checked custody for one replacement of an exact primitive field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralScalarFieldStorePlan {
    /// Exact completed source range check. Zero for deferred or unjudged stores.
    pub range_decision: arena::Handle<crate::CheckedIntegerAssignmentDecision>,
    pub statement_index: u32,
    pub destination: CheckedStoreRoot,
    /// Exact structural path from the destination root to the carrier;
    /// the final primitive field is retained separately below.
    pub carrier_path: Vec<CheckedUnitStructuralPathSegment>,
    pub field_identity: String,
    pub primitive_type: typed_trees::types::PrimitiveType,
    pub value: CheckedStoreValue,
}

/// Checked custody for one replacement of an exact unrestricted-sum field.
/// The destination carries the same root, path and write-frame custody a
/// scalar field store proves; the stored value is a whole owned place of the
/// field's declared sum type, so replacement copies every bit and moves
/// nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralCaseFieldStorePlan {
    pub statement_index: u32,
    pub destination: CheckedStoreRoot,
    /// Exact structural path from the destination root to the carrier;
    /// the final sum-typed field is retained separately below.
    pub carrier_path: Vec<CheckedUnitStructuralPathSegment>,
    pub field_identity: String,
    /// Whole owned place supplying the stored value; its checked type is the
    /// field's declared unrestricted sum type.
    pub value: CheckedUnitStructuralArgumentPlan,
}

/// Checked custody for one replacement of an exact sum field by an authored
/// case literal: `place.field = Sum::Case { .. }`. No held place supplies
/// the stored value — the store itself establishes the literal into the
/// field's storage — so the plan retains the authored construction and its
/// scalar operands instead of a source argument. Each operand was bound at
/// the statement under `StructuralValueField { expression: literal, .. }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStructuralCaseLiteralFieldStorePlan {
    pub statement_index: u32,
    pub destination: CheckedStoreRoot,
    /// Exact structural path from the destination root to the carrier;
    /// the final sum-typed field is retained separately below.
    pub carrier_path: Vec<CheckedUnitStructuralPathSegment>,
    pub field_identity: String,
    /// The authored case literal — the namespace subject every scalar operand
    /// bound against under `StructuralValueField`.
    pub literal: typed_trees::expression::ExpressionHandle,
    pub case_identity: String,
    /// Exact type identity the windowed field pair names: the literal's
    /// normalized type, which admission proved equals the field's declared
    /// sum type.
    pub type_identity: String,
    /// Scalar operands in authored order, covering payload and named common
    /// fields; each `field_identity` matches its terminal declaration.
    pub fields: Vec<CheckedScalarCaseFieldPlan>,
}
