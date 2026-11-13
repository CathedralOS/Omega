//! Check native compiler and hosted execution's source-free dependencies.
//!
//! Uses Cargo's declared path dependencies (including optional, build, and
//! target-specific edges, excluding dev-dependencies for production boundaries)
//! to enforce the Psi/Omega firewall and stage boundaries.

use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const NATIVE_ROOT: &str = "omega-rust/omega/compilation/native-compiler";
pub const PROVIDER_SELECTION_ROOT: &str =
    "omega-rust/omega/compilation/realization/provider-selection";
pub const PROVIDER_ADMISSION_ROOT: &str =
    "omega-rust/omega/compilation/realization/provider-admission";
pub const TRUST_ROOT: &str = "omega-rust/omega/compilation/realization/trust-model";
pub const RUNTIME_ROOT: &str = "omega-rust/omega/interpreter/terminal-runtime";
pub const NATIVE_PROVIDER_RUNTIME_ROOT: &str =
    "omega-rust/omega/interpreter/native-provider-runtime";

pub const PERMITTED_OWNERS: &[&str] = &[
    NATIVE_ROOT,
    PROVIDER_SELECTION_ROOT,
    PROVIDER_ADMISSION_ROOT,
    "omega-rust/omega/compilation/pipeline",
    "omega-rust/omega/compilation/representations",
    "omega-rust/omega/compilation/backend",
    "omega-rust/omega/foundation",
    "omega-rust/psi/foundation",
    "omega-rust/psi/compilation/representations/terminal-psi",
    "omega-rust/psi/compilation/terminal-installation",
    "omega-rust/psi/compilation/proof/admission",
];

const PSI: &str = "omega-rust/psi";
const OMEGA: &str = "omega-rust/omega";
const PSI_PIPELINE: &str = "omega-rust/psi/compilation/pipeline";
const OMEGA_PIPELINE: &str = "omega-rust/omega/compilation/pipeline";
const PACKAGE_MANAGER: &str = "omega-rust/omega/packages/package-manager";
const PRODUCT_COORDINATOR: &str = "omega-rust/omega/compilation/compiler";
const PRODUCTS: &str = "omega-rust/omega/compilation/products";
const BUILD_EVALUATION: &str = "omega-rust/omega/build-system/build-settings";
const BUILD_COORDINATOR: &str = "omega-rust/omega/build-system/orchestration";
const CLI_APPLICATION: &str = "omega-rust/omega/application/cli";
const TRUST_MODEL: &str = "omega-rust/omega/compilation/realization/trust-model";
const REVIEW_EVIDENCE: &str = "omega-rust/omega/packages/review-evidence";
const PSI_FACADE: &str = "omega-rust/psi/compilation/psi-compiler";
const TERMINAL_PRODUCTION: &str = "omega-rust/psi/compilation/psi-compiler/terminal-production";
const SELECTED_DISPATCH: &str = "omega-rust/omega/compilation/realization/selected-dispatch";
const PROVIDER_PLANNING: &str = "omega-rust/omega/compilation/realization/provider-planning";
const REPRESENTATION_PLANNING: &str =
    "omega-rust/psi/compilation/validation/representation-planning";

const SOURCE_STAGES: &[&str] = &[
    "omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees",
    "omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees",
    "omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees",
    "omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees",
];

const PARSER_STAGES: &[&str] = &[
    "omega-rust/psi/compilation/pipeline/00_source-files-to-tokens",
    "omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees",
];

const TERMINAL_STAGES: &[&str] = &[
    "omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi",
    "omega-rust/psi/compilation/pipeline/07_lowered-psi-to-lowered-psi",
    "omega-rust/psi/compilation/pipeline/08_lowered-psi-to-terminal-psi",
];

const OMEGA_COMPILATION: &[&str] = &[
    "omega-rust/omega/compilation/pipeline",
    "omega-rust/omega/compilation/backend",
    "omega-rust/omega/compilation/representations",
    "omega-rust/omega/compilation/realization",
];

const SOURCE_SIDE_PSI: &[&str] = &[
    "omega-rust/psi/compilation/representations/syntax-trees",
    "omega-rust/psi/compilation/representations/symbol-resolved-trees",
    "omega-rust/psi/compilation/representations/typed-trees",
    "omega-rust/psi/compilation/representations/checked-trees",
    PSI_PIPELINE,
    PSI_FACADE,
    "omega-rust/psi/compilation/build-time-evaluation",
    REPRESENTATION_PLANNING,
    "omega-rust/psi/interpreter",
];

