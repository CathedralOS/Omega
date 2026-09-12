//! Executable scalar evaluation, separate from pure proposition expressions.

use super::*;
use crate::CheckedUnitStructuralArgumentPlan;
use typed_trees::types::PrimitiveType;

pub type CheckedScalarComputationHandle = Handle<CheckedScalarComputation>;

/// A real structural value whose scalar payload is evaluated in authored order.
/// Field and case symbols retain nominal identity; no tag occupies a scalar
/// parameter slot, and evaluation is not permission to copy an affine value.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedScalarCaseConstruction {
    pub expression: typed_trees::expression::ExpressionHandle,
    pub type_reference: typed_trees::types::TypeReferenceHandle,
    pub case: SymbolHandle,
    pub fields: HandleSpan<CheckedScalarCaseComputationField>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedScalarCaseComputationField {
    pub symbol: SymbolHandle,
    pub value: CheckedScalarComputationHandle,
}

/// A structural operand names existing storage or constructs a value at its
/// authored evaluation point, including inside selective control. Call formal
/// positions and membership observations reuse these operands without inventing
/// source locals or assigning a structural value a scalar binding slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedScalarComputationStructuralArgument {
    Place(CheckedUnitStructuralArgumentPlan),
    Case(CheckedScalarCaseConstruction),
    Array {
        expression: typed_trees::expression::ExpressionHandle,
        /// Retains every dimension and the primitive carrier even with no leaves.
        type_reference: typed_trees::types::TypeReferenceHandle,
        elements: HandleSpan<CheckedScalarComputationHandle>,
    },
}

impl Default for CheckedScalarComputationStructuralArgument {
    fn default() -> Self {
        Self::Place(CheckedUnitStructuralArgumentPlan::default())
    }
}

impl CheckedScalarComputationStructuralArgument {
    pub fn as_place(&self) -> Option<&CheckedUnitStructuralArgumentPlan> {
        match self {
            Self::Place(place) => Some(place),
            Self::Array { .. } | Self::Case(_) => None,
        }
    }

