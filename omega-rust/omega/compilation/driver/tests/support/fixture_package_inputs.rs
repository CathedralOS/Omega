//! The fixture package-inputs harness shared by the compiler's test targets.
//!
//! Every test target that compiles a repository fixture, a sample or a scratch
//! project against the bundled standard library reads its package mode from
//! here, so one projection decides what a fixture depends on and one match
//! over the target vocabulary decides which reviewed entry it accepts:
//!
//! - [`fixture_path_dependencies`] projects the `depend`/`depend_as`/
//!   `build_depend*` rows a fixture authored in its own `build.omg`;
//! - [`repository_fixture_package_inputs`] binds that closure with the
//!   compilation root at marker 1 and the bundled standard library at
//!   marker 2, the identity entry and dangerous-service acceptance name;
//! - [`reviewed_repository_fixture_package_inputs`] adds this repository's
//!   test acceptance: the target profile's reviewed `ProgramEntry` candidate
//!   and the selected package-owned services the preliminary checked
//!   graph requires;
//! - the `*_build` writers restate a fixture's authored rows for a scratch
//!   copy of its source, through absolute paths, so no test target types the
//!   standard library's location into a synthesized `build.omg`.
//!
//! A test target that must bind the standard library for a project which
//! authors no dependency row — a scratch package with no `build.omg`, or a
//! relocated copy of the library — says so through
//! [`standard_library_package_inputs`] instead of restating the graph.
//!
//! Acceptance here is test policy, not evidence that an audit occurred and not
//! production accepted-lock recovery; every admitted row is derived from and
//! replayed against the exact preliminary checked graph.

// Shared across test targets through `#[path]`; each target uses the subset
// its fixtures need.
#![allow(dead_code)]

use build_declarations::{
    BuildDeclaration, BuildDeclarationKind, DependencyPurpose, extract_build_declaration,
    is_dependency_call_name, project_build_entry_syntax, project_dependency_rows,
};
use compiler::{CheckedCompileRequest, compile_to_checked};
use diagnostics::Diagnostic;
use package_compilation::{
    AcceptedSemanticBinding, AcceptedSemanticBindingRole, PackageCompilationInputs,
    PackageDependencyBinding, PackageSourceBinding,
};
use semantic_vocabulary::PackageKeyIdentity;
use source_files_to_tokens::Lexer;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::statement::StatementNode;
use target::TargetProfile;
use tokens_to_syntax_trees::parse_syntax_trees;

#[path = "console_acceptance.rs"]
pub mod console_acceptance;
#[path = "dangerous_service_acceptance.rs"]
pub mod dangerous_service_acceptance;
#[path = "linux_entry_acceptance.rs"]
pub mod linux_entry_acceptance;
#[path = "macos_entry_acceptance.rs"]
pub mod macos_entry_acceptance;
#[path = "process_exit_acceptance.rs"]
pub mod process_exit_acceptance;
#[path = "uefi_entry_acceptance.rs"]
pub mod uefi_entry_acceptance;
#[path = "windows_entry_acceptance.rs"]
pub mod windows_entry_acceptance;

pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler crate should live under omega-rust/omega/compilation/driver")
        .to_path_buf()
}

pub fn fixture_package_identity(marker: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([marker; 32])
        .expect("repository fixture package identity is nonzero")
}

/// The bundled standard library's root.
///
/// Package mode is never decided from this path. It is named here only so the
/// standard library keeps one fixed fixture identity (marker 2), which the
/// entry and dangerous-service acceptance below must be able to name, and so
/// a scratch project that binds the library by hand names the same root.
pub fn bundled_standard_library_root() -> PathBuf {
    repo_root().join("source/library/std")
}

/// The bundled standard library's root as a `Source::Path` location literal:
/// absolute, with forward slashes on every host.
pub fn bundled_standard_library_location() -> String {
    bundled_standard_library_root()
        .to_string_lossy()
        .replace('\\', "/")
}

