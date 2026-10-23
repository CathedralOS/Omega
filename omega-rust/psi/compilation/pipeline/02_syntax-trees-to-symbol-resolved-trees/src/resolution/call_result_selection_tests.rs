//! Compare actual selected declarations, not just expression counts. Preparation
//! must retain lexical and final routing while keeping implementation bodies out
//! of its product. Full resolution is the independent reference operation.

use super::{ResolutionRequest, resolve_call_result_selection, resolve_numeric_probe};
use symbol_resolved_trees::{
    SymbolResolvedTrees, expression::ExpressionNode, machine::MachineBodyStatus,
};

fn call_identities(program: &SymbolResolvedTrees) -> Vec<String> {
    program
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .filter_map(|(_, expression)| {
            let ExpressionNode::Call(call) = expression else {
                return None;
            };
            assert!(
                call.target_symbol.is_valid(),
                "unselected call: {:?}",
                call.target
            );
            let target = program.symbols.get(call.target_symbol);
            let declaration = program.symbols.symbol_source_span(call.target_symbol);
            let parent = if target.parent.is_valid() {
                program.symbols.symbol_source_span(target.parent)
            } else {
                None
            };
            assert!(
                declaration.is_some() || parent.is_some(),
                "selected declaration needs source identity"
            );
            Some(format!(
                "{:?} {:?} {:?} {:?}",
                call.target.source_span(),
                target.kind,
                declaration,
                parent
            ))
        })
        .collect()
}

fn compare(source_text: &str) -> (SymbolResolvedTrees, SymbolResolvedTrees) {
    let tokens = source_files_to_tokens::Lexer::new(source_text)
        .tokenize()
        .unwrap();
    let mut sources = source::SourceMap::default();
    let source_id = sources
        .add("call-results/main.omg".into(), source_text.to_owned())
        .source_id;
    let sources = std::sync::Arc::new(sources);
    let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
    let request = |syntax| ResolutionRequest {
        syntax,
        sources: Some(sources.clone()),
        top_level_bindings: Vec::new(),
    };
    let full = resolve_numeric_probe(request(syntax.clone())).unwrap();
    let selected = resolve_call_result_selection(request(syntax)).unwrap();
    let expected = call_identities(&full);
    assert!(!expected.is_empty());
    assert_eq!(call_identities(&selected), expected);
    assert!(
        selected
            .machines
            .iter()
            .filter(|machine| machine.body_is_present)
            .all(|machine| machine.body_status == MachineBodyStatus::Deferred)
    );
    (full, selected)
}

#[test]
fn call_results_keep_nested_calls_and_preceding_locals_without_runtime_body_rows() {
    let (full, selected) = compare(
        r#"
        machine identity<T>(value: T) -> T { value }
        machine increment(value: u64) -> u64 { value + 1 }
        machine caller(value: u64) -> u64 {
            let first: u64 = value + 2;
            let second: u64 = first + 3;
            identity<u64>(increment(second))
        }
    "#,
    );
    assert!(
        selected
            .tables
            .bodies
            .expressions
            .iter_expressions()
            .count()
            < full.tables.bodies.expressions.iter_expressions().count()
    );
}

#[test]
fn call_results_keep_each_target_body_selected_declaration() {
    compare(
        r#"
        linux_x86_64 machine helper(value: u64) -> u64 { value + 1 }
        macos_arm64 machine helper(value: u64) -> u64 { value + 2 }
        linux_x86_64 machine caller(value: u64) -> u64 { helper(value) }
        macos_arm64 machine caller(value: u64) -> u64 { helper(value) }
    "#,
    );
}

#[test]
fn call_results_keep_owned_and_inherited_conformance_routing() {
    compare(
        r#"
        trait Reading {
            machine read(value: u64) -> u64;
            machine forward(value: u64) -> u64 { Reading::read(value) }
        }
        First satisfies Reading {
            machine read(value: u64) -> u64 { value }
            machine forward(value: u64) -> u64 { Reading::read(value) }
        }
        Second satisfies Reading {
            machine read(value: u64) -> u64 { value + 1 }
        }
    "#,
    );
}
