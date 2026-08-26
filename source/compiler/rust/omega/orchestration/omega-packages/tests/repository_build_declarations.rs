use omega_packages::{
    BuildDeclaration, PackageName, WorkspaceMemberPath, extract_build_declaration,
};
use std::fs;
use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../../..")
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

const ROLE_MIGRATION_EXCEPTIONS: &[&str] = &[
    "fail/build/build-boundary-rowless",
    "fail/build/build-effects-undeclared",
    "fail/build/build-service-name-spoof",
    "pass/providers/component-owner-provider-override-compile",
    "pass/providers/test-owner-provider-override-compile",
];

fn canary_migration_key(canaries: &Path, root: &Path) -> String {
    root.strip_prefix(canaries)
        .expect("canary root must be beneath the canary corpus")
        .to_string_lossy()
        .replace(['_', '\\'], "-")
}

fn expected_canary_application_name(root: &Path) -> String {
    let leaf = root
        .file_name()
        .and_then(|name| name.to_str())
        .expect("canary root must have a UTF-8 leaf name");
    if root.ends_with("pass/arithmetic/float_trapping_invalid_traps")
        || root.ends_with("pass/arithmetic/float_trapping_overflow_traps")
    {
        return format!("arithmetic-{}", leaf.replace('_', "-"));
    }
    leaf.replace('_', "-")
}

#[test]
fn repository_workspace_declares_its_members_in_authored_order() {
    let declaration = extract_build_declaration(repository_root()).unwrap();
    assert_eq!(
        declaration,
        BuildDeclaration::Workspace(omega_packages::WorkspaceDeclaration {
            members: vec![
                WorkspaceMemberPath::parse("omega/language/std").unwrap(),
                WorkspaceMemberPath::parse("source/compiler/omega/psi").unwrap(),
                WorkspaceMemberPath::parse("source/compiler/omega").unwrap(),
            ],
        })
    );
}

#[test]
fn compiler_application_and_standard_library_declare_their_kinds() {
    let root = repository_root();
    assert_eq!(
        extract_build_declaration(root.join("source/compiler/omega")).unwrap(),
        BuildDeclaration::Application(omega_packages::ApplicationDeclaration {
            name: PackageName::parse("omega-compiler").unwrap(),
        })
    );
    assert_eq!(
        extract_build_declaration(root.join("source/compiler/omega/psi")).unwrap(),
        BuildDeclaration::Package(omega_packages::PackageDeclaration {
            name: PackageName::parse("psi").unwrap(),
        })
    );
    assert_eq!(
        extract_build_declaration(root.join("omega/language/std")).unwrap(),
        BuildDeclaration::Package(omega_packages::PackageDeclaration {
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
            BuildDeclaration::Application(omega_packages::ApplicationDeclaration {
                name: PackageName::parse(&expected_name).unwrap(),
            }),
            "unexpected sample application declaration in {}",
            root.display()
        );
    }
}

#[test]
fn ordinary_canary_projects_declare_canonical_application_roles() {
    let canaries = repository_root().join("tests/canaries");
    let mut roots = Vec::new();
    collect_build_roots(&canaries, &mut roots);
    assert_eq!(
        roots.len(),
        1_115,
        "unexpected canary build-root population"
    );

    let mut exceptions = 0;
    let mut applications = 0;
    for root in roots {
        let migration_key = canary_migration_key(&canaries, &root);
        if ROLE_MIGRATION_EXCEPTIONS.contains(&migration_key.as_str()) {
            exceptions += 1;
            continue;
        }

        let expected_name = expected_canary_application_name(&root);
        assert_eq!(
            extract_build_declaration(&root).unwrap_or_else(|error| {
                panic!(
                    "project role projection failed for {}: {error}",
                    root.display()
                )
            }),
            BuildDeclaration::Application(omega_packages::ApplicationDeclaration {
                name: PackageName::parse(&expected_name).unwrap(),
            }),
            "unexpected canary application declaration in {}",
            root.display()
        );
        applications += 1;
    }

    assert_eq!(exceptions, ROLE_MIGRATION_EXCEPTIONS.len());
    assert_eq!(applications, 1_110);
}
