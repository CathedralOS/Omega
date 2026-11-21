use omega_lowering::{AssemblyOperand, BlockTerminator, OperationKind, OperationResult, TargetIdentity, lower_package};
use omega_preparation::prepare_source_package;
use omega_resolver::{ParsedSource, StageSource};
use omega_typer::{
    AssemblyLocation, AssemblyTypes, InstructionExit, InstructionInput, InstructionParameter, InstructionSemantics, InstructionValue, InstructionVariant,
    InstructionWrite, TypedInstructionOperand, ValueType, resolve_source_package, resolve_source_package_with_assembly,
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
        locations: [
            ("r0".into(), AssemblyLocation::working_register(word.clone())),
            ("z".into(), AssemblyLocation::working_register(ValueType::Bool)),
            ("other".into(), AssemblyLocation::working_register(ValueType::Bool)),
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
                            destination: InstructionInput::Location(flag.into()),
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
                        condition: InstructionInput::Location(flag.into()),
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
            assert_eq!(op.implicit_locations, [(flag.into(), AssemblyLocation::working_register(ValueType::Bool))]);
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
            let [register] = &instruction.implicit_locations[..] else {
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
        assert_eq!([expression, register].map(|i| i.implicit_locations[0].name.as_str()), ["z", "other"]);
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
            destination: InstructionInput::Location("r0".into()),
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

/// Fixture authority: the `fixture-cpu` target of `catalog`. These locations and
/// mnemonics are test contracts for that fixture, not a model of any real ISA.
/// `ctl` and `dev` are twins: both Boolean and persistent, and only `ctl` is a register.
fn location_catalog() -> Arc<AssemblyTypes> {
    use omega_typer::{LocationLifetime::*, LocationRole::*};
    let mut selected = catalog("z");
    let target = Arc::make_mut(&mut selected);
    let declare = |role, lifetime| AssemblyLocation {
        ty: ValueType::Bool,
        role,
        lifetime,
    };
    target
        .locations
        .insert("ctl".into(), declare(Register, Persistent));
    target
        .locations
        .insert("dev".into(), declare(State, Persistent));
    target
        .locations
        .insert("b".into(), AssemblyLocation::working_register(ValueType::Bool));
    target.locations.insert(
        "r0lo".into(),
        AssemblyLocation::working_register(ValueType::Integer { signed: false, width: Some(8) }),
    );
    target.locations.insert(
        "lo".into(),
        AssemblyLocation::working_register(ValueType::Integer { signed: false, width: Some(8) }),
    );
    let location = |name: &str| InstructionInput::Location(name.into());
    let write = |name: &str, value| InstructionWrite {
        destination: location(name),
        value,
    };
    for (mnemonic, writes) in [
        ("setctl", vec![write("ctl", InstructionValue::Boolean(true))]),
        ("setdev", vec![write("dev", InstructionValue::Boolean(true))]),
        ("setz", vec![write("z", InstructionValue::Boolean(true))]),
        ("rdctl", vec![write("b", InstructionValue::Copy(location("ctl")))]),
        ("rddev", vec![write("b", InstructionValue::Copy(location("dev")))]),
        ("rdz", vec![write("b", InstructionValue::Copy(location("z")))]),
        ("rdlo", vec![write("lo", InstructionValue::Copy(location("r0lo")))]),
    ] {
        target.instructions.insert(
            mnemonic.into(),
            vec![InstructionVariant {
                parameters: vec![],
                semantics: Arc::new(InstructionSemantics {
                    writes,
                    exit: InstructionExit::Continue,
                }),
            }],
        );
    }
    selected
}

/// Each published assembly instruction with its operation, in body order.
fn published(body: &omega_lowering::MachineBody) -> Vec<(omega_lowering::OperationId, &omega_lowering::AssemblyInstruction)> {
    body.operations
        .iter()
        .filter_map(|op| match &op.kind {
            OperationKind::AssemblyInstruction(i) => Some((op.id, &**i)),
            _ => None,
        })
        .collect()
}

fn entry_of<'a>(
    body: &'a omega_lowering::MachineBody,
    instruction: &omega_lowering::AssemblyInstruction,
) -> &'a omega_lowering::AssemblyEntry {
    match &body.operations[instruction.entry.index()].kind {
        OperationKind::BeginAssembly(entry) => entry,
        _ => panic!("an instruction names its region entry"),
    }
}

#[test]
fn catalog_twins_differ_only_in_register_role_and_keep_each_authored_clobber_set() {
    use omega_typer::{LocationLifetime, LocationRole};
    let catalog = location_catalog();
    // The twins' declarations differ in role alone.
    assert_eq!(
        AssemblyLocation {
            role: LocationRole::State,
            ..catalog.locations["ctl"].clone()
        },
        catalog.locations["dev"]
    );
    let text = "machine m() { asm clobbers ctl { setctl; } asm clobbers none { setctl; } asm clobbers none { setdev; } }";
    parse_with(text, |sources| {
        let package = resolve_source_package_with_assembly(sources, catalog.clone()).unwrap();
        let prepared = prepare_source_package(&package).unwrap();
        let typed = |mnemonic: &str| {
            prepared
                .typed()
                .instructions
                .iter()
                .find(|i| i.mnemonic == mnemonic)
                .unwrap()
        };
        assert_eq!(typed("setctl").implicit_locations, [("ctl".into(), catalog.locations["ctl"].clone())]);
        assert_eq!(typed("setdev").implicit_locations, [("dev".into(), catalog.locations["dev"].clone())]);
        let lowered = lower_package(&prepared).unwrap();
        let body = &lowered.machines[0].body;
        let instructions = published(body);
        for (_, instruction) in &instructions {
            assert_eq!(instruction.semantics, catalog.instructions[instruction.mnemonic.as_str()][0].semantics);
            let [location] = instruction.implicit_locations.as_slice() else {
                panic!("expected the one written location")
            };
            assert_eq!(
                lowered.types.get(location.ty),
                Some(&omega_psi::TypeShape::PrimitiveScalar(omega_psi::ScalarType::Boolean))
            );
        }
        // Per region: the authored set, the reader's changed registers and state writes,
        // and the written location's published role and lifetime. No stage judges the
        // authored set; the checker compares it with the changed registers.
        let regions: Vec<_> = instructions
            .iter()
            .map(|(_, i)| {
                let written = &i.implicit_locations[0];
                (
                    entry_of(body, i)
                        .clobbers
                        .as_ref()
                        .map(|names| names.iter().map(String::as_str).collect::<Vec<_>>()),
                    psi_handoff_consumer::assembly_changed_registers(i),
                    psi_handoff_consumer::assembly_state_writes(i),
                    (written.role, written.lifetime),
                )
            })
            .collect();
        spec_corpus::observe_handoff!(
            psi "psi-output/assembly-location-role-twins",
            assert_eq!(
                regions,
                [
                    (Some(vec!["ctl"]), Some(vec!["ctl"]), Some(vec![]), (LocationRole::Register, LocationLifetime::Persistent)),
                    (Some(vec![]), Some(vec!["ctl"]), Some(vec![]), (LocationRole::Register, LocationLifetime::Persistent)),
                    (Some(vec![]), Some(vec![]), Some(vec!["dev"]), (LocationRole::State, LocationLifetime::Persistent)),
                ]
            ),
        );
    });
    // The state twin has no register name, so it can never be spelled as a clobber.
    let text = "machine m() { asm clobbers dev { setdev; } }";
    parse_with(text, |sources| {
        let error = resolve_source_package_with_assembly(sources, location_catalog()).unwrap_err();
        assert_eq!(
            (error.message.as_str(), error.span.slice(text.as_bytes())),
            ("unavailable target register dev", Some(b"dev".as_slice()))
        );
    });
}

#[test]
fn location_lifetimes_decide_supply_across_blocks_and_regions() {
    use psi_handoff_consumer::LocationSupply::*;
    let catalog = location_catalog();
    // One region with three blocks. The changed flag drives the branch, and both the
    // flag and the persistent register reach the taken block through the region's edge.
    let text = "asm machine run() { setctl; setz; jz yes; transition -> no; asm machine yes { rdz; rdctl; ret; } asm machine no { ret; } }";
    parse_with(text, |sources| {
        let package = resolve_source_package_with_assembly(sources, catalog.clone()).unwrap();
        let lowered = lower_package(&prepare_source_package(&package).unwrap()).unwrap();
        let body = &lowered.machines[0].body;
        let instructions = published(body);
        // Canonical source order, the selected target on every record, and one region.
        assert_eq!(
            instructions
                .iter()
                .map(|(_, i)| i.mnemonic.as_str())
                .collect::<Vec<_>>(),
            ["setctl", "setz", "jz", "rdz", "rdctl", "ret", "ret"]
        );
        let entry = entry_of(body, instructions[0].1);
        assert_eq!(entry.target, catalog.target);
        assert!(
            instructions
                .iter()
                .all(|(_, i)| i.target == catalog.target && i.entry == instructions[0].1.entry)
        );
        let [
            (set_ctl, _),
            (set_z, setz),
            (_, jz),
            (read_z, _),
            (read_ctl, _),
            ..,
        ] = instructions.as_slice()
        else {
            unreachable!()
        };
        assert_eq!(psi_handoff_consumer::assembly_changed_registers(setz), Some(vec!["z"]));
        assert_eq!(
            jz.semantics.exit,
            InstructionExit::Branch {
                condition: InstructionInput::Location("z".into()),
                target: 0
            }
        );
        let BlockTerminator::Conditional { when_true, when_false, .. } = &body.blocks[0].terminator else {
            panic!("expected the conditional transfer")
        };
        assert_eq!(
            psi_handoff_consumer::assembly_branch(body, &body.blocks[0]),
            Some(("z", when_true.target, when_false.target))
        );
        assert!(
            body.blocks[1..]
                .iter()
                .all(|block| matches!(block.terminator, BlockTerminator::Return { .. }))
        );
        spec_corpus::observe_handoff!(
            psi "psi-output/assembly-location-block-supply",
            assert_eq!(
                [(*read_z, "z"), (*read_ctl, "ctl")].map(|(op, name)| psi_handoff_consumer::assembly_location_supply(body, op, name)),
                [Some(vec![Written(*set_z)]), Some(vec![Written(*set_ctl)])]
            ),
        );
    });
    // Two inline regions around a two-armed match. Only the persistent locations cross
    // the region entry and the arms' blocks; the region-scoped flag starts unsupplied.
    let text =
        "machine f() {} machine g() {} machine m(flag: bool) { asm { setctl; setz; } match flag { true -> f(), false -> g(), }; asm { rdctl; rdz; rddev; } }";
    parse_with(text, |sources| {
        let package = resolve_source_package_with_assembly(sources, catalog.clone()).unwrap();
        let lowered = lower_package(&prepare_source_package(&package).unwrap()).unwrap();
        let machine = lowered
            .machines
            .iter()
            .find(|machine| published(&machine.body).len() == 5)
            .expect("the machine holding both regions");
        let body = &machine.body;
        let instructions = published(body);
        let [
            (set_ctl, first),
            _,
            (read_ctl, later),
            (read_z, _),
            (read_dev, _),
        ] = instructions.as_slice()
        else {
            panic!("expected setctl, setz, rdctl, rdz and rddev")
        };
        assert_ne!(first.entry, later.entry);
        let block_of = |op: omega_lowering::OperationId| {
            body.blocks
                .iter()
                .position(|block| block.operations.contains(&op.index()))
        };
        assert_ne!(block_of(*set_ctl), block_of(*read_ctl));
        assert!(body.blocks.len() >= 4, "the match contributes its arms and a join");
        spec_corpus::observe_handoff!(
            psi "psi-output/assembly-location-region-supply",
            assert_eq!(
                [(*read_ctl, "ctl"), (*read_z, "z"), (*read_dev, "dev")].map(|(op, name)| psi_handoff_consumer::assembly_location_supply(body, op, name)),
                [Some(vec![Written(*set_ctl)]), Some(vec![RegionStart]), Some(vec![MachineEntry])]
            ),
        );
    });
}

#[test]
fn declared_locations_stay_independent_so_overlapping_views_are_not_modelled() {
    // Stated limit: `r0lo` is declared beside `r0`, but the vocabulary has no overlap.
    // A write to r0 changes only r0, so a catalog with aliased views cannot be supplied.
    let text = "asm machine low(n:u32) { set r0, n; rdlo; ret; }";
    parse_with(text, |sources| {
        let package = resolve_source_package_with_assembly(sources, location_catalog()).unwrap();
        let lowered = lower_package(&prepare_source_package(&package).unwrap()).unwrap();
        let body = &lowered.machines[0].body;
        let instructions = published(body);
        let [(_, set), (read, _), _] = instructions.as_slice() else {
            panic!("expected set, rdlo and ret")
        };
        assert_eq!(psi_handoff_consumer::assembly_changed_registers(set), Some(vec!["r0", "z"]));
        assert_eq!(
            psi_handoff_consumer::assembly_location_supply(body, *read, "r0lo"),
            Some(vec![psi_handoff_consumer::LocationSupply::RegionStart])
        );
    });
}

/// The spec's inline body contract (`wiki/spec/language/assembly.md:142-150`),
/// wrapped in the smallest machine that supplies its explicit `context` value.
const FENCE_REGION: &str = "data Context { ready: bool; }\nmachine fence(context: Context) {\nasm\n    requires context.ready\n    clobbers none\n    ensures context.ready\n{\n    lfence; sfence\n}\n}\n";

#[test]
fn effects_outside_the_location_vocabulary_stay_refused() {
    // ISA barrier ordering has no vocabulary, so no fixture contracts a fence and the
    // spec's fence region stops at its first instruction.
    parse_with(FENCE_REGION, |sources| {
        let error = resolve_source_package_with_assembly(sources, location_catalog()).unwrap_err();
        assert_eq!(
            (error.message.as_str(), error.span.slice(FENCE_REGION.as_bytes())),
            ("unavailable target instruction lfence", Some(b"lfence".as_slice()))
        );
    });
    // A memory operand is an ordinary array value, not an access, so no scalar contract matches it.
    let text = "machine m(n:u32) { asm { set [n], n; } }";
    parse_with(text, |sources| {
        let package = resolve_source_package_with_assembly(sources, location_catalog()).unwrap();
        let error = prepare_source_package(&package).unwrap_err();
        assert_eq!(error.message, "no matching instruction operands for set");
        assert_eq!(error.span.slice(text.as_bytes()), Some(b"set [n], n".as_slice()));
    });
}

#[test]
fn a_branch_to_another_entry_is_refused_by_typing_and_its_local_twin_lowers() {
    // The twins differ only in where `yes` is declared: nested in `choose` or as its own entry.
    let local = "asm machine choose(n:u32) { set r0,n; jz yes; transition -> no; asm machine yes {ret;} asm machine no {ret;} }";
    let foreign = "asm machine choose(n:u32) { set r0,n; jz yes; transition -> no; asm machine no {ret;} } asm machine yes {ret;}";
    let lowered = parse_with(local, |sources| {
        let p = resolve_source_package_with_assembly(sources, catalog("z")).unwrap();
        lower_package(&prepare_source_package(&p).unwrap()).unwrap()
    });
    let BlockTerminator::Conditional { when_true, .. } = &lowered.machines[0].body.blocks[0].terminator else {
        panic!("expected the conditional transfer to the local block")
    };
    assert!(lowered.machines[0].body.operations.iter().any(|op| matches!(&op.kind,
        OperationKind::AssemblyInstruction(i) if i.operands == [AssemblyOperand::LocalBlock(when_true.target)])));
    parse_with(foreign, |sources| {
        // Typing runs when preparation receives the package, before any lowering.
        let p = resolve_source_package_with_assembly(sources, catalog("z")).unwrap();
        let error = prepare_source_package(&p).unwrap_err();
        assert_eq!(error.message, "instruction target lies outside this assembly region");
        let start = foreign.find("jz yes").unwrap() + 3;
        assert_eq!(error.span.slice(foreign.as_bytes()), Some(&foreign.as_bytes()[start..start + 3]));
    });
}
