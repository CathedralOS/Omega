use super::OwnedImageProvider;
use crate::executable_installation::test_support::{
    admit, artifact, artifact_placement_constraints, authority_commitments, certificate, entry_id,
    frozen, id, installed_code, relocatable_artifact,
};
use crate::executable_installation::{
    AdmittedArtifact, Artifact, ArtifactEntry, ArtifactId, ArtifactRelocationKind,
    DecodedArtifactRelocation, EntryContractDigest, EntryReferenceAuthority,
    EntryReferenceFactDigest, EntryReferenceReceipt, EntrySetId, InstallAuthority,
    InstallationFactDigest, InstalledCode, InstalledCodeId, InstalledEntryReference,
    MachineContractSetId, MachineFootprintId, MappingQuarantineCause, PlacementPlanId,
    QuarantinedInstallation, RelocationSetId, ReplacementAuthority, ReplacementFactDigest,
    ReplacementOutcome, RetiredInstallation, RetirementAuthority, RetirementFactDigest,
    RetirementReceipt, UninstallOutcome, ValidatedPlacement, install_validated,
    quarantine_installed, replace_installed, retire_installed, uninstall_installed,
    validate_final_placement,
};
use layout_plans::{EntryStubId, RelocationTarget};
use target::Architecture;

/// A two-site artifact: entries at byte offsets 8 and 32 over a 64-byte image,
/// so each declared site has a real bounded extent (24 and 32 bytes) that a
/// patch fragment must fit.
fn two_site_artifact(identity: u64) -> Artifact {
    let constraints = artifact_placement_constraints();
    Artifact::from_canonical_decode(
        id(identity, ArtifactId::from_normalized_identity),
        Architecture::X86_64,
        vec![0x41; 64],
        id(30, MachineContractSetId::from_normalized_identity),
        id(31, MachineFootprintId::from_normalized_identity),
        id(32, PlacementPlanId::from_normalized_identity),
        constraints,
        id(33, EntrySetId::from_normalized_identity),
        vec![
            ArtifactEntry::from_canonical_decode(entry_id(identity + 1000), 8),
            ArtifactEntry::from_canonical_decode(entry_id(identity + 2000), 32),
        ],
        id(34, RelocationSetId::from_normalized_identity),
        Vec::new(),
        authority_commitments(constraints),
    )
    .expect("two-site artifact")
}

fn validated(admitted: &AdmittedArtifact, placement: u64, base: u64) -> ValidatedPlacement {
    let frozen = frozen(admitted, placement, base);
    let certificate = certificate(&frozen, 500 + placement);
    validate_final_placement(frozen, &certificate).expect("validated placement")
}

/// Drive the full contracted join through the provider: the receiver demands
/// the provider's published fact set, the provider performs the operation, and
/// `install_validated` consumes the resulting receipt. Returns the installed
/// custody plus the provider-issued identity under which the image resides.
fn install_through_provider(
    provider: &mut OwnedImageProvider,
    artifact: &Artifact,
    placement: u64,
    base: u64,
) -> (InstalledCodeId, InstalledCode) {
    let admitted = admit(artifact);
    let validated = validated(&admitted, placement, base);
    let authority = InstallAuthority::from_admitted_provider(&validated)
        .with_required_facts(OwnedImageProvider::install_facts());
    let (installed, receipt) = provider
        .install(&validated, &authority)
        .expect("provider performs the install");
    let installed_code =
        install_validated(validated, authority, receipt).expect("the provider's receipt installs");
    (installed, installed_code)
}

#[test]
fn install_stores_the_validated_bytes_and_the_receipt_installs() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let artifact = artifact(41);
    let admitted = admit(&artifact);
    let validated = validated(&admitted, 41, 0x4000);
    let authority = InstallAuthority::from_admitted_provider(&validated)
        .with_required_facts(OwnedImageProvider::install_facts());

    let (installed, receipt) = provider
        .install(&validated, &authority)
        .expect("provider performs the install");

    assert_eq!(
        provider.installed_image(installed),
        Some(validated.frozen.materialized.bytes())
    );
    assert_eq!(provider.write_suspended(installed), Some(true));
    install_validated(validated, authority, receipt)
        .expect("the provider's receipt passes the install gate");
}

#[test]
fn install_refuses_a_demand_for_facts_it_did_not_perform() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let admitted = admit(&artifact(41));
    let validated = validated(&admitted, 41, 0x4000);
    let authority = InstallAuthority::from_admitted_provider(&validated).with_required_facts([
        InstallationFactDigest::from_canonical_bytes(b"omega.other-provider.unperformed-step.v1"),
    ]);

    let error = provider
        .install(&validated, &authority)
        .expect_err("a demand beyond the performed set refuses");

    assert!(error.0.contains("cannot establish"));
    assert_eq!(
        provider.installed_image(InstalledCodeId::from_normalized_identity(1).unwrap()),
        None
    );
}

