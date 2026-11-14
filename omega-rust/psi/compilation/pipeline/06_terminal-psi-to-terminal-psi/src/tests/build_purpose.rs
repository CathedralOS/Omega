//! A package's Build sources and Product sources are two programs. The
//! request says which one a compilation judges; these read both from one
//! typed frontier, as a Build activation and its Product do.

use super::*;
use source::DependencyScope;
use std::sync::Arc;

const PRODUCT: &str = r#"
machine product_entry() { }
machine product_helper(value: u32) -> u32 requires value <= 3 { value + 1 }
"#;

const BUILD: &str = r#"
machine build_entry() { }
machine build_helper(value: u32) -> u32 requires value <= 3 { value + 1 }
"#;

pub(super) fn package() -> semantic_vocabulary::PackageKeyIdentity {
    semantic_vocabulary::PackageKeyIdentity::from_digest([7; 32]).unwrap()
}

/// One package read under both dependency scopes, through the ordinary
/// lexer, parser, resolver and typer.
fn frontier() -> typed_trees::TypedTrees {
    frontier_of(PRODUCT, BUILD)
}

/// The same, for any pair of Product and Build texts.
pub(super) fn frontier_of(product: &str, build: &str) -> typed_trees::TypedTrees {
    let mut sources = source::SourceMap::default();
    let mut syntax = None;
    for (path, text, scope) in [
        ("package/main.omg", product, DependencyScope::Product),
        ("package/build.omg", build, DependencyScope::Build),
    ] {
        let id = sources
            .add_checked_instance(
                path.into(),
                text,
                "package".into(),
                Some(package()),
                source::SourceOrigin::User,
                source::SourceResolutionStratum::Base,
                scope,
            )
            .source_id;
        let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
        match &mut syntax {
            None => {
                syntax =
                    Some(tokens_to_syntax_trees::parse_syntax_trees_with_id(id, &tokens).unwrap());
            }
            Some(syntax) => {
                tokens_to_syntax_trees::parse(syntax, id, &tokens).unwrap();
            }
        }
    }
    let mut request =
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax.unwrap());
    request.sources = Some(Arc::new(sources));
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(request).unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

pub(super) fn symbol(typed: &typed_trees::TypedTrees, name: &str) -> symbols::SymbolHandle {
    typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .unwrap()
        .symbol
}

pub(super) fn roster<'a>(
    entries: &'a [symbols::SymbolHandle],
    subjects: &'a [Option<semantic_vocabulary::PackageKeyIdentity>],
    purpose: DependencyScope,
) -> typed_trees_to_terminal_psi::Compilation<'a> {
    typed_trees_to_terminal_psi::Compilation {
        declaration_metadata: &[],
        entries,
        subjects,
        purpose,
        selected: &[],
        calling: None,
        providers: None,
        bindings: &[],
        supplies: &[],
    }
}

/// The names of the machines one roster claims, in elaboration order.
fn claimed(typed: &typed_trees::TypedTrees, entry: &str, purpose: DependencyScope) -> Vec<String> {
    let elaboration = typed_trees_to_terminal_psi::elaborate_for(
        typed,
        roster(&[symbol(typed, entry)], &[Some(package())], purpose),
        terminal_codec::TerminalPublication::Invocation,
    )
    .unwrap();
    elaboration
        .machines
        .iter()
        .map(|(_, symbol)| {
            typed
                .machine_by_symbol(*symbol)
                .unwrap()
                .name
                .as_str()
                .to_owned()
        })
        .collect()
}

#[test]
fn a_compilation_claims_the_subject_declarations_of_its_own_purpose_only() {
    let typed = frontier();
    // The unused helper of each program is claimed by that program's
    // compilation and by no other: neither is called by its entry.
    assert_eq!(
        claimed(&typed, "build_entry", DependencyScope::Build),
        ["build_entry", "build_helper"]
    );
    assert_eq!(
        claimed(&typed, "product_entry", DependencyScope::Product),
        ["product_entry", "product_helper"]
    );
}

/// The checker's verdict on the Build program `build`, rooted at its
/// `build_entry`, beside an empty Product.
pub(super) fn judge_build(build: &str) -> Result<RouteVerdict, crate::CompilationRefusal> {
    let typed = frontier_of("machine product_entry() { }", build);
    let entry = symbol(&typed, "build_entry");
    let mut verdicts = crate::judge_compilation(
        typed_trees_to_terminal_psi::prepare_compilation(
            &typed,
            roster(&[entry], &[Some(package())], DependencyScope::Build),
            terminal_codec::TerminalPublication::Invocation,
        )
        .program,
    );
    assert_eq!(verdicts.len(), 1);
    verdicts.remove(0)
}

/// Every verdict of the Build program `build`: its closure, then each
/// generic check form and refinement witness it declares.
pub(super) fn judge_build_all(build: &str) -> Vec<Result<RouteVerdict, crate::CompilationRefusal>> {
    let typed = frontier_of("machine product_entry() { }", build);
    let entry = symbol(&typed, "build_entry");
    crate::judge_compilation(
        typed_trees_to_terminal_psi::prepare_compilation(
            &typed,
            roster(&[entry], &[Some(package())], DependencyScope::Build),
            terminal_codec::TerminalPublication::Invocation,
        )
        .program,
    )
}

#[test]
fn the_checker_judges_a_build_program_as_an_invocation() {
    let typed = frontier();
    let entry = symbol(&typed, "build_entry");
    let mut verdicts = crate::judge_compilation(
        typed_trees_to_terminal_psi::prepare_compilation(
            &typed,
            roster(&[entry], &[Some(package())], DependencyScope::Build),
            terminal_codec::TerminalPublication::Invocation,
        )
        .program,
    );
    assert_eq!(verdicts.len(), 1);
    let verdict = verdicts.remove(0).unwrap();
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("{verdict:?}")
    };
    assert_eq!(accepted.elaboration.machines.len(), 2);
    accepted.publish_invocation().unwrap();
}

#[test]
fn a_product_declaration_is_refused_as_a_build_entry() {
    let typed = frontier();
    let refusal = typed_trees_to_terminal_psi::elaborate_for(
        &typed,
        roster(
            &[symbol(&typed, "product_entry")],
            &[Some(package())],
            DependencyScope::Build,
        ),
        terminal_codec::TerminalPublication::Invocation,
    )
    .unwrap_err();
    assert_eq!(refusal.reason, RefusalReason::Unsupported);
    assert_eq!(
        refusal.detail,
        "declaration Build entry outside the Build source context"
    );
    // The same declaration roots its own program.
    typed_trees_to_terminal_psi::elaborate_for(
        &typed,
        roster(
            &[symbol(&typed, "product_entry")],
            &[Some(package())],
            DependencyScope::Product,
        ),
        terminal_codec::TerminalPublication::Invocation,
    )
    .unwrap();
}
