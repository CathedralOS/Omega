use omega_lexer::lex;
use omega_parser::parse_file;
use omega_resolver::{Dependency, ParsedSource, StageSource, Symbol, resolve_stage};

fn run(
    files: &[(&str, &str, &str)],
    edges: &[Dependency<'_>],
) -> Result<omega_resolver::ResolvedStage, omega_resolver::NameError> {
    let parses: Vec<_> = files
        .iter()
        .map(|&(_, _, text)| {
            let tokens = lex(text.as_bytes()).unwrap();
            let tree = parse_file(text.as_bytes(), &tokens).unwrap();
            (tokens, tree)
        })
        .collect();
    let mut sources: Vec<_> = files
        .iter()
        .zip(&parses)
        .map(|(&(package, logical_path, text), (tokens, tree))| StageSource {
            package,
            source: ParsedSource {
                logical_path,
                text,
                tokens,
                tree,
            },
        })
        .collect();
    let forward = resolve_stage(&sources, edges);
    if let Ok(stage) = &forward {
        support::check_origins(stage, &sources);
    }
    sources.reverse();
    assert_eq!(forward, resolve_stage(&sources, edges));
    forward
}

#[test]
fn lexical_binding_uses_its_declaration_and_begins_after_its_initializer() {
    let text = "machine f(x: u32) -> u32 { let x = x; x }";
    let stage = run(&[("app", "main.omg", text)], &[]).unwrap();
    let offsets: Vec<_> = stage
        .uses
        .iter()
        .filter_map(|usage| match &usage.target {
            Symbol::Binding { span, .. } => Some(span.get_start_offset()),
            _ => None,
        })
        .collect();
    assert_eq!(offsets, [text.find("x:").unwrap(), text.find("x =").unwrap()]);
    assert!(run(&[("app", "main.omg", "machine f() { let x = x; x }")], &[]).is_err());
}

#[test]
fn forward_nested_helpers_bind_siblings_and_cannot_capture_runtime_parameters() {
    let text = "machine f(x: u32) { first(x) machine first(v: u32) { second(v) } machine second(v: u32) { v } }";
    let stage = run(&[("app", "main.omg", text)], &[]).unwrap();
    assert!(stage.declarations.iter().any(|d| d.path == "f::first"));
    for text in [
        "machine f(x: u32) { machine h() { x } }",
        "machine f() { machine h() {} } machine g() { f::h() }",
    ] {
        assert!(run(&[("app", "main.omg", text)], &[]).is_err());
    }
}

#[test]
fn imports_need_public_declarations_and_requester_local_edges() {
    let edge = Dependency {
        requester: "app",
        alias: "lib",
        package: "geometry",
    };
    let caller = ("app", "main.omg", "use lib::math::score; machine f() { score() }");
    let public = ("geometry", "math/s.omg", "public machine score() -> u32 { 7 }");
    let stage = run(&[caller, public], &[edge]).unwrap();
    assert!(
        stage
            .uses
            .iter()
            .any(|u| matches!(&u.target, Symbol::Declaration(id) if id.package == "geometry" && id.path == "math::score"))
    );
    assert!(run(&[caller, public], &[]).is_err());
    assert!(run(&[caller, ("geometry", "math/s.omg", "machine score() { 7 }")], &[edge]).is_err());
    assert!(
        run(
            &[
                caller,
                public,
                ("app", "other.omg", "machine other() { score() }")
            ],
            &[edge]
        )
        .is_err()
    );
}

#[test]
fn builtin_type_and_value_roles_stay_separate_and_cases_need_their_carrier() {
    let source = "data Choice { case One; } machine f(u32: u32) -> Choice { let v = u32; Choice::One }";
    run(&[("app", "main.omg", source)], &[]).unwrap();
    for text in [
        "data u0 {}",
        "trait bool {}",
        "machine f<T, u32>() {}",
        "data Choice { case One; } machine f() { One }",
        "machine f(x: u32) { u32 }",
        "machine f(x: Missing) {}",
    ] {
        assert!(run(&[("app", "main.omg", text)], &[]).is_err(), "{text}");
    }
}

#[test]
fn arm_bindings_have_arm_scope_and_an_identity_independent_of_file_order() {
    run(&[("app", "main.omg", "machine f(x: u32) -> u32 { match x { v -> v, _ -> x } }")], &[]).unwrap();
    assert!(run(&[("app", "main.omg", "machine f(x: u32) -> u32 { match x { v -> v, _ -> v } }")], &[]).is_err());
}

#[test]
fn a_call_keeps_both_operand_overloads_and_duplicate_shapes_reject_without_a_call() {
    let code = "machine draw(x: u32) {} machine draw(x: bool) {} machine main(x: bool) { draw(x) }";
    let stage = run(&[("game", "a.omg", code)], &[]).unwrap();
    assert!(
        stage
            .uses
            .iter()
            .any(|u| matches!(&u.target,Symbol::Candidates(ids) if ids.len()==2))
    );
    assert!(run(&[("game", "a.omg", "machine draw<T>(x: T) {} machine draw<U>(y: U) {}")], &[]).is_err());
}

#[test]
fn generic_arguments_take_the_selected_binders_role() {
    let text = "machine pick<T, const N: u32>() {} machine main(u32: u32) { pick::<u32, u32>() }";
    let stage = run(&[("game", "main.omg", text)], &[]).unwrap();
    let args = text.find("::<").unwrap();
    let uses: Vec<_> = stage
        .uses
        .iter()
        .filter(|u| u.span.get_start_offset() > args)
        .collect();
    assert!(matches!(&uses[0].target,Symbol::Builtin(n) if n=="u32"));
    assert!(matches!(
        uses[1].target,
        Symbol::Binding {
            kind: omega_parser::NodeKind::Param,
            ..
        }
    ));
    assert!(run(&[("game", "main.omg", "machine pick<const N: u32>() {} machine main() { pick::<u32>() }")], &[]).is_err());
}

#[test]
fn public_signature_checks_private_types_but_private_body_helpers_are_allowed() {
    run(&[("app", "main.omg", "machine hidden() {} public machine entry() { hidden() }")], &[]).unwrap();
    assert!(run(&[("app", "main.omg", "data Private {} public machine entry(x: Private) {}")], &[]).is_err());
}

#[test]
fn domain_exposure_requires_both_the_exact_carrier_and_an_import_of_the_domain() {
    let lib = ("geometry", "types.omg", "public data Point {}");
    let dom = ("policy", "screen/types.omg", "use geom::Point; public domain Point::OnScreen;");
    let edges = [
        Dependency {
            requester: "policy",
            alias: "geom",
            package: "geometry",
        },
        Dependency {
            requester: "app",
            alias: "g",
            package: "geometry",
        },
        Dependency {
            requester: "app",
            alias: "ui",
            package: "policy",
        },
    ];
    for import in ["use ui::screen;", "use ui::screen::Point::OnScreen;"] {
        let text = format!("use g::Point; {import} machine draw(p: Point in Point::OnScreen) {{}}");
        run(&[lib, dom, ("app", "main.omg", &text)], &edges).unwrap();
    }
    for text in [
        "use g::Point; machine draw(p: Point in Point::OnScreen) {}",
        "data Point {} use ui::screen; machine draw(p: Point in Point::OnScreen) {}",
    ] {
        assert!(run(&[lib, dom, ("app", "main.omg", text)], &edges).is_err());
    }
}

#[test]
fn inline_nested_data_uses_its_data_owner_and_boolean_literals_bind() {
    run(
        &[(
            "game",
            "a.omg",
            "data Box { item: data Item { case Found; }; } machine f(x: Box::Item) { match x { Box::Item::Found -> true, _ -> false } }",
        )],
        &[],
    )
    .unwrap();
}

#[test]
fn type_equality_rejects_a_value_operand_and_static_callback_contracts_bind_to_the_parameter() {
    assert!(run(&[("app", "a.omg", "machine f<T>(i32: u32) requires T == i32 {}")], &[]).is_err());
    run(&[("app", "a.omg", "machine f<T>() requires T == i32 {}")], &[]).unwrap();
    run(
        &[(
            "app",
            "a.omg",
            "machine f<const F: machine(x: u32) -> u32>(x: u32) -> u32 ensures F::ensures(x, result) { F(x) }",
        )],
        &[],
    )
    .unwrap();
}

#[test]
fn result_domains_distinguish_named_overloads_and_return_carriers_alone_do_not() {
    let source = "domain i32::Km; domain i32::Metres; machine distance() -> i32 in Km; machine distance() -> i32 in Metres; machine use_it() { distance() }";
    let stage = run(&[("app", "a.omg", source)], &[]).unwrap();
    assert!(
        stage
            .uses
            .iter()
            .any(|u| matches!(&u.target,Symbol::Candidates(ids) if ids.len()==2))
    );
    assert!(run(&[("app", "a.omg", "machine f() -> u32; machine f() -> bool;")], &[]).is_err());
}

#[test]
fn applications_bind_the_same_base_and_keep_both_argument_expressions_for_later_evaluation() {
    let text = "data Buffer<const N: u32> {} machine f(x: Buffer<8>, y: Buffer<4 + 4>) {}";
    let stage = run(&[("game", "main.omg", text)], &[]).unwrap();
    assert_eq!(stage.applications.len(), 2);
    assert_eq!(stage.applications[0].site.target, stage.applications[1].site.target);
    let arguments: Vec<_> = stage
        .applications
        .iter()
        .map(|a| a.arguments[0].slice(text.as_bytes()).unwrap())
        .collect();
    assert_eq!(arguments, [&b"8"[..], &b"4 + 4"[..]]);
}

#[test]
fn assembly_vocabulary_is_explicit_and_block_transfers_bind_to_the_local_declaration() {
    use omega_resolver::{AssemblyVocabulary, resolve_stage_for_target};
    let text = "asm machine start(x: u32) registers ecx = x { mov ecx, 1; transition -> done; asm machine done { ret; } }";
    let tokens = lex(text.as_bytes()).unwrap();
    let tree = parse_file(text.as_bytes(), &tokens).unwrap();
    let files = [StageSource {
        package: "game",
        source: ParsedSource {
            logical_path: "x.omg",
            text,
            tokens: &tokens,
            tree: &tree,
        },
    }];
    let target = AssemblyVocabulary {
        instructions: &["mov", "ret"],
        registers: &["ecx"],
    };
    let stage = resolve_stage_for_target(&files, &[], &target).unwrap();
    support::check_origins(&stage, &files);
    assert!(
        stage
            .uses
            .iter()
            .any(|u| matches!(&u.target,Symbol::Declaration(id) if id.path=="start::done"))
    );
    assert!(resolve_stage(&files, &[]).is_err());
    let missing_register = AssemblyVocabulary {
        instructions: target.instructions,
        registers: &[],
    };
    assert!(
        resolve_stage_for_target(&files, &[], &missing_register)
            .unwrap_err()
            .message
            .contains("register ecx")
    );
}

#[test]
fn subjectless_dispatch_tests_existing_values_and_shorthand_fields_read_a_value() {
    run(&[("app", "a.omg", "data Pair { v: u32; } machine f(v: u32) -> Pair { Pair { v } }")], &[]).unwrap();
    for source in [
        "machine f() { transition { absent -> 1, _ -> 0 } }",
        "data Pair { v: u32; } machine f() -> Pair { Pair { v } }",
        "data Counter {} machine Counter::increment(x: Counter) {}",
    ] {
        assert!(run(&[("app", "a.omg", source)], &[]).is_err(), "{source}");
    }
}

#[test]
fn selection_rows_bind_an_existing_machine_to_the_declared_traits_requirement() {
    let declarations = "trait Logger { machine write(); } trait Other { machine write(); } machine write_log() {} const counter = 1;";
    let source = format!("{declarations} Sink satisfies Logger {{ Logger::write = write_log; }}");
    let result = run(&[("app", "a.omg", &source)], &[]).unwrap();
    assert!(
        result
            .uses
            .iter()
            .any(|u| matches!(&u.target,Symbol::Declaration(id) if id.path=="Logger::write"))
    );
    let overloads = "trait Logger { machine write(x: u32); machine write(x: bool); } machine log_number(x: u32) {} machine log_flag(x: bool) {} Sink satisfies Logger { Logger::write = log_number; Logger::write = log_flag; }";
    let stage = run(&[("app", "a.omg", overloads)], &[]).unwrap();
    assert_eq!(
        stage
            .uses
            .iter()
            .filter(|u| matches!(&u.target, Symbol::Candidates(ids) if ids.len() == 2 && ids[0].path == "Logger::write"))
            .count(),
        2
    );
    for row in [
        "Logger::write = absent;",
        "Other::write = write_log;",
        "Logger::write = counter;",
        "Logger::write = write_log; Logger::write = write_log;",
    ] {
        let source = format!("{declarations} Sink satisfies Logger {{ {row} }}");
        assert!(run(&[("app", "a.omg", &source)], &[]).is_err(), "{row}");
    }
}

#[test]
fn static_evidence_selects_a_member_of_its_declared_trait() {
    let head = "trait Order { machine compare(x: u32); }";
    for (member, valid) in [("compare", true), ("absent", false)] {
        let text = format!("{head} machine f<const O: Order>(x: u32) {{ O::{member}(x) }}");
        let stage = run(&[("app", "main.omg", &text)], &[]);
        assert_eq!(stage.is_ok(), valid, "{stage:?}");
    }
}

#[test]
fn deep_left_associated_names_and_repeated_permutations_keep_every_origin() {
    let text = format!("machine f(x: u32) -> u32 {{ {} }}", vec!["x"; 10_000].join(" + "));
    let result = run(&[("app", "main.omg", &text)], &[]).unwrap();
    let bound: Vec<_> = result
        .uses
        .iter()
        .filter(|u| matches!(u.target, Symbol::Binding { .. }))
        .collect();
    let expected: Vec<_> = text
        .match_indices("x")
        .skip(1)
        .map(|(offset, _)| offset)
        .collect();
    assert_eq!(
        bound
            .iter()
            .map(|u| u.span.get_start_offset())
            .collect::<Vec<_>>(),
        expected
    );
}

#[test]
fn operator_declarations_and_uses_retain_the_token_and_declaring_home() {
    let unit = "public domain i32::Km;";
    let operator = "use units; public machine + Km::add(x: i32 in Km, y: i32 in Km) -> i32 in Km { x }";
    let caller = "use units; machine combine(x: i32 in Km,y: i32 in Km) { x + y }";
    let stage = run(
        &[
            ("app", "units/u.omg", unit),
            ("app", "operations.omg", operator),
            ("app", "main.omg", caller),
        ],
        &[],
    )
    .unwrap();
    assert_eq!(stage.operator_declarations[0].token, "+");
    assert_eq!(stage.operator_declarations[0].home.as_ref().unwrap().path, "units::i32::Km");
    assert!(
        stage
            .operators
            .iter()
            .any(|op| op.file == "main.omg" && op.token == "+")
    );
    let foreign = operator.replace("use units;", "use lib::units;");
    let duplicate = operator.replace("Km::add", "Km::sum");
    assert!(
        run(
            &[
                ("app", "units/u.omg", unit),
                ("app", "a.omg", operator),
                ("app", "b.omg", &duplicate)
            ],
            &[]
        )
        .unwrap_err()
        .message
        .contains("duplicate operator binding")
    );
    assert!(
        run(
            &[
                ("lib", "units/u.omg", unit),
                ("app", "operations.omg", &foreign)
            ],
            &[Dependency {
                requester: "app",
                alias: "lib",
                package: "lib"
            }]
        )
        .is_err()
    );
}

#[test]
fn same_spelled_operand_types_imported_from_distinct_packages_distinguish_overloads() {
    let files = [
        ("app", "one.omg", "use left::Point; machine draw(p: Point) {}"),
        ("app", "two.omg", "use right::Point; machine draw(p: Point) {}"),
        ("app", "caller.omg", "use left::Point; machine call(p: Point) { draw(p) }"),
        ("a", "p.omg", "public data Point {}"),
        ("b", "p.omg", "public data Point {}"),
    ];
    let edges = [
        Dependency {
            requester: "app",
            alias: "left",
            package: "a",
        },
        Dependency {
            requester: "app",
            alias: "right",
            package: "b",
        },
    ];
    let stage = run(&files, &edges).unwrap();
    assert!(
        stage
            .uses
            .iter()
            .any(|u| matches!(&u.target,Symbol::Candidates(ids) if ids.len()==2 && ids[0]!=ids[1]))
    );
}

#[test]
fn admitted_dependencies_are_requester_local_and_never_transitive() {
    let files = [
        ("app", "main.omg", "machine run() { dep::work() }"),
        ("lib", "l.omg", "public machine work() { dep::leaf() }"),
        ("core", "c.omg", "public machine leaf() {}"),
    ];
    let edges = [
        Dependency {
            requester: "app",
            alias: "dep",
            package: "lib",
        },
        Dependency {
            requester: "lib",
            alias: "dep",
            package: "core",
        },
    ];
    let stage = run(&files, &edges).unwrap();
    assert!(
        stage
            .uses
            .iter()
            .any(|u| u.package == "lib" && matches!(&u.target,Symbol::Declaration(id) if id.package=="core"))
    );
    let bad = [
        ("app", "main.omg", "machine run() { core::leaf() }"),
        files[1],
        files[2],
    ];
    assert!(run(&bad, &edges).is_err());
    let conflicting_alias = Dependency {
        requester: "app",
        alias: "dep",
        package: "core",
    };
    assert!(
        run(&files, &[edges[0], conflicting_alias])
            .unwrap_err()
            .message
            .contains("conflicting packages")
    );
    assert!(
        run(&[files[0], files[0]], &[])
            .unwrap_err()
            .message
            .contains("duplicate logical file")
    );
}

#[test]
fn namespace_and_import_order_never_break_a_tie() {
    let files = [
        ("app", "main.omg", "use a::f; use b::f; machine g() { f() }"),
        ("app", "a/f.omg", "machine f() {}"),
        ("app", "b/f.omg", "machine f() {}"),
    ];
    let error = run(&files, &[]).unwrap_err();
    assert!(error.message.contains("a::f") && error.message.contains("b::f"));
    let repeat = [
        ("app", "main.omg", "use a::f; use a::f; machine g() { f() }"),
        files[1],
        files[2],
    ];
    run(&repeat, &[]).unwrap();
    for path in ["i32/x.omg", "not-a-module/x.omg", "../main.omg", "/main.omg"] {
        assert!(run(&[("app", path, "machine f() {}")], &[]).is_err());
    }
}

#[test]
fn package_local_declaration_identity_survives_a_filename_change() {
    let text = "machine action(x: u32) -> u32 { x }";
    let before = run(&[("app", "jobs/first.omg", text)], &[]).unwrap();
    let after = run(&[("app", "jobs/renamed.omg", text)], &[]).unwrap();
    assert_eq!(before.declarations, after.declarations);
    assert_ne!(before.definition_sites, after.definition_sites);
}

#[test]
fn qualified_members_keep_the_application_on_the_owner_segment() {
    let text = "data Packet<const N: u32> { data Header {} } const Size: u32 = 8; machine parse(h: Packet<Size>::Header) {}";
    let stage = run(&[("app", "packet.omg", text)], &[]).unwrap();
    let application = &stage.applications[0];
    assert!(matches!(&application.site.target,Symbol::Declaration(id) if id.path=="Packet"));
    assert_eq!(application.site.span.slice(text.as_bytes()).unwrap(), b"Packet");
    assert!(
        stage
            .uses
            .iter()
            .any(|u| matches!(&u.target,Symbol::Declaration(id) if id.path=="Packet::Header"))
    );
}

#[test]
fn inherited_trait_rows_keep_the_declaring_owner() {
    let source = "trait Parent { machine read(); } trait Child: Parent {} machine read_log() {} Log satisfies Child { Parent::read = read_log; } machine f<const C: Child>() { C::read() }";
    let stage = run(&[("app", "a.omg", source)], &[]).unwrap();
    assert!(
        stage
            .uses
            .iter()
            .any(|u| matches!(&u.target,Symbol::EvidenceMember{requirements,..} if requirements[0].path=="Parent::read"))
    );
    assert!(run(&[("app", "a.omg", "trait A: B {} trait B: A {}")], &[]).is_err());
    assert!(run(&[("app", "a.omg", "boundary trait B {} trait A: B {}")], &[]).is_err());
    run(
        &[
            ("lib", "p.omg", "public trait Base { machine read(); } public trait Derived: Base {}"),
            ("app", "main.omg", "use dep::Derived::read; machine f() { read() }"),
        ],
        &[Dependency {
            requester: "app",
            alias: "dep",
            package: "lib",
        }],
    )
    .unwrap();
}

#[test]
fn a_type_directed_overload_can_defer_the_role_of_a_generic_name() {
    let source = "machine route<T>(x: u32) {} machine route<const N: bool>(x: bool) {} machine f(bool: bool) { route::<bool>(bool) }";
    let stage = run(&[("app", "a.omg", source)], &[]).unwrap();
    assert!(
        stage
            .uses
            .iter()
            .any(|u| matches!(&u.target,Symbol::ByOverload(options) if options.len()==2
        && options.iter().any(|o|matches!(o.target.as_deref(),Some(Symbol::Builtin(n)) if n=="bool"))
        && options.iter().any(|o|matches!(o.target.as_deref(),Some(Symbol::Binding{..})))))
    );
}
#[test]
fn a_public_conformance_exposes_its_interface_and_keeps_its_realizations_private() {
    let provider = (
        "lib",
        "lib.omg",
        "public trait Logger { machine write(); } public Log satisfies Logger { machine write() {} }",
    );
    let edge = Dependency {
        requester: "app",
        alias: "provider",
        package: "lib",
    };
    let caller = ("app", "main.omg", "machine f() { provider::Log.write() }");
    run(&[caller, provider], &[edge]).unwrap();
    let direct = ("app", "main.omg", "machine f() { provider::Log::write() }");
    assert!(run(&[direct, provider], &[edge]).is_err());
    let reused = (
        "lib",
        "lib.omg",
        "public trait Logger { machine write(); } machine private_log() {} public Log satisfies Logger { Logger::write = private_log; }",
    );
    run(&[caller, reused], &[edge]).unwrap();
}

#[test]
fn ambiguous_domains_report_each_owner_carrier_and_exposing_import() {
    let caller = "use geom::Point; use left; use right; machine f(p: Point in Point::Ready) {}";
    let domain = "use geom::Point; domain Point::Ready;";
    let files = [
        ("app", "main.omg", caller),
        ("app", "geom/p.omg", "data Point {}"),
        ("app", "left/d.omg", domain),
        ("app", "right/d.omg", domain),
    ];
    let error = run(&files, &[]).unwrap_err();
    for label in [
        "app::left::Point::Ready",
        "app::right::Point::Ready",
        "app::geom::Point",
    ] {
        assert!(error.message.contains(label), "{}", error.message);
    }
    for import in ["left", "right"] {
        let start = caller.find(import).unwrap();
        assert!(
            error
                .message
                .contains(&format!("main.omg@{start}..{}", start + import.len()))
        );
    }
}
mod support;

#[test]
fn universe_and_value_binders_keep_their_name_roles() {
    use omega_parser::NodeKind::KindedParam;
    let redeclared = "machine f<u8:core::Type<0>>(){}";
    assert_eq!(
        run(&[("app", "probe.omg", redeclared)], &[])
            .unwrap_err()
            .message,
        "builtin type u8 cannot be redeclared"
    );
    let mixed = "machine f<A:core::Type<0>,N:u8>() requires A == N {}";
    assert_eq!(
        run(&[("app", "probe.omg", mixed)], &[]).unwrap_err().message,
        "type equality mixes a type and a value"
    );
    let source = "data X{} machine pick<A:core::Type<0>>(x:u8){} machine pick<N:u8>(x:bool){} machine f<X:u8>(){pick::<X>(true)}";
    let stage = run(&[("app", "probe.omg", source)], &[]).unwrap();
    let alternatives = stage
        .uses
        .iter()
        .find_map(|usage| match &usage.target {
            Symbol::ByOverload(alternatives) if usage.span.slice(source.as_bytes()).unwrap() == b"X" => Some(alternatives),
            _ => None,
        })
        .unwrap();
    assert_eq!(alternatives.len(), 2);
    for alt in alternatives {
        assert_eq!(alt.binder, Some(KindedParam));
        match alt.candidate.operands[0].as_str() {
            "u8" => assert!(matches!(alt.target.as_deref(),Some(Symbol::Declaration(id)) if id.path=="X")),
            "bool" => assert!(matches!(alt.target.as_deref(), Some(Symbol::Binding { kind: KindedParam, .. }))),
            _ => panic!("unexpected overload"),
        }
    }
    let source = "data X{} machine pick<A:core::Type<0>>(){} machine f<X:u8>(){pick::<X>()}";
    let stage = run(&[("app", "probe.omg", source)], &[]).unwrap();
    let start = source.rfind("X").unwrap();
    assert!(
        stage
            .uses
            .iter()
            .any(|u| u.span.get_start_offset() == start && matches!(&u.target,Symbol::Declaration(id) if id.path=="X"))
    );
}

#[test]
fn parameters_must_have_distinct_names_within_each_parameter_list() {
    for (source, expected) in [
        ("data View<'value,value>{body:@'value i32;}", "duplicate generic parameter value"),
        ("machine f<T,T>(){}", "duplicate generic parameter T"),
        ("machine f(x:u8,x:bool){}", "duplicate parameter x"),
    ] {
        let error = run(&[("app", "probe.omg", source)], &[]).unwrap_err();
        assert_eq!(error.message, expected);
        let name = expected.split_whitespace().last().unwrap();
        let first = source.find(name).unwrap();
        let second = first + name.len() + source[first + name.len()..].find(name).unwrap();
        assert_eq!(error.span.get_start_offset(), second);
    }
    run(&[("app", "probe.omg", "machine f<T>(x:T)->T{g(x) machine g<T>(x:T)->T{x}}")], &[]).unwrap();
}

#[test]
fn a_shadowing_initializer_reads_the_previous_binding_and_the_tail_reads_the_new_one() {
    for name in ["index", "value", "item"] {
        let source = format!("machine f({name}:u8)->u8{{let {name}={name}; {name}}}");
        let sites: Vec<_> = source.match_indices(name).map(|(offset, _)| offset).collect();
        let stage = run(&[("app", "scope.omg", &source)], &[]).unwrap();
        for (use_at, declared_at) in [(sites[2], sites[0]), (sites[3], sites[1])] {
            let target = &stage
                .uses
                .iter()
                .find(|u| u.span.get_start_offset() == use_at)
                .unwrap()
                .target;
            assert!(
                matches!(target, Symbol::Binding { span, .. } if span.get_start_offset() == declared_at),
                "{source}: {target:?}"
            );
        }
        let outside = format!("machine f(x:u8)->u8{{match x {{{name} -> {name}, _ -> {name}}}}}");
        let error = run(&[("app", "scope.omg", &outside)], &[]).unwrap_err();
        assert_eq!(error.message, format!("unresolved name {name}"));
        assert_eq!(error.span.get_start_offset(), outside.rfind(name).unwrap());
    }
}

#[test]
fn alias_binder_roles_follow_the_imports_of_each_overload() {
    let caller = "data X{} machine f<X:u8>(){pick::<X>(true)}";
    let files = [
        ("app", "type/kind.omg", "public data Sort=core::Type<0>;"),
        ("app", "value/kind.omg", "public data Sort=u8;"),
        ("app", "a.omg", "use type::Sort; machine pick<A:Sort>(x:u8){}"),
        ("app", "b.omg", "use value::Sort; machine pick<N:Sort>(x:bool){}"),
        ("app", "caller.omg", caller),
    ];
    let stage = run(&files, &[]).unwrap();
    let site = caller.rfind("X").unwrap();
    let target = &stage
        .uses
        .iter()
        .find(|u| u.file == "caller.omg" && u.span.get_start_offset() == site)
        .unwrap()
        .target;
    let Symbol::ByOverload(options) = target else { panic!("{target:?}") };
    assert_eq!(options.len(), 2);
    for option in options {
        match option.candidate.operands[0].as_str() {
            "u8" => assert!(matches!(option.target.as_deref(),Some(Symbol::Declaration(decl)) if decl.path=="X")),
            "bool" => assert!(matches!(
                option.target.as_deref(),
                Some(Symbol::Binding {
                    kind: omega_parser::NodeKind::KindedParam,
                    ..
                })
            )),
            operand => panic!("unexpected overload {operand}"),
        }
    }
    for (src, message) in [
        ("data Sort=core::Type<0>; machine f<u8:Sort>(){}", "builtin type u8 cannot be redeclared"),
        ("data Byte=u8; machine f<N:Byte>(){} machine g(){f::<u8>()}", "unresolved name u8"),
    ] {
        assert_eq!(run(&[("app", "main.omg", src)], &[]).unwrap_err().message, message);
    }
}

#[test]
fn imported_overloads_come_only_from_the_requesters_selected_package() {
    let imported = "use dep::math::score; machine caller() { score(1) }";
    let bare = "machine caller() { score(1) }";
    let overloads = "public machine score(n:u8) {} public machine score(n:u32) {}";
    let other = "public machine score(n:bool) {}";
    let call_start = imported.rfind("score").unwrap();
    for (selected, inaccessible) in [("alpha", "beta"), ("beta", "alpha")] {
        let edge = Dependency {
            requester: "app",
            alias: "dep",
            package: selected,
        };
        let mut files = vec![
            ("app", "main.omg", imported),
            (selected, "math/api.omg", overloads),
        ];
        for extra in [None, Some((inaccessible, "math/api.omg", other))] {
            files.extend(extra);
            let stage = run(&files, &[edge]).unwrap();
            let call = stage
                .uses
                .iter()
                .find(|usage| usage.package == "app" && usage.file == "main.omg" && usage.span.get_start_offset() == call_start)
                .unwrap();
            assert_eq!(call.span.slice(imported.as_bytes()), Some("score".as_bytes()));
            let Symbol::Candidates(candidates) = &call.target else {
                panic!("expected both imported overloads: {call:?}");
            };
            assert_eq!(candidates.len(), 2);
            assert!(
                candidates
                    .iter()
                    .all(|id| id.package == selected && id.path == "math::score")
            );
            for operand in ["u8", "u32"] {
                assert!(candidates.iter().any(|id| id.operands == [operand]), "{candidates:?}");
            }
        }

        // Without a direct dependency the import fails, even if a different
        // package has the same alias. Without the import the call itself fails.
        let other_edge = Dependency {
            requester: inaccessible,
            ..edge
        };
        let error = run(&files, &[other_edge]).unwrap_err();
        assert_eq!(
            (error.package.as_str(), error.file.as_str(), error.message.as_str()),
            ("app", "main.omg", "unavailable import dep::math::score")
        );
        assert_eq!(error.span.slice(imported.as_bytes()), Some("dep::math::score".as_bytes()));

        files[0].2 = bare;
        let error = run(&files, &[edge]).unwrap_err();
        assert_eq!(
            (error.package.as_str(), error.file.as_str(), error.message.as_str()),
            ("app", "main.omg", "unresolved name score")
        );
        assert_eq!(error.span.slice(bare.as_bytes()), Some("score".as_bytes()));
    }
}

#[test]
fn child_domains_follow_the_visible_parent_on_the_annotated_carrier() {
    let declarations = "domain i32::Positive; domain i32::Positive::Small; domain i32::Positive::Small::Tiny; domain u8::Positive; domain u8::Positive::Small; domain<T> T::Small;";
    let caller = "machine inspect(x:i32 in Positive::Small::Tiny,y:u8 in Positive::Small) {}";
    let stage = run(
        &[
            ("pkg", "domains.omg", declarations),
            ("pkg", "caller.omg", caller),
        ],
        &[],
    )
    .unwrap();
    for (spelling, want) in [
        ("Positive::Small::Tiny", "i32::Positive::Small::Tiny"),
        ("Positive::Small", "u8::Positive::Small"),
    ] {
        let uses: Vec<_> = stage
            .uses
            .iter()
            .filter(|u| u.file == "caller.omg" && u.span.slice(caller.as_bytes()) == Some(spelling.as_bytes()))
            .collect();
        assert!(matches!(uses.as_slice(), [u] if matches!(&u.target,Symbol::Declaration(id) if id.path==want)));
    }
    for (decls, path) in [
        (declarations, "Bogus::i32::Positive"),
        ("domain u8::Positive; domain u8::Positive::Small;", "Positive::Small"),
        ("domain i32::Other; domain i32::Other::Small;", "Positive::Small"),
    ] {
        let source = format!("machine inspect(x:i32 in {path}) {{}}");
        let err = run(&[("pkg", "domains.omg", decls), ("pkg", "caller.omg", &source)], &[]).unwrap_err();
        assert_eq!(err.message, format!("unresolved name {path}"));
        assert_eq!(err.span.slice(source.as_bytes()), Some(path.as_bytes()));
    }

    let domains = ("lib", "units/limits.omg", "public domain i32::Positive; public domain i32::Positive::Small;");
    let edge = Dependency {
        requester: "app",
        alias: "limits",
        package: "lib",
    };
    let body = "machine inspect(x:i32 in Positive::Small) {}";
    let imported = format!("use limits::units; {body}");
    run(&[domains, ("app", "caller.omg", &imported)], &[edge]).unwrap();
    let err = run(&[domains, ("app", "caller.omg", body)], &[edge]).unwrap_err();
    assert_eq!(err.message, "unresolved name Positive::Small");
}

fn chapter_block(
    chapter: &str,
    line: usize,
) -> String {
    let mut blocks = Vec::new();
    spec_corpus::extract_examples_from_markdown("chapter", chapter, &mut blocks);
    blocks
        .into_iter()
        .find(|b| b.fence_line_number == line)
        .unwrap()
        .source_text
}

#[test]
fn published_import_exposes_the_exact_remote_domain() {
    let text = chapter_block(include_str!("../../../wiki/spec/language/modules.md"), 102);
    let requester = format!("{text}\nmachine check(p:ui_policy::screen::Point in OnScreen) {{}}");
    let library = ("policy", "screen/point.omg", "public data Point{} public domain Point::OnScreen;");
    let edge = Dependency {
        requester: "app",
        alias: "ui_policy",
        package: "policy",
    };
    let files = [library, ("app", "main.omg", requester.as_str())];
    let resolved = run(&files, &[edge]).unwrap();
    let usage = resolved
        .uses
        .iter()
        .find(|u| u.file == "main.omg" && u.span.slice(requester.as_bytes()) == Some(b"OnScreen"))
        .unwrap();
    assert!(matches!(&usage.target,Symbol::Declaration(d) if d.package=="policy" && d.path=="screen::Point::OnScreen"));
    let error = run(&files, &[]).unwrap_err();
    assert_eq!(error.file, "main.omg");
    assert!(error.message.contains("ui_policy"), "{error:?}");
}

#[test]
fn published_type_equality_rejects_a_shadowing_value() {
    let clause = chapter_block(include_str!("../../../wiki/spec/language/generics.md"), 306);
    let ok = format!("machine restrict<T>() {clause} {{}}");
    run(&[("app", "restrict.omg", &ok)], &[]).unwrap();
    let shadow = ok.replace("restrict<T>()", "restrict<T>(u32:u16)");
    let error = run(&[("app", "restrict.omg", &shadow)], &[]).unwrap_err();
    assert_eq!(error.message, "type equality mixes a type and a value");
    assert_eq!(error.span.slice(shadow.as_bytes()), Some(b"T == u32".as_slice()));
}
