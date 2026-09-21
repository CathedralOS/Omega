//! One-field mutation coverage for the native publication receipt families —
//! the flat [`ExecutablePublicationReceipt`] and the macOS package
//! [`NativePackagePublicationReceipt`] — each with its containing report
//! custody.
//!
//! The flat receipt's producer is `CompileReport::publish_retained_native_artifact`,
//! which mints the containing identity as a three-stage SHA-256 chain from the
//! retained artifact's outputs, the published container bytes, and the
//! destination path: the certificate digest commits to the artifact identity,
//! the semantic and proof bytes, the target, the image-symbol digest, the
//! boundary-contract fingerprint, and the validation and inventory digests and
//! report fingerprints; the evidence digest commits to the artifact identity,
//! certificate, callback-placement fingerprint, inventory and validation
//! digests and fingerprints, and the container commitment; the installation
//! digest commits to the evidence digest, the callback-placement fingerprint,
//! the destination path, and the container commitment.
//!
//! Three replays see different layers of that chain:
//!
//! - `has_consistent_installation_identity` recomputes the evidence and
//!   installation digests from the receipt's own fields, so a substitution
//!   that leaves them stale is rejected by the receipt's own custody, the
//!   report-custody replay, and the admission-time `CompileReport::checked`
//!   construction;
//! - `replay_mint` below re-derives the certificate layer as well — the one
//!   place `boundary_contract_report_fingerprint` reaches the chain, so the
//!   only replay that sees a stale substitution of it;
//! - a substitution whose containing identity is honestly recomputed produces
//!   a different self-consistent record: the `installation_evidence_digest` a
//!   deployment journal pins as the published record identity diverges, and
//!   the report-level joins that bind the destination path and the container
//!   commitment to the package receipt still reject the substitution.
//!
//! The receipt has no wire encoding — construction is its only admitted form —
//! so every field is representable and the coverage splits between the stale
//! and honestly recomputed legs above.
//!
//! The package receipt's producer is `publish_macos_application_package`,
//! which mints `package_evidence_digest` over the package root, the authored
//! application name and identifier, the inner executable's container
//! commitment, and the ordered member rows — each member's package-relative
//! path, byte count, and component digest, plus the member count itself. The
//! retained `executable_byte_count` never enters that digest: its replay
//! binding is the `Contents/MacOS/<name>` member's own byte count and the flat
//! receipt's container byte count at the report-level join. The same join
//! cross-binds the package root through `inner_executable_path`, the
//! container commitment, and the retained application name and identifier
//! metadata; member internals sit outside the join, so a member-level
//! substitution honestly recomputed is a different self-consistent record and
//! the divergent published identity is the rejection. Substitutions the
//! executable-member join cannot adopt — a renamed or duplicated
//! `Contents/MacOS/<name>` member, a count disagreeing with the retained byte
//! count, a member roster without exactly one inner executable — reject even
//! under an honestly recomputed containing identity.

use std::path::Path;

use super::custody_test_support::*;
use crate::CompileOutputKind;
use optimization_core::OneFieldSubstitutionMatrix;

