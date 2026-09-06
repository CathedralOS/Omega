use crate::borrow::build_borrow_facts;
use crate::capabilities::build_capability_facts;
use crate::flow::{build_domain_facts, build_flow_facts_with_service_reaches};
use crate::operators::{
    bind_boundary_operator_application_demands, build_operator_facts,
    select_pending_domain_operator_meanings,
};
use crate::proof::build_proof_facts_with_operators;
use crate::semantic::build_semantic_facts;
use crate::values::build_value_facts;
use checked_trees::CheckFacts;
use flow_effects::OperationalPlan;
use proof::obligations::ProofPlan;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

mod carry;
mod crash_calls;
mod index_compatibility;
#[cfg(test)]
mod scalar_contract_tests;

pub(crate) use crash_calls::{infer_checked_crash_causes, infer_checked_machine_crash_causes};

#[derive(Clone, Copy)]
struct MachineSuspensionRow {
    symbol: SymbolHandle,
    transitive_may_suspend: bool,
}

#[derive(Clone, Copy)]
struct MachineBlockingRow {
    symbol: SymbolHandle,
    transitive_may_block: bool,
}

fn project_operational_rows(
    operational: &OperationalPlan,
) -> (Vec<MachineSuspensionRow>, Vec<MachineBlockingRow>) {
    let suspensions = operational
        .machines()
        .iter()
        .map(|summary| MachineSuspensionRow {
            symbol: summary.symbol,
            transitive_may_suspend: summary.transitive_may_suspend,
        })
        .collect();
    let blocking = operational
        .machines()
        .iter()
        .map(|summary| MachineBlockingRow {
            symbol: summary.symbol,
            transitive_may_block: summary.transitive_may_block,
        })
        .collect();
    (suspensions, blocking)
}

pub(crate) fn build_check_facts(
    program: &TypedTrees,
    proof_plan: &ProofPlan<'_>,
    operational: OperationalPlan,
    validation_facts: &validation::ProgramValidationFacts,
    nominal_machine_uses: Vec<validation::ValidatedNominalMachineUse>,
) -> Result<CheckFacts, Vec<diagnostics::Diagnostic>> {
    let borrow = build_borrow_facts(program);
    let mut values = build_value_facts(program, proof_plan);
    let mut operators = build_operator_facts(program, &values);
    let mut proof = build_proof_facts_with_operators(program, proof_plan, &borrow, &operators);
    crate::proof::bind_float_meaning_projection_facts(
        program,
        &mut proof,
        &validation_facts.float_meaning_projection_invocations,
        &validation_facts.float_meaning_equality_propositions,
    )?;
    crate::proof::bind_proof_output_call_facts(program, &mut proof)?;
    crate::proof::bind_evidence_forwarding_facts(program, &mut proof)?;
    crate::proof::bind_outcome_specific_arm_facts(program, &mut proof)?;
    crate::proof::bind_evidence_projection_facts(program, &mut proof)?;
    let mut semantic = build_semantic_facts(program, &proof);
    let domains = build_domain_facts(program, &semantic);
    let dynamic_conformances = build_dynamic_conformance_facts(program)?;
    let service_reach_inference = validation::infer_service_reaches(program, &operational);
    // Meaning selection depends only on declarations and signatures. Complete
    // selected scalar plans before flow captures their evaluated local values.
    select_pending_domain_operator_meanings(program, &mut operators);
    bind_boundary_operator_application_demands(
        program,
        &validation_facts.boundary_operator_applications,
        &mut operators,
    )?;
    values.scalar_expressions = crate::values::build_checked_scalar_expression_plans(
        program,
        &operators,
        &validation_facts.exact_integer_casts,
    );
    let mut flow = build_flow_facts_with_service_reaches(
        program,
        &borrow,
        &proof,
        &mut semantic,
        &domains,
        &operational,
        &service_reach_inference,
        &values.scalar_expressions,
    );
    crate::review_sources::bind_checked_body_call_source_spans(program, &mut flow)?;
    values.scalar_computations = crate::values::build_checked_scalar_computation_plans(
        program,
        &operators,
        &flow,
        &proof,
        &values.scalar_expressions,
        &validation_facts.exact_integer_casts,
    );
    let index_compatibility = index_compatibility::build_index_compatibility_facts(
        program, &operators, &semantic, &flow,
    )?;
    flow.terminal_scalar_graphs = crate::flow::build_checked_scalar_graph_plans(
        program,
        &values.scalar_expressions,
        &values.scalar_computations,
    );
    flow.terminal_machines = crate::flow::build_checked_terminal_machine_selections(program);
    flow.terminal_debug = crate::flow::build_checked_terminal_debug_plans(program);
    let capabilities = build_capability_facts(program, &service_reach_inference, &flow);
    let (machine_suspensions, machine_blocking) = project_operational_rows(&operational);
    // EFX: the durable service-reach fixed point is built independently from
    // resolved boundary-trait identities and stored as a first-class checked
    // root with grouped machine/state/call arenas.
    let service_reaches = build_service_reach_facts(program, service_reach_inference);
    // STR4 checked plans, slice 2: semantic-domain commitments per machine.
    let qualifications = build_qualification_facts(program);
    // EFX: direct synchronous invocation is a separately published checked
    // axis, never reconstructed from service reach or flow-call topology.
    let synchronous_invocations = build_synchronous_invocation_facts(program);
    // EFX: suspension is published independently from worker blocking and
    // retains public negative guarantees separately from private inference.
    let suspensions = build_suspension_facts(program, &machine_suspensions);
    // EFX: worker blocking is published independently from suspension and
    // retains public negative guarantees separately from private inference.
    let blocking = build_blocking_facts(program, &machine_blocking);
    // TPR/EFX: termination is published as an independent exact-machine root.
    let termination = build_termination_facts(program, &flow, &semantic, validation_facts)?;
    // R5/STR: body-derived mutation frames are an independent checked axis,
    // never a field of the published machine contract.
    let mutation = build_mutation_facts(program);
    // STR4 checked plans: the normalized machine contracts (published
    // halves + fingerprint; prover-independent by construction).
    let contract_plans = build_contract_plans(
        program,
        &service_reaches,
        &synchronous_invocations,
        &suspensions,
        &blocking,
        &termination,
        &mutation,
        &capabilities,
        &flow,
        &operators,
        &validation_facts.exact_integer_casts,
    )?;
    proof.contract_entailment_assumption_discharges =
        crate::proof::build_contract_entailment_assumption_discharges(program, &contract_plans)?;
    let nominal_machine_uses =
        build_nominal_machine_use_facts(program, nominal_machine_uses, &contract_plans)?;
    // CRY1: materialize the effective structural policy once in the checked
    // fact layer; authored clauses remain minimum promises on typed data.
    let carry = carry::build_carry_facts(program);
    let mut fact_call_projections = Vec::new();
    let mut fact_call_projection_diagnostics = Vec::new();
    for call in &validation_facts.integer_embedding_calls {
        let exact_source = matches!(program.expression_table.expression(call.call_expression),
            typed_trees::expression::ExpressionNode::Call(source) if source.target_symbol == call.target_state)
            && program
                .machines()
                .iter()
                .filter(|machine| machine.symbol == call.target_machine)
                .filter(|machine| {
                    program
                        .machine_states(machine)
                        .iter()
                        .filter(|state| state.symbol == call.target_state)
                        .count()
                        == 1
                })
                .count()
                == 1;
        let total = termination.for_machine(call.target_machine).is_some_and(|plan| {
            matches!(&plan.checked_summary,
                language_semantics::TerminationGuarantee::Terminates { premises } if premises.is_empty())
        });
        if !exact_source || !total {
            let target = program.symbols.name(call.target_state);
            fact_call_projection_diagnostics.push(diagnostics::Diagnostic::error(format!(
                "`embed` source call `{target}` is not denotational: the exact selected machine is not unconditionally terminating"
            )));
        }
    }
    for projection in &validation_facts.fact_call_projections {
        let total = termination
            .for_machine(projection.target_machine)
            .is_some_and(|plan| {
                matches!(
                    &plan.checked_summary,
                    language_semantics::TerminationGuarantee::Terminates { premises }
                        if premises.is_empty()
                )
            });
        if !total {
            let target = program.symbols.name(projection.target_state);
            fact_call_projection_diagnostics.push(diagnostics::Diagnostic::error(format!(
                "fact-position projection from call `{target}` is not denotational: the selected machine is not unconditionally terminating"
            )));
            continue;
        }
        fact_call_projections.push(checked_trees::CheckedFactCallProjection {
            projection_expression: projection.projection_expression,
            call_expression: projection.call_expression,
            target_machine: projection.target_machine,
            target_state: projection.target_state,
            machine_arguments: projection.machine_arguments.clone(),
            result_type: projection.result_type,
            field: projection.field,
        });
    }
    if !fact_call_projection_diagnostics.is_empty() {
        return Err(fact_call_projection_diagnostics);
    }

    let mut facts = CheckFacts::with_roots(
        semantic,
        borrow,
        proof,
        values,
        domains,
        dynamic_conformances,
        nominal_machine_uses,
        operators,
        capabilities,
        flow,
        index_compatibility,
        mutation,
        service_reaches,
        synchronous_invocations,
        suspensions,
        blocking,
        termination,
        qualifications,
        contract_plans,
        carry,
        fact_call_projections,
    );
    facts.placed_view_inputs = build_checked_placed_view_inputs(program);
    Ok(facts)
}

fn build_checked_placed_view_inputs(
    program: &TypedTrees,
) -> Vec<checked_trees::CheckedPlacedViewInput> {
    let mut inputs = Vec::new();
    for machine in program.machines() {
        if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
            || !machine.body_is_present
            || !machine.lifetime_parameters.is_empty()
            || !program.machine_type_parameters(machine).is_empty()
        {
            continue;
        }
        for state in program.machine_states(machine) {
            for (position, parameter) in program.state_parameters(state).iter().enumerate() {
                let typed_trees::types::TypeReferenceNode::Reference {
                    referee,
                    access,
                    lifetime: _,
                } = program
                    .type_reference_table
                    .type_reference(parameter.type_reference)
                else {
                    continue;
                };
                let Some(view) = program.placed_view_plan_for_type_reference(*referee) else {
                    continue;
                };
                inputs.push(checked_trees::CheckedPlacedViewInput {
                    machine: machine.symbol,
                    state: state.symbol,
                    position: u32::try_from(position)
                        .expect("state parameter count must fit checked input position"),
                    parameter: parameter.symbol,
                    reference_access: *access,
                    binding_is_const: parameter.is_const,
                    binding_is_mutable: parameter.is_mutable,
                    view: view.data_symbol,
                    policy: view.policy_symbol,
                    policy_plan_machine: view.policy_plan_machine_symbol,
                    schema: view.schema_symbol,
                    placement: view.placement.clone(),
                });
            }
        }
    }
    inputs
}

