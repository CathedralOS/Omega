//! Substitutable-field inventory for the captured-source custody matrix
//! driven by `captured_source/tests.rs`'s
//! `captured_build_source_input_rejects_every_one_field_substitution`.
//!
//! The declared lanes are the `CapturedBuildSourceInput` record's
//! representable custody axes: every retained entry field — path, kind,
//! extent, bytes, digest, executable class, and roster membership — every
//! row field and the content commitment of the containing
//! `CanonicalFilesystemMetadataIndex`, and the derived `file_bytes`
//! projection. Keeping the inventory in its own file names the boundary
//! between what is declared substitutable and the shared
//! `run_one_field_substitution_matrix` driver that exercises every leg.
//!
//! The family's independent checker has three stages, each naming its own
//! verdict: `from_capture_rows` must re-admit the record's own index and
//! entries and re-derive the same record (`Admission` and
//! `ProjectionDiverged` rejections), the admitted record must survive
//! re-inspection materialization (`Materialization`), and the surviving
//! record must reproduce the retained baseline evidence
//! (`DivergentEvidence` — a substitution that binds different content to
//! the published index can never equal it).
//!
//! One substitution only the input-rows domain can express — a duplicated
//! relative path, unrepresentable inside the record's `BTreeMap` roster —
//! stays authored beside the matrix, together with the index's own
//! policy-version encoding rejection.

use checked_interpreter::{
    CanonicalFilesystemMetadataIndex, CanonicalFilesystemMetadataRow,
    CanonicalFilesystemMetadataRowKind,
};
use diagnostics::Diagnostic;
use std::sync::Arc;

use crate::captured_source::{
    CapturedBuildSourceInput, CapturedSourceEntry, CapturedSourceMaterializationError,
};

optimization_core::custody_field_inventory! {
    /// One substitutable lane of a retained `CapturedBuildSourceInput`;
    /// checked by re-admission, projection re-derivation, re-inspection
    /// materialization, and comparison against the retained baseline in the
    /// driving test.
    pub enum CapturedSourceFieldForTest {
        // --- retained entry fields the joined index still admits ---
        BannerFileBytes,
        BannerFileDigest,
        LinkTarget,
        LinkDigest,
        // --- retained entry fields the join refuses ---
        BannerPathRenamed,
        LinkPathEscapesRoot,
        LinkPathEmpty,
        LinkPathNul,
        LinkPathBackslash,
        BannerKindToDirectory,
        TemplatesKindToFile,
        BannerKindToSymlink,
        LinkKindToFile,
        LinkKindToDirectory,
        BannerByteLength,
        RunnerExecutableClass,
        LinkTargetLength,
        BannerRowDropped,
        ToolsRowDropped,
        LinkRowDropped,
        OutsideRowInserted,
        FileValuesSwapped,
        // --- the containing metadata index ---
        IndexPathRenamed,
        IndexExecutableClass,
        IndexFileExtent,
        IndexSymlinkExtent,
        IndexKindSwapped,
        IndexRowDropped,
        IndexRowInserted,
        IndexCommitmentForged,
        // --- the derived projection ---
        FileBytesProjection,
    }
}

/// The named rejection `from_capture_rows` reports for a record whose own
/// fields no longer join. The diagnostics carry the offending path, so the
/// matrix compares the named kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapturedSourceAdmissionKind {
    /// An entry's path is not a canonical relative path.
    NoncanonicalPath,
    /// An entry names a path the index does not list.
    AbsentFromIndex,
    /// An entry's kind or extent disagrees with its index row.
    KindOrExtentMismatch,
    /// The roster omits a file the index lists.
    OmitsFile,
    /// The roster omits a directory the index lists.
    OmitsDirectory,
    /// The roster omits a symlink the index lists.
    OmitsSymlink,
}

/// Classify the admission diagnostics into the family's named vocabulary;
/// any other message means the substitution produced a verdict shape the
/// family does not declare.
pub fn captured_source_admission_kind(diagnostics: &[Diagnostic]) -> CapturedSourceAdmissionKind {
    use CapturedSourceAdmissionKind as Kind;
    let contains = |fragment: &str| {
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(fragment))
    };
    if contains("noncanonical relative path") {
        Kind::NoncanonicalPath
    } else if contains("absent from its canonical source metadata index") {
        Kind::AbsentFromIndex
    } else if contains("disagrees with its canonical source metadata kind or extent") {
        Kind::KindOrExtentMismatch
    } else if contains("omits canonical source file") {
        Kind::OmitsFile
    } else if contains("omits canonical source directory") {
        Kind::OmitsDirectory
    } else if contains("omits canonical source symlink") {
        Kind::OmitsSymlink
    } else {
        panic!("admission verdict outside the declared vocabulary: {diagnostics:?}")
    }
}