/// The `Build::depend` row a synthesized `build.omg` authors to take the
/// bundled standard library as an ordinary product dependency, indented as a
/// build-entry statement and newline-terminated.
pub fn bundled_standard_library_dependency_declaration() -> String {
    format!(
        "    Build::depend(builder, Source::Path {{ location: \"{}\" }});\n",
        bundled_standard_library_location()
    )
}

/// The `Build::depend_as` row that takes the bundled standard library under
/// an authored `alias`.
pub fn bundled_standard_library_dependency_declaration_as(alias: &str) -> String {
    format!(
        "    Build::depend_as(builder, \"{alias}\", Source::Path {{ location: \"{}\" }});\n",
        bundled_standard_library_location()
    )
}

/// One dependency a fixture authored in its own `build.omg`, resolved against
/// the project that declared it.
#[derive(Debug)]
pub struct FixturePathDependency {
    /// The alias the requester imports this dependency under: the explicit
    /// `depend_as` literal when the fixture authored one, otherwise the
    /// dependency package's declared name with `-` replaced by `_`.
    pub alias: String,
    /// Whether the fixture authored the alias itself.
    pub explicit_alias: bool,
    /// The name the dependency's own `build.omg` declares.
    pub package_name: String,
    /// The `Source::Path` location literal exactly as the fixture authored it.
    pub authored_location: String,
    /// The dependency root the authored location resolved to, canonicalized.
    pub location: PathBuf,
    /// Which scope the authored row authorizes.
    pub purpose: DependencyPurpose,
}

/// The reader's entry to fixture package mode: exactly the dependencies a
/// fixture authored, read through the build vocabulary that owns them.
///
/// Dependency rows extend the package identity authored by `Build::package`
/// or `Build::application`. A project with no declaration and no dependency
/// row compiles without package inputs. Every row must
/// name `Source::Path { location: "<path>" }`; that location must resolve
/// relative to the declaring project; and the directory it names must itself
/// declare `Build::package(builder, ...)`. Nothing else is admitted: the harness never
/// supplies a location, a package name or an alias the fixture did not author,
/// and every such departure panics with the fixture and the authored text
/// rather than quietly dropping the edge. Conditional `*_when` vocabulary
/// never projects a row, so a fixture that authors it is rejected here too.
///
/// A `build.omg` that does not lex, parse, or project a build entry or its
/// rows is not the harness's to judge: the compiler rejects it with the
/// diagnostic the fixture exists to pin (`fail/build/*`), so such a fixture
/// wires nothing and compiles without package inputs.
pub fn fixture_path_dependencies(project_root: &Path) -> Vec<FixturePathDependency> {
    let Ok(source) = fs::read_to_string(project_root.join("build.omg")) else {
        return Vec::new();
    };
    let Ok(tokens) = Lexer::new(&source).tokenize() else {
        return Vec::new();
    };
    let Ok(trees) = parse_syntax_trees(&tokens) else {
        return Vec::new();
    };
    let Ok(build) = project_build_entry_syntax(&trees) else {
        return Vec::new();
    };
    let Ok(rows) = project_dependency_rows(&trees, &build) else {
        return Vec::new();
    };
    // Conditional `*_when` vocabulary is recognized but never projects a row,
    // so a fixture authoring it would compile without that edge; refuse it the
    // way the package manager does, by name, over the entry's own statements.
    let entry = trees.items.state(build.build_entry());
    for statement_handle in trees.items.statements(entry.statements) {
        let StatementNode::Call(call) = trees.statements.statement(*statement_handle) else {
            continue;
        };
        assert!(
            !is_dependency_call_name(call.target.as_str())
                || rows.iter().any(|row| row.statement() == *statement_handle),
            "fixture {} authors `{}`, conditional dependency vocabulary that projects no row the harness could wire",
            project_root.display(),
            call.target.as_str()
        );
    }
    rows.into_iter()
        .map(|row| {
            let (authored_location, location) =
                fixture_dependency_location(&trees, project_root, row.source());
            let package_name = fixture_dependency_package_name(project_root, &location);
            let authored_alias = row
                .alias()
                .map(|handle| fixture_build_string_literal(&trees, project_root, handle));
            FixturePathDependency {
                alias: authored_alias
                    .clone()
                    .unwrap_or_else(|| package_name.clone()),
                explicit_alias: authored_alias.is_some(),
                package_name,
                authored_location,
                location,
                purpose: row.purpose(),
            }
        })
        .collect()
}

