//! One hand-built program, as typing leaves it:
//!
//! ```omega
//! machine forward<T>(item: T) -> T { item }
//! machine relay<T>(item: T) -> T ensures item { forward<T>(item) }
//! machine sized<const N: u64>() { }
//! machine main() {
//!     let a = relay<u8>(1);
//!     let b = relay<u8>(2);
//!     let c = relay<bool>(true);
//! }
//! machine counted() { sized<3>(); }
//! ```
//!
//! `main` holds closed type applications, which are served. `counted` holds
//! an application of a value parameter, which is not: it is the failing twin.

use super::prepare;
use arena::HandleSpan;
use symbols::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder};
use typed_trees::TypedTrees;
use typed_trees::data::{TypeParameter, TypeParameterKind};
use typed_trees::deferred_refusals::DeferredRequest;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, StaticMachineArgument, TableCallExpression, TableNamePath,
};
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
use typed_trees::state::State;
use typed_trees::statement::{
    StatementNode, TableCall, TableLocalData, TableTransition, TransitionTargetNode,
};
use typed_trees::static_applications::StaticMachineBindingValue;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

struct Generic {
    machine: SymbolHandle,
    state: SymbolHandle,
    parameter: SymbolHandle,
    item: SymbolHandle,
}

struct Source {
    program: TypedTrees,
    byte: SymbolHandle,
    boolean: SymbolHandle,
    forward: Generic,
    relay: Generic,
    sized_state: SymbolHandle,
    main: SymbolHandle,
    counted: SymbolHandle,
}

fn named(
    program: &mut TypedTrees,
    symbol: SymbolHandle,
    name: &'static str,
) -> TypeReferenceHandle {
    program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol,
            name: Identifier::generated(name),
        })
}

fn name(program: &mut TypedTrees, symbol: SymbolHandle, spelled: &'static str) -> ExpressionHandle {
    let mut members = HandleSpan::empty();
    program
        .expression_table
        .push_name_path_member(&mut members, Identifier::generated(spelled));
    program
        .expression_table
        .insert(ExpressionNode::Name(TableNamePath {
            members,
            symbol,
            head_symbol: symbol,
            ..TableNamePath::default()
        }))
}

fn argument(symbol: SymbolHandle, spelled: &'static str) -> StaticMachineArgument {
    StaticMachineArgument {
        path: Box::new([Identifier::generated(spelled)]),
        symbol,
        ..StaticMachineArgument::default()
    }
}

/// `target<static>(operand)`, as an expression held by `holder`.
fn call(
    program: &mut TypedTrees,
    holder: SymbolHandle,
    target: SymbolHandle,
    spelled: &'static str,
    static_argument: StaticMachineArgument,
    operand: ExpressionHandle,
) -> ExpressionHandle {
    let application = program.expression_table.retain_static_machine_application(
        target,
        Identifier::generated(spelled),
        std::slice::from_ref(&static_argument),
    );
    program
        .expression_table
        .static_machine_application_mut(application)
        .lexical_owner = holder;
    let arguments = program
        .expression_table
        .insert_expression_handles([operand]);
    program
        .expression_table
        .insert(ExpressionNode::Call(TableCallExpression {
            runtime_call: None,
            authored_call_selection: None,
            authored_call_binding: None,
            build_selection: arena::Handle::invalid(),
            authored_machine_application: application,
            exact_coercion_target: TypeReferenceHandle::invalid(),
            requirement_application: TypeReferenceHandle::invalid(),
            receiver: ExpressionHandle::invalid(),
            target_symbol: target,
            static_machine_parameter: SymbolHandle::invalid(),
            target: Identifier::generated(spelled),
            static_requirement_dispatch: None,
            machine_arguments: Box::new([static_argument]),
            quotient_operation: None,
            private_layout_operation: None,
            installed_interrupt_operation: None,
            layout_query: None,
            native_code_layout: None,
            arguments,
            evidence_arguments: Box::new([]),
            operational_acknowledgement: Default::default(),
        }))
}

