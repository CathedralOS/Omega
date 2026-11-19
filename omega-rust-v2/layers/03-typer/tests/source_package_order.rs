use omega_lexer::SourceSpan;
use omega_resolver::{ParsedSource, StageSource};
use omega_typer::{TypeError, TypedStage, ValueType, resolve_source_package, type_source_package};

const FILE_ORDERS: [[usize; 3]; 6] = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
const DRAW: &str = include_str!("../fixtures/draw.omg");
const RENDER: &str = include_str!("../fixtures/render.omg");
const WRONG_RENDER: &str = include_str!("../fixtures/wrong_result.omg");
const READ: &str = "machine read(items: @[bool], i: u64) -> bool { items[i] }";
const WRONG_READ: &str = "machine read(items: @[bool], i: u64) -> u8 { items[i] }";

fn type_package(
    render: &str,
    read: &str,
    order: [usize; 3],
) -> Result<TypedStage, TypeError> {
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
    type_source_package(&sources, &resolved)
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
