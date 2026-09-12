//! The package-qualified customer keeps nominal identity through native execution.

use super::membership::execute;
use super::*;
use package_compilation::{
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use semantic_vocabulary::PackageKeyIdentity;
use std::path::PathBuf;

struct Sources(PathBuf);

impl Drop for Sources {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn package_qualified_constructors_and_locals_execute_with_their_declaring_case() {
    let directory = std::env::temp_dir().join(format!(
        "omega-native-package-membership-{}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).unwrap();
    let sources = Sources(directory);
    let root = sources.0.join("root");
    let shapes = sources.0.join("shapes");
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(&shapes).unwrap();
    std::fs::write(
        shapes.join("settings.omg"),
        "module settings; pub data Choice { case Empty; case Some(value: u32); }",
    )
    .unwrap();
    let identity = |marker| PackageKeyIdentity::from_digest([marker; 32]).unwrap();
    for (constructor, expected) in [
        ("shapes::settings::Choice::Empty", true),
        ("shapes::settings::Choice::Empty {}", true),
        ("shapes::settings::Choice::Some { value: 37 }", false),
    ] {
        std::fs::write(
            root.join("main.omg"),
            format!(
                "use shapes::settings;
             data Choice {{ case Some(value: bool); case Empty; }}
             machine is_empty() -> bool {{ {constructor} in shapes::settings::Choice::Empty }}
             machine is_some() -> bool {{ {constructor} in shapes::settings::Choice::Some }}
             machine local_is_empty() -> bool {{
                 let value: shapes::settings::Choice = {constructor};
                 value in shapes::settings::Choice::Empty
             }}"
            ),
        )
        .unwrap();
        let inputs = PackageCompilationInputs::new_package(
            identity(1),
            vec![
                PackageSourceBinding::new(identity(1), "root", root.clone()),
                PackageSourceBinding::new(identity(2), "shapes", shapes.clone()),
            ],
            vec![PackageDependencyBinding::new(
                identity(1),
                "shapes",
                identity(2),
            )],
        )
        .unwrap();
        let checked = compiler::compile_to_checked(compiler::CheckedCompileRequest {
            package_inputs: Some(inputs),
            ..compiler::CheckedCompileRequest::new(&root.join("main.omg"), None)
        })
        .expect("package-qualified case customer checks");
        for (entry, expected) in [
            ("is_empty", expected),
            ("is_some", !expected),
            ("local_is_empty", expected),
        ] {
            let artifact = terminal_production::produce_terminal_artifact(&checked, entry)
                .expect("package-qualified construction reaches Terminal");
            let artifact = CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes()).unwrap();
            execute(
                &artifact,
                &format!(
                    "#include <stdbool.h>\nextern bool omega_entry(void);\nint main(void) {{ return omega_entry() == {expected} ? 0 : 1; }}"
                ),
            );
        }
    }
}
