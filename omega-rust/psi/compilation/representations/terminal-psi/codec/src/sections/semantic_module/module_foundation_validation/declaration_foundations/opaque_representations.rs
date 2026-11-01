//! Exact selected storage roles at ingress. These associations describe the
//! checked producer's selection; they cannot mint Build selection authority.

use crate::codec_error::{CodecError, malformed};
use terminal_psi::{
    StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralRecordStorage,
    StructuralTypeShape, TerminalModule,
};

pub(super) fn validate(module: &TerminalModule) -> Result<(), CodecError> {
    super::super::super::wire::require_increasing(
        module.opaque_representations.iter().map(|row| row.opaque),
        "opaque representation storage applications",
    )?;
    for relation in &module.opaque_representations {
        let opaque = module.structural_type(relation.opaque).ok_or(
            CodecError::MalformedStructuralFoundation(
                "opaque representation lost its opaque storage",
            ),
        )?;
        let carrier = module.structural_type(relation.carrier).ok_or(
            CodecError::MalformedStructuralFoundation(
                "opaque representation lost its carrier storage",
            ),
        )?;
        if opaque.id == carrier.id
            || opaque.identity != relation.opaque_storage_identity
            || carrier.identity != relation.carrier_storage_identity
            || [opaque.identity, carrier.identity]
                .iter()
                .any(|identity| module.identities.text(*identity).is_none_or(str::is_empty))
        {
            return malformed("opaque representation substituted its exact storage identity");
        }
        relation
            .application
            .validate_identity_encoding()
            .map_err(CodecError::NativeLayout)?;
        if !same_shape(module, &opaque.shape, &carrier.shape)
            || opaque.scalar_restriction != carrier.scalar_restriction
        {
            return malformed(
                "opaque representation changed its selected carrier storage contract",
            );
        }
        // Nominal layouts use their retained exact provenance, never a name
        // inferred from a generic instance or a layout policy's spelling.
        let opaque_native = module
            .native_layouts
            .records()
            .find(|row| row.record == opaque.id);
        let carrier_native = module
            .native_layouts
            .records()
            .find(|row| row.record == carrier.id);
        match (opaque_native, carrier_native) {
            (None, None) => {}
            (Some(left), Some(right)) => {
                let left_fields = module
                    .native_layouts
                    .fields(left)
                    .map_err(CodecError::NativeLayout)?;
                let right_fields = module
                    .native_layouts
                    .fields(right)
                    .map_err(CodecError::NativeLayout)?;
                if left.data_identity != right.data_identity
                    || left.policy_identity != right.policy_identity
                    || left.semantic_identity != right.semantic_identity
                    || left_fields.len() != right_fields.len()
                    || left_fields.iter().zip(right_fields).any(|(left, right)| {
                        left.declaration_identity != right.declaration_identity
                    })
                    || module
                        .native_layouts
                        .realizations(left)
                        .map_err(CodecError::NativeLayout)?
                        != module
                            .native_layouts
                            .realizations(right)
                            .map_err(CodecError::NativeLayout)?
                {
                    return malformed(
                        "opaque representation changed its selected native layout provenance",
                    );
                }
            }
            _ => {
                return malformed(
                    "opaque representation lost its selected native layout provenance",
                );
            }
        }
    }
    Ok(())
}

fn same_fields(left: &[StructuralFieldDeclaration], right: &[StructuralFieldDeclaration]) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.identity == right.identity
                && left.relevance == right.relevance
                && left.field_type == right.field_type
                && left.storage == right.storage
                && left.inline_carrier == right.inline_carrier
        })
}

fn same_cases(left: &[StructuralCaseDeclaration], right: &[StructuralCaseDeclaration]) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.identity == right.identity && same_fields(&left.fields, &right.fields)
        })
}

fn same_shape(
    module: &TerminalModule,
    left: &StructuralTypeShape,
    right: &StructuralTypeShape,
) -> bool {
    match (left, right) {
        (
            StructuralTypeShape::Record {
                fields: left,
                storage: left_storage,
            },
            StructuralTypeShape::Record {
                fields: right,
                storage: right_storage,
            },
        ) => {
            same_fields(left, right)
                && same_record_storage(module, left, *left_storage, right, *right_storage)
        }
        (StructuralTypeShape::Sum { cases: left }, StructuralTypeShape::Sum { cases: right }) => {
            same_cases(left, right)
        }
        (
            StructuralTypeShape::Mixed {
                fields: left,
                cases: left_cases,
            },
            StructuralTypeShape::Mixed {
                fields: right,
                cases: right_cases,
            },
        ) => same_fields(left, right) && same_cases(left_cases, right_cases),
        _ => left == right,
    }
}

