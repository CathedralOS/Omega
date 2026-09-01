//! Dedicated source-free lowering for one direct local dynamic scalar call.
//!
//! The caller's named dynamic value never escapes this closed use, so no
//! descriptor or private table is materialized. The emitted direct call still
//! retains the exact selected conformance application, source field subloan,
//! requirement row, and realization callable in Terminal custody.

use psi_checked_trees::{
    CheckedBooleanExpression, CheckedDirectDynamicScalarCallPlan, CheckedScalarExpression,
    CheckedStructuralAccess, CheckedStructuralPredicatePathSegment, CheckedUnitStructuralFieldType,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralTypeShape,
};
use psi_core::StructuralPlaceKind;
use psi_language_semantics::{Multiplicity, ServiceReachSummary};
use psi_terminal::{
    Block, ClosedConformanceApplication, ClosedConformanceCallableResult,
    ClosedConformanceRealizationCallable, ClosedConformanceRow, MachineContract, Operation,
    OperationKind, OperationResult, StructuralAccess, StructuralArgument, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, TerminalDirectDynamicDispatch,
    TerminalDynamicConformanceSelection, TerminalDynamicDispatchCatalog, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
    closed_conformance_application_commitment, closed_conformance_application_report_fingerprint,
};

use super::*;

struct DirectCallerShape {
    attachment_type_identity: String,
}

