//! Reconstruct layout catalog associations and validate every retained geometry.
//! The checked producer owns policy evaluation; these checks grant no execution
//! authority and do not pretend to replay an omitted source computation.
use super::super::wire::strictly_increasing;
use crate::codec_error::{CodecError, malformed};
use semantic_vocabulary::StructuralTypeId;
use std::collections::BTreeSet;
use terminal_psi::{
    StructuralRecordStorage, StructuralTypeShape, TargetLayoutSchemaFieldKind,
    TerminalIdentityIndex, TerminalModule,
};

pub(super) fn validate(
    module: &TerminalModule,
    attachments: &BTreeSet<StructuralTypeId>,
) -> Result<(), CodecError> {
    if !strictly_increasing(
        module
            .target_layout_applications
            .iter()
            .map(|application| application.id),
    ) {
        return malformed("layout applications are not strictly ordered");
    }
    let mut records = BTreeSet::new();
    for application in &module.target_layout_applications {
        if application.id.0 == 0
            || application.policy_identity == TerminalIdentityIndex::ABSENT
            || application.plan_identity == TerminalIdentityIndex::ABSENT
            || !records.insert(application.record)
        {
            return malformed("layout application has absent or duplicate identity");
        }
        let Some(record) = module.structural_type(application.record) else {
            return malformed("layout application has no record");
        };
        let StructuralTypeShape::Record {
            fields,
            storage: StructuralRecordStorage::TargetLayout(id),
        } = &record.shape
        else {
            return malformed("layout application has no exact pending record marker");
        };
        if *id != application.id {
            return malformed("layout application record marker differs");
        }
        let Some(schema) = module.structural_type(application.schema) else {
            return malformed("layout application has no semantic schema");
        };
        let StructuralTypeShape::Record {
            fields: schema_fields,
            storage,
        } = &schema.shape
        else {
            return malformed("layout schema ABI supports records only");
        };
        if matches!(storage, StructuralRecordStorage::TargetLayout(_))
            || application.schema_argument.abi_version != 1
            || application.schema_argument.fields.len() > 32
        {
            return malformed("layout schema argument is not an invariant supported ABI");
        }
        let live_fields = fields
            .iter()
            .filter(|field| !field.relevance.is_erased())
            .collect::<Vec<_>>();
        let live_schema = schema_fields
            .iter()
            .filter(|field| !field.relevance.is_erased())
            .collect::<Vec<_>>();
        if live_fields.len() != application.schema_argument.fields.len()
            || live_schema.len() != live_fields.len()
        {
            return malformed("layout schema argument does not cover exact live fields");
        }
        let mut keys = BTreeSet::new();
        let mut stable = BTreeSet::new();
        for ((input, field), source) in application
            .schema_argument
            .fields
            .iter()
            .zip(&live_fields)
            .zip(&live_schema)
        {
            if input.record_field != field.id
                || input.schema_field != source.id
                || field.field_type != source.field_type
                || input.key == 0
                || !keys.insert(input.key)
                || input
                    .identity
                    .is_some_and(|identity| !stable.insert(identity))
            {
                return malformed("layout schema argument has stale field correspondence");
            }
            let geometry = if let Some(scalar) = source.field_type.scalar_type() {
                terminal_semantics::reference_storage_byte_width(scalar).map(|width| {
                    (
                        u64::from(width),
                        u64::from(width),
                        TargetLayoutSchemaFieldKind::Scalar,
                    )
                })
            } else {
                source
                    .field_type
                    .as_structural()
                    .and_then(|child| {
                        terminal_semantics::reference_storage_layout(
                            module.structural_types.iter(),
                            child,
                        )
                    })
                    .map(|(size, align)| (size, align, TargetLayoutSchemaFieldKind::Nested))
            };
            if geometry != Some((input.byte_size, input.byte_alignment, input.kind))
                || input.byte_size == 0
            {
                return malformed("layout schema argument differs from invariant field geometry");
            }
        }
        if application.realizations.is_empty() {
            return malformed("fixed representation omits its target inputs");
        }
        let mut targets = BTreeSet::new();
        for realization in &application.realizations {
            if realization.target.is_empty()
                || !targets.insert(&realization.target)
                || realization.pointer_size == 0
                || !realization.pointer_alignment.is_power_of_two()
                || realization.pointer_size % realization.pointer_alignment != 0
                || realization.fields.len() != live_fields.len()
                || realization
                    .fields
                    .iter()
                    .zip(&live_fields)
                    .any(|(stored, field)| stored.field != field.id)
            {
                return malformed(
                    "fixed representation changed its target input or exact semantic fields",
                );
            }
            let mut resolved = record.clone();
            let StructuralTypeShape::Record { fields, storage } = &mut resolved.shape else {
                unreachable!()
            };
            *storage = StructuralRecordStorage::Authored(realization.storage);
            for field in fields
                .iter_mut()
                .filter(|field| !field.relevance.is_erased())
            {
                field.storage = realization
                    .fields
                    .iter()
                    .find(|stored| stored.field == field.id)
                    .map(|stored| stored.storage.clone());
            }
            super::structural_type_foundations::validate_type_shape(
                module,
                &resolved,
                attachments,
            )?;
        }
    }
    for declaration in &module.structural_types {
        if let StructuralTypeShape::Record {
            storage: StructuralRecordStorage::TargetLayout(application),
            ..
        } = declaration.shape
            && !module
                .target_layout_applications
                .iter()
                .any(|row| row.id == application && row.record == declaration.id)
        {
            return malformed("pending record has no exact layout application");
        }
    }
    Ok(())
}
