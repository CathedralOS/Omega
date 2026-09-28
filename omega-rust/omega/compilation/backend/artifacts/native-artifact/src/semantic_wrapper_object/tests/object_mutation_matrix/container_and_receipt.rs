//! One-field mutations of the retained container and the custody receipt.
//!
//! The owning stage replays the container by decoding `container.bytes`,
//! re-encoding the expected object, and comparing the whole retained container
//! (`ContainerMismatch`), and replays the receipt by recomputing `custody`
//! over the expected artifacts (`ReceiptMismatch`). Container identity is
//! additionally bound into the manifest, so manifest replay rejects it too.
//! Every byte substitution of the canonical container is a non-canonical wire
//! record: the embedded object identity covers the entire payload.
use super::super::super::OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt;
use super::super::custody;
use super::super::{
    OptimizedProgramStorageSemanticWrapperObjectRecordError,
    decode_optimized_program_storage_semantic_wrapper_object,
    encode_optimized_program_storage_semantic_wrapper_object, template,
};
use super::fixture::staged_parts;
use crate::semantic_wrapper_object::manifest::validate_manifest;
use optimization_core::{
    OptimizedObjectArtifactIdentity, OptimizedProgramStorageSemanticWrapperObjectContainerIdentity,
    OptimizedProgramStorageSemanticWrapperObjectIdentity,
    OptimizedProgramStorageSemanticWrapperObjectManifestIdentity,
};

type ReceiptMutation = fn(&mut OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt);

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

#[test]
fn wrapper_object_custody_receipt_rejects_every_one_field_substitution() {
    let (object, container, manifest, receipt) = staged_parts();
    let mutations: [(&str, ReceiptMutation); 5] = [
        ("source_artifact", |receipt| {
            receipt.source_artifact =
                OptimizedObjectArtifactIdentity::from_canonical_bytes(b"mutated-artifact")
        }),
        ("source_signature", |receipt| {
            receipt.source_signature = [0xa7; 32]
        }),
        ("object", |receipt| {
            receipt.object =
                OptimizedProgramStorageSemanticWrapperObjectIdentity::from_canonical_bytes(
                    b"mutated-object",
                )
        }),
        ("container", |receipt| {
            receipt.container =
                OptimizedProgramStorageSemanticWrapperObjectContainerIdentity::from_canonical_bytes(
                    b"mutated-container",
                )
        }),
        ("manifest", |receipt| {
            receipt.manifest =
                OptimizedProgramStorageSemanticWrapperObjectManifestIdentity::from_canonical_bytes(
                    b"mutated-manifest",
                )
        }),
    ];

    for (field, mutate) in mutations {
        let mut mutated = receipt;
        mutate(&mut mutated);
        // `validate_optimized_program_storage_semantic_wrapper_object`
        // recomputes `custody(&expected, &container, &manifest)` and rejects
        // `staged.custody` on inequality as `ReceiptMismatch`; this is the same
        // recomputation at component scope.
        assert_ne!(
            mutated,
            custody(&object, &container, &manifest),
            "independent custody recomputation must reject substituted {field}",
        );
    }
}
