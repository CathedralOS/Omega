//! Publish exact guarded layout folds in the emitted structural/callable namespace.
//!
//! The checked marker owns application and schema identity. Its typed record
//! supplies the exact invariant Schema argument and every admitted member's
//! geometry; neither native target selection nor name-based reconstruction is
//! part of this join.

use super::{BoundaryParameters, CheckedTrees, LoweringError, UnitPlans, unsupported};
use checked_trees::{CheckedUnitRecordStorage, CheckedUnitStructuralTypeShape};
use semantic_vocabulary::{AuthoredRecordStorage, MachineId, StructuralTypeId};
use symbols::SymbolHandle;
use terminal_psi::{
    BoundaryMachineDeclaration, StructuralFieldDeclaration, StructuralRecordStorage,
    StructuralTypeDeclaration, StructuralTypeShape, TargetLayoutApplication,
    TargetLayoutFieldStorage, TargetLayoutPolicy, TargetLayoutRealization,
    TargetLayoutSchemaArgument, TargetLayoutSchemaField, TargetLayoutSchemaFieldKind,
    TargetMachineCallable, TargetMachineFamily, TargetMachineSupply,
};
use typed_trees::type_identity::TypeIdentityId;

pub(super) fn lower(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    plans: UnitPlans<'_>,
    types: &[StructuralTypeDeclaration],
    type_ids: &[(TypeIdentityId, StructuralTypeId)],
    machines: &[(SymbolHandle, MachineId)],
    boundary_parameters: &[BoundaryParameters],
    boundaries: &[BoundaryMachineDeclaration],
    families: &[TargetMachineFamily],
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
        let plan =
            checked
                .typed
                .plan_laid_layout(*application)
                .ok_or(LoweringError::Unsupported(
                    "target layout application lost its typed policy result",
                ))?;
        let span = match plan.realizations {
            typed_trees::PlanLaidRealizations::TargetFamily(span)
            | typed_trees::PlanLaidRealizations::TargetContexts { layouts: span, .. } => span,
            _ => return unsupported("target layout marker names an unguarded application"),
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
        let (policy, policy_identity, expected) =
            if let typed_trees::PlanLaidRealizations::TargetContexts { contexts, .. } =
                plan.realizations
            {
                let contexts = checked.typed.plan_laid_contexts.span(contexts).ok_or(
                    LoweringError::Unsupported("target layout body lost its declared contexts"),
                )?;
                let callable = super::target_families::callable(
                    plan.policy_plan_machine_symbol,
                    machines,
                    boundary_parameters,
                )?;
                let TargetMachineCallable::Machine(machine) = callable else {
                    return unsupported("contextual layout policy has no exact checked body");
                };
                // Body declarations are attached after closure assembly. Use
                // their existing identity producer now; final codec admission
                // joins this identity to that exact emitted declaration.
                let declaration = crate::machine_lowering::declarations::identity(
                    identities,
                    checked,
                    exact_toolchain_sources,
                    plan.policy_plan_machine_symbol,
                )
                .map_err(LoweringError::InvalidDeclarationIdentity)?;
                let expected = contexts
                    .iter()
                    .map(|target| {
                        (
                            target.as_str().to_owned(),
                            plan.policy_plan_machine_symbol,
                            callable,
                        )
                    })
                    .collect::<Vec<_>>();
                (
                    TargetLayoutPolicy::Body {
                        machine,
                        contexts: contexts
                            .iter()
                            .map(|target| target.as_str().to_owned())
                            .collect(),
                    },
                    declaration.semantic_declaration,
                    expected,
                )
            } else {
                let typed_family = checked
                    .typed
                    .target_machine_families
                    .iter()
                    .find(|family| family.canonical == plan.policy_plan_machine_symbol)
                    .ok_or(LoweringError::Unsupported(
                        "target layout application lost its policy family",
                    ))?;
                let canonical = super::target_families::callable(
                    typed_family.canonical,
                    machines,
                    boundary_parameters,
                )?;
                let family = families
                    .iter()
                    .find(|family| family.canonical == canonical)
                    .ok_or(LoweringError::Unsupported(
                        "target layout policy family was not emitted",
                    ))?;
                let TargetMachineCallable::Boundary(head) = canonical else {
                    return unsupported(
                        "target layout family has no independent emitted contract head",
                    );
                };
                let policy_identity = boundaries
                    .iter()
                    .find(|boundary| boundary.id == head)
                    .map(|boundary| boundary.identity)
                    .ok_or(LoweringError::Unsupported(
                        "target layout policy head lost its emitted identity",
                    ))?;
                if typed_family.members.len() != family.members.len() {
                    return unsupported(
                        "target layout application does not cover its complete policy family",
                    );
                }
                let mut expected = Vec::with_capacity(family.members.len());
                for (typed_member, member) in typed_family.members.iter().zip(&family.members) {
                    if typed_member.target.as_str() != member.target
                        || member.supply != TargetMachineSupply::CheckedBody
                    {
                        return unsupported("target layout family lost its exact checked roster");
                    }
                    expected.push((member.target.clone(), typed_member.machine, member.callable));
                }
                (
                    TargetLayoutPolicy::Family(canonical),
                    policy_identity,
                    expected,
                )
            };
        if alternatives.is_empty() || alternatives.len() != expected.len() {
            return unsupported("target layout application omits a declared evaluation context");
        }
        let mut realizations = Vec::with_capacity(alternatives.len());
        for (alternative, (target, source_machine, expected_callable)) in
            alternatives.iter().zip(&expected)
        {
            let callable = super::target_families::callable(
                alternative.machine,
                machines,
                boundary_parameters,
            )?;
            if alternative.target.as_str() != target
                || alternative.machine != *source_machine
                || callable != *expected_callable
            {
                return unsupported(
                    "target layout alternative changed exact family member custody",
                );
            }
            let TargetMachineCallable::Machine(machine) = callable else {
                return unsupported("target layout policy alternative has no emitted checked body");
            };
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
                member: machine,
                storage: AuthoredRecordStorage {
                    byte_size,
                    byte_alignment: geometry.validated_layout.align,
                },
                fields,
            });
        }
        applications.push(TargetLayoutApplication {
            id: *id,
            record: record.id,
            schema: schema.id,
            policy_identity,
            policy,
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
