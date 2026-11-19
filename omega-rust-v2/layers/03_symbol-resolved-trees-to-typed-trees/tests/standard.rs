#[path = "support/evaluation.rs"]
mod evaluation;
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
fn specialized_call_results_match_the_owning_expression_and_arguments() {
    use omega_typer::{Site, SpecializedIndexValue, TypingEnvironment, ValueType, type_stage_in_environment};
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
    let start = source.find("width(N)").unwrap();
    let u8_type = ValueType::Integer { signed: false, width: Some(8) };
    let argument = |n: u8| ValueType::StaticInteger {
        carrier: Box::new(u8_type.clone()),
        value: n.into(),
    };
    let run = |values: &[SpecializedIndexValue]| {
        type_stage_in_environment(
            &names,
            &TypingEnvironment {
                specialized_indices: values,
                ..Default::default()
            },
        )
    };
    let mut value = SpecializedIndexValue {
        request: omega_typer::StaticValueRequest {
            node: evaluation::node(source, start, "width(N)", omega_parser::NodeKind::Call),
            expression: Site {
                package: "app".into(),
                file: "receipt.omg".into(),
                span: omega_lexer::SourceSpan::new(start, start + "width(N)".len()),
            },
            carrier: omega_typer::ValueType::Integer { signed: false, width: Some(8) },
        },
        arguments: vec![argument(6)],
        value: omega_typer::StaticValue::Integer(6.into()),
    };
    assert!(
        run(std::slice::from_ref(&value))
            .unwrap_err()
            .message
            .contains("needs an admitted evaluation")
    );
    value.arguments[0] = argument(7);
    run(std::slice::from_ref(&value)).unwrap();
    let mut misaddressed = value.clone();
    misaddressed.request.node = evaluation::node(source, source.rfind("width(N)").unwrap(), "width(N)", omega_parser::NodeKind::Call);
    let error = run(&[misaddressed]).unwrap_err();
    assert_eq!(error.span, value.request.expression.span);
    assert_eq!(error.message, "evaluation result refers to an unknown static expression");
    value.value = omega_typer::StaticValue::Integer(256.into());
    assert!(run(&[value]).unwrap_err().message.contains("exceeds u8"));
}
#[test]
fn specialization_uses_the_selected_trait_operation() {
    use omega_typer::{Site, SpecializedIndexValue, TypingEnvironment, ValueType, type_stage_in_environment};
    let source = "trait Add{machine + combine(x:u8,y:u8)->u8;} trait Bound:Add{} Selected satisfies Bound{machine combine(x:u8,y:u8)->u8{x}} data B<const K:u8>{} machine buffer<const N:u8,const E:Bound>()->B<N+1>{B::<N+1>{}} machine use_it()->B<7>{buffer::<7,Selected>()}";
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
        request: omega_typer::StaticValueRequest {
            node: evaluation::node(source, start, "N+1", omega_parser::NodeKind::Binary),
            expression: Site {
                package: "app".into(),
                file: "op.omg".into(),
                span: omega_lexer::SourceSpan::new(start, start + 3),
            },
            carrier: omega_typer::ValueType::Integer { signed: false, width: Some(8) },
        },
        arguments: vec![
            ValueType::StaticInteger {
                carrier: Box::new(ValueType::Integer { signed: false, width: Some(8) }),
                value: 7.into(),
            },
            ValueType::Implementation(Box::new(selected)),
        ],
        value: omega_typer::StaticValue::Integer(7.into()),
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
    wrong.value = omega_typer::StaticValue::Integer(8.into());
    assert!(
        check(&[value, wrong])
            .unwrap_err()
            .message
            .contains("conflicting evaluations")
    );
}
#[test]
fn unrelated_trait_operators_preserve_primitive_static_arithmetic() {
    let source = "trait Other{machine + combine(x:u8,y:u8)->u8;} trait Marker{} Selected satisfies Marker{} data B<const K:u8>{} machine buffer<const N:u8,const E:Marker>()->B<N+1>{B::<N+1>{}} machine use_it()->B<8>{buffer::<7,Selected>()}";
    check(source).unwrap();
    assert!(check(&source.replace("->B<8>", "->B<7>")).is_err());
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
fn the_checked_core_uses_the_same_descending_rank_explicitly_and_by_default() {
    let source = "
        domain u64::Count requires self>0;
        machine implicit(n:u64) terminates by n in 0..=256 {}
        machine explicit(n:u64) terminates by n -> omega::language::core::Nat::Descending in 0..=256 {}
        machine unqualified_default(n:u64 in Count) terminates by (n as -Count) {}
        machine unqualified_explicit(n:u64 in Count) terminates by (n as -Count) -> omega::language::core::Nat::Descending {}
        machine distance(lo:u64,hi:u64) terminates by (lo,hi) -> omega::language::core::Nat::BoundedDistance in 0..=256 {}";
    let stage = check(source).unwrap();
    assert_eq!(stage.ranking_witnesses.len(), 5);
    let core = omega_toolchain::core_interface();
    let rank = omega_typer::ValueType::Integer {
        signed: false,
        width: Some(64),
    };
    for (machine, view) in [
        ("implicit", "Nat::Descending"),
        ("explicit", "Nat::Descending"),
        ("unqualified_default", "Nat::Descending"),
        ("unqualified_explicit", "Nat::Descending"),
        ("distance", "Nat::BoundedDistance"),
    ] {
        let witness = stage
            .ranking_witnesses
            .iter()
            .find(|witness| witness.machine.path == machine)
            .unwrap();
        assert_eq!(&witness.view, core.id(view));
        assert_eq!(witness.rank, rank);
    }

    let error = check(&source.replace(" -> omega::language::core::Nat::BoundedDistance", "")).unwrap_err();
    assert!(error.contains("carrier has no admitted canonical ranking view"), "{error}");
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
fn raw_stages_cannot_define_sealed_toolchain_operations() {
    let text = "machine read() {}";
    let tokens = omega_lexer::lex(text.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(text.as_bytes(), &tokens).unwrap();
    let input = StageSource {
        package: omega_toolchain::TOOLCHAIN_PACKAGE,
        source: ParsedSource {
            logical_path: "indexing/read.omg",
            text,
            tokens: &tokens,
            tree: &tree,
        },
    };
    let bare = omega_resolver::resolve_stage(&[input], &[]).unwrap();
    let error = omega_typer::type_stage(&bare).unwrap_err();
    assert_eq!(error.file, "indexing/read.omg");
    assert!(error.message.contains("reserved"));
}

/// Read the predicates at the declaration issued by L2. Every root must also have L3's
/// Boolean operation at that exact node, so a retained source string alone cannot pass.
fn typed_predicates(
    library: &omega_typer::TypedTrees<'_>,
    machine: &omega_resolver::DeclarationId,
    clause: omega_parser::NodeKind,
) -> Vec<String> {
    for tree in library.trees() {
        let file = tree.resolved().file;
        if let Some((node, _)) = tree
            .resolved()
            .declaration_nodes()
            .find(|(_, id)| *id == machine)
        {
            return file.nodes[node]
                .children
                .iter()
                .filter(|&&n| file.nodes[n].kind == clause)
                .map(|&n| {
                    let expr = file.nodes[n].children[0];
                    assert!(
                        tree.semantics_at(expr)
                            .any(|s| matches!(s, omega_typer::NodeSemantics::Operation(op) if op.result == omega_typer::ValueType::Bool))
                    );
                    let range = &file.nodes[expr];
                    file.tokens[range.first_token..range.end_token]
                        .iter()
                        .map(|token| token.text)
                        .collect()
                })
                .collect();
        }
    }
    panic!("missing library declaration {machine:?}")
}

#[test]
fn core_interfaces_supply_checked_types_without_adding_source_bodies() {
    use omega_parser::NodeKind::{Ensures, Requires};
    let names = resolve_source_package(&[]).unwrap();
    assert!(names.uses().next().is_none());
    let stage = type_source_package(&names).unwrap();
    assert!(stage.expressions.is_empty());
    let library = stage.library.expect("admitted library");
    let core = omega_toolchain::core_interface();
    let machine = stage
        .signatures
        .iter()
        .find(|s| &s.machine == core.id("access::new_plan"))
        .unwrap();
    let producer = library
        .signatures
        .iter()
        .find(|s| s.machine == machine.machine)
        .unwrap();
    assert!(std::sync::Arc::ptr_eq(machine, producer));
    assert!(machine.has_guarantee && machine.terminates);
    assert_eq!(machine.pause.block, Some(false));
    assert_eq!(machine.crash_ceiling.as_deref(), Some([].as_slice()));
    assert_eq!(machine.reach.as_deref(), Some([].as_slice()));
    assert_eq!(stage.machine_effects.len(), library.machine_effects.len());
    for (imported, checked) in stage.machine_effects.iter().zip(&library.machine_effects) {
        assert!(std::sync::Arc::ptr_eq(imported, checked));
    }
    for (owner, fields) in &library.record_fields {
        assert!(std::sync::Arc::ptr_eq(fields, &stage.record_fields[owner]), "{owner:?}");
    }
    assert_eq!(
        typed_predicates(library, &machine.machine, Ensures),
        [
            "result.field_count==0",
            "result.reader_count==0",
            "result.writer_count==0",
            "result.reach.service_count==0"
        ]
    );
    assert_eq!(
        typed_predicates(library, core.id("AccessPlan"), Requires),
        [
            "self.field_count<=32",
            "self.reader_count<=32",
            "self.writer_count<=32"
        ]
    );
    assert_eq!(typed_predicates(library, core.id("BoundaryReach"), Requires), ["self.service_count<=32"]);
    assert!(
        stage
            .obligations
            .iter()
            .any(|o| o.site == machine.site && matches!(&o.kind, omega_typer::ObligationKind::VerifyLibraryImplementation(id) if id == &machine.machine))
    );
}

#[test]
fn bracket_contracts_publish_exact_bounds_and_range_length() {
    use omega_parser::NodeKind::{Ensures, Requires};
    use omega_toolchain::CollectionPrimitive::*;
    let stage = check("machine read(view:@[u8])->u8{view[0]}").unwrap();
    let selected = &stage.collection_accesses[0].machine;
    let library = stage.library.unwrap();
    let core = omega_toolchain::core_interface();
    assert_eq!(selected, core.collection(Read));
    for op in [Read, Element, ElementMut] {
        assert_eq!(typed_predicates(library, core.collection(op), Requires), ["index<items.len"]);
    }
    for op in [Range, RangeMut] {
        assert_eq!(typed_predicates(library, core.collection(op), Requires), ["start<=end", "end<=items.len"]);
        assert_eq!(typed_predicates(library, core.collection(op), Ensures), ["result.len==end-start"]);
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

#[test]
fn grouping_preserves_the_typed_equality_operation() {
    for (left, right, operation) in [
        ("T", "u32", "type =="),
        ("addr", "addr", "type =="),
        ("x", "x", "=="),
    ] {
        let mut baseline = None;
        for predicate in [
            format!("{left} == {right}"),
            format!("({left}) == {right}"),
            format!("{left} == ({right})"),
            format!("(({left})) == (({right}))"),
        ] {
            let source = format!("machine f<T>(x:u32) requires {predicate} {{}}");
            let typed = check(&source).unwrap();
            let [op] = typed.operations.as_slice() else {
                panic!("expected one operation: {predicate}")
            };
            assert_eq!(op.token, operation);
            assert_eq!(op.result, omega_typer::ValueType::Bool);
            assert_eq!(op.site.span.slice(source.as_bytes()), Some(predicate.as_bytes()));
            assert_eq!(baseline.get_or_insert_with(|| op.operands.clone()), &op.operands);
        }
    }
}

#[test]
fn receipt_admission_rejects_an_open_parameter_in_each_type_child() {
    use ValueType as T;
    use omega_typer::{
        Access, CallableType, DomainApplication, IntegerIndex, RequirementSlot, ServiceApplication, Site, SpecializedIndexValue, TypingEnvironment, ValueType,
        type_stage_in_environment,
    };
    let source = "data B<const N:u8>{} machine width<T>()->u8{1} machine probe<T>()->B<width::<T>()>{B::<width::<T>()>{}}";
    let tokens = omega_lexer::lex(source.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(source.as_bytes(), &tokens).unwrap();
    let resolved = omega_resolver::resolve_stage(
        &[StageSource {
            package: "app",
            source: ParsedSource {
                logical_path: "app.omg",
                text: source,
                tokens: &tokens,
                tree: &tree,
            },
        }],
        &[],
    )
    .unwrap();
    let owner = resolved.declarations.iter().find(|id| id.path == "B").unwrap();
    let start = source.find("width::<T>()").unwrap();
    let site = Site {
        package: "app".into(),
        file: "app.omg".into(),
        span: omega_lexer::SourceSpan::new(start, start + "width::<T>()".len()),
    };
    let callable = CallableType {
        parameters: vec![T::Bool],
        result: T::Bool,
        requirement: None,
        reach: None,
        contract: site.clone(),
        closed_runtime: false,
        pause: Default::default(),
        has_guarantee: false,
        terminates: false,
        crash_ceiling: None,
    };
    let shapes = |child: T| {
        let boxed = || Box::new(child.clone());
        let children = || vec![T::Bool, child.clone()];
        let slot = || RequirementSlot {
            declaration: owner.clone(),
            trait_arguments: children(),
        };
        let domain = || DomainApplication {
            declaration: owner.clone(),
            arguments: children(),
        };
        vec![
            ("direct", child.clone()),
            ("squash", T::Squash(boxed())),
            ("level of", T::LevelOf(boxed())),
            ("universe level", T::Universe { strict: false, level: boxed() }),
            ("integer width", T::IndexedInteger { signed: false, width: boxed() }),
            ("reference", T::Reference(Access::Shared, boxed())),
            ("array", T::Array(boxed(), 1)),
            ("slice", T::Slice(boxed())),
            (
                "integer carrier",
                T::StaticInteger {
                    carrier: boxed(),
                    value: 1.into(),
                },
            ),
            ("function input", T::Function(boxed(), Box::new(T::Bool))),
            ("function output", T::Function(Box::new(T::Bool), boxed())),
            ("value at lifetime", T::AtLifetime(boxed(), Box::new(T::Bool))),
            ("lifetime", T::AtLifetime(Box::new(T::Bool), boxed())),
            ("indexed array element", T::IndexedArray(boxed(), Box::new(T::Bool))),
            ("array index", T::IndexedArray(Box::new(T::Bool), boxed())),
            (
                "mathematical subject",
                T::MathematicalValue {
                    definition: Box::new(owner.clone()),
                    subjects: children(),
                },
            ),
            (
                "family",
                T::FamilyApplication {
                    family: boxed(),
                    subjects: vec![],
                },
            ),
            (
                "family subject",
                T::FamilyApplication {
                    family: Box::new(T::Bool),
                    subjects: children(),
                },
            ),
            ("tuple", T::Tuple(children())),
            ("type argument", T::Applied(Box::new(owner.clone()), children())),
            ("domain carrier", T::Qualified(boxed(), vec![])),
            ("domain argument", T::Qualified(Box::new(T::Bool), vec![domain()])),
            ("evidence", T::Evidence(Box::new(slot()))),
            (
                "callable input",
                T::Callable(Box::new(CallableType {
                    parameters: children(),
                    ..callable.clone()
                })),
            ),
            (
                "callable result",
                T::Callable(Box::new(CallableType {
                    result: child.clone(),
                    ..callable.clone()
                })),
            ),
            (
                "callable requirement",
                T::Callable(Box::new(CallableType {
                    requirement: Some(slot()),
                    ..callable.clone()
                })),
            ),
            (
                "callable reach",
                T::Callable(Box::new(CallableType {
                    reach: Some(vec![ServiceApplication {
                        service: owner.clone(),
                        arguments: children(),
                    }]),
                    ..callable.clone()
                })),
            ),
        ]
    };
    let admit = |arg| {
        let receipt = SpecializedIndexValue {
            request: omega_typer::StaticValueRequest {
                node: evaluation::node(source, start, "width::<T>()", omega_parser::NodeKind::Call),
                expression: site.clone(),
                carrier: omega_typer::ValueType::Integer { signed: false, width: Some(8) },
            },
            arguments: vec![arg],
            value: omega_typer::StaticValue::Integer(1.into()),
        };
        type_stage_in_environment(
            &resolved,
            &TypingEnvironment {
                specialized_indices: std::slice::from_ref(&receipt),
                ..Default::default()
            },
        )
        .map(|_| ())
    };
    let parameter = T::Parameter {
        owner: Box::new(owner.clone()),
        index: 0,
    };
    for ((position, open), (_, closed)) in shapes(parameter.clone()).into_iter().zip(shapes(T::Bool)) {
        let refusal = admit(open).expect_err(position);
        assert_eq!(refusal.message, "evaluation requires closed arguments", "{position}");
        assert_eq!(refusal.span, site.span, "{position}");
        admit(closed).expect(position);
    }
    // A formula remains deferred until evaluation materializes its static integer.
    let deferred = T::StaticFormula {
        term: IntegerIndex::Binder {
            owner: Box::new(owner.clone()),
            index: 0,
        },
        carrier: Box::new(T::Bool),
    };
    assert_eq!(admit(deferred).unwrap_err().message, "evaluation requires closed arguments");
}

#[test]
fn static_call_selection_uses_the_normal_typer_for_each_overload() {
    use omega_typer::{IntegerIndex, NodeSemantics, ValueType};
    for other in [
        "machine width(x:bool)->bool{x}",
        "machine width(x:u8,y:u8)->u8{x+y}",
        "machine width(x:u16)->u16{width(x)}",
    ] {
        let source = format!("data B<const N:u8>{{}} machine width(x:u8)->u8{{x}} {other} machine via<const N:u8>()->B<width(N)>{{B::<width(N)>{{}}}}");
        let tokens = omega_lexer::lex(source.as_bytes()).unwrap();
        let tree = omega_parser::parse_file(source.as_bytes(), &tokens).unwrap();
        let names = resolve_source_package(&[StageSource {
            package: "app",
            source: ParsedSource {
                logical_path: "static.omg",
                text: &source,
                tokens: &tokens,
                tree: &tree,
            },
        }])
        .unwrap();
        let product = type_source_package(&names).unwrap();
        let sig = product
            .signatures
            .iter()
            .find(|sig| sig.machine.path == "via")
            .unwrap();
        let ValueType::Applied(_, args) = &sig.result else { panic!("applied result") };
        let [
            ValueType::StaticFormula {
                term: IntegerIndex::Deferred { template, .. },
                ..
            },
        ] = args.as_slice()
        else {
            panic!("deferred call")
        };
        let request = template.request();
        let typed = product.trees().nth(request.node.file).unwrap();
        assert!(
            typed
                .semantics_at(request.node.node)
                .any(|record| matches!(record, NodeSemantics::Call(c) if c.machine.path == "width" && c.machine.operands == ["u8"]))
        );
    }
}

#[test]
fn each_collection_signature_carries_the_published_primitive_contract() {
    use omega_toolchain::CollectionPrimitive::*;
    let typed = omega_typer::standard_library();
    let core = omega_toolchain::core_interface();
    for (primitive, access, result, arity) in [
        (Read, "@", "value", 2),
        (Element, "@", "@", 2),
        (ElementMut, "@mut", "@mut", 2),
        (Range, "@", "slice", 3),
        (RangeMut, "@mut", "slice", 3),
    ] {
        let id = core.collection(primitive);
        let signature = typed.signatures.iter().find(|s| &s.machine == id).unwrap();
        let parameter = omega_typer::ValueType::Parameter {
            owner: Box::new(id.clone()),
            index: 0,
        };
        let mode = if access == "@" {
            omega_typer::Access::Shared
        } else {
            omega_typer::Access::Mut
        };
        let view = omega_typer::ValueType::Reference(mode.clone(), Box::new(omega_typer::ValueType::Slice(Box::new(parameter.clone()))));
        assert_eq!(signature.parameters[0].1, view);
        assert_eq!(signature.parameters.len(), arity);
        for (_, ty) in &signature.parameters[1..] {
            assert_eq!(ty.to_string(), "u64");
        }
        assert_eq!(
            signature.result,
            match result {
                "slice" => view,
                "value" => parameter,
                _ => omega_typer::ValueType::Reference(mode, Box::new(parameter)),
            }
        );
        let bounds: &[_] = if primitive == Read { &[(0, omega_typer::Multiplicity::Copy)] } else { &[] };
        assert_eq!(signature.multiplicity_bounds, bounds);
        let effects = typed
            .machine_effects
            .iter()
            .find(|effects| &effects.machine == id)
            .unwrap();
        assert!(effects.reach.is_empty() && effects.invokes.is_empty());
        assert_eq!(effects.crash_ceiling.as_deref(), Some(&[][..]));
    }
}

#[test]
fn named_and_bracket_reads_use_the_same_library_signature_and_copy_bound() {
    let source = "use omega::indexing; machine f(view:@[u8], i:u64)->u8{let n=indexing::read::<u8>(view,i); view[i]}";
    let stage = check(source).unwrap();
    let [named] = stage.calls.as_slice() else { panic!("expected the named call") };
    let [bracket] = stage.collection_accesses.as_slice() else {
        panic!("expected the bracket access")
    };
    assert_eq!(named.machine, bracket.machine);
    assert_eq!(named.result, bracket.result);
    assert_eq!(named.type_arguments.as_slice(), std::slice::from_ref(&bracket.element));
    assert!(
        named
            .operands
            .iter()
            .all(|operand| operand.parameter.file == "indexing/collections.omg")
    );
    for site in [&named.site, &bracket.site] {
        assert!(
            stage
                .obligations
                .iter()
                .any(|o| &o.site == site && matches!(&o.kind, omega_typer::ObligationKind::CallPrecondition(id) if id == &named.machine))
        );
    }
    for expression in ["indexing::read::<Handle>(view,i)", "view[i]"] {
        let source = format!("use omega::indexing; data Handle{{}} machine f(view:@[Handle],i:u64)->Handle{{{expression}}}");
        assert!(check(&source).is_err(), "{source}");
    }
}