#[test]
fn executable_publication_receipt_rejects_every_one_field_substitution() {
    let case = honest_flat_publication_case();
    let inputs = &case.inputs;
    let authentic = &case.authentic;
    assert!(
        authentic.has_consistent_installation_identity(),
        "the producer mint is self-consistent"
    );
    assert!(
        replay_mint(authentic, inputs),
        "the producer mint replays over the receipt's claimed fields"
    );
    let flat_report = report(
        true,
        CompileOutputKind::NativeExecutable,
        Some(authentic.clone()),
    );
    assert!(flat_report.has_consistent_executable_publication_custody());
    assert_eq!(
        flat_report.checked_native_executable_path(),
        Some(inputs.output_path.as_path())
    );
    let packaged = packaged_report(authentic, authentic);
    assert!(packaged.has_consistent_executable_publication_custody());
    assert_eq!(
        packaged.checked_native_package_path(),
        Some(Path::new("build/window-app.app"))
    );

    // A substitution that leaves the containing digests stale is rejected by
    // the receipt's own recomputation, the producer mint replay, the report
    // custody join, and the admission-time construction — except the
    // certificate-layer `boundary_contract` lanes, which stay self-consistent
    // and reject at the certificate replay instead.
    optimization_core::run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "flat executable publication receipt under a stale containing identity",
        fields: FlatPublicationStaleFieldForTest::INVENTORY,
        honest: &honest_flat_publication_case,
        donor: foreign_flat_publication_donor(),
        custody: &|case: &FlatPublicationCase| case.receipt.clone(),
        substitute: &corrupt_flat_publication_stale_for_test,
        check: &check_flat_stale_custody,
        outcome: &flat_stale_outcome,
        joined_replay: Some(&flat_stale_joined_replay),
    });

    // A substitution whose containing identity is honestly recomputed is a
    // different self-consistent record — representable and admitted — but its
    // installation identity diverges from the published record a deployment
    // journal pins, so the journal replay still rejects it. Per-leg joined
    // assertions cover the packaged-custody axes the flat report cannot see.
    optimization_core::run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "flat executable publication receipt under an honestly recomputed identity",
        fields: FlatPublicationRecomputedFieldForTest::INVENTORY,
        honest: &honest_flat_publication_case,
        donor: foreign_flat_publication_donor(),
        custody: &|case: &FlatPublicationCase| case.receipt.clone(),
        substitute: &corrupt_flat_publication_recomputed_for_test,
        check: &check_flat_recomputed_custody,
        outcome: &flat_recomputed_outcome,
        joined_replay: Some(&flat_recomputed_joined_replay),
    });
}

#[test]
fn native_package_publication_receipt_rejects_every_one_field_substitution() {
    let case = honest_package_publication_case();
    let inputs = &case.inputs;
    let flat = &case.flat;
    let authentic = &case.authentic;
    assert!(
        authentic.has_consistent_package_identity(),
        "the producer mint is self-consistent"
    );
    assert_eq!(
        authentic.inner_executable_path().as_deref(),
        Some(inputs.output_path.as_path()),
        "the package receipt names the flat receipt's destination"
    );
    let published = packaged_report_with(flat, authentic.clone());
    assert!(published.has_consistent_executable_publication_custody());
    assert_eq!(
        published.checked_native_package_path(),
        Some(Path::new("build/window-app.app"))
    );
    assert_eq!(
        published.checked_native_executable_path(),
        Some(inputs.output_path.as_path())
    );

    // A substitution that keeps the minted evidence digest is rejected by
    // the receipt's own recomputation — the digest mismatch, or the
    // executable-member join for a field the digest does not commit — and by
    // the report-level custody join that replays the receipt first.
    optimization_core::run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "native package publication receipt under a stale containing identity",
        fields: NativePackageStaleFieldForTest::INVENTORY,
        honest: &honest_package_publication_case,
        donor: foreign_package_publication_donor(),
        custody: &|case: &PackagePublicationCase| case.receipt.clone(),
        substitute: &corrupt_package_stale_for_test,
        check: &check_package_stale_custody,
        outcome: &package_stale_outcome,
        joined_replay: None,
    });

    // A substitution whose containing identity is honestly recomputed is a
    // different self-consistent receipt: the package evidence digest a
    // deployment journal pins as the published record identity diverges. The
    // report-level join sees the package root, the container commitment and
    // byte count, and the retained application name and identifier — a
    // foreign receipt claiming a different value on any of those axes is
    // rejected outright; member internals the join cannot see are adopted as
    // a different self-consistent publication under the divergent record
    // identity; and substitutions the executable-member join cannot adopt
    // reject even under honest recomputation.
    optimization_core::run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "native package publication receipt under an honestly recomputed identity",
        fields: NativePackageRecomputedFieldForTest::INVENTORY,
        honest: &honest_package_publication_case,
        donor: foreign_package_publication_donor(),
        custody: &|case: &PackagePublicationCase| case.receipt.clone(),
        substitute: &corrupt_package_recomputed_for_test,
        check: &check_package_recomputed_custody,
        outcome: &package_recomputed_outcome,
        joined_replay: Some(&package_recomputed_joined_replay),
    });
}
