use crate::frontend_fixture::{Lexer, parse_syntax_trees};
use crate::resolution::declarations::assembly;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::item::{Item, Machine};
use syntax_trees::statement::{AssemblyExit, StatementHandle, StatementNode};

fn prepare_source(source: &str) -> Result<SyntaxTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source).tokenize().expect("lex assembly source");
    let mut syntax = parse_syntax_trees(&tokens).expect("parse authored assembly");
    assembly::prepare(&mut syntax)?;
    Ok(syntax)
}

fn root_machine(syntax: &SyntaxTrees) -> &Machine {
    syntax
        .root_items()
        .find_map(|item| match item {
            Item::Machine(machine) => Some(machine),
            _ => None,
        })
        .expect("machine root item")
}

fn instruction_statements(
    parsed: &syntax_trees::SyntaxTrees,
    statements: &[StatementHandle],
) -> Vec<StatementHandle> {
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

fn main_statements(parsed: &syntax_trees::SyntaxTrees) -> Vec<StatementHandle> {
    let machine = root_machine(parsed);
    let entry = parsed.items.state_handles(machine.states)[0];
    let statements = parsed
        .items
        .statements(parsed.items.state(entry).statements);
    instruction_statements(parsed, statements)
}

#[test]
fn prepares_x86_memory_fences_as_zero_operand_intrinsics() {
    let source = r#"
        data Main {}

        machine Main::main(main: @mut Main) {
            asm clobbers none { lfence; sfence; mfence }
        }
        "#;

    let parsed = prepare_source(source).expect("prepare assembly instructions");
    let statements = main_statements(&parsed);

    assert_eq!(statements.len(), 3);
    for (statement, target) in statements
        .iter()
        .zip(["asm#lfence", "asm#sfence", "asm#mfence"])
    {
        let StatementNode::Call(call) = parsed.statements.statement(*statement) else {
            panic!("fence should desugar to a call statement");
        };
        assert_eq!(call.target.as_str(&parsed), target);
        assert!(call.receiver.is_empty());
        assert_eq!(call.arguments.count(), 0);
    }
}

#[test]
fn prepares_x86_interrupt_control_as_zero_operand_intrinsics() {
    let source = r#"
        data Main {}

        machine Main::main(main: @mut Main) reaches MachineControl {
            asm clobbers none { cli; sti }
        }
        "#;

    let parsed = prepare_source(source).expect("prepare assembly instructions");
    let statements = main_statements(&parsed);

    assert_eq!(statements.len(), 2);
    for (statement, target) in statements.iter().zip(["asm#cli", "asm#sti"]) {
        let StatementNode::Call(call) = parsed.statements.statement(*statement) else {
            panic!("interrupt control should desugar to a call statement");
        };
        assert_eq!(call.target.as_str(&parsed), target);
        assert!(call.receiver.is_empty());
        assert_eq!(call.arguments.count(), 0);
    }
}

#[test]
fn prepares_x86_flags_as_explicit_value_operations() {
    let source = r#"
        data Main { saved: u64; }

        machine Main::main(main: @mut Main) reaches MachineControl {
            asm clobbers r10, r15 {
                pushfq main.saved;
                popfq main.saved
            }
        }
        "#;

    let parsed = prepare_source(source).expect("prepare assembly instructions");
    let statements = main_statements(&parsed);
    assert_eq!(statements.len(), 2);

    let StatementNode::Assignment(snapshot) = parsed.statements.statement(statements[0]) else {
        panic!("pushfq should desugar to a destination assignment");
    };
    let ExpressionNode::Call(call) = parsed.expressions.expression(snapshot.value) else {
        panic!("pushfq assignment should contain the snapshot intrinsic");
    };
    assert_eq!(call.target.as_str(&parsed), "asm#pushfq");
    assert_eq!(call.arguments.count(), 0);

    let StatementNode::Call(restore) = parsed.statements.statement(statements[1]) else {
        panic!("popfq should desugar to a call statement");
    };
    assert_eq!(restore.target.as_str(&parsed), "asm#popfq");
    assert_eq!(restore.arguments.count(), 1);
}

#[test]
fn prepares_x86_msr_as_structured_value_operations() {
    let source = r#"
        data Main { value: u64; }

        machine Main::main(main: @mut Main) reaches MachineControl {
            asm clobbers rax, rcx, rdx, r10, r11, r15 {
                rdmsr main.value, 3221225600;
                wrmsr 3221225600, main.value
            }
        }
        "#;

    let parsed = prepare_source(source).expect("prepare assembly instructions");
    let statements = main_statements(&parsed);
    assert_eq!(statements.len(), 2);

    let StatementNode::Assignment(read) = parsed.statements.statement(statements[0]) else {
        panic!("rdmsr should desugar to a destination assignment");
    };
    let ExpressionNode::Call(read_call) = parsed.expressions.expression(read.value) else {
        panic!("rdmsr assignment should contain the read intrinsic");
    };
    assert_eq!(read_call.target.as_str(&parsed), "asm#rdmsr");
    assert_eq!(read_call.arguments.count(), 1);

    let StatementNode::Call(write) = parsed.statements.statement(statements[1]) else {
        panic!("wrmsr should desugar to a call statement");
    };
    assert_eq!(write.target.as_str(&parsed), "asm#wrmsr");
    assert_eq!(write.arguments.count(), 2);
}

#[test]
fn prepares_x86_control_registers_as_structured_value_operations() {
    let source = r#"
        data Main { value: u64; }

        machine Main::main(main: @mut Main) reaches MachineControl {
            asm clobbers rax, r10, r11, r15 {
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

    let parsed = prepare_source(source).expect("prepare assembly instructions");
    let statements = main_statements(&parsed);
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
        assert_eq!(call.target.as_str(&parsed), target);
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
        assert_eq!(write.target.as_str(&parsed), target);
        assert_eq!(write.arguments.count(), 1);
    }
}

#[test]
fn prepares_multi_instruction_asm_in_nested_machines_and_trait_defaults() {
    let source = r#"
        trait Idle<Processor> {
            machine idle(value: @mut Processor) {
                asm { hlt; hlt }
            }
        }

        data Main {}

        machine Main::main(main: @mut Main) {
            transition { _ -> next() }

            machine next() {
                asm { hlt; hlt }
            }
        }
        "#;

    let parsed = prepare_source(source).expect("prepare assembly instructions");
    let trait_definition = parsed
        .root_items()
        .find_map(|item| match item {
            Item::Trait(definition) => Some(definition),
            _ => None,
        })
        .expect("trait root item");
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
    let machine = root_machine(&parsed);
    let child = machine
        .nested_machines
        .handles()
        .find_map(|handle| match parsed.items.item(handle) {
            syntax_trees::item::Item::Machine(child) => Some(child),
            _ => None,
        })
        .expect("nested machine");
    let explicit = parsed.items.state_handles(child.states)[0];
    assert_eq!(
        instruction_statements(
            &parsed,
            parsed
                .items
                .statements(parsed.items.state(explicit).statements)
        )
        .len(),
        2,
        "nested machine should retain both asm instructions"
    );
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
            machine Main::main(main: @mut Main) {{ asm {{ {instruction} }} }}
            "#
        );
        let error = prepare_source(&source).expect_err("asm instruction should reject");
        let rendered = format!("{error:?}");
        assert!(
            rendered.contains(expected),
            "expected `{expected}` for `{instruction}`, got `{rendered}`"
        );
    }
}

