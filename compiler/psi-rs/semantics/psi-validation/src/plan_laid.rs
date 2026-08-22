//! Independent identity replay for compiler-derived plan-laid value types.

use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;

pub(crate) fn validate_plans(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    let mut data_symbols = Vec::with_capacity(program.plan_laid_layouts.len());
    for plan in &program.plan_laid_layouts {
        if data_symbols.contains(&plan.data_symbol) {
            diagnostics.push(Diagnostic::error(format!(
                "plan-laid value type `{}` repeats its exact synthesized data identity",
                plan.data_name
            )));
            continue;
        }
        data_symbols.push(plan.data_symbol);

        let Some(data) = program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == plan.data_symbol)
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
            || plan.offsets.len() != plan.field_symbols.len()
        {
            diagnostics.push(Diagnostic::error(format!(
                "plan-laid value type `{}` changed its exact synthesized field identity inventory",
                plan.data_name
            )));
            continue;
        }

        let Some(schema) = program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == plan.schema_symbol)
        else {
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
        if !geometry_matches(plan, &schema_fields) {
            diagnostics.push(Diagnostic::error(format!(
                "plan-laid value type `{}` changed its exact validated geometry projection",
                plan.data_name
            )));
            continue;
        }
        if !stored_integer_capabilities_match(program, plan, &schema_fields) {
            diagnostics.push(Diagnostic::error(format!(
                "plan-laid value type `{}` changed its exact stored-integer type capability",
                plan.data_name
            )));
            continue;
        }

        let Some(policy) = program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == plan.policy_symbol)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "plan-laid value type `{}` no longer names its exact nominal policy identity",
                plan.data_name
            )));
            continue;
        };
        let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == plan.policy_plan_machine_symbol)
        else {
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

fn stored_integer_capabilities_match(
    program: &TypedTrees,
    plan: &psi_typed_trees::PlanLaidLayout,
    schema_fields: &[&psi_typed_trees::data::DataField],
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
    program: &TypedTrees,
    field_type: psi_typed_trees::types::TypeReferenceHandle,
    stored_width: u64,
    interpretation: psi_layout_plans::IntegerInterpretation,
) -> Option<bool> {
    if !(1..=64).contains(&stored_width) {
        return None;
    }
    let primitive = program.primitive_type_reference(field_type)?;
    let (admitted_minimum, admitted_maximum) = if let Some(range) =
        psi_typed_trees::wire::scalar_representation_range(program, field_type)
    {
        (i128::from(range.minimum), i128::from(range.maximum))
    } else {
        let width = primitive.scalar_byte_size()? * 8;
        if primitive.is_signed_integer() {
            (-(1i128 << (width - 1)), (1i128 << (width - 1)) - 1)
        } else {
            (0, (1i128 << width) - 1)
        }
    };
    let (stored_minimum, stored_maximum) = match interpretation {
        psi_layout_plans::IntegerInterpretation::Signed => (
            -(1i128 << (stored_width - 1)),
            (1i128 << (stored_width - 1)) - 1,
        ),
        psi_layout_plans::IntegerInterpretation::Unsigned => (0, (1i128 << stored_width) - 1),
    };
    Some(admitted_minimum >= stored_minimum && admitted_maximum <= stored_maximum)
}

fn geometry_matches(
    plan: &psi_typed_trees::PlanLaidLayout,
    schema_fields: &[&psi_typed_trees::data::DataField],
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
                    psi_layout_plans::LayoutPlacementReport::At { .. }
                ) =>
            {
                let psi_layout_plans::LayoutPlacementReport::At { offset } = entry.placement else {
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
                    psi_layout_plans::LayoutPlacementReport::IntegerAt { .. }
                ) =>
            {
                let psi_layout_plans::LayoutPlacementReport::IntegerAt {
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
                integer_fields.push(psi_typed_trees::PlanLaidIntegerField {
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
                            psi_layout_plans::LayoutPlacementReport::At { .. }
                        )
                    }) =>
            {
                let mut element_offsets = entries
                    .iter()
                    .map(|entry| match entry.placement {
                        psi_layout_plans::LayoutPlacementReport::At { offset } => offset,
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
                repeated_fields.push(psi_typed_trees::PlanLaidRepeatedField {
                    field_index,
                    element_stride,
                });
            }
            entries
                if !entries.is_empty()
                    && entries.iter().all(|entry| {
                        matches!(
                            entry.placement,
                            psi_layout_plans::LayoutPlacementReport::Bits { .. }
                        )
                    }) =>
            {
                let mut fragments = Vec::with_capacity(entries.len());
                for entry in entries {
                    let psi_layout_plans::LayoutPlacementReport::Bits {
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
                    fragments.push(psi_typed_trees::PlanLaidBitFragment {
                        container_byte_offset,
                        container_width_bits,
                        destination_lsb,
                        source_lsb,
                        width,
                    });
                }
                bit_fields.push(psi_typed_trees::PlanLaidBitField {
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
    program: &'program TypedTrees,
    data: &psi_typed_trees::data::DataDefinition,
) -> Vec<&'program psi_typed_trees::data::DataField> {
    program
        .data_members(data)
        .iter()
        .filter_map(|member| match member {
            psi_typed_trees::data::DataMember::Field(field) if !field.relevance.is_erased() => {
                Some(field)
            }
            psi_typed_trees::data::DataMember::Field(_)
            | psi_typed_trees::data::DataMember::Variant(_) => None,
        })
        .collect()
}
