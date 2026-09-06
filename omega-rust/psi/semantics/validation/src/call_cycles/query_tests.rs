use super::{runtime_ranking, validate_machine_call_cycles};
use crate::symbols::TopLevelSymbols;
use crate::validated_runtime_recursive_components;
use diagnostics::Diagnostic;
use source::SourceMap;
use source_files_to_tokens::Lexer;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionNode};
use typed_trees::statement::{StatementNode, TransitionTargetNode};

fn typed_source(source: &str) -> TypedTrees {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize query fixture");
    let mut sources = SourceMap::default();
    let source_id = sources
        .add("runtime_component_query.omg".into(), source.to_owned())
        .source_id;
    let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens)
        .expect("parse query fixture");
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources(
        &syntax,
        std::sync::Arc::new(sources),
    )
    .expect("resolve query fixture");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type query fixture")
}

fn typed_pair(body: &str, extra: &str) -> TypedTrees {
    typed_source(&format!(
        "machine unrelated() -> u64 {{ transition {{ _ -> 0 }} }}
         machine scan_a(remaining: u64) -> u64
         terminates by remaining;
         {{ transition remaining == 0 {{ true -> 0 false -> scan_b(remaining) }} }}
         machine scan_b(remaining: u64) -> u64
         terminates by remaining;
         {{ {body} }}
         {extra}"
    ))
}

const DECREASE: &str = "transition remaining == 0 { true -> 0 false -> scan_a(remaining - 1) }";

fn machine_symbol(program: &TypedTrees, name: &str) -> SymbolHandle {
    program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .expect("fixture machine")
        .symbol
}

fn assert_admitted_pair(program: &TypedTrees) {
    assert_eq!(
        validated_runtime_recursive_components(program),
        vec![vec![
            machine_symbol(program, "scan_a"),
            machine_symbol(program, "scan_b"),
        ]]
    );
    let mut diagnostics = Vec::new();
    let symbols = TopLevelSymbols::build(program, &mut diagnostics);
    validate_machine_call_cycles(program, &symbols, &mut diagnostics);
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
}

fn assert_cycle_rejected(program: &TypedTrees) {
    assert!(validated_runtime_recursive_components(program).is_empty());
    let mut diagnostics = Vec::new();
    let symbols = TopLevelSymbols::build(program, &mut diagnostics);
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    validate_machine_call_cycles(program, &symbols, &mut diagnostics);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("machine call cycle")),
        "{diagnostics:#?}"
    );
}

#[test]
fn query_returns_exact_runtime_members_without_acyclic_neighbors() {
    assert_admitted_pair(&typed_pair(DECREASE, ""));
}

#[test]
fn removing_strict_descent_rejects_the_preserving_cycle() {
    let mut program = typed_pair(DECREASE, "");
    assert_admitted_pair(&program);
    let decreases = program
        .expression_table
        .iter_expressions()
        .filter_map(|(handle, expression)| match expression {
            ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::Subtract => {
                Some((handle, binary.left))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(!decreases.is_empty());
    for (handle, left) in decreases {
        let preserved = program.expression_table.expression(left).clone();
        *program.expression_table.expression_mut(handle) = preserved;
    }
    assert_cycle_rejected(&program);
}

#[test]
fn strict_parallel_call_does_not_hide_a_preserving_cycle() {
    let program = typed_pair(
        "transition remaining > 0 {
            true -> scan_a(remaining - 1)
            false -> scan_a(remaining)
        }",
        "",
    );
    assert_cycle_rejected(&program);
}

#[test]
fn missing_target_identity_cannot_borrow_a_parallel_call() {
    let mut program = typed_pair(
        "transition remaining > 1 { true -> scan_a(remaining - 1) }
         transition remaining > 0 { true -> scan_a(remaining - 1) false -> 0 }",
        "",
    );
    assert_admitted_pair(&program);
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "scan_b")
        .unwrap();
    let statements = program.machine_states(machine)[0].statement_nodes;
    let (statement, mut target) = program
        .statement_table
        .iter_statements(statements)
        .find_map(|(handle, statement)| {
            let StatementNode::Transition(transition) = statement else {
                return None;
            };
            let target = program.statement_table.transition_target(transition.target);
            matches!(target, TransitionTargetNode::Named { .. }).then(|| (handle, target.clone()))
        })
        .expect("parallel recursive call");
    let TransitionTargetNode::Named { path, .. } = &mut target else {
        unreachable!()
    };
    path.symbol = SymbolHandle::invalid();
    path.head_symbol = SymbolHandle::invalid();
    let target = program.statement_table.insert_transition_target(target);
    let StatementNode::Transition(transition) = program.statement_table.statement_mut(statement)
    else {
        unreachable!()
    };
    transition.target = target;
    assert_cycle_rejected(&program);
}

#[test]
fn missing_member_ranking_rejects_the_component() {
    let mut program = typed_pair(DECREASE, "");
    assert_admitted_pair(&program);
    let symbol = machine_symbol(&program, "scan_b");
    program
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.symbol == symbol)
        .unwrap()
        .termination_plan
        .implementation_witness = None;
    assert_cycle_rejected(&program);
}

#[test]
fn symbol_construction_errors_grant_no_components() {
    let mut program = typed_pair(DECREASE, "data Duplicate {}");
    assert_admitted_pair(&program);
    let duplicate = program.data_definitions()[0].clone();
    program.push_data_definition(duplicate);
    let mut diagnostics = Vec::new();
    TopLevelSymbols::build(&program, &mut diagnostics);
    assert!(diagnostics.iter().any(Diagnostic::is_error));
    assert!(validated_runtime_recursive_components(&program).is_empty());
}

#[test]
fn proof_dependency_prevents_admitting_a_runtime_subset() {
    let program = typed_pair(
        "transition remaining == 0 {
            true -> proof_bridge(ProofTree::End)
            false -> scan_a(remaining - 1)
        }",
        "data ProofTree { case End; case Branch(child: ProofTree); }
         machine proof_bridge(value: ProofTree) -> u64 { transition { _ -> scan_a(0) } }",
    );
    let proof_only = typed_trees::proof_only::classify(&program);
    let bridge = machine_symbol(&program, "proof_bridge");
    assert!(program.machines().iter().any(|machine| {
        machine.symbol == bridge && proof_only.is_proof_machine(&program, machine)
    }));
    // Omitting the proof-to-runtime dependency incorrectly leaves an admitted
    // runtime pair. The public query must use the validator's complete graph.
    let mut incomplete = vec![Vec::new(); program.machines().len()];
    runtime_ranking::extend_runtime_adjacency(&program, &proof_only, &mut incomplete);
    assert_eq!(
        runtime_ranking::admitted_components(&program, &proof_only, &incomplete).len(),
        1
    );
    assert_cycle_rejected(&program);
}
