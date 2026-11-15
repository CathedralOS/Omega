//! Keep the policy's evaluated placements when records enter Terminal Psi.
//! The semantic schema and represented record have distinct identities and
//! field namespaces; both the checker and native emission consume this same
//! existing layout application, before the module receives a verdict.

use semantic_vocabulary::{
    AuthoredFieldStorage, AuthoredRecordStorage, MachineId, StructuralFieldId,
};
use symbols::SymbolHandle;
use terminal_check_tables::{Checked, Construct, Refusal, RefusalReason};
use terminal_psi::{
    LayoutApplicationId, NativeLayoutCatalog, NativeLayoutFieldIdentity, NativeLayoutRealization,
    StructuralRecordStorage, StructuralTypeShape, TargetLayoutApplication,
    TargetLayoutFieldLayoutSource, TargetLayoutFieldStorage, TargetLayoutInput,
    TargetLayoutRealization, TargetLayoutSchemaArgument, TargetLayoutSchemaField,
    TargetLayoutSchemaFieldKind, TerminalModule,
};
use typed_trees::{PlanLaidLayout, PlanLaidRealizations, PlanLaidTargetLayout};

use super::{Program, malformed};

fn unavailable(machine: MachineId, detail: impl Into<String>) -> Refusal {
    Refusal {
        reason: RefusalReason::Unsupported,
        machine,
        concerns: Construct::Machine,
        source_span: None,
        detail: detail.into(),
    }
}