#[test]
fn install_refuses_an_artifact_for_another_architecture() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let foreign = relocatable_artifact(55, Architecture::Aarch64, vec![0; 8], Vec::new());
    let admitted = admit(&foreign);
    let validated = validated(&admitted, 55, 0x4000);
    let authority = InstallAuthority::from_admitted_provider(&validated);

    let error = provider
        .install(&validated, &authority)
        .expect_err("a foreign-architecture artifact refuses");

    assert!(error.0.contains("cannot install"));
}

#[test]
fn patch_splices_admitted_fragments_at_declared_sites_and_the_receipt_replaces() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (superseded_id, superseded) =
        install_through_provider(&mut provider, &two_site_artifact(41), 41, 0x4000);
    let (_successor_id, successor) =
        install_through_provider(&mut provider, &artifact(42), 42, 0x8000);

    let site_a = entry_id(1041);
    let site_b = entry_id(2041);
    let fragment_a = admit(&relocatable_artifact(
        71,
        Architecture::X86_64,
        vec![0xCC; 6],
        Vec::new(),
    ));
    let fragment_b = admit(&relocatable_artifact(
        72,
        Architecture::X86_64,
        vec![0x90; 4],
        Vec::new(),
    ));
    let authority = ReplacementAuthority::from_admitted_provider(
        &superseded,
        &successor,
        [(site_a, fragment_a.clone()), (site_b, fragment_b.clone())],
        OwnedImageProvider::patch_facts(),
    );
    let receipt = provider
        .patch(&superseded, &successor, &authority)
        .expect("provider patches the demanded sites");

    let image = provider
        .installed_image(superseded_id)
        .expect("resident image");
    assert_eq!(&image[8..14], &[0xCC; 6]);
    assert_eq!(&image[32..36], &[0x90; 4]);
    assert_eq!(&image[14..32], &[0x41; 18]);
    assert_eq!(provider.write_suspended(superseded_id), Some(true));

    let retirement_authority = RetirementAuthority::from_admitted_provider(
        &superseded,
        OwnedImageProvider::retire_facts(),
    );
    let retirement = provider
        .retire(&superseded, &retirement_authority)
        .expect("provider performs the retirement the drain consumes");
    let outcome = replace_installed(
        superseded,
        &successor,
        authority,
        receipt,
        retirement_authority,
        retirement,
        None,
    )
    .expect("the provider's patch receipt reaches the drain");
    assert!(
        matches!(outcome, ReplacementOutcome::Retired { .. }),
        "a complete drain retires rather than quarantining"
    );
    assert!(provider.release(superseded_id).unwrap());
    assert_eq!(provider.installed_image(superseded_id), None);
}

#[test]
fn patch_refuses_a_site_that_is_not_a_declared_entry() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (superseded_id, superseded) =
        install_through_provider(&mut provider, &two_site_artifact(41), 41, 0x4000);
    let (_successor_id, successor) =
        install_through_provider(&mut provider, &artifact(42), 42, 0x8000);
    let fragment = admit(&relocatable_artifact(
        71,
        Architecture::X86_64,
        vec![0xCC; 4],
        Vec::new(),
    ));
    let authority = ReplacementAuthority::from_admitted_provider(
        &superseded,
        &successor,
        [(entry_id(9999), fragment)],
        OwnedImageProvider::patch_facts(),
    );

    let error = provider
        .patch(&superseded, &successor, &authority)
        .expect_err("an undeclared site refuses");

    assert!(error.0.contains("not a declared entry"));
    assert_eq!(
        provider.installed_image(superseded_id),
        Some(&[0x41; 64][..])
    );
}

#[test]
fn patch_refuses_a_fragment_carrying_relocations() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (superseded_id, superseded) =
        install_through_provider(&mut provider, &two_site_artifact(41), 41, 0x4000);
    let (_successor_id, successor) =
        install_through_provider(&mut provider, &artifact(42), 42, 0x8000);
    let fragment = admit(&relocatable_artifact(
        71,
        Architecture::X86_64,
        vec![0xCC; 8],
        vec![DecodedArtifactRelocation {
            kind: ArtifactRelocationKind::Absolute64,
            destination_offset: 0,
            target: RelocationTarget::Entry(entry_id(1071)),
            addend: 0,
        }],
    ));
    let authority = ReplacementAuthority::from_admitted_provider(
        &superseded,
        &successor,
        [(entry_id(1041), fragment)],
        OwnedImageProvider::patch_facts(),
    );

    let error = provider
        .patch(&superseded, &successor, &authority)
        .expect_err("a relocated fragment refuses");

    assert!(error.0.contains("position-independent"));
    assert_eq!(
        provider.installed_image(superseded_id),
        Some(&[0x41; 64][..])
    );
}

