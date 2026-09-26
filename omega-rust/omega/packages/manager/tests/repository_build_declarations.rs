use package_manager::declarations::PackageName;
use package_manager::declarations::{
    BuildDeclaration, DependencySourceRequest, WorkspaceMemberPath, extract_build_declaration,
    extract_build_dependency_projection,
};
use package_manager::resolution::graph::GitResolutionOptions;
use package_manager::resolution::graph::{
    PackageSourceClosureLimits, resolve_external_local_project_closure,
};
use package_source::PrimaryGitChoices;
use package_source::{ExternalSourceContext, LocalSourceLimits, SourceResolverStorage};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempTree(PathBuf);

impl TempTree {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "omega-repository-build-{name}-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create repository build test tree");
        Self(path)
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|ancestor| {
            ancestor.join("Cargo.toml").is_file() && ancestor.join("source/omega").is_dir()
        })
        .expect("package-manager should live beneath the Omega repository")
        .to_path_buf()
}

fn collect_build_roots(directory: &Path, roots: &mut Vec<PathBuf>) {
    let mut entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("read sample directory {}: {error}", directory.display()))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|error| panic!("read sample entry in {}: {error}", directory.display()));
    entries.sort_by_key(fs::DirEntry::file_name);

    if entries.iter().any(|entry| entry.file_name() == "build.omg") {
        roots.push(directory.to_owned());
    }

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_build_roots(&path, roots);
        }
    }
}

/// Registered submodule paths, which this repository does not own.
///
/// A submodule carries another repository's content and its own gates, and
/// this checkout must not edit it, so a violation inside one cannot be
/// repaired here -- asserting on it only makes this check permanently red and
/// blind to the samples it does govern. `samples/apps/squalr` is the live
/// case: all seventeen of its member declarations spell hyphenated package
/// identities, which Omega's snake_case identity rule refuses before any role
/// is projected.
fn submodule_roots() -> Vec<PathBuf> {
    let modules = repository_root().join(".gitmodules");
    let Ok(text) = fs::read_to_string(modules) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| line.trim().strip_prefix("path = "))
        .map(|path| repository_root().join(path.trim()))
        .collect()
}

fn expected_sample_application_name(root: &Path) -> String {
    let leaf = root
        .file_name()
        .and_then(|name| name.to_str())
        .expect("sample root must have a UTF-8 leaf name");
    if leaf == "vending_machine" {
        let category = root
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .expect("duplicate sample name must have a UTF-8 category");
        return format!("{category}_vending_machine");
    }
    leaf.to_owned()
}

fn expected_omega_case_application_name(root: &Path) -> String {
    if root.ends_with("pass/float/build_runtime_semantics_twins_windows_x64") {
        return "windows_x64_baseline_float_semantic_edge_twin".to_owned();
    }
    if root.ends_with("pass/float/build_runtime_semantics_twins_x86_baseline") {
        return "x86_baseline_float_semantic_edge_twin".to_owned();
    }
    root.file_name()
        .and_then(|name| name.to_str())
        .expect("Omega case root must have a UTF-8 leaf name")
        .to_owned()
}

/// Samples that bind hosted entries without any package dependency, each
/// stating why in its own `build.omg`: `uefi/uefi_hello` is freestanding, the
/// two wrapping-arithmetic subjects and `cli/basics/standalone` are
/// dependency-free benchmark subjects `tools/benchmark` measures without
/// settling a dependency graph, and `cli/proofs/structural_proofs` emits no
/// runtime code. Every other sample declares the ordinary std edge.
const DEPENDENCY_FREE_SAMPLES: &[&str] = &[
    "cli/arithmetic/wrapping_collatz_max",
    "cli/arithmetic/wrapping_square_sum",
    "cli/basics/standalone",
    "cli/proofs/structural_proofs",
    "uefi/uefi_hello",
];

#[test]
fn repository_workspace_declares_its_members_in_authored_order() {
    let declaration = extract_build_declaration(repository_root()).unwrap();
    assert_eq!(
        declaration,
        BuildDeclaration::Workspace(package_manager::declarations::WorkspaceDeclaration {
            members: vec![
                WorkspaceMemberPath::parse("source/library/std").unwrap(),
                WorkspaceMemberPath::parse("source/psi").unwrap(),
                WorkspaceMemberPath::parse("source/omega").unwrap(),
            ],
        })
    );
}

