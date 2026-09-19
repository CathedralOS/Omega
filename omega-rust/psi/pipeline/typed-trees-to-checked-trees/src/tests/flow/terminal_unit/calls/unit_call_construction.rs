use crate::tests::flow::terminal_unit::{
    CheckedBoundaryMachineResultPlan, CheckedScalarExpression, CheckedScalarExpressionRole,
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralFieldType, CheckedUnitStructuralTypeShape,
    Multiplicity, PrimitiveType, checked, machine_named,
};
use checked_trees::{
    CheckedComposedUnitControlTerminatorPlan, CheckedScalarBinding, CheckedScalarBindingValue,
};

#[test]
fn provider_attachment_retains_ordinary_state_local_construction() {
    let checked = checked(
        r#"
        boundary trait Output { machine write(value: u64) reaches Output; }
        data Region { value: u64; }
        machine Region::new(value: u64) -> Region { Region { value: value } }
        machine Region::get(&self) -> u64 { self.value }
        data Main { output: Output; }
        machine Main::main(&mut self, input: u64) reaches Output {
            let region: Region = Region::new(input);
            let observed: u64 = region.get();
            transition observed == input { true -> passed() false -> failed() }
            state passed(&mut self) { self.output.write(11); }
            state failed(&mut self) { self.output.write(255); }
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "Main::main"))
        .expect("ordinary state locals do not become provider requirements");
    assert_eq!(plan.states.len(), 3);
    assert_eq!(plan.provider_attachment_requirements.len(), 1);
    let output = checked
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Output")
        .unwrap();
    let requirement = &checked.trait_machine_signatures(output)[0];
    assert_eq!(
        plan.provider_attachment_requirements[0].boundary,
        requirement.symbol
    );
}

