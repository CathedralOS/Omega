use crate::{
    BuildIncludedSource, EvaluationObservations, FilesystemGrantRootIdentity,
    FilesystemInputOutputTreeReplayRecord, FilesystemOutputDirectoryReplayRecord,
    FilesystemOutputFileReplayRecord, FilesystemOutputHardLinkReplayKind,
    FilesystemOutputHardLinkReplayRecord, FilesystemOutputSymlinkReplayRecord,
    FilesystemOutputTreeEntryReplayRecord, FilesystemOutputWriteReplayRecord, FilesystemReplay,
    FilesystemReplayReadKind, FilesystemReplayReadRecord, FilesystemSourceInputReplayEventRecord,
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

fn hard_link(
    kind: FilesystemOutputHardLinkReplayKind,
    existing: &[u8],
    path: &[u8],
) -> FilesystemOutputTreeEntryReplayRecord {
    FilesystemOutputTreeEntryReplayRecord::HardLink(
        FilesystemOutputHardLinkReplayRecord::new(kind, root(2), existing.to_vec(), path.to_vec())
            .expect("output hard link"),
    )
}

#[test]
fn output_only_tree_round_trips_exact_mixed_entries_and_handoff() {
    let entries = vec![
        directory(b"generated"),
        file(b"generated/artifact", 1, b"bytes"),
        symlink(b"generated/symbolic", b"artifact"),
        hard_link(
            FilesystemOutputHardLinkReplayKind::Portable,
            b"generated/artifact",
            b"generated/alias",
        ),
    ];
    let included = BuildIncludedSource::from_coordinate(root(2), b"generated/artifact".to_vec(), 4)
        .expect("handoff follows the Output close with no Source prefix");
    let record =
        FilesystemInputOutputTreeReplayRecord::output_only(entries, vec![included.clone()])
            .expect("mixed Output-only tree");
    assert!(record.source_input().is_none());

    let replay = FilesystemReplay::from_input_output_tree_record(record)
        .expect("Output-only tree fits replay custody");
    assert_eq!(
        replay
            .attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![11, 1, 5, 8, 20, 19]
    );
    assert!(
        (0..replay.attempts().len()).all(|index| replay.executes_output_attempt(index)),
        "every Output-only attempt executes against the replay namespace"
    );
    assert_eq!(replay.expected_included_sources(), &[included.clone()]);

    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        replay.attempts().to_vec(),
        vec![included.clone()],
    );
    let decoded = FilesystemReplay::from_input_output_observations(&observations)
        .expect("observed Output-only tree retains the exact grammar");
    assert_eq!(decoded.output_entries().len(), 4);
    assert_eq!(decoded.output_directories().len(), 1);
    assert_eq!(decoded.output_files().len(), 1);
    assert_eq!(decoded.output_symlinks().len(), 1);
    assert_eq!(decoded.output_hard_links().len(), 1);
    assert_eq!(decoded.expected_included_sources(), &[included]);
}

#[test]
fn output_only_tree_rejects_empty_output_and_early_handoff() {
    assert!(FilesystemInputOutputTreeReplayRecord::output_only(Vec::new(), Vec::new()).is_err());
    assert!(FilesystemSourceInputReplayRecord::new(Vec::new()).is_err());

    let early = BuildIncludedSource::from_coordinate(root(2), b"artifact".to_vec(), 2)
        .expect("coordinate shape is valid before tree ordering is checked");
    assert!(
        FilesystemInputOutputTreeReplayRecord::output_only(
            vec![file(b"artifact", 1, b"bytes")],
            vec![early],
        )
        .is_err()
    );

    let empty = EvaluationObservations::from_filesystem_operation_attempts(Vec::new(), Vec::new());
    assert!(FilesystemReplay::from_input_output_observations(&empty).is_err());
}

