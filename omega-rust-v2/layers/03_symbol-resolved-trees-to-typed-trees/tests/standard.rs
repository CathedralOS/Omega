#[cfg(feature = "diagnostic-origins")]
#[path = "support/diagnostic_origin.rs"]
mod diagnostic_origin;
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
/// Negative typing cases must reach the typer with their original source intact.
fn assert_typing_refusal(source: &str, file: &str, expression: &str, site: &str, message: &str) {
    let tokens = omega_lexer::lex(source.as_bytes()).expect("the negative package must lex");
    let tree = omega_parser::parse_file(source.as_bytes(), &tokens).expect("the negative package must parse");
    let names = resolve_source_package(&[StageSource {
        package: "app",
        source: ParsedSource { logical_path: file, text: source, tokens: &tokens, tree: &tree },
    }]).expect("the negative package must resolve");
    let error = type_source_package(&names).expect_err("the typer must refuse this expression");
    let start = source.rfind(expression).unwrap() + expression.find(site).unwrap();
    let end = start + site.len();
    assert!(!error.waiting_for_static_value());
    assert_eq!((&*error.package, &*error.file, &*error.message), ("app", file, message));
    assert_eq!((error.span.get_start_offset(), error.span.get_end_offset()), (start, end));
    #[cfg(feature = "diagnostic-origins")]
    diagnostic_origin::report_return(&error, "", Some(("app", file, start, end, message)));
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
    spec_corpus::observe_handoff!(
        "matching-computed-index-results/refusal",
        assert_eq!(error.message, "evaluation result refers to an unknown static expression")
    );
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
    spec_corpus::observe_handoff!(
        "dependent-applications/producer",
        assert!(
            stage
                .calls
                .iter()
                .any(|c| c.machine.path == "Add::combine" && c.evidence.is_some() && c.use_site == omega_typer::CallUse::StaticArgument)
        )
    );
    let mut wrong = value.clone();
    wrong.value = omega_typer::StaticValue::Integer(8.into());
    spec_corpus::observe_handoff!(
        "dependent-applications/refusal",
        assert!(
            check(&[value, wrong])
                .unwrap_err()
                .message
                .contains("conflicting evaluations")
        )
    );
}
#[test]
fn unrelated_trait_operators_preserve_primitive_static_arithmetic() {
    let source = "trait Other{machine + combine(x:u8,y:u8)->u8;} trait Marker{} Selected satisfies Marker{} data B<const K:u8>{} machine buffer<const N:u8,const E:Marker>()->B<N+1>{B::<N+1>{}} machine use_it()->B<8>{buffer::<7,Selected>()}";
    check(source).unwrap();
    let call = "buffer::<7,Selected>()";
    assert_typing_refusal(&source.replace("->B<8>", "->B<7>"), "app.omg", call, call, r"expected app\B<7>, found app\B<8>");
}
#[test]
fn production_core_selects_the_policy_and_ranking_declarations() {
    let source = "use omega\\language\\core; use omega\\language\\core\\reflect; use omega\\language\\core\\access; Rule satisfies AccessPolicy { machine plan<'t>(schema:TypeSchema<'t>)->AccessPlan<'t> reaches none crashes none terminates {new_plan::<'t>(@schema, Issuance::Unrestricted{})} } data C{} domain C::Locked access Rule; machine visit(n:u64) terminates by n in 0..=9 {}";
    let stage = check(source).unwrap();
    assert_eq!(stage.access_policies.len(), 1);
    assert_eq!(stage.ranking_witnesses[0].view.package, "@omega/toolchain");
    assert!(
        stage
            .calls
            .iter()
            .any(|c| c.machine.path == "language\\core\\access\\new_plan")
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
        machine explicit(n:u64) terminates by n -> omega\\language\\core\\Nat\\Descending in 0..=256 {}
        machine unqualified_default(n:u64 in Count) terminates by (n as -Count) {}
        machine unqualified_explicit(n:u64 in Count) terminates by (n as -Count) -> omega\\language\\core\\Nat\\Descending {}
        machine distance(lo:u64,hi:u64) terminates by (lo,hi) -> omega\\language\\core\\Nat\\BoundedDistance in 0..=256 {}";
    let stage = check(source).unwrap();
    assert_eq!(stage.ranking_witnesses.len(), 5);
    let core = omega_toolchain::core_interface();
    let rank = omega_typer::ValueType::Integer {
        signed: false,
        width: Some(64),
    };
    for (machine, view) in [
        ("implicit", "Nat\\Descending"),
        ("explicit", "Nat\\Descending"),
        ("unqualified_default", "Nat\\Descending"),
        ("unqualified_explicit", "Nat\\Descending"),
        ("distance", "Nat\\BoundedDistance"),
    ] {
        let witness = stage
            .ranking_witnesses
            .iter()
            .find(|witness| witness.machine.path == machine)
            .unwrap();
        assert_eq!(&witness.view, core.id(view));
        assert_eq!(witness.rank, rank);
    }

    let error = check(&source.replace(" -> omega\\language\\core\\Nat\\BoundedDistance", "")).unwrap_err();
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
fn direct_boundary_entry_checks_the_callers_invocation_ceiling() {
    use omega_typer::{BoundarySlot, RequirementSlot, Site, SlotDirection, TypingEnvironment, type_stage_in_environment};

    let source = "trait Host{machine enter();} trait Other{machine tick();} machine direct() invokes Other {Host::enter()}";
    let file = "slot.omg";
    let tokens = omega_lexer::lex(source.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(source.as_bytes(), &tokens).unwrap();
    let sources = [StageSource {
        package: "app",
        source: ParsedSource { logical_path: file, text: source, tokens: &tokens, tree: &tree },
    }];
    let names = omega_resolver::resolve_stage(&sources, &[]).unwrap();
    let outbound = BoundarySlot {
        requirement: RequirementSlot {
            declaration: names.declarations.iter().find(|id| id.path == "Host::enter").unwrap().clone(),
            trait_arguments: vec![],
        },
        arguments: vec![],
        direction: SlotDirection::ProgramToProvider,
    };
    let inbound = BoundarySlot { direction: SlotDirection::EnvironmentToProgram, ..outbound.clone() };
    let run = |slots: &[BoundarySlot]| type_stage_in_environment(&names, &TypingEnvironment { boundary_slots: slots, ..Default::default() });
    let call = "Host::enter()";
    let start = source.find(call).unwrap();
    let end = start + call.len();
    let site = Site { package: "app".into(), file: file.into(), span: omega_lexer::SourceSpan::new(start, end) };

    // The outbound control must match the applied slot, not merely type as an ordinary call.
    let stage = run(std::slice::from_ref(&outbound)).unwrap();
    assert_eq!(stage.boundary_calls, [(site.clone(), outbound.clone())]);

    let refused = |slots: &[BoundarySlot], message: &str| {
        let error = run(slots).expect_err("the boundary call must be refused");
        assert!(!error.waiting_for_static_value());
        assert_eq!((&*error.package, &*error.file, error.span, &*error.message), ("app", file, site.span, message));
        #[cfg(feature = "diagnostic-origins")]
        diagnostic_origin::report_return(&error, "", Some(("app", file, start, end, message)));
    };
    refused(std::slice::from_ref(&inbound), "call exceeds the declared synchronous invocation ceiling");
    refused(&[outbound, inbound], "competing directions for an applied boundary slot");
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
    let inputs = [input];
    let bare = omega_resolver::resolve_stage(&inputs, &[]).unwrap();
    let refusal = omega_toolchain::resolve_source_package(&inputs).expect_err("reserved package must be refused");
    let kind = omega_resolver::NameErrorKind::ReservedPackageIdentity;
    spec_corpus::assert_resolver_refusal!(
        refusal,
        kind,
        omega_toolchain::TOOLCHAIN_PACKAGE,
        "indexing/read.omg",
        omega_lexer::SourceSpan::new(0, 0),
        "package identity reserved for the toolchain"
    );
    let error = omega_typer::type_stage(&bare).unwrap_err();
    assert_eq!(
        (error.package, error.file, error.span, error.message),
        (refusal.package, refusal.file, refusal.span, refusal.message)
    );
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
        .find(|s| &s.machine == core.id("access\\new_plan"))
        .unwrap();
    let producer = library
        .signatures
        .iter()
        .find(|s| s.machine == machine.machine)
        .unwrap();
    spec_corpus::observe_handoff!("typing-from-resolved-bindings/producer", assert!(std::sync::Arc::ptr_eq(machine, producer)));
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
    // The body's guarantees stay in the library forest; the empty importer owes nothing.
    assert!(stage.obligations.is_empty(), "{:?}", stage.obligations);
    assert_eq!(guarantee_rows(library, &machine.machine).len(), 4);
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
        requires: false,
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
    let admit_value = |arg, value| {
        let receipt = SpecializedIndexValue {
            request: omega_typer::StaticValueRequest {
                node: evaluation::node(source, start, "width::<T>()", omega_parser::NodeKind::Call),
                expression: site.clone(),
                carrier: omega_typer::ValueType::Integer { signed: false, width: Some(8) },
            },
            arguments: vec![arg],
            value,
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
    let admit = |arg| admit_value(arg, omega_typer::StaticValue::Integer(1.into()));
    // This generic body has no closed caller. Its supplied receipt still has to inhabit u8.
    for value in [
        omega_typer::StaticValue::Integer(256.into()),
        omega_typer::StaticValue::Bool(true),
    ] {
        let err = admit_value(T::Bool, value).unwrap_err();
        spec_corpus::observe_handoff!(
            "generic-parameter-correspondence/refusal",
            assert_eq!(err.message, "specialized evaluation result does not match its carrier")
        );
        assert_eq!(err.span, site.span);
    }
    let parameter = T::Parameter {
        owner: Box::new(owner.clone()),
        index: 0,
    };
    for ((position, open), (_, closed)) in shapes(parameter.clone()).into_iter().zip(shapes(T::Bool)) {
        let refusal = admit(open).expect_err(position);
        assert_eq!(refusal.message, "evaluation requires closed arguments", "{position}");
        assert_eq!(refusal.span, site.span, "{position}");
        // These synthetic shapes isolate structural closure. Some have no valid source
        // spelling, such as an integer value whose carrier was replaced with bool.
        omega_typed::ClosedArguments::try_from(vec![closed]).expect(position);
    }
    for closed in [T::Bool, T::Tuple(vec![T::Bool]), T::Array(Box::new(T::Bool), 4)] {
        admit(closed).expect("a closed type argument can name an unused application");
    }
    let invalid_integer = T::StaticInteger {
        carrier: Box::new(T::Bool),
        value: 1.into(),
    };
    let refusal = admit(T::Tuple(vec![invalid_integer])).expect_err("integer with a Boolean carrier inside a closed tuple");
    assert_eq!(
        refusal.message,
        "invalid evaluation argument 1 of probe: static integer 1 has non-integer carrier bool"
    );
    assert_eq!(refusal.span, site.span);
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
    let source = "use omega\\indexing; machine f(view:@[u8], i:u64)->u8{let n=indexing\\read::<u8>(view,i); view[i]}";
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
    for (expression, site, message) in [
        (r"indexing\read::<Handle>(view,i)", r"indexing\read::<Handle>", r"type argument app\Handle does not satisfy Copy for indexing\read"),
        ("view[i]", "view[i]", "indexed read requires a copy element"),
    ] {
        let source = format!("use omega\\indexing; data Handle{{}} machine f(view:@[Handle],i:u64)->Handle{{{expression}}}");
        check(&source.replace("data Handle{}", "data Handle [copy]{}")).expect("the copyable element must type");
        assert_typing_refusal(&source, "app.omg", expression, site, message);
    }
}

#[test]
fn symbolic_queries_check_the_operand_and_cannot_escape_as_callbacks() {
    for (expression, site, message) in [
        (r"reflect\field(read)", "read", "expected a field declaration"),
        (r"reflect\callable(R::x)", "R::x", "expected a callable declaration"),
        (r"reflect\callable(read())", "read()", "expected a callable declaration"),
        (r"reflect\field(1)", "1", "expected a field declaration"),
        (r"reflect\field()", r"reflect\field()", "a declaration query requires one declaration path"),
        (
            r"reflect\callable(read, read)",
            r"reflect\callable(read, read)",
            "a declaration query requires one declaration path",
        ),
        (r"reflect\field", r"reflect\field", "a declaration query cannot be used as a value"),
    ] {
        let text = format!(r"use omega\language\core\reflect; data R{{x:u8;}} machine read()->u8{{1}} machine f(){{let d={expression};}}");
        assert_typing_refusal(&text, "selector.omg", expression, site, message);
    }
    check(r"machine field(n:u8)->u8{n} machine use_field()->u8{field(1)}").unwrap();
    check(r"use omega\language\core\reflect; machine shadow(field:u8)->u8{field}").unwrap();
}

/// The access policy program of the L5 access-plan tests, byte for byte. `Rule::plan`
/// calls the toolchain's `new_plan`, so it really selects that library machine.
const ACCESS_PLAN: &str = "use omega\\language\\core;
                  use omega\\language\\core\\reflect;
                  use omega\\language\\core\\access;
                  domain i32::Locked access Rule;
                  Rule satisfies AccessPolicy {
                      machine plan<'t>(schema:TypeSchema<'t>)->AccessPlan<'t> reaches none crashes none terminates {
                          new_plan::<'t>(@schema, Issuance::Unrestricted{})
                      }
                  }";

/// A caller whose own precondition is discharged only by `new_plan`'s first guarantee
/// (`target/l5-loop-01/integration/core-owner-integration/run/relies/policy.omg`, byte for byte).
const RELIES: &str = "use omega\\language\\core;
use omega\\language\\core\\reflect;
use omega\\language\\core\\access;
machine need_empty(count: u64) requires count == 0 reaches none crashes none terminates {}
Rule satisfies AccessPolicy {
    machine plan<'t>(schema:TypeSchema<'t>)->AccessPlan<'t> reaches none crashes none terminates {
        let made: AccessPlan<'t> = new_plan::<'t>(@schema, Issuance::Unrestricted{});
        need_empty(made.field_count);
        made
    }
}
";

/// The toolchain's own source of the library declaration.
const CORE_ACCESS: &str = include_str!("../../../toolchain/core/access.omg");

/// Type one `app` package through the standard profile.
fn check_files(files: &[(&str, &str)]) -> Result<TypedStage, omega_typer::TypeError> {
    let lexed: Vec<_> = files
        .iter()
        .map(|(path, text)| {
            let tokens = omega_lexer::lex(text.as_bytes()).expect("the package must lex");
            let tree = omega_parser::parse_file(text.as_bytes(), &tokens).expect("the package must parse");
            (*path, *text, tokens, tree)
        })
        .collect();
    let inputs: Vec<_> = lexed
        .iter()
        .map(|(path, text, tokens, tree)| StageSource {
            package: "app",
            source: ParsedSource {
                logical_path: path,
                text,
                tokens,
                tree,
            },
        })
        .collect();
    let resolved = resolve_source_package(&inputs).expect("the package must resolve");
    type_source_package(&resolved).map(|trees| trees.into_facts())
}

/// Rows in an importer's obligation list whose site lies in the toolchain package.
fn library_rows(obligations: &[omega_typer::TypingObligation]) -> Vec<&omega_typer::TypingObligation> {
    obligations
        .iter()
        .filter(|o| o.site.package == omega_toolchain::TOOLCHAIN_PACKAGE)
        .collect()
}

/// `ProveGuarantee` rows that name `machine`, in a forest's own obligation list.
fn guarantee_rows<'a>(
    forest: &'a TypedStage,
    machine: &omega_resolver::DeclarationId,
) -> Vec<&'a omega_typer::TypingObligation> {
    forest
        .obligations
        .iter()
        .filter(|o| matches!(&o.kind, omega_typer::ObligationKind::ProveGuarantee { machine: owner, .. } if owner == machine))
        .collect()
}