/// Read one authored `Source::Path { location }` and resolve it against the
/// declaring project, returning the literal as authored beside its resolved
/// root. Every other source kind is a fixture the harness cannot acquire, so
/// it fails loudly instead of compiling without the edge.
fn fixture_dependency_location(
    trees: &SyntaxTrees,
    project_root: &Path,
    source: ExpressionHandle,
) -> (String, PathBuf) {
    let ExpressionNode::StructLiteral(literal) = trees.expressions.expression(source) else {
        panic!(
            "fixture {} declares an unsupported dependency source: the canary harness wires only a direct `Source::Path` literal",
            project_root.display()
        )
    };
    let constructor = literal.constructor_name.as_str();
    let (type_name, case_name) = constructor
        .rsplit_once("::")
        .map_or((constructor, None), |(owner, case)| (owner, Some(case)));
    assert!(
        type_name == "Source" && case_name == Some("Path"),
        "fixture {} declares an unsupported dependency source `{constructor}`: the canary harness wires only `Source::Path`",
        project_root.display()
    );
    let [field] = trees.expressions.struct_fields(literal.fields) else {
        panic!(
            "fixture {}: `Source::Path` takes exactly a `location` field",
            project_root.display()
        )
    };
    assert_eq!(
        field.name.as_str(),
        "location",
        "fixture {}: `Source::Path` takes exactly a `location` field",
        project_root.display()
    );
    let authored = fixture_build_string_literal(trees, project_root, field.value);
    let location = fs::canonicalize(project_root.join(&authored)).unwrap_or_else(|error| {
        panic!(
            "fixture {} declares dependency location `{authored}`, which does not resolve: {error}",
            project_root.display()
        )
    });
    (authored, location)
}

/// The dependency's own declared package name. A fixture cannot depend on a
/// directory that declares no package, and the harness never types the name in.
fn fixture_dependency_package_name(project_root: &Path, location: &Path) -> String {
    match extract_build_declaration(location) {
        Ok(BuildDeclaration::Package(package)) => package.name.into_string(),
        Ok(_) => panic!(
            "fixture {} depends on {}, which declares no package",
            project_root.display(),
            location.display()
        ),
        Err(error) => panic!(
            "fixture {} depends on {}: {error}",
            project_root.display(),
            location.display()
        ),
    }
}

fn fixture_build_string_literal(
    trees: &SyntaxTrees,
    project_root: &Path,
    handle: ExpressionHandle,
) -> String {
    let ExpressionNode::String(bytes) = trees.expressions.expression(handle) else {
        panic!(
            "fixture {}: dependency arguments must be direct string literals",
            project_root.display()
        )
    };
    std::str::from_utf8(bytes)
        .unwrap_or_else(|_| {
            panic!(
                "fixture {}: dependency literal is not utf8",
                project_root.display()
            )
        })
        .to_owned()
}

/// Restate a fixture's authored dependency rows for a scratch copy of its
/// source.
///
/// A scratch copy loses the fixture's directory, so each authored location is
/// restated as the absolute path it resolved to; the operation and an
/// explicitly authored alias are preserved exactly as the fixture wrote them.
pub fn fixture_dependency_declarations(project_root: &Path) -> String {
    fixture_path_dependencies(project_root)
        .iter()
        .map(|dependency| {
            let location = dependency.location.to_string_lossy().replace('\\', "/");
            let operation = if dependency.purpose.is_product() {
                "depend"
            } else {
                "build_depend"
            };
            if dependency.explicit_alias {
                format!(
                    "    builder.{operation}_as(\"{}\", Source::Path {{\n        location: \"{location}\"\n    }});\n",
                    dependency.alias
                )
            } else {
                format!(
                    "    builder.{operation}(Source::Path {{\n        location: \"{location}\"\n    }});\n"
                )
            }
        })
        .collect()
}

