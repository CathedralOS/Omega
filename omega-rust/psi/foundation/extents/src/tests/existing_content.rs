use super::{id, local_root_grant, root_grant};
use crate::{
    ExtentContentCustodyReceiptId, ExtentContentInterpretation, ExtentContentInterpretationId,
    ExtentContentValidityReceiptId, ResidentClaimId,
};

#[test]
fn provider_root_mints_exact_existing_content_authority_once() {
    let (extent, content) = root_grant(81)
        .mint_provider_existing_content(
            0x8000,
            64,
            ExtentContentInterpretation::from_sha256_commitment(
                id(82, ExtentContentInterpretationId::from_normalized_identity),
                [0x82; 32],
            ),
            id(85, ResidentClaimId::from_normalized_identity),
            id(83, ExtentContentValidityReceiptId::from_normalized_identity),
            id(84, ExtentContentCustodyReceiptId::from_normalized_identity),
        )
        .expect("provider existing-content root");
    assert_eq!(content.origin(), extent.origin());
    assert_eq!(content.lineage_root(), extent.lineage_root());
    assert_eq!((content.base(), content.length()), (0x8000, 64));
    assert_eq!(content.address_space(), extent.address_space());
    assert_eq!(content.provenance(), extent.provenance());
    assert_eq!(content.era(), extent.era());
    assert_eq!(
        content
            .interpretation()
            .compatibility_fingerprint()
            .normalized_identity(),
        82
    );
    assert_eq!(content.resident_claim().normalized_identity(), 85);
    assert_eq!(content.validity_receipt().normalized_identity(), 83);
    assert_eq!(content.custody_receipt().normalized_identity(), 84);
}

#[test]
fn coincident_provider_content_issuances_retain_distinct_resident_claims() {
    let (_, first) = root_grant(91)
        .mint_provider_existing_content(
            0xa000,
            64,
            ExtentContentInterpretation::from_sha256_commitment(
                id(92, ExtentContentInterpretationId::from_normalized_identity),
                [0x92; 32],
            ),
            id(93, ResidentClaimId::from_normalized_identity),
            id(94, ExtentContentValidityReceiptId::from_normalized_identity),
            id(95, ExtentContentCustodyReceiptId::from_normalized_identity),
        )
        .expect("first resident provider issuance");
    let (_, second) = root_grant(96)
        .mint_provider_existing_content(
            0xa000,
            64,
            ExtentContentInterpretation::from_sha256_commitment(
                id(92, ExtentContentInterpretationId::from_normalized_identity),
                [0x92; 32],
            ),
            id(97, ResidentClaimId::from_normalized_identity),
            id(94, ExtentContentValidityReceiptId::from_normalized_identity),
            id(95, ExtentContentCustodyReceiptId::from_normalized_identity),
        )
        .expect("second resident provider issuance");

    assert_eq!(
        (first.base(), first.length()),
        (second.base(), second.length())
    );
    assert_eq!(first.interpretation(), second.interpretation());
    assert_eq!(first.validity_receipt(), second.validity_receipt());
    assert_eq!(first.custody_receipt(), second.custody_receipt());
    assert_ne!(first.origin(), second.origin());
    assert_ne!(first.resident_claim(), second.resident_claim());
}

#[test]
fn program_local_root_cannot_mint_provider_existing_content_authority() {
    let error = local_root_grant(85, 86)
        .mint_provider_existing_content(
            0x9000,
            64,
            ExtentContentInterpretation::from_sha256_commitment(
                id(87, ExtentContentInterpretationId::from_normalized_identity),
                [0x87; 32],
            ),
            id(90, ResidentClaimId::from_normalized_identity),
            id(88, ExtentContentValidityReceiptId::from_normalized_identity),
            id(89, ExtentContentCustodyReceiptId::from_normalized_identity),
        )
        .expect_err("local capacity cannot assert provider-held content");
    assert!(error.diagnostic().0.contains("program-local"));
    let extent = error
        .into_grant()
        .mint(0x9000, 64)
        .expect("rejected content route returns the root grant");
    assert!(extent.program_local_origin().is_some());
}