#[test]
fn prepares_register_move_as_an_ordinary_checked_assignment() {
    let source = r#"
        data Main {
            value: i32;
            buffer: [u64; 4];
        }

        machine Main::main(main: @mut Main) {
            let mut scratch: u64 = 0;
            asm {
                mov main.value, 3;
                movq scratch, main.buffer[1]
            }
        }
        "#;

    let parsed = prepare_source(source).expect("prepare assembly instructions");
    let statements = main_statements(&parsed);

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

#[test]
fn each_assembly_owns_its_register_bindings_and_preserves_adjacent_locals() {
    let source = r#"
        machine main() {
            let mut eax: u32 = 0;
            asm { mov eax, 1 }
            asm { mov eax, 2 }
            eax = 3;
        }
    "#;
    let tree = prepare_source(source).expect("prepare both register scopes");
    let mut declarations = Vec::new();
    let mut assignments = Vec::new();
    for statement in main_statements(&tree) {
        match tree.statements.statement(statement) {
            StatementNode::LocalData(local) => declarations.push(local.name.as_str(&tree)),
            StatementNode::Assignment(assignment) => {
                let ExpressionNode::Name { members, .. } =
                    tree.expressions.expression(assignment.target)
                else {
                    panic!("assignment to a single name");
                };
                let [name] = tree.expressions.identifier_path_members(*members) else {
                    panic!("assignment to a single name");
                };
                assignments.push(name.as_str(&tree));
            }
            other => panic!("unexpected statement {other:?}"),
        }
    }
    let [outer, first, second] = declarations.as_slice() else {
        panic!("expected an ordinary local and two private registers: {declarations:?}");
    };
    assert_eq!(*outer, "eax");
    assert_ne!(first, outer);
    assert_ne!(second, outer);
    assert_ne!(first, second);
    assert_eq!(assignments, [*first, *second, *outer]);
}

#[test]
fn prepares_memory_transfers_as_place_assignments() {
    let source = r#"
        data Main {
            value: u64;
            buffer: [u64; 4];
        }

        machine Main::main(main: @mut Main) {
            asm {
                str main.value, main.buffer[0];
                ldr main.value, main.buffer[0]
            }
        }
        "#;

    let parsed = prepare_source(source).expect("prepare assembly instructions");
    let statements = main_statements(&parsed);

    assert_eq!(statements.len(), 2, "two desugared transfers");
    let StatementNode::Assignment(store) = parsed.statements.statement(statements[0]) else {
        panic!("str should desugar to an assignment");
    };
    assert!(matches!(
        parsed.expressions.expression(store.target),
        ExpressionNode::Indexed(_)
    ));
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
            machine Main::main(main: @mut Main) {{ asm {{ {instruction} }} }}
            "#
        );
        let error = prepare_source(&source)
            .expect_err("a bracketed transfer operand keeps the bracket refusal");
        let rendered = format!("{error:?}");
        assert!(
            rendered.contains("bracketed `[...]` memory addressing"),
            "expected bracketed-operand refusal for `{instruction}`, got `{rendered}`"
        );
    }
}

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
            machine Main::main(main: @mut Main) {{ asm {{ {instruction} }} }}
            "#
        );
        let error = prepare_source(&source).expect_err("bracketed operand should reject");
        let rendered = format!("{error:?}");
        assert!(
            rendered.contains("bracketed `[...]` memory addressing"),
            "expected bracketed-memory diagnostic for `{instruction}`, got `{rendered}`"
        );
    }
}

