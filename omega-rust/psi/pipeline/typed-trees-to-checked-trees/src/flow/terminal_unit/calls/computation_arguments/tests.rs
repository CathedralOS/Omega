use super::super::super::scalar_targets::registered_structural_graph_target;
use super::super::super::structural_scalar_graph_signature;
use super::*;

const SOURCE: &str = r#"
data Limits { limit: u64; divisor: u64 [3..=5]; }
machine reset(value: &mut u64) -> u64 { value = 0; 0 }
machine inspect(marker: u64, limits: Limits) -> u64 {
    let mut scratch: u64 = marker;
    let reset_result: u64 = reset(&mut scratch);
    limits.limit
}
machine enter(limits: Limits, marker: u64) -> u64 {
    let inspected: u64 = inspect(marker, limits);
    inspected
}
"#;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    crate::lower_typed_trees(typed).unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"))
}

fn machine<'program>(
    program: &'program TypedTrees,
    name: &str,
) -> &'program typed_trees::machine::Machine {
    program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .unwrap()
}

#[test]
fn owned_scalar_graphs_read_and_forward_customer_limits() {
    let checked = checked(SOURCE);
    for (name, scalar_position, owned_position, local_count) in
        [("inspect", 0, 1, 1), ("enter", 1, 0, 0)]
    {
        let machine = machine(&checked.typed, name);
        assert_eq!(checked.machine_states(machine).len(), 1);
        let graph = checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(machine.symbol)
            .expect("owned scalar graph");
        let [state] = graph.states.as_slice() else {
            panic!("one authored state")
        };
        assert_eq!(state.scalar_parameters[0].source_position, scalar_position);
        assert_eq!(state.parameter_types, [PrimitiveType::U64]);
        assert_eq!(state.structural_parameters.len(), 1);
        let parameter = &state.structural_parameters[0];
        assert_eq!(parameter.position, owned_position);
        assert_eq!(parameter.access, CheckedStructuralAccess::Owned);
        assert_eq!(parameter.multiplicity, Multiplicity::Affine);
        assert!(parameter.qualifications.is_empty());
        assert!(parameter.fused_service_erasure.is_none());
        assert_eq!(state.primitive_locals.len(), local_count);
        assert!(
            checked
                .facts
                .flow
                .terminal_structural_scalar_returns
                .for_machine(machine.symbol)
                .is_none()
        );
    }
    let enter = machine(&checked.typed, "enter");
    let state = &checked.machine_states(enter)[0];
    assert_eq!(
        checked
            .statement_table
            .statements(state.statement_nodes)
            .len(),
        2
    );
    let plans = &checked.facts.values.scalar_computations;
    let root = plans
        .root_at(
            state.symbol,
            0,
            CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 },
        )
        .expect("owned forwarding computation");
    let checked_trees::CheckedScalarComputationKind::Call {
        arguments,
        structural_arguments,
        ..
    } = plans.nodes.get(root.root).kind
    else {
        panic!("retained call")
    };
    assert_eq!(arguments.count(), 1);
    let [argument] = plans
        .structural_arguments
        .span(structural_arguments)
        .unwrap()
    else {
        panic!("one owned argument")
    };
    assert_eq!(
        argument.as_place().unwrap().source,
        CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 0 }
    );
    assert_eq!(
        argument.as_place().unwrap().access,
        CheckedStructuralAccess::Owned
    );
    assert!(argument.as_place().unwrap().path.is_empty());
}

#[test]
fn owned_and_borrowed_signatures_preserve_positions_and_return_owners() {
    let checked = checked(&format!(
        "{SOURCE}\n\
        machine read(limits: Limits) -> u64 {{ limits.limit }}\n\
        machine peek(value: &u64, answer: u64) -> u64 {{ answer }}\n\
        machine mixed(before: u64, limits: Limits, slot: &mut u64, after: u64) -> u64 {{\
            let observed: u64 = inspect(before, limits);\
            let reset_result: u64 = reset(&mut slot); observed }}"
    ));
    let mixed = machine(&checked.typed, "mixed");
    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(mixed.symbol)
        .unwrap();
    let state = &graph.states[0];
    assert_eq!(
        state
            .scalar_parameters
            .iter()
            .map(|parameter| parameter.source_position)
            .collect::<Vec<_>>(),
        [0, 3]
    );
    assert_eq!(
        state
            .structural_parameters
            .iter()
            .map(|parameter| (parameter.position, parameter.access))
            .collect::<Vec<_>>(),
        [
            (1, CheckedStructuralAccess::Owned),
            (2, CheckedStructuralAccess::MutableBorrow)
        ]
    );
    let read = machine(&checked.typed, "read").symbol;
    assert!(
        checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(read)
            .is_some()
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(read)
            .is_none()
    );
    let peek = machine(&checked.typed, "peek").symbol;
    assert!(
        checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(peek)
            .is_none()
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .for_machine(peek)
            .is_some()
    );
}

