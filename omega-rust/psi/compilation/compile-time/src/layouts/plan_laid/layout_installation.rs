use diagnostics::Diagnostic;
use layout_plans::LayoutPlacementReport;
use typed_trees::{
    PlanLaidBitField, PlanLaidBitFragment, PlanLaidGeometry, PlanLaidIntegerField, PlanLaidLayout,
    PlanLaidRealizations, PlanLaidRepeatedField, PlanLaidSchemaArgument, PlanLaidSchemaField,
    PlanLaidSchemaFieldKind, PlanLaidTargetLayout, TypedTrees,
};

use super::PlanLaidRecord;

pub(crate) fn compute_plan_laid_layouts_in(
    typed: &mut TypedTrees,
    records: &[PlanLaidRecord],
    selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
    domain: &crate::build_time_evaluation::EvaluationDomain,
) -> Result<(), Vec<Diagnostic>> {
    if records.is_empty() {
        return Ok(());
    }

    let execution = domain.execution_program(typed);
    let mut layouts = Vec::with_capacity(records.len());
    let mut target_layouts = arena::Arena::new();
    let mut captured_schema_fields = arena::Arena::new();
    for record in records {
        let policy_name = record
            .policy_machine
            .strip_suffix("::plan")
            .unwrap_or(&record.policy_machine);
        let policy_symbol = typed
            .data_definition_by_name_str(policy_name)
            .map(|data| data.symbol)
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "plan-laid value type `{}` lost its exact nominal policy identity",
                    record.synthetic_name
                ))]
            })?;
        let policy_plan_machine_symbol = typed
            .realized_machine_named(&record.policy_machine)
            .map(|machine| machine.symbol)
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "plan-laid value type `{}` lost its exact policy plan machine",
                    record.synthetic_name
                ))]
            })?;

        let synthesized_data = typed
            .data_definition_by_name_str(&record.synthetic_name)
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "plan-laid value type `{}` lost its exact synthesized data identity",
                    record.synthetic_name
                ))]
            })?;
        let data_symbol = synthesized_data.symbol;
        let field_symbols = typed
            .live_data_fields(typed.data_members(synthesized_data))
            .map(|field| field.symbol)
            .collect::<Vec<_>>();

        let schema = typed
            .data_definition_by_name_str(&record.schema_data)
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "plan-laid value type `{}` lost its exact source schema identity",
                    record.synthetic_name
                ))]
            })?;
        let schema_symbol = schema.symbol;
        let schema_fields = typed
            .live_data_fields(typed.data_members(schema))
            .map(|field| (field.name.as_str().to_owned(), field.type_reference))
            .collect::<Vec<_>>();
        let schema_field_symbols = typed
            .live_data_fields(typed.data_members(schema))
            .map(|field| field.symbol)
            .collect::<Vec<_>>();
        require_invariant_schema(typed, records, schema_symbol).map_err(|reason| {
            vec![Diagnostic::error(format!(
                "plan-laid value type `{}`: {reason}",
                record.synthetic_name
            ))]
        })?;
        let family = typed
            .target_machine_families
            .iter()
            .find(|family| family.canonical == policy_plan_machine_symbol);
        if family.is_none()
            && typed
                .machine_by_symbol(policy_plan_machine_symbol)
                .is_some_and(|machine| machine.target_family_interface)
        {
            return Err(vec![Diagnostic::error(format!(
                "plan-laid value type `{}`: policy family lost its structural roster",
                record.synthetic_name
            ))]);
        }
        let family_schema = family
            .map(|_| {
                let pending = records
                    .iter()
                    .filter_map(|record| {
                        let data = execution.data_definition_by_name_str(&record.synthetic_name)?;
                        execution
                            .plan_laid_layout(data.symbol)
                            .is_none()
                            .then_some(data.symbol)
                    })
                    .collect::<Vec<_>>();
                super::super::schema_reflection::require_materialized_schema(
                    &execution,
                    schema_symbol,
                    &pending,
                )?;
                prepare_family_schema(&execution, schema_symbol, &record.schema_data)
            })
            .transpose()
            .map_err(|reason| {
                vec![Diagnostic::error(format!(
                    "plan-laid value type `{}`: {reason}",
                    record.synthetic_name
                ))]
            })?;
        let schema_argument = family_schema.as_ref().map(|(fields, _, retained)| {
            debug_assert_eq!(fields.len(), retained.len());
            PlanLaidSchemaArgument {
                abi_version: 1,
                fields: captured_schema_fields.insert_many(retained.iter().cloned()),
            }
        });
        let admission =
            crate::BuildTimeAdmissionPlan::infer(&execution, selection_authority.clone());
        let evaluate = |machine_symbol| {
            let machine = execution.machine_by_symbol(machine_symbol).ok_or_else(|| {
                vec![Diagnostic::error(
                    "layout policy lost its exact machine declaration",
                )]
            })?;
            for source in &record.invocation_sources {
                admission
                    .require_common_floor_for_invocation(
                        &execution,
                        machine,
                        crate::BuildTimeInvocationCustody::Source(*source),
                    )
                    .map_err(|reason| vec![Diagnostic::error(reason.to_string())])?;
            }
            let selection = selection_authority
                .clone()
                .zip(record.invocation_sources.first())
                .map(|(authority, source)| crate::AuthorizedSelection {
                    authority,
                    custody: crate::BuildTimeInvocationCustody::Source(*source),
                });
            let report = match &family_schema {
                Some((fields, fingerprint, _)) => {
                    super::super::layout_plans::compute_native_layout_plan_with_schema(
                        &execution,
                        machine_symbol,
                        &record.schema_data,
                        fields,
                        *fingerprint,
                        selection,
                        &domain.accounting,
                    )
                }
                None => {
                    let (fields, fingerprint) = super::super::schema_reflection::schema_fields(
                        &execution,
                        &record.schema_data,
                    )
                    .map_err(|reason| vec![Diagnostic::error(reason)])?;
                    super::super::layout_plans::compute_native_layout_plan_with_schema(
                        &execution,
                        machine_symbol,
                        &record.schema_data,
                        &fields,
                        fingerprint,
                        selection,
                        &domain.accounting,
                    )
                }
            }
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
            install_geometry(&execution, record, &schema_fields, report)
        };
        let realizations = if let Some(family) = family {
            validate_family_roster(&execution, family).map_err(|reason| {
                vec![Diagnostic::error(format!(
                    "plan-laid value type `{}`: {reason}",
                    record.synthetic_name
                ))]
            })?;
            let mut alternatives = Vec::with_capacity(family.members.len());
            for member in &family.members {
                let geometry =
                    evaluate(member.machine).map_err(|diagnostics| {
                        diagnostics.into_iter().map(|diagnostic| {
                        Diagnostic::error(format!(
                            "plan-laid value type `{}` target `{}` member `{}`: {diagnostic}",
                            record.synthetic_name,
                            member.target,
                            execution.symbols.name(member.machine),
                        ))
                    }).collect::<Vec<_>>()
                    })?;
                alternatives.push(PlanLaidTargetLayout {
                    target: member.target.clone(),
                    machine: member.machine,
                    geometry,
                });
            }
            PlanLaidRealizations::TargetFamily(target_layouts.insert_many(alternatives))
        } else {
            PlanLaidRealizations::Invariant(evaluate(policy_plan_machine_symbol).map_err(
                |diagnostics| {
                    diagnostics
                        .into_iter()
                        .map(|diagnostic| {
                            Diagnostic::error(format!(
                                "plan-laid value type `{}`: {diagnostic}",
                                record.synthetic_name
                            ))
                        })
                        .collect::<Vec<_>>()
                },
            )?)
        };
        layouts.push(PlanLaidLayout {
            data_name: record.synthetic_name.clone(),
            data_symbol,
            field_symbols,
            schema_symbol,
            schema_field_symbols,
            policy_symbol,
            policy_plan_machine_symbol,
            schema_argument,
            realizations,
        });
    }

    typed.plan_laid_layouts = layouts;
    typed.plan_laid_target_layouts = target_layouts;
    typed.plan_laid_schema_fields = captured_schema_fields;
    Ok(())
}

