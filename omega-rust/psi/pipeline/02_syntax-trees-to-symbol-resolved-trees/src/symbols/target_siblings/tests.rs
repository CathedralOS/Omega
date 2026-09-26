//! Calls inside a target sibling's body bind to the same target's siblings,
//! both for a free helper only that target declares and for an attached
//! method reached through `self`.
use crate::symbol_resolved_trees::SymbolResolvedTrees;
use crate::{ResolutionRequest, resolve};
use source_files_to_tokens::Lexer;
use symbols::SymbolHandle;

fn resolve_source(source: &str) -> SymbolResolvedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    resolve(ResolutionRequest::new(&syntax)).expect("siblings resolve")
}

fn machine_symbol(program: &SymbolResolvedTrees, name: &str) -> SymbolHandle {
    program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .unwrap_or_else(|| panic!("machine {name} is lowered"))
        .symbol
}

/// The callee machine of the first `let` initializer call in `machine`'s
/// first state.
fn first_local_call_callee(program: &SymbolResolvedTrees, machine: &str) -> SymbolHandle {
    let record = program
        .machines
        .iter()
        .find(|candidate| candidate.name.as_str() == machine)
        .unwrap_or_else(|| panic!("machine {machine} is lowered"));
    let state = program.machine_state(program.machine_state_handles(record.states)[0]);
    let statements = state.statement_nodes;
    for offset in 0..statements.count() {
        let handle = arena::Handle::from_parts(
            statements.start().arena_index() + offset,
            statements.start().generation(),
        );
        let crate::symbol_resolved_trees::statement::StatementNode::LocalData(local) =
            program.tables.bodies.statements.statement(handle)
        else {
            continue;
        };
        let crate::symbol_resolved_trees::expression::ExpressionNode::Call(call) = program
            .tables
            .bodies
            .expressions
            .expression(local.initial_value)
        else {
            continue;
        };
        assert!(
            call.target_symbol.is_valid(),
            "call `{}` inside {machine} stays unbound",
            call.target
        );
        return program.symbols.get(call.target_symbol).parent;
    }
    panic!("{machine} has no local initializer call");
}

const SOURCE: &str = "data Helper { seed: i32; }
demo_target machine free_helper(clock_id: u32) -> u64 { transition { _ -> 5 } }
demo_target machine Helper::perf(&mut self) -> u64 {
    let v: u64 = free_helper(6);
    transition { _ -> v }
}
demo_target machine Helper::internal(&self) -> i32 { transition { _ -> 63 } }
demo_target machine Helper::entry_point(&self) -> i32 {
    let inner: i32 = self.internal();
    transition { _ -> inner }
}";

#[test]
fn sibling_bodies_lower_with_their_target_and_authored_name() {
    let program = resolve_source(SOURCE);
    let targets = program
        .machines
        .iter()
        .map(|machine| {
            (
                machine.name.as_str().to_owned(),
                machine
                    .target
                    .as_ref()
                    .map(|target| target.as_str().to_owned()),
                program.symbols.name(machine.symbol).to_owned(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        targets,
        [
            (
                "free_helper".to_owned(),
                Some("demo_target".to_owned()),
                "free_helper::demo_target".to_owned()
            ),
            (
                "Helper::perf".to_owned(),
                Some("demo_target".to_owned()),
                "Helper::perf::demo_target".to_owned()
            ),
            (
                "Helper::internal".to_owned(),
                Some("demo_target".to_owned()),
                "Helper::internal::demo_target".to_owned()
            ),
            (
                "Helper::entry_point".to_owned(),
                Some("demo_target".to_owned()),
                "Helper::entry_point::demo_target".to_owned()
            ),
        ]
    );
}

#[test]
fn free_call_inside_a_sibling_binds_to_the_same_targets_sibling() {
    let program = resolve_source(SOURCE);
    assert_eq!(
        first_local_call_callee(&program, "Helper::perf"),
        machine_symbol(&program, "free_helper")
    );
}

#[test]
fn self_method_call_inside_a_sibling_binds_to_the_same_targets_sibling() {
    let program = resolve_source(SOURCE);
    assert_eq!(
        first_local_call_callee(&program, "Helper::entry_point"),
        machine_symbol(&program, "Helper::internal")
    );
}
