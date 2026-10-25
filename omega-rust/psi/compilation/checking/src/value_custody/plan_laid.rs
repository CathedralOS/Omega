//! Formation and storage obligations of compiler-derived plan-laid value types.
//! Declaration checking validates layout identity and geometry; assignment
//! checking proves the concrete value fits when the semantic carrier is wider
//! than storage. Borrowing or recasting the destination does not change that law.

use diagnostics::Diagnostic;

pub(crate) fn validate_plans(
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut data_symbols = Vec::with_capacity(program.plan_laid_layouts.len());
    for plan in &program.plan_laid_layouts {
        if !plan.is_materialized() {
            diagnostics.push(Diagnostic::error(format!(
                "plan-laid value type `{}` still has pending geometry before source formation checking",
                plan.data_name
            )));
            continue;
        }
        if data_symbols.contains(&plan.data_symbol) {
            diagnostics.push(Diagnostic::error(format!(
                "plan-laid value type `{}` repeats its exact synthesized data identity",
                plan.data_name
            )));
            continue;
        }
        data_symbols.push(plan.data_symbol);
        if !crate::layout_checking::layout_family_is_complete(program, plan) {
            diagnostics.push(Diagnostic::error(format!(
                "plan-laid value type `{}` lost its complete exact target-family application roster",
                plan.data_name
            )));
            continue;
        }

        let Some(data) =
            crate::declarations::data_queries::data_definition_by_symbol(program, plan.data_symbol)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "plan-laid value type `{}` no longer names its exact synthesized data identity",
                plan.data_name
            )));
            continue;
        };
        let data_fields = runtime_fields(program, data);
        let data_field_symbols = data_fields
            .iter()
            .map(|field| field.symbol)
            .collect::<Vec<_>>();
        if data_field_symbols != plan.field_symbols
            || plan
                .geometries(program.typed_program())
                .any(|geometry| geometry.offsets.len() != plan.field_symbols.len())
        {
            diagnostics.push(Diagnostic::error(format!(
                "plan-laid value type `{}` changed its exact synthesized field identity inventory",
                plan.data_name
            )));
            continue;
        }

        let Some(schema) = crate::declarations::data_queries::data_definition_by_symbol(
            program,
            plan.schema_symbol,
        ) else {
            diagnostics.push(Diagnostic::error(format!(
                "plan-laid value type `{}` no longer names its exact source schema identity",
                plan.data_name
            )));
            continue;
        };
        let schema_fields = runtime_fields(program, schema);
        if schema_fields
            .iter()
            .map(|field| field.symbol)
            .ne(plan.schema_field_symbols.iter().copied())
            || plan.schema_field_symbols.len() != plan.field_symbols.len()
        {
            diagnostics.push(Diagnostic::error(format!(
                "plan-laid value type `{}` changed its exact source schema field identity inventory",
                plan.data_name
            )));
            continue;
        }
        if !schema_argument_matches(program, plan, &schema_fields) {
            diagnostics.push(Diagnostic::error(format!(
                "plan-laid value type `{}` lost its exact complete captured Schema argument",
                plan.data_name
            )));
            continue;
        }
        if data_fields
            .iter()
            .zip(&schema_fields)
            .any(|(data_field, schema_field)| {
                data_field.identity != schema_field.identity
                    || (schema_field.identity.is_none()
                        && data_field.name.as_str() != schema_field.name.as_str())
                    || program.display_type_reference_with_constraints(data_field.type_reference)
                        != program
                            .display_type_reference_with_constraints(schema_field.type_reference)
            })
        {
            diagnostics.push(Diagnostic::error(format!(
                "plan-laid value type `{}` changed its exact schema-to-synthesized field correspondence",
                plan.data_name
            )));
            continue;
        }
        if !plan.geometries(program.typed_program()).all(|geometry| {
            target_neutral_report_matches(program, geometry, schema, &schema_fields)
        }) {
            diagnostics.push(Diagnostic::error(format!(
                "plan-laid value type `{}` changed its exact target-neutral layout report identity",
                plan.data_name
            )));
            continue;
        }
        if !plan
            .geometries(program.typed_program())
            .all(|geometry| geometry_matches(geometry, &schema_fields))
        {
            diagnostics.push(Diagnostic::error(format!(
                "plan-laid value type `{}` changed its exact validated geometry projection",
                plan.data_name
            )));
            continue;
        }
        if !plan
            .geometries(program.typed_program())
            .all(|geometry| stored_integer_capabilities_match(program, geometry, &schema_fields))
        {
            diagnostics.push(Diagnostic::error(format!(
                "plan-laid value type `{}` changed its exact stored-integer type capability",
                plan.data_name
            )));
            continue;
        }

        let Some(policy) = crate::declarations::data_queries::data_definition_by_symbol(
            program,
            plan.policy_symbol,
        ) else {
            diagnostics.push(Diagnostic::error(format!(
                "plan-laid value type `{}` no longer names its exact nominal policy identity",
                plan.data_name
            )));
            continue;
        };
        let Some(machine) = program.machine(plan.policy_plan_machine_symbol) else {
            diagnostics.push(Diagnostic::error(format!(
                "plan-laid value type `{}` no longer names its exact policy plan machine",
                plan.data_name
            )));
            continue;
        };
        if machine
            .attached_data
            .as_ref()
            .is_none_or(|attached| attached.as_str() != policy.name.as_str())
            || machine.name.as_str() != format!("{}::plan", policy.name.as_str())
        {
            diagnostics.push(Diagnostic::error(format!(
                "plan-laid value type `{}` changed its exact nominal policy binding",
                plan.data_name
            )));
        }
    }
}

