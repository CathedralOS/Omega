use crate::parser::parse_syntax_trees;
use source_files_to_tokens::Lexer;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::statement::StatementNode;

// Inspect the instruction payload while retaining the new lexical graph
// boundary. These legacy catalog cases each have one unlabelled block.
pub(super) fn instruction_statements(
    parsed: &syntax_trees::SyntaxTrees,
    statements: &[syntax_trees::statement::StatementHandle],
) -> Vec<syntax_trees::statement::StatementHandle> {
    let mut instructions = Vec::new();
    for handle in statements {
        match parsed.statements.statement(*handle) {
            StatementNode::Assembly(graph) => {
                let blocks = parsed
                    .statements
                    .assembly_blocks(parsed.statements.assembly(*graph).blocks);
                assert_eq!(blocks.len(), 1, "one implicit catalog block");
                assert!(blocks[0].requires.is_empty() && blocks[0].ensures.is_empty());
                instructions.extend_from_slice(
                    parsed
                        .statements
                        .assembly_instructions(blocks[0].instructions),
                );
            }
            _ => instructions.push(*handle),
        }
    }
    instructions
}

#[test]
fn parses_x86_memory_fences_as_zero_operand_intrinsics() {
    let source = r#"
        data Main {}

        machine Main::main(main: &mut Main) {
            asm where clobbers none { lfence; sfence; mfence }
        }
        "#;

    let parsed = super::parse_source(source, "parse should succeed");
    let machine = super::root_machine(&parsed, "machine root item");
    let entry = parsed.items.state_handles(machine.states)[0];
    let statements = instruction_statements(
        &parsed,
        parsed
            .items
            .statements(parsed.items.state(entry).statements),
    );

    assert_eq!(statements.len(), 3);
    for (statement, target) in statements
        .iter()
        .zip(["asm#lfence", "asm#sfence", "asm#mfence"])
    {
        let StatementNode::Call(call) = parsed.statements.statement(*statement) else {
            panic!("fence should desugar to a call statement");
        };
        assert_eq!(call.target.as_str(), target);
        assert!(call.receiver.is_empty());
        assert_eq!(call.arguments.count(), 0);
    }
}

#[test]
fn parses_x86_interrupt_control_as_zero_operand_intrinsics() {
    let source = r#"
        data Main {}

        machine Main::main(main: &mut Main) reaches MachineControl {
            asm where clobbers none { cli; sti }
        }
        "#;

    let parsed = super::parse_source(source, "parse should succeed");
    let machine = super::root_machine(&parsed, "machine root item");
    let entry = parsed.items.state_handles(machine.states)[0];
    let statements = instruction_statements(
        &parsed,
        parsed
            .items
            .statements(parsed.items.state(entry).statements),
    );

    assert_eq!(statements.len(), 2);
    for (statement, target) in statements.iter().zip(["asm#cli", "asm#sti"]) {
        let StatementNode::Call(call) = parsed.statements.statement(*statement) else {
            panic!("interrupt control should desugar to a call statement");
        };
        assert_eq!(call.target.as_str(), target);
        assert!(call.receiver.is_empty());
        assert_eq!(call.arguments.count(), 0);
    }
}

#[test]
fn parses_x86_flags_as_explicit_value_operations() {
    let source = r#"
        data Main { saved: u64; }

        machine Main::main(main: &mut Main) reaches MachineControl {
            asm where clobbers r10, r15 {
                pushfq main.saved;
                popfq main.saved
            }
        }
        "#;

    let parsed = super::parse_source(source, "parse should succeed");
    let machine = super::root_machine(&parsed, "machine root item");
    let state = parsed
        .items
        .state(parsed.items.state_handles(machine.states)[0]);
    let statements = instruction_statements(&parsed, parsed.items.statements(state.statements));
    assert_eq!(statements.len(), 2);

    let StatementNode::Assignment(snapshot) = parsed.statements.statement(statements[0]) else {
        panic!("pushfq should desugar to a destination assignment");
    };
    let ExpressionNode::Call(call) = parsed.expressions.expression(snapshot.value) else {
        panic!("pushfq assignment should contain the snapshot intrinsic");
    };
    assert_eq!(call.target.as_str(), "asm#pushfq");
    assert_eq!(call.arguments.count(), 0);

    let StatementNode::Call(restore) = parsed.statements.statement(statements[1]) else {
        panic!("popfq should desugar to a call statement");
    };
    assert_eq!(restore.target.as_str(), "asm#popfq");
    assert_eq!(restore.arguments.count(), 1);
}