/// The root package a fixture's own build declaration names, with its role.
fn fixture_root_declaration(project_root: &Path) -> (BuildDeclarationKind, String) {
    let declaration = extract_build_declaration(project_root)
        .unwrap_or_else(|error| panic!("fixture {}: {error}", project_root.display()));
    let root_role = declaration.kind();
    let root_name = match declaration {
        BuildDeclaration::Application(application) => application.name,
        BuildDeclaration::Package(package) => package.name,
        BuildDeclaration::Workspace(_) => {
            panic!(
                "fixture {} cannot be a workspace root",
                project_root.display()
            )
        }
    };
    (root_role, root_name.into_string())
}

/// Package inputs for a repository fixture, or `None` when it authored no
/// dependency at all and did not request package identity. A fixture that
/// declares `Build::package` or `Build::application` compiles in package mode
/// even with an empty dependency set.
///
/// Identities: the compilation root takes marker 1 and the bundled standard
/// library marker 2 — pinned so entry and dangerous-service acceptance can
/// name it — and every other package takes the next free marker in the order
/// the walk reaches it. Edges project transitively: a dependency's own
/// authored rows join the same inputs. Product edges authorize imports for
/// every package in scope, while a build edge authorizes only the compilation
/// root's build entry, exactly as the package manager's compiler input does,
/// because a dependency's build context resolves in its own compilation where
/// that package is the root.
pub fn repository_fixture_package_inputs(root_path: &Path) -> Option<PackageCompilationInputs> {
    let project_root = root_path
        .parent()
        .expect("fixture source has a project root");
    fixture_package_inputs_bound_to(project_root, project_root)
}

/// Package inputs for a copy of a repository fixture staged under another
/// directory: the rows are read from the fixture's authored project, where
/// their relative locations resolve, and the root package is bound to the
/// copy. `None` when the fixture authored no dependency or project declaration.
pub fn copied_fixture_package_inputs(
    authored_project_root: &Path,
    copied_project_root: &Path,
) -> Option<PackageCompilationInputs> {
    fixture_package_inputs_bound_to(authored_project_root, copied_project_root)
}

/// Package inputs binding only the compilation root, for a fixture that
/// authored no dependency but is still compiled in package mode.
pub fn dependency_free_fixture_package_inputs(root_path: &Path) -> PackageCompilationInputs {
    let project_root = root_path
        .parent()
        .expect("fixture source has a project root");
    let (root_role, root_name) = fixture_root_declaration(project_root);
    dependency_free_inputs(project_root, project_root, root_role, root_name)
}

fn dependency_free_inputs(
    authored_root: &Path,
    bound_root: &Path,
    root_role: BuildDeclarationKind,
    root_name: String,
) -> PackageCompilationInputs {
    let root_identity = fixture_package_identity(1);
    PackageCompilationInputs::new(
        root_identity,
        root_role,
        vec![PackageSourceBinding::new(
            root_identity,
            root_name,
            bound_root.to_path_buf(),
        )],
        Vec::new(),
    )
    .unwrap_or_else(|errors| panic!("fixture {}: {errors:#?}", authored_root.display()))
}

/// Both authored package and application declarations retain their package
/// identity even when they have no dependencies.
fn dependency_free_package_inputs_bound_to(
    authored_root: &Path,
    bound_root: &Path,
) -> Option<PackageCompilationInputs> {
    let declaration = extract_build_declaration(authored_root).ok()?;
    let role = declaration.kind();
    let name = match declaration {
        BuildDeclaration::Package(package) => package.name,
        BuildDeclaration::Application(application) => application.name,
        BuildDeclaration::Workspace(_) => return None,
    };
    Some(dependency_free_inputs(
        authored_root,
        bound_root,
        role,
        name.into_string(),
    ))
}

