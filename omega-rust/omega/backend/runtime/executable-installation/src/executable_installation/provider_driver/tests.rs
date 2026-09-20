use super::CallCodeError;
use crate::executable_installation::test_support::{
    admit, artifact, certificate, entry_id, frozen_for_audience, installed_code,
    relocatable_artifact,
};
use crate::executable_installation::{
    EntryContractDigest, EntryReferenceAuthority, InstallAuthority, InstallationAudience,
    InstallationFactDigest, InstalledCode, InstalledCodeId, MappingQuarantineCause,
    OwnedImageProvider, ReplacementAuthority, ReplacementOutcome, RetirementAuthority,
    UninstallOutcome, ValidatedPlacement, validate_final_placement,
};
use target::Architecture;

const CONTRACT: &[u8] = b"omega.test.driver-entry-contract.v1";

fn seal_contract() -> EntryContractDigest {
    EntryContractDigest::from_canonical_bytes(CONTRACT)
}

fn valid(identity: u64, audience: InstallationAudience) -> ValidatedPlacement {
    valid_at(identity, 0x4000, audience)
}

fn valid_at(identity: u64, base: u64, audience: InstallationAudience) -> ValidatedPlacement {
    let artifact = artifact(identity);
    let admitted = admit(&artifact);
    let frozen = frozen_for_audience(&admitted, identity, base, audience);
    let certificate = certificate(&frozen, 500 + identity);
    validate_final_placement(frozen, &certificate).expect("validated placement")
}

fn install(
    provider: &mut OwnedImageProvider,
    identity: u64,
    audience: InstallationAudience,
) -> (InstalledCodeId, InstalledCode) {
    let validated = valid(identity, audience);
    let authority = InstallAuthority::from_admitted_provider(&validated);
    provider
        .install_code(validated, authority)
        .expect("the provider's composed install drives")
}

#[test]
fn install_code_performs_the_operation_and_consumes_its_receipt() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let validated = valid(41, InstallationAudience::FutureFetcher);
    let expected = validated.frozen.materialized.bytes().to_vec();
    let authority = InstallAuthority::from_admitted_provider(&validated)
        .with_required_facts(OwnedImageProvider::install_facts());

    let (installed_id, installed) = provider
        .install_code(validated, authority)
        .expect("the composed install drives");

    assert_eq!(installed.identity(), installed_id);
    assert_eq!(
        provider.installed_image(installed_id),
        Some(expected.as_slice())
    );
    assert_eq!(provider.write_suspended(installed_id), Some(true));
    assert_eq!(provider.execute_enabled(installed_id), Some(true));
}

#[test]
fn install_code_returns_every_input_when_facts_are_unsatisfiable() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let validated = valid(41, InstallationAudience::FutureFetcher);
    let authority = InstallAuthority::from_admitted_provider(&validated).with_required_facts([
        InstallationFactDigest::from_canonical_bytes(b"omega.other-provider.unperformed-step.v1"),
    ]);

    let error = provider
        .install_code(validated, authority)
        .expect_err("a demand beyond the performed set refuses");

    assert!(error.diagnostic().0.contains("cannot establish"));
    assert!(error.receipt().is_none());
    let (validated, authority, receipt) = error.into_parts();
    assert!(receipt.is_none());
    let _ = (validated, authority);
}

#[test]
fn install_code_returns_every_input_for_a_misscoped_authority() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let validated = valid(41, InstallationAudience::FutureFetcher);
    // An authority scoped to a different validated placement refuses at the
    // provider: its evidence names the handed placement, not this one.
    let other = valid(42, InstallationAudience::FutureFetcher);
    let authority = InstallAuthority::from_admitted_provider(&other);

    let error = provider
        .install_code(validated, authority)
        .expect_err("a mis-scoped authority refuses");

    assert!(error.diagnostic().0.contains("not scoped"));
    let (_validated, _authority, receipt) = error.into_parts();
    assert!(receipt.is_none(), "the provider refuses before minting");
}