    pub fn as_place_mut(&mut self) -> Option<&mut CheckedUnitStructuralArgumentPlan> {
        match self {
            Self::Place(place) => Some(place),
            Self::Array { .. } | Self::Case(_) => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedScalarComputationPlans {
    pub roots: Arena<CheckedScalarComputationRoot>,
    pub nodes: Arena<CheckedScalarComputation>,
    pub operands: Arena<CheckedScalarComputationHandle>,
    pub structural_arguments: Arena<CheckedScalarComputationStructuralArgument>,
    pub dispatch_arms: Arena<CheckedScalarDispatchArm>,
    pub case_fields: Arena<CheckedScalarCaseComputationField>,
}

impl CheckedScalarComputationPlans {
    pub fn root_at(
        &self,
        state: SymbolHandle,
        statement_ordinal: u32,
        role: CheckedScalarExpressionRole,
    ) -> Option<&CheckedScalarComputationRoot> {
        let mut roots = self.roots.iter().map(|(_, root)| root).filter(|root| {
            root.state == state && root.statement_ordinal == statement_ordinal && root.role == role
        });
        let root = roots.next()?;
        roots.next().is_none().then_some(root)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedScalarComputationRoot {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub statement_ordinal: u32,
    pub role: CheckedScalarExpressionRole,
    pub root: CheckedScalarComputationHandle,
}

impl Default for CheckedScalarComputationRoot {
    fn default() -> Self {
        Self {
            machine: SymbolHandle::invalid(),
            state: SymbolHandle::invalid(),
            statement_ordinal: 0,
            role: CheckedScalarExpressionRole::Return,
            root: Handle::invalid(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedScalarComputation {
    /// Exact authored expression for a destination or call-argument root.
    /// Other intermediate nodes use the zero handle; source spans are not identity.
    pub authored_root: typed_trees::expression::ExpressionHandle,
    /// Pure value occurrence, separate from an enclosing folded destination.
    /// Generated short-circuit constants and non-value nodes use the zero handle.
    pub value_source: typed_trees::expression::ExpressionHandle,
    pub primitive_type: PrimitiveType,
    pub kind: CheckedScalarComputationKind,
}

impl Default for CheckedScalarComputation {
    fn default() -> Self {
        Self {
            authored_root: Handle::invalid(),
            value_source: Handle::invalid(),
            primitive_type: PrimitiveType::Bool,
            kind: CheckedScalarComputationKind::Value(CheckedScalarExpression::Boolean(Box::new(
                CheckedBooleanExpression::Constant(false),
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedScalarComputationKind {
    /// Read one scalar field at its authored evaluation point without moving
    /// the containing owner or assigning it a scalar parameter position.
    StructuralField {
        source_expression: typed_trees::expression::ExpressionHandle,
        subject: CheckedUnitStructuralArgumentPlan,
        field: SymbolHandle,
    },
    /// Observe an established structural operand without consuming its owner.
    CaseMembership {
        source_expression: typed_trees::expression::ExpressionHandle,
        subject: CheckedScalarComputationStructuralArgument,
        case: SymbolHandle,
    },
    SelectedComparison {
        operator_use: Handle<crate::CheckedOperatorUseFact>,
        left: CheckedScalarComputationHandle,
        right: CheckedScalarComputationHandle,
    },
    /// Representation-identical semantic qualification transfer at one authored
    /// cast, including explicit non-owning erasure to a bare result.
    /// The full result reference retains domain instances; this node does not
    /// establish membership or authorize erasing the qualification at publication.
    Qualification {
        source_expression: typed_trees::expression::ExpressionHandle,
        operand: CheckedScalarComputationHandle,
        result_type: typed_trees::types::TypeReferenceHandle,
    },
    /// Pure source expression in the enclosing state's scalar namespace.
    Value(CheckedScalarExpression),
    /// Save the subject once, then test arms in order and evaluate one result.
    Dispatch {
        source_expression: typed_trees::expression::ExpressionHandle,
        subject: CheckedScalarComputationHandle,
        arms: HandleSpan<CheckedScalarDispatchArm>,
    },
    Call {
        source_call: Handle<crate::FlowCallFact>,
        target_machine: SymbolHandle,
        target_state: SymbolHandle,
        /// Authored occurrence identity, not its position in execution order.
        call_ordinal: u32,
        /// Dense scalar operands. The exact callee signature interleaves these
        /// with structural arguments in authored formal-position order.
        arguments: HandleSpan<CheckedScalarComputationHandle>,
        structural_arguments: HandleSpan<CheckedScalarComputationStructuralArgument>,
    },
    Select {
        /// Exact conditional occurrence, retained when an enclosing constant
        /// condition folds away and the destination root names that outer form.
        source_expression: typed_trees::expression::ExpressionHandle,
        condition: CheckedScalarComputationHandle,
        when_true: CheckedScalarComputationHandle,
        when_false: CheckedScalarComputationHandle,
    },
    /// Evaluate operands left-to-right once, then apply a pure template whose
    /// Parameter positions name only these computed operands, not source locals.
    Apply {
        /// Exact operation occurrence, retained when an enclosing selection folds.
        source_expression: typed_trees::expression::ExpressionHandle,
        expression: CheckedScalarExpression,
        operands: HandleSpan<CheckedScalarComputationHandle>,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedScalarDispatchArm {
    /// Zero for builtin comparisons; otherwise the exact implicit arm use.
    pub equality_use: Handle<crate::CheckedOperatorUseFact>,
    pub source_arm: Handle<typed_trees::expression::TableMatchArm>,
    pub pattern: CheckedScalarDispatchPattern,
    pub value: CheckedScalarComputationHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum CheckedScalarDispatchPattern {
    Value(CheckedScalarComputationHandle),
    #[default]
    Wildcard,
}
