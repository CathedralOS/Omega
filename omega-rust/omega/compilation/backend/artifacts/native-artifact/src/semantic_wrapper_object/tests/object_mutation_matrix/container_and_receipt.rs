//! One-field mutations of the retained container and the custody receipt.
//!
//! The owning stage replays the container by decoding `container.bytes`,
//! re-encoding the expected object, and comparing the whole retained container
//! (`ContainerMismatch`), and replays the receipt by recomputing `custody`
//! over the expected artifacts (`ReceiptMismatch`). Container identity is
//! additionally bound into the manifest, so manifest replay rejects it too.
//! Every byte substitution of the canonical container is a non-canonical wire
//! record: the embedded object identity covers the entire payload.
//!
//! The receipt legs run through the shared
//! `run_one_field_substitution_matrix` driver over the declared
//! `ReceiptFieldForTest` inventory; the container substitutions, whose
//! rejections span decode, whole-container comparison and manifest replay,
//! stay authored explicitly.
use super::super::super::OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt;
use super::super::custody;
use super::super::{
    OptimizedProgramStorageSemanticWrapperObjectRecordError,
    decode_optimized_program_storage_semantic_wrapper_object,
    encode_optimized_program_storage_semantic_wrapper_object, template,
};
use super::fixture::{foreign_parts, staged_parts};
use crate::semantic_wrapper_object::manifest::validate_manifest;
use optimization_core::{
    MutationOutcome, OneFieldSubstitutionMatrix, OptimizedObjectArtifactIdentity,
    OptimizedProgramStorageSemanticWrapperObjectContainerIdentity,
    OptimizedProgramStorageSemanticWrapperObjectIdentity,
    OptimizedProgramStorageSemanticWrapperObjectManifestIdentity,
    run_one_field_substitution_matrix,
};

#[test]
fn wrapper_object_container_rejects_every_one_field_substitution() {
    let (object, container, manifest, _) = staged_parts();
    let expected_container =
        encode_optimized_program_storage_semantic_wrapper_object(&object, &template()).unwrap();

    // `container.object` substitution: the manifest does not bind this field,
    // so the join that rejects it is the staged whole-container comparison
    // (`staged.container != container` → `ContainerMismatch`).
    let mut mutated = container.clone();
    mutated.object = OptimizedProgramStorageSemanticWrapperObjectIdentity::from_canonical_bytes(
        b"mutated-container-object",
    );
    assert_ne!(
        mutated, expected_container,
        "retained container must differ from the canonical re-encode after \
         substituting container.object",
    );

    // `container.identity` substitution: rejected twice — the staged container
    // comparison and manifest replay, since `manifest.container` binds it.
    let mut mutated = container.clone();
    mutated.identity =
        OptimizedProgramStorageSemanticWrapperObjectContainerIdentity::from_canonical_bytes(
            b"mutated-container",
        );
    assert_ne!(mutated, expected_container);
    assert_eq!(
        validate_manifest(&object, &mutated, &manifest),
        Err(OptimizedProgramStorageSemanticWrapperObjectRecordError::ManifestMismatch),
        "retained manifest replay must reject a substituted container identity",
    );

    // `container.bytes` substitutions: the embedded object identity covers the
    // whole payload, so every byte substitution is rejected at decoding as
    // non-canonical before any container comparison.
    let mut bad_magic = container.clone();
    bad_magic.bytes[0] ^= 1;
    assert!(decode_optimized_program_storage_semantic_wrapper_object(&bad_magic.bytes).is_err());
    let mut bad_payload = container.clone();
    bad_payload.bytes[44] ^= 1;
    assert_eq!(
        decode_optimized_program_storage_semantic_wrapper_object(&bad_payload.bytes),
        Err(crate::semantic_wrapper_object::OptimizedProgramStorageSemanticWrapperObjectDecodeError::IdentityMismatch),
        "payload substitution must fail the embedded identity check",
    );
    let mut truncated = container.clone();
    truncated.bytes.pop();
    assert_eq!(
        decode_optimized_program_storage_semantic_wrapper_object(&truncated.bytes),
        Err(crate::semantic_wrapper_object::OptimizedProgramStorageSemanticWrapperObjectDecodeError::Truncated),
    );
    let mut extended = container.clone();
    extended.bytes.push(0);
    assert_eq!(
        decode_optimized_program_storage_semantic_wrapper_object(&extended.bytes),
        Err(crate::semantic_wrapper_object::OptimizedProgramStorageSemanticWrapperObjectDecodeError::TrailingBytes),
    );
    for mutated in [bad_magic, bad_payload, truncated, extended] {
        assert_ne!(
            mutated, expected_container,
            "every container.bytes substitution must differ from the canonical re-encode",
        );
    }

    // A stale container identity over substituted bytes is rejected at the
    // same staged comparison even though the payload is itself well-formed.
    let mut mutated = container.clone();
    mutated.bytes[44] ^= 1;
    assert!(
        decode_optimized_program_storage_semantic_wrapper_object(&mutated.bytes).is_err(),
        "substituted bytes cannot decode while the embedded identity is authentic",
    );
}

optimization_core::custody_field_inventory! {
    /// One field of the wrapper object custody receipt; checked by
    /// recomputing `custody` over the expected plan, container and manifest,
    /// as `validate_optimized_program_storage_semantic_wrapper_object` does
    /// before rejecting `staged.custody` as `ReceiptMismatch`.
    pub enum ReceiptFieldForTest {
        SourceArtifact,
        SourceSignature,
        Object,
        Container,
        Manifest,
    }
}

/// Substitute exactly one receipt field with a fixed alternate. The receipt
/// carries no containing identity, so there is nothing to re-authenticate.
fn substitute_receipt_for_test(
    receipt: &mut OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt,
    field: ReceiptFieldForTest,
    _donor: &OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt,
) {
    match field {
        ReceiptFieldForTest::SourceArtifact => {
            receipt.source_artifact =
                OptimizedObjectArtifactIdentity::from_canonical_bytes(b"mutated-artifact")
        }
        ReceiptFieldForTest::SourceSignature => receipt.source_signature = [0xa7; 32],
        ReceiptFieldForTest::Object => {
            receipt.object =
                OptimizedProgramStorageSemanticWrapperObjectIdentity::from_canonical_bytes(
                    b"mutated-object",
                )
        }
        ReceiptFieldForTest::Container => {
            receipt.container =
                OptimizedProgramStorageSemanticWrapperObjectContainerIdentity::from_canonical_bytes(
                    b"mutated-container",
                )
        }
        ReceiptFieldForTest::Manifest => {
            receipt.manifest =
                OptimizedProgramStorageSemanticWrapperObjectManifestIdentity::from_canonical_bytes(
                    b"mutated-manifest",
                )
        }
    }
}

#[test]
fn wrapper_object_custody_receipt_rejects_every_one_field_substitution() {
    let (object, container, manifest, _) = staged_parts();
    let (_, _, _, foreign) = foreign_parts();
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "wrapper object custody receipt",
        fields: ReceiptFieldForTest::INVENTORY,
        honest: &|| staged_parts().3,
        donor: foreign,
        custody: &|receipt| *receipt,
        substitute: &substitute_receipt_for_test,
        // The independent recomputation at component scope: honest replay
        // rebuilds the receipt from the retained artifacts, and the rebuilt
        // receipt must differ from the substituted one.
        check: &|_| {
            Ok::<_, OptimizedProgramStorageSemanticWrapperObjectRecordError>(custody(
                &object, &container, &manifest,
            ))
        },
        outcome: &|_| MutationOutcome::RebuiltCustodyDiffers,
        joined_replay: None,
    });
}
