//! One-field mutation matrix over the retained staged-output tree.
//!
//! The record's substitutable lanes — every retained entry field that keeps
//! canonical shape, roster membership, the canonical-shape violations
//! validation refuses, the retained-size ceiling, and each field of the
//! containing `BuildStagedOutputTreeCommitment` — are declared once in
//! `staged_output_custody_fields.rs` and driven by the shared
//! `run_one_field_substitution_matrix` driver. A content substitution
//! recomputes the containing commitment honestly, so the substituted record
//! is a different honest tree whose claims the baseline materialization
//! refuses to match; a claim substitution is refused at validation's
//! commitment join instead. The independent checker stays this family's:
//! `validate_retained_tree` then `verify_materialized_tree` against the
//! materialized baseline.
//!
//! What stays authored beside the matrix is not a one-field substitution:
//! the host-representability assertions that only exist where the host
//! cannot materialize the kind at all, the include-source handoff's own
//! rejection vocabulary, the forged-source digest, and constructor
//! admission.

mod file_selection;
mod staged_output_custody_fields;

use super::{
    BuildStagedOutputTree, BuildStagedOutputTreeCommitment, RetainedStagedOutputEntry,
    RetainedStagedOutputEntryKind, commitment_for_retained_entries, select_included_sources,
};
use crate::materialization::{validate_retained_tree, verify_materialized_tree};
use crate::{OutputTreeEntry, from_entries};
use optimization_core::{
    MutationOutcome, OneFieldSubstitutionMatrix, run_one_field_substitution_matrix,
};
use staged_output_custody_fields::{
    StagedOutputTreeFieldForTest, StagedTreeCheck, staged_tree_custody_outcome,
    staged_tree_replay_kind, substitute_staged_tree_for_test,
};
#[cfg(unix)]
use staged_output_custody_fields::{
    UnixStagedOutputFieldForTest, substitute_unix_staged_tree_for_test, unix_staged_tree_outcome,
};
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

/// The retained custody view a substitution must move: the honestly
/// recomputed content identity together with the claim the record actually
/// publishes. A content substitution moves the first half, a claim
/// substitution the second — both diverge from the honest record's pair.
fn retained_custody(
    record: &BuildStagedOutputTree,
) -> (
    Option<BuildStagedOutputTreeCommitment>,
    BuildStagedOutputTreeCommitment,
) {
    (
        commitment_for_retained_entries(&record.entries),
        record.commitment(),
    )
}

/// The family's independent checker: first the record's own validation —
/// canonical shape and the claimed commitment — then an independent replay
/// walk of the baseline materialization the record claims to describe. The
/// first stage's exact error and the replay stage's named drift kind are the
/// verdict the matrix compares.
fn replay_against_baseline(
    baseline_directory: &std::path::Path,
    record: &BuildStagedOutputTree,
) -> Result<
    (
        Option<BuildStagedOutputTreeCommitment>,
        BuildStagedOutputTreeCommitment,
    ),
    StagedTreeCheck,
> {
    validate_retained_tree(record).map_err(StagedTreeCheck::Validation)?;
    verify_materialized_tree(baseline_directory, record)
        .map_err(|error| StagedTreeCheck::Replay(staged_tree_replay_kind(&error)))?;
    Ok(retained_custody(record))
}

/// The honest-direction join shared by both matrices: a canonical
/// substitution materializes under its own divergent identity, and a
/// non-canonical record still rejects at validation before any write.
fn materialize_honest_direction(
    session: &Session,
    record: &BuildStagedOutputTree,
    field: impl std::fmt::Debug,
    expected: MutationOutcome<StagedTreeCheck>,
) {
    let destination = session.destination(&format!("{field:?}-honest"));
    match expected {
        MutationOutcome::ExactError(StagedTreeCheck::Validation(expected)) => {
            let error = record
                .materialize_into(&destination)
                .expect_err("a non-canonical record must reject");
            assert_eq!(
                error, expected,
                "{field:?}: the materializer names the same validation rejection"
            );
            assert!(
                std::fs::read_dir(&destination)
                    .expect("enumerate rejected destination")
                    .next()
                    .is_none(),
                "{field:?}: a rejected record must leave its destination empty"
            );
        }
        _ => assert_eq!(
            record
                .materialize_into(&destination)
                .expect("a canonical substitution materializes under its honest identity"),
            record.commitment(),
            "{field:?}"
        ),
    }
}

#[test]
fn staged_output_tree_rejects_every_one_field_substitution() {
    let session = Session::new();
    let baseline = baseline_tree();
    let baseline_commitment = baseline.commitment();
    assert_eq!(baseline.entry_count(), 6);
    assert_eq!(baseline.file_bytes(), 35);

    // Control: the baseline retains its own identity and materializes
    // under it — the replay destination every leg is checked against.
    let baseline_directory = session.destination("control");
    assert_eq!(
        baseline
            .materialize_into(&baseline_directory)
            .expect("baseline materializes"),
        baseline_commitment
    );

    let check =
        |record: &BuildStagedOutputTree| replay_against_baseline(&baseline_directory, record);

    // Every declared field: the substitution changes the record (the
    // custody view pairs the honestly recomputed content identity with the
    // retained claim), the containing identity recomputes honestly, and the
    // independent validation-plus-replay chain names the rejection. The
    // joined replay materializes the honest direction itself, so a
    // canonical substitution proves it is real evidence and a non-canonical
    // record proves validation — not the replay — refuses it before any
    // write.
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "staged output tree",
        fields: StagedOutputTreeFieldForTest::INVENTORY,
        honest: &|| baseline.clone(),
        donor: foreign_tree(),
        custody: &retained_custody,
        substitute: &substitute_staged_tree_for_test,
        check: &check,
        outcome: &staged_tree_custody_outcome,
        joined_replay: Some(&|record, field| {
            materialize_honest_direction(
                &session,
                record,
                field,
                staged_tree_custody_outcome(field),
            )
        }),
    });

    // The unix roster carries a real symlink and an executable file, so the
    // link-target spelling and the executable class are representable fields
    // on hosts that materialize them faithfully.
    #[cfg(unix)]
    {
        let unix_baseline = from_entries(&[
            OutputTreeEntry::directory(b"bin"),
            OutputTreeEntry::symbolic_link(b"bin/current", b"tool"),
            OutputTreeEntry::regular_file(b"bin/tool", b"#!/tool\n", true),
            OutputTreeEntry::regular_file(b"plain.txt", b"plain", false),
        ])
        .expect("unix baseline retained tree");
        let unix_directory = session.destination("unix-control");
        assert_eq!(
            unix_baseline
                .materialize_into(&unix_directory)
                .expect("unix baseline materializes"),
            unix_baseline.commitment()
        );
        let unix_check =
            |record: &BuildStagedOutputTree| replay_against_baseline(&unix_directory, record);
        run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
            family: "unix staged output tree",
            fields: UnixStagedOutputFieldForTest::INVENTORY,
            honest: &|| unix_baseline.clone(),
            donor: foreign_tree(),
            custody: &retained_custody,
            substitute: &substitute_unix_staged_tree_for_test,
            check: &unix_check,
            outcome: &unix_staged_tree_outcome,
            joined_replay: Some(&|record, field| {
                materialize_honest_direction(
                    &session,
                    record,
                    field,
                    unix_staged_tree_outcome(field),
                )
            }),
        });
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
            .materialize_into(session.destination("executable-mode"))
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
            .materialize_into(session.destination("symlink-kind"))
            .expect_err("an unrepresentable kind rejects");
        assert!(
            error
                .message()
                .contains("cannot faithfully materialize retained symlink kind")
        );
    }

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