/// The named rejection re-inspection materialization reports for an
/// admitted record. The messages embed the materialization path, so the
/// matrix compares the named kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapturedSourceMaterializationKind {
    /// A retained file's re-read bytes do not digest to its retained digest.
    ContentDrifted,
    /// This host cannot materialize the record's executable class.
    UnrepresentableExecutableMode,
}

/// Classify a materialization rejection into the family's named vocabulary;
/// any other message means the substitution produced a verdict shape the
/// family does not declare.
pub fn captured_source_materialization_kind(
    error: &CapturedSourceMaterializationError,
) -> CapturedSourceMaterializationKind {
    use CapturedSourceMaterializationKind as Kind;
    let message = error.message();
    if message.contains("content drifted") {
        Kind::ContentDrifted
    } else if message.contains("executable mode") {
        Kind::UnrepresentableExecutableMode
    } else {
        panic!("materialization verdict outside the declared vocabulary: {message}")
    }
}

/// The family's independent-checker verdict for one substituted record.
#[derive(Debug, PartialEq)]
pub enum CapturedSourceCheck {
    /// `from_capture_rows` refuses the record's own index and entries: the
    /// named admission rejection.
    Admission(CapturedSourceAdmissionKind),
    /// Re-admission derives a different record: a projection field was
    /// forged outside what the join re-derives.
    ProjectionDiverged,
    /// The admitted record fails re-inspection materialization: the named
    /// rejection kind.
    Materialization(CapturedSourceMaterializationKind),
    /// The admitted record materializes honestly but cannot reproduce the
    /// retained baseline evidence — it binds different content to the
    /// published index, so the downstream source-consumption commitment can
    /// never match it.
    DivergentEvidence,
}

/// Replace the containing index with one whose rows `mutate` rewrote,
/// keeping the record's own source-content commitment.
fn rewrite_index(
    record: &mut CapturedBuildSourceInput,
    mutate: impl Fn(&mut Vec<CanonicalFilesystemMetadataRow>),
) {
    let commitment = *record.canonical_source_metadata.source_content_commitment();
    let mut rows = record.canonical_source_metadata.rows().collect::<Vec<_>>();
    mutate(&mut rows);
    record.canonical_source_metadata =
        CanonicalFilesystemMetadataIndex::version_1(commitment, rows)
            .expect("a substituted index still encodes canonically");
}

