//! Operand helpers coexist with dynamic realizations and closed-sum payloads.
use super::{CheckedTrees, checked_source, lower_machine};
use lowered_psi::LoweredPsi;
use terminal_psi::{OperationKind, Terminator};
use typed_trees::{expression::ExpressionNode, statement::StatementNode};

mod dynamic_unit;

fn roundtrip(checked: &CheckedTrees) -> LoweredPsi {
    let lowered = lower_machine(checked, "Main::main").expect("computed leaves lower");
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).expect("encode module");
    let evidence =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .expect("encode proof");
    let module = terminal_codec::decode_module(&semantic).expect("decode module");
    let proof = terminal_codec::decode_proof_bundle(&evidence).expect("decode proof");
    assert_eq!(module, lowered.semantic_module);
    assert_eq!(proof, lowered.proof_bundle);
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("independent verification of decoded module and evidence");
    let mut identities = module
        .machines
        .iter()
        .map(|machine| machine.id)
        .collect::<Vec<_>>();
    identities.sort();
    identities.dedup();
    assert_eq!(
        identities.len(),
        module.machines.len(),
        "machine identities remain disjoint"
    );
    lowered
}

fn assert_trailing_provider_field_custody(checked: &CheckedTrees) {
    let program = &checked.typed;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .unwrap();
    let Some((receiver, self_expression, inherited, expression)) =
        program.machine_states(machine).iter().find_map(|state| {
            let StatementNode::Expression(expression) = program
                .statement_table
                .statements(state.statement_nodes)
                .last()?
            else {
                return None;
            };
            let ExpressionNode::Call(call) = program.expression_table.expression(*expression)
            else {
                return None;
            };
            let ExpressionNode::Member(member) = program.expression_table.expression(call.receiver)
            else {
                return None;
            };
            Some((
                call.receiver,
                member.receiver,
                member.member_symbol,
                *expression,
            ))
        })
    else {
        // The other fixture spelling uses statement calls.
        return;
    };
    let field = validation::exact_self_field(program, machine, receiver)
        .unwrap()
        .symbol;
    let other_field = program
        .data_definitions()
        .iter()
        .find_map(|data| {
            program
                .data_members(data)
                .iter()
                .find_map(|member| match member {
                    typed_trees::data::DataMember::Field(candidate)
                        if candidate.symbol != field =>
                    {
                        Some(candidate.symbol)
                    }
                    typed_trees::data::DataMember::Variant(variant) => program
                        .data_payload_fields(variant)
                        .iter()
                        .find_map(|candidate| {
                            (candidate.symbol != field).then_some(candidate.symbol)
                        }),
                    _ => None,
                })
        })
        .expect("another live storage field");
    let (captured, call) = checked
        .facts
        .flow
        .control
        .calls
        .iter()
        .find(|(_, call)| call.authored_expression == expression && call.call_ordinal == 0)
        .unwrap();
    assert_eq!(call.receiver_symbol, field);
    assert_ne!(
        inherited, field,
        "inherited scope slot is not storage identity"
    );
    for mutation in 0..5 {
        let mut changed = checked.clone();
        match mutation {
            0 | 1 => {
                let ExpressionNode::Member(member) =
                    changed.typed.expression_table.expression_mut(receiver)
                else {
                    unreachable!()
                };
                member.member_symbol = if mutation == 0 {
                    symbols::SymbolHandle::invalid()
                } else {
                    other_field
                };
            }
            2 | 3 => {
                let ExpressionNode::Name(name) = changed
                    .typed
                    .expression_table
                    .expression_mut(self_expression)
                else {
                    unreachable!()
                };
                if mutation == 2 {
                    name.symbol = other_field;
                } else {
                    name.head_symbol = other_field;
                }
            }
            4 => {
                changed
                    .facts
                    .flow
                    .control
                    .calls
                    .get_mut(captured)
                    .receiver_symbol = inherited
            }
            _ => unreachable!(),
        }
        assert!(
            lower_machine(&changed, "Main::main").is_err(),
            "provider-field source/capture mutation={mutation} must reject"
        );
    }
}

