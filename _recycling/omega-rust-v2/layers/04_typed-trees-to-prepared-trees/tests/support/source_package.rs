use omega_resolver::{ParsedSource, StageSource};
use omega_typer::{SourcePackage, resolve_source_package};

pub(super) fn source_package<'text, const N: usize>(files: [(&'text str, &'text str); N]) -> SourcePackage<'text> {
    let parsed = files.map(|(path, source)| {
        let tokens = omega_lexer::lex(source.as_bytes()).unwrap();
        let tree = omega_parser::parse_file(source.as_bytes(), &tokens).unwrap();
        (path, source, tokens, tree)
    });
    let sources = parsed.each_ref().map(|(path, text, tokens, tree)| StageSource {
        package: "app",
        source: ParsedSource {
            logical_path: path,
            text,
            tokens,
            tree,
        },
    });
    resolve_source_package(&sources).unwrap()
}
