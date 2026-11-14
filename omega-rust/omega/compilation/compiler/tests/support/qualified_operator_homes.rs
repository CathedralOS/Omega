//! Normal package preparation must retain operator homes through publication.

use std::path::{Path, PathBuf};

#[path = "fixture_package_inputs.rs"]
mod fixture_package_inputs;

const FILES: &[&str] = &[
    "app/main.omg",
    "app/build.omg",
    "provider/main.omg",
    "provider/units.omg",
    "provider/build.omg",
];

struct Project(PathBuf);

impl Project {
    fn copy_fixture(label: &str) -> Self {
        let projects =
            fixture_package_inputs::repo_root().join("build/qualified-operator-home/test-projects");
        std::fs::create_dir_all(&projects).unwrap();
        let root = (0u64..)
            .find_map(|sequence| {
                let path = projects.join(format!(
                    "omega-operator-home-{}-{label}-{sequence}",
                    std::process::id()
                ));
                match std::fs::create_dir(&path) {
                    Ok(()) => Some(path),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => None,
                    Err(error) => panic!("create private fixture: {error}"),
                }
            })
            .unwrap();
        let fixture = super::unbuilt_frontier::repository(
            "tests/fixtures/run/operators/qualified_operator_home",
        );
        for file in FILES {
            let target = root.join(file);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::copy(fixture.join(file), target).unwrap();
        }
        Self(root)
    }

    fn main(&self) -> PathBuf {
        self.0.join("app/main.omg")
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn check(main: &Path) -> Result<compiler::CheckedCompilation, Vec<diagnostics::Diagnostic>> {
    // Dependencies belong to the same executed discovery and Product admission
    // used by package-aware fixtures, before checking the retained target plan.
    let mut prepared = fixture_package_inputs::reviewed_repository_fixture_package_inputs(
        main,
        Some("linux_x86_64"),
    )?;
    fixture_package_inputs::check_fixture(
        &mut prepared,
        "linux_x86_64",
        compiler::OptimizationRollback::default(),
    )
}

#[test]
fn domain_operators_cross_package_boundaries_without_importing_their_module() {
    let _guard = super::FIXTURE_COMPILATIONS.lock().unwrap();
    std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            let positive = Project::copy_fixture("public");
            let checked = check(&positive.main())
                .unwrap_or_else(|diagnostics| panic!("public operator source: {diagnostics:?}"));
            let accepted = checked.terminal_publication().accepted().unwrap();
            let publication = accepted.closure().publish().unwrap();
            assert_eq!(publication.artifact.semantic_module(), accepted.module());
            let (_, application) = compiler::prepare_program_entry(&checked).unwrap();
            assert_eq!(
                application
                    .receipt
                    .unwrap()
                    .unwrap()
                    .terminal_psi_identity(),
                publication.artifact.manifest().semantic()
            );

            let private = Project::copy_fixture("private");
            let declaration = private.0.join("provider/main.omg");
            let original = std::fs::read_to_string(&declaration).unwrap();
            let changed = original.replacen(
                "public machine + Distance::add",
                "machine + Distance::add",
                1,
            );
            assert_ne!(changed, original);
            std::fs::write(declaration, changed).unwrap();
            let failure = check(&private.main())
                .expect_err("a public domain must not publish its private operator");
            let diagnostics = failure;
            assert!(
                diagnostics.iter().any(|diagnostic| {
                    diagnostic.message.contains("operator")
                        || diagnostic.message.contains("private")
                }),
                "{diagnostics:#?}"
            );
        })
        .unwrap()
        .join()
        .unwrap();
    });
}
