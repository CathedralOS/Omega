#[test]
fn target_instruction_operands_are_checked_against_the_admitted_target() {
    use omega_resolver::{AssemblyVocabulary, ParsedSource, StageSource, resolve_stage_for_target};
    use omega_typer::{AssemblyTypes, type_stage_for_target};
    let code = "asm machine copy(n:u8) registers r0=n clobbers none { mov r0, 7; }";
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
            instructions: &["mov"],
            registers: &["r0"],
        },
    )
    .unwrap();
    let byte = ValueType::Integer { width: Some(8), signed: false };
    let mut target = AssemblyTypes {
        target: "test8".into(),
        registers: [("r0".into(), byte.clone())].into(),
        instructions: [("mov".into(), vec![vec![byte.clone(), byte]])].into(),
    };
    let stage = type_stage_for_target(&sources, &resolved, &target).unwrap();
    assert_eq!(stage.instructions.len(), 1);
    assert_eq!(stage.instructions[0].target, "test8");
    target.instructions.get_mut("mov").unwrap()[0][1] = ValueType::Bool;
    assert!(
        type_stage_for_target(&sources, &resolved, &target)
            .unwrap_err()
            .message
            .contains("no matching instruction operands")
    );
}
