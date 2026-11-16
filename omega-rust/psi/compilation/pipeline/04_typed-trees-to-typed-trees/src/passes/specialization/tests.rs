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
use numerics::integer_carrier::FixedInteger;
use symbols::{
    BUILTIN_TYPE_COUNT, BuiltinTypeAtom, SymbolHandle, SymbolKind, SymbolNameRef,
    SymbolTableBuilder, builtin_type_symbols,
};
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
        builtin_type_symbols().into_iter().chain([
            (SymbolKind::Machine, SymbolNameRef::Static("forward")),
            (SymbolKind::Machine, SymbolNameRef::Static(relay_name)),
            (SymbolKind::Machine, SymbolNameRef::Static("sized")),
            (SymbolKind::Machine, SymbolNameRef::Static("main")),
            (SymbolKind::Machine, SymbolNameRef::Static("counted")),
        ]),
    );
    let [forward, relay, sized, main, counted] =
        std::array::from_fn(|position| top.handle_at((BUILTIN_TYPE_COUNT + position) as u32));
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

    let [byte, boolean, wide] = [
        BuiltinTypeAtom::Integer(FixedInteger::unsigned(8)),
        BuiltinTypeAtom::Bool,
        BuiltinTypeAtom::Integer(FixedInteger::unsigned(64)),
    ]
    .map(|atom| program.symbols.builtin_atom_symbol(atom).unwrap());

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
    let Some([state]) = program.machine_states(machine).as_array() else {
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
fn qualified_operator_labels_do_not_hide_actual_attachment_obligations() {
    for attached in [false, true] {
        let Source {
            mut program,
            relay,
            main,
            byte,
            ..
        } = source();
        let attachment = named(&mut program, byte, "u8");
        let position = program
            .machines()
            .iter()
            .position(|machine| machine.symbol == relay.machine)
            .unwrap();
        let machine = program.machine_mut(position);
        machine.spelling = Some(language_core::operator_spelling::OperatorSpelling::Add);
        machine.attached_data = Some(Identifier::generated("Distance"));
        if attached {
            machine.attached_data_application = attachment;
        }
        prepare(&mut program).unwrap();
        assert_eq!(program.deferred_refusal(main).is_some(), attached);
        assert_eq!(
            program
                .machine_specializations
                .iter()
                .any(|row| row.template == relay.machine),
            !attached
        );
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
        let Some([item]) = program.state_parameters(state).as_array() else {
            panic!("one formal");
        };
        assert_ne!(item.symbol, relay.item);
        assert_eq!(named_symbol(&program, item.type_reference), applied);
        assert_eq!(named_symbol(&program, state.return_type), applied);
        // The contract speaks of the generated machine's own formal.
        let Some([contract]) = program.machine_contracts(generated).as_array() else {
            panic!("one clause");
        };
        let Some([ProofFact::Expression(promised)]) =
            program.proof_facts.span_or_empty(contract.facts).as_array()
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
    let Some([item]) = program
        .state_parameters(only_state(&program, relay.machine))
        .as_array()
    else {
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
fn copied_pattern_types_are_owned_by_each_application_not_the_template() {
    use typed_trees::expression::{
        MatchPattern, TableDestructurePattern, TableMatchArm, TableMatchExpression,
    };
    let Source {
        mut program,
        byte,
        boolean,
        relay,
        main,
        ..
    } = source();
    let reference = named(&mut program, relay.parameter, "T");
    let subject = name(&mut program, relay.item, "item");
    let arms = program.expression_table.insert_match_arms([TableMatchArm {
        pattern: MatchPattern::Destructure(TableDestructurePattern {
            application_type: reference,
            ..Default::default()
        }),
        value: subject,
        ..Default::default()
    }]);
    let dispatch = program
        .expression_table
        .insert(ExpressionNode::Match(TableMatchExpression {
            subject,
            arms,
            ..Default::default()
        }));
    program
        .expression_table
        .set_constructed_value_type_reference(dispatch, reference);
    program
        .expression_table
        .set_construction_required_type_reference(dispatch, reference);
    let statement = returns(&mut program, dispatch);
    let statements = program.statement_table.insert_statements([statement]);
    let state = program
        .machine_by_symbol(relay.machine)
        .unwrap()
        .states
        .start();
    program.machine_states.get_mut(state).statement_nodes = statements;

    prepare(&mut program).expect("the pattern application is copied");
    for (call, expected) in calls(&program, main).iter().zip([byte, byte, boolean]) {
        let machine = program.symbols.get(call.target_symbol).parent;
        let state = only_state(&program, machine);
        let mut found = false;
        for statement in program.statement_table.statements(state.statement_nodes) {
            for (_, root) in program.tables.expression_roots(statement) {
                program.subtree_for_each(root, Default::default(), |handle, node| {
                    if let ExpressionNode::Match(dispatch) = node {
                        let MatchPattern::Destructure(pattern) =
                            program.expression_table.match_arms(dispatch.arms)[0].pattern
                        else {
                            panic!("destructure");
                        };
                        assert_ne!(pattern.application_type, reference);
                        assert_eq!(named_symbol(&program, pattern.application_type), expected);
                        assert_eq!(
                            named_symbol(
                                &program,
                                program
                                    .expression_table
                                    .constructed_value_type_reference(handle)
                            ),
                            expected
                        );
                        assert_eq!(
                            named_symbol(
                                &program,
                                program
                                    .expression_table
                                    .construction_required_type_reference(handle)
                            ),
                            expected
                        );
                        found = true;
                    }
                });
            }
        }
        assert!(found);
    }
    assert_eq!(
        named_symbol(&program, reference),
        relay.parameter,
        "the generic declaration is unchanged"
    );
}

#[test]
fn generated_entry_ownership_refuses_missing_foreign_and_ambiguous_correspondence() {
    let Source {
        mut program,
        relay,
        forward,
        main,
        ..
    } = source();
    prepare(&mut program).unwrap();
    let entry = calls(&program, main)[0].target_symbol;
    let machine = program.symbols.get(entry).parent;
    let position = program
        .machine_specializations
        .iter()
        .position(|row| row.instance == machine)
        .unwrap();
    let row = program.machine_specializations[position].clone();
    assert_eq!(
        program.executable_callable_template(entry).unwrap(),
        Some(relay.state)
    );

    program
        .machine_specializations
        .edit(|rows| rows[position].states = HandleSpan::empty());
    assert!(
        program.executable_callable_template(entry).is_err(),
        "symbol provenance must not repair missing application ownership"
    );
    program
        .machine_specializations
        .edit(|rows| rows[position].states = row.states);
    program
        .captured_machine_states
        .get_mut(row.states.start())
        .template = forward.state;
    assert!(
        program.executable_callable_template(entry).is_err(),
        "another template's state cannot be substituted"
    );
    program
        .captured_machine_states
        .get_mut(row.states.start())
        .template = relay.state;
    program.machine_specializations.push(row);
    assert!(program.machine_specialization(machine).is_none());
    assert!(program.executable_application_template(machine).is_err());
    assert!(program.executable_callable_template(entry).is_err());
}

#[test]
fn a_source_application_key_uses_bound_arguments_not_generated_names() {
    let mut first = source();
    let mut later = source();
    later.program.symbols.insert_generated_root_from(
        later.relay.machine,
        SymbolKind::Machine,
        "unrelated",
    );
    prepare(&mut first.program).unwrap();
    prepare(&mut later.program).unwrap();
    let selected = |source: &Source, ordinal: usize| {
        let entry = calls(&source.program, source.main)[ordinal].target_symbol;
        source
            .program
            .machine_specialization(source.program.symbols.get(entry).parent)
            .unwrap()
            .clone()
    };
    let at_byte = selected(&first, 0);
    let later_byte = selected(&later, 0);
    assert_ne!(
        first.program.symbols.name(at_byte.instance),
        later.program.symbols.name(later_byte.instance)
    );
    let key = first
        .program
        .specialization_source_identity(&at_byte)
        .unwrap();
    assert_eq!(
        key,
        later
            .program
            .specialization_source_identity(&later_byte)
            .unwrap()
    );
    assert_ne!(
        key,
        first
            .program
            .specialization_source_identity(&selected(&first, 2))
            .unwrap()
    );
    let mut wrong = at_byte.clone();
    wrong.binding_application = selected(&first, 2).binding_application;
    assert_ne!(
        key,
        first
            .program
            .specialization_source_identity(&wrong)
            .unwrap()
    );
    first
        .program
        .expression_table
        .set_static_machine_application_bindings(at_byte.binding_application, &[]);
    assert!(
        first
            .program
            .specialization_source_identity(&at_byte)
            .is_err()
    );
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
    let Some([item]) = program.state_parameters(forwarded).as_array() else {
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
    let source_operand = program
        .specialization_source_operand(program.machine_specialization(generated).unwrap())
        .unwrap();
    assert_eq!(
        source_operand.application.unwrap().arguments[0]
            .const_literal
            .as_ref()
            .unwrap()
            .canonical_spelling()
            .as_str(),
        "3",
        "Build captures the same literal, not the parameter's receiving type"
    );
    let promised = |machine: SymbolHandle| {
        let machine = program.machine_by_symbol(machine).expect("a machine");
        let Some([contract]) = program.machine_contracts(machine).as_array() else {
            panic!("one contract");
        };
        let Some([ProofFact::Expression(expression)]) =
            program.proof_facts.span_or_empty(contract.facts).as_array()
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
    let generated_application = |value| {
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
        (
            program.symbols.name(machine).to_owned(),
            program
                .specialization_source_identity(program.machine_specialization(machine).unwrap())
                .unwrap(),
        )
    };
    assert_eq!(generated_application(3), generated_application(3));
    let three = generated_application(3);
    let four = generated_application(4);
    assert_ne!(three.0, four.0);
    assert_ne!(three.1, four.1, "the source key retains the static value");
    assert!(four.0.starts_with("sized$specialized$4$"));
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
        let Some([item]) = program.state_parameters(state).as_array() else {
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
            program.machine_specializations.edit(|rows| {
                rows.iter_mut()
                    .find(|row| row.instance == child)
                    .unwrap()
                    .binding_application = foreign
            });
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
        Err(super::Stop::Unserved(None))
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

#[test]
fn copied_progress_keeps_its_promise_and_rebinds_private_witnesses() {
    use language_semantics::{
        ProgressPremise, ProgressSubject, RankingViewId, RankingWitness, SemanticDomainId,
        TerminationGuarantee, TerminationInterface,
    };

    for missing_custody in [false, true] {
        let Source {
            mut program,
            relay,
            main,
            ..
        } = source();
        let subject = name(&mut program, relay.item, "item");
        let witness = RankingWitness {
            subjects: vec!["item".into()],
            ranking_view: RankingViewId::NAT_DESCENDING,
            view_path: "Nat::Descending".into(),
            ..RankingWitness::default()
        };
        let position = program
            .machines()
            .iter()
            .position(|machine| machine.symbol == relay.machine)
            .unwrap();
        let original = program.machine_mut(position);
        original.termination_plan.interface =
            TerminationInterface::Published(TerminationGuarantee::Terminates {
                premises: vec![ProgressPremise {
                    profile: SemanticDomainId::default(),
                    subject: ProgressSubject {
                        root: relay.item,
                        projections: Vec::new(),
                    },
                }],
            });
        original.termination_plan.implementation_witness = Some(witness.clone());
        let original_plan = original.termination_plan.clone();
        if !missing_custody {
            program.ranking_expression_custody.push(
                typed_trees::ranking::RankingExpressionCustody {
                    machine: relay.machine,
                    subjects: vec![subject],
                    ..Default::default()
                },
            );
        }
        prepare(&mut program).unwrap();
        if missing_custody {
            assert!(
                program
                    .deferred_refusal(main)
                    .unwrap()
                    .detail
                    .contains("original expression custody")
            );
            assert_eq!(calls(&program, main)[0].target_symbol, relay.state);
            continue;
        }
        let mut copied_subjects = Vec::new();
        for call in [calls(&program, main)[0], calls(&program, main)[2]] {
            let owner = program.symbols.get(call.target_symbol).parent;
            let machine = program.machine_by_symbol(owner).unwrap();
            let Some([formal]) = program
                .state_parameters(only_state(&program, owner))
                .as_array()
            else {
                panic!("one copied formal");
            };
            assert_ne!(formal.symbol, relay.item);
            let TerminationInterface::Published(TerminationGuarantee::Terminates { premises }) =
                &machine.termination_plan.interface
            else {
                panic!("the original public promise survives");
            };
            assert_eq!(premises.len(), 1);
            assert_eq!(premises[0].subject.root, formal.symbol);
            assert_eq!(
                machine.termination_plan.checked_summary,
                TerminationGuarantee::NoGuarantee
            );
            assert_eq!(
                machine.termination_plan.implementation_witness,
                Some(witness.clone())
            );
            let custody = program.ranking_expression_custody_for(owner).unwrap();
            let [copied] = custody.subjects.as_slice() else {
                panic!("one copied ranking subject");
            };
            assert_ne!(*copied, subject);
            assert_eq!(
                program
                    .expression_table
                    .name_path_node(*copied)
                    .unwrap()
                    .symbol,
                formal.symbol
            );
            copied_subjects.push(*copied);
        }
        assert_ne!(copied_subjects[0], copied_subjects[1]);
        assert_eq!(
            program
                .machine_by_symbol(relay.machine)
                .unwrap()
                .termination_plan,
            original_plan
        );
        assert_eq!(
            program
                .expression_table
                .name_path_node(subject)
                .unwrap()
                .symbol,
            relay.item
        );
    }
}

#[test]
fn the_application_index_finds_each_generated_machine_by_its_complete_key() {
    // Applications of three unrelated templates arrive interleaved and out of
    // key order. Each distinct (template, arguments) key gets one row, rows
    // keep encounter order, and an equal key from another template or with
    // another argument kind is a different application.
    use super::{Generated, GeneratedMachines, Identity};
    let templates = [3, 1, 2].map(|slot| SymbolHandle::from_parts(slot, 1));
    let arguments = |n: u32| -> Vec<Identity> {
        match n % 3 {
            0 => vec![Identity::Declaration(SymbolHandle::from_parts(100 + n, 1))],
            1 => vec![
                Identity::Scalar(format!("{n}")),
                Identity::Declaration(SymbolHandle::from_parts(n, 2)),
            ],
            _ => vec![
                Identity::Type(Default::default()),
                Identity::Scalar(format!("{n}")),
            ],
        }
    };
    let requests = (0..300u32)
        .rev()
        .flat_map(|n| templates.iter().map(move |&template| (template, n)))
        .collect::<Vec<_>>();
    let mut generated = GeneratedMachines::default();
    for (order, &(template, n)) in requests.iter().enumerate() {
        let key = arguments(n);
        let slot = generated
            .search(template, &key)
            .expect_err("a first request is not yet generated");
        let made = generated.insert(
            slot,
            Generated {
                template,
                arguments: key.clone(),
                machine: SymbolHandle::from_parts(10_000 + order as u32, 1),
                state: SymbolHandle::from_parts(20_000 + order as u32, 1),
            },
        );
        assert!(made.arguments == key);
    }
    for (order, &(template, n)) in requests.iter().enumerate() {
        let row = generated
            .search(template, &arguments(n))
            .expect("a repeated request finds its machine");
        assert_eq!(row, order, "rows keep the order applications were met");
        assert_eq!(generated.rows[row].template, template);
    }
    // Same payload, different template, generation or argument kind: unseen.
    let unseen = [
        (SymbolHandle::from_parts(4, 1), arguments(0)),
        (
            templates[0],
            vec![Identity::Declaration(SymbolHandle::from_parts(100, 2))],
        ),
        (templates[0], vec![Identity::Scalar("0".into())]),
        (templates[0], Vec::new()),
    ];
    for (template, key) in unseen {
        assert!(generated.search(template, &key).is_err());
    }
}

#[test]
fn retained_type_arguments_require_the_exact_lifetime_scope() {
    let mut builder = SymbolTableBuilder::default();
    let data = builder.insert_root(SymbolKind::Data, SymbolNameRef::Static("Message"));
    let caller = builder.insert_root(SymbolKind::Machine, SymbolNameRef::Static("caller"));
    let foreign = builder.insert_root(SymbolKind::Machine, SymbolNameRef::Static("foreign"));
    let mut program = TypedTrees {
        symbols: builder.finish(),
        ..Default::default()
    };
    program.push_data_definition(typed_trees::data::DataDefinition {
        symbol: data,
        name: Identifier::generated("Message"),
        lifetime_parameters: vec![Identifier::generated("text")],
        ..Default::default()
    });
    for (symbol, name, region) in [(caller, "caller", "bytes"), (foreign, "foreign", "bytes")] {
        let state = program.machine_states.insert_many([State::default()]);
        program.push_machine(Machine {
            symbol,
            name: Identifier::generated(name),
            lifetime_parameters: vec![Identifier::generated(region)],
            states: state,
            ..Default::default()
        });
    }
    let original = program.machine_by_symbol(caller).unwrap().clone();
    let identity = program.machine_callable_identity_text(&original);
    assert!(
        identity.is_some(),
        "the unchanged caller has a published identity"
    );
    // The selected source application and its retained type must agree even
    // when both scopes declare the same spelling. Only its original resolved
    // declaration/ordinal can supply that region after an application is copied.
    let mut selected = StaticMachineArgument {
        symbol: data,
        path: vec![Identifier::generated("Message")].into_boxed_slice(),
        application: Some(Box::new(typed_trees::expression::StaticSymbolApplication {
            lifetime_arguments: vec![Identifier::generated("bytes")].into_boxed_slice(),
            member: arena::Handle::invalid(),
            arguments: Box::new([]),
        })),
        ..Default::default()
    };
    selected.type_reference = program
        .static_operand_type_reference_in_scope(&selected, caller)
        .unwrap();
    selected.type_reference = program
        .apply_callable_type_in_scope(
            selected.type_reference,
            &[],
            &[(
                Identifier::generated("bytes"),
                Identifier::generated("bytes"),
            )],
            caller,
            caller,
            &[],
        )
        .unwrap();
    assert!(super::named_type(&mut program, &selected).is_ok());
    let selected_identity = typed_trees::type_identity::applied_type_identity(
        &program,
        selected.type_reference,
        caller,
        &[],
    )
    .unwrap();
    let mut legitimate_foreign = selected.clone();
    legitimate_foreign.type_reference = TypeReferenceHandle::invalid();
    legitimate_foreign.type_reference = program
        .static_operand_type_reference_in_scope(&legitimate_foreign, foreign)
        .unwrap();
    legitimate_foreign.type_reference = program
        .apply_callable_type_in_scope(
            legitimate_foreign.type_reference,
            &[],
            &[(
                Identifier::generated("bytes"),
                Identifier::generated("bytes"),
            )],
            foreign,
            foreign,
            &[],
        )
        .unwrap();
    assert!(super::named_type(&mut program, &legitimate_foreign).is_ok());
    assert_ne!(
        selected_identity,
        typed_trees::type_identity::applied_type_identity(
            &program,
            legitimate_foreign.type_reference,
            foreign,
            &[]
        )
        .unwrap(),
        "same-spelled lifetimes retain distinct exact supplying scopes"
    );
    let retained = program
        .type_reference_table
        .operand_lifetimes(selected.type_reference)
        .to_vec();
    assert_eq!(
        retained,
        vec![typed_trees::type_identity::LifetimeParameterIdentity {
            declaration: caller,
            ordinal: 0
        }]
    );
    let mut exchanged = selected.clone();
    exchanged.application.as_mut().unwrap().lifetime_arguments[0] = Identifier::generated("other");
    assert!(super::named_type(&mut program, &exchanged).is_err());
    assert_eq!(
        program
            .type_reference_table
            .operand_lifetimes(selected.type_reference),
        retained
    );
    let original_type = program
        .type_reference_table
        .type_reference(selected.type_reference)
        .clone();
    let contradictory = program.type_reference_table.insert(original_type);
    program
        .type_reference_table
        .retain_operand_lifetime_owner(contradictory, foreign)
        .unwrap();
    program
        .type_reference_table
        .retain_operand_lifetimes(contradictory, retained.iter().copied())
        .unwrap();
    let mut exchanged = selected.clone();
    exchanged.type_reference = contradictory;
    assert!(
        super::named_type(&mut program, &exchanged).is_err(),
        "a same-spelled foreign binder cannot replace the actual retained region"
    );
    for (owner, lifetime, accepted) in [
        (caller, "bytes", true),
        (foreign, "other", false),
        (SymbolHandle::invalid(), "bytes", false),
        (caller, "missing", false),
    ] {
        let reference = program
            .type_reference_table
            .insert(TypeReferenceNode::Generic {
                base_symbol: data,
                base_name: Identifier::generated("Message"),
                lifetime_arguments: vec![Identifier::generated(lifetime)],
                arguments: HandleSpan::empty(),
            });
        program
            .type_reference_table
            .retain_operand_lifetime_owner(reference, owner)
            .unwrap();
        let argument = StaticMachineArgument {
            type_reference: reference,
            ..Default::default()
        };
        assert_eq!(super::named_type(&mut program, &argument).is_ok(), accepted);
        assert_eq!(
            program
                .machine_by_symbol(caller)
                .unwrap()
                .lifetime_parameters,
            original.lifetime_parameters
        );
        assert_eq!(
            program.machine_callable_identity_text(program.machine_by_symbol(caller).unwrap()),
            identity
        );
    }
}

#[test]
fn a_retained_type_parameter_stays_open_and_keeps_its_selected_declaration() {
    let Source {
        mut program,
        forward,
        relay,
        boolean,
        ..
    } = source();
    let mut selected = argument(relay.parameter, "T");
    selected.type_reference = program
        .static_operand_type_reference_in_scope(&selected, relay.machine)
        .expect("the authored type argument retains its original scope");
    assert!(
        !program
            .prepare_data_application_view(selected.type_reference)
            .unwrap(),
        "an open parameter is not a closed data application"
    );
    let operand = super::Operand::read(
        &mut program,
        forward.parameter,
        &TypeParameterKind::Type,
        &selected,
    )
    .unwrap_or_else(|_| panic!("an open type parameter can be passed to another generic body"));
    assert!(operand.is_open(&program));
    assert_eq!(operand.declaration(), Some(relay.parameter));
    assert_eq!(
        operand.value,
        StaticMachineBindingValue::Type(selected.type_reference)
    );
    assert_eq!(
        program
            .type_reference_table
            .operand_lifetime_owner(selected.type_reference),
        relay.machine
    );

    let mut changed = selected.clone();
    changed.symbol = boolean;
    assert!(super::named_type(&mut program, &changed).is_err());
    let parameters = program
        .machine_by_symbol(relay.machine)
        .unwrap()
        .type_parameters;
    let carrier = named(&mut program, boolean, "bool");
    program
        .data_type_parameters
        .get_mut(parameters.start())
        .kind = TypeParameterKind::Const {
        type_reference: carrier,
    };
    assert!(
        super::named_type(&mut program, &selected).is_err(),
        "a const parameter cannot be passed as a type"
    );
}
