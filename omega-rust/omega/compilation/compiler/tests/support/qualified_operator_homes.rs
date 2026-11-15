//! Normal package preparation must retain operator homes through publication.

use std::path::{Path, PathBuf};

const FILES: &[&str] = &[
    "app/main.omg",
    "app/build.omg",
    "provider/main.omg",
    "provider/units.omg",
    "provider/operations.omg",
    "provider/build.omg",
];

struct Project(PathBuf);

impl Project {
    fn copy_fixture(label: &str) -> Self {
        let projects =
            super::unbuilt_frontier::repository("build/qualified-operator-home/test-projects");
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

fn terminal_product(
    main: &Path,
) -> Result<
    terminal_psi_to_abstract_operations::RetainedTerminalArtifact,
    Vec<diagnostics::Diagnostic>,
> {
    // The public application owner settles the authored graph, admits its real
    // Product and services, and publishes the selected entry. Fixture-specific
    // semantic-role discovery supplies no alternative acceptance here.
    let mut outcomes = orchestration::build(orchestration::BuildRequest {
        stage_validators: Default::default(),
        root_path: main.to_path_buf(),
        build_directory: Some(main.parent().unwrap().join("build/terminal")),
        targets: orchestration::BuildTargets::Single(
            target::TargetProfile::from_canonical_target_name("linux_x86_64")
                .map_err(|error| vec![error])?,
        ),
        product: orchestration::ProjectProduct::Terminal,
        timings: false,
        offline: true,
        verify: false,
        require_package_project: true,
        optimization_rollback: Default::default(),
        build_snapshot: None,
    })
    .map_err(orchestration::BuildError::into_diagnostics)?;
    assert_eq!(outcomes.len(), 1, "one actual target");
    outcomes
        .pop()
        .unwrap()
        .result
        .and_then(orchestration::BuildOutcome::into_report)
        .map_err(orchestration::BuildError::into_diagnostics)?
        .report
        .into_retained_terminal_artifact()
        .ok_or_else(|| {
            vec![diagnostics::Diagnostic::error(
                "ordinary Terminal Product retained no artifact",
            )]
        })
}

#[test]
fn domain_operators_cross_package_boundaries_without_importing_their_module() {
    let _guard = super::FIXTURE_COMPILATIONS.lock().unwrap();
    std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            let positive = Project::copy_fixture("public");
            let product = terminal_product(&positive.main())
                .unwrap_or_else(|diagnostics| panic!("public operator source: {diagnostics:?}"));
            let artifact = product.artifact();
            let evidence = product.evidence().expect("actual accepted operation seals");
            assert_eq!(evidence.module, artifact.view().identity());
            // Total wrapping arithmetic and predicate-free domains need no
            // safety/contract seal. The separate requires twin pins a real
            // call obligation without changing this program to manufacture one.
            let entry = product
                .native_realization_proposal()
                .unwrap()
                .program_entry();
            assert_eq!(
                entry.checked_entry().terminal_psi_identity(),
                artifact.manifest().semantic()
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
            let failure = terminal_product(&private.main())
                .expect_err("a public domain must not publish its private operator");
            let diagnostics = failure;
            assert!(
                diagnostics.iter().any(|diagnostic| {
                    // The package name also contains "operator": it must not
                    // make a setup/refusal elsewhere count as this oracle.
                    diagnostic.message.contains(
                        "operator on an operand that is not a plain or policy-qualified scalar",
                    ) || diagnostic.message.contains("no visible operator")
                        || diagnostic.message.contains("private operator")
                }),
                "{diagnostics:#?}"
            );
        })
        .unwrap()
        .join()
        .unwrap();
    });
}

#[test]
fn a_selected_operator_keeps_its_authored_call_requirement() {
    let _guard = super::FIXTURE_COMPILATIONS.lock().unwrap();
    std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            let restricted = Project::copy_fixture("requires");
            let declaration = restricted.0.join("provider/main.omg");
            let original = std::fs::read_to_string(&declaration).unwrap();
            let changed = original.replacen(
                ") -> u32 in Distance {",
                ") -> u32 in Distance requires (left as -Distance) <= 1u32 {",
                1,
            );
            assert_ne!(changed, original);
            std::fs::write(declaration, changed).unwrap();
            let diagnostics = terminal_product(&restricted.main())
                .expect_err("the unchanged left operand 2 cannot satisfy the selected requirement");
            assert!(
                diagnostics.iter().any(|diagnostic| {
                    diagnostic.message.contains("cannot prove")
                        && diagnostic.message.contains("requires")
                }),
                "{diagnostics:#?}"
            );
        })
        .unwrap()
        .join()
        .unwrap();
    });
}
