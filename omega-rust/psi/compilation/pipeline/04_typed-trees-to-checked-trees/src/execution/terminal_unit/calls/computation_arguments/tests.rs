use super::super::super::scalar_targets::registered_structural_graph_target;
use super::super::super::structural_scalar_graph_signature;
use super::{
    CheckedStructuralAccess, ExpressionNode, Multiplicity, PrimitiveType, SymbolHandle,
    TypeReferenceNode, TypedTrees,
};
use crate::execution::terminal_unit::CheckedScalarExpressionRole;
use crate::execution::terminal_unit::PermissionAccess;
use crate::execution::terminal_unit::PermissionClaimIdentity;
use crate::execution::terminal_unit::PermissionEventKind;
use crate::execution::terminal_unit::PermissionEventSource;
use crate::execution::terminal_unit::calls::computation_arguments::owned_parameter_argument;
use crate::execution::terminal_unit::structural_computation_argument;

use crate::tests::front_end::checked_program_result;

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

#[test]
fn qualified_byte_field_calls_retain_their_short_circuit_return_owner() {
    let checked = checked(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../tests/omega/pass/domains/mutable_receiver_declared_field_exit/main.omg"
    )));
    for name in ["replace", "Packet::replace"] {
        let owner = machine(&checked.typed, name);
        let state = &checked.machine_states(owner)[0];
        let plans = &checked.facts.values.scalar_computations;
        let root = plans
            .root_at(state.symbol, 2, CheckedScalarExpressionRole::Return)
            .expect("qualified field call belongs to the scalar return computation");
        let checked_trees::CheckedScalarComputationKind::Select { when_true, .. } =
            plans.nodes.get(root.root).kind
        else {
            panic!("the return retains its short-circuit selection");
        };
        let checked_trees::CheckedScalarComputationKind::Call {
            source_call,
            structural_arguments,
            ..
        } = plans.nodes.get(when_true).kind
        else {
            panic!("the selected branch owns observe");
        };
        let [checked_trees::CheckedScalarComputationStructuralArgument::Place(argument)] = plans
            .structural_arguments
            .span(structural_arguments)
            .unwrap()
        else {
            panic!("one projected structural operand");
        };
        assert_eq!(argument.access, CheckedStructuralAccess::SharedBorrow);
        assert!(matches!(
            argument.source,
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: 0
            }
        ));
        assert_eq!(argument.path.len(), 2);
        assert!(argument.path.iter().all(|segment| matches!(
            segment,
            checked_trees::CheckedUnitStructuralPathSegment::Field(_)
        )));
        let call = checked.facts.flow.control.calls.get(source_call);
        assert_eq!(call.statement_index, 2);
        assert_eq!(call.call_ordinal, 0);
        let target = machine(&checked.typed, "observe");
        let parameter = &checked.state_parameters(&checked.machine_states(target)[0])[0];
        assert_eq!(
            argument.type_identity,
            super::super::super::types::byte_sequence_type_identity(
                &checked.typed,
                parameter.type_reference,
                &[],
                &[],
            )
            .unwrap()
        );
    }
}

#[test]
fn qualified_byte_owner_arguments_share_parameter_and_local_borrow_admission() {
    let checked = checked(
        r#"
        domain [u8; 8]::NonEmpty(bytes) requires non_empty(bytes);
        machine observe(bytes: &[u8; 8] in NonEmpty) -> bool { bytes.len > 0 }
        machine forwarded(flag: bool, bytes: &[u8; 8] in NonEmpty) -> bool {
            flag && observe(bytes)
        }
        machine local(flag: bool) -> bool {
            let bytes: [u8; 8] in NonEmpty = "ready";
            flag && observe(&bytes)
        }
        "#,
    );
    for (name, statement) in [("forwarded", 0), ("local", 1)] {
        let owner = machine(&checked.typed, name);
        let state = &checked.machine_states(owner)[0];
        let plans = &checked.facts.values.scalar_computations;
        let root = plans
            .root_at(state.symbol, statement, CheckedScalarExpressionRole::Return)
            .expect("whole qualified byte owner keeps its computation");
        let checked_trees::CheckedScalarComputationKind::Select { when_true, .. } =
            plans.nodes.get(root.root).kind
        else {
            panic!("short-circuit return");
        };
        let checked_trees::CheckedScalarComputationKind::Call {
            structural_arguments,
            ..
        } = plans.nodes.get(when_true).kind
        else {
            panic!("selected observe call");
        };
        let [checked_trees::CheckedScalarComputationStructuralArgument::Place(argument)] = plans
            .structural_arguments
            .span(structural_arguments)
            .unwrap()
        else {
            panic!("one whole storage loan");
        };
        assert!(argument.path.is_empty());
        assert_eq!(argument.access, CheckedStructuralAccess::SharedBorrow);
    }
}

