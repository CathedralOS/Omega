//! Fresh structural values and their selected scalar dispatch paths.

use crate::{CheckedOperatorUseFact, CheckedScalarComputationHandle, CheckedScalarDispatchPattern};
use arena::Arena;
use arena::{Handle, HandleSpan};
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, TableMatchArm};
use typed_trees::types::TypeReferenceHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedStructuralValuePlans {
    pub roots: Arena<CheckedStructuralValueRoot>,
    pub nodes: Arena<CheckedStructuralValue>,
    pub dispatch_arms: Arena<CheckedStructuralDispatchArm>,
    pub record_fields: Arena<CheckedStructuralRecordField>,
}

pub type CheckedStructuralValueHandle = Handle<CheckedStructuralValue>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedStructuralValueRoot {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub statement_ordinal: u32,
    pub expression: ExpressionHandle,
    pub type_reference: TypeReferenceHandle,
    pub root: CheckedStructuralValueHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedStructuralValue {
    pub expression: ExpressionHandle,
    pub kind: CheckedStructuralValueKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedStructuralValueKind {
    /// Capture existing borrowed storage into an owned permission carrier.
    /// A record field consumes this carrier, never a scalar referent snapshot.
    Reference {
        source: crate::CheckedUnitStructuralArgumentPlan,
    },
    Case(crate::CheckedScalarCaseConstruction),
    /// An owned construction of a named case whose payload fields are not all
    /// scalar. Each field composes exactly as it does inside a `Record` — a
    /// scalar computation or a nested structural value — while the selected
    /// case contributes the discriminated tag a bare field set cannot spell.
    StructuralCase {
        data_symbol: SymbolHandle,
        case: SymbolHandle,
        fields: HandleSpan<CheckedStructuralRecordField>,
    },
    Call {
        source_call: Handle<crate::FlowCallFact>,
    },
    Record {
        data_symbol: SymbolHandle,
        fields: HandleSpan<CheckedStructuralRecordField>,
    },
    /// Existing storage is transferred, never reconstructed as a fresh case.
    /// Conditional transfer and residual ownership come from checked flow.
    Place(crate::CheckedUnitStructuralArgumentPlan),
    /// A borrowed `&[T]` view of the contiguous elements an owned collection
    /// place holds. `source` is that lent place under its shared loan, taken
    /// from checked borrow admission rather than from the authored view
    /// spelling; the view carries its own stored length and copies no
    /// elements, so the lent storage keeps its owner and extent.
    BorrowedSliceView {
        source: crate::CheckedUnitStructuralArgumentPlan,
    },
    /// An owned child projected out of `source` (a `Place` or `Call` node)
    /// along an exact field/fixed-index path. The untouched residual siblings
    /// die on the selected edge; `type_identity` is the normalized projected
    /// (leaf) type, matching the enclosing result expectation.
    Projection {
        source: CheckedStructuralValueHandle,
        path: Vec<crate::CheckedUnitStructuralPathSegment>,
        type_identity: String,
    },
    Dispatch {
        subject: CheckedScalarComputationHandle,
        arms: HandleSpan<CheckedStructuralDispatchArm>,
    },
    /// An element-wise owned construction of a fixed array of structural
    /// values. Each element is its own structural node — a record literal,
    /// a place, a call product — so field scalars, ownership, and calls
    /// compose exactly as they do inside a record constructor; the array
    /// contributes only their ordered container.
    FixedArray {
        elements: Vec<CheckedStructuralValueHandle>,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedStructuralRecordField {
    pub field: SymbolHandle,
    pub expression: ExpressionHandle,
    pub type_reference: TypeReferenceHandle,
    pub value: CheckedStructuralRecordFieldValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedStructuralRecordFieldValue {
    Scalar(CheckedScalarComputationHandle),
    Structural(CheckedStructuralValueHandle),
}

impl Default for CheckedStructuralRecordFieldValue {
    fn default() -> Self {
        Self::Scalar(Handle::invalid())
    }
}

impl Default for CheckedStructuralValueKind {
    fn default() -> Self {
        Self::Case(crate::CheckedScalarCaseConstruction::default())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedStructuralDispatchArm {
    pub source_arm: Handle<TableMatchArm>,
    pub equality_use: Handle<CheckedOperatorUseFact>,
    pub pattern: CheckedScalarDispatchPattern,
    pub value: CheckedStructuralValueHandle,
}

impl CheckedStructuralValuePlans {
    /// Continuation and primary returns may share a statement, but never an
    /// authored value occurrence. Rejoin both coordinates without ambiguity.
    pub fn root_for_expression(
        &self,
        state: SymbolHandle,
        statement_ordinal: u32,
        expression: ExpressionHandle,
    ) -> Option<&CheckedStructuralValueRoot> {
        let mut roots = self.roots.iter().map(|(_, root)| root).filter(|root| {
            root.state == state
                && root.statement_ordinal == statement_ordinal
                && root.expression == expression
        });
        let root = roots.next()?;
        roots.next().is_none().then_some(root)
    }

    pub fn root_at(
        &self,
        state: SymbolHandle,
        statement_ordinal: u32,
    ) -> Option<&CheckedStructuralValueRoot> {
        let mut roots = self
            .roots
            .iter()
            .map(|(_, root)| root)
            .filter(|root| root.state == state && root.statement_ordinal == statement_ordinal);
        let root = roots.next()?;
        roots.next().is_none().then_some(root)
    }
}
