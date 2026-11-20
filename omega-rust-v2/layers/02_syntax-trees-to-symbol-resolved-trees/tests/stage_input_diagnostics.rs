//! Stage-admission refusals observed at L2's public entry points. Each malformed
//! input sits beside a well-formed control that the same entry points accept.

#[path = "support/diagnostic_kind.rs"]
mod diagnostic_kind;

use omega_lexer::{SourceSpan, lex};
use omega_parser::parse_file;
use omega_resolver::{
    AssemblyVocabulary, CompiledInterface, Dependency, NameError, NameErrorKind, NameErrorOrigin, ParsedSource, StageSource, Symbol, resolve_stage,
    resolve_stage_for_target, resolve_stage_with_exports,
};

/// Stage-input refusals describe a whole file or edge rather than a source range.
fn whole_file() -> SourceSpan {
    SourceSpan::new(0, 0)
}

fn with_sources(
    files: &[(&str, &str, &str)],
    check: impl FnOnce(&[StageSource<'_, '_>]),
) {
    let parses: Vec<_> = files
        .iter()
        .map(|&(_, _, text)| {
            let tokens = lex(text.as_bytes()).unwrap();
            let tree = parse_file(text.as_bytes(), &tokens).unwrap();
            (tokens, tree)
        })
        .collect();
    let sources: Vec<_> = files
        .iter()
        .zip(&parses)
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
    check(&sources);
}

fn copy<'a, 'p>(input: &StageSource<'a, 'p>) -> StageSource<'a, 'p> {
    StageSource {
        package: input.package,
        source: ParsedSource {
            logical_path: input.source.logical_path,
            text: input.source.text,
            tokens: input.source.tokens,
            tree: input.source.tree,
        },
    }
}

fn reversed<'a, 'p>(sources: &[StageSource<'a, 'p>]) -> Vec<StageSource<'a, 'p>> {
    sources.iter().rev().map(copy).collect()
}