#[test]
fn provider_control_retains_copy_case_constructor_call_closure() {
    let checked = checked(
        r#"
        boundary trait Output { machine write(value: u64) reaches Output; }
        data MemoryAlignment [copy] {
            case Alignment1;
            case Alignment2;
            case Alignment4;
            case Alignment8;
        }
        machine MemoryAlignment::default() -> MemoryAlignment {
            MemoryAlignment::Alignment4
        }
        machine MemoryAlignment::from(size: i32) -> MemoryAlignment {
            transition size {
                1 -> (MemoryAlignment::Alignment1)
                2 -> (MemoryAlignment::Alignment2)
                4 -> (MemoryAlignment::Alignment4)
                8 -> (MemoryAlignment::Alignment8)
                _ -> (MemoryAlignment::Alignment1)
            }
        }
        machine MemoryAlignment::get_size_in_bytes(&self) -> u64 [1..=8] {
            transition self {
                MemoryAlignment::Alignment1 -> (1)
                MemoryAlignment::Alignment2 -> (2)
                MemoryAlignment::Alignment4 -> (4)
                MemoryAlignment::Alignment8 -> (8)
            }
        }
        data Main { output: Output; }
        machine Main::main(&mut self) reaches Output {
            let default_alignment: MemoryAlignment = MemoryAlignment::default();
            let fallback_alignment: MemoryAlignment = MemoryAlignment::from(3);
            let default_size: u64 = default_alignment.get_size_in_bytes();
            let fallback_size: u64 = fallback_alignment.get_size_in_bytes();
            transition default_size == 4 && fallback_size == 1 {
                true -> passed()
                false -> failed()
            }
            state passed(&mut self) { self.output.write(11); }
            state failed(&mut self) { self.output.write(255); }
        }
        "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let constructor = plans
        .composed_for_machine(machine_named(&checked, "MemoryAlignment::from"))
        .expect("ordered copy-case returns retain a complete constructor plan");
    let CheckedComposedUnitControlTerminatorPlan::Guarded {
        arms,
        fallback,
        return_values,
    } = &constructor.states[0].terminator
    else {
        panic!("constructor retains ordinary structural values at ordered source destinations");
    };
    assert_eq!(arms.len() + usize::from(fallback.is_some()), 5);
    assert_eq!(return_values.len(), 5);
    let plan = plans
        .composed_for_machine(machine_named(&checked, "Main::main"))
        .expect("provider control retains its complete copy-case constructor closure");
    assert_eq!(plan.states.len(), 3);
    let constructors = plan.states[0]
        .operations
        .iter()
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::StructuralCall { target_machine, .. } => {
                Some(*target_machine)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        constructors,
        [
            machine_named(&checked, "MemoryAlignment::default"),
            machine_named(&checked, "MemoryAlignment::from"),
        ],
        "both exact producers remain in authored order"
    );
    let output = checked
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Output")
        .expect("provider trait");
    assert_eq!(plan.provider_attachment_requirements.len(), 1);
    assert_eq!(
        plan.provider_attachment_requirements[0].boundary,
        checked.trait_machine_signatures(output)[0].symbol,
        "only the selected leaf's boundary operation requests the provider"
    );
}

#[test]
fn ordered_structural_returns_keep_payload_effects_at_selected_destinations() {
    let checked = checked(
        r#"
        data Choice [copy] {
            case First(before: u64, after: u64);
            case Second(before: u64, after: u64);
            case Last(before: u64, after: u64);
        }
        machine replace(value: &mut u64, next: u64) -> u64 {
            let before: u64 = value;
            value = next;
            before
        }
        machine choose(selector: i32, value: &mut u64) -> Choice {
            transition {
                selector < 2 -> (Choice::First {
                    after: replace(&mut value, 11), before: replace(&mut value, 12)
                })
                selector < 4 -> (Choice::Second {
                    after: replace(&mut value, 21), before: replace(&mut value, 22)
                })
                _ -> (Choice::Last {
                    after: replace(&mut value, 31), before: replace(&mut value, 32)
                })
            }
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "choose"))
        .expect("ordered return payload computations compose with borrowed mutation");
    let state = &plan.states[0];
    assert!(
        state.operations.is_empty(),
        "selected payload calls cannot become prefix effects"
    );
    let CheckedComposedUnitControlTerminatorPlan::Guarded {
        arms,
        fallback,
        return_values,
    } = &state.terminator
    else {
        panic!("ordered source control");
    };
    assert_eq!(return_values.len(), 3);
    assert_eq!(state.operation_dependencies().count(), 3);
    let source = checked
        .machine_states(
            checked
                .machines()
                .iter()
                .find(|machine| machine.symbol == plan.machine)
                .unwrap(),
        )
        .iter()
        .find(|source| source.symbol == state.state)
        .unwrap();
    let statements = checked.statement_table.statements(source.statement_nodes);
    let computations = &checked.facts.values.scalar_computations;
    let guards = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .guarded_exits
        .span(*arms)
        .unwrap();
    for (destination, operation) in guards
        .iter()
        .map(|guard| &guard.destination)
        .chain(fallback.iter())
        .zip(return_values)
    {
        let checked_trees::CheckedScalarBranchDestination::Return {
            statement_ordinal,
            is_continuation,
        } = destination
        else {
            panic!("value destination");
        };
        let typed_trees::statement::StatementNode::Transition(transition) =
            &statements[*statement_ordinal as usize]
        else {
            panic!("authored return transition");
        };
        let target = if *is_continuation {
            transition.continuation
        } else {
            transition.target
        };
        let typed_trees::statement::TransitionTargetNode::Value(expression) =
            checked.statement_table.transition_target(target)
        else {
            panic!("authored constructor expression");
        };
        let CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            result,
            value,
            discard_result_on_return: false,
            ..
        } = operation
        else {
            panic!("selected ordinary value establishment");
        };
        assert_eq!(result.statement_index, *statement_ordinal);
        let root = checked
            .facts
            .values
            .structural_values
            .root_for_expression(state.state, *statement_ordinal, *expression)
            .unwrap();
        assert_eq!(root.root, *value);
        let checked_trees::CheckedStructuralValueKind::Case(construction) = &checked
            .facts
            .values
            .structural_values
            .nodes
            .get(*value)
            .kind
        else {
            panic!("real case constructor");
        };
        let fields = computations.case_fields.span(construction.fields).unwrap();
        assert_eq!(fields.len(), 2);
        for (field_ordinal, field) in fields.iter().enumerate() {
            let root = computations
                .root_at(
                    state.state,
                    *statement_ordinal,
                    CheckedScalarExpressionRole::StructuralValueField {
                        expression: *expression,
                        field_ordinal: u32::try_from(field_ordinal).unwrap(),
                    },
                )
                .unwrap();
            assert_eq!(
                root.root, field.value,
                "payload operands retain authored order and exact source scopes"
            );
        }
    }
}

