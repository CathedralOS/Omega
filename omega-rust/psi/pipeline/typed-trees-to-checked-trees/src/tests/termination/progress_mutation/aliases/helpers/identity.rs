use super::*;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;

fn typed_fixture(extra: &str) -> typed_trees::TypedTrees {
    let source = fixture_source(
        "let borrowed: &Context = forward(context); transition { _ -> 0 }",
        true,
        false,
        extra,
    );
    typed_source(&source)
}

fn typed_source(source: &str) -> typed_trees::TypedTrees {
    let tokens = Lexer::new(source).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    lower_symbol_resolved_trees(&resolved).unwrap()
}

#[test]
fn a_slice_view_spelling_does_not_replace_a_declared_helper_body() {
    for loaded in [false, true] {
        let field_type = if loaded { "&Context" } else { "Context" };
        let result = if loaded {
            "self.selected"
        } else {
            "&self.selected"
        };
        let mut program = typed_source(&format!(
            "{CONTEXT_FIXTURE}
             data Holder {{ selected: {field_type}; }}
             machine Holder::as_mut_slice(&self) -> &Context {{ {result} }}
             machine forward(holder: &Holder) -> &Context {{ holder.as_mut_slice() }}
             machine replace(holder: &Holder) -> u64 {{
                 let borrowed: &Context = forward(holder);
                 transition {{ _ -> 0 }}
             }}"
        ));
        if loaded {
            assert_eq!(origin(&program), None);
        } else {
            let (_, segments) = origin(&program).expect("body-proven selected field");
            let [facts::PlaceSegment::Field { symbol }] = segments.as_slice() else {
                panic!("body selection must survive the helper name: {segments:?}")
            };
            assert_eq!(
                program.symbols.display_path(*symbol, "::"),
                "Holder::selected"
            );
        }
        let calls: Vec<_> = program.expression_table.iter_expressions().filter_map(|(handle, expression)| {
            matches!(expression, ExpressionNode::Call(call) if call.target.as_str() == "as_mut_slice").then_some(handle)
        }).collect();
        assert!(!calls.is_empty());
        for expression in calls {
            let ExpressionNode::Call(call) = program.expression_table.expression_mut(expression)
            else {
                unreachable!()
            };
            call.target_symbol = SymbolHandle::invalid();
        }
        assert_eq!(
            origin(&program),
            None,
            "an unresolved name cannot supply exact identity"
        );
    }
}

fn result_expression(program: &typed_trees::TypedTrees, name: &str) -> ExpressionHandle {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let StatementNode::Expression(expression) = program
        .statement_table
        .statements(state.statement_nodes)
        .last()
        .unwrap()
    else {
        panic!("helper terminal result")
    };
    *expression
}

fn origin(program: &typed_trees::TypedTrees) -> Option<(SymbolHandle, Vec<facts::PlaceSegment>)> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "replace")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let statements = program.statement_table.statements(state.statement_nodes);
    let StatementNode::LocalData(local) = &statements[0] else {
        panic!("reference local")
    };
    validation::CallFrameResolver::new(program)
        .unwrap()
        .local_reference_origin_before_statement(machine, statements.last().unwrap(), local.symbol)
}

fn assert_context(program: &typed_trees::TypedTrees) {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "replace")
        .unwrap();
    let context = program.state_parameters(&program.machine_states(machine)[0])[0].symbol;
    assert_eq!(origin(program), Some((context, vec![])));
}

#[test]
fn an_unresolved_helper_target_cannot_recover_identity_from_its_name() {
    for nested in [false, true] {
        let mut program = typed_fixture(
            "machine inner(context: &Context) -> &Context { context }
             machine forward(context: &Context) -> &Context { inner(context) }",
        );
        assert_context(&program);
        let target = if nested { "inner" } else { "forward" };
        let calls: Vec<_> = program
            .expression_table
            .iter_expressions()
            .filter_map(|(handle, expression)| {
                matches!(expression, ExpressionNode::Call(call) if call.target.as_str() == target)
                    .then_some(handle)
            })
            .collect();
        assert!(!calls.is_empty(), "retained call to {target}");
        for expression in calls {
            let ExpressionNode::Call(call) = program.expression_table.expression_mut(expression)
            else {
                unreachable!()
            };
            call.target_symbol = SymbolHandle::invalid();
        }
        assert_eq!(origin(&program), None);
    }
}

#[test]
fn an_erased_helper_alias_identity_cannot_flatten_to_a_valid_parameter() {
    let mut program = typed_fixture(
        "machine forward(context: &Context) -> &Context {
             let saved: &Context = context;
             saved
         }",
    );
    assert_context(&program);
    let expression = result_expression(&program, "forward");
    let ExpressionNode::Name(name) = program.expression_table.expression_mut(expression) else {
        panic!("name")
    };
    name.head_symbol = SymbolHandle::invalid();
    name.symbol = SymbolHandle::invalid();
    assert_eq!(origin(&program), None);
}

#[test]
fn a_foreign_helper_parameter_cannot_supply_a_same_spelling_origin() {
    let mut program = typed_fixture(
        "machine forward(context: &Context) -> &Context { context }
         machine unrelated(context: &Context) -> &Context { context }",
    );
    assert_context(&program);
    let foreign = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "unrelated")
        .unwrap();
    let foreign = program.state_parameters(&program.machine_states(foreign)[0])[0].symbol;
    let expression = result_expression(&program, "forward");
    let ExpressionNode::Name(name) = program.expression_table.expression_mut(expression) else {
        panic!("name")
    };
    name.head_symbol = foreign;
    name.symbol = foreign;
    assert_eq!(origin(&program), None);
}

#[test]
fn a_readonly_spelling_cannot_hide_a_mutable_binding_replacement() {
    let mut program = typed_source(&format!(
        "{CONTEXT_FIXTURE}
         machine replace(context: &mut Context, replacement: &Context) -> u64 {{
             let mut read_only: &Context = replacement;
             let writable: &mut Context = context;
             read_only = replacement;
             writable.counter = 1;
             transition {{ _ -> 0 }}
         }}"
    ));
    let frame = |program: &typed_trees::TypedTrees| {
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "replace")
            .unwrap();
        let state = &program.machine_states(machine)[0];
        validation::CallFrameResolver::new(program)
            .unwrap()
            .inferred_state_write_frame(machine, state)
    };
    assert_eq!(
        frame(&program),
        facts::NormalizedWriteFrame::complete(vec!["$P0.counter".to_owned()])
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "replace")
        .unwrap();
    let statements = program
        .statement_table
        .statements(program.machine_states(machine)[0].statement_nodes);
    let StatementNode::LocalData(writable) = &statements[1] else {
        panic!("mutable local")
    };
    let writable = writable.symbol;
    let StatementNode::Assignment(assignment) = &statements[2] else {
        panic!("readonly replacement")
    };
    let target = assignment.target;
    let ExpressionNode::Name(name) = program.expression_table.expression_mut(target) else {
        panic!("binding target")
    };
    name.head_symbol = writable;
    name.symbol = writable;
    assert_eq!(frame(&program), facts::NormalizedWriteFrame::opaque());
}
