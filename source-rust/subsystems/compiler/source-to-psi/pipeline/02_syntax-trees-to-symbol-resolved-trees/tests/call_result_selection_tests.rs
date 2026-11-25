//! Compare selected call declarations when bodies are completed together or individually.

use crate::resolution::frontend_fixture::Lexer;
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
    let full = resolve(super::source_request(source_text)).unwrap();
    let mut frontier = crate::resolution::resolve_deferred(super::source_request(source_text)).unwrap();
    let selected = frontier.shared_trees();
    loop {
        let next = frontier.pending_machines().next();
        let Some(machine) = next else { break };
        frontier = frontier.complete_machine_bodies(&[machine]).unwrap().into_resolved_frontier();
    }
    let mut expected = call_identities(&full);
    let mut completed = call_identities(frontier.trees());
    expected.sort();
    completed.sort();
    assert!(!expected.is_empty());
    assert_eq!(completed, expected);
    assert!(
        selected
            .machines
            .iter()
            .filter(|machine| machine.body_is_present)
            .all(|machine| machine.body_status == MachineBodyStatus::Deferred)
    );
    (full, std::sync::Arc::unwrap_or_clone(selected))
}

#[test]
fn a_qualified_contract_call_keeps_its_entry_when_bodies_are_completed() {
    compare(
        r#"
        machine Flags::identity(flag: bool) -> bool { flag }
        machine check(flag: bool) -> bool
        ensures result == Flags::identity(flag)
        { flag }
        "#,
    );
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
            identity::<u64>(increment(second))
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
        data Envelope { case Active(cell: @mutable Cell); case Empty; }
        machine store(cell: @mutable Cell) -> bool { cell.value == 41 }
        machine caller(envelope: Envelope) -> bool {
            match envelope {
                Envelope::Active { cell } -> store(@mutable cell),
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
        machine inspect<Kind, Element, const N: u64>(value: @Buffer<Element, N>) -> u64
        requires Kind == Buffer<Element, N> { 1u64 }
        machine caller(value: @Buffer<u8, 2>) -> u64 {
            inspect::<Buffer<u8, 2>, u8, 2>(value);
            inspect::<Buffer<u8, 2>, u8, 2>(value)
        }
    "#;
    compare(text);
}

#[test]
fn a_structural_equation_machine_still_cannot_escape_as_a_plain_name() {
    let text = r#"
        data Buffer<Element, const N: u64> { items: [Element; N]; }
        machine inspect<Kind, Element, const N: u64>(value: @Buffer<Element, N>) -> u64
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
            "const N: u64 = 9u64;\n\
             data Buffer<Element, const N: u64> {{ items: [Element; N]; }}\n\
             machine inspect<Kind, Element, const N: u64>(value: @Buffer<Element, N>) -> u64\n\
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
        let equations = resolved.machine_declared_type_equations(machine);
        assert_eq!(equations.len(), expected, "{requirement}");
        if expected != 0 {
            let parameters = resolved.machine_type_parameters(machine);
            assert!(matches!(&equations[0].parameter,
                crate::source_resolved::types::TypeReference::Named { name, symbol, .. }
                    if name.as_str() == "Kind" && *symbol == parameters[0].symbol));
            let crate::source_resolved::types::TypeReference::Generic(definition) =
                &equations[0].definition
            else {
                panic!("the definition keeps its constructor")
            };
            let arguments = resolved.child_type_references(definition.arguments);
            assert_eq!(arguments.len(), 2);
            for (argument, parameter) in arguments.iter().zip(&parameters[1..]) {
                assert!(
                    matches!(argument,
                    crate::source_resolved::types::TypeReference::Named { symbol, .. }
                        if *symbol == parameter.symbol),
                    "each argument must name its actual declared binder: {argument:?}"
                );
            }
        }
    }
}

#[test]
fn build_bind_uses_the_selected_operation_record_before_root_operand_grammar() {
    use crate::source_resolved::statement::StatementNode;

    for (parameters, arguments) in [
        ("a: u64, b: u64", "1, 2"),
        ("a: u64, b: u64, c: u64", "1, 2, 3"),
    ] {
        for (owner, binder) in [("Local", "<const Build: Writer>"), ("Build", "")] {
            let source = format!(
                "trait Writer {{ machine bind({parameters}); }}
                machine write({parameters}) {{}}
                {owner} satisfies Writer {{ Writer::bind = write; }}
                machine apply{binder}() {{ Build::bind({arguments}); }}"
            );
            let program = resolve(super::source_request(&source))
                .unwrap_or_else(|errors| panic!("{source}: {errors:?}"));
            let machine = program
                .machines
                .iter()
                .find(|machine| machine.name.as_str() == "apply")
                .unwrap();
            let state = program.machine_state(program.machine_state_handles(machine.states)[0]);
            let [StatementNode::Call(call)] = program.state_body(state) else {
                panic!("expected an ordinary call: {:?}", program.state_body(state));
            };
            assert!(call.target_symbol.is_valid(), "{source}: {call:?}");
        }
    }

    let source = "data Build {} machine apply<const Build: u64>(builder: @mutable Build) {
        Build::bind(builder, Slot, main);
    }";
    let program = resolve(super::source_request(source)).unwrap();
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "apply")
        .unwrap();
    let state = program.machine_state(program.machine_state_handles(machine.states)[0]);
    assert!(matches!(
        program.state_body(state),
        [StatementNode::RootBinding(_)]
    ));
}

