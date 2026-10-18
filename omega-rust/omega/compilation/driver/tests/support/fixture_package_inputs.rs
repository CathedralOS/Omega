//! The fixture package-inputs harness shared by the compiler's test targets.
//!
//! Every test target that compiles a repository fixture, a sample or a scratch
//! project against the bundled standard library reads its package mode from
//! here, so one projection decides what a fixture depends on and one match
//! over the target vocabulary decides which reviewed entry it accepts:
//!
//! - [`fixture_path_dependencies`] projects the `depend`/`depend_as`/
//!   `build_depend*` rows a fixture authored in its own `build.omg`;
//! - [`fixture_package_inputs_bound_to`] binds that closure with the
//!   compilation root at marker 1 and the bundled standard library at
//!   marker 2, the identity entry and dangerous-service acceptance name;
//! - [`join_dependency_builds`] executes the dependency Build occurrences that
//!   closure requires through the driver's package route and joins their
//!   generated source into the root's inputs, as the product compile does;
//! - [`reviewed_repository_fixture_package_inputs`] stages the fixture's
//!   sealed source snapshot, binds the root package to it, and adds this
//!   repository's test acceptance: the target profile's reviewed
//!   `ProgramEntry` candidate and the selected package-owned services the
//!   preliminary checked graph requires;
//! - the `*_build` writers restate a fixture's authored rows for a scratch
//!   copy of its source, through absolute paths, so no test target types the
//!   standard library's location into a synthesized `build.omg`.
//!
//! The root package is always bound to a sealed snapshot staged by
//! [`sealed_fixture_staging`], never to the checked-out fixture directory,
//! because a package-aware build activation captures canonical Source
//! metadata from the physical root and that capture admits only the modes
//! acquisition leaves. The authored directory remains where dependency rows
//! are read and where their relative locations resolve, so the two roots are
//! threaded separately rather than one standing in for the other.
//!
//! Acceptance here is test policy, not evidence that an audit occurred and not
//! production accepted-lock recovery; every admitted row is derived from and
//! replayed against the exact preliminary checked graph.
//!
//! The bundled library supplies the default reviewed `ProgramEntry` candidate.
//! When a fixture needs its consumer to accept a *different* package's entry
//! contract — for example a vendored `targets/<target>/entry.omg` whose
//! physical convention intentionally disagrees with the selected slot — it
//! authors `accepted_entry_supplier.txt` at the fixture root with one declared
//! dependency's package name per line. Each named package's contract source is
//! compiled through the reviewed-candidate route and its binding replaces the
//! bundled default for that compile's entry role, so an explicit supplier
//! decision cannot create a duplicate accepted binding role.

// Shared across test targets through `#[path]`; each target uses the subset
// its fixtures need.
#![allow(dead_code)]

use build_declarations::{
    BuildDeclaration, BuildDeclarationKind, DependencyPurpose, extract_build_declaration,
    is_dependency_call_name, project_build_entry_syntax, project_dependency_rows,
};
use compiler::CheckedCompileRequest;
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
#[path = "process_exit_acceptance.rs"]
pub mod process_exit_acceptance;
#[path = "sealed_fixture_staging.rs"]
pub mod sealed_fixture_staging;