#[test]
fn parses_x86_msr_as_structured_value_operations() {
    let source = r#"
        data Main { value: u64; }

        machine Main::main(main: &mut Main) reaches MachineControl {
            asm where clobbers rax, rcx, rdx, r10, r11, r15 {
                rdmsr main.value, 3221225600;
                wrmsr 3221225600, main.value
            }
        }
        "#;

    let parsed = super::parse_source(source, "parse should succeed");
    let machine = super::root_machine(&parsed, "machine root item");
    let state = parsed
        .items
        .state(parsed.items.state_handles(machine.states)[0]);
    let statements = instruction_statements(&parsed, parsed.items.statements(state.statements));
    assert_eq!(statements.len(), 2);

    let StatementNode::Assignment(read) = parsed.statements.statement(statements[0]) else {
        panic!("rdmsr should desugar to a destination assignment");
    };
    let ExpressionNode::Call(read_call) = parsed.expressions.expression(read.value) else {
        panic!("rdmsr assignment should contain the read intrinsic");
    };
    assert_eq!(read_call.target.as_str(), "asm#rdmsr");
    assert_eq!(read_call.arguments.count(), 1);

    let StatementNode::Call(write) = parsed.statements.statement(statements[1]) else {
        panic!("wrmsr should desugar to a call statement");
    };
    assert_eq!(write.target.as_str(), "asm#wrmsr");
    assert_eq!(write.arguments.count(), 2);
}

#[test]
fn parses_x86_control_registers_as_structured_value_operations() {
    let source = r#"
        data Main { value: u64; }

        machine Main::main(main: &mut Main) reaches MachineControl {
            asm where clobbers rax, r10, r11, r15 {
                read_cr0 main.value;
                write_cr0 main.value;
                read_cr2 main.value;
                read_cr3 main.value;
                write_cr3 main.value;
                read_cr4 main.value;
                write_cr4 main.value
            }
        }
        "#;

    let parsed = super::parse_source(source, "parse should succeed");
    let machine = super::root_machine(&parsed, "machine root item");
    let state = parsed
        .items
        .state(parsed.items.state_handles(machine.states)[0]);
    let statements = instruction_statements(&parsed, parsed.items.statements(state.statements));
    assert_eq!(statements.len(), 7);

    for (statement_index, target) in [
        (0, "asm#read_cr0"),
        (2, "asm#read_cr2"),
        (3, "asm#read_cr3"),
        (5, "asm#read_cr4"),
    ] {
        let StatementNode::Assignment(read) =
            parsed.statements.statement(statements[statement_index])
        else {
            panic!("control-register read should desugar to assignment");
        };
        let ExpressionNode::Call(call) = parsed.expressions.expression(read.value) else {
            panic!("control-register assignment should contain read intrinsic");
        };
        assert_eq!(call.target.as_str(), target);
        assert_eq!(call.arguments.count(), 0);
    }

    for (statement_index, target) in [
        (1, "asm#write_cr0"),
        (4, "asm#write_cr3"),
        (6, "asm#write_cr4"),
    ] {
        let StatementNode::Call(write) = parsed.statements.statement(statements[statement_index])
        else {
            panic!("control-register write should desugar to call");
        };
        assert_eq!(write.target.as_str(), target);
        assert_eq!(write.arguments.count(), 1);
    }
}