/// A captured argument is a complete ordered input, not a best-effort lookup.
/// An empty span is valid only for an actually empty live schema; the option
/// distinguishes that input from missing family-fold custody.
fn schema_argument_matches(
    program: &dyn crate::CheckingInput,
    plan: &typed_trees::PlanLaidLayout,
    schema_fields: &[&typed_trees::data::DataField],
) -> bool {
    let Some(argument) = plan.schema_argument else {
        return matches!(
            plan.realizations,
            typed_trees::PlanLaidRealizations::Invariant(_)
        );
    };
    if argument.abi_version != 1 {
        return false;
    }
    let Some(rows) = program.plan_laid_schema_fields.span(argument.fields) else {
        return false;
    };
    if rows.len() != schema_fields.len() || rows.len() > 32 {
        return false;
    }
    rows.iter()
        .zip(schema_fields)
        .enumerate()
        .all(|(index, (row, field))| {
            row.schema_field == field.symbol
                && row.identity == field.identity
                && row.key != 0
                && !rows[..index].iter().any(|previous| {
                    previous.key == row.key || previous.schema_field == row.schema_field
                })
                && row.byte_size != 0
                && row.byte_alignment.is_power_of_two()
                && row.kind
                    == if program
                        .primitive_type_reference(field.type_reference)
                        .is_some()
                    {
                        typed_trees::PlanLaidSchemaFieldKind::Scalar
                    } else {
                        typed_trees::PlanLaidSchemaFieldKind::Nested
                    }
        })
}