pub(super) fn lower_direct_dynamic_composed_unit_machine(
    checked: &CheckedTrees,
    plan: &CheckedDirectDynamicScalarCallPlan,
) -> Result<LoweredTerminalPsi, LoweringError> {
    let caller = validate_exact_direct_plan(checked, plan)?;
    let (structural_types, type_ids) =
        lower_direct_structural_types(checked, plan, &caller.attachment_type_identity)?;
    let caller_attachment = lookup_type_id(&type_ids, &caller.attachment_type_identity)?;
    let caller_self = StructuralParameterDeclaration {
        place: place_id(1),
        position: 0,
        is_self: true,
        structural_type: caller_attachment,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let caller_parameters = vec![caller_self.clone()];
    let source = validate_and_lower_source(&caller_self, plan, &structural_types, &type_ids)?;

    let caller_machine = machine_id(1);
    let realization_machine = machine_id(2);
    let call_operation = operation_id(1);
    let call_result_type = terminal_scalar_type(plan.result.primitive_type)?;
    let callable_result = terminal_callable_result(plan.result.primitive_type)?;
    let callable_identity =
        evidence_lowering::checked_evidence_machine_identity(checked, plan.realization_machine)?;
    if callable_identity != plan.realization_identity {
        return unsupported("direct dynamic realization callable identity drifted");
    }

    let (application, selected_row) = lower_exact_application(
        checked,
        plan,
        caller_machine,
        realization_machine,
        callable_result,
        &callable_identity,
    )?;
    let selection = TerminalDynamicConformanceSelection {
        owner: caller_machine,
        ordinal: 0,
        source: source.clone(),
        conformance_application_report_fingerprint: application.report_fingerprint,
        conformance_application_commitment: application.commitment,
    };
    let dispatch = TerminalDirectDynamicDispatch {
        owner: caller_machine,
        operation: call_operation,
        selection_ordinal: 0,
        declaring_trait_identity: selected_row.declaring_trait_identity.clone(),
        public_requirement_identity: selected_row.public_requirement_identity.clone(),
        requirement_identity: selected_row.requirement_identity.clone(),
        realization_identity: selected_row.realization_identity.clone(),
        realization_callable_identity: callable_identity,
        realization: realization_machine,
    };

    let source_type = lookup_type_id(&type_ids, &plan.source_type_identity)?;
    let realization_parameter = StructuralParameterDeclaration {
        place: place_id(2),
        position: 0,
        is_self: true,
        structural_type: source_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };

    let realization_operations = lower_realization_operations(
        &plan.realization_return_expression,
        call_result_type,
        &realization_parameter,
        &structural_types,
    )?;
    let realization_value = realization_operations
        .last()
        .and_then(|operation| operation.result.scalar())
        .map(|result| result.id)
        .ok_or(LoweringError::Unsupported(
            "direct dynamic realization did not emit one scalar result",
        ))?;

    let caller_block = block_id(1);
    let realization_block = block_id(2);
    let caller_reach = lower_installation_machine_service_ceiling(
        checked,
        plan.caller_machine,
        checked
            .facts
            .service_reaches
            .plan_for_machine(plan.caller_machine)
            .ok_or(LoweringError::Unsupported(
                "direct dynamic caller has no checked service contract",
            ))?,
        exact_machine_service_summary(checked, plan.caller_machine)?,
        &[],
    )?;
    let realization_reach = exact_empty_machine_service_ceiling(
        checked,
        plan.realization_machine,
        plan.checked_call_service_reach,
    )?;
    let root_service_reach = lower_root_service_reach(checked, plan.caller_machine, &[])?;

    Ok(LoweredTerminalPsi {
        semantic_module: TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: caller_machine,
            structural_types,
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach,
            placed_view_inputs: Vec::new(),
            reborrow_root_handoffs: Vec::new(),
            reborrow_restored_call_uses: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            proof_output_calls: Vec::new(),
            proof_recursive_components: Vec::new(),
            closed_conformance_applications: vec![application],
            dynamic_dispatch: TerminalDynamicDispatchCatalog {
                selections: vec![selection],
                direct_dispatches: vec![dispatch],
            },
            quotient_correspondences: Vec::new(),
            machines: vec![
                TerminalMachine {
                    id: caller_machine,
                    attachment: Some(caller_attachment),
                    parameters: Vec::new(),
                    structural_parameters: caller_parameters.clone(),
                    ranked_scc: None,
                    result: TerminalMachineResult::Unit,
                    structural_places: caller_parameters
                        .iter()
                        .map(|parameter| StructuralPlaceDeclaration {
                            id: parameter.place,
                            kind: StructuralPlaceKind::Parameter {
                                position: parameter.position,
                                is_self: parameter.is_self,
                            },
                        })
                        .collect(),
                    entry_claims: Vec::new(),
                    published_service_ceiling: caller_reach,
                    content_entry_claims: Vec::new(),
                    content_identity_reshuffles: Vec::new(),
                    content_partition_compositions: Vec::new(),
                    entry: caller_block,
                    blocks: vec![Block {
                        id: caller_block,
                        parameters: Vec::new(),
                        operations: vec![Operation {
                            id: call_operation,
                            result: OperationResult::Scalar(ValueDeclaration {
                                id: value_id(1),
                                scalar_type: call_result_type,
                            }),
                            kind: OperationKind::CallStructuralScalar {
                                callee: realization_machine,
                                structural_arguments: vec![source],
                                claim_transfers: Vec::new(),
                                requirement_obligations: Vec::new(),
                                crash_continuations: Vec::new(),
                            },
                        }],
                        terminator: Terminator::ReturnUnit {
                            edge: edge_id(1),
                            trivial_affine_discards: Vec::new(),
                        },
                    }],
                    contract: empty_terminal_contract(caller_machine.get()),
                },
                TerminalMachine {
                    id: realization_machine,
                    attachment: Some(source_type),
                    parameters: Vec::new(),
                    structural_parameters: vec![realization_parameter.clone()],
                    ranked_scc: None,
                    result: TerminalMachineResult::Scalar(ValueDeclaration {
                        id: value_id(2),
                        scalar_type: call_result_type,
                    }),
                    structural_places: vec![StructuralPlaceDeclaration {
                        id: realization_parameter.place,
                        kind: StructuralPlaceKind::Parameter {
                            position: realization_parameter.position,
                            is_self: realization_parameter.is_self,
                        },
                    }],
                    entry_claims: Vec::new(),
                    published_service_ceiling: realization_reach,
                    content_entry_claims: Vec::new(),
                    content_identity_reshuffles: Vec::new(),
                    content_partition_compositions: Vec::new(),
                    entry: realization_block,
                    blocks: vec![Block {
                        id: realization_block,
                        parameters: Vec::new(),
                        operations: realization_operations,
                        terminator: Terminator::Return {
                            cleanup_actions: Vec::new(),
                            edge: edge_id(2),
                            value: realization_value,
                        },
                    }],
                    contract: empty_terminal_contract(realization_machine.get()),
                },
            ],
        },
        proof_bundle: ProofBundle {
            recursive_components: Vec::new(),
            evidence_producers: Vec::new(),
            evidence: Vec::new(),
        },
        debug_map: None,
        source_call_occurrences: vec![LoweredSourceCallOccurrence {
            source_site: None,
            source_state: plan.caller_state,
            statement_index: usize::try_from(plan.coordinate.statement_index).map_err(|_| {
                LoweringError::Unsupported("direct dynamic statement coordinate exceeds usize")
            })?,
            call_ordinal: usize::try_from(plan.coordinate.call_ordinal).map_err(|_| {
                LoweringError::Unsupported("direct dynamic call ordinal exceeds usize")
            })?,
            terminal_operation: call_operation,
            source_target: plan.requirement,
        }],
        selected_ieee_float_fma_occurrences: Vec::new(),
    })
}

