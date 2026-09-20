//! Custody-substitution machinery for the native publication receipt
//! matrices — see `custody_tests.rs` for the family semantics. Both receipts
//! exercise each representable substitution under two postures: a stale
//! containing identity (`*StaleFieldForTest` inventories) and an honestly
//! recomputed foreign record (`*RecomputedFieldForTest` inventories), each
//! consumed by the shared `run_one_field_substitution_matrix` driver.
//!
//! Non-representable lanes stay in the test itself: the
//! `boundary_contract_report_fingerprint` certificate-only reach is a
//! distinct stale posture (`CertificateLayerRejected`), and the coupled
//! multi-field lanes (`ContainerCommitmentCoupled`, `ApplicationNameCoupled`,
//! `ExecutableByteCountCoupled`) are declared variants whose substitution
//! moves the joined claims together — the record's custody axes, not single
//! struct fields, are the mutation unit.

use std::path::{Path, PathBuf};

use crate::executable_publication::{
    executable_container_digest, native_publication_certificate_digest,
    native_publication_evidence_digest,
};
use crate::package::{
    NativePackageEvidenceDigest, NativePackagePublicationReceipt, PackageComponentDigest,
    PackagePublicationComponent, package_component_digest,
};
use crate::{
    CompileOutputKind, CompileReport, ExecutableInstallationEvidenceDigest,
    ExecutablePublicationReceipt, NativePublicationCertificateDigest,
    NativePublicationEvidenceDigest, executable_installation_evidence_digest,
};
use optimization_core::MutationOutcome;

/// The producer inputs a flat publication receipt commits to. The semantic
/// and proof bytes, the target, and the image-symbol digest never appear on
/// the receipt itself — the certificate digest commits to them — so the mint
/// replay holds them fixed while the receipt's claimed fields vary.
pub struct PublicationInputs {
    pub output_path: PathBuf,
    pub native_artifact_identity: [u8; 32],
    pub semantic_bytes: Vec<u8>,
    pub proof_bytes: Vec<u8>,
    pub target: target::NativeTarget,
    pub final_image_symbol_digest: image::FinalImageSymbolDigest,
    pub boundary_contract_report_fingerprint: Option<u64>,
    pub compiler_text_validation_digest: image::CompilerTextDerivationDigest,
    pub compiler_function_validation_digest: image::CompilerFunctionValidationDigest,
    pub compiler_function_validation_report_fingerprint: u64,
    pub inventory_digest: image::PlacedExecutableRegionInventoryDigest,
    pub inventory_report_fingerprint: u64,
    pub callback_placement_identity_report_fingerprint: u64,
    pub container_bytes: Vec<u8>,
}

/// A real `CompilerFunctionValidationDigest` minted over an evidence record,
/// so a substitution carries a well-formed foreign value rather than noise.
pub fn function_validation_digest(
    final_text_validation_report_fingerprint: u64,
) -> image::CompilerFunctionValidationDigest {
    image::CompilerFunctionValidationEvidence {
        function_count: 1,
        instruction_count: 2,
        zero_width_instruction_count: 0,
        frame_prologue_byte_count: 4,
        frame_epilogue_byte_count: 8,
        fragment_manifest_report_fingerprint: 3,
        frame_application_report_fingerprint: 4,
        boundary_contract_report_fingerprint: Some(2),
        final_region_binding_report_fingerprint: 6,
        final_text_validation_report_fingerprint,
    }
    .evidence_digest()
}

/// One honest input set: the destination is the inner executable of the
/// `window-app` package tree so the same receipt binds a flat report and a
/// packaged one.
pub fn honest_inputs() -> PublicationInputs {
    let image = image::FinalImage::with_capacity(
        target::NativeTarget::linux_x64(),
        image::FinalImageMemory::default(),
        arena::Handle::invalid(),
        0,
        0,
        0,
    );
    PublicationInputs {
        output_path: "build/window-app.app/Contents/MacOS/window-app".into(),
        native_artifact_identity: [9; 32],
        semantic_bytes: b"canonical terminal semantics".to_vec(),
        proof_bytes: b"terminal proof bytes".to_vec(),
        target: target::NativeTarget::linux_x64(),
        final_image_symbol_digest: image::final_image_symbol_digest(&image),
        boundary_contract_report_fingerprint: Some(2),
        compiler_text_validation_digest: image::CompilerTextDerivationDigest::from_digest([3; 32]),
        compiler_function_validation_digest: function_validation_digest(7),
        compiler_function_validation_report_fingerprint: 4,
        inventory_digest: image::PlacedExecutableRegionInventoryDigest::from_digest([5; 32]),
        inventory_report_fingerprint: 6,
        callback_placement_identity_report_fingerprint: 8,
        container_bytes: b"the exact published executable container bytes".to_vec(),
    }
}

/// Mint the receipt exactly as `publish_retained_native_artifact` does:
/// certificate over the retained artifact's outputs, evidence over the
/// certificate and container commitment, installation over the evidence and
/// the destination.
pub fn mint(inputs: &PublicationInputs) -> ExecutablePublicationReceipt {
    let container_digest = executable_container_digest(&inputs.container_bytes);
    let certificate_digest = native_publication_certificate_digest(
        &inputs.native_artifact_identity,
        &inputs.semantic_bytes,
        &inputs.proof_bytes,
        inputs.target,
        inputs.final_image_symbol_digest,
        inputs.boundary_contract_report_fingerprint,
        inputs.compiler_text_validation_digest,
        inputs.compiler_function_validation_digest,
        inputs.compiler_function_validation_report_fingerprint,
        inputs.inventory_digest,
        inputs.inventory_report_fingerprint,
    );
    let publication_evidence_digest = native_publication_evidence_digest(
        &inputs.native_artifact_identity,
        certificate_digest,
        inputs.callback_placement_identity_report_fingerprint,
        inputs.inventory_digest,
        inputs.inventory_report_fingerprint,
        inputs.compiler_text_validation_digest,
        inputs.compiler_function_validation_digest,
        inputs.compiler_function_validation_report_fingerprint,
        inputs.container_bytes.len(),
        container_digest,
    );
    let installation_evidence_digest = executable_installation_evidence_digest(
        publication_evidence_digest,
        inputs.callback_placement_identity_report_fingerprint,
        &inputs.output_path,
        inputs.container_bytes.len(),
        container_digest,
    );
    ExecutablePublicationReceipt::new(
        inputs.output_path.clone(),
        inputs.native_artifact_identity,
        certificate_digest,
        inputs.callback_placement_identity_report_fingerprint,
        inputs.boundary_contract_report_fingerprint,
        inputs.inventory_digest,
        inputs.inventory_report_fingerprint,
        inputs.compiler_text_validation_digest,
        inputs.compiler_function_validation_digest,
        inputs.compiler_function_validation_report_fingerprint,
        publication_evidence_digest,
        inputs.container_bytes.len(),
        container_digest,
        installation_evidence_digest,
    )
}

