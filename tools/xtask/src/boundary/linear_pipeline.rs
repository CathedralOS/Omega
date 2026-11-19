//! Check the actual workspace, including nested operation crates and dependencies
//! hidden behind shared libraries. Cargo metadata includes optional and target
//! dependencies even on a machine that cannot execute the target.
use super::{Metadata, native_stages, normalize_relative, production_edges, under};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;

pub(super) const TEST_SUPPORT: &str = "omega-rust/test-support";

pub(super) const STAGES: [&str; 20] = [
    "source-rust/subsystems/compiler/psi-to-native/pipeline/00_psi-to-admitted-native-inputs",
    "source-rust/subsystems/compiler/source-to-psi",
    "source-rust/subsystems/compiler/build/package-compilation/psi-publication",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/01_admitted-native-inputs-to-target-bound-program",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/02_target-bound-program-to-abstract-operations",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/03_abstract-operations-to-abstract-operations",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/04_abstract-operations-to-target-operations",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/05_target-operations-to-selected-instructions",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/06_selected-instructions-to-selected-instructions",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/07_selected-instructions-to-register-homes",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/08_register-homes-to-post-allocation-machine",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/09_post-allocation-machine-to-framed-machine",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/10_framed-machine-to-bound-framed-machine",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/11_bound-framed-machine-to-selected-form-encoding",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/12_selected-form-encoding-to-resolved-layout",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/13_resolved-layout-to-resolved-layout",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/14_resolved-layout-to-placed-functions",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/15_placed-functions-to-objects",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/16_objects-to-image",
    "source-rust/subsystems/compiler/build/build-output/product-publication",
];
const RETIRED: [&str; 4] = [
    "omega-rust/omega/compilation/pipeline/07_native-layout",
    "omega-rust/omega/compilation/backend",
    "omega-rust/omega/compilation/realization",
    "omega-rust/omega/foundation",
];
pub(super) const TEXT_READER: &str =
    "source-rust/subsystems/compiler/source-to-psi/pipeline/01_tokens-to-syntax-trees/source-syntax";

pub(super) const FRONTEND: [&str; 4] = [
    "source-rust/subsystems/compiler/source-to-psi/pipeline/00_source-files-to-tokens",
    "source-rust/subsystems/compiler/source-to-psi/pipeline/01_tokens-to-syntax-trees",
    "source-rust/subsystems/compiler/source-to-psi/pipeline/representations/tokens",
    "source-rust/subsystems/compiler/source-to-psi/pipeline/representations/syntax-trees",
];

pub(super) const PSI_STAGES: [&str; 8] = [
    FRONTEND[0],
    FRONTEND[1],
    "source-rust/subsystems/compiler/source-to-psi/pipeline/02_syntax-trees-to-symbol-resolved-trees",
    "source-rust/subsystems/compiler/source-to-psi/pipeline/03_symbol-resolved-trees-to-typed-trees",
    "source-rust/subsystems/compiler/source-to-psi/pipeline/04_typed-trees-to-typed-trees",
    "source-rust/subsystems/compiler/source-to-psi/pipeline/05_typed-trees-to-lowered-program",
    "source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program",
    "source-rust/subsystems/compiler/source-to-psi/pipeline/07_checked-program-to-psi",
];

fn stage(path: &str) -> Option<(usize, usize)> {
    if let Some(index) = PSI_STAGES.iter().position(|root| under(path, &[*root])) {
        return Some((1, index));
    }
    STAGES
        .iter()
        .position(|root| path == *root || (*root != STAGES[1] && under(path, &[*root])))
        .map(|position| (position, usize::MAX))
}

