//! Ordered source call coordinates for cross-machine interval arrivals.
//!
//! This inventory carries no intervals or caller premises. Each consumer still
//! checks the original call, guard, argument bindings and callee entry.

use crate::CheckingInput;
use std::sync::Arc;
use symbols::{SymbolHandle, SymbolKeyMap};
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::{StatementNode, TransitionTargetHandle, TransitionTargetNode};

#[derive(Clone, Copy)]
pub(super) enum ArrivalSiteKind {
    StatementCall,
    Primary,
    Continuation,
}

pub(super) struct ArrivalSite {
    pub caller: usize,
    pub state: usize,
    pub statement: usize,
    pub kind: ArrivalSiteKind,
}

#[derive(Default)]
pub(crate) struct CrossMachineArrivalSites {
    by_entry: SymbolKeyMap<(SymbolHandle, SymbolHandle), Vec<ArrivalSite>>,
}

pub(super) fn prepared(program: &dyn CheckingInput) -> Arc<CrossMachineArrivalSites> {
    crate::frozen_program::frozen_memo(
        program,
        |memos| &memos.cross_machine_arrival_sites,
        || CrossMachineArrivalSites::build(program),
    )
}

impl CrossMachineArrivalSites {
    pub(super) fn for_entry(&self, machine: SymbolHandle, state: SymbolHandle) -> &[ArrivalSite] {
        self.by_entry
            .get(&(machine, state))
            .map_or(&[], Vec::as_slice)
    }

    fn build(program: &dyn CheckingInput) -> Self {
        let mut sites = Self::default();
        for (caller, machine) in program.machines().iter().enumerate() {
            for (state, body) in program.machine_states(machine).iter().enumerate() {
                for (statement, node) in program
                    .statement_table
                    .indexed_statements(body.statement_nodes)
                {
                    match node {
                        StatementNode::Call(call) => sites.record(
                            program,
                            call.target_symbol,
                            ArrivalSite {
                                caller,
                                state,
                                statement,
                                kind: ArrivalSiteKind::StatementCall,
                            },
                        ),
                        StatementNode::Transition(transition) => {
                            for (target, kind) in [
                                (transition.target, ArrivalSiteKind::Primary),
                                (transition.continuation, ArrivalSiteKind::Continuation),
                            ] {
                                if let Some(symbol) = target_symbol(program, target) {
                                    sites.record(
                                        program,
                                        symbol,
                                        ArrivalSite {
                                            caller,
                                            state,
                                            statement,
                                            kind,
                                        },
                                    );
                                }
                            }
                        }
                        // The arrival owner never harvested nested calls in
                        // these forms. A broader call inventory would add facts.
                        StatementNode::Expression(_)
                        | StatementNode::LocalData(_)
                        | StatementNode::Assignment(_)
                        | StatementNode::Assembly(_)
                        | StatementNode::AssemblyFact(_) => {}
                    }
                }
            }
        }
        sites
    }

    fn record(&mut self, program: &dyn CheckingInput, symbol: SymbolHandle, site: ArrivalSite) {
        let Some(state) = crate::call_target_state(program, symbol) else {
            return;
        };
        let Some(machine) = program.machine_by_symbol(program.symbols.get(state.symbol).parent)
        else {
            return;
        };
        self.by_entry
            .entry((machine.symbol, state.symbol))
            .or_default()
            .push(site);
    }
}

fn target_symbol(
    program: &dyn CheckingInput,
    target: TransitionTargetHandle,
) -> Option<SymbolHandle> {
    if !target.is_valid() {
        return None;
    }
    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named { path, .. } => Some(path.symbol),
        TransitionTargetNode::Value(expression) => {
            let ExpressionNode::Call(call) = program.expression_table.expression(*expression)
            else {
                return None;
            };
            Some(call.target_symbol)
        }
        TransitionTargetNode::Terminal => None,
    }
}