/// Re-derive the certificate the receipt's claimed fields commit to, holding
/// the producer inputs the receipt does not carry — the artifact's semantic
/// and proof bytes, target, and image-symbol digest — at their retained
/// values. A claimed field the stored certificate was not minted over
/// disagrees here even when the downstream digests are honestly recomputed.
pub fn claimed_certificate(
    receipt: &ExecutablePublicationReceipt,
    inputs: &PublicationInputs,
) -> NativePublicationCertificateDigest {
    native_publication_certificate_digest(
        &receipt.native_artifact_identity,
        &inputs.semantic_bytes,
        &inputs.proof_bytes,
        inputs.target,
        inputs.final_image_symbol_digest,
        receipt.boundary_contract_report_fingerprint,
        receipt.compiler_text_validation_digest,
        receipt.compiler_function_validation_digest,
        receipt.compiler_function_validation_report_fingerprint,
        receipt.inventory_digest,
        receipt.inventory_report_fingerprint,
    )
}

/// Re-derive the evidence digest the receipt's claimed fields commit to.
pub fn claimed_evidence(receipt: &ExecutablePublicationReceipt) -> NativePublicationEvidenceDigest {
    native_publication_evidence_digest(
        &receipt.native_artifact_identity,
        receipt.certificate_digest,
        receipt.callback_placement_identity_report_fingerprint,
        receipt.inventory_digest,
        receipt.inventory_report_fingerprint,
        receipt.compiler_text_validation_digest,
        receipt.compiler_function_validation_digest,
        receipt.compiler_function_validation_report_fingerprint,
        receipt.container_byte_count,
        receipt.container_digest,
    )
}

/// Re-derive the installation digest the receipt's claimed fields commit to.
pub fn claimed_installation(
    receipt: &ExecutablePublicationReceipt,
) -> ExecutableInstallationEvidenceDigest {
    executable_installation_evidence_digest(
        receipt.publication_evidence_digest,
        receipt.callback_placement_identity_report_fingerprint,
        &receipt.output_path,
        receipt.container_byte_count,
        receipt.container_digest,
    )
}

/// Replay the producer mint over the receipt's claimed fields: every stored
/// digest must equal the digest the claimed inputs produce. This checks one
/// layer deeper than `has_consistent_installation_identity`, which recomputes
/// the evidence and installation digests but cannot see inside the
/// certificate.
pub fn replay_mint(receipt: &ExecutablePublicationReceipt, inputs: &PublicationInputs) -> bool {
    claimed_certificate(receipt, inputs) == receipt.certificate_digest
        && claimed_evidence(receipt) == receipt.publication_evidence_digest
        && claimed_installation(receipt) == receipt.installation_evidence_digest
}

/// Honestly recompute the containing identity after a substitution, exactly
/// as a mutated-but-self-consistent record must: certificate over the claimed
/// inputs, then evidence, then installation. Callers that substitute the
/// certificate digest itself keep their foreign claim and recompute only the
/// downstream stages.
pub fn recompute_containing_identity(
    receipt: &mut ExecutablePublicationReceipt,
    inputs: &PublicationInputs,
) {
    receipt.certificate_digest = claimed_certificate(receipt, inputs);
    receipt.publication_evidence_digest = claimed_evidence(receipt);
    receipt.installation_evidence_digest = claimed_installation(receipt);
}

pub fn report(
    wrote_output: bool,
    output_kind: CompileOutputKind,
    flat: Option<ExecutablePublicationReceipt>,
) -> CompileReport {
    CompileReport {
        build_outputs: None,
        build_observation: None,
        root_path: "Main/main.omg".into(),
        source_file_count: 1,
        wrote_output,
        output_kind,
        retained_native_artifact: None,
        artifact: None,
        executable_publication: flat,
        optimization_rollback: None,
        production_manifest: None,
        trust_admission_settlement: Default::default(),
        pcc_requests: build_evaluation::PccRequests::default(),
        terminal_admission_profile: proof_admission::AdmissionProfile::default(),
        pcc_publications: Vec::new(),
        application_name: None,
        application_identifier: None,
        application_intent: None,
        package_publication: None,
    }
}

/// A packaged publication report: the flat receipt joined to the package
/// receipt the authentic publication produced — the authentic package root,
/// name, identifier, and container commitment — plus the retained application
/// metadata.
pub fn packaged_report(
    flat: &ExecutablePublicationReceipt,
    authentic: &ExecutablePublicationReceipt,
) -> CompileReport {
    let mut report = report(
        true,
        CompileOutputKind::NativeExecutable,
        Some(flat.clone()),
    );
    report.package_publication = Some(NativePackagePublicationReceipt::new(
        PathBuf::from("build/window-app.app"),
        "window-app".to_owned(),
        build_evaluation::ApplicationIdentifier::new(b"com.omega.window-app")
            .expect("valid identifier"),
        authentic.container_digest,
        authentic.container_byte_count,
        vec![
            PackagePublicationComponent {
                relative_path: Path::new("Contents").join("Info.plist"),
                byte_count: 4,
                digest: PackageComponentDigest::from_digest([3; 32]),
            },
            PackagePublicationComponent {
                relative_path: Path::new("Contents").join("MacOS").join("window-app"),
                byte_count: authentic.container_byte_count,
                digest: PackageComponentDigest::from_digest([7; 32]),
            },
        ],
    ));
    report.application_name = Some("window-app".to_owned());
    report.application_identifier = Some(
        build_evaluation::ApplicationIdentifier::new(b"com.omega.window-app")
            .expect("valid identifier"),
    );
    report.application_intent = Some(build_evaluation::HostedApplicationIntent::Gui);
    report
}