#[test]
fn compiler_application_and_standard_library_declare_their_kinds() {
    let root = repository_root();
    assert_eq!(
        extract_build_declaration(root.join("source/omega")).unwrap(),
        BuildDeclaration::Application(package_manager::declarations::ApplicationDeclaration {
            name: PackageName::parse("omega_compiler").unwrap(),
            artifact_only: false,
        })
    );
    assert_eq!(
        extract_build_declaration(root.join("source/psi")).unwrap(),
        BuildDeclaration::Package(package_manager::declarations::PackageDeclaration {
            name: PackageName::parse("psi").unwrap(),
        })
    );
    assert_eq!(
        extract_build_declaration(root.join("source/library/std")).unwrap(),
        BuildDeclaration::Package(package_manager::declarations::PackageDeclaration {
            name: PackageName::parse("omega_language_std").unwrap(),
        })
    );
}

#[test]
fn compiler_product_and_parser_resolve_standard_library_as_an_ordinary_dependency() {
    let repository = repository_root();
    let temp = TempTree::new("ordinary-standard-library-edges");
    let storage = SourceResolverStorage::for_hardened_base(
        temp.0.join("resolved"),
        PrimaryGitChoices::default(),
    )
    .expect("create repository project resolver storage");

    let compiler = resolve_external_local_project_closure(
        repository.join("source/omega"),
        ExternalSourceContext::derive(b"repository-compiler-product"),
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
        GitResolutionOptions::default(),
    )
    .expect("resolve compiler product dependency closure");
    assert_eq!(compiler.graph().packages().len(), 3);
    let compiler_root = compiler
        .graph()
        .package(compiler.graph().root())
        .expect("compiler product root package");
    let compiler_dependencies = compiler_root.dependencies();
    assert_eq!(compiler_dependencies.len(), 2);
    assert_eq!(compiler_dependencies[0].alias().as_str(), "psi");
    assert_eq!(compiler_dependencies[0].target().name().as_str(), "psi");
    assert_eq!(
        compiler_dependencies[1].alias().as_str(),
        "omega_language_std"
    );
    assert_eq!(
        compiler_dependencies[1].target().name().as_str(),
        "omega_language_std"
    );
    let compiler_psi = compiler
        .graph()
        .package(compiler_dependencies[0].target())
        .expect("compiler product psi dependency");
    let [psi_standard_library] = compiler_psi.dependencies() else {
        panic!("psi should declare exactly one ordinary standard-library dependency")
    };
    assert_eq!(psi_standard_library.alias().as_str(), "omega_language_std");
    assert_eq!(
        psi_standard_library.target(),
        compiler_dependencies[1].target(),
        "compiler and psi should reconcile the same standard-library package"
    );

    let parser = resolve_external_local_project_closure(
        repository.join("source/psi"),
        ExternalSourceContext::derive(b"repository-parser-package"),
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
        GitResolutionOptions::default(),
    )
    .expect("resolve parser package dependency closure");
    assert_eq!(parser.graph().packages().len(), 2);
    let parser_root = parser
        .graph()
        .package(parser.graph().root())
        .expect("parser root package");
    let [parser_standard_library] = parser_root.dependencies() else {
        panic!("parser should declare exactly one ordinary standard-library dependency")
    };
    assert_eq!(
        parser_standard_library.alias().as_str(),
        "omega_language_std"
    );
    assert_eq!(
        parser_standard_library.target().name().as_str(),
        "omega_language_std"
    );
}

