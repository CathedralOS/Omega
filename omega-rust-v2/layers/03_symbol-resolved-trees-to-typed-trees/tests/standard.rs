use omega_resolver::{ParsedSource, StageSource};
use omega_typer::{TypedStage, resolve_source_package, type_source_package};
fn check(source: &str) -> Result<TypedStage, String> {
    let tokens = omega_lexer::lex(source.as_bytes()).map_err(|e| e.to_string())?;
    let tree = omega_parser::parse_file(source.as_bytes(), &tokens).map_err(|e| e.to_string())?;
    let inputs = [StageSource {
        package: "app",
        source: ParsedSource {
            logical_path: "app.omg",
            text: source,
            tokens: &tokens,
            tree: &tree,
        },
    }];
    let resolved = resolve_source_package(&inputs).map_err(|e| e.message)?;
    type_source_package(&resolved)
        .map(|trees| trees.into_facts())
        .map_err(|e| format!("{}: {}", e.file, e.message))
}
#[test]
fn closed_machine_evaluations_match_the_declaration_and_every_argument() {
    use omega_typer::{EvaluatedIntegerCall, TypingEnvironment, type_stage_in_environment};
    let source = "data B<const K:u8>{} machine width(x:u8)->u8 requires x>0 {x-1} machine buffer<const N:u8>()->B<width(N)>{B::<width(N)>{}} machine make()->B<6>{buffer::<7>()}";
    let tokens = omega_lexer::lex(source.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(source.as_bytes(), &tokens).unwrap();
    let files = [StageSource {
        package: "app",
        source: ParsedSource {
            logical_path: "receipt.omg",
            text: source,
            tokens: &tokens,
            tree: &tree,
        },
    }];
    let names = omega_resolver::resolve_stage(&files, &[]).unwrap();
    let machine = names
        .declarations
        .iter()
        .find(|id| id.path == "width")
        .unwrap()
        .clone();
    let run = |values: &[EvaluatedIntegerCall]| {
        type_stage_in_environment(
            &names,
            &TypingEnvironment {
                evaluated_integer_calls: values,
                ..Default::default()
            },
        )
    };
    let mut value = EvaluatedIntegerCall {
        machine,
        inputs: vec![6.into()],
        result: 6.into(),
    };
    assert!(
        run(std::slice::from_ref(&value))
            .unwrap_err()
            .message
            .contains("width(7) needs an admitted evaluation")
    );
    value.inputs[0] = 7.into();
    run(std::slice::from_ref(&value)).unwrap();
    value.result = 256.into();
    assert!(
        run(&[value])
            .unwrap_err()
            .message
            .contains("outside its declared integer signature")
    );
}
#[test]
fn specialization_uses_the_selected_trait_operation() {
    use omega_typer::{Site, SpecializedIndexValue, TypingEnvironment, ValueType, type_stage_in_environment};
    let source = "trait Add{machine + combine(x:u8,y:u8)->u8;} Selected satisfies Add{machine combine(x:u8,y:u8)->u8{x}} data B<const K:u8>{} machine buffer<const N:u8,const E:Add>()->B<N+1>{B::<N+1>{}} machine use_it()->B<7>{buffer::<7,Selected>()}";
    let tokens = omega_lexer::lex(source.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(source.as_bytes(), &tokens).unwrap();
    let files = [StageSource {
        package: "app",
        source: ParsedSource {
            logical_path: "op.omg",
            text: source,
            tokens: &tokens,
            tree: &tree,
        },
    }];
    let names = omega_resolver::resolve_stage(&files, &[]).unwrap();
    let selected = names
        .declarations
        .iter()
        .find(|id| id.path == "Selected")
        .unwrap()
        .clone();
    let start = source.find("N+1").unwrap();
    let value = SpecializedIndexValue {
        expression: Site {
            package: "app".into(),
            file: "op.omg".into(),
            span: omega_lexer::SourceSpan::new(start, start + 3),
        },
        arguments: vec![
            ValueType::StaticInteger {
                carrier: Box::new(ValueType::Integer { signed: false, width: Some(8) }),
                value: 7.into(),
            },
            ValueType::Implementation(Box::new(selected)),
        ],
        value: 7.into(),
    };
    let check = |receipts: &[SpecializedIndexValue]| {
        type_stage_in_environment(
            &names,
            &TypingEnvironment {
                specialized_indices: receipts,
                ..Default::default()
            },
        )
    };
    assert!(
        check(&[])
            .unwrap_err()
            .message
            .contains("needs an admitted evaluation")
    );
    let stage = check(std::slice::from_ref(&value)).unwrap();
    assert!(
        stage
            .calls
            .iter()
            .any(|c| c.machine.path == "Add::combine" && c.evidence.is_some() && c.use_site == omega_typer::CallUse::StaticArgument)
    );
    let mut wrong = value.clone();
    wrong.value = 8.into();
    assert!(
        check(&[value, wrong])
            .unwrap_err()
            .message
            .contains("conflicting evaluations")
    );
}
#[test]
fn production_core_selects_the_policy_and_ranking_declarations() {
    let source = "use omega::language::core; use omega::language::core::reflect; use omega::language::core::access; Rule satisfies AccessPolicy { machine plan<'t>(schema:TypeSchema<'t>)->AccessPlan<'t> reaches none crashes none terminates {new_plan::<'t>(@schema, Issuance::Unrestricted{})} } data C{} domain C::Locked access Rule; machine visit(n:u64) terminates by n in 0..=9 {}";
    let stage = check(source).unwrap();
    assert_eq!(stage.access_policies.len(), 1);
    assert_eq!(stage.ranking_witnesses[0].view.package, "@omega/toolchain");
    assert!(
        stage
            .calls
            .iter()
            .any(|c| c.machine.path == "language::core::access::new_plan")
    );
    assert!(
        check(&source.replace("0..=9", "0..=false"))
            .unwrap_err()
            .contains("expected u64")
    );
    assert!(
        check("machine fake(x:u64)->u64{x} machine f(x:u64) terminates by x -> fake {}")
            .unwrap_err()
            .contains("absent from the admitted theory")
    );
    assert!(check("data Schema{} data Plan{} trait AccessPolicy{machine plan(s:Schema)->Plan;} Rule satisfies AccessPolicy{machine plan(s:Schema)->Plan{Plan{}}} data C{} domain C::Locked access Rule;").unwrap_err().contains("does not supply the admitted plan"));
    assert!(
        check("machine fake(x:u8)->u8;")
            .unwrap_err()
            .contains("bodyless machine")
    );
}
#[test]
fn an_inbound_slot_counts_as_entry_and_an_outbound_provider_propagates_entries() {
    use omega_typer::{BoundarySlot, InvocationTarget, RequirementSlot, SlotDirection, TypingEnvironment, type_stage_in_environment};
    let source = "trait Host{machine enter();} trait Poll{machine poll() invokes Host;} machine outer(){Poll::poll()} machine direct(){Host::enter()}";
    let tokens = omega_lexer::lex(source.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(source.as_bytes(), &tokens).unwrap();
    let sources = [StageSource {
        package: "app",
        source: ParsedSource {
            logical_path: "slot.omg",
            text: source,
            tokens: &tokens,
            tree: &tree,
        },
    }];
    let names = omega_resolver::resolve_stage(&sources, &[]).unwrap();
    let id = |name: &str| {
        names
            .declarations
            .iter()
            .find(|id| id.path == name)
            .unwrap()
            .clone()
    };
    let slot = |name: &str, direction| BoundarySlot {
        requirement: RequirementSlot {
            declaration: id(name),
            trait_arguments: vec![],
        },
        arguments: vec![],
        direction,
    };
    let slots = [
        slot("Host::enter", SlotDirection::EnvironmentToProgram),
        slot("Poll::poll", SlotDirection::ProgramToProvider),
    ];
    let stage = type_stage_in_environment(
        &names,
        &TypingEnvironment {
            boundary_slots: &slots,
            ..Default::default()
        },
    )
    .unwrap();
    let entries = |name: &str| {
        &stage
            .machine_effects
            .iter()
            .find(|e| e.machine.path == name)
            .unwrap()
            .invokes
    };
    assert!(
        entries("direct")
            .iter()
            .any(|e| matches!(e,InvocationTarget::Declaration(slot) if slot.declaration.path=="Host::enter"))
    );
    assert!(
        entries("outer")
            .iter()
            .any(|e| matches!(e,InvocationTarget::Declaration(slot) if slot.declaration.path=="Host"))
    );
    assert!(
        !entries("outer")
            .iter()
            .any(|e| matches!(e,InvocationTarget::Declaration(slot) if slot.declaration.path=="Poll::poll"))
    );
}

#[test]
fn core_interfaces_supply_checked_types_without_adding_source_bodies() {
    use omega_typer::{
        ContractPredicate::Equal,
        ContractTerm::{Field, Natural, Result},
        ObligationKind,
    };
    let names = resolve_source_package(&[]).unwrap();
    assert!(names.uses.is_empty());
    let stage = type_source_package(&names).unwrap();
    assert!(stage.expressions.is_empty());
    let machine = stage
        .signatures
        .iter()
        .find(|s| s.machine.path == "language::core::access::new_plan")
        .unwrap();
    let contract = stage
        .interface_contracts
        .iter()
        .find(|row| row.declaration == machine.machine)
        .unwrap();
    assert_eq!(
        contract.ensures,
        &[
            Equal(Field(&Result, "field_count"), Natural(0)),
            Equal(Field(&Result, "reader_count"), Natural(0)),
            Equal(Field(&Result, "writer_count"), Natural(0)),
            Equal(Field(&Field(&Result, "reach"), "service_count"), Natural(0))
        ]
    );
    assert!(
        stage
            .obligations
            .iter()
            .any(|o| matches!(&o.kind,ObligationKind::VerifyLibraryImplementation(id) if *id==machine.machine))
    );
}

#[test]
fn bracket_contracts_publish_exact_bounds_and_range_length() {
    use omega_typer::ContractPredicate::{Equal, Less, LessEqual};
    use omega_typer::ContractTerm::{Argument, Length, Result, Subtract};
    let stage = check("machine read(view:@[u8])->u8{view[0]}").unwrap();
    let selected = &stage.collection_accesses[0].machine;
    let row = stage
        .interface_contracts
        .iter()
        .find(|row| row.declaration == *selected)
        .unwrap();
    assert_eq!(row.requires, &[Less(Argument(1), Length(&Argument(0)))]);
    assert_eq!(row.copy_parameters, &[0]);
    for name in ["indexing::range", "indexing::range_mut"] {
        let row = stage
            .interface_contracts
            .iter()
            .find(|row| row.declaration.path == name)
            .unwrap();
        assert_eq!(
            row.requires,
            &[
                LessEqual(Argument(1), Argument(2)),
                LessEqual(Argument(2), Length(&Argument(0)))
            ]
        );
        assert_eq!(row.ensures, &[Equal(Length(&Result), Subtract(&Argument(2), &Argument(1)))]);
        assert!(row.copy_parameters.is_empty());
    }
}

#[test]
fn declared_operand_type_selects_the_overload_across_numeric_widths_and_results() {
    // A typed parameter has exactly one matching overload. Its caller can promise
    // any result type, but that promise cannot change which machine was called.
    let carriers = ["bool", "i8", "u8", "u16", "u32"];
    let overloads = carriers.map(|ty| format!("machine draw(x: {ty}) -> {ty} {{ x }}"));
    for declarations in [
        overloads.join("\n"),
        overloads.iter().rev().cloned().collect::<Vec<_>>().join("\n"),
    ] {
        for argument in carriers {
            for result in carriers {
                let source = format!("{declarations}\nmachine render(x: {argument}) -> {result} {{ draw(x) }}");
                if argument == result {
                    let stage = check(&source).unwrap();
                    let [selected] = stage.calls.as_slice() else {
                        panic!("expected one call for {argument}");
                    };
                    assert_eq!(selected.machine.operands, [argument]);
                    assert_eq!(selected.result.to_string(), result);
                } else {
                    assert_eq!(check(&source).unwrap_err(), format!("app.omg: expected {result}, found {argument}"));
                }
            }
        }
    }
}
