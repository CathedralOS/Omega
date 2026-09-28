//! Substitutable-field inventories for the staged-output-tree custody
//! matrices driven by `staged_output_tree/tests.rs`'s
//! `staged_output_tree_rejects_every_one_field_substitution`.
//!
//! The declared lanes are the `BuildStagedOutputTree` record's representable
//! custody axes: every retained entry field that keeps canonical shape, the
//! roster's membership, the canonical-shape violations validation refuses,
//! the retained-size ceiling, and each field of the containing
//! `BuildStagedOutputTreeCommitment`. Keeping the inventories in their own
//! file names the boundary between what is declared substitutable and the
//! shared `run_one_field_substitution_matrix` driver that exercises every
//! leg.
//!
//! Outcomes split two ways inside the family's independent checker
//! (`validate_retained_tree` then `verify_materialized_tree` against the
//! baseline materialization): a record whose canonical shape or claimed
//! commitment fails validation is refused with the exact materialization
//! error (`Validation`), and a canonical record whose claims diverge from
//! the observed baseline is refused by the replay walk with the named drift
//! kind (`Replay`). The replay's messages embed the materialization
//! destination, so the matrix compares classified kinds rather than exact
//! errors there.
//!
//! `UnixStagedOutputFieldForTest` is a second family over a symlink- and
//! executable-bearing roster: the replay destination cannot exist on hosts
//! that cannot materialize those kinds, so that matrix is `#[cfg(unix)]`
//! like the bespoke unrepresentable-mode assertions that stay beside it.

use crate::staged_output_tree::{
    BuildStagedOutputMaterializationError, BuildStagedOutputTree, BuildStagedOutputTreeCommitment,
    MAX_STAGED_OUTPUT_ENTRIES, MAX_STAGED_OUTPUT_UNIQUE_FILE_BYTES, RetainedStagedOutputEntry,
    RetainedStagedOutputEntryKind, commitment_for_retained_entries, empty,
};
use std::sync::Arc;

use super::file_entry;

optimization_core::custody_field_inventory! {
    /// One substitutable lane of the retained `BuildStagedOutputTree`;
    /// checked by `validate_retained_tree` plus `verify_materialized_tree`
    /// replay against the baseline materialization in the driving test.
    /// Content lanes recompute the containing commitment honestly — a
    /// canonical substitution is a different honest tree the baseline
    /// materialization refuses to match — while the `Commitment*` lanes
    /// substitute the published identity itself over honest content.
    pub enum StagedOutputTreeFieldForTest {
        // --- retained entry fields, canonical shape held ---
        NestedFilePath,
        GeneratedSourcePath,
        TopLevelFilePath,
        NestedFileBytes,
        SameLengthBytes,
        EmptiedFileBytes,
        DuplicateContentBytes,
        FileKindToDirectory,
        // --- roster membership ---
        LeafRowDropped,
        FileRowDropped,
        ForgedRowInserted,
        SubtreeDropped,
        RosterEmptied,
        // --- representable content the canonical shape refuses ---
        PathEscapesRoot,
        PathNotUtf8,
        PathAbsolute,
        PathTrailingSlash,
        PathEmpty,
        PathEmptyComponent,
        PathBackslashComponent,
        PathReservedName,
        PathTrailingDot,
        ParentDirectoryMissing,
        DirectoryBecomesFile,
        OrderSwapped,
        RowDuplicated,
        RosterReversed,
        DirectoryRowDropped,
        // --- retained-size ceiling ---
        EntryCeilingExceeded,
        // --- the containing identity itself ---
        CommitmentDigestForged,
        CommitmentDigestZeroed,
        CommitmentEntryCountForged,
        CommitmentFileBytesForged,
        CommitmentFileBytesOverCeiling,
        CommitmentForeignEmpty,
        CommitmentForeignTree,
    }
}