/// The inner executable's package-relative coordinate:
/// `Contents/MacOS/<name>`.
pub fn executable_member_path(name: &str) -> PathBuf {
    Path::new("Contents").join("MacOS").join(name)
}

/// One honest `window-app` package minted exactly as
/// `publish_macos_application_package` records it: the plist member first,
/// the inner executable member bound to the flat receipt's container
/// custody, then the requested `.psi` companion — each row carrying the
/// member's package-relative path, byte count, and component digest in the
/// producer's fixed order.
pub fn mint_package(
    flat: &ExecutablePublicationReceipt,
    inputs: &PublicationInputs,
) -> NativePackagePublicationReceipt {
    let plist = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?><plist/>".as_slice();
    let psi = inputs.semantic_bytes.as_slice();
    NativePackagePublicationReceipt::new(
        PathBuf::from("build/window-app.app"),
        "window-app".to_owned(),
        build_evaluation::ApplicationIdentifier::new(b"com.omega.window-app")
            .expect("valid identifier"),
        flat.container_digest,
        flat.container_byte_count,
        vec![
            PackagePublicationComponent {
                relative_path: Path::new("Contents").join("Info.plist"),
                byte_count: plist.len(),
                digest: package_component_digest(plist),
            },
            PackagePublicationComponent {
                relative_path: executable_member_path("window-app"),
                byte_count: flat.container_byte_count,
                digest: package_component_digest(&inputs.container_bytes),
            },
            PackagePublicationComponent {
                relative_path: executable_member_path("window-app.psi"),
                byte_count: psi.len(),
                digest: package_component_digest(psi),
            },
        ],
    )
}

/// Honestly recompute the package receipt's containing identity after a
/// substitution. `NativePackagePublicationReceipt::new` is the only mint —
/// the receipt has no wire form — so rebuilding over the mutated fields is
/// the honest recomputation.
pub fn recomputed_package(
    receipt: &NativePackagePublicationReceipt,
) -> NativePackagePublicationReceipt {
    NativePackagePublicationReceipt::new(
        receipt.package_root.clone(),
        receipt.application_name.clone(),
        receipt.application_identifier.clone(),
        receipt.executable_container_digest,
        receipt.executable_byte_count,
        receipt.components.clone(),
    )
}

/// A packaged publication report over an explicit package receipt: the flat
/// receipt joined to `package`, with the retained application metadata the
/// authentic build recorded.
pub fn packaged_report_with(
    flat: &ExecutablePublicationReceipt,
    package: NativePackagePublicationReceipt,
) -> CompileReport {
    let mut report = report(
        true,
        CompileOutputKind::NativeExecutable,
        Some(flat.clone()),
    );
    report.package_publication = Some(package);
    report.application_name = Some("window-app".to_owned());
    report.application_identifier = Some(
        build_evaluation::ApplicationIdentifier::new(b"com.omega.window-app")
            .expect("valid identifier"),
    );
    report.application_intent = Some(build_evaluation::HostedApplicationIntent::Gui);
    report
}

optimization_core::custody_field_inventory! {
    /// One representable stale-custody lane of the flat
    /// `ExecutablePublicationReceipt`: every claimed field the producer mint
    /// commits to, plus the certificate-layer `boundary_contract`
    /// fingerprint and the containing digest fields themselves. The
    /// substitution leaves the stored containing digests untouched, so the
    /// independent replay is the rejection. The two `boundary_contract` lanes
    /// (drop and substitute) keep a self-consistent record — the field is
    /// outside the evidence and installation recomputations — and reject at
    /// the certificate layer instead.
    pub enum FlatPublicationStaleFieldForTest {
        OutputPath,
        NativeArtifactIdentity,
        CertificateDigest,
        CallbackPlacementFingerprint,
        BoundaryContractDropped,
        BoundaryContractSubstituted,
        InventoryDigest,
        InventoryReportFingerprint,
        TextValidationDigest,
        FunctionValidationDigest,
        FunctionValidationFingerprint,
        ContainerByteCount,
        ContainerDigest,
        PublicationEvidenceDigest,
        InstallationEvidenceDigest,
    }
}

optimization_core::custody_field_inventory! {
    /// One honestly-recomputed lane of the flat
    /// `ExecutablePublicationReceipt`: the substitution plus the containing
    /// identity recomputed exactly as the mint would, producing a
    /// self-consistent foreign record whose published installation identity
    /// diverges — the deployment-journal rejection. The
    /// `ContainerCommitmentCoupled` lane mutates the byte count and digest
    /// together over foreign container bytes, since each alone leaves the
    /// pair inconsistent. The `publication_evidence_digest` and
    /// `installation_evidence_digest` fields have no honestly-recomputed
    /// lane: recomputation is what produces them.
    pub enum FlatPublicationRecomputedFieldForTest {
        OutputPath,
        NativeArtifactIdentity,
        CallbackPlacementFingerprint,
        BoundaryContractDropped,
        BoundaryContractSubstituted,
        InventoryDigest,
        InventoryReportFingerprint,
        TextValidationDigest,
        FunctionValidationDigest,
        FunctionValidationFingerprint,
        ContainerByteCount,
        ContainerDigest,
        ContainerCommitmentCoupled,
    }
}

optimization_core::custody_field_inventory! {
    /// One representable stale-custody lane of the
    /// `NativePackagePublicationReceipt`: the package root, application name
    /// and identifier, container commitment, retained executable byte count,
    /// every member row field, the member-set count and order axes, and the
    /// containing evidence digest itself. The substitution keeps the minted
    /// `package_evidence_digest`, so the receipt's own recomputation rejects.
    pub enum NativePackageStaleFieldForTest {
        PackageRoot,
        ApplicationName,
        ApplicationIdentifier,
        ContainerCommitment,
        ExecutableByteCount,
        PlistPath,
        PlistByteCount,
        PlistDigest,
        ExecutableMemberPath,
        ExecutableMemberByteCount,
        ExecutableMemberDigest,
        CompanionPath,
        CompanionDigest,
        PlistMemberDropped,
        CompanionMemberDropped,
        ExecutableMemberDropped,
        PlistMemberDuplicated,
        ExecutableMemberDuplicated,
        MemberRosterReordered,
        ForeignMemberAppended,
        MemberRosterEmptied,
        PackageEvidenceDigest,
    }
}