const OMEGA_STAGES: &[&str] = &[
    "omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations",
    "omega-rust/omega/compilation/pipeline/01_abstract-operations-to-abstract-operations",
    "omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations",
    "omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions",
    "omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions",
    "omega-rust/omega/compilation/pipeline/05_selected-instructions-to-register-homes",
    "omega-rust/omega/compilation/pipeline/06_register-homes-to-post-allocation-machine",
    "omega-rust/omega/compilation/pipeline/07_post-allocation-machine-to-selected-form-encoding",
    "omega-rust/omega/compilation/pipeline/08_selected-form-encoding-to-resolved-layout",
    "omega-rust/omega/compilation/pipeline/09_resolved-layout-to-resolved-layout",
];

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Allowance {
    pub owner: &'static str,
    pub dependency: &'static str,
}

const FIREWALL_ALLOWANCES: &[Allowance] = &[
    Allowance {
        owner: PROVIDER_PLANNING,
        dependency: "omega-rust/psi/compilation/representations/checked-trees",
    },
    Allowance {
        owner: SELECTED_DISPATCH,
        dependency: "omega-rust/psi/compilation/representations/typed-trees",
    },
    Allowance {
        owner: SELECTED_DISPATCH,
        dependency: "omega-rust/psi/compilation/representations/checked-trees",
    },
    Allowance {
        owner: SELECTED_DISPATCH,
        dependency: TERMINAL_PRODUCTION,
    },
    Allowance {
        owner: PROVIDER_PLANNING,
        dependency: "omega-rust/psi/compilation/build-time-evaluation",
    },
];

pub struct Rule {
    pub owners: Vec<&'static str>,
    pub forbidden: Vec<&'static str>,
    pub reason: &'static str,
    pub except_owners: Vec<&'static str>,
    pub allowed: Vec<Allowance>,
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
}

