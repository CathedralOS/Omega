use omega_lexer::lex;
use omega_parser::parse_file;
use omega_resolver::{ParsedSource, StageSource, resolve_stage};
use omega_typer::{TypeError, TypedStage, TypingEnvironment, type_stage_in_environment};

pub fn check(files: &[(&str, &str)]) -> Result<TypedStage, TypeError> {
    check_in(files, &TypingEnvironment::default())
}

pub fn check_in(
    files: &[(&str, &str)],
    environment: &TypingEnvironment<'_>,
) -> Result<TypedStage, TypeError> {
    let files: Vec<_> = files.iter().map(|&(path, text)| ("game", path, text)).collect();
    let resolved = resolve_packages(&files, &[]);
    type_stage_in_environment(&resolved, environment).map(|trees| trees.into_facts())
}

pub fn resolve_packages<'a>(
    files: &[(&'a str, &'a str, &'a str)],
    edges: &[omega_resolver::Dependency<'_>],
) -> omega_resolver::SymbolResolvedTrees<'a> {
    let mut parsed = Vec::new();
    for &(package, path, text) in files {
        let tokens = lex(text.as_bytes()).unwrap_or_else(|error| panic!("{path}: {error:?}\n{text}"));
        let tree = parse_file(text.as_bytes(), &tokens).unwrap_or_else(|error| panic!("{path}: {error:?}\n{text}"));
        parsed.push((package, path, text, tokens, tree));
    }
    let sources: Vec<_> = parsed
        .iter()
        .map(|(package, path, text, tokens, tree)| StageSource {
            package,
            source: ParsedSource {
                logical_path: path,
                text,
                tokens,
                tree,
            },
        })
        .collect();
    resolve_stage(&sources, edges).unwrap_or_else(|error| {
        let source = files
            .iter()
            .find(|(package, path, _)| *package == error.package && *path == error.file)
            .map_or("", |(_, _, text)| *text);
        panic!("{error:?}\n{source}");
    })
}
