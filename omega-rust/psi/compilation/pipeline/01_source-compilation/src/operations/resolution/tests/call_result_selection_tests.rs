/*
    Compare actual selected declarations, not just expression counts. Preparation
    must retain lexical and final routing while keeping implementation bodies out
    of its product. Full resolution is the independent reference operation.
*/

use crate::resolution::frontend_fixture::Lexer;
use crate::resolution::pre_resolution::resolve_call_result_selection;
use crate::resolution::tables::body_expressions;
use crate::resolution::{ResolutionRequest, resolve};
use crate::source_resolved::{
    SymbolResolvedTrees, expression::ExpressionNode, machine::MachineBodyStatus,
};

fn call_identities(program: &SymbolResolvedTrees) -> Vec<String> {
    body_expressions(program)
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
    let tokens = Lexer::new(source_text).tokenize().unwrap();
    let mut sources = crate::source_inputs::SourceMap::default();
    let source_id = sources
        .add("call-results/main.omg".into(), source_text.to_owned())
        .source_id;
    let sources = std::sync::Arc::new(sources);
    let syntax =
        crate::resolution::frontend_fixture::parse_syntax_trees_with_id(source_id, &tokens)
            .unwrap();
    let request = |syntax| ResolutionRequest {
        syntax,
        sources: Some(sources.clone()),
        top_level_bindings: Vec::new(),
        retained_base: None,
    };
    let full = resolve(request(syntax.clone())).unwrap();
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
        body_expressions(&selected).iter_expressions().count()
            < full.tables.bodies.expressions.iter_expressions().count()
    );
}

#[test]
fn call_results_retain_match_capture_around_selected_payload_calls() {
    compare(
        r#"
        data Cell { value: i32; }
        data Envelope { case Active(cell: &mut Cell); case Empty; }
        machine store(cell: &mut Cell) -> bool { cell.value == 41 }
        machine caller(envelope: Envelope) -> bool {
            match envelope {
                Envelope::Active { cell } -> store(&mut cell)
                Envelope::Empty -> false
            }
        }
    "#,
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

#[test]
fn structural_equation_calls_keep_their_callee_occurrence() {
    let text = r#"
        data Buffer<Element, const N: u64> { items: [Element; N]; }
        machine inspect<Kind, Element, const N: u64>(value: &Buffer<Element, N>) -> u64
        requires Kind == Buffer<Element, N> { 1u64 }
        machine caller(value: &Buffer<u8, 2>) -> u64 {
            inspect<Buffer<u8, 2>, u8, 2>(value);
            inspect<Buffer<u8, 2>, u8, 2>(value)
        }
    "#;
    compare(text);
}

#[test]
fn a_structural_equation_machine_still_cannot_escape_as_a_plain_name() {
    let text = r#"
        data Buffer<Element, const N: u64> { items: [Element; N]; }
        machine inspect<Kind, Element, const N: u64>(value: &Buffer<Element, N>) -> u64
        requires Kind == Buffer<Element, N> { 1u64 }
        machine caller() { let selected = inspect; }
    "#;
    let tokens = Lexer::new(text).tokenize().unwrap();
    let mut sources = crate::source_inputs::SourceMap::default();
    let source_id = sources
        .add("equation-selection/main.omg".into(), text.to_owned())
        .source_id;
    let syntax = crate::frontend_fixture::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
    let result = resolve(ResolutionRequest {
        syntax,
        sources: Some(std::sync::Arc::new(sources)),
        top_level_bindings: Vec::new(),
        retained_base: None,
    });
    let Err(errors) = result else {
        panic!("the unresolved structural selection must refuse");
    };
    assert!(
        errors.iter().any(|error| error.message == "machine with structural equations requires a closed explicit call; noncall selection retains an unsolved equation"),
        "{errors:?}"
    );
}

#[test]
fn conjunctions_keep_only_structural_definitions() {
    for (requirement, expected) in [
        ("Kind == Buffer<Element, N> && Element == u8", 1),
        ("Kind == Buffer<Element, N> || Element == u8", 0),
        ("!(Kind == Buffer<Element, N>)", 0),
    ] {
        let text = format!(
            "data Buffer<Element, const N: u64> {{ items: [Element; N]; }}\n\
             machine inspect<Kind, Element, const N: u64>(value: &Buffer<Element, N>) -> u64\n\
             requires {requirement} {{ 1u64 }}"
        );
        let tokens = Lexer::new(&text).tokenize().unwrap();
        let mut sources = crate::source_inputs::SourceMap::default();
        let source_id = sources
            .add("equation-definitions/main.omg".into(), text)
            .source_id;
        let syntax =
            crate::frontend_fixture::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
        let resolved = resolve(ResolutionRequest {
            syntax,
            sources: Some(std::sync::Arc::new(sources)),
            top_level_bindings: Vec::new(),
            retained_base: None,
        })
        .unwrap();
        let machine = resolved
            .machines
            .iter()
            .find(|machine| !machine.type_parameters.is_empty())
            .unwrap();
        let equations = resolved
            .declarations()
            .declared_type_equations
            .span_or_empty(machine.declared_type_equations);
        assert_eq!(equations.len(), expected, "{requirement}");
        if expected != 0 {
            assert!(matches!(&equations[0].parameter,
                crate::source_resolved::types::TypeReference::Named { name, .. }
                    if name.as_str() == "Kind"));
        }
    }
}