/// The structural roster is authoritative, including deployment target names
/// outside the native target catalog. Never infer membership from spelling.
fn validate_family_roster(
    typed: &TypedTrees,
    family: &typed_trees::machine::TargetMachineFamily,
) -> Result<(), String> {
    let canonical = typed.machine_by_symbol(family.canonical).ok_or_else(|| {
        format!(
            "layout policy family `{}` lost its canonical interface",
            family.name
        )
    })?;
    let canonical_identity = typed.machine_callable_identity(canonical);
    if family.members.is_empty()
        || !canonical.target_family_interface
        || !canonical_identity.is_valid()
    {
        return Err(format!(
            "layout policy family `{}` has no complete structural roster",
            family.name
        ));
    }
    for (position, member) in family.members.iter().enumerate() {
        if family.members[..position]
            .iter()
            .any(|earlier| earlier.target == member.target || earlier.machine == member.machine)
        {
            return Err(format!(
                "layout policy family `{}` has duplicate target/member `{}`",
                family.name, member.target
            ));
        }
        let machine = typed.machine_by_symbol(member.machine).ok_or_else(|| {
            format!(
                "layout policy family `{}` target `{}` lost its exact member",
                family.name, member.target
            )
        })?;
        if member.target.as_str().is_empty()
            || machine.authored_target.as_ref() != Some(&member.target)
            || machine.target_family_interface
            || machine.purpose != family.purpose
            || machine.attached_data_symbol != canonical.attached_data_symbol
            || typed.machine_callable_identity(machine) != canonical_identity
            || typed.machine_entry_result_type_identity(machine)
                != typed.machine_entry_result_type_identity(canonical)
        {
            return Err(format!(
                "layout policy family `{}` target `{}` has an inconsistent member `{}`",
                family.name,
                member.target,
                typed.symbols.name(member.machine)
            ));
        }
    }
    Ok(())
}