#[test]
fn linear_result_arguments_continue_the_producing_call_claim() {
    let checked = checked(
        r#"
        data ByteUnit {}
        data CountedQuantity<Unit> { magnitude: u64; }
        trait Content<A> { machine project(subject: &Self) -> A; }
        data Region [linear] { length: u64; }
        domain Region::Owned;
        machine Owned::content(region: &Region) -> CountedQuantity<ByteUnit>
        satisfies Content<CountedQuantity<ByteUnit>>::project
        { CountedQuantity { magnitude: region.length } }
        data Main {}
        machine Main::forward(region: Region in Owned) -> Region in Owned { region }
        machine Main::enter(region: Region in Owned) -> Region in Owned {
            let first: Region in Owned = Main::forward(region);
            let second: Region in Owned = Main::forward(first);
            Main::forward(second)
        }
    "#,
    );
    let root = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .find(|plan| plan.machine == machine_named(&checked, "Main::enter"))
        .expect("ordinary graph carries successive linear results");
    let calls = root
        .states
        .iter()
        .flat_map(|state| &state.operations)
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::StructuralCall {
                result,
                custody,
                structural_arguments,
                discard_result_on_return,
                ..
            } => {
                assert!(
                    !discard_result_on_return,
                    "linear custody is never automatic cleanup debt"
                );
                Some((result, custody, structural_arguments))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 3);
    for pair in calls.windows(2) {
        let (produced, frontier, _) = pair[0];
        let (returned, custody, arguments) = pair[1];
        assert_eq!(
            arguments[0].source_structural_result_binding_ordinal(),
            Some(produced.binding_ordinal)
        );
        assert_ne!(produced.binding_ordinal, returned.binding_ordinal);
        assert_eq!(
            frontier.result_qualifications,
            custody.result_qualifications
        );
        assert_eq!(
            frontier.returned_claim_transfers[0].caller_claim,
            custody.claim_transfers[0].claim_identity
        );
    }
}

#[test]
fn retains_owned_affine_i64_record_literal_for_direct_unit_call() {
    let checked = checked(
        r#"
        data Packet { value: i64; }

        data Sink {}
        machine Sink::accept(packet: Packet) {}

        data Root {}
        machine Root::enter() {
            let packet: Packet = Packet { value: 7 };
            Sink::accept(move packet);
        }
        "#,
    );
    let root = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "Root::enter"))
        .expect("owned affine scalar-record caller plan");
    assert!(matches!(
        root.operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                result,
                discard_result_on_return: false,
                ..
            },
            CheckedUnitEffectOperationPlan::CallUnit {
                structural_arguments,
                ..
            },
            CheckedUnitEffectOperationPlan::Complete {
                trivial_affine_local_discard_ordinals,
                ..
            }
        ] if result.binding_ordinal == 0
            && matches!(structural_arguments.as_slice(), [argument]
                if argument.source_structural_result_binding_ordinal() == Some(0)
                    && argument.path.is_empty()
                    && argument.access == checked_trees::CheckedStructuralAccess::Owned)
            && trivial_affine_local_discard_ordinals.is_empty()
    ));
    let CheckedUnitEffectOperationPlan::EstablishStructuralValue { value, .. } =
        &root.operations[0]
    else {
        panic!("record construction");
    };
    let checked_trees::CheckedStructuralValueKind::Record { fields, .. } = checked
        .facts
        .values
        .structural_values
        .nodes
        .get(*value)
        .kind
    else {
        panic!("record fields");
    };
    let [field] = checked
        .facts
        .values
        .structural_values
        .record_fields
        .span_or_empty(fields)
    else {
        panic!("one field");
    };
    let checked_trees::CheckedStructuralRecordFieldValue::Scalar(value) = field.value else {
        panic!("exact scalar field operand");
    };
    assert!(
        matches!(&checked.facts.values.scalar_computations.nodes.get(value).kind,
        checked_trees::CheckedScalarComputationKind::Value(CheckedScalarExpression::IntegerLiteral { literal }) if literal.value_i64() == Some(7))
    );
}

#[test]
fn retains_exact_byte_literal_for_static_bodyless_boundary() {
    let checked = checked(
        r#"
        boundary trait Console {
            machine write_line(text: &[u8])
            reaches Console;
        }

        data Root {}
        machine Root::enter()
        reaches Console
        {
            Console::write_line("\x80A");
        }
        "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let boundary = plans
        .boundary_machines
        .iter()
        .find(|boundary| boundary.structural_parameters.len() == 1)
        .expect("write_line boundary structural parameter");
    assert!(boundary.scalar_parameters.is_empty());
    let byte_type = plans
        .structural_types
        .iter()
        .find(|shape| shape.identity == boundary.structural_parameters[0].type_identity)
        .expect("borrowed byte-sequence shape");
    assert!(matches!(
        byte_type.shape,
        CheckedUnitStructuralTypeShape::ByteSequence(
            checked_trees::CheckedByteSequenceCarrier::BorrowedView
        )
    ));
    let root = plans
        .for_machine(machine_named(&checked, "enter"))
        .expect("literal boundary caller plan");
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        structural_arguments,
        ..
    } = &root.operations[0]
    else {
        panic!("write_line should retain a bodyless boundary call")
    };
    assert!(matches!(
        structural_arguments.as_slice(),
        [argument]
            if argument.source_parameter_index().is_none()
                && argument.path.is_empty()
                && argument.byte_sequence_literal() == Some(&[0x80, b'A'][..])
    ));
}