pub fn repo_root() -> PathBuf {
    if let Some(root) = std::env::var_os("OMEGA_CORPUS_ROOT") {
        return PathBuf::from(root);
    }
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

/// The root package's binding.
///
/// `bound_root` is a sealed source snapshot, so the binding carries the
/// compiler-captured canonical Source metadata a package-aware build
/// activation requires before any build effect runs. The capture re-inspects
/// the physical modes itself; a root that is not sealed fails it, and that is
/// this harness staging the root wrongly, never a fixture's own refusal.
fn sealed_root_source_binding(
    identity: PackageKeyIdentity,
    canonical_name: String,
    bound_root: &Path,
) -> PackageSourceBinding {
    PackageSourceBinding::new(identity, canonical_name, bound_root.to_path_buf())
        .with_canonical_source_metadata()
        .unwrap_or_else(|reason| {
            panic!(
                "fixture root {} is not a sealed source snapshot: {reason}",
                bound_root.display()
            )
        })
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
        vec![sealed_root_source_binding(
            root_identity,
            root_name,
            bound_root,
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

/// One package a fixture's dependency rows reach.
struct FixtureGraphPackage {
    identity: PackageKeyIdentity,
    binding: PackageSourceBinding,
    /// Where `binding` binds the package's source.
    bound_root: PathBuf,
    /// The authored directory the package's rows are read from.
    authored_root: PathBuf,
    role: BuildDeclarationKind,
    /// The authored edges this package declares, each naming its target.
    edges: Vec<(DependencyPurpose, String, PackageKeyIdentity)>,
}

/// Every package a fixture's dependency rows reach, transitively, including
/// the packages a dependency's own build edges name, each bound once under
/// one identity, so that the root compilation and every dependency Build
/// occurrence describe the same packages.
///
/// Identities: the compilation root takes marker 1 and the bundled standard
/// library marker 2 — pinned so entry and dangerous-service acceptance can
/// name it — and every other package takes the next free marker in the order
/// the walk reaches it. The walk first reaches what the root's own
/// compilation names, then what a dependency's build edges add, so a
/// package the root names keeps the marker it had before dependency Builds
/// were staged.
struct FixturePackageGraph {
    /// In marker order; the root is first.
    packages: Vec<FixtureGraphPackage>,
}

impl FixturePackageGraph {
    /// `bound_root` is where the root package's source is bound and
    /// `authored_root` is where its `build.omg` is read and its relative
    /// dependency locations resolve. They differ because the root is bound
    /// to a sealed snapshot staged out of the authored directory.
    fn new(authored_root: &Path, bound_root: &Path) -> Self {
        let (root_role, root_name) = fixture_root_declaration(authored_root);
        let root_identity = fixture_package_identity(1);
        let mut walk = FixtureGraphWalk {
            packages: vec![FixtureGraphPackage {
                identity: root_identity,
                binding: sealed_root_source_binding(root_identity, root_name, bound_root),
                bound_root: bound_root.to_path_buf(),
                authored_root: authored_root.to_path_buf(),
                role: root_role,
                edges: Vec::new(),
            }],
            bound: HashMap::from([(
                fs::canonicalize(authored_root)
                    .unwrap_or_else(|error| panic!("fixture {}: {error}", authored_root.display())),
                root_identity,
            )]),
            standard_library_root: fs::canonicalize(bundled_standard_library_root())
                .expect("bundled standard library root resolves"),
            next_marker: 3,
            pending: vec![(authored_root.to_path_buf(), root_identity)],
        };
        let mut deferred = Vec::new();
        loop {
            while let Some((requester_root, requester)) = walk.pending.pop() {
                for dependency in fixture_path_dependencies(&requester_root) {
                    // A dependency's build edge resolves in that dependency's
                    // own Build occurrence, not in the root's compilation.
                    if !dependency.purpose.is_product() && requester != root_identity {
                        deferred.push((requester, dependency));
                        continue;
                    }
                    walk.add_edge(requester, dependency);
                }
            }
            if deferred.is_empty() {
                break;
            }
            for (requester, dependency) in std::mem::take(&mut deferred) {
                walk.add_edge(requester, dependency);
            }
        }
        Self {
            packages: walk.packages,
        }
    }

    fn package(&self, identity: PackageKeyIdentity) -> &FixtureGraphPackage {
        self.packages
            .iter()
            .find(|package| package.identity == identity)
            .expect("fixture graph edges name bound packages")
    }

    /// The compilation inputs rooted at `root`: its own edges of both
    /// purposes and every reached package's product edges. A build edge
    /// authorizes only its requester's build entry, exactly as the package
    /// manager's compiler input does, because a dependency's build context
    /// resolves in its own compilation where that package is the root.
    ///
    /// `root_binding` binds the root package: its graph binding for the
    /// fixture's own compilation, a sealed dependency snapshot for a
    /// dependency Build occurrence.
    fn inputs_rooted_at(
        &self,
        root: PackageKeyIdentity,
        root_binding: PackageSourceBinding,
    ) -> PackageCompilationInputs {
        let mut reached = vec![root];
        let mut dependencies = Vec::new();
        let mut cursor = 0;
        while let Some(&requester) = reached.get(cursor) {
            cursor += 1;
            for (purpose, alias, target) in &self.package(requester).edges {
                if !purpose.is_product() && requester != root {
                    continue;
                }
                dependencies.push(PackageDependencyBinding::for_purpose(
                    requester,
                    alias.as_str(),
                    *target,
                    *purpose,
                ));
                if !reached.contains(target) {
                    reached.push(*target);
                }
            }
        }
        let root_package = self.package(root);
        let packages = std::iter::once(root_binding)
            .chain(
                self.packages
                    .iter()
                    .filter(|package| {
                        package.identity != root && reached.contains(&package.identity)
                    })
                    .map(|package| package.binding.clone()),
            )
            .collect();
        PackageCompilationInputs::new(root, root_package.role, packages, dependencies)
            .unwrap_or_else(|errors| {
                panic!(
                    "fixture package graph rooted at {}: {errors:#?}",
                    root_package.bound_root.display()
                )
            })
    }

    /// Every dependency Build occurrence a compilation over `inputs`
    /// requires, transitively, ordered dependency-first, as the package
    /// manager orders its occurrence roster: each occurrence's own required
    /// occurrences come before it. Each occurrence is rooted at a sealed
    /// snapshot of its package, which is answered beside its inputs.
    fn dependency_occurrences(
        &self,
        inputs: &PackageCompilationInputs,
    ) -> Vec<DependencyOccurrence> {
        let mut ordered = Vec::new();
        for (package, purpose) in inputs.required_dependency_source_instances() {
            self.visit_occurrence(package, purpose, &mut ordered);
        }
        ordered
    }

    fn visit_occurrence(
        &self,
        package: PackageKeyIdentity,
        purpose: DependencyPurpose,
        ordered: &mut Vec<DependencyOccurrence>,
    ) {
        if ordered
            .iter()
            .any(|occurrence| occurrence.package == package && occurrence.purpose == purpose)
        {
            return;
        }
        let graph_package = self.package(package);
        let snapshot = sealed_fixture_staging::stage_sealed_dependency_snapshot(
            &repo_root(),
            &graph_package.authored_root,
        );
        let name = package_name_of(&graph_package.authored_root);
        let inputs = self
            .inputs_rooted_at(
                package,
                sealed_root_source_binding(package, name, &snapshot),
            )
            .with_compilation_purpose(purpose)
            .unwrap_or_else(|errors| panic!("fixture dependency occurrence: {errors:#?}"));
        for (required, required_purpose) in inputs.required_dependency_source_instances() {
            self.visit_occurrence(required, required_purpose, ordered);
        }
        ordered.push(DependencyOccurrence {
            package,
            purpose,
            snapshot,
            inputs,
        });
    }
}

/// One dependency Build occurrence: the package, the purpose it is
/// activated for, the sealed snapshot it is rooted at, and its inputs.
struct DependencyOccurrence {
    package: PackageKeyIdentity,
    purpose: DependencyPurpose,
    snapshot: PathBuf,
    inputs: PackageCompilationInputs,
}

/// The package name a dependency directory's own `build.omg` declares.
fn package_name_of(authored_root: &Path) -> String {
    match extract_build_declaration(authored_root) {
        Ok(BuildDeclaration::Package(package)) => package.name.into_string(),
        _ => panic!(
            "fixture dependency {} declares no package",
            authored_root.display()
        ),
    }
}

/// Construction state for [`FixturePackageGraph::new`].
struct FixtureGraphWalk {
    packages: Vec<FixtureGraphPackage>,
    bound: HashMap<PathBuf, PackageKeyIdentity>,
    standard_library_root: PathBuf,
    next_marker: u8,
    pending: Vec<(PathBuf, PackageKeyIdentity)>,
}

impl FixtureGraphWalk {
    fn add_edge(&mut self, requester: PackageKeyIdentity, dependency: FixturePathDependency) {
        let target = match self.bound.get(&dependency.location) {
            Some(bound_target) => *bound_target,
            None => {
                let identity = if dependency.location == self.standard_library_root {
                    fixture_package_identity(2)
                } else {
                    let marker = self.next_marker;
                    self.next_marker = self
                        .next_marker
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
                let bound_location = staged_member_root(&dependency.location, &self.bound);
                self.packages.push(FixtureGraphPackage {
                    identity,
                    binding: PackageSourceBinding::new(
                        identity,
                        dependency.package_name.clone(),
                        bound_location.clone(),
                    ),
                    bound_root: bound_location,
                    authored_root: dependency.location.clone(),
                    role: BuildDeclarationKind::Package,
                    edges: Vec::new(),
                });
                self.bound.insert(dependency.location.clone(), identity);
                self.pending.push((dependency.location.clone(), identity));
                identity
            }
        };
        self.packages
            .iter_mut()
            .find(|package| package.identity == requester)
            .expect("fixture graph requester is bound")
            .edges
            .push((dependency.purpose, dependency.alias, target));
    }
}

/// Package inputs for a repository fixture, or `None` when it authored no
/// dependency at all and did not request package identity. A fixture that
/// declares `Build::package` or `Build::application` compiles in package mode
/// even with an empty dependency set. The graph is returned beside the inputs
/// so the caller can stage the dependency Build occurrences they require.
fn fixture_package_inputs_bound_to(
    authored_root: &Path,
    bound_root: &Path,
) -> Option<(PackageCompilationInputs, Option<FixturePackageGraph>)> {
    if fixture_path_dependencies(authored_root).is_empty() {
        return dependency_free_package_inputs_bound_to(authored_root, bound_root)
            .map(|inputs| (inputs, None));
    }
    let graph = FixturePackageGraph::new(authored_root, bound_root);
    let root = fixture_package_identity(1);
    let inputs = graph.inputs_rooted_at(root, graph.package(root).binding.clone());
    Some((inputs, Some(graph)))
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
    derive_supplier_program_entry_binding(
        target_name,
        standard_library,
        standard_library_root,
        vec![PackageSourceBinding::new(
            standard_library,
            "omega_language_std",
            standard_library_root.to_path_buf(),
        )],
        Vec::new(),
    )
}

/// The reviewed `ProgramEntry` candidate an explicit entry-contract supplier
/// owns for `target_name`, derived exactly the way the bundled candidate is:
/// the supplier's own `targets/<target>/entry.omg` is compiled as a package
/// root with the supplier's authored dependency edges, and its exact
/// schema declaration binding is replayed from that checked graph.
fn derive_supplier_program_entry_binding(
    target_name: &str,
    supplier: PackageKeyIdentity,
    supplier_root: &Path,
    packages: Vec<PackageSourceBinding>,
    dependencies: Vec<PackageDependencyBinding>,
) -> Result<Option<AcceptedSemanticBinding>, Vec<Diagnostic>> {
    let profile = TargetProfile::from_canonical_target_name(target_name)
        .map_err(|diagnostic| vec![diagnostic])?;
    let (target_name, role, schema) = match profile {
        TargetProfile::MacosArm64 => (
            "macos_arm64",
            AcceptedSemanticBindingRole::MacosArm64ProgramEntry,
            "MacosApplication",
        ),
        TargetProfile::LinuxX64 => (
            "linux_x86_64",
            AcceptedSemanticBindingRole::LinuxX86_64ProgramEntry,
            "LinuxX86_64Application",
        ),
        TargetProfile::LinuxArm64 => (
            "linux_arm64",
            AcceptedSemanticBindingRole::LinuxArm64ProgramEntry,
            "LinuxArm64Application",
        ),
        TargetProfile::WindowsX64 => (
            "windows_x86_64",
            AcceptedSemanticBindingRole::WindowsX64ProgramEntry,
            "WindowsX86_64Application",
        ),
        TargetProfile::UefiX64 => (
            "uefi_x86_64",
            AcceptedSemanticBindingRole::UefiX64ProgramEntry,
            "UefiApplication",
        ),
        TargetProfile::MacosX64 => (
            "macos_x86_64",
            AcceptedSemanticBindingRole::MacosX64ProgramEntry,
            "MacosX64Application",
        ),
        TargetProfile::CrossPlatformCli
        | TargetProfile::LocalUnchecked
        | TargetProfile::AlphaBootstrap => return Ok(None),
    };
    // Check the package-owned target contract independently on every derivation;
    // application discovery alone cannot authorize the program-entry role.
    let inputs = PackageCompilationInputs::new_package(supplier, packages, dependencies).map_err(
        |errors| {
            vec![Diagnostic::error(format!(
                "entry fixture inputs: {errors:?}"
            ))]
        },
    )?;
    let checked = compiler::CompilationSession::default()
        .check(CheckedCompileRequest {
            package_inputs: Some(inputs),
            ..CheckedCompileRequest::new(
                &supplier_root.join(format!("targets/{target_name}/entry.omg")),
                Some(target_name),
            )
        })
        .result
        .map_err(|failure| failure.into_diagnostics())?;
    checked
        .execution()
        .candidate_service_binding(
            &checked.providers().boundary_calling_plan_realizations,
            role,
            supplier,
            schema,
        )
        .map(Some)
        .map_err(|diagnostic| vec![diagnostic])
}

/// One repository fixture prepared for compilation, the way a real build
/// prepares a package.
///
/// [`root_path`](Self::root_path) is the staged snapshot's root source file,
/// not the authored one: the root package is bound to the sealed snapshot, so
/// the compilation that consumes these inputs has to read its root from the
/// same place. Every leg compiles that path.
pub struct ReviewedFixtureCompilation {
    pub root_path: PathBuf,
    pub package_inputs: Option<PackageCompilationInputs>,
    pub assumed_permissions:
        Option<package_evidence::permission_acceptance::TerminalPermissionAcceptance>,
}

/// Consume the fixture's explicit consent through the same package-product
/// entrance as accepted-lock compilation. The caller may batch these requests
/// for a target cohort; each target carries its own freshly projected rows.
pub fn accepted_fixture_product_request(
    prepared: ReviewedFixtureCompilation,
    target: Option<&str>,
    build_dir: PathBuf,
    product: compiler::CompilationProduct,
) -> Result<compiler::PackageCompilationRequest, Vec<Diagnostic>> {
    let snapshot = fixture_build_snapshot(&prepared.root_path)?;
    let mut request = CheckedCompileRequest::new(&prepared.root_path, target);
    request.package_inputs = prepared.package_inputs;
    request.build_snapshot = snapshot;
    request.build_dir = Some(build_dir);
    Ok(compiler::PackageCompilationRequest {
        occurrences: vec![request],
        product,
        terminal_permissions: prepared.assumed_permissions,
    })
}

pub fn produce_accepted_fixture(
    request: compiler::PackageCompilationRequest,
) -> Result<compiler::CompileReport, Vec<Diagnostic>> {
    compiler::CompilationSession::default()
        .compile_packages(vec![request])
        .pop()
        .expect("one fixture product")
        .map_err(|error| error.failure.into_diagnostics())
        .and_then(|output| {
            output
                .into_report()
                .map_err(compiler::CompilationProductError::into_diagnostics)
        })
}

/// Generic authored host-input fixtures: each immediate directory under
/// `build-inputs/` is one named inventory. Read the already sealed fixture
/// staging, not an ambient host path selected by Omega code. This is request
/// setup in the harness, never a shipped CLI fixture recognizer.
pub fn fixture_build_snapshot(
    root_path: &Path,
) -> Result<Option<package_compilation::BuildSnapshotRequest>, Vec<Diagnostic>> {
    let Some(root) = root_path.parent() else {
        return Ok(None);
    };
    let input_root = root.join("build-inputs");
    let listing = match fs::read_dir(&input_root) {
        Ok(listing) => listing,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(vec![Diagnostic::error(format!(
                "cannot list fixture build inputs: {error}"
            ))]);
        }
    };
    let mut inputs = std::collections::BTreeMap::new();
    for entry in listing {
        let entry = entry.map_err(|error| {
            vec![Diagnostic::error(format!(
                "cannot read fixture input slot: {error}"
            ))]
        })?;
        if !entry
            .file_type()
            .map_err(|error| vec![Diagnostic::error(error.to_string())])?
            .is_dir()
        {
            return Err(vec![Diagnostic::error(
                "fixture build input slots must be concrete directories",
            )]);
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| {
                vec![Diagnostic::error(
                    "fixture input slot must have a UTF-8 name",
                )]
            })?
            .into_bytes();
        let captured =
            package_compilation::capture_package_source_input(&entry.path()).map_err(|reason| {
                vec![Diagnostic::error(format!(
                    "cannot capture fixture input slot: {reason}"
                ))]
            })?;
        inputs.insert(name, captured);
    }
    package_compilation::BuildSnapshotRequest::new(std::iter::empty::<Vec<u8>>())
        .with_inputs(inputs)
        .map(Some)
}

/// The authored package graph plus this repository's test acceptance: the
/// bundled library's target entry candidate when present, then the services the
/// preliminary checked graph requires, including root-owned services. The
/// package inputs are `None` for a fixture that stays standalone; its source
/// is staged either way, so one route compiles every fixture.
pub fn reviewed_repository_fixture_package_inputs(
    authored_root_path: &Path,
    target_name: Option<&str>,
) -> Result<ReviewedFixtureCompilation, Vec<Diagnostic>> {
    let authored_project_root = authored_root_path
        .parent()
        .expect("fixture source has a project root");
    let staged_project_root =
        sealed_fixture_staging::stage_sealed_snapshot(&repo_root(), authored_project_root);
    let root_path = staged_project_root.join(
        authored_root_path
            .file_name()
            .expect("fixture source has a file name"),
    );
    // Dependency rows are read and resolved against the authored project: a
    // fixture spells the bundled library as a path relative to its own
    // directory, which no snapshot location reproduces.
    let Some((mut package_inputs, graph)) =
        fixture_package_inputs_bound_to(authored_project_root, &staged_project_root)
    else {
        return Ok(ReviewedFixtureCompilation {
            root_path,
            package_inputs: None,
            assumed_permissions: None,
        });
    };
    if let Some(graph) = &graph {
        package_inputs = join_dependency_builds(
            graph,
            package_inputs,
            &root_path,
            authored_project_root,
            target_name,
        )?;
    }
    // Hosted entry acceptance belongs to the bundled standard library's exact
    // package identity. Service acceptance below follows each selected owner.
    let standard_library_identity = fixture_package_identity(2);
    let declares_standard_library = package_inputs
        .packages()
        .any(|(identity, _)| identity == standard_library_identity);
    let standard_library_root = bundled_standard_library_root();
    let mut bindings = Vec::new();
    // A fixture that authors `accepted_entry_supplier.txt` has decided which
    // declared dependency's entry contract its consumer accepted for this
    // compile's entry role; that binding replaces the bundled default (the
    // role admits exactly one accepted binding).
    let accepted_suppliers = accepted_entry_suppliers(authored_project_root);
    for supplier_name in &accepted_suppliers {
        let Some(target_name) = target_name else {
            panic!(
                "fixture {} authors accepted_entry_supplier.txt but is compiled for no target",
                authored_project_root.display()
            )
        };
        bindings.push(accepted_supplier_program_entry_binding(
            authored_project_root,
            target_name,
            supplier_name,
            &package_inputs,
        )?);
    }
    // Entry acceptance is decided by the target profile, not by spelling.
    if accepted_suppliers.is_empty()
        && declares_standard_library
        && let Some(target_name) = target_name
    {
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
    // Discovery runs the fixture's own build machine, so it needs the same
    // writable staging a real activation gets; the default beside the root
    // source is inside the sealed snapshot.
    let preliminary = compiler::CompilationSession::default()
        .check(CheckedCompileRequest {
            package_inputs: Some(package_inputs.clone().with_semantic_role_discovery(true)),
            permit_unsettled_fused_service_fields: true,
            build_snapshot: fixture_build_snapshot(&root_path)?,
            build_dir: Some(sealed_fixture_staging::fresh_staging_build_dir(
                &repo_root(),
                authored_project_root,
            )),
            ..CheckedCompileRequest::new(&root_path, target_name)
        })
        .result
        .map_err(|failure| failure.into_diagnostics())?;
    for (owner, _) in package_inputs.packages() {
        let required =
            dangerous_service_acceptance::required_dangerous_services(&preliminary, owner);
        bindings.extend(dangerous_service_bindings(&preliminary, owner, &required)?);
    }
    let package_inputs = package_inputs
        .with_accepted_semantic_bindings(bindings)
        .map_err(|errors| {
            vec![Diagnostic::error(format!(
                "cannot admit repository fixture semantic binding: {errors:?}"
            ))]
        })?;
    // Explicit repository fixture consent, never inferred merely from Build
    // registration. Retain the exact fresh root/target/source rows and replay
    // them through ordinary package compilation; production lock admission is
    // unchanged and a stale row still fails its normal comparison.
    let consent = authored_project_root.join("accepted_assumed_bindings.txt");
    let assumed_permissions = if consent.is_file() {
        let text = fs::read_to_string(&consent)
            .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
        if text.trim() != "Accept this fixture's exact checked root assumed binding contract rows."
        {
            return Err(vec![Diagnostic::error(
                "invalid explicit assumed-binding fixture consent",
            )]);
        }
        let profile = target::TargetProfile::from_omega_target_name(target_name)
            .map_err(|error| vec![error])?;
        let package = preliminary.sources().package_identity().ok_or_else(|| {
            vec![Diagnostic::error(
                "assumed-binding fixture consent requires a root package",
            )]
        })?;
        let policy =
            package_evidence::project_checked_package_policy(&preliminary, profile, package)?;
        let mut rows: Vec<_> =
            policy
                .acceptance_rows_with_limits(
                    package_evidence::record::PackagePolicyRowLimits::default(),
                )
                .map_err(|error| vec![Diagnostic::error(error.to_string())])?
                .0
                .into_iter()
                .filter(|row| {
                    matches!(
                        row.kind(),
                        package_evidence::record::PackagePolicyRowKind::AssumedBindingContract
                            | package_evidence::record::PackagePolicyRowKind::TerminalPermission
                    )
                })
                .map(|row| {
                    (
                        optimization_core::digest_encoder::content_identity(row.key_bytes()),
                        row.into_canonical_text(),
                    )
                })
                .collect();
        if preliminary
            .execution()
            .selected_provider_plan_facts
            .assumed_bindings()
            .is_empty()
        {
            return Err(vec![Diagnostic::error(
                "assumed-binding fixture consent names no assumed contracts",
            )]);
        }
        rows.sort_unstable_by_key(|(key, _)| *key);
        Some(
            package_evidence::permission_acceptance::TerminalPermissionAcceptance {
                schema_version: package_evidence::record::PACKAGE_POLICY_ROW_VERSION,
                package,
                target: profile,
                rows,
            },
        )
    } else {
        None
    };
    Ok(ReviewedFixtureCompilation {
        root_path,
        package_inputs: Some(package_inputs),
        assumed_permissions,
    })
}

/// Execute every dependency Build occurrence the root compilation requires
/// and join their generated source into its inputs, the way the product
/// compile does: the occurrences go through the driver's package route
/// ([`compiler::CompilationSession::join_dependency_builds`]), dependency-first,
/// each rooted at its own package with its own build edges, staged in its own
/// writable directory, and run for the product target or, for a Build
/// occurrence, the host's execution profile — as the package manager plans
/// them. Without this a fixture whose dependency generates source compiles
/// without that source and can check where `omega --check` refuses.
///
/// A product-scope occurrence's generated source is target-scoped, so a
/// targetless compile cannot consume it; that compile keeps its dependencies'
/// authored source only. A fixture whose product dependency generates source
/// binds a target in its own `build.omg` so its check leg realizes one.
fn join_dependency_builds(
    graph: &FixturePackageGraph,
    inputs: PackageCompilationInputs,
    root_path: &Path,
    authored_project_root: &Path,
    target_name: Option<&str>,
) -> Result<PackageCompilationInputs, Vec<Diagnostic>> {
    let consumes_product_output = inputs
        .required_dependency_source_instances()
        .iter()
        .any(|(_, purpose)| purpose.is_product());
    if target_name.is_none() && consumes_product_output {
        return Ok(inputs);
    }
    let occurrences = graph.dependency_occurrences(&inputs);
    if occurrences.is_empty() {
        return Ok(inputs);
    }
    let execution_profile = TargetProfile::host_if_supported();
    let staging = {
        let fixture_staging =
            sealed_fixture_staging::fresh_staging_build_dir(&repo_root(), authored_project_root);
        let mut name = fixture_staging
            .file_name()
            .expect("fixture staging has a name")
            .to_os_string();
        name.push("-dependencies");
        fixture_staging.with_file_name(name)
    };
    let _ = fs::remove_dir_all(&staging);
    let mut requests = Vec::with_capacity(occurrences.len() + 1);
    for DependencyOccurrence {
        purpose,
        snapshot,
        inputs: occurrence_inputs,
        ..
    } in occurrences
    {
        let occurrence_target = if purpose.is_product() {
            target_name.map(str::to_owned)
        } else {
            let Some(profile) = execution_profile else {
                return Err(vec![Diagnostic::error(
                    "Build dependencies require an admitted execution profile",
                )]);
            };
            Some(profile.target_name().to_owned())
        };
        let main = snapshot.join("main.omg");
        let entry = if main.is_file() {
            main
        } else {
            snapshot.join("build.omg")
        };
        requests.push(CheckedCompileRequest {
            package_inputs: Some(occurrence_inputs),
            build_execution_profile: execution_profile,
            build_dir: Some(staging.join(format!(
                "{}-{}",
                requests.len(),
                if purpose.is_product() {
                    "product"
                } else {
                    "build"
                }
            ))),
            build_snapshot: Some(package_compilation::BuildSnapshotRequest::new(
                std::iter::empty::<Vec<u8>>(),
            )),
            ..CheckedCompileRequest::new(&entry, occurrence_target.as_deref())
        });
    }
    requests.push(CheckedCompileRequest {
        package_inputs: Some(inputs),
        build_execution_profile: execution_profile,
        ..CheckedCompileRequest::new(root_path, target_name)
    });
    with_dependency_build_session(|session| session.join_dependency_builds(requests))
        .map_err(|failure| failure.failure.into_diagnostics())
}

/// Lend `work` a compiler session reserved for dependency Build occurrences,
/// reused across fixtures.
///
/// The bundled standard library is a Product dependency of most fixtures, and
/// its occurrence has the same source custody, inputs and target in every one
/// of them. A session keeps prepared source and the pure-Build store across
/// requests and matches complete inputs before reusing either, which is how
/// one package invocation serves several targets; lending the same sessions
/// to every fixture lets that occurrence prepare once per worker instead of
/// once per compile. A session is held by one worker at a time and returns to
/// the pool afterwards; a panicking compile drops its session.
fn with_dependency_build_session<T>(
    work: impl FnOnce(&mut compiler::CompilationSession) -> T,
) -> T {
    static IDLE: std::sync::Mutex<Vec<compiler::CompilationSession>> =
        std::sync::Mutex::new(Vec::new());
    let idle = || {
        IDLE.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    };
    let mut session = idle().pop().unwrap_or_default();
    let result = work(&mut session);
    idle().push(session);
    result
}

/// The package names a fixture's `accepted_entry_supplier.txt` declares, one
/// per line; blank lines and `//` comments are inert. The file is optional:
/// absent means the bundled library's reviewed candidate is the accepted one.
fn accepted_entry_suppliers(authored_project_root: &Path) -> Vec<String> {
    let Ok(text) = fs::read_to_string(authored_project_root.join("accepted_entry_supplier.txt"))
    else {
        return Vec::new();
    };
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("//"))
        .map(str::to_owned)
        .collect()
}

/// The reviewed `ProgramEntry` binding a fixture explicitly accepted from one
/// declared dependency. The named package must be one of the fixture's
/// declared dependencies, and the candidate is derived with the supplier's own
/// authored edges so its contract source compiles exactly the way the
/// consumer's package graph would compile it.
fn accepted_supplier_program_entry_binding(
    authored_project_root: &Path,
    target_name: &str,
    supplier_name: &str,
    package_inputs: &PackageCompilationInputs,
) -> Result<AcceptedSemanticBinding, Vec<Diagnostic>> {
    let dependency = fixture_path_dependencies(authored_project_root)
        .into_iter()
        .find(|dependency| {
            dependency.package_name == supplier_name || dependency.alias == supplier_name
        })
        .unwrap_or_else(|| {
            panic!(
                "fixture {} names `{supplier_name}` in accepted_entry_supplier.txt, which is not a declared dependency",
                authored_project_root.display()
            )
        });
    let (supplier, supplier_root) = package_inputs
        .packages()
        .find(|(identity, _)| {
            package_inputs.package_name(*identity) == Some(dependency.package_name.as_str())
        })
        .unwrap_or_else(|| {
            panic!(
                "fixture {}: accepted entry supplier `{supplier_name}` is missing from the bound package graph",
                authored_project_root.display()
            )
        });
    let mut packages = vec![PackageSourceBinding::new(
        supplier,
        dependency.package_name.clone(),
        supplier_root.to_path_buf(),
    )];
    let mut dependencies = Vec::new();
    for sub_dependency in fixture_path_dependencies(&dependency.location) {
        let (sub_identity, sub_root) = package_inputs
            .packages()
            .find(|(identity, _)| {
                package_inputs.package_name(*identity)
                    == Some(sub_dependency.package_name.as_str())
            })
            .unwrap_or_else(|| {
                panic!(
                    "fixture {}: accepted entry supplier `{supplier_name}` depends on `{}`, which is missing from the bound package graph",
                    authored_project_root.display(),
                    sub_dependency.package_name
                )
            });
        packages.push(PackageSourceBinding::new(
            sub_identity,
            sub_dependency.package_name,
            sub_root.to_path_buf(),
        ));
        dependencies.push(PackageDependencyBinding::for_purpose(
            supplier,
            sub_dependency.alias,
            sub_identity,
            sub_dependency.purpose,
        ));
    }
    derive_supplier_program_entry_binding(
        target_name,
        supplier,
        supplier_root,
        packages,
        dependencies,
    )?
    .ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "accepted entry supplier `{supplier_name}` has no reviewed ProgramEntry candidate for {target_name}"
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
                    &preliminary.providers().boundary_calling_plan_realizations,
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
                    &preliminary.providers().boundary_calling_plan_realizations,
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