fn returns(program: &mut TypedTrees, value: ExpressionHandle) -> StatementNode {
    let target = program
        .statement_table
        .insert_transition_target(TransitionTargetNode::Value(value));
    StatementNode::Transition(TableTransition {
        target,
        ..TableTransition::default()
    })
}

fn push_machine(
    program: &mut TypedTrees,
    symbol: SymbolHandle,
    spelled: &'static str,
    type_parameters: HandleSpan<TypeParameter>,
    contracts: HandleSpan<SignatureContract>,
    state: State,
) {
    let states = program.machine_states.insert_many([state]);
    program.push_machine(Machine {
        symbol,
        name: Identifier::generated(spelled),
        type_parameters,
        contracts,
        states,
        body_is_present: true,
        ..Default::default()
    });
}

/// A generic machine `spelled<T>(item: T) -> T`, without its body.
fn generic(builder: &mut SymbolTableBuilder, machine: SymbolHandle) -> Generic {
    let children = builder.insert_children(
        machine,
        [
            (SymbolKind::TypeParameter, SymbolNameRef::Static("T")),
            (SymbolKind::State, SymbolNameRef::Static("entry")),
        ],
    );
    let (parameter, state) = (children.handle_at(0), children.handle_at(1));
    let item = builder
        .insert_children(
            state,
            [(SymbolKind::Parameter, SymbolNameRef::Static("item"))],
        )
        .handle_at(0);
    Generic {
        machine,
        state,
        parameter,
        item,
    }
}

fn signature(program: &mut TypedTrees, generic: &Generic) -> (HandleSpan<TypeParameter>, State) {
    let type_parameters = program.data_type_parameters.insert_many([TypeParameter {
        symbol: generic.parameter,
        name: Identifier::generated("T"),
        kind: TypeParameterKind::Type,
        ..Default::default()
    }]);
    let item_type = named(program, generic.parameter, "T");
    let parameters = program.state_parameters.insert_many([StateParameter {
        symbol: generic.item,
        name: Identifier::generated("item"),
        type_reference: item_type,
        ..Default::default()
    }]);
    let return_type = named(program, generic.parameter, "T");
    (
        type_parameters,
        State {
            symbol: generic.state,
            name: Identifier::generated("entry"),
            parameters,
            return_type,
            ..Default::default()
        },
    )
}

fn source() -> Source {
    source_named("relay")
}

