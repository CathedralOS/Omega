//! Reference transport survives ingress without becoming pointee storage.

use super::target_family_tests::identity_table;
use crate::{CodecError, decode_module, encode_module};
use semantic_vocabulary::{StructuralFieldId, StructuralTypeId};
use terminal_psi::{
    BindingRelevance, StructuralAccess, StructuralFieldDeclaration, StructuralFieldType,
    StructuralRecordStorage, StructuralReferentIdentity, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalIdentityIndex, TerminalModule,
};

fn reference_module(foreign_descriptor: bool) -> TerminalModule {
    let mut module = TerminalModule::declarations();
    module.identities = identity_table(4);
    module
        .structural_referents
        .push(StructuralReferentIdentity {
            id: StructuralTypeId::new(1).unwrap(),
            identity: TerminalIdentityIndex(1),
            foreign_descriptor,
            abstract_opaque: false,
        });
    module.structural_types.push(StructuralTypeDeclaration {
        id: StructuralTypeId::new(2).unwrap(),
        identity: TerminalIdentityIndex(2),
        quotient_carrier: None,
        owns_linear_claim: false,
        copyable: true,
        no_code_affine_cleanup: false,
        descriptor_cleanup: true,
        scalar_restriction: None,
        shape: StructuralTypeShape::Reference {
            referent: StructuralTypeId::new(1).unwrap(),
            access: StructuralAccess::SharedBorrow,
        },
    });
    // The independent task catalog uses the same leaf encoding, even when no
    // executable task occurrence demands that catalog in this declaration unit.
    module.task_activation_demands.structural_types = module.structural_types.clone();
    module.task_activation_demands.structural_referents = module.structural_referents.clone();
    module
}

#[test]
fn reference_transport_round_trip_preserves_both_roles_without_storage() {
    for descriptor in [false, true] {
        let module = reference_module(descriptor);
        let bytes = encode_module(&module).unwrap();
        let decoded = decode_module(&bytes).unwrap();
        assert_eq!(decoded, module);
        assert_eq!(encode_module(&decoded).unwrap(), bytes);
        let referent = StructuralTypeId::new(1).unwrap();
        assert_eq!(
            decoded.foreign_reference_descriptor(referent),
            Some(descriptor)
        );
        assert!(decoded.structural_type(referent).is_none());
    }
}

#[test]
fn reference_transport_cannot_supply_owned_field_storage() {
    for descriptor in [false, true] {
        let mut module = reference_module(descriptor);
        module.native_layouts.insert(
            StructuralTypeId::new(3).unwrap(),
            TerminalIdentityIndex(3),
            TerminalIdentityIndex::ABSENT,
            TerminalIdentityIndex::ABSENT,
            [terminal_psi::NativeLayoutFieldIdentity {
                field: StructuralFieldId::new(1).unwrap(),
                declaration_identity: TerminalIdentityIndex(4),
            }],
            [],
        );
        module.structural_types.push(StructuralTypeDeclaration {
            id: StructuralTypeId::new(3).unwrap(),
            identity: TerminalIdentityIndex(3),
            quotient_carrier: None,
            owns_linear_claim: false,
            copyable: false,
            no_code_affine_cleanup: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            shape: StructuralTypeShape::Record {
                storage: StructuralRecordStorage::Packed,
                fields: vec![StructuralFieldDeclaration {
                    inline_carrier: None,
                    id: StructuralFieldId::new(1).unwrap(),
                    identity: TerminalIdentityIndex(4),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(StructuralTypeId::new(1).unwrap()),
                    storage: None,
                }],
            },
        });
        let bytes = encode_module(&module).unwrap();
        let error = decode_module(&bytes).expect_err("transport supplied owned storage");
        assert!(
            error.to_string().contains("unknown structural type"),
            "{error}"
        );
    }
}

#[test]
fn reference_transport_rejects_the_pre_role_wire_format() {
    let mut bytes = encode_module(&reference_module(false)).unwrap();
    bytes[8..10].copy_from_slice(&223_u16.to_le_bytes());
    assert!(matches!(
        decode_module(&bytes),
        Err(CodecError::UnsupportedFormatMarker(223))
    ));
}