/// The family's honest-recomputation hook: mutate exactly the declared
/// field of `record`. Entry-roster legs edit the retained `BTreeMap`;
/// `Index*` legs rewrite one row or the content commitment of the
/// containing index; `FileBytesProjection` forges the derived cache. The
/// record's other fields stay untouched — `file_bytes` is recomputed by
/// admission itself, and every admissible substitution here keeps the
/// baseline's file lengths, so no leg needs to re-derive it by hand.
/// `donor` is an authentic foreign capture; no leg draws its values.
pub fn substitute_captured_source_for_test(
    record: &mut CapturedBuildSourceInput,
    field: CapturedSourceFieldForTest,
    _donor: &CapturedBuildSourceInput,
) {
    use CapturedSourceFieldForTest as Leg;
    match field {
        // Same-length bytes keep the joined extent; `file` derives the
        // honest content digest at the substitution boundary.
        Leg::BannerFileBytes => {
            record.entries.insert(
                b"templates/banner.tmpl".to_vec(),
                CapturedSourceEntry::file(b"HELLO WORLE".to_vec(), false),
            );
        }
        // A forged digest disagrees with the retained bytes: admission
        // accepts it (the digest is re-inspected, not joined).
        Leg::BannerFileDigest => {
            record.entries.insert(
                b"templates/banner.tmpl".to_vec(),
                CapturedSourceEntry::File {
                    bytes: Arc::from(b"HELLO WORLD".as_slice()),
                    digest: [0xEE; 32],
                    executable: false,
                },
            );
        }
        // A same-length target keeps the joined spelling extent.
        Leg::LinkTarget => {
            record.entries.insert(
                b"link".to_vec(),
                CapturedSourceEntry::symlink(b"templater".to_vec()),
            );
        }
        // A forged inert-link digest is identity-bound only: materialization
        // never re-inspects captured links.
        Leg::LinkDigest => {
            record.entries.insert(
                b"link".to_vec(),
                CapturedSourceEntry::Symlink {
                    target: Arc::from(b"templates".as_slice()),
                    digest: [0xEE; 32],
                },
            );
        }
        Leg::BannerPathRenamed => {
            let entry = record
                .entries
                .remove(b"templates/banner.tmpl".as_slice())
                .expect("banner entry");
            record
                .entries
                .insert(b"templates/renamed.tmpl".to_vec(), entry);
        }
        Leg::LinkPathEscapesRoot => {
            let entry = record
                .entries
                .remove(b"link".as_slice())
                .expect("link entry");
            record.entries.insert(b"../escape".to_vec(), entry);
        }
        Leg::LinkPathEmpty => {
            let entry = record
                .entries
                .remove(b"link".as_slice())
                .expect("link entry");
            record.entries.insert(Vec::new(), entry);
        }
        Leg::LinkPathNul => {
            let entry = record
                .entries
                .remove(b"link".as_slice())
                .expect("link entry");
            record.entries.insert(b"li\0nk".to_vec(), entry);
        }
        Leg::LinkPathBackslash => {
            let entry = record
                .entries
                .remove(b"link".as_slice())
                .expect("link entry");
            record.entries.insert(b"tem\\plates".to_vec(), entry);
        }
        Leg::BannerKindToDirectory => {
            record.entries.insert(
                b"templates/banner.tmpl".to_vec(),
                CapturedSourceEntry::Directory,
            );
        }
        Leg::TemplatesKindToFile => {
            record.entries.insert(
                b"templates".to_vec(),
                CapturedSourceEntry::file(b"NOT A DIRECTORY".to_vec(), false),
            );
        }
        Leg::BannerKindToSymlink => {
            record.entries.insert(
                b"templates/banner.tmpl".to_vec(),
                CapturedSourceEntry::symlink(b"eleven byte".to_vec()),
            );
        }
        Leg::LinkKindToFile => {
            record.entries.insert(
                b"link".to_vec(),
                CapturedSourceEntry::file(b"nine byte".to_vec(), false),
            );
        }
        Leg::LinkKindToDirectory => {
            record
                .entries
                .insert(b"link".to_vec(), CapturedSourceEntry::Directory);
        }
        Leg::BannerByteLength => {
            record.entries.insert(
                b"templates/banner.tmpl".to_vec(),
                CapturedSourceEntry::file(b"SHORT".to_vec(), false),
            );
        }
        Leg::RunnerExecutableClass => {
            record.entries.insert(
                b"tools/run.sh".to_vec(),
                CapturedSourceEntry::file(b"RUN".to_vec(), false),
            );
        }
        Leg::LinkTargetLength => {
            record.entries.insert(
                b"link".to_vec(),
                CapturedSourceEntry::symlink(b"too-long-target".to_vec()),
            );
        }
        Leg::BannerRowDropped => {
            record.entries.remove(b"templates/banner.tmpl".as_slice());
        }
        Leg::ToolsRowDropped => {
            record.entries.remove(b"tools".as_slice());
        }
        Leg::LinkRowDropped => {
            record.entries.remove(b"link".as_slice());
        }
        Leg::OutsideRowInserted => {
            record.entries.insert(
                b"extra.txt".to_vec(),
                CapturedSourceEntry::file(b"EXTRA".to_vec(), false),
            );
        }
        Leg::FileValuesSwapped => {
            record.entries.insert(
                b"templates/banner.tmpl".to_vec(),
                CapturedSourceEntry::file(b"RUN".to_vec(), true),
            );
            record.entries.insert(
                b"tools/run.sh".to_vec(),
                CapturedSourceEntry::file(b"HELLO WORLD".to_vec(), false),
            );
        }
        Leg::IndexPathRenamed => rewrite_index(record, |rows| {
            for row in rows.iter_mut() {
                if row.relative_path() == b"templates/banner.tmpl" {
                    *row = CanonicalFilesystemMetadataRow::new(
                        b"templates/renamed.tmpl".to_vec(),
                        row.kind(),
                    );
                }
            }
        }),
        Leg::IndexExecutableClass => rewrite_index(record, |rows| {
            for row in rows.iter_mut() {
                if row.relative_path() == b"templates/banner.tmpl" {
                    *row = CanonicalFilesystemMetadataRow::new(
                        row.relative_path().to_vec(),
                        CanonicalFilesystemMetadataRowKind::File {
                            executable: true,
                            logical_byte_length: 11,
                        },
                    );
                }
            }
        }),
        Leg::IndexFileExtent => rewrite_index(record, |rows| {
            for row in rows.iter_mut() {
                if row.relative_path() == b"templates/banner.tmpl" {
                    *row = CanonicalFilesystemMetadataRow::new(
                        row.relative_path().to_vec(),
                        CanonicalFilesystemMetadataRowKind::File {
                            executable: false,
                            logical_byte_length: 12,
                        },
                    );
                }
            }
        }),
        Leg::IndexSymlinkExtent => rewrite_index(record, |rows| {
            for row in rows.iter_mut() {
                if row.relative_path() == b"link" {
                    *row = CanonicalFilesystemMetadataRow::new(
                        row.relative_path().to_vec(),
                        CanonicalFilesystemMetadataRowKind::Symlink {
                            target_spelling_logical_byte_length: 10,
                        },
                    );
                }
            }
        }),
        Leg::IndexKindSwapped => rewrite_index(record, |rows| {
            for row in rows.iter_mut() {
                if row.relative_path() == b"templates/banner.tmpl" {
                    *row = CanonicalFilesystemMetadataRow::new(
                        row.relative_path().to_vec(),
                        CanonicalFilesystemMetadataRowKind::Directory,
                    );
                }
            }
        }),
        // The retained entry loses its joined row, so the entry-side
        // absence check fires before the index-coverage check.
        Leg::IndexRowDropped => rewrite_index(record, |rows| {
            rows.retain(|row| row.relative_path() != b"tools/run.sh");
        }),
        Leg::IndexRowInserted => rewrite_index(record, |rows| {
            rows.push(CanonicalFilesystemMetadataRow::new(
                b"extra.txt".to_vec(),
                CanonicalFilesystemMetadataRowKind::File {
                    executable: false,
                    logical_byte_length: 1,
                },
            ));
        }),
        // The source-content commitment is opaque to this join — the record
        // accepts it — but it is the published source identity, so the
        // substituted record can never reproduce the baseline.
        Leg::IndexCommitmentForged => {
            let rows = record.canonical_source_metadata.rows().collect::<Vec<_>>();
            record.canonical_source_metadata =
                CanonicalFilesystemMetadataIndex::version_1([0xEE; 32], rows)
                    .expect("a forged commitment still encodes canonically");
        }
        Leg::FileBytesProjection => record.file_bytes += 1,
    }
}