const DYNAMIC_CONTINUATION_SOURCE: &str = r#"
        boundary trait Console {
            machine exit_process(return_code: i32) reaches Console;
        }
        trait Measure { machine measure(&self) -> i32; }
        data Item [copy] { value: i32; }
        Primary: Item satisfies Measure {
            machine measure(&self) -> i32 { transition { _ -> self.value } }
        }
        data Main { console: Console; selected: Item; }
        machine Main::main(&mut self) reaches Console {
            let erased: &dyn Measure = &self.selected as &dyn Item::Primary;
            let result: i32 = forward(erased);
            transition result == 0 { true -> good() _ -> bad() }
            state good(&mut self) { self.console.exit_process(helper(70i32)); }
            state bad(&mut self) { self.console.exit_process(helper(71i32)); }
        }
        machine forward(erased: &dyn Measure) -> i32 {
            let result: i32 = finish(erased);
            transition { _ -> result }
        }
        machine finish(erased: &dyn Measure) -> i32 {
            let result: i32 = erased.measure();
            transition { _ -> result }
        }
        machine helper(value: i32) -> i32 { identity(value) }
        machine identity(value: i32) -> i32
        requires 0i32 == 0i32
        ensures 0i32 == 0i32
        { value }
    "#;

#[test]
fn dynamic_continuation_operands_preserve_forwarding_and_helper_identities() {
    let source = DYNAMIC_CONTINUATION_SOURCE;
    for source in [
        source.to_owned(),
        source
            .replace(
                "exit_process(helper(70i32));",
                "exit_process(helper(70i32))",
            )
            .replace(
                "exit_process(helper(71i32));",
                "exit_process(helper(71i32))",
            ),
    ] {
        let checked = checked_source(&source);
        let [plan] = checked
            .facts
            .flow
            .terminal_unit_effects
            .dynamic_dispatch
            .direct_scalar_calls
            .as_slice()
        else {
            panic!("one authored dynamic scalar continuation");
        };
        assert_eq!(plan.forwarding_transfers.len(), 1);
        assert!(plan.unit_continuation.is_some());
        let lowered = roundtrip(&checked);
        assert_trailing_provider_field_custody(&checked);
        assert_eq!(lowered.semantic_module.machines.len(), 6);
        assert_eq!(lowered.source_call_occurrences.len(), 8);
        let dynamic = &lowered.semantic_module.dynamic_dispatch;
        assert_eq!(dynamic.parameters.len(), 2);
        assert_eq!(dynamic.arguments.len(), 2);
        assert_eq!(dynamic.parameter_dispatches.len(), 1);
        let caller = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == lowered.semantic_module.entry)
            .unwrap();
        let operations = caller
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .collect::<Vec<_>>();
        assert_eq!(
            operations
                .iter()
                .filter(|operation| matches!(operation.kind, OperationKind::BoundaryCall { .. }))
                .count(),
            2
        );
        assert!(
            caller.blocks.len() > 3,
            "operand evaluation retains private blocks"
        );
    }
}