fn require_invariant_schema(
    typed: &TypedTrees,
    records: &[PlanLaidRecord],
    schema: symbols::SymbolHandle,
) -> Result<(), String> {
    // Pending applications are checked before any layout is installed, so this
    // check cannot accidentally depend on the order of policy evaluation.
    let pending = records
        .iter()
        .filter_map(|record| {
            let policy = typed.realized_machine_named(&record.policy_machine)?;
            if !policy.target_family_interface {
                return None;
            }
            typed
                .data_definition_by_name_str(&record.synthetic_name)
                .map(|data| data.symbol)
        })
        .collect::<Vec<_>>();
    super::super::schema_reflection::require_invariant_schema(typed, schema, &pending)
}

/// Version 1 records the complete active record argument; unsupported ABI axes
/// reject before evaluation so omitted inputs can never masquerade as a fold.
fn prepare_family_schema(
    typed: &TypedTrees,
    schema_symbol: symbols::SymbolHandle,
    schema_name: &str,
) -> Result<
    (
        Vec<super::super::schema_reflection::SchemaFieldInfo>,
        u64,
        Vec<PlanLaidSchemaField>,
    ),
    String,
> {
    let schema = typed
        .data_definition_by_symbol(schema_symbol)
        .ok_or_else(|| "layout family lost its exact schema declaration".to_owned())?;
    if !schema.retired_identities.is_empty() {
        return Err(format!(
            "target-family layout schema `{schema_name}` has retired identities; retained Schema ABI v1 supports active record fields only"
        ));
    }
    if typed
        .data_members(schema)
        .iter()
        .any(|member| member.as_field().is_none())
    {
        return Err(format!(
            "target-family layout schema `{schema_name}` has cases; retained Schema ABI v1 supports records only"
        ));
    }
    let (fields, fingerprint) = super::super::schema_reflection::schema_fields(typed, schema_name)?;
    for field in typed.live_data_fields(typed.data_members(schema)) {
        if !fields.iter().any(|row| row.schema_field == field.symbol) {
            return Err(format!(
                "target-family layout schema `{schema_name}` field `{}` has zero-sized relevant storage; retained Schema ABI v1 requires every relevant field to have an active row",
                field.name
            ));
        }
    }
    let retained = fields.iter().map(|field| {
        let kind = match field.kind {
            "Scalar" => PlanLaidSchemaFieldKind::Scalar,
            "Nested" => PlanLaidSchemaFieldKind::Nested,
            kind => return Err(format!(
                "target-family layout schema `{schema_name}` field `{}` uses `{kind}`; retained Schema ABI v1 supports Scalar and Nested fields only",
                field.name
            )),
        };
        Ok(PlanLaidSchemaField {
            schema_field: field.schema_field,
            key: field.key,
            identity: field.identity,
            byte_size: field.size,
            byte_alignment: field.align,
            kind,
        })
    }).collect::<Result<Vec<_>, String>>()?;
    Ok((fields, fingerprint, retained))
}