fn source_named(relay_name: &'static str) -> Source {
    let mut builder = SymbolTableBuilder::new();
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let top = builder.insert_children(
        root,
        [
            (SymbolKind::BuiltinType, SymbolNameRef::Static("u8")),
            (SymbolKind::BuiltinType, SymbolNameRef::Static("bool")),
            (SymbolKind::BuiltinType, SymbolNameRef::Static("u64")),
            (SymbolKind::Machine, SymbolNameRef::Static("forward")),
            (SymbolKind::Machine, SymbolNameRef::Static(relay_name)),
            (SymbolKind::Machine, SymbolNameRef::Static("sized")),
            (SymbolKind::Machine, SymbolNameRef::Static("main")),
            (SymbolKind::Machine, SymbolNameRef::Static("counted")),
        ],
    );
    let [byte, boolean, wide, forward, relay, sized, main, counted] =
        std::array::from_fn(|position| top.handle_at(position as u32));
    let forward = generic(&mut builder, forward);
    let relay = generic(&mut builder, relay);
    let sized_children = builder.insert_children(
        sized,
        [
            (SymbolKind::TypeParameter, SymbolNameRef::Static("N")),
            (SymbolKind::State, SymbolNameRef::Static("entry")),
        ],
    );
    let (sized_parameter, sized_state) = (sized_children.handle_at(0), sized_children.handle_at(1));
    let state_of = |builder: &mut SymbolTableBuilder, machine| {
        builder
            .insert_children(
                machine,
                [(SymbolKind::State, SymbolNameRef::Static("entry"))],
            )
            .handle_at(0)
    };
    let main_state = state_of(&mut builder, main);
    let counted_state = state_of(&mut builder, counted);
    let locals = builder.insert_children(
        main_state,
        [
            (SymbolKind::Local, SymbolNameRef::Static("a")),
            (SymbolKind::Local, SymbolNameRef::Static("b")),
            (SymbolKind::Local, SymbolNameRef::Static("c")),
        ],
    );
    let mut program = TypedTrees {
        symbols: builder.finish(),
        ..Default::default()
    };

    // forward<T>(item: T) -> T { item }
    let (type_parameters, mut state) = signature(&mut program, &forward);
    let item = name(&mut program, forward.item, "item");
    let body = returns(&mut program, item);
    state.statement_nodes = program.statement_table.insert_statements([body]);
    push_machine(
        &mut program,
        forward.machine,
        "forward",
        type_parameters,
        HandleSpan::empty(),
        state,
    );

    // relay<T>(item: T) -> T ensures item { forward<T>(item) }
    let (type_parameters, mut state) = signature(&mut program, &relay);
    let promised = name(&mut program, relay.item, "item");
    let facts = program
        .proof_facts
        .insert_many([ProofFact::Expression(promised)]);
    let contracts = program.signature_contracts.insert_many([SignatureContract {
        kind: SignatureContractKind::Ensures,
        facts,
        ..Default::default()
    }]);
    let item = name(&mut program, relay.item, "item");
    let forwarded = call(
        &mut program,
        relay.machine,
        forward.state,
        "forward",
        argument(relay.parameter, "T"),
        item,
    );
    let body = returns(&mut program, forwarded);
    state.statement_nodes = program.statement_table.insert_statements([body]);
    push_machine(
        &mut program,
        relay.machine,
        relay_name,
        type_parameters,
        contracts,
        state,
    );

    // sized<const N: u64>() { }
    let carrier = named(&mut program, wide, "u64");
    let type_parameters = program.data_type_parameters.insert_many([TypeParameter {
        symbol: sized_parameter,
        name: Identifier::generated("N"),
        kind: TypeParameterKind::Const {
            type_reference: carrier,
        },
        ..Default::default()
    }]);
    push_machine(
        &mut program,
        sized,
        "sized",
        type_parameters,
        HandleSpan::empty(),
        State {
            symbol: sized_state,
            name: Identifier::generated("entry"),
            ..Default::default()
        },
    );

    // main() { let a = relay<u8>(1); let b = relay<u8>(2); let c = relay<bool>(true); }
    let mut statements = Vec::new();
    for (position, (applied, spelled, operand)) in [
        (byte, "u8", ExpressionNode::Boolean(false)),
        (byte, "u8", ExpressionNode::Boolean(false)),
        (boolean, "bool", ExpressionNode::Boolean(true)),
    ]
    .into_iter()
    .enumerate()
    {
        let operand = program.expression_table.insert(operand);
        let initial_value = call(
            &mut program,
            main,
            relay.state,
            relay_name,
            argument(applied, spelled),
            operand,
        );
        statements.push(StatementNode::LocalData(TableLocalData {
            symbol: locals.handle_at(position as u32),
            name: Identifier::generated(["a", "b", "c"][position]),
            initial_value,
            ..Default::default()
        }));
    }
    let statement_nodes = program.statement_table.insert_statements(statements);
    push_machine(
        &mut program,
        main,
        "main",
        HandleSpan::empty(),
        HandleSpan::empty(),
        State {
            symbol: main_state,
            name: Identifier::generated("entry"),
            statement_nodes,
            ..Default::default()
        },
    );

    // counted() { sized<3>(); }
    let three = StaticMachineArgument {
        path: Box::new([Identifier::generated("3")]),
        ..StaticMachineArgument::default()
    };
    let application = program.expression_table.retain_static_machine_application(
        sized_state,
        Identifier::generated("sized"),
        std::slice::from_ref(&three),
    );
    let statement_nodes = program
        .statement_table
        .insert_statements([StatementNode::Call(TableCall {
            authored_machine_application: application,
            target_symbol: sized_state,
            target: Identifier::generated("sized"),
            machine_arguments: Box::new([three]),
            ..Default::default()
        })]);
    push_machine(
        &mut program,
        counted,
        "counted",
        HandleSpan::empty(),
        HandleSpan::empty(),
        State {
            symbol: counted_state,
            name: Identifier::generated("entry"),
            statement_nodes,
            ..Default::default()
        },
    );

    Source {
        program,
        byte,
        boolean,
        forward,
        relay,
        sized_state,
        main,
        counted,
    }
}

