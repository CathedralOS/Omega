use std::fs;

use optimization_policy_offline::decode_offline_policy_corpus;

use super::fixture::{FixtureDirectory, arguments, encoded_log};
use crate::run;

#[test]
fn command_captures_canonical_logs_into_a_new_validated_artifact() {
    let directory = FixtureDirectory::new();
    let first = directory.path("first.log");
    let second = directory.path("second.log");
    let output = directory.path("captured.corpus");
    fs::write(&first, encoded_log(b"first-command-source")).unwrap();
    fs::write(&second, encoded_log(b"second-command-source")).unwrap();

    run(arguments(&output, &[&second, &first])).unwrap();

    let encoded = fs::read(&output).unwrap();
    let corpus = decode_offline_policy_corpus(&encoded).unwrap();
    assert_eq!(corpus.receipt().log_count(), 2);
    assert_eq!(corpus.receipt().decision_count(), 2);
    assert_eq!(corpus.encode(), encoded);

    assert!(run(arguments(&output, &[&first])).is_err());
    assert_eq!(fs::read(output).unwrap(), encoded);
}
