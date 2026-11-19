use omega_lexer::SourceSpan;
use omega_resolver::{ParsedSource, StageSource};
use omega_typer::{NodeSemantics, TypeError, TypedTrees, ValueType, resolve_source_package, type_source_package};

const FILE_ORDERS: [[usize; 3]; 6] = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
const DRAW: &str = include_str!("../fixtures/draw.omg");
const RENDER: &str = include_str!("../fixtures/render.omg");
const WRONG_RENDER: &str = include_str!("../fixtures/wrong_result.omg");
const READ: &str = "machine read(items: @[bool], i: u64) -> bool { items[i] }";
const WRONG_READ: &str = "machine read(items: @[bool], i: u64) -> u8 { items[i] }";

fn type_package<'text>(
    render: &'text str,
    read: &'text str,
    order: [usize; 3],
) -> Result<TypedTrees<'text>, TypeError> {
    let parsed = [("draw.omg", DRAW), ("render.omg", render), ("view.omg", read)].map(|(path, text)| {
        let tokens = omega_lexer::lex(text.as_bytes()).unwrap();
        let tree = omega_parser::parse_file(text.as_bytes(), &tokens).unwrap();
        (path, text, tokens, tree)
    });
    let sources = order.map(|index| {
        let (path, text, tokens, tree) = &parsed[index];
        StageSource {
            package: "app",
            source: ParsedSource {
                logical_path: path,
                text,
                tokens,
                tree,
            },
        }
    });
    let resolved = resolve_source_package(&sources).expect("all three files resolve, including the negative twins");
    drop(parsed);
    type_source_package(&resolved)
}

#[test]
fn source_package_preserves_selections_and_obligations_in_every_file_order() {
    let expected = type_package(RENDER, READ, FILE_ORDERS[0]).unwrap();
    let [draw] = expected.calls.as_slice() else {
        panic!("expected one source call");
    };
    assert_eq!(draw.machine.path, "draw");
    assert_eq!(draw.machine.operands, ["bool"]);
    assert_eq!(draw.result, ValueType::Bool);
    let [read] = expected.collection_accesses.as_slice() else {
        panic!("expected one indexed read");
    };
    assert_eq!(read.machine.path, "indexing::read");
    assert!(
        expected
            .obligations
            .iter()
            .any(|obligation| obligation.site == read.site
                && matches!(&obligation.kind, omega_typer::ObligationKind::CallPrecondition(machine) if *machine == read.machine))
    );

    for order in FILE_ORDERS {
        assert_eq!(type_package(RENDER, READ, order).unwrap(), expected, "order {order:?}");
    }
}

#[test]
fn source_package_reports_the_same_first_error_in_every_file_order() {
    for (render, read, file, text, expression, message) in [
        (WRONG_RENDER, READ, "render.omg", WRONG_RENDER, "draw(flag)", "expected u32, found bool"),
        (RENDER, WRONG_READ, "view.omg", WRONG_READ, "items[i]", "expected u8, found bool"),
        (WRONG_RENDER, WRONG_READ, "render.omg", WRONG_RENDER, "draw(flag)", "expected u32, found bool"),
    ] {
        let start = text.find(expression).unwrap();
        let expected = TypeError {
            package: "app".into(),
            file: file.into(),
            span: SourceSpan::new(start, start + expression.len()),
            message: message.into(),
        };
        for order in FILE_ORDERS {
            assert_eq!(type_package(render, read, order).unwrap_err(), expected, "order {order:?}");
        }
    }
}

#[test]
fn a_consumer_walks_both_typed_arms_without_parser_inputs() {
    use omega_parser::NodeKind;
    use omega_typer::ObligationKind;
    let source = "machine choose(b:bool,items:@[bool])->bool {match b {true->draw(b),false->items[0],}}";
    let typed = type_package(source, READ, FILE_ORDERS[2]).unwrap();
    let tree = typed
        .trees()
        .find(|t| t.resolved().file.path == "render.omg")
        .unwrap();
    let file = tree.resolved().file;
    // The tail and the match occupy the same bytes, but only the match is typed.
    let tail = file
        .nodes
        .iter()
        .position(|node| node.kind == NodeKind::Tail)
        .unwrap();
    let match_node = file.nodes[tail].children[0];
    assert_eq!(file.span(tail), file.span(match_node));
    assert_eq!(tree.semantics_at(tail).count(), 0);
    assert!(
        tree.semantics_at(match_node)
            .any(|fact| matches!(fact, NodeSemantics::Expression(e) if e.ty == ValueType::Bool))
    );
    let dispatch = file
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::Match)
        .unwrap();
    let [_, first_arm, second_arm] = *dispatch.children.as_slice() else {
        panic!("expected two arms")
    };
    let first = file.nodes[first_arm].children[1];
    let second = file.nodes[second_arm].children[1];
    assert_eq!(file.nodes[first].kind, NodeKind::Call);
    assert_eq!(file.nodes[second].kind, NodeKind::Index);
    let callee = file.nodes[first].children[0];
    assert!(matches!(tree.resolved().symbol(callee), Some(omega_resolver::Symbol::Candidates(ids)) if ids.len() == 2));
    let selected = tree
        .semantics_at(first)
        .find_map(|fact| match fact {
            NodeSemantics::Call(call) => Some(call),
            _ => None,
        })
        .unwrap();
    assert_eq!(selected.machine.operands, ["bool"]);
    assert_eq!(file.span(selected.operands[0].node).slice(source.as_bytes()), Some(b"b".as_slice()));
    assert_eq!(selected.site.span.slice(source.as_bytes()), Some(b"draw(b)".as_slice()));
    assert!(
        tree.semantics_at(second)
            .any(|fact| matches!(fact, NodeSemantics::Expression(e) if e.ty == ValueType::Bool))
    );
    assert!(typed.obligations.iter().any(|o| o.site.package == file.package
        && o.site.file == file.path
        && o.site.span == file.span(second)
        && matches!(&o.kind, ObligationKind::CallPrecondition(id) if id.path == "indexing::read")));
}
