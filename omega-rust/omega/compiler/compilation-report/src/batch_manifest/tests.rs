use super::{
    BatchChildCommitment, BatchChildOutcome, BatchChildRow, BatchCompilationManifest,
    BatchCompilationManifestIdentity,
};
use crate::{ProductionArtifactIdentity, ProductionCompilationManifestIdentity};
use diagnostics::Diagnostic;
use target::TargetProfile;

fn rejected_row(diagnostics: &[Diagnostic]) -> BatchChildRow {
    BatchChildRow::new(
        Some(TargetProfile::LinuxX64),
        BatchChildOutcome::rejected(diagnostics).expect("nonempty diagnostics bind"),
    )
}

#[test]
fn manifest_binds_the_explicit_set_in_order() {
    let rows = vec![
        BatchChildRow::new(
            Some(TargetProfile::LinuxArm64),
            BatchChildOutcome::Succeeded {
                commitment: BatchChildCommitment::Checked,
            },
        ),
        rejected_row(&[Diagnostic::error("uefi_x86_64 unavailable")]),
        BatchChildRow::new(
            Some(TargetProfile::WindowsX64),
            BatchChildOutcome::Succeeded {
                commitment: BatchChildCommitment::Artifact(
                    ProductionArtifactIdentity::BuildOutputs([7; 32]),
                ),
            },
        ),
    ];
    let manifest = BatchCompilationManifest::new(rows.clone()).expect("three rows bind");
    assert_eq!(manifest.rows(), rows.as_slice());
    assert_eq!(manifest.rows().len(), 3);
    assert!(manifest.validate());
    let again = BatchCompilationManifest::new(rows).expect("identical rows rebind");
    assert_eq!(manifest.identity(), again.identity());
}

#[test]
fn identity_changes_when_any_row_changes() {
    let rows = vec![
        BatchChildRow::new(
            Some(TargetProfile::LinuxX64),
            BatchChildOutcome::Succeeded {
                commitment: BatchChildCommitment::Checked,
            },
        ),
        BatchChildRow::new(
            Some(TargetProfile::WindowsX64),
            BatchChildOutcome::Succeeded {
                commitment: BatchChildCommitment::Checked,
            },
        ),
    ];
    let first = BatchCompilationManifest::new(rows.clone()).unwrap();
    let mut reordered = rows;
    reordered.swap(0, 1);
    let second = BatchCompilationManifest::new(reordered).unwrap();
    assert_ne!(first.identity(), second.identity());
    assert_ne!(first.canonical_bytes(), second.canonical_bytes());
}

#[test]
fn rejected_rows_bind_their_diagnostics() {
    let baseline = rejected_row(&[Diagnostic::error("first failure")]);
    let other = rejected_row(&[
        Diagnostic::error("first failure"),
        Diagnostic::warning("second warning"),
    ]);
    assert_ne!(baseline, other);
    assert_eq!(
        BatchChildOutcome::rejected(&[]),
        Err("a rejected batch child must carry its diagnostics")
    );
}

#[test]
fn empty_set_is_not_a_batch() {
    assert_eq!(
        BatchCompilationManifest::new(Vec::new()).err(),
        Some("a batch manifest requires at least one child outcome")
    );
}

#[test]
fn validate_rejects_tampered_bytes_and_substituted_identity() {
    let rows = vec![BatchChildRow::new(
        None,
        BatchChildOutcome::Succeeded {
            commitment: BatchChildCommitment::Manifest(
                ProductionCompilationManifestIdentity::for_test([9; 32]),
            ),
        },
    )];
    let manifest = BatchCompilationManifest::new(rows.clone()).unwrap();
    assert!(manifest.validate());

    let mut wrong_bytes = manifest.canonical_bytes().to_vec();
    *wrong_bytes.last_mut().unwrap() ^= 1;
    let tampered_bytes =
        BatchCompilationManifest::for_test(rows.clone(), wrong_bytes, manifest.identity());
    assert!(!tampered_bytes.validate());

    let tampered_identity = BatchCompilationManifest::for_test(
        rows,
        manifest.canonical_bytes().to_vec(),
        BatchCompilationManifestIdentity::for_test([0; 32]),
    );
    assert!(!tampered_identity.validate());
}
