use omega_lowering::{AssemblyOperand, BlockTerminator, OperationKind, OperationResult, TargetIdentity, lower_package};
use omega_preparation::prepare_source_package;
use omega_resolver::{ParsedSource, StageSource};
use omega_typer::{
    AssemblyTypes, InstructionExit, InstructionInput, InstructionParameter, InstructionSemantics, InstructionValue, InstructionVariant, InstructionWrite,
    TypedInstructionOperand, ValueType, resolve_source_package, resolve_source_package_with_assembly,
};
use std::sync::Arc;

fn catalog(flag: &str) -> Arc<AssemblyTypes> {
    let word = ValueType::Integer {
        signed: false,
        width: Some(32),
    };
    let scalar = InstructionParameter::Scalar(word.clone());
    let variant = |parameters, writes, exit| {
        vec![InstructionVariant {
            parameters,
            semantics: Arc::new(InstructionSemantics { writes, exit }),
        }]
    };
    Arc::new(AssemblyTypes {
        target: TargetIdentity {
            profile: "fixture-cpu".into(),
            semantics_version: 1,
            realization: vec![1].into(),
        },
        registers: [
            ("r0".into(), word.clone()),
            ("z".into(), ValueType::Bool),
            ("other".into(), ValueType::Bool),
        ]
        .into(),
        instructions: [
            (
                "set".into(),
                variant(
                    vec![scalar.clone(), scalar],
                    vec![
                        InstructionWrite {
                            destination: InstructionInput::Operand(0),
                            value: InstructionValue::Copy(InstructionInput::Operand(1)),
                        },
                        InstructionWrite {
                            destination: InstructionInput::Register(flag.into()),
                            value: InstructionValue::IsZero(InstructionInput::Operand(1)),
                        },
                    ],
                    InstructionExit::Continue,
                ),
            ),
            (
                "jz".into(),
                variant(
                    vec![InstructionParameter::LocalBlock],
                    vec![],
                    InstructionExit::Branch {
                        condition: InstructionInput::Register(flag.into()),
                        target: 0,
                    },
                ),
            ),
            ("ret".into(), variant(vec![], vec![], InstructionExit::Return)),
        ]
        .into(),
    })
}

fn parse_with<T>(
    text: &str,
    f: impl FnOnce(&[StageSource<'_, '_>]) -> T,
) -> T {
    let tokens = omega_lexer::lex(text.as_bytes()).unwrap();
    let syntax = omega_parser::parse_file(text.as_bytes(), &tokens).unwrap();
    f(&[StageSource {
        package: "app",
        source: ParsedSource {
            logical_path: "example.omg",
            text,
            tokens: &tokens,
            tree: &syntax,
        },
    }])
}

#[test]
fn the_admitted_semantics_and_exact_inputs_reach_psi_from_the_package_path() {
    let text = "asm machine choose(n:u32) { set r0,n; jz yes; transition -> no; asm machine yes {ret;} asm machine no {ret;} }";
    for flag in ["z", "other"] {
        let lowered = parse_with(text, |sources| {
            let no_target = resolve_source_package(sources).unwrap_err();
            assert_eq!(no_target.message, "unavailable target instruction set");
            assert_eq!(no_target.span.slice(text.as_bytes()), Some(b"set".as_slice()));
            let selected = catalog(flag);
            let package = resolve_source_package_with_assembly(sources, selected.clone()).unwrap();
            let prepared = prepare_source_package(&package).unwrap();
            let op = prepared
                .typed()
                .instructions
                .iter()
                .find(|i| i.mnemonic == "set")
                .unwrap();
            assert_eq!(op.target, selected.target);
            assert!(matches!(&op.operands[..],[TypedInstructionOperand::Register{name,..},TypedInstructionOperand::Value{..}] if name=="r0"));
            assert_eq!(op.implicit_registers, [(flag.into(), ValueType::Bool)]);
            assert_eq!(op.semantics, selected.instructions["set"][0].semantics);
            lower_package(&prepared).unwrap()
        });
        let [machine] = lowered.machines.as_slice() else {
            panic!("nested assembly labels form one callable body")
        };
        let [entry, set, branch, _, _] = machine.body.operations.as_slice() else {
            panic!("expected register-region entry and four instructions")
        };
        let OperationKind::AssemblyInstruction(set) = &set.kind else {
            panic!("expected selected set")
        };
        assert_eq!(set.entry, entry.id);
        assert!(
            matches!(set.operands.as_slice(),[AssemblyOperand::Register(r),AssemblyOperand::Value(value)] if r.name=="r0" && machine.body.parameters==[omega_lowering::Parameter::Scalar(*value)])
        );
        assert_eq!(set.semantics, catalog(flag).instructions["set"][0].semantics);
        let OperationKind::AssemblyInstruction(jump) = &branch.kind else {
            panic!("expected flag observation")
        };
        let BlockTerminator::Conditional {
            condition,
            when_true,
            when_false,
        } = &machine.body.blocks[0].terminator
        else {
            panic!("expected conditional transfer")
        };
        assert_eq!(branch.result, OperationResult::Scalar(*condition));
        assert_eq!(jump.operands, [AssemblyOperand::LocalBlock(when_true.target)]);
        for instruction in [set, jump] {
            let [register] = &instruction.implicit_registers[..] else {
                panic!("expected exactly the flag from the selected catalog")
            };
            assert_eq!(register.name, flag);
            assert_eq!(
                lowered.types.get(register.ty),
                Some(&omega_psi::TypeShape::PrimitiveScalar(omega_psi::ScalarType::Boolean))
            );
        }
        // An independent reader follows the selected write and branch to the flag the catalog chose.
        spec_corpus::observe_handoff!(
            "mathematical-terms-and-assembly/consumer",
            assert_eq!(
                psi_handoff_consumer::assembly_branch(&machine.body, &machine.body.blocks[0]),
                Some((flag, when_true.target, when_false.target))
            )
        );
        assert_eq!((when_true.target.get(), when_false.target.get()), (2, 3));
        assert!(!when_true.requires_adjacency && when_false.requires_adjacency);
    }
}