/// Package inputs binding `project_root` as the compilation root and one
/// standard library at `standard_library_root` under its ordinary alias, for
/// a project that authors no such row itself: a scratch package written
/// without a `build.omg`, or a fixture pointed at a relocated copy of the
/// library. A fixture that authored the row takes
/// [`repository_fixture_package_inputs`] instead.
pub fn standard_library_package_inputs(
    project_root: &Path,
    standard_library_root: &Path,
) -> PackageCompilationInputs {
    let (root_role, root_name) = fixture_root_declaration(project_root);
    let root_identity = fixture_package_identity(1);
    let standard_library_identity = fixture_package_identity(2);
    PackageCompilationInputs::new(
        root_identity,
        root_role,
        vec![
            PackageSourceBinding::new(root_identity, root_name, project_root.to_path_buf()),
            PackageSourceBinding::new(
                standard_library_identity,
                "omega_language_std",
                standard_library_root.to_path_buf(),
            ),
        ],
        vec![PackageDependencyBinding::new(
            root_identity,
            "omega_language_std",
            standard_library_identity,
        )],
    )
    .unwrap_or_else(|errors| panic!("fixture {}: {errors:#?}", project_root.display()))
}

/// `location` itself when it is disjoint from every bound root, or a staged
/// copy of it when it nests. Acquisition gives each package its own directory
/// in the real build; a fixture compiled in place has no such step, and a
/// member authored inside its requester would otherwise present two source
/// roots where one contains the other.
///
/// The copy is keyed by the member's canonical path and this process, so
/// concurrent corpus workers never share one staging directory, and it is
/// refreshed on every call rather than reused across runs.
fn staged_member_root(location: &Path, bound: &HashMap<PathBuf, PackageKeyIdentity>) -> PathBuf {
    let nests = bound
        .keys()
        .any(|root| location.starts_with(root) && location != root || root.starts_with(location));
    if !nests {
        return location.to_path_buf();
    }
    let mut key = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(&location, &mut key);
    let staged = std::env::temp_dir().join(format!(
        "omega-fixture-member-{}-{:016x}",
        std::process::id(),
        std::hash::Hasher::finish(&key),
    ));
    let _ = fs::remove_dir_all(&staged);
    copy_member_tree(location, &staged).unwrap_or_else(|error| {
        panic!(
            "fixture member {} could not be staged at {}: {error}",
            location.display(),
            staged.display()
        )
    });
    staged
}

