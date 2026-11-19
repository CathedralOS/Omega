//! Source checking admits the standard core from the toolchain's embedded
//! files. Package-local declarations can share their names without acquiring
//! the core's primitive or policy identities.
use crate::*;
use omega_resolver::{Dependency, NameError, ParsedSource};
use std::sync::OnceLock;

struct CoreFile {
    path: &'static str,
    text: &'static str,
    tokens: Vec<omega_lexer::Token>,
    tree: omega_parser::SyntaxTree,
}
fn core() -> &'static [CoreFile] {
    static FILES: OnceLock<Vec<CoreFile>> = OnceLock::new();
    FILES.get_or_init(|| {
        [
            ("language/core/reflect/schema.omg", include_str!("../core/schema.omg")),
            ("language/core/policy.omg", include_str!("../core/access_policy.omg")),
            ("language/core/access/new_plan.omg", include_str!("../core/new_plan.omg")),
            ("language/core/ranking.omg", include_str!("../core/ranking.omg")),
        ]
        .into_iter()
        .map(|(path, text)| {
            let tokens = omega_lexer::lex(text.as_bytes()).expect("invalid embedded standard tokens");
            let tree = omega_parser::parse_file(text.as_bytes(), &tokens).unwrap_or_else(|error| panic!("invalid embedded {path}: {error:?}"));
            CoreFile { path, text, tokens, tree }
        })
        .collect()
    })
}
fn inputs<'a>(source: &[StageSource<'a>]) -> Result<Vec<StageSource<'a>>, NameError> {
    if let Some(s) = source.iter().find(|u| u.package == collections::PACKAGE) {
        return Err(NameError {
            package: s.package.into(),
            file: s.source.logical_path.into(),
            span: SourceSpan::new(0, 0),
            message: "package identity reserved for the toolchain".into(),
        });
    }
    let mut files: Vec<_> = source
        .iter()
        .map(|s| StageSource {
            package: s.package,
            source: ParsedSource {
                logical_path: s.source.logical_path,
                text: s.source.text,
                tokens: s.source.tokens,
                tree: s.source.tree,
            },
        })
        .collect();
    files.extend(core().iter().map(|f| StageSource {
        package: collections::PACKAGE,
        source: ParsedSource {
            logical_path: f.path,
            text: f.text,
            tokens: &f.tokens,
            tree: &f.tree,
        },
    }));
    Ok(files)
}
pub fn resolve_source_package(sources: &[StageSource<'_>]) -> Result<ResolvedStage, NameError> {
    let files = inputs(sources)?;
    let packages = files
        .iter()
        .map(|f| f.package)
        .collect::<std::collections::BTreeSet<_>>();
    let dependencies = packages
        .into_iter()
        .map(|requester| Dependency {
            requester,
            alias: "omega",
            package: collections::PACKAGE,
        })
        .collect::<Vec<_>>();
    omega_resolver::resolve_stage(&files, &dependencies)
}
pub fn type_source_package(
    sources: &[StageSource<'_>],
    resolved: &ResolvedStage,
) -> Result<TypedStage, TypeError> {
    let files = inputs(sources).map_err(|e| TypeError {
        package: e.package,
        file: e.file,
        span: e.span,
        message: e.message,
    })?;
    let declaration = |path: &str| {
        resolved
            .declarations
            .iter()
            .find(|id| id.package == collections::PACKAGE && id.kind == NodeKind::Machine && id.path == path)
            .ok_or_else(|| TypeError {
                package: collections::PACKAGE.into(),
                file: "".into(),
                span: SourceSpan::new(0, 0),
                message: format!("missing admitted core declaration {path}"),
            })
    };
    let natural = ValueType::integer("u64").unwrap();
    let views = [
        RankingView {
            declaration: declaration("language::core::Nat::Descending")?.clone(),
            subject: natural.clone(),
            parameters: vec![],
            rank: natural.clone(),
            canonical_default: true,
        },
        RankingView {
            declaration: declaration("language::core::Nat::BoundedDistance")?.clone(),
            subject: ValueType::Tuple(vec![natural.clone(), natural.clone()]),
            parameters: vec![],
            rank: natural,
            canonical_default: false,
        },
    ];
    type_admitted(
        &files,
        resolved,
        true,
        &TypingEnvironment {
            ranking_views: &views,
            access_plan_requirement: Some(declaration("language::core::AccessPolicy::plan")?),
            ..Default::default()
        },
    )
}