#[test]
fn parses_multi_instruction_asm_in_states_and_trait_defaults() {
    let source = r#"
        trait Idle<Processor> {
            machine idle(value: &mut Processor) {
                asm { hlt; hlt }
            }
        }

        data Main {}

        machine Main::main(main: &mut Main) {
            transition { _ -> next() }

            state next() {
                asm { hlt; hlt }
            }
        }
        "#;

    let parsed = super::parse_source(source, "parse should succeed");
    let trait_definition = super::root_trait(&parsed, "trait root item");
    let default_signature = parsed
        .items
        .state_signatures(trait_definition.machines)
        .first()
        .map(|handle| parsed.items.state_signature(*handle))
        .expect("default trait signature");
    assert_eq!(
        instruction_statements(
            &parsed,
            parsed.items.statements(default_signature.default_body)
        )
        .len(),
        2,
        "trait default should retain both asm instructions"
    );
    let machine = super::root_machine(&parsed, "machine root item");
    let explicit = parsed
        .items
        .state_handles(machine.states)
        .get(1)
        .copied()
        .expect("explicit state");
    assert_eq!(
        instruction_statements(
            &parsed,
            parsed
                .items
                .statements(parsed.items.state(explicit).statements)
        )
        .len(),
        2,
        "explicit state should retain both asm instructions"
    );
}

#[test]
fn rejects_ambiguous_or_empty_multi_instruction_asm_blocks() {
    for (block, expected) in [
        (
            "asm { out main.port, main.value hlt }",
            "multiple asm instructions must be separated by `;`",
        ),
        (
            "asm { jmp done(); hlt }",
            "an asm control transfer must be the final instruction",
        ),
        (
            "asm {}",
            "an asm block must contain at least one known instruction",
        ),
        (
            "asm where requires true {}",
            "an asm block must contain at least one known instruction",
        ),
    ] {
        let source = format!(
            r#"
            data Main {{
                port: u16;
                value: u8;
            }}

            machine Main::main(main: &mut Main) {{
                {block}
                state done() {{}}
            }}
            "#
        );
        let error = super::parse_source_error(
            &source,
            "tokenize should succeed",
            "asm block should reject",
        );
        let rendered = format!("{error:?}");
        assert!(
            rendered.contains(expected),
            "expected `{expected}`, got `{rendered}`"
        );
    }
}

#[test]
fn rejects_asm_availability_and_unmodeled_operation_classes() {
    for (instruction, expected) in [
        ("iretq", "deriver-only"),
        ("lidt main.value", "deriver-only"),
        ("ret", "creates a hidden control exit"),
        (
            "ldrb x0, main.value",
            "no structured operand provenance/permission contract",
        ),
        (
            "ldur x0, main.value",
            "no structured operand provenance/permission contract",
        ),
    ] {
        let source = format!(
            r#"
            data Main {{ value: i32; }}
            machine Main::main(main: &mut Main) {{ asm {{ {instruction} }} }}
            "#
        );
        let error = super::parse_source_error(
            &source,
            "tokenize should succeed",
            "asm instruction should reject",
        );
        let rendered = format!("{error:?}");
        assert!(
            rendered.contains(expected),
            "expected `{expected}` for `{instruction}`, got `{rendered}`"
        );
    }
}

/// `mov`/`movq` spell a structured data move: both operands are ordinary Omega
/// expressions and the instruction desugars to a checked assignment, exactly
/// like `jmp` desugars to a checked transition (catalog RegisterMove row).
#[test]
fn parses_register_move_as_an_ordinary_checked_assignment() {
    let source = r#"
        data Main {
            value: i32;
            buffer: [u64; 4];
        }

        machine Main::main(main: &mut Main) {
            let mut scratch: u64 = 0;
            asm {
                mov main.value, 3;
                movq scratch, main.buffer[1]
            }
        }
        "#;

    let parsed = super::parse_source(source, "register moves should parse");
    let machine = super::root_machine(&parsed, "machine root item");
    let entry = parsed.items.state_handles(machine.states)[0];
    let statements = instruction_statements(
        &parsed,
        parsed
            .items
            .statements(parsed.items.state(entry).statements),
    );

    assert_eq!(statements.len(), 3, "let + two desugared moves");
    let StatementNode::Assignment(first) = parsed.statements.statement(statements[1]) else {
        panic!("mov should desugar to an assignment");
    };
    assert!(matches!(
        parsed.expressions.expression(first.value),
        ExpressionNode::Integer(_)
    ));
    let StatementNode::Assignment(second) = parsed.statements.statement(statements[2]) else {
        panic!("movq should desugar to an assignment");
    };
    assert!(matches!(
        parsed.expressions.expression(second.value),
        ExpressionNode::Indexed(_)
    ));
}

