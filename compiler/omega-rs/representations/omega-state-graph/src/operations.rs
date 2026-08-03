use psi_arena::HandleSpan;
use psi_symbols::SymbolHandle;
use psi_typed_trees::expression::ExpressionHandle;
use psi_typed_trees::name::Identifier;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    pub statement_index: usize,
    pub kind: OperationKind,
    pub expressions: OperationExpressionRefs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationKind {
    Assignment,
    Call {
        receiver_symbol: SymbolHandle,
        target_symbol: SymbolHandle,
        has_receiver: bool,
        receiver: Identifier,
        target: Identifier,
    },
    ConstantIntegerAssignment,
    Expression,
    LocalData,
    StaticAssignment,
}

impl Default for Operation {
    fn default() -> Self {
        Self {
            statement_index: 0,
            kind: OperationKind::LocalData,
            expressions: OperationExpressionRefs::None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OperationExpressionRefs {
    Assignment {
        target: ExpressionHandle,
        value: ExpressionHandle,
    },
    Call {
        arguments: HandleSpan<ExpressionHandle>,
    },
    Expression(ExpressionHandle),
    #[default]
    None,
}
