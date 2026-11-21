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
    spec_corpus::observe_handoff!("assembly-symbols/refusal", assert_eq!(unknown_register.span.slice(code.as_bytes()), Some(b"none".as_slice())));
    assert_eq!(unknown_register.message, "unavailable target register none");
    let byte = ValueType::Integer { width: Some(8), signed: false };
    use omega_typer::{
        InstructionExit, InstructionInput, InstructionParameter, InstructionSemantics, InstructionValue, InstructionVariant, InstructionWrite,
        TypedInstructionOperand,
    };
    let mut target = AssemblyTypes {
        target: omega_target_contract::TargetIdentity {
            profile: "test8".into(),
            semantics_version: 1,
            realization: vec![1].into(),
        },
        locations: ["r0", "none", "scratch"]
            .into_iter()
            .map(|r| (r.into(), omega_typer::AssemblyLocation::working_register(byte.clone())))
            .collect(),
        instructions: ["mov", "mov.byte"]
            .into_iter()
            .map(|name| {
                (
                    name.into(),
                    vec![InstructionVariant {
                        parameters: vec![InstructionParameter::Scalar(byte.clone()); 2],
                        semantics: std::sync::Arc::new(InstructionSemantics {
                            writes: vec![InstructionWrite {
                                destination: InstructionInput::Operand(0),
                                value: InstructionValue::Copy(InstructionInput::Operand(1)),
                            }],
                            exit: InstructionExit::Continue,
                        }),
                    }],
                )
            })
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
            assert_eq!(instruction.target, target.target);
            spec_corpus::observe_handoff!("assembly-symbols/consumer", assert!(matches!(instruction.operands.as_slice(), [TypedInstructionOperand::Register{ty:a,..},TypedInstructionOperand::Value{ty:b,..}] if a==&byte && b==&byte)));
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
    target
        .locations
        .insert("none".into(), omega_typer::AssemblyLocation::working_register(ValueType::Bool));
    let mismatch = type_stage_for_target(&resolved, &target).unwrap_err();
    spec_corpus::observe_handoff!("mathematical-terms-and-assembly/refusal", assert_eq!(mismatch.span.slice(code.as_bytes()), Some(b"n".as_slice())));
    assert_eq!(mismatch.message, "expected bool, found u8");
    target
        .locations
        .insert("none".into(), omega_typer::AssemblyLocation::working_register(byte.clone()));
    target.locations.remove("scratch");
    assert_eq!(
        type_stage_for_target(&resolved, &target).unwrap_err().message,
        "missing register type for scratch"
    );
    target
        .locations
        .insert("scratch".into(), omega_typer::AssemblyLocation::working_register(byte));
    target.instructions.get_mut("mov").unwrap()[0].parameters[1] = InstructionParameter::Scalar(ValueType::Bool);
    assert!(
        type_stage_for_target(&resolved, &target)
            .unwrap_err()
            .message
            .contains("no matching instruction operands")
    );
}

