//! Check the actual workspace, including nested operation crates and dependencies
//! hidden behind shared libraries. Cargo metadata includes optional and target
//! dependencies even on a machine that cannot execute the target.
use super::{Metadata, native_stages, normalize_relative, production_edges, under};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;

fn recycled(root: &str, path: &str) -> bool {
    Path::new(path)
        .strip_prefix(root)
        .is_ok_and(|relative| relative.iter().any(|component| component == "_recycling"))
}

pub(super) const NATIVE_STAGES: [&str; 14] = [
    "source-rust/subsystems/compiler/psi-to-native/pipeline/00_psi-to-admitted-native-inputs",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/01_admitted-native-inputs-to-target-bound-program",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/02_target-bound-program-to-abstract-operations",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/04_abstract-operations-to-target-operations",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/05_target-operations-to-selected-instructions",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/06_selected-instructions-to-selected-instructions",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/07_selected-instructions-to-register-homes",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/08_register-homes-to-post-allocation-machine",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/09_post-allocation-machine-to-framed-machine",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/11_bound-framed-machine-to-selected-form-encoding",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/12_selected-form-encoding-to-resolved-layout",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/13_resolved-layout-to-placed-functions",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/14_placed-functions-to-objects",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image",
];
pub(super) const SOURCE_DRIVER: &str = "source-rust/subsystems/compiler/source-to-psi";
const SOURCE_PIPELINE: &str = "source-rust/subsystems/compiler/source-to-psi/pipeline";
const NATIVE_PIPELINE: &str = "source-rust/subsystems/compiler/psi-to-native/pipeline";
const REPRESENTATIONS: [&str; 2] = [
    "source-rust/subsystems/compiler/source-to-psi/representations",
    "source-rust/subsystems/compiler/psi-to-native/representations",
];

const RETIRED: [&str; 5] = [
    "source-rust/subsystems/compiler/psi-to-native/saved-inputs",
    "omega-rust/omega/compilation/pipeline/07_native-layout",
    "omega-rust/omega/compilation/backend",
    "omega-rust/omega/compilation/realization",
    "omega-rust/omega/foundation",
];
pub(super) const TEXT_READER: &str = "source-rust/subsystems/compiler/source-to-psi/source-syntax";

pub(super) const FRONTEND: [&str; 4] = [
    "source-rust/subsystems/compiler/source-to-psi/pipeline/00_source-files-to-tokens",
    "source-rust/subsystems/compiler/source-to-psi/pipeline/01_tokens-to-syntax-trees",
    "source-rust/subsystems/compiler/source-to-psi/representations/tokens",
    "source-rust/subsystems/compiler/source-to-psi/representations/syntax-trees",
];

pub(super) const PSI_STAGES: [&str; 8] = [
    FRONTEND[0],
    FRONTEND[1],
    "source-rust/subsystems/compiler/source-to-psi/pipeline/02_syntax-trees-to-symbol-resolved-trees",
    "source-rust/subsystems/compiler/source-to-psi/pipeline/03_symbol-resolved-trees-to-typed-trees",
    "source-rust/subsystems/compiler/source-to-psi/pipeline/04_typed-trees-to-prepared-typed-trees",
    "source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program",
    "source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program",
    "source-rust/subsystems/compiler/source-to-psi/pipeline/07_checked-program-to-psi",
];

