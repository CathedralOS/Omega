pub mod view;

use crate::FactHandle;
use arena::{Handle, HandleSpan};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::name::Identifier;
use typed_trees::types::TypeConstraintNode;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProgramPoint {
    #[default]
    Global,
    Definition {
        symbol: SymbolHandle,
    },
    Machine {
        machine_symbol: SymbolHandle,
    },
    State {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    },
    Statement {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
    },
    Call {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        call_ordinal: usize,
    },
    CallRequires {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        call_ordinal: usize,
    },
    CallEnsures {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        call_ordinal: usize,
    },
    TransitionArm {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        /// Invalid identifies the guard-false fallthrough to the next arm.
        transition_target: typed_trees::statement::TransitionTargetHandle,
    },
    Exit {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        transition_target: typed_trees::statement::TransitionTargetHandle,
    },
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FactRef {
    pub fact: FactHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FactContext {
    pub point: ProgramPoint,
    pub facts: HandleSpan<FactRef>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolFactSet {
    pub symbol: SymbolHandle,
    pub facts: HandleSpan<FactRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BooleanFact {
    pub expression: ExpressionHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DomainMembershipFact {
    pub value: ExpressionHandle,
    pub domain: HandleSpan<Identifier>,
    pub domain_symbol: SymbolHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeConstraintFact {
    pub constraint: Handle<TypeConstraintNode>,
}