#[test]
fn executable_samples_declare_canonical_roles_and_ordinary_standard_library_edges() {
    let samples = repository_root().join("samples");
    let mut roots = Vec::new();
    collect_build_roots(&samples, &mut roots);
    assert!(!roots.is_empty(), "the sample tree must not be empty");

    // A sample may be a workspace whose members carry their own declarations
    // and mirror an upstream layout. Roots arrive parent-first, so a member is
    // recognized by the workspace root it sits under.
    let mut workspaces: Vec<PathBuf> = Vec::new();
    let submodules = submodule_roots();
    for root in roots {
        if submodules
            .iter()
            .any(|submodule| root.starts_with(submodule))
        {
            continue;
        }
        let expected_name = expected_sample_application_name(&root);
        let projection = extract_build_dependency_projection(&root).unwrap_or_else(|error| {
            panic!(
                "project role/dependency projection failed for {}: {error}",
                root.display()
            )
        });
        if let BuildDeclaration::Workspace(workspace) = projection.declaration() {
            for member in &workspace.members {
                let member_root = root.join(member.as_str());
                assert!(
                    member_root.join("build.omg").is_file(),
                    "workspace {} declares member {} with no build declaration",
                    root.display(),
                    member.as_str()
                );
            }
            assert!(
                projection.product_dependencies().is_empty(),
                "a workspace root declares membership, not product dependencies, in {}",
                root.display()
            );
            workspaces.push(root);
            continue;
        }
        if let Some(workspace) = workspaces
            .iter()
            .find(|workspace| root.starts_with(workspace))
        {
            // A workspace member keeps its authored role and name; its edges
            // mirror the upstream package graph rather than the sample tree's
            // single standard-library convention, so they are the workspace
            // owner's contract and are not asserted here.
            let name = match projection.declaration() {
                BuildDeclaration::Package(package) => package.name.as_str(),
                BuildDeclaration::Application(application) => application.name.as_str(),
                BuildDeclaration::Workspace(_) => unreachable!("handled above"),
            };
            assert_eq!(
                name,
                expected_name,
                "workspace member in {} under {} must be named for its directory",
                root.display(),
                workspace.display()
            );
            continue;
        }
        assert_eq!(
            projection.declaration(),
            &BuildDeclaration::Application(package_manager::declarations::ApplicationDeclaration {
                name: PackageName::parse(&expected_name).unwrap(),
                artifact_only: false,
            }),
            "unexpected sample application declaration in {}",
            root.display()
        );

        let expected_dependencies = if DEPENDENCY_FREE_SAMPLES
            .iter()
            .any(|sample| root.ends_with(sample))
        {
            Vec::new()
        } else {
            let location = if root.starts_with(samples.join("cli")) {
                "../../../../source/library/std"
            } else if root.starts_with(samples.join("gui")) {
                "../../../source/library/std"
            } else {
                "../../source/library/std"
            };
            vec![DependencySourceRequest::Path {
                explicit_alias: None,
                location: location.to_owned(),
            }]
        };
        assert_eq!(
            projection.product_dependencies(),
            expected_dependencies,
            "unexpected sample dependency declaration in {}",
            root.display()
        );
    }
}
/// Build roots of the language corpus tiers, excluding the package projects
/// that share `tests/omega` with them.
fn omega_case_roots(tiers: &[&str]) -> Vec<PathBuf> {
    let cases = repository_root().join("tests/omega");
    let mut roots = Vec::new();
    for tier in tiers {
        collect_build_roots(&cases.join(tier), &mut roots);
    }
    assert!(!roots.is_empty(), "Omega case corpus must not be empty");
    roots
}

/// Every `.omg` source a case root owns, including member sources in
/// subdirectories such as `platform/`, except the build declaration itself and
/// the sources of nested package roots, which declare their own edges.
fn collect_case_member_sources(directory: &Path, sources: &mut Vec<PathBuf>) {
    let mut entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("read case {}: {error}", directory.display()))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|error| panic!("read case entry in {}: {error}", directory.display()));
    entries.sort_by_key(fs::DirEntry::path);
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            if !path.join("build.omg").is_file() {
                collect_case_member_sources(&path, sources);
            }
        } else if path.extension().is_some_and(|extension| extension == "omg")
            && path.file_name().is_some_and(|name| name != "build.omg")
        {
            sources.push(path);
        }
    }
}

