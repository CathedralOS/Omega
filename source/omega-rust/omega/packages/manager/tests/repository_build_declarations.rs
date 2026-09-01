use omega_package_manager::declarations::PackageName;
use omega_package_manager::declarations::{
    BuildDeclaration, BuildDeclarationError, WorkspaceMemberPath, extract_build_declaration,
};
use omega_package_manager::resolution::graph::{
    PackageSourceClosureLimits, resolve_external_local_project_closure_with_storage,
};
use omega_package_source::{ExternalSourceContext, LocalSourceLimits, SourceResolverStorage};
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
        .find(|ancestor| ancestor.join("TASKS_PACKAGE_MANAGER.md").is_file())
        .expect("omega-package-manager should live beneath the Omega repository")
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
        return format!("{category}-vending-machine");
    }
    leaf.replace('_', "-")
}

fn expected_omega_case_application_name(root: &Path) -> String {
    let leaf = root
        .file_name()
        .and_then(|name| name.to_str())
        .expect("Omega case root must have a UTF-8 leaf name");
    if root.ends_with("pass/arithmetic/float_trapping_invalid_traps")
        || root.ends_with("pass/arithmetic/float_trapping_overflow_traps")
    {
        return format!("arithmetic-{}", leaf.replace('_', "-"));
    }
    if root.ends_with("pass/float/build_runtime_semantics_twins_windows_x64") {
        return "windows-x64-baseline-float-semantic-edge-twin".to_owned();
    }
    if root.ends_with("pass/float/build_runtime_semantics_twins_x86_baseline") {
        return "x86-baseline-float-semantic-edge-twin".to_owned();
    }
    leaf.replace('_', "-")
}

const DECLARATION_REJECTION_CASES: &[&str] = &["fail/build/build-machine-wrong-arity"];

fn omega_case_key(cases: &Path, root: &Path) -> String {
    root.strip_prefix(cases)
        .expect("Omega case root must be beneath the Omega case corpus")
        .to_string_lossy()
        .replace(['_', '\\'], "-")
}

#[test]
fn repository_workspace_declares_its_members_in_authored_order() {
    let declaration = extract_build_declaration(repository_root()).unwrap();
    assert_eq!(
        declaration,
        BuildDeclaration::Workspace(omega_package_manager::declarations::WorkspaceDeclaration {
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
        BuildDeclaration::Application(
            omega_package_manager::declarations::ApplicationDeclaration {
                name: PackageName::parse("omega-compiler").unwrap(),
            }
        )
    );
    assert_eq!(
        extract_build_declaration(root.join("source/psi")).unwrap(),
        BuildDeclaration::Package(omega_package_manager::declarations::PackageDeclaration {
            name: PackageName::parse("psi").unwrap(),
        })
    );
    assert_eq!(
        extract_build_declaration(root.join("source/library/std")).unwrap(),
        BuildDeclaration::Package(omega_package_manager::declarations::PackageDeclaration {
            name: PackageName::parse("omega-language-std").unwrap(),
        })
    );
}

#[test]
fn compiler_product_and_parser_resolve_standard_library_as_an_ordinary_dependency() {
    let repository = repository_root();
    let temp = TempTree::new("ordinary-standard-library-edges");
    let storage = SourceResolverStorage::for_hardened_base(temp.0.join("resolved"))
        .expect("create repository project resolver storage");

    let compiler = resolve_external_local_project_closure_with_storage(
        &repository.join("source/omega"),
        ExternalSourceContext::derive(b"repository-compiler-product"),
        omega_target::TargetProfile::CrossPlatformCli,
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
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
        "omega-language-std"
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

    let parser = resolve_external_local_project_closure_with_storage(
        &repository.join("source/psi"),
        ExternalSourceContext::derive(b"repository-parser-package"),
        omega_target::TargetProfile::CrossPlatformCli,
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
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
        "omega-language-std"
    );
}

#[test]
fn executable_samples_declare_canonical_application_roles() {
    let samples = repository_root().join("samples");
    let mut roots = Vec::new();
    collect_build_roots(&samples, &mut roots);
    assert_eq!(roots.len(), 140, "unexpected executable sample population");

    for root in roots {
        let expected_name = expected_sample_application_name(&root);
        assert_eq!(
            extract_build_declaration(&root).unwrap_or_else(|error| {
                panic!(
                    "project role projection failed for {}: {error}",
                    root.display()
                )
            }),
            BuildDeclaration::Application(
                omega_package_manager::declarations::ApplicationDeclaration {
                    name: PackageName::parse(&expected_name).unwrap(),
                }
            ),
            "unexpected sample application declaration in {}",
            root.display()
        );
    }
}

#[test]
fn ordinary_omega_case_projects_declare_canonical_application_roles() {
    let cases = repository_root().join("tests/omega");
    let mut roots = Vec::new();
    collect_build_roots(&cases, &mut roots);
    assert!(!roots.is_empty(), "Omega case corpus must not be empty");
    let root_count = roots.len();

    let mut applications = 0;
    let mut declaration_rejections = 0;
    for root in roots {
        if DECLARATION_REJECTION_CASES.contains(&omega_case_key(&cases, &root).as_str()) {
            assert_eq!(
                extract_build_declaration(&root),
                Err(BuildDeclarationError::InvalidBuildParameter),
                "unexpected declaration rejection in {}",
                root.display()
            );
            declaration_rejections += 1;
            continue;
        }

        let expected_name = expected_omega_case_application_name(&root);
        assert_eq!(
            extract_build_declaration(&root).unwrap_or_else(|error| {
                panic!(
                    "project role projection failed for {}: {error}",
                    root.display()
                )
            }),
            BuildDeclaration::Application(
                omega_package_manager::declarations::ApplicationDeclaration {
                    name: PackageName::parse(&expected_name).unwrap(),
                }
            ),
            "unexpected Omega case application declaration in {}",
            root.display()
        );
        applications += 1;
    }

    assert_eq!(declaration_rejections, DECLARATION_REJECTION_CASES.len());
    assert_eq!(applications + declaration_rejections, root_count);
}