/// The named drift kind `verify_materialized_tree` reports when a canonical
/// substituted record is replayed against the baseline materialization. The
/// rejection messages embed the destination path, so the matrix compares
/// the named kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StagedTreeReplayKind {
    /// An observed baseline entry is absent from the record's claims.
    AbsentFromClaims,
    /// An observed entry's concrete kind disagrees with the claimed kind.
    KindMismatch,
    /// An observed file's bytes disagree with the claimed bytes.
    BytesMismatch,
    /// An observed file's length or mode disagrees with the claim.
    LengthOrModeMismatch,
    /// An observed symlink's spelling disagrees with the claimed target.
    TargetMismatch,
    /// A claimed entry is missing from the observed baseline.
    ClaimedEntryMissing,
}

/// Classify a replay rejection into the family's named drift vocabulary;
/// any other message means the substitution produced a verdict shape the
/// family does not declare.
pub fn staged_tree_replay_kind(
    error: &BuildStagedOutputMaterializationError,
) -> StagedTreeReplayKind {
    use StagedTreeReplayKind as Kind;
    let message = error.message();
    if message.contains("is absent from retained content") {
        Kind::AbsentFromClaims
    } else if message.contains("disagrees with retained kind") {
        Kind::KindMismatch
    } else if message.contains("disagrees with retained bytes") {
        Kind::BytesMismatch
    } else if message.contains("disagrees with retained length or mode") {
        Kind::LengthOrModeMismatch
    } else if message.contains("disagrees with retained spelling") {
        Kind::TargetMismatch
    } else if message.contains("is missing after materialization") {
        Kind::ClaimedEntryMissing
    } else {
        panic!("replay verdict outside the declared vocabulary: {message}")
    }
}

/// The family's independent-checker verdict: validation refuses the record's
/// own canonical shape or claimed identity (`Validation`, exact error —
/// validation messages name only the entry's own path), or the record
/// passes validation and the replay walk refuses it against the observed
/// baseline (`Replay`, named drift kind).
#[derive(Debug, PartialEq)]
pub enum StagedTreeCheck {
    /// `validate_retained_tree` rejected the record: the exact
    /// materialization error it names.
    Validation(BuildStagedOutputMaterializationError),
    /// `verify_materialized_tree` rejected the canonical record against the
    /// baseline materialization: the named drift kind.
    Replay(StagedTreeReplayKind),
}

/// Recompute the containing identity over the mutated roster: the honest
/// direction a content substitution must still take. The digest accounts
/// unique file content, so the ceiling legs' oversized roster and the
/// non-canonical lanes commit just the same — canonical shape belongs to
/// validation, not to the commitment.
fn recommit(tree: &mut BuildStagedOutputTree) {
    tree.commitment = commitment_for_retained_entries(&tree.entries)
        .expect("a mutated roster stays within the unique-content ceiling");
}