#[test]
fn owned_computation_rejects_synchronized_access_roster_tampering() {
    let checked = checked(SOURCE);
    let enter = machine(&checked.typed, "enter");
    let state = &checked.machine_states(enter)[0];
    let plans = &checked.facts.values.scalar_computations;
    let root = plans
        .root_at(
            state.symbol,
            0,
            CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 },
        )
        .unwrap();
    let checked_trees::CheckedScalarComputationKind::Call { source_call, .. } =
        plans.nodes.get(root.root).kind
    else {
        panic!("call")
    };
    let original = checked.facts.flow.control.calls.get(source_call);
    let borrow_call = checked
        .facts
        .borrow
        .calls
        .iter()
        .find(|(_, call)| {
            call.target_symbol == original.target_symbol
                && call.statement_index == original.statement_index
                && call.call_ordinal == original.call_ordinal
        })
        .unwrap()
        .0;
    for mutation in 0..4 {
        let mut flow = checked.facts.flow.clone();
        let mut borrow = checked.facts.borrow.clone();
        let mut rows = borrow
            .argument_accesses
            .span(original.accesses)
            .unwrap()
            .to_vec();
        assert_eq!(rows.len(), 2);
        match mutation {
            0 => {
                rows.pop();
            }
            1 => rows.reverse(),
            2 => rows[1].root_symbol = rows[0].root_symbol,
            _ => rows[1].kind = checked_trees::BorrowAccessKind::Mutable,
        }
        let accesses = borrow.argument_accesses.insert_many(rows);
        borrow.calls.get_mut(borrow_call).accesses = accesses;
        flow.control.calls.get_mut(source_call).accesses = accesses;
        let rebuilt = crate::values::build_checked_scalar_computation_plans(
            &checked.typed,
            &checked.facts.operators,
            &flow,
            &borrow,
            &checked.facts.proof,
            &checked.facts.values.scalar_expressions,
            &[],
        );
        assert!(
            rebuilt
                .root_at(
                    state.symbol,
                    0,
                    CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 }
                )
                .is_none(),
            "access mutation {mutation}"
        );
    }
}

#[test]
fn owned_actual_rejoins_authored_position_name_type_and_immutable_source() {
    let checked = checked(&SOURCE.replace(
        "enter(limits: Limits, marker: u64)",
        "enter(limits: Limits, marker: u64, other: Limits)",
    ));
    let enter = machine(&checked.typed, "enter");
    let state = &checked.machine_states(enter)[0];
    let inspect = machine(&checked.typed, "inspect");
    let target = &checked.state_parameters(&checked.machine_states(inspect)[0])[1];
    let call = checked
        .facts
        .flow
        .control
        .calls
        .iter()
        .map(|(_, call)| call)
        .find(|call| call.target_symbol == checked.machine_states(inspect)[0].symbol)
        .unwrap();
    let ExpressionNode::Call(authored) = checked
        .expression_table
        .expression(call.authored_expression)
    else {
        panic!("authored call")
    };
    let arguments = checked
        .expression_table
        .expression_handles(authored.arguments);
    assert!(
        structural_computation_argument(
            &checked.typed,
            &checked.facts.borrow,
            enter.symbol,
            state,
            call,
            arguments[1],
            target
        )
        .is_some()
    );
    assert!(
        structural_computation_argument(
            &checked.typed,
            &checked.facts.borrow,
            enter.symbol,
            state,
            call,
            arguments[0],
            target
        )
        .is_none()
    );
    for mutation in 0..5 {
        let mut program = checked.typed.clone();
        let source_symbol = checked.state_parameters(state)[0].symbol;
        let source_handle = program
            .state_parameters
            .iter()
            .find(|(_, parameter)| parameter.symbol == source_symbol)
            .unwrap()
            .0;
        match mutation {
            0 => program.state_parameters.get_mut(source_handle).is_mutable = true,
            1 => {
                program
                    .state_parameters
                    .get_mut(source_handle)
                    .type_reference = checked.state_parameters(state)[1].type_reference
            }
            2 => {
                let ExpressionNode::Name(name) =
                    program.expression_table.expression_mut(arguments[1])
                else {
                    panic!("owned name")
                };
                name.head_symbol = SymbolHandle::invalid();
            }
            3 | 4 => {
                let ExpressionNode::Name(name) =
                    program.expression_table.expression_mut(arguments[1])
                else {
                    panic!("owned name")
                };
                name.symbol =
                    checked.state_parameters(state)[if mutation == 3 { 1 } else { 2 }].symbol;
                name.head_symbol = name.symbol;
            }
            _ => unreachable!(),
        }
        assert!(
            structural_computation_argument(
                &program,
                &checked.facts.borrow,
                enter.symbol,
                state,
                call,
                arguments[1],
                target
            )
            .is_none(),
            "source mutation {mutation}"
        );
    }
}