#[test]
fn mutable_copy_operands_observe_storage_without_reading_static_qualifiers() {
    let checked = checked(
        r#"
        data Settings [copy] { count: i32; }
        machine Settings::stamp(settings: &mut Settings, count: i32) -> i32 {
            settings.count = count; count
        }
        machine Settings::observe(settings: Settings, later: i32) -> i32 { settings.count }
        machine enter() -> i32 {
            let mut settings: Settings = Settings { count: 17 };
            let before: i32 = Settings::observe(settings, Settings::stamp(&mut settings, 25));
            before
        }
    "#,
    );
    let owner = machine(&checked.typed, "enter");
    let state = &checked.machine_states(owner)[0];
    let plans = &checked.facts.values.scalar_computations;
    let root = plans
        .root_at(
            state.symbol,
            1,
            CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 },
        )
        .expect("copy and nested mutation retain their ordinary computation");
    let checked_trees::CheckedScalarComputationKind::Call {
        source_call,
        structural_arguments,
        ..
    } = plans.nodes.get(root.root).kind
    else {
        panic!("outer call");
    };
    let [checked_trees::CheckedScalarComputationStructuralArgument::Place(argument)] = plans
        .structural_arguments
        .span(structural_arguments)
        .unwrap()
    else {
        panic!("one owned copy");
    };
    let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } =
        argument.source
    else {
        panic!("original local storage");
    };
    assert_eq!(argument.access, CheckedStructuralAccess::Owned);
    let call = checked.facts.flow.control.calls.get(source_call);
    let observations = checked
        .facts
        .borrow
        .argument_accesses
        .span(call.accesses)
        .unwrap();
    assert_eq!(
        observations.len(),
        2,
        "the copy and nested actual each observe storage: {observations:?}"
    );
    assert!(
        observations
            .iter()
            .all(|access| access.root_symbol == symbol)
    );
    // The copy ends at once; the nested `&mut` is a mutable loan that ends
    // when `stamp` returns its scalar, not a read of the same local.
    assert_eq!(
        observations
            .iter()
            .map(|access| (access.kind.clone(), access.end_before_access))
            .collect::<Vec<_>>(),
        [
            (checked_trees::BorrowAccessKind::Read, Some(1)),
            (checked_trees::BorrowAccessKind::Mutable, Some(2)),
        ]
    );
}