/// Crash refinement gains path-conditioned and permission-frontier evidence
/// during checked-fact validation. Refresh only that independently mutable
/// axis so the realized envelope cannot retain the earlier pre-check snapshot.
pub(crate) fn refresh_realized_contract_envelopes(facts: &mut CheckFacts) {
    for envelope in &mut facts.contract_plans.realized_envelopes {
        let contract = facts
            .contract_plans
            .machines
            .iter()
            .find(|contract| contract.machine == envelope.machine)
            .expect("every realized envelope must retain its exact machine contract");
        assert_eq!(
            envelope.contract_report_fingerprint, contract.report_fingerprint,
            "realized envelope contract identity drifted during checked validation"
        );
        envelope.checked_crash = contract.crash.clone();
    }
    facts
        .contract_plans
        .validate_resource_envelopes()
        .expect("checked resource envelopes must survive independent post-validation replay");
}

fn build_nominal_machine_use_facts(
    program: &TypedTrees,
    nominal_machine_uses: Vec<validation::ValidatedNominalMachineUse>,
    contract_plans: &checked_trees::MachineContractPlans,
) -> Result<checked_trees::NominalMachineUseFacts, Vec<diagnostics::Diagnostic>> {
    let mut checked = Vec::with_capacity(nominal_machine_uses.len());
    for nominal_use in nominal_machine_uses {
        let Some(published) = contract_plans.crash_capsule(
            nominal_use.satisfaction_trait,
            nominal_use.satisfaction_requirement,
        ) else {
            return Err(vec![diagnostics::Diagnostic::error(
                "admitted nominal machine use is missing its published requirement contract identity",
            )]);
        };
        let Some(actual) = contract_plans.for_machine(nominal_use.selected_machine) else {
            return Err(vec![diagnostics::Diagnostic::error(
                "admitted nominal machine use is missing its selected machine contract identity",
            )]);
        };
        let Some(actual_envelope) = contract_plans.realized_envelope(nominal_use.selected_machine)
        else {
            return Err(vec![diagnostics::Diagnostic::error(
                "admitted nominal machine use is missing its realized contract envelope",
            )]);
        };
        let published_fingerprint = published.target_contract_report_fingerprint();
        let published_commitment = published.target_contract_commitment();
        let actual_fingerprint = actual.report_fingerprint;
        if published_commitment.is_zero()
            || actual.commitment.is_zero()
            || actual_envelope.contract_report_fingerprint != actual_fingerprint
            || actual_envelope.contract_commitment != actual.commitment
        {
            return Err(vec![diagnostics::Diagnostic::error(
                "admitted nominal machine use retained an empty contract-envelope identity",
            )]);
        }
        let callback_placement = match program.boundary_calling_plan_identity(
            nominal_use.satisfaction_trait,
            nominal_use.satisfaction_requirement,
        ) {
            Some(boundary_calling_plan_identity) => {
                let Some(resource_envelope) = contract_plans
                    .resource_envelope(nominal_use.selected_machine, nominal_use.selected_entry)
                else {
                    return Err(vec![diagnostics::Diagnostic::error(
                        "admitted nominal callback use is missing its exact checked entry resource envelope",
                    )]);
                };
                let resource_receipt =
                    checked_trees::CheckedCallbackResourceReceipt::try_from_entry_envelope(
                        resource_envelope,
                    )
                    .map_err(|error| {
                        vec![diagnostics::Diagnostic::error(format!(
                            "admitted nominal callback resource receipt failed checked replay: {error}"
                        ))]
                })?;
                Some(checked_trees::CheckedCallbackPlacementIdentity {
                    boundary_calling_plan_report_fingerprint: boundary_calling_plan_identity
                        .report_fingerprint,
                    boundary_calling_plan_commitment: boundary_calling_plan_identity.commitment,
                    resource_receipt,
                })
            }
            None => None,
        };
        if callback_placement
            .is_some_and(|placement| placement.boundary_calling_plan_commitment.is_zero())
        {
            return Err(vec![diagnostics::Diagnostic::error(
                "admitted nominal callback use is missing its evaluated boundary calling-plan identity",
            )]);
        }
        checked.push(checked_trees::CheckedNominalMachineUse {
            site: match nominal_use.site {
                validation::ValidatedNominalMachineUseSite::Statement(handle) => {
                    checked_trees::NominalMachineUseSite::Statement(handle)
                }
                validation::ValidatedNominalMachineUseSite::Expression(handle) => {
                    checked_trees::NominalMachineUseSite::Expression(handle)
                }
            },
            registration_operation: nominal_use.registration_operation,
            static_machine_ordinal: nominal_use.static_machine_ordinal,
            selected_machine: nominal_use.selected_machine,
            selected_entry: nominal_use.selected_entry,
            satisfaction_trait: nominal_use.satisfaction_trait,
            satisfaction_requirement: nominal_use.satisfaction_requirement,
            canonical_requirement_overload: nominal_use.canonical_requirement_overload,
            published_requirement_envelope: checked_trees::CheckedMachineContractEnvelopeIdentity {
                contract_report_fingerprint: published_fingerprint,
                contract_commitment: published_commitment,
            },
            selected_actual_envelope: checked_trees::CheckedMachineContractEnvelopeIdentity {
                contract_report_fingerprint: actual_fingerprint,
                contract_commitment: actual.commitment,
            },
            callback_placement,
            refinement: checked_trees::CheckedMachineContractRefinement {
                published_requirement_report_fingerprint: published_fingerprint,
                published_requirement_commitment: published_commitment,
                selected_actual_report_fingerprint: actual_fingerprint,
                selected_actual_commitment: actual.commitment,
            },
        });
    }
    checked_trees::NominalMachineUseFacts::try_with_uses(checked)
        .map_err(|message| vec![diagnostics::Diagnostic::error(message)])
}

fn build_termination_facts(
    program: &TypedTrees,
    flow: &checked_trees::FlowFacts,
    semantic: &facts::FactPlan,
    validation: &validation::ProgramValidationFacts,
) -> Result<checked_trees::TerminationFacts, Vec<diagnostics::Diagnostic>> {
    let summaries = crate::checks::termination::analyze_checked_progress(program, flow, semantic)?;
    Ok(checked_trees::TerminationFacts {
        machines: program
            .machines()
            .iter()
            .map(|machine| checked_trees::MachineTerminationFact {
                machine: machine.symbol,
                plan: crate::checks::termination::build_checked_termination_plan_with_summary(
                    program,
                    machine,
                    summaries
                        .iter()
                        .find(|summary| summary.machine == machine.symbol)
                        .map(|summary| summary.guarantee.clone())
                        .unwrap_or(language_semantics::TerminationGuarantee::NoGuarantee),
                ),
            })
            .collect(),
        build_bound_progress: summaries
            .into_iter()
            .filter(|summary| !summary.build_bound_demands.is_empty())
            .map(
                |summary| checked_trees::MachineBuildBoundProgressDemands {
                    machine: summary.machine,
                    demands: summary.build_bound_demands,
                },
            )
            .collect(),
        proof_recursive_components: validation
            .proof_recursive_components
            .iter()
            .map(
                |component| checked_trees::CheckedProofRecursiveComponent {
                    members: component
                        .members
                        .iter()
                        .map(|member| checked_trees::CheckedProofRecursiveMember {
                            machine: member.machine,
                            rank_parameter: member.rank_parameter,
                        })
                        .collect(),
                    ranking_relation: match component.ranking_relation {
                        validation::ValidatedProofRankingRelation::StructuralSubterm => {
                            checked_trees::CheckedProofRankingRelation::StructuralSubterm
                        }
                    },
                    rank_type_identity: component.rank_type_identity.clone(),
                    edges: component
                        .edges
                        .iter()
                        .map(|edge| checked_trees::CheckedProofRecursiveEdge {
                            caller: edge.caller,
                            callee: edge.callee,
                            site: match edge.site {
                                validation::ValidatedProofRecursiveCallSite::Statement {
                                    state,
                                    statement_index,
                                } => checked_trees::CheckedProofRecursiveCallSite::Statement {
                                    state,
                                    statement_index,
                                },
                                validation::ValidatedProofRecursiveCallSite::Expression {
                                    state,
                                    statement_index,
                                    expression_ordinal,
                                } => checked_trees::CheckedProofRecursiveCallSite::Expression {
                                    state,
                                    statement_index,
                                    expression_ordinal,
                                },
                                validation::ValidatedProofRecursiveCallSite::Transition {
                                    state,
                                    statement_index,
                                    lane,
                                } => checked_trees::CheckedProofRecursiveCallSite::Transition {
                                    state,
                                    statement_index,
                                    lane: match lane {
                                        validation::ValidatedProofRecursiveTransitionLane::Target => checked_trees::CheckedProofRecursiveTransitionLane::Target,
                                        validation::ValidatedProofRecursiveTransitionLane::Continuation => checked_trees::CheckedProofRecursiveTransitionLane::Continuation,
                                    },
                                },
                            },
                            caller_rank_parameter: edge.caller_rank_parameter,
                            callee_rank_parameter: edge.callee_rank_parameter,
                            strict_member_path: edge.strict_member_path.clone(),
                        })
                        .collect(),
                },
            )
            .collect(),
    })
}

fn build_blocking_facts(
    program: &TypedTrees,
    blocking: &[MachineBlockingRow],
) -> checked_trees::BlockingFacts {
    let machines = program
        .machines()
        .iter()
        .map(|machine| {
            let blocking_row = blocking.iter().find(|row| row.symbol == machine.symbol);
            let publishes_operational_contract = machine.is_public
                || machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody;
            checked_trees::MachineBlockingFact {
                machine: machine.symbol,
                plan: language_semantics::BlockingPlan {
                    interface: if publishes_operational_contract || machine.blocks {
                        language_semantics::BlockingInterface::PublishedMayBlock(machine.blocks)
                    } else {
                        language_semantics::BlockingInterface::InternalInferred
                    },
                    checked_may_block: blocking_row.is_some_and(|row| row.transitive_may_block),
                },
            }
        })
        .collect();
    checked_trees::BlockingFacts { machines }
}

