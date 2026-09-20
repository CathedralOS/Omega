use super::OwnedImageProvider;
use crate::executable_installation::test_support::{
    admit, artifact, artifact_placement_constraints, authority_commitments, certificate, entry_id,
    frozen, id, installed_code, relocatable_artifact,
};
use crate::executable_installation::{
    AdmittedArtifact, Artifact, ArtifactEntry, ArtifactId, ArtifactRelocationKind,
    DecodedArtifactRelocation, EntrySetId, InstallAuthority, InstallationFactDigest, InstalledCode,
    InstalledCodeId, MachineContractSetId, MachineFootprintId, PlacementPlanId, RelocationSetId,
    ReplacementAuthority, ReplacementFactDigest, ReplacementOutcome, RetirementAuthority,
    RetirementReceipt, ValidatedPlacement, install_validated, replace_installed,
    validate_final_placement,
};
use layout_plans::RelocationTarget;
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

    let retirement_authority =
        RetirementAuthority::from_admitted_provider(&superseded, std::iter::empty());
    let retirement =
        RetirementReceipt::from_provider(&superseded, true, true, true, std::iter::empty());
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
    assert!(provider.release(superseded_id));
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
fn release_drops_only_resident_images() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (installed, _code) = install_through_provider(&mut provider, &artifact(41), 41, 0x4000);
    let unknown = InstalledCodeId::from_normalized_identity(77).unwrap();

    assert!(!provider.release(unknown));
    assert!(provider.release(installed));
    assert_eq!(provider.installed_image(installed), None);
    assert!(!provider.release(installed));
}
