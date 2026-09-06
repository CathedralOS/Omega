use super::*;

mod identities;
use arena::HandleSpan;
use checked_trees::CheckedTrees;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;

#[derive(Clone, Copy, Debug)]
enum CallForm {
    Statement,
    ValueOperand,
}

const CALL_FORMS: [CallForm; 2] = [CallForm::Statement, CallForm::ValueOperand];

fn typed_fixture(form: CallForm) -> TypedTrees {
    let body = match form {
        CallForm::Statement => "carrier.context.increment_counter(); 0",
        CallForm::ValueOperand => {
            "let observed: u64 = carrier.context.increment_counter(); observed"
        }
    };
    // Put the foreign declaration first and give both owners the same shape,
    // signature, and body: only nominal identity distinguishes their methods.
    let source = format!(
        "data Context {{ counter: u64; }}
         data Decoy {{ counter: u64; }}
         data Carrier {{ context: &mut Context; }}
         data ForeignCarrier {{ context: &mut Decoy; }}
         machine Decoy::increment_counter(&mut self) -> u64 {{
             self.counter = 1; 0
         }}
         machine Context::increment_counter(&mut self) -> u64 {{
             self.counter = 1; 0
         }}
         machine unrelated(mut carrier: Carrier) -> u64 {{ 0 }}
         machine inspect(mut carrier: Carrier) -> u64 {{ {body} }}"
    );
    typed_source(&source)
}

fn typed_source(source: &str) -> TypedTrees {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize projected call");
    let syntax = parse_syntax_trees(&tokens).expect("parse projected call");
    let resolved = lower_syntax_trees(&syntax).expect("resolve projected call");
    lower_symbol_resolved_trees(&resolved).expect("type projected call")
}

fn field_symbol(program: &TypedTrees, owner: &str, name: &str) -> SymbolHandle {
    let definition = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == owner)
        .expect("field owner");
    program
        .data_members(definition)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field) if field.name.as_str() == name => {
                Some(field.symbol)
            }
            _ => None,
        })
        .expect("declared field")
}

fn method_symbol(program: &TypedTrees, owner: &str) -> SymbolHandle {
    let owner = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == owner)
        .expect("nominal owner")
        .symbol;
    let targets = program
        .machines()
        .iter()
        .filter(|machine| machine.attached_data_symbol == owner)
        .flat_map(|machine| program.machine_states(machine))
        .filter(|state| state.name.as_str() == "increment_counter")
        .map(|state| state.symbol)
        .collect::<Vec<_>>();
    let [target] = targets.as_slice() else {
        panic!("one exact method under the nominal owner: {targets:?}")
    };
    assert!(target.is_valid());
    *target
}

fn statement_call_mut(program: &mut TypedTrees) -> &mut typed_trees::statement::TableCall {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .expect("caller machine");
    let state = &program.machine_states(machine)[0];
    let root = program.state_parameters(state)[0].symbol;
    let statements = state.statement_nodes;
    let StatementNode::Call(call) = &mut program.statement_table.statements_mut(statements)[0]
    else {
        panic!("authored projected statement call")
    };
    assert_eq!(call.receiver_root_symbol, root, "exact owned input root");
    call
}

fn assert_exact_selection(program: &CheckedTrees) {
    let expected = method_symbol(program, "Context");
    assert_ne!(expected, method_symbol(program, "Decoy"));
    let calls = program
        .authored_declaration_selections()
        .iter()
        .filter(|selection| selection.kind() == AuthoredDeclarationSelectionKind::Call)
        .collect::<Vec<_>>();
    let [selection] = calls.as_slice() else {
        panic!("one authored method occurrence, including normalized copies: {calls:?}")
    };
    assert!(matches!(
        selection.target(),
        AuthoredDeclarationSelectionTarget::Resolved(target)
            if target.selected_symbol() == expected
    ));
    let flow_calls = program
        .facts
        .flow
        .control
        .calls
        .iter()
        .filter(|(_, call)| call.authored_source_span == Some(selection.source_span()))
        .collect::<Vec<_>>();
    assert!(
        !flow_calls.is_empty(),
        "checked flow retains this authored call"
    );
    for (_, call) in flow_calls {
        assert_eq!(call.target_symbol, expected);
        assert!(call.authored_source_custody_valid);
    }
    assert!(program.authored_declaration_selections().all_finalized());
}

