use crate::{
    BuildIncludedSource, FilesystemGrantRootIdentity, FilesystemInputOutputTreeReplayRecord,
    FilesystemOutputDirectoryReplayRecord, FilesystemOutputFileReplayRecord,
    FilesystemOutputSymlinkReplayRecord, FilesystemOutputTreeEntryReplayRecord,
    FilesystemOutputWriteReplayRecord, FilesystemReplay, FilesystemReplayReadKind,
    FilesystemReplayReadRecord, FilesystemSourceInputReplayEventRecord,
    FilesystemSourceInputReplayRecord, FilesystemSourceReadChainReplayRecord,
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

fn directory(path: &[u8]) -> FilesystemOutputTreeEntryReplayRecord {
    FilesystemOutputTreeEntryReplayRecord::Directory(
        FilesystemOutputDirectoryReplayRecord::new(root(2), path.to_vec()).expect("directory path"),
    )
}

fn file(path: &[u8], identity: u64, bytes: &[u8]) -> FilesystemOutputTreeEntryReplayRecord {
    let write = FilesystemOutputWriteReplayRecord::new(
        bytes.to_vec(),
        i64::try_from(bytes.len()).expect("test bytes fit i64"),
        0,
    )
    .expect("complete write");
    FilesystemOutputTreeEntryReplayRecord::File(
        FilesystemOutputFileReplayRecord::with_writes(
            root(2),
            path.to_vec(),
            identity,
            0,
            vec![write],
            0,
        )
        .expect("output file"),
    )
}

fn symlink(path: &[u8], target: &[u8]) -> FilesystemOutputTreeEntryReplayRecord {
    FilesystemOutputTreeEntryReplayRecord::Symlink(
        FilesystemOutputSymlinkReplayRecord::new(root(2), path.to_vec(), target.to_vec())
            .expect("output symlink"),
    )
}

#[test]
fn typed_tree_retains_mixed_authored_order_and_nested_file_handoff() {
    let entries = vec![
        directory(b"generated"),
        file(b"generated/table.omg", 2, b"data Table {}\n"),
        directory(b"assets"),
        file(b"assets/data.bin", 3, b"data"),
    ];
    let included =
        BuildIncludedSource::from_coordinate(root(2), b"generated/table.omg".to_vec(), 7)
            .expect("generated-source coordinate");
    let record =
        FilesystemInputOutputTreeReplayRecord::new(source_input(), entries, vec![included.clone()])
            .expect("mixed output tree");
    let replay = FilesystemReplay::from_input_output_tree_record(record)
        .expect("mixed output tree fits replay custody");
    assert_eq!(
        replay
            .attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 11, 1, 5, 8, 11, 1, 5, 8]
    );
    assert_eq!(
        replay
            .output_directories()
            .iter()
            .map(FilesystemOutputDirectoryReplayRecord::output_relative_path)
            .collect::<Vec<_>>(),
        vec![b"generated".as_slice(), b"assets"]
    );
    assert_eq!(
        replay
            .output_files()
            .iter()
            .map(FilesystemOutputFileReplayRecord::output_relative_path)
            .collect::<Vec<_>>(),
        vec![b"generated/table.omg".as_slice(), b"assets/data.bin"]
    );
    assert_eq!(replay.expected_included_sources(), &[included]);
}

#[test]
fn typed_tree_rejects_missing_late_or_nondirectory_parents() {
    assert!(
        FilesystemInputOutputTreeReplayRecord::new(
            source_input(),
            vec![file(b"generated/table.omg", 2, b"data")],
            Vec::new(),
        )
        .is_err()
    );
    assert!(
        FilesystemInputOutputTreeReplayRecord::new(
            source_input(),
            vec![
                file(b"generated/table.omg", 2, b"data"),
                directory(b"generated")
            ],
            Vec::new(),
        )
        .is_err()
    );
    assert!(
        FilesystemInputOutputTreeReplayRecord::new(
            source_input(),
            vec![
                file(b"generated", 2, b"data"),
                file(b"generated/table.omg", 3, b"data")
            ],
            Vec::new(),
        )
        .is_err()
    );
}

#[test]
fn typed_tree_rejects_namespace_and_descriptor_collisions() {
    assert!(
        FilesystemInputOutputTreeReplayRecord::new(
            source_input(),
            vec![directory(b"generated"), file(b"generated", 2, b"data")],
            Vec::new(),
        )
        .is_err()
    );
    assert!(
        FilesystemInputOutputTreeReplayRecord::new(
            source_input(),
            vec![file(b"first", 2, b"a"), file(b"second", 2, b"b")],
            Vec::new(),
        )
        .is_err()
    );
}

#[test]
fn typed_tree_retains_exact_symlink_spelling_and_namespace_order() {
    let record = FilesystemInputOutputTreeReplayRecord::new(
        source_input(),
        vec![
            file(b"artifact", 2, b"bytes"),
            directory(b"links"),
            symlink(b"links/artifact", b"../artifact"),
        ],
        Vec::new(),
    )
    .expect("symlink-bearing output tree");
    let replay = FilesystemReplay::from_input_output_tree_record(record)
        .expect("symlink-bearing replay fits custody");
    assert_eq!(
        replay
            .attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 1, 5, 8, 11, 20]
    );
    let symlinks = replay.output_symlinks();
    let [link] = symlinks.as_slice() else {
        panic!("one exact symlink")
    };
    assert_eq!(link.output_relative_path(), b"links/artifact");
    assert_eq!(link.target_spelling(), b"../artifact");
}