/// The portable family's honest-recomputation hook: mutate exactly the
/// declared field of `tree`, then recompute the containing commitment for
/// every content lane so the substituted record keeps its own honest
/// identity. The `Commitment*` lanes substitute the published identity
/// itself — the claim is the field, so nothing recomputes. `donor` supplies
/// the authentic foreign commitment the `CommitmentForeignTree` leg borrows;
/// the remaining legs take fixed non-identity alternates.
pub fn substitute_staged_tree_for_test(
    tree: &mut BuildStagedOutputTree,
    field: StagedOutputTreeFieldForTest,
    donor: &BuildStagedOutputTree,
) {
    use StagedOutputTreeFieldForTest as Leg;
    match field {
        Leg::NestedFilePath => {
            tree.entries[2].relative_path = b"docs/inner/b.txt".to_vec();
            recommit(tree);
        }
        Leg::GeneratedSourcePath => {
            tree.entries[3].relative_path = b"docs/inner/gen.txt".to_vec();
            recommit(tree);
        }
        Leg::TopLevelFilePath => {
            tree.entries[4].relative_path = b"report.bin".to_vec();
            recommit(tree);
        }
        Leg::NestedFileBytes => {
            tree.entries[2].kind = RetainedStagedOutputEntryKind::File {
                bytes: Arc::from(b"alpha-2".as_slice()),
                executable: false,
            };
            recommit(tree);
        }
        Leg::SameLengthBytes => {
            tree.entries[4].kind = RetainedStagedOutputEntryKind::File {
                bytes: Arc::from(b"report-cody".as_slice()),
                executable: false,
            };
            recommit(tree);
        }
        Leg::EmptiedFileBytes => {
            tree.entries[5].kind = RetainedStagedOutputEntryKind::File {
                bytes: Arc::from(b"".as_slice()),
                executable: false,
            };
            recommit(tree);
        }
        // Duplicate content folds into the same unique byte account, so this
        // substitution also diverges `file_bytes` below the baseline.
        Leg::DuplicateContentBytes => {
            tree.entries[5].kind = RetainedStagedOutputEntryKind::File {
                bytes: Arc::from(b"report-body".as_slice()),
                executable: false,
            };
            recommit(tree);
        }
        Leg::FileKindToDirectory => {
            tree.entries[2].kind = RetainedStagedOutputEntryKind::Directory;
            recommit(tree);
        }
        Leg::LeafRowDropped => {
            tree.entries.remove(5);
            recommit(tree);
        }
        Leg::FileRowDropped => {
            tree.entries.remove(2);
            recommit(tree);
        }
        Leg::ForgedRowInserted => {
            tree.entries.insert(4, file_entry(b"logs.txt", b"log"));
            recommit(tree);
        }
        Leg::SubtreeDropped => {
            tree.entries.drain(0..4);
            recommit(tree);
        }
        Leg::RosterEmptied => {
            tree.entries.clear();
            recommit(tree);
        }
        Leg::PathEscapesRoot => {
            tree.entries[5].relative_path = b"x/../escape".to_vec();
            recommit(tree);
        }
        Leg::PathNotUtf8 => {
            tree.entries[5].relative_path = b"\xff.bin".to_vec();
            recommit(tree);
        }
        Leg::PathAbsolute => {
            tree.entries[0].relative_path = b"/abs".to_vec();
            recommit(tree);
        }
        Leg::PathTrailingSlash => {
            tree.entries[5].relative_path = b"x-dir/".to_vec();
            recommit(tree);
        }
        Leg::PathEmpty => {
            tree.entries[0].relative_path = Vec::new();
            recommit(tree);
        }
        Leg::PathEmptyComponent => {
            tree.entries[2].relative_path = b"docs/inner//a.txt".to_vec();
            recommit(tree);
        }
        Leg::PathBackslashComponent => {
            tree.entries[5].relative_path = b"x\\y".to_vec();
            recommit(tree);
        }
        Leg::PathReservedName => {
            tree.entries[0].relative_path = b"NUL.txt".to_vec();
            recommit(tree);
        }
        Leg::PathTrailingDot => {
            tree.entries[5].relative_path = b"x.".to_vec();
            recommit(tree);
        }
        Leg::ParentDirectoryMissing => {
            tree.entries[5].relative_path = b"z-missing/a.txt".to_vec();
            recommit(tree);
        }
        Leg::DirectoryBecomesFile => {
            tree.entries[0].kind = RetainedStagedOutputEntryKind::File {
                bytes: Arc::from(b"dir".as_slice()),
                executable: false,
            };
            recommit(tree);
        }
        Leg::OrderSwapped => {
            tree.entries.swap(3, 4);
            recommit(tree);
        }
        Leg::RowDuplicated => {
            tree.entries.insert(5, tree.entries[4].clone());
            recommit(tree);
        }
        Leg::RosterReversed => {
            tree.entries.reverse();
            recommit(tree);
        }
        Leg::DirectoryRowDropped => {
            tree.entries.remove(0);
            recommit(tree);
        }
        Leg::EntryCeilingExceeded => {
            tree.entries = (0..=MAX_STAGED_OUTPUT_ENTRIES)
                .map(|index| RetainedStagedOutputEntry {
                    relative_path: format!("d{index:05}").into_bytes(),
                    kind: RetainedStagedOutputEntryKind::Directory,
                })
                .collect();
            recommit(tree);
        }
        Leg::CommitmentDigestForged => tree.commitment.digest[0] ^= 0x01,
        Leg::CommitmentDigestZeroed => {
            tree.commitment = BuildStagedOutputTreeCommitment {
                digest: [0; 32],
                ..tree.commitment
            };
        }
        Leg::CommitmentEntryCountForged => tree.commitment.entry_count += 1,
        Leg::CommitmentFileBytesForged => tree.commitment.file_bytes += 1,
        Leg::CommitmentFileBytesOverCeiling => {
            tree.commitment.file_bytes = MAX_STAGED_OUTPUT_UNIQUE_FILE_BYTES + 1;
        }
        Leg::CommitmentForeignEmpty => tree.commitment = empty().commitment(),
        Leg::CommitmentForeignTree => tree.commitment = donor.commitment(),
    }
}