#[test]
fn retains_owned_structural_result_for_static_bodyless_boundary() {
    let checked = checked(
        r#"
        data ByteRead {
            case Eof;
            case Byte(value: i32);
        }

        boundary trait Console {
            machine read_byte() -> ByteRead
            reaches Console;
        }

        data Root {}
        machine Root::enter()
        reaches Console
        {
            let result: ByteRead = Console::read_byte();
        }
        "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let boundary = plans
        .boundary_machines
        .iter()
        .find(|boundary| {
            matches!(
                boundary.result,
                CheckedBoundaryMachineResultPlan::Structural { .. }
            )
        })
        .expect("read_byte boundary structural result");
    assert!(matches!(
        &boundary.result,
        CheckedBoundaryMachineResultPlan::Structural {
            multiplicity: Multiplicity::Affine,
            qualifications,
            ..
        } if qualifications.is_empty()
    ));
    let root = plans
        .for_machine(machine_named(&checked, "enter"))
        .expect("structural boundary-result caller plan");
    assert!(matches!(
        root.operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                result,
                discard_result_on_return: true,
                ..
            },
            CheckedUnitEffectOperationPlan::Complete { .. }
        ] if result.type_identity.contains("ByteRead")
            && result.multiplicity == Multiplicity::Affine
    ));
}

#[test]
fn retains_owned_structural_result_for_attached_bodyless_boundary() {
    let checked = checked(
        r#"
        data ByteRead {
            case Eof;
            case Byte(value: i32 [0..=255]);
        }

        boundary trait Console {
            machine read_byte() -> ByteRead reaches Console;
        }

        data Main { console: Console; }
        machine Main::main(&mut self) reaches Console {
            let result: ByteRead = self.console.read_byte();
        }
        "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let root = plans
        .for_machine(machine_named(&checked, "main"))
        .expect("attached structural boundary-result caller plan");
    assert!(matches!(
        root.operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::BoundaryStructuralCall { .. },
            CheckedUnitEffectOperationPlan::Complete { .. }
        ]
    ));
}

#[test]
fn closed_sum_dispatch_allows_unused_scalar_payload_bindings() {
    let checked = checked(
        r#"
        data Reading {
            case Empty;
            case Value(value: u64, ignored: u64);
            case Full(count: u64);
        }
        machine consume(observed: Reading) {
            transition observed {
                Reading::Value { value, ignored } -> retain(value)
                Reading::Full { count } -> done()
                Reading::Empty -> done()
            }
            state retain(value: u64) {}
            state done() {}
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "consume"))
        .expect("unused scalar payloads do not prevent consuming the owned sum");
    let CheckedComposedUnitControlTerminatorPlan::ClosedSum { cases, .. } =
        &plan.states[0].terminator
    else {
        panic!("entry consumes its owned case subject")
    };
    assert_eq!(cases.len(), 3);
    for case in cases {
        if case.case_identity == "Value" {
            assert!(matches!(case.payloads.as_slice(), [payload]
                if payload.field_identity == "value"
                    && payload.primitive_type == PrimitiveType::U64));
        } else {
            assert!(case.payloads.is_empty());
        }
    }
}

#[test]
fn retains_closed_sum_inspection_after_structural_boundary_result() {
    let checked = checked(
        r#"
        data ByteRead {
            case Eof;
            case Byte(value: i32 [0..=255]);
        }

        boundary trait Console {
            machine read_byte() -> ByteRead reaches Console;
            machine write_byte(value: i32) reaches Console;
            machine exit_process(return_code: i32) reaches Console;
        }

        data Main { console: Console; }
        machine Main::main(&mut self) reaches Console {
            let result: ByteRead = self.console.read_byte();
            transition result {
                ByteRead::Byte { value } -> byte(value)
                ByteRead::Eof -> eof()
            }

            state byte(&mut self, value: i32 [0..=255]) {
                self.console.write_byte(value);
                self.console.exit_process(70);
            }

            state eof(&mut self) {
                self.console.exit_process(70);
            }
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "main"))
        .expect("closed-sum inspection plan");
    let [entry, byte, eof] = plan.states.as_slice() else {
        panic!("closed-sum inspection should retain exactly three states")
    };
    assert!(matches!(
        entry.operations.as_slice(),
        [CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
            discard_result_on_return: false,
            ..
        }]
    ));
    let CheckedComposedUnitControlTerminatorPlan::ClosedSum { cases, .. } = &entry.terminator
    else {
        panic!("entry should inspect the structural result")
    };
    assert_eq!(cases.len(), 2);
    assert!(cases.iter().any(|case| {
        case.case_identity == "Byte"
            && case.successor.target_state == byte.state
            && matches!(case.payloads.as_slice(), [payload]
                if payload.field_identity == "value"
                    && payload.primitive_type == PrimitiveType::I32)
    }));
    assert!(cases.iter().any(|case| {
        case.case_identity == "Eof"
            && case.successor.target_state == eof.state
            && case.payloads.is_empty()
    }));
}

