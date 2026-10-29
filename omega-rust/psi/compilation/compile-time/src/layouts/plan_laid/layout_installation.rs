use diagnostics::Diagnostic;
use layout_plans::LayoutPlacementReport;
use typed_trees::{
    PlanLaidBitField, PlanLaidBitFragment, PlanLaidGeometry, PlanLaidIntegerField, PlanLaidLayout,
    PlanLaidRealizations, PlanLaidRepeatedField, PlanLaidSchemaArgument, PlanLaidSchemaField,
    PlanLaidSchemaFieldKind, PlanLaidTargetLayout, TypedTrees,
};

struct FixedRepresentationRequest {
    application_name: String,
    policy_machine: String,
}

pub(crate) fn compute_plan_laid_layouts_in(
    typed: &mut TypedTrees,
    selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
    domain: &crate::build_time_evaluation::EvaluationDomain,
) -> Result<(), Vec<Diagnostic>> {
    let fail = |reason: String| vec![Diagnostic::error(reason)];
    let mut requests = Vec::new();
    for reference in domain.references(typed).type_references {
        let Some(storage) = typed.layout_storage_arguments(reference) else {
            continue;
        };
        if requests.iter().any(
            |known: &typed_trees::layout_storage::LayoutStorageArguments| {
                typed.normalized_type_identity(known.application)
                    == typed.normalized_type_identity(storage.application)
            },
        ) {
            continue;
        }
        // Open constructor recipes remain source-checked templates; actual
        // storage demands retain their closed semantic field views.
        if typed
            .closed_data_definition(storage.semantic_type)
            .is_some()
        {
            requests.push(storage);
        }
    }
    if requests.is_empty() {
        return Ok(());
    }
    if domain.layout_targets.is_empty() {
        return Err(fail(
            "fixed representation evaluation requires explicit target representation facts".into(),
        ));
    }
    for (position, target) in domain.layout_targets.iter().enumerate() {
        target.validate().map_err(|reason| fail(reason.into()))?;
        if domain.layout_targets[..position]
            .iter()
            .any(|known| known.profile == target.profile)
        {
            return Err(fail(
                "fixed representation evaluation repeats a target input".into(),
            ));
        }
    }
    typed_trees_to_typed_trees::specialize_static_machine_calls(typed)?;
    let execution = domain.execution_program(typed);
    let admission = crate::BuildTimeAdmissionPlan::infer(&execution, selection_authority.clone());
    let mut layouts = Vec::new();
    let mut target_layouts = arena::Arena::new();
    let mut captured_schema_fields = arena::Arena::new();
    for storage in requests {
        // Schema reflection consumes the exact checked declaration, independently
        // of the narrower roster of bodies permitted to execute. Describing T
        // never promotes T or any Product machine into the execution domain.
        let semantic = typed
            .closed_data_definition(storage.semantic_type)
            .ok_or_else(|| {
                fail(format!(
                    "fixed representation lost its exact declared semantic application `{}`",
                    typed.display_type_reference(storage.semantic_type)
                ))
            })?;
        if semantic.supply_mode != language_semantics::DataSupplyMode::CheckedShape {
            return Err(fail(
                "fixed representation cannot reflect opaque semantic storage".into(),
            ));
        }
        if typed
            .data_members(semantic)
            .iter()
            .any(|member| member.as_field().is_none())
        {
            return Err(fail("fixed representation requires a record: programmable case/tag placement is not supported by this placement vocabulary".into()));
        }
        let selected = execution
            .static_operand_from_type_reference(storage.policy)
            .map_err(|reason| fail(reason.into()))?;
        let application = validation::conformance_applications::close_conformance_application(
            &execution, &selected,
        )
        .map_err(|error| vec![error])?;
        if application.trait_definition != storage.contract {
            return Err(fail(
                "fixed representation policy does not satisfy the exact core FixedLayout contract"
                    .into(),
            ));
        }
        let [row] = application.rows.as_slice() else {
            return Err(fail(
                "fixed representation policy must retain its one exact plan row".into(),
            ));
        };
        let realized = execution
            .captured_machine_applications
            .iter()
            .find_map(|(_, captured)| {
                (captured.template == row.realization_machine
                    && captured.conformance_application == application)
                    .then_some(captured.instance)
            })
            .unwrap_or(row.realization_machine);
        let machine = execution
            .machine_by_symbol(realized)
            .ok_or_else(|| fail("fixed representation lost its checked policy body".into()))?;
        let sources = execution
            .authored_declaration_selections()
            .iter()
            .filter_map(|selection| {
                use language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget;
                matches!(selection.target(), AuthoredDeclarationSelectionTarget::Resolved(target)
                if target.selected_symbol() == application.declaration)
                .then_some(selection.source_span())
            })
            .collect::<Vec<_>>();
        if sources.is_empty() {
            return Err(fail(
                "fixed representation policy lost its authored selection custody".into(),
            ));
        }
        for source in &sources {
            admission
                .require_common_floor_for_invocation(
                    &execution,
                    machine,
                    crate::BuildTimeInvocationCustody::Source(*source),
                )
                .map_err(|reason| fail(reason.to_string()))?;
        }
        let (fields, fingerprint) =
            super::super::schema_reflection::schema_fields_for(typed, semantic).map_err(fail)?;
        let schema_value =
            super::super::schema_reflection::build_selected_schema_value(typed, semantic, &fields)
                .map_err(fail)?;
        let retained = fields.iter().map(|field| Ok(PlanLaidSchemaField {
            schema_field: field.schema_field, key: field.key, identity: field.identity,
            byte_size: field.size, byte_alignment: field.align,
            kind: match field.kind { "Scalar" => PlanLaidSchemaFieldKind::Scalar,
                "Nested" => PlanLaidSchemaFieldKind::Nested,
                _ => return Err(fail("fixed representation retained Schema ABI does not support this field kind".into())), },
        })).collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
        let schema_argument = Some(PlanLaidSchemaArgument {
            abi_version: 1,
            fields: captured_schema_fields.insert_many(retained),
        });
        let schema_fields = typed
            .live_data_fields(typed.data_members(semantic))
            .map(|field| (field.name.as_str().to_owned(), field.type_reference))
            .collect::<Vec<_>>();
        let field_symbols = fields
            .iter()
            .map(|field| field.schema_field)
            .collect::<Vec<_>>();
        let record = FixedRepresentationRequest {
            application_name: typed.display_type_reference(storage.application),
            policy_machine: machine.name.as_str().to_owned(),
        };
        let mut alternatives = Vec::new();
        for target in &domain.layout_targets {
            let mut arguments = vec![
                schema_value.clone(),
                build_time_values::BuildTimeValue::LayoutTarget(target.clone()),
            ];
            crate::machine_execution::admission::require_established_arguments(
                &execution,
                machine,
                &mut arguments,
            )
            .map_err(fail)?;
            let evaluation = domain
                .accounting
                .evaluate(
                    checked_interpreter::BuildMachineProgram::Typed(&execution),
                    checked_interpreter::BuildMachineEvaluationRequest::symbol(
                        machine.symbol,
                        arguments,
                    ),
                )
                .map_err(|reason| {
                    fail(format!(
                        "layout policy `{}` under `{}`: {reason}",
                        machine.name, target.profile
                    ))
                })?;
            let layout = super::super::layout_plans::validate_plan(
                evaluation.value(),
                &fields,
                fingerprint,
                machine.name.as_str(),
            )
            .map_err(fail)?;
            let private_callback_demands =
                super::super::layout_plans::normalize_private_callback_demands(
                    &execution,
                    machine,
                    evaluation.private_layout_placements(),
                    machine.name.as_str(),
                    Some((
                        &selected,
                        execution
                            .data_implementation_applications
                            .iter()
                            .find_map(|(_, demand)| {
                                (execution.normalized_type_identity(demand.source_application)
                                    == execution.normalized_type_identity(storage.application))
                                .then_some(demand.lexical_owner)
                            })
                            .unwrap_or_else(|| {
                                execution
                                    .type_reference_table
                                    .operand_lifetime_owner(storage.policy)
                            }),
                    )),
                )
                .map_err(fail)?;
            let geometry = install_geometry(
                typed,
                &record,
                &schema_fields,
                crate::layouts::layout_plans::NativeLayoutPlanReport {
                    layout,
                    private_callback_demands,
                },
            )?;
            alternatives.push(PlanLaidTargetLayout {
                target: target.profile.as_str().into(),
                machine: machine.symbol,
                target_facts: target.clone(),
                geometry,
            });
        }
        layouts.push(PlanLaidLayout {
            application: storage.application,
            semantic_type: storage.semantic_type,
            policy_application: application.clone(),
            target_inputs: domain.layout_targets.clone(),
            data_name: record.application_name,
            data_symbol: semantic.symbol,
            field_symbols: field_symbols.clone(),
            schema_symbol: semantic.symbol,
            schema_field_symbols: field_symbols,
            policy_symbol: application.declaration,
            policy_plan_machine_symbol: machine.symbol,
            schema_argument,
            realizations: PlanLaidRealizations::TargetInputs(
                target_layouts.insert_many(alternatives),
            ),
        });
    }
    typed.plan_laid_layouts = layouts;
    typed.plan_laid_target_layouts = target_layouts;
    typed.plan_laid_schema_fields = captured_schema_fields;
    Ok(())
}