fn replace_typed_callees(program: &mut TypedTrees, target: SymbolHandle, erase_receiver: bool) {
    let mut changed = 0;
    let expressions = program
        .expression_table
        .iter_expressions()
        .filter_map(|(expression, node)| {
            matches!(node, ExpressionNode::Call(call) if call.target.as_str() == "increment_counter")
                .then_some(expression)
        })
        .collect::<Vec<_>>();
    for expression in expressions {
        let ExpressionNode::Call(call) = program.expression_table.expression_mut(expression) else {
            unreachable!()
        };
        call.target_symbol = target;
        if erase_receiver {
            call.receiver = ExpressionHandle::invalid();
        }
        changed += 1;
    }
    let statements = program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine))
        .map(|state| state.statement_nodes)
        .collect::<Vec<_>>();
    for statements in statements {
        for statement in program.statement_table.statements_mut(statements) {
            let StatementNode::Call(call) = statement else {
                continue;
            };
            if call.target.as_str() != "increment_counter" {
                continue;
            }
            call.target_symbol = target;
            if erase_receiver {
                call.receiver = HandleSpan::empty();
                call.receiver_root_symbol = SymbolHandle::invalid();
                call.receiver_symbol = SymbolHandle::invalid();
            }
            changed += 1;
        }
    }
    assert!(changed > 0, "tampering must reach retained call nodes");
}

#[test]
fn projected_reference_calls_select_the_exact_nominal_method() {
    for form in CALL_FORMS {
        let mut checked = lower_typed_trees(typed_fixture(form)).expect("projected call checks");
        assert_exact_selection(&checked);
        if matches!(form, CallForm::Statement) {
            let endpoint = statement_call_mut(&mut checked.typed).receiver_symbol;
            assert!(
                endpoint.is_valid(),
                "checked projected endpoint is explicit"
            );
            assert_eq!(
                checked.symbols.display_path(endpoint, "::"),
                "Carrier::context"
            );
        }
    }
}

#[test]
fn erased_projected_callees_can_bind_from_the_exact_receiver() {
    for form in CALL_FORMS {
        let mut typed = typed_fixture(form);
        replace_typed_callees(&mut typed, SymbolHandle::invalid(), false);
        let checked = lower_typed_trees(typed).expect("exact receiver permits late binding");
        assert_exact_selection(&checked);
    }
}

#[test]
fn nested_statement_receiver_paths_retain_every_semantic_field() {
    let mut program = typed_source(
        "data Context { counter: u64; }
         data Inner { context: &mut Context; }
         data Carrier { inner: Inner; }
         machine Context::increment_counter(&mut self) -> u64 { self.counter = 1; 0 }
         machine inspect(mut carrier: Carrier) -> u64 {
             carrier.inner.context.increment_counter(); 0
         }",
    );
    crate::lookup::resolve_projected_receiver_calls(&mut program)
        .expect("nested regular fields resolve before checking");
    let inner = field_symbol(&program, "Carrier", "inner");
    let context = field_symbol(&program, "Inner", "context");
    let expected_target = method_symbol(&program, "Context");
    let call = statement_call_mut(&mut program).clone();
    assert_eq!(call.receiver_symbol, context);
    assert_eq!(call.target_symbol, expected_target);
    let expected = [call.receiver_root_symbol, inner, context];
    assert!(expected.iter().all(|symbol| symbol.is_valid()));
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let path = crate::lookup::statement_call_receiver_path(&program, state, 0, &call)
        .expect("nested authored statement receiver path");
    assert_eq!(
        path.members()
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["carrier", "inner", "context"]
    );
    // This path also feeds semantic_places::receiver_place_for_call; dropping
    // the middle symbol aliases distinct nested receiver subobjects.
    assert_eq!(path.member_symbols(), expected.as_slice());
}
