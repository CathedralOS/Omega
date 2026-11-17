//! Check the actual workspace, including nested operation crates and dependencies
//! hidden behind shared libraries. Cargo metadata includes optional and target
//! dependencies even on a machine that cannot execute the target.
use super::{Metadata, normalize_relative, production_edges, under};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;

pub(super) const STAGES: [&str; 11] = [
    "omega-rust/omega/compilation/pipeline/00_compilation-inputs",
    "omega-rust/psi/compilation/pipeline/01_source-compilation",
    "omega-rust/omega/compilation/pipeline/02_portable-publication",
    "omega-rust/omega/compilation/pipeline/03_native-program-planning",
    "omega-rust/omega/compilation/pipeline/04_physical-program-planning",
    "omega-rust/omega/compilation/pipeline/05_installation-binding",
    "omega-rust/omega/compilation/pipeline/06_native-encoding",
    "omega-rust/omega/compilation/pipeline/07_native-layout",
    "omega-rust/omega/compilation/pipeline/08_object-construction",
    "omega-rust/omega/compilation/pipeline/09_image-construction",
    "omega-rust/omega/compilation/pipeline/10_product-publication",
];
const RETIRED: [&str; 3] = [
    "omega-rust/omega/compilation/backend",
    "omega-rust/omega/compilation/realization",
    "omega-rust/omega/foundation",
];
pub(super) const TEXT_READER: &str =
    "omega-rust/psi/compilation/pipeline/01_source-compilation/src/operations/source-syntax";

pub(super) const FRONTEND: [&str; 4] = [
    "omega-rust/psi/compilation/pipeline/01_source-compilation/src/operations/lexing",
    "omega-rust/psi/compilation/pipeline/01_source-compilation/src/operations/parsing",
    "omega-rust/psi/compilation/representations/tokens",
    "omega-rust/psi/compilation/representations/syntax-trees",
];

fn stage(path: &str) -> Option<usize> {
    STAGES.iter().position(|root| under(path, &[*root]))
}

fn forbidden_frontend(start: &str, dependency: &str) -> bool {
    let [lexer, parser, tokens, syntax] = FRONTEND;
    match start {
        TEXT_READER => stage(dependency).is_some() && ![lexer, parser].contains(&dependency),
        path if path == lexer => [parser, syntax].contains(&dependency),
        path if path == parser => dependency == lexer,
        path if path == tokens || path == syntax => FRONTEND.contains(&dependency),
        _ => false,
    }
}

pub(super) fn violations(metadata: &Metadata) -> Result<Vec<String>, String> {
    let mut names = BTreeMap::new();
    for package in &metadata.packages {
        let parent = Path::new(&package.manifest_path)
            .parent()
            .ok_or("manifest has no parent")?;
        names.insert(
            normalize_relative(Path::new(&metadata.workspace_root), parent)?,
            &package.name,
        );
    }
    let mut adjacency = BTreeMap::<String, Vec<String>>::new();
    for (_, owner, dependency) in production_edges(metadata)? {
        adjacency.entry(owner).or_default().push(dependency);
    }
    let mut errors = Vec::new();
    for root in STAGES {
        if !names.keys().any(|path| under(path, &[root])) {
            errors.push(format!("numbered pipeline owner has no package: {root}"));
        }
    }
    for root in FRONTEND.into_iter().chain([TEXT_READER]) {
        if !names.contains_key(root) {
            errors.push(format!(
                "streaming frontend boundary has no package: {root}"
            ));
        }
    }
    for start in names.keys() {
        let phase = stage(start);
        if under(start, &RETIRED)
            || (under(start, &[super::PSI_PIPELINE, super::OMEGA_PIPELINE]) && phase.is_none())
        {
            errors.push(format!(
                "pipeline package is outside its declared numbered owner: {start}"
            ));
        }
        let runtime =
            [super::RUNTIME_ROOT, super::NATIVE_PROVIDER_RUNTIME_ROOT].contains(&start.as_str());
        if phase.is_none() && !runtime && !FRONTEND.contains(&start.as_str()) {
            continue;
        }
        let mut visited = BTreeSet::from([start.as_str()]);
        let mut queue = VecDeque::from([(start.as_str(), names[start].to_string())]);
        while let Some((here, chain)) = queue.pop_front() {
            for next in adjacency.get(here).into_iter().flatten() {
                if !visited.insert(next) {
                    continue;
                }
                let name = names
                    .get(next)
                    .ok_or_else(|| format!("undeclared workspace dependency: {next}"))?;
                let chain = format!("{chain} -> {name}");
                let later =
                    stage(next).is_some_and(|number| phase.is_some_and(|start| number > start));
                if later || (runtime && stage(next).is_some()) || forbidden_frontend(start, next) {
                    errors.push(format!("forbidden production dependency: {chain} ({next})"));
                } else {
                    queue.push_back((next.as_str(), chain));
                }
            }
        }
    }
    Ok(errors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boundary::{Dependency, Package};

    #[test]
    fn nested_stages_and_streaming_reject_indirect_production_edges() {
        const SHARED: &str = "shared";
        const ENCODER: &str =
            "omega-rust/omega/compilation/pipeline/06_native-encoding/test-encoder";
        let paths = STAGES
            .into_iter()
            .chain(FRONTEND)
            .chain([TEXT_READER, SHARED, ENCODER]);
        let mut packages: Vec<_> = paths
            .map(|path| Package {
                name: path.rsplit('/').next().unwrap().to_string(),
                manifest_path: format!("/repo/{path}/Cargo.toml"),
                dependencies: Vec::new(),
            })
            .collect();
        let mut edge = |from: &str, to: &str, kind: Option<&str>| {
            let manifest = format!("/repo/{from}/Cargo.toml");
            let owner = packages
                .iter_mut()
                .find(|pkg| pkg.manifest_path == manifest)
                .unwrap();
            owner.dependencies.push(Dependency {
                name: "alias".to_owned(),
                path: Some(format!("/repo/{to}")),
                kind: kind.map(str::to_owned),
            });
        };
        edge(TEXT_READER, SHARED, None);
        edge(STAGES[4], SHARED, None);
        edge(FRONTEND[0], FRONTEND[1], Some("dev"));
        edge(FRONTEND[1], SHARED, Some("build"));
        edge(SHARED, ENCODER, None);
        edge(SHARED, FRONTEND[0], None);
        edge(SHARED, STAGES[1], None);
        let errors = violations(&Metadata {
            workspace_root: "/repo".into(),
            packages,
        })
        .unwrap();
        assert!(
            errors
                .iter()
                .any(|v| v.contains("04_physical-program-planning -> shared -> test-encoder"))
        );
        assert!(
            errors
                .iter()
                .any(|v| v.contains("parsing -> shared -> lexing"))
        );
        assert!(!errors.iter().any(|v| v.contains("lexing -> parsing")));
        assert!(
            errors
                .iter()
                .any(|v| v.contains("source-syntax -> shared -> 01_source-compilation"))
        );
        assert!(
            errors
                .iter()
                .any(|v| v.contains("source-syntax -> shared -> test-encoder"))
        );
        assert!(
            !errors
                .iter()
                .any(|v| v.contains("source-syntax -> shared -> lexing"))
        );
    }
}
