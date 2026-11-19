use omega_lexer::lex;
use omega_parser::{NodeKind, parse_file};
use omega_resolver::{ParsedSource, StageSource, Symbol, SymbolResolvedTrees, resolve_stage};

fn resolve(source: &str) -> SymbolResolvedTrees<'_> {
    let tokens = lex(source.as_bytes()).unwrap();
    let tree = parse_file(source.as_bytes(), &tokens).unwrap();
    resolve_stage(
        &[StageSource {
            package: "app",
            source: ParsedSource {
                logical_path: "main.omg",
                text: source,
                tokens: &tokens,
                tree: &tree,
            },
        }],
        &[],
    )
    .unwrap()
}

#[test]
fn applied_machine_generic_argument_keeps_both_application_owners() {
    let stage = resolve("machine id<T>(x:T)->T{x} machine apply<const F:machine(x:u8)->u8>(x:u8)->u8{F(x)}          machine f(x:u8)->u8{apply::<id::<u8>>(x)}");
    let tree = stage.trees().next().unwrap();
    let owners: Vec<_> = tree
        .file
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.kind == NodeKind::ExpressionArgs)
        .map(|(node, _)| {
            let Symbol::Declaration(id) = &tree.application_name(node).unwrap().target else {
                panic!("the application must bind to its machine");
            };
            id.path.as_str()
        })
        .collect();
    assert_eq!(owners, ["apply", "id"]);
}

#[test]
fn a_public_signature_use_preserves_private_candidates_for_typed_selection() {
    let stage = resolve("public machine pred(x:u8)->bool{true} machine pred(x:u16)->bool{true}          public machine f(x:u8) requires pred(x) {}");
    let tree = stage.trees().next().unwrap();
    let site = tree
        .occurrences()
        .find(|name| matches!(&name.target, Symbol::Candidates(_)))
        .unwrap();
    let [private] = tree.private_signature_candidates(site.node.node) else {
        panic!("the use must distinguish the private overload");
    };
    assert_eq!(private.operands, ["u16"]);
}

#[test]
fn a_public_termination_guarantee_can_use_private_witness_declarations() {
    let stage = resolve("data Down{} const limit:u8=7; public machine f(x:u8) terminates by x -> Down in limit {}");
    let names: Vec<_> = stage
        .uses()
        .filter_map(|name| match &name.target {
            Symbol::Declaration(id) => Some(id.path.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(names, ["Down", "limit"]);
}

#[test]
fn an_applied_argument_can_preserve_both_declaration_roles() {
    let stage = resolve("data item<T>{} machine item<T>(x:T)->T{x} machine use_type<T>(){}          machine f(){use_type::<item::<u8>>();}");
    let tree = stage.trees().next().unwrap();
    let (_, argument) = tree
        .applications()
        .find(|(_, name)| matches!(&name.target, Symbol::ScopedNames { .. }))
        .expect("the inner application must preserve both type and value declarations");
    let roles: Vec<_> = argument
        .target
        .visible_names()
        .iter()
        .map(|symbol| match symbol {
            Symbol::Declaration(id) => id.kind,
            other => panic!("unexpected application owner {other:?}"),
        })
        .collect();
    assert_eq!(roles, [NodeKind::Data, NodeKind::Machine]);
}