/// The contracted memory transfers spell their memory operand as a typed
/// Omega place: `ldr <dest>, <place>` desugars to `<dest> = <place>` and
/// `str <value>, <place>` to `<place> = <value>` — the place carries the
/// provenance/permission contract (catalog MemoryTransfer rows).
#[test]
fn parses_memory_transfers_as_place_assignments() {
    let source = r#"
        data Main {
            value: u64;
            buffer: [u64; 4];
        }

        machine Main::main(main: &mut Main) {
            asm {
                str main.value, main.buffer[0];
                ldr main.value, main.buffer[0]
            }
        }
        "#;

    let parsed = super::parse_source(source, "memory transfers should parse");
    let machine = super::root_machine(&parsed, "machine root item");
    let entry = parsed.items.state_handles(machine.states)[0];
    let statements = instruction_statements(
        &parsed,
        parsed
            .items
            .statements(parsed.items.state(entry).statements),
    );

    assert_eq!(statements.len(), 2, "two desugared transfers");
    // `str value, place` writes the value INTO the place: the place is the
    // assignment target, the spelled value operand the source.
    let StatementNode::Assignment(store) = parsed.statements.statement(statements[0]) else {
        panic!("str should desugar to an assignment");
    };
    assert!(matches!(
        parsed.expressions.expression(store.target),
        ExpressionNode::Indexed(_)
    ));
    // `ldr dest, place` reads the place INTO the destination.
    let StatementNode::Assignment(load) = parsed.statements.statement(statements[1]) else {
        panic!("ldr should desugar to an assignment");
    };
    assert!(matches!(
        parsed.expressions.expression(load.value),
        ExpressionNode::Indexed(_)
    ));

    for instruction in [
        "ldr main.value, [main.buffer]",
        "str [main.buffer], main.value",
    ] {
        let source = format!(
            r#"
            data Main {{ value: u64; buffer: [u64; 4]; }}
            machine Main::main(main: &mut Main) {{ asm {{ {instruction} }} }}
            "#
        );
        let error = super::parse_source_error(
            &source,
            "tokenize should succeed",
            "a bracketed transfer operand keeps the bracket refusal",
        );
        let rendered = format!("{error:?}");
        assert!(
            rendered.contains("bracketed `[...]` memory addressing"),
            "expected bracketed-operand refusal for `{instruction}`, got `{rendered}`"
        );
    }
}

/// The move contract covers place/value operands only: a bracketed `[address]`
/// operand is raw memory addressing and keeps the unmodeled-memory refusal.
#[test]
fn rejects_bracketed_memory_operands_on_register_moves() {
    for instruction in [
        "mov main.value, [main.other]",
        "mov [main.value], main.other",
        "movq main.value, [main.buffer]",
    ] {
        let source = format!(
            r#"
            data Main {{ value: i32; other: i32; buffer: [u64; 4]; }}
            machine Main::main(main: &mut Main) {{ asm {{ {instruction} }} }}
            "#
        );
        let error = super::parse_source_error(
            &source,
            "tokenize should succeed",
            "bracketed operand should reject",
        );
        let rendered = format!("{error:?}");
        assert!(
            rendered.contains("bracketed `[...]` memory addressing"),
            "expected bracketed-memory diagnostic for `{instruction}`, got `{rendered}`"
        );
    }
}