fn spelled<'a>(
    text: &'a str,
    site: &omega_typer::Site,
) -> &'a str {
    std::str::from_utf8(site.span.slice(text.as_bytes()).expect("site inside its file")).unwrap()
}

/// The `new_plan` signature as the toolchain file spells it, without its body.
fn core_signature() -> &'static str {
    let start = CORE_ACCESS.find("public machine new_plan").unwrap();
    let end = CORE_ACCESS[start..].find("\n{").unwrap() + start;
    &CORE_ACCESS[start..end]
}

/// The complete `new_plan` declaration, body included.
fn core_declaration() -> &'static str {
    CORE_ACCESS[CORE_ACCESS.find("public machine new_plan").unwrap()..].trim_end()
}

fn calls_from<'a>(
    stage: &'a TypedStage,
    caller: &str,
) -> Vec<&'a omega_resolver::DeclarationId> {
    stage
        .calls
        .iter()
        .filter(|c| c.caller.as_ref().is_some_and(|m| m.path == caller))
        .map(|c| &c.machine)
        .collect()
}

/// `new_plan` owes its four guarantees as `ProveGuarantee` rows of its own body, sited in the
/// toolchain file and kept in the library forest. A normal package that selects it and one
/// that never calls it import none of those rows: the selecting package's dependency is the
/// call, which names the library declaration. Copying a library row into an importer, as the
/// baseline did, is visible to the same reader.
#[test]
fn a_library_constructor_keeps_its_guarantees_in_its_defining_forest() {
    let core = omega_toolchain::core_interface();
    let selected = core.id("access\\new_plan");
    let selecting = check_files(&[("policy.omg", ACCESS_PLAN)]).unwrap();
    let (prefix, _) = ACCESS_PLAN.split_once(" access Rule;").unwrap();
    let unused = check_files(&[("policy.omg", &format!("{prefix};"))]).unwrap();
    let library = selecting.library.expect("admitted library");

    let rows = guarantee_rows(library, selected);
    assert_eq!(
        rows.iter()
            .map(|o| (o.site.package.as_str(), o.site.file.as_str(), spelled(CORE_ACCESS, &o.site)))
            .collect::<Vec<_>>(),
        [
            (omega_toolchain::TOOLCHAIN_PACKAGE, "language/core/access/access.omg", "result.field_count == 0"),
            (
                omega_toolchain::TOOLCHAIN_PACKAGE,
                "language/core/access/access.omg",
                "result.reader_count == 0"
            ),
            (
                omega_toolchain::TOOLCHAIN_PACKAGE,
                "language/core/access/access.omg",
                "result.writer_count == 0"
            ),
            (
                omega_toolchain::TOOLCHAIN_PACKAGE,
                "language/core/access/access.omg",
                "result.reach.service_count == 0"
            ),
        ]
    );
    let tree = library.trees().nth(rows[0].node.file).unwrap();
    for row in &rows {
        assert!(
            tree.semantics_at(row.node.node)
                .any(|fact| matches!(fact, omega_typer::NodeSemantics::Obligation(found) if found == *row))
        );
    }
    assert!(
        !library
            .obligations
            .iter()
            .any(|o| matches!(&o.kind, omega_typer::ObligationKind::VerifyLibraryImplementation(owner) if owner == selected)),
        "a checked body is not a bodyless library implementation"
    );
    assert!(!unused.calls.iter().any(|c| &c.machine == selected));
    spec_corpus::observe_handoff!(
        "library-definition-guarantees/producer",
        assert_eq!(
            (
                calls_from(&selecting, "Rule::plan"),
                rows.len(),
                library_rows(&selecting.obligations).len(),
                library_rows(&unused.obligations).len()
            ),
            (vec![selected], 4, 0, 0)
        )
    );
    // Corrupt control: the baseline's import put the library row first in every importer.
    let mut copied = unused.obligations.clone();
    copied.insert(0, rows[0].clone());
    assert_eq!(library_rows(&copied), [rows[0]]);
}