fn target_neutral_report_matches(
    program: &dyn crate::CheckingInput,
    plan: &typed_trees::PlanLaidGeometry,
    schema: &typed_trees::data::DataDefinition,
    schema_fields: &[&typed_trees::data::DataField],
) -> bool {
    let report = &plan.validated_layout;
    if report.schema_report_fingerprint
        != typed_trees::identity::normalized_schema_report_fingerprint(
            program.typed_program(),
            schema,
        )
    {
        return false;
    }

    let mut matched_entries = 0usize;
    let mut offsets = Vec::with_capacity(schema_fields.len());
    let mut has_only_single_at_entries = true;
    for field in schema_fields {
        let entries = report
            .entries
            .iter()
            .filter(|entry| {
                entry.member_identity == field.identity
                    && (field.identity.is_some() || entry.field == field.name.as_str())
            })
            .collect::<Vec<_>>();
        matched_entries += entries.len();
        match entries.as_slice() {
            [entry] => match entry.placement {
                layout_plans::LayoutPlacementReport::At { offset } => offsets.push(offset),
                layout_plans::LayoutPlacementReport::IntegerAt { .. }
                | layout_plans::LayoutPlacementReport::Bits { .. } => {
                    has_only_single_at_entries = false;
                }
            },
            _ => has_only_single_at_entries = false,
        }
    }
    let expected_offsets = has_only_single_at_entries.then_some(offsets);
    matched_entries == report.entries.len() && report.offsets == expected_offsets
}

fn stored_integer_capabilities_match(
    program: &dyn crate::CheckingInput,
    plan: &typed_trees::PlanLaidGeometry,
    schema_fields: &[&typed_trees::data::DataField],
) -> bool {
    plan.integer_fields.iter().all(|integer| {
        let Some(field) = schema_fields.get(integer.field_index) else {
            return false;
        };
        stored_integer_write_is_total(
            program,
            field.type_reference,
            u64::from(integer.stored_width_bits),
            integer.interpretation,
        ) == Some(integer.write_is_total)
    })
}

fn stored_integer_write_is_total(
    program: &dyn crate::CheckingInput,
    field_type: typed_trees::types::TypeReferenceHandle,
    stored_width: u64,
    interpretation: layout_plans::IntegerInterpretation,
) -> Option<bool> {
    if !(1..=64).contains(&stored_width) {
        return None;
    }
    let primitive = program.primitive_type_reference(field_type)?;
    let (admitted_minimum, admitted_maximum) =
        if let Some(range) = crate::scalar_representation_range(program, field_type) {
            (i128::from(range.minimum), i128::from(range.maximum))
        } else {
            let width = primitive.scalar_byte_size()? * 8;
            if primitive.is_signed_integer() {
                (-(1i128 << (width - 1)), (1i128 << (width - 1)) - 1)
            } else {
                (0, (1i128 << width) - 1)
            }
        };
    let (stored_minimum, stored_maximum) = stored_integer_bounds(stored_width, interpretation)?;
    Some(admitted_minimum >= stored_minimum && admitted_maximum <= stored_maximum)
}

fn stored_integer_bounds(
    stored_width: u64,
    interpretation: layout_plans::IntegerInterpretation,
) -> Option<(i128, i128)> {
    if !(1..=64).contains(&stored_width) {
        return None;
    }
    Some(interpretation.stored_bounds(stored_width))
}