optimization_core::custody_field_inventory! {
    /// One honestly-recomputed lane of the `NativePackagePublicationReceipt`:
    /// the substitution rebuilt through `NativePackagePublicationReceipt::new`
    /// — the only mint — producing either a self-consistent foreign record
    /// (join-rejected or member-adopted) or a record the executable-member
    /// join still cannot adopt. `ApplicationNameCoupled` and
    /// `ExecutableByteCountCoupled` move the field and its joined claim
    /// together. `PackageEvidenceDigest` has no recomputed lane:
    /// recomputation is what produces it.
    pub enum NativePackageRecomputedFieldForTest {
        PackageRoot,
        ApplicationName,
        ApplicationNameCoupled,
        ApplicationIdentifier,
        ContainerCommitment,
        ExecutableByteCount,
        ExecutableByteCountCoupled,
        PlistPath,
        PlistByteCount,
        PlistDigest,
        ExecutableMemberPath,
        ExecutableMemberByteCount,
        ExecutableMemberDigest,
        CompanionPath,
        CompanionDigest,
        PlistMemberDropped,
        CompanionMemberDropped,
        ExecutableMemberDropped,
        PlistMemberDuplicated,
        ExecutableMemberDuplicated,
        MemberRosterReordered,
        ForeignMemberAppended,
        MemberRosterEmptied,
    }
}

/// How a stale flat-receipt substitution rejects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlatStaleRejection {
    /// The containing digests disagree with the claimed fields: the receipt's
    /// own recomputation, the mint replay, the report custody join, and the
    /// admission-time construction all reject.
    StaleChainRejected,
    /// `boundary_contract_report_fingerprint` sits outside the evidence and
    /// installation recomputations, so the record stays self-consistent; the
    /// certificate-layer mint replay is what rejects.
    CertificateLayerRejected,
    Unclassified,
}

/// How an honestly recomputed flat-receipt substitution rejects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlatRecomputedRejection {
    /// A self-consistent foreign record whose divergent published identity a
    /// deployment journal rejects; admitted as a different publication.
    ForeignRecordAdmitted,
    Unclassified,
}

/// How a stale package-receipt substitution rejects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageStaleRejection {
    /// The receipt's own replay rejects the stale containing identity and
    /// the report-level custody join replays the receipt first.
    StaleIdentityRejected,
    Unclassified,
}

/// How an honestly recomputed package-receipt substitution rejects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageRecomputedRejection {
    /// A self-consistent foreign record whose joined claims the report-level
    /// join replays and rejects.
    JoinRejected,
    /// A self-consistent foreign record whose member internals the report
    /// join cannot see; the divergent record identity is the rejection.
    MemberInternalsAdopted,
    /// Recomputation cannot adopt the substitution: the executable-member
    /// join still rejects the rebuilt record.
    JoinUnadoptable,
    Unclassified,
}

/// A flat-receipt matrix case: the producer inputs, the claimed receipt, and
/// the authentic minted record the divergence and join checks replay against.
pub struct FlatPublicationCase {
    pub inputs: PublicationInputs,
    pub receipt: ExecutablePublicationReceipt,
    pub authentic: ExecutablePublicationReceipt,
}

/// The honestly produced flat publication: minted inputs with the receipt's
/// claimed fields exactly as the producer recorded them.
pub fn honest_flat_publication_case() -> FlatPublicationCase {
    let inputs = honest_inputs();
    let receipt = mint(&inputs);
    let authentic = receipt.clone();
    FlatPublicationCase {
        inputs,
        receipt,
        authentic,
    }
}

/// An authentic foreign flat publication: a different destination path under
/// an honestly recomputed installation identity — the divergent record the
/// foreign lanes produce.
pub fn foreign_flat_publication_donor() -> FlatPublicationCase {
    let mut case = honest_flat_publication_case();
    case.receipt.output_path = "build/foreign.app/Contents/MacOS/foreign-app".into();
    case.receipt.installation_evidence_digest = claimed_installation(&case.receipt);
    case
}

/// Mutate exactly one declared stale lane: the claimed field changes and the
/// stored containing digests stay as minted. `donor` supplies the foreign
/// values where the lane has one; fixed foreign constants pin the rest.
pub fn corrupt_flat_publication_stale_for_test(
    case: &mut FlatPublicationCase,
    field: FlatPublicationStaleFieldForTest,
    _donor: &FlatPublicationCase,
) {
    use FlatPublicationStaleFieldForTest as Field;
    let receipt = &mut case.receipt;
    match field {
        Field::OutputPath => {
            receipt.output_path = "build/window-app.app/Contents/MacOS/renamed".into();
        }
        Field::NativeArtifactIdentity => receipt.native_artifact_identity[0] ^= 0xff,
        Field::CertificateDigest => {
            receipt.certificate_digest =
                NativePublicationCertificateDigest::from_digest([0x55; 32]);
        }
        Field::CallbackPlacementFingerprint => {
            receipt.callback_placement_identity_report_fingerprint ^= 1;
        }
        Field::BoundaryContractDropped => receipt.boundary_contract_report_fingerprint = None,
        Field::BoundaryContractSubstituted => {
            receipt.boundary_contract_report_fingerprint = Some(9);
        }
        Field::InventoryDigest => {
            receipt.inventory_digest =
                image::PlacedExecutableRegionInventoryDigest::from_digest([0x55; 32]);
        }
        Field::InventoryReportFingerprint => receipt.inventory_report_fingerprint ^= 1,
        Field::TextValidationDigest => {
            receipt.compiler_text_validation_digest =
                image::CompilerTextDerivationDigest::from_digest([0x55; 32]);
        }
        Field::FunctionValidationDigest => {
            receipt.compiler_function_validation_digest = function_validation_digest(99);
        }
        Field::FunctionValidationFingerprint => {
            receipt.compiler_function_validation_report_fingerprint ^= 1;
        }
        Field::ContainerByteCount => receipt.container_byte_count += 1,
        Field::ContainerDigest => {
            receipt.container_digest = crate::ExecutableContainerDigest::from_digest([0x55; 32]);
        }
        Field::PublicationEvidenceDigest => {
            receipt.publication_evidence_digest =
                NativePublicationEvidenceDigest::from_digest([0x55; 32]);
        }
        Field::InstallationEvidenceDigest => {
            receipt.installation_evidence_digest =
                ExecutableInstallationEvidenceDigest::from_digest([0x55; 32]);
        }
    }
}

