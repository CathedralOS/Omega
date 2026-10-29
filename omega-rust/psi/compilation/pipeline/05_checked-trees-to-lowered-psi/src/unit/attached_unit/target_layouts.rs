//! Publish exact guarded layout folds in the emitted structural/callable namespace.
//!
//! The checked marker owns application and schema identity. Its typed record
//! supplies the exact invariant Schema argument and every admitted member's
//! geometry; neither native target selection nor name-based reconstruction is
//! part of this join.

use super::{CheckedTrees, LoweringError, UnitPlans, unsupported};
use checked_trees::{CheckedUnitRecordStorage, CheckedUnitStructuralTypeShape};
use semantic_vocabulary::{AuthoredRecordStorage, MachineId, StructuralTypeId};
use symbols::SymbolHandle;
use terminal_psi::{
    StructuralFieldDeclaration, StructuralRecordStorage, StructuralTypeDeclaration,
    StructuralTypeShape, TargetLayoutApplication, TargetLayoutFieldStorage,
    TargetLayoutRealization, TargetLayoutSchemaArgument, TargetLayoutSchemaField,
    TargetLayoutSchemaFieldKind,
};
use typed_trees::type_identity::TypeIdentityId;

pub(super) fn lower(
    checked: &CheckedTrees,
    plans: UnitPlans<'_>,
    types: &[StructuralTypeDeclaration],
    type_ids: &[(TypeIdentityId, StructuralTypeId)],
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
) -> Result<Vec<TargetLayoutApplication>, LoweringError> {
    let mut applications = Vec::new();
    for record in types {
        let StructuralTypeShape::Record {
            fields,
            storage: StructuralRecordStorage::TargetLayout(id),
        } = &record.shape
        else {
            continue;
        };
        let type_identity = type_ids
            .iter()
            .find_map(|(identity, id)| (*id == record.id).then_some(*identity))
            .ok_or(LoweringError::Unsupported(
                "target layout record lost its exact type identity mapping",
            ))?;
        let checked_record = super::catalog::selected_declaration(plans, type_identity)?;
        let CheckedUnitStructuralTypeShape::Record {
            storage:
                CheckedUnitRecordStorage::TargetLayout {
                    application,
                    schema_type_identity,
                },
            ..
        } = &checked_record.shape
        else {
            return unsupported("target layout record lost its exact checked application marker");
        };
        let plan = checked
            .typed
            .plan_laid_layout_for_type(*application)
            .ok_or(LoweringError::Unsupported(
                "target layout application lost its typed policy result",
            ))?;
        let typed_trees::PlanLaidRealizations::TargetInputs(span) = plan.realizations else {
            return unsupported("target layout marker names an invariant application");
        };
        let alternatives =
            checked
                .typed
                .plan_laid_target_layouts
                .span(span)
                .ok_or(LoweringError::Unsupported(
                    "target layout application lost its alternative span",
                ))?;
        let schema_id = crate::terminal_identities::lookup_type_id(type_ids, schema_type_identity)?;
        let schema = types
            .iter()
            .find(|candidate| candidate.id == schema_id)
            .ok_or(LoweringError::Unsupported(
                "target layout application lost its retained schema",
            ))?;
        let StructuralTypeShape::Record {
            fields: schema_fields,
            ..
        } = &schema.shape
        else {
            return unsupported("target layout Schema ABI requires a record schema");
        };
        let record_fields = live_fields(fields);
        let schema_fields = live_fields(schema_fields);
        validate_field_inventory(
            checked,
            plan.data_symbol,
            &plan.field_symbols,
            &record_fields,
        )?;
        validate_field_inventory(
            checked,
            plan.schema_symbol,
            &plan.schema_field_symbols,
            &schema_fields,
        )?;
        let argument = plan
            .schema_argument
            .as_ref()
            .ok_or(LoweringError::Unsupported(
                "target layout application lost its exact Schema argument",
            ))?;
        if argument.abi_version != 1 {
            return unsupported("target layout application uses an unsupported Schema ABI version");
        }
        let argument_fields = checked
            .typed
            .plan_laid_schema_fields
            .span(argument.fields)
            .ok_or(LoweringError::Unsupported(
                "target layout application lost its Schema argument span",
            ))?;
        if argument_fields.len() != record_fields.len()
            || argument_fields.len() != schema_fields.len()
        {
            return unsupported("unsupported target layout Schema argument field inventory");
        }
        let mut captured_fields = Vec::with_capacity(argument_fields.len());
        for (position, field) in argument_fields.iter().enumerate() {
            if field.schema_field != plan.schema_field_symbols[position] {
                return unsupported(
                    "target layout Schema argument changed exact field identity/order",
                );
            }
            captured_fields.push(TargetLayoutSchemaField {
                record_field: record_fields[position].id,
                schema_field: schema_fields[position].id,
                key: field.key,
                identity: field.identity,
                byte_size: field.byte_size,
                byte_alignment: field.byte_alignment,
                kind: match field.kind {
                    typed_trees::PlanLaidSchemaFieldKind::Scalar => {
                        TargetLayoutSchemaFieldKind::Scalar
                    }
                    typed_trees::PlanLaidSchemaFieldKind::Nested => {
                        TargetLayoutSchemaFieldKind::Nested
                    }
                },
            });
        }
        let policy_identity = identities.intern(
            &checked
                .typed
                .package_qualified_type_identity_text(plan.application),
        );
        let policy_machine = checked
            .typed
            .machine_by_symbol(plan.policy_plan_machine_symbol)
            .ok_or(LoweringError::Unsupported(
                "fixed representation lost its exact checked plan",
            ))?;
        let plan_identity = identities.intern(
            &validation::contract_identities::normalized_machine_identity(
                &checked.typed,
                policy_machine,
            )
            .ok_or(LoweringError::Unsupported(
                "fixed representation policy has no checked callable identity",
            ))?,
        );
        if alternatives.is_empty() {
            return unsupported("fixed representation omitted its target inputs");
        }
        let mut realizations = Vec::with_capacity(alternatives.len());
        for alternative in alternatives {
            if alternative.machine != plan.policy_plan_machine_symbol
                || alternative.target.as_str() != alternative.target_facts.profile
                || alternative.target_facts.validate().is_err()
            {
                return unsupported("fixed representation changed its exact target input custody");
            }
            let geometry = &alternative.geometry;
            if !geometry.private_callback_demands.is_empty() {
                return unsupported(
                    "target layout publication cannot discard private callback demands",
                );
            }
            let byte_size = geometry
                .validated_layout
                .size
                .ok_or(LoweringError::Unsupported(
                    "target layout alternative has no fixed extent",
                ))?;
            if usize::try_from(byte_size).ok() != Some(geometry.size)
                || usize::try_from(geometry.validated_layout.align).ok() != Some(geometry.align)
                || geometry.offsets.len() != record_fields.len()
            {
                return unsupported("unsupported target layout alternative geometry");
            }
            let fields = record_fields
                .iter()
                .enumerate()
                .map(|(position, field)| {
                    Ok(TargetLayoutFieldStorage {
                        field: field.id,
                        storage: geometry
                            .authored_field_storage(
                                position,
                                field.field_type.scalar_type().is_some(),
                            )
                            .ok_or(LoweringError::Unsupported(
                                "target layout field has no exact supported storage",
                            ))?,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            realizations.push(TargetLayoutRealization {
                target: alternative.target.as_str().to_owned(),
                pointer_size: alternative.target_facts.pointer_size,
                pointer_alignment: alternative.target_facts.pointer_alignment,
                little_endian: alternative.target_facts.little_endian,
                storage: AuthoredRecordStorage {
                    byte_size,
                    byte_alignment: geometry.validated_layout.align,
                },
                fields,
            });
        }
        realizations.sort_by(|a, b| a.target.cmp(&b.target));
        applications.push(TargetLayoutApplication {
            id: *id,
            record: record.id,
            schema: schema.id,
            policy_identity,
            plan_identity,
            schema_argument: TargetLayoutSchemaArgument {
                abi_version: argument.abi_version,
                fields: captured_fields,
            },
            realizations,
        });
    }
    applications.sort_by_key(|application| application.id);
    Ok(applications)
}

fn live_fields(fields: &[StructuralFieldDeclaration]) -> Vec<&StructuralFieldDeclaration> {
    fields
        .iter()
        .filter(|field| !field.relevance.is_erased())
        .collect()
}

fn validate_field_inventory(
    checked: &CheckedTrees,
    owner: SymbolHandle,
    symbols: &[SymbolHandle],
    fields: &[&StructuralFieldDeclaration],
) -> Result<(), LoweringError> {
    let data = checked
        .typed
        .data_definition_by_symbol(owner)
        .ok_or(LoweringError::Unsupported(
            "target layout lost an exact data declaration",
        ))?;
    if symbols.len() != fields.len()
        || symbols.iter().copied().ne(checked
            .typed
            .live_data_fields(checked.typed.data_members(data))
            .map(|field| field.symbol))
    {
        return unsupported("unsupported target layout source/emitted live field inventory");
    }
    Ok(())
}