fn validate_exact_direct_plan(
    checked: &CheckedTrees,
    plan: &CheckedDirectDynamicScalarCallPlan,
) -> Result<DirectCallerShape, LoweringError> {
    let exact_selections = checked
        .facts
        .dynamic_conformances
        .binding_facts()
        .selections
        .into_iter()
        .filter(|selection| selection == &plan.selection)
        .count();
    if exact_selections != 1
        || plan.selection.machine != plan.caller_machine
        || plan.selection.state != plan.caller_state
        || plan.selection.binding != plan.receiver_binding
        || plan.selection.target_trait != plan.target_trait
        || plan.selection.conformance != Some(plan.selected_conformance)
        || plan.selection.source_symbol != plan.source_field
        || plan.selection.statement_index != 0
        || plan.coordinate.statement_index != 1
        || plan.coordinate.call_ordinal != 0
        || plan.result.statement_index != plan.coordinate.statement_index
        || plan.result.binding_ordinal != 0
        || plan.selection.statement_index
            >= usize::try_from(plan.coordinate.statement_index).map_err(|_| {
                LoweringError::Unsupported("direct dynamic statement coordinate exceeds usize")
            })?
    {
        return unsupported("direct dynamic dispatch plan no longer matches its checked selection");
    }
    let selected_rows = plan
        .selection
        .rows
        .iter()
        .filter(|row| {
            row.declaring_trait == plan.declaring_trait
                && row.requirement == plan.requirement
                && row.realization_machine == plan.realization_machine
                && row.realization_state == plan.realization_state
                && row.requirement_identity == plan.requirement_identity
                && row.realization_identity == plan.realization_identity
        })
        .count();
    if selected_rows != 1 {
        return unsupported("direct dynamic dispatch lost its exact selected conformance row");
    }
    if checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(plan.caller_machine)
        .is_some()
        || checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_for_machine(plan.caller_machine)
            .is_some()
    {
        return unsupported("direct dynamic caller overlaps another checked Unit route");
    }
    let state_facts = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .filter_map(|(_, state)| {
            (state.machine_symbol == plan.caller_machine && state.state_symbol == plan.caller_state)
                .then_some(state)
        })
        .collect::<Vec<_>>();
    let [state] = state_facts.as_slice() else {
        return unsupported("direct dynamic caller has no exact checked flow state");
    };
    let calls = checked.facts.flow.control.calls.span_or_empty(state.calls);
    let [call] = calls else {
        return unsupported("direct dynamic caller must contain one checked call");
    };
    if call.statement_index != plan.coordinate.statement_index as usize
        || call.call_ordinal != plan.coordinate.call_ordinal as usize
        || call.receiver_symbol != plan.receiver_binding
        || call.target_symbol != plan.requirement
        || !call.has_receiver
        || call.service_reach != plan.checked_call_service_reach
        || checked
            .facts
            .flow
            .control
            .statements
            .span_or_empty(state.statements)
            .len()
            != 2
    {
        return unsupported("direct dynamic call drifted from checked flow custody");
    }
    validate_empty_contract(
        checked,
        plan.caller_machine,
        plan.caller_contract_report_fingerprint,
        plan.caller_contract_commitment,
    )?;
    validate_empty_contract(
        checked,
        plan.realization_machine,
        plan.realization_contract_report_fingerprint,
        plan.realization_contract_commitment,
    )?;
    if plan.source_parameter_position != 0
        || plan.caller_multiplicity != Multiplicity::Unrestricted
        || plan.source_multiplicity != Multiplicity::Unrestricted
        || plan.caller_parameter_access != CheckedStructuralAccess::SharedBorrow
        || plan.source_access != CheckedStructuralAccess::SharedBorrow
    {
        return unsupported("direct dynamic source must be an exact shared field subloan");
    }
    let [CheckedUnitStructuralPathSegment::Field(_)] = plan.source_path.as_slice() else {
        return unsupported("direct dynamic source must be one exact attachment field");
    };
    validate_empty_service_summary(checked, plan.checked_call_service_reach)?;
    let caller_service_reach = exact_machine_service_summary(checked, plan.caller_machine)?;
    if caller_service_reach != plan.caller_service_reach {
        return unsupported("direct dynamic caller service reach drifted from checking");
    }
    validate_empty_service_summary(checked, caller_service_reach)?;
    Ok(DirectCallerShape {
        attachment_type_identity: plan.caller_attachment_type_identity.clone(),
    })
}