#[test]
fn output_only_observations_reject_malformed_source_like_prefixes() {
    let output = FilesystemReplay::from_input_output_tree_record(
        FilesystemInputOutputTreeReplayRecord::output_only(
            vec![file(b"artifact", 2, b"bytes")],
            Vec::new(),
        )
        .expect("Output-only file record"),
    )
    .expect("Output-only file replay");
    let source =
        FilesystemReplay::from_source_input_record(source_input()).expect("complete Source record");

    for malformed_prefix in [
        vec![source.attempts()[0].clone()],
        vec![source.attempts()[1].clone()],
    ] {
        let mut attempts = malformed_prefix;
        attempts.extend_from_slice(output.attempts());
        let observations =
            EvaluationObservations::from_filesystem_operation_attempts(attempts, Vec::new());
        assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());
    }
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

#[test]
fn typed_tree_retains_portable_and_windows_hard_link_spelling() {
    let record = FilesystemInputOutputTreeReplayRecord::new(
        source_input(),
        vec![
            file(b"artifact", 2, b"bytes"),
            hard_link(
                FilesystemOutputHardLinkReplayKind::Portable,
                b"artifact",
                b"portable-alias",
            ),
            hard_link(
                FilesystemOutputHardLinkReplayKind::Windows,
                b"portable-alias",
                b"windows-alias",
            ),
        ],
        Vec::new(),
    )
    .expect("hard-link-bearing output tree");
    let replay = FilesystemReplay::from_input_output_tree_record(record)
        .expect("hard-link-bearing replay fits custody");
    assert_eq!(
        replay
            .attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 1, 5, 8, 19, 27]
    );
    let hard_links = replay.output_hard_links();
    assert_eq!(hard_links.len(), 2);
    assert_eq!(hard_links[0].existing_relative_path(), b"artifact");
    assert_eq!(hard_links[0].output_relative_path(), b"portable-alias");
    assert_eq!(hard_links[0].result(), 0);
    assert_eq!(hard_links[1].existing_relative_path(), b"portable-alias");
    assert_eq!(hard_links[1].output_relative_path(), b"windows-alias");
    assert_eq!(hard_links[1].result(), 1);
}

#[test]
fn typed_tree_rejects_hard_links_without_a_prior_regular_name() {
    for entries in [
        vec![hard_link(
            FilesystemOutputHardLinkReplayKind::Portable,
            b"missing",
            b"alias",
        )],
        vec![
            hard_link(
                FilesystemOutputHardLinkReplayKind::Portable,
                b"future",
                b"alias",
            ),
            file(b"future", 2, b"bytes"),
        ],
        vec![
            directory(b"directory"),
            hard_link(
                FilesystemOutputHardLinkReplayKind::Portable,
                b"directory",
                b"alias",
            ),
        ],
        vec![
            symlink(b"symbolic", b"target"),
            hard_link(
                FilesystemOutputHardLinkReplayKind::Portable,
                b"symbolic",
                b"alias",
            ),
        ],
    ] {
        assert!(
            FilesystemInputOutputTreeReplayRecord::new(source_input(), entries, Vec::new())
                .is_err()
        );
    }
}

#[test]
fn typed_tree_rejects_hard_link_destination_collisions_and_missing_parents() {
    assert!(
        FilesystemInputOutputTreeReplayRecord::new(
            source_input(),
            vec![
                file(b"artifact", 2, b"bytes"),
                directory(b"occupied"),
                hard_link(
                    FilesystemOutputHardLinkReplayKind::Portable,
                    b"artifact",
                    b"occupied",
                ),
            ],
            Vec::new(),
        )
        .is_err()
    );
    assert!(
        FilesystemInputOutputTreeReplayRecord::new(
            source_input(),
            vec![
                file(b"artifact", 2, b"bytes"),
                hard_link(
                    FilesystemOutputHardLinkReplayKind::Portable,
                    b"artifact",
                    b"links/artifact",
                ),
            ],
            Vec::new(),
        )
        .is_err()
    );
}