#[test]
fn dynamic_result_continuation_calls_an_observable_ordinary_unit_body() {
    let source = DYNAMIC_CONTINUATION_SOURCE
        .replace(
            "data Main {",
            r#"
            machine consume(value: i32) reaches Console {
                Console::exit_process(value);
            }
            data Main {
        "#,
        )
        .replace(
            "self.console.exit_process(helper(70i32));",
            "consume(helper(70i32));",
        );
    let checked = checked_source(&source);
    let lowered = roundtrip(&checked);
    assert!(
        lowered.semantic_module.machines.iter().any(|machine| {
            machine.id != lowered.semantic_module.entry
                && machine.blocks.iter().any(|block| {
                    block.operations.iter().any(|operation| {
                        matches!(operation.kind, OperationKind::BoundaryCall { .. })
                    })
                })
        }),
        "dynamic continuation includes the observable ordinary Unit body"
    );
}

#[test]
fn closed_sum_computed_operand_keeps_payload_for_the_following_call() {
    let source = r#"
        machine identity(value: i32) -> i32 { value }
        data ByteRead { case Eof; case Byte(value: i32 [0..=255]); }
        boundary trait Console {
            machine read_byte() -> ByteRead reaches Console;
            machine write_byte(value: i32) reaches Console;
            machine exit_process(value: i32) reaches Console;
        }
        data Main { console: Console; }
        machine Main::main(&mut self) reaches Console {
            let result: ByteRead = self.console.read_byte();
            transition result {
                ByteRead::Byte { value } -> byte(value)
                ByteRead::Eof -> eof()
            }
            state byte(&mut self, value: i32 [0..=255]) {
                self.console.write_byte(identity(identity(value)));
                self.console.exit_process(value);
            }
            state eof(&mut self) { self.console.exit_process(70); }
        }
    "#;
    for source in [
        source.to_owned(),
        source
            .replace("exit_process(value);", "exit_process(value)")
            .replace("exit_process(70);", "exit_process(70)"),
    ] {
        let checked = checked_source(&source);
        // StructuralCase payload execution is not implemented by the interpreter;
        // codec roundtrips and independent verification cover this representation.
        let lowered = roundtrip(&checked);
        assert_trailing_provider_field_custody(&checked);
        assert_eq!(lowered.semantic_module.machines.len(), 2);
        assert_eq!(lowered.source_call_occurrences.len(), 6);
        let caller = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == lowered.semantic_module.entry)
            .unwrap();
        assert!(matches!(
            caller.blocks[0].terminator,
            Terminator::StructuralCase { .. }
        ));
        let operations = caller
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .collect::<Vec<_>>();
        assert_eq!(
            operations
                .iter()
                .filter(|operation| matches!(operation.kind, OperationKind::BoundaryCall { .. }))
                .count(),
            4
        );
        let (completion, arguments) = caller
            .blocks
            .iter()
            .find_map(|block| {
                let arguments = block
                    .operations
                    .iter()
                    .filter_map(|operation| {
                        if let OperationKind::BoundaryCall { arguments, .. } = &operation.kind {
                            Some(arguments)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                (arguments.len() == 2).then_some((block, arguments))
            })
            .expect("computed write and subsequent exit share the leaf completion");
        assert_eq!(arguments[1].as_slice(), [completion.parameters[0].id]);
        assert_ne!(
            arguments[0], arguments[1],
            "later use selects retained payload, not computed operand slot"
        );
    }
}

const CLOSED_SUM_UNIT_SOURCE: &str = r#"
        machine identity(value: i32) -> i32 { value }
        data ByteRead { case Eof; case Byte(value: i32 [0..=255]); }
        boundary trait Console {
            machine read_byte() -> ByteRead reaches Console;
            machine write_byte(value: i32) reaches Console;
            machine exit_process(value: i32) reaches Console;
        }
        machine consume(value: i32) reaches Console {
            Console::write_byte(value);
        }
        data Main { console: Console; }
        machine Main::main(&mut self) reaches Console {
            let result: ByteRead = self.console.read_byte();
            transition result {
                ByteRead::Byte { value } -> byte(value)
                ByteRead::Eof -> eof()
            }
            state byte(&mut self, value: i32 [0..=255]) {
                consume(identity(value));
                self.console.exit_process(value);
            }
            state eof(&mut self) { self.console.exit_process(70); }
        }
        "#;

#[test]
fn closed_sum_calls_share_the_ordinary_structural_result_namespace() {
    let source = CLOSED_SUM_UNIT_SOURCE
        .replace("data Main {", "data Scratch { value: i32; } data Main {")
        .replace(
            "self.console.exit_process(value);",
            "let scratch: Scratch = Scratch { value: 7 }; let second: ByteRead = self.console.read_byte(); self.console.exit_process(value);",
        );
    let checked = checked_source(&source);
    let lowered = roundtrip(&checked);
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .unwrap();
    assert_eq!(
        entry
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| matches!(
                operation.result,
                terminal_psi::OperationResult::Structural(_)
            ))
            .count(),
        3
    );
}

#[test]
fn closed_sum_graph_keeps_shared_and_mutable_receiver_custody() {
    for (receiver, access, terminal_access) in [
        (
            "&self",
            checked_trees::CheckedStructuralAccess::SharedBorrow,
            terminal_psi::StructuralAccess::SharedBorrow,
        ),
        (
            "&mut self",
            checked_trees::CheckedStructuralAccess::MutableBorrow,
            terminal_psi::StructuralAccess::MutableBorrow,
        ),
    ] {
        let checked = checked_source(&CLOSED_SUM_UNIT_SOURCE.replace("&mut self", receiver));
        let plan_index = checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter()
            .position(|plan| {
                checked.machines().iter().any(|machine| {
                    machine.symbol == plan.machine && machine.name.as_str() == "Main::main"
                })
            })
            .unwrap_or_else(|| {
                panic!(
                    "ordinary graph plan for {receiver}: {:?}",
                    checked.facts.flow.terminal_unit_effects.omissions
                )
            });
        let plan = &checked.facts.flow.terminal_unit_effects.composed_machines[plan_index];
        assert_eq!(plan.states.len(), 3);
        for state in &plan.states {
            let [parameter] = state.structural_parameters.as_slice() else {
                panic!("every state retains the receiver");
            };
            assert!(parameter.is_self);
            assert_eq!(parameter.access, access);
        }
        let lowered = roundtrip(&checked);
        let entry = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == lowered.semantic_module.entry)
            .unwrap();
        assert_eq!(entry.structural_parameters.len(), 1);
        assert_eq!(entry.structural_parameters[0].access, terminal_access);

        for mutation in 0..2 {
            let mut changed = checked.clone();
            let state = &mut changed.facts.flow.terminal_unit_effects.composed_machines[plan_index]
                .states[1];
            if mutation == 0 {
                state.structural_parameters.clear();
            } else {
                state.structural_parameters[0].access = match access {
                    checked_trees::CheckedStructuralAccess::SharedBorrow => {
                        checked_trees::CheckedStructuralAccess::MutableBorrow
                    }
                    _ => checked_trees::CheckedStructuralAccess::SharedBorrow,
                };
            }
            assert!(
                lower_machine(&changed, "Main::main").is_err(),
                "receiver custody mutation {mutation} must reject"
            );
        }
    }
}

#[test]
fn closed_sum_payload_calls_an_observable_ordinary_unit_body() {
    let checked = checked_source(CLOSED_SUM_UNIT_SOURCE);
    let lowered = roundtrip(&checked);
    assert_eq!(lowered.semantic_module.machines.len(), 3);
    assert!(lowered.semantic_module.machines.iter().any(|machine| {
        machine.id != lowered.semantic_module.entry
            && machine.blocks.iter().any(|block| {
                block
                    .operations
                    .iter()
                    .any(|operation| matches!(operation.kind, OperationKind::BoundaryCall { .. }))
            })
    }));
}

#[test]
fn closed_sum_returning_arms_discard_their_own_boundary_results() {
    let source = CLOSED_SUM_UNIT_SOURCE
        .replace(
            "consume(identity(value));",
            "self.console.write_byte(value);",
        )
        .replace(
            "self.console.exit_process(value);",
            "let second: ByteRead = self.console.read_byte();",
        )
        .replace(
            "self.console.exit_process(70);",
            "let second: ByteRead = self.console.read_byte();",
        );
    let checked = checked_source(&source);
    let lowered = roundtrip(&checked);
    let machine = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .unwrap();
    let mut result_places = Vec::new();
    let mut returns = 0;
    for block in &machine.blocks {
        let results = block
            .operations
            .iter()
            .filter_map(|operation| match &operation.result {
                terminal_psi::OperationResult::Structural(result) => Some(result.place),
                _ => None,
            })
            .collect::<Vec<_>>();
        if let terminal_psi::Terminator::ReturnUnit {
            trivial_affine_discards,
            ..
        } = &block.terminator
        {
            assert_eq!(results.len(), 1);
            assert_eq!(trivial_affine_discards, &results);
            returns += 1;
        }
        result_places.extend(results);
    }
    assert_eq!(returns, 2);
    assert_eq!(result_places.len(), 3);
    result_places.sort();
    result_places.dedup();
    assert_eq!(result_places.len(), 3, "state-local results do not alias");

    let arm_states = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .flat_map(|plan| plan.states.iter().skip(1).map(|state| state.state))
        .collect::<Vec<_>>();
    let (drop_handle, _) = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .find(|(_, event)| {
            arm_states.contains(&event.state_symbol)
                && event.kind == language_semantics::PermissionEventKind::AffineDrop
                && event.source == language_semantics::PermissionEventSource::StateExit
        })
        .expect("source-owned arm-local result disposal");
    for mutation in 0..2 {
        let mut changed = checked.clone();
        let event = changed
            .facts
            .flow
            .ownership
            .permissions
            .get_mut(drop_handle);
        if mutation == 0 {
            event.machine_symbol = Default::default();
        } else {
            event.provenance = language_semantics::PermissionProvenance::Unknown;
        }
        assert!(
            lower_machine(&changed, "Main::main").is_err(),
            "missing or substituted return disposal must reject"
        );
    }
    let mut missing_cleanup = lowered.semantic_module.clone();
    let return_block = missing_cleanup
        .machines
        .iter_mut()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .unwrap()
        .blocks
        .iter_mut()
        .find(|block| matches!(block.terminator, Terminator::ReturnUnit { .. }))
        .unwrap();
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut return_block.terminator
    else {
        unreachable!();
    };
    trivial_affine_discards.clear();
    assert!(
        terminal_verifier::verify_module(
            &missing_cleanup,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default()
        )
        .is_err(),
        "portable checking must reject omitted cleanup"
    );

    let (plan_index, state_index, operation_index) = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .enumerate()
        .find_map(|(plan_index, plan)| {
            plan.states
                .iter()
                .enumerate()
                .skip(1)
                .find_map(|(state_index, state)| {
                    state
                        .operations
                        .iter()
                        .position(|operation| {
                            matches!(operation,
                    checked_trees::CheckedUnitEffectOperationPlan::BoundaryStructuralCall { .. })
                        })
                        .map(|operation_index| (plan_index, state_index, operation_index))
                })
        })
        .unwrap();
    for mutation in 0..4 {
        let mut changed = checked.clone();
        let checked_trees::CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
            coordinate,
            result,
            discard_result_on_return,
            ..
        } = &mut changed.facts.flow.terminal_unit_effects.composed_machines[plan_index].states
            [state_index]
            .operations[operation_index]
        else {
            panic!("result call")
        };
        match mutation {
            0 => *discard_result_on_return = true,
            1 => result.statement_index += 1,
            2 => coordinate.statement_index += 1,
            3 => result.binding_ordinal += 1,
            _ => unreachable!(),
        }
        assert!(
            lower_machine(&changed, "Main::main").is_err(),
            "result source/return custody mutation {mutation} must reject"
        );
    }
}

