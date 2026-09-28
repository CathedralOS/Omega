use super::{AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionTarget};
use crate::tests::front_end::typed_program;

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
        CallForm::Statement => "_ = carrier.context.increment_counter(); 0",
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
    typed_program(&source)
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