/// Mutate exactly one recomputed lane: the claimed field changes and the
/// containing identity is honestly recomputed to the depth the mint chain
/// commits it — certificate first for claimed-input fields, evidence and
/// installation for the compact coordinates and container commitment,
/// installation alone for the destination path.
pub fn corrupt_flat_publication_recomputed_for_test(
    case: &mut FlatPublicationCase,
    field: FlatPublicationRecomputedFieldForTest,
    _donor: &FlatPublicationCase,
) {
    use FlatPublicationRecomputedFieldForTest as Field;
    let receipt = &mut case.receipt;
    match field {
        Field::OutputPath => {
            receipt.output_path = "build/window-app.app/Contents/MacOS/renamed".into();
            receipt.installation_evidence_digest = claimed_installation(receipt);
        }
        Field::NativeArtifactIdentity => {
            receipt.native_artifact_identity[0] ^= 0xff;
            recompute_containing_identity(receipt, &case.inputs);
        }
        Field::CallbackPlacementFingerprint => {
            receipt.callback_placement_identity_report_fingerprint ^= 1;
            receipt.publication_evidence_digest = claimed_evidence(receipt);
            receipt.installation_evidence_digest = claimed_installation(receipt);
        }
        Field::BoundaryContractDropped => {
            receipt.boundary_contract_report_fingerprint = None;
            recompute_containing_identity(receipt, &case.inputs);
        }
        Field::BoundaryContractSubstituted => {
            receipt.boundary_contract_report_fingerprint = Some(9);
            recompute_containing_identity(receipt, &case.inputs);
        }
        Field::InventoryDigest => {
            receipt.inventory_digest =
                image::PlacedExecutableRegionInventoryDigest::from_digest([0x55; 32]);
            recompute_containing_identity(receipt, &case.inputs);
        }
        Field::InventoryReportFingerprint => {
            receipt.inventory_report_fingerprint ^= 1;
            recompute_containing_identity(receipt, &case.inputs);
        }
        Field::TextValidationDigest => {
            receipt.compiler_text_validation_digest =
                image::CompilerTextDerivationDigest::from_digest([0x55; 32]);
            recompute_containing_identity(receipt, &case.inputs);
        }
        Field::FunctionValidationDigest => {
            receipt.compiler_function_validation_digest = function_validation_digest(99);
            recompute_containing_identity(receipt, &case.inputs);
        }
        Field::FunctionValidationFingerprint => {
            receipt.compiler_function_validation_report_fingerprint ^= 1;
            recompute_containing_identity(receipt, &case.inputs);
        }
        Field::ContainerByteCount => {
            receipt.container_byte_count += 1;
            receipt.publication_evidence_digest = claimed_evidence(receipt);
            receipt.installation_evidence_digest = claimed_installation(receipt);
        }
        Field::ContainerDigest => {
            receipt.container_digest = crate::ExecutableContainerDigest::from_digest([0x55; 32]);
            receipt.publication_evidence_digest = claimed_evidence(receipt);
            receipt.installation_evidence_digest = claimed_installation(receipt);
        }
        Field::ContainerCommitmentCoupled => {
            let foreign_bytes = b"a different published container".as_slice();
            receipt.container_byte_count = foreign_bytes.len();
            receipt.container_digest = executable_container_digest(foreign_bytes);
            receipt.publication_evidence_digest = claimed_evidence(receipt);
            receipt.installation_evidence_digest = claimed_installation(receipt);
        }
    }
}

/// The stale family's independent checker: replay the receipt's own custody
/// recomputation, the producer mint over the claimed fields, the flat report
/// custody join, and the admission-time construction.
pub fn check_flat_stale_custody(
    case: &FlatPublicationCase,
) -> Result<ExecutablePublicationReceipt, FlatStaleRejection> {
    let changed = &case.receipt;
    let consistent = changed.has_consistent_installation_identity();
    let replays = replay_mint(changed, &case.inputs);
    let stale = report(
        true,
        CompileOutputKind::NativeExecutable,
        Some(changed.clone()),
    );
    let report_custody = stale.has_consistent_executable_publication_custody();
    let checked_path = stale.checked_native_executable_path();
    let admitted = CompileReport::checked(
        "Main/main.omg".into(),
        1,
        true,
        CompileOutputKind::NativeExecutable,
        Some(changed.clone()),
    )
    .is_ok();
    if !consistent && !replays && !report_custody && checked_path.is_none() && !admitted {
        Err(FlatStaleRejection::StaleChainRejected)
    } else if consistent
        && changed.installation_evidence_digest == case.authentic.installation_evidence_digest
        && !replays
        && claimed_certificate(changed, &case.inputs) != changed.certificate_digest
    {
        // Self-consistent record, unchanged record identity, but the stored
        // certificate was minted over the authentic field — the certificate
        // layer is the rejection authority.
        Err(FlatStaleRejection::CertificateLayerRejected)
    } else {
        Err(FlatStaleRejection::Unclassified)
    }
}

/// The exact stale rejection each lane must produce.
pub fn flat_stale_outcome(
    field: FlatPublicationStaleFieldForTest,
) -> MutationOutcome<FlatStaleRejection> {
    use FlatPublicationStaleFieldForTest as Field;
    let rejection = match field {
        Field::BoundaryContractDropped | Field::BoundaryContractSubstituted => {
            FlatStaleRejection::CertificateLayerRejected
        }
        _ => FlatStaleRejection::StaleChainRejected,
    };
    MutationOutcome::ExactError(rejection)
}