#[test]
fn patch_refuses_a_fragment_that_overruns_the_site_extent() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (superseded_id, superseded) =
        install_through_provider(&mut provider, &two_site_artifact(41), 41, 0x4000);
    let (_successor_id, successor) =
        install_through_provider(&mut provider, &artifact(42), 42, 0x8000);
    // Site b at offset 32 has a 32-byte extent to the image end; a 64-byte
    // fragment cannot fit it.
    let fragment = admit(&relocatable_artifact(
        71,
        Architecture::X86_64,
        vec![0x90; 64],
        Vec::new(),
    ));
    let authority = ReplacementAuthority::from_admitted_provider(
        &superseded,
        &successor,
        [(entry_id(2041), fragment)],
        OwnedImageProvider::patch_facts(),
    );

    let error = provider
        .patch(&superseded, &successor, &authority)
        .expect_err("an oversized fragment refuses");

    assert!(error.0.contains("does not fit"));
    assert_eq!(
        provider.installed_image(superseded_id),
        Some(&[0x41; 64][..])
    );
}

#[test]
fn patch_refuses_an_authority_not_scoped_to_the_handed_realizations() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (_superseded_id, superseded) =
        install_through_provider(&mut provider, &two_site_artifact(41), 41, 0x4000);
    let (_successor_id, successor) =
        install_through_provider(&mut provider, &artifact(42), 42, 0x8000);
    let (_other_id, other_successor) =
        install_through_provider(&mut provider, &artifact(43), 43, 0xC000);
    let fragment = admit(&relocatable_artifact(
        71,
        Architecture::X86_64,
        vec![0xCC; 4],
        Vec::new(),
    ));
    let authority = ReplacementAuthority::from_admitted_provider(
        &superseded,
        &other_successor,
        [(entry_id(1041), fragment)],
        OwnedImageProvider::patch_facts(),
    );

    let error = provider
        .patch(&superseded, &successor, &authority)
        .expect_err("an authority naming another successor refuses");

    assert!(error.0.contains("not scoped"));
}

#[test]
fn patch_refuses_to_replace_a_realization_with_itself() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (_superseded_id, superseded) =
        install_through_provider(&mut provider, &two_site_artifact(41), 41, 0x4000);
    let fragment = admit(&relocatable_artifact(
        71,
        Architecture::X86_64,
        vec![0xCC; 4],
        Vec::new(),
    ));
    let authority = ReplacementAuthority::from_admitted_provider(
        &superseded,
        &superseded,
        [(entry_id(1041), fragment)],
        OwnedImageProvider::patch_facts(),
    );

    let error = provider
        .patch(&superseded, &superseded, &authority)
        .expect_err("a realization cannot replace itself");

    assert!(error.0.contains("cannot replace itself"));
}

#[test]
fn patch_refuses_a_demand_for_facts_it_did_not_perform() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (_superseded_id, superseded) =
        install_through_provider(&mut provider, &two_site_artifact(41), 41, 0x4000);
    let (_successor_id, successor) =
        install_through_provider(&mut provider, &artifact(42), 42, 0x8000);
    let fragment = admit(&relocatable_artifact(
        71,
        Architecture::X86_64,
        vec![0xCC; 4],
        Vec::new(),
    ));
    let authority = ReplacementAuthority::from_admitted_provider(
        &superseded,
        &successor,
        [(entry_id(1041), fragment)],
        [ReplacementFactDigest::from_canonical_bytes(
            b"omega.other-provider.unperformed-step.v1",
        )],
    );

    let error = provider
        .patch(&superseded, &successor, &authority)
        .expect_err("a demand beyond the performed set refuses");

    assert!(error.0.contains("cannot establish"));
}

#[test]
fn patch_refuses_when_the_provider_holds_no_resident_successor() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (_superseded_id, superseded) =
        install_through_provider(&mut provider, &two_site_artifact(41), 41, 0x4000);
    // A realization installed by a different provider (here: the test support
    // path, not this provider) is not resident in this provider's custody.
    let foreign_admitted = admit(&artifact(42));
    let foreign_successor = installed_code(&foreign_admitted, 42, 0x8000);
    let fragment = admit(&relocatable_artifact(
        71,
        Architecture::X86_64,
        vec![0xCC; 4],
        Vec::new(),
    ));
    let authority = ReplacementAuthority::from_admitted_provider(
        &superseded,
        &foreign_successor,
        [(entry_id(1041), fragment)],
        OwnedImageProvider::patch_facts(),
    );

    let error = provider
        .patch(&superseded, &foreign_successor, &authority)
        .expect_err("a nonresident successor refuses");

    assert!(error.0.contains("no resident image"));
}

#[test]
fn patch_refuses_a_superseded_whose_execute_authority_was_removed() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (_superseded_id, superseded) =
        install_through_provider(&mut provider, &two_site_artifact(41), 41, 0x4000);
    let (_successor_id, successor) =
        install_through_provider(&mut provider, &artifact(42), 42, 0x8000);
    let retirement_authority = RetirementAuthority::from_admitted_provider(
        &superseded,
        OwnedImageProvider::retire_facts(),
    );
    provider
        .retire(&superseded, &retirement_authority)
        .expect("provider performs the retirement");

    // Live-site patching has no live site left here: the drained image must
    // not have its restored write authority re-suspended under dead code.
    let fragment = admit(&relocatable_artifact(
        71,
        Architecture::X86_64,
        vec![0xCC; 4],
        Vec::new(),
    ));
    let authority = ReplacementAuthority::from_admitted_provider(
        &superseded,
        &successor,
        [(entry_id(1041), fragment)],
        OwnedImageProvider::patch_facts(),
    );
    let error = provider
        .patch(&superseded, &successor, &authority)
        .expect_err("patching a drained realization refuses");
    assert!(error.0.contains("execute authority"));
    assert_eq!(provider.write_suspended(_superseded_id), Some(false));
}