#[test]
fn retains_boundary_case_payload_and_mutable_view_on_the_same_state_edge() {
    let checked = checked(
        r#"
        data ByteRead { case Eof; case Byte(value: i32 [0..=255]); }
        boundary trait Console {
            machine read_byte() -> ByteRead reaches Console;
        }
        machine read_one(out: &mut [u8]) reaches Console {
            transition out.len > 0 { true -> read(out, 7) false -> done() }
            state read(out: &mut [u8], count: u64) {
                let observed: ByteRead = Console::read_byte();
                transition observed {
                    ByteRead::Byte { value } -> store(out, count, value)
                    ByteRead::Eof -> done()
                }
            }
            state store(out: &mut [u8], count: u64, value: i32 [0..=255]) {
                out[0] = value as u8;
            }
            state done() {}
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "read_one"))
        .expect("boundary payload inspection composes with the ordinary state graph");
    let read = &plan.states[1];
    assert!(matches!(
        read.operations.as_slice(),
        [CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
            discard_result_on_return: false,
            ..
        }]
    ));
    let CheckedComposedUnitControlTerminatorPlan::ClosedSum { cases, .. } = &read.terminator else {
        panic!("case edge");
    };
    let byte = cases
        .iter()
        .find(|case| case.case_identity == "Byte")
        .unwrap();
    assert_eq!(byte.successor.target_state, plan.states[2].state);
    assert_eq!(byte.successor.transfers.len(), 1);
    assert_eq!(byte.successor.transfers[0].target_parameter_index, 0);
    assert_eq!(byte.successor.scalar_arguments.len(), 1);
    assert_eq!(
        byte.successor.scalar_arguments[0].target_scalar_parameter_index,
        0
    );
    assert_eq!(byte.payloads.len(), 1);
    assert_eq!(byte.payloads[0].target_scalar_parameter_index, 1);
    assert_eq!(byte.payloads[0].field_identity, "value");
    let eof = cases
        .iter()
        .find(|case| case.case_identity == "Eof")
        .unwrap();
    assert!(eof.successor.transfers.is_empty());
    assert!(eof.successor.scalar_arguments.is_empty());
    assert!(eof.payloads.is_empty());
    let mut missing_cleanup = checked.facts.clone();
    missing_cleanup.flow.terminal_structural_control_cleanups = Default::default();
    let rejected = crate::execution::terminal_unit::build_checked_unit_effect_plans(
        &checked.typed,
        &missing_cleanup,
        crate::execution::terminal_unit::ScalarCalleePlans {
            boundary_returns: &missing_cleanup.flow.terminal_boundary_scalar_returns,
            structural_returns: &missing_cleanup.flow.terminal_structural_scalar_returns,
        },
        &[],
        &[],
    );
    assert!(
        rejected
            .composed_for_machine(machine_named(&checked, "read_one"))
            .is_none(),
        "case inspection must not bypass independently checked edge cleanup"
    );
    for mutation in ["root", "missing", "provenance"] {
        let mut changed = checked.facts.clone();
        let handles = changed
            .flow
            .ownership
            .permissions
            .iter()
            .filter(|(_, event)| {
                event.state_symbol == read.state
                    && event.source == language_semantics::PermissionEventSource::StateExit
                    && event.kind == language_semantics::PermissionEventKind::AffineDrop
            })
            .map(|(handle, _)| handle)
            .collect::<Vec<_>>();
        assert!(!handles.is_empty());
        for handle in handles {
            let event = changed.flow.ownership.permissions.get_mut(handle);
            match mutation {
                "missing" => event.source = language_semantics::PermissionEventSource::StateEntry,
                "root" => event.root = facts::PlaceRoot::Unknown,
                "provenance" => {
                    event.provenance = language_semantics::PermissionProvenance::Unknown
                }
                _ => unreachable!(),
            }
        }
        let rejected = crate::execution::terminal_unit::build_checked_unit_effect_plans(
            &checked.typed,
            &changed,
            crate::execution::terminal_unit::ScalarCalleePlans {
                boundary_returns: &changed.flow.terminal_boundary_scalar_returns,
                structural_returns: &changed.flow.terminal_structural_scalar_returns,
            },
            &[],
            &[],
        );
        assert!(
            rejected
                .composed_for_machine(machine_named(&checked, "read_one"))
                .is_none(),
            "case result cleanup must be present and retain its exact source root"
        );
    }
}

