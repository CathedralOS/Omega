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
        if !same_shape(&opaque.shape, &carrier.shape)
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

fn same_shape(left: &StructuralTypeShape, right: &StructuralTypeShape) -> bool {
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
                && (left_storage == right_storage
                    || matches!(
                        (left_storage, right_storage),
                        (
                            StructuralRecordStorage::TargetLayout(_),
                            StructuralRecordStorage::TargetLayout(_)
                        )
                    ))
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
