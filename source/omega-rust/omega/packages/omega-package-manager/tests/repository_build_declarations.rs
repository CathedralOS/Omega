use omega_package_manager::declarations::{
    BuildDeclaration, PackageDeclarationError, WorkspaceMemberPath, extract_build_declaration,
};
use omega_package_manager::identity::PackageName;
use std::fs;
use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../..")
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
                Err(PackageDeclarationError::InvalidBuildParameter),
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
