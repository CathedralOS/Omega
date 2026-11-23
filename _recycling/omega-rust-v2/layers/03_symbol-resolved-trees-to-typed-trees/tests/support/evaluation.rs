use omega_lexer::{SourceSpan, lex};
use omega_parser::{NodeKind, parse_file};
use omega_resolver::TreeNodeId;

pub fn node(
    source: &str,
    start: usize,
    expression: &str,
    kind: NodeKind,
) -> TreeNodeId {
    let tokens = lex(source.as_bytes()).unwrap();
    let tree = parse_file(source.as_bytes(), &tokens).unwrap();
    let syntax = tree.index(source, &tokens).unwrap();
    let span = SourceSpan::new(start, start + expression.len());
    let node = (0..syntax.nodes.len())
        .find(|&node| syntax.nodes[node].kind == kind && syntax.span(node) == span)
        .unwrap_or_else(|| panic!("expected {kind:?} at {span:?} in {source}"));
    TreeNodeId { file: 0, node }
}