fn copy_member_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_member_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn fixture_package_inputs_bound_to(
    authored_root: &Path,
    bound_root: &Path,
) -> Option<PackageCompilationInputs> {
    if fixture_path_dependencies(authored_root).is_empty() {
        return dependency_free_package_inputs_bound_to(authored_root, bound_root);
    }

    let (root_role, root_name) = fixture_root_declaration(authored_root);
    let root_identity = fixture_package_identity(1);
    let standard_library_root = fs::canonicalize(bundled_standard_library_root())
        .expect("bundled standard library root resolves");

    let mut packages = vec![PackageSourceBinding::new(
        root_identity,
        root_name,
        bound_root.to_path_buf(),
    )];
    let mut dependencies = Vec::new();
    let mut bound: HashMap<PathBuf, PackageKeyIdentity> = HashMap::new();
    bound.insert(
        fs::canonicalize(authored_root)
            .unwrap_or_else(|error| panic!("fixture {}: {error}", authored_root.display())),
        root_identity,
    );
    let mut next_marker: u8 = 3;
    let mut pending = vec![(authored_root.to_path_buf(), root_identity)];
    while let Some((requester_root, requester)) = pending.pop() {
        for dependency in fixture_path_dependencies(&requester_root) {
            if !dependency.purpose.is_product() && requester != root_identity {
                continue;
            }
            let target = match bound.get(&dependency.location) {
                Some(bound_target) => *bound_target,
                None => {
                    let identity = if dependency.location == standard_library_root {
                        fixture_package_identity(2)
                    } else {
                        let marker = next_marker;
                        next_marker = next_marker
                            .checked_add(1)
                            .expect("fixture package markers stay distinct");
                        fixture_package_identity(marker)
                    };
                    // Source roots must be disjoint. A fixture may author its
                    // member inside its own directory (`depend_as("leaf",
                    // Source::Path { location: "leaf" })`), which the real
                    // build accepts because acquisition snapshots every
                    // package into its own cache directory before compiling.
                    // Binding the authored nesting directly instead would
                    // report overlapping roots, so stage the member the same
                    // way. The walk continues from the AUTHORED location, so
                    // the member's own relative rows still resolve.
                    let bound_location = staged_member_root(&dependency.location, &bound);
                    packages.push(PackageSourceBinding::new(
                        identity,
                        dependency.package_name.clone(),
                        bound_location,
                    ));
                    bound.insert(dependency.location.clone(), identity);
                    pending.push((dependency.location.clone(), identity));
                    identity
                }
            };
            dependencies.push(PackageDependencyBinding::for_purpose(
                requester,
                dependency.alias.as_str(),
                target,
                dependency.purpose,
            ));
        }
    }

    Some(
        PackageCompilationInputs::new(root_identity, root_role, packages, dependencies)
            .unwrap_or_else(|errors| panic!("fixture {}: {errors:#?}", authored_root.display())),
    )
}

/// The reviewed `ProgramEntry` candidate the standard library at
/// `standard_library_root` owns for `target_name`.
///
/// Entry acceptance is decided by the target profile, not by spelling: every
/// hosted profile with a reviewed ProgramEntry candidate binds it here, and a
/// profile without one (macOS x86-64, the cross-platform, unchecked and
/// bootstrap profiles) is `None` rather than falling through a chain of
/// string comparisons. An unknown target name is the vocabulary's diagnostic.
pub fn candidate_program_entry_binding(
    target_name: &str,
    standard_library_root: &Path,
    standard_library: PackageKeyIdentity,
) -> Result<Option<AcceptedSemanticBinding>, Vec<Diagnostic>> {
    // The candidate depends only on these three inputs, and each derivation
    // checks the whole standard library entry: a test that compiles many
    // fixtures (the sample run oracle) would otherwise repeat that check for
    // every one. Compute outside the lock; a racing duplicate is only waste.
    type Key = (String, std::path::PathBuf, PackageKeyIdentity);
    type Candidate = Result<Option<AcceptedSemanticBinding>, Vec<Diagnostic>>;
    static CANDIDATES: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<Key, Candidate>>,
    > = std::sync::OnceLock::new();
    let candidates = CANDIDATES.get_or_init(Default::default);
    let key = (
        target_name.to_owned(),
        standard_library_root.to_path_buf(),
        standard_library,
    );
    if let Some(candidate) = candidates
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&key)
    {
        return candidate.clone();
    }
    let candidate =
        derive_program_entry_binding(target_name, standard_library_root, standard_library);
    candidates
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(key, candidate.clone());
    candidate
}

