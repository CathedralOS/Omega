#[test]
fn target_instruction_operands_are_checked_against_the_admitted_target() {
    use omega_resolver::{AssemblyVocabulary, ParsedSource, StageSource, resolve_stage_for_target};
    use omega_typer::{AssemblyTypes, type_stage_for_target};
    let code = "asm machine copy(n:u8) registers none=n clobbers r0, scratch { mov r0, 7; mov.byte r0, 1; transition -> done; asm machine done {} }";
    let tokens = omega_lexer::lex(code.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(code.as_bytes(), &tokens).unwrap();
    let sources = [StageSource {
        package: "game",
        source: ParsedSource {
            logical_path: "asm.omg",
            text: code,
            tokens: &tokens,
            tree: &tree,
        },
    }];
    let resolved = resolve_stage_for_target(
        &sources,
        &[],
        &AssemblyVocabulary {
            instructions: &["mov", "mov.byte"],
            registers: &["r0", "none", "scratch"],
        },
    )
    .unwrap();
    let unknown_register = resolve_stage_for_target(
        &sources,
        &[],
        &AssemblyVocabulary {
            instructions: &["mov", "mov.byte"],
            registers: &["r0", "scratch"],
        },
    )
    .unwrap_err();
    assert_eq!(unknown_register.span.slice(code.as_bytes()), Some(b"none".as_slice()));
    assert_eq!(unknown_register.message, "unavailable target register none");
    let byte = ValueType::Integer { width: Some(8), signed: false };
    let mut target = AssemblyTypes {
        target: "test8".into(),
        registers: ["r0", "none", "scratch"]
            .into_iter()
            .map(|r| (r.into(), byte.clone()))
            .collect(),
        instructions: ["mov", "mov.byte"]
            .into_iter()
            .map(|name| (name.into(), vec![vec![byte.clone(), byte.clone()]]))
            .collect(),
    };
    let stage = type_stage_for_target(&resolved, &target).unwrap();
    let tree = stage.resolved().trees().next().unwrap();
    let transfer = tree
        .file
        .nodes
        .iter()
        .position(|node| node.kind == omega_parser::NodeKind::AsmTransition)
        .unwrap();
    assert!(matches!(tree.symbol(transfer), Some(omega_resolver::Symbol::Declaration(id)) if id.path == "copy::done"));
    assert_eq!(stage.instructions.len(), 2);
    let typed_tree = stage.trees().next().unwrap();
    let mnemonics: Vec<_> = tree
        .file
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.kind == omega_parser::NodeKind::Instruction)
        .map(|(node, _)| {
            let instruction = typed_tree
                .semantics_at(node)
                .find_map(|fact| match fact {
                    omega_typer::NodeSemantics::Instruction(instruction) => Some(instruction),
                    _ => None,
                })
                .unwrap();
            assert_eq!(instruction.node, tree.file.node_id(node));
            assert_eq!(instruction.target, "test8");
            assert_eq!(instruction.operands, [byte.clone(), byte.clone()]);
            instruction.mnemonic.as_str()
        })
        .collect();
    assert_eq!(mnemonics, ["mov", "mov.byte"]);
    for (kind, expected) in [
        (omega_parser::NodeKind::Registers, &["none"][..]),
        (omega_parser::NodeKind::Clobbers, &["r0", "scratch"][..]),
    ] {
        let node = tree.file.nodes.iter().position(|n| n.kind == kind).unwrap();
        let registers: Vec<_> = tree
            .names(node)
            .filter_map(|n| match &n.target {
                omega_resolver::Symbol::TargetRegister(name) => Some(name.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(registers, expected);
    }
    target.registers.insert("none".into(), ValueType::Bool);
    let mismatch = type_stage_for_target(&resolved, &target).unwrap_err();
    assert_eq!(mismatch.span.slice(code.as_bytes()), Some(b"n".as_slice()));
    assert_eq!(mismatch.message, "expected bool, found u8");
    target.registers.insert("none".into(), byte.clone());
    target.registers.remove("scratch");
    assert_eq!(
        type_stage_for_target(&resolved, &target).unwrap_err().message,
        "missing register type for scratch"
    );
    target.registers.insert("scratch".into(), byte);
    target.instructions.get_mut("mov").unwrap()[0][1] = ValueType::Bool;
    assert!(
        type_stage_for_target(&resolved, &target)
            .unwrap_err()
            .message
            .contains("no matching instruction operands")
    );
}
