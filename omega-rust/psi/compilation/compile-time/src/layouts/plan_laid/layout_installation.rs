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

struct ScopedRepresentationRequest {
    storage: typed_trees::layout_storage::LayoutStorageArguments,
    targets: Vec<layout_plans::LayoutTargetFacts>,
    dependencies: Vec<RepresentationDependency>,
    invocation_owners: Vec<symbols::SymbolHandle>,
}

#[derive(Clone, Copy)]
struct RepresentationDependency {
    storage: typed_trees::layout_storage::LayoutStorageArguments,
    lexical_owner: symbols::SymbolHandle,
}

fn applicable_targets(
    typed: &TypedTrees,
    domain: &crate::build_time_evaluation::EvaluationDomain,
    storage: typed_trees::layout_storage::LayoutStorageArguments,
) -> Vec<layout_plans::LayoutTargetFacts> {
    let Some(inputs) = &domain.layout_applications else {
        return domain.layout_targets.clone();
    };
    let identity = typed.normalized_type_identity(storage.application);
    let mut targets = Vec::new();
    for input in inputs
        .iter()
        .filter(|input| typed.type_identity_by_text(&input.application_identity) == identity)
    {
        if !targets.contains(&input.target) {
            targets.push(input.target.clone());
        }
    }
    targets
}

fn invocation_owners(
    typed: &TypedTrees,
    domain: &crate::build_time_evaluation::EvaluationDomain,
    storage: typed_trees::layout_storage::LayoutStorageArguments,
    owner: symbols::SymbolHandle,
) -> Vec<symbols::SymbolHandle> {
    // A universal recipe is demanded by this exact source occurrence, not a
    // consumer's closed application. It must pass the same package/admission
    // floor even when its open static operands do not affect the output.
    if !typed_trees::domain::index_argument_is_closed(typed, storage.policy) {
        return vec![owner];
    }
    let Some(inputs) = &domain.layout_applications else {
        return vec![owner];
    };
    let identity = typed.normalized_type_identity(storage.application);
    let mut owners = Vec::new();
    for input in inputs
        .iter()
        .filter(|input| typed.type_identity_by_text(&input.application_identity) == identity)
    {
        if !owners.contains(&input.lexical_owner) {
            owners.push(input.lexical_owner);
        }
    }
    owners
}

