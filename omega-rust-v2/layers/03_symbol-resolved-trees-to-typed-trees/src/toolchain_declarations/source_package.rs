//! Source packages import the standard library through its typed interface.
use super::core_interface::core_interface;
use crate::typing::type_stage::type_admitted_stage;
use crate::{RankingView, TypeError, TypingEnvironment, ValueType};
use omega_lexer::SourceSpan;
use omega_resolver::{Dependency, NameError, StageSource, SymbolResolvedTrees};

pub(crate) const TOOLCHAIN_PACKAGE: &str = "@omega/toolchain";

pub fn resolve_source_package<'text>(sources: &[StageSource<'text, '_>]) -> Result<SymbolResolvedTrees<'text>, NameError> {
    if let Some(source) = sources.iter().find(|s| s.package == TOOLCHAIN_PACKAGE) {
        return Err(NameError {
            package: source.package.into(),
            file: source.source.logical_path.into(),
            span: SourceSpan::new(0, 0),
            message: "package identity reserved for the toolchain".into(),
        });
    }
    let dependencies: Vec<_> = sources
        .iter()
        .map(|s| s.package)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .map(|requester| Dependency {
            requester,
            alias: "omega",
            package: TOOLCHAIN_PACKAGE,
        })
        .collect();
    omega_resolver::resolve_stage_with_exports(sources, &dependencies, &Default::default(), &core_interface().exports)
}
pub fn type_source_package<'text>(resolved: &SymbolResolvedTrees<'text>) -> Result<crate::TypedTrees<'text>, TypeError> {
    let core = core_interface();
    let admitted: std::collections::HashSet<_> = resolved
        .declarations
        .iter()
        .filter(|id| id.package == TOOLCHAIN_PACKAGE)
        .collect();
    for export in &core.exports {
        if !admitted.contains(&export.id) {
            return Err(TypeError {
                package: TOOLCHAIN_PACKAGE.into(),
                file: export.id.path.clone(),
                span: SourceSpan::new(0, 0),
                message: format!("missing admitted core declaration {}", export.id.path),
            });
        }
    }
    let natural = ValueType::integer("u64").unwrap();
    let views = [
        RankingView {
            declaration: core.id("Nat::Descending").clone(),
            subject: natural.clone(),
            parameters: vec![],
            rank: natural.clone(),
            canonical_default: true,
        },
        RankingView {
            declaration: core.id("Nat::BoundedDistance").clone(),
            subject: ValueType::Tuple(vec![natural.clone(), natural.clone()]),
            parameters: vec![],
            rank: natural,
            canonical_default: false,
        },
    ];
    type_admitted_stage(
        resolved,
        true,
        &TypingEnvironment {
            ranking_views: &views,
            access_plan_requirement: Some(core.id("AccessPolicy::plan")),
            ..Default::default()
        },
    )
}
