//! A constant may show its type through its initializer instead of an
//! annotation: `const LIMIT = 7u8;` means `const LIMIT: u8 = 7u8;`.
//!
//! The program declares four such constants (a suffixed integer, a negative
//! suffixed integer, a Boolean and a cast) and passes each where only its own
//! type fits. It exits 0 only when every value reads back as written. One twin
//! writes `7u16`, so `LIMIT` is a `u16` passed where a `u8` is required, and it
//! must be refused exactly as the annotated `const LIMIT: u16 = 7u16;` is. The
//! other writes `7`, an initializer that names no type, which the spec refuses
//! at the initializer.
//!
//! `wiki/spec/language/numeric_values.md#literal-landing-and-destinations`:
//! "`let count = 5u8;` keeps the suffix's carrier", and "An unannotated binding
//! whose initializer is wholly anonymous, such as `let count = 5;` or
//! `const limit = 2 + 3;`, is refused at the initializer". The parser's own
//! test is `pipeline/01_tokens-to-syntax-trees/tests/unit/unannotated_constants.rs`.

#[path = "support/program.rs"]
mod program;
mod support;

use program::invoke;
use std::path::PathBuf;

fn fixture(tier: &str, case: &str) -> String {
    let path: PathBuf = support::repository_root()
        .join("source-rust/application/tests/fixtures/constants")
        .join(tier)
        .join(case)
        .join("main.omg");
    std::fs::read_to_string(path).unwrap()
}


/// The report from the first `error:` line on, with the temporary directory
/// removed from any path and the byte offsets in generated names (`#261.0`)
/// removed, so two runs whose sources differ before the refusal compare.
fn refusal(report: &str) -> String {
    let from_error = &report[report.find("error:").expect("a refusal")..];
    let mut without_offsets = String::with_capacity(from_error.len());
    let mut characters = from_error.chars().peekable();
    while let Some(character) = characters.next() {
        without_offsets.push(character);
        if character == '#' {
            while characters.next_if(char::is_ascii_digit).is_some() {}
        }
    }
    without_offsets
        .lines()
        .map(|line| match line.find("main.omg") {
            Some(at) => {
                let start = line[..at]
                    .rfind([' ', '\t'])
                    .map_or(0, |space| space + 1);
                format!("{}{}", &line[..start], &line[at..])
            }
            None => line.to_owned(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn constants_that_show_their_type_through_their_initializer_run_natively_and_in_psi() {
    program::assert_silent_return(&fixture("run", "unannotated_constants"));
}

#[test]
fn a_u16_suffix_makes_the_constant_a_u16_like_an_annotation_does() {
    // The twin differs only in the suffix of `LIMIT`.
    let positive = fixture("run", "unannotated_constants");
    let twin = fixture("fail", "unannotated_constant_carrier");
    assert_eq!(positive.replace("7u8;", "7u16;"), twin);

    let (output, unannotated) = invoke(&twin, &["--check"]);
    assert!(!output.status.success(), "{unannotated}");
    let annotated_source = twin.replace("const LIMIT = 7u16;", "const LIMIT: u16 = 7u16;");
    let (output, annotated) = invoke(&annotated_source, &["--check"]);
    assert!(!output.status.success(), "{annotated}");
    assert_eq!(refusal(&unannotated), refusal(&annotated));
}

#[test]
fn a_constant_whose_initializer_names_no_type_is_refused_at_the_initializer() {
    // The twin differs only in dropping the suffix of `LIMIT`.
    let positive = fixture("run", "unannotated_constants");
    let twin = fixture("fail", "anonymous_constant");
    assert_eq!(positive.replace("7u8;", "7;"), twin);

    let (output, report) = invoke(&twin, &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("main.omg:1:15: this constant has no type annotation and its initializer is wholly anonymous"),
        "{report}"
    );
    assert_eq!(&twin.lines().next().unwrap()[14..], "7;");
}