/// The link dependency that must survive. `need_empty(made.field_count)` owes its own
/// precondition at the caller; its only evidence is `new_plan`'s `ensures result.field_count
/// == 0`, which the library body owes in its own forest. The caller owes no copy of that
/// guarantee; its call names the library declaration whose checked definition it relies on.
#[test]
fn a_caller_relying_on_a_library_guarantee_owes_only_its_own_precondition() {
    use omega_typer::ObligationKind::{CallPrecondition, ProveGuarantee};
    let core = omega_toolchain::core_interface();
    let selected = core.id("access\\new_plan");
    let stage = check_files(&[("policy.omg", RELIES)]).unwrap();
    let need = &stage
        .signatures
        .iter()
        .find(|s| s.machine.path == "need_empty")
        .unwrap()
        .machine;
    let local: Vec<_> = stage
        .obligations
        .iter()
        .filter(|o| matches!(&o.kind, CallPrecondition(id) if id == need))
        .map(|o| (o.site.package.as_str(), o.site.file.as_str(), spelled(RELIES, &o.site)))
        .collect();
    assert_eq!(local, [("app", "policy.omg", "need_empty(made.field_count)")]);
    assert!(
        !stage
            .obligations
            .iter()
            .any(|o| matches!(&o.kind, ProveGuarantee { machine, .. } if machine == selected))
    );
    assert!(library_rows(&stage.obligations).is_empty());
    assert_eq!(calls_from(&stage, "Rule::plan"), [selected, need]);
    let library = stage.library.unwrap();
    assert!(
        guarantee_rows(library, selected)
            .iter()
            .any(|o| spelled(CORE_ACCESS, &o.site) == "result.field_count == 0")
    );
}