fn derive_program_entry_binding(
    target_name: &str,
    standard_library_root: &Path,
    standard_library: PackageKeyIdentity,
) -> Result<Option<AcceptedSemanticBinding>, Vec<Diagnostic>> {
    let profile = TargetProfile::from_canonical_target_name(target_name)
        .map_err(|diagnostic| vec![diagnostic])?;
    Ok(match profile {
        TargetProfile::MacosArm64 => Some(macos_entry_acceptance::candidate_macos_entry_binding(
            standard_library_root,
            standard_library,
        )?),
        TargetProfile::LinuxX64 => Some(
            linux_entry_acceptance::candidate_linux_x86_64_entry_binding(
                standard_library_root,
                standard_library,
            )?,
        ),
        TargetProfile::LinuxArm64 => {
            Some(linux_entry_acceptance::candidate_linux_arm64_entry_binding(
                standard_library_root,
                standard_library,
            )?)
        }
        TargetProfile::WindowsX64 => Some(
            windows_entry_acceptance::candidate_windows_x86_64_entry_binding(
                standard_library_root,
                standard_library,
            )?,
        ),
        TargetProfile::UefiX64 => Some(uefi_entry_acceptance::candidate_uefi_entry_binding(
            standard_library_root,
            standard_library,
        )?),
        TargetProfile::MacosX64 => Some(
            macos_entry_acceptance::candidate_macos_x86_64_entry_binding(
                standard_library_root,
                standard_library,
            )?,
        ),
        TargetProfile::CrossPlatformCli
        | TargetProfile::LocalUnchecked
        | TargetProfile::AlphaBootstrap => None,
    })
}

/// The authored package graph plus this repository's test acceptance: the
/// bundled library's target entry candidate when present, then the services the
/// preliminary checked graph requires, including root-owned services. `None`
/// for a fixture that stays standalone.
pub fn reviewed_repository_fixture_package_inputs(
    root_path: &Path,
    target_name: Option<&str>,
) -> Result<Option<PackageCompilationInputs>, Vec<Diagnostic>> {
    let Some(mut package_inputs) = repository_fixture_package_inputs(root_path) else {
        return Ok(None);
    };
    // Hosted entry acceptance belongs to the bundled standard library's exact
    // package identity. Service acceptance below follows each selected owner.
    let standard_library_identity = fixture_package_identity(2);
    let declares_standard_library = package_inputs
        .packages()
        .any(|(identity, _)| identity == standard_library_identity);
    let standard_library_root = bundled_standard_library_root();
    let mut bindings = Vec::new();
    // Entry acceptance is decided by the target profile, not by spelling.
    if declares_standard_library && let Some(target_name) = target_name {
        bindings.extend(memoized_program_entry_binding(
            target_name,
            &standard_library_root,
            standard_library_identity,
        )?);
    }
    if !bindings.is_empty() {
        package_inputs = package_inputs
            .with_accepted_semantic_bindings(bindings.clone())
            .map_err(|errors| {
                vec![Diagnostic::error(format!(
                    "entry fixture acceptance: {errors:?}"
                ))]
            })?;
    }
    // Discovery publishes candidate source semantics before schema/plan
    // commitments. Only the following accepted pass grants execution authority.
    // Reconstruct from this exact graph: a target/use-flags cache cannot retain
    // package ownership, source-dependent contracts or provider-plan identity.
    let preliminary = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs.clone().with_semantic_role_discovery(true)),
        permit_unsettled_fused_service_fields: true,
        ..CheckedCompileRequest::new(root_path, target_name)
    })?;
    for (owner, _) in package_inputs.packages() {
        let required =
            dangerous_service_acceptance::required_dangerous_services(&preliminary, owner);
        bindings.extend(dangerous_service_bindings(&preliminary, owner, &required)?);
    }
    package_inputs
        .with_accepted_semantic_bindings(bindings)
        .map(Some)
        .map_err(|errors| {
            vec![Diagnostic::error(format!(
                "cannot admit repository fixture semantic binding: {errors:?}"
            ))]
        })
}

/// The entry binding depends only on the target and the bundled library, and
/// deriving it reads the library's declarations (1.9 s per fixture on the
/// measured host), so one process derives it once per target.
fn memoized_program_entry_binding(
    target_name: &str,
    standard_library_root: &Path,
    standard_library: PackageKeyIdentity,
) -> Result<Option<AcceptedSemanticBinding>, Vec<Diagnostic>> {
    static MEMO: std::sync::OnceLock<
        std::sync::Mutex<HashMap<String, Option<AcceptedSemanticBinding>>>,
    > = std::sync::OnceLock::new();
    let memo = MEMO.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    if let Ok(memo) = memo.lock()
        && let Some(binding) = memo.get(target_name)
    {
        return Ok(binding.clone());
    }
    let binding =
        candidate_program_entry_binding(target_name, standard_library_root, standard_library)?;
    if let Ok(mut memo) = memo.lock() {
        memo.insert(target_name.to_owned(), binding.clone());
    }
    Ok(binding)
}