/*
    A selection row names its realization by spelling, and normalization
    realizes only the machines that spelling reaches. The row must still pick
    the one named machine among same-shaped declarations, and a spelling that
    reaches no machine must be refused rather than filled by another one.
*/
#[test]
fn a_selection_row_realizes_only_the_machine_its_spelling_selects() {
    let decoys = (0..8)
        .map(|index| format!("machine decoy{index}(a: u64) {{}}\n"))
        .collect::<String>();
    let source = format!(
        "trait Writer {{ machine bind(a: u64); }}
        {decoys}machine write(a: u64) {{}}
        Build satisfies Writer {{ Writer::bind = write; }}"
    );
    let program = resolve(super::source_request(&source))
        .unwrap_or_else(|errors| panic!("{source}: {errors:?}"));
    let write = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "write")
        .unwrap()
        .symbol;
    let rows = program
        .conformances
        .iter()
        .flat_map(|conformance| conformance.rows.iter())
        .collect::<Vec<_>>();
    let [row] = rows.as_slice() else {
        panic!("one selection row: {rows:?}")
    };
    assert_eq!(row.realization_machine, write);

    let missing = source.replace("Writer::bind = write;", "Writer::bind = absent;");
    let errors = resolve(super::source_request(&missing)).unwrap_err();
    let [error] = errors.as_slice() else {
        panic!("{errors:?}")
    };
    assert!(
        error
            .message
            .contains("member `bind` does not identify one exact inherited overload"),
        "{error:?}"
    );
}

#[test]
fn malformed_selected_root_bindings_report_the_authored_bind_or_operand() {
    for (args, expected) in [("builder, slot", "bind"), ("builder, 7, entry", "7")] {
        let source = format!(
            "data Build {{}} machine plan(builder: Build) {{\n    Build::bind({args});\n}}\n"
        );
        let errors = resolve(super::source_request(&source)).unwrap_err();
        let [error] = errors.as_slice() else {
            panic!("{errors:?}")
        };
        assert!(error.message.contains("root-slot binding requires an explicit Build argument, one slot path and one implementation path or description expression"), "{error:?}");
        let span = error.source_span.unwrap().span;
        assert_eq!(&source[span.start..span.end], expected);
    }
}

#[test]
fn a_selected_root_binding_cannot_run_in_a_block_value() {
    let source = "data Build {} machine plan(builder: Build, choice: u64) -> u64 {
        match choice { _ -> { Build::bind(builder, slot, entry); 0 } }
    }";
    let errors = resolve(super::source_request(source)).unwrap_err();
    let [error] = errors.as_slice() else {
        panic!("{errors:?}")
    };
    assert!(
        error
            .message
            .contains("root or proof-output binding statement cannot appear in it")
    );
    let span = error.source_span.unwrap().span;
    assert_eq!(&source[span.start..span.end], "Build");
}