#[test]
fn patch_refuses_a_successor_whose_execute_authority_was_removed() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (_superseded_id, superseded) =
        install_through_provider(&mut provider, &two_site_artifact(41), 41, 0x4000);
    let (_successor_id, successor) =
        install_through_provider(&mut provider, &artifact(42), 42, 0x8000);
    let retirement_authority =
        RetirementAuthority::from_admitted_provider(&successor, OwnedImageProvider::retire_facts());
    provider
        .retire(&successor, &retirement_authority)
        .expect("provider performs the retirement");

    // The successor is resident but drained: patched sites would route calls
    // into a mapping no executor can enter.
    let fragment = admit(&relocatable_artifact(
        71,
        Architecture::X86_64,
        vec![0xCC; 4],
        Vec::new(),
    ));
    let authority = ReplacementAuthority::from_admitted_provider(
        &superseded,
        &successor,
        [(entry_id(1041), fragment)],
        OwnedImageProvider::patch_facts(),
    );
    let error = provider
        .patch(&superseded, &successor, &authority)
        .expect_err("routing to a drained successor refuses");
    assert!(error.0.contains("successor whose execute authority"));
}

/// The contract identity every seal test demands; open vocabulary, so any
/// canonical bytes name a contract.
const SEAL_CONTRACT_BYTES: &[u8] = b"omega.test.entry-contract.sealed-call.v1";

fn seal_contract() -> EntryContractDigest {
    EntryContractDigest::from_canonical_bytes(SEAL_CONTRACT_BYTES)
}

/// Drive the contracted entry-sealing join through the provider: the
/// receiver demands the provider's published fact set, the provider performs
/// the operation, and `InstalledCode::seal_entry_reference` consumes the
/// resulting receipt.
fn seal_through_provider<'installed>(
    provider: &OwnedImageProvider,
    installed: &'installed InstalledCode,
    entry: EntryStubId,
) -> InstalledEntryReference<'installed> {
    let authority =
        EntryReferenceAuthority::from_admitted_provider(installed, entry, seal_contract())
            .with_required_facts(OwnedImageProvider::seal_facts());
    let receipt = provider
        .seal_entry(installed, &authority)
        .expect("provider performs the entry-sealing operation");
    installed
        .seal_entry_reference(authority, receipt)
        .expect("the provider's receipt seals")
}

/// Patch one declared site of a provider-resident realization with a
/// position-independent fragment.
fn patch_through_provider(
    provider: &mut OwnedImageProvider,
    superseded: &InstalledCode,
    successor: &InstalledCode,
    site: EntryStubId,
    fragment_bytes: Vec<u8>,
) {
    let fragment = admit(&relocatable_artifact(
        71,
        Architecture::X86_64,
        fragment_bytes,
        Vec::new(),
    ));
    let authority = ReplacementAuthority::from_admitted_provider(
        superseded,
        successor,
        [(site, fragment)],
        OwnedImageProvider::patch_facts(),
    );
    provider
        .patch(superseded, successor, &authority)
        .expect("provider patches the demanded site");
}

#[test]
fn seal_entry_mints_a_receipt_the_gate_seals() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (installed_id, installed) =
        install_through_provider(&mut provider, &two_site_artifact(41), 41, 0x4000);

    let reference = seal_through_provider(&provider, &installed, entry_id(1041));

    assert_eq!(reference.entry(), entry_id(1041));
    assert_eq!(reference.contract(), seal_contract());
    assert_eq!(reference.installed_code(), installed_id);
}

#[test]
fn seal_entry_replays_the_committed_fragment_content_after_patch() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (_superseded_id, superseded) =
        install_through_provider(&mut provider, &two_site_artifact(41), 41, 0x4000);
    let (_successor_id, successor) =
        install_through_provider(&mut provider, &artifact(42), 42, 0x8000);
    patch_through_provider(
        &mut provider,
        &superseded,
        &successor,
        entry_id(1041),
        vec![0xCC; 6],
    );

    // The resident extent no longer equals the bytes the artifact was
    // installed with — the seal replays the committed patch content.
    let reference = seal_through_provider(&provider, &superseded, entry_id(1041));

    assert_eq!(reference.entry(), entry_id(1041));
}