fn same_record_storage(
    module: &TerminalModule,
    left_fields: &[StructuralFieldDeclaration],
    left: StructuralRecordStorage,
    right_fields: &[StructuralFieldDeclaration],
    right: StructuralRecordStorage,
) -> bool {
    let (StructuralRecordStorage::TargetLayout(left), StructuralRecordStorage::TargetLayout(right)) =
        (left, right)
    else {
        return left == right;
    };
    let Some(mut normalized) = module
        .target_layout_applications
        .iter()
        .find(|row| row.id == left)
        .cloned()
    else {
        return false;
    };
    let Some(expected) = module
        .target_layout_applications
        .iter()
        .find(|row| row.id == right)
    else {
        return false;
    };
    // The producer allocates fresh record/member coordinates for the opaque
    // occurrence. Normalize only those coordinates; every policy, schema,
    // target input, placement, codec and retired-field fact must still match.
    normalized.id = expected.id;
    normalized.record = expected.record;
    let rebase = |identity| {
        let ordinal = left_fields.iter().position(|field| field.id == identity)?;
        right_fields.get(ordinal).map(|field| field.id)
    };
    for realization in &mut normalized.realizations {
        for field in &mut realization.schema_argument.fields {
            let Some(identity) = rebase(field.record_field) else {
                return false;
            };
            field.record_field = identity;
        }
        for field in &mut realization.fields {
            let Some(identity) = rebase(field.field) else {
                return false;
            };
            field.field = identity;
        }
    }
    normalized == *expected
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{
        AuthoredFieldStorage, AuthoredRecordStorage, StructuralFieldId, StructuralTypeId,
    };
    use terminal_psi::{
        BindingRelevance, LayoutApplicationId, StructuralFieldType, TargetLayoutApplication,
        TargetLayoutFieldStorage, TargetLayoutInput, TargetLayoutRealization,
        TargetLayoutSchemaArgument, TargetLayoutSchemaField, TargetLayoutSchemaFieldKind,
        TerminalIdentityIndex,
    };

    #[test]
    fn opaque_target_storage_compares_full_placements_after_coordinate_rebase() {
        let fields = |first| {
            [first, first + 1].map(|identity| StructuralFieldDeclaration {
                id: StructuralFieldId::new(identity).unwrap(),
                identity: TerminalIdentityIndex(identity as u32),
                relevance: BindingRelevance::Relevant,
                storage: None,
                inline_carrier: None,
                field_type: StructuralFieldType::Scalar(semantic_vocabulary::ScalarType::Boolean),
            })
        };
        let left = fields(1);
        let right = fields(3);
        let input = TargetLayoutInput {
            target: "linux_x86_64".into(),
            pointer_size: 8,
            pointer_alignment: 8,
            little_endian: true,
        };
        let application = |raw, fields: &[StructuralFieldDeclaration]| TargetLayoutApplication {
            id: LayoutApplicationId(raw),
            record: StructuralTypeId::new(u64::from(raw)).unwrap(),
            schema: StructuralTypeId::new(99).unwrap(),
            policy_identity: TerminalIdentityIndex(1),
            plan_identity: TerminalIdentityIndex(2),
            target_inputs: vec![input.clone()],
            realizations: vec![TargetLayoutRealization {
                target: input.target.clone(),
                pointer_size: 8,
                pointer_alignment: 8,
                little_endian: true,
                storage: AuthoredRecordStorage {
                    byte_size: 16,
                    byte_alignment: 8,
                },
                schema_argument: TargetLayoutSchemaArgument {
                    abi_version: 3,
                    retired_field_identities: vec![],
                    fields: fields
                        .iter()
                        .enumerate()
                        .map(|(ordinal, field)| TargetLayoutSchemaField {
                            record_field: field.id,
                            schema_field: StructuralFieldId::new(ordinal as u64 + 5).unwrap(),
                            key: ordinal as u64 + 1,
                            identity: None,
                            byte_size: 8,
                            byte_alignment: 8,
                            kind: TargetLayoutSchemaFieldKind::Scalar,
                        })
                        .collect(),
                },
                fields: fields
                    .iter()
                    .enumerate()
                    .map(|(ordinal, field)| TargetLayoutFieldStorage {
                        field: field.id,
                        storage: AuthoredFieldStorage::Direct {
                            byte_offset: ordinal as u64 * 8,
                        },
                    })
                    .collect(),
            }],
        };
        let mut module = TerminalModule::declarations();
        module.target_layout_applications = vec![application(1, &left), application(2, &right)];
        let matches = |module: &TerminalModule| {
            same_record_storage(
                module,
                &left,
                StructuralRecordStorage::TargetLayout(LayoutApplicationId(1)),
                &right,
                StructuralRecordStorage::TargetLayout(LayoutApplicationId(2)),
            )
        };
        assert!(matches(&module));
        let original = module.clone();
        let placements = &mut module.target_layout_applications[1].realizations[0].fields;
        placements[0].storage = AuthoredFieldStorage::Direct { byte_offset: 8 };
        placements[1].storage = AuthoredFieldStorage::Direct { byte_offset: 0 };
        assert!(
            !matches(&module),
            "equal size/alignment cannot justify swapped fields"
        );
        let mut module = original.clone();
        module.target_layout_applications[0].realizations[0].fields[0].field =
            StructuralFieldId::new(99).unwrap();
        assert!(
            !matches(&module),
            "unknown local coordinates cannot be rebased"
        );
        let mut module = original;
        module.target_layout_applications[1].target_inputs[0].little_endian = false;
        assert!(
            !matches(&module),
            "all independent target facts remain exact"
        );
    }
}
