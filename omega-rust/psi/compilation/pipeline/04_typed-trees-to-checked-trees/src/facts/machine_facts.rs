//! Termination, blocking, suspension and synchronous invocation facts.

use crate::facts::{MachineBlockingRow, MachineSuspensionRow};

pub(crate) fn build_termination_facts(
    program: &dyn ::validation::CheckingInput,
    flow: &checked_trees::FlowFacts,
    semantic: &facts::FactPlan,
    cleanup_dependencies: &arena::Arena<flow_effects::CleanupDependency>,
    validation: &validation::ProgramValidationFacts,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Result<checked_trees::TerminationFacts, Vec<diagnostics::Diagnostic>> {
    let rank_arrivals = crate::checks::termination::admit_machine_rankings(program, call_frames);
    let summaries = crate::checks::termination::analyze_checked_progress(
        program,
        flow,
        semantic,
        cleanup_dependencies,
        &validation.proof_recursive_components,
        &validation.self_call_descents,
        call_frames,
        &rank_arrivals,
    )?;
    Ok(checked_trees::TerminationFacts {
        rank_arrivals,
        machines: program
            .machines()
            .iter()
            .map(|machine| checked_trees::MachineTerminationFact {
                machine: machine.symbol,
                plan: crate::checks::termination::build_checked_termination_plan(
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

pub(crate) fn build_blocking_facts(
    program: &dyn ::validation::CheckingInput,
    blocking: &[MachineBlockingRow],
) -> checked_trees::BlockingFacts {
    let machines = program
        .machines()
        .iter()
        .map(|machine| {
            let blocking_row = blocking.iter().find(|row| row.symbol == machine.symbol);
            let checked_may_block = blocking_row.is_some_and(|row| row.transitive_may_block);
            // A virtual family head publishes the completed member envelope,
            // while every authored member retains its own operational contract.
            let published_may_block = if machine.target_family_interface {
                checked_may_block
            } else {
                machine.blocks
            };
            let publishes_operational_contract = machine.is_public
                || machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody;
            checked_trees::MachineBlockingFact {
                machine: machine.symbol,
                plan: language_semantics::BlockingPlan {
                    interface: if publishes_operational_contract || machine.blocks {
                        language_semantics::BlockingInterface::PublishedMayBlock(
                            published_may_block,
                        )
                    } else {
                        language_semantics::BlockingInterface::InternalInferred
                    },
                    checked_may_block,
                },
            }
        })
        .collect();
    checked_trees::BlockingFacts { machines }
}

pub(crate) fn build_suspension_facts(
    program: &dyn ::validation::CheckingInput,
    suspensions: &[MachineSuspensionRow],
) -> checked_trees::SuspensionFacts {
    let machines = program
        .machines()
        .iter()
        .map(|machine| {
            let suspension_row = suspensions.iter().find(|row| row.symbol == machine.symbol);
            let checked_may_suspend = suspension_row.is_some_and(|row| row.transitive_may_suspend);
            let published_may_suspend = if machine.target_family_interface {
                checked_may_suspend
            } else {
                machine.suspends
            };
            let publishes_operational_contract = machine.is_public
                || machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody;
            checked_trees::MachineSuspensionFact {
                machine: machine.symbol,
                plan: language_semantics::SuspensionPlan {
                    interface: if publishes_operational_contract || machine.suspends {
                        language_semantics::SuspensionInterface::PublishedMaySuspend(
                            published_may_suspend,
                        )
                    } else {
                        language_semantics::SuspensionInterface::InternalInferred
                    },
                    checked_may_suspend,
                },
            }
        })
        .collect();
    checked_trees::SuspensionFacts { machines }
}

pub(crate) fn build_synchronous_invocation_facts(
    program: &dyn ::validation::CheckingInput,
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
                    .trait_definition_by_symbol(symbol)
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