#[test]
fn owned_graph_signature_admits_affine_and_copy_but_rejects_linear_mutable_and_qualified_inputs() {
    let checked = checked(SOURCE);
    let inspect = machine(&checked.typed, "inspect");
    let state = &checked.machine_states(inspect)[0];
    assert!(structural_scalar_graph_signature(&checked.typed, state).is_some());
    for multiplicity in [
        Multiplicity::Affine,
        Multiplicity::Unrestricted,
        Multiplicity::Linear,
    ] {
        let mut program = checked.typed.clone();
        let definition = program
            .data_definitions
            .iter()
            .find(|(_, definition)| definition.name.as_str() == "Limits")
            .unwrap()
            .0;
        program
            .data_definitions
            .get_mut(definition)
            .properties
            .multiplicity = multiplicity;
        let admitted = multiplicity != Multiplicity::Linear;
        assert_eq!(
            structural_scalar_graph_signature(&program, state).is_some(),
            admitted
        );
        assert_eq!(
            owned_parameter_argument(
                &program,
                state,
                checked.state_parameters(state)[1].symbol,
                &checked.state_parameters(state)[1]
            )
            .is_some(),
            admitted
        );
    }
    let mut program = checked.typed.clone();
    let symbol = checked.state_parameters(state)[1].symbol;
    let parameter = program
        .state_parameters
        .iter()
        .find(|(_, parameter)| parameter.symbol == symbol)
        .unwrap()
        .0;
    program.state_parameters.get_mut(parameter).is_mutable = true;
    assert!(structural_scalar_graph_signature(&program, state).is_none());
    let mut program = checked.typed.clone();
    let reference = program
        .type_reference_table
        .insert(TypeReferenceNode::Constrained {
            base_type: checked.state_parameters(state)[1].type_reference,
            constraints: arena::HandleSpan::empty(),
        });
    program.state_parameters.get_mut(parameter).type_reference = reference;
    assert!(
        structural_scalar_graph_signature(&program, state).is_none(),
        "even empty qualification wrappers cannot erase into owned admission"
    );
    assert!(
        owned_parameter_argument(&program, state, symbol, &checked.state_parameters(state)[1])
            .is_none()
    );
}

#[test]
fn affine_discard_permissions_do_not_authorize_nominal_cleanup_erasure() {
    for source in [
        format!("{SOURCE}\nmachine Limits::drop(&mut self) {{}}"),
        format!(
            "data Payload {{ value: u64; }}\n{}\nmachine Payload::drop(&mut self) {{}}",
            SOURCE.replace("data Limits {", "data Limits { payload: Payload;")
        ),
    ] {
        let checked = checked(&source);
        let inspect = machine(&checked.typed, "inspect");
        let state = &checked.machine_states(inspect)[0];
        let parameter = &checked.state_parameters(state)[1];
        assert_eq!(
            crate::checks::type_multiplicity(&checked, parameter.type_reference),
            Multiplicity::Affine
        );
        assert!(
            !validation::has_plain_owned_contents_with_numeric_constraints(
                &checked,
                parameter.type_reference
            )
        );
        assert!(
            checked
                .facts
                .flow
                .ownership
                .permissions
                .iter()
                .any(|(_, event)| {
                    event.machine_symbol == inspect.symbol
                        && event.state_symbol == state.symbol
                        && event.source == PermissionEventSource::StateExit
                        && event.kind == PermissionEventKind::AffineDrop
                        && event.root == facts::PlaceRoot::Symbol(parameter.symbol)
                        && event.access == PermissionAccess::Owned
                        && event.claim_identity == PermissionClaimIdentity::Unknown
                        && !event.obligation_live
                }),
            "ordinary affine discard permission also exists for nominal cleanup"
        );
        assert!(structural_scalar_graph_signature(&checked, state).is_none());
        assert!(owned_parameter_argument(&checked, state, parameter.symbol, parameter).is_none());
        assert!(
            checked
                .facts
                .flow
                .terminal_scalar_graphs
                .for_machine(inspect.symbol)
                .is_none()
        );
    }
}

#[test]
fn owned_graph_receiving_check_rejects_retained_signature_tampering() {
    let checked = checked(SOURCE);
    let inspect = machine(&checked.typed, "inspect");
    let state = checked.machine_states(inspect)[0].symbol;
    assert!(
        registered_structural_graph_target(
            &checked.typed,
            &checked.facts,
            inspect.symbol,
            state,
            PrimitiveType::U64
        )
        .is_some()
    );
    for mutation in 0..7 {
        let mut facts = checked.facts.clone();
        let graph = facts
            .flow
            .terminal_scalar_graphs
            .machines
            .iter_mut()
            .find(|graph| graph.machine == inspect.symbol)
            .unwrap();
        let retained = &mut graph.states[0];
        match mutation {
            0 => retained.structural_parameters[0].position = 0,
            1 => retained.structural_parameters[0].access = CheckedStructuralAccess::SharedBorrow,
            2 => retained.structural_parameters[0].multiplicity = Multiplicity::Unrestricted,
            3 => retained.structural_parameters[0].type_identity = "different".to_owned(),
            4 => retained.scalar_parameters[0].source_position = 1,
            5 => retained.structural_parameters[0].is_self = true,
            _ => facts.flow.terminal_scalar_graphs.structural_types.clear(),
        }
        assert!(
            registered_structural_graph_target(
                &checked.typed,
                &facts,
                inspect.symbol,
                state,
                PrimitiveType::U64
            )
            .is_none(),
            "signature mutation {mutation}"
        );
    }
}