#[test]
fn seal_entry_refuses_a_demand_for_facts_it_did_not_perform() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (_id, installed) =
        install_through_provider(&mut provider, &two_site_artifact(41), 41, 0x4000);
    let authority = EntryReferenceAuthority::from_admitted_provider(
        &installed,
        entry_id(1041),
        seal_contract(),
    )
    .with_required_facts([EntryReferenceFactDigest::from_canonical_bytes(
        b"omega.other-provider.unperformed-step.v1",
    )]);

    let error = provider
        .seal_entry(&installed, &authority)
        .expect_err("a demand beyond the performed set refuses");

    assert!(error.0.contains("cannot establish"));
}

#[test]
fn seal_entry_refuses_an_authority_scoped_to_another_realization() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (_id, installed) =
        install_through_provider(&mut provider, &two_site_artifact(41), 41, 0x4000);
    let (_other_id, other) =
        install_through_provider(&mut provider, &two_site_artifact(42), 42, 0x8000);
    let authority =
        EntryReferenceAuthority::from_admitted_provider(&other, entry_id(1042), seal_contract())
            .with_required_facts(OwnedImageProvider::seal_facts());

    let error = provider
        .seal_entry(&installed, &authority)
        .expect_err("an authority scoped to another realization refuses");

    assert!(error.0.contains("not scoped"));
}

#[test]
fn seal_entry_refuses_when_the_provider_holds_no_resident_image() {
    let provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    // A realization installed by a different provider (here: the test support
    // path, not this provider) is not resident in this provider's custody.
    let admitted = admit(&two_site_artifact(41));
    let installed = installed_code(&admitted, 41, 0x4000);
    let authority = EntryReferenceAuthority::from_admitted_provider(
        &installed,
        entry_id(1041),
        seal_contract(),
    )
    .with_required_facts(OwnedImageProvider::seal_facts());

    let error = provider
        .seal_entry(&installed, &authority)
        .expect_err("a nonresident realization refuses");

    assert!(error.0.contains("no resident image"));
}

#[test]
fn seal_entry_refuses_an_undeclared_entry() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (_id, installed) =
        install_through_provider(&mut provider, &two_site_artifact(41), 41, 0x4000);
    let authority = EntryReferenceAuthority::from_admitted_provider(
        &installed,
        entry_id(9999),
        seal_contract(),
    )
    .with_required_facts(OwnedImageProvider::seal_facts());

    let error = provider
        .seal_entry(&installed, &authority)
        .expect_err("an undeclared entry refuses");

    assert!(error.0.contains("not a declared entry"));
}

#[test]
fn call_hands_the_resident_entry_extent_to_the_sealed_call() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (installed_id, installed) =
        install_through_provider(&mut provider, &two_site_artifact(41), 41, 0x4000);
    let reference = seal_through_provider(&provider, &installed, entry_id(1041));

    let call = provider
        .call(&installed, &reference)
        .expect("the sealed reference invokes");

    assert_eq!(call.installed_code(), installed_id);
    assert_eq!(call.entry(), entry_id(1041));
    assert_eq!(call.contract(), seal_contract());
    // Entry 1041's extent spans its offset 8 up to the next declared entry
    // at 32: 24 bytes of the resident image.
    assert_eq!(call.code(), &[0x41; 24]);
}

#[test]
fn call_enters_the_committed_fragment_content_after_patch() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (_superseded_id, superseded) =
        install_through_provider(&mut provider, &two_site_artifact(41), 41, 0x4000);
    let (_successor_id, successor) =
        install_through_provider(&mut provider, &artifact(42), 42, 0x8000);
    patch_through_provider(
        &mut provider,
        &superseded,
        &successor,
        entry_id(1041),
        vec![0xCC; 6],
    );
    let reference = seal_through_provider(&provider, &superseded, entry_id(1041));

    let call = provider
        .call(&superseded, &reference)
        .expect("the sealed reference invokes the patched entry");

    // The patched extent replays the committed fragment head plus the
    // retained tail, not the superseded pre-patch bytes.
    let mut expected = vec![0xCC; 6];
    expected.extend_from_slice(&[0x41; 18]);
    assert_eq!(call.code(), expected.as_slice());
}

#[test]
fn call_refuses_a_reference_sealing_another_occurrence() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (_first_id, first) =
        install_through_provider(&mut provider, &two_site_artifact(41), 41, 0x4000);
    let (_second_id, second) =
        install_through_provider(&mut provider, &two_site_artifact(42), 42, 0x8000);
    let reference = seal_through_provider(&provider, &first, entry_id(1041));

    let error = provider
        .call(&second, &reference)
        .expect_err("a reference sealing another occurrence refuses");

    assert!(error.0.contains("does not seal"));
}

#[test]
fn call_refuses_when_the_provider_holds_no_resident_image() {
    let provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let admitted = admit(&two_site_artifact(41));
    let installed = installed_code(&admitted, 41, 0x4000);
    // A sealed reference can come from any provider's operation; the test
    // support path mints the receipt directly.
    let authority = EntryReferenceAuthority::from_admitted_provider(
        &installed,
        entry_id(1041),
        seal_contract(),
    );
    let receipt = EntryReferenceReceipt::from_provider(
        &installed,
        entry_id(1041),
        seal_contract(),
        true,
        true,
    );
    let reference = installed
        .seal_entry_reference(authority, receipt)
        .expect("the hand-minted receipt seals");

    let error = provider
        .call(&installed, &reference)
        .expect_err("a nonresident realization refuses");

    assert!(error.0.contains("no resident image"));
}