fn inline_carrier_module(byte_field: bool) -> TerminalModule {
    let mut module = TerminalModule::declarations();
    module.identities = identity_table(4);
    let (shape, field_type) = if byte_field {
        let carrier = terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity: 6 };
        (
            StructuralTypeShape::ByteSequence(carrier),
            StructuralFieldType::ByteSequence(carrier),
        )
    } else {
        (
            StructuralTypeShape::PrimitiveScalar(semantic_vocabulary::ScalarType::Boolean),
            StructuralFieldType::Scalar(semantic_vocabulary::ScalarType::Boolean),
        )
    };
    let carrier = StructuralTypeDeclaration {
        id: StructuralTypeId::new(1).unwrap(),
        identity: TerminalIdentityIndex(1),
        quotient_carrier: None,
        owns_linear_claim: false,
        copyable: true,
        no_code_affine_cleanup: true,
        descriptor_cleanup: false,
        scalar_restriction: None,
        shape,
    };
    let mut duplicate = carrier.clone();
    duplicate.id = StructuralTypeId::new(2).unwrap();
    duplicate.identity = TerminalIdentityIndex(2);
    let mut record = carrier.clone();
    record.id = StructuralTypeId::new(3).unwrap();
    record.identity = TerminalIdentityIndex(3);
    record.shape = StructuralTypeShape::Record {
        storage: StructuralRecordStorage::Packed,
        fields: vec![StructuralFieldDeclaration {
            id: StructuralFieldId::new(1).unwrap(),
            identity: TerminalIdentityIndex(4),
            relevance: BindingRelevance::Relevant,
            storage: None,
            inline_carrier: Some(carrier.id),
            field_type,
        }],
    };
    let StructuralTypeShape::Record { fields, .. } = &record.shape else {
        panic!("record")
    };
    module.native_layouts.insert(
        record.id,
        record.identity,
        TerminalIdentityIndex::ABSENT,
        TerminalIdentityIndex::ABSENT,
        fields
            .iter()
            .map(|field| terminal_psi::NativeLayoutFieldIdentity {
                field: field.id,
                declaration_identity: field.identity,
            }),
        [],
    );
    module.structural_types = vec![carrier, duplicate, record];
    module
}

#[test]
fn inline_carrier_round_trip_retains_exact_scalar_and_byte_identities() {
    for byte_field in [false, true] {
        let module = inline_carrier_module(byte_field);
        let bytes = encode_module(&module).unwrap();
        let decoded = decode_module(&bytes).unwrap();
        assert_eq!(decoded, module);
        assert_eq!(encode_module(&decoded).unwrap(), bytes);
    }
}

#[test]
fn inline_carrier_ingress_rejects_missing_incompatible_and_identity_only_types() {
    for byte_field in [false, true] {
        for identity in [3, 4, 99] {
            let mut module = inline_carrier_module(byte_field);
            let StructuralTypeShape::Record { fields, .. } = &mut module.structural_types[2].shape
            else {
                panic!("record")
            };
            fields[0].inline_carrier = Some(StructuralTypeId::new(identity).unwrap());
            if identity == 4 {
                module.identities = identity_table(5);
                module
                    .structural_referents
                    .push(StructuralReferentIdentity {
                        id: StructuralTypeId::new(4).unwrap(),
                        identity: TerminalIdentityIndex(4),
                        foreign_descriptor: true,
                        abstract_opaque: false,
                    });
                let mut reference = module.structural_types[0].clone();
                reference.id = StructuralTypeId::new(5).unwrap();
                reference.identity = TerminalIdentityIndex(5);
                reference.no_code_affine_cleanup = false;
                reference.descriptor_cleanup = true;
                reference.shape = StructuralTypeShape::Reference {
                    referent: StructuralTypeId::new(4).unwrap(),
                    access: StructuralAccess::SharedBorrow,
                };
                module.structural_types.push(reference);
            }
            let error =
                decode_module(&super::module_wire::encode_raw(&module).unwrap()).unwrap_err();
            assert!(error.to_string().contains("inline field"), "{error}");
        }
    }
}