fn validate_and_lower_source(
    caller_self: &StructuralParameterDeclaration,
    plan: &CheckedDirectDynamicScalarCallPlan,
    structural_types: &[psi_terminal::StructuralTypeDeclaration],
    type_ids: &[(String, psi_core::StructuralTypeId)],
) -> Result<StructuralArgument, LoweringError> {
    if plan.source_parameter_position != caller_self.position
        || plan.caller_parameter_access != CheckedStructuralAccess::SharedBorrow
        || !caller_self.is_self
        || caller_self.multiplicity != StructuralMultiplicity::Unrestricted
        || caller_self.access != StructuralAccess::SharedBorrow
    {
        return unsupported("direct dynamic caller self does not license a shared field subloan");
    }
    let attachment_id = lookup_type_id(type_ids, &plan.caller_attachment_type_identity)?;
    let source_type = lookup_type_id(type_ids, &plan.source_type_identity)?;
    let attachment = structural_types
        .iter()
        .find(|declaration| declaration.id == attachment_id)
        .ok_or(LoweringError::Unsupported(
            "direct dynamic caller attachment declaration is absent",
        ))?;
    let [CheckedUnitStructuralPathSegment::Field(field_identity)] = plan.source_path.as_slice()
    else {
        unreachable!("direct source path was validated")
    };
    let psi_terminal::StructuralTypeShape::Record { fields } = &attachment.shape else {
        return unsupported("direct dynamic caller attachment must be a record");
    };
    let matching_fields = fields
        .iter()
        .filter(|field| {
            field.identity == *field_identity
                && field.field_type == psi_terminal::StructuralFieldType::Structural(source_type)
        })
        .count();
    if matching_fields != 1 {
        return unsupported("direct dynamic source field no longer matches its structural carrier");
    }
    Ok(StructuralArgument {
        place: caller_self.place,
        path: lower_structural_path(&plan.source_path),
        access: StructuralAccess::SharedBorrow,
    })
}

