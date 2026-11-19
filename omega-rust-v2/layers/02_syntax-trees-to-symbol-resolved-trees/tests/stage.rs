use omega_lexer::lex;
use omega_parser::parse_file;
use omega_resolver::{Dependency, ParsedSource, StageSource, Symbol, resolve_stage};

fn run<'text>(
    files: &[(&'text str, &'text str, &'text str)],
    edges: &[Dependency<'_>],
) -> Result<omega_resolver::SymbolResolvedTrees<'text>, omega_resolver::NameError> {
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
fn ranking_order_resolves_to_its_declaration_and_bound_to_its_parameter() {
    for order in ["Down", "Down(limit)"] {
        let text = format!("data Down {{}} machine f(x:u8, limit:u8) terminates by x -> {order} in limit {{}}");
        let stage = run(&[("app", "rank.omg", &text)], &[]).unwrap();
        let order_offset = text.rfind("Down").unwrap();
        let bound_offset = text.rfind("limit").unwrap();
        let target_at = |offset| {
            &stage
                .uses()
                .find(|u| u.span.get_start_offset() == offset)
                .unwrap()
                .target
        };
        assert!(matches!(target_at(order_offset), Symbol::Declaration(id) if id.path == "Down"));
        assert!(matches!(target_at(bound_offset), Symbol::Binding { span, .. } if span.get_start_offset() == text.find("limit:u8").unwrap()));
    }
}

#[test]
fn lexical_binding_uses_its_declaration_and_begins_after_its_initializer() {
    let text = "machine f(x: u32) -> u32 { let x = x; x }";
    let stage = run(&[("app", "main.omg", text)], &[]).unwrap();
    let offsets: Vec<_> = stage
        .uses()
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
            .uses()
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
            .uses()
            .any(|u| matches!(&u.target,Symbol::Candidates(ids) if ids.len()==2))
    );
    assert!(run(&[("game", "a.omg", "machine draw<T>(x: T) {} machine draw<U>(y: U) {}")], &[]).is_err());
}

#[test]
fn both_generic_argument_positions_keep_the_same_visible_names() {
    let text = "machine pick<T, const N: u32>() {} machine main(u32: u32) { pick::<u32, u32>() }";
    let stage = run(&[("game", "main.omg", text)], &[]).unwrap();
    let args = text.find("::<").unwrap();
    let uses: Vec<_> = stage
        .uses()
        .filter(|u| u.span.get_start_offset() > args)
        .collect();
    assert_eq!(uses.len(), 2);
    for argument in uses {
        assert!(
            matches!(&argument.target, Symbol::ScopedNames { context: omega_resolver::NameContext::GenericArgument, names } if matches!(names.as_slice(),
            [Symbol::Binding { kind: omega_parser::NodeKind::Param, .. }, Symbol::Builtin(name)] if name == "u32"))
        );
    }
    assert!(
        run(
            &[("game", "main.omg", "machine pick<const N: u32>() {} machine main() { pick::<missing>() }")],
            &[]
        )
        .is_err()
    );
}

#[test]
fn forwarded_callback_and_data_name_share_the_resolved_application() {
    use omega_parser::NodeKind::{ConstParam, Data};
    let source =
        "data G {} trait Step<T>{machine apply(v:T);} machine via<T,const F:Step<T>::apply>(v:T){F(v)} machine wrap<const G:machine>(v:G){via::<G,G>(v)}";
    let stage = run(&[("game", "main.omg", source)], &[]).unwrap();
    let tree = stage.trees().next().unwrap();
    let (list, _) = tree
        .applications()
        .find(|(_, name)| matches!(&name.target, Symbol::Declaration(id) if id.path == "via"))
        .unwrap();
    let arguments = &tree.file.nodes[list].children;
    assert_eq!(arguments.len(), 2);
    for &arg in arguments {
        let Some(Symbol::ScopedNames { names, .. }) = tree.symbol(arg) else {
            panic!("expected both visible names")
        };
        let [
            Symbol::Binding {
                kind: ConstParam, binder_node, ..
            },
            Symbol::Declaration(id),
        ] = names.as_slice()
        else {
            panic!("expected callback and data")
        };
        assert_eq!(id.kind, Data);
        assert_eq!(id.path, "G");
        let head = tree.file.nodes[*binder_node].children[0];
        assert!(matches!(tree.symbol(head), Some(Symbol::Builtin(name)) if name == "machine"));
    }
}

#[test]
fn public_signature_checks_private_types_but_private_body_helpers_are_allowed() {
    let private = "public data Buffer<N:u8>{} const size:u8=3; public machine f(x:Buffer<size>){}";
    assert_eq!(
        run(&[("app", "private.omg", private)], &[])
            .unwrap_err()
            .message,
        "public signature exposes private declaration size"
    );
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
fn compiled_domains_keep_the_carrier_from_the_declaring_package() {
    let geometry = ("geometry", "types.omg", "public data Point {}");
    let policy = (
        "policy",
        "screen/d.omg",
        "use geom::Point; public domain Point::Ready; public domain Point::Ready::Stable;",
    );
    let policy_edge = Dependency {
        requester: "policy",
        alias: "geom",
        package: "geometry",
    };
    let library = run(&[geometry, policy], &[policy_edge]).unwrap();
    let exports = library.compiled_interface();
    let parent = exports
        .declarations
        .iter()
        .find(|e| e.id.path == "screen::Point::Ready")
        .unwrap();
    assert!(matches!(&parent.domain_carrier, Some(omega_resolver::DomainCarrier::Named(id)) if id.package == "geometry" && id.path == "Point"));
    let consumer = |text: &str, deps: &[Dependency<'_>]| {
        let tokens = lex(text.as_bytes()).unwrap();
        let tree = parse_file(text.as_bytes(), &tokens).unwrap();
        let source = StageSource {
            package: "app",
            source: ParsedSource {
                logical_path: "main.omg",
                text,
                tokens: &tokens,
                tree: &tree,
            },
        };
        omega_resolver::resolve_stage_with_exports(&[source], deps, &Default::default(), &exports).map(|stage| {
            stage
                .uses()
                .filter_map(|u| match &u.target {
                    Symbol::Declaration(id) => Some(id.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        })
    };
    let edges = [
        Dependency {
            requester: "app",
            alias: "ui",
            package: "policy",
        },
        Dependency {
            requester: "app",
            alias: "geom",
            package: "geometry",
        },
    ];
    let uses = consumer("use geom::Point; use ui::screen; machine f(p:Point in Point::Ready::Stable){}", &edges).unwrap();
    assert!(
        uses.iter()
            .any(|id| id.package == "policy" && id.path == "screen::Point::Ready::Stable")
    );
    assert!(consumer("data Point{} use ui::screen; machine f(p:Point in Point::Ready){}", &edges).is_err());
    assert_eq!(
        consumer("use ui::screen::Point::Ready::Stable;", &edges[..1])
            .unwrap_err()
            .message,
        "no authority to name carrier geometry::Point"
    );
}

#[test]
fn selecting_a_child_domain_requires_visibility_of_its_carrier() {
    let edges = [
        Dependency {
            requester: "lib",
            alias: "geom",
            package: "geometry",
        },
        Dependency {
            requester: "app",
            alias: "lib",
            package: "lib",
        },
        Dependency {
            requester: "app",
            alias: "geom",
            package: "geometry",
        },
    ];
    for name in ["Point::Ready", "Point::Ready::Stable"] {
        let source = format!("use lib::{name};");
        let files = [
            ("app", "main.omg", source.as_str()),
            ("geometry", "types.omg", "public data Point {}"),
            (
                "lib",
                "domains.omg",
                "use geom::Point; public domain Point::Ready; public domain Point::Ready::Stable;",
            ),
        ];
        run(&files, &edges).unwrap();
        let error = run(&files, &edges[..2]).unwrap_err();
        assert_eq!(error.message, "no authority to name carrier geometry::Point");
        assert_eq!(error.span.slice(source.as_bytes()), Some(&source.as_bytes()[4..source.len() - 1]));
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
fn static_callback_contracts_bind_to_the_parameter() {
    for bound in ["machine(x:u32)->u32", "Step::run", "u32"] {
        let code = format!("trait Step{{machine run(x:u32)->u32;}} machine f<const F:{bound}>(x:u32)->u32 ensures F::ensures(x,result) {{x}}");
        let names = run(&[("app", "a.omg", &code)], &[]).unwrap();
        let projection = names
            .uses()
            .find(|u| matches!(u.target, Symbol::StaticMember { .. }))
            .unwrap();
        let Symbol::StaticMember { binder, member } = &projection.target else {
            unreachable!()
        };
        assert_eq!(member, "ensures");
        assert_eq!(binder.get_start_offset(), code.find("F:").unwrap());
        assert_eq!(projection.span.slice(code.as_bytes()), Some(b"F::ensures".as_slice()));
    }
}

#[test]
fn result_domains_distinguish_named_overloads_and_return_carriers_alone_do_not() {
    let source = "domain i32::Km; domain i32::Metres; machine distance() -> i32 in Km; machine distance() -> i32 in Metres; machine use_it() { distance() }";
    let stage = run(&[("app", "a.omg", source)], &[]).unwrap();
    assert!(
        stage
            .uses()
            .any(|u| matches!(&u.target,Symbol::Candidates(ids) if ids.len()==2))
    );
    assert!(run(&[("app", "a.omg", "machine f() -> u32; machine f() -> bool;")], &[]).is_err());
}

#[test]
fn applications_bind_the_same_base_and_keep_both_argument_expressions_for_later_evaluation() {
    let text = "data Buffer<const N: u32> {} machine f(x: Buffer<8>, y: Buffer<4 + 4>) {}";
    let stage = run(&[("game", "main.omg", text)], &[]).unwrap();
    let tree = stage.trees().next().unwrap();
    let applications: Vec<_> = tree.applications().collect();
    let [(first, a), (second, b)] = applications.as_slice() else {
        panic!("expected two applications")
    };
    assert_eq!(a.target, b.target);
    let arguments = [first, second].map(|&section| {
        tree.file
            .span(tree.file.nodes[section].children[0])
            .slice(text.as_bytes())
            .unwrap()
    });
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
            .uses()
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
fn assembly_transfers_require_one_visible_local_continuation() {
    for (text, message) in [
        (
            "asm machine entry { transition -> next; } asm machine next {}",
            "unresolved local assembly block next",
        ),
        (
            "asm machine entry { transition -> next; } asm machine other { asm machine next {} }",
            "unresolved local assembly block next",
        ),
        (
            "asm machine entry { transition -> next; asm machine next(x:u8) {} asm machine next(x:u16) {} }",
            "ambiguous local assembly block next",
        ),
    ] {
        let error = run(&[("app", "asm.omg", text)], &[]).unwrap_err();
        assert_eq!(error.message, message);
        assert_eq!(error.span.get_start_offset(), text.find("-> next").unwrap() + 3);
    }
    let text = "asm machine entry { transition -> first; asm machine first { transition -> next; } asm machine next {} }";
    let stage = run(&[("app", "asm.omg", text)], &[]).unwrap();
    let tree = stage.trees().next().unwrap();
    let labels: Vec<_> = tree
        .file
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.kind == omega_parser::NodeKind::AsmTransition)
        .map(|(n, _)| match tree.symbol(n).unwrap() {
            Symbol::Declaration(id) => id.path.as_str(),
            target => panic!("unexpected transfer {target:?}"),
        })
        .collect();
    assert_eq!(labels, ["entry::first", "entry::next"]);
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
            .uses()
            .any(|u| matches!(&u.target,Symbol::Declaration(id) if id.path=="Logger::write"))
    );
    let overloads = "trait Logger { machine write(x: u32); machine write(x: bool); } machine log_number(x: u32) {} machine log_flag(x: bool) {} Sink satisfies Logger { Logger::write = log_number; Logger::write = log_flag; }";
    let stage = run(&[("app", "a.omg", overloads)], &[]).unwrap();
    assert_eq!(
        stage
            .uses()
            .filter(|u| matches!(&u.target, Symbol::Candidates(ids) if ids.len() == 2 && ids[0].path == "Logger::write"))
            .count(),
        2
    );
    let duplicate = format!("{declarations} Sink satisfies Logger {{ Logger::write = write_log; Logger::write = write_log; }}");
    let bound = run(&[("app", "a.omg", &duplicate)], &[]).unwrap();
    assert_eq!(
        bound
            .uses()
            .filter(|name| matches!(&name.target, Symbol::Declaration(id) if id.path == "Logger::write"))
            .count(),
        2
    );
    for row in [
        "Logger::write = absent;",
        "Other::write = write_log;",
        "Logger::write = counter;",
    ] {
        let source = format!("{declarations} Sink satisfies Logger {{ {row} }}");
        assert!(run(&[("app", "a.omg", &source)], &[]).is_err(), "{row}");
    }
}

#[test]
fn lexical_static_members_bind_before_their_schema_is_known() {
    for member in ["compare", "absent"] {
        let text = format!("trait Order {{ machine compare(x: u32); }} data Alias = Order; machine f<const O: Alias>(x: u32) {{ O::{member}(x) }}");
        let stage = run(&[("app", "main.omg", &text)], &[]).unwrap();
        assert!(
            stage
                .uses()
                .any(|name| matches!(&name.target, Symbol::StaticMember {binder, member: selected}
            if binder.get_start_offset() == text.find("O: Alias").unwrap() && selected == member))
        );
    }
}

#[test]
fn a_static_root_collision_retains_both_visible_names() {
    let applied = "trait R { machine m<T>(); } machine f<const E:R>() { E::<u8>::m(); E::m::<u8>(); }";
    let names = run(&[("app", "apply.omg", applied)], &[]).unwrap();
    let owners: Vec<_> = names
        .trees()
        .next()
        .unwrap()
        .applications()
        .map(|(_, name)| &name.target)
        .collect();
    assert!(matches!(owners.as_slice(), [Symbol::Binding {span,..}, Symbol::StaticMember {binder,member}] if span==binder && member=="m"));
    let text = "machine f<const O: u8>() { O::run() }";
    let stage = run(
        &[
            ("app", "main.omg", text),
            ("app", "O/run.omg", "machine run() {}"),
        ],
        &[],
    )
    .unwrap();
    let prefix = stage
        .uses()
        .find(|name| name.role == omega_resolver::NameRole::CompetingStaticRoot)
        .unwrap();
    assert!(matches!(&prefix.target, Symbol::Binding {span,..} if span.get_start_offset() == text.find("O:").unwrap()));
    assert_eq!(prefix.span.get_start_offset(), text.find("O::run").unwrap());
    assert!(
        stage
            .uses()
            .any(|name| name.node == prefix.node && matches!(&name.target, Symbol::Declaration(id) if id.path == "O::run"))
    );
}

#[test]
fn deep_left_associated_names_and_repeated_permutations_keep_every_origin() {
    let text = format!("machine f(x: u32) -> u32 {{ {} }}", vec!["x"; 10_000].join(" + "));
    let result = run(&[("app", "main.omg", &text)], &[]).unwrap();
    let bound: Vec<_> = result
        .uses()
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
    assert!(stage.operator_declarations[0].public);
    assert!(matches!(&stage.operator_declarations[0].declared_home, Some(Symbol::Declaration(id)) if id.path == "units::i32::Km"));
    assert!(
        stage
            .trees()
            .filter(|t| t.file.path == "main.omg")
            .any(|t| (0..t.file.nodes.len()).any(|n| t.file.operator(n).is_some_and(|op| op.token == "+")))
    );
    let traits = "trait public {machine + add(x:u8,y:u8)->u8;} public trait Visible {machine - sub(x:u8,y:u8)->u8;}";
    let members = run(&[("app", "ops.omg", traits)], &[]).unwrap();
    assert!(!members.operator_declarations[0].public);
    assert!(members.operator_declarations[1].public);
    let foreign = operator.replace("use units;", "use lib::units;");
    let duplicate = operator.replace("Km::add", "Km::sum");
    let stage = run(
        &[
            ("app", "units/u.omg", unit),
            ("app", "a.omg", operator),
            ("app", "b.omg", &duplicate),
        ],
        &[],
    )
    .unwrap();
    assert_eq!(stage.operator_declarations.len(), 2);
    let implicit = operator.replace("Km::add", "add");
    let stage = run(
        &[
            ("app", "units/u.omg", unit),
            ("app", "operations.omg", &implicit),
        ],
        &[],
    )
    .unwrap();
    assert!(stage.operator_declarations[0].declared_home.is_none());
    let unknown = operator.replace("Km::add", "Missing::add");
    assert!(
        run(
            &[
                ("app", "units/u.omg", unit),
                ("app", "operations.omg", &unknown)
            ],
            &[]
        )
        .unwrap_err()
        .message
        .contains("unresolved machine owner Missing")
    );
    let stage = run(
        &[
            ("lib", "units/u.omg", unit),
            ("app", "operations.omg", &foreign),
        ],
        &[Dependency {
            requester: "app",
            alias: "lib",
            package: "lib",
        }],
    )
    .unwrap();
    let binding = &stage.operator_declarations[0];
    assert_eq!(binding.machine.package, "app");
    assert!(matches!(&binding.declared_home, Some(Symbol::Declaration(id)) if id.package == "lib"));
    let tree = stage.trees().nth(binding.node.file).unwrap();
    assert_eq!(tree.file.path, "operations.omg");
    assert_eq!(tree.file.span(binding.node.node).slice(foreign.as_bytes()), Some(&b"+"[..]));
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
            .uses()
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
            .trees()
            .filter(|tree| tree.file.package == "lib")
            .flat_map(|tree| tree.occurrences())
            .any(|u| matches!(&u.target,Symbol::Declaration(id) if id.package=="core"))
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
    let before = before.trees().next().unwrap();
    let after = after.trees().next().unwrap();
    assert_ne!(before.file.path, after.file.path);
    assert!(before.definitions().eq(after.definitions()));
}

#[test]
fn qualified_members_keep_the_application_on_the_owner_segment() {
    let text = "data Packet<const N: u32> { data Header {} } const Size: u32 = 8; machine parse(h: Packet<Size>::Header) {}";
    let stage = run(&[("app", "packet.omg", text)], &[]).unwrap();
    let (_, name) = stage.trees().next().unwrap().applications().next().unwrap();
    assert!(matches!(&name.target, Symbol::Declaration(id) if id.path=="Packet"));
    assert_eq!(name.span.slice(text.as_bytes()), Some(b"Packet".as_slice()));
    assert!(
        stage
            .uses()
            .any(|u| matches!(&u.target,Symbol::Declaration(id) if id.path=="Packet::Header"))
    );
}

#[test]
fn inherited_trait_rows_keep_the_declaring_owner() {
    let source = "trait Parent { machine read(); } trait Middle: Parent {} trait Child: Middle {} machine read_log() {} Log satisfies Child { Parent::read = read_log; } machine f<const C: Child>() { C::read() }";
    let stage = run(&[("app", "a.omg", source)], &[]).unwrap();
    assert!(
        stage
            .uses()
            .any(|u| matches!(&u.target,Symbol::StaticMember{binder,member} if binder.get_start_offset()==source.find("C: Child").unwrap() && member=="read"))
    );
    let id = |path| stage.declarations.iter().find(|id| id.path == path).unwrap();
    assert_eq!(stage.trait_ancestors(id("Child")).collect::<Vec<_>>(), [("app", "Middle"), ("app", "Parent")]);
    assert_eq!(stage.trait_ancestors(id("Parent")).count(), 0);
    assert_eq!(stage.trait_ancestors(id("read_log")).count(), 0);
    assert!(run(&[("app", "a.omg", "trait A: B {} trait B: A {}")], &[]).is_err());
    run(&[("app", "a.omg", "boundary trait B {} trait A: B {}")], &[]).unwrap();
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
    assert!(stage.uses().any(
        |u| matches!(&u.target,Symbol::ScopedNames { names, .. } if matches!(names.as_slice(), [Symbol::Binding { .. }, Symbol::Builtin(n)] if n == "bool"))
    ));
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
fn annotated_binders_leave_their_equality_role_to_typing() {
    let mixed = "machine f<A:core::Type<0>,N:u8>() requires A == N {}";
    let resolved = run(&[("app", "probe.omg", mixed)], &[]).unwrap();
    let tree = resolved.trees().next().unwrap();
    assert!((0..tree.file.nodes.len()).any(|n| tree.file.operator(n).is_some_and(|op| op.token == "==")));
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
                .uses()
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
fn imported_alias_bounds_leave_both_visible_argument_names_at_the_call() {
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
        .trees()
        .filter(|tree| tree.file.path == "caller.omg")
        .flat_map(|tree| tree.occurrences())
        .find(|u| u.span.get_start_offset() == site)
        .unwrap()
        .target;
    assert!(
        matches!(target, Symbol::ScopedNames { context: omega_resolver::NameContext::GenericArgument, names } if matches!(names.as_slice(),
        [Symbol::Binding { kind: omega_parser::NodeKind::KindedParam, .. }, Symbol::Declaration(decl)] if decl.path == "X"))
    );
    let call = stage
        .trees()
        .filter(|tree| tree.file.path == "caller.omg")
        .flat_map(|tree| tree.occurrences())
        .find(|u| matches!(&u.target, Symbol::Candidates(_)))
        .unwrap();
    assert!(matches!(&call.target, Symbol::Candidates(candidates) if candidates.len() == 2));
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
                .trees()
                .find(|tree| tree.file.package == "app" && tree.file.path == "main.omg")
                .unwrap()
                .occurrences()
                .find(|usage| usage.span.get_start_offset() == call_start)
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
            .trees()
            .filter(|tree| tree.file.path == "caller.omg")
            .flat_map(|tree| tree.occurrences())
            .filter(|u| u.span.slice(caller.as_bytes()) == Some(spelling.as_bytes()))
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

#[test]
fn annotated_domain_heads_preserve_the_lexical_type_candidates() {
    for bound in ["Sort", "Byte"] {
        let code = format!("data T {{}} data Sort=core::Type<0>; data Byte=u8; domain<const T:{bound},const N:u8> T::Mark<N>;");
        let stage = run(&[("app", "domain.omg", &code)], &[]).unwrap();
        let tree = stage.trees().next().unwrap();
        let head = tree
            .file
            .nodes
            .iter()
            .position(|n| n.kind == omega_parser::NodeKind::PathType && n.path() == "T::Mark")
            .unwrap();
        let symbol = tree.symbol(head).unwrap();
        assert!(
            matches!(symbol, Symbol::ScopedNames { context: omega_resolver::NameContext::Type, names }
            if matches!(&names[..], [Symbol::Binding { kind: omega_parser::NodeKind::ConstParam, .. }, Symbol::Declaration(id)] if id.path=="T")),
            "{symbol:?}"
        );
    }
}

#[test]
fn competing_attachments_record_the_binder_that_can_make_a_carrier_generic() {
    let code = "data T{} data K=u8; domain<const N:u8,const T:K> T::Tag<N,T>; domain<const A:u8,const B:u8> u16::Tag<A,B>; machine f(x:u16 in Tag<1,2>) {}";
    let stage = run(&[("app", "domains.omg", code)], &[]).unwrap();
    let choices = stage
        .uses()
        .find_map(|usage| match &usage.target {
            Symbol::DomainCandidates { candidates, .. } => Some(candidates),
            _ => None,
        })
        .unwrap();
    assert_eq!(choices.len(), 2);
    for choice in choices {
        match choice.declaration.path.as_str() {
            "u16::Tag" => assert!(choice.type_binder.is_none()),
            "T::Tag" => assert_eq!(choice.type_binder.as_ref(), Some(&(choice.declaration.clone(), 1))),
            _ => panic!("unexpected domain {choice:?}"),
        }
    }
}

#[test]
fn explicit_domain_qualifiers_bind_the_lexical_carrier_and_the_index_owner() {
    let source = "domain<T, const N:u8> T::Level<N>; machine f<U>(x:U in U::Level<2>) {}";
    let stage = run(&[("app", "use.omg", source)], &[]).unwrap();
    let tree = stage.trees().next().unwrap();
    let application = |operand: &[u8]| {
        tree.applications()
            .find_map(|(section, name)| {
                tree.file.nodes[section]
                    .children
                    .iter()
                    .any(|&n| tree.file.span(n).slice(source.as_bytes()) == Some(operand))
                    .then_some(name)
            })
            .unwrap()
    };
    let owner = application(b"N");
    assert!(matches!(&owner.target, Symbol::Declaration(id) if id.path=="T::Level"));
    assert_eq!(owner.span.slice(source.as_bytes()), Some(b"Level".as_slice()));
    let name = application(b"2");
    assert!(matches!(&name.target, Symbol::Declaration(id) if id.path == "T::Level"));
    assert!(matches!(tree.qualifier(name.node.node), Some(Symbol::Binding {span,..}) if span.get_start_offset() == source.find("<U>").unwrap()+1));
}

#[test]
fn a_domain_index_is_bound_in_the_declaring_telescope() {
    for bad in ["Other", "2", "N + 1"] {
        let source = format!("data Other {{}} domain<const N:u8> u8::Limit<{bad}>;");
        let error = run(&[("app", "index.omg", &source)], &[]).unwrap_err();
        assert_eq!(error.span.slice(source.as_bytes()), Some(bad.as_bytes()));
        assert!(error.message.contains("one of its own binders"), "{error:?}");
    }
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
fn reflection_selectors_need_visible_query_names_and_preserve_the_operand_declarations() {
    use omega_parser::NodeKind;
    let selectors = chapter_block(include_str!("../../../wiki/spec/language/reflection.md"), 77);
    let mut source = String::from("use omega::language::core::reflect; machine inspect(){");
    for selector in selectors.lines() {
        source.push_str(&format!("_ = {selector};"));
    }
    source.push('}');
    let mut files = vec![
        (
            "app",
            "types.omg",
            "data Controller{status:u8;} data Buffer<T>{used:T;} trait ControllerWriter{machine write();}",
        ),
        ("app", "Controller/update.omg", "public machine update(){}"),
        ("app", "select.omg", source.as_str()),
        ("core", "language/core/reflect/types.omg", "public data FieldInfo{} public data CallableInfo{}"),
    ];
    let edges = [Dependency {
        requester: "app",
        alias: "omega",
        package: "core",
    }];
    let error = run(&files, &edges).unwrap_err();
    assert_eq!(error.message, "unresolved name reflect::field");
    // Supply the query names separately so this checks ordinary resolution
    // independently of which primitives the toolchain ships.
    files.push((
        "core",
        "language/core/reflect/queries.omg",
        "public machine field(); public machine callable();",
    ));
    let stage = run(&files, &edges).unwrap();
    let client = stage
        .trees()
        .find(|tree| tree.file.path == "select.omg")
        .unwrap();
    let selected: Vec<_> = client
        .occurrences()
        .filter_map(|u| match &u.target {
            Symbol::Declaration(id) if id.package == "app" && u.role == omega_resolver::NameRole::Reference => Some((id.path.as_str(), id.kind)),
            _ => None,
        })
        .collect();
    assert_eq!(
        selected,
        [
            ("Controller::status", NodeKind::Field),
            ("Buffer::used", NodeKind::Field),
            ("Controller::update", NodeKind::Machine),
            ("ControllerWriter::write", NodeKind::Machine)
        ]
    );
    assert!(
        client
            .applications()
            .any(|(section, name)| matches!(&name.target, Symbol::Declaration(id) if id.path == "Buffer")
                && client.file.span(section).slice(source.as_bytes()) == Some(b"::<u32>"))
    );
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
        .trees()
        .filter(|tree| tree.file.path == "main.omg")
        .flat_map(|tree| tree.occurrences())
        .find(|u| u.span.slice(requester.as_bytes()) == Some(b"OnScreen"))
        .unwrap();
    assert!(matches!(&usage.target,Symbol::Declaration(d) if d.package=="policy" && d.path=="screen::Point::OnScreen"));
    let error = run(&files, &[]).unwrap_err();
    assert_eq!(error.file, "main.omg");
    assert!(error.message.contains("ui_policy"), "{error:?}");
}

#[test]
fn published_equality_binds_a_shadowing_parameter() {
    let clause = chapter_block(include_str!("../../../wiki/spec/language/generics.md"), 306);
    let ok = format!("machine restrict<T>() {clause} {{}}");
    run(&[("app", "restrict.omg", &ok)], &[]).unwrap();
    let shadow = ok.replace("restrict<T>()", "restrict<T>(u32:u16)");
    let resolved = run(&[("app", "restrict.omg", &shadow)], &[]).unwrap();
    let at = shadow.find("T == u32").unwrap() + "T == ".len();
    assert!(resolved.uses().any(|name| name.span.get_start_offset() == at
        && matches!(
            name.target,
            Symbol::Binding {
                kind: omega_parser::NodeKind::Param,
                ..
            }
        )));
}

#[test]
fn equality_keeps_its_operand_bindings_through_grouping() {
    for (left, right) in [
        ("T", "u32"),
        ("addr", "addr"),
        ("x", "x"),
        ("T", "x"),
        ("x", "T"),
    ] {
        for (l, r) in [
            (left.to_owned(), right.to_owned()),
            (format!("({left})"), right.to_owned()),
            (left.to_owned(), format!("({right})")),
            (format!("(({left}))"), format!("(({right}))")),
        ] {
            let text = format!("machine f<T>(x:u32) requires {l} == {r} {{}}");
            let resolved = run(&[("app", "f.omg", &text)], &[]).unwrap();
            let tree = resolved.trees().next().unwrap();
            let mut operators = (0..tree.file.nodes.len()).filter_map(|n| tree.file.operator(n).map(|op| (n, op)));
            let (node, operator) = operators.next().unwrap();
            assert!(operators.next().is_none());
            assert_eq!(operator.token, "==");
            let [lhs, rhs] = *tree.file.nodes[node].children.as_slice() else {
                panic!("binary operands")
            };
            for (operand, spelling) in [(lhs, left), (rhs, right)] {
                let n = tree.file.ungroup(operand);
                assert_eq!(tree.file.span(n).slice(text.as_bytes()), Some(spelling.as_bytes()));
                assert!(tree.symbol(n).is_some());
            }
        }
    }
}

/// The spelling and binding of every name use that starts inside `region` of `source`.
fn uses_within(
    stage: &omega_resolver::SymbolResolvedTrees<'_>,
    source: &str,
    region: &str,
) -> Vec<(String, String)> {
    let start = source.find(region).unwrap();
    let end = start + region.len();
    stage
        .uses()
        .filter(|u| (start..end).contains(&u.span.get_start_offset()))
        .map(|u| {
            let [binding] = u.target.visible_names() else {
                panic!("unexpected alternatives at {:?}", u.span)
            };
            let target = match binding {
                Symbol::Builtin(name) => format!("builtin {name}"),
                Symbol::Declaration(id) => format!("{:?} {}", id.kind, id.path),
                Symbol::Binding { kind, .. } => format!("binding {kind:?}"),
                other => format!("{other:?}"),
            };
            (String::from_utf8_lossy(u.span.slice(source.as_bytes()).unwrap()).into_owned(), target)
        })
        .collect()
}

// `u8 in P` written as a generic argument is a qualified type when the binder
// takes a type, in both spellings of generic arguments. Under a value binder the
// same tree is a membership test, and binds exactly as the parenthesized
// membership expression does.
#[test]
fn a_qualified_generic_argument_takes_the_selected_binders_role() {
    let typed = "domain u8::P; data Box<T> { v: T; } machine id<T>(v: T) -> T { v } machine c(x: u8, b: Box<u8 in P>) { _ = id::<u8 in P>(x); }";
    let stage = run(&[("game", "main.omg", typed)], &[]).unwrap();
    let qualified = [
        ("u8".to_owned(), "builtin u8".to_owned()),
        ("P".to_owned(), "Domain u8::P".to_owned()),
    ];
    assert_eq!(uses_within(&stage, typed, "<u8 in P>"), qualified);
    assert_eq!(uses_within(&stage, typed, "::<u8 in P>"), qualified);
    let valued = "domain u8::P; data Check<const B: bool> {} machine c<const N: u8>(a: Check<N in P>, b: Check<(N in P)>) {}";
    let stage = run(&[("game", "main.omg", valued)], &[]).unwrap();
    let membership = [
        ("N".to_owned(), "binding ConstParam".to_owned()),
        ("P".to_owned(), "Domain u8::P".to_owned()),
    ];
    assert_eq!(uses_within(&stage, valued, "<N in P>"), membership);
    assert_eq!(uses_within(&stage, valued, "<(N in P)>"), membership);
    // The binder's normalized bound determines whether N may be used as a type.
    let mismatched = "domain u8::P; data Box<T> {} machine c<const N: u8>(b: Box<N in P>) {}";
    let stage = run(&[("game", "main.omg", mismatched)], &[]).unwrap();
    assert_eq!(uses_within(&stage, mismatched, "<N in P>"), membership);
}

/// The declaration that each use spelled `spelling` in `file` selects.
fn selected(
    stage: &omega_resolver::SymbolResolvedTrees<'_>,
    file: &str,
    text: &str,
    spelling: &str,
) -> Vec<String> {
    stage
        .trees()
        .filter(|tree| tree.file.path == file)
        .flat_map(|tree| tree.occurrences())
        .filter(|u| text.get(u.span.get_start_offset()..u.span.get_end_offset()) == Some(spelling))
        .map(|u| match &u.target {
            Symbol::Declaration(id) => format!("{}::{}", id.package, id.path),
            other => format!("{other:?}"),
        })
        .collect()
}

#[test]
fn a_qualified_path_and_its_import_select_the_same_root_module_declaration() {
    // `combat` names the package's root module wherever the path is written, as it
    // does in `use combat::score`. A same-named child directory of the caller is not
    // reached by a qualified path or by an import.
    let root = ("app", "combat/s.omg", "machine score() -> u32 { 1 } data Point { x: u32; }");
    let child = ("app", "game/combat/s.omg", "machine score() -> u32 { 2 } data Point { y: u32; }");
    let qualified = "machine f(p: combat::Point) -> u32 { combat::score() }";
    let imported = "use combat::score; use combat::Point; machine f(p: Point) -> u32 { score() }";
    for caller in ["main.omg", "game/main.omg"] {
        for text in [qualified, imported] {
            let stage = run(&[root, child, ("app", caller, text)], &[]).unwrap();
            for (spelling, expected) in [
                ("combat::score", "app::combat::score"),
                ("score", "app::combat::score"),
                ("combat::Point", "app::combat::Point"),
                ("Point", "app::combat::Point"),
            ] {
                for target in selected(&stage, caller, text, spelling) {
                    assert_eq!(target, expected, "{caller}: `{text}` selects {spelling}");
                }
            }
            assert!(!selected(&stage, caller, text, "combat::score").is_empty());
        }
    }
    for (text, message) in [
        ("machine f() -> u32 { combat::score() }", "unresolved name combat::score"),
        ("use combat::score; machine f() -> u32 { score() }", "unavailable import combat::score"),
        ("machine f(p: combat::Point) {}", "unresolved name combat::Point"),
    ] {
        let error = run(&[child, ("app", "game/main.omg", text)], &[])
            .err()
            .unwrap_or_else(|| panic!("`{text}` in game/ reached game::combat"));
        assert_eq!((error.file.as_str(), error.message.as_str()), ("game/main.omg", message));
    }
}

#[test]
fn a_dependency_alias_that_also_names_a_root_directory_or_declaration_is_refused_in_every_directory() {
    let edge = Dependency {
        requester: "app",
        alias: "dep",
        package: "lib",
    };
    let library = ("lib", "l.omg", "public machine item() -> u32 { 1 }");
    let directory = ("app", "dep/x.omg", "machine item() -> u32 { 2 }");
    let qualified = "machine f() -> u32 { dep::item() }";
    let imported = "use dep::item; machine f() -> u32 { item() }";
    for caller in ["main.omg", "sub/main.omg"] {
        for text in [qualified, imported] {
            // A root declaration named like the alias competes the same way.
            for local in [directory, ("app", "d.omg", "data dep { case item; }")] {
                let error = run(&[library, local, ("app", caller, text)], &[edge])
                    .err()
                    .unwrap_or_else(|| {
                        let stage = run(&[library, local, ("app", caller, text)], &[edge]).unwrap();
                        panic!("{caller}: `{text}` was accepted and selects {:?}", selected(&stage, caller, text, "dep::item"))
                    });
                assert_eq!(error.file, caller);
                assert_eq!(error.message, "competing module paths dep: package lib and app::dep");
                assert_eq!(text.get(error.span.get_start_offset()..error.span.get_end_offset()), Some("dep::item"));
            }
            for (files, edges, expected) in [
                (vec![library, ("app", caller, text)], vec![edge], "lib::item"),
                (vec![directory, ("app", caller, text)], vec![], "app::dep::item"),
            ] {
                let stage = run(&files, &edges).unwrap();
                let mut targets = selected(&stage, caller, text, "dep::item");
                targets.extend(selected(&stage, caller, text, "item"));
                assert!(!targets.is_empty());
                assert!(targets.iter().all(|t| t == expected), "{caller}: `{text}` selects {targets:?}");
            }
        }
    }
}

#[test]
fn compiled_exports_cannot_replace_source_identity_or_duplicate_an_admitted_declaration() {
    use omega_parser::NodeKind;
    use omega_resolver::{CompiledInterface, InterfaceExport, resolve_stage_with_exports};
    let text = "public data Packet {}";
    let tokens = lex(text.as_bytes()).unwrap();
    let parsed = parse_file(text.as_bytes(), &tokens).unwrap();
    let source = StageSource {
        package: "wire",
        source: ParsedSource {
            logical_path: "packet.omg",
            text,
            tokens: &tokens,
            tree: &parsed,
        },
    };
    let files = [source];
    let local = resolve_stage(&files, &[]).unwrap();
    let id = &local.declarations[0];
    let export = |kind| InterfaceExport {
        id: omega_resolver::DeclarationId { kind, ..id.clone() },
        module: "".into(),
        trait_parents: vec![],
        domain_carrier: None,
    };
    let check = |sources: &[_], declarations| {
        let interface = CompiledInterface {
            modules: local.compiled_interface().modules,
            declarations,
        };
        resolve_stage_with_exports(sources, &[], &Default::default(), &interface).map(|stage| stage.declarations.clone())
    };
    assert_eq!(check(&[], vec![export(NodeKind::Data)]).unwrap(), std::slice::from_ref(id));
    for (sources, exports, reason) in [
        (&files[..], vec![export(NodeKind::Data)], "source package collides with a compiled interface"),
        (
            &[][..],
            vec![export(NodeKind::Data), export(NodeKind::Data)],
            "duplicate declaration in compiled interface",
        ),
        (&[][..], vec![export(NodeKind::Body)], "unsupported declaration in compiled interface"),
    ] {
        let error = check(sources, exports).unwrap_err();
        assert_eq!(
            (error.package.as_str(), error.file.as_str(), error.message.as_str()),
            ("wire", "Packet", reason)
        );
    }
}

/// What each use spelled `word` in `file` refers to: a constant's path, or the
/// kind and offset of the binding.
fn word_targets(
    stage: &omega_resolver::SymbolResolvedTrees<'_>,
    file: &str,
    text: &str,
    word: &str,
) -> Vec<String> {
    stage
        .trees()
        .filter(|tree| tree.file.path == file)
        .flat_map(|tree| tree.occurrences())
        .filter(|u| text.get(u.span.get_start_offset()..u.span.get_end_offset()) == Some(word))
        .map(|u| match &u.target {
            Symbol::Declaration(id) => format!("{:?} {}::{}", id.kind, id.package, id.path),
            Symbol::Binding { span, kind, .. } => format!("{kind:?}@{}", span.get_start_offset()),
            other => format!("{other:?}"),
        })
        .collect()
}

#[test]
fn a_bare_subject_arm_binds_even_when_a_constant_has_the_same_name() {
    let consts = ("app", "consts.omg", "const x: u32 = 7;");
    for text in [
        "machine f(v: u32) -> u32 { match v { x -> x } }",
        "machine f(v: u32) -> u32 { transition v { x -> x } }",
        "machine f(x: u32, v: u32) -> u32 { match v { x -> x, _ -> 0 } }",
        "machine f(v: u32) -> u32 { let x = 3; match v { x -> x, _ -> 0 } }",
        "machine f<const x: u32>(v: u32) -> u32 { match v { x -> x, _ -> 0 } }",
    ] {
        let pattern = text.find("{ x ->").unwrap() + 2;
        let binder = format!("BindingPattern@{pattern}");
        for files in [
            vec![("app", "main.omg", text)],
            vec![consts, ("app", "main.omg", text)],
        ] {
            let stage = run(&files, &[]).unwrap();
            assert_eq!(
                word_targets(&stage, "main.omg", text, "x"),
                [binder.as_str()],
                "{text} with {} files",
                files.len()
            );
        }
    }
    let text = "use limits::x; machine f(v:u32)->u32 { match v { limits::x -> 1, x -> x } }";
    let stage = run(
        &[
            ("app", "limits/values.omg", consts.2),
            ("app", "main.omg", text),
        ],
        &[],
    )
    .unwrap();
    assert_eq!(
        word_targets(&stage, "main.omg", text, "limits::x"),
        ["Const app::limits::x", "Const app::limits::x"]
    );
    let binder = text.find(", x ->").unwrap() + 2;
    assert_eq!(word_targets(&stage, "main.omg", text, "x"), [format!("BindingPattern@{binder}")]);

    let text = "use left::x; use right::x; machine f(v:u32)->u32 { match v { x -> x } }";
    let left = ("app", "left/values.omg", "const x:u32=1;");
    let right = ("app", "right/values.omg", "const x:u32=2;");
    let stage = run(&[left, right, ("app", "main.omg", text)], &[]).unwrap();
    let binder = text.find("{ x ->").unwrap() + 2;
    assert_eq!(word_targets(&stage, "main.omg", text, "x"), [format!("BindingPattern@{binder}")]);
    let ordinary = text.replace("match v { x -> x }", "x");
    assert_eq!(
        run(&[left, right, ("app", "main.omg", &ordinary)], &[])
            .unwrap_err()
            .message,
        "competing declarations: app::left::x, app::right::x"
    );
}

#[test]
fn a_top_level_qualified_machine_head_needs_a_module_trait_or_implementation_owner() {
    // `machine f::helper` outside `f` would add a helper to f's nested scope from
    // another file, and an owner that names nothing qualifies nothing.
    let caller = ("app", "main.omg", "machine f() { helper(1u32) }");
    for (files, head, message) in [
        (
            vec![("app", "side.omg", "machine f::helper(v: u32) {}"), caller],
            "f::helper",
            "machine f cannot qualify a declaration outside its body",
        ),
        (
            vec![
                ("app", "side.omg", "machine f::helper() {}"),
                ("app", "main.omg", "machine f() { helper() machine helper() {} }"),
            ],
            "f::helper",
            "machine f cannot qualify a declaration outside its body",
        ),
        (
            vec![("app", "main.omg", "machine f::helper() {} machine g() { f::helper() }")],
            "f::helper",
            "unresolved machine owner f",
        ),
        (
            vec![("app", "main.omg", "data Counter {} machine Counter::increment(x: Counter) {}")],
            "Counter::increment",
            "a data carrier cannot own a machine",
        ),
    ] {
        let error = run(&files, &[])
            .err()
            .unwrap_or_else(|| panic!("{files:?} was accepted"));
        let text = files.iter().find(|f| f.1 == error.file).unwrap().2;
        assert_eq!(
            (text.get(error.span.get_start_offset()..error.span.get_end_offset()), error.message.as_str()),
            (Some(head), message),
            "{files:?}"
        );
    }
    // A named implementation, a domain and a module can each own a top-level machine.
    let module_source = "machine + combat::sum(a:u32,b:u32)->u32 {a}";
    let module = run(
        &[
            ("app", "combat/helper.omg", "machine helper(){}"),
            ("app", "main.omg", module_source),
        ],
        &[],
    )
    .unwrap();
    let [binding] = module.operator_declarations.as_slice() else {
        panic!("expected one binding")
    };
    assert_eq!(binding.machine.path, "combat::sum");
    assert!(binding.declared_home.is_none());
    for files in [
        vec![(
            "app",
            "main.omg",
            "domain i32::Km; machine + Km::add(x: i32 in Km, y: i32 in Km) -> i32 in Km { x }",
        )],
        vec![(
            "app",
            "main.omg",
            "trait Logger { machine write(); } Sink satisfies Logger { Logger::write = Sink::write; } machine Sink::write() {}",
        )],
        vec![
            ("app", "combat/x.omg", "machine other() {}"),
            ("app", "main.omg", "machine combat::score() {} machine g() { combat::score() }"),
        ],
    ] {
        run(&files, &[]).unwrap_or_else(|e| panic!("{files:?}: {e:?}"));
    }
}

#[test]
fn importing_an_inherited_requirement_keeps_the_final_signature_owner() {
    let caller = "use views; use views::Child::read; use views::Child::read; machine copy(x:numbers::Word)->numbers::Word{x} Adapter satisfies Child { Child::read = copy; }";
    let files = [
        ("app", "main.omg", caller),
        ("app", "numbers/types.omg", "public data Word {}"),
        ("app", "api/base.omg", "use numbers::Word; public trait Parent {machine read(x:Word)->Word;}"),
        ("app", "views/child.omg", "use api::Parent; public trait Child:Parent {}"),
    ];
    let stage = run(&files, &[]).unwrap();
    let declaration = stage
        .declarations
        .iter()
        .find(|d| d.path == "api::Parent::read")
        .unwrap();
    assert_eq!(declaration.signature_owners, [("app".into(), "numbers::Word".into())]);
    let selections: Vec<_> = stage
        .trees()
        .filter(|tree| tree.file.path == "main.omg")
        .flat_map(|tree| tree.occurrences())
        .filter(|u| matches!(&u.target, Symbol::Declaration(id) if id == declaration))
        .map(|u| u.span.slice(caller.as_bytes()).unwrap())
        .collect();
    assert_eq!(
        selections,
        [
            b"views::Child::read".as_slice(),
            b"views::Child::read",
            b"Child::read"
        ]
    );
}

#[test]
fn a_value_binder_cannot_merge_overloads_on_distinct_imported_types() {
    let files = [
        ("app", "red/point.omg", "public data Point {}"),
        ("app", "blue/point.omg", "public data Point {}"),
        ("app", "a.omg", "use red::Point; machine visit<const Point:u8>(p:Point) {}"),
        ("app", "b.omg", "use blue::Point; machine visit<const Point:u8>(p:Point) {}"),
        ("app", "caller.omg", "machine f(p:red::Point) { visit::<1>(p); }"),
    ];
    let stage = run(&files, &[]).unwrap();
    let candidates = stage
        .trees()
        .filter(|tree| tree.file.path == "caller.omg")
        .flat_map(|tree| tree.occurrences())
        .find_map(|name| match &name.target {
            Symbol::Candidates(ids) => Some(ids),
            _ => None,
        })
        .unwrap();
    let mut owners: Vec<_> = candidates
        .iter()
        .map(|id| id.signature_owners.as_slice())
        .collect();
    owners.sort();
    assert_eq!(
        owners,
        [
            &[("app".into(), "blue::Point".into())][..],
            &[("app".into(), "red::Point".into())]
        ]
    );
}

#[test]
fn a_public_value_name_cannot_expose_a_private_traits_inherited_requirements() {
    let caller = ("app", "main.omg", "use dep::Child::read;");
    let edge = Dependency {
        requester: "app",
        alias: "dep",
        package: "lib",
    };
    let library = "public trait Parent {machine read();} trait Child:Parent {} public machine Child() {}";
    let err = run(&[caller, ("lib", "lib.omg", library)], &[edge]).unwrap_err();
    assert_eq!(err.message, "unavailable import dep::Child::read");
    assert_eq!(err.span.slice(caller.2.as_bytes()), Some(b"dep::Child::read".as_slice()));
}

#[test]
fn declaration_qualifiers_report_the_name_that_cannot_select_an_owner() {
    for (program, spelling, message) in [
        ("trait Child:Owner {}", "Owner", "ambiguous trait Owner"),
        ("domain Point::Ready;", "Point::Ready", "ambiguous carrier Point"),
        ("machine Owner::run() {}", "Owner::run", "ambiguous machine owner Owner"),
        (
            "const Count:u8=0; machine Count::run() {}",
            "Count::run",
            "Count cannot qualify a machine declaration",
        ),
    ] {
        let source = format!("use left; use right; {program}");
        let files = [
            ("app", "main.omg", source.as_str()),
            ("app", "left/api.omg", "trait Owner {} data Point {}"),
            ("app", "right/api.omg", "trait Owner {} data Point {}"),
        ];
        let err = run(&files, &[]).unwrap_err();
        assert_eq!(err.message, message);
        assert_eq!(err.span.slice(source.as_bytes()), Some(spelling.as_bytes()));
    }
}

#[test]
fn conformance_rows_bind_only_machine_candidates() {
    let schema = "trait R { machine run(x:u8)->u8; }";
    for (data, reason) in [
        ("data D{} I satisfies D {}", "conformance requires a trait"),
        ("I satisfies u8 {}", "conformance requires an exact trait"),
        ("I satisfies @R { R::run = R::run; }", "missing conformance trait"),
        (
            "let mapping(x:u8):u8=x; I satisfies R { R::run = mapping; }",
            "a selection row must name a machine",
        ),
        ("I satisfies R { R::run = true; }", "a selection row must name a machine"),
    ] {
        let source = format!("{schema} {data}");
        let err = run(&[("app", "main.omg", &source)], &[]).unwrap_err();
        assert_eq!(err.message, reason, "{data}");
    }
    let mixed = format!("{schema} let mapping(x:u8):u8=x; machine mapping(x:u8)->u8 {{x}} I satisfies R {{ R::run = mapping; }}");
    let stage = run(&[("app", "main.omg", &mixed)], &[]).unwrap();
    let row = stage
        .uses()
        .find(|u| u.span.get_start_offset() == mixed.rfind("mapping").unwrap())
        .unwrap();
    assert!(matches!(&row.target, Symbol::Declaration(id) if id.kind==omega_parser::NodeKind::Machine));
}

#[test]
fn an_applied_overload_keeps_arguments_for_each_candidate_at_the_same_site() {
    let source = "machine pick<T>(x:u8) {} machine pick<T>(x:bool) {} machine caller() {pick::<u16>(true);}";
    let stage = run(&[("app", "overloads.omg", source)], &[]).unwrap();
    let tree = stage.trees().next().unwrap();
    let mut applications = tree.applications();
    let (_, name) = applications.next().unwrap();
    assert!(applications.next().is_none());
    let Symbol::Candidates(ids) = &name.target else {
        panic!("expected overloads")
    };
    assert_eq!(ids.len(), 2);
    for id in ids {
        let [arg] = tree.arguments_for(name.node.node, id) else {
            panic!("missing argument for {id:?}")
        };
        assert_eq!(tree.file.span(*arg).slice(source.as_bytes()), Some(b"u16".as_slice()));
    }
    assert!(tree.member(name.node.node).is_none());
    assert!(tree.application_name(name.node.node).is_none());
    let caller = stage.declarations.iter().find(|d| d.path == "caller").unwrap();
    assert!(tree.arguments_for(name.node.node, caller).is_empty());
}

#[test]
fn qualified_lookup_preserves_each_segments_scope() {
    let source = "machine caller() {nested::api::<u8>::score();}";
    let err = run(
        &[
            ("app", "main.omg", source),
            ("app", "nested/api/lib.omg", "machine score() {}"),
        ],
        &[],
    )
    .unwrap_err();
    assert_eq!(err.message, "unresolved application owner nested::api");
    assert_eq!(err.span.slice(source.as_bytes()), Some(b"nested::api".as_slice()));
    for (source, name) in [
        ("machine f(x:u8){machine helper(){}} machine f(x:bool){helper();}", "helper"),
        ("data Choice {case One;} use Choice::One; machine f() {One}", "One"),
    ] {
        let err = run(&[("app", "main.omg", source)], &[]).unwrap_err();
        assert_eq!(err.message, format!("unresolved name {name}"));
        assert_eq!(err.span.get_start_offset(), source.rfind(name).unwrap());
    }
}

#[test]
fn a_trait_inheritance_cycle_identifies_the_trait_declaration() {
    let source = "machine A() {} trait A:B {} trait B:A {}";
    let err = run(&[("app", "cycle.omg", source)], &[]).unwrap_err();
    assert_eq!(err.message, "cyclic trait inheritance");
    assert_eq!(err.span.slice(source.as_bytes()), Some(b"trait A:B {}".as_slice()));
}

#[test]
fn module_import_exposes_only_public_foreign_domain_attachments() {
    let library = ("lib", "units/defs.omg", "public domain<T> T::Any; domain u8::Hidden;");
    let edge = Dependency {
        requester: "app",
        alias: "dep",
        package: "lib",
    };
    for (path, decl) in [
        ("u8::Any", "units::T::Any"),
        ("dep::units::T::Any", "units::T::Any"),
    ] {
        let source = format!("use dep::units; machine f(x:u8 in {path}) {{}}");
        let stage = run(&[library, ("app", "main.omg", &source)], &[edge]).unwrap();
        let name = stage
            .trees()
            .filter(|tree| tree.file.path == "main.omg")
            .flat_map(|tree| tree.occurrences())
            .find(|u| u.span.slice(source.as_bytes()) == Some(path.as_bytes()))
            .unwrap();
        assert!(matches!(&name.target,Symbol::Declaration(id) if id.package=="lib" && id.path==decl));
    }
    let source = "use dep::units; machine f(x:u8 in u8::Hidden) {}";
    let err = run(&[library, ("app", "main.omg", source)], &[edge]).unwrap_err();
    assert_eq!(err.message, "unresolved name u8::Hidden");
}

#[test]
fn a_domain_passed_to_a_bound_parameter_retains_its_written_carrier() {
    let source = "domain<T> T::Flag; machine take<const A:core::Type<0>>(){} machine test(){take::<u8::Flag>();}";
    let stage = run(&[("app", "main.omg", source)], &[]).unwrap();
    let tree = stage.trees().next().unwrap();
    let usage = stage
        .uses()
        .find(|u| u.span.slice(source.as_bytes()) == Some(b"u8::Flag"))
        .unwrap();
    assert!(matches!(&usage.target,Symbol::Declaration(id) if id.path=="T::Flag"));
    assert!(matches!(tree.qualifier(usage.node.node),Some(Symbol::Builtin(name)) if name=="u8"));
}

#[test]
fn a_compiled_trait_retains_its_identity_as_a_parent_and_a_call_qualifier() {
    let schema = run(
        &[("wire", "schema.omg", "public trait Parent {machine run();} public trait Middle:Parent {}")],
        &[],
    )
    .unwrap();
    let mut exports = schema.compiled_interface();
    exports.declarations.reverse();
    let middle = exports
        .declarations
        .iter()
        .find(|e| e.id.path == "Middle")
        .unwrap();
    assert_eq!(
        middle.trait_parents.as_slice(),
        [schema
            .declarations
            .iter()
            .find(|id| id.path == "Parent")
            .unwrap()
            .clone()]
    );
    let text = "trait Child:wire::Middle {} machine f(){wire::Middle::run();}";
    let tokens = lex(text.as_bytes()).unwrap();
    let parsed = parse_file(text.as_bytes(), &tokens).unwrap();
    let sources = [StageSource {
        package: "app",
        source: ParsedSource {
            logical_path: "main.omg",
            text,
            tokens: &tokens,
            tree: &parsed,
        },
    }];
    let stage = omega_resolver::resolve_stage_with_exports(
        &sources,
        &[Dependency {
            requester: "app",
            alias: "wire",
            package: "wire",
        }],
        &Default::default(),
        &exports,
    )
    .unwrap();
    let child = stage.declarations.iter().find(|id| id.path == "Child").unwrap();
    assert_eq!(stage.trait_ancestors(child).collect::<Vec<_>>(), [("wire", "Middle"), ("wire", "Parent")]);
    let call = stage
        .uses()
        .find(|u| u.span.slice(text.as_bytes()) == Some(b"wire::Middle::run"))
        .unwrap();
    let tree = stage.trees().next().unwrap();
    assert!(matches!(tree.qualifier(call.node.node),Some(Symbol::Declaration(id)) if id.path=="Middle" && id.package=="wire"));
    assert!(matches!(&call.target,Symbol::Declaration(id) if id.path=="Parent::run" && id.package=="wire"));

    let parent = exports
        .declarations
        .iter()
        .position(|e| e.id.path == "Parent")
        .unwrap();
    exports.declarations[parent].trait_parents.push(child.clone());
    let reject = |exports: &omega_resolver::CompiledInterface| {
        omega_resolver::resolve_stage_with_exports(&[], &[], &Default::default(), exports)
            .unwrap_err()
            .message
    };
    assert_eq!(reject(&exports), "unknown parent trait in compiled interface");
    let middle = exports
        .declarations
        .iter()
        .find(|e| e.id.path == "Middle")
        .unwrap()
        .id
        .clone();
    exports.declarations[parent].trait_parents = vec![middle];
    assert_eq!(reject(&exports), "cyclic trait inheritance");
}

#[test]
fn packaged_witnesses_do_not_take_a_written_argument_slot() {
    let text = "data Packet<some S,const N:u64>{view:@S;bytes:[u8;N];} data X {} const X:u64=2; machine f(input:Packet<X>){}";
    let stage = run(&[("app", "packet.omg", text)], &[]).unwrap();
    let site = text.rfind("X>").unwrap();
    let name = stage
        .uses()
        .find(|u| u.span.get_start_offset() == site)
        .unwrap();
    assert!(
        matches!(&name.target, Symbol::ScopedNames { context: omega_resolver::NameContext::GenericArgument, names }
            if matches!(names.as_slice(), [Symbol::Declaration(data), Symbol::Declaration(constant)]
                if data.path == "X" && data.kind == omega_parser::NodeKind::Data
                && constant.path == "X" && constant.kind == omega_parser::NodeKind::Const))
    );
}

#[test]
fn patterns_publish_the_local_they_introduce_at_its_exact_spelling() {
    let source =
        "data Pair {left:u8;right:u8;} const K:u8=1; machine f(p:Pair)->u8 { let Pair{left as slot,right as _}=p; match slot { K -> 0, other -> other, } }";
    let bound = run(&[("app", "p.omg", source)], &[]).unwrap();
    let tree = bound.trees().next().unwrap();
    let bindings: Vec<_> = (0..tree.file.nodes.len())
        .filter_map(|node| {
            tree.pattern_binding(node)
                .map(|span| (tree.file.span(node).slice(source.as_bytes()).unwrap(), span.slice(source.as_bytes()).unwrap()))
        })
        .collect();
    assert_eq!(
        bindings,
        [
            (b"left as slot".as_slice(), b"slot".as_slice()),
            (b"K".as_slice(), b"K".as_slice()),
            (b"other".as_slice(), b"other".as_slice())
        ]
    );
}

#[test]
fn each_resolved_tree_retains_its_requesters_direct_dependencies() {
    let files = [
        ("app", "main.omg", "machine run(){}"),
        ("facade", "api.omg", "public machine point(){}"),
        ("geometry", "point.omg", "public data Point{}"),
    ];
    let edges = [
        Dependency {
            requester: "app",
            alias: "api",
            package: "facade",
        },
        Dependency {
            requester: "facade",
            alias: "geo",
            package: "geometry",
        },
    ];
    let stage = run(&files, &edges).unwrap();
    let app = stage.trees().find(|tree| tree.file.package == "app").unwrap();
    assert!(app.can_select_from("app"));
    assert!(app.can_select_from("facade"));
    assert!(!app.can_select_from("geometry"));
    let facade = stage
        .trees()
        .find(|tree| tree.file.package == "facade")
        .unwrap();
    assert!(facade.can_select_from("geometry"));
    assert!(!facade.can_select_from("app"));
}

#[test]
fn compiled_modules_and_lookup_results_survive_export_reordering() {
    let files = [
        ("lib", "detail/only.omg", "data Hidden{}"),
        (
            "lib",
            "api/items.omg",
            "public const n:u8=1; public machine draw(x:u8){} public machine draw(x:u16){}",
        ),
        ("other", "other.omg", "public const n:u8=2;"),
    ];
    let edges = [
        Dependency {
            requester: "app",
            alias: "lib",
            package: "lib",
        },
        Dependency {
            requester: "app",
            alias: "other",
            package: "other",
        },
    ];
    let mut interface = run(&files, &[]).unwrap().compiled_interface();
    let consume = |text, interface: &omega_resolver::CompiledInterface| {
        let tokens = lex(str::as_bytes(text)).unwrap();
        let tree = parse_file(text.as_bytes(), &tokens).unwrap();
        let source = ParsedSource {
            logical_path: "main.omg",
            text,
            tokens: &tokens,
            tree: &tree,
        };
        omega_resolver::resolve_stage_with_exports(&[StageSource { package: "app", source }], &edges, &Default::default(), interface)
    };
    let client = "use lib::detail; use lib::api::draw; machine f(){draw(1u8)}";
    let source = run(&[files[0], files[1], files[2], ("app", "main.omg", client)], &edges).unwrap();
    let compiled = consume(client, &interface).unwrap();
    let client_uses = source
        .trees()
        .find(|tree| tree.file.package == "app")
        .unwrap()
        .occurrences();
    assert!(client_uses.eq(compiled.uses()));
    let hidden = "use lib::detail::Hidden;";
    let hidden_refusal = consume(hidden, &interface).unwrap_err();
    assert_eq!(hidden_refusal.message, "unavailable import lib::detail::Hidden");
    assert_eq!(hidden_refusal.span.slice(hidden.as_bytes()), Some(b"lib::detail::Hidden".as_slice()));
    let competing = "use lib::api::n; use other::n; machine f(){n}";
    let refusal = consume(competing, &interface).unwrap_err();
    assert_eq!(refusal.message, "competing declarations: lib::api::n, other::n");
    for (text, expected) in [(hidden, &hidden_refusal), (competing, &refusal)] {
        let from_source = run(&[files[0], files[1], files[2], ("app", "main.omg", text)], &edges).unwrap_err();
        assert_eq!(&from_source, expected);
    }
    interface.declarations.reverse();
    assert_eq!(compiled, consume(client, &interface).unwrap());
    assert_eq!(hidden_refusal, consume(hidden, &interface).unwrap_err());
    assert_eq!(refusal, consume(competing, &interface).unwrap_err());
}

#[test]
fn an_interface_exports_public_declarations_with_their_defining_module() {
    let code = "public data Box<T>{value:T;} data Secret{v:u8;} public machine wrap<T>(n:T)->Box<T>{ Box::<T>{value:n} machine local(){} }";
    let stage = run(&[("lib", "kit/api.omg", code)], &[]).unwrap();
    let exports = stage.compiled_interface();
    let paths: Vec<_> = exports
        .declarations
        .iter()
        .map(|export| export.id.path.as_str())
        .collect();
    assert_eq!(paths, ["kit::Box", "kit::Box::value", "kit::wrap"]);
    assert!(exports.declarations.iter().all(|export| export.module == "kit"));
    let imported = omega_resolver::resolve_stage_with_exports(&[], &[], &Default::default(), &exports).unwrap();
    assert_eq!(
        imported.declarations,
        exports
            .declarations
            .into_iter()
            .map(|e| e.id)
            .collect::<Vec<_>>()
    );
}

#[test]
fn identical_source_nodes_in_two_packages_keep_their_own_application_bindings() {
    let caller = "use dep::api; machine f(x:Box<u8>) {}";
    let library = "public data Box<T>{}";
    let stage = run(
        &[
            ("left", "main.omg", caller),
            ("right", "main.omg", caller),
            ("a", "api/types.omg", library),
            ("b", "api/types.omg", library),
        ],
        &[
            Dependency {
                requester: "left",
                alias: "dep",
                package: "a",
            },
            Dependency {
                requester: "right",
                alias: "dep",
                package: "b",
            },
        ],
    )
    .unwrap();
    let mut nodes = Vec::new();
    for tree in stage.trees().filter(|tree| tree.file.path == "main.omg") {
        let expected = if tree.file.package == "left" { "a" } else { "b" };
        let [(section, name)] = tree.applications().collect::<Vec<_>>()[..] else {
            panic!("expected one application")
        };
        assert!(matches!(&name.target, Symbol::Declaration(id) if id.package == expected && id.path == "api::Box"));
        assert!(
            tree.names(name.node.node)
                .any(|found| std::ptr::eq(found, name))
        );
        assert_eq!(tree.file.nodes[section].parent, Some(name.node.node));
        assert!(tree.occurrences().map(|usage| usage.node).is_sorted());
        nodes.push(name.node);
    }
    assert_eq!(nodes.len(), 2);
    assert_eq!(nodes[0].node, nodes[1].node);
    assert_ne!(nodes[0].file, nodes[1].file);
}

#[test]
fn qualified_machine_heads_retain_the_exact_owner_occurrence() {
    use omega_resolver::NameRole;

    let source =
        "trait Logger { machine write(); } Sink satisfies Logger { Logger::write = Sink::write; } machine Sink::write() {} machine support::helper() {}";
    let stage = run(
        &[
            ("app", "main.omg", source),
            ("app", "support/marker.omg", "machine marker() {}"),
        ],
        &[],
    )
    .unwrap();
    let qualifier_at = |head: &str| {
        let offset = source.rfind(head).unwrap();
        stage
            .uses()
            .find(|name| name.role == NameRole::Qualifier && name.span.get_start_offset() == offset)
            .unwrap()
    };
    let conformance = qualifier_at("Sink::write");
    assert_eq!(conformance.span.slice(source.as_bytes()), Some(b"Sink".as_slice()));
    assert!(
        matches!(&conformance.target, Symbol::Declaration(id) if id.package == "app" && id.path == "Sink" && id.kind == omega_parser::NodeKind::Conformance)
    );
    let module = qualifier_at("support::helper");
    assert_eq!(module.span.slice(source.as_bytes()), Some(b"support".as_slice()));
    assert!(matches!(&module.target, Symbol::Module { package, path } if package == "app" && path == "support"));
}

#[test]
fn importing_a_module_cannot_hide_a_dependency_alias_conflict() {
    let edge = Dependency {
        requester: "app",
        alias: "dep",
        package: "lib",
    };
    let source = "use other::dep; machine f() { dep::run() }";
    let files = [
        ("app", "main.omg", source),
        ("app", "other/dep/run.omg", "machine run() {}"),
        ("lib", "root.omg", "public machine run() {}"),
    ];
    let stage = run(&files, &[edge]).unwrap();
    assert_eq!(selected(&stage, "main.omg", source, "dep::run"), ["app::other::dep::run"]);

    let mut conflicting = files.to_vec();
    conflicting.push(("app", "dep/marker.omg", "machine marker() {}"));
    let error = run(&conflicting, &[edge]).unwrap_err();
    assert_eq!(error.message, "competing module paths dep: package lib and app::dep");
    assert_eq!(error.span.slice(source.as_bytes()), Some(b"dep::run".as_slice()));
}
