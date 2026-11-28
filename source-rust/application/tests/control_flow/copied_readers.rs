use super::program;

const READERS: &str = include_str!("../fixtures/callables/run/copied_reader_fields/main.omg");

#[test]
fn copied_fields_dispatch_with_their_own_readers_state() {
    program::assert_silent_return(READERS);
}

#[test]
fn a_callback_from_an_unrelated_reader_cannot_be_repacked() {
    let own = "let copy: Reader<'a> = reader;";
    assert_eq!(READERS.matches(own).count(), 1);
    let crossed = READERS.replace(own, "let copy: Reader<'a> = other;");
    let (output, report) = program::invoke(&crossed, &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(report.contains("conflicting generic arguments"), "{report}");
}
