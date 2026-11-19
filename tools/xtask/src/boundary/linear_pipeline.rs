//! Check the actual workspace, including nested operation crates and dependencies
//! hidden behind shared libraries. Cargo metadata includes optional and target
//! dependencies even on a machine that cannot execute the target.
use super::{Metadata, normalize_relative, production_edges, under};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;

pub(super) const TEST_SUPPORT: &str = "omega-rust/test-support";

pub(super) const STAGES: [&str; 20] = [
    "omega-rust/omega/compilation/pipeline/01_compilation-request-to-admitted-inputs",
    "omega-rust/psi/compilation/pipeline/source-compilation",
    "omega-rust/omega/compilation/products/source-publication",
    "omega-rust/omega/compilation/pipeline/02_provider-demand-to-selected-execution",
    "omega-rust/omega/compilation/pipeline/03_terminal-psi-to-abstract-operations",
    "omega-rust/omega/compilation/pipeline/04_abstract-operations-to-abstract-operations",
    "omega-rust/omega/compilation/pipeline/05_abstract-operations-to-target-operations",
    "omega-rust/omega/compilation/pipeline/06_target-operations-to-selected-instructions",
    "omega-rust/omega/compilation/pipeline/07_selected-instructions-to-selected-instructions",
    "omega-rust/omega/compilation/pipeline/08_selected-instructions-to-register-homes",
    "omega-rust/omega/compilation/pipeline/09_register-homes-to-post-allocation-machine",
    "omega-rust/omega/compilation/pipeline/10_post-allocation-machine-to-framed-machine",
    "omega-rust/omega/compilation/pipeline/11_framed-machine-to-bound-framed-machine",
    "omega-rust/omega/compilation/pipeline/12_bound-framed-machine-to-selected-form-encoding",
    "omega-rust/omega/compilation/pipeline/13_selected-form-encoding-to-resolved-layout",
    "omega-rust/omega/compilation/pipeline/14_resolved-layout-to-resolved-layout",
    "omega-rust/omega/compilation/pipeline/15_resolved-layout-to-placed-functions",
    "omega-rust/omega/compilation/pipeline/16_placed-functions-to-objects",
    "omega-rust/omega/compilation/pipeline/17_objects-to-image",
    "omega-rust/omega/compilation/pipeline/18_finished-products-to-installed-files",
];
const RETIRED: [&str; 4] = [
    "omega-rust/omega/compilation/pipeline/07_native-layout",
    "omega-rust/omega/compilation/backend",
    "omega-rust/omega/compilation/realization",
    "omega-rust/omega/foundation",
];
pub(super) const TEXT_READER: &str =
    "omega-rust/psi/compilation/pipeline/source-compilation/src/operations/source-syntax";

pub(super) const FRONTEND: [&str; 4] = [
    "omega-rust/psi/compilation/pipeline/00_source-files-to-tokens",
    "omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees",
    "omega-rust/psi/compilation/representations/tokens",
    "omega-rust/psi/compilation/representations/syntax-trees",
];

const PSI_STAGES: [&str; 8] = [
    FRONTEND[0],
    FRONTEND[1],
    "omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees",
    "omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees",
    "omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees",
    "omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi",
    "omega-rust/psi/compilation/pipeline/06_terminal-psi-to-checked-terminal-psi",
    "omega-rust/psi/compilation/pipeline/07_checked-terminal-psi-to-published-psi",
];

fn stage(path: &str) -> Option<(usize, usize)> {
    if let Some(index) = PSI_STAGES.iter().position(|root| under(path, &[*root])) {
        return Some((1, index));
    }
    STAGES
        .iter()
        .position(|root| under(path, &[*root]))
        .map(|position| (position, usize::MAX))
}