pub(crate) fn compute_plan_laid_layouts_in(
    typed: &mut TypedTrees,
    selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
    domain: &crate::build_time_evaluation::EvaluationDomain,
) -> Result<(), Vec<Diagnostic>> {
    let fail = |reason: String| vec![Diagnostic::error(reason)];
    let mut requests = Vec::new();
    for occurrence in
        typed_trees::references::collect_selected_program_type_occurrences(typed, domain.work())
    {
        let Some(storage) = typed.layout_storage_arguments(occurrence.reference) else {
            continue;
        };
        validation::conformance_applications::validate_data_evidence_application(
            typed,
            storage.application,
            occurrence.lexical_owner,
        )?;
        if let Some(known) = requests
            .iter_mut()
            .find(|known: &&mut ScopedRepresentationRequest| {
                typed.normalized_type_identity(known.storage.application)
                    == typed.normalized_type_identity(storage.application)
            })
        {
            for owner in invocation_owners(typed, domain, storage, occurrence.lexical_owner) {
                if !known.invocation_owners.contains(&owner) {
                    known.invocation_owners.push(owner);
                }
            }
            continue;
        }
        // A symbolic policy application owes the generic body's universal
        // geometry, independently of its consumers' concrete arguments. The
        // ordinary evaluator receives no value for those symbolic operands:
        // only an output that does not read them can complete. Explicit target
        // facts remain known inputs; no caller's policy instance supplies proof.
        if typed
            .closed_data_field_definition(storage.semantic_type)
            .is_some()
        {
            let policy_is_closed =
                typed_trees::domain::index_argument_is_closed(typed, storage.policy);
            let policy_declaration = typed.type_reference_table.type_symbol(storage.policy);
            if !policy_is_closed
                && policy_declaration.is_valid()
                && typed.symbols.get(policy_declaration).kind
                    == symbols::SymbolKind::ConformanceParameter
            {
                // An abstract implementation parameter names no selected body.
                // Its field telescope remains usable, but physical consumers
                // still owe geometry after an exact implementation is supplied.
                continue;
            }
            let targets = if policy_is_closed {
                applicable_targets(typed, domain, storage)
            } else {
                domain.layout_targets.clone()
            };
            // An empty deployment roster cannot prove universal geometry.
            // Leave the original physical obligation unavailable, so settled
            // consumers still refuse rather than treating no alternatives as
            // successful representation compatibility.
            if !policy_is_closed && targets.is_empty() {
                continue;
            }
            requests.push(ScopedRepresentationRequest {
                storage,
                targets,
                dependencies: representation_dependencies(typed, storage.semantic_type)?,
                invocation_owners: invocation_owners(
                    typed,
                    domain,
                    storage,
                    occurrence.lexical_owner,
                ),
            });
        }
    }
    // Storage dependencies are exact applied types, not nominal names. A
    // reference descriptor does not demand its referent's representation.
    let mut pending = (0..requests.len()).collect::<Vec<_>>();
    while let Some(index) = pending.pop() {
        let inherited = requests[index].targets.clone();
        for dependency in requests[index].dependencies.clone() {
            if let Some(position) = requests.iter().position(|known| {
                typed.normalized_type_identity(known.storage.application)
                    == typed.normalized_type_identity(dependency.storage.application)
            }) {
                let known = &mut requests[position];
                let before = known.targets.len();
                if !inherited.is_empty()
                    && !known.invocation_owners.contains(&dependency.lexical_owner)
                {
                    known.invocation_owners.push(dependency.lexical_owner);
                }
                for target in &inherited {
                    if !known.targets.contains(target) {
                        known.targets.push(target.clone());
                    }
                }
                if known.targets.len() != before && !pending.contains(&position) {
                    pending.push(position);
                }
                continue;
            }
            validation::conformance_applications::validate_data_evidence_application(
                typed,
                dependency.storage.application,
                dependency.lexical_owner,
            )?;
            pending.push(requests.len());
            requests.push(ScopedRepresentationRequest {
                storage: dependency.storage,
                targets: inherited.clone(),
                dependencies: representation_dependencies(typed, dependency.storage.semantic_type)?,
                invocation_owners: if inherited.is_empty() {
                    Vec::new()
                } else {
                    vec![dependency.lexical_owner]
                },
            });
        }
    }
    let mut ordered = Vec::with_capacity(requests.len());
    while !requests.is_empty() {
        let mut ready = None;
        for (position, request) in requests.iter().enumerate() {
            if request.dependencies.iter().all(|dependency| {
                !requests.iter().any(|pending| {
                    typed.normalized_type_identity(pending.storage.application)
                        == typed.normalized_type_identity(dependency.storage.application)
                })
            }) {
                ready = Some(position);
                break;
            }
        }
        let Some(position) = ready else {
            return Err(fail(
                "fixed representation applications have a cyclic storage dependency".into(),
            ));
        };
        ordered.push(requests.remove(position));
    }
    let requests = ordered;
    if requests.is_empty() {
        return Ok(());
    }
    if domain.layout_targets.is_empty() && domain.layout_applications.is_none() {
        // Source completion may establish semantic constants before deployment
        // inputs exist. Retain exact constructor applications as obligations;
        // publish no provisional geometry. The settled Source facade requires
        // their physical closure before checking or Terminal production.
        return Ok(());
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
    typed_trees_to_typed_trees::prepare_execution_applications(typed)?;
    // A continuation replaces only the applications it owns. Earlier source
    // continuations and their arena-backed field/target rows remain available
    // to later dependencies; clearing the whole catalog loses that custody.
    let mut replaced = requests
        .iter()
        .map(|request| typed.normalized_type_identity(request.storage.application))
        .collect::<Vec<_>>();
    replaced.sort_unstable();
    replaced.dedup();
    let existing = std::mem::take(&mut typed.plan_laid_layouts);
    typed.plan_laid_layouts = existing
        .into_iter()
        .filter(|plan| {
            replaced
                .binary_search(&typed.normalized_type_identity(plan.application))
                .is_err()
        })
        .collect();
    let mut execution = domain.execution_program(typed);
    let admission = crate::BuildTimeAdmissionPlan::infer(&execution, selection_authority.clone());
    for request in requests {
        let storage = request.storage;
        let policy_is_closed = typed_trees::domain::index_argument_is_closed(typed, storage.policy);
        let mut targets = request.targets;
        let invocation_owners = request.invocation_owners;
        targets.sort_by(|left, right| left.profile.cmp(&right.profile));
        // Schema reflection consumes the exact checked declaration, independently
        // of the narrower roster of bodies permitted to execute. Describing T
        // never promotes T or any Product machine into the execution domain.
        let semantic = typed
            .closed_data_field_definition(storage.semantic_type)
            .ok_or_else(|| {
                fail(format!(
                    "fixed representation lost its exact declared semantic application `{}`",
                    typed.display_type_reference(storage.semantic_type)
                ))
            })?
            .clone();
        if semantic.supply_mode != language_semantics::DataSupplyMode::CheckedShape {
            return Err(fail(
                "fixed representation cannot reflect opaque semantic storage".into(),
            ));
        }
        if typed
            .data_members(&semantic)
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
        if !targets.is_empty() && invocation_owners.is_empty() {
            return Err(fail(
                "fixed representation invocation lost its exact demanding occurrence custody"
                    .into(),
            ));
        }
        for owner in invocation_owners.iter().filter(|_| !targets.is_empty()) {
            // A generated invocation keeps its exact symbol and complete
            // specialization custody. Only its authored package provenance is
            // inherited through the mandatory symbol derivation chain.
            if !owner.is_valid()
                || execution
                    .symbols
                    .symbol_provenance_source_span(*owner)
                    .is_none()
            {
                return Err(fail(
                    "fixed representation invocation has an invalid source owner".into(),
                ));
            }
            admission
                .require_common_floor_for_invocation(
                    &execution,
                    machine,
                    crate::BuildTimeInvocationCustody::Symbol(*owner),
                )
                .map_err(|reason| fail(reason.to_string()))?;
        }
        let schema_fields = typed
            .live_data_fields(typed.data_members(&semantic))
            .map(|field| (field.name.as_str().to_owned(), field.type_reference))
            .collect::<Vec<_>>();
        let mut field_symbols = None;
        let record = FixedRepresentationRequest {
            application_name: typed.display_type_reference(storage.application),
            policy_machine: machine.name.as_str().to_owned(),
        };
        let mut alternatives = Vec::new();
        for target in &targets {
            let (fields, fingerprint) = super::super::schema_reflection::schema_fields_for_target(
                typed,
                &semantic,
                Some(target),
            )
            .map_err(fail)?;
            let symbols = fields
                .iter()
                .map(|field| field.schema_field)
                .collect::<Vec<_>>();
            if field_symbols
                .as_ref()
                .is_some_and(|known| *known != symbols)
            {
                return Err(fail(
                    "target schema changed its exact semantic field roster".into(),
                ));
            }
            field_symbols = Some(symbols);
            let schema_value = super::super::schema_reflection::build_selected_schema_value(
                typed, &semantic, &fields,
            )
            .map_err(fail)?;
            let retained = super::super::schema_reflection::canonical_schema_fields(&fields).into_iter().map(|field| Ok(PlanLaidSchemaField {
            schema_field: field.schema_field, key: field.key, identity: field.identity,
            byte_size: field.size, byte_alignment: field.align,
            kind: match field.kind { "Scalar" => PlanLaidSchemaFieldKind::Scalar,
                "Nested" => PlanLaidSchemaFieldKind::Nested,
                "Repeated" => PlanLaidSchemaFieldKind::Repeated,
                _ => return Err(fail("fixed representation retained Schema ABI does not support this field kind".into())), },
        })).collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
            let mut retired_identities = semantic.retired_identities.clone();
            retired_identities.sort_unstable();
            let schema_argument = Some(PlanLaidSchemaArgument {
                abi_version: 3,
                fields: typed.plan_laid_schema_fields.insert_many(retained),
                retired_field_identities: typed.plan_laid_retired_field_identities.insert_many(
                    retired_identities.iter().map(|identity| {
                        typed_trees::PlanLaidRetiredFieldIdentity {
                            identity: *identity,
                        }
                    }),
                ),
            });
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
                    fail(if policy_is_closed {
                        format!(
                            "layout policy `{}` under `{}`: {reason}",
                            machine.name, target.profile
                        )
                    } else {
                        format!(
                            "universal fixed representation `{}` under `{}` cannot establish its geometry without a bound static operand: {reason}",
                            record.application_name, target.profile
                        )
                    })
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
                schema_argument,
                target: target.profile.as_str().into(),
                machine: machine.symbol,
                target_facts: target.clone(),
                geometry,
            });
        }
        // A retained description with no Build application has no invocation
        // or geometry. Keep its exact field telescope and pending marker;
        // physical consumers still require a realization for their target.
        let field_symbols = field_symbols.unwrap_or_else(|| {
            typed
                .live_data_fields(typed.data_members(&semantic))
                .map(|field| field.symbol)
                .collect()
        });
        let layout = PlanLaidLayout {
            application: storage.application,
            semantic_type: storage.semantic_type,
            policy_application: application.clone(),
            target_inputs: targets,
            data_name: record.application_name,
            data_symbol: semantic.symbol,
            field_symbols: field_symbols.clone(),
            schema_symbol: semantic.symbol,
            schema_field_symbols: field_symbols,
            policy_symbol: application.declaration,
            policy_plan_machine_symbol: machine.symbol,
            schema_argument: None,
            realizations: PlanLaidRealizations::TargetInputs(
                typed.plan_laid_target_layouts.insert_many(alternatives),
            ),
        };
        typed.plan_laid_layouts.push(layout);
        // The immutable executable declarations are shared for this work
        // domain. Only completed layout evidence changes between dependency
        // steps; a later policy must observe those actual completed rows.
        execution.plan_laid_layouts = typed.plan_laid_layouts.clone();
        execution.plan_laid_target_layouts = typed.plan_laid_target_layouts.clone();
        execution.plan_laid_schema_fields = typed.plan_laid_schema_fields.clone();
        execution.plan_laid_retired_field_identities =
            typed.plan_laid_retired_field_identities.clone();
    }
    Ok(())
}

