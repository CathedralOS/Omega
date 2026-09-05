use super::{
    FilesystemSourceDirectoryReadChainReplayRecord, FilesystemSourceDirectoryReadReplayRecord,
    source_directory_chain_is_exact,
};
use crate::{
    EvaluationObservations, FilesystemGrantRootIdentity, FilesystemObservationProvider,
    FilesystemObservedByteRegionKind, FilesystemOperationAttempt, FilesystemOperationResult,
    FilesystemReplay, FilesystemScalarOperandValue, FilesystemSourceInputReplayEventRecord,
    FilesystemSourceInputReplayRecord,
};

fn root(value: u32) -> FilesystemGrantRootIdentity {
    FilesystemGrantRootIdentity::new(value).expect("test root is nonzero")
}

fn populated_read() -> FilesystemSourceDirectoryReadReplayRecord {
    FilesystemSourceDirectoryReadReplayRecord::new(
        6,
        3,
        0,
        vec![1, 2, 3, 4, 5, 6],
        vec![10, 11, 12, 13, 14, 15],
        vec![b'a', b'b', b'c', 13, 14, 15],
        0,
        0,
        3,
    )
    .expect("directory record prefix and cursor are exact")
}

fn end_read() -> FilesystemSourceDirectoryReadReplayRecord {
    FilesystemSourceDirectoryReadReplayRecord::new(
        6,
        0,
        0,
        vec![20, 21, 22, 23, 24, 25],
        vec![30, 31, 32, 33, 34, 35],
        vec![30, 31, 32, 33, 34, 35],
        3,
        3,
        3,
    )
    .expect("end-of-directory leaves both carriers unchanged")
}

fn chain() -> FilesystemSourceDirectoryReadChainReplayRecord {
    FilesystemSourceDirectoryReadChainReplayRecord::new(
        root(1),
        b"entries".to_vec(),
        7,
        0,
        vec![populated_read(), end_read()],
        0,
    )
    .expect("closed directory chain is canonical")
}

#[test]
fn typed_directory_chain_reconstructs_exact_open_reads_and_close() {
    let source = FilesystemSourceInputReplayRecord::new(vec![
        FilesystemSourceInputReplayEventRecord::DirectoryReadChain(chain()),
    ])
    .expect("directory chain is one Source event");
    let replay = FilesystemReplay::from_source_input_record(source)
        .expect("typed directory chain fits replay custody");
    assert_eq!(
        replay
            .attempts()
            .iter()
            .map(FilesystemOperationAttempt::operation_tag)
            .collect::<Vec<_>>(),
        vec![2, 23, 23, 8]
    );
    assert!(source_directory_chain_is_exact(replay.attempts()));

    let populated = &replay.attempts()[1];
    assert_eq!(
        populated.provider(),
        FilesystemObservationProvider::RealScoped
    );
    assert_eq!(
        populated.scalar_operands()[0].value(),
        FilesystemScalarOperandValue::U64(6)
    );
    assert_eq!(
        populated.result(),
        Some(FilesystemOperationResult::Scalar(3))
    );
    assert_eq!(
        populated.observed_byte_regions()[0].kind(),
        FilesystemObservedByteRegionKind::DirectoryRecords
    );
    assert_eq!(populated.observed_byte_regions()[0].length(), 3);
    assert_eq!(
        &populated.mutable_byte_operands()[0].post_bytes()[..3],
        b"abc"
    );
    assert_eq!(populated.mutable_i64_operands()[0].pre_value(), 0);
    assert_eq!(populated.mutable_i64_operands()[0].post_value(), 3);

    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        replay.attempts().to_vec(),
        Vec::new(),
    );
    FilesystemReplay::from_source_input_observations(&observations)
        .expect("observed directory chain passes the same exact grammar");
}

#[test]
fn directory_records_reject_inconsistent_carriers() {
    assert!(
        FilesystemSourceDirectoryReadReplayRecord::new(
            2,
            3,
            0,
            vec![0; 4],
            vec![0; 4],
            vec![1, 2, 3, 0],
            0,
            0,
            1,
        )
        .is_err()
    );
    assert!(
        FilesystemSourceDirectoryReadReplayRecord::new(
            4,
            0,
            0,
            vec![0; 3],
            vec![0; 4],
            vec![0; 4],
            0,
            0,
            0,
        )
        .is_err()
    );
    assert!(
        FilesystemSourceDirectoryReadChainReplayRecord::new(
            root(1),
            b"../escape".to_vec(),
            7,
            0,
            vec![populated_read()],
            0,
        )
        .is_err()
    );
}

#[test]
fn observed_directory_chain_rejects_tampered_exact_lanes() {
    let source = FilesystemSourceInputReplayRecord::new(vec![
        FilesystemSourceInputReplayEventRecord::DirectoryReadChain(chain()),
    ])
    .unwrap();
    let canonical = FilesystemReplay::from_source_input_record(source)
        .unwrap()
        .attempts()
        .to_vec();
    let mut cases = Vec::new();
    let mut attempts = canonical.clone();
    attempts[1].operation_tag = 4;
    cases.push(attempts);
    let mut attempts = canonical.clone();
    attempts[1].observed_byte_regions[0].kind =
        FilesystemObservedByteRegionKind::SequentialFileRead;
    cases.push(attempts);
    let mut attempts = canonical.clone();
    attempts[1].mutable_i64_operands[0].operand_ordinal = 2;
    cases.push(attempts);
    let mut attempts = canonical.clone();
    attempts[1].mutable_byte_operands[0].post_bytes[5] ^= 1;
    cases.push(attempts);
    let mut attempts = canonical;
    attempts.pop();
    cases.push(attempts);

    for attempts in cases {
        let observations =
            EvaluationObservations::from_filesystem_operation_attempts(attempts, Vec::new());
        assert!(FilesystemReplay::from_source_input_observations(&observations).is_err());
    }
}
