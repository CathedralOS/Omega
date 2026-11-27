//! Check native compiler and hosted execution's source-free dependencies.
//!
//! Uses Cargo's declared path dependencies (including optional, build, and
//! target-specific edges, excluding dev-dependencies for production boundaries)
//! to enforce the Psi/Omega firewall and stage boundaries.

mod driver_calls;
mod linear_pipeline;
mod native_stages;
mod psi_ir;

use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const NATIVE_ROOT: &str = native_stages::DRIVER;
pub const PROVIDER_SELECTION_ROOT: &str = "source-rust/subsystems/compiler/psi-to-native/verification/provider-correspondence";
pub const RUNTIME_ROOT: &str = "source-rust/subsystems/interpreter/host";
pub const RETAINED_IMAGE_RUNTIME: &str = "source-rust/subsystems/interpreter/host/retained-image";
pub const NATIVE_PROVIDER_RUNTIME_ROOT: &str = "source-rust/subsystems/interpreter/host/native-worker";

const BOUND_FRAMED_MACHINE: &str = "source-rust/subsystems/compiler/psi-to-native/representations/bound-framed-machine";
const BINDING_ISSUER: &str = "source-rust/subsystems/compiler/psi-to-native/representations/bound-framed-machine/issuer";

const PSI_RECORDS: &str = "source-rust/subsystems/compiler/psi";
const PSI_ARTIFACT: &str = "source-rust/subsystems/compiler/psi/artifact";
const PSI_EXECUTION_INPUTS: &str = "source-rust/subsystems/compiler/psi/execution-inputs";
const PSI_SEMANTICS: &str = "source-rust/subsystems/compiler/psi/semantics";
const NUMERICS: &str = "source-rust/subsystems/compiler/foundation/numerics";
const INTERPRETER: &str = "source-rust/subsystems/interpreter";
// Each of these owners is available to both pipelines. None may call either compiler.
const SHARED_PSI: &[&str] = &[PSI_RECORDS, NUMERICS];
const PSI_CONSUMERS: &[&str] = &[PSI_ARTIFACT, PSI_EXECUTION_INPUTS, PSI_SEMANTICS, INTERPRETER];

// The native driver's own package is admitted separately. Runtime consumers
// may read the declared native contracts without gaining the driver entrance.
pub const PERMITTED_OWNERS: &[&str] = &[
    NATIVE_ROOT,
    "source-rust/subsystems/compiler/psi-to-native/pipeline",
    "source-rust/subsystems/compiler/psi-to-native/verification",
    "source-rust/subsystems/compiler/psi-to-native/targets",
    "source-rust/subsystems/compiler/psi-to-native/representations",
    PSI_RECORDS,
    "source-rust/subsystems/compiler/foundation",
    "source-rust/foundation",
    "source-rust/subsystems/compiler/build/model",
    "source-rust/subsystems/compiler/build/build-time",
    "source-rust/subsystems/compiler/build/evaluation/authority",
    "source-rust/subsystems/compiler/build/providers",
    INTERPRETER,
];

const SOURCE_COMPILER: &str = "source-rust/subsystems/compiler/source-to-psi";
const PACKAGE_MANAGER: &str = "source-rust/subsystems/package-manager";
const PRODUCT_COORDINATOR: &str = "source-rust/subsystems/compiler";
#[cfg(test)]
const BUILD_COORDINATOR: &str = "source-rust/subsystems/compiler/build";
const SOURCE_OPERATIONS: &str = "source-rust/subsystems/source";
#[cfg(test)]
const CLI_APPLICATION: &str = "source-rust/application";
const REVIEW_EVIDENCE: &str = "source-rust/subsystems/package-manager/review/evidence";
#[cfg(test)]
const PSI_FACADE: &str = linear_pipeline::SOURCE_DRIVER;
const SOURCE_PROVIDER_PUBLICATION: &str =
    "source-rust/subsystems/compiler/source-to-psi/representations/published-target-providers";
const NATIVE_PROVIDER_DEMAND: &str =
    "source-rust/subsystems/compiler/psi-to-native/pipeline/00_psi-to-admitted-native-inputs/provider-demand";
const FRONTEND_OPERATIONS: &[&str] = &[linear_pipeline::FRONTEND[0], linear_pipeline::FRONTEND[1]];

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Allowance {
    pub owner: &'static str,
    pub dependency: &'static str,
}

pub struct Rule {
    pub owners: Vec<&'static str>,
    pub forbidden: Vec<&'static str>,
    pub forbidden_packages: Vec<&'static str>,
    pub reason: &'static str,
    pub except_owners: Vec<&'static str>,
    pub except_package: Option<&'static str>,
    pub allowed: Vec<Allowance>,
}

impl Rule {
    fn forbids(&self, dependency: &str) -> bool {
        self.forbidden_packages.contains(&dependency) || under(dependency, &self.forbidden)
    }
}

#[derive(Deserialize, Clone)]
pub struct Metadata {
    pub workspace_root: String,
    pub packages: Vec<Package>,
}

#[derive(Deserialize, Clone)]
pub struct Package {
    pub name: String,
    pub manifest_path: String,
    pub dependencies: Vec<Dependency>,
}

#[derive(Deserialize, Clone)]
pub struct Dependency {
    pub name: String,
    pub kind: Option<String>,
    pub path: Option<String>,
    #[serde(default)]
    pub features: Vec<String>,
}

fn under(relative: &str, owners: &[&str]) -> bool {
    owners.iter().any(|owner| {
        owner.is_empty()
            || relative == *owner
            || relative
                .strip_prefix(owner)
                .is_some_and(|rest| rest.starts_with('/'))
    })
}

fn normalize_relative(root: &Path, target: &Path) -> Result<String, String> {
    let root_canon = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let target_canon = target
        .canonicalize()
        .unwrap_or_else(|_| target.to_path_buf());
    target_canon
        .strip_prefix(&root_canon)
        .or_else(|_| target.strip_prefix(root))
        .map(|rel| rel.to_string_lossy().replace('\\', "/"))
        .map_err(|_| {
            format!(
                "path is outside workspace root: {}",
                target.to_string_lossy()
            )
        })
}

pub fn forbidden_rules() -> Vec<Rule> {
    use linear_pipeline::{NATIVE_STAGES, SOURCE_DRIVER};
    let rule = |owners, forbidden, reason| Rule {
        owners,
        forbidden,
        forbidden_packages: vec![],
        reason,
        except_owners: vec![],
        except_package: None,
        allowed: vec![],
    };
    // Stage-reader rules below apply to the application, project coordinator,
    // package manager, and helpers by default. Review and native verification
    // also have specific forbidden package dependencies.
    let mut rules = vec![
        Rule {
            forbidden_packages: vec![PACKAGE_MANAGER, PRODUCT_COORDINATOR],
            ..rule(vec![REVIEW_EVIDENCE], vec![],
                "review evidence is captured independently of package policy and compilation")
        },
        Rule {
            forbidden_packages: vec![NATIVE_ROOT],
            ..rule(vec![PROVIDER_SELECTION_ROOT], vec![SOURCE_COMPILER],
                "provider correspondence compares admitted declarations")
        },
    ];
    rules.push(Rule {
        forbidden_packages: vec![BINDING_ISSUER],
        allowed: vec![
            Allowance { owner: NATIVE_STAGES[10], dependency: BINDING_ISSUER },
            Allowance { owner: BOUND_FRAMED_MACHINE, dependency: BINDING_ISSUER },
        ],
        ..rule(vec![""], vec![], "only installation binding may issue a bound framed machine")
    });
    rules.push(Rule {
        forbidden_packages: vec![SOURCE_PROVIDER_PUBLICATION],
        allowed: vec![
            Allowance { owner: linear_pipeline::PSI_STAGES[7], dependency: SOURCE_PROVIDER_PUBLICATION },
            Allowance { owner: NATIVE_PROVIDER_DEMAND, dependency: SOURCE_PROVIDER_PUBLICATION },
        ],
        ..rule(vec![""], vec![], "only source publication and native input admission exchange completed Build provider demand")
    });
    rules.push(rule(
        vec!["source-rust/subsystems/compiler/psi/execution-inputs/providers"],
        vec!["source-rust/subsystems/compiler/build"],
        "portable provider selection reads the artifact and receiving installation",
    ));
    // Only the source driver runs the full numbered source pipeline. The text reader
    // owns single-file syntax for the parse command and package edits. The other readers take stage 07 records.
    rules.push(Rule {
        except_package: Some(SOURCE_DRIVER),
        except_owners: linear_pipeline::PSI_STAGES.to_vec(),
        forbidden_packages: vec![linear_pipeline::TEXT_READER],
        allowed: [
            (PACKAGE_MANAGER, linear_pipeline::TEXT_READER),
            (SOURCE_OPERATIONS, linear_pipeline::TEXT_READER),
            (linear_pipeline::TEXT_READER, linear_pipeline::PSI_STAGES[0]),
            (linear_pipeline::TEXT_READER, linear_pipeline::PSI_STAGES[1]),
            ("source-rust/subsystems/compiler/build/evaluation/result", linear_pipeline::PSI_STAGES[7]),
            ("source-rust/subsystems/compiler/source-to-psi/representations/evaluation-receipts", linear_pipeline::PSI_STAGES[7]),
        ].into_iter().map(|(owner, dependency)| Allowance { owner, dependency }).collect(),
        ..rule(
            vec![""],
            linear_pipeline::PSI_STAGES.to_vec(),
            "source stages are consumed by the source driver, numbered stages, or an explicitly scoped reader",
        )
    });
    rules.push(Rule {
        forbidden_packages: vec![PRODUCT_COORDINATOR, SOURCE_DRIVER, native_stages::DRIVER, native_stages::PRODUCT_PUBLICATION],
        ..rule(
            vec![PACKAGE_MANAGER],
            vec![],
            "package management does not execute compiler stages",
        )
    });
    rules.extend(native_stages::rules());
    rules.push(rule(
        NATIVE_STAGES[5..=9].to_vec(),
        vec![EARLIER_PROGRAM_FORMS, SEALED_TERMINAL_MODULE],
        "physical planning reads the retained target operations",
    ));
    rules.push(rule(
        vec![FRONTEND_OPERATIONS[0]],
        vec![FRONTEND_OPERATIONS[1], linear_pipeline::FRONTEND[3]],
        "the lexer produces tokens",
    ));
    rules.push(Rule {
        except_package: Some(linear_pipeline::TEXT_READER),
        ..rule(
            vec![FRONTEND_OPERATIONS[1]],
            vec![FRONTEND_OPERATIONS[0]],
            "the parser consumes token streams",
        )
    });
    rules.push(Rule {
        forbidden_packages: vec![SOURCE_DRIVER],
        ..rule(
            vec!["source-rust/subsystems/compiler/source-to-psi/representations"],
            vec![],
            "source records cannot depend on the source driver",
        )
    });
    rules
}