fn only_state(program: &TypedTrees, machine: SymbolHandle) -> &State {
    let machine = program.machine_by_symbol(machine).expect("a machine");
    let [state] = program.machine_states(machine) else {
        panic!("one state");
    };
    state
}

/// The calls a machine's one state holds as local initializers or as the
/// value it returns, in statement order.
fn calls(program: &TypedTrees, machine: SymbolHandle) -> Vec<&TableCallExpression> {
    only_state(program, machine)
        .statement_nodes
        .handles()
        .filter_map(|handle| {
            let value = match program.statement_table.statement(handle) {
                StatementNode::LocalData(local) => local.initial_value,
                StatementNode::Transition(transition) => {
                    match program.statement_table.transition_target(transition.target) {
                        TransitionTargetNode::Value(value) => *value,
                        _ => return None,
                    }
                }
                _ => return None,
            };
            match program.expression_table.expression(value) {
                ExpressionNode::Call(call) => Some(call),
                _ => None,
            }
        })
        .collect()
}

fn named_symbol(program: &TypedTrees, reference: TypeReferenceHandle) -> SymbolHandle {
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Named { symbol, .. } => *symbol,
        other => panic!("a named type, found {other:?}"),
    }
}

#[test]
fn a_closed_type_application_enters_one_generated_machine_per_argument_tuple() {
    let Source {
        mut program,
        byte,
        boolean,
        relay,
        main,
        ..
    } = source();
    prepare(&mut program).expect("the pass refuses no program");

    let entered = calls(&program, main)
        .iter()
        .map(|call| {
            // The generated machine has no parameter left to apply.
            assert!(call.machine_arguments.is_empty());
            call.target_symbol
        })
        .collect::<Vec<_>>();
    let [at_byte, again, at_boolean] = entered[..] else {
        panic!("three calls");
    };
    assert_eq!(at_byte, again, "one application, one machine");
    assert_ne!(at_byte, at_boolean);

    for (state, applied) in [(at_byte, byte), (at_boolean, boolean)] {
        // The derivation names the generic machine's own state.
        assert_eq!(program.symbols.get(state).generated_from, relay.state);
        let machine = program.symbols.get(state).parent;
        let generated = program.machine_by_symbol(machine).expect("generated");
        assert!(program.machine_type_parameters(generated).is_empty());
        let state = only_state(&program, machine);
        let [item] = program.state_parameters(state) else {
            panic!("one formal");
        };
        assert_ne!(item.symbol, relay.item);
        assert_eq!(named_symbol(&program, item.type_reference), applied);
        assert_eq!(named_symbol(&program, state.return_type), applied);
        // The contract speaks of the generated machine's own formal.
        let [contract] = program.machine_contracts(generated) else {
            panic!("one clause");
        };
        let [ProofFact::Expression(promised)] = program.proof_facts.span_or_empty(contract.facts)
        else {
            panic!("one fact");
        };
        let ExpressionNode::Name(path) = program.expression_table.expression(*promised) else {
            panic!("a name");
        };
        assert_eq!(path.symbol, item.symbol);
        assert_eq!(
            program
                .executable_application_template(machine)
                .expect("one template"),
            Some(relay.machine)
        );
    }

    // The generic machine is left as written.
    let [item] = program.state_parameters(only_state(&program, relay.machine)) else {
        panic!("one formal");
    };
    assert_eq!(named_symbol(&program, item.type_reference), relay.parameter);
    assert!(program.deferred_refusal(main).is_none());
}

