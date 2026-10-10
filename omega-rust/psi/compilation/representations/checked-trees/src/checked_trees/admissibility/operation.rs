use crate::{
    AcceptanceSummary, AcceptanceVerdict, AcceptanceView, CallAcceptance,
    OperatorAcceptance, StateOperationAcceptance, StateOperationAcceptanceKind,
};

impl<'facts> AcceptanceView for StateOperationAcceptance<'facts> {
    fn summary(&self) -> AcceptanceSummary {
        match self {
            Self::Statement(statement) => statement.summary(),
            Self::Call(call) => call.summary(),
            Self::Exit(exit) => exit.summary(),
            Self::Operator(operator) => operator.summary(),
        }
    }
}

impl<'facts> StateOperationAcceptance<'facts> {
    super::acceptance_accessors!();

    pub const fn kind(&self) -> StateOperationAcceptanceKind {
        match self {
            Self::Statement(_) => StateOperationAcceptanceKind::Statement,
            Self::Call(_) => StateOperationAcceptanceKind::Call,
            Self::Exit(_) => StateOperationAcceptanceKind::Exit,
            Self::Operator(_) => StateOperationAcceptanceKind::Operator,
        }
    }

    pub const fn as_call(&self) -> Option<CallAcceptance<'facts>> {
        match self {
            Self::Call(call) => Some(*call),
            Self::Statement(_) | Self::Exit(_) | Self::Operator(_) => None,
        }
    }

    pub const fn as_operator(&self) -> Option<OperatorAcceptance<'facts>> {
        match self {
            Self::Operator(operator) => Some(*operator),
            Self::Statement(_) | Self::Call(_) | Self::Exit(_) => None,
        }
    }

    pub fn statement_index(&self) -> usize {
        match self {
            Self::Statement(statement) => statement.statement().statement_index,
            Self::Call(call) => call.call().statement_index,
            Self::Exit(exit) => exit.exit().statement_index,
            Self::Operator(operator) => match operator.operator_use().origin {
                crate::CheckedValueOrigin::StateStatement {
                    statement_index, ..
                } => statement_index,
                _ => 0,
            },
        }
    }
}
