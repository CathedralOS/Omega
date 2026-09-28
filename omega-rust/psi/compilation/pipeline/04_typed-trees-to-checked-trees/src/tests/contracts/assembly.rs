use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;

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

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
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

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect_err("authored asm ensures must be proved, not minted");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove asm `ensures` fact at block exit")
            && diagnostic.message.contains("self.ready")
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

    let diagnostics = lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect_err("flags snapshot must invalidate facts about its destination");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot prove asm `ensures` fact at block exit")
            && diagnostic.message.contains("self.saved == 2")
    }));
}