/// A case that reaches std as an ordinary package names the
/// `omega_language_std` dependency alias, and its build declares one Path edge
/// to `source/library/std` exactly when its sources use that alias: no case
/// carries an unused edge. Cases without the edge either use no std or stay on
/// the bundled toolchain libraries (`pass/memory/interrupt_table_canary`), and
/// package-mode cases such as `pass/proofs/quotient_define_managed_compile`
/// carry package identity through `builder.package`.
#[test]
fn omega_cases_declare_a_standard_library_edge_exactly_when_they_use_it() {
    let standard_library = repository_root()
        .join("source/library/std")
        .canonicalize()
        .expect("canonical standard-library root");
    let mut violations = Vec::new();
    for root in omega_case_roots(&["pass", "run"]) {
        let mut uses_dependency_alias = false;
        let mut sources = Vec::new();
        collect_case_member_sources(&root, &mut sources);
        for source in sources {
            let contents = fs::read_to_string(&source)
                .unwrap_or_else(|error| panic!("read {}: {error}", source.display()));
            uses_dependency_alias |= contents.contains("omega_language_std");
        }

        let projection = extract_build_dependency_projection(&root).unwrap_or_else(|error| {
            panic!(
                "case role/dependency projection failed for {}: {error}",
                root.display()
            )
        });
        let standard_library_edges = projection
            .product_dependencies()
            .iter()
            .filter(|dependency| match dependency {
                DependencySourceRequest::Path { location, .. } => root
                    .join(location)
                    .canonicalize()
                    .is_ok_and(|resolved| resolved == standard_library),
                _ => false,
            })
            .count();
        if standard_library_edges != usize::from(uses_dependency_alias) {
            violations.push(format!(
                "{}: {standard_library_edges} std edges, alias used: {uses_dependency_alias}",
                root.display()
            ));
        }
    }
    assert!(violations.is_empty(), "{}", violations.join("\n"));
}

/// A case that declares an application names it after its directory, so its
/// products and diagnostics identify the case; package-mode cases and nested
/// member packages declare `builder.package` with a valid package name.
#[test]
fn omega_case_applications_are_named_after_their_case() {
    for root in omega_case_roots(&["pass", "run"]) {
        match extract_build_declaration(&root).unwrap_or_else(|error| {
            panic!(
                "project role projection failed for {}: {error}",
                root.display()
            )
        }) {
            BuildDeclaration::Application(declaration) => assert_eq!(
                declaration,
                package_manager::declarations::ApplicationDeclaration {
                    name: PackageName::parse(&expected_omega_case_application_name(&root)).unwrap(),
                    artifact_only: false,
                },
                "unexpected Omega case application declaration in {}",
                root.display()
            ),
            BuildDeclaration::Package(_) => {}
            other => panic!(
                "Omega case {} declares neither an application nor a package: {other:?}",
                root.display()
            ),
        }
    }
}

/// Declared path dependencies are load-bearing, not just spelled:
/// `pass/proofs/kernel_theorem_equality_certificates` once declared six
/// `../` segments where five reach the repository root, so its authored path
/// pointed outside the repository entirely while only the declared location
/// string was checked. Every declared `Path` dependency in the Omega case
/// corpus must resolve inside the repository to a directory bearing its own
/// `build.omg`.
#[test]
fn declared_path_dependencies_resolve_to_repository_package_roots() {
    let repository = repository_root()
        .canonicalize()
        .expect("canonical repository root");
    let mut path_dependencies = 0;
    for root in omega_case_roots(&["pass", "fail", "run"]) {
        let Ok(projection) = extract_build_dependency_projection(&root) else {
            continue;
        };
        for dependency in projection.product_dependencies() {
            let DependencySourceRequest::Path { location, .. } = dependency else {
                continue;
            };
            let resolved = root.join(location).canonicalize().unwrap_or_else(|error| {
                panic!(
                    "declared dependency path {location} in {} does not resolve: {error}",
                    root.display()
                )
            });
            assert!(
                resolved.starts_with(&repository),
                "declared dependency path {location} in {} resolves outside the repository to {}",
                root.display(),
                resolved.display()
            );
            assert!(
                resolved.join("build.omg").is_file(),
                "declared dependency path {location} in {} resolves to {} which is not a package root",
                root.display(),
                resolved.display()
            );
            path_dependencies += 1;
        }
    }
    assert!(
        path_dependencies > 0,
        "the Omega case corpus must declare path dependencies"
    );
}