impl Program<'_> {
    pub(crate) fn publish_record_layouts(
        &self,
        machine: MachineId,
        module: &mut TerminalModule,
    ) -> Checked<()> {
        let typed = self.typed;
        if typed.plan_laid_layouts.is_empty() {
            return Ok(());
        }
        // Only actual applications participate in these joins. Prepare their
        // ordered coordinates once instead of scanning every plan per record.
        let mut plans = typed
            .plan_laid_layouts
            .iter()
            .enumerate()
            .map(|(ordinal, plan)| (typed.normalized_type_identity(plan.application), ordinal))
            .collect::<Vec<_>>();
        plans.sort_unstable_by_key(|(identity, _)| *identity);
        if plans.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(malformed(
                machine,
                "one storage application has multiple layouts",
            ));
        }
        let mut schemas = self
            .record_sources
            .iter()
            .filter_map(|(record, source)| {
                source.application.as_ref().map(|application| {
                    (
                        typed.normalized_type_identity(application.reference),
                        record,
                    )
                })
            })
            .collect::<Vec<_>>();
        schemas.sort_unstable_by_key(|(identity, _)| *identity);
        let mut identities = std::mem::take(&mut module.identities).into_builder();
        let previous = std::mem::take(&mut module.native_layouts);
        let mut native = NativeLayoutCatalog::default();
        let mut applications = Vec::new();
        for old in previous.records() {
            let source = terminal_check_tables::row(
                &self.record_sources,
                old.record,
                machine,
                Construct::Machine,
            )?;
            let selected = source.application.as_ref().and_then(|application| {
                plans
                    .binary_search_by_key(
                        &typed.normalized_type_identity(application.reference),
                        |(identity, _)| *identity,
                    )
                    .ok()
                    .map(|position| plans[position].1)
            });
            let Some(ordinal) = selected else {
                native.insert(
                    old.record,
                    old.data_identity,
                    old.policy_identity,
                    old.semantic_identity,
                    previous
                        .fields(old)
                        .map_err(|reason| unavailable(machine, reason))?
                        .iter()
                        .cloned(),
                    previous
                        .realizations(old)
                        .map_err(|reason| unavailable(machine, reason))?
                        .iter()
                        .cloned(),
                );
                continue;
            };
            let plan = &typed.plan_laid_layouts[ordinal];
            let data = typed
                .closed_data_definition(plan.semantic_type)
                .ok_or_else(|| malformed(machine, "layout lost its semantic declaration"))?;
            if data.symbol != source.declaration
                || plan.data_symbol != data.symbol
                || plan.schema_symbol != data.symbol
            {
                return Err(malformed(
                    machine,
                    "layout changed its retained semantic owner",
                ));
            }
            let semantic_application = typed.normalized_type_identity(plan.semantic_type);
            let first = schemas.partition_point(|(identity, _)| *identity < semantic_application);
            let last = schemas.partition_point(|(identity, _)| *identity <= semantic_application);
            if last - first != 1 {
                return Err(malformed(
                    machine,
                    "layout lost its unique retained semantic schema",
                ));
            }
            let schema = schemas[first].1;
            let record =
                terminal_check_tables::row(&self.records, old.record, machine, Construct::Machine)?;
            let schema_record =
                terminal_check_tables::row(&self.records, schema, machine, Construct::Machine)?;
            let schema_source = terminal_check_tables::row(
                &self.record_sources,
                schema,
                machine,
                Construct::Machine,
            )?;
            let fields = field_coordinates(&source.fields, record.fields(), machine)?;
            let schema_fields =
                field_coordinates(&schema_source.fields, schema_record.fields(), machine)?;
            let policy = plan
                .native_policy_identity(typed)
                .map_err(|reason| unavailable(machine, reason))?;
            let semantic = plan
                .native_semantic_identity(typed)
                .map_err(|reason| unavailable(machine, reason))?;
            let data_identity = plan
                .native_data_identity(typed, data, &policy)
                .map_err(|reason| unavailable(machine, reason))?;
            let policy_identity = identities.intern(&policy);
            // The complete storage application retains both the semantic type
            // and selected policy tuple; a bare member name would lose them.
            let plan_identity = identities.intern(&record.identity);
            let ordinal = u32::try_from(ordinal)
                .ok()
                .and_then(|index| index.checked_add(1))
                .ok_or_else(|| malformed(machine, "layout application identity overflows"))?;
            let id = LayoutApplicationId(ordinal);
            let PlanLaidRealizations::TargetInputs(span) = plan.realizations else {
                return Err(unavailable(
                    machine,
                    "layout has no retained target input roster",
                ));
            };
            let mut members = typed
                .plan_laid_target_layouts
                .span(span)
                .ok_or_else(|| malformed(machine, "layout target members are unavailable"))?
                .iter()
                .collect::<Vec<_>>();
            members.sort_unstable_by(|left, right| left.target.as_str().cmp(right.target.as_str()));
            let mut inputs = plan
                .target_inputs
                .iter()
                .map(|facts| TargetLayoutInput {
                    target: facts.profile.clone(),
                    pointer_size: facts.pointer_size,
                    pointer_alignment: facts.pointer_alignment,
                    case_tag_size: facts.case_tag_size,
                    case_tag_alignment: facts.case_tag_alignment,
                    little_endian: facts.little_endian,
                })
                .collect::<Vec<_>>();
            inputs.sort_unstable_by(|left, right| left.target.cmp(&right.target));
            if inputs.is_empty()
                || inputs.len() != members.len()
                || inputs
                    .windows(2)
                    .any(|pair| pair[0].target == pair[1].target)
            {
                return Err(malformed(
                    machine,
                    "layout target roster is absent or duplicated",
                ));
            }
            let mut realizations = Vec::with_capacity(members.len());
            let mut reports = Vec::with_capacity(members.len());
            for (input, member) in inputs.iter().zip(members) {
                if member.target.as_str() != input.target
                    || member.target_facts.profile != input.target
                    || member.target_facts.pointer_size != input.pointer_size
                    || member.target_facts.pointer_alignment != input.pointer_alignment
                    || member.target_facts.case_tag_size != input.case_tag_size
                    || member.target_facts.case_tag_alignment != input.case_tag_alignment
                    || member.target_facts.little_endian != input.little_endian
                {
                    return Err(malformed(
                        machine,
                        "layout target member changed its producing inputs",
                    ));
                }
                realizations.push(realization(
                    typed,
                    plan,
                    member,
                    &fields,
                    &schema_fields,
                    input,
                    machine,
                )?);
                reports.push(NativeLayoutRealization {
                    target: Some(input.clone()),
                    report: member.geometry.native_layout_report(),
                });
            }
            let mut nominal = Vec::with_capacity(source.fields.len());
            for (field, symbol) in record.fields().iter().zip(&source.fields) {
                let identity = typed
                    .normalized_hermetic_symbol_identity(*symbol)
                    .map_err(|reason| unavailable(machine, reason))?;
                nominal.push(NativeLayoutFieldIdentity {
                    field: field.id,
                    declaration_identity: identities.intern(&identity),
                });
            }
            native.insert(
                old.record,
                identities.intern(&data_identity),
                policy_identity,
                identities.intern(&semantic),
                nominal,
                reports,
            );
            let position = usize::try_from(old.record.get() - 1)
                .map_err(|_| malformed(machine, "record coordinate exceeds host storage"))?;
            let declaration = module
                .structural_types
                .get_mut(position)
                .filter(|declaration| declaration.id == old.record)
                .ok_or_else(|| {
                    malformed(machine, "layout record is outside its structural owner")
                })?;
            let StructuralTypeShape::Record { storage, .. } = &mut declaration.shape else {
                return Err(malformed(machine, "layout application is not a record"));
            };
            *storage = StructuralRecordStorage::TargetLayout(id);
            applications.push(TargetLayoutApplication {
                id,
                record: old.record,
                schema,
                policy_identity,
                plan_identity,
                target_inputs: inputs,
                realizations,
            });
        }
        applications.sort_unstable_by_key(|application| application.id);
        module.identities = identities.build();
        module.native_layouts = native;
        module.target_layout_applications = applications;
        Ok(())
    }
}

fn field_coordinates(
    symbols: &[SymbolHandle],
    fields: &[crate::body::RecordField],
    machine: MachineId,
) -> Checked<Vec<(SymbolHandle, StructuralFieldId)>> {
    if symbols.len() != fields.len() {
        return Err(malformed(machine, "layout source and field rosters differ"));
    }
    let mut coordinates = symbols
        .iter()
        .copied()
        .zip(fields.iter().map(|field| field.id))
        .collect::<Vec<_>>();
    coordinates.sort_unstable_by_key(|(symbol, _)| (symbol.arena_index(), symbol.generation()));
    if coordinates.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(malformed(machine, "layout source field is duplicated"));
    }
    Ok(coordinates)
}