/// A representation is the record one stage hands the next, so it may not link
/// a numbered stage, directly or through a helper: every reader of the record
/// would then compile that stage's code, and the pipeline order would no longer
/// say which stage made a decision. Move the type the representation needs into
/// `representations/` instead.
///
/// Each row is a link that existed when this check landed. Never add a row.
/// The PR that removes a link deletes its row; `stale_allowance_violations`
/// refuses a row whose link is gone, so the list only shrinks.
pub(super) const LEGACY_REPRESENTATION_STAGE_LINKS: [(&str, &str); 3] = [
    // Encoding admission calls each target encoder's selected-form validators.
    (
        "source-rust/subsystems/compiler/psi-to-native/representations/machine-code/encoding-admission",
        "source-rust/subsystems/compiler/psi-to-native/pipeline/11_bound-framed-machine-to-selected-form-encoding/isa-aarch64",
    ),
    (
        "source-rust/subsystems/compiler/psi-to-native/representations/machine-code/encoding-admission",
        "source-rust/subsystems/compiler/psi-to-native/pipeline/11_bound-framed-machine-to-selected-form-encoding/isa-x86_64",
    ),
    // Evaluation receipts name Psi publication's call-site and program records.
    (
        "source-rust/subsystems/compiler/source-to-psi/representations/evaluation-receipts",
        "source-rust/subsystems/compiler/source-to-psi/pipeline/07_checked-program-to-psi",
    ),
];

/// The legacy links whose stage lies under `stages`, as rule allowances.
pub(super) fn legacy_representation_links_into(
    stages: &[&str],
) -> impl Iterator<Item = (&'static str, &'static str)> {
    LEGACY_REPRESENTATION_STAGE_LINKS
        .into_iter()
        .filter(move |(_, stage)| under(stage, stages))
}