fn lower_direct_structural_types(
    checked: &CheckedTrees,
    plan: &CheckedDirectDynamicScalarCallPlan,
    caller_attachment: &str,
) -> Result<
    (
        Vec<psi_terminal::StructuralTypeDeclaration>,
        Vec<(String, psi_core::StructuralTypeId)>,
    ),
    LoweringError,
> {
    if caller_attachment != plan.caller_attachment_type_identity {
        return unsupported("direct dynamic caller attachment identity drifted");
    }
    let roots = &checked.facts.flow.terminal_unit_effects.structural_types;
    let caller_roots = roots
        .iter()
        .filter(|candidate| candidate.identity == caller_attachment)
        .collect::<Vec<_>>();
    let [caller] = caller_roots.as_slice() else {
        return unsupported("direct dynamic caller attachment shape is absent or ambiguous");
    };
    let [CheckedUnitStructuralPathSegment::Field(source_field)] = plan.source_path.as_slice()
    else {
        return unsupported("direct dynamic source must be one exact attachment field");
    };
    let CheckedUnitStructuralTypeShape::Record { fields } = &caller.shape else {
        return unsupported("direct dynamic caller attachment must be a record");
    };
    let matching_fields = fields
        .iter()
        .filter(|field| {
            field.identity == *source_field
                && field.field_type
                    == CheckedUnitStructuralFieldType::Structural {
                        type_identity: plan.source_type_identity.clone(),
                    }
        })
        .count();
    if matching_fields != 1 {
        return unsupported("direct dynamic checked source field no longer matches its carrier");
    }
    attached_unit::lower_unit_structural_type_roots(
        checked,
        &[
            caller_attachment.to_owned(),
            plan.source_type_identity.clone(),
        ],
    )
}