/// The recomputed family's independent checker: the honestly recomputed
/// record must stay self-consistent under its own replay and the mint replay,
/// diverge in the published record identity a deployment journal pins, and be
/// admitted as a different publication.
pub fn check_flat_recomputed_custody(
    case: &FlatPublicationCase,
) -> Result<ExecutablePublicationReceipt, FlatRecomputedRejection> {
    let changed = &case.receipt;
    let consistent = changed.has_consistent_installation_identity();
    let replays = replay_mint(changed, &case.inputs);
    let divergent =
        changed.installation_evidence_digest != case.authentic.installation_evidence_digest;
    let admitted = CompileReport::checked(
        "Main/main.omg".into(),
        1,
        true,
        CompileOutputKind::NativeExecutable,
        Some(changed.clone()),
    )
    .is_ok();
    if consistent && replays && divergent && admitted {
        Err(FlatRecomputedRejection::ForeignRecordAdmitted)
    } else {
        Err(FlatRecomputedRejection::Unclassified)
    }
}

/// Every recomputed lane produces a self-consistent foreign record.
pub fn flat_recomputed_outcome(
    _field: FlatPublicationRecomputedFieldForTest,
) -> MutationOutcome<FlatRecomputedRejection> {
    MutationOutcome::ExactError(FlatRecomputedRejection::ForeignRecordAdmitted)
}

/// Per-leg joined assertions the classification cannot express: the flat
/// report adopts the claimed destination verbatim, and the packaged join
/// rejects exactly the axes it covers.
pub fn flat_recomputed_joined_replay(
    case: &FlatPublicationCase,
    field: FlatPublicationRecomputedFieldForTest,
) {
    use FlatPublicationRecomputedFieldForTest as Field;
    match field {
        Field::OutputPath => {
            // A flat report cannot see the drift: the self-consistent
            // foreign receipt exposes its claimed path verbatim.
            let adopted = report(
                true,
                CompileOutputKind::NativeExecutable,
                Some(case.receipt.clone()),
            );
            assert!(adopted.has_consistent_executable_publication_custody());
            assert_eq!(
                adopted.checked_native_executable_path(),
                Some(Path::new("build/window-app.app/Contents/MacOS/renamed")),
                "{field:?}: the flat receipt's claimed destination is adopted verbatim"
            );
            assert!(
                !packaged_report(&case.receipt, &case.authentic)
                    .has_consistent_executable_publication_custody(),
                "{field:?}: the package/executable path join rejects the substitution"
            );
        }
        Field::ContainerByteCount | Field::ContainerDigest | Field::ContainerCommitmentCoupled => {
            assert!(
                !packaged_report(&case.receipt, &case.authentic)
                    .has_consistent_executable_publication_custody(),
                "{field:?}: the package container-commitment join rejects the substitution"
            );
        }
        Field::InventoryReportFingerprint => {
            assert!(
                packaged_report(&case.receipt, &case.authentic)
                    .has_consistent_executable_publication_custody(),
                "{field:?}: the package join does not cover the inventory fingerprint"
            );
        }
        _ => {}
    }
}

/// Stale-lane joined assertions: the fields whose honest-recomputation
/// posture has no lane carry their extra replay evidence here.
pub fn flat_stale_joined_replay(
    case: &FlatPublicationCase,
    field: FlatPublicationStaleFieldForTest,
) {
    use FlatPublicationStaleFieldForTest as Field;
    match field {
        Field::CertificateDigest => {
            // Honestly recomputing the downstream chain over a foreign
            // certificate keeps the receipt self-consistent, but the
            // certificate layer replays the claimed inputs and disagrees.
            let mut foreign = case.receipt.clone();
            foreign.publication_evidence_digest = claimed_evidence(&foreign);
            foreign.installation_evidence_digest = claimed_installation(&foreign);
            assert!(
                foreign.has_consistent_installation_identity(),
                "{field:?}: a foreign certificate under an honestly recomputed downstream chain stays self-consistent"
            );
            assert_ne!(
                claimed_certificate(&foreign, &case.inputs),
                foreign.certificate_digest,
                "{field:?}: the certificate layer replays the claimed inputs and disagrees"
            );
            assert!(!replay_mint(&foreign, &case.inputs));
            assert_ne!(
                foreign.installation_evidence_digest, case.authentic.installation_evidence_digest,
                "{field:?}: the published record identity a deployment journal replays diverges"
            );
        }
        Field::PublicationEvidenceDigest => {
            // A substituted evidence digest can never be honestly
            // recomputed: recomputation is what produces it. Even under a
            // recomputed downstream installation digest the claimed evidence
            // disagrees with the minted one.
            let mut changed = case.receipt.clone();
            changed.installation_evidence_digest = claimed_installation(&changed);
            assert!(
                !changed.has_consistent_installation_identity(),
                "{field:?}: a substituted evidence digest under a recomputed installation still rejects"
            );
            assert!(!replay_mint(&changed, &case.inputs));
        }
        _ => {}
    }
}

/// A package-receipt matrix case: the producer inputs, the flat receipt the
/// package joins against, the claimed package receipt, and the authentic
/// minted package record.
pub struct PackagePublicationCase {
    pub inputs: PublicationInputs,
    pub flat: ExecutablePublicationReceipt,
    pub receipt: NativePackagePublicationReceipt,
    pub authentic: NativePackagePublicationReceipt,
}

/// The honestly produced package publication: the flat mint's package
/// recorded exactly as `publish_macos_application_package` minted it.
pub fn honest_package_publication_case() -> PackagePublicationCase {
    let inputs = honest_inputs();
    let flat = mint(&inputs);
    let receipt = mint_package(&flat, &inputs);
    let authentic = receipt.clone();
    PackagePublicationCase {
        inputs,
        flat,
        receipt,
        authentic,
    }
}

/// An authentic foreign package publication: a different application name
/// under an honestly recomputed containing identity.
pub fn foreign_package_publication_donor() -> PackagePublicationCase {
    let mut case = honest_package_publication_case();
    case.receipt.application_name = "foreign-app".to_owned();
    case.receipt = recomputed_package(&case.receipt);
    case
}

/// The foreign identifier lanes mint over: a real well-formed identifier.
fn foreign_identifier() -> build_evaluation::ApplicationIdentifier {
    build_evaluation::ApplicationIdentifier::new(b"com.other.app").expect("valid identifier")
}