/// Drive the contracted retirement join through the provider: the receiver
/// demands the provider's published fact set, the provider performs the
/// operation, and `retire_installed` consumes the resulting receipt.
fn retire_through_provider(
    provider: &mut OwnedImageProvider,
    installed: InstalledCode,
) -> RetiredInstallation {
    let authority =
        RetirementAuthority::from_admitted_provider(&installed, OwnedImageProvider::retire_facts());
    let receipt = provider
        .retire(&installed, &authority)
        .expect("provider performs the retirement");
    retire_installed(installed, authority, receipt).expect("the provider's receipt retires")
}

#[test]
fn retire_unwinds_the_write_to_execute_transition_and_the_receipt_retires() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (installed_id, installed) =
        install_through_provider(&mut provider, &two_site_artifact(41), 41, 0x4000);
    let reference = seal_through_provider(&provider, &installed, entry_id(1041));
    let call = provider
        .call(&installed, &reference)
        .expect("the sealed reference invokes before retirement");
    assert_eq!(call.code(), &[0x41; 24]);
    // The in-flight call borrows the provider and `reference` borrows
    // `installed`; neither is used again, so the exclusive retirement
    // receiver and the move below are reachable — that reachability is the
    // executor-quiescence evidence the provider reports.

    let authority =
        RetirementAuthority::from_admitted_provider(&installed, OwnedImageProvider::retire_facts());
    let receipt = provider
        .retire(&installed, &authority)
        .expect("provider performs the retirement");

    // Execute authority removed, write authority restored: the W+NX state
    // the write-to-execute transition entered is unwound, in reverse.
    assert_eq!(provider.execute_enabled(installed_id), Some(false));
    assert_eq!(provider.write_suspended(installed_id), Some(false));
    let seal_authority = EntryReferenceAuthority::from_admitted_provider(
        &installed,
        entry_id(1041),
        seal_contract(),
    );
    let error = provider
        .seal_entry(&installed, &seal_authority)
        .expect_err("sealing an entry after retirement refuses");
    assert!(error.0.contains("execute authority"));

    retire_installed(installed, authority, receipt)
        .expect("the provider's receipt passes the retirement gate");
}

#[test]
fn call_refuses_after_the_realization_is_retired() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (_installed_id, installed) =
        install_through_provider(&mut provider, &two_site_artifact(41), 41, 0x4000);
    let reference = seal_through_provider(&provider, &installed, entry_id(1041));
    let authority =
        RetirementAuthority::from_admitted_provider(&installed, OwnedImageProvider::retire_facts());
    provider
        .retire(&installed, &authority)
        .expect("provider performs the retirement");

    let error = provider
        .call(&installed, &reference)
        .expect_err("a call into a retired realization refuses");

    assert!(error.0.contains("execute authority"));
}

#[test]
fn retire_refuses_a_demand_for_facts_it_did_not_perform() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (installed_id, installed) =
        install_through_provider(&mut provider, &artifact(41), 41, 0x4000);
    let authority = RetirementAuthority::from_admitted_provider(
        &installed,
        [RetirementFactDigest::from_canonical_bytes(
            b"omega.other-provider.unperformed-step.v1",
        )],
    );

    let error = provider
        .retire(&installed, &authority)
        .expect_err("a demand beyond the performed set refuses");

    assert!(error.0.contains("cannot establish"));
    assert_eq!(provider.execute_enabled(installed_id), Some(true));
}

#[test]
fn retire_refuses_an_authority_scoped_to_another_realization() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (_id, installed) = install_through_provider(&mut provider, &artifact(41), 41, 0x4000);
    let (_other_id, other) = install_through_provider(&mut provider, &artifact(42), 42, 0x8000);
    let authority =
        RetirementAuthority::from_admitted_provider(&other, OwnedImageProvider::retire_facts());

    let error = provider
        .retire(&installed, &authority)
        .expect_err("an authority scoped to another realization refuses");

    assert!(error.0.contains("not scoped"));
    assert_eq!(provider.execute_enabled(installed.identity()), Some(true));
}

#[test]
fn retire_refuses_when_the_provider_holds_no_resident_image() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    // A realization installed by a different provider (here: the test support
    // path, not this provider) is not resident in this provider's custody.
    let admitted = admit(&two_site_artifact(41));
    let installed = installed_code(&admitted, 41, 0x4000);
    let authority =
        RetirementAuthority::from_admitted_provider(&installed, OwnedImageProvider::retire_facts());

    let error = provider
        .retire(&installed, &authority)
        .expect_err("a nonresident realization refuses");

    assert!(error.0.contains("no resident image"));
}