/// The same declaration bytes at the toolchain's logical path, but in the `app` package,
/// differ from the library machine only by package. Without its body (the signature/bodyless
/// twin) L3 refuses it at its declaration. With the toolchain's own body it is an ordinary
/// `app` machine that owes its own guarantees; no library row enters the package.
#[test]
fn a_same_path_copy_in_another_package_cannot_acquire_library_supply() {
    let core = omega_toolchain::core_interface();
    let selected = core.id("access\\new_plan");
    let path = "language/core/access/access.omg";
    let header = "use omega\\language\\core;\nuse omega\\language\\core\\reflect;\n\n";
    let declaration = format!("{};", core_signature());
    let copy = format!("{header}{declaration}\n");
    let error = check_files(&[(path, &copy)]).expect_err("an app declaration cannot use the toolchain's supply");
    let at = copy.find(&declaration).unwrap();
    spec_corpus::observe_handoff!(
        "library-definition-guarantees/refusal",
        assert_eq!(
            (
                error.package.as_str(),
                error.file.as_str(),
                error.span.get_start_offset()..error.span.get_end_offset(),
                error.message.as_str()
            ),
            ("app", path, at..at + declaration.len(), "bodyless machine has no authorized supply")
        ),
    );
    #[cfg(feature = "diagnostic-origins")]
    diagnostic_origin::report_return(
        &error,
        "",
        Some(("app", path, at, at + declaration.len(), "bodyless machine has no authorized supply")),
    );

    let with_body = format!("{header}{}\n", core_declaration());
    let checked = check_files(&[(path, &with_body)]).unwrap();
    let local = &checked
        .signatures
        .iter()
        .find(|s| s.machine.package == "app")
        .unwrap()
        .machine;
    assert_eq!((local.package.as_str(), local.path.as_str()), ("app", selected.path.as_str()));
    assert_ne!(local, selected);
    assert!(!core.supplies(local));
    let own: Vec<_> = guarantee_rows(&checked, local)
        .iter()
        .map(|o| (o.site.package.as_str(), spelled(&with_body, &o.site)))
        .collect();
    assert_eq!(
        own,
        [
            ("app", "result.field_count == 0"),
            ("app", "result.reader_count == 0"),
            ("app", "result.writer_count == 0"),
            ("app", "result.reach.service_count == 0")
        ]
    );
    assert!(library_rows(&checked.obligations).is_empty());

    let other = check_files(&[(path, "public machine new_plan(x: u8) -> u8 { x }")]).unwrap();
    assert!(other.obligations.is_empty(), "{:?}", other.obligations);
}

