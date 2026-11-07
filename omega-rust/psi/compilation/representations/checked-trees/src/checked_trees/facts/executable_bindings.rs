//! Final source occurrence bindings consumed by executable lowering.
//!
//! Checking freezes these after specialization and selection finalization.
//! Each row records a resolved supplier or selection; none grants authority,
//! copies storage, or supplies an alternative proof. Lowering never reopens
//! an authored expression or statement to reconstruct these bindings.
use arena::Handle;
use symbols::SymbolHandle;
use typed_trees::expression::{BuildSelectionRequest, CompletedValueCapture, ExpressionHandle};
use typed_trees::statement::TransitionTargetHandle;
use typed_trees::types::TypeReferenceHandle;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum CheckedExpressionSource {
    #[default]
    Invalid,
    Value,
    Saved {
        capture: Handle<CompletedValueCapture>,
        source: ExpressionHandle,
    },
    ProviderReadiness {
        selection: Handle<BuildSelectionRequest>,
        requirement: TypeReferenceHandle,
    },
}
impl CheckedExpressionSource {
    pub fn is_saved(self) -> bool {
        matches!(self, Self::Saved { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub enum CheckedCaptureArgumentValue {
    Type(typed_trees::type_identity::TypeIdentityId),
    /// Invalid when this operand has no closed static const identity.
    Const(typed_trees::type_identity::TypeIdentityId),
    Machine,
    RuntimeValue,
    Conformance,
}
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedCaptureArgument {
    pub parameter: SymbolHandle,
    pub value: CheckedCaptureArgumentValue,
}
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedCaptureInputs {
    pub instance: SymbolHandle,
    pub arguments: Vec<CheckedCaptureArgument>,
    pub machines: Vec<SymbolHandle>,
}
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedSelectedMachine {
    pub symbol: SymbolHandle,
    /// Display only; selection identity is the resolved symbol.
    pub diagnostic_path: String,
}
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedRequirementMachines {
    pub requirement: SymbolHandle,
    pub machines: Vec<CheckedSelectedMachine>,
}
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedSourceCallTarget {
    pub target: SymbolHandle,
    /// Original call label, used only by diagnostics.
    pub diagnostic_name: String,
}
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedSourceTransition {
    pub state: SymbolHandle,
    pub statement_index: usize,
    /// Exact flow-outgoing coordinate of the primary target.
    pub primary: TransitionTargetHandle,
    /// Resolved named targets, primary then continuation. Frame traversal
    /// distinguishes intra-machine transfers from calls through its owner.
    pub named_targets: Vec<SymbolHandle>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedExecutableBindings {
    // The handle beside each dense arena slot preserves generation identity.
    expressions: Vec<(ExpressionHandle, CheckedExpressionSource)>,
    pub captures: Vec<CheckedCaptureInputs>,
    pub requirements: Vec<CheckedRequirementMachines>,
    pub calls: Vec<CheckedSourceCallTarget>,
    pub transitions: Vec<CheckedSourceTransition>,
}
impl CheckedExecutableBindings {
    pub fn insert_expression(
        &mut self,
        expression: ExpressionHandle,
        source: CheckedExpressionSource,
    ) {
        let index = expression.arena_index() as usize;
        if self.expressions.len() <= index {
            self.expressions.resize(
                index + 1,
                (
                    ExpressionHandle::invalid(),
                    CheckedExpressionSource::Invalid,
                ),
            );
        }
        self.expressions[index] = (expression, source);
    }
    pub fn expression(&self, expression: ExpressionHandle) -> CheckedExpressionSource {
        self.expressions
            .get(expression.arena_index() as usize)
            .filter(|(handle, _)| expression.is_valid() && *handle == expression)
            .map_or(CheckedExpressionSource::Invalid, |(_, source)| *source)
    }
    pub fn transition(
        &self,
        state: SymbolHandle,
        statement_index: usize,
    ) -> Option<&CheckedSourceTransition> {
        self.transitions
            .iter()
            .find(|row| row.state == state && row.statement_index == statement_index)
    }
}
