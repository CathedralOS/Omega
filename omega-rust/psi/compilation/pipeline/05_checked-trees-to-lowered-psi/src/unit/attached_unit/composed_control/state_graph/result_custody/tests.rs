use super::*;

#[test]
fn boundary_operands_retire_local_owners_with_exact_transfer_receipts() {
    for producer in ["Host::make(10)", "Packet { value: 10 }", "make(10)"] {
        let checked = crate::front_end::checked_program(&format!(
            r#"
            data Packet {{ value: u32; }}
            boundary trait Host {{
                machine make(value: u32) -> Packet;
                machine consume(value: Packet) -> u32;
            }}
            machine make(value: u32) -> Packet {{ Packet {{ value: value }} }}
            machine main() -> bool reaches Host {{
                let first: Packet = {producer};
                let second: Packet = Host::make(20);
                let first_value: u32 = Host::consume(first);
                let second_value: u32 = Host::consume(second);
                transition first_value == 10 && second_value == 20 {{
                    true -> yes()
                    false -> no()
                }}
                state yes() -> bool {{ true }}
                state no() -> bool {{ false }}
            }}
            "#
        ));
        let lowered = crate::lower_machine(&checked, crate::TerminalMachineSelection::Name("main"))
            .expect("boundary calls consume local owners independently of their producer");
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("boundary operand transfer verifies without an exit drop of moved storage");

        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "main")
            .expect("main");
        let state = &checked.machine_states(machine)[0];
        let StatementNode::LocalData(first) =
            &checked.statement_table.statements(state.statement_nodes)[0]
        else {
            panic!("first packet declaration");
        };
        let transfer = checked
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .find_map(|(handle, event)| {
                (event.machine_symbol == machine.symbol
                    && event.state_symbol == state.symbol
                    && event.root == facts::PlaceRoot::Symbol(first.symbol)
                    && event.kind == PermissionEventKind::Transfer)
                    .then_some(handle)
            })
            .expect("first packet's owned call transfer");
        for corruption in ["missing", "duplicate", "occurrence", "provenance"] {
            let mut forged = checked.clone();
            let permissions = &mut forged.facts.flow.ownership.permissions;
            match corruption {
                "missing" => {
                    permissions.get_mut(transfer).machine_symbol = Default::default();
                }
                "duplicate" => {
                    let event = permissions.get(transfer).clone();
                    permissions.insert(event);
                }
                "occurrence" => {
                    permissions.get_mut(transfer).source = PermissionEventSource::StateExit;
                }
                "provenance" => {
                    permissions.get_mut(transfer).provenance =
                        language_semantics::PermissionProvenance::Unknown;
                }
                _ => unreachable!(),
            }
            assert!(
                crate::lower_machine(&forged, crate::TerminalMachineSelection::Name("main"))
                    .is_err(),
                "boundary transfer with {corruption} evidence must not publish"
            );
        }
    }
}

