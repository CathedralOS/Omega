//! One-field mutation matrix over the retained staged-output tree.

mod file_selection;

use super::{
    BuildStagedOutputTree, BuildStagedOutputTreeCommitment, RetainedStagedOutputEntry,
    RetainedStagedOutputEntryKind, commitment_for_retained_entries, empty, select_included_sources,
};
use crate::staged_output_tree::{MAX_STAGED_OUTPUT_ENTRIES, MAX_STAGED_OUTPUT_UNIQUE_FILE_BYTES};
use crate::{OutputTreeEntry, from_entries};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_SESSION: AtomicU64 = AtomicU64::new(0);

/// One temporary session root; every call to `destination` returns a fresh
/// empty directory a materialization may fill or must leave untouched.
struct Session(PathBuf);

impl Session {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "omega-staged-output-substitution-{}-{}",
            std::process::id(),
            NEXT_SESSION.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir(&path).expect("create substitution session");
        Self(path)
    }

    fn destination(&self, label: &str) -> PathBuf {
        // Labels name cases (`commitment::foreign-tree`); a colon is not a
        // valid Windows path character, so the directory takes a dash.
        let destination = self.0.join(label.replace(':', "-"));
        std::fs::create_dir(&destination).expect("create empty destination");
        destination
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Canonical retained roster: two directories, a nested file pair, and two
/// top-level files, one of them a generated `.omg` source for the
/// include-source join below.
fn baseline_tree() -> BuildStagedOutputTree {
    from_entries(&[
        OutputTreeEntry::directory(b"docs"),
        OutputTreeEntry::directory(b"docs/inner"),
        OutputTreeEntry::regular_file(b"docs/inner/a.txt", b"alpha", false),
        OutputTreeEntry::regular_file(b"docs/inner/gen.omg", b"data Generated {}\n", false),
        OutputTreeEntry::regular_file(b"report.txt", b"report-body", false),
        OutputTreeEntry::regular_file(b"z-last.bin", b"z", false),
    ])
    .expect("baseline retained tree")
}

/// A different valid retained tree for foreign-commitment substitutions.
fn foreign_tree() -> BuildStagedOutputTree {
    from_entries(&[OutputTreeEntry::regular_file(b"other.txt", b"other", false)])
        .expect("foreign retained tree")
}

fn file_entry(relative_path: &[u8], bytes: &[u8]) -> RetainedStagedOutputEntry {
    RetainedStagedOutputEntry {
        relative_path: relative_path.to_vec(),
        kind: RetainedStagedOutputEntryKind::File {
            bytes: Arc::from(bytes),
            executable: false,
        },
    }
}

fn tree_with(
    commitment: BuildStagedOutputTreeCommitment,
    entries: Vec<RetainedStagedOutputEntry>,
) -> BuildStagedOutputTree {
    BuildStagedOutputTree {
        commitment,
        entries,
    }
}

/// A stale-identity reconstruction must reject inside `validate_retained_tree`,
/// before the destination is touched.
fn assert_stale_identity_rejects(
    session: &Session,
    label: &str,
    baseline_commitment: BuildStagedOutputTreeCommitment,
    entries: &[RetainedStagedOutputEntry],
) {
    let stale = tree_with(baseline_commitment, entries.to_vec());
    let destination = session.destination(label);
    let error = stale
        .materialize_into(&destination)
        .expect_err("the stale published identity must reject");
    assert!(
        error.message().contains("disagrees with its commitment"),
        "{label}: unexpected rejection: {}",
        error.message()
    );
    assert!(
        std::fs::read_dir(&destination)
            .expect("enumerate rejected destination")
            .next()
            .is_none(),
        "{label}: a rejected record must leave its destination empty"
    );
}

/// The canonical-substitution driver shared by the portable and unix
/// matrices: the substitution must change the record, an honestly
/// recomputed commitment must diverge, the record under the published
/// identity must reject before any write, and the honestly re-committed
/// record must materialize to its own divergent identity.
fn assert_representable_substitution(
    session: &Session,
    name: &str,
    baseline: &BuildStagedOutputTree,
    baseline_commitment: BuildStagedOutputTreeCommitment,
    index: usize,
    mutate: impl Fn(&mut RetainedStagedOutputEntry),
) {
    let mut mutated = baseline.clone();
    mutate(&mut mutated.entries[index]);
    assert_ne!(
        mutated.entries[index], baseline.entries[index],
        "{name}: the substitution must change the retained record"
    );
    let recomputed = commitment_for_retained_entries(&mutated.entries)
        .expect("mutated roster stays within the unique-content ceiling");
    assert_ne!(
        recomputed, baseline_commitment,
        "{name}: an honestly recomputed commitment must diverge"
    );

    assert_stale_identity_rejects(session, name, baseline_commitment, &mutated.entries);

    mutated.commitment = recomputed;
    let honest = session.destination(&format!("{name}-honest"));
    assert_eq!(
        mutated
            .materialize_into(&honest)
            .expect("a canonical substitution materializes under its honest identity"),
        recomputed,
        "{name}"
    );
}

#[test]
fn staged_output_tree_rejects_every_one_field_substitution() {
    let session = Session::new();
    let baseline = baseline_tree();
    let baseline_commitment = baseline.commitment();
    assert_eq!(baseline.entry_count(), 6);
    assert_eq!(baseline.file_bytes(), 35);

    // Control: the baseline retains its own identity.
    let control = session.destination("control");
    assert_eq!(
        baseline
            .materialize_into(&control)
            .expect("baseline materializes"),
        baseline_commitment
    );

    // ── Every representable `RetainedStagedOutputEntry` field, canonical
    //    shape retained (portable kinds). ──
    let representable: Vec<(
        &'static str,
        usize,
        Box<dyn Fn(&mut RetainedStagedOutputEntry)>,
    )> = vec![
        (
            "entries[2].relative_path",
            2,
            Box::new(|entry| entry.relative_path = b"docs/inner/b.txt".to_vec()),
        ),
        (
            "entries[3].relative_path",
            3,
            Box::new(|entry| entry.relative_path = b"docs/inner/gen.txt".to_vec()),
        ),
        (
            "entries[4].relative_path",
            4,
            Box::new(|entry| entry.relative_path = b"report.bin".to_vec()),
        ),
        (
            "entries[2].bytes",
            2,
            Box::new(|entry| {
                entry.kind = RetainedStagedOutputEntryKind::File {
                    bytes: Arc::from(b"alpha-2".as_slice()),
                    executable: false,
                };
            }),
        ),
        (
            "entries[4].bytes::same-length",
            4,
            Box::new(|entry| {
                entry.kind = RetainedStagedOutputEntryKind::File {
                    bytes: Arc::from(b"report-cody".as_slice()),
                    executable: false,
                };
            }),
        ),
        (
            "entries[5].bytes::empty",
            5,
            Box::new(|entry| {
                entry.kind = RetainedStagedOutputEntryKind::File {
                    bytes: Arc::from(b"".as_slice()),
                    executable: false,
                };
            }),
        ),
        (
            // Duplicate content folds into the same unique byte account, so
            // this substitution diverges `file_bytes` below the baseline.
            "entries[5].bytes::duplicate-content",
            5,
            Box::new(|entry| {
                entry.kind = RetainedStagedOutputEntryKind::File {
                    bytes: Arc::from(b"report-body".as_slice()),
                    executable: false,
                };
            }),
        ),
        (
            "entries[2].kind::file-to-directory",
            2,
            Box::new(|entry| entry.kind = RetainedStagedOutputEntryKind::Directory),
        ),
    ];
    for (name, index, mutate) in representable {
        assert_representable_substitution(
            &session,
            name,
            &baseline,
            baseline_commitment,
            index,
            mutate,
        );
    }

    // The same axes on a roster carrying a real symlink and an executable
    // file: the link target spelling and the executable class are
    // representable fields on hosts that materialize them faithfully.
    #[cfg(unix)]
    {
        let unix_baseline = from_entries(&[
            OutputTreeEntry::directory(b"bin"),
            OutputTreeEntry::symbolic_link(b"bin/current", b"tool"),
            OutputTreeEntry::regular_file(b"bin/tool", b"#!/tool\n", true),
            OutputTreeEntry::regular_file(b"plain.txt", b"plain", false),
        ])
        .expect("unix baseline retained tree");
        let unix_commitment = unix_baseline.commitment();
        let unix_representable: Vec<(
            &'static str,
            usize,
            Box<dyn Fn(&mut RetainedStagedOutputEntry)>,
        )> = vec![
            (
                "symlink target",
                1,
                Box::new(|entry| {
                    entry.kind = RetainedStagedOutputEntryKind::Symlink {
                        target: b"nested/tool".to_vec(),
                    };
                }),
            ),
            (
                "symlink target::parent-relative",
                1,
                Box::new(|entry| {
                    entry.kind = RetainedStagedOutputEntryKind::Symlink {
                        target: b"../plain.txt".to_vec(),
                    };
                }),
            ),
            (
                "symlink kind::to-file",
                1,
                Box::new(|entry| {
                    entry.kind = RetainedStagedOutputEntryKind::File {
                        bytes: Arc::from(b"alias".as_slice()),
                        executable: false,
                    };
                }),
            ),
            (
                "executable class::cleared",
                2,
                Box::new(|entry| {
                    entry.kind = RetainedStagedOutputEntryKind::File {
                        bytes: Arc::from(b"#!/tool\n".as_slice()),
                        executable: false,
                    };
                }),
            ),
        ];
        for (name, index, mutate) in unix_representable {
            assert_representable_substitution(
                &session,
                name,
                &unix_baseline,
                unix_commitment,
                index,
                mutate,
            );
        }
    }

    // On hosts without a faithful executable mode or symlink kind the same
    // substitutions stay representable in the commitment and the honestly
    // re-committed record rejects at validation instead of materializing.
    #[cfg(not(unix))]
    {
        let mut executable = baseline.clone();
        executable.entries[5].kind = RetainedStagedOutputEntryKind::File {
            bytes: Arc::from(b"z".as_slice()),
            executable: true,
        };
        executable.commitment =
            commitment_for_retained_entries(&executable.entries).expect("recomputed");
        assert_ne!(executable.commitment, baseline_commitment);
        let error = executable
            .materialize_into(&session.destination("executable-mode"))
            .expect_err("an unrepresentable mode rejects");
        assert!(
            error
                .message()
                .contains("cannot represent retained executable file mode")
        );

        let mut linked = baseline.clone();
        linked.entries[4].kind = RetainedStagedOutputEntryKind::Symlink {
            target: b"docs/inner/a.txt".to_vec(),
        };
        linked.commitment = commitment_for_retained_entries(&linked.entries).expect("recomputed");
        assert_ne!(linked.commitment, baseline_commitment);
        let error = linked
            .materialize_into(&session.destination("symlink-kind"))
            .expect_err("an unrepresentable kind rejects");
        assert!(
            error
                .message()
                .contains("cannot faithfully materialize retained symlink kind")
        );
    }

    // ── Non-canonical records reject at validation in both commitment
    //    directions: each substitution cannot keep the canonical joins, so
    //    even the honestly re-committed record rejects before any write. ──
    let noncanonical: Vec<(
        &'static str,
        Box<dyn Fn(&mut Vec<RetainedStagedOutputEntry>)>,
        &'static str,
    )> = vec![
        (
            "path escapes root",
            Box::new(|entries| entries[5].relative_path = b"x/../escape".to_vec()),
            "non-portable path component",
        ),
        (
            "path is not UTF-8",
            Box::new(|entries| entries[5].relative_path = b"\xff.bin".to_vec()),
            "not canonical UTF-8",
        ),
        (
            "path is absolute",
            Box::new(|entries| entries[0].relative_path = b"/abs".to_vec()),
            "nonempty relative slash path",
        ),
        (
            "path has a trailing slash",
            Box::new(|entries| entries[5].relative_path = b"x-dir/".to_vec()),
            "nonempty relative slash path",
        ),
        (
            "path is empty",
            Box::new(|entries| entries[0].relative_path = Vec::new()),
            "nonempty relative slash path",
        ),
        (
            // `docs/inner//a.txt` keeps the strict sort at position 2, so
            // validation reaches the component check instead of the order
            // check.
            "path has an empty component",
            Box::new(|entries| entries[2].relative_path = b"docs/inner//a.txt".to_vec()),
            "non-portable path component",
        ),
        (
            "path has a backslash component",
            Box::new(|entries| entries[5].relative_path = b"x\\y".to_vec()),
            "non-portable path component",
        ),
        (
            "path uses a reserved device name",
            Box::new(|entries| entries[0].relative_path = b"NUL.txt".to_vec()),
            "reserved portable device name",
        ),
        (
            "path component has a trailing dot",
            Box::new(|entries| entries[5].relative_path = b"x.".to_vec()),
            "non-portable path component",
        ),
        (
            // `z-missing/a.txt` keeps the strict sort at the last position
            // while its parent directory is absent from the roster.
            "entry parent is missing",
            Box::new(|entries| entries[5].relative_path = b"z-missing/a.txt".to_vec()),
            "missing or non-directory parent",
        ),
        (
            "directory becomes a file",
            Box::new(|entries| {
                entries[0].kind = RetainedStagedOutputEntryKind::File {
                    bytes: Arc::from(b"dir".as_slice()),
                    executable: false,
                };
            }),
            "missing or non-directory parent",
        ),
        (
            "roster order swapped",
            Box::new(|entries| entries.swap(3, 4)),
            "not in strict canonical order",
        ),
        (
            "roster row duplicated",
            Box::new(|entries| entries.insert(5, entries[4].clone())),
            "not in strict canonical order",
        ),
        (
            "roster reversed",
            Box::new(|entries| entries.reverse()),
            "not in strict canonical order",
        ),
        (
            "directory row dropped",
            Box::new(|entries| {
                entries.remove(0);
            }),
            "missing or non-directory parent",
        ),
    ];
    for (name, mutate, fragment) in noncanonical {
        let mut entries = baseline.entries.clone();
        mutate(&mut entries);
        // The digest still diverges under honest recomputation, but the
        // record cannot verify at all: canonical-shape rejection precedes
        // the commitment join inside validation.
        let recomputed = commitment_for_retained_entries(&entries)
            .expect("mutated roster stays within the unique-content ceiling");
        assert_ne!(
            recomputed, baseline_commitment,
            "{name}: the substituted record must diverge"
        );
        for (direction, commitment) in [("stale", baseline_commitment), ("honest", recomputed)] {
            let record = tree_with(commitment, entries.clone());
            let destination = session.destination(&format!("{name}-{direction}"));
            let error = record
                .materialize_into(&destination)
                .expect_err("a non-canonical record must reject");
            assert!(
                error.message().contains(fragment),
                "{name}-{direction}: expected `{fragment}` in: {}",
                error.message()
            );
            assert!(
                std::fs::read_dir(&destination)
                    .expect("enumerate rejected destination")
                    .next()
                    .is_none(),
                "{name}-{direction}: a rejected record must leave its destination empty"
            );
        }
    }

    // ── Roster substitutions that keep the canonical shape diverge the
    //    recomputed identity, reject under the published one, and
    //    materialize only as the mutated record. ──
    let roster: Vec<(
        &'static str,
        Box<dyn Fn(&mut Vec<RetainedStagedOutputEntry>)>,
        u64,
        u64,
    )> = vec![
        (
            "leaf row dropped",
            Box::new(|entries| {
                entries.remove(5);
            }),
            5,
            34,
        ),
        (
            "file row dropped",
            Box::new(|entries| {
                entries.remove(2);
            }),
            5,
            30,
        ),
        (
            "forged row inserted at canonical position",
            Box::new(|entries| entries.insert(4, file_entry(b"logs.txt", b"log"))),
            7,
            38,
        ),
        (
            "subtree dropped",
            Box::new(|entries| {
                entries.drain(0..4);
            }),
            2,
            12,
        ),
        ("roster emptied", Box::new(|entries| entries.clear()), 0, 0),
    ];
    for (name, mutate, entry_count, file_bytes) in roster {
        let mut entries = baseline.entries.clone();
        mutate(&mut entries);
        let recomputed = commitment_for_retained_entries(&entries)
            .expect("mutated roster stays within the unique-content ceiling");
        assert_ne!(
            recomputed, baseline_commitment,
            "{name}: an honestly recomputed commitment must diverge"
        );
        assert_eq!(recomputed.entry_count(), entry_count, "{name}");
        assert_eq!(recomputed.file_bytes(), file_bytes, "{name}");
        assert_stale_identity_rejects(&session, name, baseline_commitment, &entries);
        let record = tree_with(recomputed, entries);
        let destination = session.destination(&format!("{name}-honest"));
        assert_eq!(
            record
                .materialize_into(&destination)
                .expect("canonical roster substitution materializes"),
            recomputed,
            "{name}"
        );
    }

    // ── Every `BuildStagedOutputTreeCommitment` field substitutes
    //    independently: the retained content never verifies against the
    //    substituted containing identity. ──
    let mut forged_digest = baseline_commitment;
    forged_digest.digest[0] ^= 0x01;
    let mut forged_entry_count = baseline_commitment;
    forged_entry_count.entry_count += 1;
    let mut forged_file_bytes = baseline_commitment;
    forged_file_bytes.file_bytes += 1;
    let mut over_ceiling = baseline_commitment;
    over_ceiling.file_bytes = MAX_STAGED_OUTPUT_UNIQUE_FILE_BYTES + 1;
    let commitment_cases: Vec<(&'static str, BuildStagedOutputTreeCommitment, &'static str)> = vec![
        (
            "commitment.digest",
            forged_digest,
            "disagrees with its commitment",
        ),
        (
            "commitment.digest::zeroed",
            BuildStagedOutputTreeCommitment {
                digest: [0; 32],
                ..baseline_commitment
            },
            "disagrees with its commitment",
        ),
        (
            "commitment.entry_count",
            forged_entry_count,
            "disagrees with its commitment",
        ),
        (
            "commitment.file_bytes",
            forged_file_bytes,
            "disagrees with its commitment",
        ),
        (
            "commitment.file_bytes::over-ceiling",
            over_ceiling,
            "unique-content ceiling",
        ),
        (
            "commitment::foreign-empty",
            empty().commitment(),
            "disagrees with its commitment",
        ),
        (
            "commitment::foreign-tree",
            foreign_tree().commitment(),
            "disagrees with its commitment",
        ),
    ];
    for (name, commitment, fragment) in commitment_cases {
        assert_ne!(commitment, baseline_commitment, "{name}");
        let record = tree_with(commitment, baseline.entries.clone());
        let destination = session.destination(name);
        let error = record
            .materialize_into(&destination)
            .expect_err("a substituted containing identity must reject");
        assert!(
            error.message().contains(fragment),
            "{name}: expected `{fragment}` in: {}",
            error.message()
        );
        assert!(
            std::fs::read_dir(&destination)
                .expect("enumerate rejected destination")
                .next()
                .is_none(),
            "{name}: a rejected record must leave its destination empty"
        );
    }

    // The retained-side entry ceiling is an encoding rejection: a record
    // that cannot be represented canonically never reaches the commitment
    // join. The constructor admission ceiling is already covered by
    // `tree_from_entries.rs`.
    let oversized: Vec<RetainedStagedOutputEntry> = (0..=MAX_STAGED_OUTPUT_ENTRIES)
        .map(|index| RetainedStagedOutputEntry {
            relative_path: format!("d{index:05}").into_bytes(),
            kind: RetainedStagedOutputEntryKind::Directory,
        })
        .collect();
    let oversized_commitment =
        commitment_for_retained_entries(&oversized).expect("the digest is not the ceiling");
    let record = tree_with(oversized_commitment, oversized);
    let error = record
        .materialize_into(session.destination("entry-ceiling"))
        .expect_err("an over-ceiling roster rejects");
    assert!(
        error.message().contains("4096-entry ceiling"),
        "unexpected rejection: {}",
        error.message()
    );

    // ── The include-source join: the generated-source handoff binds the
    //    sealed entry, so entry substitutions reject or diverge the
    //    retained evidence in both directions. ──
    let included = select_included_sources(&baseline, &[b"docs/inner/gen.omg".to_vec()])
        .expect("baseline generated source");
    let [baseline_source] = included.as_slice() else {
        panic!("one handoff retains one source")
    };
    assert_eq!(baseline_source.relative_path(), b"docs/inner/gen.omg");

    let mut bytes_substituted = baseline.clone();
    bytes_substituted.entries[3].kind = RetainedStagedOutputEntryKind::File {
        bytes: Arc::from(b"data Generated {0}\n".as_slice()),
        executable: false,
    };
    let included = select_included_sources(&bytes_substituted, &[b"docs/inner/gen.omg".to_vec()])
        .expect("a canonical byte substitution still hands off");
    assert_ne!(
        included[0].digest(),
        baseline_source.digest(),
        "the retained source evidence must diverge with the sealed bytes"
    );
    assert_eq!(included[0].bytes(), b"data Generated {0}\n");

    let mut kind_substituted = baseline.clone();
    kind_substituted.entries[3].kind = RetainedStagedOutputEntryKind::Directory;
    let error = select_included_sources(&kind_substituted, &[b"docs/inner/gen.omg".to_vec()])
        .expect_err("a non-file entry cannot hand off");
    assert!(
        error[0].message.contains("is not a regular file"),
        "unexpected diagnostics: {error:?}"
    );

    let mut executable_substituted = baseline.clone();
    executable_substituted.entries[3].kind = RetainedStagedOutputEntryKind::File {
        bytes: Arc::from(b"data Generated {}\n".as_slice()),
        executable: true,
    };
    let error = select_included_sources(&executable_substituted, &[b"docs/inner/gen.omg".to_vec()])
        .expect_err("an executable output cannot hand off as source");
    assert!(
        error[0].message.contains("must not be executable"),
        "unexpected diagnostics: {error:?}"
    );

    let mut path_substituted = baseline.clone();
    path_substituted.entries[3].relative_path = b"docs/inner/gen.txt".to_vec();
    let error = select_included_sources(&path_substituted, &[b"docs/inner/gen.omg".to_vec()])
        .expect_err("a renamed entry leaves the handoff absent");
    assert!(
        error[0]
            .message
            .contains("absent from the captured staged-output tree"),
        "unexpected diagnostics: {error:?}"
    );

    let mut dropped = baseline.clone();
    dropped.entries.remove(3);
    let error = select_included_sources(&dropped, &[b"docs/inner/gen.omg".to_vec()])
        .expect_err("a dropped entry leaves the handoff absent");
    assert!(
        error[0]
            .message
            .contains("absent from the captured staged-output tree"),
        "unexpected diagnostics: {error:?}"
    );

    let mut unhanded = baseline.clone();
    unhanded.entries[5].relative_path = b"z-last.omg".to_vec();
    let error = select_included_sources(&unhanded, &[b"docs/inner/gen.omg".to_vec()])
        .expect_err("a captured .omg output without a handoff rejects");
    assert!(
        error[0]
            .message
            .contains("no explicit include_source handoff"),
        "unexpected diagnostics: {error:?}"
    );

    // A forged digest on the selected source diverges the record; the
    // downstream source-consumption commitment binds the field, so the
    // substitution can never reproduce the published identity there.
    let mut forged_source = baseline_source.clone();
    forged_source.digest = [0xEE; 32];
    assert_ne!(forged_source, *baseline_source);
    assert_ne!(forged_source.digest(), baseline_source.digest());

    // ── Construction: a substituted entry list builds the
    //    honestly mutated record, never the baseline identity. ──
    let constructed = from_entries(&[
        OutputTreeEntry::directory(b"docs"),
        OutputTreeEntry::directory(b"docs/inner"),
        OutputTreeEntry::regular_file(b"docs/inner/a.txt", b"substituted", false),
        OutputTreeEntry::regular_file(b"docs/inner/gen.omg", b"data Generated {}\n", false),
        OutputTreeEntry::regular_file(b"report.txt", b"report-body", false),
        OutputTreeEntry::regular_file(b"z-last.bin", b"z", false),
    ])
    .expect("substituted entries");
    let mut expected = baseline.clone();
    expected.entries[2].kind = RetainedStagedOutputEntryKind::File {
        bytes: Arc::from(b"substituted".as_slice()),
        executable: false,
    };
    expected.commitment = commitment_for_retained_entries(&expected.entries).expect("recomputed");
    assert_eq!(constructed, expected);
    assert_ne!(constructed.commitment(), baseline_commitment);
    assert!(
        from_entries(&[
            OutputTreeEntry::directory(b"docs"),
            OutputTreeEntry::directory(b"docs"),
        ])
        .is_err(),
        "a duplicated entry rejects at admission"
    );
    let dropped_entry = from_entries(&[
        OutputTreeEntry::directory(b"docs"),
        OutputTreeEntry::directory(b"docs/inner"),
        OutputTreeEntry::regular_file(b"docs/inner/a.txt", b"alpha", false),
        OutputTreeEntry::regular_file(b"report.txt", b"report-body", false),
        OutputTreeEntry::regular_file(b"z-last.bin", b"z", false),
    ])
    .expect("dropped entry still constructs");
    assert_ne!(dropped_entry.commitment(), baseline_commitment);
}
