//! A constant may show its type through its initializer instead of an
//! annotation: `const LIMIT = 7u8;` means `const LIMIT: u8 = 7u8;`, and an
//! initializer whose type only typing can find (`const second = flag();`,
//! `const first = second;`, `const mask = 2u8 + 3;`) takes that inferred type.
//!
//! Later uses never choose the declaration's type. Both positive programs run
//! natively and in Psi and exit 0 only when every value reads back as written.
//! Each negative twin changes one line and must be refused: a mismatched suffix
//! or inferred type at a use site, a cyclic unannotated constant, conflicting
//! typed operands, or a wholly anonymous initializer.
//!
//! `wiki/spec/language/numeric_values.md#literal-landing-and-destinations`.
//! The parser's own test is
//! `pipeline/01_tokens-to-syntax-trees/tests/unit/unannotated_constants.rs`.

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
                let start = line[..at].rfind([' ', '\t']).map_or(0, |space| space + 1);
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

const TYPING_INFERRED_PROGRAM: &str = "\
domain u32::Metres requires self > 0;
data Pair [copy] { left: u8; right: u8; }

machine flag() -> bool crashes none { true }

const first = second;
const second = flag();
const mask = 2u8 + 3;
const flipped = 3 + 2u8;
const span = 5u32 as +Metres;
const pair = Pair { left: 1, right: 2 };
const bytes = [1u8, 2, 3];

machine main() crashes Trap {
    let ok: bool = first;
    let total: u8 = mask + flipped + pair.left + pair.right + bytes[0u64] + bytes[1u64] + bytes[2u64];
    let distance: u32 in Metres = span;
    let raw: u32 = distance as -Metres;
    transition ok && total == 19u8 && raw == 5u32 {
        true -> done(),
        false -> failed()
    }
    machine done() {}
    machine failed() crashes Trap { crash Trap; }
}
";

#[test]
fn unannotated_constants_inferred_by_typing_agree_natively_and_in_psi() {
    program::assert_silent_return(TYPING_INFERRED_PROGRAM);
}

#[test]
fn later_use_cannot_change_an_inferred_constant_type() {
    let twin = TYPING_INFERRED_PROGRAM.replace("let ok: bool = first;", "let ok: u8 = first;");
    assert_ne!(twin, TYPING_INFERRED_PROGRAM);
    let (output, report) = invoke(&twin, &["--check"]);
    assert!(!output.status.success(), "{report}");
}

#[test]
fn cyclic_unannotated_constants_are_refused() {
    let twin = TYPING_INFERRED_PROGRAM.replace("const second = flag();", "const second = first;");
    assert_ne!(twin, TYPING_INFERRED_PROGRAM);
    let (output, report) = invoke(&twin, &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(report.contains("cyclic initializer dependency"), "{report}");
}

#[test]
fn conflicting_typed_operands_in_unannotated_constant_are_refused() {
    let twin = TYPING_INFERRED_PROGRAM.replace("const mask = 2u8 + 3;", "const mask = 2u8 + 3u16;");
    assert_ne!(twin, TYPING_INFERRED_PROGRAM);
    let (output, report) = invoke(&twin, &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(report.contains("no determined type"), "{report}");
}

#[test]
fn wholly_anonymous_unannotated_constant_is_still_refused() {
    let twin = TYPING_INFERRED_PROGRAM.replace("const mask = 2u8 + 3;", "const mask = 2 + 3;");
    assert_ne!(twin, TYPING_INFERRED_PROGRAM);
    let (output, report) = invoke(&twin, &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(report.contains("wholly anonymous"), "{report}");
}
