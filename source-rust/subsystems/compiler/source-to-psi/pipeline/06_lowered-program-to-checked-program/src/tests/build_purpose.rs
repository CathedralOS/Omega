//! A package's Build sources and Product sources are two programs. The
//! request says which one a compilation judges; these read both from one
//! typed frontier, as a Build activation and its Product do.

use super::*;
use source_provenance::DependencyScope;
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
fn frontier() -> crate::source_state::TypedTrees {
    frontier_of(PRODUCT, BUILD)
}

/// The same, for any pair of Product and Build texts.
pub(super) fn frontier_of(product: &str, build: &str) -> crate::source_state::TypedTrees {
    frontier_with_core(product, build, DependencyScope::Product, &[])
}

/// Load the exact bundled declarations a focused source program imports.
/// Their real toolchain origin and package-relative paths retain recognition;
/// inserting a local declaration with the same spelling would not. Each
/// declaration is loaded under the program purpose of its actual importer.
pub(super) fn frontier_with_core(
    product: &str,
    build: &str,
    core_scope: DependencyScope,
    core: &[(&str, &str, &str)],
) -> crate::source_state::TypedTrees {
    let mut sources = crate::source_inputs::SourceMap::default();
    let mut syntax = None;
    let mut product_source = None;
    let mut build_source = None;
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
                source_admission::SourceOrigin::User,
                source_admission::SourceResolutionStratum::Base,
                scope,
            )
            .source_id;
        if scope == DependencyScope::Product {
            product_source = Some(id);
        } else {
            build_source = Some(id);
        }
        let tokens = crate::checking::frontend_fixture::Lexer::new(text)
            .tokenize()
            .unwrap();
        match &mut syntax {
            None => {
                syntax = Some(
                    crate::checking::frontend_fixture::parse_syntax_trees_with_id(id, &tokens)
                        .unwrap(),
                );
            }
            Some(syntax) => {
                crate::checking::frontend_fixture::parse(syntax, id, &tokens).unwrap();
            }
        }
    }
    let mut syntax = syntax.unwrap();
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../../../source/library/core");
    let mut bindings = Vec::with_capacity(core.len());
    let importing_source = match core_scope {
        DependencyScope::Product => product_source.unwrap(),
        DependencyScope::Build => build_source.unwrap(),
    };
    for &(file, declaration, text) in core {
        let id = sources
            .add_checked_instance(
                root.join(file),
                text,
                root.clone(),
                None,
                source_admission::SourceOrigin::Toolchain,
                source_admission::SourceResolutionStratum::Base,
                core_scope,
            )
            .source_id;
        let tokens = crate::checking::frontend_fixture::Lexer::new(text)
            .tokenize()
            .unwrap();
        crate::checking::frontend_fixture::parse(&mut syntax, id, &tokens).unwrap();
        bindings.push(
            crate::source_symbols::SourceScopedTopLevelBinding::logical_module_import(
                importing_source,
                id,
                &format!("omega::language::core::{declaration}"),
                3,
            ),
        );
    }
    let mut request = crate::resolution::ResolutionRequest::new(syntax);
    request.sources = Some(Arc::new(sources));
    request.top_level_bindings = bindings;
    let resolved = crate::resolution::resolve(request).unwrap();
    crate::typing::lower_symbol_resolved_trees(&resolved).unwrap()
}

#[test]
fn bundled_core_imports_keep_the_callers_checked_purpose() {
    let core = &[(
        "byte_observation.omg",
        "byte_observation",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../../source/library/core/byte_observation/byte_observation.omg"
        )),
    )];
    let imports = "use omega::language::core::byte_observation; machine importer() {}";
    let empty = "machine other() {}";
    for scope in [DependencyScope::Product, DependencyScope::Build] {
        let (product, build) = match scope {
            DependencyScope::Product => (imports, empty),
            DependencyScope::Build => (empty, imports),
        };
        let typed = frontier_with_core(product, build, scope, core);
        let sources = typed.symbols.source_map().unwrap();
        let declaration = sources
            .files()
            .find(|file| file.origin.is_toolchain())
            .unwrap();
        assert_eq!(declaration.dependency_scope, scope);
        let declaration_span = declaration.source_span(source_text::Span::new(0, 1));
        for caller in sources.files().filter(|file| !file.origin.is_toolchain()) {
            assert_eq!(
                sources.reference_can_see_declaration(
                    caller.source_span(source_text::Span::new(0, 1)),
                    declaration_span
                ),
                caller.dependency_scope == scope,
                "Core source cannot cross a checked program purpose",
            );
        }
    }
}

