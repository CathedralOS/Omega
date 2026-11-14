//! One hand-built program, as typing leaves it:
//!
//! ```omega
//! machine forward<T>(item: T) -> T { item }
//! machine relay<T>(item: T) -> T ensures item { forward<T>(item) }
//! machine sized<const N: u64>() ensures N { }
//! machine main() {
//!     let a = relay<u8>(1);
//!     let b = relay<u8>(2);
//!     let c = relay<bool>(true);
//! }
//! machine counted() { sized<3>(); }
//! ```
//!
//! `main` holds closed type applications, which are served. `counted` holds
//! an application of a value parameter whose argument typing left as a
//! spelling with no literal, which is not: it is the failing twin. The same
//! call with the literal `3` is served (`counted_at`).

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
    nested: Option<Generic>,
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
    source_with_nested(relay_name, false)
}

fn source_with_nested(relay_name: &'static str, with_nested: bool) -> Source {
    source_with_child_telescope(relay_name, with_nested, false)
}

fn source_with_child_telescope(
    relay_name: &'static str,
    with_nested: bool,
    independent_child: bool,
) -> Source {
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
    let (relay, nested) = if with_nested {
        let children = builder.insert_children(
            relay,
            [
                (SymbolKind::TypeParameter, SymbolNameRef::Static("T")),
                (SymbolKind::State, SymbolNameRef::Static("entry")),
            ],
        );
        let (parameter, state) = (children.handle_at(0), children.handle_at(1));
        let children = builder.insert_children(
            state,
            [
                (SymbolKind::Parameter, SymbolNameRef::Static("item")),
                (SymbolKind::Machine, SymbolNameRef::Static("nested")),
            ],
        );
        let (item, machine) = (children.handle_at(0), children.handle_at(1));
        let child_roster = if independent_child {
            vec![
                (SymbolKind::State, SymbolNameRef::Static("entry")),
                (SymbolKind::TypeParameter, SymbolNameRef::Static("T")),
            ]
        } else {
            vec![(SymbolKind::State, SymbolNameRef::Static("entry"))]
        };
        let children = builder.insert_children(machine, child_roster);
        let nested_state = children.handle_at(0);
        let nested_parameter = if independent_child {
            children.handle_at(1)
        } else {
            parameter
        };
        let nested_item = builder
            .insert_children(
                nested_state,
                [(SymbolKind::Parameter, SymbolNameRef::Static("item"))],
            )
            .handle_at(0);
        (
            Generic {
                machine: relay,
                state,
                parameter,
                item,
            },
            Some(Generic {
                machine,
                state: nested_state,
                parameter: nested_parameter,
                item: nested_item,
            }),
        )
    } else {
        (generic(&mut builder, relay), None)
    };
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
    if let Some(nested) = &nested {
        let ExpressionNode::Call(call) = program.expression_table.expression_mut(forwarded) else {
            panic!("call fixture");
        };
        call.target_symbol = nested.state;
        call.target = Identifier::generated("nested");
        let application = call.authored_machine_application;
        program
            .expression_table
            .static_machine_application_mut(application)
            .registration_operation = nested.state;
    }
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

    if let Some(nested) = &nested {
        let type_parameters = program.data_type_parameters.insert_many([TypeParameter {
            symbol: nested.parameter,
            name: Identifier::generated("T"),
            kind: TypeParameterKind::Type,
            ..Default::default()
        }]);
        let item_type = named(&mut program, nested.parameter, "T");
        let parameters = program.state_parameters.insert_many([StateParameter {
            symbol: nested.item,
            name: Identifier::generated("item"),
            type_reference: item_type,
            ..Default::default()
        }]);
        let item = name(&mut program, nested.item, "item");
        let forwarded = call(
            &mut program,
            nested.machine,
            forward.state,
            "forward",
            argument(nested.parameter, "T"),
            item,
        );
        let body = returns(&mut program, forwarded);
        let state = State {
            symbol: nested.state,
            name: Identifier::generated("entry"),
            parameters,
            return_type: item_type,
            statement_nodes: program.statement_table.insert_statements([body]),
            ..Default::default()
        };
        push_machine(
            &mut program,
            nested.machine,
            "nested",
            type_parameters,
            HandleSpan::empty(),
            state,
        );
    }

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
    let promised = name(&mut program, sized_parameter, "N");
    let facts = program
        .proof_facts
        .insert_many([ProofFact::Expression(promised)]);
    let contracts = program.signature_contracts.insert_many([SignatureContract {
        kind: SignatureContractKind::Ensures,
        facts,
        ..Default::default()
    }]);
    push_machine(
        &mut program,
        sized,
        "sized",
        type_parameters,
        contracts,
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
        nested,
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

/// `source()` with `counted`'s one call given the integer literal `value`
/// as typing records one: `sized<value>()`.
fn counted_at(value: i64) -> Source {
    let mut source = source();
    let literal = StaticMachineArgument {
        const_literal: Some(numerics::literals::IntegerLiteral::from_value(value)),
        ..StaticMachineArgument::default()
    };
    let program = &mut source.program;
    let statements = only_state(program, source.counted).statement_nodes;
    let handle = statements.handles().next().expect("one call");
    let StatementNode::Call(call) = program.statement_table.statement_mut(handle) else {
        panic!("one call");
    };
    call.machine_arguments = Box::new([literal.clone()]);
    let application = call.authored_machine_application;
    program
        .expression_table
        .static_machine_application_arguments_mut(application)[0] = literal;
    source
}

/// A value parameter given one integer literal is served like a type
/// parameter: the call enters a machine generated for that value, the
/// application binds the parameter as a constant of its declared type, and
/// where the copied contract named the parameter it holds the literal,
/// landed in the parameter's type. The generic machine keeps the name.
#[test]
fn a_literal_value_application_enters_a_machine_with_the_value_in_place() {
    let Source {
        mut program,
        sized_state,
        counted,
        ..
    } = counted_at(3);
    prepare(&mut program).expect("the pass refuses no program");

    let state = only_state(&program, counted);
    let [StatementNode::Call(call)] = program.statement_table.statements(state.statement_nodes)
    else {
        panic!("one call");
    };
    assert!(program.deferred_refusal(counted).is_none());
    assert_ne!(call.target_symbol, sized_state);
    assert!(call.machine_arguments.is_empty());
    let [binding] = program
        .expression_table
        .static_machine_application_bindings(call.authored_machine_application)
    else {
        panic!("one binding");
    };
    assert!(matches!(binding.value, StaticMachineBindingValue::Const(_)));

    let generated = program.symbols.get(call.target_symbol).parent;
    let sized = program.symbols.get(sized_state).parent;
    let promised = |machine: SymbolHandle| {
        let machine = program.machine_by_symbol(machine).expect("a machine");
        let [contract] = program.machine_contracts(machine) else {
            panic!("one contract");
        };
        let [ProofFact::Expression(expression)] = program.proof_facts.span_or_empty(contract.facts)
        else {
            panic!("one fact");
        };
        program.expression_table.expression(*expression).clone()
    };
    let ExpressionNode::Integer(literal) = promised(generated) else {
        panic!("the generated machine promises a literal");
    };
    assert_eq!(literal.canonical_spelling().as_str(), "3");
    assert_eq!(
        literal.landing().map(|landing| landing.landed_type),
        Some(numerics::literals::LandedIntegerType::U64)
    );
    assert!(matches!(promised(sized), ExpressionNode::Name(_)));
    assert!(
        program
            .machine_by_symbol(generated)
            .is_some_and(|machine| { machine.type_parameters.is_empty() })
    );
}

/// One value, one machine: two programs that differ only in the literal
/// name their generated machines apart, and the machine is keyed by the
/// value, not by where the call stands.
#[test]
fn a_generated_machine_is_named_for_its_value() {
    let generated_name = |value| {
        let Source {
            mut program,
            counted,
            ..
        } = counted_at(value);
        prepare(&mut program).expect("the pass refuses no program");
        let state = only_state(&program, counted);
        let [StatementNode::Call(call)] = program.statement_table.statements(state.statement_nodes)
        else {
            panic!("one call");
        };
        let machine = program.symbols.get(call.target_symbol).parent;
        program.symbols.name(machine).to_owned()
    };
    assert_eq!(generated_name(3), generated_name(3));
    assert_ne!(generated_name(3), generated_name(4));
    assert!(generated_name(4).starts_with("sized$specialized$4$"));
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

#[test]
fn nested_declarations_keep_each_enclosing_application_and_their_own_call_owner() {
    let Source {
        mut program,
        byte,
        boolean,
        relay,
        nested,
        main,
        ..
    } = source_with_nested("relay", true);
    let nested = nested.unwrap();
    prepare(&mut program).unwrap();
    let entered = calls(&program, main)
        .iter()
        .map(|call| call.target_symbol)
        .collect::<Vec<_>>();
    assert_eq!(entered[0], entered[1]);
    assert_ne!(entered[0], entered[2]);
    let mut children = Vec::new();
    for (entry, actual) in [(entered[0], byte), (entered[2], boolean)] {
        let owner = program.symbols.get(entry).parent;
        let child_call = calls(&program, owner)[0];
        assert!(child_call.machine_arguments.is_empty());
        let child_entry = child_call.target_symbol;
        let child = program.symbols.get(child_entry).parent;
        children.push(child);
        assert_eq!(program.symbols.get(child).parent, entry);
        assert_eq!(program.symbols.get(child).generated_from, nested.machine);
        assert_eq!(
            program.symbols.get(child_entry).generated_from,
            nested.state
        );
        assert_eq!(
            program.executable_application_template(child).unwrap(),
            Some(nested.machine)
        );
        let state = only_state(&program, child);
        let [item] = program.state_parameters(state) else {
            panic!("one formal");
        };
        assert_ne!(item.symbol, nested.item);
        assert_eq!(named_symbol(&program, item.type_reference), actual);
        assert_eq!(named_symbol(&program, state.return_type), actual);
        let call = calls(&program, child)[0];
        let selected = program.symbols.get(call.target_symbol).parent;
        assert_eq!(
            named_symbol(&program, only_state(&program, selected).return_type),
            actual
        );
        assert_eq!(
            program
                .expression_table
                .static_machine_application(call.authored_machine_application)
                .lexical_owner,
            child
        );
        let receipt = program.machine_specialization(child).unwrap();
        assert_eq!(
            receipt.template_parameters,
            program
                .machine_by_symbol(relay.machine)
                .unwrap()
                .type_parameters
        );
        assert_eq!(
            program
                .machine_specialization(owner)
                .unwrap()
                .binding_application,
            receipt.binding_application
        );
    }
    assert_ne!(children[0], children[1]);
    assert_eq!(
        named_symbol(&program, only_state(&program, nested.machine).return_type),
        relay.parameter
    );
    assert!(program.deferred_refusal(main).is_none());
}

#[test]
fn an_unserved_child_telescope_leaves_the_whole_specialization_unwritten() {
    let Source {
        mut program,
        relay,
        nested,
        main,
        ..
    } = source_with_child_telescope("relay", true, true);
    let nested = nested.unwrap();
    assert_ne!(nested.parameter, relay.parameter);
    assert_eq!(program.symbols.get(nested.parameter).parent, nested.machine);
    let symbols = program.symbols.symbols().len();
    let machines = program.machines().len();
    prepare(&mut program).unwrap();
    assert_eq!(program.symbols.symbols().len(), symbols);
    assert_eq!(program.machines().len(), machines);
    assert!(program.deferred_refusal(main).is_some());
}

#[test]
fn an_existing_nested_copy_does_not_accept_another_argument_or_source_selection() {
    for foreign_owner in [false, true] {
        let Source {
            mut program,
            byte,
            boolean,
            main,
            forward,
            ..
        } = source_with_nested("relay", true);
        prepare(&mut program).unwrap();
        let owner = program
            .symbols
            .get(calls(&program, main)[0].target_symbol)
            .parent;
        let position = program
            .machines()
            .iter()
            .position(|machine| machine.symbol == owner)
            .unwrap();
        let site = super::call_sites(&program, position)[0];
        let child = super::entered_machine(&program, site.applied(&program).unwrap().target)
            .unwrap()
            .0;
        let requested = if foreign_owner { byte } else { boolean };
        let argument = argument(requested, if foreign_owner { "u8" } else { "bool" });
        let application = site.call(&program).unwrap().application();
        let super::Site::Expression(handle) = site else {
            panic!("fixture expression");
        };
        let ExpressionNode::Call(call) = program.expression_table.expression_mut(handle) else {
            panic!("fixture call");
        };
        call.machine_arguments = vec![argument.clone()].into_boxed_slice();
        program
            .expression_table
            .static_machine_application_arguments_mut(application)[0] = argument;
        if foreign_owner {
            let foreign = program
                .machine_specializations
                .iter()
                .find(|row| row.template == forward.machine)
                .unwrap()
                .binding_application;
            program
                .machine_specializations
                .iter_mut()
                .find(|row| row.instance == child)
                .unwrap()
                .binding_application = foreign;
        }
        let symbols = program.symbols.symbols().len();
        prepare(&mut program).unwrap();
        assert_eq!(program.symbols.symbols().len(), symbols);
        assert_eq!(
            program.deferred_refusal(owner).map(|row| row.request),
            Some(DeferredRequest::StaticApplication)
        );
        assert_eq!(calls(&program, owner)[0].machine_arguments.len(), 1);
    }
}

#[test]
fn an_already_bound_tuple_keeps_even_an_unused_parameter() {
    let mut builder = SymbolTableBuilder::default();
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let declarations = builder.insert_children(
        root,
        [
            (SymbolKind::BuiltinType, SymbolNameRef::Static("u8")),
            (SymbolKind::BuiltinType, SymbolNameRef::Static("bool")),
            (SymbolKind::Machine, SymbolNameRef::Static("owner")),
        ],
    );
    let byte = declarations.handle_at(0);
    let boolean = declarations.handle_at(1);
    let owner = declarations.handle_at(2);
    let binders = builder.insert_children(
        owner,
        [
            (SymbolKind::TypeParameter, SymbolNameRef::Static("T")),
            (SymbolKind::TypeParameter, SymbolNameRef::Static("Unused")),
        ],
    );
    let parameters = binders
        .handles()
        .zip(["T", "Unused"])
        .map(|(symbol, name)| TypeParameter {
            symbol,
            name: Identifier::generated(name),
            ..Default::default()
        })
        .collect::<Vec<_>>();
    let mut program = TypedTrees {
        symbols: builder.finish(),
        ..Default::default()
    };
    let actual = [argument(byte, "u8"), argument(boolean, "bool")];
    let application = program.expression_table.retain_static_machine_application(
        owner,
        Identifier::generated("owner"),
        &actual,
    );
    let selected = parameters
        .iter()
        .zip(&actual)
        .map(|(parameter, argument)| {
            super::Operand::read(&mut program, parameter.symbol, &parameter.kind, argument)
                .unwrap_or_else(|_| panic!("closed argument"))
        })
        .collect::<Vec<_>>();
    super::bind(&mut program, application, &selected);
    assert!(
        super::bound_arguments(&mut program, &parameters, &parameters, application, &actual)
            .is_ok()
    );
    let different_unused = [argument(byte, "u8"), argument(byte, "u8")];
    assert!(matches!(
        super::bound_arguments(
            &mut program,
            &parameters,
            &parameters,
            application,
            &different_unused
        ),
        Err(super::Stop::Unserved)
    ));
}

#[test]
fn copied_headers_retain_written_empty_and_nonempty_reach_rows() {
    let Source {
        mut program,
        forward,
        relay,
        ..
    } = source();
    let service =
        program
            .symbols
            .insert_generated_root_from(forward.machine, SymbolKind::Trait, "Service");
    for (owner, targets) in [
        (relay.machine, Vec::new()),
        (
            forward.machine,
            vec![typed_trees::signature::AuthoredServiceReachTarget {
                service,
                source_span: Default::default(),
            }],
        ),
    ] {
        program
            .authored_service_reach_rows
            .push(typed_trees::signature::AuthoredServiceReachRow {
                owner,
                keyword_source_spans: vec![Default::default()],
                targets,
                installation_bound: false,
            });
    }
    prepare(&mut program).unwrap();
    for template in [relay.machine, forward.machine] {
        let original = program
            .authored_service_reach_rows_for(template)
            .next()
            .unwrap();
        let mut count = 0;
        for row in program
            .machine_specializations
            .iter()
            .filter(|row| row.template == template)
        {
            let copied = program
                .authored_service_reach_rows_for(row.instance)
                .collect::<Vec<_>>();
            assert_eq!(copied.len(), 1);
            let mut expected = original.clone();
            expected.owner = row.instance;
            assert_eq!(copied[0], &expected);
            count += 1;
        }
        assert!(count > 0);
    }
}

#[test]
fn an_installation_bound_reach_row_remains_unserved_before_copying() {
    let Source {
        mut program,
        relay,
        main,
        ..
    } = source();
    program
        .authored_service_reach_rows
        .push(typed_trees::signature::AuthoredServiceReachRow {
            owner: relay.machine,
            keyword_source_spans: vec![Default::default()],
            targets: Vec::new(),
            installation_bound: true,
        });
    let count = program.machines().len();
    prepare(&mut program).unwrap();
    assert_eq!(program.machines().len(), count);
    assert!(program.deferred_refusal(main).is_some());
}
