use crate::{
    FilesystemGrantRootIdentity, FilesystemInputOutputDirectoryReplayRecord,
    FilesystemOutputDirectoryReplayRecord, FilesystemReplay, FilesystemReplayReadKind,
    FilesystemReplayReadRecord, FilesystemSourceInputReplayEventRecord,
    FilesystemSourceInputReplayRecord, FilesystemSourceReadChainReplayRecord,
    MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORIES, MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_PATH_BYTES,
    MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_RETAINED_PATH_BYTES,
};

fn root(value: u32) -> FilesystemGrantRootIdentity {
    FilesystemGrantRootIdentity::new(value).expect("test root is nonzero")
}

fn source_input() -> FilesystemSourceInputReplayRecord {
    let read = FilesystemReplayReadRecord::new(
        FilesystemReplayReadKind::Sequential,
        0,
        0,
        0,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("empty read is exact");
    let chain = FilesystemSourceReadChainReplayRecord::new(
        root(1),
        b"main.omg".to_vec(),
        1,
        0,
        vec![read],
        0,
    )
    .expect("source chain");
    FilesystemSourceInputReplayRecord::new(vec![FilesystemSourceInputReplayEventRecord::ReadChain(
        chain,
    )])
    .expect("source input")
}

#[test]
fn typed_record_retains_an_ordered_exact_directory_tree() {
    assert_eq!(MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORIES, 4_096);
    assert_eq!(
        MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_RETAINED_PATH_BYTES,
        16 * 1024 * 1024
    );
    let directories = [b"generated".as_slice(), b"generated/nested", b"sibling"]
        .into_iter()
        .map(|path| FilesystemOutputDirectoryReplayRecord::new(root(2), path.to_vec()))
        .collect::<Result<Vec<_>, _>>()
        .expect("canonical directory paths");
    let record = FilesystemInputOutputDirectoryReplayRecord::new(source_input(), directories)
        .expect("distinct Source and Output roots");
    let replay = FilesystemReplay::from_input_output_directory_record(record)
        .expect("typed directory replay");
    assert_eq!(
        replay
            .attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 11, 11, 11]
    );
    assert_eq!(
        replay
            .output_directories()
            .iter()
            .map(FilesystemOutputDirectoryReplayRecord::output_relative_path)
            .collect::<Vec<_>>(),
        vec![b"generated".as_slice(), b"generated/nested", b"sibling"]
    );
}

#[test]
fn typed_record_rejects_missing_or_late_parents_and_duplicate_paths() {
    let nested = FilesystemOutputDirectoryReplayRecord::new(root(2), b"generated/nested".to_vec())
        .expect("nested path is individually canonical");
    assert!(FilesystemInputOutputDirectoryReplayRecord::new(source_input(), vec![nested]).is_err());

    let parent = FilesystemOutputDirectoryReplayRecord::new(root(2), b"generated".to_vec())
        .expect("parent path");
    let child = FilesystemOutputDirectoryReplayRecord::new(root(2), b"generated/nested".to_vec())
        .expect("child path");
    assert!(
        FilesystemInputOutputDirectoryReplayRecord::new(
            source_input(),
            vec![child, parent.clone()]
        )
        .is_err()
    );
    assert!(
        FilesystemInputOutputDirectoryReplayRecord::new(
            source_input(),
            vec![parent.clone(), parent]
        )
        .is_err()
    );
}

#[test]
fn typed_record_rejects_overlong_paths() {
    assert!(
        FilesystemOutputDirectoryReplayRecord::new(
            root(2),
            vec![b'a'; MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_PATH_BYTES + 1],
        )
        .is_err()
    );
}

#[test]
fn typed_record_rejects_source_output_root_alias() {
    let directory = FilesystemOutputDirectoryReplayRecord::new(root(1), b"generated".to_vec())
        .expect("directory path");
    assert!(
        FilesystemInputOutputDirectoryReplayRecord::new(source_input(), vec![directory]).is_err()
    );
}

#[test]
fn typed_record_rejects_empty_or_mixed_root_directory_trees() {
    assert!(FilesystemInputOutputDirectoryReplayRecord::new(source_input(), Vec::new()).is_err());
    let first = FilesystemOutputDirectoryReplayRecord::new(root(2), b"first".to_vec())
        .expect("first directory");
    let second = FilesystemOutputDirectoryReplayRecord::new(root(3), b"second".to_vec())
        .expect("second directory");
    assert!(
        FilesystemInputOutputDirectoryReplayRecord::new(source_input(), vec![first, second])
            .is_err()
    );
}
