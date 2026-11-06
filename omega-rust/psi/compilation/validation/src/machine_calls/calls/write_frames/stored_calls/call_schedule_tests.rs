use super::*;
use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
use typed_trees::{
    TypedTrees,
    expression::{ExpressionNode, TableCallExpression},
    name::Identifier,
    types::TypeReferenceNode,
};

#[test]
fn unavailable_transfer_inventory_preserves_operand_call_order() {
    let mut builder = SymbolTableBuilder::default();
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("test"));
    let owner = builder
        .insert_children(
            root,
            [(SymbolKind::Machine, SymbolNameRef::Static("Caller"))],
        )
        .start();
    let state_symbol = builder
        .insert_children(owner, [(SymbolKind::State, SymbolNameRef::Static("entry"))])
        .start();
    let mut program = TypedTrees {
        symbols: builder.finish(),
        ..Default::default()
    };
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let state = typed_trees::state::State {
        symbol: state_symbol,
        name: Identifier::generated("entry"),
        return_type: unit,
        ..Default::default()
    };
    let mut machine = Machine {
        symbol: owner,
        name: Identifier::generated("Caller"),
        ..Default::default()
    };
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);
    // The schedule records invocation occurrences before asking for any
    // transfer. Reusing this callable gives two distinct nested call handles.
    let mut call = |arguments| {
        let arguments = program
            .expression_table
            .insert_expression_handles(arguments);
        program
            .expression_table
            .insert(ExpressionNode::Call(TableCallExpression {
                authored_call_selection: None,
                authored_call_binding: None,
                authored_machine_application: Default::default(),
                requirement_application: Default::default(),
                exact_coercion_target: Default::default(),
                receiver: ExpressionHandle::invalid(),
                target_symbol: state_symbol,
                static_machine_parameter: Default::default(),
                target: Identifier::generated("entry"),
                static_requirement_dispatch: None,
                machine_arguments: Default::default(),
                quotient_operation: None,
                private_layout_operation: None,
                layout_query: None,
                native_code_layout: None,
                build_selection: Default::default(),
                arguments,
                evidence_arguments: Default::default(),
                operational_acknowledgement: Default::default(),
            }))
    };
    let inner = call(Vec::new());
    let outer = call(vec![inner]);
    let statement = typed_trees::statement::StatementNode::Expression(outer);
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let mut diagnostics = Vec::new();
    let symbols = TopLevelSymbols::build(&program, &mut diagnostics);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let schedule = call_contexts(
        &program,
        machine,
        state,
        &statement,
        None,
        &symbols,
        &mut FrameInference::for_state(state.symbol),
    )
    .expect("evaluation order remains available without transfer evidence");
    assert_eq!(schedule.contexts.len(), 3);
    assert_eq!(schedule.fallthrough, 2);
    assert!(matches!(
        schedule.contexts[0],
        StoredReferenceCallContext::Entry
    ));
    for (position, expected) in [(1, inner), (2, outer)] {
        let StoredReferenceCallContext::Invocation {
            entry,
            expression,
            effects,
        } = &schedule.contexts[position]
        else {
            panic!("expected an invocation")
        };
        assert_eq!(*entry, position - 1);
        assert_eq!(*expression, expected);
        assert!(
            effects.is_none(),
            "missing inventory cannot prove an empty transfer"
        );
    }
}