fn production_edges(metadata: &Metadata) -> Result<Vec<(&Package, String, String)>, String> {
    let root = Path::new(&metadata.workspace_root);
    let mut edges = Vec::new();
    for package in &metadata.packages {
        let manifest = Path::new(&package.manifest_path);
        let Some(parent) = manifest.parent() else {
            continue;
        };
        let owner = normalize_relative(root, parent)?;
        for dependency in &package.dependencies {
            if dependency.kind.as_deref() == Some("dev") {
                continue;
            }
            let Some(dep_path) = &dependency.path else {
                continue;
            };
            let relative = normalize_relative(root, Path::new(dep_path))?;
            edges.push((package, owner.clone(), relative));
        }
    }
    Ok(edges)
}

pub fn forbidden_dependency_violations(
    metadata: &Metadata,
    rules: &[Rule],
) -> Result<Vec<String>, String> {
    let mut violations = Vec::new();
    for (package, owner, relative) in production_edges(metadata)? {
        for rule in rules {
            if under(&owner, &rule.owners)
                && !under(&owner, &rule.except_owners)
                && Some(owner.as_str()) != rule.except_package
                && rule.forbids(&relative)
                && !rule
                    .allowed
                    .iter()
                    .any(|allowed| allowed.owner == owner && allowed.dependency == relative)
            {
                let dep_name = Path::new(&relative)
                    .file_name()
                    .map(|n| n.to_string_lossy())
                    .unwrap_or_default();
                violations.push(format!("{} -> {dep_name}: {}", package.name, rule.reason));
            }
        }
    }
    Ok(violations)
}

pub fn stale_allowance_violations(
    metadata: &Metadata,
    rules: &[Rule],
) -> Result<Vec<String>, String> {
    let edges: BTreeSet<(String, String)> = production_edges(metadata)?
        .into_iter()
        .map(|(_, owner, relative)| (owner, relative))
        .collect();
    let mut unique_allowances = Vec::new();
    for rule in rules {
        for &allowed in &rule.allowed {
            if !unique_allowances.contains(&allowed) {
                unique_allowances.push(allowed);
            }
        }
    }
    let mut violations = Vec::new();
    for allowed in unique_allowances {
        let covered = rules.iter().any(|rule| {
            rule.allowed.contains(&allowed)
                && under(allowed.owner, &rule.owners)
                && rule.forbids(allowed.dependency)
        });
        if !covered {
            violations.push(format!(
                "{} -> {}: allowance names an edge no rule forbids; delete the row",
                allowed.owner, allowed.dependency
            ));
        } else if !edges.contains(&(allowed.owner.to_owned(), allowed.dependency.to_owned())) {
            violations.push(format!(
                "{} -> {}: edge removed; remove the obsolete allowance",
                allowed.owner, allowed.dependency
            ));
        }
    }
    Ok(violations)
}

/// Earlier executable program forms: the abstract program, the optimization
/// unit, and the unit's independent semantic checker (all under this path).
const EARLIER_PROGRAM_FORMS: &str =
    "source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations";

/// Stage 02's working program: `TargetOperationPlan` and its control graph.
/// The records it shares with later forms live in `target-vocabulary`, nested
/// in this directory as a separate package.
const STAGE_02_WORKING_FORM: &str =
    "source-rust/subsystems/compiler/psi-to-native/representations/target-operations";

/// Instruction selection, the installation record it is handed, and the forms
/// from the prepared plan onward. None of them is handed stage 02's working
/// program, so none may be able to name it.
const AFTER_PREPARATION: &[&str] = &[
    "source-rust/subsystems/compiler/psi-to-native/verification/installation-admission/bound-installations",
    "source-rust/subsystems/compiler/psi-to-native/representations/target-operations/target-vocabulary",
    "source-rust/subsystems/compiler/psi-to-native/representations/target-operations/legalized-operations",
    "source-rust/subsystems/compiler/psi-to-native/representations/selected-instructions",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/06_selected-instructions-to-selected-instructions/analyses",
    "source-rust/subsystems/compiler/psi-to-native/representations/register-homes",
    "source-rust/subsystems/compiler/psi-to-native/representations/post-allocation-machine",
    "source-rust/subsystems/compiler/psi-to-native/representations/machine-code",
    "source-rust/subsystems/compiler/psi-to-native/targets/register-environment",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/11_bound-framed-machine-to-selected-form-encoding",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/12_selected-form-encoding-to-resolved-layout",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/13_resolved-layout-to-placed-functions",
];

/// The crate that defines the sealed Terminal Psi module
/// (`ClosedTerminalModule`) and the canonical artifact. Native forms use the
/// Psi record encoders for their identities. Reaching `terminal-codec` would let
/// those forms name the sealed module.
const SEALED_TERMINAL_MODULE: &str =
    "source-rust/subsystems/compiler/psi/artifact/codec";

/// Selection and later forms carry shared contracts, not the prepared plan's
/// instruction graph. Follow indirect production edges as well as direct ones.
const AFTER_SELECTION: &[&str] = &[
    "source-rust/subsystems/compiler/psi-to-native/representations/selected-instructions",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/06_selected-instructions-to-selected-instructions/analyses",
    "source-rust/subsystems/compiler/psi-to-native/representations/register-homes",
    "source-rust/subsystems/compiler/psi-to-native/representations/post-allocation-machine",
    "source-rust/subsystems/compiler/psi-to-native/representations/machine-code",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/11_bound-framed-machine-to-selected-form-encoding",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/12_selected-form-encoding-to-resolved-layout",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/13_resolved-layout-to-placed-functions",
];

const PREPARED_EXECUTION_PLAN: &str =
    "source-rust/subsystems/compiler/psi-to-native/representations/target-operations/legalized-operations";

/// Instruction selection, the forms from the prepared plan onward, the stages
/// that turn one into the next, the target setup and installation record they
/// share, and the leaf vocabulary they carry. Instruction selection is handed
/// only the prepared execution plan and its bound installations; none of these
/// crates may reach an earlier program form. The check follows every
/// production edge, so a new path through a helper crate is caught too.
const SELECTION_REPRESENTATIONS: &[&str] = &[
    "source-rust/subsystems/compiler/psi-to-native/verification/installation-admission/bound-installations",
    "source-rust/subsystems/compiler/psi-to-native/representations/operation-operands",
    "source-rust/subsystems/compiler/psi-to-native/representations/operation-evidence",
    "source-rust/subsystems/compiler/psi-to-native/representations/structural-storage",
    "source-rust/subsystems/compiler/psi-to-native/representations/structural-storage/queries",
    "source-rust/subsystems/compiler/psi-to-native/representations/target-operations",
    "source-rust/subsystems/compiler/psi-to-native/representations/target-operations/legalized-operations",
    "source-rust/subsystems/compiler/psi-to-native/representations/selected-instructions",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/06_selected-instructions-to-selected-instructions/analyses",
    "source-rust/subsystems/compiler/psi-to-native/representations/register-homes",
    "source-rust/subsystems/compiler/psi-to-native/representations/post-allocation-machine",
    "source-rust/subsystems/compiler/psi-to-native/representations/machine-code",
    "source-rust/subsystems/compiler/psi-to-native/targets/register-environment",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/11_bound-framed-machine-to-selected-form-encoding",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/12_selected-form-encoding-to-resolved-layout",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/13_resolved-layout-to-placed-functions",
];

/// Every crate named by `SELECTION_REPRESENTATIONS` or `AFTER_PREPARATION`,
/// once each.
fn native_forms() -> Vec<&'static str> {
    let mut forms: Vec<&'static str> = SELECTION_REPRESENTATIONS
        .iter()
        .chain(AFTER_PREPARATION)
        .copied()
        .collect();
    forms.sort_unstable();
    forms.dedup();
    forms
}

/// Report each of `roots` that can reach a crate under `forbidden` through
/// production dependencies, with the chain that reaches it. Dev-dependencies
/// are ignored; optional dependencies count, since a feature can enable them.
pub fn transitive_forbidden_violations(
    metadata: &Metadata,
    roots: &[&str],
    forbidden: &str,
    reason: &str,
) -> Result<Vec<String>, String> {
    transitive_violations(
        metadata,
        roots,
        |directory| under(directory, &[forbidden]),
        reason,
    )
}

/// As `transitive_forbidden_violations`, for one crate exactly: crates nested
/// in its directory are separate packages and stay allowed.
pub fn transitive_exact_violations(
    metadata: &Metadata,
    roots: &[&str],
    forbidden: &str,
    reason: &str,
) -> Result<Vec<String>, String> {
    transitive_violations(metadata, roots, |directory| directory == forbidden, reason)
}

fn transitive_violations(
    metadata: &Metadata,
    roots: &[&str],
    forbidden: impl Fn(&str) -> bool,
    reason: &str,
) -> Result<Vec<String>, String> {
    let root = Path::new(&metadata.workspace_root);
    let mut packages: BTreeMap<String, &Package> = BTreeMap::new();
    for package in &metadata.packages {
        if let Some(parent) = Path::new(&package.manifest_path).parent() {
            packages.insert(normalize_relative(root, parent)?, package);
        }
    }
    let mut violations = Vec::new();
    for &start in roots {
        let Some(&first) = packages.get(start) else {
            return Err(format!("{start} is absent from the workspace"));
        };
        let mut pending = vec![(start.to_owned(), vec![first.name.clone()])];
        let mut visited = BTreeSet::new();
        while let Some((directory, chain)) = pending.pop() {
            if !visited.insert(directory.clone()) {
                continue;
            }
            if forbidden(&directory) {
                violations.push(format!("{}: {reason}", chain.join(" -> ")));
                continue;
            }
            let Some(&package) = packages.get(&directory) else {
                return Err(format!(
                    "{}: path dependency is not a workspace member: {directory}",
                    chain.join(" -> ")
                ));
            };
            for dependency in &package.dependencies {
                if dependency.kind.as_deref() == Some("dev") {
                    continue;
                }
                let Some(dep_path) = &dependency.path else {
                    continue;
                };
                let rel = normalize_relative(root, Path::new(dep_path))?;
                let mut next = chain.clone();
                next.push(
                    packages
                        .get(&rel)
                        .map_or_else(|| dependency.name.clone(), |package| package.name.clone()),
                );
                pending.push((rel, next));
            }
        }
    }
    Ok(violations)
}

