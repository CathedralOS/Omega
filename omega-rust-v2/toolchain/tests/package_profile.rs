use omega_resolver::{ParsedSource, StageSource, Symbol};
use omega_toolchain::{TOOLCHAIN_PACKAGE, core_interface, resolve_source_package};

#[test]
fn the_package_profile_binds_source_imports_to_its_published_declarations() {
    let text = "use omega::language::core; machine f<'a>(x:core::AccessPlan<'a>){}";
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
    let constructor = core.id("access::new_plan");
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
            (core.id("Nat::Descending"), true),
            (core.id("Nat::BoundedDistance"), false)
        ]
    );
    assert_eq!(core.access_plan_requirement(), core.id("AccessPolicy::plan"));
}