fn field(
    coordinates: &[(SymbolHandle, StructuralFieldId)],
    symbol: SymbolHandle,
    machine: MachineId,
) -> Checked<StructuralFieldId> {
    coordinates
        .binary_search_by_key(
            &(symbol.arena_index(), symbol.generation()),
            |(symbol, _)| (symbol.arena_index(), symbol.generation()),
        )
        .ok()
        .map(|index| coordinates[index].1)
        .ok_or_else(|| malformed(machine, "layout field has no exact source correspondence"))
}

fn realization(
    typed: &typed_trees::TypedTrees,
    plan: &PlanLaidLayout,
    member: &PlanLaidTargetLayout,
    fields: &[(SymbolHandle, StructuralFieldId)],
    schema_fields: &[(SymbolHandle, StructuralFieldId)],
    input: &TargetLayoutInput,
    machine: MachineId,
) -> Checked<TargetLayoutRealization> {
    let schema = member
        .schema_argument
        .ok_or_else(|| malformed(machine, "layout target lost its evaluated schema"))?;
    let source = typed
        .plan_laid_schema_fields
        .span(schema.fields)
        .ok_or_else(|| malformed(machine, "layout schema fields are unavailable"))?;
    let retired = typed
        .plan_laid_retired_field_identities
        .span(schema.retired_field_identities)
        .ok_or_else(|| malformed(machine, "layout retired identities are unavailable"))?;
    let offsets = member
        .geometry
        .validated_layout
        .offsets
        .as_ref()
        .ok_or_else(|| unavailable(machine, "layout has no complete fixed field placements"))?;
    if source.len() != fields.len()
        || offsets.len() != source.len()
        || plan.field_symbols.len() != source.len()
        || plan.schema_field_symbols.len() != source.len()
    {
        return Err(malformed(
            machine,
            "layout placements and semantic fields differ",
        ));
    }
    let mut arguments = Vec::with_capacity(source.len());
    let mut placements = Vec::with_capacity(source.len());
    for ((source, offset), (record_symbol, schema_symbol)) in source
        .iter()
        .zip(offsets)
        .zip(plan.field_symbols.iter().zip(&plan.schema_field_symbols))
    {
        if source.schema_field != *schema_symbol {
            return Err(malformed(machine, "layout key changed its schema field"));
        }
        let record_field = field(fields, *record_symbol, machine)?;
        arguments.push(TargetLayoutSchemaField {
            record_field,
            schema_field: field(schema_fields, *schema_symbol, machine)?,
            key: source.key,
            identity: source.identity,
            byte_size: source.byte_size,
            byte_alignment: source.byte_alignment,
            kind: match source.kind {
                typed_trees::PlanLaidSchemaFieldKind::Scalar => TargetLayoutSchemaFieldKind::Scalar,
                typed_trees::PlanLaidSchemaFieldKind::Nested => TargetLayoutSchemaFieldKind::Nested,
                typed_trees::PlanLaidSchemaFieldKind::Repeated => {
                    TargetLayoutSchemaFieldKind::Repeated
                }
            },
            layout_source: match source.layout_source {
                typed_trees::PlanLaidFieldLayoutSource::Default => {
                    TargetLayoutFieldLayoutSource::Default
                }
                typed_trees::PlanLaidFieldLayoutSource::Scalar => {
                    TargetLayoutFieldLayoutSource::Scalar
                }
                typed_trees::PlanLaidFieldLayoutSource::Explicit => {
                    TargetLayoutFieldLayoutSource::Explicit
                }
            },
        });
        placements.push(TargetLayoutFieldStorage {
            field: record_field,
            storage: AuthoredFieldStorage::Direct {
                byte_offset: *offset,
            },
        });
    }
    placements.sort_unstable_by_key(|placement| placement.field);
    let mut expected_fields = fields.iter().map(|(_, field)| *field).collect::<Vec<_>>();
    expected_fields.sort_unstable();
    if placements
        .iter()
        .map(|placement| placement.field)
        .ne(expected_fields)
    {
        return Err(malformed(
            machine,
            "layout does not cover the exact represented fields",
        ));
    }
    Ok(TargetLayoutRealization {
        schema_argument: TargetLayoutSchemaArgument {
            abi_version: schema.abi_version,
            fields: arguments,
            retired_field_identities: retired.iter().map(|field| field.identity).collect(),
        },
        target: input.target.clone(),
        pointer_size: input.pointer_size,
        pointer_alignment: input.pointer_alignment,
        case_tag_size: input.case_tag_size,
        case_tag_alignment: input.case_tag_alignment,
        little_endian: input.little_endian,
        storage: AuthoredRecordStorage {
            byte_size: member
                .geometry
                .validated_layout
                .size
                .ok_or_else(|| unavailable(machine, "layout has no fixed complete extent"))?,
            byte_alignment: member.geometry.validated_layout.align,
        },
        fields: placements,
    })
}
