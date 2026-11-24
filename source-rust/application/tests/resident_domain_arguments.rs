//! Publish a core Resident reference with the policy type supplied by its caller.
#[path = "support/program.rs"]
mod program;

#[test]
fn a_resident_forward_preserves_its_exact_domain_argument() {
    let source = r#"
use omega\language\core\Extent;
data Word { value: u64; }
domain Word::Placed;
domain Word::Other;
machine carry(input: @Extent in Granted & Resident<Word in Word::Placed, Word>)
    -> @Extent in Granted & Resident<Word in Word::Placed, Word> { input }
machine main() {}
"#;
    let (output, report) = program::invoke(source, &["--check"]);
    assert!(output.status.success(), "{report}");

    let changed = source.replace(
        "-> @Extent in Granted & Resident<Word in Word::Placed",
        "-> @Extent in Granted & Resident<Word in Word::Other",
    );
    let (output, report) = program::invoke(&changed, &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(report.contains("changes its referent type"), "{report}");
}