/// Every public entry point and both input orders accept the control.
fn accept(
    sources: &[StageSource<'_, '_>],
    dependencies: &[Dependency<'_>],
) {
    let assembly = AssemblyVocabulary::default();
    let interface = CompiledInterface::default();
    let backward_sources = reversed(sources);
    let backward_dependencies: Vec<_> = dependencies.iter().rev().copied().collect();
    for result in [
        resolve_stage(sources, dependencies),
        resolve_stage_for_target(sources, dependencies, &assembly),
        resolve_stage_with_exports(sources, dependencies, &assembly, &interface),
        resolve_stage(&backward_sources, &backward_dependencies),
    ] {
        let stage = result.unwrap_or_else(|error| panic!("control was refused: {error:?}"));
        assert_eq!(stage.trees().len(), sources.len());
    }
}

/// Every public entry point and both input orders return the same stage-input refusal.
fn refuse(
    sources: &[StageSource<'_, '_>],
    dependencies: &[Dependency<'_>],
) -> NameError {
    let assembly = AssemblyVocabulary::default();
    let interface = CompiledInterface::default();
    let error = resolve_stage(sources, dependencies).unwrap_err();
    assert_eq!(resolve_stage_for_target(sources, dependencies, &assembly).unwrap_err(), error);
    assert_eq!(resolve_stage_with_exports(sources, dependencies, &assembly, &interface).unwrap_err(), error);
    let backward_dependencies: Vec<_> = dependencies.iter().rev().copied().collect();
    assert_eq!(resolve_stage(&reversed(sources), &backward_dependencies).unwrap_err(), error);
    assert_eq!(error.kind.origin(), NameErrorOrigin::StageInput, "{error:?}");
    error
}

#[test]
fn parsed_sources_whose_tokens_or_text_do_not_index_the_tree_are_refused() {
    let files = [("app", "combat/main.omg", "machine main() { score() } machine score() {}"), ("app", "util.omg", "machine helper() {}")];
    with_sources(&files, |sources| {
        accept(sources, &[]);
        let missing_tokens = StageSource {
            package: sources[0].package,
            source: ParsedSource {
                tokens: &[],
                ..sources[0].source
            },
        };
        let truncated_tokens = StageSource {
            package: sources[0].package,
            source: ParsedSource {
                tokens: &sources[0].source.tokens[..1],
                ..sources[0].source
            },
        };
        let missing_text = StageSource {
            package: sources[0].package,
            source: ParsedSource {
                text: "",
                ..sources[0].source
            },
        };
        for inconsistent in [missing_tokens, truncated_tokens, missing_text] {
            let stage = [inconsistent, copy(&sources[1])];
            let error = refuse(&stage, &[]);
            diagnostic_kind::assert_refusal(
                &error,
                NameErrorKind::InconsistentParsedSource,
                "app",
                "combat/main.omg",
                whole_file(),
                "inconsistent parsed source",
            );
        }
    });
}

#[test]
fn a_stale_parse_that_omits_a_meaningful_suffix_is_refused() {
    let files = [
        ("app", "combat/main.omg", "machine main() {}"),
        ("app", "combat/main.omg", "machine main() {} machine helper() {}"),
        ("app", "combat/main.omg", "machine main() {} // trailing trivia"),
    ];
    with_sources(&files, |sources| {
        let complete = &sources[1..2];
        accept(complete, &[]);
        let stage = resolve_stage(complete, &[]).unwrap();
        let tree = stage.trees().next().unwrap();
        assert!(
            tree.definitions()
                .any(|(span, id)| { id.package == "app" && id.path == "combat\\helper" && span == SourceSpan::new(18, 37) })
        );

        let prefix_tree_with = |i: usize| StageSource {
            package: sources[i].package,
            source: ParsedSource {
                tree: sources[0].source.tree,
                ..sources[i].source
            },
        };
        // Reusing the same prefix tree is harmless when no meaningful token was added.
        accept(&[prefix_tree_with(2)], &[]);
        let prefix_tree_and_tokens = StageSource {
            package: sources[1].package,
            source: ParsedSource {
                tree: sources[0].source.tree,
                tokens: sources[0].source.tokens,
                ..sources[1].source
            },
        };
        for inconsistent in [prefix_tree_with(1), prefix_tree_and_tokens] {
            let error = refuse(&[inconsistent], &[]);
            diagnostic_kind::assert_refusal(
                &error,
                NameErrorKind::InconsistentParsedSource,
                "app",
                "combat/main.omg",
                whole_file(),
                "inconsistent parsed source",
            );
        }
    });
}

#[test]
fn a_package_admits_each_logical_path_once() {
    let distinct_paths = [("app", "combat/main.omg", "machine run() {}"), ("app", "combat/other.omg", "machine walk() {}")];
    with_sources(&distinct_paths, |sources| accept(sources, &[]));
    let distinct_packages = [("app", "combat/main.omg", "machine run() {}"), ("geometry", "combat/main.omg", "machine run() {}")];
    with_sources(&distinct_packages, |sources| accept(sources, &[]));

    let repeated = [
        ("geometry", "combat/main.omg", "machine run() {}"),
        ("app", "combat/main.omg", "machine run() {}"),
        ("app", "combat/main.omg", "machine run() {}"),
    ];
    let different_contents = [("app", "main.omg", "machine run() {}"), ("app", "main.omg", "machine walk() {}")];
    for (files, file) in [(&repeated[..], "combat/main.omg"), (&different_contents[..], "main.omg")] {
        with_sources(files, |sources| {
            let error = refuse(sources, &[]);
            diagnostic_kind::assert_refusal(&error, NameErrorKind::DuplicateLogicalFile, "app", file, whole_file(), "duplicate logical file");
        });
    }
}

#[test]
fn logical_paths_name_a_file_below_identifier_directories() {
    for path in ["main.omg", "combat/main.omg", "combat/ai/scoring.omg", "_private/x.omg", "combat/main", "u8.omg", "combat/a.b.omg"] {
        with_sources(&[("app", path, "machine run() {}")], |sources| accept(sources, &[]));
    }
    for path in [
        "",
        ".",
        "..",
        "combat/",
        "combat/.",
        "combat/..",
        "/main.omg",
        "combat//main.omg",
        "./main.omg",
        "../main.omg",
        "combat\\main.omg",
        "C:main.omg",
        "C:/main.omg",
        "1combat/main.omg",
        "combat-ai/main.omg",
        "u8/main.omg",
        "bool/main.omg",
        "combat/u32/main.omg",
    ] {
        with_sources(&[("app", path, "machine run() {}")], |sources| {
            let error = refuse(sources, &[]);
            diagnostic_kind::assert_refusal(
                &error,
                NameErrorKind::InvalidLogicalPath,
                "app",
                path,
                whole_file(),
                &format!("invalid package-relative logical path {path}"),
            );
        });
    }
}

#[test]
fn a_requester_maps_each_dependency_alias_to_one_package() {
    let files = [
        ("app", "main.omg", "machine run() { dep\\area() }"),
        ("geometry", "g.omg", "public machine area() { shared\\step() }"),
        ("physics", "p.omg", "public machine step() {}"),
    ];
    let edge = |requester, alias, package| Dependency { requester, alias, package };
    with_sources(&files, |sources| {
        let admitted = [edge("app", "dep", "geometry"), edge("geometry", "shared", "physics")];
        accept(sources, &admitted);
        let stage = resolve_stage(sources, &admitted).unwrap();
        for package in ["geometry", "physics"] {
            assert!(
                stage
                    .uses()
                    .any(|u| matches!(&u.target, Symbol::Declaration(id) if id.package == package)),
                "{package}"
            );
        }
        // Repeating an identical edge and reusing an alias in another requester are not conflicts.
        accept(sources, &[admitted[0], admitted[0], admitted[1], edge("geometry", "dep", "physics")]);

        let error = refuse(sources, &[admitted[0], admitted[1], edge("app", "dep", "physics")]);
        diagnostic_kind::assert_refusal(
            &error,
            NameErrorKind::ConflictingDependencyAlias,
            "app",
            "",
            whole_file(),
            "conflicting packages for dependency alias dep",
        );
        let error = refuse(sources, &[edge("geometry", "shared", "app"), admitted[0], admitted[1]]);
        diagnostic_kind::assert_refusal(
            &error,
            NameErrorKind::ConflictingDependencyAlias,
            "geometry",
            "",
            whole_file(),
            "conflicting packages for dependency alias shared",
        );
    });
}