#[test]
fn prepares_exact_asm_clobber_contracts() {
    for block in [
        "asm clobbers none { hlt }",
        "asm clobbers r11, rax, rdx, r10, r15 { out main.port, main.value }",
    ] {
        let source = format!(
            r#"
            data Main {{ port: u16; value: u8; }}
            machine Main::main(main: @mut Main) {{ {block} }}
            "#
        );
        prepare_source(&source).expect("prepare exact assembly clobbers");
    }
}

#[test]
fn prepares_asm_requires_facts_at_entry_and_exit() {
    let source = r#"
        data Main { port: u16; value: u8; ready: bool; }
        machine Main::main(main: @mut Main) {
            asm
                requires main.ready
                clobbers rax, rdx, r10, r11, r15
                ensures main.ready
            { out main.port, main.value }
        }
    "#;
    let parsed = prepare_source(source).expect("prepare assembly facts");
    let machine = root_machine(&parsed);
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
        StatementNode::Call(call) if call.target.as_str(&parsed) == "asm#port_out"));
}

#[test]
fn rejects_unknown_asm_mnemonics() {
    for block in ["asm { db 0xF4 }", "asm { swapgs }"] {
        let source = format!(
            r#"
            data Main {{
                value: i32;
            }}

            machine Main::main(main: @mut Main) {{
                {block}
            }}
            "#
        );

        let error = prepare_source(&source)
            .expect_err("unknown mnemonic must be refused by the instruction catalog");
        let message = format!("{error:?}");
        assert!(
            message.contains("only known-contract instructions compile"),
            "unexpected error for {block}: {message}"
        );
    }
}

