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
            || module
                .identities
                .text(application.policy_identity)
                .is_none()
            || module.identities.text(application.plan_identity).is_none()
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
            || record.owns_linear_claim != schema.owns_linear_claim
            || record.copyable != schema.copyable
        {
            return malformed("layout schema differs from the semantic custody shape");
        }
        let live_fields = fields
            .iter()
            .filter(|field| !field.relevance.is_erased())
            .collect::<Vec<_>>();
        let live_schema = schema_fields
            .iter()
            .filter(|field| !field.relevance.is_erased())
            .collect::<Vec<_>>();
        if application.target_inputs.len() != application.realizations.len()
            || !strictly_increasing(
                application
                    .target_inputs
                    .iter()
                    .map(|input| input.target.as_str()),
            )
        {
            return malformed(
                "layout realizations do not cover the independent complete target input roster",
            );
        }
        let mut targets = BTreeSet::new();
        for (realization, input) in application
            .realizations
            .iter()
            .zip(&application.target_inputs)
        {
            if realization.target != input.target
                || realization.pointer_size != input.pointer_size
                || realization.pointer_alignment != input.pointer_alignment
                || realization.little_endian != input.little_endian
            {
                return malformed("layout realization changed its independent issuer target input");
            }
            if realization.schema_argument.abi_version != 3
                || realization.schema_argument.fields.len() > 32
                || live_fields.len() != realization.schema_argument.fields.len()
                || live_schema.len() != live_fields.len()
            {
                return malformed("layout schema argument does not cover exact live fields");
            }
            let retired = &realization.schema_argument.retired_field_identities;
            if retired.len() > 32
                || retired.iter().enumerate().any(|(index, identity)| {
                    retired[..index].contains(identity)
                        || realization
                            .schema_argument
                            .fields
                            .iter()
                            .any(|field| field.identity == Some(*identity))
                })
            {
                return malformed(
                    "layout schema argument has conflicting retired field identities",
                );
            }
            let inputs = &realization.schema_argument.fields;
            let numbered = inputs.iter().any(|field| field.identity.is_some());
            if (numbered
                && (inputs.iter().any(|field| field.identity.is_none())
                    || !strictly_increasing(inputs.iter().filter_map(|field| field.identity))))
                || !strictly_increasing(retired.iter().copied())
            {
                return malformed("layout schema argument is not in canonical identity order");
            }
            let mut keys = BTreeSet::new();
            let mut stable = BTreeSet::new();
            let mut captured = BTreeSet::new();
            for (ordinal, input) in inputs.iter().enumerate() {
                let Some(position) = live_fields
                    .iter()
                    .position(|field| field.id == input.record_field)
                else {
                    return malformed("layout schema argument names an absent record field");
                };
                let field = live_fields[position];
                let source = live_schema[position];
                if !captured.insert(input.record_field)
                    || (!numbered && position != ordinal)
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
                            schema_storage_layout(module, child, realization, &mut BTreeSet::new())
                        })
                        .map(|(size, align)| {
                            (
                                size,
                                align,
                                if source
                                    .field_type
                                    .as_structural()
                                    .and_then(|id| module.structural_type(id))
                                    .is_some_and(|ty| {
                                        matches!(ty.shape, StructuralTypeShape::FixedArray { .. })
                                    })
                                {
                                    TargetLayoutSchemaFieldKind::Repeated
                                } else {
                                    TargetLayoutSchemaFieldKind::Nested
                                },
                            )
                        })
                };
                if geometry != Some((input.byte_size, input.byte_alignment, input.kind))
                    || input.byte_size == 0
                {
                    return malformed(
                        "layout schema argument differs from invariant field geometry",
                    );
                }
            }
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

/// Reconstruct the reflected source geometry using the exact target's pointer
/// facts. A pointer never recursively demands its referent's storage. A nested
/// representation reads that child's admitted result for this same target.
fn schema_storage_layout(
    module: &TerminalModule,
    identity: StructuralTypeId,
    target: &terminal_psi::TargetLayoutRealization,
    visiting: &mut BTreeSet<StructuralTypeId>,
) -> Option<(u64, u64)> {
    use terminal_psi::ByteSequenceCarrier;
    if !visiting.insert(identity) {
        return None;
    }
    let result = (|| {
        let ty = module.structural_type(identity)?;
        match &ty.shape {
            StructuralTypeShape::Reference { .. } => {
                Some((target.pointer_size, target.pointer_alignment))
            }
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView { .. }) => {
                Some((target.pointer_size, target.pointer_alignment))
            }
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView { .. })
            | StructuralTypeShape::ElementView { .. } => Some((
                target.pointer_size.checked_mul(2)?,
                target.pointer_alignment,
            )),
            StructuralTypeShape::FixedArray { element, length } => {
                let (size, align) = schema_storage_layout(module, *element, target, visiting)?;
                Some((size.checked_mul(length.literal()?)?, align))
            }
            StructuralTypeShape::Record { fields, storage } => match storage {
                StructuralRecordStorage::TargetLayout(id) => module
                    .target_layout_applications
                    .iter()
                    .find(|application| application.id == *id && application.record == identity)?
                    .realizations
                    .iter()
                    .find(|row| row.target == target.target)
                    .map(|row| (row.storage.byte_size, row.storage.byte_alignment)),
                StructuralRecordStorage::Authored(storage) => {
                    Some((storage.byte_size, storage.byte_alignment))
                }
                StructuralRecordStorage::Packed => {
                    let mut offset = 0u64;
                    let mut alignment = 1u64;
                    for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                        let (size, align) = if let Some(scalar) = field.field_type.scalar_type() {
                            let size = u64::from(terminal_semantics::reference_storage_byte_width(
                                scalar,
                            )?);
                            (size, size)
                        } else {
                            schema_storage_layout(
                                module,
                                field.field_type.as_structural()?,
                                target,
                                visiting,
                            )?
                        };
                        offset = offset
                            .checked_add(align.checked_sub(1)?)?
                            .checked_div(align)?
                            .checked_mul(align)?
                            .checked_add(size)?;
                        alignment = alignment.max(align);
                    }
                    Some((
                        offset
                            .checked_add(alignment - 1)?
                            .checked_div(alignment)?
                            .checked_mul(alignment)?,
                        alignment,
                    ))
                }
                _ => None,
            },
            _ => terminal_semantics::reference_storage_layout(
                module.structural_types.iter(),
                identity,
            ),
        }
    })();
    visiting.remove(&identity);
    result
}