fn install_geometry(
    typed: &TypedTrees,
    record: &PlanLaidRecord,
    schema_fields: &[(String, typed_trees::types::TypeReferenceHandle)],
    native_report: crate::layouts::layout_plans::NativeLayoutPlanReport,
) -> Result<PlanLaidGeometry, Vec<Diagnostic>> {
    let report = &native_report.layout;
    let size = report.size.ok_or_else(|| vec![Diagnostic::error(format!(
        "policy `{}` produced a dynamic plan: a dynamic plan cannot be a value type -- values need offsets, bytes need mints",
        record.policy_machine
    ))])?;
    let field_count = schema_fields.len();

    let mut offsets = vec![None; field_count];
    let mut bit_fields = Vec::<PlanLaidBitField>::new();
    let mut integer_fields = Vec::<PlanLaidIntegerField>::new();
    let mut repeated_fields = Vec::<PlanLaidRepeatedField>::new();
    for (field_index, (field_name, field_type)) in schema_fields.iter().enumerate() {
        let field_entries = report
            .entries
            .iter()
            .filter(|entry| entry.field == *field_name)
            .collect::<Vec<_>>();
        match field_entries.as_slice() {
            [entry] if matches!(entry.placement, LayoutPlacementReport::At { .. }) => {
                let LayoutPlacementReport::At { offset } = entry.placement else {
                    unreachable!()
                };
                offsets[field_index] = Some(usize::try_from(offset).map_err(|_| {
                    vec![Diagnostic::error(format!(
                        "plan-laid value type `{}`: byte offset {offset} cannot be represented on this compiler host",
                        record.synthetic_name
                    ))]
                })?);
            }
            [entry] if matches!(entry.placement, LayoutPlacementReport::IntegerAt { .. }) => {
                let LayoutPlacementReport::IntegerAt {
                    offset,
                    stored_width,
                    interpretation,
                } = entry.placement
                else {
                    unreachable!()
                };
                offsets[field_index] = Some(usize::try_from(offset).map_err(|_| {
                    vec![Diagnostic::error(format!(
                        "plan-laid value type `{}`: stored-integer byte offset {offset} cannot be represented on this compiler host",
                        record.synthetic_name
                    ))]
                })?);
                integer_fields.push(PlanLaidIntegerField {
                    field_index,
                    stored_width_bits: u16::try_from(stored_width).map_err(|_| {
                        vec![Diagnostic::error(format!(
                            "plan-laid value type `{}`: stored-integer width {stored_width} exceeds the backend width vocabulary",
                            record.synthetic_name
                        ))]
                    })?,
                    interpretation,
                    write_is_total: stored_integer_write_is_total(
                        typed,
                        *field_type,
                        stored_width,
                        interpretation,
                    ),
                });
            }
            entries
                if entries.len() > 1
                    && entries.iter().all(|entry| {
                        matches!(entry.placement, LayoutPlacementReport::At { .. })
                    }) =>
            {
                let typed_trees::types::TypeReferenceNode::FixedArray {
                    length: typed_trees::types::FixedArrayLength::Literal(element_count),
                    ..
                } = typed.type_reference_table.type_reference(*field_type)
                else {
                    return Err(vec![Diagnostic::error(format!(
                        "plan-laid value type `{}`: field `{field_name}` has repeated byte placements but is not a literal outer fixed array",
                        record.synthetic_name
                    ))]);
                };
                if entries.len() != *element_count {
                    return Err(vec![Diagnostic::error(format!(
                        "plan-laid value type `{}`: field `{field_name}` has {} element placements but its outer fixed array has {element_count} elements",
                        record.synthetic_name,
                        entries.len()
                    ))]);
                }
                let mut element_offsets = entries
                    .iter()
                    .map(|entry| match entry.placement {
                        LayoutPlacementReport::At { offset } => offset,
                        _ => unreachable!("repeated placements were filtered to At"),
                    })
                    .collect::<Vec<_>>();
                element_offsets.sort_unstable();
                let stride = element_offsets[1] - element_offsets[0];
                if stride == 0
                    || element_offsets
                        .windows(2)
                        .any(|pair| pair[1] - pair[0] != stride)
                {
                    return Err(vec![Diagnostic::error(format!(
                        "plan-laid value type `{}`: field `{field_name}` does not retain one positive constant destination stride",
                        record.synthetic_name
                    ))]);
                }
                offsets[field_index] = Some(
                    usize::try_from(element_offsets[0]).map_err(|_| {
                        vec![Diagnostic::error(format!(
                            "plan-laid value type `{}`: repeated field offset {} cannot be represented on this compiler host",
                            record.synthetic_name, element_offsets[0]
                        ))]
                    })?,
                );
                repeated_fields.push(PlanLaidRepeatedField {
                    field_index,
                    element_stride: usize::try_from(stride).map_err(|_| {
                        vec![Diagnostic::error(format!(
                            "plan-laid value type `{}`: repeated field stride {stride} cannot be represented on this compiler host",
                            record.synthetic_name
                        ))]
                    })?,
                });
            }
            entries
                if entries
                    .iter()
                    .all(|entry| matches!(entry.placement, LayoutPlacementReport::Bits { .. })) =>
            {
                let mut fragments = Vec::with_capacity(entries.len());
                for entry in entries {
                    let LayoutPlacementReport::Bits {
                        container,
                        container_width,
                        destination_lsb,
                        source_lsb,
                        width,
                    } = entry.placement
                    else {
                        unreachable!()
                    };
                    let fragment = PlanLaidBitFragment {
                        container_byte_offset: usize::try_from(container).map_err(|_| {
                            vec![Diagnostic::error(format!(
                                "plan-laid value type `{}`: bit-container offset {container} cannot be represented on this compiler host",
                                record.synthetic_name
                            ))]
                        })?,
                        container_width_bits: u16::try_from(container_width).map_err(|_| {
                            vec![Diagnostic::error(format!(
                                "plan-laid value type `{}`: bit-container width {container_width} exceeds the backend width vocabulary",
                                record.synthetic_name
                            ))]
                        })?,
                        destination_lsb: u16::try_from(destination_lsb).map_err(|_| {
                            vec![Diagnostic::error(format!(
                                "plan-laid value type `{}`: destination bit {destination_lsb} exceeds the backend width vocabulary",
                                record.synthetic_name
                            ))]
                        })?,
                        source_lsb: u16::try_from(source_lsb).map_err(|_| {
                            vec![Diagnostic::error(format!(
                                "plan-laid value type `{}`: source bit {source_lsb} exceeds the backend width vocabulary",
                                record.synthetic_name
                            ))]
                        })?,
                        width: u16::try_from(width).map_err(|_| {
                            vec![Diagnostic::error(format!(
                                "plan-laid value type `{}`: fragment width {width} exceeds the backend width vocabulary",
                                record.synthetic_name
                            ))]
                        })?,
                    };
                    offsets[field_index].get_or_insert(fragment.container_byte_offset);
                    fragments.push(fragment);
                }
                bit_fields.push(PlanLaidBitField {
                    field_index,
                    fragments,
                });
            }
            _ => {
                return Err(vec![Diagnostic::error(format!(
                    "plan-laid value type `{}`: field `{field_name}` does not have one \
                     byte placement or a completely tiled bit-fragment placement",
                    record.synthetic_name
                ))]);
            }
        }
    }

    // Normalized plans retain target-independent u64 geometry. This
    // consumer needs host-sized layout indices, so narrow only here and
    // reject rather than panicking on a narrower compiler host.
    let offsets = offsets
        .iter()
        .copied()
        .collect::<Option<Vec<_>>>()
        .expect("validated plan supplies every field");
    let size = usize::try_from(size).map_err(|_| {
        vec![Diagnostic::error(format!(
            "plan-laid value type `{}`: fixed size {size} cannot be represented on this compiler host",
            record.synthetic_name
        ))]
    })?;
    let align = usize::try_from(report.align).map_err(|_| {
        vec![Diagnostic::error(format!(
            "plan-laid value type `{}`: alignment {} cannot be represented on this compiler host",
            record.synthetic_name, report.align
        ))]
    })?;
    Ok(PlanLaidGeometry {
        validated_layout: report.clone(),
        private_callback_demands: native_report.private_callback_demands,
        offsets,
        bit_fields,
        integer_fields,
        repeated_fields,
        size,
        align,
    })
}

fn stored_integer_write_is_total(
    typed: &TypedTrees,
    field_type: typed_trees::types::TypeReferenceHandle,
    stored_width: u64,
    interpretation: layout_plans::IntegerInterpretation,
) -> bool {
    let Some(primitive) = typed.primitive_type_reference(field_type) else {
        return false;
    };
    let (admitted_minimum, admitted_maximum) =
        if let Some(range) = validation::scalar_representation_range(typed, field_type) {
            (i128::from(range.minimum), i128::from(range.maximum))
        } else {
            let width = primitive.scalar_byte_size().unwrap_or(0) * 8;
            if width == 0 {
                return false;
            }
            if primitive.is_signed_integer() {
                (-(1i128 << (width - 1)), (1i128 << (width - 1)) - 1)
            } else {
                (0, (1i128 << width) - 1)
            }
        };
    let (stored_minimum, stored_maximum) = interpretation.stored_bounds(stored_width);
    admitted_minimum >= stored_minimum && admitted_maximum <= stored_maximum
}