pub fn dependency_violations(
    metadata: &Metadata,
    entrance_path: &str,
    owners: &[&str],
) -> Result<Vec<String>, String> {
    let root = Path::new(&metadata.workspace_root);
    let mut packages: BTreeMap<String, &Package> = BTreeMap::new();
    for package in &metadata.packages {
        if let Some(parent) = Path::new(&package.manifest_path).parent() {
            let rel = normalize_relative(root, parent)?;
            packages.insert(rel, package);
        }
    }
    if !packages.contains_key(entrance_path) {
        return Err(format!("{entrance_path} is absent from the workspace"));
    }
    let mut pending = vec![(entrance_path.to_owned(), Vec::<String>::new())];
    let mut visited = BTreeSet::new();
    let mut violations = Vec::new();
    while let Some((directory, parents)) = pending.pop() {
        if !visited.insert(directory.clone()) {
            continue;
        }
        let Some(&package) = packages.get(&directory) else {
            return Err(format!(
                "path dependency is not a workspace member: {directory}"
            ));
        };
        let mut chain = parents;
        chain.push(package.name.clone());
        if !under(&directory, owners) {
            violations.push(format!("{}: {directory}", chain.join(" -> ")));
            continue;
        }
        for dependency in &package.dependencies {
            if dependency.kind.as_deref() == Some("dev") {
                continue;
            }
            let Some(dep_path) = &dependency.path else {
                continue;
            };
            let rel = normalize_relative(root, Path::new(dep_path))?;
            pending.push((rel, chain.clone()));
        }
    }
    Ok(violations)
}

/// The shared Psi format can be used by either compiler. Its dependency tree
/// cannot contain a compiler, and neither compiler may run the other through a helper.
pub fn pipeline_dependency_violations(metadata: &Metadata) -> Result<Vec<String>, String> {
    const SOURCE: &str = "source-rust/subsystems/compiler/source-to-psi";
    const BUILD: &str = "source-rust/subsystems/compiler/build";
    const NATIVE: &str = native_stages::DRIVER;

    let root = Path::new(&metadata.workspace_root);
    let mut packages = BTreeMap::new();
    let mut edges = BTreeMap::<String, Vec<String>>::new();
    let mut violations = Vec::new();
    for package in &metadata.packages {
        let directory = Path::new(&package.manifest_path)
            .parent()
            .ok_or("manifest has no parent")?;
        let owner = normalize_relative(root, directory)?;
        packages.insert(owner.clone(), &package.name);
        for dep in &package.dependencies {
            let Some(path) = &dep.path else { continue };
            let dependency = normalize_relative(root, Path::new(path))?;
            if dep.kind.as_deref() == Some("dev") {
                if (under(&owner, &[SOURCE]) || under(&owner, SHARED_PSI)) && under(&dependency, &[NATIVE]) {
                    violations.push(format!(
                        "{} -> {}: a source or Psi test imports native compilation ({dependency})",
                        package.name, dep.name
                    ));
                } else if under(&owner, SHARED_PSI)
                    && (under(&dependency, &[SOURCE, BUILD]) || dependency == PRODUCT_COORDINATOR)
                {
                    violations.push(format!("{} -> {}: a Psi test imports compilation or Build ({dependency})", package.name, dep.name));
                } else if under(&owner, &[PSI_RECORDS])
                    && !under(&owner, PSI_CONSUMERS)
                    && under(&dependency, PSI_CONSUMERS)
                {
                    violations.push(format!("{} -> {}: a Psi record test imports an artifact or execution owner ({dependency})", package.name, dep.name));
                }
            } else {
                edges.entry(owner.clone()).or_default().push(dependency);
            }
        }
    }
    // Psi records and their readers share a directory. Readers may use other readers
    // while the program records, including their nested crates, must remain independent.
    for (owners, excluded_owners, forbidden, forbidden_packages) in [
        (&[SOURCE][..], &[][..], &[NATIVE][..], &[][..]),
        (SHARED_PSI, &[][..], &[SOURCE, NATIVE, BUILD][..], &[PRODUCT_COORDINATOR][..]),
        (&[PSI_RECORDS][..], PSI_CONSUMERS, PSI_CONSUMERS, &[][..]),
        (&[NATIVE][..], &[][..], &[SOURCE][..], &[][..]),
    ] {
        let mut pending = VecDeque::new();
        let mut seen = BTreeSet::new();
        for (path, name) in &packages {
            if under(path, owners) && !under(path, excluded_owners) {
                pending.push_back((path.as_str(), name.to_string()));
                seen.insert(path.as_str());
            }
        }
        if pending.is_empty() {
            return Err(format!("compiler boundary has no package under {}", owners.join(", ")));
        }
        while let Some((path, chain)) = pending.pop_front() {
            for next in edges.get(path).into_iter().flatten() {
                if !seen.insert(next.as_str()) {
                    continue;
                }
                let next_name = packages
                    .get(next)
                    .map_or(next.as_str(), |name| name.as_str());
                let chain = format!("{chain} -> {next_name}");
                // Native admission reads the published record. Keep walking its dependencies
                // so a record cannot hide a call back into the source compiler.
                let reads_publication = owners == [NATIVE]
                    && path == NATIVE_PROVIDER_DEMAND
                    && next == SOURCE_PROVIDER_PUBLICATION;
                if (under(next, forbidden) && !reads_publication)
                    || forbidden_packages.contains(&next.as_str())
                {
                    violations.push(format!("forbidden compiler dependency: {chain} ({next})"));
                } else {
                    pending.push_back((next, chain));
                }
            }
        }
    }
    Ok(violations)
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>, violations: &mut Vec<String>) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) => {
            violations.push(format!("{}: {error}", dir.display()));
            return;
        }
    };
    let mut paths = Vec::new();
    for entry in entries {
        match entry {
            Ok(entry) => paths.push(entry.path()),
            Err(error) => violations.push(format!("{}: {error}", dir.display())),
        }
    }
    paths.sort();
    for path in paths {
        match path.metadata() {
            Ok(metadata) if metadata.is_dir() => {
                if path.file_name().is_some_and(|name| name != "_recycling") {
                    collect_rs_files(&path, out, violations);
                }
            }
            Ok(_) if path.extension().is_some_and(|ext| ext == "rs") => out.push(path),
            Ok(_) => {}
            Err(error) => violations.push(format!("{}: {error}", path.display())),
        }
    }
}

fn read_audited_source(path: &Path, relative: &str, violations: &mut Vec<String>) -> Option<String> {
    match fs::read_to_string(path) {
        Ok(text) => Some(text),
        Err(error) => {
            violations.push(format!("{relative}: {error}"));
            None
        }
    }
}

fn line_has_retired_string_representation_spelling(line: &str) -> bool {
    if !line.contains("\"string\"") {
        return false;
    }
    let compact: String = line.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.contains("name==\"string\"")
        || compact.contains("name!=\"string\"")
        || compact.contains("\"string\"==name")
        || compact.contains("\"string\"!=name")
    {
        return true;
    }
    if (compact.contains(")==\"string\"")
        || compact.contains(")!=\"string\"")
        || compact.contains("\"string\"==")
        || compact.contains("\"string\"!="))
        && compact.contains(".as_str(")
    {
        return true;
    }
    if compact.contains("matches!(")
        && compact.contains(".as_str(")
        && compact.contains("\"string\"")
    {
        return true;
    }
    false
}

/// The entry points of the target-operation plan validator.
const PLAN_VALIDATOR_ENTRIES: &[&str] = &[
    "verify_execution_plan(",
    "verify_target_operation_plan_with_closed_module(",
];
/// The crate that defines the validator, and the one production caller: the
/// explicit stage-validator request (`omega --validator ...`).
const PLAN_VALIDATOR_CALLERS: &[&str] = &[
    "source-rust/subsystems/compiler/psi-to-native/pipeline/04_abstract-operations-to-target-operations/src/preparation/",
    "source-rust/subsystems/compiler/psi-to-native/verification/published-objects/src/lib.rs",
];

/// The target-operation accessor for its working program.
const WORKING_PROGRAM_ACCESSORS: &[&str] = &["working_program_for_validation"];
const TARGET_REPLAY_RECORD: &str = "source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations/verified-optimization-input/src/publication/target_replay.rs";
/// Binding belongs to the driver after preparation, not to setters on the
/// prepared stage output. Neither the old definitions nor callers may return.
const RETIRED_INSTALLATION_SETTERS: &[&str] = &[
    "with_installed_entry_execution",
    "with_installed_stack_requirements",
];
/// Stage 04 and its preparation crate, and the one file through which image
/// emission's replay validators read the working program.
const WORKING_PROGRAM_READERS: &[&str] = &[
    "source-rust/subsystems/compiler/psi-to-native/pipeline/04_abstract-operations-to-target-operations/src/target_lowering/",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image/image-emission/src/retained_working_program.rs",
    "source-rust/subsystems/compiler/psi-to-native/verification/published-objects/src/lib.rs",
];

/// Naming either gateway is restricted, so importing it under another name
/// cannot give object production access to the target-operation working program.
const IMAGE_REPLAY_GATEWAYS: &[&str] = &["retained_working_program", "retained_source_validation"];
const IMAGE_REPLAY_READERS: &[&str] = &[
    "source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image/image-emission/src/retained_working_program.rs",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image/image-emission/src/function_fragments/source/retained_source_validation.rs",
    "source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image/image-emission/src/function_fragments/validation.rs",
];
const IMAGE_REPLAY_DECLARATIONS: &[(&str, &str)] = &[
    (
        "source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image/image-emission/src/lib.rs",
        "mod retained_working_program;",
    ),
    (
        "source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image/image-emission/src/function_fragments.rs",
        "pub(super) mod retained_source_validation;",
    ),
];

fn is_test_source(relative: &str) -> bool {
    relative.contains("/tests/") || Path::new(relative).file_name().is_some_and(|name| name == "tests.rs")
}

fn line_calls_any(line: &str, entries: &[&str]) -> bool {
    let code = line.trim_start();
    !code.starts_with("//") && entries.iter().any(|entry| code.contains(entry))
}

/// Report each line in the active Rust implementation that calls one of `entries`
/// outside the files or directories in `allowed` and outside tests.
fn call_violations(
    root: &Path,
    entries: &[&str],
    allowed: &[&str],
    declarations: &[(&str, &str)],
    reason: &str,
) -> Vec<String> {
    call_violations_in(root, "source-rust", entries, allowed, declarations, reason)
}

