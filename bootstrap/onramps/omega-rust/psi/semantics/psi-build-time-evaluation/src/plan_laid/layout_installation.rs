use psi_diagnostics::Diagnostic;
use psi_layout_plans::LayoutPlacementReport;
use psi_typed_trees::{
    PlanLaidBitField, PlanLaidBitFragment, PlanLaidIntegerField, PlanLaidLayout,
    PlanLaidRepeatedField, TypedTrees,
};

use super::PlanLaidRecord;

/// Evaluate + validate each discovered policy application (the L2/L3
/// pipeline), require a fully static plan, and record the placements for the
/// native layout builder.
pub fn compute_plan_laid_layouts(
    typed: &mut TypedTrees,
    records: &[PlanLaidRecord],
) -> Result<(), Vec<Diagnostic>> {
    compute_plan_laid_layouts_with_authority(typed, records, None)
}

pub fn compute_plan_laid_layouts_with_authority(
    typed: &mut TypedTrees,
    records: &[PlanLaidRecord],
    selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
) -> Result<(), Vec<Diagnostic>> {
    if records.is_empty() {
        return Ok(());
    }

    let mut layouts = Vec::with_capacity(records.len());
    for record in records {
        if let Some(selection_authority) = selection_authority.clone() {
            let machine = typed
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == record.policy_machine)
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "plan-laid value type `{}`: no machine named `{}` exists",
                        record.synthetic_name, record.policy_machine
                    ))]
                })?;
            let admission = crate::BuildTimeAdmissionPlan::infer_with_selection_authority(
                typed,
                Some(selection_authority),
            );
            for source in &record.invocation_sources {
                admission
                    .require_common_floor_for_invocation(
                        typed,
                        machine,
                        crate::BuildTimeInvocationCustody::Source(*source),
                    )
                    .map_err(|reason| {
                        vec![Diagnostic::error(format!(
                            "plan-laid value type `{}`: {reason}",
                            record.synthetic_name
                        ))]
                    })?;
            }
        }
        let report = crate::compute_layout_plan(typed, &record.policy_machine, &record.schema_data)
            .map_err(|reason| {
                vec![Diagnostic::error(format!(
                    "plan-laid value type `{}`: {reason}",
                    record.synthetic_name
                ))]
            })?;
        let Some(size) = report.size else {
            return Err(vec![Diagnostic::error(format!(
                "plan-laid value type `{}`: policy `{}` produced a dynamic plan; a dynamic \
                 plan cannot be a value type -- values need offsets, bytes need mints",
                record.synthetic_name, record.policy_machine
            ))]);
        };

        let policy_name = record
            .policy_machine
            .strip_suffix("::plan")
            .unwrap_or(&record.policy_machine);
        let policy_symbol = typed
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == policy_name)
            .map(|data| data.symbol)
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "plan-laid value type `{}` lost its exact nominal policy identity",
                    record.synthetic_name
                ))]
            })?;
        let policy_plan_machine_symbol = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == record.policy_machine)
            .map(|machine| machine.symbol)
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "plan-laid value type `{}` lost its exact policy plan machine",
                    record.synthetic_name
                ))]
            })?;

        let synthesized_data = typed
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == record.synthetic_name)
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "plan-laid value type `{}` lost its exact synthesized data identity",
                    record.synthetic_name
                ))]
            })?;
        let data_symbol = synthesized_data.symbol;
        let field_symbols = typed
            .data_members(synthesized_data)
            .iter()
            .filter_map(|member| match member {
                psi_typed_trees::data::DataMember::Field(field) if !field.relevance.is_erased() => {
                    Some(field.symbol)
                }
                psi_typed_trees::data::DataMember::Field(_)
                | psi_typed_trees::data::DataMember::Variant(_) => None,
            })
            .collect::<Vec<_>>();

        let schema = typed
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == record.schema_data)
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "plan-laid value type `{}` lost its exact source schema identity",
                    record.synthetic_name
                ))]
            })?;
        let schema_symbol = schema.symbol;
        let schema_fields = typed
            .data_members(schema)
            .iter()
            .filter_map(|member| match member {
                psi_typed_trees::data::DataMember::Field(field) => (!field.relevance.is_erased())
                    .then_some((field.name.as_str().to_owned(), field.type_reference)),
                psi_typed_trees::data::DataMember::Variant(_) => None,
            })
            .collect::<Vec<_>>();
        let schema_field_symbols = typed
            .data_members(schema)
            .iter()
            .filter_map(|member| match member {
                psi_typed_trees::data::DataMember::Field(field) if !field.relevance.is_erased() => {
                    Some(field.symbol)
                }
                psi_typed_trees::data::DataMember::Field(_)
                | psi_typed_trees::data::DataMember::Variant(_) => None,
            })
            .collect::<Vec<_>>();
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
                    let psi_typed_trees::types::TypeReferenceNode::FixedArray {
                        length: psi_typed_trees::types::FixedArrayLength::Literal(element_count),
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
                    if entries.iter().all(|entry| {
                        matches!(entry.placement, LayoutPlacementReport::Bits { .. })
                    }) =>
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
        layouts.push(PlanLaidLayout {
            data_name: record.synthetic_name.clone(),
            data_symbol,
            field_symbols,
            schema_symbol,
            schema_field_symbols,
            policy_symbol,
            policy_plan_machine_symbol,
            validated_layout: report.clone(),
            offsets,
            bit_fields,
            integer_fields,
            repeated_fields,
            size,
            align,
        });
    }

    typed.plan_laid_layouts = layouts;
    Ok(())
}

fn stored_integer_write_is_total(
    typed: &TypedTrees,
    field_type: psi_typed_trees::types::TypeReferenceHandle,
    stored_width: u64,
    interpretation: psi_layout_plans::IntegerInterpretation,
) -> bool {
    let Some(primitive) = typed.primitive_type_reference(field_type) else {
        return false;
    };
    let (admitted_minimum, admitted_maximum) = if let Some(range) =
        psi_typed_trees::wire::scalar_representation_range(typed, field_type)
    {
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
    let (stored_minimum, stored_maximum) = match interpretation {
        psi_layout_plans::IntegerInterpretation::Signed => (
            -(1i128 << (stored_width - 1)),
            (1i128 << (stored_width - 1)) - 1,
        ),
        psi_layout_plans::IntegerInterpretation::Unsigned => (0, (1i128 << stored_width) - 1),
    };
    admitted_minimum >= stored_minimum && admitted_maximum <= stored_maximum
}