#[test]
fn projected_const_record_arguments_retain_each_closed_call_in_a_sum() {
    for (parameters, expression) in [
        ("pair: &Pair", "read(&pair.left) + read(&pair.right)"),
        (
            "left: &Buffer<2>, right: &Buffer<4>",
            "read(left) + read(right)",
        ),
    ] {
        let source = format!(
            r#"
        data Buffer<const Capacity: u64> where Capacity >= 1 {{
            items: [u32 in Wrapping; Capacity];
        }}
        data Pair {{ left: Buffer<2>; right: Buffer<4>; }}
        machine read<const Length: u64>(buffer: &Buffer<Length>) -> u32 in Wrapping
        requires Length >= 1;
        {{ buffer.items[0] }}
        machine sum({parameters}) -> u32 in Wrapping {{
            let total: u32 in Wrapping = {expression};
            total
        }}
        "#,
        );
        // The product normalizes closed data applications before checking.
        // Retain that phase here so the borrow can consume exact instances.
        let typed = crate::tests::front_end::typed_program_with_generic_data_result(&source)
            .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
        let checked = crate::lower_typed_trees(typed, &crate::CheckingRequest::settled())
            .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
        let owner = machine(&checked.typed, "sum");
        let state = &checked.machine_states(owner)[0];
        let plans = &checked.facts.values.scalar_computations;
        let root = plans.root_at(
            state.symbol,
            0,
            CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 },
        );
        assert!(
            root.is_some(),
            "{expression} retains its two borrowed call operands"
        );
        let calls = plans
            .nodes
            .iter()
            .filter_map(|(_, node)| {
                if let checked_trees::CheckedScalarComputationKind::Call {
                    target_state,
                    structural_arguments,
                    ..
                } = node.kind
                {
                    Some((target_state, structural_arguments))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 2);
        assert_ne!(
            calls[0].0, calls[1].0,
            "capacities retain distinct specializations"
        );
        for (_, arguments) in calls {
            assert_eq!(plans.structural_arguments.span_or_empty(arguments).len(), 1);
        }
    }
}

fn checked(source: &str) -> checked_trees::CheckedTrees {
    checked_program_result(source).unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"))
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
    for mutation in 0..6 {
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
            3 => rows[1].kind = checked_trees::BorrowAccessKind::Mutable,
            4 => rows[1].end_before_access = Some(0),
            _ => rows[1].end_before_access = Some(usize::MAX),
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
            Some(crate::execution::terminal_unit::ScalarCalleePlans {
                boundary_returns: &checked.facts.flow.terminal_boundary_scalar_returns,
                structural_returns: &checked.facts.flow.terminal_structural_scalar_returns
            }),
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
                Some(crate::execution::terminal_unit::ScalarCalleePlans {
                    boundary_returns: &facts.flow.terminal_boundary_scalar_returns,
                    structural_returns: &facts.flow.terminal_structural_scalar_returns
                }),
                inspect.symbol,
                state,
                PrimitiveType::U64
            )
            .is_none(),
            "signature mutation {mutation}"
        );
    }
}

#[test]
fn mutable_receiver_computation_retains_its_projected_loan_and_source_access() {
    let checked = checked(
        r#"
        data Counter { count: i32 in Wrapping; }
        data Main { base: i32 in Wrapping; counter: Counter; }
        machine Counter::double(&mut self, value: i32 in Wrapping) -> i32 in Wrapping {
            self.count = self.count + 1;
            value * 2
        }
        machine Main::run(&mut self) {
            self.base = 10;
            let result: i32 in Wrapping = self.base + self.counter.double(6) * 3;
            transition result == 46 { true -> done() false -> done() }
            state done(&mut self) { }
        }
        "#,
    );
    let caller = machine(&checked.typed, "Main::run");
    let state = &checked.machine_states(caller)[0];
    let callee = machine(&checked.typed, "Counter::double");
    let target = &checked.state_parameters(&checked.machine_states(callee)[0])[0];
    let plans = &checked.facts.values.scalar_computations;
    assert!(
        plans
            .root_at(
                state.symbol,
                1,
                CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 },
            )
            .is_some(),
        "the call remains an operand of the authored arithmetic initializer",
    );
    let (source_call, structural_arguments) = plans
        .nodes
        .iter()
        .find_map(|(_, node)| {
            let checked_trees::CheckedScalarComputationKind::Call {
                target_machine,
                source_call,
                structural_arguments,
                ..
            } = node.kind
            else {
                return None;
            };
            (target_machine == callee.symbol).then_some((source_call, structural_arguments))
        })
        .expect("mutable receiver call");
    let arguments = plans
        .structural_arguments
        .span(structural_arguments)
        .unwrap();
    let [checked_trees::CheckedScalarComputationStructuralArgument::Place(argument)] = arguments
    else {
        panic!("one exact receiver loan");
    };
    assert_eq!(argument.access, CheckedStructuralAccess::MutableBorrow);
    assert!(matches!(
        argument.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 0 }
    ));
    assert_eq!(argument.path.len(), 1);
    let call = checked.facts.flow.control.calls.get(source_call);
    let ExpressionNode::Call(authored) = checked
        .expression_table
        .expression(call.authored_expression)
    else {
        panic!("authored call");
    };
    let receiver = authored.receiver;
    let source_parameter = checked.state_parameters(state)[0].symbol;
    for access in [
        language_semantics::ReferenceAccess::Shared,
        language_semantics::ReferenceAccess::WriteOnly,
    ] {
        let mut program = checked.typed.clone();
        let handle = program
            .state_parameters
            .iter()
            .find(|(_, parameter)| parameter.symbol == source_parameter)
            .unwrap()
            .0;
        let mut reference = program
            .type_reference_table
            .type_reference(program.state_parameters.get(handle).type_reference)
            .clone();
        let TypeReferenceNode::Reference {
            access: retained, ..
        } = &mut reference
        else {
            panic!("borrowed source receiver");
        };
        *retained = access;
        let reference = program.type_reference_table.insert(reference);
        program.state_parameters.get_mut(handle).type_reference = reference;
        assert!(
            structural_computation_argument(
                &program,
                &checked.facts.borrow,
                caller.symbol,
                state,
                call,
                receiver,
                target,
            )
            .is_none(),
            "source {access:?} cannot grant a mutable readable loan"
        );
    }
}