/// The evidence a substitution that admission still accepts produces when
/// the checker replays it: on hosts that materialize every retained kind,
/// the record survives re-inspection and diverges from the retained
/// evidence; on hosts that cannot represent the record's executable class,
/// re-inspection names that boundary instead.
#[cfg(unix)]
fn divergent_verdict() -> CapturedSourceCheck {
    CapturedSourceCheck::DivergentEvidence
}

/// The non-unix direction of [`divergent_verdict`]: the baseline record
/// carries an executable file this host cannot materialize, so
/// re-inspection — not divergence — is the named boundary.
#[cfg(not(unix))]
fn divergent_verdict() -> CapturedSourceCheck {
    CapturedSourceCheck::Materialization(
        CapturedSourceMaterializationKind::UnrepresentableExecutableMode,
    )
}

/// The exact checker verdict each declared leg must produce.
pub fn captured_source_custody_outcome(
    field: CapturedSourceFieldForTest,
) -> optimization_core::MutationOutcome<CapturedSourceCheck> {
    use CapturedSourceCheck as Check;
    use CapturedSourceFieldForTest as Leg;
    let admission = |kind| Check::Admission(kind);
    let check = match field {
        Leg::BannerFileBytes | Leg::LinkTarget | Leg::LinkDigest => divergent_verdict(),
        Leg::BannerFileDigest => {
            let verdict = divergent_verdict();
            match verdict {
                // A forged digest is rejected by re-inspection wherever the
                // record can materialize at all.
                Check::DivergentEvidence => {
                    Check::Materialization(CapturedSourceMaterializationKind::ContentDrifted)
                }
                other => other,
            }
        }
        Leg::BannerPathRenamed
        | Leg::OutsideRowInserted
        | Leg::IndexPathRenamed
        | Leg::IndexRowDropped => admission(CapturedSourceAdmissionKind::AbsentFromIndex),
        Leg::LinkPathEscapesRoot
        | Leg::LinkPathEmpty
        | Leg::LinkPathNul
        | Leg::LinkPathBackslash => admission(CapturedSourceAdmissionKind::NoncanonicalPath),
        Leg::BannerKindToDirectory
        | Leg::TemplatesKindToFile
        | Leg::BannerKindToSymlink
        | Leg::LinkKindToFile
        | Leg::LinkKindToDirectory
        | Leg::BannerByteLength
        | Leg::RunnerExecutableClass
        | Leg::LinkTargetLength
        | Leg::FileValuesSwapped
        | Leg::IndexExecutableClass
        | Leg::IndexFileExtent
        | Leg::IndexSymlinkExtent
        | Leg::IndexKindSwapped => admission(CapturedSourceAdmissionKind::KindOrExtentMismatch),
        Leg::BannerRowDropped | Leg::IndexRowInserted => {
            admission(CapturedSourceAdmissionKind::OmitsFile)
        }
        Leg::ToolsRowDropped => admission(CapturedSourceAdmissionKind::OmitsDirectory),
        Leg::LinkRowDropped => admission(CapturedSourceAdmissionKind::OmitsSymlink),
        Leg::IndexCommitmentForged => divergent_verdict(),
        Leg::FileBytesProjection => Check::ProjectionDiverged,
    };
    optimization_core::MutationOutcome::ExactError(check)
}