#[test]
fn a_query_name_without_query_identity_still_specializes() {
    let Source {
        mut program,
        relay,
        main,
        ..
    } = source_named("schema");
    assert_eq!(program.symbols.name(relay.machine), "schema");
    assert!(program.core_vocabulary(relay.machine).is_none());
    prepare(&mut program).expect("the ordinary generic call is served");
    let selected = calls(&program, main);
    assert_eq!(selected.len(), 3);
    for call in selected {
        assert!(call.machine_arguments.is_empty());
        assert_eq!(
            program.symbols.get(call.target_symbol).generated_from,
            relay.state
        );
        let generated = program.symbols.get(call.target_symbol).parent;
        assert_eq!(
            program.executable_application_template(generated).unwrap(),
            Some(relay.machine)
        );
    }
    assert!(program.deferred_refusal(main).is_none());
}

#[test]
fn an_application_inside_a_generated_body_is_served_in_turn() {
    let Source {
        mut program,
        byte,
        forward,
        relay,
        main,
        ..
    } = source();
    prepare(&mut program).expect("the pass refuses no program");

    // `forward<T>` in the generic `relay` stays open: it is bound to the
    // parameter and still enters the generic `forward`.
    let [open] = calls(&program, relay.machine)[..] else {
        panic!("one call");
    };
    assert_eq!(open.target_symbol, forward.state);
    let [binding] = program
        .expression_table
        .static_machine_application_bindings(open.authored_machine_application)
    else {
        panic!("one binding");
    };
    assert_eq!(binding.parameter, forward.parameter);
    let StaticMachineBindingValue::Type(bound) = &binding.value else {
        panic!("a type");
    };
    assert_eq!(named_symbol(&program, *bound), relay.parameter);
    assert!(program.deferred_refusal(relay.machine).is_none());

    // In `relay` at `u8` the same call is `forward<u8>` and enters the
    // machine generated for it.
    let at_byte = program
        .symbols
        .get(calls(&program, main)[0].target_symbol)
        .parent;
    let [closed] = calls(&program, at_byte)[..] else {
        panic!("one call");
    };
    assert!(closed.machine_arguments.is_empty());
    assert_eq!(
        program.symbols.get(closed.target_symbol).generated_from,
        forward.state
    );
    let forwarded = only_state(&program, program.symbols.get(closed.target_symbol).parent);
    let [item] = program.state_parameters(forwarded) else {
        panic!("one formal");
    };
    assert_eq!(named_symbol(&program, item.type_reference), byte);
    // `relay` and `forward`, each at `u8` and at `bool`.
    assert_eq!(program.machine_specializations.len(), 4);
}

#[test]
fn an_application_the_pass_does_not_serve_is_refused_on_its_holder_only() {
    let Source {
        mut program,
        sized_state,
        main,
        counted,
        ..
    } = source();
    prepare(&mut program).expect("the pass refuses no program");

    let state = only_state(&program, counted);
    let [StatementNode::Call(call)] = program.statement_table.statements(state.statement_nodes)
    else {
        panic!("one call");
    };
    // The call is as typing wrote it: nothing was generated for it.
    assert_eq!(call.target_symbol, sized_state);
    assert_eq!(call.machine_arguments.len(), 1);
    assert!(
        program
            .expression_table
            .static_machine_application_bindings(call.authored_machine_application)
            .is_empty()
    );
    assert_eq!(
        program.deferred_refusal(counted).map(|row| row.request),
        Some(DeferredRequest::StaticApplication)
    );
    assert!(program.deferred_refusal(main).is_none());
}