/// The dangerous-service rows the preliminary checked graph admits, every one
/// derived from and replayed against that graph.
fn dangerous_service_bindings(
    preliminary: &compiler::CheckedCompilation,
    owner: PackageKeyIdentity,
    required: &dangerous_service_acceptance::RequiredDangerousServices,
) -> Result<Vec<AcceptedSemanticBinding>, Vec<Diagnostic>> {
    let mut bindings = Vec::new();
    if required.filesystem {
        bindings.push(
            preliminary
                .execution()
                .candidate_service_binding(
                    AcceptedSemanticBindingRole::FilesystemHostService,
                    owner,
                    "FilesystemHost",
                )
                .map_err(|diagnostic| vec![diagnostic])?,
        );
    }
    if required.time {
        bindings.push(
            preliminary
                .execution()
                .candidate_service_binding(
                    AcceptedSemanticBindingRole::TimeHostService,
                    owner,
                    "TimeHost",
                )
                .map_err(|diagnostic| vec![diagnostic])?,
        );
    }
    if let Some(console) = required.console {
        bindings.push(console_acceptance::candidate_console_exit_binding(
            preliminary,
            owner,
            console.output,
            console.input,
        )?);
    }
    if required.process_exit {
        bindings.push(process_exit_acceptance::candidate_process_exit_binding(
            preliminary,
            owner,
        )?);
    }
    Ok(bindings)
}

pub fn hosted_main_program_entry_build_for(canary: &Path, target: &str) -> String {
    let root_owner = hosted_program_entry_owner(target);
    format!(
        "machine build(builder: &mut Build) {{\n    Build::application(builder, \"hosted_main_program_entry\");\n{}    builder.roots.bind({root_owner}::ProgramEntry, Main::main);\n}}\n",
        fixture_dependency_declarations(canary)
    )
}

/// The root-slot owner a scratch build binds `ProgramEntry` under, read from
/// the target vocabulary. Only profiles with a native realization own a
/// hosted ProgramEntry root; asking for any other profile is a harness
/// defect, not a fixture outcome.
pub fn hosted_program_entry_owner(target: &str) -> &'static str {
    let profile = TargetProfile::from_canonical_target_name(target)
        .unwrap_or_else(|error| panic!("harness names an unknown target `{target}`: {error}"));
    assert!(
        profile.native_realization().is_some(),
        "no hosted ProgramEntry root owner for target `{target}`"
    );
    profile.root_slot_owner_name()
}

/// The cross-target application build written for a fixture copied into a
/// scratch project: binds the compiled target's `ProgramEntry` to `Main::main`
/// so native production passes exact entry admission, restates the
/// dependencies the fixture authored through absolute paths, and mirrors the
/// authored freestanding EFI profile for `uefi_x86_64`.
pub fn cross_target_program_entry_build(canary: &Path, target: &str) -> String {
    let root_owner = hosted_program_entry_owner(target);
    let mut build =
        "machine build(builder: &mut Build) {\n    Build::application(builder, \"cross_target_canary\");\n"
            .to_owned();
    build.push_str(&fixture_dependency_declarations(canary));
    if target == "uefi_x86_64" {
        build.push_str(
            "    builder.subsystem = Subsystem::EfiApplication;\n    builder.freestanding = true;\n",
        );
    }
    build.push_str(&format!(
        "    builder.roots.bind({root_owner}::ProgramEntry, Main::main);\n}}\n"
    ));
    build
}