#[test]
fn retains_arm_local_boundary_result_discard_on_each_closed_sum_return() {
    let checked = checked(
        r#"
        data ByteRead { case Eof; case Byte(value: i32); }
        boundary trait Console {
            machine read_byte() -> ByteRead reaches Console;
            machine write_byte(value: i32) reaches Console;
        }
        data Main { console: Console; }
        machine Main::main(&mut self) reaches Console {
            let first: ByteRead = self.console.read_byte();
            transition first {
                ByteRead::Byte { value } -> byte(value)
                ByteRead::Eof -> eof()
            }
            state byte(&mut self, value: i32) {
                self.console.write_byte(value);
                let second: ByteRead = self.console.read_byte();
            }
            state eof(&mut self) {
                let second: ByteRead = self.console.read_byte();
            }
        }
    "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "main"))
        .expect("each returning sum arm retains its local boundary result");
    assert_eq!(plan.states.len(), 3);
    for (state, statement_index) in plan.states[1..].iter().zip([1, 0]) {
        assert!(matches!(
            state.terminator,
            CheckedComposedUnitControlTerminatorPlan::ReturnUnit
        ));
        let reads = state
            .operations
            .iter()
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    result,
                    discard_result_on_return,
                    ..
                } => Some((result, discard_result_on_return)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [(result, discard)] = reads.as_slice() else {
            panic!("one arm-local read")
        };
        assert!(**discard);
        assert_eq!(result.statement_index, statement_index);
        assert_eq!(
            result.binding_ordinal, 0,
            "result ordinals are state-local, not preceding-call counts"
        );
    }
}

#[test]
fn specializes_one_provider_backed_attachment_field_into_exact_boundary_requirements() {
    let checked = checked(
        r#"
        boundary trait Console {
            machine write_line(text: &[u8])
            reaches Console;
            machine exit_process(return_code: i32)
            reaches Console;
        }

        data Main { console: Console; }
        machine Main::main(&mut self)
        reaches Console
        {
            self.console.write_line("Hello");
            self.console.exit_process(0);
        }
        "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let main = plans
        .for_machine(machine_named(&checked, "main"))
        .expect("provider-backed Main should retain a checked Unit plan");
    assert!(main.structural_parameters.is_empty());
    assert_eq!(main.provider_attachment_requirements.len(), 2);
    assert!(
        main.provider_attachment_requirements
            .iter()
            .all(|requirement| requirement.field_identity == "console")
    );
    let attachment = plans
        .structural_types
        .iter()
        .find(|shape| Some(shape.identity.as_str()) == main.attachment_type_identity.as_deref())
        .expect("Main attachment shape");
    let CheckedUnitStructuralTypeShape::Record { fields } = &attachment.shape else {
        panic!("Main should remain a record")
    };
    assert!(matches!(
        fields.as_slice(),
        [field]
            if field.identity == "console"
                && matches!(field.field_type,
                    CheckedUnitStructuralFieldType::ProviderBacked { .. })
    ));
}

#[test]
fn composes_conditional_unit_control_with_exact_boundary_call_leaves() {
    let checked = checked(
        r#"
        boundary trait Host { machine exit(code: i32); }
        data Root {}
        machine Root::enter(flag: bool) reaches Host {
            transition flag { true -> yes() _ -> no() }
            state yes() { Host::exit(1); }
            state no() { Host::exit(2); }
        }
        "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let machine = plans
        .composed_for_machine(machine_named(&checked, "enter"))
        .expect("three-state Unit control and boundary effects compose atomically");
    assert!(plans.for_machine(machine.machine).is_none());
    let [entry, when_true, when_false] = machine.states.as_slice() else {
        panic!("composed Unit plan retains exactly three states")
    };
    assert!(entry.operations.is_empty());
    assert!(matches!(
        entry.terminator,
        checked_trees::CheckedComposedUnitControlTerminatorPlan::Conditional { .. }
    ));
    for leaf in [when_true, when_false] {
        assert!(matches!(
            leaf.operations.as_slice(),
            [CheckedUnitEffectOperationPlan::BoundaryCall { .. }]
        ));
        assert!(matches!(
            leaf.terminator,
            checked_trees::CheckedComposedUnitControlTerminatorPlan::ReturnUnit
        ));
    }
}

#[test]
fn composes_closed_guard_with_one_provider_backed_attachment() {
    let checked = checked(
        r#"
        boundary trait Console {
            machine exit_process(return_code: i32)
            reaches Console;
        }
        const PAGE_SIZE: u32 = 64;
        data Main { console: Console; }
        machine Main::main(&mut self)
        reaches Console
        {
            transition PAGE_SIZE == 64 { true -> yes() _ -> no() }
            state yes(&mut self) { self.console.exit_process(70); }
            state no(&mut self) { self.console.exit_process(71); }
        }
        "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let machine = plans
        .composed_for_machine(machine_named(&checked, "main"))
        .expect("closed guard and provider attachment compose atomically");
    assert!(machine.states[0].scalar_parameters.is_empty());
    assert_eq!(machine.provider_attachment_requirements.len(), 1);
    assert_eq!(
        machine.provider_attachment_requirements[0].field_identity,
        "console"
    );
    assert!(matches!(
        machine.states[0].terminator,
        checked_trees::CheckedComposedUnitControlTerminatorPlan::Conditional { .. }
    ));
}

#[test]
fn composes_one_compile_known_u64_binding_with_exact_boundary_leaves() {
    let checked = checked(
        r#"
        boundary trait Host { machine exit(code: i32); }
        data Root { values: [i32; 5]; }
        machine Root::enter(&mut self) reaches Host {
            let length: u64 = (self.values[1..4]).len;
            transition length == 3 {
                true -> yes()
                false -> no()
            }
            state yes(&mut self) { Host::exit(1); }
            state no(&mut self) { Host::exit(2); }
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "enter"))
        .expect("one compile-known scalar local should compose with the exact control graph");
    let [entry, when_true, when_false] = plan.states.as_slice() else {
        panic!("one-local composed Unit plan retains three states")
    };
    assert!(matches!(
        entry.bindings.as_slice(),
        [CheckedScalarBinding {
            destination: checked_trees::CheckedScalarBindingDestination::Immutable,
            statement_ordinal: 0,
            primitive_type: PrimitiveType::U64,
            value: CheckedScalarBindingValue::Expression,
        }]
    ));
    assert!(matches!(
        entry.binding_initializers.as_slice(),
        [CheckedScalarExpression::IntegerLiteral { literal }] if literal.value_u64() == Some(3)
    ));
    let CheckedComposedUnitControlTerminatorPlan::Conditional {
        when_true: true_edge,
        when_false: false_edge,
        ..
    } = &entry.terminator
    else {
        panic!("one-local entry remains conditional")
    };
    assert_eq!(true_edge.statement_ordinal, 1);
    assert_eq!(false_edge.statement_ordinal, 2);
    for leaf in [when_true, when_false] {
        assert!(leaf.binding_initializers.is_empty());
        assert!(matches!(
            leaf.operations.as_slice(),
            [CheckedUnitEffectOperationPlan::BoundaryCall { .. }]
        ));
    }
}

#[test]
fn rejects_the_whole_composed_control_plan_when_one_leaf_loses_scalar_evidence() {
    let checked = checked(
        r#"
        boundary trait Host { machine exit(code: i32); }
        data Helper {}
        machine Helper::touch() {}
        data Root {}
        machine Root::enter(flag: bool) reaches Host {
            transition flag { true -> yes() _ -> no() }
            state yes() { Host::exit(1); }
            state no() { Helper::touch(); let local: u8 = 1u8; }
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "enter"))
        .expect("boundary leaf and call followed by local compose");
    assert!(matches!(
        plan.states[1].operations.as_slice(),
        [CheckedUnitEffectOperationPlan::BoundaryCall { .. }]
    ));
    let leaf = &plan.states[2];
    assert!(matches!(leaf.operations.as_slice(), [
        CheckedUnitEffectOperationPlan::CallUnit { coordinate, .. },
        CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, value: checked_trees::CheckedCallScalarArgument::Pure(_), .. },
    ] if coordinate.statement_index == 0 && result.statement_index == 1 && result.binding_ordinal == 0));
    let mut facts = checked.facts.clone();
    let before = facts.values.scalar_expressions.expressions.len();
    facts
        .values
        .scalar_expressions
        .expressions
        .retain(|expression| {
            expression.state != leaf.state
                || expression.statement_ordinal != 1
                || expression.role
                    != CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 }
        });
    assert_eq!(
        facts.values.scalar_expressions.expressions.len() + 1,
        before
    );
    let plans = crate::execution::terminal_unit::build_checked_unit_effect_plans(
        &checked.typed,
        &facts,
        crate::execution::terminal_unit::ScalarCalleePlans {
            boundary_returns: &facts.flow.terminal_boundary_scalar_returns,
            structural_returns: &facts.flow.terminal_structural_scalar_returns,
        },
        &[],
        &[],
    );
    assert!(
        plans
            .composed_for_machine(machine_named(&checked, "enter"))
            .is_none(),
        "one leaf missing required scalar evidence must remove the composed plan atomically"
    );
    assert!(
        plans
            .for_machine(machine_named(&checked, "touch"))
            .is_some()
    );
}

#[test]
fn retains_multiple_calls_in_a_composed_leaf_beside_a_boundary_leaf() {
    let checked = checked(
        r#"
        boundary trait Host { machine exit(code: i32); }
        data Helper {}
        machine Helper::touch() {}
        data Root {}
        machine Root::enter(flag: bool) reaches Host {
            transition flag { true -> yes() _ -> no() }
            state yes() { Host::exit(1); }
            state no() { Helper::touch(); Helper::touch(); }
        }
        "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let plan = plans
        .composed_for_machine(machine_named(&checked, "enter"))
        .expect("both authored call-only leaves are supported");
    let [_, when_true, when_false] = plan.states.as_slice() else {
        panic!("all three authored states are retained");
    };
    assert!(matches!(
        when_true.operations.as_slice(),
        [CheckedUnitEffectOperationPlan::BoundaryCall { .. }]
    ));
    let touch = machine_named(&checked, "touch");
    assert!(plans.for_machine(touch).is_some());
    assert_eq!(when_false.operations.len(), 2);
    for (ordinal, operation) in when_false.operations.iter().enumerate() {
        assert!(matches!(operation,
            CheckedUnitEffectOperationPlan::CallUnit { coordinate, target_machine, .. }
                if *target_machine == touch && coordinate.statement_index as usize == ordinal));
    }
}

#[test]
fn provider_attachment_specialization_rejects_ambiguous_or_unrouted_fields() {
    for source in [
        r#"
        boundary trait Console { machine exit_process(return_code: i32) reaches Console; }
        data Main { console: Console; backup: Console; }
        machine Main::main(&mut self) reaches Console {
            self.console.exit_process(0);
        }
        "#,
        r#"
        boundary trait Console { machine exit_process(return_code: i32) reaches Console; }
        data Main { console: Console; }
        machine Main::main(&mut self) reaches Console {
            Console::exit_process(0);
        }
        "#,
    ] {
        let checked = checked(source);
        assert!(
            checked
                .facts
                .flow
                .terminal_unit_effects
                .for_machine(machine_named(&checked, "main"))
                .is_none(),
            "unsupported provider-backed attachment shapes must fail closed"
        );
    }
}

#[test]
fn unused_provider_attachment_has_an_empty_requirement_set() {
    let checked = checked(
        r#"
        boundary trait Console { machine exit_process(return_code: i32) reaches Console; }
        data Main { console: Console; }
        machine Main::main(&mut self) {}
    "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "main"))
        .expect("an unused provider field still has a checked machine plan");
    assert!(plan.provider_attachment_requirements.is_empty());
    assert_eq!(
        plan.attachment_type_identity.as_deref(),
        Some("named(name(Main))")
    );
}

#[test]
fn retains_shared_byte_sequence_forwarding_access() {
    let checked = checked(
        r#"
        boundary trait Console {
            machine write_line(text: &[u8])
            reaches Console;
        }

        data Root {}
        machine Root::enter(text: &[u8])
        reaches Console
        {
            Console::write_line(text);
        }
        "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let enter = plans
        .for_machine(machine_named(&checked, "enter"))
        .expect("shared byte-sequence forwarding plan");
    assert_eq!(
        enter.structural_parameters[0].access,
        checked_trees::CheckedStructuralAccess::SharedBorrow
    );
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        structural_arguments,
        ..
    } = &enter.operations[0]
    else {
        panic!("shared forwarding should retain its boundary call")
    };
    assert_eq!(
        structural_arguments[0].access,
        checked_trees::CheckedStructuralAccess::SharedBorrow
    );
}