#[test]
fn closed_sum_unused_payload_keeps_source_disposal_and_marker_checks() {
    let checked = checked_source(
        r#"
        data ByteRead { case Eof; case Byte(value: i32 [0..=255]); }
        boundary trait Console {
            machine read_byte() -> ByteRead reaches Console;
            machine exit_process(value: i32) reaches Console;
        }
        data Main { console: Console; }
        machine Main::main(&mut self) reaches Console {
            let observed: ByteRead = self.console.read_byte();
            transition observed {
                ByteRead::Byte { value } -> done()
                ByteRead::Eof -> done()
            }
            state done(&mut self) { self.console.exit_process(32); }
        }
        "#,
    );
    roundtrip(&checked);
    let (event_handle, _) = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .find(|(_, event)| {
            event.kind == language_semantics::PermissionEventKind::AffineDrop
                && event.source == language_semantics::PermissionEventSource::StateExit
        })
        .expect("source-owned case subject disposal");
    let mut changed = checked.clone();
    changed
        .facts
        .flow
        .ownership
        .permissions
        .get_mut(event_handle)
        .obligation_live = true;
    assert!(
        lower_machine(&changed, "Main::main").is_err(),
        "unused payloads cannot erase a live disposal obligation"
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("entry machine");
    let statements = checked.machine_states(machine)[0].statement_nodes;
    let mut changed = checked.clone();
    let marker = changed
        .typed
        .statement_table
        .statements_mut(statements)
        .iter_mut()
        .find_map(|statement| match statement {
            StatementNode::LocalData(local)
                if local.name.as_str().starts_with("__arm_destructure#V=") =>
            {
                Some(local)
            }
            _ => None,
        })
        .expect("authored case marker");
    marker.name = marker
        .name
        .as_str()
        .replace("#value#", "#fabricated#")
        .as_str()
        .into();
    assert!(
        lower_machine(&changed, "Main::main").is_err(),
        "even an unused binding must name its declared payload"
    );

    let mut changed = checked_source(CLOSED_SUM_UNIT_SOURCE);
    let cases = changed
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter_mut()
        .flat_map(|plan| &mut plan.states)
        .find_map(|state| match &mut state.terminator {
            checked_trees::CheckedComposedUnitControlTerminatorPlan::ClosedSum {
                cases, ..
            } => Some(cases),
            _ => None,
        })
        .expect("consumed payload dispatch");
    for case in cases {
        case.payloads.clear();
    }
    assert!(
        lower_machine(&changed, "Main::main").is_err(),
        "a source-used payload cannot be omitted from its actual transfer"
    );
}

#[test]
fn closed_sum_unit_closure_shares_helpers_and_preserves_payload_and_cleanup() {
    for qualified in [false, true] {
        for trailing in [false, true] {
            let source = CLOSED_SUM_UNIT_SOURCE
                .replace(
                    "machine consume(value: i32)",
                    "data Empty {}\n machine consume(value: i32)",
                )
                .replace(
                    "Console::write_byte(value);",
                    r#"
                    Console::write_byte(identity(value));
                    forward(identity(value));
                "#,
                )
                .replace(
                    "data Main {",
                    r#"
                    machine forward(value: i32) reaches Console {
                        let first: Empty = Empty {};
                        let second: Empty = Empty {};
                        Console::write_byte(identity(value));
                    }
                    data Main {
                "#,
                )
                .replace(
                    "consume(identity(value));",
                    r#"
                    self.console.write_byte(value);
                    consume(identity(identity(value)));
                "#,
                )
                .replace(
                    "self.console.exit_process(value);",
                    "self.console.exit_process(value); consume(identity(value));",
                )
                .replace(
                    "self.console.exit_process(70);",
                    "consume(identity(70i32));",
                );
            let source = if qualified {
                source
                    .replace("data Empty {}", "data Empty {} data Relay {}")
                    .replace("consume(", "Relay::consume(")
            } else {
                source
            };
            let source = if trailing {
                source
                    .replace("consume(identity(value));", "consume(identity(value))")
                    .replace("consume(identity(70i32));", "consume(identity(70i32))")
            } else {
                source
            };
            let checked = checked_source(&source);
            let lowered = roundtrip(&checked);
            assert_eq!(lowered.semantic_module.machines.len(), 4);
            assert_closed_sum_unit_catalog(&checked, &lowered);
            assert_closed_sum_unit_source_custody(&checked);
            let artifact =
                terminal_production::TerminalProductionRequest::new(&checked, "Main::main")
                    .produce_artifact()
                    .expect("complete ordinary callees survive Terminal publication");
            assert_eq!(
                terminal_codec::decode_module(artifact.semantic_bytes()).unwrap(),
                lowered.semantic_module
            );
        }
    }
}

fn assert_closed_sum_unit_source_custody(checked: &CheckedTrees) {
    let plans = &checked.facts.flow.terminal_unit_effects.composed_machines;
    let (plan_index, state_index, operation_index) = plans
        .iter()
        .enumerate()
        .find_map(|(plan_index, plan)| {
            plan.states
                .iter()
                .enumerate()
                .find_map(|(state_index, state)| {
                    state
                        .operations
                        .iter()
                        .position(|operation| {
                            matches!(
                                operation,
                                checked_trees::CheckedUnitEffectOperationPlan::CallUnit { .. }
                            )
                        })
                        .map(|operation_index| (plan_index, state_index, operation_index))
                })
        })
        .expect("closed-sum ordinary Unit operation");
    let checked_trees::CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
        target_machine: entry_boundary,
        ..
    } = plans[plan_index].states[0].operations[0]
    else {
        panic!("closed-sum entry boundary")
    };
    let mut changed = checked.clone();
    changed.facts.flow.terminal_unit_effects.composed_machines[plan_index]
        .provider_attachment_requirements
        .retain(|requirement| requirement.boundary != entry_boundary);
    assert!(
        lower_machine(&changed, "Main::main").is_err(),
        "ordinary callee ownership cannot erase the root's entry-boundary requirement"
    );
    let mut changed = checked.clone();
    changed.facts.flow.terminal_unit_effects.composed_machines[plan_index].states[state_index]
        .operations[operation_index] =
        checked_trees::CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
            statement_index: 0,
            declaration_ordinal: 0,
            type_identity: "Empty".to_owned(),
        };
    assert!(
        lower_machine(&changed, "Main::main").is_err(),
        "a fabricated non-call leaf operation is not authored evidence"
    );
    for (handle, root) in checked.facts.values.scalar_computations.roots.iter() {
        if !matches!(
            root.role,
            checked_trees::CheckedScalarExpressionRole::UnitCallArgument { .. }
        ) {
            continue;
        }
        let mut changed = checked.clone();
        changed
            .facts
            .values
            .scalar_computations
            .roots
            .get_mut(handle)
            .statement_ordinal += 1;
        assert!(
            lower_machine(&changed, "Main::main").is_err(),
            "operand coordinate drift rejects"
        );
    }
    let checked_trees::CheckedUnitEffectOperationPlan::CallUnit { target_state, .. } =
        &plans[plan_index].states[state_index].operations[operation_index]
    else {
        unreachable!();
    };
    let (handle, _) = checked
        .facts
        .flow
        .control
        .calls
        .iter()
        .find(|(_, call)| call.target_symbol == *target_state && call.call_ordinal == 0)
        .expect("captured closed-sum leaf call");
    let mut changed = checked.clone();
    changed
        .facts
        .flow
        .control
        .calls
        .get_mut(handle)
        .target_symbol = symbols::SymbolHandle::invalid();
    assert!(
        lower_machine(&changed, "Main::main").is_err(),
        "captured leaf target drift rejects"
    );
}

