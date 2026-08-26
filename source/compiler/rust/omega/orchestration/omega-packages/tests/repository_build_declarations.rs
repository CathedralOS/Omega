use omega_packages::{
    BuildDeclaration, PackageName, WorkspaceMemberPath, extract_build_declaration,
};
use std::path::PathBuf;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../../..")
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