/// Native admission precedes the two source/publication slots in the combined order.
fn native_number((phase, _): (usize, usize)) -> Option<usize> {
    match phase {
        0 => Some(0),
        3..=18 => Some(phase - 2),
        _ => None,
    }
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
                        "source-rust/subsystems/compiler/source-to-psi/pipeline/representations/symbol-resolved-trees/declarations",
                        "source-rust/subsystems/compiler/source-to-psi/pipeline/representations/symbol-resolved-trees",
                        "source-rust/subsystems/compiler/source-to-psi/pipeline/representations/typed-trees",
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
    let readers = native_stages::scoped_readers();
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
                let name = names
                    .get(next)
                    .ok_or_else(|| format!("undeclared workspace dependency: {next}"))?;
                let chain = format!("{chain} -> {name}");
                let dependency_phase = stage(next);
                let later = dependency_phase
                    .is_some_and(|number| phase.is_some_and(|start| number > start));
                // Follow shared dependencies until the next stage API. Each numbered
                // owner is checked separately, so its own predecessor starts a new walk.
                let native = phase.and_then(native_number);
                let native_pair = native.zip(dependency_phase.and_then(native_number));
                // Follow the actual importing edge. Another helper that reaches the
                // same destination must still justify its own access.
                let scoped_reader = readers
                    .iter()
                    .any(|edge| edge.owner == here && edge.dependency == next);
                let enters_coordinator = native.is_some()
                    && [native_stages::DRIVER, native_stages::OBJECT_REPLAY]
                        .contains(&next.as_str());
                if enters_coordinator
                    || (runtime && dependency_phase.is_some())
                    || forbidden_frontend(start, next)
                    || (!scoped_reader
                        && (later
                            || native_pair
                                .is_some_and(|(owner, dependency)| dependency + 1 < owner)))
                {
                    errors.push(format!("forbidden production dependency: {chain} ({next})"));
                } else if !scoped_reader
                    && !native_pair.is_some_and(|(owner, dependency)| dependency + 1 == owner)
                    && visited.insert(next)
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
    use crate::boundary::tests::pkg;
    use crate::boundary::{Dependency, Package};

    #[test]
    fn a_shared_helper_cannot_reenter_a_driver_or_borrow_another_readers_permission() {
        let allowed = native_stages::scoped_readers()
            .into_iter()
            .find(|edge| edge.owner.ends_with("/verified-optimization-input"))
            .unwrap();
        let driver = native_stages::DRIVER;
        let replay = native_stages::OBJECT_REPLAY;
        let metadata = Metadata {
            workspace_root: "/repo".into(),
            packages: vec![
                pkg(
                    "optimizer",
                    STAGES[5],
                    &[("checked", allowed.owner, None), ("helper", "other", None)],
                ),
                pkg(
                    "checked",
                    allowed.owner,
                    &[("entry", allowed.dependency, None)],
                ),
                pkg(
                    "helper",
                    "other",
                    &[
                        ("entry", allowed.dependency, None),
                        ("driver", driver, None),
                        ("replay", replay, None),
                    ],
                ),
                pkg("entry", allowed.dependency, &[]),
                pkg("driver", driver, &[]),
                pkg("replay", replay, &[]),
            ],
        };
        let errors = violations(&metadata).unwrap();
        assert!(
            !errors
                .iter()
                .any(|message| message.contains("optimizer -> checked -> entry"))
        );
        for destination in ["entry", "driver", "replay"] {
            assert!(
                errors
                    .iter()
                    .any(|message| message
                        .contains(&format!("optimizer -> helper -> {destination}")))
            );
        }
    }

    #[test]
    fn nested_stages_and_streaming_reject_indirect_production_edges() {
        const SHARED: &str = "shared";
        const ENCODER: &str = "source-rust/subsystems/compiler/psi-to-native/pipeline/11_bound-framed-machine-to-selected-form-encoding/test-encoder";
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
            v.contains("02_target-bound-program-to-abstract-operations -> shared -> test-encoder")
        }));
        assert!(errors.iter().any(|v| {
            v.contains("01_tokens-to-syntax-trees -> shared -> 00_source-files-to-tokens")
        }));
        assert!(errors.iter().any(|error| error.contains(
            "06_selected-instructions-to-selected-instructions -> 08_register-homes-to-post-allocation-machine"
        )));
        assert!(errors.iter().any(|error| error.contains(
            "06_lowered-program-to-checked-program -> shared -> 00_source-files-to-tokens"
        )));
        assert!(errors.iter().any(|v| v.contains(
            "07_selected-instructions-to-register-homes -> shared -> 01_admitted-native-inputs-to-target-bound-program"
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
                .any(|v| v.contains("source-syntax -> shared -> source-to-psi"))
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