/// Whether `dependency` belongs to a numbered source or native stage.
fn numbered_stage(dependency: &str) -> bool {
    under(dependency, &PSI_STAGES) || under(dependency, &NATIVE_STAGES)
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Phase {
    NativeAdmission,
    Source(usize),
    Native(usize),
    ProductPublication,
}

impl Phase {
    fn native_number(self) -> Option<usize> {
        match self {
            Self::NativeAdmission => Some(0),
            Self::Native(index) => Some(index),
            _ => None,
        }
    }
}

fn stage(path: &str) -> Option<Phase> {
    if path == SOURCE_DRIVER {
        return Some(Phase::Source(usize::MAX));
    }
    if let Some(index) = PSI_STAGES.iter().position(|root| under(path, &[*root])) {
        return Some(Phase::Source(index));
    }
    if under(path, &[native_stages::PRODUCT_PUBLICATION]) {
        return Some(Phase::ProductPublication);
    }
    NATIVE_STAGES
        .iter()
        .position(|root| under(path, &[*root]))
        .map(|index| {
            if index == 0 {
                Phase::NativeAdmission
            } else {
                Phase::Native(index)
            }
        })
}

/// The crate that lets a source stage issue its product, the number of the
/// stage that produces it, and the representation whose constructor asks for
/// the capability. A product can be built only where its issuer is linked, so
/// no other package may link one, in a test either.
const SOURCE_PRODUCT_ISSUERS: [(&str, usize, &str); 3] = [
    (
        "source-rust/subsystems/compiler/source-to-psi/representations/prepared-source/issuer",
        4,
        "source-rust/subsystems/compiler/source-to-psi/representations/prepared-source",
    ),
    (
        "source-rust/subsystems/compiler/source-to-psi/representations/lowered-source-publication/issuer",
        5,
        "source-rust/subsystems/compiler/source-to-psi/representations/lowered-source-publication",
    ),
    (
        "source-rust/subsystems/compiler/source-to-psi/representations/checked-program/issuer",
        6,
        "source-rust/subsystems/compiler/source-to-psi/representations/checked-program",
    ),
];

/// Whether `owner` may link `dependency` when that is a product issuer.
fn may_issue(owner: &str, dependency: &str) -> bool {
    SOURCE_PRODUCT_ISSUERS
        .iter()
        .find(|(issuer, ..)| *issuer == dependency)
        .is_none_or(|(_, stage, representation)| {
            owner == *representation || under(owner, &[PSI_STAGES[*stage]])
        })
}

/// A source stage reads the representation an earlier stage produced. Linking
/// that stage's crate instead lets it call the code that made the decision, so
/// a wrong answer could come from either place. Move the type the stage reads
/// into `representations/`. The Build text reader is not a stage: it runs the
/// lexer and parser on purpose.
fn links_another_source_stage(start: &str, dependency: &str) -> bool {
    let number = |path: &str| PSI_STAGES.iter().position(|root| under(path, &[*root]));
    if dependency == TEXT_READER {
        return number(start).is_some();
    }
    start != TEXT_READER
        && number(start)
            .zip(number(dependency))
            .is_some_and(|(owner, linked)| owner != linked)
}

fn forbidden_frontend(start: &str, dependency: &str) -> bool {
    let [lexer, parser, tokens, syntax] = FRONTEND;
    if links_another_source_stage(start, dependency) {
        return true;
    }
    match start {
        TEXT_READER => stage(dependency).is_some() && ![lexer, parser].contains(&dependency),
        path if path == PSI_STAGES[6] => under(
            dependency,
            &[
                tokens,
                syntax,
                "source-rust/subsystems/compiler/source-to-psi/representations/symbol-resolved-trees/declarations",
                "source-rust/subsystems/compiler/source-to-psi/representations/symbol-resolved-trees",
                "source-rust/subsystems/compiler/source-to-psi/representations/typed-trees",
            ],
        ),
        path if path == lexer => dependency == syntax,
        path if path == tokens || path == syntax => FRONTEND.contains(&dependency),
        _ => false,
    }
}

/// A module included from the driver need not be a Cargo package. Check the
/// physical directories as well as Cargo metadata to catch both kinds of additions.
pub(super) fn layout_violations(root: &Path) -> Result<Vec<String>, String> {
    let mut errors = Vec::new();
    for (pipeline, stages) in [
        (SOURCE_PIPELINE, &PSI_STAGES[..]),
        (NATIVE_PIPELINE, &NATIVE_STAGES[..]),
    ] {
        let mut missing = stages.iter().copied().collect::<BTreeSet<_>>();
        let entries =
            std::fs::read_dir(root.join(pipeline)).map_err(|e| format!("{pipeline}: {e}"))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("{pipeline}: {e}"))?;
            let kind = entry.file_type().map_err(|e| format!("{pipeline}: {e}"))?;
            let relative = format!("{pipeline}/{}", entry.file_name().to_string_lossy());
            if kind.is_dir() && missing.remove(relative.as_str()) {
                continue;
            }
            if !(kind.is_file() && entry.file_name() == "README.md") {
                errors.push(format!("{relative}: only the numbered transformation directories and README.md belong in the pipeline"));
            }
        }
        errors.extend(
            missing
                .into_iter()
                .map(|path| format!("{path}: numbered transformation directory is missing")),
        );
    }
    errors.sort();
    Ok(errors)
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
        if recycled(&metadata.workspace_root, &package.manifest_path) {
            errors.push(format!(
                "workspace member is under _recycling: {}",
                package.name
            ));
        }
        let root = Path::new(&metadata.workspace_root);
        let owner = Path::new(&package.manifest_path)
            .parent()
            .ok_or("manifest has no parent")?;
        let owner = normalize_relative(root, owner)?;
        for dep in &package.dependencies {
            // Test dependencies count: a test that links an issuer could build
            // a product no stage produced.
            if let Some(path) = dep.path.as_deref()
                && normalize_relative(root, Path::new(path))
                    .is_ok_and(|dependency| !may_issue(&owner, &dependency))
            {
                errors.push(format!(
                    "{} -> {}: only the producing source stage may issue this product",
                    package.name, dep.name
                ));
            }
            if dep
                .path
                .as_deref()
                .is_some_and(|path| recycled(&metadata.workspace_root, path))
            {
                errors.push(format!(
                    "{} -> {}: Cargo dependency points into _recycling",
                    package.name, dep.name
                ));
            }
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
    for (_, owner, dependency) in production_edges(metadata)? {
        adjacency.entry(owner).or_default().push(dependency);
    }
    for root in NATIVE_STAGES
        .into_iter()
        .chain([SOURCE_DRIVER, native_stages::PRODUCT_PUBLICATION])
        .chain(PSI_STAGES[2..].iter().copied())
    {
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
            || (under(start, &[SOURCE_PIPELINE, NATIVE_PIPELINE]) && phase.is_none())
        {
            errors.push(format!(
                "pipeline package is outside its declared numbered owner: {start}"
            ));
        }
        let runtime = [
            super::RUNTIME_ROOT,
            super::NATIVE_PROVIDER_RUNTIME_ROOT,
            super::RETAINED_IMAGE_RUNTIME,
        ]
        .contains(&start.as_str());
        let representation = under(start, &REPRESENTATIONS);
        if phase.is_none()
            && !runtime
            && !representation
            && !FRONTEND.contains(&start.as_str())
            && start != TEXT_READER
        {
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
                // A representation's walk ends at the first numbered stage it
                // reaches. Only the representation's own legacy link may name one.
                if representation && numbered_stage(next) {
                    let legacy = here == start
                        && LEGACY_REPRESENTATION_STAGE_LINKS
                            .contains(&(start.as_str(), next.as_str()));
                    if !legacy {
                        errors.push(format!(
                            "representation links a numbered stage: {chain} ({next}); move the type it needs into representations/"
                        ));
                    }
                    continue;
                }
                let dependency_phase = stage(next);
                let later = dependency_phase
                    .is_some_and(|number| phase.is_some_and(|start| number > start));
                // Follow shared dependencies until the next stage API. Each numbered
                // owner is checked separately, so its own predecessor starts a new walk.
                let native = phase.and_then(Phase::native_number);
                let native_pair = native.zip(dependency_phase.and_then(Phase::native_number));
                // Follow the actual importing edge. Another helper that reaches the
                // same destination must still justify its own access.
                let scoped_reader = readers
                    .iter()
                    .any(|edge| edge.owner == here && edge.dependency == next);
                let stage_or_representation =
                    representation || native.is_some() || under(start, &PSI_STAGES);
                let enters_coordinator = (runtime || stage_or_representation)
                    && [
                        SOURCE_DRIVER,
                        native_stages::DRIVER,
                        native_stages::OBJECT_REPLAY,
                        super::PRODUCT_COORDINATOR,
                    ]
                    .contains(&next.as_str());
                // Only the retained-image leaf may inspect the completed image directly.
                let reads_retained_image = start == super::RETAINED_IMAGE_RUNTIME
                    && here == super::RETAINED_IMAGE_RUNTIME
                    && next == native_stages::NATIVE_ARTIFACT;
                if enters_coordinator
                    || (runtime && dependency_phase.is_some() && !reads_retained_image)
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
    fn only_the_retained_image_host_may_read_the_completed_image() {
        let host = super::super::RETAINED_IMAGE_RUNTIME;
        let core = super::super::RUNTIME_ROOT;
        let image = native_stages::NATIVE_ARTIFACT;
        let metadata = Metadata {
            workspace_root: "/repo".into(),
            packages: vec![
                pkg("core", core, &[("image-host", host, None)]),
                pkg(
                    "image-host",
                    host,
                    &[("image", image, None), ("helper", "shared", None)],
                ),
                pkg("image", image, &[]),
                pkg(
                    "helper",
                    "shared",
                    &[("driver", native_stages::DRIVER, None)],
                ),
                pkg("driver", native_stages::DRIVER, &[]),
            ],
        };
        let errors = violations(&metadata).unwrap();
        assert!(
            errors
                .iter()
                .any(|row| row.contains("core -> image-host -> image"))
        );
        assert!(
            errors
                .iter()
                .any(|row| row.contains("image-host -> helper -> driver"))
        );
        assert!(!errors.iter().any(|row| row.starts_with("forbidden production dependency: image-host -> image ")));
    }

    /// A new representation-to-stage link is refused, directly or through a
    /// helper or another representation's legacy link, while stages may still
    /// read representations. A legacy row whose link is gone is refused too.
    #[test]
    fn a_representation_cannot_link_a_numbered_stage() {
        const RECORDS: &str =
            "source-rust/subsystems/compiler/psi-to-native/representations/new-records";
        const SOURCE_RECORDS: &str =
            "source-rust/subsystems/compiler/source-to-psi/representations/new-records";
        let [(encoding, aarch64), (_, x86), (receipts, publication)] =
            LEGACY_REPRESENTATION_STAGE_LINKS;
        let metadata = Metadata {
            workspace_root: "/repo".into(),
            packages: vec![
                pkg(
                    "records",
                    RECORDS,
                    &[
                        ("allocation", NATIVE_STAGES[7], None),
                        ("helper", "helper", None),
                        ("encoding", encoding, None),
                    ],
                ),
                pkg(
                    "source-records",
                    SOURCE_RECORDS,
                    &[("typing", PSI_STAGES[3], None)],
                ),
                pkg("helper", "helper", &[("selection", NATIVE_STAGES[5], None)]),
                pkg(
                    "encoding",
                    encoding,
                    &[("aarch64", aarch64, None), ("x86", x86, None)],
                ),
                pkg("receipts", receipts, &[("publication", publication, None)]),
                pkg(
                    "allocation",
                    NATIVE_STAGES[7],
                    &[("records", RECORDS, None)],
                ),
                pkg("selection", NATIVE_STAGES[5], &[]),
                pkg(
                    "typing",
                    PSI_STAGES[3],
                    &[("source-records", SOURCE_RECORDS, None)],
                ),
                pkg("aarch64", aarch64, &[]),
                pkg("x86", x86, &[]),
                pkg("publication", publication, &[]),
            ],
        };
        let errors: Vec<_> = violations(&metadata)
            .unwrap()
            .into_iter()
            .filter(|error| error.starts_with("representation links a numbered stage"))
            .collect();
        for chain in [
            ": records -> allocation (",
            ": records -> helper -> selection (",
            ": records -> encoding -> aarch64 (",
            ": records -> encoding -> x86 (",
            ": source-records -> typing (",
        ] {
            assert!(
                errors.iter().any(|error| error.contains(chain)),
                "{chain}: {errors:?}"
            );
        }
        assert_eq!(errors.len(), 5, "{errors:?}");

        // Dropping a legacy link leaves its row behind, which the shared
        // allowance check refuses so the row is deleted with the link.
        let mut without_receipts = metadata.clone();
        let receipts_package = without_receipts
            .packages
            .iter_mut()
            .find(|package| package.name == "receipts")
            .unwrap();
        receipts_package.dependencies.clear();
        let stale = crate::boundary::stale_allowance_violations(
            &without_receipts,
            &crate::boundary::forbidden_rules(),
        )
        .unwrap();
        assert!(
            stale.iter().any(|error| error
                == &format!(
                    "{receipts} -> {publication}: edge removed; remove the obsolete allowance"
                )),
            "{stale:?}"
        );
        assert!(
            !stale
                .iter()
                .any(|error| error.starts_with(&format!("{encoding} -> "))),
            "{stale:?}"
        );
    }

    #[test]
    fn loose_packages_and_loose_driver_modules_cannot_reenter_the_pipeline() {
        let temp = std::env::temp_dir().join(format!(
            "omega-xtask-numbered-pipeline-{}",
            std::process::id()
        ));
        for stage in PSI_STAGES.iter().chain(&NATIVE_STAGES) {
            std::fs::create_dir_all(temp.join(stage)).unwrap();
        }
        std::fs::write(temp.join(SOURCE_PIPELINE).join("README.md"), "").unwrap();
        assert!(layout_violations(&temp).unwrap().is_empty());
        std::fs::create_dir(temp.join(NATIVE_PIPELINE).join("helper")).unwrap();
        std::fs::write(temp.join(SOURCE_PIPELINE).join("driver.rs"), "").unwrap();
        let errors = layout_violations(&temp).unwrap();
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert!(errors.iter().any(|e| e.contains("/helper:")));
        assert!(errors.iter().any(|e| e.contains("/driver.rs:")));
        std::fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn shared_representations_cannot_reenter_a_public_compiler_or_replay_driver_through_a_helper() {
        for representation in REPRESENTATIONS {
            for driver in [
                SOURCE_DRIVER,
                native_stages::DRIVER,
                native_stages::OBJECT_REPLAY,
                super::super::PRODUCT_COORDINATOR,
            ] {
                let metadata = Metadata {
                    workspace_root: "/repo".into(),
                    packages: vec![
                        pkg("records", representation, &[("helper", "helper", None)]),
                        pkg("helper", "helper", &[("compile", driver, None)]),
                        pkg("compile", driver, &[]),
                    ],
                };
                assert!(
                    violations(&metadata)
                        .unwrap()
                        .iter()
                        .any(|error| error.contains("records -> helper -> compile"))
                );
            }
        }
    }

    #[test]
    fn a_shared_helper_cannot_reenter_a_driver_or_borrow_another_readers_permission() {
        let allowed = native_stages::scoped_readers()
            .into_iter()
            .find(|edge| edge.owner.ends_with("/encoding-admission"))
            .unwrap();
        let driver = native_stages::DRIVER;
        let replay = native_stages::OBJECT_REPLAY;
        let metadata = Metadata {
            workspace_root: "/repo".into(),
            packages: vec![
                pkg(
                    "lowering",
                    NATIVE_STAGES[3],
                    &[("encoding", allowed.owner, None), ("helper", "other", None)],
                ),
                pkg(
                    "encoding",
                    allowed.owner,
                    &[("encoder", allowed.dependency, None)],
                ),
                pkg(
                    "helper",
                    "other",
                    &[
                        ("encoder", allowed.dependency, None),
                        ("driver", driver, None),
                        ("replay", replay, None),
                    ],
                ),
                pkg("encoder", allowed.dependency, &[]),
                pkg("driver", driver, &[]),
                pkg("replay", replay, &[]),
            ],
        };
        let errors = violations(&metadata).unwrap();
        assert!(
            !errors
                .iter()
                .any(|message| message.contains("lowering -> encoding -> encoder"))
        );
        for destination in ["encoder", "driver", "replay"] {
            assert!(
                errors.iter().any(
                    |message| message.contains(&format!("lowering -> helper -> {destination}"))
                )
            );
        }
    }

    /// A stage that links another stage is refused, directly or through a
    /// helper. The fixture holds the three links V1 used to keep: lowering to
    /// preparation, and publication to lowering and to the checker.
    #[test]
    fn a_source_stage_cannot_link_another_stages_implementation() {
        const SHARED: &str = "shared";
        let name = |path: &'static str| path.rsplit('/').next().unwrap();
        let stage = |index: usize, links: &[&'static str]| {
            let links: Vec<_> = links.iter().map(|path| (name(path), *path, None)).collect();
            pkg(name(PSI_STAGES[index]), PSI_STAGES[index], &links)
        };
        let metadata = Metadata {
            workspace_root: "/repo".into(),
            packages: vec![
                stage(2, &[]),
                stage(3, &[SHARED]),
                stage(4, &[]),
                stage(5, &[PSI_STAGES[4]]),
                stage(6, &[]),
                stage(7, &[PSI_STAGES[5], PSI_STAGES[6]]),
                pkg(
                    SHARED,
                    SHARED,
                    &[(name(PSI_STAGES[2]), PSI_STAGES[2], None)],
                ),
            ],
        };
        let errors: Vec<_> = violations(&metadata)
            .unwrap()
            .into_iter()
            .filter(|error| error.starts_with("forbidden production dependency"))
            .collect();
        let [typer, lowering, checker, publication] =
            [3, 5, 6, 7].map(|index| name(PSI_STAGES[index]));
        let [resolver, preparation] = [2, 4].map(|index| name(PSI_STAGES[index]));
        assert_eq!(errors.len(), 4, "{errors:?}");
        for chain in [
            format!(": {typer} -> shared -> {resolver} "),
            format!(": {lowering} -> {preparation} "),
            format!(": {publication} -> {lowering} "),
            format!(": {publication} -> {checker} "),
        ] {
            assert!(
                errors.iter().any(|error| error.contains(&chain)),
                "{chain}: {errors:?}"
            );
        }
    }

    /// Only the stage that produces a product, and the representation whose
    /// constructor takes the capability, may link its issuer. A test
    /// dependency is refused like any other.
    #[test]
    fn only_the_producing_stage_may_link_a_product_issuer() {
        let name = |path: &'static str| path.rsplit('/').next().unwrap();
        for (issuer, producer, representation) in SOURCE_PRODUCT_ISSUERS {
            let key = |kind| [("issuer", issuer, kind)];
            let consumer = PSI_STAGES[producer + 1];
            let metadata = Metadata {
                workspace_root: "/repo".into(),
                packages: vec![
                    pkg("issuer", issuer, &[]),
                    pkg("product", representation, &key(None)),
                    pkg(name(PSI_STAGES[producer]), PSI_STAGES[producer], &key(None)),
                    pkg(name(consumer), consumer, &key(Some("dev"))),
                    pkg("source-to-psi", SOURCE_DRIVER, &key(None)),
                ],
            };
            let errors: Vec<_> = violations(&metadata)
                .unwrap()
                .into_iter()
                .filter(|error| error.contains("may issue this product"))
                .collect();
            assert_eq!(errors.len(), 2, "{errors:?}");
            for refused in [name(consumer), "source-to-psi"] {
                assert!(
                    errors
                        .iter()
                        .any(|error| error.starts_with(&format!("{refused} -> issuer:"))),
                    "{refused}: {errors:?}"
                );
            }
        }
    }

    #[test]
    fn nested_stages_and_streaming_reject_indirect_production_edges() {
        const SHARED: &str = "shared";
        const ENCODER: &str = "source-rust/subsystems/compiler/psi-to-native/pipeline/11_bound-framed-machine-to-selected-form-encoding/test-encoder";
        const FIXTURE: &str = "_recycling/source-rust/test-support/image-fixtures";
        let paths = NATIVE_STAGES
            .into_iter()
            .chain([SOURCE_DRIVER, native_stages::PRODUCT_PUBLICATION])
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
        edge(SOURCE_DRIVER, FIXTURE, Some("dev"));
        edge(TEXT_READER, SHARED, None);
        edge(PSI_STAGES[6], SHARED, None);
        edge(PSI_STAGES[6], PSI_STAGES[5], Some("dev"));
        edge(NATIVE_STAGES[2], SHARED, None);
        edge(NATIVE_STAGES[6], SHARED, None);
        edge(SHARED, NATIVE_STAGES[1], None);
        edge(NATIVE_STAGES[5], NATIVE_STAGES[7], None);
        edge(FRONTEND[0], FRONTEND[1], Some("dev"));
        edge(FRONTEND[1], SHARED, Some("build"));
        edge(SHARED, ENCODER, None);
        edge(SHARED, FRONTEND[0], None);
        edge(SHARED, SOURCE_DRIVER, None);
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
        assert!(
            errors
                .iter()
                .any(|v| v.contains("workspace member is under _recycling: image-fixtures"))
        );
        assert!(
            errors
                .iter()
                .any(|v| v.contains("shared -> alias: Cargo dependency points into _recycling"))
        );
        assert!(errors.iter().any(|v| {
            v.contains("source-to-psi -> alias: Cargo dependency points into _recycling")
        }));
        let feature_errors: Vec<_> = errors
            .iter()
            .filter(|error| error.contains("enables test-support"))
            .collect();
        assert_eq!(feature_errors.len(), 1);
        assert!(feature_errors[0].contains("shared -> alias"));
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