/// Each forest indexes its own files from zero, in path order. A three-file importer whose last file holds
/// the toolchain's `new_plan` declaration uses the same file index (2) as the library's
/// `access.omg`, so `TreeNodeId`s of the two forests coincide. A reader must keep rows with
/// their forest: the importer's list holds exactly its own copy's rows, the library's list
/// holds the library's, and no record appears in both.
#[test]
fn identical_node_indices_in_two_forests_are_counted_once_each() {
    use omega_typer::NodeSemantics;
    let core = omega_toolchain::core_interface();
    let selected = core.id("access\\new_plan");
    let header = "use omega\\language\\core;\nuse omega\\language\\core\\reflect;\n\n";
    let copy = format!("{header}{}\n", core_declaration());
    let files = [
        ("a.omg", "machine a(){}"),
        ("b.omg", "machine b(){}"),
        ("language/core/access/access.omg", copy.as_str()),
    ];
    with_typed(&files, |stage| {
        let library = stage.library.unwrap();
        let local = &stage
            .signatures
            .iter()
            .find(|s| s.machine.package == "app" && s.machine.path.ends_with("new_plan"))
            .unwrap()
            .machine;
        let mine = guarantee_rows(stage, local);
        let theirs = guarantee_rows(library, selected);
        assert_eq!((mine.len(), theirs.len()), (4, 4));
        assert_eq!(
            mine[0].node.file, theirs[0].node.file,
            "both forests index access.omg at the same file position"
        );
        for row in &stage.obligations {
            assert_eq!(row.site.package, "app");
            assert!(!library.obligations.contains(row), "{row:?}");
        }
        // Per-node lookup in each forest returns only that forest's rows.
        for (forest, row, package) in [
            (stage, mine[0], "app"),
            (library, theirs[0], omega_toolchain::TOOLCHAIN_PACKAGE),
        ] {
            let tree = forest.trees().nth(row.node.file).unwrap();
            let at_node: Vec<_> = tree
                .semantics_at(row.node.node)
                .filter_map(|fact| match fact {
                    NodeSemantics::Obligation(found) => Some(found),
                    _ => None,
                })
                .collect();
            assert!(
                at_node.contains(&row) && at_node.iter().all(|found| found.site.package == package),
                "{at_node:?}"
            );
        }
    });
}

