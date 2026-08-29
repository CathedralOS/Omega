use crate::{
    FilesystemGrantRootIdentity, FilesystemInputOutputDirectoryReplayRecord,
    FilesystemOutputDirectoryReplayRecord, FilesystemReplay, FilesystemReplayReadKind,
    FilesystemReplayReadRecord, FilesystemSourceInputReplayEventRecord,
    FilesystemSourceInputReplayRecord, FilesystemSourceReadChainReplayRecord,
    MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORIES, MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_PATH_BYTES,
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
fn typed_record_retains_one_exact_directory_attempt() {
    assert_eq!(MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORIES, 1);
    let directory = FilesystemOutputDirectoryReplayRecord::new(root(2), b"generated".to_vec())
        .expect("direct-child directory");
    let record = FilesystemInputOutputDirectoryReplayRecord::new(source_input(), directory)
        .expect("distinct Source and Output roots");
    let replay = FilesystemReplay::from_input_output_directory_record(record)
        .expect("typed directory replay");
    assert_eq!(
        replay
            .attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 11]
    );
    assert_eq!(
        replay
            .output_directory()
            .expect("directory record")
            .output_relative_path(),
        b"generated"
    );
}

#[test]
fn typed_record_rejects_non_direct_or_overlong_paths() {
    assert!(
        FilesystemOutputDirectoryReplayRecord::new(root(2), b"nested/generated".to_vec()).is_err()
    );
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
    assert!(FilesystemInputOutputDirectoryReplayRecord::new(source_input(), directory).is_err());
}
