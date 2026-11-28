use super::program;

const SOURCE: &str = include_str!("../fixtures/callables/run/some_reader/main.omg");

fn assert_refused_at_change(before: &str, after: &str) {
    assert_eq!(SOURCE.matches(before).count(), 1);
    let line = SOURCE
        .lines()
        .position(|text| text.contains(before))
        .unwrap()
        + 1;
    let source = SOURCE.replace(before, after);
    let (output, report) = program::invoke(&source, &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(report.contains(&format!("main.omg:{line}:")), "{report}");
}

#[test]
fn different_reader_types_share_one_array_and_call_their_own_machines() {
    // Only the Socket callback turns its stored 41 into 42. Even if the target
    // lays out both byte fields alike, calling the wrong machine traps in main.
    program::assert_silent_return(SOURCE);
}

#[test]
fn scalar_and_record_readers_dispatch_from_adjacent_array_elements() {
    program::assert_silent_return(include_str!(
        "../fixtures/callables/run/some_reader_scalar_record/main.omg"
    ));
}

#[test]
fn a_reader_rejects_a_callback_for_a_different_state() {
    assert_refused_at_change("read: read_file", "read: read_socket");
}

#[test]
fn one_open_reader_cannot_supply_another_readers_state() {
    assert_refused_at_change("(read)(left.state)", "(read)(right.state)");
}
