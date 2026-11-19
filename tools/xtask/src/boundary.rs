//! Check native compiler and hosted execution's source-free dependencies.
//!
//! Uses Cargo's declared path dependencies (including optional, build, and
//! target-specific edges, excluding dev-dependencies for production boundaries)
//! to enforce the Psi/Omega firewall and stage boundaries.

mod linear_pipeline;

use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const NATIVE_ROOT: &str = "omega-rust/omega/compilation/native-compiler";
const PUBLISHED_PROGRAM: &str = "omega-rust/omega/compilation/published-program";
pub const PROVIDER_SELECTION_ROOT: &str =
    "omega-rust/omega/compilation/verification/provider-correspondence";
pub const PROVIDER_ADMISSION_ROOT: &str =
    "omega-rust/omega/compilation/verification/provider-admission";
pub const RUNTIME_ROOT: &str = "omega-rust/omega/interpreter/terminal-runtime";
pub const NATIVE_PROVIDER_RUNTIME_ROOT: &str =
    "omega-rust/omega/interpreter/native-provider-runtime";

pub const PERMITTED_OWNERS: &[&str] = &[
    NATIVE_ROOT,
    PROVIDER_SELECTION_ROOT,
    PROVIDER_ADMISSION_ROOT,
    "omega-rust/omega/compilation/pipeline",
    "omega-rust/omega/compilation/representations",
    "omega-rust/omega/compilation/target-description",
    "omega-rust/omega/compilation/verification",
    "omega-rust/omega/host-support/platform-custody",
    "omega-rust/psi/foundation",
    "omega-rust/psi/evaluation",
    "omega-rust/psi/interpreter",
    "omega-rust/psi/runtime",
    "omega-rust/psi/compilation/proof-admission",
    "omega-rust/psi/compilation/access-admission",
    "omega-rust/psi/compilation/terminal-installation",
    "source-rust/subsystems/compiler/psi/layout/integer-defaults",
    "omega-rust/psi/compilation/representations/terminal-psi",
    "omega-rust/psi/compilation/representations/access-vocabulary",
    "omega-rust/psi/compilation/representations/application-identity",
    "omega-rust/psi/compilation/representations/declaration-contracts",
    "omega-rust/psi/compilation/representations/declaration-identity",
    "omega-rust/psi/compilation/representations/extent-contracts",
    "source-rust/subsystems/compiler/foundation/numeric-literals",
    "omega-rust/psi/compilation/representations/optimization-policy",
    "source-rust/subsystems/compiler/psi/semantics/numeric",
    "omega-rust/psi/compilation/representations/schema-reflection",
    "omega-rust/psi/compilation/representations/semantic-vocabulary",
    "omega-rust/psi/compilation/representations/source-effects",
    "source-rust/subsystems/compiler/foundation/language-core",
    "source-rust/subsystems/compiler/psi/layout",
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
const CLI_APPLICATION: &str = "omega-rust/omega/application";
const REVIEW_EVIDENCE: &str = "omega-rust/omega/packages/review-evidence";
#[cfg(test)]
const PSI_FACADE: &str = linear_pipeline::STAGES[1];
const FRONTEND_OPERATIONS: &[&str] = &[linear_pipeline::FRONTEND[0], linear_pipeline::FRONTEND[1]];
const OMEGA_COMPILATION: &[&str] = &[
    NATIVE_ROOT,
    PUBLISHED_PROGRAM,
    PROVIDER_ADMISSION_ROOT,
    PROVIDER_SELECTION_ROOT,
    "omega-rust/omega/compilation/pipeline",
    "omega-rust/omega/compilation/representations",
    "omega-rust/omega/compilation/target-description",
    "omega-rust/omega/compilation/verification",
];
const SOURCE_SIDE_PSI: &[&str] = &[
    "omega-rust/psi/compilation/representations/bundled-declarations",
    "omega-rust/psi/compilation/representations/source-declarations",
    "omega-rust/psi/compilation/representations/symbol-resolved-trees",
    "omega-rust/psi/compilation/representations/typed-trees",
    PSI_PIPELINE,
    "omega-rust/psi/compilation/representations/source-admission",
    "omega-rust/psi/compilation/representations/syntax-trees",
    "omega-rust/psi/compilation/representations/tokens",
];

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Allowance {
    pub owner: &'static str,
    pub dependency: &'static str,
}

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
    use linear_pipeline::STAGES;
    let rule = |owners, forbidden, reason| Rule {
        owners,
        forbidden,
        reason,
        except_owners: vec![],
        allowed: vec![],
    };
    let mut rules = vec![
        rule(
            vec![CLI_APPLICATION],
            vec![PSI_PIPELINE, OMEGA_PIPELINE],
            "the CLI enters through project orchestration",
        ),
        rule(
            vec![BUILD_COORDINATOR, BUILD_EVALUATION],
            vec![
                OMEGA_PIPELINE,
                FRONTEND_OPERATIONS[0],
                FRONTEND_OPERATIONS[1],
            ],
            "Build uses the product and source entry points",
        ),
        rule(
            vec![PRODUCT_COORDINATOR],
            STAGES[3..].to_vec(),
            "the native driver orders target compilation",
        ),
        rule(
            vec![
                PRODUCT_COORDINATOR,
                PRODUCTS,
                BUILD_EVALUATION,
                PACKAGE_MANAGER,
                "omega-rust/omega/packages/package-compilation",
                "omega-rust/omega/packages/build-declarations",
            ],
            FRONTEND_OPERATIONS.to_vec(),
            "source consumers activate checked files through the streaming source owner",
        ),
        rule(
            vec![REVIEW_EVIDENCE],
            vec![PACKAGE_MANAGER, PRODUCT_COORDINATOR],
            "review evidence is captured independently of package policy and compilation",
        ),
        rule(
            OMEGA_COMPILATION.to_vec(),
            SOURCE_SIDE_PSI.to_vec(),
            "native compilation consumes canonical Psi and admitted inputs",
        ),
        rule(
            vec![PROVIDER_SELECTION_ROOT],
            vec![PSI_PIPELINE, NATIVE_ROOT],
            "provider correspondence compares admitted declarations",
        ),
    ];
    // Portable publication takes the accepted source result and ends that
    // ownership. Other native phases may never import a source operation.
    rules.push(rule(
        vec![STAGES[2]],
        FRONTEND_OPERATIONS.to_vec(),
        "portable publication takes the accepted source result",
    ));
    // Dependency edits preserve the spelling and trivia of a single Build
    // file. The text reader's production closure is checked in linear_pipeline.
    rules.push(Rule {
        allowed: vec![Allowance {
            owner: PACKAGE_MANAGER,
            dependency: linear_pipeline::TEXT_READER,
        }],
        ..rule(
            vec![PACKAGE_MANAGER],
            STAGES.to_vec(),
            "package management does not execute compiler stages",
        )
    });
    for (index, stage) in STAGES.iter().enumerate().skip(3) {
        let mut external = rule(
            vec![""],
            vec![stage],
            "only the native driver and the next stage consume a stage API in production",
        );
        external.except_owners.extend([
            linear_pipeline::TEST_SUPPORT,
            NATIVE_ROOT,
            *stage,
            "omega-rust/omega/compilation/verification",
        ]);
        external
            .except_owners
            .extend(STAGES.get(index + 1).copied());
        rules.push(external);
    }
    rules.push(rule(
        STAGES[7..=11].to_vec(),
        vec![EARLIER_PROGRAM_FORMS, SEALED_TERMINAL_MODULE],
        "physical planning reads the retained target operations",
    ));
    rules.push(rule(
        vec![FRONTEND_OPERATIONS[0]],
        vec![FRONTEND_OPERATIONS[1], linear_pipeline::FRONTEND[3]],
        "the lexer produces tokens",
    ));
    rules.push(rule(
        vec![FRONTEND_OPERATIONS[1]],
        vec![FRONTEND_OPERATIONS[0]],
        "the parser consumes token streams",
    ));
    rules.push(rule(
        vec!["omega-rust/psi/compilation/representations/terminal-psi/check-tables"],
        vec![PSI_PIPELINE],
        "checker records cannot depend on a source producer",
    ));
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
    "omega-rust/omega/compilation/representations/abstract-operations";

/// Stage 02's working program: `TargetOperationPlan` and its control graph.
/// The records it shares with later forms live in `target-vocabulary`, nested
/// in this directory as a separate package.
const STAGE_02_WORKING_FORM: &str =
    "omega-rust/omega/compilation/representations/target-operations";

/// Instruction selection, the installation record it is handed, and the forms
/// from the prepared plan onward. None of them is handed stage 02's working
/// program, so none may be able to name it.
const AFTER_PREPARATION: &[&str] = &[
    "omega-rust/omega/compilation/representations/bound-installations",
    "omega-rust/omega/compilation/representations/target-operations/target-vocabulary",
    "omega-rust/omega/compilation/representations/target-operations/legalized-operations",
    "omega-rust/omega/compilation/representations/selected-instructions",
    "omega-rust/omega/compilation/representations/selected-instructions/analyses",
    "omega-rust/omega/compilation/representations/register-homes",
    "omega-rust/omega/compilation/representations/post-allocation-machine",
    "omega-rust/omega/compilation/representations/machine-code",
    "omega-rust/omega/compilation/target-description/register-environment",
    "omega-rust/omega/compilation/pipeline/12_bound-framed-machine-to-selected-form-encoding",
    "omega-rust/omega/compilation/pipeline/13_selected-form-encoding-to-resolved-layout",
    "omega-rust/omega/compilation/pipeline/14_resolved-layout-to-resolved-layout",
    "omega-rust/omega/compilation/pipeline/15_resolved-layout-to-placed-functions",
];

/// The crate that defines the sealed Terminal Psi module
/// (`ClosedTerminalModule`) and the canonical artifact. Native forms use the
/// canonical vocabulary encoders from `terminal-wire`, which `terminal-codec`
/// re-exports; reaching `terminal-codec` would let them name the sealed module.
const SEALED_TERMINAL_MODULE: &str =
    "omega-rust/psi/compilation/representations/terminal-psi/codec";

/// Selection and later forms carry shared contracts, not the prepared plan's
/// instruction graph. Follow indirect production edges as well as direct ones.
const AFTER_SELECTION: &[&str] = &[
    "omega-rust/omega/compilation/representations/selected-instructions",
    "omega-rust/omega/compilation/representations/selected-instructions/analyses",
    "omega-rust/omega/compilation/representations/register-homes",
    "omega-rust/omega/compilation/representations/post-allocation-machine",
    "omega-rust/omega/compilation/representations/machine-code",
    "omega-rust/omega/compilation/pipeline/12_bound-framed-machine-to-selected-form-encoding",
    "omega-rust/omega/compilation/pipeline/13_selected-form-encoding-to-resolved-layout",
    "omega-rust/omega/compilation/pipeline/14_resolved-layout-to-resolved-layout",
    "omega-rust/omega/compilation/pipeline/15_resolved-layout-to-placed-functions",
];

const PREPARED_EXECUTION_PLAN: &str =
    "omega-rust/omega/compilation/representations/target-operations/legalized-operations";

/// Instruction selection, the forms from the prepared plan onward, the stages
/// that turn one into the next, the target setup and installation record they
/// share, and the leaf vocabulary they carry. Instruction selection is handed
/// only the prepared execution plan and its bound installations; none of these
/// crates may reach an earlier program form. The check follows every
/// production edge, so a new path through a helper crate is caught too.
const SELECTION_REPRESENTATIONS: &[&str] = &[
    "omega-rust/omega/compilation/representations/bound-installations",
    "omega-rust/omega/compilation/representations/native-vocabulary",
    "omega-rust/omega/compilation/representations/target-operations",
    "omega-rust/omega/compilation/representations/target-operations/legalized-operations",
    "omega-rust/omega/compilation/representations/selected-instructions",
    "omega-rust/omega/compilation/representations/selected-instructions/analyses",
    "omega-rust/omega/compilation/representations/register-homes",
    "omega-rust/omega/compilation/representations/post-allocation-machine",
    "omega-rust/omega/compilation/representations/machine-code",
    "omega-rust/omega/compilation/target-description/register-environment",
    "omega-rust/omega/compilation/pipeline/12_bound-framed-machine-to-selected-form-encoding",
    "omega-rust/omega/compilation/pipeline/13_selected-form-encoding-to-resolved-layout",
    "omega-rust/omega/compilation/pipeline/14_resolved-layout-to-resolved-layout",
    "omega-rust/omega/compilation/pipeline/15_resolved-layout-to-placed-functions",
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
                continue;
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

/// The entry points of stage 02's validator.
const PLAN_VALIDATOR_ENTRIES: &[&str] = &[
    "verify_execution_plan(",
    "verify_target_operation_plan_with_closed_module(",
];
/// The crate that defines the validator, and the one production caller: the
/// explicit stage-validator request (`omega --validator ...`).
const PLAN_VALIDATOR_CALLERS: &[&str] = &[
    "omega-rust/omega/compilation/pipeline/05_abstract-operations-to-target-operations/src/preparation/",
    "source-rust/subsystems/compiler/psi-to-native/replay/published-objects/src/lib.rs",
];

/// Stage 02's accessor for its working program.
const WORKING_PROGRAM_ACCESSORS: &[&str] = &["working_program_for_validation"];
const TARGET_REPLAY_RECORD: &str = "omega-rust/omega/compilation/representations/abstract-operations/verified-optimization-input/src/publication/target_replay.rs";
/// Binding belongs to the driver after preparation, not to setters on the
/// prepared stage output. Neither the old definitions nor callers may return.
const RETIRED_INSTALLATION_SETTERS: &[&str] = &[
    "with_installed_entry_execution",
    "with_installed_stack_requirements",
];
/// Stage 02 and its preparation crate, and the one file through which image
/// emission's replay validators read the working program.
const WORKING_PROGRAM_READERS: &[&str] = &[
    "omega-rust/omega/compilation/pipeline/05_abstract-operations-to-target-operations/src/target_lowering/",
    "omega-rust/omega/compilation/pipeline/17_objects-to-image/image-emission/src/retained_working_program.rs",
    "source-rust/subsystems/compiler/psi-to-native/replay/published-objects/src/lib.rs",
];

/// Naming either gateway is restricted, so importing it under another name
/// cannot give object production access to stage 02's working program.
const IMAGE_REPLAY_GATEWAYS: &[&str] = &["retained_working_program", "retained_source_validation"];
const IMAGE_REPLAY_READERS: &[&str] = &[
    "omega-rust/omega/compilation/pipeline/17_objects-to-image/image-emission/src/retained_working_program.rs",
    "omega-rust/omega/compilation/pipeline/17_objects-to-image/image-emission/src/function_fragments/source/retained_source_validation.rs",
    "omega-rust/omega/compilation/pipeline/17_objects-to-image/image-emission/src/function_fragments/validation.rs",
];
const IMAGE_REPLAY_DECLARATIONS: &[(&str, &str)] = &[
    (
        "omega-rust/omega/compilation/pipeline/17_objects-to-image/image-emission/src/lib.rs",
        "mod retained_working_program;",
    ),
    (
        "omega-rust/omega/compilation/pipeline/17_objects-to-image/image-emission/src/function_fragments.rs",
        "pub(super) mod retained_source_validation;",
    ),
];

fn is_test_source(relative: &str) -> bool {
    relative.contains("/tests/") || relative.ends_with("tests.rs")
}

fn line_calls_any(line: &str, entries: &[&str]) -> bool {
    let code = line.trim_start();
    !code.starts_with("//") && entries.iter().any(|entry| code.contains(entry))
}

/// Report each line under `omega-rust/omega` that calls one of `entries`
/// outside the files or directories in `allowed` and outside tests.
fn call_violations(
    root: &Path,
    entries: &[&str],
    allowed: &[&str],
    declarations: &[(&str, &str)],
    reason: &str,
) -> Vec<String> {
    call_violations_in(root, OMEGA, entries, allowed, declarations, reason)
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
    collect_rs_files(&root.join(scope), &mut files);
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
        let Ok(text) = fs::read_to_string(&path) else {
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
        "omega-rust/omega/compilation/pipeline/06_target-operations-to-selected-instructions/src/instruction_selection/optimized.rs",
        "stage_optimized_instruction_selection",
        1,
    ),
    (
        "omega-rust/omega/compilation/pipeline/10_post-allocation-machine-to-framed-machine/src/lib.rs",
        "plan_function_frame",
        1,
    ),
    (
        "omega-rust/omega/compilation/pipeline/12_bound-framed-machine-to-selected-form-encoding/src/lib.rs",
        "encode_bound_function_body",
        1,
    ),
    (
        "source-rust/subsystems/compiler/psi-to-native/pipeline/representations/machine-code/encoding-admission/src/lib.rs",
        "validate_optimized_layout_independent_selected_form_encoding",
        1,
    ),
    (
        "omega-rust/omega/compilation/pipeline/13_selected-form-encoding-to-resolved-layout/src/lib.rs",
        "place_encoded_program",
        1,
    ),
    (
        "omega-rust/omega/compilation/pipeline/14_resolved-layout-to-resolved-layout/src/lib.rs",
        "relax_resolved_layout",
        1,
    ),
    (
        "source-rust/subsystems/compiler/psi-to-native/pipeline/representations/machine-code/encoding-admission/src/layout/mod.rs",
        "validate_optimized_resolved_selected_form_layout",
        1,
    ),
    (
        "omega-rust/omega/compilation/pipeline/15_resolved-layout-to-placed-functions/src/function_realization.rs",
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

/// Ordinary compilation does not re-verify its own stage output
/// (omega-rust/pipeline.md, "Verification placement"). Stage 02's validator
/// derives the execution plan a second time, so a call to it from the default
/// path would both hide a preparation bug and double the stage's work. Only
/// the explicit validator request and tests may call it.
pub fn plan_validator_call_violations(root: &Path) -> Vec<String> {
    call_violations(
        root,
        PLAN_VALIDATOR_ENTRIES,
        PLAN_VALIDATOR_CALLERS,
        &[],
        "stage 02's plan validator is called outside the explicit validator request and tests",
    )
}

/// Stage 02 publishes the prepared execution plan; its working program is
/// scratch that only stage 02 and validators of its output may read. A later
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
        "stage 02's working program is read outside stage 02, its validators and tests; read the prepared execution plan",
    )
}

/// Package review reads issued source demand, and an audited image reads its
/// own frozen target. Only the original publishing operations can issue them.
fn retained_publication_violations(root: &Path) -> Vec<String> {
    let mut errors = call_violations_in(
        root,
        "omega-rust",
        &["publish_package_review_numeric_requirements"],
        &[
            "omega-rust/psi/compilation/pipeline/07_checked-terminal-psi-to-published-psi/src/provider_supply/package_review.rs",
        ],
        &[(
            "omega-rust/psi/compilation/pipeline/07_checked-terminal-psi-to-published-psi/src/package_review.rs",
            "pub(crate) fn publish_package_review_numeric_requirements(",
        )],
        "package numeric review demand can only be issued when publishing the accepted provider request",
    );
    errors.extend(call_violations(root, &["retain_selected_body_facts"], &["omega-rust/omega/compilation/pipeline/06_target-operations-to-selected-instructions/src/instruction_selection/optimized.rs"], &[("omega-rust/omega/compilation/representations/physical-program/src/lib.rs", "pub fn retain_selected_body_facts(")], "only instruction selection may issue the joined body facts"));
    errors.extend(call_violations(root, &["into_framed_body_facts"], &["omega-rust/omega/compilation/pipeline/10_post-allocation-machine-to-framed-machine/entry-planning/src/imports.rs"], &[("omega-rust/omega/compilation/representations/physical-program/src/lib.rs", "pub fn into_framed_body_facts(")], "only frame publication may open the carried body facts"));
    errors.extend(call_violations(
        root,
        &["retain_target_operations"],
        &["omega-rust/omega/compilation/pipeline/05_abstract-operations-to-target-operations/src/target_lowering/mod.rs"],
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
            "omega-rust/omega/compilation/native-compiler/src/physical_publication_tests.rs",
            "omega-rust/omega/compilation/pipeline/17_objects-to-image/image-emission/src/retained_working_program.rs",
            "omega-rust/omega/compilation/pipeline/17_objects-to-image/image-emission/src/function_fragments/source/retained_source_validation.rs",
            "source-rust/subsystems/compiler/psi-to-native/replay/published-objects/src/lib.rs",
        ],
        &[
            (TARGET_REPLAY_RECORD, "pub fn target_for_correspondence(&self) -> &TargetOperationsForReplay {"),
            ("omega-rust/omega/compilation/pipeline/17_objects-to-image/image-emission/src/object_artifact.rs", ".map(crate::RetainedTargetOperations::target_for_correspondence)"),
        ],
        "only explicit object correspondence audits may inspect the original target forms",
    ));
    errors
}

/// Production must not reach stage 02 through a replay helper or its import.
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
/// root, not the stage 02 execution plan or stage 01 optimization output.
pub fn backend_execution_plan_read_violations(root: &Path) -> Vec<String> {
    call_violations_in(
        root,
        "omega-rust/omega/compilation/pipeline/17_objects-to-image",
        &["execution_plan", "pre_physical_manifest("],
        &[
            "source-rust/subsystems/compiler/psi-to-native/replay/published-objects/src/lib.rs",
            "omega-rust/omega/compilation/pipeline/17_objects-to-image/image-emission/src/function_fragments/source/retained_source_validation.rs",
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
        let mut violations = linear_pipeline::violations(&metadata)?;
        violations.extend(representation_spelling_violations(&root));
        violations.extend(crate::boundary_contracts::violations(
            &root,
            &root.join("omega-rust/pipeline-contracts.json"),
        )?);
        violations.extend(plan_validator_call_violations(&root));
        violations.extend(single_form_entry_violations(&root));
        violations.extend(working_program_read_violations(&root));
        violations.extend(retained_publication_violations(&root));
        violations.extend(image_replay_read_violations(&root));
        violations.extend(backend_execution_plan_read_violations(&root));
        violations.extend(installation_mutation_violations(&root));
        violations.extend(transitive_forbidden_violations(
            &metadata,
            SELECTION_REPRESENTATIONS,
            EARLIER_PROGRAM_FORMS,
            "a representation instruction selection reads or writes reaches an earlier program form; move the shared leaf type to native-vocabulary",
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
            "a native form reaches the sealed Terminal Psi module; name the encoder from terminal-wire",
        )?);
        violations.extend(psi_omega_dependency_violations(&metadata)?);
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
        // Native planning now owns trust reconstruction as a private operation.
        violations.extend(dependency_violations(
            &metadata,
            linear_pipeline::STAGES[3],
            provider_selection_owners,
        )?);
        let mut runtime_owners = vec![
            RUNTIME_ROOT,
            NATIVE_PROVIDER_RUNTIME_ROOT,
            "omega-rust/omega/host-support/bounded-process",
            "omega-rust/psi/interpreter",
        ];
        runtime_owners.extend_from_slice(&PERMITTED_OWNERS[1..]);
        violations.extend(dependency_violations(
            &metadata,
            RUNTIME_ROOT,
            &runtime_owners,
        )?);
        let mut published_program_owners = runtime_owners.clone();
        published_program_owners.extend([
            NATIVE_ROOT,
            PUBLISHED_PROGRAM,
            "omega-rust/omega/compilation/products/optimization-requests",
        ]);
        violations.extend(dependency_violations(
            &metadata,
            PUBLISHED_PROGRAM,
            &published_program_owners,
        )?);
        let mut native_provider_owners = vec![
            NATIVE_PROVIDER_RUNTIME_ROOT,
            "omega-rust/omega/host-support/bounded-process",
        ];
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
                    features: Vec::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn facade_boundaries_follow_dependency_paths_not_source_or_alias_spelling() {
        for (owner, stage, facade) in [
            (CLI_APPLICATION, FRONTEND_OPERATIONS[0], BUILD_COORDINATOR),
            (
                BUILD_COORDINATOR,
                linear_pipeline::STAGES[3],
                PRODUCT_COORDINATOR,
            ),
            (PRODUCT_COORDINATOR, FRONTEND_OPERATIONS[0], PSI_FACADE),
            (
                "samples/scientific-driver",
                linear_pipeline::STAGES[3],
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
    fn only_the_native_driver_runs_omega_stages() {
        const LIBRARY: &str =
            "omega-rust/omega/compilation/pipeline/15_resolved-layout-to-placed-functions";
        const PACKAGES: &str = "omega-rust/omega/packages/package-manager";
        for (owner, dependency, kind, permitted) in [
            (NATIVE_ROOT, linear_pipeline::STAGES[9], None, true),
            (LIBRARY, linear_pipeline::STAGES[4], None, false),
            (LIBRARY, linear_pipeline::STAGES[4], Some("build"), false),
            (LIBRARY, linear_pipeline::STAGES[4], Some("dev"), true),
            (PACKAGES, linear_pipeline::STAGES[3], None, false),
            (PACKAGES, linear_pipeline::STAGES[1], None, false),
            (PACKAGES, linear_pipeline::TEXT_READER, None, true),
            (LIBRARY, linear_pipeline::STAGES[3], None, false),
            // Machine emission reads stage 09's output as a representation.
            (LIBRARY, linear_pipeline::STAGES[9], None, false),
            // Image writers must not reach back to select instructions.
            (
                "omega-rust/omega/compilation/pipeline/17_objects-to-image/image-emission",
                linear_pipeline::STAGES[4],
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
            (linear_pipeline::STAGES[4], linear_pipeline::STAGES[3], true),
            (
                linear_pipeline::STAGES[5],
                linear_pipeline::STAGES[3],
                false,
            ),
            (linear_pipeline::STAGES[5], linear_pipeline::STAGES[4], true),
            (
                linear_pipeline::STAGES[6],
                linear_pipeline::STAGES[3],
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
            "omega-rust/omega/compilation/representations/target-operations/legalized-operations";
        let helper = "omega-rust/omega/compilation/backend/helper";
        let unit =
            "omega-rust/omega/compilation/representations/abstract-operations/optimization-unit";
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
    fn a_nested_package_is_not_the_crate_it_sits_in() {
        let form = "omega-rust/omega/compilation/representations/machine-code";
        let working = STAGE_02_WORKING_FORM;
        let vocabulary =
            "omega-rust/omega/compilation/representations/target-operations/target-vocabulary";
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
            "omega-rust/omega/compilation/representations/target-operations/legalized-operations";
        let helper = "omega-rust/omega/compilation/backend/helper";
        let wire = "omega-rust/psi/compilation/representations/terminal-psi/wire";
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
            "omega-rust/omega/compilation/native-compiler/src/physical_publication_tests.rs"
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
            "omega-rust/omega/compilation/compiler/tests/executed_fixture_discovery.rs"
        ));
        assert!(!is_test_source(
            "omega-rust/omega/compilation/native-compiler/src/pipeline.rs"
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
        let image = "omega-rust/omega/compilation/pipeline/17_objects-to-image/image-emission/src";
        let files = [
            (
                "omega-rust/omega/packages/review-evidence/src/projection.rs".to_owned(),
                "let recalculate = AcceptedTerminalSource::publish_package_review_numeric_requirements;\n\
                 let pair = SemanticEmissionFacts::retain_target_operations;\n\
                 let read = RetainedTargetOperations::target_for_correspondence;\n\
                 let retained = ObjectArtifact::retained_target_operations;\n",
            ),
            (
                "source-rust/subsystems/compiler/psi-to-native/replay/published-objects/src/lib.rs".to_owned(),
                "current.working_program_for_validation();\ncurrent.execution_plan();\n",
            ),
            (
                "omega-rust/omega/compilation/native-compiler/src/pipeline.rs".to_owned(),
                "let pair = SemanticEmissionFacts::retain_target_operations;\n",
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
            5,
            "{publication_violations:?}"
        );
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
        const ABSTRACT: &str = "omega-rust/omega/compilation/representations/abstract-operations";
        for (dependency, kind, permitted) in [
            (ABSTRACT, None, false),
            (
                "omega-rust/omega/compilation/representations/abstract-operations/optimization-unit",
                None,
                false,
            ),
            (
                "omega-rust/omega/compilation/representations/abstract-operations/optimization-unit-semantics",
                None,
                false,
            ),
            (
                "omega-rust/psi/compilation/representations/terminal-psi/codec",
                None,
                false,
            ),
            // Tests may build a plan the way stage 02 does.
            (ABSTRACT, Some("dev"), true),
            (
                "omega-rust/omega/compilation/representations/target-operations/legalized-operations",
                None,
                true,
            ),
            (
                "omega-rust/omega/compilation/representations/native-vocabulary",
                None,
                true,
            ),
        ] {
            let metadata = Metadata {
                workspace_root: "/repo".to_owned(),
                packages: vec![pkg(
                    "selection",
                    linear_pipeline::STAGES[7],
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
                "omega-rust/psi/compilation/representations/terminal-psi/check-tables",
                "omega-rust/psi/compilation/representations/semantic-vocabulary",
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
    fn rejects_source_tree_dependency_behind_native_compiler() {
        let metadata = Metadata {
            workspace_root: "/repo".to_owned(),
            packages: vec![
                pkg(
                    "native-compiler",
                    NATIVE_ROOT,
                    &[(
                        "effects",
                        "omega-rust/omega/compilation/representations/native-effects",
                        None,
                    )],
                ),
                pkg(
                    "effects",
                    "omega-rust/omega/compilation/representations/native-effects",
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