fn build_suspension_facts(
    program: &TypedTrees,
    suspensions: &[MachineSuspensionRow],
) -> checked_trees::SuspensionFacts {
    let machines = program
        .machines()
        .iter()
        .map(|machine| {
            let suspension_row = suspensions.iter().find(|row| row.symbol == machine.symbol);
            let publishes_operational_contract = machine.is_public
                || machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody;
            checked_trees::MachineSuspensionFact {
                machine: machine.symbol,
                plan: language_semantics::SuspensionPlan {
                    interface: if publishes_operational_contract || machine.suspends {
                        language_semantics::SuspensionInterface::PublishedMaySuspend(
                            machine.suspends,
                        )
                    } else {
                        language_semantics::SuspensionInterface::InternalInferred
                    },
                    checked_may_suspend: suspension_row
                        .is_some_and(|row| row.transitive_may_suspend),
                },
            }
        })
        .collect();
    checked_trees::SuspensionFacts { machines }
}

fn build_synchronous_invocation_facts(
    program: &TypedTrees,
) -> checked_trees::SynchronousInvocationFacts {
    let inference = validation::infer_synchronous_invocations(program);
    let machines = program
        .machines()
        .iter()
        .map(|machine| {
            let invocation_summary = inference.for_machine(machine.symbol);
            let canonical_invocation = |target: flow_effects::InvocationTarget| match target {
                flow_effects::InvocationTarget::Parameter(index) => format!("parameter:{index}"),
                flow_effects::InvocationTarget::Service(symbol) => program
                    .traits()
                    .iter()
                    .find(|definition| definition.symbol == symbol)
                    .map(|definition| format!("service:{}", definition.name))
                    .unwrap_or_else(|| format!("service:#{}", symbol.arena_index())),
            };
            let published_targets = invocation_summary
                .map(|summary| summary.published.clone())
                .unwrap_or_default();
            let checked_inferred_targets = invocation_summary
                .map(|summary| summary.inferred_transitive.clone())
                .unwrap_or_default();
            let mut published = published_targets
                .iter()
                .copied()
                .map(canonical_invocation)
                .collect::<Vec<_>>();
            published.sort_unstable();
            published.dedup();
            let mut checked_inferred = checked_inferred_targets
                .iter()
                .copied()
                .map(canonical_invocation)
                .collect::<Vec<_>>();
            checked_inferred.sort_unstable();
            checked_inferred.dedup();
            let publishes_invocations = machine.supply_mode
                != language_semantics::MachineSupplyMode::CheckedBody
                || machine.is_public
                || !program.machine_invokes(machine).is_empty();

            checked_trees::MachineSynchronousInvocationFact {
                machine: machine.symbol,
                published_targets,
                checked_inferred_targets,
                plan: language_semantics::SynchronousInvocationPlan {
                    interface: if publishes_invocations {
                        language_semantics::SynchronousInvocationInterface::PublishedCeiling
                    } else {
                        language_semantics::SynchronousInvocationInterface::InternalInferred
                    },
                    published,
                    checked_inferred,
                },
            }
        })
        .collect();
    checked_trees::SynchronousInvocationFacts { machines }
}

fn build_dynamic_conformance_facts(
    program: &TypedTrees,
) -> Result<checked_trees::DynamicConformanceFacts, Vec<diagnostics::Diagnostic>> {
    let mut selections = Vec::new();
    let mut diagnostics = Vec::new();
    let validated_selections = validation::collect_dynamic_conformance_selections(program)?;
    let validated_storages =
        validation::collect_dynamic_descriptor_storages(program, &validated_selections);
    for selection in validated_selections {
        let selected = selected_data_conformance(program, &selection);
        let mut rows = Vec::new();
        for row in selected
            .and_then(|conformance| program.closed_conformance_rows(conformance))
            .unwrap_or_default()
        {
            if !row.realization_machine.is_valid() || !row.realization_state.is_valid() {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "dynamic conformance row `{}::{}` reached checked lowering without an exact checked realization",
                    row.declaring_trait_name, row.requirement_name
                )));
                continue;
            }
            let (requirement_identity, realization_identity) =
                match normalized_dynamic_row_identities(program, row) {
                    Ok(identities) => identities,
                    Err(diagnostic) => {
                        diagnostics.push(diagnostic);
                        continue;
                    }
                };
            rows.push(checked_trees::DynamicConformanceRowFact {
                declaring_trait: row.declaring_trait,
                requirement: row.requirement,
                requirement_identity,
                realization_machine: row.realization_machine,
                realization_state: row.realization_state,
                realization_identity,
                source: match row.source {
                    typed_trees::trait_definition::ConformanceRowSource::Inline => {
                        checked_trees::DynamicConformanceRowSource::Inline
                    }
                    typed_trees::trait_definition::ConformanceRowSource::Reference => {
                        checked_trees::DynamicConformanceRowSource::Reference
                    }
                    typed_trees::trait_definition::ConformanceRowSource::TraitDefault => {
                        checked_trees::DynamicConformanceRowSource::TraitDefault
                    }
                },
            });
        }
        selections.push(checked_trees::DynamicConformanceSelectionFact {
            occurrence: selection.occurrence,
            binding: selection.binding,
            binding_name: selection.binding_name.clone(),
            machine: selection.machine,
            state: selection.state,
            statement_index: selection.statement_index,
            source_symbol: selection.source_symbol,
            source_name: selection.source_name.clone(),
            source_path: selection.source_path.clone(),
            source_data: selection.source_data,
            target_trait: selection.target_trait,
            conformance: selection.conformance,
            rows,
        });
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let binding_facts = checked_trees::DynamicConformanceFacts {
        selections: selections.clone(),
        storages: Vec::new(),
    }
    .binding_facts();
    let mut storages = Vec::with_capacity(validated_storages.len());
    for storage in validated_storages {
        let Some(selection) = binding_facts.selections.iter().find(|candidate| {
            candidate.machine == storage.selection.machine
                && candidate.state == storage.selection.state
                && candidate.statement_index == storage.selection.statement_index
                && candidate.binding == storage.selection.binding
                && candidate.target_trait == storage.selection.target_trait
                && candidate.conformance == storage.selection.conformance
        }) else {
            diagnostics.push(diagnostics::Diagnostic::error(
                "dynamic descriptor storage lost its exact checked conformance selection",
            ));
            continue;
        };
        storages.push(checked_trees::DynamicDescriptorStorageFact {
            occurrence: storage.occurrence,
            machine: storage.machine,
            state: storage.state,
            statement_index: storage.statement_index,
            destination_binding: storage.destination_binding,
            destination_name: storage.destination_name,
            destination_field: storage.destination_field,
            destination_path: storage.destination_path,
            source_binding: storage.source_binding,
            source_name: storage.source_name,
            source_path: storage.source_path,
            selection: selection.clone(),
        });
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(checked_trees::DynamicConformanceFacts {
        selections,
        storages,
    })
}

pub(crate) fn normalized_dynamic_row_identities(
    program: &TypedTrees,
    row: &typed_trees::trait_definition::ConformanceRow,
) -> Result<(String, String), diagnostics::Diagnostic> {
    let mut declaring_traits = program
        .traits()
        .iter()
        .filter(|definition| definition.symbol == row.declaring_trait);
    let declaring_trait = declaring_traits.next().ok_or_else(|| {
        diagnostics::Diagnostic::error(
            "dynamic conformance row has no exact declaring trait for normalized identity",
        )
    })?;
    if declaring_traits.next().is_some() {
        return Err(diagnostics::Diagnostic::error(
            "dynamic conformance row has an ambiguous declaring trait for normalized identity",
        ));
    }

    let mut requirements = program
        .trait_machine_signatures(declaring_trait)
        .iter()
        .filter(|requirement| requirement.symbol == row.requirement);
    let requirement = requirements.next().ok_or_else(|| {
        diagnostics::Diagnostic::error(
            "dynamic conformance row has no exact requirement for normalized identity",
        )
    })?;
    if requirements.next().is_some() {
        return Err(diagnostics::Diagnostic::error(
            "dynamic conformance row has an ambiguous requirement for normalized identity",
        ));
    }

    let mut realization_machines = program
        .machines()
        .iter()
        .filter(|machine| machine.symbol == row.realization_machine);
    let realization_machine = realization_machines.next().ok_or_else(|| {
        diagnostics::Diagnostic::error(
            "dynamic conformance row has no exact realization machine for normalized identity",
        )
    })?;
    if realization_machines.next().is_some() {
        return Err(diagnostics::Diagnostic::error(
            "dynamic conformance row has an ambiguous realization machine for normalized identity",
        ));
    }
    let realization_identity = program
        .normalized_machine_overload_identity(realization_machine)
        .ok_or_else(|| {
            diagnostics::Diagnostic::error(
                "dynamic conformance row realization has no normalized callable identity",
            )
        })?
        .identity();
    let requirement_identity = program
        .normalized_trait_requirement_overload_identity(declaring_trait, requirement)
        .identity();
    Ok((requirement_identity, realization_identity))
}

fn selected_data_conformance<'program>(
    program: &'program TypedTrees,
    selection: &validation::DynamicConformanceSelection,
) -> Option<&'program typed_trees::trait_definition::Conformance> {
    if let Some(symbol) = selection.conformance {
        return program
            .conformances()
            .iter()
            .find(|conformance| conformance.symbol == symbol);
    }
    let source_name = program.symbols.name(selection.source_data);
    let trait_name = program.symbols.name(selection.target_trait);
    let mut matches = program.conformances().iter().filter(|conformance| {
        conformance
            .carrier_name()
            .is_some_and(|carrier| carrier.as_str() == source_name)
            && conformance.trait_name.as_str() == trait_name
            && conformance.alias.is_none()
    });
    let selected = matches.next()?;
    matches.next().is_none().then_some(selected)
}