#[test]
fn computed_receiver_alone_keeps_the_unit_callers_storage() {
    let checked = checked(
        r#"
        data Counter { count: u64; }
        data Main { counter: Counter; }
        machine Counter::replace(&mut self) -> u64 { self.count = 7; 7 }
        machine Main::run(&mut self) {
            let saved: u64 in Wrapping = self.counter.replace() * 10;
        }
    "#,
    );
    let caller = machine(&checked.typed, "Main::run");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(caller.symbol)
        .expect("unit caller retains its computed local");
    let [receiver] = plan.structural_parameters.as_slice() else {
        panic!("computed receiver use must retain one structural parameter");
    };
    assert!(receiver.is_self);
    assert_eq!(receiver.access, CheckedStructuralAccess::MutableBorrow);
}

#[test]
fn computed_guard_receiver_retains_nested_predicate_byte_fields() {
    let checked = checked(
        r#"
        domain [u8; 16]::Text requires valid_utf8(self);
        data Cell [copy] { case Empty; case Room; }
        data Label { text: [u8; 16] in Text; }
        data Navigation { destination: Cell; label: Label; }
        data Main { navigation: Navigation; }
        machine Navigation::permits(&mut self, destination: Cell) -> bool {
            transition { destination == Cell::Empty -> false _ -> true }
        }
        machine Main::run(&mut self) {
            transition { _ -> attempt() }
            state attempt(&mut self) {
                let destination: Cell = self.navigation.destination;
                transition self.navigation.permits(destination) {
                    true -> allowed()
                    false -> denied()
                }
            }
            state allowed(&mut self) { }
            state denied(&mut self) { }
        }
    "#,
    );
    let caller = machine(&checked.typed, "Main::run");
    let state = &checked.machine_states(caller)[1];
    let plans = &checked.facts.values.scalar_computations;
    let root = plans
        .root_at(state.symbol, 1, CheckedScalarExpressionRole::Guard)
        .expect("borrowing a byte-bearing receiver retains the computed guard");
    // A Boolean dispatch may wrap the call in its authored truth comparison.
    // Follow that guard's graph, rather than requiring a call at its root.
    let callee = machine(&checked.typed, "Navigation::permits");
    let mut pending = vec![root.root];
    let mut calls = Vec::new();
    while let Some(handle) = pending.pop() {
        use checked_trees::CheckedScalarComputationKind as Computation;
        match plans.nodes.get(handle).kind {
            Computation::Call {
                target_machine,
                structural_arguments,
                ..
            } if target_machine == callee.symbol => calls.push(structural_arguments),
            Computation::Apply { operands, .. } => {
                pending.extend(plans.operands.span(operands).unwrap().iter().copied())
            }
            Computation::Select {
                condition,
                when_true,
                when_false,
                ..
            } => pending.extend([condition, when_true, when_false]),
            Computation::SelectedComparison { left, right, .. } => pending.extend([left, right]),
            Computation::Value(_) => {}
            _ => panic!("unexpected computation in the Boolean receiver guard"),
        }
    }
    let [structural_arguments] = calls.as_slice() else {
        panic!("the selected guard evaluates its exact receiver call once");
    };
    let [
        checked_trees::CheckedScalarComputationStructuralArgument::Place(receiver),
        checked_trees::CheckedScalarComputationStructuralArgument::Place(destination),
    ] = plans
        .structural_arguments
        .span(*structural_arguments)
        .unwrap()
    else {
        panic!("the receiver loan and copied destination keep separate operands");
    };
    assert_eq!(receiver.access, CheckedStructuralAccess::MutableBorrow);
    assert_eq!(
        receiver.path,
        vec![checked_trees::CheckedUnitStructuralPathSegment::Field(
            "navigation".to_owned()
        )]
    );
    assert!(matches!(
        receiver.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 0 }
    ));
    assert_eq!(destination.access, CheckedStructuralAccess::Owned);
    assert!(matches!(
        destination.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. }
    ));
}