fn call_violations_in(
    root: &Path,
    scope: &str,
    entries: &[&str],
    allowed: &[&str],
    declarations: &[(&str, &str)],
    reason: &str,
) -> Vec<String> {
    let mut violations = Vec::new();
    let mut files = Vec::new();
    let directory = root.join(scope);
    if !directory.is_dir() {
        return vec![format!("{scope}: implementation directory is missing")];
    }
    collect_rs_files(&directory, &mut files, &mut violations);
    for path in files {
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if allowed.iter().any(|owner| {
            if owner.ends_with('/') {
                rel.starts_with(owner)
            } else {
                rel == *owner
            }
        }) || is_test_source(&rel)
        {
            continue;
        }
        let Some(text) = read_audited_source(&path, &rel, &mut violations) else {
            continue;
        };
        for (index, line) in text.lines().enumerate() {
            if line_calls_any(line, entries)
                && !declarations
                    .iter()
                    .any(|(file, declaration)| rel == *file && line.trim() == *declaration)
            {
                violations.push(format!("{rel}:{}: {reason}", index + 1));
            }
        }
    }
    violations
}

/// Each ordinary stage reads its current input form, carrying the instructions,
/// contracts and settled requirements its successor needs, not earlier program
/// containers. Explicit validator requests borrow original replay owners apart
/// from the ordinary path. Each row names a file, its entry and allowed arity;
/// parameters past the first are configuration.
const SINGLE_FORM_ENTRIES: &[(&str, &str, usize)] = &[
    (
        "source-rust/subsystems/compiler/psi-to-native/pipeline/05_target-operations-to-selected-instructions/src/lib.rs",
        "select_instructions",
        1,
    ),
    (
        "source-rust/subsystems/compiler/psi-to-native/pipeline/09_post-allocation-machine-to-framed-machine/src/lib.rs",
        "plan_function_frame",
        1,
    ),
    (
        "source-rust/subsystems/compiler/psi-to-native/pipeline/11_bound-framed-machine-to-selected-form-encoding/src/lib.rs",
        "encode_bound_function_body",
        1,
    ),
    (
        "source-rust/subsystems/compiler/psi-to-native/representations/machine-code/encoding-admission/src/lib.rs",
        "validate_optimized_layout_independent_selected_form_encoding",
        1,
    ),
    (
        "source-rust/subsystems/compiler/psi-to-native/pipeline/12_selected-form-encoding-to-resolved-layout/src/lib.rs",
        "resolve_program",
        1,
    ),
    (
        "source-rust/subsystems/compiler/psi-to-native/representations/machine-code/encoding-admission/src/layout/mod.rs",
        "validate_optimized_resolved_selected_form_layout",
        1,
    ),
    (
        "source-rust/subsystems/compiler/psi-to-native/pipeline/13_resolved-layout-to-placed-functions/src/function_realization.rs",
        "assemble_fixed_frame_realization",
        1,
    ),
];

/// The parameters of `pub fn <name>` in `text`, split at top-level commas,
/// or `None` when the function is not defined there.
fn public_function_parameters(text: &str, name: &str) -> Option<Vec<String>> {
    let needle = format!("pub fn {name}");
    let rest = text.match_indices(&needle).find_map(|(index, _)| {
        let rest = &text[index + needle.len()..];
        rest.starts_with(['<', '(']).then_some(rest)
    })?;
    // Skip a generic parameter list, which may itself contain parentheses.
    let mut depth = 0usize;
    let mut open = None;
    for (index, character) in rest.char_indices() {
        match character {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            '(' if depth == 0 => {
                open = Some(index);
                break;
            }
            _ => {}
        }
    }
    let body = &rest[open? + 1..];
    let mut parameters = Vec::new();
    let mut current = String::new();
    let mut nesting = 0usize;
    for character in body.chars() {
        match character {
            '(' | '<' | '[' | '{' => nesting += 1,
            ')' if nesting == 0 => break,
            ')' | '>' | ']' | '}' => nesting = nesting.saturating_sub(1),
            ',' if nesting == 0 => {
                parameters.push(std::mem::take(&mut current));
                continue;
            }
            _ => {}
        }
        current.push(character);
    }
    parameters.push(current);
    Some(
        parameters
            .into_iter()
            .map(|parameter| parameter.trim().to_owned())
            .filter(|parameter| !parameter.is_empty())
            .collect(),
    )
}

pub fn single_form_entry_violations(root: &Path) -> Vec<String> {
    let mut violations = Vec::new();
    for &(file, name, allowed) in SINGLE_FORM_ENTRIES {
        let Ok(text) = fs::read_to_string(root.join(file)) else {
            violations.push(format!("{file}: missing; it should define `{name}`"));
            continue;
        };
        match public_function_parameters(&text, name) {
            None => violations.push(format!("{file}: `pub fn {name}` is not defined here")),
            Some(parameters) if parameters.len() != allowed => violations.push(format!(
                "{file}: `{name}` takes {} parameters, not {allowed}; carry what it needs in its input form",
                parameters.len()
            )),
            Some(_) => {}
        }
    }
    violations
}

/// Keep the target-operation plan validator on explicit validation and test
/// paths. It derives the execution plan again, so calling it during ordinary
/// compilation would repeat the preparation work.
pub fn plan_validator_call_violations(root: &Path) -> Vec<String> {
    call_violations(
        root,
        PLAN_VALIDATOR_ENTRIES,
        PLAN_VALIDATOR_CALLERS,
        &[],
        "stage 02's plan validator is called outside the explicit validator request and tests",
    )
}

/// Stage 04 publishes the prepared execution plan; its working program is
/// scratch that only stage 04 and validators of its output may read. A later
/// stage that read it would depend on a form the plan was meant to replace.
pub fn working_program_read_violations(root: &Path) -> Vec<String> {
    call_violations(
        root,
        WORKING_PROGRAM_ACCESSORS,
        WORKING_PROGRAM_READERS,
        &[(
            TARGET_REPLAY_RECORD,
            "pub fn working_program_for_validation(&self) -> &TargetOperationPlan {",
        )],
        "the target-operation working program is read outside stage 04, its validators and tests; read the prepared execution plan",
    )
}

/// Package review reads issued source demand, and an audited image reads its
/// own frozen target. Only the original publishing operations can issue them.
fn retained_publication_violations(root: &Path) -> Vec<String> {
    let mut errors = call_violations_in(
        root,
        "source-rust",
        &["publish_package_review_numeric_requirements"],
        &[
            "source-rust/subsystems/compiler/source-to-psi/pipeline/07_checked-program-to-psi/src/provider_supply/package_review.rs",
        ],
        &[(
            "source-rust/subsystems/compiler/source-to-psi/pipeline/07_checked-program-to-psi/src/source/package_review.rs",
            "pub(crate) fn publish_package_review_numeric_requirements(",
        )],
        "package numeric review demand can only be issued when publishing the accepted provider request",
    );
    errors.extend(call_violations(
        root,
        &["issue_target_provider_demand"],
        &[
            "source-rust/subsystems/compiler/source-to-psi/representations/published-target-providers/publication.rs",
            "source-rust/subsystems/compiler/source-to-psi/pipeline/07_checked-program-to-psi/src/provider_supply/demand.rs",
        ],
        &[],
        "only the accepted source judgment can issue target provider demand for its published artifact",
    ));
    errors.extend(call_violations(root, &["retain_selected_body_facts"], &["source-rust/subsystems/compiler/psi-to-native/pipeline/05_target-operations-to-selected-instructions/src/lib.rs"], &[("source-rust/subsystems/compiler/psi-to-native/representations/post-allocation-machine/body-facts/src/lib.rs", "pub fn retain_selected_body_facts(")], "only instruction selection may issue the joined body facts"));
    errors.extend(call_violations(root, &["into_framed_body_facts"], &["source-rust/subsystems/compiler/psi-to-native/pipeline/09_post-allocation-machine-to-framed-machine/entry-planning/src/imports.rs"], &[("source-rust/subsystems/compiler/psi-to-native/representations/post-allocation-machine/body-facts/src/lib.rs", "pub fn into_framed_body_facts(")], "only frame publication may open the carried body facts"));
    errors.extend(call_violations(
        root,
        &["retain_target_operations"],
        &["source-rust/subsystems/compiler/psi-to-native/pipeline/04_abstract-operations-to-target-operations/src/target_lowering/mod.rs"],
        &[(TARGET_REPLAY_RECORD, "pub fn retain_target_operations(")],
        "only native program planning can issue the archived target with its emitted facts",
    ));
    errors.extend(call_violations(
        root,
        &[
            "target_for_correspondence",
            ".retained_target_operations",
            "::retained_target_operations",
        ],
        &[
            "source-rust/subsystems/compiler/psi-to-native/tests/physical_publication/mod.rs",
            "source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image/image-emission/src/retained_working_program.rs",
            "source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image/image-emission/src/function_fragments/source/retained_source_validation.rs",
            "source-rust/subsystems/compiler/psi-to-native/verification/published-objects/src/lib.rs",
        ],
        &[
            (TARGET_REPLAY_RECORD, "pub fn target_for_correspondence(&self) -> &TargetOperationsForReplay {"),
            ("source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image/image-emission/src/object_artifact.rs", ".map(crate::RetainedTargetOperations::target_for_correspondence)"),
        ],
        "only explicit object correspondence audits may inspect the original target forms",
    ));
    errors
}

/// Production must not reach stage 04 through a replay helper or its import.
pub fn image_replay_read_violations(root: &Path) -> Vec<String> {
    call_violations(
        root,
        IMAGE_REPLAY_GATEWAYS,
        IMAGE_REPLAY_READERS,
        IMAGE_REPLAY_DECLARATIONS,
        "image production names a retained-program replay gateway; read the prepared execution plan",
    )
}

/// Backend publication reads current callable facts and the carried preparation
/// root, not the target operation plan or the earlier optimization output.
pub fn backend_execution_plan_read_violations(root: &Path) -> Vec<String> {
    call_violations_in(
        root,
        "source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image",
        &["execution_plan", "pre_physical_manifest("],
        &[
            "source-rust/subsystems/compiler/psi-to-native/verification/published-objects/src/lib.rs",
            "source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image/image-emission/src/function_fragments/source/retained_source_validation.rs",
        ],
        &[],
        "backend production reads an earlier execution plan; read current execution contracts",
    )
}

pub fn installation_mutation_violations(root: &Path) -> Vec<String> {
    call_violations(
        root,
        RETIRED_INSTALLATION_SETTERS,
        &[],
        &[],
        "installation records mutate a prepared stage output; bind one checked receipt in the driver and pass it beside the plan",
    )
}