#[allow(clippy::too_many_arguments)]
fn lower_exact_application(
    checked: &CheckedTrees,
    plan: &CheckedDirectDynamicScalarCallPlan,
    owner: psi_core::MachineId,
    realization: psi_core::MachineId,
    callable_result: ClosedConformanceCallableResult,
    callable_identity: &str,
) -> Result<(ClosedConformanceApplication, ClosedConformanceRow), LoweringError> {
    let conformances = checked
        .typed
        .conformances()
        .iter()
        .filter(|conformance| conformance.symbol == plan.selected_conformance)
        .collect::<Vec<_>>();
    let [conformance] = conformances.as_slice() else {
        return unsupported("direct dynamic selection lost its exact conformance declaration");
    };
    let traits = checked
        .typed
        .traits()
        .iter()
        .filter(|definition| definition.symbol == plan.target_trait)
        .collect::<Vec<_>>();
    let [target_trait] = traits.as_slice() else {
        return unsupported("direct dynamic selection lost its exact target trait");
    };
    if conformance.carrier_symbol != plan.selection.source_data
        || conformance.trait_symbol != plan.target_trait
        || !conformance.lifetime_parameters.is_empty()
        || !checked
            .typed
            .conformance_type_parameters(conformance)
            .is_empty()
        || !checked
            .typed
            .type_reference_table
            .type_reference_handles(conformance.arguments)
            .is_empty()
        || !conformance.trait_lifetime_arguments.is_empty()
        || !target_trait.lifetime_parameters.is_empty()
        || !checked.typed.trait_type_parameters(target_trait).is_empty()
    {
        return unsupported("generic dynamic conformance applications require a later producer");
    }
    let closed_rows =
        checked
            .typed
            .closed_conformance_rows(conformance)
            .ok_or(LoweringError::Unsupported(
                "direct dynamic selection is not a closed conformance",
            ))?;
    if closed_rows.len() != plan.selection.rows.len() {
        return unsupported("direct dynamic selection row map is incomplete");
    }

    let mut rows = Vec::with_capacity(closed_rows.len());
    let mut selected_row = None;
    for (closed, retained) in closed_rows.iter().zip(&plan.selection.rows) {
        let requirement_identity = evidence_lowering::checked_evidence_requirement_identity(
            checked,
            closed.declaring_trait,
            closed.requirement,
        )?;
        let realization_identity = evidence_lowering::checked_evidence_machine_identity(
            checked,
            closed.realization_machine,
        )?;
        if closed.declaring_trait != retained.declaring_trait
            || closed.requirement != retained.requirement
            || closed.realization_machine != retained.realization_machine
            || closed.realization_state != retained.realization_state
            || requirement_identity != retained.requirement_identity
            || realization_identity != retained.realization_identity
        {
            return unsupported("direct dynamic selection row map drifted from checking");
        }
        let selected = closed.declaring_trait == plan.declaring_trait
            && closed.requirement == plan.requirement
            && closed.realization_machine == plan.realization_machine
            && closed.realization_state == plan.realization_state;
        let row = ClosedConformanceRow {
            declaring_trait_identity: checked.symbols.display_path(closed.declaring_trait, "::"),
            public_requirement_identity: requirement_identity,
            requirement_identity: checked.symbols.display_path(closed.requirement, "::"),
            realization_identity: checked.symbols.display_path(closed.realization_state, "::"),
            realization_callable_identity: selected.then(|| callable_identity.to_owned()),
        };
        if selected && selected_row.replace(row.clone()).is_some() {
            return unsupported("direct dynamic selected row is duplicated");
        }
        rows.push(row);
    }
    let selected_row = selected_row.ok_or(LoweringError::Unsupported(
        "direct dynamic selected row is absent",
    ))?;
    if selected_row.public_requirement_identity != plan.requirement_identity {
        return unsupported("direct dynamic public requirement identity drifted");
    }

    let mut application = ClosedConformanceApplication {
        owner,
        declaration_identity: checked
            .symbols
            .display_path(plan.selected_conformance, "::"),
        telescope: Vec::new(),
        subject_identity: Some(plan.source_type_identity.clone()),
        trait_identity: checked.symbols.display_path(plan.target_trait, "::"),
        trait_lifetime_arguments: Vec::new(),
        trait_arguments: Vec::new(),
        realization_callables: vec![ClosedConformanceRealizationCallable {
            source_callable_identity: callable_identity.to_owned(),
            machine: realization,
            result: callable_result,
        }],
        rows,
        report_fingerprint: 0,
        commitment: Default::default(),
    };
    application.report_fingerprint =
        closed_conformance_application_report_fingerprint(&application);
    application.commitment = closed_conformance_application_commitment(&application);
    Ok((application, selected_row))
}

fn validate_empty_contract(
    checked: &CheckedTrees,
    machine: psi_symbols::SymbolHandle,
    report_fingerprint: u64,
    commitment: psi_checked_trees::MachineContractCommitment,
) -> Result<(), LoweringError> {
    let contract =
        checked
            .facts
            .contract_plans
            .for_machine(machine)
            .ok_or(LoweringError::Unsupported(
                "direct dynamic machine has no exact checked contract",
            ))?;
    if report_fingerprint == 0
        || commitment.is_zero()
        || contract.report_fingerprint != report_fingerprint
        || contract.commitment != commitment
        || !contract.closed_scalar_values.requires().is_empty()
        || !contract.closed_scalar_values.ensures().is_empty()
        || contract.closed_scalar_values.has_crash_clauses()
        || contract.closed_scalar_values.has_outcome_specific_clauses()
        || !contract.crash.published().is_empty()
    {
        return unsupported("direct dynamic machine requires an unsupported contract lane");
    }
    Ok(())
}

fn validate_empty_service_summary(
    checked: &CheckedTrees,
    summary: ServiceReachSummary,
) -> Result<(), LoweringError> {
    let mut services = Vec::new();
    collect_service_summary(&checked.facts.service_reaches.rows, summary, &mut services)?;
    if !services.is_empty() {
        return unsupported("direct dynamic scalar call with service reach is unsupported");
    }
    Ok(())
}