fn geometry_matches(
    plan: &typed_trees::PlanLaidGeometry,
    schema_fields: &[&typed_trees::data::DataField],
) -> bool {
    let report = &plan.validated_layout;
    if report.size.and_then(|size| usize::try_from(size).ok()) != Some(plan.size)
        || usize::try_from(report.align).ok() != Some(plan.align)
    {
        return false;
    }

    let mut offsets = Vec::with_capacity(schema_fields.len());
    let mut bit_fields = Vec::new();
    let mut integer_fields = Vec::new();
    let mut repeated_fields = Vec::new();
    for (field_index, field) in schema_fields.iter().enumerate() {
        let entries = report
            .entries
            .iter()
            .filter(|entry| {
                entry.member_identity == field.identity
                    && (field.identity.is_some() || entry.field == field.name.as_str())
            })
            .collect::<Vec<_>>();
        match entries.as_slice() {
            [entry]
                if matches!(
                    entry.placement,
                    layout_plans::LayoutPlacementReport::At { .. }
                ) =>
            {
                let layout_plans::LayoutPlacementReport::At { offset } = entry.placement else {
                    unreachable!()
                };
                let Ok(offset) = usize::try_from(offset) else {
                    return false;
                };
                offsets.push(offset);
            }
            [entry]
                if matches!(
                    entry.placement,
                    layout_plans::LayoutPlacementReport::IntegerAt { .. }
                ) =>
            {
                let layout_plans::LayoutPlacementReport::IntegerAt {
                    offset,
                    stored_width,
                    interpretation,
                } = entry.placement
                else {
                    unreachable!()
                };
                let (Ok(offset), Ok(stored_width_bits)) =
                    (usize::try_from(offset), u16::try_from(stored_width))
                else {
                    return false;
                };
                offsets.push(offset);
                let Some(retained) = plan
                    .integer_fields
                    .iter()
                    .find(|integer| integer.field_index == field_index)
                else {
                    return false;
                };
                integer_fields.push(typed_trees::PlanLaidIntegerField {
                    field_index,
                    stored_width_bits,
                    interpretation,
                    write_is_total: retained.write_is_total,
                });
            }
            entries
                if entries.len() > 1
                    && entries.iter().all(|entry| {
                        matches!(
                            entry.placement,
                            layout_plans::LayoutPlacementReport::At { .. }
                        )
                    }) =>
            {
                let mut element_offsets = entries
                    .iter()
                    .map(|entry| match entry.placement {
                        layout_plans::LayoutPlacementReport::At { offset } => offset,
                        _ => unreachable!(),
                    })
                    .collect::<Vec<_>>();
                element_offsets.sort_unstable();
                let Some(stride) = element_offsets[1].checked_sub(element_offsets[0]) else {
                    return false;
                };
                if stride == 0
                    || element_offsets
                        .windows(2)
                        .any(|window| window[1].checked_sub(window[0]) != Some(stride))
                {
                    return false;
                }
                let (Ok(offset), Ok(element_stride)) =
                    (usize::try_from(element_offsets[0]), usize::try_from(stride))
                else {
                    return false;
                };
                offsets.push(offset);
                repeated_fields.push(typed_trees::PlanLaidRepeatedField {
                    field_index,
                    element_stride,
                });
            }
            entries
                if !entries.is_empty()
                    && entries.iter().all(|entry| {
                        matches!(
                            entry.placement,
                            layout_plans::LayoutPlacementReport::Bits { .. }
                        )
                    }) =>
            {
                let mut fragments = Vec::with_capacity(entries.len());
                for entry in entries {
                    let layout_plans::LayoutPlacementReport::Bits {
                        container,
                        container_width,
                        destination_lsb,
                        source_lsb,
                        width,
                    } = entry.placement
                    else {
                        unreachable!()
                    };
                    let (
                        Ok(container_byte_offset),
                        Ok(container_width_bits),
                        Ok(destination_lsb),
                        Ok(source_lsb),
                        Ok(width),
                    ) = (
                        usize::try_from(container),
                        u16::try_from(container_width),
                        u16::try_from(destination_lsb),
                        u16::try_from(source_lsb),
                        u16::try_from(width),
                    )
                    else {
                        return false;
                    };
                    if fragments.is_empty() {
                        offsets.push(container_byte_offset);
                    }
                    fragments.push(typed_trees::PlanLaidBitFragment {
                        container_byte_offset,
                        container_width_bits,
                        destination_lsb,
                        source_lsb,
                        width,
                    });
                }
                bit_fields.push(typed_trees::PlanLaidBitField {
                    field_index,
                    fragments,
                });
            }
            _ => return false,
        }
    }

    offsets == plan.offsets
        && bit_fields == plan.bit_fields
        && integer_fields == plan.integer_fields
        && repeated_fields == plan.repeated_fields
}

fn runtime_fields<'program>(
    program: &'program dyn crate::CheckingInput,
    data: &typed_trees::data::DataDefinition,
) -> Vec<&'program typed_trees::data::DataField> {
    program
        .live_data_fields(program.data_members(data))
        .collect()
}