#[test]
fn qualified_byte_disposition_replays_constructor_and_call_local_carriers() {
    let checked = crate::front_end::checked_program(
        r#"
        domain [u8; 16]::Utf8(bytes) requires valid_utf8(bytes);
        machine text() -> [u8; 16] in Utf8 { "Go!" }
        machine observe(bytes: &[u8]) -> bool {
            transition bytes == "Go!" { true -> true false -> false }
        }
        machine main() -> bool {
            transition { _ -> arrived() }
            state arrived() {
                let constructed: [u8; 16] in Utf8 = "Go" + "!";
                let first: bool = observe(constructed);
                let returned: [u8; 16] in Utf8 = text();
                let second: bool = observe(returned);
                first && second
            }
        }
        "#,
    );
    let lowered = crate::lower_machine(&checked, crate::TerminalMachineSelection::Name("main"))
        .expect("qualified locals keep canonical byte carriers after shared observation");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("original owner predicates and shared loan custody verify");
    // Shared presentation must not turn a forged completion signature into
    // evidence. Check the semantic module directly, before any proof envelope
    // identity could mask the receiving call-contract rejection.
    for corrupt_qualifications in [true, false] {
        let mut module = lowered.semantic_module.clone();
        let view_type = module
            .structural_types
            .iter()
            .find(|declaration| {
                matches!(
                    declaration.shape,
                    terminal_psi::StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                    )
                )
            })
            .expect("observer's view carrier")
            .id;
        let result = module
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
            .flat_map(|block| &mut block.operations)
            .find_map(|operation| {
                if !matches!(
                    operation.kind,
                    terminal_psi::OperationKind::CallStructural { .. }
                        | terminal_psi::OperationKind::CallStructuralWithScalarArguments { .. }
                ) {
                    return None;
                }
                match &mut operation.result {
                    terminal_psi::OperationResult::Structural(result)
                        if !result.qualifications.is_empty() =>
                    {
                        Some(result)
                    }
                    _ => None,
                }
            })
            .expect("qualified ordinary byte completion");
        if corrupt_qualifications {
            result.qualifications.clear();
        } else {
            result.structural_type = view_type;
        }
        assert!(
            terminal_verifier::validate_module(&module).is_err(),
            "a borrowed argument cannot legitimize altered call-result custody"
        );
    }
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "main")
        .expect("main");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .find(|plan| plan.machine == machine.symbol)
        .expect("composed main");
    let mut replayed = 0;
    for (state_index, state) in plan.states.iter().enumerate() {
        let authored = checked
            .machine_states(machine)
            .iter()
            .find(|authored| authored.symbol == state.state)
            .expect("authored state");
        let statements = checked.statement_table.statements(authored.statement_nodes);
        for (operation_index, operation) in state.operations.iter().enumerate() {
            let Some(result) = operation.structural_result_binding() else {
                continue;
            };
            let Some(StatementNode::LocalData(local)) =
                statements.get(result.statement_index as usize)
            else {
                continue;
            };
            if validation::bounded_byte_buffer_capacity(&checked.typed, local.type_reference)
                .is_none()
            {
                continue;
            }
            validate_disposition(&checked, machine.symbol, authored, local, result)
                .expect("exact local receipt");
            // Result types belong to the producer's independent replay, not
            // state-exit disposal. Corrupt the actual operation and exercise
            // that boundary for construction and call results alike.
            for corrupt_identity in [true, false] {
                let mut wrong = checked.clone();
                let body = wrong
                    .facts
                    .flow
                    .terminal_unit_effects
                    .composed_machines
                    .iter_mut()
                    .find(|plan| plan.machine == machine.symbol)
                    .expect("composed main");
                let binding = match &mut body.states[state_index].operations[operation_index] {
                    CheckedUnitEffectOperationPlan::EstablishByteSequence { result, .. } => result,
                    CheckedUnitEffectOperationPlan::Call(call) => {
                        call.structural_result_mut().expect("call result").0
                    }
                    operation => panic!("unexpected qualified-byte producer: {operation:?}"),
                };
                if corrupt_identity {
                    binding.type_identity.push_str("different-capacity");
                } else {
                    binding.multiplicity = Multiplicity::Affine;
                }
                assert!(
                    crate::lower_machine(&wrong, crate::TerminalMachineSelection::Name("main"))
                        .is_err(),
                    "forged producer identity or custody must not reach Terminal Psi"
                );
            }
            replayed += 1;
        }
    }
    assert_eq!(
        replayed, 2,
        "both constructor and ordinary-call results are replayed"
    );
}

#[test]
fn shared_record_disposition_uses_the_produced_carrier() {
    for body in [
        r#"
        machine check(root: &mut Root) -> bool {
            let plain: &Desc = &root.desc;
            transition plain.tag == root.expected {
                true -> done(root)
                false -> bad(root)
            }
            state done(root: &mut Root) { true }
            state bad(root: &mut Root) { false }
        }
        "#,
        r#"
        machine check(root: &mut Root) -> bool {
            transition { _ -> arrived(root) }
            state arrived(root: &mut Root) {
                let plain: &Desc = &root.desc;
                transition plain.tag == root.expected {
                    true -> done(root)
                    false -> bad(root)
                }
            }
            state done(root: &mut Root) { true }
            state bad(root: &mut Root) { false }
        }
        "#,
        r#"
        machine borrow(root: &Root) -> &Desc { &root.desc }
        machine check(root: &mut Root) -> bool {
            let plain: &Desc = borrow(root);
            transition plain.tag == root.expected {
                true -> done(root)
                false -> bad(root)
            }
            state done(root: &mut Root) { true }
            state bad(root: &mut Root) { false }
        }
        "#,
    ] {
        let source =
            format!("data Desc {{ tag: u32; }} data Root {{ desc: Desc; expected: u32; }} {body}");
        let checked = crate::front_end::checked_program(&source);
        let lowered =
            crate::lower_machine(&checked, crate::TerminalMachineSelection::Name("check"))
                .expect("shared aliases and returned carriers compose with owner transfers");
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("exact loan and source custody verify independently");
    }
}
