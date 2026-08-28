use super::*;

#[test]
fn accepts_proven_asm_entry_and_exit_facts() {
    let source = r#"
        data Main { port: u16; value: u8; ready: bool; }

        machine Main::main(&mut self) reaches PortIo
        requires self.ready
        {
            asm where
                requires self.ready
                clobbers rax, rdx, r10, r11, r15
                ensures self.ready
            { out self.port, self.value }
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("preserved entry fact should prove both asm assertions");
}

#[test]
fn rejects_unproven_asm_requires_at_block_entry() {
    let source = r#"
        data Main { port: u16; value: u8; ready: bool; }

        machine Main::main(&mut self) reaches PortIo {
            asm where
                requires self.ready
                clobbers rax, rdx, r10, r11, r15
            { out self.port, self.value }
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("unproven asm requires must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove asm `requires` fact at block entry")
            && diagnostic.message.contains("self.ready")
    }));
}

#[test]
fn rejects_unproven_asm_ensures_at_block_exit() {
    let source = r#"
        data Main { port: u16; value: u8; ready: bool; }

        machine Main::main(&mut self) reaches PortIo {
            asm where
                clobbers rax, rdx, r10, r11, r15
                ensures self.ready
            { out self.port, self.value }
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("authored asm ensures must be proved, not minted");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove asm `ensures` fact at block exit")
            && diagnostic.message.contains("self.ready")
    }));
}

#[test]
fn rejects_asm_ensures_invalidated_by_port_input() {
    let source = r#"
        data Main { port: u16; value: u8; }

        machine Main::main(&mut self) reaches PortIo
        requires self.value == 1
        {
            asm where
                requires self.value == 1
                clobbers rax, rdx, r10, r15
                ensures self.value == 1
            { in self.value, self.port }
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("port input must invalidate facts about its destination");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove asm `ensures` fact at block exit")
            && diagnostic.message.contains("self.value == 1")
    }));
}

#[test]
fn rejects_asm_ensures_invalidated_by_flags_snapshot() {
    let source = r#"
        data Main { saved: u64; }

        machine Main::main(&mut self)
        requires self.saved == 2
        {
            asm where
                requires self.saved == 2
                clobbers r10, r15
                ensures self.saved == 2
            { pushfq self.saved }
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("flags snapshot must invalidate facts about its destination");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove asm `ensures` fact at block exit")
            && diagnostic.message.contains("self.saved == 2")
    }));
}

#[test]
fn rejects_asm_ensures_invalidated_by_msr_read() {
    let source = r#"
        data Main { value: u64; }

        machine Main::main(&mut self) reaches MachineControl
        requires self.value == 2
        {
            asm where
                requires self.value == 2
                clobbers rax, rcx, rdx, r10, r11, r15
                ensures self.value == 2
            { rdmsr self.value, 3221225600 }
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("MSR read must invalidate facts about its destination");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove asm `ensures` fact at block exit")
            && diagnostic.message.contains("self.value == 2")
    }));
}

#[test]
fn preserves_asm_ensures_across_unrelated_port_input() {
    let source = r#"
        data Main { port: u16; value: u8; ready: bool; }

        machine Main::main(&mut self) reaches PortIo
        requires self.ready
        {
            asm where
                requires self.ready
                clobbers rax, rdx, r10, r15
                ensures self.ready
            { in self.value, self.port }
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("a write to value must not invalidate a fact about ready");
}

#[test]
fn rejects_non_boolean_asm_fact_place() {
    let source = r#"
        data Main { port: u16; value: u8; }

        machine Main::main(&mut self) reaches PortIo {
            asm where
                requires self.value
                clobbers rax, rdx, r10, r11, r15
            { out self.port, self.value }
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("numeric asm fact must reject before proof discharge");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("asm `requires` fact")
            && diagnostic.message.contains("is not boolean-shaped")
    }));
}

#[test]
fn canonical_asm_services_enter_normalized_reach_inference() {
    let source = r#"
        data Main { port: u16; value: u8; }

        machine Main::main(&mut self) reaches MachineControl + PortIo {
            asm { hlt; out self.port, self.value }
        }
    "#;

    let typed = parse_typed_trees(source);
    let operations = psi_effects::infer_operational_may(&typed);
    let reaches = psi_effects::infer_service_reaches(&typed, &operations);
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("main machine");
    let summary = reaches
        .for_machine(machine.symbol)
        .expect("service summary");
    let state = reaches
        .states_for(summary)
        .first()
        .expect("main state service summary");
    for name in ["MachineControl", "PortIo"] {
        let service = typed
            .service_reaches
            .id_for_name(name)
            .expect("canonical asm service");
        assert!(reaches.services(summary.inferred_direct).contains(&service));
        assert!(
            reaches
                .services(summary.inferred_transitive)
                .contains(&service)
        );
        assert!(reaches.services(state.inferred_direct).contains(&service));
        assert!(
            reaches
                .services(state.inferred_transitive)
                .contains(&service)
        );
    }
    let calls = reaches.calls_for(state);
    assert_eq!(calls.len(), 2);
    assert!(
        calls
            .iter()
            .all(|call| !reaches.services(call.inferred_direct).is_empty())
    );
}

#[test]
fn rejects_missing_direct_port_io_service_declaration() {
    let source = r#"
        data Main { port: u16; value: u8; }

        machine Main::main(&mut self) {
            asm { out self.port, self.value }
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("direct port assembly must publish PortIo");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("add `reaches PortIo`") })
    );
}

#[test]
fn rejects_missing_transitive_machine_control_service_declaration() {
    let source = r#"
        data Main {}

        machine Main::helper(&mut self) reaches MachineControl {
            asm { hlt }
        }

        machine Main::main(&mut self) {
            self.helper();
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("an asm service may not be laundered through a helper");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("reaches inline-assembly service `MachineControl`")
            && diagnostic
                .message
                .contains("call path to inline assembly for `MachineControl`")
    }));
}