#[test]
fn retire_leaves_the_image_resident_until_the_caller_releases() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (installed_id, installed) =
        install_through_provider(&mut provider, &artifact(41), 41, 0x4000);

    let retired = retire_through_provider(&mut provider, installed);

    assert!(provider.installed_image(installed_id).is_some());
    assert!(provider.release(installed_id).unwrap());
    assert_eq!(provider.installed_image(installed_id), None);
    drop(retired);
}

#[test]
fn release_drops_only_resident_images() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (installed, code) = install_through_provider(&mut provider, &artifact(41), 41, 0x4000);
    let unknown = InstalledCodeId::from_normalized_identity(77).unwrap();

    assert!(!provider.release(unknown).unwrap());
    let _retired = retire_through_provider(&mut provider, code);
    assert!(provider.release(installed).unwrap());
    assert_eq!(provider.installed_image(installed), None);
    assert!(!provider.release(installed).unwrap());
}

#[test]
fn release_refuses_a_range_whose_execution_state_was_never_unwound() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (installed_id, installed) =
        install_through_provider(&mut provider, &artifact(41), 41, 0x4000);

    // Freshly installed: execute authority granted and write authority
    // suspended, so freeing the range would reclaim storage under a mapping
    // no retirement proved unreachable.
    let error = provider
        .release(installed_id)
        .expect_err("an undrained range cannot be released");
    assert!(error.0.contains("execution state"));
    assert!(provider.installed_image(installed_id).is_some());

    // Sealing changes nothing: the still-served mapping stays resident.
    let _reference = seal_through_provider(&provider, &installed, entry_id(1041));
    let error = provider
        .release(installed_id)
        .expect_err("a sealed mapping still cannot be released");
    assert!(error.0.contains("execution state"));
    assert!(provider.installed_image(installed_id).is_some());

    // Once retirement unwinds the execution state the same release frees.
    // `reference` borrows `installed` and is never used again, so the move
    // into `retire_through_provider` is reachable.
    let retired = retire_through_provider(&mut provider, installed);
    assert!(provider.release(installed_id).unwrap());
    assert_eq!(provider.installed_image(installed_id), None);
    let _ = retired;
}

/// Park a realization whose drain cannot complete: the provider performs the
/// quarantine transition and the crossing consumes the receipt it minted.
fn quarantine_through_provider(
    provider: &mut OwnedImageProvider,
    installed: InstalledCode,
    cause: MappingQuarantineCause,
) -> QuarantinedInstallation {
    let receipt = provider
        .quarantine(&installed, cause)
        .expect("provider performs the quarantine");
    quarantine_installed(installed, receipt).expect("the provider's receipt quarantines")
}

#[test]
fn quarantine_parks_the_mapping_as_an_unserved_reserved_range() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (installed_id, installed) =
        install_through_provider(&mut provider, &two_site_artifact(41), 41, 0x4000);
    let reference = seal_through_provider(&provider, &installed, entry_id(1041));
    let context = installed.receipt_context();
    let extent = installed.validated.frozen.placement.extent.length();

    let receipt = provider
        .quarantine(
            &installed,
            MappingQuarantineCause::IncompleteDrain {
                residual_authority_count: 1,
            },
        )
        .expect("provider performs the quarantine");

    // Execute off, write authority restored, the range retained but no
    // longer served — the owned-buffer trapping reservation. A stale call
    // faults rather than being served.
    assert_eq!(provider.execute_enabled(installed_id), Some(false));
    assert_eq!(provider.write_suspended(installed_id), Some(false));
    assert_eq!(provider.quarantined(installed_id), Some(true));
    let error = provider
        .call(&installed, &reference)
        .expect_err("a call into a quarantined mapping faults");
    assert!(error.0.contains("quarantined"));

    let quarantined =
        quarantine_installed(installed, receipt).expect("the provider's receipt quarantines");
    assert_eq!(quarantined.installed_code(), installed_id);
    assert_eq!(quarantined.attributed_capacity_loss(), extent);
    let fault = quarantined
        .stale_entry_fault(&context)
        .expect("a stale entry attempt faults at the trapping mapping");
    assert!(!fault.discharged_obligations());
}

#[test]
fn uninstall_routes_a_failed_drain_to_the_provider_quarantine() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (_installed_id, installed) =
        install_through_provider(&mut provider, &artifact(41), 41, 0x4000);
    let authority =
        RetirementAuthority::from_admitted_provider(&installed, OwnedImageProvider::retire_facts());
    // The drain refuses: this receipt establishes none of the demanded
    // retirement facts — residual authority outstanding sends the uninstall
    // to the quarantine ending the provider's receipt supplies.
    let drain =
        RetirementReceipt::from_provider(&installed, false, false, false, std::iter::empty());
    let quarantine = provider
        .quarantine(
            &installed,
            MappingQuarantineCause::IncompleteDrain {
                residual_authority_count: 2,
            },
        )
        .expect("provider performs the quarantine");

    let outcome = uninstall_installed(installed, authority, drain, Some(quarantine))
        .expect("the failed drain parks at the provider's trapping quarantine");

    let UninstallOutcome::Quarantined(quarantined) = outcome else {
        panic!("an incomplete drain quarantines rather than retiring")
    };
    assert_eq!(
        quarantined.cause(),
        &MappingQuarantineCause::IncompleteDrain {
            residual_authority_count: 2
        }
    );
}