/// A borrowed parameter forwarded into a scalar computation names its slot in
/// the structural roster. The roster strips `[erased]` formals, so an erased
/// proof formal ahead of `values` must not shift the forwarded index, and
/// one after it changes nothing.
#[test]
fn erased_formals_occupy_no_structural_slot_in_forwarded_computation_arguments() {
    let checked = checked(
        r#"
        data Proof { tag: i32; }
        data Inner { a: u8; b: u8; }
        data Main { inner: Inner; result: u8; }
        machine first(values: &Inner) -> u8 { values.a }
        machine Main::relay(&mut self, witness [erased]: Proof, values: &Inner, trailing [erased]: Proof) {
            self.result = first(values);
        }
        "#,
    );
    let relay = machine(&checked.typed, "Main::relay");
    let callee = machine(&checked.typed, "first");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(relay.symbol)
        .expect("the relay keeps its unit plan");
    let roster = plan
        .structural_parameters
        .iter()
        .map(|parameter| parameter.position)
        .collect::<Vec<_>>();
    assert_eq!(roster, [0, 2], "the roster keeps `self` and `values` only");
    let plans = &checked.facts.values.scalar_computations;
    let structural_arguments = plans
        .nodes
        .iter()
        .find_map(|(_, node)| {
            let checked_trees::CheckedScalarComputationKind::Call {
                target_machine,
                structural_arguments,
                ..
            } = node.kind
            else {
                return None;
            };
            (target_machine == callee.symbol).then_some(structural_arguments)
        })
        .expect("the forwarded computation call");
    let [checked_trees::CheckedScalarComputationStructuralArgument::Place(argument)] = plans
        .structural_arguments
        .span(structural_arguments)
        .unwrap()
    else {
        panic!("one forwarded loan");
    };
    assert!(
        matches!(
            argument.source,
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: 1
            }
        ),
        "`values` is roster slot 1, not a slot counted past `witness`: {:?}",
        argument.source
    );
}

/// A context record that holds a `Binding<R>` lends itself to a scalar call
/// nested in a constructor operand exactly as one without it. The binding
/// field's carrier is collected as its own payload-free `FusedServiceBinding`
/// member type, never as an empty record; that member must not decline the
/// loan of the enclosing record, or the ordinary effect body loses the store.
#[test]
fn binding_holding_context_lends_itself_to_a_nested_scalar_call() {
    let mut typed = crate::tests::front_end::typed_program_with_core_service(
        r#"
pub boundary trait Host { machine mark(value: i32); }
pub data Packet { samples: [i32; 2]; stamp: i32; }
pub data Context { host: Binding<Host>; packet: Packet; samples: [i32; 2]; }
machine stamp(context: &mut Context) -> i32 { context.samples[0] = 90; 7 }
machine snapshot(context: &mut Context) reaches Host {
    context.packet = Packet { samples: context.samples, stamp: stamp(context) };
    context.host.mark(1);
}
"#,
    );
    crate::tests::bind_fixture_fused_service_erasures(&mut typed);
    let checked = crate::lower_typed_trees(typed, &crate::CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
    let snapshot = machine(&checked.typed, "snapshot");
    let callee = machine(&checked.typed, "stamp");
    let effects = &checked.facts.flow.terminal_unit_effects;
    effects.for_machine(snapshot.symbol).unwrap_or_else(|| {
        panic!(
            "the snapshot store keeps its ordinary effect body: {:?}",
            effects.omission_for_machine(snapshot.symbol)
        )
    });
    let plans = &checked.facts.values.scalar_computations;
    let structural_arguments = plans
        .nodes
        .iter()
        .find_map(|(_, node)| {
            let checked_trees::CheckedScalarComputationKind::Call {
                target_machine,
                structural_arguments,
                ..
            } = node.kind
            else {
                return None;
            };
            (target_machine == callee.symbol).then_some(structural_arguments)
        })
        .expect("the constructor operand keeps its scalar call");
    let [checked_trees::CheckedScalarComputationStructuralArgument::Place(argument)] = plans
        .structural_arguments
        .span(structural_arguments)
        .unwrap()
    else {
        panic!("one forwarded context loan");
    };
    assert_eq!(argument.access, CheckedStructuralAccess::MutableBorrow);
    assert!(argument.path.is_empty(), "the whole context is lent");
    assert!(matches!(
        argument.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 0 }
    ));
    // The binding stays a nonconstructible carrier beside the context record.
    let shape = |identity: &str| {
        effects
            .structural_types
            .iter()
            .find(|plan| plan.identity == identity)
            .map(|plan| &plan.shape)
    };
    let Some(checked_trees::CheckedUnitStructuralTypeShape::Record { fields }) =
        shape(&argument.type_identity)
    else {
        panic!("the lent context is a record");
    };
    let binding = fields
        .iter()
        .find_map(|field| match &field.field_type {
            checked_trees::CheckedUnitStructuralFieldType::FusedServiceBacked {
                provider_type_identity,
                ..
            } => Some(provider_type_identity.as_str()),
            _ => None,
        })
        .expect("the host field keeps its fused service receipt");
    assert_eq!(
        shape(binding),
        Some(&checked_trees::CheckedUnitStructuralTypeShape::FusedServiceBinding)
    );
}

