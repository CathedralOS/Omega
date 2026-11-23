use omega_resolver::{Dependency, ParsedSource, StageSource, resolve_stage};
use omega_typer::{TypedTrees, type_stage};

/// Resolve and type both packages together. Only the client may name `geo`.
pub(super) fn typed<'text>(
    library: &'text str,
    client: &'text str,
) -> TypedTrees<'text> {
    let texts = [("geometry", "geometry.omg", library), ("client", "client.omg", client)];
    let syntax: Vec<_> = texts
        .iter()
        .map(|&(_, _, text)| {
            let tokens = omega_lexer::lex(text.as_bytes()).unwrap();
            let tree = omega_parser::parse_file(text.as_bytes(), &tokens).unwrap();
            (tokens, tree)
        })
        .collect();
    let sources: Vec<_> = texts
        .iter()
        .zip(&syntax)
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
    let dependencies = [Dependency {
        requester: "client",
        alias: "geo",
        package: "geometry",
    }];
    let resolved = resolve_stage(&sources, &dependencies).unwrap_or_else(|error| panic!("{error:?}"));
    type_stage(&resolved).unwrap_or_else(|error| panic!("{error:?}"))
}