#[test]
fn a_register_source_selects_its_own_instruction_form() {
    parse_with("asm machine copy(n:u32) {set r0,n;set r0,r0;ret;}", |sources| {
        let mut selected = catalog("z");
        let word = ValueType::Integer {
            signed: false,
            width: Some(32),
        };
        let forms = Arc::make_mut(&mut selected)
            .instructions
            .get_mut("set")
            .unwrap();
        forms[0].parameters = vec![
            InstructionParameter::Register(word.clone()),
            InstructionParameter::Expression(word.clone()),
        ];
        let mut from_register = catalog("other").instructions["set"][0].clone();
        from_register.parameters = vec![
            InstructionParameter::Register(word.clone()),
            InstructionParameter::Register(word),
        ];
        let register_semantics = from_register.semantics.clone();
        forms.push(from_register);
        let package = resolve_source_package_with_assembly(sources, selected).unwrap();
        let prepared = prepare_source_package(&package).unwrap();
        let lowered = lower_package(&prepared).unwrap();
        let sets = lowered.machines[0]
            .body
            .operations
            .iter()
            .filter_map(|op| match &op.kind {
                OperationKind::AssemblyInstruction(i) if i.mnemonic == "set" => Some(i),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [expression, register] = sets.as_slice() else {
            panic!("expected both set instructions")
        };
        assert_eq!([expression, register].map(|i| i.implicit_registers[0].name.as_str()), ["z", "other"]);
        assert_eq!(register.semantics, register_semantics);
        assert!(matches!(register.operands.as_slice(), [AssemblyOperand::Register(dst), AssemblyOperand::Register(src)] if dst == src && src.name == "r0"));
    });
}

#[test]
fn assembly_requires_both_an_exact_target_and_an_admitted_mnemonic() {
    parse_with("asm machine done {ret;}", |sources| {
        let mut missing = catalog("z");
        Arc::make_mut(&mut missing).instructions.remove("ret");
        let unknown = resolve_source_package_with_assembly(sources, missing).unwrap_err();
        assert_eq!(unknown.message, "unavailable target instruction ret");
        assert_eq!(unknown.span.slice(sources[0].source.text.as_bytes()), Some(b"ret".as_slice()));
        let mut anonymous = catalog("z");
        Arc::make_mut(&mut anonymous).target.realization = vec![].into();
        let package = resolve_source_package_with_assembly(sources, anonymous).unwrap();
        let error = prepare_source_package(&package).unwrap_err();
        assert_eq!(error.message, "the target capsule has no exact selected identity");
    });
}

#[test]
fn preparation_cannot_mix_an_instruction_catalog_with_another_layout_target() {
    use omega_preparation::{PreparationRound, TargetSemantics, prepare_source_package_for_target};
    parse_with("asm machine done {ret;}", |sources| {
        let catalog = catalog("z");
        let p = resolve_source_package_with_assembly(sources, catalog.clone()).unwrap();
        let make_target = |byte| {
            TargetSemantics::new(
                TargetIdentity {
                    realization: vec![byte].into(),
                    ..catalog.target.clone()
                },
                [],
                [],
            )
            .unwrap()
        };
        let matching = make_target(1);
        assert!(prepare_source_package_for_target(&p, &[], &matching).is_ok());
        assert!(PreparationRound::begin(&p, &[], Some(&matching)).is_ok());
        let other = make_target(2);
        let direct = prepare_source_package_for_target(&p, &[], &other).unwrap_err();
        let round = PreparationRound::begin(&p, &[], Some(&other))
            .err()
            .expect("round must reject mismatch");
        spec_corpus::observe_handoff!("open-target-and-selected-layout/refusal", assert_eq!(round.reason(), Some(omega_preparation::PreparationReason::AssemblyTargetMismatch)));
        assert_eq!(direct.message, "assembly catalog and preparation selected different targets");
        assert_eq!(round.diagnostic(), &direct);
    });
}

#[test]
fn selected_instruction_cannot_claim_two_simultaneous_writes_to_the_same_register() {
    parse_with("asm machine done {set r0,1;ret;}", |sources| {
        let mut c = catalog("z");
        let variant = &mut Arc::make_mut(&mut c).instructions.get_mut("set").unwrap()[0];
        Arc::make_mut(&mut variant.semantics).writes[1] = InstructionWrite {
            destination: InstructionInput::Register("r0".into()),
            value: InstructionValue::Copy(InstructionInput::Operand(1)),
        };
        let p = resolve_source_package_with_assembly(sources, c).unwrap();
        let err = prepare_source_package(&p).unwrap_err();
        assert_eq!(err.message, "instruction writes the same register twice");
    });
}

#[test]
fn a_direct_backedge_reuses_the_assembly_region_without_reinitializing_it() {
    parse_with(
        "asm machine again(n:u32) registers r0=n {transition -> check;asm machine check {jump check;}}",
        |sources| {
            let mut c = catalog("z");
            Arc::make_mut(&mut c).instructions.insert(
                "jump".into(),
                vec![InstructionVariant {
                    parameters: vec![InstructionParameter::LocalBlock],
                    semantics: Arc::new(InstructionSemantics {
                        writes: vec![],
                        exit: InstructionExit::Jump { target: 0 },
                    }),
                }],
            );
            let p = resolve_source_package_with_assembly(sources, c).unwrap();
            let prepared = prepare_source_package(&p).unwrap();
            let lowered = lower_package(&prepared).unwrap();
            let body = &lowered.machines[0].body;
            let [entry, local] = body.blocks.as_slice() else {
                panic!("expected one local continuation")
            };
            let BlockTerminator::Jump(first) = &entry.terminator else {
                panic!("no entry transition")
            };
            let BlockTerminator::Jump(backedge) = &local.terminator else {
                panic!("no loop edge")
            };
            assert_eq!(first.target, backedge.target);
            spec_corpus::observe_handoff!(
                psi "psi-output/assembly-backedge",
                assert!(first.requires_adjacency && !backedge.requires_adjacency),
            );
            let [setup, branch] = body.operations.as_slice() else {
                panic!("expected one entry and one branch")
            };
            let OperationKind::AssemblyInstruction(i) = &branch.kind else {
                panic!("no selected jump")
            };
            assert_eq!(i.entry, setup.id);
            assert_eq!(i.operands, [AssemblyOperand::LocalBlock(backedge.target)]);
        },
    );
}

#[test]
fn a_conditional_jump_cannot_invent_a_missing_successor() {
    parse_with("asm machine choose(n:u32){set r0,n;jz yes;asm machine yes{ret;}}", |sources| {
        let p = resolve_source_package_with_assembly(sources, catalog("z")).unwrap();
        let prep = prepare_source_package(&p).unwrap();
        let err = lower_package(&prep).unwrap_err();
        assert_eq!(err.message, "conditional assembly instruction needs an explicit adjacent transition");
    });
}

#[test]
fn separate_inline_assembly_regions_have_distinct_scratch_state() {
    parse_with(
        "machine store(n:u32,m:u32){asm registers r0=n clobbers r0,z {set r0,n;} asm registers r0=m clobbers r0,z {set r0,m;}}",
        |sources| {
            let p = resolve_source_package_with_assembly(sources, catalog("z")).unwrap();
            let prep = prepare_source_package(&p).unwrap();
            let lowered = lower_package(&prep).unwrap();
            let [a, x, b, y] = lowered.machines[0].body.operations.as_slice() else {
                panic!("expected two regions")
            };
            use omega_lowering::Parameter::Scalar;
            let [Scalar(n), Scalar(m)] = lowered.machines[0].body.parameters.as_slice() else {
                panic!("expected store(n:u32,m:u32)")
            };
            for (entry, op, source) in [(a, x, n), (b, y, m)] {
                let OperationKind::BeginAssembly(region) = &entry.kind else {
                    panic!("missing entry")
                };
                let [(register, initializer)] = region.bindings.as_slice() else {
                    panic!("missing initialized register")
                };
                assert_eq!(register.name, "r0");
                spec_corpus::observe_handoff!(
                    psi "psi-output/assembly-initializers",
                    assert_eq!(initializer, source),
                );
                assert_eq!(region.clobbers.as_deref(), Some(&["r0".to_owned(), "z".to_owned()][..]));
                let OperationKind::AssemblyInstruction(instruction) = &op.kind else {
                    panic!("missing instruction")
                };
                assert_eq!(entry.id, instruction.entry);
                assert_eq!(instruction.operands[1], AssemblyOperand::Value(*source));
                assert_eq!(region.target, instruction.target);
            }
            assert_ne!(a.id, b.id);
        },
    );
}