/// Typing publishes each location's catalog role and lifetime wherever the program
/// or the selected contract names it: a `registers` binding, a register operand and
/// an implicit location. Fixture authority: the `test8` target of this test only.
#[test]
fn typed_assembly_records_carry_each_catalog_locations_role_and_lifetime() {
    use omega_resolver::{AssemblyVocabulary, ParsedSource, StageSource, resolve_stage_for_target};
    use omega_typer::{
        AssemblyLocation, AssemblyTypes, InstructionExit, InstructionInput, InstructionParameter, InstructionSemantics, InstructionValue, InstructionVariant,
        InstructionWrite, LocationLifetime, LocationRole, TypedInstructionOperand, type_stage_for_target,
    };
    let declare = |role, lifetime| AssemblyLocation {
        ty: ValueType::Bool,
        role,
        lifetime,
    };
    let (ctl, dev) = (
        declare(LocationRole::Register, LocationLifetime::Persistent),
        declare(LocationRole::State, LocationLifetime::Persistent),
    );
    let contract = |parameters, destination, source| {
        vec![InstructionVariant {
            parameters,
            semantics: std::sync::Arc::new(InstructionSemantics {
                writes: vec![InstructionWrite {
                    destination,
                    value: InstructionValue::Copy(source),
                }],
                exit: InstructionExit::Continue,
            }),
        }]
    };
    let target = AssemblyTypes {
        target: omega_target_contract::TargetIdentity {
            profile: "test8".into(),
            semantics_version: 1,
            realization: vec![1].into(),
        },
        locations: [("ctl".into(), ctl.clone()), ("dev".into(), dev.clone())].into(),
        instructions: [
            (
                "copy".into(),
                contract(
                    vec![InstructionParameter::Register(ValueType::Bool); 2],
                    InstructionInput::Operand(0),
                    InstructionInput::Operand(1),
                ),
            ),
            (
                "save".into(),
                contract(vec![], InstructionInput::Location("dev".into()), InstructionInput::Location("ctl".into())),
            ),
        ]
        .into(),
    };
    // Only the register is a source name; the toolchain builds the same vocabulary.
    assert_eq!(target.register_names(), ["ctl"]);
    assert!(target.register("dev").is_none());
    let code = "asm machine keep() registers ctl = true clobbers ctl { copy ctl, ctl; save; }";
    let tokens = omega_lexer::lex(code.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(code.as_bytes(), &tokens).unwrap();
    let sources = [StageSource {
        package: "game",
        source: ParsedSource {
            logical_path: "keep.omg",
            text: code,
            tokens: &tokens,
            tree: &tree,
        },
    }];
    let vocabulary = AssemblyVocabulary {
        instructions: &["copy", "save"],
        registers: &target.register_names(),
    };
    let resolved = resolve_stage_for_target(&sources, &[], &vocabulary).unwrap();
    let stage = type_stage_for_target(&resolved, &target).unwrap();
    let typed_tree = stage.trees().next().unwrap();
    let region = (0..typed_tree.resolved().file.nodes.len())
        .find_map(|node| typed_tree.assembly_region(node))
        .unwrap();
    let [(_, bound, location)] = region.bindings.as_slice() else {
        panic!("expected the ctl binding")
    };
    assert_eq!((bound.as_str(), location), ("ctl", &ctl));
    let [copy, save] = stage.instructions.as_slice() else {
        panic!("expected copy and save")
    };
    assert!(
        copy.operands
            .iter()
            .all(|operand| matches!(operand, TypedInstructionOperand::Register { name, ty: ValueType::Bool, lifetime: LocationLifetime::Persistent, .. } if name == "ctl"))
    );
    assert!(copy.implicit_locations.is_empty());
    assert_eq!(save.implicit_locations, [("ctl".into(), ctl), ("dev".into(), dev)]);
}

#[test]
fn a_local_block_operand_names_a_continuation_of_its_own_region() {
    use omega_resolver::{AssemblyVocabulary, ParsedSource, StageSource, resolve_stage_for_target};
    use omega_typer::{AssemblyTypes, InstructionExit, InstructionParameter, InstructionSemantics, InstructionVariant, TypedInstructionOperand, type_stage_for_target};
    let variant = |parameters, exit| vec![InstructionVariant { parameters, semantics: std::sync::Arc::new(InstructionSemantics { writes: vec![], exit }) }];
    let target = AssemblyTypes {
        target: omega_target_contract::TargetIdentity { profile: "test8".into(), semantics_version: 1, realization: vec![1].into() },
        locations: Default::default(),
        instructions: [
            ("jmp".into(), variant(vec![InstructionParameter::LocalBlock], InstructionExit::Jump { target: 0 })),
            ("ret".into(), variant(vec![], InstructionExit::Return)),
        ]
        .into(),
    };
    let type_code = |code: &str| {
        let tokens = omega_lexer::lex(code.as_bytes()).unwrap();
        let tree = omega_parser::parse_file(code.as_bytes(), &tokens).unwrap();
        let sources = [StageSource { package: "game", source: ParsedSource { logical_path: "asm.omg", text: code, tokens: &tokens, tree: &tree } }];
        let vocabulary = AssemblyVocabulary { instructions: &["jmp", "ret"], registers: &[] };
        let resolved = resolve_stage_for_target(&sources, &[], &vocabulary).unwrap();
        type_stage_for_target(&resolved, &target).map(|stage| stage.into_facts())
    };
    // A sibling edge and a backedge both stay inside the region of `drain`.
    let local = "asm machine drain() { transition -> check; asm machine check { jmp done; } asm machine done { jmp check; } }";
    let stage = type_code(local).unwrap();
    let targets: Vec<_> = stage
        .instructions
        .iter()
        .map(|i| match i.operands.as_slice() {
            [TypedInstructionOperand::Block { declaration, .. }] => declaration.path.as_str(),
            other => panic!("expected one local block operand, found {other:?}"),
        })
        .collect();
    assert_eq!(targets, ["drain::done", "drain::check"]);
    // The same jump to a separately callable entry, to the region's own root, and from an inline region.
    for (code, operand) in [
        ("asm machine drain() { transition -> check; asm machine check { jmp done; } } asm machine done { ret; }", "done"),
        ("asm machine drain() { transition -> check; asm machine check { jmp drain; } }", "drain"),
        ("asm machine done { ret; } machine run() { asm { jmp done; } }", "done"),
    ] {
        let error = type_code(code).unwrap_err();
        assert_eq!(error.message, "instruction target lies outside this assembly region", "{code}");
        let start = code.find(&format!("jmp {operand}")).unwrap() + 4;
        assert_eq!(error.span.slice(code.as_bytes()), Some(&code.as_bytes()[start..start + operand.len()]), "{code}");
    }
}