fn install_geometry(
    typed: &TypedTrees,
    record: &FixedRepresentationRequest,
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
                        record.application_name
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
                        record.application_name
                    ))]
                })?);
                integer_fields.push(PlanLaidIntegerField {
                    field_index,
                    stored_width_bits: u16::try_from(stored_width).map_err(|_| {
                        vec![Diagnostic::error(format!(
                            "plan-laid value type `{}`: stored-integer width {stored_width} exceeds the backend width vocabulary",
                            record.application_name
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
                        record.application_name
                    ))]);
                };
                if entries.len() != *element_count {
                    return Err(vec![Diagnostic::error(format!(
                        "plan-laid value type `{}`: field `{field_name}` has {} element placements but its outer fixed array has {element_count} elements",
                        record.application_name,
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
                        record.application_name
                    ))]);
                }
                offsets[field_index] = Some(
                    usize::try_from(element_offsets[0]).map_err(|_| {
                        vec![Diagnostic::error(format!(
                            "plan-laid value type `{}`: repeated field offset {} cannot be represented on this compiler host",
                            record.application_name, element_offsets[0]
                        ))]
                    })?,
                );
                repeated_fields.push(PlanLaidRepeatedField {
                    field_index,
                    element_stride: usize::try_from(stride).map_err(|_| {
                        vec![Diagnostic::error(format!(
                            "plan-laid value type `{}`: repeated field stride {stride} cannot be represented on this compiler host",
                            record.application_name
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
                                record.application_name
                            ))]
                        })?,
                        container_width_bits: u16::try_from(container_width).map_err(|_| {
                            vec![Diagnostic::error(format!(
                                "plan-laid value type `{}`: bit-container width {container_width} exceeds the backend width vocabulary",
                                record.application_name
                            ))]
                        })?,
                        destination_lsb: u16::try_from(destination_lsb).map_err(|_| {
                            vec![Diagnostic::error(format!(
                                "plan-laid value type `{}`: destination bit {destination_lsb} exceeds the backend width vocabulary",
                                record.application_name
                            ))]
                        })?,
                        source_lsb: u16::try_from(source_lsb).map_err(|_| {
                            vec![Diagnostic::error(format!(
                                "plan-laid value type `{}`: source bit {source_lsb} exceeds the backend width vocabulary",
                                record.application_name
                            ))]
                        })?,
                        width: u16::try_from(width).map_err(|_| {
                            vec![Diagnostic::error(format!(
                                "plan-laid value type `{}`: fragment width {width} exceeds the backend width vocabulary",
                                record.application_name
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
                    record.application_name
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
            record.application_name
        ))]
    })?;
    let align = usize::try_from(report.align).map_err(|_| {
        vec![Diagnostic::error(format!(
            "plan-laid value type `{}`: alignment {} cannot be represented on this compiler host",
            record.application_name, report.align
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
