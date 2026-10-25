//! Reconstruct layout catalog associations and validate every retained geometry.
//! The checked producer owns policy evaluation; these checks grant no execution
//! authority and do not pretend to replay an omitted source computation.
use crate::codec_error::{CodecError, malformed};
use semantic_vocabulary::StructuralTypeId;
use std::collections::BTreeSet;
use terminal_psi::{
    StructuralRecordStorage, StructuralTypeShape, TargetLayoutPolicy, TargetLayoutSchemaFieldKind,
    TargetMachineCallable, TargetMachineSupply, TerminalIdentityIndex, TerminalModule,
};

pub(super) fn validate(
    module: &TerminalModule,
    attachments: &BTreeSet<StructuralTypeId>,
) -> Result<(), CodecError> {
    if module
        .target_layout_applications
        .windows(2)
        .any(|pair| pair[0].id >= pair[1].id)
    {
        return malformed("layout applications are not strictly ordered");
    }
    let mut records = BTreeSet::new();
    for application in &module.target_layout_applications {
        if application.id.0 == 0
            || application.policy_identity == TerminalIdentityIndex::ABSENT
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
        let expected = match &application.policy {
            TargetLayoutPolicy::Body { machine, contexts } => {
                let Some(declaration) = module
                    .machines
                    .iter()
                    .find(|body| body.id == *machine)
                    .and_then(|body| body.declaration.as_ref())
                else {
                    return malformed("layout application has no exact checked policy body");
                };
                if declaration.semantic_declaration != application.policy_identity
                    || contexts.is_empty()
                    || contexts.iter().any(String::is_empty)
                    || !contexts.windows(2).all(|pair| pair[0] < pair[1])
                {
                    return malformed("layout policy body has stale identity or context roster");
                }
                contexts
                    .iter()
                    .map(|target| (target.as_str(), *machine))
                    .collect::<Vec<_>>()
            }
            TargetLayoutPolicy::Family(canonical) => {
                let Some(family) = module
                    .target_machine_families
                    .iter()
                    .find(|family| family.canonical == *canonical)
                else {
                    return malformed("layout application has no exact target family");
                };
                let TargetMachineCallable::Boundary(head) = *canonical else {
                    return malformed("layout application has no independent policy contract head");
                };
                let Some(policy) =
                    terminal_psi::find_boundary_machine(&module.boundary_machines, head)
                else {
                    return malformed("layout application has no policy contract declaration");
                };
                if policy.identity != application.policy_identity {
                    return malformed(
                        "layout application policy identity differs from its exact head",
                    );
                }
                let mut expected = Vec::with_capacity(family.members.len());
                for member in &family.members {
                    let TargetMachineCallable::Machine(machine) = member.callable else {
                        return malformed("layout policy family has no exact checked body");
                    };
                    if member.supply != TargetMachineSupply::CheckedBody {
                        return malformed("layout policy family member has no checked supply");
                    }
                    expected.push((member.target.as_str(), machine));
                }
                expected
            }
        };
        if application.realizations.len() != expected.len() || application.realizations.is_empty() {
            return malformed("layout application omits family alternatives");
        }
        for (realization, (target, machine)) in application.realizations.iter().zip(&expected) {
            if realization.target != *target
                || realization.member != *machine
                || realization.fields.len() != live_fields.len()
                || realization
                    .fields
                    .iter()
                    .zip(&live_fields)
                    .any(|(stored, field)| stored.field != field.id)
            {
                return malformed(
                    "layout alternative differs from its exact checked family member or fields",
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
