use platform_custody::temporary_directory::TemporaryDirectory;
#[path = "project.rs"]
mod fixture;

/// Execute the fixture against its declared process status and compare the raw streams.
pub fn assert_declared_exit(project: &std::path::Path, entry: &str) {
    let expected: i32 = std::fs::read_to_string(project.join("expected_native_exit.txt"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let scratch = TemporaryDirectory::create("application-observations", false).unwrap();
    let directory = scratch.path();
    let output = fixture::omega(
        project,
        &["run", "--outputs", directory.to_str().unwrap(), entry],
    );
    assert!(output.status.success(), "{output:?}");
    let termination = std::fs::read_to_string(directory.join("termination.tsv")).unwrap();
    let columns: Vec<_> = termination.trim_end().split('\t').collect();
    let actual = match columns.as_slice() {
        ["psi", "exit", status] => status.parse::<i32>().unwrap(),
        ["psi", "return"] => 0,
        _ => panic!("unexpected interpreter outcome: {termination}"),
    };
    assert_eq!(actual, expected, "{termination}");
    let built = fixture::build_and_run(project, &[entry]);
    assert_eq!(built.status.code(), Some(expected), "{built:?}");
    assert_eq!(
        std::fs::read(directory.join("stdout")).unwrap(),
        built.stdout
    );
    assert_eq!(
        std::fs::read(directory.join("stderr")).unwrap(),
        built.stderr
    );
}