#[test]
fn opaque_storage_roles_round_trip_and_reject_redirected_carriers() {
    use language_semantics::opaque_representations::{
        OpaqueRepresentationApplicationOrigin, OpaqueRepresentationCopyDisposition,
        OpaqueRepresentationLifecycleDisposition, SelectedOpaqueApplication,
        selected_application_commitment,
    };
    let mut module = inline_carrier_module(false);
    module.identities = identity_table(5);
    let mut carrier = module.structural_types[2].clone();
    carrier.id = StructuralTypeId::new(4).unwrap();
    carrier.identity = TerminalIdentityIndex(5);
    let StructuralTypeShape::Record { fields, .. } = &mut carrier.shape else {
        panic!("record")
    };
    fields[0].id = StructuralFieldId::new(2).unwrap();
    module.native_layouts.insert(
        carrier.id,
        TerminalIdentityIndex(3),
        TerminalIdentityIndex::ABSENT,
        TerminalIdentityIndex::ABSENT,
        fields
            .iter()
            .map(|field| terminal_psi::NativeLayoutFieldIdentity {
                field: field.id,
                declaration_identity: field.identity,
            }),
        [],
    );
    module.structural_types.push(carrier);
    // This synthetic preimage is descriptive codec input, not admitted Build
    // selection. Runtime authority cannot be obtained from this fixture.
    let identity_encoding = b"opaque storage correspondence codec control".to_vec();
    let (application_report_fingerprint, conformance_application_commitment) =
        language_semantics::conformance_identity::source_identity_commitments(&identity_encoding);
    let lifecycle = OpaqueRepresentationLifecycleDisposition::Inert;
    let copy_disposition = OpaqueRepresentationCopyDisposition::PlacementOnly;
    let origin = OpaqueRepresentationApplicationOrigin::NamedConformance;
    module
        .opaque_representations
        .push(terminal_psi::TerminalOpaqueRepresentation {
            opaque: StructuralTypeId::new(3).unwrap(),
            carrier: StructuralTypeId::new(4).unwrap(),
            opaque_storage_identity: TerminalIdentityIndex(3),
            carrier_storage_identity: TerminalIdentityIndex(5),
            application: SelectedOpaqueApplication {
                opaque_identity: "package::Opaque".into(),
                carrier_identity: "package::Carrier".into(),
                conformance_identity: "package::Representation".into(),
                application_report_fingerprint,
                conformance_application_commitment,
                identity_encoding,
                schema_version: 1,
                lifecycle,
                copy_disposition,
                origin,
                selected_application_commitment: selected_application_commitment(
                    conformance_application_commitment,
                    lifecycle,
                    copy_disposition,
                    origin,
                ),
            },
        });
    let bytes = encode_module(&module).unwrap();
    assert_eq!(decode_module(&bytes).unwrap(), module);
    let mut redirected = module.clone();
    redirected.opaque_representations[0].carrier = StructuralTypeId::new(2).unwrap();
    let error = decode_module(&super::module_wire::encode_raw(&redirected).unwrap()).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("substituted its exact storage identity"),
        "{error}"
    );
    // Even coordinated descriptive coordinates cannot substitute another
    // physical contract while leaving the selected storage graph unchanged.
    redirected.opaque_representations[0].carrier_storage_identity = TerminalIdentityIndex(2);
    let error = decode_module(&super::module_wire::encode_raw(&redirected).unwrap()).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("selected carrier storage contract"),
        "{error}"
    );
    let mut changed_native = module;
    changed_native.native_layouts = terminal_psi::NativeLayoutCatalog::default();
    for declaration in changed_native
        .structural_types
        .iter()
        .filter(|row| row.is_record_shape())
    {
        changed_native.native_layouts.insert(
            declaration.id,
            declaration.identity,
            TerminalIdentityIndex::ABSENT,
            TerminalIdentityIndex::ABSENT,
            declaration
                .shape
                .record_fields()
                .unwrap()
                .iter()
                .map(|field| terminal_psi::NativeLayoutFieldIdentity {
                    field: field.id,
                    declaration_identity: field.identity,
                }),
            [],
        );
    }
    let error =
        decode_module(&super::module_wire::encode_raw(&changed_native).unwrap()).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("selected native layout provenance"),
        "{error}"
    );
}