#[test]
fn seal_code_entry_and_call_code_drive_the_entry_path() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (installed_id, installed) = install(&mut provider, 41, InstallationAudience::FutureFetcher);
    let authority = EntryReferenceAuthority::from_admitted_provider(
        &installed,
        entry_id(1041),
        seal_contract(),
    )
    .with_required_facts(OwnedImageProvider::seal_facts());

    let call = provider
        .call_code(&installed, authority)
        .expect("the composed seal and call drives");

    assert_eq!(call.installed_code(), installed_id);
    assert_eq!(call.entry(), entry_id(1041));
    assert_eq!(call.contract(), seal_contract());
    // artifact(41) declares its single entry at offset 16: the entered extent
    // is 16..64 of the resident image.
    let image = provider
        .installed_image(installed_id)
        .expect("resident image");
    assert_eq!(call.code(), &image[16..]);
}

#[test]
fn call_code_returns_the_authority_when_the_seal_refuses() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (_installed_id, installed) =
        install(&mut provider, 41, InstallationAudience::FutureFetcher);
    let authority = EntryReferenceAuthority::from_admitted_provider(
        &installed,
        entry_id(9999),
        seal_contract(),
    );

    let error = provider
        .call_code(&installed, authority)
        .expect_err("an undeclared entry refuses at the seal");

    let CallCodeError::Seal(error) = error else {
        panic!("a seal-stage refusal returns the authority and diagnostic")
    };
    assert!(error.diagnostic().0.contains("not a declared entry"));
    let (_authority, receipt) = error.into_parts();
    assert!(receipt.is_none(), "the provider refuses before minting");
}

#[test]
fn call_code_refuses_to_seal_into_a_retired_realization() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (_id, installed) = install(&mut provider, 41, InstallationAudience::FutureFetcher);
    let retirement =
        RetirementAuthority::from_admitted_provider(&installed, OwnedImageProvider::retire_facts());
    provider
        .retire(&installed, &retirement)
        .expect("the provider performs the retirement");
    let authority = EntryReferenceAuthority::from_admitted_provider(
        &installed,
        entry_id(1041),
        seal_contract(),
    );

    let error = provider
        .call_code(&installed, authority)
        .expect_err("a retired realization refuses to seal");

    assert!(error.diagnostic().0.contains("execute authority"));
}

#[test]
fn retire_code_consumes_the_drain_and_returns_the_placement() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (installed_id, installed) = install(&mut provider, 41, InstallationAudience::FutureFetcher);
    let authority =
        RetirementAuthority::from_admitted_provider(&installed, OwnedImageProvider::retire_facts());

    let (returned_id, retired) = provider
        .retire_code(installed, authority)
        .expect("the composed retirement drives");

    assert_eq!(returned_id, installed_id);
    assert_eq!(provider.execute_enabled(installed_id), Some(false));
    assert_eq!(provider.write_suspended(installed_id), Some(false));
    // The drained storage stays resident until the caller releases it.
    assert!(provider.installed_image(installed_id).is_some());
    assert!(provider.release(installed_id).unwrap());
    assert_eq!(provider.installed_image(installed_id), None);
    let _placement = retired.into_placement();
}

#[test]
fn uninstall_code_without_residual_authority_retires() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (installed_id, installed) = install(&mut provider, 41, InstallationAudience::FutureFetcher);
    let authority =
        RetirementAuthority::from_admitted_provider(&installed, OwnedImageProvider::retire_facts());

    let (returned_id, outcome) = provider
        .uninstall_code(installed, authority, None)
        .expect("the composed uninstall drives");

    assert_eq!(returned_id, installed_id);
    let UninstallOutcome::Retired(retired) = outcome else {
        panic!("a complete drain retires")
    };
    assert!(provider.installed_image(installed_id).is_some());
    assert!(provider.release(installed_id).unwrap());
    let _ = retired;
}

