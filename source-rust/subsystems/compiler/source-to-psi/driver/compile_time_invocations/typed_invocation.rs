//! A compile-time expression runs as the return value of an ordinary typed
//! caller. Its generated declarations inherit the requesting source owner.

use crate::source_state::TypedTrees;
use crate::source_state::expression::ExpressionHandle;
use crate::source_state::machine::Machine;
use crate::source_state::name::Identifier;
use crate::source_state::state::State;
use crate::source_state::statement::{StatementNode, TableTransition, TransitionTargetNode};
use crate::source_state::types::TypeReferenceHandle;
use symbols::{SymbolHandle, SymbolKind};

pub(super) struct TypedInvocationDeclaration {
    pub(super) symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    name: Identifier,
}

impl TypedInvocationDeclaration {
    /// A selected call needs its lexical owner while resolving the signature
    /// that will supply this caller's lifetime parameters.
    pub(super) fn new(program: &mut TypedTrees, requester: SymbolHandle, name: &str) -> Self {
        let symbol =
            program
                .symbols
                .insert_generated_root_from(requester, SymbolKind::Machine, name);
        let state_symbol = program
            .symbols
            .insert_generated_children(symbol, [(SymbolKind::State, name)])
            .start();
        Self {
            symbol,
            state_symbol,
            name: Identifier::generated(name),
        }
    }

    /// Register the caller's binders before instantiating any types in its body.
    pub(super) fn define(
        self,
        program: &mut TypedTrees,
        return_type: TypeReferenceHandle,
        lifetime_parameters: Vec<Identifier>,
    ) -> TypedInvocationBody {
        let machine = Machine {
            symbol: self.symbol,
            name: self.name.clone(),
            lifetime_parameters,
            ..Default::default()
        };
        let state = State {
            symbol: self.state_symbol,
            name: self.name,
            return_type,
            ..Default::default()
        };
        program.push_machine(machine.clone());
        TypedInvocationBody { machine, state }
    }
}

pub(super) struct TypedInvocationBody {
    pub(super) machine: Machine,
    pub(super) state: State,
}

impl TypedInvocationBody {
    pub(super) fn return_expression(
        mut self,
        program: &mut TypedTrees,
        expression: ExpressionHandle,
    ) -> Result<SymbolHandle, String> {
        let owner = self.machine.symbol;
        let positions = program.machine_positions_for_symbol(owner);
        let [position] = positions.as_slice() else {
            return Err("source invocation lost its unique generated caller declaration".into());
        };
        let target = program
            .statement_table
            .insert_transition_target(TransitionTargetNode::Value(expression));
        let source_span = program.expression_table.source_span(expression);
        program.statement_table.push_statement(
            &mut self.state.statement_nodes,
            StatementNode::Transition(TableTransition {
                target,
                source_span,
                ..Default::default()
            }),
        );
        program.push_machine_state(&mut self.machine, self.state);
        program.set_machine(*position as usize, self.machine);
        Ok(owner)
    }
}