#[test]
fn quarantine_refuses_without_an_attributed_residual_holder() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (installed_id, installed) =
        install_through_provider(&mut provider, &artifact(41), 41, 0x4000);

    let error = provider
        .quarantine(
            &installed,
            MappingQuarantineCause::IncompleteDrain {
                residual_authority_count: 0,
            },
        )
        .expect_err("an unattributed quarantine refuses");

    assert!(error.0.contains("attributed"));
    // A refused quarantine mutates nothing: the mapping stays served.
    assert_eq!(provider.execute_enabled(installed_id), Some(true));
    assert_eq!(provider.quarantined(installed_id), Some(false));
}

#[test]
fn quarantine_refuses_when_the_provider_holds_no_resident_image() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let admitted = admit(&artifact(41));
    let installed = installed_code(&admitted, 41, 0x4000);

    let error = provider
        .quarantine(
            &installed,
            MappingQuarantineCause::PossibleOpaqueHolder {
                provider_identity: "other-provider".into(),
            },
        )
        .expect_err("a nonresident realization refuses");

    assert!(error.0.contains("no resident image"));
}

#[test]
fn quarantine_refuses_a_second_parking_and_retire_refuses_a_parked_mapping() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (_installed_id, installed) =
        install_through_provider(&mut provider, &artifact(41), 41, 0x4000);
    let cause = || MappingQuarantineCause::IncompleteDrain {
        residual_authority_count: 1,
    };

    provider
        .quarantine(&installed, cause())
        .expect("provider performs the first quarantine");

    let error = provider
        .quarantine(&installed, cause())
        .expect_err("a second parking refuses");
    assert!(error.0.contains("already quarantined"));
    let authority =
        RetirementAuthority::from_admitted_provider(&installed, OwnedImageProvider::retire_facts());
    let error = provider
        .retire(&installed, &authority)
        .expect_err("the drain ending is exclusive once quarantined");
    assert!(error.0.contains("already quarantined"));
}

#[test]
fn release_refuses_to_free_a_quarantined_range() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (installed_id, installed) =
        install_through_provider(&mut provider, &artifact(41), 41, 0x4000);

    quarantine_through_provider(
        &mut provider,
        installed,
        MappingQuarantineCause::PossibleOpaqueHolder {
            provider_identity: "mystery-holder".into(),
        },
    );

    let error = provider
        .release(installed_id)
        .expect_err("a quarantined range stays reserved");
    assert!(error.0.contains("stays reserved"));
    assert_eq!(provider.quarantined(installed_id), Some(true));
    assert!(provider.installed_image(installed_id).is_some());
}

#[test]
fn seal_and_patch_refuse_a_quarantined_mapping() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (installed_id, installed) =
        install_through_provider(&mut provider, &two_site_artifact(41), 41, 0x4000);
    let (_successor_id, successor) =
        install_through_provider(&mut provider, &artifact(42), 42, 0x8000);
    provider
        .quarantine(
            &installed,
            MappingQuarantineCause::IncompleteDrain {
                residual_authority_count: 1,
            },
        )
        .expect("provider performs the quarantine");
    assert_eq!(provider.quarantined(installed_id), Some(true));

    let seal_authority = EntryReferenceAuthority::from_admitted_provider(
        &installed,
        entry_id(1041),
        seal_contract(),
    );
    let error = provider
        .seal_entry(&installed, &seal_authority)
        .expect_err("sealing a quarantined mapping refuses");
    assert!(error.0.contains("quarantined"));

    let patch_authority = ReplacementAuthority::from_admitted_provider(
        &installed,
        &successor,
        [],
        OwnedImageProvider::patch_facts(),
    );
    let error = provider
        .patch(&installed, &successor, &patch_authority)
        .expect_err("patching a quarantined mapping refuses");
    assert!(error.0.contains("quarantined"));

    // And the symmetric direction: calls cannot route to a quarantined
    // successor either.
    provider
        .quarantine(
            &successor,
            MappingQuarantineCause::IncompleteDrain {
                residual_authority_count: 1,
            },
        )
        .expect("provider quarantines the successor");
    let (_other_id, other) = install_through_provider(&mut provider, &artifact(43), 43, 0xC000);
    let patch_authority = ReplacementAuthority::from_admitted_provider(
        &other,
        &successor,
        [],
        OwnedImageProvider::patch_facts(),
    );
    let error = provider
        .patch(&other, &successor, &patch_authority)
        .expect_err("patching toward a quarantined successor refuses");
    assert!(error.0.contains("quarantined"));
}