#[test]
fn prepares_asm_jmp_block_as_transition_statement() {
    let source = r#"
        data Main {
            value: i32;
        }

        machine Main::main(main: @mut Main) {
            asm {
                jmp other()
            }

            machine other() {}
        }
        "#;

    let parsed = prepare_source(source).expect("prepare assembly instructions");
    let machine = root_machine(&parsed);
    let entry = parsed.items.state_handles(machine.states)[0];
    let state = parsed.items.state(entry);
    let instructions = instruction_statements(&parsed, parsed.items.statements(state.statements));
    let statement = *instructions.first().expect("asm transition");
    assert!(matches!(
        parsed.statements.statement(statement),
        StatementNode::Transition(_)
    ));
}

#[test]
fn jump_label_can_share_the_spelling_of_an_unsupported_register_view() {
    let tree = prepare_source("machine main() { asm { jmp al; al: lfence } }")
        .expect("label names are independent of register aliases");
    let machine = root_machine(&tree);
    let state = tree.items.state_handles(machine.states)[0];
    let [statement] = tree.items.statements(tree.items.state(state).statements) else {
        panic!("one assembly graph and no scratch bindings");
    };
    let StatementNode::Assembly(graph) = tree.statements.statement(*statement) else {
        panic!("prepared assembly");
    };
    let [entry, target] = tree
        .statements
        .assembly_blocks(tree.statements.assembly(*graph).blocks)
    else {
        panic!("two labeled basic blocks");
    };
    let AssemblyExit::Jump(label) = &entry.exit else {
        panic!("local jump");
    };
    assert_eq!(label.as_str(&tree), "al");
    assert_eq!(target.label.as_str(&tree), label.as_str(&tree));
}

#[test]
fn prepares_asm_mnemonics_as_intrinsic_calls() {
    let source = r#"
        data Main {
            port: u16;
        }

        machine Main::main(main: @mut Main) {
            let mut status: u8 = 0;
            asm { hlt }
            asm { out main.port, status }
            asm { in status, main.port }
        }
        "#;

    let parsed = prepare_source(source).expect("prepare assembly instructions");
    let machine = root_machine(&parsed);
    let entry = parsed.items.state_handles(machine.states)[0];
    let state = parsed.items.state(entry);
    let statements = instruction_statements(&parsed, parsed.items.statements(state.statements));
    assert_eq!(statements.len(), 4, "let + three asm statements");

    let StatementNode::Call(hlt) = parsed.statements.statement(statements[1]).clone() else {
        panic!("asm {{ hlt }} should desugar to a call statement");
    };
    assert_eq!(hlt.target.as_str(&parsed), "asm#hlt");
    assert!(hlt.receiver.is_empty());
    assert_eq!(hlt.arguments.count(), 0);

    let StatementNode::Call(out) = parsed.statements.statement(statements[2]).clone() else {
        panic!("asm {{ out .. }} should desugar to a call statement");
    };
    assert_eq!(out.target.as_str(&parsed), "asm#port_out");
    assert_eq!(out.arguments.count(), 2);

    let StatementNode::Assignment(read) = parsed.statements.statement(statements[3]).clone() else {
        panic!("asm {{ in .. }} should desugar to an assignment");
    };
    let ExpressionNode::Call(port_in) = parsed.expressions.expression(read.value).clone() else {
        panic!("asm {{ in .. }} assignment value should be the intrinsic call");
    };
    assert_eq!(port_in.target.as_str(&parsed), "asm#port_in");
    assert_eq!(port_in.arguments.count(), 1);
    assert!(!port_in.receiver.is_valid());
}

