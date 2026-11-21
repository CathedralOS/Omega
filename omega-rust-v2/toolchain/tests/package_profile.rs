use omega_resolver::{ParsedSource, StageSource, Symbol};
use omega_toolchain::{TOOLCHAIN_PACKAGE, core_interface, resolve_source_package};

#[test]
fn the_package_profile_binds_source_imports_to_its_published_declarations() {
    let text = "use omega\\language\\core; machine f<'a>(x:core\\AccessPlan<'a>){}";
    let tokens = omega_lexer::lex(text.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(text.as_bytes(), &tokens).unwrap();
    let source = |package| StageSource {
        package,
        source: ParsedSource {
            logical_path: "app.omg",
            text,
            tokens: &tokens,
            tree: &tree,
        },
    };
    let resolved = resolve_source_package(&[source("app")]).unwrap();
    let plan = core_interface().id("AccessPlan");
    assert!(
        resolved
            .uses()
            .any(|name| matches!(&name.target, Symbol::Declaration(id) if id == plan))
    );
    let refusal = resolve_source_package(&[source(TOOLCHAIN_PACKAGE)]).unwrap_err();
    assert_eq!(refusal.package, TOOLCHAIN_PACKAGE);
    assert!(refusal.message.contains("reserved"));
}

#[test]
fn constructor_guarantees_are_published_by_the_library() {
    let core = core_interface();
    let constructor = core.id("access\\new_plan");
    let (tree, node) = core
        .declarations
        .trees()
        .find_map(|tree| {
            tree.declaration_nodes()
                .find(|(_, id)| *id == constructor)
                .map(|(node, _)| (tree, node))
        })
        .unwrap();
    let guarantees: Vec<_> = tree.file.nodes[node]
        .children
        .iter()
        .filter(|&&n| tree.file.nodes[n].kind == omega_parser::NodeKind::Ensures)
        .map(|&n| {
            let expr = &tree.file.nodes[tree.file.nodes[n].children[0]];
            tree.file.tokens[expr.first_token..expr.end_token]
                .iter()
                .map(|t| t.text)
                .collect::<String>()
        })
        .collect();
    assert_eq!(
        guarantees,
        [
            "result.field_count==0",
            "result.reader_count==0",
            "result.writer_count==0",
            "result.reach.service_count==0"
        ]
    );
}

#[test]
fn the_profile_identifies_the_default_ranking_view_and_the_access_requirement() {
    let core = core_interface();
    assert_eq!(
        core.ranking_views(),
        [
            (core.id("Nat\\Descending"), true),
            (core.id("Nat\\BoundedDistance"), false)
        ]
    );
    assert_eq!(core.access_plan_requirement(), core.id("AccessPolicy::plan"));
}

/// The closed catalog holds exactly the bodyless core machines outside a trait.
/// A core machine with a checked body, such as `access\new_plan`, is not in it
/// (language/machines.md#supply; packages/toolchain.md).
#[test]
fn the_closed_catalog_is_exactly_the_bodyless_core_machines() {
    use omega_parser::NodeKind::{Body, Machine, Trait};
    let core = core_interface();
    let mut seen = Vec::new();
    for tree in core.declarations.trees() {
        let file = tree.file;
        for (node, id) in tree.declaration_nodes() {
            if file.nodes[node].kind != Machine
                || file.nodes[node]
                    .parent
                    .is_some_and(|p| file.nodes[p].kind == Trait)
            {
                continue;
            }
            let bodyless = file.child(node, Body).is_none();
            assert_eq!(core.supplies(id), bodyless, "{}", id.path);
            seen.push((id.path.as_str(), bodyless));
        }
    }
    assert!(seen.contains(&("language\\core\\access\\new_plan", false)));
    assert!(seen.contains(&("language\\core\\size_of", true)));
    assert_eq!(seen.iter().filter(|(_, bodyless)| *bodyless).count(), 9, "{seen:?}");
}

#[test]
fn the_profile_owns_the_canonical_process_exit_requirement() {
    let core = core_interface();
    let (owner, requirement) = core.process_exit_requirement();
    assert_eq!((owner.package.as_str(), owner.path.as_str()), (TOOLCHAIN_PACKAGE, "language\\core\\ProcessExit"));
    assert_eq!(owner.kind, omega_parser::NodeKind::Trait);
    assert_eq!(
        (requirement.package.as_str(), requirement.path.as_str()),
        (TOOLCHAIN_PACKAGE, "language\\core\\ProcessExit::exit_process")
    );
    assert_eq!(requirement.kind, omega_parser::NodeKind::Machine);
    assert_eq!(core.id("Never").kind, omega_parser::NodeKind::Data);
}

#[test]
fn reflection_queries_resolve_through_the_package_interface() {
    use omega_toolchain::DeclarationQuery;

    for (imports, expression) in [
        (r"use omega\language\core\reflect;", r"reflect\field"),
        (r"use omega\language\core\reflect;", "field"),
        (r"use omega\language\core\reflect\field;", "field"),
        ("", r"omega\language\core\reflect\field"),
    ] {
        let text = format!("{imports} data R{{x:u8;}} machine f(){{let d={expression}(R::x);}}");
        let tokens = omega_lexer::lex(text.as_bytes()).unwrap();
        let tree = omega_parser::parse_file(text.as_bytes(), &tokens).unwrap();
        let resolved = resolve_source_package(&[StageSource {
            package: "app",
            source: ParsedSource {
                logical_path: "app.omg",
                text: &text,
                tokens: &tokens,
                tree: &tree,
            },
        }])
        .unwrap();
        let start = text.rfind(expression).unwrap();
        assert!(
            resolved.uses().any(|name| name.span.get_start_offset() == start
                && matches!(&name.target, Symbol::DeclarationQuery { package, path }
                    if DeclarationQuery::identify(package, path) == Some(DeclarationQuery::Field))),
            "{text}"
        );
        assert!(
            resolved.uses().any(|name| matches!(&name.target,
            Symbol::Declaration(id) if id.package == "app" && id.path == "R::x")),
            "{text}"
        );
    }
}

#[test]
fn equally_visible_query_and_machine_report_both_identities() {
    let text = r"use omega\language\core\reflect; machine field(){} machine f(){field();}";
    let tokens = omega_lexer::lex(text.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(text.as_bytes(), &tokens).unwrap();
    let refusal = resolve_source_package(&[StageSource {
        package: "app",
        source: ParsedSource { logical_path: "app.omg", text, tokens: &tokens, tree: &tree },
    }]).unwrap_err();
    assert_eq!(refusal.kind, omega_resolver::NameErrorKind::CompetingDeclarations);
    assert_eq!(refusal.message, r"competing declarations: @omega/toolchain\language\core\reflect\field, app\field");
    assert_eq!(refusal.span.get_start_offset(), text.rfind("field").unwrap());
    assert_eq!(refusal.span.slice(text.as_bytes()), Some(b"field".as_slice()));
}

#[test]
fn admitted_dependency_edges_reach_only_their_requester_beside_the_toolchain_edge() {
    use omega_resolver::{Dependency, NameErrorKind};
    use omega_toolchain::resolve_admitted_packages;

    let files = [
        ("app", "main.omg", r"use kit\score; use omega\language\core; machine f() -> u32 { score() }"),
        ("lib", "main.omg", "public machine v() -> u32 { 1 }"),
        ("lib", "other.omg", "public machine score() -> u32 { 7 }"),
        ("tool", "main.omg", r"use kit\score; machine g() -> u32 { score() }"),
    ];
    let parsed: Vec<_> = files
        .iter()
        .map(|(_, _, text)| {
            let tokens = omega_lexer::lex(text.as_bytes()).unwrap();
            let tree = omega_parser::parse_file(text.as_bytes(), &tokens).unwrap();
            (tokens, tree)
        })
        .collect();
    let sources = |count: usize| -> Vec<StageSource> {
        files
            .iter()
            .zip(&parsed)
            .take(count)
            .map(|((package, path, text), (tokens, tree))| StageSource {
                package,
                source: ParsedSource {
                    logical_path: path,
                    text,
                    tokens,
                    tree,
                },
            })
            .collect()
    };
    let kit = Dependency {
        requester: "app",
        alias: "kit",
        package: "lib",
    };
    let resolved = resolve_admitted_packages(&sources(3), &[kit]).unwrap();
    let call = files[0].2.rfind("score").unwrap();
    assert!(
        resolved
            .uses()
            .any(|name| name.span.get_start_offset() == call && matches!(&name.target, Symbol::Declaration(id) if id.package == "lib" && id.path == "score"))
    );

    // Without the admitted edge the same root has no `kit` alias.
    let missing = resolve_admitted_packages(&sources(3), &[]).unwrap_err();
    assert_eq!((missing.package.as_str(), missing.message.as_str()), ("app", r"unavailable import kit\score"));
    assert_eq!(missing.span.get_start_offset(), files[0].2.find("kit").unwrap());

    // The edge belongs to `app`; a second requester naming the same alias has no authority.
    let other = resolve_admitted_packages(&sources(4), &[kit]).unwrap_err();
    assert_eq!((other.package.as_str(), other.message.as_str()), ("tool", r"unavailable import kit\score"));

    // An admitted edge cannot retarget the toolchain alias the profile supplies.
    let retarget = Dependency {
        requester: "app",
        alias: "omega",
        package: "lib",
    };
    let conflict = resolve_admitted_packages(&sources(3), &[kit, retarget]).unwrap_err();
    assert_eq!(conflict.kind, NameErrorKind::ConflictingDependencyAlias);
    assert_eq!(conflict.message, "conflicting packages for dependency alias omega");
}