/// Type one `app` package and hand its typed forest to `inspect`.
fn with_typed<R>(
    files: &[(&str, &str)],
    inspect: impl FnOnce(&omega_typer::TypedTrees<'_>) -> R,
) -> R {
    let lexed: Vec<_> = files
        .iter()
        .map(|(path, text)| {
            let tokens = omega_lexer::lex(text.as_bytes()).unwrap();
            let tree = omega_parser::parse_file(text.as_bytes(), &tokens).unwrap();
            (*path, *text, tokens, tree)
        })
        .collect();
    let inputs: Vec<_> = lexed
        .iter()
        .map(|(path, text, tokens, tree)| StageSource {
            package: "app",
            source: ParsedSource {
                logical_path: path,
                text,
                tokens,
                tree,
            },
        })
        .collect();
    let resolved = resolve_source_package(&inputs).unwrap();
    inspect(&type_source_package(&resolved).unwrap())
}

/// Whether the zero of `ty` holds a borrowed reference. A record zeroes every
/// field; a sum zeroes only its first declared case's payload
/// (language/data_and_literals.md). The walk reads the declarations L3 published.
fn zero_holds_reference(
    library: &omega_typer::TypedTrees<'_>,
    ty: &omega_typer::ValueType,
) -> bool {
    use omega_typer::ValueType::*;
    match ty {
        Reference(..) => true,
        Array(element, count) => *count > 0 && zero_holds_reference(library, element),
        Tuple(items) => items.iter().any(|item| zero_holds_reference(library, item)),
        AtLifetime(inner, _) | Qualified(inner, _) => zero_holds_reference(library, inner),
        Nominal(owner) | Applied(owner, _) => {
            let first_case = library
                .trees()
                .flat_map(|tree| {
                    tree.resolved()
                        .declaration_nodes()
                        .map(|(_, id)| id.clone())
                        .collect::<Vec<_>>()
                })
                .find(|id| library.case_owners.get(id) == Some(owner.as_ref()));
            let shape = first_case.as_ref().unwrap_or(owner.as_ref());
            library.record_fields.get(shape).is_some_and(|fields| {
                fields
                    .iter()
                    .any(|field| zero_holds_reference(library, &field.ty))
            })
        }
        _ => false,
    }
}

/// The omitted fields of each `..` construction whose owner is `owner`, in field order.
fn zeroed_fields<'a>(
    obligations: &'a [omega_typer::TypingObligation],
    owner: &str,
) -> Vec<(&'a str, &'a omega_typer::ValueType)> {
    obligations
        .iter()
        .filter_map(|o| match &o.kind {
            omega_typer::ObligationKind::ZeroField { field, ty } => field
                .path
                .strip_prefix(owner)
                .and_then(|name| name.strip_prefix("::"))
                .map(|name| (name, ty)),
            _ => None,
        })
        .collect()
}