/// Mutate exactly one declared stale lane: the claimed field changes and the
/// stored `package_evidence_digest` stays as minted.
pub fn corrupt_package_stale_for_test(
    case: &mut PackagePublicationCase,
    field: NativePackageStaleFieldForTest,
    _donor: &PackagePublicationCase,
) {
    use NativePackageStaleFieldForTest as Field;
    let receipt = &mut case.receipt;
    match field {
        Field::PackageRoot => receipt.package_root = "build/foreign.app".into(),
        Field::ApplicationName => receipt.application_name = "other-app".to_owned(),
        Field::ApplicationIdentifier => receipt.application_identifier = foreign_identifier(),
        Field::ContainerCommitment => {
            receipt.executable_container_digest =
                crate::ExecutableContainerDigest::from_digest([0x55; 32]);
        }
        Field::ExecutableByteCount => receipt.executable_byte_count += 1,
        Field::PlistPath => {
            receipt.components[0].relative_path = Path::new("Contents").join("Info-foreign.plist");
        }
        Field::PlistByteCount => receipt.components[0].byte_count += 1,
        Field::PlistDigest => {
            receipt.components[0].digest = PackageComponentDigest::from_digest([0x44; 32]);
        }
        Field::ExecutableMemberPath => {
            receipt.components[1].relative_path = executable_member_path("renamed");
        }
        Field::ExecutableMemberByteCount => receipt.components[1].byte_count += 1,
        Field::ExecutableMemberDigest => {
            receipt.components[1].digest = PackageComponentDigest::from_digest([0x66; 32]);
        }
        Field::CompanionPath => {
            receipt.components[2].relative_path = executable_member_path("window-app.proof");
        }
        Field::CompanionDigest => {
            receipt.components[2].digest = PackageComponentDigest::from_digest([0x77; 32]);
        }
        Field::PlistMemberDropped => {
            receipt.components.remove(0);
        }
        Field::CompanionMemberDropped => {
            receipt.components.pop();
        }
        Field::ExecutableMemberDropped => {
            receipt.components.remove(1);
        }
        Field::PlistMemberDuplicated => {
            let row = receipt.components[0].clone();
            receipt.components.push(row);
        }
        Field::ExecutableMemberDuplicated => {
            let row = receipt.components[1].clone();
            receipt.components.push(row);
        }
        Field::MemberRosterReordered => receipt.components.swap(0, 1),
        Field::ForeignMemberAppended => {
            receipt.components.push(PackagePublicationComponent {
                relative_path: executable_member_path("window-app.extra"),
                byte_count: 4,
                digest: package_component_digest(b"extra"),
            });
        }
        Field::MemberRosterEmptied => receipt.components.clear(),
        Field::PackageEvidenceDigest => {
            receipt.package_evidence_digest = NativePackageEvidenceDigest::from_digest([0x55; 32]);
        }
    }
}

/// Mutate exactly one recomputed lane's claimed fields; the checker rebuilds
/// the containing identity through the mint. The `*Coupled` lanes move the
/// field and the claim that joins it together.
pub fn corrupt_package_recomputed_for_test(
    case: &mut PackagePublicationCase,
    field: NativePackageRecomputedFieldForTest,
    _donor: &PackagePublicationCase,
) {
    use NativePackageRecomputedFieldForTest as Field;
    let receipt = &mut case.receipt;
    match field {
        Field::PackageRoot => receipt.package_root = "build/foreign.app".into(),
        Field::ApplicationName => receipt.application_name = "other-app".to_owned(),
        Field::ApplicationNameCoupled => {
            receipt.application_name = "other-app".to_owned();
            receipt.components[1].relative_path = executable_member_path("other-app");
        }
        Field::ApplicationIdentifier => receipt.application_identifier = foreign_identifier(),
        Field::ContainerCommitment => {
            receipt.executable_container_digest =
                crate::ExecutableContainerDigest::from_digest([0x55; 32]);
        }
        Field::ExecutableByteCount => receipt.executable_byte_count += 1,
        Field::ExecutableByteCountCoupled => {
            receipt.executable_byte_count += 1;
            receipt.components[1].byte_count += 1;
        }
        Field::PlistPath => {
            receipt.components[0].relative_path = Path::new("Contents").join("Info-foreign.plist");
        }
        Field::PlistByteCount => receipt.components[0].byte_count += 1,
        Field::PlistDigest => {
            receipt.components[0].digest = PackageComponentDigest::from_digest([0x44; 32]);
        }
        Field::ExecutableMemberPath => {
            receipt.components[1].relative_path = executable_member_path("renamed");
        }
        Field::ExecutableMemberByteCount => receipt.components[1].byte_count += 1,
        Field::ExecutableMemberDigest => {
            receipt.components[1].digest = PackageComponentDigest::from_digest([0x66; 32]);
        }
        Field::CompanionPath => {
            receipt.components[2].relative_path = executable_member_path("window-app.proof");
        }
        Field::CompanionDigest => {
            receipt.components[2].digest = PackageComponentDigest::from_digest([0x77; 32]);
        }
        Field::PlistMemberDropped => {
            receipt.components.remove(0);
        }
        Field::CompanionMemberDropped => {
            receipt.components.pop();
        }
        Field::ExecutableMemberDropped => {
            receipt.components.remove(1);
        }
        Field::PlistMemberDuplicated => {
            let row = receipt.components[0].clone();
            receipt.components.push(row);
        }
        Field::ExecutableMemberDuplicated => {
            let row = receipt.components[1].clone();
            receipt.components.push(row);
        }
        Field::MemberRosterReordered => receipt.components.swap(0, 1),
        Field::ForeignMemberAppended => {
            receipt.components.push(PackagePublicationComponent {
                relative_path: executable_member_path("window-app.extra"),
                byte_count: 4,
                digest: package_component_digest(b"extra"),
            });
        }
        Field::MemberRosterEmptied => receipt.components.clear(),
    }
}

/// The stale family's independent checker: the receipt's own replay rejects
/// the stale containing identity, and the report-level custody join replays
/// the receipt first — no checked package or executable path escapes.
pub fn check_package_stale_custody(
    case: &PackagePublicationCase,
) -> Result<NativePackagePublicationReceipt, PackageStaleRejection> {
    let changed = &case.receipt;
    let consistent = changed.has_consistent_package_identity();
    let stale = packaged_report_with(&case.flat, changed.clone());
    let report_custody = stale.has_consistent_executable_publication_custody();
    if !consistent
        && !report_custody
        && stale.checked_native_package_path().is_none()
        && stale.checked_native_executable_path().is_none()
    {
        Err(PackageStaleRejection::StaleIdentityRejected)
    } else {
        Err(PackageStaleRejection::Unclassified)
    }
}

