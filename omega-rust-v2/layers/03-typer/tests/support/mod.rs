use omega_lexer::lex;
use omega_parser::parse_file;
use omega_resolver::{ParsedSource, StageSource, resolve_stage};
use omega_typer::{TypeError, TypedStage, type_stage};

pub fn check(files: &[(&str, &str)]) -> Result<TypedStage, TypeError> {
    let mut parsed = Vec::new();
    for &(path, text) in files {
        let tokens = lex(text.as_bytes()).unwrap();
        let tree = parse_file(text.as_bytes(), &tokens).unwrap();
        parsed.push((path, text, tokens, tree));
    }
    let sources: Vec<_> = parsed
        .iter()
        .map(|(path, text, tokens, tree)| StageSource {
            package: "game",
            source: ParsedSource {
                logical_path: path,
                text,
                tokens,
                tree,
            },
        })
        .collect();
    let resolved = resolve_stage(&sources, &[]).unwrap();
    type_stage(&sources, &resolved)
}
