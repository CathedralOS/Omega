//! A type alias may assert its expansion's multiplicity: `data Count [copy] =
//! u64;` and `data Held [linear] = Lease;` where `Lease` is linear.
//!
//! The program copies a `Count` (it reads `n` after binding `m` to it) and
//! exits 0 only when the sum is right. Its twin differs in one word: it
//! asserts that `Count` is `linear`, which `u64` is not, and the compiler
//! refuses the alias at its name. The assertion changes nothing about the type:
//! an alias and its expansion are the same type with the same multiplicity.
//!
//! `wiki/spec/language/generics.md#transparent-type-aliases`: "An alias may
//! assert its target's multiplicity under the ordinary property rules. A carry
//! list is invalid on both the alias and its target." The parser records the
//! assertion (`pipeline/01_tokens-to-syntax-trees/tests/unit/alias_properties.rs`)
//! and typing compares it with the expansion's multiplicity.

mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::{Command, Output, Stdio};

fn fixture(tier: &str) -> String {
    let path = support::repository_root()
        .join("source-rust/application/tests/fixtures/aliases")
        .join(tier)
        .join("asserted_multiplicity/main.omg");
    std::fs::read_to_string(path).unwrap()
}

/// Run `omega` on a copy of one source file, so nothing the compiler writes
/// lands in the checkout.
fn invoke(source: &str, arguments: &[&str]) -> (Output, String) {
    let scratch = TemporaryDirectory::create("alias-multiplicity", false).unwrap();
    std::fs::write(scratch.path().join("main.omg"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(arguments)
        .arg("main.omg")
        .current_dir(scratch.path())
        .stdin(Stdio::null())
        .output()
        .expect("the compiler runs on the fixture");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    eprintln!("{report}");
    (output, report)
}

#[test]
fn aliases_that_assert_their_expansion_multiplicity_run_natively_and_in_psi() {
    let (output, report) = invoke(&fixture("run"), &["run", "--both"]);
    assert_eq!(output.status.code(), Some(0), "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn an_alias_asserting_linear_over_a_copy_type_is_refused_at_its_name() {
    // The twins differ only in the assertion on `Count`.
    assert_eq!(
        fixture("run").replace("data Count [copy]", "data Count [linear]"),
        fixture("fail")
    );
    let (output, report) = invoke(&fixture("fail"), &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains(
            "type alias `Count` asserts that its expansion is `linear`, but the expansion is `copy`"
        ),
        "{report}"
    );
    assert!(report.contains("main.omg:3:6"), "{report}");
    assert_eq!(&fixture("fail").lines().nth(2).unwrap()[5..10], "Count");
}

#[test]
fn an_alias_asserting_copy_over_a_linear_or_ordinary_record_is_refused() {
    for (record, found) in [
        ("data Lease [linear]", "`linear`"),
        ("data Lease", "affine"),
    ] {
        let source = fixture("run")
            .replace("data Lease [linear]", record)
            .replace("data Held [linear]", "data Held [copy]");
        let (output, report) = invoke(&source, &["--check"]);
        assert!(!output.status.success(), "{report}");
        assert!(
            report.contains(&format!(
                "type alias `Held` asserts that its expansion is `copy`, but the expansion is {found}"
            )),
            "{report}"
        );
    }
}