fn forbidden_frontend(start: &str, dependency: &str) -> bool {
    let [lexer, parser, tokens, syntax] = FRONTEND;
    match start {
        TEXT_READER => stage(dependency).is_some() && ![lexer, parser].contains(&dependency),
        path if path == PSI_STAGES[6] => {
            under(dependency, &PSI_STAGES[..6])
                || under(
                    dependency,
                    &[
                        tokens,
                        syntax,
                        "omega-rust/psi/compilation/representations/source-declarations",
                        "omega-rust/psi/compilation/representations/symbol-resolved-trees",
                        "omega-rust/psi/compilation/representations/typed-trees",
                    ],
                )
        }
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
    let mut errors = Vec::new();
    // Cargo unifies requested features across workspace consumers, including
    // sample binaries and build dependencies compiled alongside the compiler.
    for package in &metadata.packages {
        for dep in &package.dependencies {
            if dep.kind.as_deref() != Some("dev")
                && dep.path.is_some()
                && dep.features.iter().any(|feature| feature == "test-support")
            {
                errors.push(format!(
                    "production dependency enables test-support: {} -> {}",
                    package.name, dep.name
                ));
            }
        }
    }
    for (package, owner, dependency) in production_edges(metadata)? {
        if under(&dependency, &[TEST_SUPPORT]) && !under(&owner, &[TEST_SUPPORT]) {
            errors.push(format!(
                "production dependency on a test fixture: {} -> {dependency}",
                package.name
            ));
        }
        adjacency.entry(owner).or_default().push(dependency);
    }
    for root in STAGES.into_iter().chain(PSI_STAGES[2..].iter().copied()) {
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
                let dependency_phase = stage(next);
                let later = dependency_phase
                    .is_some_and(|number| phase.is_some_and(|start| number > start));
                // Follow shared dependencies until the next stage API. Each numbered
                // owner is checked separately, so its own predecessor starts a new walk.
                let native_pair = phase
                    .zip(dependency_phase)
                    .filter(|((owner, _), (dependency, _))| *owner >= 3 && *dependency >= 3);
                let skips_predecessor =
                    native_pair.is_some_and(|((owner, _), (dependency, _))| dependency + 1 < owner);
                if later
                    || skips_predecessor
                    || (runtime && dependency_phase.is_some())
                    || forbidden_frontend(start, next)
                {
                    errors.push(format!("forbidden production dependency: {chain} ({next})"));
                } else if !native_pair
                    .is_some_and(|((owner, _), (dependency, _))| dependency + 1 == owner)
                {
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
        const ENCODER: &str = "omega-rust/omega/compilation/pipeline/12_bound-framed-machine-to-selected-form-encoding/test-encoder";
        const FIXTURE: &str = "omega-rust/test-support/image-fixtures";
        let paths = STAGES
            .into_iter()
            .chain(FRONTEND)
            .chain(PSI_STAGES[2..].iter().copied())
            .chain([TEXT_READER, SHARED, ENCODER, FIXTURE]);
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
                features: if to == FIXTURE {
                    vec!["test-support".to_owned()]
                } else {
                    Vec::new()
                },
            });
        };
        edge(SHARED, FIXTURE, None);
        edge(STAGES[1], FIXTURE, Some("dev"));
        edge(TEXT_READER, SHARED, None);
        edge(PSI_STAGES[6], SHARED, None);
        edge(PSI_STAGES[6], PSI_STAGES[5], Some("dev"));
        edge(STAGES[4], SHARED, None);
        edge(STAGES[9], SHARED, None);
        edge(SHARED, STAGES[3], None);
        edge(STAGES[8], STAGES[10], None);
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
        assert!(errors.iter().any(|v| {
            v.contains("03_terminal-psi-to-abstract-operations -> shared -> test-encoder")
        }));
        assert!(errors.iter().any(|v| {
            v.contains("01_tokens-to-syntax-trees -> shared -> 00_source-files-to-tokens")
        }));
        assert!(errors.iter().any(|error| error.contains(
            "07_selected-instructions-to-selected-instructions -> 09_register-homes-to-post-allocation-machine"
        )));
        assert!(errors.iter().any(|error| error.contains(
            "06_terminal-psi-to-checked-terminal-psi -> shared -> 00_source-files-to-tokens"
        )));
        assert!(errors.iter().any(|v| v.contains(
            "08_selected-instructions-to-register-homes -> shared -> 02_provider-demand-to-selected-execution"
        )));
        let fixture_errors: Vec<_> = errors
            .iter()
            .filter(|v| v.contains("production dependency on a test fixture"))
            .collect();
        assert_eq!(fixture_errors.len(), 1);
        let feature_errors: Vec<_> = errors
            .iter()
            .filter(|error| error.contains("enables test-support"))
            .collect();
        assert_eq!(feature_errors.len(), 1);
        assert!(feature_errors[0].contains("shared -> alias"));
        assert!(fixture_errors[0].contains("shared -> omega-rust/test-support/image-fixtures"));
        assert!(
            !errors
                .iter()
                .any(|v| v.contains("00_source-files-to-tokens -> 01_tokens-to-syntax-trees"))
        );
        assert!(
            errors
                .iter()
                .any(|v| v.contains("source-syntax -> shared -> source-compilation"))
        );
        assert!(
            errors
                .iter()
                .any(|v| v.contains("source-syntax -> shared -> test-encoder"))
        );
        assert!(
            !errors
                .iter()
                .any(|v| v.contains("source-syntax -> shared -> 00_source-files-to-tokens"))
        );
    }
}
