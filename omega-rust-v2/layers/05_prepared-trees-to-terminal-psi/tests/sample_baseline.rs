//! Compatibility observations use pinned Git objects so editing a sample cannot change the result.
#[path = "sample_baseline/cases.rs"]
mod cases;
#[path = "sample_baseline/pipeline.rs"]
mod pipeline;

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};

const REVISION: &str = "36d45157895e1674fcdc01222fdee0faa1b68082";

#[test]
#[ignore = "reads the pinned sample revision from repository history"]
fn fixed_sample_configurations_through_l5() {
    let mut git = Command::new("git")
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("start git");
    let mut requests = git.stdin.take().unwrap();
    let mut replies = BufReader::new(git.stdout.take().unwrap());
    let mut blob = |directory: &str, file: &str| {
        let object = format!("{REVISION}:samples/cli/{directory}/{file}");
        writeln!(requests, "{object}").unwrap();
        let mut header = String::new();
        replies.read_line(&mut header).unwrap();
        let fields: Vec<_> = header.split_whitespace().collect();
        assert!(matches!(&fields[..], [_, "blob", _]), "cannot read {object}: {header}");
        let mut bytes = vec![0; fields[2].parse().unwrap()];
        replies.read_exact(&mut bytes).unwrap();
        let mut separator = [0];
        replies.read_exact(&mut separator).unwrap();
        assert_eq!(separator, [b'\n']);
        bytes
    };
    let expected: Vec<_> = include_str!("sample_baseline/observations.tsv")
        .lines()
        .filter(|line| !line.starts_with('#'))
        .collect();
    assert_eq!(cases::CASES.len(), expected.len(), "review the fixed cohort size when changing cases");
    let mut changes = Vec::new();
    for (case, previous) in cases::CASES.iter().zip(expected) {
        let build = blob(case.directory, "build.omg");
        case.confirm_configuration(std::str::from_utf8(&build).unwrap());
        let files = match case.main_override {
            Some(main) => current_import_files(main),
            None => case
                .files
                .iter()
                .map(|&path| (path, blob(case.directory, path)))
                .collect(),
        };
        let outcome = match case.expectation.observe(&files) {
            Ok(outcome) => outcome,
            Err(failure) => {
                changes.push(format!("{}: {failure}", case.id));
                continue;
            }
        };
        let row = format!("{}\t{outcome}", case.id);
        println!("{row}");
        if row != previous {
            changes.push(format!("previous: {previous}\n current: {row}"));
        }
    }
    drop(requests);
    assert!(git.wait().unwrap().success());
    assert!(changes.is_empty(), "review changes before updating the baseline:\n{}", changes.join("\n"));
}

fn current_import_files(main: &[u8]) -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("main.omg", main.to_vec()),
        ("m/a.omg", include_bytes!("sample_baseline/fixtures/direct_file_imports/m/a.omg").to_vec()),
        ("m/b.omg", include_bytes!("sample_baseline/fixtures/direct_file_imports/m/b.omg").to_vec()),
    ]
}

#[test]
fn directory_import_and_its_missing_import_twin() {
    let main = include_bytes!("sample_baseline/fixtures/direct_file_imports/main.omg");
    let twin = include_bytes!("sample_baseline/fixtures/direct_file_imports/missing_import.omg");
    let main = std::str::from_utf8(main).unwrap().replace("\r\n", "\n");
    let twin = std::str::from_utf8(twin).unwrap().replace("\r\n", "\n");
    assert_eq!(main.replace("use m\\A;\n", ""), twin);
    let product = pipeline::lower(&current_import_files(main.as_bytes())).expect("the valid import must lower");
    assert!(
        cases::Check::NestedField.matches(&product),
        "expected the return value read from B.value then A.value"
    );
    let refusal = pipeline::lower(&current_import_files(twin.as_bytes())).unwrap_err();
    assert_eq!(refusal, cases::MISSING_IMPORT);
}