/// STR4 checked plans (machine_taxonomy.md): assemble each machine's
/// normalized contract plan from the published halves already carried on
/// the records (supply mode, service/operational ceilings, published termination),
/// with a deterministic fingerprint over them. Only DECLARED material
/// enters -- acceptance 8 (a stronger prover cannot change an exported
/// contract ID) holds by construction.
fn build_contract_plans(
    program: &TypedTrees,
    service_reaches: &checked_trees::ServiceReachFacts,
    synchronous_invocations: &checked_trees::SynchronousInvocationFacts,
    suspensions: &checked_trees::SuspensionFacts,
    blocking: &checked_trees::BlockingFacts,
    termination: &checked_trees::TerminationFacts,
    mutation: &checked_trees::MutationFacts,
    capabilities: &flow_effects::CapabilityFlowPlan,
    flow: &checked_trees::FlowFacts,
    operators: &checked_trees::CheckedOperatorFacts,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Result<checked_trees::MachineContractPlans, Vec<diagnostics::Diagnostic>> {
    let mut machines = Vec::new();
    let content_conservation = validation::build_content_conservation_plans(program);
    for machine in program.machines() {
        let service_fact = service_reaches.for_machine(machine.symbol);
        let published_service_row = service_fact
            .map(|fact| fact.published_ceiling)
            .unwrap_or(language_semantics::ServiceReachRowTable::EMPTY_ROW);
        let published_service_names = service_reaches
            .rows
            .services(published_service_row)
            .iter()
            .filter_map(|service| service_reaches.services.definition(*service))
            .map(|definition| definition.name.clone())
            .collect::<Vec<_>>();
        let synchronous_invocation = synchronous_invocations
            .for_machine(machine.symbol)
            .expect("every checked machine must publish synchronous invocation facts");
        let suspension = suspensions
            .for_machine(machine.symbol)
            .expect("every checked machine must publish suspension facts");
        let blocking = blocking
            .for_machine(machine.symbol)
            .expect("every checked machine must publish blocking facts");
        let termination = termination
            .for_machine(machine.symbol)
            .expect("every checked machine must publish termination facts");
        // Slice 2: the declared requires/ensures facts in a CANONICAL,
        // clause-order-independent encoding (each fact serializes to a
        // stable byte form; the set sorts before folding). Parameter
        // RENAMES change the identity in v1 -- positional normalization is
        // the recorded follow-up.
        let mut canonical_facts: Vec<Vec<u8>> = Vec::new();
        // The callable shape is contract identity too. A selected static
        // machine changing parameter mode/type, result type, or state surface
        // must invalidate every specialization that recorded its contract ID.
        // Encode generic binders positionally so a rename remains invisible.
        let generic_binders: Vec<(String, String)> = program
            .machine_type_parameters(machine)
            .iter()
            .enumerate()
            .map(|(index, parameter)| (parameter.name.as_str().to_owned(), format!("$G{index}")))
            .collect();
        for state in program.machine_states(machine) {
            let mut encoded = vec![0xa0];
            let state_parameters = program.state_parameters(state);
            for parameter in state_parameters {
                encoded.push(u8::from(parameter.is_self));
                encoded.push(u8::from(parameter.is_mutable));
                encoded.push(u8::from(parameter.is_const));
                encode_type_spelling(
                    &program.display_type_reference(parameter.type_reference),
                    &generic_binders,
                    &mut encoded,
                );
            }
            encoded.push(0xaf);
            encode_type_spelling(
                &program.display_type_reference(state.return_type),
                &generic_binders,
                &mut encoded,
            );
            let parameter_names = state_parameters
                .iter()
                .map(|parameter| parameter.name.as_str().to_owned())
                .collect::<Vec<_>>();
            let state_contracts = encode_contract_set_canonical(
                program,
                program.state_contracts(state),
                &parameter_names,
                &content_conservation,
                &[0xae],
                true,
                true,
            );
            for contract in state_contracts {
                encoded.extend(contract);
                encoded.push(0xad);
            }
            canonical_facts.push(encoded);
        }
        // Positional parameter normalization: a contract fact naming the
        // machine's Nth parameter encodes as P<N>, so RENAMES never change
        // the identity (the substitutable contract is positional).
        let parameter_names: Vec<String> = program
            .machine_states(machine)
            .first()
            .map(|entry| {
                program
                    .state_parameters(entry)
                    .iter()
                    .map(|parameter| parameter.name.as_str().to_owned())
                    .collect()
            })
            .unwrap_or_default();
        let crash = build_published_crash_plan(
            program,
            machine,
            &parameter_names,
            &content_conservation,
            operators,
            exact_integer_casts,
        );
        canonical_facts.extend(encode_contract_set_canonical(
            program,
            program.machine_contracts(machine),
            &parameter_names,
            &content_conservation,
            &[],
            false,
            false,
        ));
        canonical_facts.sort();
        let closed_scalar_values =
            build_closed_scalar_value_contract_plan(program, machine, operators);
        let identity = checked_trees::contract_identity(
            machine.supply_mode,
            &published_service_names,
            synchronous_invocation.interface,
            &synchronous_invocation.published,
            suspension.interface,
            blocking.interface,
            &crash,
            &termination.interface,
            &canonical_facts,
        );
        machines.push(checked_trees::MachineContractPlan {
            machine: machine.symbol,
            closed_scalar_values,
            crash,
            report_fingerprint: identity.report_fingerprint,
            commitment: identity.commitment,
        });
    }
    let crash_capsules = build_crash_contract_capsules(program, &content_conservation);
    crash_calls::attach_checked_crash_calls(
        program,
        operators,
        exact_integer_casts,
        flow,
        &content_conservation,
        &crash_capsules,
        &mut machines,
    );
    let realized_envelopes =
        machines
            .iter()
            .map(|contract| {
                let machine = program
                    .machines()
                    .iter()
                    .find(|machine| machine.symbol == contract.machine)
                    .expect("every contract must retain its exact typed machine");
                let service_fact = service_reaches
                    .for_machine(contract.machine)
                    .expect("every checked machine must retain service-reach facts");
                let effective_service_reach = service_reaches
                    .rows
                    .services(service_fact.effective)
                    .iter()
                    .filter_map(|service| service_reaches.services.definition(*service))
                    .map(|definition| definition.name.clone())
                    .collect::<Vec<_>>();
                let concrete_service_reach = service_reaches
                    .rows
                    .services(service_fact.concrete_effective)
                    .iter()
                    .filter_map(|service| service_reaches.services.definition(*service))
                    .map(|definition| definition.name.clone())
                    .collect::<Vec<_>>();
                let invocation = synchronous_invocations
                    .for_machine(contract.machine)
                    .expect("every checked machine must retain invocation facts");
                let suspension = suspensions
                    .for_machine(contract.machine)
                    .expect("every checked machine must retain suspension facts");
                let blocking = blocking
                    .for_machine(contract.machine)
                    .expect("every checked machine must retain blocking facts");
                let termination = termination
                    .for_machine(contract.machine)
                    .expect("every checked machine must retain termination facts");
                let mutation = mutation
                    .for_machine(contract.machine)
                    .map(|fact| fact.state_write_frames.clone())
                    .unwrap_or_default();
                let capability_rows = capabilities
                    .flows()
                    .filter(|flow| flow.machine_symbol == contract.machine)
                    .copied()
                    .collect();
                checked_trees::RealizedMachineContractEnvelope {
                machine: contract.machine,
                contract_report_fingerprint: contract.report_fingerprint,
                contract_commitment: contract.commitment,
                effective_service_reach,
                concrete_service_reach,
                unresolved_installation_reaches: service_fact
                    .unresolved_installation_reaches
                    .clone(),
                effective_synchronous_invocations: invocation.checked_inferred.clone(),
                checked_may_suspend: suspension.checked_may_suspend,
                checked_may_block: blocking.checked_may_block,
                checked_termination: termination.checked_summary.clone(),
                checked_crash: contract.crash.clone(),
                mutation,
                capabilities: capability_rows,
                resources:
                    checked_trees::CheckedMachineResourceEnvelopes::from_checked_contract_entries(
                        contract.machine,
                        contract.report_fingerprint,
                        contract.commitment,
                        program.machine_states(machine).iter().map(|entry| entry.symbol),
                    ),
            }
            })
            .collect();
    let plans = checked_trees::MachineContractPlans {
        machines,
        crash_capsules,
        realized_envelopes,
    };
    plans.validate_resource_envelopes().map_err(|error| {
        vec![diagnostics::Diagnostic::error(format!(
            "checked resource-envelope replay failed: {error}"
        ))]
    })?;
    validate_checked_resource_envelope_coverage(program, &plans)?;
    Ok(plans)
}

/// Reconstruct the complete per-entry resource roster from typed ownership
/// and declaration order. Structural checked validation above does not reopen
/// typed trees, so this second gate is what rejects a missing, foreign, or
/// reordered entry before the source-independent carrier leaves this stage.
fn validate_checked_resource_envelope_coverage(
    program: &TypedTrees,
    plans: &checked_trees::MachineContractPlans,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    for machine in program.machines() {
        let contract = plans.for_machine(machine.symbol).ok_or_else(|| {
            vec![diagnostics::Diagnostic::error(
                "checked resource-envelope replay is missing an exact machine contract",
            )]
        })?;
        let realized = plans.realized_envelope(machine.symbol).ok_or_else(|| {
            vec![diagnostics::Diagnostic::error(
                "checked resource-envelope replay is missing an exact realized machine row",
            )]
        })?;
        let expected_entries = program.machine_states(machine);
        if realized.resources.len() != expected_entries.len() {
            return Err(vec![diagnostics::Diagnostic::error(
                "checked resource-envelope replay does not cover every owned machine entry",
            )]);
        }
        for (resource, entry) in realized.resources.iter().zip(expected_entries) {
            let replayed = checked_trees::CheckedEntryResourceEnvelope::from_checked_contract(
                machine.symbol,
                entry.symbol,
                contract.report_fingerprint,
                contract.commitment,
            );
            if resource != &replayed {
                return Err(vec![diagnostics::Diagnostic::error(
                    "checked resource-envelope replay changed entry ownership or declaration order",
                )]);
            }
        }
    }
    Ok(())
}

fn build_mutation_facts(program: &TypedTrees) -> checked_trees::MutationFacts {
    let frame_resolver = validation::CallFrameResolver::new(program);
    let machines = program
        .machines()
        .iter()
        .map(|machine| {
            let states = program.machine_states(machine);
            let frames = frame_resolver.as_ref().map_or_else(
                || {
                    (0..states.len())
                        .map(|_| facts::NormalizedWriteFrame::opaque())
                        .collect()
                },
                |resolver| resolver.inferred_machine_state_write_frames(machine),
            );
            checked_trees::MachineMutationFact {
                machine: machine.symbol,
                state_write_frames: states
                    .iter()
                    .zip(frames)
                    .map(|(state, frame)| checked_trees::StateWriteFramePlan {
                        state: state.symbol,
                        frame,
                    })
                    .collect(),
            }
        })
        .collect();
    checked_trees::MutationFacts { machines }
}

fn build_closed_scalar_value_contract_plan(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    operators: &checked_trees::CheckedOperatorFacts,
) -> checked_trees::ClosedScalarValueContractPlan {
    use typed_trees::{
        domain::ProofFact,
        expression::{BinaryOperator, ExpressionNode},
        signature::SignatureContractKind,
    };

    let boolean_type =
        program
            .type_reference_table
            .named_references()
            .find_map(|(type_reference, symbol, _)| {
                (program.symbols.builtin_type_atom(symbol) == Some(symbols::BuiltinTypeAtom::Bool))
                    .then_some(type_reference)
            });

    let lower_clause = |contract: &typed_trees::signature::SignatureContract| {
        let [ProofFact::Expression(expression)] = program.proof_facts.span_or_empty(contract.facts)
        else {
            return None;
        };
        if let Some(predicate) = crate::values::lower_integer_contract_predicate(
            program,
            operators,
            machine,
            *expression,
            contract.kind == SignatureContractKind::Ensures,
        ) {
            return Some(checked_trees::ClosedScalarContractValue::Predicate(
                predicate,
            ));
        }
        let ExpressionNode::Binary(binary) = program.expression_table.expression(*expression)
        else {
            return None;
        };
        if binary.operator != BinaryOperator::Equal {
            return None;
        }
        // Boolean literals have one exact builtin carrier. Integer literals
        // retain wildcard typing rather than guessing a contextual landing.
        let operand_types = [binary.left, binary.right].map(|operand| {
            matches!(
                program.expression_table.expression(operand),
                ExpressionNode::Boolean(_)
            )
            .then_some(boolean_type)
            .flatten()
        });
        if operators.uses.iter().any(|(_, operator)| {
            operator.expression == *expression
                && operator.status
                    != checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
        }) || !typed_trees::operator::has_builtin_spelled_expression_meaning(
            program,
            machine.symbol,
            *expression,
            language_core::OperatorSpelling::Equal,
            &operand_types,
        ) {
            return None;
        }
        match (
            program.expression_table.expression(binary.left),
            program.expression_table.expression(binary.right),
        ) {
            (ExpressionNode::Boolean(left), ExpressionNode::Boolean(right)) if left == right => {
                Some(checked_trees::ClosedScalarContractValue::Boolean(*left))
            }
            (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) if left == right => {
                Some(checked_trees::ClosedScalarContractValue::Integer(
                    left.clone(),
                ))
            }
            _ => None,
        }
    };

    let mut requires = Vec::new();
    let mut ensures = Vec::new();
    let mut has_crash_clauses = false;
    let mut has_outcome_specific_clauses = false;
    for contract in program.machine_contracts(machine) {
        if matches!(
            &contract.kind,
            SignatureContractKind::EnsuresForResultCase { .. }
        ) {
            has_outcome_specific_clauses = true;
            continue;
        }
        // Named witness-bearing lanes are checked and lowered through the
        // evidence contract plan. They do not participate in the independent
        // closed scalar value contract used by terminal scalar production.
        if contract.binding.is_some() {
            continue;
        }
        match contract.kind {
            SignatureContractKind::Requires => requires.push(lower_clause(contract)),
            SignatureContractKind::Ensures => ensures.push(lower_clause(contract)),
            SignatureContractKind::EnsuresForResultCase { .. } => unreachable!(
                "outcome-specific clauses were separated from unconditional scalar contracts"
            ),
            SignatureContractKind::Crashes { .. } => has_crash_clauses = true,
        }
    }
    requires.extend(
        crate::values::lower_integer_parameter_range_requirements(program, operators, machine)
            .into_iter()
            .map(|predicate| predicate.map(checked_trees::ClosedScalarContractValue::Predicate)),
    );
    checked_trees::ClosedScalarValueContractPlan::new(
        requires,
        ensures,
        has_crash_clauses,
        has_outcome_specific_clauses,
    )
}

fn build_crash_contract_capsules(
    program: &TypedTrees,
    content_conservation: &[validation::ContentConservationSourcePlan],
) -> Vec<checked_trees::CrashContractCapsule> {
    let mut signatures = Vec::new();
    for machine in program.machines() {
        for (owner_symbol, target_state, signature) in
            crate::proof::machine_parameter_evidence_signatures(
                program,
                program.machine_type_parameters(machine),
            )
        {
            // The binder symbol is the callable target inside the generic
            // body. A nominal requirement symbol remains authority metadata;
            // it is never a second alias for the parameter call target.
            signatures.push((owner_symbol, target_state, signature));
        }
    }
    for definition in program.data_definitions() {
        signatures.extend(crate::proof::machine_parameter_evidence_signatures(
            program,
            program.data_type_parameters(definition),
        ));
    }
    for definition in program.domain_definitions() {
        signatures.extend(crate::proof::machine_parameter_evidence_signatures(
            program,
            program.domain_type_parameters(definition),
        ));
    }
    for definition in program.traits() {
        signatures.extend(crate::proof::machine_parameter_evidence_signatures(
            program,
            program.trait_type_parameters(definition),
        ));
        for signature in program.trait_machine_signatures(definition) {
            signatures.extend(crate::proof::machine_parameter_evidence_signatures(
                program,
                program.state_signature_type_parameters(signature),
            ));
            signatures.push((definition.symbol, signature.symbol, signature));
        }
    }

    let mut capsules = signatures
        .into_iter()
        .map(|(target_machine, target_state, signature)| {
            let parameters = program.state_signature_parameters(signature);
            let parameter_names = parameters
                .iter()
                .map(|parameter| parameter.name.as_str().to_owned())
                .collect::<Vec<_>>();
            let contracts = program.state_signature_contracts(signature);
            let published = build_published_crash_buckets(
                program,
                contracts,
                &parameter_names,
                content_conservation,
                None,
                None,
                &[],
            );
            let crash = checked_trees::CrashPlan::published_ceiling(published.clone());

            let published_service_names = program
                .service_reach_rows
                .services(signature.service_reach_row)
                .iter()
                .filter_map(|service| program.service_reaches.definition(*service))
                .map(|definition| definition.name.clone())
                .collect::<Vec<_>>();
            let published_invocations =
                validation::declared_signature_invocations(program, signature)
                    .into_iter()
                    .map(|invocation| match invocation {
                        flow_effects::InvocationTarget::Parameter(index) => {
                            format!("parameter:{index}")
                        }
                        flow_effects::InvocationTarget::Service(symbol) => program
                            .traits()
                            .iter()
                            .find(|definition| definition.symbol == symbol)
                            .map(|definition| format!("service:{}", definition.name))
                            .unwrap_or_else(|| format!("service:#{}", symbol.arena_index())),
                    })
                    .collect::<Vec<_>>();

            let generic_binders = program
                .state_signature_type_parameters(signature)
                .iter()
                .enumerate()
                .map(|(index, parameter)| {
                    (parameter.name.as_str().to_owned(), format!("$G{index}"))
                })
                .collect::<Vec<_>>();
            let mut callable_shape = vec![0xa0];
            for parameter in parameters {
                callable_shape.push(u8::from(parameter.is_self));
                callable_shape.push(u8::from(parameter.is_mutable));
                callable_shape.push(u8::from(parameter.is_const));
                encode_type_spelling(
                    &program.display_type_reference(parameter.type_reference),
                    &generic_binders,
                    &mut callable_shape,
                );
            }
            callable_shape.push(0xaf);
            encode_type_spelling(
                &program.display_type_reference(signature.return_type),
                &generic_binders,
                &mut callable_shape,
            );
            for contract in encode_contract_set_canonical(
                program,
                contracts,
                &parameter_names,
                content_conservation,
                &[0xae],
                true,
                false,
            ) {
                callable_shape.extend(contract);
                callable_shape.push(0xad);
            }
            let canonical_facts = vec![callable_shape];
            let termination = language_semantics::TerminationInterface::Published(
                signature.termination_guarantee.clone(),
            );
            let identity = checked_trees::contract_identity(
                language_semantics::MachineSupplyMode::Requirement,
                &published_service_names,
                language_semantics::SynchronousInvocationInterface::PublishedCeiling,
                &published_invocations,
                language_semantics::SuspensionInterface::PublishedMaySuspend(signature.suspends),
                language_semantics::BlockingInterface::PublishedMayBlock(signature.blocks),
                &crash,
                &termination,
                &canonical_facts,
            );
            checked_trees::CrashContractCapsule::new_with_commitment(
                target_machine,
                target_state,
                identity.report_fingerprint,
                identity.commitment,
                published,
            )
            .with_operational_envelope(
                published_service_names,
                published_invocations,
                signature.suspends,
                signature.blocks,
                signature.termination_guarantee.clone(),
            )
        })
        .collect::<Vec<_>>();
    capsules.sort_by_key(|capsule| {
        (
            capsule.target_machine().arena_index(),
            capsule.target_machine().generation(),
            capsule.target_state().arena_index(),
            capsule.target_state().generation(),
        )
    });
    capsules.dedup();
    capsules
}

fn encode_signature_contract_kind(
    kind: &typed_trees::signature::SignatureContractKind,
    output: &mut Vec<u8>,
) {
    match kind {
        typed_trees::signature::SignatureContractKind::Requires => output.push(1),
        typed_trees::signature::SignatureContractKind::Ensures => output.push(2),
        typed_trees::signature::SignatureContractKind::EnsuresForResultCase {
            result_data,
            result_case,
        } => {
            output.push(3);
            output.extend_from_slice(&result_data.arena_index().to_le_bytes());
            output.extend_from_slice(&result_case.arena_index().to_le_bytes());
        }
        typed_trees::signature::SignatureContractKind::Crashes { cause } => {
            output.push(4);
            output.push(match cause {
                typed_trees::signature::CrashCause::Trap => 1,
                typed_trees::signature::CrashCause::Abort => 2,
            });
        }
    }
}

fn build_published_crash_plan(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
    operators: &checked_trees::CheckedOperatorFacts,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> checked_trees::CrashPlan {
    let structural_runtime_requirements =
        build_structural_runtime_requirements(program, machine, operators, exact_integer_casts);
    let published = build_published_crash_buckets(
        program,
        program.machine_contracts(machine),
        parameter_names,
        content_conservation,
        Some(machine),
        Some(operators),
        exact_integer_casts,
    );
    let plan = (if machine.is_public
        || machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || !published.is_empty()
    {
        checked_trees::CrashPlan::published_ceiling(published)
    } else {
        checked_trees::CrashPlan::default()
    })
    .with_structural_runtime_requirements(structural_runtime_requirements);
    let checked_sites = build_checked_crash_sites(program, machine, &plan);
    plan.with_checked_sites(checked_sites)
        .expect("one checked crash cause occupies each transition site")
}

fn build_structural_runtime_requirements(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    operators: &checked_trees::CheckedOperatorFacts,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<Vec<checked_trees::CheckedBooleanExpression>> {
    let entry = program.machine_states(machine).first()?;
    program
        .machine_contracts(machine)
        .iter()
        .chain(program.state_contracts(entry))
        .filter(|contract| contract.kind == typed_trees::signature::SignatureContractKind::Requires)
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .map(|fact| {
            let typed_trees::domain::ProofFact::Expression(expression) = fact else {
                return None;
            };
            crate::values::lower_machine_parameter_boolean_expression(
                program,
                operators,
                machine,
                *expression,
                exact_integer_casts,
            )
        })
        .collect()
}

pub(crate) fn derive_authored_machine_crash_buckets(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> Vec<checked_trees::CrashRouteBucket> {
    let parameter_names = program
        .machine_states(machine)
        .first()
        .map(|entry| {
            program
                .state_parameters(entry)
                .iter()
                .map(|parameter| parameter.name.as_str().to_owned())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let conservation = validation::build_content_conservation_plans(program);
    build_published_crash_buckets(
        program,
        program.machine_contracts(machine),
        &parameter_names,
        &conservation,
        Some(machine),
        None,
        &[],
    )
}

pub(crate) fn derive_authored_signature_crash_buckets(
    program: &TypedTrees,
    signature: &typed_trees::signature::StateSignature,
) -> Vec<checked_trees::CrashRouteBucket> {
    let parameter_names = program
        .state_signature_parameters(signature)
        .iter()
        .map(|parameter| parameter.name.as_str().to_owned())
        .collect::<Vec<_>>();
    let conservation = validation::build_content_conservation_plans(program);
    build_published_crash_buckets(
        program,
        program.state_signature_contracts(signature),
        &parameter_names,
        &conservation,
        None,
        None,
        &[],
    )
}

fn build_published_crash_buckets(
    program: &TypedTrees,
    contracts: &[typed_trees::signature::SignatureContract],
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
    machine: Option<&typed_trees::machine::Machine>,
    operators: Option<&checked_trees::CheckedOperatorFacts>,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Vec<checked_trees::CrashRouteBucket> {
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct Bucket {
        unconditional: bool,
        routes: Vec<checked_trees::CrashPredicateIdentity>,
    }

    let mut buckets = BTreeMap::<checked_trees::CrashCause, Bucket>::new();
    for contract in contracts {
        let typed_trees::signature::SignatureContractKind::Crashes { cause } = &contract.kind
        else {
            continue;
        };
        let cause = match cause {
            typed_trees::signature::CrashCause::Trap => checked_trees::CrashCause::Trap,
            typed_trees::signature::CrashCause::Abort => checked_trees::CrashCause::Abort,
        };
        let bucket = buckets.entry(cause).or_default();
        let facts = program.proof_facts.span_or_empty(contract.facts);
        if facts.is_empty() || facts.iter().any(|fact| is_true_crash_route(program, fact)) {
            bucket.unconditional = true;
            continue;
        }
        for fact in facts {
            let mut route = Vec::new();
            encode_contract_fact_canonical(
                program,
                fact,
                parameter_names,
                content_conservation,
                false,
                &mut route,
            );
            let identity = match fact {
                typed_trees::domain::ProofFact::Expression(expression) => {
                    let structured = crash_calls::crash_predicate_from_expression(
                        program,
                        *expression,
                        parameter_names,
                        Some(content_conservation),
                    );
                    let scalar = machine.zip(operators).and_then(|(machine, operators)| {
                        crate::values::lower_machine_parameter_boolean_expression(
                            program,
                            operators,
                            machine,
                            *expression,
                            exact_integer_casts,
                        )
                    });
                    let identity = if let Some(scalar) = scalar {
                        checked_trees::CrashPredicateIdentity::from_expression_and_scalar(
                            structured, scalar,
                        )
                    } else {
                        checked_trees::CrashPredicateIdentity::from_expression(structured)
                    };
                    debug_assert_eq!(identity.canonical_bytes(), route);
                    identity
                }
                _ => checked_trees::CrashPredicateIdentity::from_canonical_bytes(route),
            };
            bucket.routes.push(identity);
        }
    }

    buckets
        .into_iter()
        .map(|(cause, mut bucket)| {
            let alternative_guards = if bucket.unconditional {
                vec![checked_trees::CrashRouteGuard::Truth]
            } else {
                bucket.routes.sort();
                bucket.routes.dedup();
                bucket
                    .routes
                    .into_iter()
                    .map(checked_trees::CrashRouteGuard::Predicate)
                    .collect()
            };
            checked_trees::CrashRouteBucket::new(cause, alternative_guards)
                .expect("an authored crash bucket has a canonical nonempty route set")
        })
        .collect()
}

fn build_checked_crash_sites(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    crash_plan: &checked_trees::CrashPlan,
) -> Vec<checked_trees::CheckedCrashSite> {
    let mut sites = Vec::new();
    for state in program.machine_states(machine) {
        for (statement_ordinal, statement) in program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .enumerate()
        {
            let typed_trees::statement::StatementNode::Transition(transition) = statement else {
                continue;
            };
            let typed_trees::statement::TransitionExit::Crash(cause) = transition.exit else {
                continue;
            };
            let cause = match cause {
                typed_trees::signature::CrashCause::Trap => checked_trees::CrashCause::Trap,
                typed_trees::signature::CrashCause::Abort => checked_trees::CrashCause::Abort,
            };
            // An unconditional same-cause route covers every possible path
            // guard. Guarded buckets join only after path-conditioned
            // entailment exists.
            let guard_covering_buckets = crash_plan
                .published_with_ids()
                .filter_map(|(id, bucket)| {
                    (bucket.cause() == cause && bucket.is_unconditional()).then_some(id)
                })
                .collect();
            sites.push(checked_trees::CheckedCrashSite::new(
                checked_trees::CrashSiteLocation::new(
                    state.symbol,
                    u32::try_from(statement_ordinal)
                        .expect("state-local statement ordinal exceeds checked identity range"),
                ),
                cause,
                guard_covering_buckets,
                Vec::new(),
            ));
        }
    }
    sites
}

fn is_true_crash_route(program: &TypedTrees, fact: &typed_trees::domain::ProofFact) -> bool {
    matches!(
        fact,
        typed_trees::domain::ProofFact::Expression(expression)
            if matches!(
                program.expression_table.expression(*expression),
                typed_trees::expression::ExpressionNode::Boolean(true)
            )
    )
}

/// Encode contracts as semantic sets. Crash clauses are first merged by cause:
/// their facts are alternative routes, duplicate routes are irrelevant, and
/// one unconditional clause subsumes every guarded route in the same bucket.
/// This keeps public contract identity independent of clause grouping while
/// preserving the bucket itself as identity-bearing material.
pub(crate) fn encode_contract_set_canonical(
    program: &TypedTrees,
    contracts: &[typed_trees::signature::SignatureContract],
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
    entry_prefix: &[u8],
    canonicalize_membership_value: bool,
    include_crashes: bool,
) -> Vec<Vec<u8>> {
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct CrashBucket {
        unconditional: bool,
        routes: Vec<Vec<u8>>,
    }

    let mut encoded = Vec::new();
    let mut crash_buckets = BTreeMap::<Vec<u8>, CrashBucket>::new();
    for contract in contracts {
        let mut contract_prefix = entry_prefix.to_vec();
        encode_signature_contract_kind(&contract.kind, &mut contract_prefix);
        let facts = program.proof_facts.span_or_empty(contract.facts);
        let is_crash = matches!(
            contract.kind,
            typed_trees::signature::SignatureContractKind::Crashes { .. }
        );
        if is_crash && !include_crashes {
            continue;
        }
        if is_crash {
            let bucket = crash_buckets.entry(contract_prefix).or_default();
            if facts.is_empty() || facts.iter().any(|fact| is_true_crash_route(program, fact)) {
                bucket.unconditional = true;
            } else {
                for fact in facts {
                    let mut route = Vec::new();
                    encode_contract_fact_canonical(
                        program,
                        fact,
                        parameter_names,
                        content_conservation,
                        canonicalize_membership_value,
                        &mut route,
                    );
                    bucket.routes.push(route);
                }
            }
            continue;
        }

        for fact in facts {
            let mut contract_bytes = contract_prefix.clone();
            encode_contract_fact_canonical(
                program,
                fact,
                parameter_names,
                content_conservation,
                canonicalize_membership_value,
                &mut contract_bytes,
            );
            encoded.push(contract_bytes);
        }
    }

    for (contract_prefix, mut bucket) in crash_buckets {
        if bucket.unconditional {
            let mut contract = contract_prefix;
            contract.push(0);
            encoded.push(contract);
            continue;
        }
        bucket.routes.sort();
        bucket.routes.dedup();
        for route in bucket.routes {
            let mut contract = contract_prefix.clone();
            contract.push(1);
            contract.extend(route);
            encoded.push(contract);
        }
    }
    encoded.sort();
    encoded
}

fn encode_contract_fact_canonical(
    program: &TypedTrees,
    fact: &typed_trees::domain::ProofFact,
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
    canonicalize_membership_value: bool,
    output: &mut Vec<u8>,
) {
    match fact {
        typed_trees::domain::ProofFact::Expression(expression) => {
            output.push(1);
            encode_contract_expression_canonical(
                program,
                *expression,
                parameter_names,
                content_conservation,
                output,
            );
        }
        typed_trees::domain::ProofFact::Membership(membership) => {
            output.push(2);
            if canonicalize_membership_value {
                encode_expression_canonical(program, membership.value, parameter_names, output);
            } else {
                output.extend(
                    program
                        .expression_table
                        .display_name(membership.value)
                        .as_bytes(),
                );
            }
            output.push(0);
            let domain_path = if canonicalize_membership_value {
                program.domain_path_members(membership.domain)
            } else {
                program
                    .expression_table
                    .name_path_members(membership.domain)
            };
            for member in domain_path {
                output.extend(member.as_str().as_bytes());
                output.push(b':');
            }
        }
        typed_trees::domain::ProofFact::Proposition(application) => {
            output.push(3);
            encode_proposition_application_canonical(program, application, parameter_names, output);
        }
    }
}

fn encode_type_spelling(text: &str, binders: &[(String, String)], output: &mut Vec<u8>) {
    let mut word = String::new();
    let flush = |word: &mut String, output: &mut Vec<u8>| {
        if word.is_empty() {
            return;
        }
        if let Some((_, replacement)) = binders.iter().find(|(name, _)| name == word) {
            output.extend(replacement.as_bytes());
        } else {
            output.extend(word.as_bytes());
        }
        word.clear();
    };
    for character in text.chars() {
        if character.is_ascii_alphanumeric() || character == '_' {
            word.push(character);
        } else {
            flush(&mut word, output);
            output.extend(character.to_string().as_bytes());
        }
    }
    flush(&mut word, output);
    output.push(0);
}

/// STR4 checked plans, slice 2 (decision 19): collect each machine's
/// semantic-domain COMMITMENTS -- v1 walks its statements' expressions for
/// arithmetic-policy casts (`x as u8 in Saturating`; the compiler-blessed
/// closed semantic-facet subset) and normalizes the policy to its FIXED
/// SemanticDomainTable identity. Sorted + deduped; cast-free machines carry
/// no entry.
fn build_qualification_facts(program: &TypedTrees) -> checked_trees::QualificationFacts {
    use checked_trees::VacuousQualificationUse;
    use language_semantics::SemanticDomainTable;
    use std::collections::HashSet;
    use symbols::SymbolHandle;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};

    fn domain_is_vacuous(
        program: &TypedTrees,
        domain_symbol: SymbolHandle,
        stack: &mut Vec<SymbolHandle>,
    ) -> bool {
        if !domain_symbol.is_valid() || stack.contains(&domain_symbol) {
            return false;
        }
        let Some(domain) = program
            .domain_definitions()
            .iter()
            .find(|candidate| candidate.symbol == domain_symbol)
        else {
            return false;
        };
        if let Some(alias) = domain.alias.as_ref() {
            if alias.constituents.is_empty() {
                return false;
            }
            stack.push(domain_symbol);
            let vacuous = alias
                .constituents
                .iter()
                .all(|constituent| domain_is_vacuous(program, constituent.domain_symbol, stack));
            stack.pop();
            return vacuous;
        }
        !domain.predicate_body.is_present() && domain.establishment_routes.is_empty()
    }

    fn collect_casts(
        program: &TypedTrees,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement_index: u32,
        expression: ExpressionHandle,
        committed: &mut Vec<language_semantics::SemanticDomainId>,
        vacuous_uses: &mut Vec<VacuousQualificationUse>,
        visited: &mut HashSet<u32>,
    ) {
        if !expression.is_valid() || !visited.insert(expression.arena_index()) {
            return;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Cast(cast) => {
                let policy = match cast.domain {
                    numerics::arithmetic::ArithmeticDomain::Exact => None,
                    numerics::arithmetic::ArithmeticDomain::Wrapping => {
                        Some(SemanticDomainTable::WRAPPING)
                    }
                    numerics::arithmetic::ArithmeticDomain::Saturating => {
                        Some(SemanticDomainTable::SATURATING)
                    }
                    numerics::arithmetic::ArithmeticDomain::Trapping => {
                        Some(SemanticDomainTable::TRAPPING)
                    }
                };
                if let Some(policy) = policy {
                    committed.push(policy);
                }
                // Declared-domain casts already carry the normalized symbol
                // selected before validation. Checked facts consume that
                // identity directly rather than repeating short-name lookup.
                if cast.semantic_domain_symbol.is_valid()
                    && let Some(domain) = program
                        .domain_definitions()
                        .iter()
                        .find(|domain| domain.symbol == cast.semantic_domain_symbol)
                {
                    let semantic_id = if cast.semantic_domain_id.is_valid() {
                        cast.semantic_domain_id
                    } else {
                        domain.semantic_id
                    };
                    if semantic_id.is_valid() {
                        committed.push(semantic_id);
                    }
                    if domain_is_vacuous(program, domain.symbol, &mut Vec::new()) {
                        vacuous_uses.push(VacuousQualificationUse {
                            machine,
                            state,
                            statement_index,
                            expression,
                            domain: domain.symbol,
                            semantic_domain: semantic_id,
                        });
                    }
                }
                collect_casts(
                    program,
                    machine,
                    state,
                    statement_index,
                    cast.value,
                    committed,
                    vacuous_uses,
                    visited,
                );
            }
            ExpressionNode::Binary(binary) => {
                collect_casts(
                    program,
                    machine,
                    state,
                    statement_index,
                    binary.left,
                    committed,
                    vacuous_uses,
                    visited,
                );
                collect_casts(
                    program,
                    machine,
                    state,
                    statement_index,
                    binary.right,
                    committed,
                    vacuous_uses,
                    visited,
                );
            }
            ExpressionNode::Unary(unary) => collect_casts(
                program,
                machine,
                state,
                statement_index,
                unary.operand,
                committed,
                vacuous_uses,
                visited,
            ),
            ExpressionNode::Member(member) => collect_casts(
                program,
                machine,
                state,
                statement_index,
                member.receiver,
                committed,
                vacuous_uses,
                visited,
            ),
            ExpressionNode::Borrow(inner) => collect_casts(
                program,
                machine,
                state,
                statement_index,
                inner.target,
                committed,
                vacuous_uses,
                visited,
            ),
            ExpressionNode::Indexed(indexed) => {
                collect_casts(
                    program,
                    machine,
                    state,
                    statement_index,
                    indexed.collection,
                    committed,
                    vacuous_uses,
                    visited,
                );
                collect_casts(
                    program,
                    machine,
                    state,
                    statement_index,
                    indexed.index,
                    committed,
                    vacuous_uses,
                    visited,
                );
            }
            ExpressionNode::Range(range) => {
                collect_casts(
                    program,
                    machine,
                    state,
                    statement_index,
                    range.start,
                    committed,
                    vacuous_uses,
                    visited,
                );
                collect_casts(
                    program,
                    machine,
                    state,
                    statement_index,
                    range.end,
                    committed,
                    vacuous_uses,
                    visited,
                );
            }
            ExpressionNode::Call(call) => {
                collect_casts(
                    program,
                    machine,
                    state,
                    statement_index,
                    call.receiver,
                    committed,
                    vacuous_uses,
                    visited,
                );
                for argument in program.expression_table.expression_handles(call.arguments) {
                    collect_casts(
                        program,
                        machine,
                        state,
                        statement_index,
                        *argument,
                        committed,
                        vacuous_uses,
                        visited,
                    );
                }
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in program.expression_table.struct_fields(literal.fields) {
                    collect_casts(
                        program,
                        machine,
                        state,
                        statement_index,
                        field.value,
                        committed,
                        vacuous_uses,
                        visited,
                    );
                }
            }
            ExpressionNode::ArrayLiteral(items) => {
                for item in program.expression_table.expression_handles(*items) {
                    collect_casts(
                        program,
                        machine,
                        state,
                        statement_index,
                        *item,
                        committed,
                        vacuous_uses,
                        visited,
                    );
                }
            }
            _ => {}
        }
    }

    let mut machines = Vec::new();
    let mut vacuous_uses = Vec::new();
    for machine in program.machines() {
        let mut committed = Vec::new();
        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                use typed_trees::statement::StatementNode;
                let statement_index =
                    u32::try_from(statement_index).expect("qualification statement index overflow");
                let mut visited = HashSet::new();
                match statement {
                    StatementNode::AssemblyFact(_) => {}
                    StatementNode::Assignment(assignment) => {
                        collect_casts(
                            program,
                            machine.symbol,
                            state.symbol,
                            statement_index,
                            assignment.target,
                            &mut committed,
                            &mut vacuous_uses,
                            &mut visited,
                        );
                        collect_casts(
                            program,
                            machine.symbol,
                            state.symbol,
                            statement_index,
                            assignment.value,
                            &mut committed,
                            &mut vacuous_uses,
                            &mut visited,
                        );
                    }
                    StatementNode::Expression(expression) => {
                        collect_casts(
                            program,
                            machine.symbol,
                            state.symbol,
                            statement_index,
                            *expression,
                            &mut committed,
                            &mut vacuous_uses,
                            &mut visited,
                        );
                    }
                    StatementNode::LocalData(local) => {
                        collect_casts(
                            program,
                            machine.symbol,
                            state.symbol,
                            statement_index,
                            local.initial_value,
                            &mut committed,
                            &mut vacuous_uses,
                            &mut visited,
                        );
                    }
                    StatementNode::Call(call) => {
                        for argument in program.statement_table.expression_handles(call.arguments) {
                            collect_casts(
                                program,
                                machine.symbol,
                                state.symbol,
                                statement_index,
                                *argument,
                                &mut committed,
                                &mut vacuous_uses,
                                &mut visited,
                            );
                        }
                    }
                    StatementNode::Transition(transition) => {
                        if let typed_trees::statement::TransitionGuardNode::When(guard) =
                            &transition.guard
                        {
                            collect_casts(
                                program,
                                machine.symbol,
                                state.symbol,
                                statement_index,
                                *guard,
                                &mut committed,
                                &mut vacuous_uses,
                                &mut visited,
                            );
                        }
                        for target in [transition.target, transition.continuation] {
                            if !target.is_valid() {
                                continue;
                            }
                            match program.statement_table.transition_target(target) {
                                typed_trees::statement::TransitionTargetNode::Value(value) => {
                                    collect_casts(
                                        program,
                                        machine.symbol,
                                        state.symbol,
                                        statement_index,
                                        *value,
                                        &mut committed,
                                        &mut vacuous_uses,
                                        &mut visited,
                                    )
                                }
                                typed_trees::statement::TransitionTargetNode::Named {
                                    arguments,
                                    ..
                                } => {
                                    for argument in
                                        program.statement_table.expression_handles(*arguments)
                                    {
                                        collect_casts(
                                            program,
                                            machine.symbol,
                                            state.symbol,
                                            statement_index,
                                            *argument,
                                            &mut committed,
                                            &mut vacuous_uses,
                                            &mut visited,
                                        );
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
        committed.sort_by_key(|id| id.0);
        committed.dedup();
        if !committed.is_empty() {
            machines.push(checked_trees::MachineQualifications {
                machine: machine.symbol,
                body_committed: committed,
            });
        }
    }
    checked_trees::QualificationFacts {
        machines,
        vacuous_uses,
        content: checked_trees::ContentProjectionFacts {
            plans: validation::build_content_projection_plans(program),
            conservation_plans: validation::build_content_conservation_plans(program)
                .into_iter()
                .map(|source| source.plan)
                .collect(),
            identity_reshuffles: Vec::new(),
            partition_compositions: Vec::new(),
            retained_borrow_custodies: Vec::new(),
        },
    }
}

/// Build the boundary-symbol service fixed point without consulting the
/// legacy global effect catalog. A direct boundary-signature call contributes
/// its containing service plus the signature's explicitly reached services.
/// Checked local callees contribute honest inferred bodies; requirements,
/// boundaries, external realizations, and authored checked ceilings contribute
/// their published row.
fn build_service_reach_facts(
    program: &TypedTrees,
    inferred: flow_effects::ServiceReachInferencePlan,
) -> checked_trees::ServiceReachFacts {
    checked_trees::ServiceReachFacts {
        services: program.service_reaches.clone(),
        rows: inferred.rows,
        root_machines: remap_service_reach_span(inferred.root_machines),
        machines: inferred
            .machines
            .map(|machine| checked_trees::MachineServiceReachRows {
                machine: machine.machine,
                interface: machine.interface,
                published_ceiling: machine.published,
                inferred_direct: machine.inferred_direct,
                inferred_transitive: machine.inferred_transitive,
                concrete_transitive: machine.concrete_transitive,
                effective: machine.effective,
                concrete_effective: machine.concrete_effective,
                unresolved_installation_reaches: machine.unresolved_installation_reaches,
                states: remap_service_reach_span(machine.states),
            }),
        states: inferred
            .states
            .map(|state| checked_trees::StateServiceReachRows {
                state: state.state,
                inferred_direct: state.inferred_direct,
                inferred_transitive: state.inferred_transitive,
                concrete_direct: state.concrete_direct,
                concrete_transitive: state.concrete_transitive,
                unresolved_installation_reaches: state.unresolved_installation_reaches,
                calls: remap_service_reach_span(state.calls),
            }),
        calls: inferred
            .calls
            .map(|call| checked_trees::CallServiceReachRows {
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
                target_state: call.target_state,
                target_machine: call.target_machine,
                inferred_direct: call.inferred_direct,
                inferred_transitive: call.inferred_transitive,
                concrete_direct: call.concrete_direct,
                concrete_transitive: call.concrete_transitive,
                unresolved_installation_reaches: call.unresolved_installation_reaches,
            }),
    }
}

fn remap_service_reach_span<From, To>(span: arena::HandleSpan<From>) -> arena::HandleSpan<To> {
    let start = span.start();
    arena::HandleSpan::from_parts(
        arena::Handle::from_parts(start.arena_index(), start.generation()),
        span.count(),
    )
}

/// A stable, spelling-independent byte encoding of a contract fact
/// expression: prefix walk with operator tags, name paths as text, integer
/// literals as text (exact at any magnitude). Deterministic across
/// programs for the same declared clause.
fn encode_expression_canonical(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
    parameter_names: &[String],
    out: &mut Vec<u8>,
) {
    use typed_trees::expression::ExpressionNode;
    if !expression.is_valid() {
        out.push(0);
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => {
            out.push(1);
            out.push(binary.operator as u8);
            encode_expression_canonical(program, binary.left, parameter_names, out);
            encode_expression_canonical(program, binary.right, parameter_names, out);
        }
        ExpressionNode::Unary(unary) => {
            out.push(2);
            out.push(unary.operator as u8);
            encode_expression_canonical(program, unary.operand, parameter_names, out);
        }
        ExpressionNode::Integer(value) => {
            out.push(3);
            out.extend(value.text().as_bytes());
            out.push(0);
        }
        ExpressionNode::Boolean(value) => {
            out.push(4);
            out.push(u8::from(*value));
        }
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            // A bare parameter name normalizes to its POSITION -- renames
            // never change the contract identity.
            if let [single] = members
                && let Some(index) = parameter_names
                    .iter()
                    .position(|name| name == single.as_str())
            {
                out.push(9);
                out.extend(
                    u32::try_from(index)
                        .expect("parameter index fits u32")
                        .to_le_bytes(),
                );
                return;
            }
            out.push(5);
            for member in members {
                out.extend(member.as_str().as_bytes());
                out.push(b'.');
            }
            out.push(0);
        }
        ExpressionNode::Member(member) => {
            out.push(6);
            encode_expression_canonical(program, member.receiver, parameter_names, out);
            out.extend(member.member.as_str().as_bytes());
            out.push(0);
        }
        ExpressionNode::Call(call) => {
            out.push(7);
            out.extend(call.target.as_str().as_bytes());
            out.push(0);
            encode_expression_canonical(program, call.receiver, parameter_names, out);
            for argument in program.expression_table.expression_handles(call.arguments) {
                encode_expression_canonical(program, *argument, parameter_names, out);
            }
            out.push(0xfe);
        }
        // Anything else falls back to the display name -- stable per
        // spelling (a conservative widening; refine per-node as shapes
        // arrive in contracts).
        other => {
            let _ = other;
            out.push(8);
            out.extend(program.expression_table.display_name(expression).as_bytes());
            out.push(0);
        }
    }
}

fn encode_proposition_application_canonical(
    program: &TypedTrees,
    application: &typed_trees::proposition::PropositionApplication,
    parameter_names: &[String],
    out: &mut Vec<u8>,
) {
    let binder_labels = application
        .binder_arguments
        .iter()
        .map(|binder| binder.display_name())
        .collect::<Vec<_>>();
    let argument_labels = program
        .expression_table
        .expression_handles(application.arguments)
        .iter()
        .map(|argument| {
            let mut bytes = Vec::new();
            encode_expression_canonical(program, *argument, parameter_names, &mut bytes);
            bytes
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        })
        .collect::<Vec<_>>();
    if let Some(formula) = program.normalize_proposition_application_with_labels(
        application,
        &binder_labels,
        &argument_labels,
    ) {
        out.extend(formula.identity_label().as_bytes());
    } else {
        // Invalid/cyclic aliases are rejected by validation; retaining a
        // fail-closed marker prevents an accidental identity collision here.
        out.extend(b"invalid-proposition-application");
    }
    out.push(0);
}

fn encode_contract_expression_canonical(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
    out: &mut Vec<u8>,
) {
    if let Some(conservation) = content_conservation
        .iter()
        .find(|candidate| candidate.source_expression == expression)
    {
        out.push(0xcc);
        out.extend(
            language_semantics::content::content_conservation_plan_bytes(&conservation.plan),
        );
        return;
    }
    encode_expression_canonical(program, expression, parameter_names, out);
}

/// Canonical identity of one body-derived path predicate in the namespace of
/// a machine's published crash routes. This deliberately shares the exact
/// encoder used by [`build_published_crash_plan`]: checked guard coverage may
/// join a source expression to a published bucket only after both normalize to
/// the same source-handle-free bytes.
pub(crate) fn canonical_crash_path_predicate(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
    negated: bool,
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
) -> checked_trees::CrashPredicateIdentity {
    let expression = crash_calls::crash_predicate_from_expression(
        program,
        expression,
        parameter_names,
        Some(content_conservation),
    );
    let expression = if negated {
        checked_trees::CrashPredicateExpression::Unary {
            operator: typed_trees::expression::UnaryOperator::LogicalNot as u8,
            operand: Box::new(expression),
        }
    } else {
        expression
    };
    checked_trees::CrashPredicateIdentity::from_expression(expression)
}

/// Canonical identity of a checker-derived binary predicate assembled from
/// existing typed operands. This uses the published crash-route encoder but
/// never rewrites the published contract itself.
pub(crate) fn canonical_crash_binary_path_predicate(
    program: &TypedTrees,
    operator: typed_trees::expression::BinaryOperator,
    left: typed_trees::expression::ExpressionHandle,
    right: typed_trees::expression::ExpressionHandle,
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
) -> checked_trees::CrashPredicateIdentity {
    let left = crash_calls::crash_predicate_from_expression(
        program,
        left,
        parameter_names,
        Some(content_conservation),
    );
    let right = crash_calls::crash_predicate_from_expression(
        program,
        right,
        parameter_names,
        Some(content_conservation),
    );
    checked_trees::CrashPredicateIdentity::from_expression(
        checked_trees::CrashPredicateExpression::Binary {
            operator: operator as u8,
            left: Box::new(left),
            right: Box::new(right),
        },
    )
}

/// Canonical source-handle-free identity for one operand participating in a
/// checker-derived crash-predicate relation. This is an internal join key;
/// only complete predicate identities enter checked crash plans.
pub(crate) fn canonical_crash_operand_identity(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
) -> checked_trees::CrashPredicateIdentity {
    let mut bytes = vec![0x6f]; // checker-private operand namespace
    encode_contract_expression_canonical(
        program,
        expression,
        parameter_names,
        content_conservation,
        &mut bytes,
    );
    checked_trees::CrashPredicateIdentity::from_canonical_bytes(bytes)
}