#[test]
fn parses_exact_asm_where_clobber_contracts() {
    for block in [
        "asm where clobbers none { hlt }",
        "asm where clobbers r11, rax, rdx, r10, r15 { out main.port, main.value }",
    ] {
        let source = format!(
            r#"
            data Main {{ port: u16; value: u8; }}
            machine Main::main(main: &mut Main) {{ {block} }}
            "#
        );
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize should succeed");
        parse_syntax_trees(&tokens).expect("exact asm clobber contract should parse");
    }
}

#[test]
fn parses_asm_where_facts_at_entry_and_exit() {
    let source = r#"
        data Main { port: u16; value: u8; ready: bool; }
        machine Main::main(main: &mut Main) {
            asm where
                requires main.ready
                clobbers rax, rdx, r10, r11, r15
                ensures main.ready
            { out main.port, main.value }
        }
    "#;
    let parsed = super::parse_source_verbose(source, "tokenize", "asm facts should parse");
    let machine = super::root_machine(&parsed, "machine");
    let state = parsed
        .items
        .state(parsed.items.state_handles(machine.states)[0]);
    let statements = parsed.items.statements(state.statements);
    assert_eq!(statements.len(), 1);
    let StatementNode::Assembly(graph) = parsed.statements.statement(statements[0]) else {
        panic!("one structured assembly scope");
    };
    let blocks = parsed
        .statements
        .assembly_blocks(parsed.statements.assembly(*graph).blocks);
    assert_eq!(blocks.len(), 1);
    let block = &blocks[0];
    assert_eq!(block.requires.count(), 1);
    assert_eq!(block.ensures.count(), 1);
    for expression in parsed
        .statements
        .assembly_expressions(block.requires)
        .iter()
        .chain(parsed.statements.assembly_expressions(block.ensures))
    {
        assert!(matches!(
            parsed.expressions.expression(*expression),
            ExpressionNode::Member(_)
        ));
    }
    let instructions = parsed.statements.assembly_instructions(block.instructions);
    assert_eq!(instructions.len(), 1);
    assert!(matches!(parsed.statements.statement(instructions[0]),
        StatementNode::Call(call) if call.target.as_str() == "asm#port_out"));
}

#[test]
fn rejects_inexact_asm_where_clobber_contracts() {
    for (block, expected) in [
        (
            "asm where clobbers rax, rdx, r10 { out main.port, main.value }",
            "missing `r11`",
        ),
        ("asm where clobbers rax { hlt }", "not clobbered `rax`"),
        ("asm where clobbers { hlt }", "spell `clobbers none`"),
        (
            "asm where ensures true { hlt }",
            "requires a falling-through block",
        ),
    ] {
        let source = format!(
            r#"
            data Main {{ port: u16; value: u8; }}
            machine Main::main(main: &mut Main) {{ {block} }}
            "#
        );
        let error = super::parse_source_error(
            &source,
            "tokenize should succeed",
            "inexact asm contract should reject",
        );
        let rendered = format!("{error:?}");
        assert!(
            rendered.contains(expected),
            "expected `{expected}` for `{block}`, got `{rendered}`"
        );
    }
}

/// Opaque asm forms have no attributable contract; only known-contract
/// instructions compile (wiki/spec/build/hardware_materialization.md).
#[test]
fn rejects_unknown_asm_mnemonics() {
    for block in ["asm { db 0xF4 }", "asm { swapgs }"] {
        let source = format!(
            r#"
            data Main {{
                value: i32;
            }}

            machine Main::main(main: &mut Main) {{
                {block}
            }}
            "#
        );

        let error = super::parse_source_error(
            &source,
            "tokenize should succeed",
            "unknown mnemonic must not parse",
        );
        let message = format!("{error:?}");
        assert!(
            message.contains("only known-contract instructions compile"),
            "unexpected error for {block}: {message}"
        );
    }
}