pub(super) fn symbol(typed: &crate::source_state::TypedTrees, name: &str) -> symbols::SymbolHandle {
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
) -> crate::lowering::Compilation<'a> {
    crate::lowering::Compilation {
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
fn claimed(
    typed: &crate::source_state::TypedTrees,
    entry: &str,
    purpose: DependencyScope,
) -> Vec<String> {
    let elaboration = crate::lowering::elaborate_for(
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
/// `build_entry`, beside an empty Product. Acceptance covers every subject
/// declaration; only then is its closure returned for table inspection.
pub(super) fn judge_build(
    build: &str,
) -> Result<RouteVerdict, crate::checking::CompilationRefusal> {
    judge_build_with_core(build, &[])
}

pub(super) fn judge_build_with_core(
    build: &str,
    core: &[(&str, &str, &str)],
) -> Result<RouteVerdict, crate::checking::CompilationRefusal> {
    let typed = frontier_with_core(
        "machine product_entry() { }",
        build,
        DependencyScope::Build,
        core,
    );
    let entry = symbol(&typed, "build_entry");
    let verdicts = crate::checking::judge_compilation(
        crate::lowering::prepare_compilation(
            &typed,
            roster(&[entry], &[Some(package())], DependencyScope::Build),
            terminal_codec::TerminalPublication::Invocation,
        )
        .expect("fixture must lower to checker input")
        .program,
    );
    super::entryless::whole_subject_result(verdicts)
}

#[test]
fn a_build_subject_cannot_hide_a_generic_guarantee_rejection() {
    let source = "machine build_entry() {}\n\
                  machine unchanged<T>(value: u32) -> u32 \
                  ensures result == value crashes none { value }\n";
    let accepted = judge_build(source);
    assert!(
        matches!(accepted, Ok(RouteVerdict::Accepted(_))),
        "{accepted:?}"
    );

    let negative = source.replace("result == value", "result != value");
    let parts = judge_build_all(&negative);
    assert!(
        matches!(parts.first(), Some(Ok(RouteVerdict::Accepted(_)))),
        "{parts:?}"
    );
    let rejected = judge_build(&negative);
    assert!(
        matches!(rejected, Ok(RouteVerdict::Rejected { ref unproved, .. })
            if unproved.iter().any(|row| matches!(row.kind, ObligationKind::NormalGuarantee { clause: 0, .. })
                && row.cause == UnprovedCause::NoDerivation)),
        "the uncalled generic declaration still owes its guarantee: {rejected:?}"
    );
}

/// Every verdict of the Build program `build`: its closure, then each
/// generic check form and refinement witness it declares.
pub(super) fn judge_build_all(
    build: &str,
) -> Vec<Result<RouteVerdict, crate::checking::CompilationRefusal>> {
    let typed = frontier_of("machine product_entry() { }", build);
    let entry = symbol(&typed, "build_entry");
    crate::checking::judge_compilation(
        crate::lowering::prepare_compilation(
            &typed,
            roster(&[entry], &[Some(package())], DependencyScope::Build),
            terminal_codec::TerminalPublication::Invocation,
        )
        .expect("fixture must lower to checker input")
        .program,
    )
}

#[test]
fn the_checker_judges_a_build_program_as_an_invocation() {
    let typed = frontier();
    let entry = symbol(&typed, "build_entry");
    let mut verdicts = crate::checking::judge_compilation(
        crate::lowering::prepare_compilation(
            &typed,
            roster(&[entry], &[Some(package())], DependencyScope::Build),
            terminal_codec::TerminalPublication::Invocation,
        )
        .expect("fixture must lower to checker input")
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
    let refusal = crate::lowering::elaborate_for(
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
    crate::lowering::elaborate_for(
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