fn under(relative: &str, owners: &[&str]) -> bool {
    owners.iter().any(|owner| {
        relative == *owner
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
    let mut rules = vec![
        Rule {
            owners: vec![PSI],
            forbidden: vec![OMEGA],
            reason: "Psi must not depend on Omega",
            except_owners: vec![],
            allowed: vec![],
        },
        Rule {
            owners: vec![CLI_APPLICATION],
            forbidden: vec![PSI_PIPELINE, OMEGA_PIPELINE],
            reason: "application operations call compiler facades, not numbered transforms",
            except_owners: vec![],
            allowed: vec![],
        },
        Rule {
            owners: vec![BUILD_COORDINATOR],
            forbidden: vec![
                PSI_PIPELINE,
                OMEGA_PIPELINE,
                "omega-rust/psi/compilation/representations/syntax-trees",
                "omega-rust/psi/compilation/representations/typed-trees",
                "omega-rust/psi/compilation/representations/checked-trees",
                "omega-rust/psi/compilation/representations/lowered-psi",
                PROVIDER_PLANNING,
            ],
            reason: "Build reaches the language through product facades, not stages or representations",
            except_owners: vec![],
            allowed: vec![],
        },
        Rule {
            owners: vec![PRODUCT_COORDINATOR],
            forbidden: vec![OMEGA_PIPELINE, TERMINAL_PRODUCTION],
            reason: "the product coordinator does not run native stages or wrap Terminal production",
            except_owners: vec![],
            allowed: vec![],
        },
        Rule {
            owners: vec![PRODUCT_COORDINATOR, PRODUCTS, BUILD_EVALUATION],
            forbidden: SOURCE_STAGES.to_vec(),
            reason: "product consumers call the Psi facade and do not assemble source stages",
            except_owners: vec![],
            allowed: vec![],
        },
        Rule {
            owners: vec![
                "omega-rust/omega/packages/package-compilation",
                "omega-rust/omega/packages/build-declarations",
                PACKAGE_MANAGER,
            ],
            forbidden: PARSER_STAGES.to_vec(),
            reason: "package consumers receive parsed sources and do not run the lexer or parser",
            except_owners: vec![],
            allowed: vec![],
        },
        Rule {
            owners: vec![PACKAGE_MANAGER],
            forbidden: vec![
                "omega-rust/omega/compilation/backend/artifacts/component-description",
                PROVIDER_PLANNING,
                "omega-rust/psi/foundation/flow-effects",
                "omega-rust/psi/foundation/symbols",
            ],
            reason: "package custody and review do no checking, provider planning or product production",
            except_owners: vec![],
            allowed: vec![],
        },
        Rule {
            owners: vec![REVIEW_EVIDENCE],
            forbidden: vec![PACKAGE_MANAGER, PRODUCT_COORDINATOR],
            reason: "review evidence capture sits below package policy and the compiler",
            except_owners: vec![],
            allowed: vec![],
        },
        Rule {
            owners: vec![TRUST_MODEL],
            forbidden: vec![
                "omega-rust/psi/compilation/build-time-evaluation",
                "omega-rust/psi/interpreter/checked-interpreter",
                "omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees",
                "omega-rust/psi/compilation/terminal-installation",
                "omega-rust/psi/compilation/representations/terminal-psi",
                "omega-rust/omega/compilation/backend/target",
                "omega-rust/psi/foundation/semantic-vocabulary",
            ],
            reason: "trust admission does not reconstruct grants from source or target owners",
            except_owners: vec![],
            allowed: vec![],
        },
        Rule {
            owners: vec![PSI_FACADE],
            forbidden: TERMINAL_STAGES.to_vec(),
            reason: "Psi 06-08 are sequenced only by terminal-production",
            except_owners: vec![TERMINAL_PRODUCTION],
            allowed: vec![],
        },
        Rule {
            owners: vec![TERMINAL_PRODUCTION],
            forbidden: vec!["omega-rust/psi/compilation/build-time-evaluation"],
            reason: "Terminal production sits below compile-time evaluation, which calls it",
            except_owners: vec![],
            allowed: vec![],
        },
        Rule {
            owners: OMEGA_COMPILATION.to_vec(),
            forbidden: SOURCE_SIDE_PSI.to_vec(),
            reason: "Omega cannot depend on source IRs, source checkers or interpreters (pipeline.md \"Public contracts\"); consume Terminal Psi and Build rows",
            except_owners: vec![],
            allowed: FIREWALL_ALLOWANCES.to_vec(),
        },
    ];
    for &owner in OMEGA_STAGES {
        rules.push(Rule {
            owners: vec![owner],
            forbidden: OMEGA_STAGES
                .iter()
                .copied()
                .filter(|&other| other != owner)
                .collect(),
            reason: "stages cannot call adjacent stages to reconstruct missing input facts (pipeline.md \"Public contracts\"); put the shared representation in compilation/representations",
            except_owners: vec![],
            allowed: FIREWALL_ALLOWANCES.to_vec(),
        });
    }
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
                && under(&relative, &rule.forbidden)
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
                && under(allowed.dependency, &rule.forbidden)
        });
        if !covered {
            violations.push(format!(
                "{} -> {}: allowance names an edge no rule forbids; delete the row",
                allowed.owner, allowed.dependency
            ));
        } else if !edges.contains(&(allowed.owner.to_owned(), allowed.dependency.to_owned())) {
            violations.push(format!(
                "{} -> {}: edge removed; delete its row from FIREWALL_ALLOWANCES so it cannot return",
                allowed.owner, allowed.dependency
            ));
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

pub fn psi_omega_dependency_violations(metadata: &Metadata) -> Result<Vec<String>, String> {
    let root = Path::new(&metadata.workspace_root);
    let mut violations = Vec::new();
    for package in &metadata.packages {
        let Some(parent) = Path::new(&package.manifest_path).parent() else {
            continue;
        };
        let source = normalize_relative(root, parent)?;
        if !under(&source, &[PSI]) {
            continue;
        }
        for dependency in &package.dependencies {
            let Some(dep_path) = &dependency.path else {
                continue;
            };
            let destination = normalize_relative(root, Path::new(dep_path))?;
            if under(&destination, &[OMEGA]) {
                violations.push(format!(
                    "{} -> {}: Psi depends on Omega ({destination})",
                    package.name, dependency.name
                ));
            }
        }
    }
    Ok(violations)
}

pub fn realization_checker_entry_violations(metadata: &Metadata) -> Result<Vec<String>, String> {
    let root = Path::new(&metadata.workspace_root);
    let mut violations = Vec::new();
    for package in &metadata.packages {
        let Some(parent) = Path::new(&package.manifest_path).parent() else {
            continue;
        };
        let source = normalize_relative(root, parent)?;
        if !under(&source, &["omega-rust/omega/compilation/realization"]) {
            continue;
        }
        for dependency in &package.dependencies {
            if dependency.kind.as_deref() == Some("dev") {
                continue;
            }
            let Some(dep_path) = &dependency.path else {
                continue;
            };
            let destination = normalize_relative(root, Path::new(dep_path))?;
            if destination == "omega-rust/psi/compilation/validation" {
                violations.push(format!(
                    "{} -> {}: realization enters source checking",
                    package.name, dependency.name
                ));
            }
        }
    }
    Ok(violations)
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            out.push(path);
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

pub fn representation_spelling_violations(root: &Path) -> Vec<String> {
    let mut violations = Vec::new();
    for owner in [PSI, OMEGA] {
        let mut files = Vec::new();
        collect_rs_files(&root.join(owner), &mut files);
        for path in files {
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
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
    if !args.is_empty() {
        eprintln!("usage: cargo xtask check-boundary");
        return Err(2);
    }
    let root = crate::repo_root();
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
        let mut violations = representation_spelling_violations(&root);
        violations.extend(psi_omega_dependency_violations(&metadata)?);
        violations.extend(realization_checker_entry_violations(&metadata)?);
        violations.extend(dependency_violations(
            &metadata,
            NATIVE_ROOT,
            PERMITTED_OWNERS,
        )?);
        let provider_selection_owners = &PERMITTED_OWNERS[1..];
        violations.extend(dependency_violations(
            &metadata,
            PROVIDER_SELECTION_ROOT,
            provider_selection_owners,
        )?);
        let mut trust_owners = vec![TRUST_ROOT];
        trust_owners.extend_from_slice(&PERMITTED_OWNERS[1..]);
        violations.extend(dependency_violations(&metadata, TRUST_ROOT, &trust_owners)?);
        let mut runtime_owners = vec![
            RUNTIME_ROOT,
            NATIVE_PROVIDER_RUNTIME_ROOT,
            "omega-rust/psi/interpreter/terminal-interpreter",
        ];
        runtime_owners.extend_from_slice(&PERMITTED_OWNERS[1..]);
        violations.extend(dependency_violations(
            &metadata,
            RUNTIME_ROOT,
            &runtime_owners,
        )?);
        let mut native_provider_owners = vec![NATIVE_PROVIDER_RUNTIME_ROOT];
        native_provider_owners.extend_from_slice(&PERMITTED_OWNERS[1..]);
        violations.extend(dependency_violations(
            &metadata,
            NATIVE_PROVIDER_RUNTIME_ROOT,
            &native_provider_owners,
        )?);
        let rules = forbidden_rules();
        violations.extend(forbidden_dependency_violations(&metadata, &rules)?);
        violations.extend(stale_allowance_violations(&metadata, &rules)?);
        Ok(violations)
    };

    match check_all() {
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

    fn pkg(name: &str, rel: &str, deps: &[(&str, &str, Option<&str>)]) -> Package {
        Package {
            name: name.to_owned(),
            manifest_path: format!("/repo/{rel}/Cargo.toml"),
            dependencies: deps
                .iter()
                .map(|(dep_name, dep_rel, kind)| Dependency {
                    name: (*dep_name).to_owned(),
                    kind: kind.map(str::to_owned),
                    path: Some(format!("/repo/{dep_rel}")),
                })
                .collect(),
        }
    }

    #[test]
    fn shared_receipt_consumers_preserve_stage_boundaries() {
        for (owner, dependency, permitted) in [
            (OMEGA_STAGES[1], OMEGA_STAGES[0], false),
            (OMEGA_STAGES[2], OMEGA_STAGES[0], false),
            (OMEGA_STAGES[2], OMEGA_STAGES[1], false),
            (OMEGA_STAGES[3], OMEGA_STAGES[0], false),
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
    fn rejects_source_tree_dependency_behind_native_compiler() {
        let metadata = Metadata {
            workspace_root: "/repo".to_owned(),
            packages: vec![
                pkg(
                    "native-compiler",
                    NATIVE_ROOT,
                    &[(
                        "effects",
                        "omega-rust/omega/compilation/backend/plans/effects",
                        None,
                    )],
                ),
                pkg(
                    "effects",
                    "omega-rust/omega/compilation/backend/plans/effects",
                    &[(
                        "typed-trees",
                        "omega-rust/psi/compilation/representations/typed-trees",
                        None,
                    )],
                ),
                pkg(
                    "typed-trees",
                    "omega-rust/psi/compilation/representations/typed-trees",
                    &[],
                ),
            ],
        };
        let violations = dependency_violations(&metadata, NATIVE_ROOT, PERMITTED_OWNERS).unwrap();
        assert_eq!(violations.len(), 1);
        assert!(violations[0].contains("native-compiler -> effects -> typed-trees"));
    }

    #[test]
    fn rejects_psi_depending_on_omega() {
        let metadata = Metadata {
            workspace_root: "/repo".to_owned(),
            packages: vec![
                pkg(
                    "language-semantics",
                    "omega-rust/psi/foundation/language-semantics",
                    &[("native-compiler", NATIVE_ROOT, Some("dev"))],
                ),
                pkg("native-compiler", NATIVE_ROOT, &[]),
            ],
        };
        let violations = psi_omega_dependency_violations(&metadata).unwrap();
        assert_eq!(violations.len(), 1);
        assert!(violations[0].contains("language-semantics -> native-compiler"));
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