/// `access\new_plan` is an ordinary core machine (resources/access.md#plans-and-admitted-supply),
/// so its supply is a checked body, not the closed catalog. The body omits the
/// plan's capacity with `..`, and every zero it asks for is reference-free: the
/// slot sums zero to their payload-free first case. The same body over the old
/// slot storage (`[AccessFieldEntry<'t>; 32]`) would zero a byte view, which `..`
/// never supplies. Its guarantees become ordinary `ensures` of that body, and
/// linking still needs the library implementation verified.
#[test]
fn the_plan_constructor_is_a_checked_core_body_that_zeroes_no_reference() {
    use omega_typer::{NodeSemantics, ObligationKind};
    let core = omega_toolchain::core_interface();
    let constructor = core.id("access\\new_plan");
    assert!(!core.supplies(constructor), "an ordinary core machine is not a catalog entry");

    let stage = check("machine idle(){}").unwrap();
    let library = stage.library.expect("admitted library");
    let (tree, declaration) = library
        .trees()
        .find_map(|tree| {
            tree.resolved()
                .declaration_nodes()
                .find(|(_, id)| *id == constructor)
                .map(|(node, _)| (tree, node))
        })
        .unwrap();
    let file = tree.resolved().file;
    let body = file
        .child(declaration, omega_parser::NodeKind::Body)
        .expect("checked body");
    let inside = body..file.nodes[body].end;
    let zero_rows: Vec<_> = library
        .obligations
        .iter()
        .filter(|o| matches!(o.kind, ObligationKind::ZeroField { .. }) && o.node.file == file_index(library, constructor))
        .filter(|o| inside.contains(&o.node.node))
        .cloned()
        .collect();
    let plan = &core.id("AccessPlan").path;
    let zeroed = zeroed_fields(&zero_rows, plan);
    assert_eq!(
        zeroed.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
        [
            "fields",
            "field_count",
            "readers",
            "reader_count",
            "writers",
            "writer_count",
            "reach"
        ]
    );
    let construction = zero_rows[0].node.node;
    assert_eq!(
        &tree
            .construction(construction)
            .expect("plan construction")
            .owner,
        core.id("AccessPlan")
    );
    assert!(zero_rows.iter().all(|o| o.node.node == construction));
    spec_corpus::observe_handoff!(
        "access-plan-constructor-supply/producer",
        assert!(zeroed.iter().all(|(_, ty)| !zero_holds_reference(library, ty)), "{zeroed:?}"),
    );
    for slot in ["AccessFieldSlot", "AccessRuleSlot"] {
        let empty = core.id(&format!("{slot}::Empty"));
        assert_eq!(library.case_owners.get(empty), Some(core.id(slot)));
        assert!(
            library
                .record_fields
                .get(empty)
                .is_none_or(|fields| fields.is_empty()),
            "{slot}::Empty has no payload"
        );
    }
    assert!(
        tree.semantics_at(construction)
            .any(|s| matches!(s, NodeSemantics::Obligation(o) if o.kind == ObligationKind::EstablishData(core.id("AccessPlan").clone())))
    );
    // These rows, and the body's four guarantees, stay in the defining forest.
    assert_eq!(guarantee_rows(library, constructor).len(), 4);
    assert!(stage.obligations.is_empty(), "{:?}", stage.obligations);

    // Contrast: the same body over a local copy of the old storage zeroes `FieldInfo` byte views.
    let old = "use omega\\language\\core; use omega\\language\\core\\reflect;
        data OldPlan<'t> { schema: TypeSchema<'t>; issuance: Issuance<'t>; fields: [AccessFieldEntry<'t>; 32]; field_count: u64; }
        machine make<'t>(schema: @TypeSchema<'t>, issuance: Issuance<'t>) -> OldPlan<'t> reaches none crashes none terminates {
            OldPlan::<'t> { schema: $schema, issuance: issuance, .. }
        }";
    let old_stage = check(old).unwrap();
    let old_zeroed = zeroed_fields(&old_stage.obligations, "OldPlan");
    assert_eq!(old_zeroed.iter().map(|(name, _)| *name).collect::<Vec<_>>(), ["fields", "field_count"]);
    assert!(zero_holds_reference(old_stage.library.unwrap(), old_zeroed[0].1));
    assert!(!zero_holds_reference(old_stage.library.unwrap(), old_zeroed[1].1));
}