const CASE_BORROW: &str = r#"
data Payload { case Leaf(value: u64); case Empty; }
machine Payload::read(payload: &Payload) -> u64 {
    transition payload { Payload::Leaf { value } -> (value) Payload::Empty -> (0) }
}
data Sum { case Found(p: Payload); case Other(p: Payload); case Missing; }
machine Sum::probe(sum: &Sum) -> u64 {
    transition sum {
        Sum::Found { p } -> (Payload::read(&p))
        Sum::Other { p } -> (Payload::read(&p))
        Sum::Missing -> (0)
    }
}
"#;

#[test]
fn case_payload_arguments_retain_exact_case_and_original_parameter() {
    let checked = checked(CASE_BORROW);
    let caller = machine(&checked.typed, "Sum::probe");
    let callee = machine(&checked.typed, "Payload::read");
    let state = &checked.machine_states(caller)[0];
    let owner = checked
        .data_definitions()
        .iter()
        .find(|owner| owner.name.as_str() == "Sum")
        .unwrap();
    let plans = &checked.facts.values.scalar_computations;
    let mut paths = Vec::new();
    for (_, node) in plans.nodes.iter() {
        let checked_trees::CheckedScalarComputationKind::Call {
            target_machine,
            source_call,
            structural_arguments,
            ..
        } = node.kind
        else {
            continue;
        };
        if target_machine != callee.symbol {
            continue;
        }
        let [checked_trees::CheckedScalarComputationStructuralArgument::Place(argument)] = plans
            .structural_arguments
            .span(structural_arguments)
            .unwrap()
        else {
            panic!("one payload loan");
        };
        assert_eq!(argument.access, CheckedStructuralAccess::SharedBorrow);
        assert!(matches!(
            argument.source,
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: 0
            }
        ));
        let [
            checked_trees::CheckedUnitStructuralPathSegment::Case(case),
            checked_trees::CheckedUnitStructuralPathSegment::Field(field),
        ] = argument.path.as_slice()
        else {
            panic!("complete case payload path: {:?}", argument.path);
        };
        let call = checked.facts.flow.control.calls.get(source_call);
        let accesses = checked
            .facts
            .borrow
            .argument_accesses
            .span(call.accesses)
            .unwrap();
        let [access] = accesses else {
            panic!("one original payload access");
        };
        assert_eq!(
            access.root_symbol,
            checked.state_parameters(state)[0].symbol
        );
        let [
            facts::PlaceSegment::Case { variant },
            facts::PlaceSegment::Field { symbol },
        ] = checked.facts.borrow.access_segments(access)
        else {
            panic!("captured access retains the complete selected payload");
        };
        let declaration = checked
            .data_members(owner)
            .iter()
            .find_map(|member| match member {
                checked_trees::data::DataMember::Variant(declaration)
                    if declaration.path_identity() == *case =>
                {
                    Some(declaration)
                }
                _ => None,
            })
            .expect("selected case belongs to the original root");
        let payload = checked
            .data_payload_fields(declaration)
            .iter()
            .find(|payload| payload.path_identity() == *field)
            .expect("selected field belongs to the exact case");
        assert_eq!(*variant, declaration.symbol);
        assert_eq!(*symbol, payload.symbol);
        paths.push((case.clone(), field.clone(), *variant, *symbol));
    }
    paths.sort_by(|left, right| left.0.cmp(&right.0));
    assert_eq!(paths.len(), 2, "both arms retain ordinary calls");
    assert_eq!(paths[0].0, "Found");
    assert_eq!(paths[1].0, "Other");
    assert_eq!(paths[0].1, "p");
    assert_eq!(paths[1].1, "p");
    assert_ne!(
        paths[0].2, paths[1].2,
        "equal payload types keep distinct cases"
    );
    assert_ne!(
        paths[0].3, paths[1].3,
        "equal field spellings keep distinct declarations"
    );
}