fn assert_closed_sum_unit_catalog(checked: &CheckedTrees, lowered: &LoweredPsi) {
    let module = &lowered.semantic_module;
    let mut blocks = std::collections::BTreeSet::new();
    let mut operations = std::collections::BTreeSet::new();
    let mut edges = std::collections::BTreeSet::new();
    let mut values = std::collections::BTreeSet::new();
    let mut places = std::collections::BTreeSet::new();
    for machine in &module.machines {
        for place in &machine.structural_places {
            assert!(places.insert(place.id));
        }
        for value in machine.parameters.iter().chain(machine.result.scalar_ref()) {
            assert!(values.insert(value.id));
        }
        for block in &machine.blocks {
            assert!(blocks.insert(block.id));
            for value in &block.parameters {
                assert!(values.insert(value.id));
            }
            for operation in &block.operations {
                assert!(operations.insert(operation.id));
                if let Some(value) = operation.result.scalar_ref() {
                    assert!(values.insert(value.id));
                }
            }
            for edge in block.terminator.edges() {
                assert!(edges.insert(edge));
            }
        }
    }
    let source_helper = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "identity")
        .unwrap();
    let mut helper_targets = std::collections::BTreeSet::new();
    let mut helper_callers = Vec::new();
    for occurrence in &lowered.source_call_occurrences {
        if occurrence.source_target != source_helper.symbol {
            continue;
        }
        if !helper_callers.contains(&occurrence.source_state) {
            helper_callers.push(occurrence.source_state);
        }
        let operation = module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .find(|operation| operation.id == occurrence.terminal_operation)
            .unwrap();
        let OperationKind::Call { callee, .. } = operation.kind else {
            panic!("exact scalar helper call");
        };
        helper_targets.insert(callee);
    }
    assert_eq!(helper_targets.len(), 1);
    assert_eq!(
        helper_callers.len(),
        4,
        "byte, eof, consume, and forward share identity"
    );
    let caller = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let entry = caller
        .blocks
        .iter()
        .find(|block| block.id == caller.entry)
        .unwrap();
    let Terminator::StructuralCase { source, .. } = entry.terminator else {
        panic!("closed-sum root");
    };
    assert!(
        entry
            .operations
            .iter()
            .any(|operation| matches!(&operation.result,
        terminal_psi::OperationResult::Structural(result) if result.place == source))
    );
    let completion = caller
        .blocks
        .iter()
        .find(|block| {
            block
                .operations
                .iter()
                .any(|operation| matches!(operation.kind, OperationKind::CallUnit { .. }))
                && block
                    .operations
                    .iter()
                    .any(|operation| matches!(operation.kind, OperationKind::BoundaryCall { .. }))
        })
        .expect("computed Unit call completes before payload is read again");
    let boundary = completion
        .operations
        .iter()
        .find_map(|operation| match &operation.kind {
            OperationKind::BoundaryCall { arguments, .. } => Some(arguments),
            _ => None,
        })
        .unwrap();
    assert_eq!(boundary.as_slice(), [completion.parameters[0].id]);
    let internal = completion
        .operations
        .iter()
        .find_map(|operation| match &operation.kind {
            OperationKind::CallUnit { arguments, .. } => Some(arguments),
            _ => None,
        })
        .unwrap();
    assert_ne!(
        internal, boundary,
        "computed argument never replaces payload namespace"
    );
    let cleanup = module
        .machines
        .iter()
        .find_map(|machine| {
            let established = machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter_map(|operation| match operation.kind {
                    OperationKind::EstablishTrivialAffineLocal { destination } => Some(destination),
                    _ => None,
                })
                .collect::<Vec<_>>();
            (established.len() == 2).then_some((machine, established))
        })
        .expect("transitive ordinary body retains both affine declarations");
    assert!(
        cleanup
            .0
            .blocks
            .iter()
            .any(|block| matches!(&block.terminator,
        Terminator::ReturnUnit { trivial_affine_discards, .. }
        if trivial_affine_discards.as_slice() == [cleanup.1[1], cleanup.1[0]]))
    );
}