/// Build the exact `Validation` verdict: `validate_retained_tree`'s
/// messages name only the record's own path, so the leg declares the
/// complete rejection.
fn validation(message: impl Into<String>) -> StagedTreeCheck {
    StagedTreeCheck::Validation(BuildStagedOutputMaterializationError {
        message: message.into(),
    })
}

/// The exact checker verdict each declared portable leg must produce: a
/// canonical substitution replays against the baseline materialization and
/// names its drift kind; a canonical-shape violation or a substituted claim
/// is the validator's exact rejection.
pub fn staged_tree_custody_outcome(
    field: StagedOutputTreeFieldForTest,
) -> optimization_core::MutationOutcome<StagedTreeCheck> {
    use StagedOutputTreeFieldForTest as Leg;
    use StagedTreeCheck as Check;
    use StagedTreeReplayKind as Replay;
    let check = match field {
        // A renamed path leaves the observed baseline entry unclaimed.
        Leg::NestedFilePath | Leg::GeneratedSourcePath | Leg::TopLevelFilePath => {
            Check::Replay(Replay::AbsentFromClaims)
        }
        // Byte substitutions that move the retained length reject at the
        // length/mode join before content is read.
        Leg::NestedFileBytes | Leg::EmptiedFileBytes | Leg::DuplicateContentBytes => {
            Check::Replay(Replay::LengthOrModeMismatch)
        }
        Leg::SameLengthBytes => Check::Replay(Replay::BytesMismatch),
        Leg::FileKindToDirectory => Check::Replay(Replay::KindMismatch),
        Leg::LeafRowDropped | Leg::FileRowDropped | Leg::SubtreeDropped | Leg::RosterEmptied => {
            Check::Replay(Replay::AbsentFromClaims)
        }
        // An inserted row is claimed but never observed.
        Leg::ForgedRowInserted => Check::Replay(Replay::ClaimedEntryMissing),
        Leg::PathEscapesRoot => {
            validation("build staged-output entry `x/../escape` has a non-portable path component")
        }
        Leg::PathNotUtf8 => validation("retained staged-output path is not canonical UTF-8"),
        Leg::PathAbsolute | Leg::PathTrailingSlash | Leg::PathEmpty => {
            validation("retained staged-output path is not a nonempty relative slash path")
        }
        Leg::PathEmptyComponent => validation(
            "build staged-output entry `docs/inner//a.txt` has a non-portable path component",
        ),
        Leg::PathBackslashComponent => {
            validation("build staged-output entry `x\\y` has a non-portable path component")
        }
        Leg::PathReservedName => {
            validation("build staged-output entry `NUL.txt` uses a reserved portable device name")
        }
        Leg::PathTrailingDot => {
            validation("build staged-output entry `x.` has a non-portable path component")
        }
        Leg::ParentDirectoryMissing => validation(
            "retained staged-output entry `z-missing/a.txt` has a missing or non-directory parent",
        ),
        Leg::DirectoryBecomesFile | Leg::DirectoryRowDropped => validation(
            "retained staged-output entry `docs/inner` has a missing or non-directory parent",
        ),
        Leg::OrderSwapped | Leg::RowDuplicated | Leg::RosterReversed => {
            validation("retained staged-output paths are not in strict canonical order")
        }
        Leg::EntryCeilingExceeded => validation(format!(
            "retained staged-output tree exceeds its {MAX_STAGED_OUTPUT_ENTRIES}-entry ceiling"
        )),
        Leg::CommitmentFileBytesOverCeiling => validation(format!(
            "retained staged-output tree exceeds its {MAX_STAGED_OUTPUT_UNIQUE_FILE_BYTES}-byte \
             unique-content ceiling"
        )),
        Leg::CommitmentDigestForged
        | Leg::CommitmentDigestZeroed
        | Leg::CommitmentEntryCountForged
        | Leg::CommitmentFileBytesForged
        | Leg::CommitmentForeignEmpty
        | Leg::CommitmentForeignTree => {
            validation("retained staged-output content disagrees with its commitment")
        }
    };
    optimization_core::MutationOutcome::ExactError(check)
}