#[test]
fn uninstall_code_with_residual_authority_quarantines() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (installed_id, installed) = install(&mut provider, 41, InstallationAudience::FutureFetcher);
    let authority =
        RetirementAuthority::from_admitted_provider(&installed, OwnedImageProvider::retire_facts());

    let (_id, outcome) = provider
        .uninstall_code(
            installed,
            authority,
            Some(MappingQuarantineCause::IncompleteDrain {
                residual_authority_count: 2,
            }),
        )
        .expect("the incomplete drain parks at the provider quarantine");

    let UninstallOutcome::Quarantined(quarantined) = outcome else {
        panic!("an incomplete drain quarantines rather than retiring")
    };
    assert_eq!(quarantined.installed_code(), installed_id);
    assert_eq!(provider.quarantined(installed_id), Some(true));
    assert!(provider.release(installed_id).is_err());
}

#[test]
fn uninstall_code_returns_every_input_when_the_provider_cannot_drain() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    // A realization this provider never installed has no resident image: the
    // provider stage refuses, and every input comes back.
    let admitted = admit(&artifact(41));
    let foreign = installed_code(&admitted, 41, 0x4000);
    let authority =
        RetirementAuthority::from_admitted_provider(&foreign, OwnedImageProvider::retire_facts());

    let error = provider
        .uninstall_code(foreign, authority, None)
        .expect_err("a nonresident realization refuses");

    assert!(error.diagnostic().0.contains("no resident image"));
    let (_installed, _authority, retirement, quarantine, _cause) = error.into_parts();
    assert!(retirement.is_none());
    assert!(quarantine.is_none());
}

#[test]
fn replace_code_patches_then_drains_the_superseded() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (superseded_id, superseded) = install(
        &mut provider,
        41,
        InstallationAudience::PossibleCurrentExecutor,
    );
    let (_successor_id, successor) = install(
        &mut provider,
        42,
        InstallationAudience::PossibleCurrentExecutor,
    );
    let fragment = admit(&relocatable_artifact(
        71,
        Architecture::X86_64,
        vec![0xCC; 6],
        Vec::new(),
    ));
    let patch_authority = ReplacementAuthority::from_admitted_provider(
        &superseded,
        &successor,
        [(entry_id(1041), fragment)],
        OwnedImageProvider::patch_facts(),
    );
    let retire_authority = RetirementAuthority::from_admitted_provider(
        &superseded,
        OwnedImageProvider::retire_facts(),
    );
    let expected: Vec<u8> = provider
        .installed_image(superseded_id)
        .expect("resident image")
        .iter()
        .enumerate()
        .map(|(offset, byte)| {
            if (16..22).contains(&offset) {
                0xCC
            } else {
                *byte
            }
        })
        .collect();

    let (_id, outcome) = provider
        .replace_code(
            superseded,
            &successor,
            patch_authority,
            retire_authority,
            None,
        )
        .expect("the composed replacement drives");

    let ReplacementOutcome::Retired { retired, patch } = outcome else {
        panic!("a complete drain retires the superseded realization")
    };
    assert_eq!(
        provider.installed_image(superseded_id),
        Some(expected.as_slice())
    );
    assert_eq!(provider.execute_enabled(superseded_id), Some(false));
    let _ = (retired, patch);
}

#[test]
fn seal_code_entry_returns_every_input_the_provider_rejects() {
    let mut provider = OwnedImageProvider::for_architecture(Architecture::X86_64);
    let (_id, installed) = install(&mut provider, 41, InstallationAudience::FutureFetcher);
    let (_other_id, other) = install(&mut provider, 42, InstallationAudience::FutureFetcher);
    let authority =
        EntryReferenceAuthority::from_admitted_provider(&other, entry_id(1042), seal_contract())
            .with_required_facts(OwnedImageProvider::seal_facts());

    let error = provider
        .seal_code_entry(&installed, authority)
        .expect_err("an authority scoped to another realization refuses");

    assert!(error.diagnostic().0.contains("not scoped"));
    let (_authority, _receipt) = error.into_parts();
}