fn file_index(
    library: &omega_typer::TypedTrees<'_>,
    machine: &omega_resolver::DeclarationId,
) -> usize {
    library
        .trees()
        .position(|tree| tree.resolved().declaration_nodes().any(|(_, id)| id == machine))
        .unwrap()
}

/// `target/l5owner/policy/empty/access_plan.omg`, byte for byte.
const EMPTY_ROUTES: &str = r"use omega\language\core;
                  use omega\language\core\reflect;
                  use omega\language\core\access;
                  domain i32::Locked access Rule;
                  Rule satisfies AccessPolicy {
                      machine plan<'t>(schema:TypeSchema<'t>)->AccessPlan<'t> reaches none crashes none terminates {
                          new_plan::<'t>(@schema, Issuance::Only{routes: []})
                      }
                  }";

/// The type L3 publishes for the expression spelled exactly at `start..end`.
fn expression_at(
    stage: &TypedStage,
    start: usize,
    end: usize,
) -> omega_typer::ValueType {
    let mut found = stage
        .expressions
        .iter()
        .filter(|e| e.site.file == "app.omg" && (e.site.span.get_start_offset(), e.site.span.get_end_offset()) == (start, end));
    let ty = found
        .next()
        .unwrap_or_else(|| panic!("no expression at bytes {start}..{end}"))
        .ty
        .clone();
    assert!(found.next().is_none(), "two expressions at bytes {start}..{end}");
    ty
}

/// resources/access.md spells `Issuance::Only { routes: [] }` for a policy that
/// admits nobody. The `[]` builds the owned `[CallableInfo<'t>]` field itself,
/// with the element identity of the constructed `Issuance<'t>`: the same `'t`
/// that `Rule::plan` passes to `new_plan`, not a fresh inferred lifetime.
#[test]
fn an_explicit_empty_route_list_lands_at_the_declared_routes_field() {
    use omega_typer::ValueType::{Applied, Slice};
    assert_eq!((EMPTY_ROUTES.len(), &EMPTY_ROUTES[414..416]), (462, "[]"));
    let stage = check(EMPTY_ROUTES).unwrap();
    let construction = "Issuance::Only{routes: []}";
    let start = EMPTY_ROUTES.find(construction).unwrap();
    let Applied(issuance, arguments) = expression_at(&stage, start, start + construction.len()) else {
        panic!("Issuance::Only must publish its applied data type")
    };
    assert_eq!(
        (issuance.package.as_str(), issuance.path.as_str()),
        ("@omega/toolchain", r"language\core\Issuance")
    );
    let [lifetime] = arguments.as_slice() else {
        panic!("Issuance takes one lifetime")
    };
    assert!(!matches!(lifetime, omega_typer::ValueType::InferredLifetime { .. }), "{lifetime:?}");
    let Slice(element) = expression_at(&stage, 414, 416) else {
        panic!("the routes literal must publish the owned [CallableInfo<'t>] field type")
    };
    let Applied(info, element_arguments) = *element else {
        panic!("routes holds CallableInfo")
    };
    assert_eq!(
        (info.package.as_str(), info.path.as_str()),
        ("@omega/toolchain", r"language\core\reflect\CallableInfo")
    );
    assert_eq!(element_arguments.as_slice(), std::slice::from_ref(lifetime));
    assert_eq!(stage.access_policies.len(), 1);
}