#[cfg(unix)]
optimization_core::custody_field_inventory! {
    /// One substitutable lane of the unix staged-output roster, which
    /// carries a real symlink and an executable file so the link-target
    /// spelling and the executable class are exercised as representable
    /// fields. Checked by the same validation-then-replay chain against the
    /// unix baseline materialization.
    pub enum UnixStagedOutputFieldForTest {
        SymlinkTargetSubstituted,
        SymlinkTargetParentRelative,
        SymlinkToFile,
        ExecutableCleared,
    }
}

/// The unix family's honest-recomputation hook: mutate exactly the declared
/// field of `tree` — a symlink target spelling or a kind/mode class — then
/// recompute the containing commitment.
#[cfg(unix)]
pub fn substitute_unix_staged_tree_for_test(
    tree: &mut BuildStagedOutputTree,
    field: UnixStagedOutputFieldForTest,
    _donor: &BuildStagedOutputTree,
) {
    use UnixStagedOutputFieldForTest as Leg;
    match field {
        Leg::SymlinkTargetSubstituted => {
            tree.entries[1].kind = RetainedStagedOutputEntryKind::Symlink {
                target: b"nested/tool".to_vec(),
            };
        }
        Leg::SymlinkTargetParentRelative => {
            tree.entries[1].kind = RetainedStagedOutputEntryKind::Symlink {
                target: b"../plain.txt".to_vec(),
            };
        }
        Leg::SymlinkToFile => {
            tree.entries[1].kind = RetainedStagedOutputEntryKind::File {
                bytes: Arc::from(b"alias".as_slice()),
                executable: false,
            };
        }
        Leg::ExecutableCleared => {
            tree.entries[2].kind = RetainedStagedOutputEntryKind::File {
                bytes: Arc::from(b"#!/tool\n".as_slice()),
                executable: false,
            };
        }
    }
    recommit(tree);
}

/// The exact checker verdict each declared unix leg must produce: every
/// lane stays canonical and is refused by the replay walk against the unix
/// baseline materialization.
#[cfg(unix)]
pub fn unix_staged_tree_outcome(
    field: UnixStagedOutputFieldForTest,
) -> optimization_core::MutationOutcome<StagedTreeCheck> {
    use StagedTreeCheck as Check;
    use StagedTreeReplayKind as Replay;
    use UnixStagedOutputFieldForTest as Leg;
    let check = match field {
        Leg::SymlinkTargetSubstituted | Leg::SymlinkTargetParentRelative => {
            Check::Replay(Replay::TargetMismatch)
        }
        Leg::SymlinkToFile => Check::Replay(Replay::KindMismatch),
        Leg::ExecutableCleared => Check::Replay(Replay::LengthOrModeMismatch),
    };
    optimization_core::MutationOutcome::ExactError(check)
}