#[test]
fn refuses_inexact_clobbers_and_control_flow_contracts() {
    for (block, expected) in [
        (
            "asm clobbers rax, rdx, r10 { out main.port, main.value }",
            "missing `r11`",
        ),
        ("asm clobbers rax { hlt }", "not clobbered `rax`"),
        (
            "asm ensures true { hlt }",
            "requires a falling-through block",
        ),
        (
            "asm { jmp done(); hlt }",
            "an asm control transfer must be the final instruction",
        ),
    ] {
        let source = format!(
            "data Main {{ port: u16; value: u8; }} machine Main::main(main: @mut Main) {{ {block} machine done() {{}} }}"
        );
        let errors = prepare_source(&source).expect_err("catalog validation should reject");
        assert!(
            format!("{errors:?}").contains(expected),
            "{block}: {errors:?}"
        );
    }
}

#[test]
fn entry_and_nested_bodies_preserve_expanded_statement_order_and_boundaries() {
    let body =
        "let first: u64 = 1; asm clobbers none { lfence; sfence } transition { _ -> done() }";
    for source in [
        format!("machine process() {{ {body} machine done() {{}} }}"),
        format!("machine process() {{ machine begin() {{ {body} }} machine done() {{}} }}"),
    ] {
        let trees = prepare_source(&source).expect("prepare assembly in each body route");
        let Item::Machine(machine) = trees.root_items().next().unwrap() else {
            panic!("machine root")
        };
        let machine = if trees
            .items
            .state(trees.items.state_handles(machine.states)[0])
            .statements
            .is_empty()
        {
            machine
                .nested_machines
                .handles()
                .find_map(|handle| match trees.items.item(handle) {
                    Item::Machine(child) if child.name.as_str(&trees) == "begin" => Some(child),
                    _ => None,
                })
                .expect("nested begin")
        } else {
            machine
        };
        let states = trees.items.state_handles(machine.states);
        assert_eq!(states.len(), 1);
        let statements = instruction_statements(
            &trees,
            trees
                .items
                .statements(trees.items.state(states[0]).statements),
        );
        assert_eq!(statements.len(), 4);
        assert!(matches!(
            trees.statements.statement(statements[0]),
            StatementNode::LocalData(_)
        ));
        for (handle, expected) in [(statements[1], "asm#lfence"), (statements[2], "asm#sfence")] {
            assert!(
                matches!(trees.statements.statement(handle), StatementNode::Call(call) if call.target.as_str(&trees) == expected)
            );
        }
        assert!(matches!(
            trees.statements.statement(statements[3]),
            StatementNode::Dispatch(_)
        ));
    }
}

#[test]
fn full_and_deferred_resolution_check_the_authored_instruction_catalog() {
    let source = "machine main() { asm { unknown_mnemonic } }";
    let tokens = Lexer::new(source).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).expect("the assembly block is well formed");
    let full =
        crate::resolution::resolve(crate::resolution::ResolutionRequest::new(syntax.clone()))
            .map(|_| ())
            .expect_err("unknown instruction");
    let deferred =
        crate::resolution::resolve_deferred(crate::resolution::ResolutionRequest::new(syntax))
            .map(|_| ())
            .expect_err("unknown instruction");
    for errors in [full, deferred] {
        assert!(
            errors.iter().any(|error| error
                .message
                .contains("only known-contract instructions compile")),
            "{errors:?}"
        );
    }
}