#[test]
fn case_argument_path_rejects_sibling_case_field_substitution() {
    let checked = checked(CASE_BORROW);
    let caller = machine(&checked.typed, "Sum::probe");
    let state = &checked.machine_states(caller)[0];
    let owner = checked
        .data_definitions()
        .iter()
        .find(|owner| owner.name.as_str() == "Sum")
        .unwrap();
    let cases = checked
        .data_members(owner)
        .iter()
        .filter_map(|member| match member {
            checked_trees::data::DataMember::Variant(case)
                if case.name.as_str() == "Found" || case.name.as_str() == "Other" =>
            {
                Some(case)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let root = facts::PlaceRoot::Symbol(checked.state_parameters(state)[0].symbol);
    let mut place = crate::flow::CanonicalPlace {
        root,
        segments: vec![
            facts::PlaceSegment::Case {
                variant: cases[0].symbol,
            },
            facts::PlaceSegment::Field {
                symbol: checked.data_payload_fields(cases[0])[0].symbol,
            },
        ],
    };
    let resolve = |place: &crate::flow::CanonicalPlace| {
        super::super::argument_paths::projected_borrowed_path(
            &checked.typed,
            caller.symbol,
            state.symbol,
            0,
            place,
        )
    };
    assert!(resolve(&place).is_some());
    place.segments[0] = facts::PlaceSegment::Case {
        variant: cases[1].symbol,
    };
    assert!(
        resolve(&place).is_none(),
        "a sibling case cannot authorize the same-typed field"
    );
    place.segments.pop();
    assert!(
        resolve(&place).is_none(),
        "a case alone is not a borrowed value"
    );
}

#[test]
fn mutable_range_scalar_initializer_retains_one_call_and_exact_borrow_evidence() {
    let checked = checked(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../tests/omega/pass/slices/mutable_field_range_scalar_initializer_exit/main.omg"
    )));
    let owner = machine(&checked.typed, "main");
    let state = &checked.machine_states(owner)[0];
    let target = checked.machine_states(machine(&checked.typed, "stamp"))[0].symbol;
    let plans = &checked.facts.values.scalar_computations;
    let role = CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 };
    let site = checked_trees::CheckedSubsliceSite::CallArgument {
        call_ordinal: 0,
        argument_ordinal: 0,
    };
    for role in [
        CheckedScalarExpressionRole::SubsliceStart { site },
        CheckedScalarExpressionRole::SubsliceEnd { site },
    ] {
        assert!(
            checked
                .facts
                .values
                .scalar_expressions
                .bound_expression_at(state.symbol, 1, role)
                .is_some(),
            "the nested scalar call retains its exact range endpoint"
        );
    }
    assert!(plans.root_at(state.symbol, 1, role).is_some());
    let calls = plans
        .nodes
        .iter()
        .filter_map(|(_, node)| {
            if let checked_trees::CheckedScalarComputationKind::Call {
                source_call,
                target_state,
                structural_arguments,
                ..
            } = node.kind
            {
                (target_state == target).then_some((source_call, structural_arguments))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let [(source_call, arguments)] = calls.as_slice() else {
        panic!("the scalar comparison must retain exactly one stamp invocation");
    };
    let [checked_trees::CheckedScalarComputationStructuralArgument::Place(argument)] =
        plans.structural_arguments.span_or_empty(*arguments)
    else {
        panic!("the exact range operand");
    };
    assert_eq!(argument.access, CheckedStructuralAccess::MutableBorrow);
    assert!(matches!(
        argument.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice { .. }
    ));

    let original = checked.facts.flow.control.calls.get(*source_call);
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
    // Synchronizing both rosters cannot turn a shared access fact into the
    // explicit mutable actual's authority. Reconstruct the whole initializer
    // so its call cannot silently survive after losing its borrow evidence.
    let mut flow = checked.facts.flow.clone();
    let mut borrow = checked.facts.borrow.clone();
    let mut accesses = borrow
        .argument_accesses
        .span(original.accesses)
        .unwrap()
        .to_vec();
    assert_eq!(accesses.len(), 1);
    assert_eq!(accesses[0].kind, checked_trees::BorrowAccessKind::Mutable);
    accesses[0].kind = checked_trees::BorrowAccessKind::Read;
    let accesses = borrow.argument_accesses.insert_many(accesses);
    borrow.calls.get_mut(borrow_call).accesses = accesses;
    flow.control.calls.get_mut(*source_call).accesses = accesses;
    let rebuilt = crate::values::build_checked_scalar_computation_plans(
        &checked.typed,
        &checked.facts.operators,
        &flow,
        &borrow,
        &checked.facts.proof,
        &checked.facts.values.scalar_expressions,
        &[],
    );
    assert!(rebuilt.root_at(state.symbol, 1, role).is_none());
}

#[test]
fn mutable_range_local_initializer_retains_the_established_view() {
    let checked = checked(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../tests/omega/pass/slices/mutable_field_range_local_initializer_exit/main.omg"
    )));
    let owner = machine(&checked.typed, "main");
    let state = &checked.machine_states(owner)[0];
    let target = checked.machine_states(machine(&checked.typed, "stamp"))[0].symbol;
    let plans = &checked.facts.values.scalar_computations;
    let role = CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 };
    assert!(
        plans.root_at(state.symbol, 2, role).is_some(),
        "local view forwarding: {:#?}",
        checked.facts.borrow
    );
    let calls = plans
        .nodes
        .iter()
        .filter_map(|(_, node)| {
            if let checked_trees::CheckedScalarComputationKind::Call {
                source_call,
                target_state,
                structural_arguments,
                ..
            } = node.kind
            {
                (target_state == target).then_some((source_call, structural_arguments))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let [(source_call, arguments)] = calls.as_slice() else {
        panic!("exactly one stamp call");
    };
    let [checked_trees::CheckedScalarComputationStructuralArgument::Place(argument)] =
        plans.structural_arguments.span_or_empty(*arguments)
    else {
        panic!("one established view operand");
    };
    assert_eq!(argument.access, CheckedStructuralAccess::MutableBorrow);
    let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } =
        argument.source
    else {
        panic!("retain the local descriptor");
    };
    let (ordinal, local) = checked
        .typed
        .state_local_declaration(state, symbol)
        .unwrap();
    assert_eq!(ordinal, 1);
    assert_eq!(local.name.as_str(), "window");

    let original = checked.facts.flow.control.calls.get(*source_call);
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
    let mut flow = checked.facts.flow.clone();
    let mut borrow = checked.facts.borrow.clone();
    let mut accesses = borrow
        .argument_accesses
        .span(original.accesses)
        .unwrap()
        .to_vec();
    assert_eq!(accesses.len(), 1);
    assert_eq!(accesses[0].kind, checked_trees::BorrowAccessKind::Mutable);
    accesses[0].kind = checked_trees::BorrowAccessKind::Read;
    let accesses = borrow.argument_accesses.insert_many(accesses);
    borrow.calls.get_mut(borrow_call).accesses = accesses;
    flow.control.calls.get_mut(*source_call).accesses = accesses;
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
        rebuilt.root_at(state.symbol, 2, role).is_none(),
        "matching forged rosters cannot authorize mutable local forwarding"
    );
}
