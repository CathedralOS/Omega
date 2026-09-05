use super::*;

fn generated_project() -> Project {
    let project = Project::new();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|ancestor| ancestor.join("tests/fixtures/packages").is_dir())
        .unwrap()
        .join("tests/fixtures/packages/generated-table");
    for relative in ["build.omg", "main.omg", "inputs/table.txt"] {
        project.write(
            &format!("producer/{relative}"),
            &fs::read_to_string(fixture.join(relative)).unwrap(),
        );
    }
    project
}

#[test]
fn generated_package_reports_after_sponsored_staging_disposal() {
    let project = generated_project();
    let authored = fs::read(project.0.join("producer/build.omg")).unwrap();
    let report = check_prepared_local_project(
        project
            .request("producer/main.omg", "observations")
            .with_artifact_policy(ArtifactEmissionPolicy::Full),
    )
    .expect("direct generated-source CHECK keeps sponsored staged-output custody");
    assert_check_only(&report);
    assert!(report.trust_admission_settlement().is_exactly_admitted());
    let contracts = fs::read_to_string(project.0.join("observations/05_machine_contracts.json"))
        .expect("normal checked observations reach the requested build directory");
    assert!(contracts.contains("table_size"));
    assert!(project.0.join("observations/00_timings.html").is_file());
    assert!(
        fs::read_dir(project.0.join("observations"))
            .unwrap()
            .all(|entry| !entry.unwrap().file_type().unwrap().is_dir()),
        "reporting leaves no sponsored session or generated staging directory"
    );
    assert_eq!(
        fs::read(project.0.join("producer/build.omg")).unwrap(),
        authored
    );
    assert!(!project.0.join("producer/table.generated.omg").exists());
    assert!(!project.0.join("producer/omega.lock").exists());
}

#[test]
fn generated_dependency_reaches_requested_package_and_application_entries() {
    for role in ["package", "application"] {
        let project = generated_project();
        project.write(
            "consumer/build.omg",
            &format!(
                r#"
machine build(builder: &mut Build) {{
    builder.{role}("generated-consumer");
    builder.depend_as("generated_table", Source::Path {{ location: "../producer" }});
}}
"#
            ),
        );
        project.write("consumer/main.omg", "the unselected entry is invalid\n");
        project.write(
            "consumer/entry.omg",
            "use generated_table::main;\nmachine main() -> u64 { table_size() }\n",
        );
        let report = check_prepared_local_project(project.request("consumer/entry.omg", "output"))
            .expect("dependency bundle enters either role's selected entry");
        assert_check_only(&report);
        assert!(report.trust_admission_settlement().is_exactly_admitted());
        assert_eq!(report.root_path().file_name().unwrap(), "entry.omg");
        assert_empty_directory(&project.0.join("output"));
        assert!(!project.0.join("consumer/table.generated.omg").exists());
    }
}

#[test]
fn invalid_generated_proof_rejects_and_disposes_staging() {
    let project = generated_project();
    let build = fs::read_to_string(project.0.join("producer/build.omg")).unwrap();
    project.write(
        "producer/build.omg",
        &build.replace(
            "table_size() -> u64 {",
            "table_size() -> u64 ensures result == 4 {",
        ),
    );
    let result = check_prepared_local_project(project.request("producer/main.omg", "rejected"));
    assert!(matches!(
        result,
        Err(CheckPreparedLocalProjectError::Review(
            CompileResolvedPackageReviewsError::Compilation { .. }
        ))
    ));
    assert_empty_directory(&project.0.join("rejected"));
    assert!(!project.0.join("producer/table.generated.omg").exists());
}
