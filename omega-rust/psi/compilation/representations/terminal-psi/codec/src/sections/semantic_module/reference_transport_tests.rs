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