fn exact_machine_service_summary(
    checked: &CheckedTrees,
    machine: psi_symbols::SymbolHandle,
) -> Result<ServiceReachSummary, LoweringError> {
    let fact =
        checked
            .facts
            .service_reaches
            .for_machine(machine)
            .ok_or(LoweringError::Unsupported(
                "direct dynamic machine has no checked service reach",
            ))?;
    Ok(ServiceReachSummary {
        direct: fact.inferred_direct,
        transitive: fact.inferred_transitive,
    })
}

fn exact_empty_machine_service_ceiling(
    checked: &CheckedTrees,
    machine: psi_symbols::SymbolHandle,
    call: ServiceReachSummary,
) -> Result<Vec<psi_core::ServiceId>, LoweringError> {
    let summary = exact_machine_service_summary(checked, machine)?;
    validate_empty_service_summary(checked, summary)?;
    let contract = checked
        .facts
        .service_reaches
        .plan_for_machine(machine)
        .ok_or(LoweringError::Unsupported(
            "direct dynamic realization has no service contract",
        ))?;
    if !checked_unit_target_reach_matches(call, contract) {
        return unsupported("direct dynamic call reach drifted from its selected realization");
    }
    lower_installation_machine_service_ceiling(checked, machine, contract, summary, &[])
}

fn terminal_callable_result(
    primitive: PrimitiveType,
) -> Result<ClosedConformanceCallableResult, LoweringError> {
    match primitive {
        PrimitiveType::Bool => Ok(ClosedConformanceCallableResult::Bool),
        _ => unsupported("direct dynamic Terminal custody currently admits only Bool"),
    }
}

fn lower_realization_operations(
    expression: &CheckedScalarExpression,
    expected: psi_core::ScalarType,
    parameter: &StructuralParameterDeclaration,
    structural_types: &[psi_terminal::StructuralTypeDeclaration],
) -> Result<Vec<Operation>, LoweringError> {
    if let CheckedScalarExpression::Boolean(boolean) = expression
        && let CheckedBooleanExpression::StructuralParameterField {
            parameter_position,
            path,
        } = boolean.as_ref()
    {
        let [CheckedStructuralPredicatePathSegment::Field(field_identity)] = path.as_slice() else {
            return unsupported("direct dynamic realization field path is unsupported");
        };
        if *parameter_position != 0 || expected != psi_core::ScalarType::Boolean {
            return unsupported("direct dynamic realization field result does not match self");
        }
        let declaration = structural_types
            .iter()
            .find(|declaration| declaration.id == parameter.structural_type)
            .ok_or(LoweringError::Unsupported(
                "direct dynamic realization self type is absent",
            ))?;
        let psi_terminal::StructuralTypeShape::Record { fields } = &declaration.shape else {
            return unsupported("direct dynamic realization self must be a record");
        };
        let matching = fields
            .iter()
            .filter(|field| {
                field.identity == *field_identity
                    && field.field_type
                        == psi_terminal::StructuralFieldType::Scalar(psi_core::ScalarType::Boolean)
            })
            .collect::<Vec<_>>();
        let [field] = matching.as_slice() else {
            return unsupported("direct dynamic realization Boolean field is absent or ambiguous");
        };
        return Ok(vec![Operation {
            id: operation_id(2),
            result: OperationResult::Scalar(ValueDeclaration {
                id: value_id(3),
                scalar_type: psi_core::ScalarType::Boolean,
            }),
            kind: OperationKind::BooleanStructuralField {
                source: parameter.place,
                field: field.id,
            },
        }]);
    }

    unsupported("direct dynamic realization must return one exact Boolean self field")
}

fn empty_terminal_contract(identity: u64) -> MachineContract {
    MachineContract {
        id: contract_id(identity),
        crash_routes: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
        outcome_specific_ensures: Vec::new(),
    }
}
