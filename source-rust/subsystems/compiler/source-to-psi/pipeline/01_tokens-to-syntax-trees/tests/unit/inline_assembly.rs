use crate::tests::{entry_state, parse_error, parse_source, root_machine_unwrap};
use syntax_trees::expression::ExpressionNode;
use syntax_trees::statement::{AssemblyOperands, StatementNode};

#[test]
fn preserves_authored_mnemonics_operands_clobbers_and_facts() {
    let source = r#"
        data Main { port: u16; value: u8; ready: bool; }
        machine Main::main(main: @mutable Main) {
            asm requires main.ready clobbers rdx, rax ensures main.ready {
                first requires main.ready:
                vendor.op main.port, main.value;
                jmp done()
            }
            machine done() {}
        }
    "#;
    let tree = parse_source(source);
    assert_eq!(
        tree.statements.statement_count(),
        1,
        "one authored assembly statement"
    );
    let machine = root_machine_unwrap(&tree);
    let state = entry_state(&tree, machine);
    let statements = tree.items.statements(state.statements);
    let [statement] = statements else {
        panic!("one authored assembly scope");
    };
    let StatementNode::AuthoredAssembly(handle) = tree.statements.statement(*statement) else {
        panic!("authored assembly");
    };
    let authored = tree.statements.authored_assembly(*handle);
    let [block] = authored.blocks.as_ref() else {
        panic!("one labeled block");
    };
    assert_eq!(block.label.as_ref().unwrap().as_str(&tree), "first");
    assert_eq!(block.requires.len(), 1);
    assert_eq!(authored.requires.len(), 1);
    assert_eq!(authored.ensures.len(), 1);
    for fact in block
        .requires
        .iter()
        .chain(authored.requires.iter())
        .chain(authored.ensures.iter())
    {
        assert!(matches!(
            tree.expressions.expression(*fact),
            ExpressionNode::Member(_)
        ));
    }
    let names: Vec<_> = authored
        .clobbers
        .as_ref()
        .unwrap()
        .iter()
        .map(|name| name.as_str(&tree))
        .collect();
    assert_eq!(names, ["rdx", "rax"]);
    let [op, jump] = block.instructions.as_ref() else {
        panic!("two authored instructions");
    };
    assert_eq!(op.mnemonic.as_str(&tree), "vendor.op");
    let span = op.mnemonic.source_span().span;
    assert_eq!(&source[span.start..span.end], "vendor.op");
    let AssemblyOperands::Expressions(args) = &op.operands else {
        panic!("ordinary operands");
    };
    assert_eq!(args.len(), 2);
    for arg in args {
        assert!(matches!(
            tree.expressions.expression(*arg),
            ExpressionNode::Member(_)
        ));
    }
    assert_eq!(jump.mnemonic.as_str(&tree), "jmp");
    assert!(matches!(&jump.operands, AssemblyOperands::Transition(_)));
}

#[test]
fn rejects_malformed_asm_syntax_at_the_parser_boundary() {
    for (block, expected) in [
        (
            "asm { out main.port, main.value hlt }",
            "multiple asm instructions must be separated by `;`",
        ),
        ("asm {}", "an asm block must contain at least one"),
        (
            "asm requires true {}",
            "an asm block must contain at least one",
        ),
        ("asm clobbers { hlt }", "spell `clobbers none`"),
        ("asm where clobbers none { hlt }", "`clobbers <registers>`"),
    ] {
        let source = format!(
            "data Main {{ port: u16; value: u8; }} machine Main::main(main: @mutable Main) {{ {block} }}"
        );
        let error = parse_error(&source);
        assert!(error.message.contains(expected), "{block}: {error:?}");
    }
}

#[test]
fn an_assembly_machine_declares_scoped_blocks_and_an_explicit_adjacent_edge() {
    let tree = parse_source(
        "asm machine answer() -> u32 { mov eax, 42; transition -> finish; asm machine finish requires eax == 42u32 { ret; } }",
    );
    let machine = root_machine_unwrap(&tree);
    assert!(machine.authored_target.is_none());
    let state = entry_state(&tree, machine);
    let [statement] = tree.items.statements(state.statements) else {
        panic!("one assembly body");
    };
    let StatementNode::AuthoredAssembly(body) = tree.statements.statement(*statement) else {
        panic!("assembly body");
    };
    let body = tree.statements.authored_assembly(*body);
    let [entry, finish] = &*body.blocks else {
        panic!("two local blocks");
    };
    assert!(body.is_machine_body);
    assert!(entry.parent.is_none());
    assert_eq!(entry.adjacent.as_ref().unwrap().as_str(&tree), "finish");
    assert_eq!(finish.parent, Some(0));
    assert_eq!(finish.requires.len(), 1);
    assert_eq!(finish.instructions[0].mnemonic.as_str(&tree), "ret");
    assert!(machine.nested_machines.is_empty());
}