pub fn representation_spelling_violations(root: &Path) -> Vec<String> {
    let mut violations = Vec::new();
    for owner in ["source-rust/subsystems/compiler", "source-rust/subsystems/interpreter"] {
        let mut files = Vec::new();
        collect_rs_files(&root.join(owner), &mut files, &mut violations);
        for path in files {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let Some(text) = read_audited_source(&path, &rel, &mut violations) else {
                continue;
            };
            for (index, line) in text.lines().enumerate() {
                if line_has_retired_string_representation_spelling(line) {
                    violations.push(format!(
                        "{rel}:{}: runtime representation depends on retired text spelling",
                        index + 1
                    ));
                }
            }
        }
    }
    violations
}

pub fn run(args: &[String]) -> Result<(), u8> {
    let ownership_only = match args {
        [] => false,
        [flag] if flag == "--ownership" => true,
        _ => {
            eprintln!("usage: cargo xtask check-boundary [--ownership]");
            return Err(2);
        }
    };
    let root = crate::repo_root()?;
    let output = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(&root)
        .output();
    let output = match output {
        Ok(output) if output.status.success() => output,
        Ok(output) => {
            eprintln!(
                "native boundary could not be checked: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
            return Err(2);
        }
        Err(error) => {
            eprintln!("native boundary could not be checked: {error}");
            return Err(2);
        }
    };
    let metadata: Metadata = match serde_json::from_slice(&output.stdout) {
        Ok(metadata) => metadata,
        Err(error) => {
            eprintln!("native boundary could not be checked: {error}");
            return Err(2);
        }
    };

    let check_all = || -> Result<Vec<String>, String> {
        let mut violations = linear_pipeline::layout_violations(&root)?;
        violations.extend(linear_pipeline::violations(&metadata)?);
        violations.extend(driver_calls::violations(&root)?);
        let rules = forbidden_rules();
        violations.extend(forbidden_dependency_violations(&metadata, &rules)?);
        violations.extend(stale_allowance_violations(&metadata, &rules)?);
        violations.extend(psi_ir::violations(&root)?);
        violations.extend(pipeline_dependency_violations(&metadata)?);
        if ownership_only {
            return Ok(violations);
        }
        violations.extend(representation_spelling_violations(&root));
        violations.extend(call_violations(
            &root,
            &["bind_target_reach"],
            &["source-rust/subsystems/compiler/psi-to-native/pipeline/01_admitted-native-inputs-to-target-bound-program/execution-scope/src/lib.rs"],
            &[(
                "source-rust/subsystems/compiler/psi-to-native/representations/target-bound-program/native_execution_scope.rs",
                "pub fn bind_target_reach(",
            )],
            "stage 01 issues reachability before a native body can be lowered",
        ));
        violations.extend(plan_validator_call_violations(&root));
        violations.extend(single_form_entry_violations(&root));
        violations.extend(working_program_read_violations(&root));
        violations.extend(retained_publication_violations(&root));
        violations.extend(image_replay_read_violations(&root));
        violations.extend(backend_execution_plan_read_violations(&root));
        violations.extend(installation_mutation_violations(&root));
        for dependency in [
            "source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations/verified-optimization-input",
            "source-rust/subsystems/compiler/psi-to-native/verification/provider-admission",
        ] {
            violations.extend(transitive_exact_violations(
                &metadata,
                &["source-rust/subsystems/compiler/psi-to-native/pipeline/01_admitted-native-inputs-to-target-bound-program/execution-scope"],
                dependency,
                "entry reachability uses the checked program entry and selected providers",
            )?);
        }
        violations.extend(transitive_forbidden_violations(
            &metadata,
            SELECTION_REPRESENTATIONS,
            EARLIER_PROGRAM_FORMS,
            "a representation instruction selection reads or writes reaches an earlier program form; name the operand, operation evidence or storage contract from its owning representation",
        )?);
        violations.extend(transitive_exact_violations(
            &metadata,
            AFTER_PREPARATION,
            STAGE_02_WORKING_FORM,
            "a stage or form after the prepared plan reaches stage 02's working program; name the record from target-vocabulary",
        )?);
        violations.extend(transitive_exact_violations(
            &metadata,
            AFTER_SELECTION,
            PREPARED_EXECUTION_PLAN,
            "a selected or later form reaches the prepared instruction graph; name shared records from target-vocabulary",
        )?);
        violations.extend(transitive_exact_violations(
            &metadata,
            &native_forms(),
            SEALED_TERMINAL_MODULE,
            "a native form reaches the sealed Terminal Psi module; use the Psi encoder for the record it carries",
        )?);
        violations.extend(transitive_exact_violations(
            &metadata,
            &[
                "source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations",
                "source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations/optimization-unit",
                "source-rust/subsystems/compiler/psi-to-native/representations/structural-storage",
            ],
            "source-rust/subsystems/compiler/psi-to-native/representations/structural-storage/queries",
            "a native record depends on storage queries that belong to the stages that call them",
        )?);
        let declaration_encoding =
            "source-rust/subsystems/compiler/psi/artifact/declaration-encoding";
        violations.extend(transitive_exact_violations(
            &metadata,
            &[declaration_encoding],
            PSI_SEMANTICS,
            "writing Psi declaration records must not compile the storage checker",
        )?);
        violations.extend(transitive_exact_violations(
            &metadata,
            &[
                declaration_encoding,
                "source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations/optimization-unit",
                "source-rust/subsystems/compiler/psi-to-native/representations/structural-storage",
            ],
            "source-rust/subsystems/compiler/psi/artifact/wire",
            "native declaration identities must use psi-declaration-encoding to keep the full artifact reader out of these crates",
        )?);
        let mut native_driver_owners = PERMITTED_OWNERS.to_vec();
        native_driver_owners.push(SOURCE_PROVIDER_PUBLICATION);
        violations.extend(dependency_violations(
            &metadata,
            NATIVE_ROOT,
            &native_driver_owners,
        )?);
        let provider_selection_owners = &PERMITTED_OWNERS[1..];
        violations.extend(dependency_violations(
            &metadata,
            PROVIDER_SELECTION_ROOT,
            provider_selection_owners,
        )?);
        // Native planning now owns trust reconstruction as a private operation.
        violations.extend(dependency_violations(
            &metadata,
            linear_pipeline::NATIVE_STAGES[1],
            &native_driver_owners[1..],
        )?);
        let mut runtime_owners = vec![
            RUNTIME_ROOT,
            NATIVE_PROVIDER_RUNTIME_ROOT,
            "source-rust/foundation/bounded-process",
            "source-rust/subsystems/interpreter",
        ];
        runtime_owners.extend_from_slice(&PERMITTED_OWNERS[1..]);
        violations.extend(dependency_violations(
            &metadata,
            RUNTIME_ROOT,
            &runtime_owners,
        )?);
        // The v1 published-program crate was retired. Its maintained native
        // preparation now belongs to the driver checked above.
        let mut native_provider_owners = vec![
            NATIVE_PROVIDER_RUNTIME_ROOT,
            "source-rust/foundation/bounded-process",
        ];
        native_provider_owners.extend_from_slice(&PERMITTED_OWNERS[1..]);
        violations.extend(dependency_violations(
            &metadata,
            NATIVE_PROVIDER_RUNTIME_ROOT,
            &native_provider_owners,
        )?);
        Ok(violations)
    };

    match check_all() {
        Ok(violations) if violations.is_empty() && ownership_only => {
            println!("compiler ownership and stage-reader checks pass");
            Ok(())
        }
        Ok(violations) if violations.is_empty() => {
            println!(
                "native/runtime boundaries: source-free dependencies, ownership and representation checks pass"
            );
            Ok(())
        }
        Ok(violations) => {
            println!("native/runtime architecture violations:");
            for violation in violations {
                println!("{violation}");
            }
            Err(1)
        }
        Err(error) => {
            eprintln!("native boundary could not be checked: {error}");
            Err(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn pkg(name: &str, rel: &str, deps: &[(&str, &str, Option<&str>)]) -> Package {
        Package {
            name: name.to_owned(),
            manifest_path: format!("/repo/{rel}/Cargo.toml"),
            dependencies: deps
                .iter()
                .map(|(dep_name, dep_rel, kind)| Dependency {
                    name: (*dep_name).to_owned(),
                    kind: kind.map(str::to_owned),
                    path: Some(format!("/repo/{dep_rel}")),
                    features: Vec::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn encoder_cannot_acquire_the_binding_issuer() {
        let rules: Vec<_> = forbidden_rules().into_iter()
            .filter(|rule| rule.forbidden_packages.contains(&BINDING_ISSUER))
            .collect();
        let key = [("installation-binding-issuer", BINDING_ISSUER, None)];
        let metadata = Metadata {
            workspace_root: "/repo".into(),
            packages: vec![
                pkg("installation-binding", linear_pipeline::NATIVE_STAGES[10], &key),
                pkg("bound-framed-machine", BOUND_FRAMED_MACHINE, &key),
                pkg("native-encoding", linear_pipeline::NATIVE_STAGES[11], &key),
            ],
        };
        let violations = forbidden_dependency_violations(&metadata, &rules).unwrap();
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].contains("native-encoding"), "{violations:?}");
    }

    #[test]
    fn facade_boundaries_follow_dependency_paths_not_source_or_alias_spelling() {
        for (owner, stage, facade) in [
            (CLI_APPLICATION, FRONTEND_OPERATIONS[0], BUILD_COORDINATOR),
            (
                BUILD_COORDINATOR,
                linear_pipeline::NATIVE_STAGES[1],
                PRODUCT_COORDINATOR,
            ),
            (PRODUCT_COORDINATOR, FRONTEND_OPERATIONS[0], PSI_FACADE),
            (
                "samples/scientific-driver",
                linear_pipeline::NATIVE_STAGES[1],
                BUILD_COORDINATOR,
            ),
        ] {
            for (dependency, kind, permitted) in [
                (stage, None, false),
                (stage, Some("build"), false),
                (stage, Some("dev"), true),
                (facade, None, true),
            ] {
                let metadata = Metadata {
                    workspace_root: "/repo".to_owned(),
                    packages: vec![pkg(
                        "arbitrary_consumer_name",
                        owner,
                        &[("innocent_alias", dependency, kind)],
                    )],
                };
                let violations =
                    forbidden_dependency_violations(&metadata, &forbidden_rules()).unwrap();
                assert_eq!(
                    violations.is_empty(),
                    permitted,
                    "{owner} -> {dependency}: {violations:?}"
                );
            }
        }
    }

    #[test]
    fn an_external_source_reader_only_receives_its_own_named_api() {
        let stages = linear_pipeline::PSI_STAGES;
        let metadata = Metadata {
            workspace_root: "/repo".into(),
            packages: vec![
                pkg(
                    "package-manager",
                    PACKAGE_MANAGER,
                    &[
                        ("read-text", linear_pipeline::TEXT_READER, None),
                        ("run-parser", stages[1], None),
                    ],
                ),
                pkg(
                    "omega",
                    CLI_APPLICATION,
                    &[
                        ("source", SOURCE_OPERATIONS, None),
                        ("bypass-source", linear_pipeline::TEXT_READER, None),
                        ("run-parser", stages[1], None),
                    ],
                ),
                pkg(
                    "omega-source",
                    SOURCE_OPERATIONS,
                    &[
                        ("parse-file", linear_pipeline::TEXT_READER, None),
                        ("run-parser", stages[1], None),
                    ],
                ),
                pkg(
                    "source-syntax",
                    linear_pipeline::TEXT_READER,
                    &[
                        ("run-lexer", stages[0], None),
                        ("run-parser", stages[1], None),
                        ("run-checker", stages[6], None),
                    ],
                ),
                pkg(
                    "neighbor",
                    "source-rust/subsystems/package-manager/helper",
                    &[("read-text", linear_pipeline::TEXT_READER, Some("build"))],
                ),
                pkg(
                    "source-driver",
                    linear_pipeline::SOURCE_DRIVER,
                    &[("lower", stages[5], None)],
                ),
                pkg(
                    "source-record",
                    linear_pipeline::FRONTEND[3],
                    &[("lower", stages[5], None)],
                ),
            ],
        };
        let rules: Vec<_> = forbidden_rules()
            .into_iter()
            .filter(|rule| rule.reason.starts_with("source stages are consumed"))
            .collect();
        let violations = forbidden_dependency_violations(&metadata, &rules).unwrap();
        assert_eq!(violations.len(), 7, "{violations:?}");
        for blocked in [
            "omega -> source-syntax",
            "omega -> 01_tokens-to-syntax-trees",
            "omega-source -> 01_tokens-to-syntax-trees",
        ] {
            assert!(
                violations.iter().any(|error| error.contains(blocked)),
                "{blocked}"
            );
        }
        assert!(
            violations
                .iter()
                .any(|error| error.contains("package-manager -> 01_tokens-to-syntax-trees"))
        );
        assert!(
            violations
                .iter()
                .any(|error| error.contains("neighbor -> source-syntax"))
        );
        assert!(
            violations
                .iter()
                .any(|error| error
                    .contains("source-syntax -> 06_lowered-program-to-checked-program"))
        );
        assert!(
            violations
                .iter()
                .any(|error| error.contains("source-record -> 05_prepared-typed-trees-to-lowered-program"))
        );
    }

    #[test]
    fn only_the_native_driver_runs_omega_stages() {
        let library = linear_pipeline::NATIVE_STAGES[13];
        let stages = linear_pipeline::NATIVE_STAGES;
        for (owner, dependency, kind, permitted) in [
            (native_stages::DRIVER, stages[7], None, true),
            (stages[14], stages[12], None, true),
            (stages[15], stages[12], None, false),
            (linear_pipeline::PSI_STAGES[7], SOURCE_PROVIDER_PUBLICATION, None, true),
            (NATIVE_PROVIDER_DEMAND, SOURCE_PROVIDER_PUBLICATION, None, true),
            (PACKAGE_MANAGER, SOURCE_PROVIDER_PUBLICATION, None, false),
            (stages[1], SOURCE_PROVIDER_PUBLICATION, None, false),
            ("source-rust/subsystems/compiler/psi/execution-inputs/providers", "source-rust/subsystems/compiler/build/evaluation/receipts", None, false),
            ("source-rust/subsystems/compiler/build/inputs", linear_pipeline::PSI_STAGES[7], None, false),
            ("source-rust/subsystems/compiler/build/inputs", stages[0], None, false),
            (library, stages[2], None, false),
            (library, stages[2], Some("build"), false),
            (library, stages[2], Some("dev"), true),
            (PACKAGE_MANAGER, stages[1], None, false),
            (PACKAGE_MANAGER, PRODUCT_COORDINATOR, None, false),
            (PACKAGE_MANAGER, native_stages::DRIVER, None, false),
            (PACKAGE_MANAGER, linear_pipeline::SOURCE_DRIVER, None, false),
            (PACKAGE_MANAGER, linear_pipeline::TEXT_READER, None, true),
            (library, stages[1], None, false),
            (library, stages[7], None, false),
            (native_stages::IMAGE_EMISSION, stages[2], None, false),
            (
                native_stages::PRODUCT_PUBLICATION,
                native_stages::NATIVE_ARTIFACT,
                None,
                true,
            ),
            (
                native_stages::PRODUCT_PUBLICATION,
                native_stages::IMAGE_EMISSION,
                None,
                false,
            ),
            (
                "source-rust/subsystems/compiler/psi-to-native/other-driver",
                stages[0],
                None,
                false,
            ),
        ] {
            let metadata = Metadata {
                workspace_root: "/repo".to_owned(),
                packages: vec![pkg("consumer", owner, &[("stage", dependency, kind)])],
            };
            let violations = forbidden_dependency_violations(&metadata, &forbidden_rules())
                .expect("fixture paths belong to the test workspace");
            assert_eq!(
                violations.is_empty(),
                permitted,
                "{owner} -> {dependency}: {violations:?}"
            );
        }
    }

    #[test]
    fn shared_receipt_consumers_preserve_stage_boundaries() {
        for (owner, dependency, permitted) in [
            (linear_pipeline::NATIVE_STAGES[2], linear_pipeline::NATIVE_STAGES[1], true),
            (
                linear_pipeline::NATIVE_STAGES[3],
                linear_pipeline::NATIVE_STAGES[1],
                false,
            ),
            (linear_pipeline::NATIVE_STAGES[3], linear_pipeline::NATIVE_STAGES[2], true),
            (
                linear_pipeline::NATIVE_STAGES[4],
                linear_pipeline::NATIVE_STAGES[1],
                false,
            ),
        ] {
            let metadata = Metadata {
                workspace_root: "/repo".to_owned(),
                packages: vec![pkg("consumer", owner, &[("producer", dependency, None)])],
            };
            let violations = forbidden_dependency_violations(&metadata, &forbidden_rules())
                .expect("fixture paths belong to the test workspace");
            assert_eq!(
                violations.is_empty(),
                permitted,
                "{owner} -> {dependency}: {violations:?}"
            );
        }
    }

    #[test]
    fn selection_representations_cannot_reach_earlier_forms_through_any_crate() {
        let plan =
            "source-rust/subsystems/compiler/psi-to-native/representations/target-operations/legalized-operations";
        let helper = "omega-rust/omega/compilation/backend/helper";
        let unit = "source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations/optimization-unit";
        let reaches = |kind: Option<&str>| Metadata {
            workspace_root: "/repo".to_owned(),
            packages: vec![
                pkg("legalized-operations", plan, &[("helper", helper, None)]),
                pkg("helper", helper, &[("optimization-unit", unit, kind)]),
                pkg("optimization-unit", unit, &[]),
            ],
        };
        let violations =
            transitive_forbidden_violations(&reaches(None), &[plan], EARLIER_PROGRAM_FORMS, "no")
                .unwrap();
        assert_eq!(
            violations,
            vec!["legalized-operations -> helper -> optimization-unit: no".to_owned()]
        );
        assert!(
            transitive_forbidden_violations(
                &reaches(Some("dev")),
                &[plan],
                EARLIER_PROGRAM_FORMS,
                "no"
            )
            .unwrap()
            .is_empty(),
            "a test-only edge is not a production path"
        );
    }

    #[test]
    fn transitive_audit_reports_a_missing_production_package() {
        let metadata = |kind| Metadata {
            workspace_root: "/repo".to_owned(),
            packages: vec![
                pkg("entry", "entry", &[("helper", "helper", None)]),
                pkg("helper", "helper", &[("leaf", "leaf", kind)]),
            ],
        };
        let check = |metadata: &Metadata, forbidden| {
            transitive_exact_violations(metadata, &["entry"], forbidden, "forbidden")
        };
        assert_eq!(
            check(&metadata(None), "unrelated").unwrap_err(),
            "entry -> helper -> leaf: path dependency is not a workspace member: leaf"
        );
        assert_eq!(
            check(&metadata(None), "leaf").unwrap(),
            ["entry -> helper -> leaf: forbidden"]
        );
        assert!(check(&metadata(Some("dev")), "unrelated").unwrap().is_empty());
        let mut complete = metadata(None);
        complete.packages.push(pkg("leaf", "leaf", &[]));
        assert!(check(&complete, "unrelated").unwrap().is_empty());
    }

    #[test]
    fn a_nested_package_is_not_the_crate_it_sits_in() {
        let form = "source-rust/subsystems/compiler/psi-to-native/representations/machine-code";
        let working = STAGE_02_WORKING_FORM;
        let vocabulary =
            "source-rust/subsystems/compiler/psi-to-native/representations/target-operations/target-vocabulary";
        let metadata = |dependency: &str| Metadata {
            workspace_root: "/repo".to_owned(),
            packages: vec![
                pkg("machine-code", form, &[("dependency", dependency, None)]),
                pkg("target-operations", working, &[]),
                pkg("target-vocabulary", vocabulary, &[]),
            ],
        };
        assert!(
            transitive_exact_violations(&metadata(vocabulary), &[form], working, "no")
                .unwrap()
                .is_empty(),
            "the vocabulary nested under target-operations is its own package"
        );
        assert_eq!(
            transitive_exact_violations(&metadata(working), &[form], working, "no").unwrap(),
            vec!["machine-code -> target-operations: no".to_owned()]
        );
    }

    #[test]
    fn every_later_form_refuses_an_indirect_prepared_graph_dependency() {
        let helper = "omega-rust/omega/compilation/backend/helper";
        for &form in AFTER_SELECTION {
            let metadata = |kind| Metadata {
                workspace_root: "/repo".to_owned(),
                packages: vec![
                    pkg("later-form", form, &[("helper", helper, None)]),
                    pkg(
                        "helper",
                        helper,
                        &[("legalized-operations", PREPARED_EXECUTION_PLAN, kind)],
                    ),
                    pkg("legalized-operations", PREPARED_EXECUTION_PLAN, &[]),
                ],
            };
            for kind in [None, Some("build")] {
                assert_eq!(
                    transitive_exact_violations(
                        &metadata(kind),
                        &[form],
                        PREPARED_EXECUTION_PLAN,
                        "no"
                    )
                    .unwrap(),
                    vec!["later-form -> helper -> legalized-operations: no".to_owned()],
                    "{form}"
                );
            }
            assert!(
                transitive_exact_violations(
                    &metadata(Some("dev")),
                    &[form],
                    PREPARED_EXECUTION_PLAN,
                    "no"
                )
                .unwrap()
                .is_empty()
            );
        }
    }

    #[test]
    fn single_form_entries_count_parameters_at_the_top_level() {
        let text = "pub fn stage<Source: Into<(u8, u8)>>(\n    source: Source,\n) -> Result<(), ()> {}\n\
                    pub fn joined(a: &[u8], b: Map<K, (V, W)>, c: impl Fn(u8, u8),) {}";
        assert_eq!(
            public_function_parameters(text, "stage"),
            Some(vec!["source: Source".to_owned()])
        );
        assert_eq!(
            public_function_parameters(text, "joined").map(|parameters| parameters.len()),
            Some(3)
        );
        assert_eq!(public_function_parameters(text, "absent"), None);
        assert_eq!(public_function_parameters(text, "stag"), None);
    }

    #[test]
    fn native_forms_reach_vocabulary_encoders_but_not_the_sealed_module() {
        let plan =
            "source-rust/subsystems/compiler/psi-to-native/representations/target-operations/legalized-operations";
        let helper = "omega-rust/omega/compilation/backend/helper";
        let wire = "source-rust/subsystems/compiler/psi/artifact/wire";
        assert!(native_forms().contains(&plan));
        let metadata = |dependency: &str, kind: Option<&str>| Metadata {
            workspace_root: "/repo".to_owned(),
            packages: vec![
                pkg("legalized-operations", plan, &[("helper", helper, None)]),
                pkg("helper", helper, &[("dependency", dependency, kind)]),
                pkg("terminal-wire", wire, &[]),
                pkg(
                    "terminal-codec",
                    SEALED_TERMINAL_MODULE,
                    &[("terminal-wire", wire, None)],
                ),
            ],
        };
        let check = |dependency, kind| {
            transitive_exact_violations(
                &metadata(dependency, kind),
                &[plan],
                SEALED_TERMINAL_MODULE,
                "no",
            )
            .unwrap()
        };
        assert!(
            check(wire, None).is_empty(),
            "the encoders alone are allowed"
        );
        assert_eq!(
            check(SEALED_TERMINAL_MODULE, None),
            vec!["legalized-operations -> helper -> terminal-codec: no".to_owned()]
        );
        assert!(
            check(SEALED_TERMINAL_MODULE, Some("dev")).is_empty(),
            "a test-only edge is not a production path"
        );
    }

    #[test]
    fn only_requests_and_tests_may_call_the_plan_validator() {
        assert!(line_calls_any(
            "    native_preparation::verify_execution_plan(plan, target)?;",
            PLAN_VALIDATOR_ENTRIES
        ));
        assert!(line_calls_any(
            "let _ = verify_target_operation_plan_with_closed_module(",
            PLAN_VALIDATOR_ENTRIES
        ));
        assert!(!line_calls_any(
            "    /// (`native_preparation::verify_execution_plan(...)`).",
            PLAN_VALIDATOR_ENTRIES
        ));
        assert!(!line_calls_any(
            "    native_preparation::prepare_execution_plan(target, evidence)?;",
            PLAN_VALIDATOR_ENTRIES
        ));
        assert!(is_test_source(
            "source-rust/subsystems/compiler/psi-to-native/tests/physical_publication/mod.rs"
        ));
        assert!(line_calls_any(
            "        .working_program_for_validation()",
            WORKING_PROGRAM_ACCESSORS
        ));
        assert!(!line_calls_any(
            "    /// `working_program_for_validation()` is for validators.",
            WORKING_PROGRAM_ACCESSORS
        ));
        assert!(is_test_source(
            "source-rust/subsystems/compiler/tests/executed_fixture_discovery.rs"
        ));
        assert!(!is_test_source(
            "source-rust/subsystems/compiler/psi-to-native/driver/mod.rs"
        ));
    }

    #[test]
    fn prepared_output_installation_setters_stay_retired() {
        for code in [
            "let changed = target.with_installed_stack_requirements(rows)?;",
            "use legacy::with_installed_entry_execution as attach;",
            "pub fn with_installed_entry_execution(",
        ] {
            assert!(line_calls_any(code, RETIRED_INSTALLATION_SETTERS));
        }
        for code in [
            "let receipt = bind_installation_authority(program, cohort, callbacks, entry, stacks)?;",
            "receipt.installed_stack_requirements()",
            "// The driver binds installation authority after preparation.",
        ] {
            assert!(!line_calls_any(code, RETIRED_INSTALLATION_SETTERS));
        }
    }

    #[test]
    fn image_production_cannot_import_or_call_replay_gateways() {
        let root = std::env::temp_dir().join(format!(
            "omega-image-replay-boundary-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let image = "source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image/image-emission/src";
        assert_eq!(
            backend_execution_plan_read_violations(&root),
            ["source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image: implementation directory is missing"]
        );
        let files = [
            (
                format!("{image}/_recycling/parked.rs"),
                "crate::retained_working_program::target_operations(original);\noriginal.execution_plan();\n",
            ),
            (
                "source-rust/subsystems/package-manager/review/evidence/src/projection.rs".to_owned(),
                "let recalculate = AcceptedTerminalSource::publish_package_review_numeric_requirements;\n\
                 let pair = SemanticEmissionFacts::retain_target_operations;\n\
                 let read = RetainedTargetOperations::target_for_correspondence;\n\
                 let retained = ObjectArtifact::retained_target_operations;\n",
            ),
            (
                "source-rust/subsystems/compiler/psi-to-native/verification/published-objects/src/lib.rs".to_owned(),
                "current.working_program_for_validation();\ncurrent.execution_plan();\n",
            ),
            (
                "source-rust/subsystems/compiler/psi-to-native/driver/mod.rs".to_owned(),
                "let pair = SemanticEmissionFacts::retain_target_operations;\n\
                 use source_provider_publication::issue_target_provider_demand as forge;\n",
            ),
            (format!("{image}/lib.rs"), "mod retained_working_program;\n"),
            (
                format!("{image}/function_fragments.rs"),
                "pub(super) mod retained_source_validation;\n\
                 fn produce() { crate::retained_working_program::target_operations(input); }\n",
            ),
            (
                format!("{image}/function_fragments/production.rs"),
                "use crate::retained_working_program as old;\n\
                 use super::source::{retained_source_validation as audit};\n\
                 let read = TargetStageOutput::working_program_for_validation;\n\
                 let plan = TargetStageOutput::execution_plan;\n\
                 let earlier = current.pre_physical_manifest();\n",
            ),
            (
                format!("{image}/function_fragments/another_producer.rs"),
                "mod retained_working_program;\n",
            ),
            (
                format!("{image}/function_fragments/replay/validation.rs"),
                "crate::retained_working_program::target_operations(input);\n\
                 current.execution_plan();\n\
                 current.pre_physical_manifest();\n",
            ),
            (
                format!("{image}/function_fragments/source/retained_source_validation.rs"),
                "use crate::retained_working_program as retained;\n",
            ),
            (
                format!("{image}/function_fragments/validation.rs"),
                "source::retained_source_validation::admit(input)?;\n",
            ),
            (
                format!("{image}/function_fragments/replay/validation.rs_extra.rs"),
                "crate::retained_working_program::target_operations(input);\n\
                 current.execution_plan();\n",
            ),
        ];
        for (relative, text) in files {
            let path = root.join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        let violations = image_replay_read_violations(&root);
        let raw_accessor_violations = working_program_read_violations(&root);
        let publication_violations = retained_publication_violations(&root);
        let execution_plan_violations = backend_execution_plan_read_violations(&root);
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(violations.len(), 6, "{violations:?}");
        assert_eq!(
            publication_violations.len(),
            6,
            "{publication_violations:?}"
        );
        assert!(publication_violations.iter().any(|row| {
            row.contains("psi-to-native/driver/mod.rs:2:") && row.contains("target provider demand")
        }));
        assert_eq!(
            execution_plan_violations.len(),
            5,
            "{execution_plan_violations:?}"
        );
        assert!(
            violations
                .iter()
                .any(|row| row.contains("function_fragments.rs:2:"))
        );
        assert_eq!(
            violations
                .iter()
                .filter(|row| row.contains("production.rs:"))
                .count(),
            2
        );
        assert!(
            violations
                .iter()
                .any(|row| row.contains("another_producer.rs:1:"))
        );
        assert!(
            violations
                .iter()
                .any(|row| row.contains("validation.rs_extra.rs:1:"))
        );
        assert_eq!(
            raw_accessor_violations.len(),
            1,
            "{raw_accessor_violations:?}"
        );
        assert!(
            violations
                .iter()
                .any(|row| row.contains("function_fragments/replay/validation.rs:1:"))
        );
        assert!(raw_accessor_violations[0].contains("production.rs:3:"));
    }

    #[test]
    fn selection_cannot_reach_the_forms_its_plan_was_prepared_from() {
        const ABSTRACT: &str = "source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations";
        for (dependency, kind, permitted) in [
            (ABSTRACT, None, false),
            (
                "source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations/optimization-unit",
                None,
                false,
            ),
            (
                "source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations/optimization-unit-semantics",
                None,
                false,
            ),
            (
                "source-rust/subsystems/compiler/psi/artifact/codec",
                None,
                false,
            ),
            // Tests may build a plan the way stage 02 does.
            (ABSTRACT, Some("dev"), true),
            (
                "source-rust/subsystems/compiler/psi-to-native/representations/target-operations/legalized-operations",
                None,
                true,
            ),
            (
                "source-rust/subsystems/compiler/psi-to-native/representations/operation-operands",
                None,
                true,
            ),
        ] {
            let metadata = Metadata {
                workspace_root: "/repo".to_owned(),
                packages: vec![pkg(
                    "selection",
                    linear_pipeline::NATIVE_STAGES[5],
                    &[("dependency", dependency, kind)],
                )],
            };
            let violations = forbidden_dependency_violations(&metadata, &forbidden_rules())
                .expect("fixture paths belong to the test workspace");
            assert_eq!(
                violations.is_empty(),
                permitted,
                "selection -> {dependency} ({kind:?}): {violations:?}"
            );
        }
    }

    #[test]
    fn streams_tokens_without_importing_the_other_operation() {
        for (owner, input, forbidden) in [
            (
                FRONTEND_OPERATIONS[0],
                linear_pipeline::FRONTEND[2],
                FRONTEND_OPERATIONS[1],
            ),
            (
                FRONTEND_OPERATIONS[1],
                linear_pipeline::FRONTEND[2],
                FRONTEND_OPERATIONS[0],
            ),
            (
                "source-rust/subsystems/compiler/source-to-psi/representations/lowered-program",
                "source-rust/subsystems/compiler/foundation/semantic-records",
                PSI_FACADE,
            ),
        ] {
            for (dependency, kind, permitted) in [
                (input, None, true),
                (forbidden, None, false),
                (forbidden, Some("dev"), true),
            ] {
                let metadata = Metadata {
                    workspace_root: "/repo".into(),
                    packages: vec![pkg("consumer", owner, &[("input", dependency, kind)])],
                };
                let violations =
                    forbidden_dependency_violations(&metadata, &forbidden_rules()).unwrap();
                assert_eq!(
                    violations.is_empty(),
                    permitted,
                    "{owner} -> {dependency}: {violations:?}"
                );
            }
        }
    }

    #[test]
    fn native_consumers_can_read_build_accounting_but_cannot_run_the_project_build() {
        let evaluation = "source-rust/subsystems/compiler/build/evaluation";
        assert!(under(
            "source-rust/subsystems/compiler/build/model",
            PERMITTED_OWNERS
        ));
        assert!(!under(evaluation, PERMITTED_OWNERS));
        assert!(!under(&format!("{evaluation}/result"), PERMITTED_OWNERS));
    }

    #[test]
    fn rejects_source_tree_dependency_behind_native_compiler() {
        let metadata = Metadata {
            workspace_root: "/repo".to_owned(),
            packages: vec![
                pkg(
                    "native-compiler",
                    NATIVE_ROOT,
                    &[(
                        "effects",
                        "source-rust/subsystems/compiler/psi-to-native/representations/operation-evidence",
                        None,
                    )],
                ),
                pkg(
                    "effects",
                    "source-rust/subsystems/compiler/psi-to-native/representations/operation-evidence",
                    &[(
                        "typed-trees",
                        "source-rust/subsystems/compiler/source-to-psi/representations/typed-trees",
                        None,
                    )],
                ),
                pkg(
                    "typed-trees",
                    "source-rust/subsystems/compiler/source-to-psi/representations/typed-trees",
                    &[],
                ),
            ],
        };
        let violations = dependency_violations(&metadata, NATIVE_ROOT, PERMITTED_OWNERS).unwrap();
        assert_eq!(violations.len(), 1);
        assert!(violations[0].contains("native-compiler -> effects -> typed-trees"));
    }

    #[test]
    fn neither_pipeline_can_reenter_the_other_through_shared_dependencies() {
        let source = "source-rust/subsystems/compiler/source-to-psi";
        let psi = "source-rust/subsystems/compiler/psi";
        let native = native_stages::DRIVER;
        let metadata = Metadata {
            workspace_root: "/repo".into(),
            packages: vec![
                pkg(
                    "source",
                    source,
                    &[("shared", "shared/source", Some("build"))],
                ),
                pkg(
                    "psi",
                    psi,
                    &[
                        ("shared", "shared/psi", None),
                        ("native", native, Some("dev")),
                    ],
                ),
                pkg(
                    "native",
                    native,
                    &[
                        ("shared", "shared/native", None),
                        ("source", source, Some("dev")),
                    ],
                ),
                pkg("from-source", "shared/source", &[("native", native, None)]),
                pkg("from-psi", "shared/psi", &[("source", source, None)]),
                pkg("from-native", "shared/native", &[("source", source, None)]),
            ],
        };
        let violations = pipeline_dependency_violations(&metadata).unwrap();
        for chain in [
            "source -> from-source -> native",
            "psi -> from-psi -> source",
            "native -> from-native -> source",
            "psi -> native: a source or Psi test",
        ] {
            assert!(
                violations.iter().any(|message| message.contains(chain)),
                "{violations:?}"
            );
        }
        assert_eq!(violations.len(), 4);
    }

    #[test]
    fn published_target_providers_cannot_reenter_source_compilation() {
        let mut metadata = Metadata {
            workspace_root: "/repo".into(),
            packages: vec![
                pkg("source", SOURCE_COMPILER, &[]),
                pkg("psi", "source-rust/subsystems/compiler/psi", &[]),
                pkg(
                    "demand",
                    NATIVE_PROVIDER_DEMAND,
                    &[("published", SOURCE_PROVIDER_PUBLICATION, None)],
                ),
                pkg("published", SOURCE_PROVIDER_PUBLICATION, &[]),
            ],
        };
        assert!(pipeline_dependency_violations(&metadata).unwrap().is_empty());
        metadata.packages[3] = pkg(
            "published",
            SOURCE_PROVIDER_PUBLICATION,
            &[("source", SOURCE_COMPILER, None)],
        );
        let violations = pipeline_dependency_violations(&metadata).unwrap();
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].contains("demand -> published -> source"));
    }

    #[test]
    fn psi_record_readers_do_not_reach_project_build_or_the_compiler_facade() {
        let build = "source-rust/subsystems/compiler/build/model";
        let native = native_stages::DRIVER;
        let codec = format!("{PSI_ARTIFACT}/codec");
        let providers = format!("{PSI_EXECUTION_INPUTS}/providers");
        let predicates = format!("{PSI_SEMANTICS}/closed-predicates");
        let helper = "shared/helper";
        for owner in [codec.as_str(), providers.as_str(), predicates.as_str(), NUMERICS] {
            let metadata = Metadata {
                workspace_root: "/repo".into(),
                packages: vec![
                    pkg("psi", PSI_RECORDS, &[]),
                    pkg("reader", owner, &[
                        ("helper", helper, None),
                        ("native", native, Some("dev")),
                    ]),
                    pkg("helper", helper, &[
                        ("facade", PRODUCT_COORDINATOR, None),
                        ("receipts", build, Some("build")),
                        ("source", SOURCE_COMPILER, None),
                    ]),
                    pkg("facade", PRODUCT_COORDINATOR, &[]),
                    pkg("receipts", build, &[]),
                    pkg("source", SOURCE_COMPILER, &[]),
                    pkg("native", native, &[]),
                ],
            };
            let violations = pipeline_dependency_violations(&metadata).unwrap();
            for chain in ["reader -> helper -> facade", "reader -> helper -> receipts",
                          "reader -> helper -> source", "reader -> native: a source or Psi test"] {
                assert!(violations.iter().any(|row| row.contains(chain)), "{owner}: {violations:?}");
            }
            assert_eq!(violations.len(), 4, "{owner}: {violations:?}");
        }
    }

    #[test]
    fn psi_program_records_cannot_import_their_readers() {
        let codec = format!("{PSI_ARTIFACT}/codec");
        let entry = format!("{PSI_EXECUTION_INPUTS}/entry-memory");
        let mut metadata = Metadata {
            workspace_root: "/repo".into(),
            packages: vec![
                pkg("psi", PSI_RECORDS, &[]),
                pkg("codec", &codec, &[("psi", PSI_RECORDS, None)]),
                pkg("entry-memory", &entry, &[("psi", PSI_RECORDS, None), ("codec", &codec, None)]),
                pkg("source", SOURCE_COMPILER, &[]),
                pkg("native", native_stages::DRIVER, &[]),
                pkg("helper", "shared/helper", &[("codec", &codec, None)]),
            ],
        };
        assert!(pipeline_dependency_violations(&metadata).unwrap().is_empty());
        metadata.packages[0] = pkg("psi", PSI_RECORDS, &[
            ("helper", "shared/helper", None), ("entry-memory", &entry, Some("dev")),
        ]);
        let violations = pipeline_dependency_violations(&metadata).unwrap();
        assert_eq!(violations.len(), 2, "{violations:?}");
        assert!(violations.iter().any(|row| row.contains("psi -> helper -> codec")));
        assert!(violations.iter().any(|row| row.contains("psi -> entry-memory")));

        let records = format!("{PSI_RECORDS}/types/reflection");
        metadata.packages[0] = pkg("psi", PSI_RECORDS, &[]);
        let predicates = format!("{PSI_SEMANTICS}/closed-predicates");
        metadata.packages.push(pkg("predicates", &predicates, &[("psi", PSI_RECORDS, None)]));
        metadata.packages.push(pkg(
            "reflection", &records, &[("codec", &codec, None), ("predicates", &predicates, None)],
        ));
        let violations = pipeline_dependency_violations(&metadata).unwrap();
        assert_eq!(violations.len(), 2, "{violations:?}");
        assert!(violations.iter().any(|row| row.contains("reflection -> codec")));
        assert!(violations.iter().any(|row| row.contains("reflection -> predicates")));
    }

    #[test]
    fn unreadable_production_source_is_reported_by_both_source_audits() {
        let root = std::env::temp_dir().join(format!(
            "omega-boundary-reads-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let compiler = root.join("source-rust/subsystems/compiler");
        let interpreter = root.join("source-rust/subsystems/interpreter");
        fs::create_dir_all(&compiler).unwrap();
        fs::create_dir_all(&interpreter).unwrap();
        let inspect_calls = || call_violations(&root, &["retired"], &[], &[], "retired call");
        let name = "source-rust/subsystems/compiler/input.rs";
        fs::write(root.join(name), "fn ok() {}").unwrap();
        assert!(inspect_calls().is_empty());
        assert!(representation_spelling_violations(&root).is_empty());

        fs::write(root.join(name), [0xff]).unwrap();
        fs::write(interpreter.join("other.rs"), "fn other() { retired(); }").unwrap();
        let calls = inspect_calls();
        let spellings = representation_spelling_violations(&root);
        fs::rename(&interpreter, root.join("outside-audit")).unwrap();
        let missing_owner = representation_spelling_violations(&root);
        fs::remove_dir_all(&root).unwrap();
        assert!(calls.iter().any(|error| error.contains(name)), "{calls:?}");
        assert!(
            calls.iter().any(|error| error.contains("other.rs:1: retired call")),
            "{calls:?}"
        );
        assert!(spellings.iter().any(|error| error.contains(name)), "{spellings:?}");
        assert!(
            missing_owner.iter().any(|error| error.starts_with(interpreter.to_string_lossy().as_ref())),
            "{missing_owner:?}"
        );
    }

    #[test]
    fn detects_retired_string_representation_spelling() {
        assert!(line_has_retired_string_representation_spelling(
            r#"if symbol.as_str() == "string" {"#
        ));
        assert!(line_has_retired_string_representation_spelling(
            r#"if name == "string" {"#
        ));
        assert!(line_has_retired_string_representation_spelling(
            r#"matches!(ty.as_str(table), "u8" | "string")"#
        ));
        assert!(!line_has_retired_string_representation_spelling(
            r#"let label = "string";"#
        ));
    }
}
