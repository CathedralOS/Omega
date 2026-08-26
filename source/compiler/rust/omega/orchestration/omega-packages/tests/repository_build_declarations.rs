use omega_packages::{
    BuildDeclaration, PackageName, WorkspaceMemberPath, extract_build_declaration,
};
use std::fs;
use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../../..")
}

fn collect_sample_roots(directory: &Path, roots: &mut Vec<PathBuf>) {
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
            collect_sample_roots(&path, roots);
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
    collect_sample_roots(&samples, &mut roots);
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
