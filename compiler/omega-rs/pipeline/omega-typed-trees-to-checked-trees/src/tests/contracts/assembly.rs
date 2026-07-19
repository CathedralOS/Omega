use super::*;

#[test]
fn accepts_proven_asm_entry_and_exit_facts() {
    let source = r#"
        data Main { port: u16; value: u8; ready: bool; }

        machine Main::main(&mut self) effects device_io
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

        machine Main::main(&mut self) effects device_io {
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

        machine Main::main(&mut self) effects device_io {
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

        machine Main::main(&mut self) effects device_io
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
fn preserves_asm_ensures_across_unrelated_port_input() {
    let source = r#"
        data Main { port: u16; value: u8; ready: bool; }

        machine Main::main(&mut self) effects device_io
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

        machine Main::main(&mut self) effects device_io {
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
