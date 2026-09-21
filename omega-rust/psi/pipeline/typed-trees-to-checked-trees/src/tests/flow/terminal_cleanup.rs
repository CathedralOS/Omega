//! Fixtures shared by the terminal cleanup tests: checked programs, scalar
//! discard positions and the machine entry state.

mod attached_returns_and_call_results;
mod machine_edges_and_projections;
mod structural_unit_control;

use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::{
    Lexer, ResolutionRequest, lower_symbol_resolved_trees, parse_syntax_trees, resolve,
};

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed, &CheckingRequest::settled()).expect("check")
}

fn typed_program(source: &str) -> typed_trees::TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

fn scalar_discard_positions(
    plan: &checked_trees::CheckedStructuralScalarReturnMachinePlan,
) -> Vec<u32> {
    plan.cleanup_actions
        .iter()
        .filter_map(|action| match action {
            checked_trees::CheckedStructuralScalarReturnCleanupAction::DiscardRoot(position) => {
                Some(*position)
            }
            checked_trees::CheckedStructuralScalarReturnCleanupAction::InvokeNominal(_) => None,
        })
        .collect()
}

fn machine_and_entry_state(
    checked: &checked_trees::CheckedTrees,
    machine_name: &str,
) -> (symbols::SymbolHandle, symbols::SymbolHandle) {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with(machine_name))
        .unwrap_or_else(|| panic!("missing machine `{machine_name}`"));
    let state = checked
        .machine_states(machine)
        .first()
        .unwrap_or_else(|| panic!("machine `{machine_name}` has no entry state"));
    (machine.symbol, state.symbol)
}