/// Product demands belong to the completed continuation roster, not to each
/// source projection separately. The selected evaluator invokes this once
/// after every owner has installed its exact representation.
pub(crate) fn validate_layout_application_coverage(
    typed: &TypedTrees,
    inputs: Option<&[crate::LayoutApplicationTargetInput]>,
) -> Result<(), Vec<Diagnostic>> {
    if let Some(inputs) = inputs {
        for input in inputs {
            let identity = typed.type_identity_by_text(&input.application_identity);
            if !identity.is_valid()
                || !typed.plan_laid_layouts.iter().any(|plan| {
                    typed.normalized_type_identity(plan.application) == identity
                        && plan.target_inputs.contains(&input.target)
                })
            {
                return Err(vec![Diagnostic::error(format!(
                    "layout application `{}` target demand for `{}` did not rejoin its exact evaluated representation",
                    input.application_identity, input.target.profile
                ))]);
            }
        }
    }
    Ok(())
}

/// Discover only by-value dependencies from the exact closed field views.
fn representation_dependencies(
    typed: &TypedTrees,
    semantic_type: typed_trees::types::TypeReferenceHandle,
) -> Result<Vec<RepresentationDependency>, Vec<Diagnostic>> {
    use typed_trees::types::TypeReferenceNode;
    let owner = typed
        .closed_data_field_definition(semantic_type)
        .map(|data| data.symbol)
        .unwrap_or_default();
    let mut work = vec![(semantic_type, owner)];
    let mut seen = Vec::new();
    let mut dependencies = Vec::new();
    while let Some((reference, lexical_owner)) = work.pop() {
        let identity = typed.normalized_type_identity(reference);
        let Some(index) = identity.storage_index() else {
            continue;
        };
        if seen.len() <= index {
            seen.resize(index + 1, false);
        }
        if std::mem::replace(&mut seen[index], true) {
            continue;
        }
        if let Some(storage) = typed.layout_storage_arguments(reference) {
            dependencies.push(RepresentationDependency {
                storage,
                lexical_owner,
            });
            continue;
        }
        let reference = typed
            .type_reference_table
            .unconstrained_type_reference(reference);
        match typed.type_reference_table.type_reference(reference) {
            TypeReferenceNode::FixedArray { element_type, .. } => {
                work.push((*element_type, lexical_owner))
            }
            TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
                if let Some(data) = typed.closed_data_definition(reference) {
                    work.extend(
                        typed
                            .live_data_fields(typed.data_members(data))
                            .map(|field| (field.type_reference, data.symbol)),
                    );
                }
            }
            _ => {}
        }
    }
    Ok(dependencies)
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
