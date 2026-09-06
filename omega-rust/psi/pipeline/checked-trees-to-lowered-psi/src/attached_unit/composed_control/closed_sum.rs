//! Exact three-state structural-result case dispatch.

use super::*;

pub(super) fn lower(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedComposedUnitControlMachinePlan,
) -> Result<SourceMappedLowered, LoweringError> {
    let admitted = admit(checked, plan)?;
    let catalogs = catalogs::lower_composed_catalogs(checked, plan, &admitted)?;
    emit(checked, plan, admitted, catalogs)
}

fn admit<'a>(
    checked: &'a CheckedTrees,
    plan: &'a checked_trees::CheckedComposedUnitControlMachinePlan,
) -> Result<admission::AdmittedComposedUnit<'a>, LoweringError> {
    let [entry, first_leaf, second_leaf] = plan.states.as_slice() else {
        return unsupported("closed-sum Unit control requires exactly three states");
    };
    let CheckedComposedUnitControlTerminatorPlan::ClosedSum { result, cases } = &entry.terminator
    else {
        return unsupported("closed-sum Unit entry lost its structural terminator");
    };
    let [entry_call] = entry.operations.as_slice() else {
        return unsupported("closed-sum Unit entry requires one structural boundary call");
    };
    let CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
        coordinate,
        target_machine,
        target_state,
        target_contract_report_fingerprint,
        service_reach,
        result: call_result,
        scalar_arguments,
        structural_arguments,
        completion_receipts,
        discard_result_on_return,
        ..
    } = entry_call
    else {
        return unsupported("closed-sum Unit entry operation is not a structural boundary call");
    };
    if !plan.body_qualifications.is_empty()
        || !entry.structural_parameters.is_empty()
        || !entry.scalar_parameters.is_empty()
        || !entry.entry_claims.is_empty()
        || !entry.bindings.is_empty()
        || !entry.binding_initializers.is_empty()
        || call_result != result
        || *discard_result_on_return
        || !scalar_arguments.is_empty()
        || !structural_arguments.is_empty()
        || !completion_receipts.is_empty()
        || cases.len() != 2
    {
        return unsupported("closed-sum Unit entry escaped the exact claim-free result lane");
    }
    admission::retain_exact_flow_call(
        checked,
        plan.machine,
        entry.state,
        *coordinate,
        *target_state,
    )?;
    let plans = &checked.facts.flow.terminal_unit_effects;
    let target = unique_unit_boundary(plans, *target_machine)?;
    let expected_result = target.result.clone();
    if !matches!(
        &expected_result,
        CheckedBoundaryMachineResultPlan::Structural {
            type_identity,
            multiplicity: Multiplicity::Affine,
            qualifications,
        } if type_identity == &result.type_identity && qualifications.is_empty()
    ) {
        return unsupported("closed-sum Unit source boundary result drifted");
    }
    let mut boundaries = Vec::new();
    retain_exact_unit_boundary(
        checked,
        plans,
        &mut boundaries,
        *target_machine,
        *target_state,
        *target_contract_report_fingerprint,
        *service_reach,
        expected_result,
    )?;

    let leaves = [first_leaf, second_leaf];
    for leaf in leaves {
        if !leaf.structural_parameters.is_empty()
            || !leaf.entry_claims.is_empty()
            || !leaf.bindings.is_empty()
            || !leaf.binding_initializers.is_empty()
            || leaf.operations.is_empty()
            || !matches!(
                leaf.terminator,
                CheckedComposedUnitControlTerminatorPlan::ReturnUnit
            )
        {
            return unsupported("closed-sum Unit leaf escaped the exact effect-and-return lane");
        }
        for operation in &leaf.operations {
            admission::retain_call_boundary(
                checked,
                plan.machine,
                leaf,
                operation,
                plans,
                &mut boundaries,
            )?;
        }
    }
    for case in cases {
        let leaf = leaves
            .iter()
            .copied()
            .find(|leaf| leaf.state == case.target_state)
            .ok_or(LoweringError::Unsupported(
                "closed-sum successor names a non-leaf state",
            ))?;
        if case.payloads.len() != leaf.scalar_parameters.len()
            || case.payloads.iter().any(|payload| {
                leaf.scalar_parameters
                    .get(payload.target_scalar_parameter_index as usize)
                    .is_none_or(|parameter| parameter.primitive_type != payload.primitive_type)
            })
        {
            return unsupported("closed-sum payload transfer drifted from its leaf signature");
        }
    }
    if cases[0].target_state == cases[1].target_state
        || entry.state == first_leaf.state
        || entry.state == second_leaf.state
        || first_leaf.state == second_leaf.state
    {
        return unsupported("closed-sum Unit control contains duplicate states");
    }
    admission::validate_contract(checked, plan)?;
    let attachment = admission::exact_attachment(checked, plan)?;
    boundaries.sort_by(|left, right| left.1.cmp(&right.1));
    boundaries.dedup_by(|left, right| left.1 == right.1);
    let called = std::iter::once(entry_call)
        .chain(leaves.into_iter().flat_map(|leaf| &leaf.operations))
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::BoundaryCall { target_machine, .. }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { target_machine, .. } => {
                Some(*target_machine)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    super::super::provider_attachments::validate_provider_attachment_requirements(
        attachment,
        &plan.provider_attachment_requirements,
        &called,
    )?;
    Ok(admission::AdmittedComposedUnit {
        entry,
        leaves: vec![first_leaf, second_leaf],
        boundaries,
        internal_targets: Vec::new(),
        custody: custody::ComposedCustody::Empty,
    })
}

