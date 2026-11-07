//! Dependency queries describe storage only; they never admit these rows as
//! executable artifacts or issue representation-selection authority.
use super::*;
use semantic_vocabulary::{
    AuthoredFieldStorage, AuthoredRecordStorage, ScalarType, StructuralFieldId,
};
use terminal_psi::{
    ArrayExtent, StructuralFieldDeclaration, StructuralFieldType, StructuralRecordStorage,
    StructuralTypeDeclaration, TargetLayoutApplication, TargetLayoutFieldStorage,
    TargetLayoutRealization, TargetLayoutSchemaArgument, TerminalIdentityIndex,
};

fn identity(value: u64) -> StructuralTypeId {
    StructuralTypeId::new(value).unwrap()
}

fn declaration(value: u64, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        id: identity(value),
        identity: TerminalIdentityIndex::ABSENT,
        shape,
        quotient_carrier: None,
        owns_linear_claim: false,
        copyable: true,
        no_code_affine_cleanup: true,
        descriptor_cleanup: false,
        scalar_restriction: None,
    }
}

fn array_module() -> TerminalModule {
    let mut module = TerminalModule::declarations();
    module.structural_types = vec![
        declaration(1, StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean)),
        declaration(
            2,
            StructuralTypeShape::FixedArray {
                element: identity(1),
                length: ArrayExtent::Application(ScalarApplicationId::new(1).unwrap()),
            },
        ),
    ];
    module
}

#[test]
fn finite_storage_depth_is_not_a_new_foreign_dependency_limit() {
    let mut module = array_module();
    for value in 3..=131 {
        module.structural_types.push(declaration(
            value,
            StructuralTypeShape::FixedArray {
                element: identity(value - 1),
                length: ArrayExtent::Literal(1),
            },
        ));
    }
    assert_eq!(
        foreign_storage_extent_dependencies(&module, [identity(131)], "macos_arm64").unwrap(),
        [ScalarApplicationId::new(1).unwrap()]
    );
}

#[test]
fn selected_opaque_carrier_owes_its_actual_array_extent() {
    use language_semantics::opaque_representations::*;
    let mut module = array_module();
    module.structural_types.push(declaration(
        3,
        StructuralTypeShape::Record {
            fields: Vec::new(),
            storage: StructuralRecordStorage::Packed,
        },
    ));
    // This operation consumes an existing selected roster. The commitment
    // constructor is exercised normally; the test issues no admitted receipt.
    let encoding = b"dependency-query-selection".to_vec();
    let (report, conformance) =
        language_semantics::conformance_identity::source_identity_commitments(&encoding);
    let lifecycle = OpaqueRepresentationLifecycleDisposition::Inert;
    let copy = OpaqueRepresentationCopyDisposition::PlacementOnly;
    let origin = OpaqueRepresentationApplicationOrigin::NamedConformance;
    module
        .opaque_representations
        .push(terminal_psi::TerminalOpaqueRepresentation {
            opaque: identity(3),
            carrier: identity(2),
            opaque_storage_identity: TerminalIdentityIndex::ABSENT,
            carrier_storage_identity: TerminalIdentityIndex::ABSENT,
            application: SelectedOpaqueApplication {
                opaque_identity: "Opaque".into(),
                carrier_identity: "Array".into(),
                conformance_identity: "Selected".into(),
                application_report_fingerprint: report,
                conformance_application_commitment: conformance,
                identity_encoding: encoding,
                schema_version: OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION,
                origin,
                lifecycle,
                copy_disposition: copy,
                selected_application_commitment: selected_application_commitment(
                    conformance,
                    lifecycle,
                    copy,
                    origin,
                ),
            },
        });
    assert_eq!(
        foreign_storage_extent_dependencies(&module, [identity(3)], "macos_arm64").unwrap(),
        [ScalarApplicationId::new(1).unwrap()]
    );
}

#[test]
fn target_stored_scalar_does_not_demand_the_logical_child_extent() {
    let mut module = array_module();
    let field = StructuralFieldId::new(1).unwrap();
    let application = terminal_psi::LayoutApplicationId(1);
    module.structural_types.push(declaration(
        3,
        StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: field,
                identity: TerminalIdentityIndex::ABSENT,
                inline_carrier: None,
                relevance: terminal_psi::BindingRelevance::Relevant,
                field_type: StructuralFieldType::Structural(identity(2)),
                storage: None,
            }],
            storage: StructuralRecordStorage::TargetLayout(application),
        },
    ));
    module
        .target_layout_applications
        .push(TargetLayoutApplication {
            id: application,
            record: identity(3),
            schema: identity(3),
            policy_identity: TerminalIdentityIndex::ABSENT,
            plan_identity: TerminalIdentityIndex::ABSENT,
            target_inputs: Vec::new(),
            realizations: vec![TargetLayoutRealization {
                schema_argument: TargetLayoutSchemaArgument {
                    abi_version: 4,
                    fields: Vec::new(),
                    retired_field_identities: Vec::new(),
                },
                target: "macos_arm64".into(),
                pointer_size: 8,
                pointer_alignment: 8,
                case_tag_size: 4,
                case_tag_alignment: 4,
                little_endian: true,
                storage: AuthoredRecordStorage {
                    byte_size: 1,
                    byte_alignment: 1,
                },
                fields: vec![TargetLayoutFieldStorage {
                    field,
                    storage: AuthoredFieldStorage::Scalar {
                        byte_offset: 0,
                        stored_scalar: Some(ScalarType::Boolean),
                    },
                }],
            }],
        });
    assert!(
        foreign_storage_extent_dependencies(&module, [identity(3)], "macos_arm64")
            .unwrap()
            .is_empty()
    );
    assert!(foreign_storage_extent_dependencies(&module, [identity(3)], "linux_x86_64").is_err());
}