/// Every stale lane rejects at the receipt's own replay.
pub fn package_stale_outcome(
    _field: NativePackageStaleFieldForTest,
) -> MutationOutcome<PackageStaleRejection> {
    MutationOutcome::ExactError(PackageStaleRejection::StaleIdentityRejected)
}

/// The recomputed family's independent checker: rebuild the containing
/// identity through the mint and replay the report-level join — the join
/// either rejects the foreign record outright, adopts its member internals
/// verbatim under a divergent record identity, or cannot adopt it because the
/// rebuilt record still fails the executable-member join.
pub fn check_package_recomputed_custody(
    case: &PackagePublicationCase,
) -> Result<NativePackagePublicationReceipt, PackageRecomputedRejection> {
    let foreign = recomputed_package(&case.receipt);
    let consistent = foreign.has_consistent_package_identity();
    if !consistent {
        return Err(PackageRecomputedRejection::JoinUnadoptable);
    }
    let divergent = foreign.package_evidence_digest != case.authentic.package_evidence_digest;
    let report = packaged_report_with(&case.flat, foreign);
    let report_custody = report.has_consistent_executable_publication_custody();
    if !divergent {
        return Err(PackageRecomputedRejection::Unclassified);
    }
    if !report_custody && report.checked_native_package_path().is_none() {
        Err(PackageRecomputedRejection::JoinRejected)
    } else if report_custody
        && report.checked_native_package_path() == Some(Path::new("build/window-app.app"))
    {
        Err(PackageRecomputedRejection::MemberInternalsAdopted)
    } else {
        Err(PackageRecomputedRejection::Unclassified)
    }
}

/// The exact recomputed rejection each lane must produce.
pub fn package_recomputed_outcome(
    field: NativePackageRecomputedFieldForTest,
) -> MutationOutcome<PackageRecomputedRejection> {
    use NativePackageRecomputedFieldForTest as Field;
    let rejection = match field {
        // Joined claims: the report join replays the package root, the
        // container commitment, and the retained application metadata.
        Field::PackageRoot
        | Field::ApplicationNameCoupled
        | Field::ApplicationIdentifier
        | Field::ContainerCommitment
        | Field::ExecutableByteCountCoupled => PackageRecomputedRejection::JoinRejected,
        // Recomputation cannot adopt the drift: the executable-member join
        // still rejects the rebuilt record.
        Field::ApplicationName
        | Field::ExecutableByteCount
        | Field::ExecutableMemberPath
        | Field::ExecutableMemberByteCount
        | Field::ExecutableMemberDropped
        | Field::ExecutableMemberDuplicated
        | Field::MemberRosterEmptied => PackageRecomputedRejection::JoinUnadoptable,
        // Member internals sit outside the report join: adopted under a
        // divergent record identity.
        _ => PackageRecomputedRejection::MemberInternalsAdopted,
    };
    MutationOutcome::ExactError(rejection)
}

/// Per-leg joined assertions the classification cannot express: the coupled
/// moves where the foreign record is adopted verbatim as a different
/// publication, and the foreign-container flat/package coupling.
pub fn package_recomputed_joined_replay(
    case: &PackagePublicationCase,
    field: NativePackageRecomputedFieldForTest,
) {
    use NativePackageRecomputedFieldForTest as Field;
    match field {
        Field::PackageRoot => {
            // A fully coupled move — the flat receipt's destination follows
            // the foreign root under its own honestly recomputed
            // installation identity — is a different self-consistent
            // publication: the claimed root is adopted verbatim and the two
            // divergent record identities are the only authority over it.
            let foreign = recomputed_package(&case.receipt);
            let mut moved_flat = case.flat.clone();
            moved_flat.output_path = "build/foreign.app/Contents/MacOS/window-app".into();
            moved_flat.installation_evidence_digest = claimed_installation(&moved_flat);
            let moved = packaged_report_with(&moved_flat, foreign);
            assert!(moved.has_consistent_executable_publication_custody());
            assert_eq!(
                moved.checked_native_package_path(),
                Some(Path::new("build/foreign.app")),
                "{field:?}: the claimed package root is adopted verbatim"
            );
            assert_eq!(
                moved.checked_native_executable_path(),
                Some(Path::new("build/foreign.app/Contents/MacOS/window-app"))
            );
        }
        Field::ApplicationNameCoupled => {
            // When every joined claim moves together — member path, flat
            // destination, and retained metadata — the foreign name is
            // adopted verbatim as a different publication; the divergent
            // record identities remain the rejection.
            let foreign = recomputed_package(&case.receipt);
            let mut renamed_flat = case.flat.clone();
            renamed_flat.output_path = "build/window-app.app/Contents/MacOS/other-app".into();
            renamed_flat.installation_evidence_digest = claimed_installation(&renamed_flat);
            let mut moved = packaged_report_with(&renamed_flat, foreign);
            moved.application_name = Some("other-app".to_owned());
            assert!(moved.has_consistent_executable_publication_custody());
            assert_eq!(
                moved.checked_native_executable_path(),
                Some(Path::new("build/window-app.app/Contents/MacOS/other-app")),
                "{field:?}: the coupled foreign name is adopted verbatim"
            );
        }
        Field::ContainerCommitment => {
            // A fully coupled foreign container — flat and package receipts
            // both claiming it under honestly recomputed identities — is a
            // different self-consistent publication; the two divergent
            // record identities are the rejection.
            let foreign = recomputed_package(&case.receipt);
            let mut foreign_flat = case.flat.clone();
            foreign_flat.container_digest =
                crate::ExecutableContainerDigest::from_digest([0x55; 32]);
            foreign_flat.publication_evidence_digest = claimed_evidence(&foreign_flat);
            foreign_flat.installation_evidence_digest = claimed_installation(&foreign_flat);
            let moved = packaged_report_with(&foreign_flat, foreign);
            assert!(
                moved.has_consistent_executable_publication_custody(),
                "{field:?}: the coupled foreign container is a different self-consistent publication"
            );
        }
        _ => {}
    }
}