fn emit(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedComposedUnitControlMachinePlan,
    admitted: admission::AdmittedComposedUnit<'_>,
    mut catalogs: catalogs::ComposedCatalogs,
) -> Result<SourceMappedLowered, LoweringError> {
    let entry = admitted.entry;
    let CheckedComposedUnitControlTerminatorPlan::ClosedSum { result, cases } = &entry.terminator
    else {
        unreachable!("closed-sum admission retained the exact entry terminator")
    };
    let [entry_call] = entry.operations.as_slice() else {
        unreachable!("closed-sum admission retained one entry operation")
    };
    let CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
        coordinate,
        source_site,
        target_machine,
        scalar_arguments,
        structural_arguments,
        completion_receipts,
        ..
    } = entry_call
    else {
        unreachable!("closed-sum admission retained a structural boundary call")
    };
    let target = catalogs
        .lowered_boundaries
        .iter()
        .find(|boundary| boundary.source == *target_machine)
        .ok_or(LoweringError::Unsupported(
            "closed-sum source boundary is absent from its catalog",
        ))?;
    let BoundaryMachineResult::Structural(boundary_result) = &target.result else {
        return unsupported("closed-sum source boundary lost its structural result");
    };
    let boundary_result = boundary_result.clone();
    if !scalar_arguments.is_empty()
        || !structural_arguments.is_empty()
        || !completion_receipts.is_empty()
        || boundary_result.multiplicity != StructuralMultiplicity::Affine
        || !boundary_result.qualifications.is_empty()
        || result.type_identity
            != catalogs
                .structural_types
                .iter()
                .find(|declaration| declaration.id == boundary_result.structural_type)
                .map(|declaration| declaration.identity.as_str())
                .unwrap_or_default()
    {
        return unsupported("closed-sum source boundary catalog drifted");
    }

    let mut next_place = catalogs.next_place;
    let result_place = place_id(allocate_dense(&mut next_place)?);
    let mut entry_operations = OperationBuffer::new(0);
    let operation = entry_operations.allocate();
    entry_operations.record_source_call(
        SourceCallCoordinate {
            state: entry.state,
            statement_index: coordinate.statement_index as usize,
            call_ordinal: coordinate.call_ordinal as usize,
        },
        *source_site,
        operation,
        *target_machine,
    )?;
    entry_operations.push(Operation {
        id: operation,
        result: OperationResult::Structural(StructuralOperationResult {
            place: result_place,
            structural_type: boundary_result.structural_type,
            multiplicity: boundary_result.multiplicity,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::BoundaryCall {
            boundary: target.id,
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            completion_receipts: Vec::new(),
        },
    });

    let state_ids = [block_id(1), block_id(2), block_id(3)];
    let mut next_value = 1_u64;
    let mut next_edge = 1_u64;
    let mut next_operation = entry_operations.next_identity;
    let declaration = catalogs
        .structural_types
        .iter()
        .find(|declaration| declaration.id == boundary_result.structural_type)
        .ok_or(LoweringError::Unsupported(
            "closed-sum result type is absent from its catalog",
        ))?;
    let declared_cases = match &declaration.shape {
        StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => cases,
        _ => return unsupported("closed-sum result declaration is not a closed sum"),
    };

    let mut blocks = Vec::with_capacity(3);
    let leaf_parameters = admitted
        .leaves
        .iter()
        .map(|leaf| {
            leaf.scalar_parameters
                .iter()
                .map(|parameter| {
                    let declaration = ValueDeclaration {
                        id: value_id(allocate_dense(&mut next_value)?),
                        scalar_type: terminal_scalar_type(parameter.primitive_type)?,
                    };
                    Ok(declaration)
                })
                .collect::<Result<Vec<_>, LoweringError>>()
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let successors = declared_cases
        .iter()
        .map(|declared| {
            let case = cases
                .iter()
                .find(|case| case.case_identity == declared.identity)
                .ok_or(LoweringError::Unsupported(
                    "closed-sum declared case lost its checked successor",
                ))?;
            let leaf_index = admitted
                .leaves
                .iter()
                .position(|leaf| leaf.state == case.target_state)
                .ok_or(LoweringError::Unsupported(
                    "closed-sum successor lost its admitted leaf",
                ))?;
            let mut payload_fields = vec![None; case.payloads.len()];
            for payload in &case.payloads {
                let field = declared
                    .fields
                    .iter()
                    .find(|field| field.identity == payload.field_identity)
                    .ok_or(LoweringError::Unsupported(
                        "closed-sum payload names an unknown field identity",
                    ))?;
                let slot = payload.target_scalar_parameter_index as usize;
                if slot >= payload_fields.len() || payload_fields[slot].replace(field.id).is_some()
                {
                    return unsupported("closed-sum payload binding order is noncanonical");
                }
            }
            Ok(StructuralCaseSuccessorEdge {
                edge: edge_id(allocate_dense(&mut next_edge)?),
                target: state_ids[leaf_index + 1],
                case: declared.id,
                payload_fields: payload_fields
                    .into_iter()
                    .collect::<Option<Vec<_>>>()
                    .ok_or(LoweringError::Unsupported(
                        "closed-sum payload binding roster is incomplete",
                    ))?,
                trivial_affine_discards: vec![result_place],
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let mut source_calls = entry_operations.source_calls;
    blocks.push(Block {
        id: state_ids[0],
        parameters: Vec::new(),
        operations: entry_operations.operations,
        terminator: Terminator::StructuralCase {
            source: result_place,
            cases: successors,
        },
    });
    let mut next_block = dense_identity(state_ids.len())?;
    for ((leaf, parameters), block) in admitted
        .leaves
        .into_iter()
        .zip(leaf_parameters)
        .zip(&state_ids[1..])
    {
        let (fragment, mut occurrences) = emission::emit_call_leaf(
            checked,
            plan.machine,
            leaf,
            *block,
            &mut catalogs,
            &[],
            &[],
            &parameters,
            &mut next_value,
            &mut next_block,
            &mut next_operation,
            &mut next_edge,
        )?;
        blocks.extend(fragment);
        source_calls.append(&mut occurrences);
    }

    let attachment = lookup_type_id(&catalogs.type_ids, &plan.attachment_type_identity)?;
    let attachment_declaration = catalogs
        .structural_types
        .iter()
        .find(|declaration| declaration.id == attachment)
        .expect("closed-sum attachment declaration was selected");
    let provider_boundaries = catalogs
        .lowered_boundaries
        .iter()
        .map(|boundary| (boundary.source, boundary.id))
        .collect::<Vec<_>>();
    let mut structural_places = vec![StructuralPlaceDeclaration {
        id: result_place,
        kind: StructuralPlaceKind::OperationResult {
            producer: operation,
            structural_type: boundary_result.structural_type,
        },
    }];
    structural_places.extend(
        super::super::provider_attachments::lower_provider_attachment_places(
            attachment,
            attachment_declaration,
            &plan.provider_attachment_requirements,
            &provider_boundaries,
            &mut next_place,
        )?,
    );
    let machine = TerminalMachine {
        id: machine_id(1),
        attachment: Some(attachment),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places,
        entry_claims: Vec::new(),
        published_service_ceiling: lower_installation_machine_service_ceiling(
            checked,
            plan.machine,
            plan.contract_service_reach,
            plan.service_reach,
            &catalogs.service_ids,
        )?,
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: state_ids[0],
        blocks,
        contract: MachineContract {
            id: contract_id(1),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    emission::finish_module(plan.machine, vec![machine], catalogs, source_calls)
}
