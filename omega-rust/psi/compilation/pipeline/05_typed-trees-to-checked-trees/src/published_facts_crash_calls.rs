/*
    Check which published crash conditions remain possible at each call.
    A selected requirement keeps its full public signature and the argument
    values evaluated for that invocation. Private helpers compose those same
    conditions with their own checked bodies.
    Symbolic conditions retain caller entry-value identities. Checked scalar
    expressions also use captured values and bounds; missing evidence leaves
    the possible crash in place.

    This file attaches checked crash calls and infers crash causes.
    `summary_predicates.rs` combines and groups possible crash conditions.
    `route_substitution.rs` replaces declared parameters with call arguments.
    `private_summaries.rs` finds crashes that can propagate through private calls.
    `requirement_contracts.rs` prepares each call's selected public signature.
*/

type CapsuleTargetKey = (
    symbols::SymbolHandle,
    symbols::SymbolHandle,
    Option<typed_trees::callable_values::RuntimeCallableContractHandle>,
);

fn capsule_target_key(capsule: &checked_trees::CrashContractCapsule) -> CapsuleTargetKey {
    (
        capsule.target_machine(),
        capsule.target_state(),
        capsule
            .runtime_callable_source()
            .map(|(contract, _)| contract),
    )
}

mod potential_causes {
    /*
        Conservative public cause envelopes over the prepared crash dependency graph.
        Invocation proofs may narrow private call routes, but cannot erase a cause
        from a generated family contract. Authored member guards stay on the head;
        only inferred member causes need an additional unconditional permission.
    */
    use super::private_summaries::{SummaryNode, needs_inferred_summary};
    use checked_trees::{CrashCause, CrashContractCapsule, MachineContractPlan};
    use symbols::{SymbolHandle, SymbolKeyMap};
    use validation::CheckingInput;

    struct CauseEquation {
        machine: SymbolHandle,
        direct: Vec<CrashCause>,
        dependencies: Vec<SymbolHandle>,
    }

    pub(super) fn infer(
        program: &dyn CheckingInput,
        nodes: &[SummaryNode],
        capsules: &[CrashContractCapsule],
        plans: &[MachineContractPlan],
    ) -> Vec<(SymbolHandle, Vec<CrashCause>)> {
        let mut plan_by_machine = SymbolKeyMap::<SymbolHandle, &MachineContractPlan>::default();
        for plan in plans {
            plan_by_machine.entry(plan.machine).or_insert(plan);
        }
        let mut capsule_by_target =
            SymbolKeyMap::<super::CapsuleTargetKey, &CrashContractCapsule>::default();
        for capsule in capsules {
            capsule_by_target
                .entry(super::capsule_target_key(capsule))
                .or_insert(capsule);
        }
        let mut equations = Vec::with_capacity(nodes.len());
        let mut node_index_by_machine = SymbolKeyMap::<SymbolHandle, usize>::default();
        for (index, node) in nodes.iter().enumerate() {
            node_index_by_machine.entry(node.machine).or_insert(index);
            let plan = plan_by_machine
                .get(&node.machine)
                .copied()
                .expect("a viable summary node has its local contract plan");
            let machine = program
                .machine_by_symbol(node.machine)
                .expect("a viable summary node has its typed declaration");
            let mut direct = Vec::new();
            let mut dependencies = Vec::new();
            if machine.target_family_interface {
                direct.extend(plan.crash.published().iter().map(|bucket| bucket.cause()));
            } else {
                direct.extend(plan.crash.checked_sites().iter().map(|site| site.cause()));
                if !plan.crash.trapping_sites().is_empty() {
                    direct.push(CrashCause::Trap);
                }
                for operator in plan.crash.checked_operators() {
                    direct.extend(operator.published.iter().map(|bucket| bucket.cause()));
                }
            }
            for member in &node.family_members {
                let member_plan = plan_by_machine
                    .get(member)
                    .copied()
                    .expect("a viable family member has its local contract plan");
                let member_machine = program
                    .machine_by_symbol(*member)
                    .expect("a viable family member has its typed declaration");
                if needs_inferred_summary(member_machine, member_plan) {
                    dependencies.push(*member);
                }
            }
            for invocation in &node.invocations {
                if let Some(&target_plan) = plan_by_machine.get(&invocation.target_machine) {
                    let target_machine = program
                        .machine_by_symbol(invocation.target_machine)
                        .expect("a viable local invocation has its typed declaration");
                    if needs_inferred_summary(target_machine, target_plan) {
                        dependencies.push(invocation.target_machine);
                    } else {
                        direct.extend(
                            target_plan
                                .crash
                                .published()
                                .iter()
                                .map(|bucket| bucket.cause()),
                        );
                    }
                } else {
                    let capsule = capsule_by_target
                        .get(&(
                            invocation.target_machine,
                            invocation.target_state,
                            invocation.runtime_contract,
                        ))
                        .copied()
                        .expect("a viable abstract invocation has its pinned crash capsule");
                    direct.extend(
                        capsule
                            .published_buckets()
                            .iter()
                            .map(|bucket| bucket.cause()),
                    );
                }
            }
            direct.sort_unstable();
            direct.dedup();
            equations.push(CauseEquation {
                machine: node.machine,
                direct,
                dependencies,
            });
        }
        let mut resolved = equations
            .iter()
            .map(|equation| (equation.machine, equation.direct.clone()))
            .collect::<Vec<_>>();
        loop {
            let mut changed = false;
            for (equation_index, equation) in equations.iter().enumerate() {
                let mut causes = equation.direct.clone();
                for dependency in &equation.dependencies {
                    let &target_index = node_index_by_machine
                        .get(dependency)
                        .expect("every summary dependency belongs to the viable fixed point");
                    causes.extend_from_slice(&resolved[target_index].1);
                }
                causes.sort_unstable();
                causes.dedup();
                let current = &mut resolved[equation_index].1;
                if *current != causes {
                    *current = causes;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| {
                program
                    .machine_by_symbol(node.machine)
                    .is_some_and(|machine| machine.target_family_interface)
            })
            .map(|(equation_index, node)| {
                let equation = &equations[equation_index];
                let mut inferred = Vec::new();
                for dependency in &equation.dependencies {
                    let &target_index = node_index_by_machine
                        .get(dependency)
                        .expect("every family dependency has a resolved cause summary");
                    inferred.extend_from_slice(&resolved[target_index].1);
                }
                inferred.sort_unstable();
                inferred.dedup();
                (node.machine, inferred)
            })
            .collect()
    }
}
mod private_summaries {
    /*
        Find crashes that can propagate through private calls and target-body groups.
        Repeat until callees no longer change their callers' possible crashes.
        Group membership determines which summaries must be considered together.
    */
    use super::requirement_contracts::RequirementCrashContracts;
    use crate::flow::expressions;
    use crate::published::facts::crash_calls::route_substitution::{
        call_argument_substitution, refine_published_crash_routes,
    };
    use crate::published::facts::crash_calls::summary_predicates::{
        CallArgumentSubstitution, SummaryCrashBucket, SummaryCrashPredicate,
        SummaryCrashRouteGuard, normalize_summary_buckets,
    };
    use crate::published::facts::crash_plan_facts::declared_parameter_names;
    use arena::Handle;
    use checked_trees::CheckedNamedOperatorUseFact;
    use checked_trees::CheckedOperatorFacts;
    use checked_trees::CheckedValueOrigin;
    use checked_trees::CrashCause;
    use checked_trees::CrashContractCapsule;
    use checked_trees::CrashPredicateIdentity;
    use checked_trees::CrashRouteBucket;
    use checked_trees::CrashRouteGuard;
    use checked_trees::CrashSiteLocation;
    use checked_trees::FlowFacts;
    use checked_trees::MachineContractPlan;
    use checked_trees::contract_plan_by_machine;
    use diagnostics::Diagnostic;
    use facts::FactPlan;
    use language_semantics::MachineSupplyMode;
    use symbols::{SymbolHandle, SymbolKeyMap};
    use typed_trees::byte_predicates::byte_predicate_call;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::machine::Machine;
    use typed_trees::machine::MachineBodyStatus;
    use typed_trees::operator::declaration_by_symbol;
    use typed_trees::signature::SignatureContract;
    use typed_trees::signature::SignatureContractKind;
    use typed_trees::signature::StateSignature;
    use validation::CallSite;
    use validation::ContentConservationSourcePlan;
    use validation::ExactIntegerCastFact;
    use validation::call_site_argument_expressions;
    use validation::checking_thread_pool;
    use validation::contract_structure::is_true_crash_route;
    use validation::find_call_site;

    pub(crate) struct PreparedCrashSummaryGraph<'proof> {
        nodes: Vec<SummaryNode>,
        family_potential_causes: Vec<(SymbolHandle, Vec<CrashCause>)>,
        pub(super) requirement_contracts: RequirementCrashContracts<'proof>,
    }

    impl PreparedCrashSummaryGraph<'_> {
        pub(crate) fn family_potential_crash_causes(&self) -> &[(SymbolHandle, Vec<CrashCause>)] {
            &self.family_potential_causes
        }
    }

    /*
        Prepare one viable dependency graph for public potential causes and later
        invocation refinement. Public causes never consume call-site proof pruning.
    */
    pub(crate) fn prepare_crash_summary_graph<'proof>(
        program: &dyn ::validation::CheckingInput,
        proof: &'proof checked_trees::ProofFacts,
        operators: &CheckedOperatorFacts,
        flow: &FlowFacts,
        crash_capsules: &[CrashContractCapsule],
        plans: &[MachineContractPlan],
        content_conservation: &[ContentConservationSourcePlan],
    ) -> Result<PreparedCrashSummaryGraph<'proof>, Vec<Diagnostic>> {
        let requirement_contracts = RequirementCrashContracts::prepare(
            program,
            proof,
            flow,
            operators,
            content_conservation,
        )?;
        let nodes = prepare_summary_nodes(
            program,
            operators,
            flow,
            crash_capsules,
            plans,
            &requirement_contracts,
        );
        let incomplete = program
            .machines()
            .iter()
            .filter(|machine| {
                machine.target_family_interface
                    && !nodes.iter().any(|node| node.machine == machine.symbol)
            })
            .map(|machine| {
                let reason = match complete_family_members(program, machine, plans) {
                    Err(reason) => reason,
                    Ok(members) => unviable_member_dependency(
                        program,
                        operators,
                        flow,
                        crash_capsules,
                        plans,
                        &nodes,
                        &members,
                        &requirement_contracts,
                    )
                    .unwrap_or_else(|| {
                        "incomplete or unresolved member invocation dependencies".to_owned()
                    }),
                };
                Diagnostic::error(format!(
                    "cannot publish target-family crash envelope for `{}`: {reason}",
                    machine.name,
                ))
            })
            .collect::<Vec<_>>();
        if !incomplete.is_empty() {
            return Err(incomplete);
        }
        let family_potential_causes =
            super::potential_causes::infer(program, &nodes, crash_capsules, plans);
        Ok(PreparedCrashSummaryGraph {
            nodes,
            family_potential_causes,
            requirement_contracts,
        })
    }

    type NamedOperatorUseMap<'a> = SymbolKeyMap<
        ExpressionHandle,
        Vec<(
            Handle<CheckedNamedOperatorUseFact>,
            &'a CheckedNamedOperatorUseFact,
        )>,
    >;

    fn build_named_operator_use_map(operators: &CheckedOperatorFacts) -> NamedOperatorUseMap<'_> {
        let mut by_expression = NamedOperatorUseMap::default();
        for (handle, named) in operators.named_uses.iter() {
            by_expression
                .entry(named.expression)
                .or_default()
                .push((handle, named));
        }
        by_expression
    }

    /*
        Name the first member dependency that kept a family out of the viable
        summary graph, so the refusal points at the authored call that caused it.
    */
    fn unviable_member_dependency(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        flow: &FlowFacts,
        crash_capsules: &[CrashContractCapsule],
        plans: &[MachineContractPlan],
        nodes: &[SummaryNode],
        members: &[SymbolHandle],
        requirement_contracts: &RequirementCrashContracts<'_>,
    ) -> Option<String> {
        let named_uses_by_expression = build_named_operator_use_map(operators);
        let viable = |machine: SymbolHandle| nodes.iter().any(|node| node.machine == machine);
        members.iter().find_map(|member| {
            let plan = contract_plan_by_machine(plans, *member)?;
            let machine = program.machine_by_symbol(*member)?;
            if !needs_inferred_summary(machine, plan) || viable(*member) {
                return None;
            }
            let member_name = program.symbols.display_path(*member, "::");
            let sites = match machine_invocation_sites(
                program, &named_uses_by_expression, flow, plan, requirement_contracts,
            ) {
                Ok(sites) => sites,
                Err(reason) => return Some(format!("retained member `{member_name}` {reason}")),
            };
            sites.iter().find_map(|site| {
                let target = program.symbols.display_path(site.target_machine, "::");
                let Some(target_plan) =
                    contract_plan_by_machine(plans, site.target_machine)
                else {
                    let pinned = crash_capsules.iter().any(|capsule| {
                        capsule.target_machine() == site.target_machine
                            && capsule.target_state() == site.target_state
                            && capsule.runtime_callable_source().map(|(contract, _)| contract) == site.runtime_contract
                    });
                    return (!pinned).then(|| {
                        format!("retained member `{member_name}` calls `{target}` without a pinned crash capsule")
                    });
                };
                program
                    .machine_by_symbol(site.target_machine)
                    .filter(|target_machine| {
                        needs_inferred_summary(target_machine, target_plan)
                            && !viable(site.target_machine)
                    })
                    .map(|_| {
                        format!("retained member `{member_name}` calls `{target}`, whose crash summary is unresolved")
                    })
            })
        })
    }

    fn prepare_summary_nodes(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        flow: &FlowFacts,
        crash_capsules: &[CrashContractCapsule],
        plans: &[MachineContractPlan],
        requirement_contracts: &RequirementCrashContracts<'_>,
    ) -> Vec<SummaryNode> {
        let named_uses_by_expression = build_named_operator_use_map(operators);
        let mut nodes = checking_thread_pool().install(|| {
            use rayon::prelude::*;
            plans
                .par_iter()
                .filter(|target| {
                    program
                        .machine_by_symbol(target.machine)
                        .is_some_and(|machine| needs_inferred_summary(machine, target))
                })
                .filter_map(|target| {
                    let machine = program.machine_by_symbol(target.machine)?;
                    let family_members = if machine.target_family_interface {
                        complete_family_members(program, machine, plans).ok()?
                    } else {
                        Vec::new()
                    };
                    Some(SummaryNode {
                        machine: target.machine,
                        direct: if machine.target_family_interface {
                            summary_published_crash_buckets(target.crash.published())
                        } else {
                            inferred_direct_body_crash_buckets(target, operators, None)
                        },
                        family_members,
                        invocations: machine_invocation_sites(
                            program,
                            &named_uses_by_expression,
                            flow,
                            target,
                            requirement_contracts,
                        )
                        .ok()?,
                    })
                })
                .collect::<Vec<_>>()
        });

        let mut plan_indices = SymbolKeyMap::<SymbolHandle, usize>::default();
        for (index, plan) in plans.iter().enumerate() {
            plan_indices.entry(plan.machine).or_insert(index);
        }
        let plan_by_machine = |machine: SymbolHandle| {
            plan_indices
                .get(&machine)
                .and_then(|&index| plans.get(index))
        };
        let mut capsule_targets = SymbolKeyMap::<super::CapsuleTargetKey, ()>::default();
        for capsule in crash_capsules {
            capsule_targets.insert(super::capsule_target_key(capsule), ());
        }

        loop {
            let viable_machines: SymbolKeyMap<SymbolHandle, ()> =
                nodes.iter().map(|node| (node.machine, ())).collect();
            let before = nodes.len();
            nodes.retain(|node| {
                node.family_members.iter().all(|member| {
                    plan_by_machine(*member).is_some_and(|plan| {
                        program.machine_by_symbol(*member).is_some_and(|machine| {
                            !needs_inferred_summary(machine, plan)
                                || viable_machines.contains_key(member)
                        })
                    })
                }) && node.invocations.iter().all(|invocation| {
                    if let Some(plan) = plan_by_machine(invocation.target_machine) {
                        program
                            .machine_by_symbol(plan.machine)
                            .is_some_and(|machine| {
                                !needs_inferred_summary(machine, plan)
                                    || viable_machines.contains_key(&invocation.target_machine)
                            })
                    } else {
                        capsule_targets.contains_key(&(
                            invocation.target_machine,
                            invocation.target_state,
                            invocation.runtime_contract,
                        ))
                    }
                })
            });
            if nodes.len() == before {
                break;
            }
        }
        nodes
    }

    pub(crate) fn infer_private_body_summaries(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        exact_integer_casts: &[ExactIntegerCastFact],
        semantic: &FactPlan,
        flow: &FlowFacts,
        content_conservation: &[ContentConservationSourcePlan],
        crash_capsules: &[CrashContractCapsule],
        plans: &[MachineContractPlan],
        prepared: &PreparedCrashSummaryGraph<'_>,
    ) -> Vec<(SymbolHandle, Vec<SummaryCrashBucket>)> {
        let nodes = &prepared.nodes;
        let mut plan_indices = SymbolKeyMap::<SymbolHandle, usize>::default();
        for (index, plan) in plans.iter().enumerate() {
            plan_indices.entry(plan.machine).or_insert(index);
        }
        let plan_by_machine = |machine: SymbolHandle| {
            plan_indices
                .get(&machine)
                .and_then(|&index| plans.get(index))
        };
        let mut node_indices = SymbolKeyMap::<SymbolHandle, usize>::default();
        for (index, node) in nodes.iter().enumerate() {
            node_indices.entry(node.machine).or_insert(index);
        }
        let mut capsule_by_target =
            SymbolKeyMap::<super::CapsuleTargetKey, &CrashContractCapsule>::default();
        for capsule in crash_capsules {
            capsule_by_target
                .entry(super::capsule_target_key(capsule))
                .or_insert(capsule);
        }

        let equations = checking_thread_pool().install(|| {
            use rayon::prelude::*;
            nodes
                .par_iter()
                .map(|node| {
                    let machine = program
                        .machine_by_symbol(node.machine)
                        .expect("a retained summary node has its typed machine");
                    let entry_conditions =
                        super::entry_paths::EntryConditions::new(program, semantic, flow);
                    let source_paths = entry_conditions.for_machine(machine.symbol);
                    let mut private_dependencies = Vec::new();
                    let mut published_dependencies = Vec::new();
                    for member in &node.family_members {
                        let target_plan = plan_by_machine(*member)
                            .expect("a viable family member has its local contract plan");
                        let target_machine = program
                            .machine_by_symbol(*member)
                            .expect("a viable family member has its typed declaration");
                        if needs_inferred_summary(target_machine, target_plan) {
                            private_dependencies.push(PrivateSummaryDependency {
                                machine: *member,
                                substitution: None,
                                path_guard: SummaryCrashRouteGuard::Truth,
                                recursive: false,
                            });
                        } else {
                            published_dependencies.extend(summary_published_crash_buckets(
                                target_plan.crash.published(),
                            ));
                        }
                    }
                    for invocation in &node.invocations {
                        let state_flow = flow
                            .control
                            .state(node.machine, invocation.caller_state)
                            .expect("a retained summary invocation has its flow state");
                        let call_flow = crate::flow::call_flows(&flow.control, state_flow.calls)
                            .iter()
                            .find(|call| {
                                call.statement_index == invocation.statement_index
                                    && call.call_ordinal == invocation.call_ordinal
                            })
                            .expect("a retained summary invocation has its flow call");
                        let call_site = find_call_site(
                            program,
                            node.machine,
                            invocation.caller_state,
                            invocation.statement_index,
                            invocation.call_ordinal,
                        )
                        .expect("a retained summary invocation has its typed call site");
                        let arguments = call_site_argument_expressions(program, &call_site);
                        let path_guard = source_paths.at_call(call_flow);

                        if let Some(target_plan) = plan_by_machine(invocation.target_machine) {
                            let target_machine = program
                                .machine_by_symbol(invocation.target_machine)
                                .expect("a local crash plan has its typed machine");
                            let target_state = program
                                .state_in_machine(target_machine, invocation.target_state)
                                .expect("a local crash invocation has its typed state");
                            let target_parameters = program.state_parameters(target_state);
                            let target_parameter_names =
                                crate::published::facts::machine_entry_parameter_names(
                                    program,
                                    target_machine,
                                );
                            if needs_inferred_summary(target_machine, target_plan) {
                                private_dependencies.push(PrivateSummaryDependency {
                                    machine: invocation.target_machine,
                                    substitution: Some(call_argument_substitution(
                                        program,
                                        operators,
                                        semantic,
                                        flow,
                                        state_flow,
                                        call_flow,
                                        target_parameters,
                                        arguments,
                                        exact_integer_casts,
                                    )),
                                    path_guard,
                                    recursive: private_dependency_reaches(
                                        program,
                                        nodes,
                                        &node_indices,
                                        &plan_indices,
                                        plans,
                                        invocation.target_machine,
                                        node.machine,
                                    ),
                                });
                            } else if !target_plan.crash.published().is_empty() {
                                published_dependencies.extend(
                                    refine_published_crash_routes(
                                        program,
                                        operators,
                                        exact_integer_casts,
                                        semantic,
                                        flow,
                                        state_flow,
                                        call_flow,
                                        &call_site,
                                        invocation.target_state,
                                        target_parameters,
                                        &target_parameter_names,
                                        target_plan.crash.published(),
                                        program.machine_contracts(target_machine),
                                        content_conservation,
                                    )
                                    .into_iter()
                                    .map(|bucket| bucket.on_path(&path_guard)),
                                );
                            }
                        } else {
                            let selected = prepared.requirement_contracts.for_call(
                                node.machine,
                                invocation.caller_state,
                                invocation.statement_index,
                                invocation.call_ordinal,
                            );
                            let (capsule, signature) = if let Some(selected) = selected {
                                (&selected.capsule, &selected.source.signature)
                            } else {
                                let capsule = capsule_by_target
                                    .get(&(invocation.target_machine, invocation.target_state, invocation.runtime_contract))
                                    .copied()
                                    .expect(
                                        "the viability pass retained only pinned requirement targets",
                                    );
                                let signature = requirement_signature(
                                    program,
                                    invocation.target_machine,
                                    invocation.target_state,
                                )
                                .expect("a pinned requirement capsule has its typed signature");
                                (capsule, signature)
                            };
                            if capsule.published_buckets().is_empty() {
                                continue;
                            }
                            let target_parameters = program.state_signature_parameters(signature);
                            let target_parameter_names = declared_parameter_names(target_parameters);
                            published_dependencies.extend(
                                refine_published_crash_routes(
                                    program,
                                    operators,
                                    exact_integer_casts,
                                    semantic,
                                    flow,
                                    state_flow,
                                    call_flow,
                                    &call_site,
                                    invocation.target_state,
                                    target_parameters,
                                    &target_parameter_names,
                                    capsule.published_buckets(),
                                    program.state_signature_contracts(signature),
                                    content_conservation,
                                )
                                .into_iter()
                                .map(|bucket| bucket.on_path(&path_guard)),
                            );
                        }
                    }
                    PrivateSummaryEquation {
                        machine: node.machine,
                        direct: if program
                            .machine_by_symbol(node.machine)
                            .is_some_and(|machine| machine.target_family_interface)
                        {
                            plan_by_machine(node.machine)
                                .map(|plan| summary_published_crash_buckets(plan.crash.published()))
                                .unwrap_or_else(|| node.direct.clone())
                        } else {
                            inferred_direct_body_crash_buckets(
                                plan_by_machine(node.machine)
                                    .expect("a retained summary node has its contract plan"),
                                operators,
                                Some(&source_paths),
                            )
                        },
                        private_dependencies,
                        published_dependencies: normalize_summary_buckets(published_dependencies),
                    }
                })
                .collect::<Vec<_>>()
        });
        solve_private_summary_fixed_point(&equations)
    }

    pub(super) struct SummaryNode {
        pub(super) machine: SymbolHandle,
        direct: Vec<SummaryCrashBucket>,
        pub(super) family_members: Vec<SymbolHandle>,
        pub(super) invocations: Vec<SummaryInvocationSite>,
    }

    #[derive(Debug, Clone, Copy)]
    pub(super) struct SummaryInvocationSite {
        caller_state: SymbolHandle,
        statement_index: usize,
        call_ordinal: usize,
        pub(super) target_machine: SymbolHandle,
        pub(super) target_state: SymbolHandle,
        pub(super) runtime_contract:
            Option<typed_trees::callable_values::RuntimeCallableContractHandle>,
    }

    pub(crate) struct PrivateSummaryEquation<'program> {
        pub(crate) machine: SymbolHandle,
        pub(crate) direct: Vec<SummaryCrashBucket>,
        pub(crate) private_dependencies: Vec<PrivateSummaryDependency<'program>>,
        pub(crate) published_dependencies: Vec<SummaryCrashBucket>,
    }

    pub(crate) struct PrivateSummaryDependency<'program> {
        pub(crate) machine: SymbolHandle,
        /* None is the positional identity of a family membership dependency. */
        pub(crate) substitution: Option<CallArgumentSubstitution<'program>>,
        pub(crate) path_guard: SummaryCrashRouteGuard,
        pub(crate) recursive: bool,
    }

    pub(crate) fn solve_private_summary_fixed_point(
        equations: &[PrivateSummaryEquation<'_>],
    ) -> Vec<(SymbolHandle, Vec<SummaryCrashBucket>)> {
        let mut equation_index_by_machine = SymbolKeyMap::<SymbolHandle, usize>::default();
        let mut resolved = Vec::with_capacity(equations.len());
        for (index, equation) in equations.iter().enumerate() {
            equation_index_by_machine
                .entry(equation.machine)
                .or_insert(index);
            resolved.push((equation.machine, equation.direct.clone()));
        }
        let mut dependents = vec![Vec::new(); equations.len()];
        for (equation_index, equation) in equations.iter().enumerate() {
            for dependency in &equation.private_dependencies {
                let &target_index = equation_index_by_machine
                    .get(&dependency.machine)
                    .expect("every private dependency belongs to the viable fixed point");
                dependents[target_index].push(equation_index);
            }
        }
        let mut dirty = vec![true; equations.len()];
        for _ in 0..=equations.len() {
            let mut changed = false;
            for (equation_index, equation) in equations.iter().enumerate() {
                if !std::mem::take(&mut dirty[equation_index]) {
                    continue;
                }
                let mut buckets = equation.direct.clone();
                buckets.extend(equation.published_dependencies.clone());
                for dependency in &equation.private_dependencies {
                    let &target_index = equation_index_by_machine
                        .get(&dependency.machine)
                        .expect("every private dependency belongs to the viable fixed point");
                    let selected = &resolved[target_index].1;
                    buckets.extend(selected.iter().map(|bucket| {
                        let bucket = if dependency.recursive {
                            SummaryCrashBucket::unconditional(bucket.cause)
                        } else if let Some(substitution) = &dependency.substitution {
                            bucket.substitute(substitution)
                        } else {
                            bucket.clone()
                        };
                        bucket.on_path(&dependency.path_guard)
                    }));
                }
                let buckets = normalize_summary_buckets(buckets);
                let current = &mut resolved[equation_index].1;
                if *current != buckets {
                    *current = buckets;
                    changed = true;
                    for &dependent in &dependents[equation_index] {
                        dirty[dependent] = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        resolved
    }

    /* Every machine edge of one body, or why one call cannot become an edge. */
    fn machine_invocation_sites(
        program: &dyn ::validation::CheckingInput,
        named_uses_by_expression: &NamedOperatorUseMap<'_>,
        flow: &FlowFacts,
        plan: &MachineContractPlan,
        requirement_contracts: &RequirementCrashContracts<'_>,
    ) -> Result<Vec<SummaryInvocationSite>, String> {
        let mut sites = Vec::new();
        for state in flow.control.machine_states(plan.machine) {
            for call in flow.control.calls.span_or_empty(state.calls) {
                let site = find_call_site(
                    program,
                    state.machine_symbol,
                    state.state_symbol,
                    call.statement_index,
                    call.call_ordinal,
                )
                .ok_or_else(|| {
                    format!(
                        "has no typed call site for statement {} call {}",
                        call.statement_index, call.call_ordinal
                    )
                })?;
                if site.is_internal_state_transfer(program, state.machine_symbol) {
                    continue;
                }
                if program
                    .symbols
                    .builtin_function_for_symbol(call.target_symbol)
                    .is_some_and(|builtin| builtin.is_asm_intrinsic())
                {
                    continue;
                }
                if crate::checks::atomic_fences::contract(program, call).is_some() {
                    continue;
                }
                if let CallSite::Expression { expression, .. } = site
                    && byte_predicate_call(program.typed_program(), expression).is_some()
                {
                    continue;
                }
                if is_selected_build_operation(program, site) {
                    continue;
                }
                if let CallSite::Expression { expression, .. } = site
                    && let Some(&(named_handle, named)) = named_uses_by_expression
                        .get(&expression)
                        .and_then(|candidates| {
                            candidates.iter().find(|(_, named)| {
                                named.selected_operator_symbol == call.target_symbol
                                    && matches!(named.origin,
                                    CheckedValueOrigin::StateStatement {
                                        machine_symbol, state_symbol, statement_index, ..
                                    } if machine_symbol == plan.machine
                                        && state_symbol == state.state_symbol
                                        && statement_index == call.statement_index)
                            })
                        })
                {
                    let operator = declaration_by_symbol(program, named.selected_operator_symbol)
                        .ok_or_else(|| {
                        "selects an operator with no typed declaration".to_owned()
                    })?;
                    let declares_crashes =
                        program.operator_contracts(operator).iter().any(|contract| {
                            matches!(contract.kind, SignatureContractKind::Crashes { .. })
                        });
                    if declares_crashes
                        && !plan.crash.checked_operators().iter().any(|site| {
                            site.named_use == named_handle
                                && site.selected_operator == named.selected_operator_symbol
                        })
                    {
                        return Err(
                            "has a crash-bearing operator use without a checked site".to_owned()
                        );
                    }
                    continue;
                }
                let public_target = requirement_contracts
                    .for_call(
                        state.machine_symbol,
                        state.state_symbol,
                        call.statement_index,
                        call.call_ordinal,
                    )
                    .map(|selected| {
                        (
                            selected.capsule.target_machine(),
                            selected.capsule.target_state(),
                        )
                    });
                let (target_machine, target_state) = public_target
                    .or_else(|| selected_invocation_target(program, site, call.target_symbol))
                    .ok_or_else(|| {
                        format!(
                            "calls `{}`, which selects no declaration",
                            spelled_callee(program, site)
                        )
                    })?;
                sites.push(SummaryInvocationSite {
                    caller_state: state.state_symbol,
                    statement_index: call.statement_index,
                    call_ordinal: call.call_ordinal,
                    target_machine,
                    target_state,

                    runtime_contract: site.runtime_call().map(|binding| binding.contract),
                });
            }
        }
        Ok(sites)
    }

    fn spelled_callee(program: &dyn ::validation::CheckingInput, site: CallSite<'_>) -> String {
        match site {
            CallSite::Statement(call) => call.target.as_str().to_owned(),
            CallSite::Expression { call, .. } => call.target.as_str().to_owned(),
            CallSite::TransitionNamed { path, .. } => {
                program.symbols.display_path(path.symbol, "::")
            }
        }
    }

    /*
        Read the retained call selection rather than recognizing a provider-default
        body or guessing an intrinsic from a diagnostic spelling. Unknown ordinary
        targets remain unresolved dependencies and invalidate their summary.
    */
    fn is_selected_build_operation(
        program: &dyn ::validation::CheckingInput,
        site: CallSite<'_>,
    ) -> bool {
        use language_semantics::declaration_selection::{
            AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionTarget, BuildOperation,
        };

        let selected = match site {
            CallSite::Statement(call) if !call.target_symbol.is_valid() => call
                .authored_call_selection
                .and_then(|occurrence| program.authored_declaration_selections().get(occurrence))
                .filter(|selection| selection.kind() == AuthoredDeclarationSelectionKind::Call)
                .map(|selection| selection.target()),
            CallSite::Expression { expression, call } if !call.target_symbol.is_valid() => {
                let mut selections = expressions(program)
                    .authored_selection_occurrences(expression)
                    .filter_map(|occurrence| {
                        program.authored_declaration_selections().get(occurrence)
                    })
                    .filter(|selection| selection.kind() == AuthoredDeclarationSelectionKind::Call);
                let selected = selections.next().map(|selection| selection.target());
                if selections.next().is_some() {
                    return false;
                }
                selected
            }
            _ => None,
        };
        let Some(AuthoredDeclarationSelectionTarget::Intrinsic(intrinsic)) = selected else {
            return false;
        };
        BuildOperation::ALL
            .iter()
            .any(|operation| operation.intrinsic() == intrinsic)
    }

    /*
        Each physical site has its own source frontier. A guard on one
        statement cannot narrow another operation or a guard's own evaluation.
    */
    fn inferred_direct_body_crash_buckets(
        target: &MachineContractPlan,
        operators: &CheckedOperatorFacts,
        source: Option<&super::entry_paths::MachineEntryConditions<'_>>,
    ) -> Vec<SummaryCrashBucket> {
        let at_site = |location| {
            source.map_or(SummaryCrashRouteGuard::Truth, |paths| {
                paths.at_site(location)
            })
        };
        let mut buckets = target
            .crash
            .checked_sites()
            .iter()
            .map(|site| {
                SummaryCrashBucket::unconditional(site.cause()).on_path(&at_site(site.location()))
            })
            .collect::<Vec<_>>();
        for site in target
            .crash
            .trapping_sites()
            .iter()
            .filter(|site| site.may_trap)
        {
            buckets.push(
                SummaryCrashBucket::unconditional(CrashCause::Trap)
                    .on_path(&at_site(site.location)),
            );
        }
        for operator in target.crash.checked_operators() {
            let origin = if operator.named_use.is_valid() {
                operators.named_uses.get(operator.named_use).origin
            } else {
                operators.uses.get(operator.operator_use).origin
            };
            let path = match origin {
                CheckedValueOrigin::StateStatement {
                    state_symbol,
                    statement_index,
                    ..
                } => at_site(CrashSiteLocation::new(
                    state_symbol,
                    u32::try_from(statement_index).expect("statement ordinal fits u32"),
                )),
                _ => SummaryCrashRouteGuard::Truth,
            };
            buckets.extend(
                summary_published_crash_buckets(&operator.surviving)
                    .into_iter()
                    .map(|bucket| bucket.on_path(&path)),
            );
        }
        normalize_summary_buckets(buckets)
    }

    fn private_dependency_reaches(
        program: &dyn ::validation::CheckingInput,
        nodes: &[SummaryNode],
        node_indices: &SymbolKeyMap<SymbolHandle, usize>,
        plan_indices: &SymbolKeyMap<SymbolHandle, usize>,
        plans: &[MachineContractPlan],
        start: SymbolHandle,
        goal: SymbolHandle,
    ) -> bool {
        let plan_by_machine = |machine: SymbolHandle| {
            plan_indices
                .get(&machine)
                .and_then(|&index| plans.get(index))
        };
        let mut pending = vec![start];
        let mut visited = Vec::new();
        while let Some(machine) = pending.pop() {
            if machine == goal {
                return true;
            }
            if visited.contains(&machine) {
                continue;
            }
            visited.push(machine);
            let Some(&node_index) = node_indices.get(&machine) else {
                continue;
            };
            let node = &nodes[node_index];
            pending.extend(
                node.family_members
                    .iter()
                    .filter(|member| {
                        plan_by_machine(**member).is_some_and(|plan| {
                            program
                                .machine_by_symbol(**member)
                                .is_some_and(|machine| needs_inferred_summary(machine, plan))
                        })
                    })
                    .copied(),
            );
            pending.extend(node.invocations.iter().filter_map(|invocation| {
                plan_by_machine(invocation.target_machine)
                    .filter(|plan| {
                        program
                            .machine_by_symbol(plan.machine)
                            .is_some_and(|machine| needs_inferred_summary(machine, plan))
                    })
                    .map(|_| invocation.target_machine)
            }));
        }
        false
    }

    pub(super) fn needs_inferred_summary(machine: &Machine, plan: &MachineContractPlan) -> bool {
        machine.target_family_interface
            || (machine.supply_mode == MachineSupplyMode::CheckedBody
                && plan.crash.published().is_empty())
    }

    /*
        Missing or unfinished members invalidate the summary rather than supplying
        an empty crash envelope. The family itself has no executable body.
    */
    fn complete_family_members(
        program: &dyn ::validation::CheckingInput,
        interface: &Machine,
        plans: &[MachineContractPlan],
    ) -> Result<Vec<SymbolHandle>, String> {
        let mut families = program
            .target_machine_families
            .iter()
            .filter(|family| family.canonical == interface.symbol);
        let family = families
            .next()
            .ok_or_else(|| "generated interface has no retained family roster".to_owned())?;
        if families.next().is_some() {
            return Err("generated interface has multiple retained family rosters".to_owned());
        }
        if family.members.is_empty() {
            return Err("retained family roster has no members".to_owned());
        }
        let parameters = program
            .machine_states(interface)
            .first()
            .map(|entry| program.state_parameters(entry))
            .ok_or_else(|| "generated interface has no entry telescope".to_owned())?;
        let mut members = Vec::with_capacity(family.members.len());
        for member in &family.members {
            let member_name = program.symbols.display_path(member.machine, "::");
            let machine = program.machine_by_symbol(member.machine).ok_or_else(|| {
                format!("retained member `{member_name}` has no typed declaration")
            })?;
            let entry = program
                .machine_states(machine)
                .first()
                .ok_or_else(|| format!("retained member `{member_name}` has no entry telescope"))?;
            let mut member_plans = plans.iter().filter(|plan| plan.machine == member.machine);
            member_plans.next().ok_or_else(|| {
                format!("retained member `{member_name}` has no owned crash plan")
            })?;
            if member_plans.next().is_some() {
                return Err(format!(
                    "retained member `{member_name}` has multiple owned crash plans"
                ));
            }
            let failure = if machine.target_family_interface {
                Some("is itself a generated interface")
            } else if machine.supply_mode == MachineSupplyMode::CheckedBody
                && !machine.body_is_present
            {
                Some("claims checked supply without a present body")
            } else if !machine.supplies_checked_body()
                && !program.machine_publishes_crash_ceiling(machine)
            {
                Some("has opaque supply without a published crash ceiling")
            } else if machine.body_status != MachineBodyStatus::Complete {
                Some("has an incomplete declaration/body status")
            } else if machine.structural_type_equations_pending {
                Some("has pending structural type equations")
            } else if program.state_parameters(entry).len() != parameters.len() {
                Some("has a different entry parameter count")
            } else if members.contains(&member.machine) {
                Some("occurs twice in the retained roster")
            } else {
                None
            };
            if let Some(failure) = failure {
                return Err(format!("retained member `{member_name}` {failure}"));
            }
            members.push(member.machine);
        }
        Ok(members)
    }

    fn summary_published_crash_buckets(buckets: &[CrashRouteBucket]) -> Vec<SummaryCrashBucket> {
        normalize_summary_buckets(
            buckets
                .iter()
                .map(|bucket| SummaryCrashBucket {
                    cause: bucket.cause(),
                    alternative_guards: bucket
                        .alternative_guards()
                        .iter()
                        .map(|guard| match guard {
                            CrashRouteGuard::Truth => SummaryCrashRouteGuard::Truth,
                            CrashRouteGuard::Predicate(predicate) => predicate.expression().map_or(
                                SummaryCrashRouteGuard::Truth,
                                |identity| {
                                    SummaryCrashRouteGuard::Predicate(SummaryCrashPredicate {
                                        identity: identity.clone(),
                                        builtin_meaning: predicate.scalar_expression().is_some_and(
                                            super::summary_predicates::executable_entry_predicate,
                                        ),
                                        scalar: predicate.scalar_expression().cloned(),
                                    })
                                },
                            ),
                        })
                        .collect(),
                })
                .collect(),
        )
    }

    pub(crate) fn requirement_signature(
        program: &dyn ::validation::CheckingInput,
        target_machine: SymbolHandle,
        target_state: SymbolHandle,
    ) -> Option<&StateSignature> {
        if target_machine == target_state {
            return program
                .machine_parameter_signature(target_state)
                .map(|(_, signature)| signature);
        }
        program
            .trait_definition_by_symbol(target_machine)
            .and_then(|definition| {
                program.trait_machine_signature_by_symbol(definition, target_state)
            })
    }

    pub(crate) fn crash_route_expressions_by_identity(
        program: &dyn ::validation::CheckingInput,
        contracts: &[SignatureContract],
        parameter_names: &[String],
        content_conservation: &[ContentConservationSourcePlan],
    ) -> std::collections::BTreeMap<CrashPredicateIdentity, ExpressionHandle> {
        let mut expressions = std::collections::BTreeMap::new();
        for contract in contracts {
            if !matches!(contract.kind, SignatureContractKind::Crashes { .. }) {
                continue;
            }
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let Some(expression) = fact.as_expression() else {
                    continue;
                };
                if is_true_crash_route(program, fact) {
                    continue;
                }
                let structured = super::crash_predicate_from_expression(
                    program,
                    *expression,
                    parameter_names,
                    Some(content_conservation),
                );
                expressions.insert(
                    CrashPredicateIdentity::from_expression(structured),
                    *expression,
                );
            }
        }
        expressions
    }

    /*
        A sealed quotient request computes by its explicitly selected ordinary
        representative. Keep that dependency in crash inference before runtime
        admission: an unknown or crashing representative must never disappear.
    */
    pub(crate) fn selected_invocation_target(
        program: &dyn ::validation::CheckingInput,
        site: CallSite<'_>,
        ordinary_target: SymbolHandle,
    ) -> Option<(SymbolHandle, SymbolHandle)> {
        let selected = match site {
            CallSite::Expression { call, .. } => call
                .quotient_operation
                .as_ref()
                .map(|request| request.representative_operation.symbol),
            _ => None,
        };
        crate::published::proof::contract_target_from_state_symbol(
            program,
            selected.unwrap_or(ordinary_target),
        )
    }

    #[cfg(test)]
    mod fixed_point_tests {
        /*
            Compare the production scheduler with the literal pre-dirtiness sweeps.
        */
        use super::*;
        use checked_trees::{CheckedBooleanExpression, CrashCause, CrashPredicateExpression};

        fn original_sweeps(
            equations: &[PrivateSummaryEquation<'_>],
        ) -> Vec<(SymbolHandle, Vec<SummaryCrashBucket>)> {
            let mut equation_index_by_machine =
                symbols::SymbolKeyMap::<SymbolHandle, usize>::default();
            let mut resolved = Vec::with_capacity(equations.len());
            for (index, equation) in equations.iter().enumerate() {
                equation_index_by_machine
                    .entry(equation.machine)
                    .or_insert(index);
                resolved.push((equation.machine, equation.direct.clone()));
            }
            for _ in 0..=equations.len() {
                let mut changed = false;
                for (equation_index, equation) in equations.iter().enumerate() {
                    let mut buckets = equation.direct.clone();
                    buckets.extend(equation.published_dependencies.clone());
                    for dependency in &equation.private_dependencies {
                        let &target_index = equation_index_by_machine
                            .get(&dependency.machine)
                            .expect("every private dependency belongs to the viable fixed point");
                        let selected = &resolved[target_index].1;
                        buckets.extend(selected.iter().map(|bucket| {
                            let bucket = if dependency.recursive {
                                SummaryCrashBucket::unconditional(bucket.cause)
                            } else if let Some(substitution) = &dependency.substitution {
                                bucket.substitute(substitution)
                            } else {
                                bucket.clone()
                            };
                            bucket.on_path(&dependency.path_guard)
                        }));
                    }
                    let buckets = normalize_summary_buckets(buckets);
                    let current = &mut resolved[equation_index].1;
                    if *current != buckets {
                        *current = buckets;
                        changed = true;
                    }
                }
                if !changed {
                    break;
                }
            }
            resolved
        }

        fn guarded(cause: CrashCause, parameter: u32, scalar: Option<bool>) -> SummaryCrashBucket {
            SummaryCrashBucket {
                cause,
                alternative_guards: vec![SummaryCrashRouteGuard::Predicate(
                    SummaryCrashPredicate {
                        identity: CrashPredicateExpression::Parameter(parameter),
                        builtin_meaning: true,
                        scalar: scalar.map(CheckedBooleanExpression::Constant),
                    },
                )],
            }
        }

        fn equation(machine: SymbolHandle) -> PrivateSummaryEquation<'static> {
            PrivateSummaryEquation {
                machine,
                direct: Vec::new(),
                private_dependencies: Vec::new(),
                published_dependencies: Vec::new(),
            }
        }

        fn dependency(
            machine: SymbolHandle,
            recursive: bool,
            parameter: Option<u32>,
        ) -> PrivateSummaryDependency<'static> {
            PrivateSummaryDependency {
                machine,
                substitution: None,
                path_guard: parameter.map_or(SummaryCrashRouteGuard::Truth, |parameter| {
                    SummaryCrashRouteGuard::Predicate(SummaryCrashPredicate {
                        identity: CrashPredicateExpression::Parameter(parameter),
                        builtin_meaning: true,
                        scalar: None,
                    })
                }),
                recursive,
            }
        }

        fn compare(equations: &[PrivateSummaryEquation<'_>]) {
            let expected = original_sweeps(equations);
            let actual = solve_private_summary_fixed_point(equations);
            assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
        }

        #[test]
        fn dirty_equations_match_original_sweeps_for_exact_dependency_graphs() {
            let machines = [
                SymbolHandle::from_parts(7, 1),
                SymbolHandle::from_parts(7, 2),
                SymbolHandle::from_parts(9, 1),
            ];
            for edges in 0_u16..(1 << 9) {
                for reverse in [false, true] {
                    let mut equations: Vec<_> = machines.into_iter().map(equation).collect();
                    equations[0].direct = vec![guarded(CrashCause::Trap, 0, Some(true))];
                    equations[1].published_dependencies = vec![guarded(CrashCause::Abort, 1, None)];
                    for (source, equation) in equations.iter_mut().enumerate() {
                        for (target, &machine) in machines.iter().enumerate() {
                            if edges & (1 << (source * 3 + target)) != 0 {
                                let recursive = source == target || (source + target) % 2 == 0;
                                let parameter = (source != target).then_some(source as u32 + 2);
                                equation
                                    .private_dependencies
                                    .push(dependency(machine, recursive, parameter));
                                if equation.private_dependencies.len() == 1 {
                                    equation
                                        .private_dependencies
                                        .push(dependency(machine, recursive, parameter));
                                }
                            }
                        }
                    }
                    if reverse {
                        equations.reverse();
                    }
                    compare(&equations);
                }
            }
            let mut equations: Vec<_> = [machines[0], machines[1], machines[0], machines[2]]
                .into_iter()
                .map(equation)
                .collect();
            equations[0].direct = vec![guarded(CrashCause::Trap, 0, Some(true))];
            equations[1].published_dependencies = vec![guarded(CrashCause::Abort, 0, Some(false))];
            equations[1]
                .private_dependencies
                .push(dependency(machines[0], false, None));
            equations[2].direct = vec![guarded(CrashCause::Trap, 0, Some(false))];
            equations[2]
                .private_dependencies
                .push(dependency(machines[1], false, None));
            equations[3]
                .private_dependencies
                .push(dependency(machines[0], false, None));
            compare(&equations);
            equations.reverse();
            compare(&equations);
            compare(&[]);
        }
    }
}
mod requirement_contracts {
    /*
        A requirement call uses the complete signature retained for that call.
        Calls with different arguments can name the same declaration, so its symbol
        alone cannot recover those arguments or their crash conditions.
    */
    use checked_trees::{CallContractSignature, CrashContractCapsule, ProofFacts};
    use diagnostics::Diagnostic;
    use symbols::SymbolHandle;

    pub(super) struct RequirementCrashContract<'proof> {
        pub(super) source: &'proof CallContractSignature,
        pub(super) capsule: CrashContractCapsule,
    }

    impl RequirementCrashContract<'_> {
        fn location(&self) -> (u32, u32, u32, u32, usize, usize) {
            call_location(
                self.source.caller_machine_symbol,
                self.source.caller_state_symbol,
                self.source.statement_index,
                self.source.call_ordinal,
            )
        }
    }

    pub(super) struct RequirementCrashContracts<'proof> {
        rows: Vec<RequirementCrashContract<'proof>>,
    }

    impl<'proof> RequirementCrashContracts<'proof> {
        pub(super) fn prepare(
            program: &dyn validation::CheckingInput,
            proof: &'proof ProofFacts,
            flow: &checked_trees::FlowFacts,
            operators: &checked_trees::CheckedOperatorFacts,
            content_conservation: &[validation::ContentConservationSourcePlan],
        ) -> Result<Self, Vec<Diagnostic>> {
            let mut applications = Vec::new();
            for (_, call) in proof.contract_calls.iter() {
                if !call.signature.is_valid() {
                    continue;
                }
                if !proof.call_contract_signatures.is_valid(call.signature) {
                    return Err(vec![Diagnostic::error(
                        "a requirement call lost its retained public signature",
                    )]);
                }
                let source = proof.call_contract_signatures.get(call.signature);
                if source.caller_machine_symbol != call.caller_machine_symbol
                    || source.caller_state_symbol != call.caller_state_symbol
                    || source.statement_index != call.statement_index
                    || source.call_ordinal != call.call_ordinal
                {
                    return Err(vec![Diagnostic::error(
                        "a requirement call retained another call's public signature",
                    )]);
                }
                applications.push(call.signature);
            }
            applications.sort_unstable_by_key(|handle| (handle.arena_index(), handle.generation()));
            applications.dedup();
            let mut rows = applications
                .into_iter()
                .map(|application| {
                    let source = proof.call_contract_signatures.get(application);
                    let site = validation::find_call_site(program, source.caller_machine_symbol, source.caller_state_symbol, source.statement_index, source.call_ordinal)
                        .ok_or_else(|| Diagnostic::error("a retained callable crash contract lost its exact source invocation"))?;
                    let selected = validation::call_contract_source(program, source.caller_machine_symbol, &site)
                        .map_err(Diagnostic::error)?.ok_or_else(|| Diagnostic::error("a retained callable crash contract lost its source-owned signature"))?;
                    if selected.signature() != &source.signature || site.target_symbol(program) != source.source_target {
                        return Err(Diagnostic::error("a retained callable crash contract belongs to another invocation signature"));
                    }
                    let (target_machine, target_state, runtime) = match (&source.source, selected) {
                        (checked_trees::CallContractSignatureSource::Requirement { declaring_trait }, validation::CallContractSource::Requirement { declaration, signature })
                            if *declaring_trait == declaration.symbol => (declaration.symbol, signature.symbol, None),
                        (checked_trees::CallContractSignatureSource::Runtime { contract, lexical_owner, callee }, validation::CallContractSource::Runtime { binding, contract: current })
                            if *contract == binding.contract && *callee == binding.callee && *lexical_owner == current.lexical_owner =>
                            (SymbolHandle::invalid(), SymbolHandle::invalid(), Some((*contract, *lexical_owner))),
                        _ => return Err(Diagnostic::error("a retained callable crash capsule lost its exact contract source")),
                    };
                    let mut capsule = super::super::crash_plan_facts::build_signature_crash_capsule(
                        program, target_machine, target_state, &source.signature, content_conservation, operators,
                    )?;
                    if let Some((contract, lexical_owner)) = runtime {
                        capsule = capsule.with_runtime_callable_source(contract, lexical_owner);
                    }
                    Ok(RequirementCrashContract { source, capsule })
                })
                .collect::<Result<Vec<_>, Diagnostic>>()
                .map_err(|diagnostic| vec![diagnostic])?;
            rows.sort_unstable_by_key(RequirementCrashContract::location);
            if rows
                .windows(2)
                .any(|pair| pair[0].location() == pair[1].location())
            {
                return Err(vec![Diagnostic::error(
                    "a source call retained conflicting public requirement contracts",
                )]);
            }
            let prepared = Self { rows };
            for (_, state) in flow.control.states.iter() {
                for call in flow.control.calls.span_or_empty(state.calls) {
                    let Some(site) = validation::find_call_site(
                        program,
                        state.machine_symbol,
                        state.state_symbol,
                        call.statement_index,
                        call.call_ordinal,
                    ) else {
                        if call.runtime_call.is_some() {
                            return Err(vec![Diagnostic::error(
                                "runtime crash call lost its exact typed invocation",
                            )]);
                        }
                        continue;
                    };
                    if site.runtime_call() != call.runtime_call {
                        return Err(vec![Diagnostic::error(
                            "runtime crash call lost its retained callee and contract custody",
                        )]);
                    }
                    let is_requirement = site.runtime_call().is_some()
                        || site.static_requirement_dispatch().is_some()
                        || validation::selected_requirement_signature(
                            program,
                            site.target_symbol(program),
                        )
                        .is_some();
                    if is_requirement
                        && prepared
                            .for_call(
                                state.machine_symbol,
                                state.state_symbol,
                                call.statement_index,
                                call.call_ordinal,
                            )
                            .is_none()
                    {
                        return Err(vec![Diagnostic::error(
                            "a source requirement call has no retained public contract",
                        )]);
                    }
                }
            }
            Ok(prepared)
        }

        pub(super) fn for_call(
            &self,
            machine: SymbolHandle,
            state: SymbolHandle,
            statement: usize,
            call: usize,
        ) -> Option<&RequirementCrashContract<'proof>> {
            self.rows
                .binary_search_by_key(&call_location(machine, state, statement, call), |row| {
                    row.location()
                })
                .ok()
                .map(|position| &self.rows[position])
        }
    }

    fn call_location(
        machine: SymbolHandle,
        state: SymbolHandle,
        statement: usize,
        call: usize,
    ) -> (u32, u32, u32, u32, usize, usize) {
        (
            machine.arena_index(),
            machine.generation(),
            state.arena_index(),
            state.generation(),
            statement,
            call,
        )
    }
}
mod route_substitution {
    /*
        Call argument substitution into published crash routes.
    */
    use crate::flow::active_context_handles;
    use crate::published::facts::crash_calls::private_summaries::crash_route_expressions_by_identity;
    use crate::published::facts::crash_calls::summary_predicates::{
        CallArgumentSubstitution, SummaryCrashBucket, SummaryCrashPredicate,
        SummaryCrashRouteGuard as RouteGuard, concrete_guard_scalar_value,
        normalize_summary_buckets, normalize_summary_guards, scalar_guard_is_integer_comparison,
        scalar_guard_proves_false, summary_boolean_value, summary_closed_float_relation,
    };
    use crate::published::facts::crash_entry_values;
    use crate::published::places::place_at;
    use checked_trees::CrashPredicateExpression;
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::signature::StateParameter;

    /*
        `owner`'s machine and state once `argument` clears the builtin-bound
        meaning gate: checked scalar evidence folds and annotates under builtin
        laws, and an actual whose own operator occurrence selected an authored
        meaning cannot be re-read under semantics its selection rejected.
    */
    fn builtin_bound_owner<'o>(
        program: &dyn ::validation::CheckingInput,
        owner: Option<(
            &'o typed_trees::machine::Machine,
            &'o typed_trees::state::State,
        )>,
        argument: ExpressionHandle,
    ) -> Option<(
        &'o typed_trees::machine::Machine,
        &'o typed_trees::state::State,
    )> {
        let (machine, state) = owner?;
        validation::has_builtin_bound_expression_meaning(program, machine, Some(state), argument)
            .then_some((machine, state))
    }

    pub(crate) enum SelectedTargetCrashRoutes<'a> {
        Published {
            buckets: &'a [checked_trees::CrashRouteBucket],
            contracts: &'a [typed_trees::signature::SignatureContract],
        },
        Private(&'a [SummaryCrashBucket]),
        Empty,
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn call_argument_substitution<'program>(
        program: &'program dyn ::validation::CheckingInput,
        operators: &checked_trees::CheckedOperatorFacts,
        semantic: &facts::FactPlan,
        flow: &'program checked_trees::FlowFacts,
        state_flow: &checked_trees::FlowStateFact,
        call_flow: &checked_trees::FlowCallFact,
        target_parameters: &'program [typed_trees::signature::StateParameter],
        arguments: &[typed_trees::expression::ExpressionHandle],
        exact_integer_casts: &[validation::ExactIntegerCastFact],
    ) -> CallArgumentSubstitution<'program> {
        let caller_state = state_flow.state_symbol;
        let before_statement = call_flow.statement_index;
        let owner = program.machines().iter().find_map(|machine| {
            program
                .state_in_machine(machine, caller_state)
                .map(|state| (machine, state))
        });
        let entry_contexts =
            active_context_handles(&flow.contexts, call_flow.entry_semantic_contexts);
        let containing_statement = owner.and_then(|(_, state)| {
            program
                .statement_table
                .statement_at(state, before_statement)
        });
        let caller_symbols: Vec<SymbolHandle> = owner
            .map(|(_, state)| {
                program
                    .state_parameters(state)
                    .iter()
                    .filter(|parameter| {
                        program
                            .primitive_type_reference(parameter.type_reference)
                            .is_some()
                    })
                    .map(|parameter| parameter.symbol)
                    .chain(
                        crate::flow::state_statements(program, state.statement_nodes)
                            .get(..before_statement)
                            .unwrap_or(&[])
                            .iter()
                            .filter_map(|statement| match statement {
                                typed_trees::statement::StatementNode::LocalData(local)
                                    if !local.is_mutable
                                        && local.initial_value.is_valid()
                                        && program
                                            .primitive_type_reference(local.type_reference)
                                            .is_some() =>
                                {
                                    Some(local.symbol)
                                }
                                _ => None,
                            }),
                    )
                    .collect()
            })
            .unwrap_or_default();
        let mut argument_index = 0usize;
        let mut identity = Vec::with_capacity(target_parameters.len());
        let mut scalar = Vec::with_capacity(target_parameters.len());
        let mut values = Vec::with_capacity(target_parameters.len());
        for parameter in target_parameters {
            let argument = arguments.get(argument_index).copied();
            argument_index = argument_index.saturating_add(1);
            let entry_identity = argument.and_then(|argument| {
                let (machine, state) = owner?;
                crash_entry_values::entry_operand(
                    program,
                    machine.symbol,
                    state.symbol,
                    before_statement,
                    argument,
                )
                .or_else(|| {
                    super::entry_equations::completed_operand(
                        program,
                        operators,
                        semantic,
                        flow,
                        state_flow,
                        before_statement,
                        &entry_contexts,
                        argument,
                    )
                })
                .or_else(|| {
                    crash_entry_values::checked_projected_entry_value(
                        program,
                        operators,
                        semantic,
                        flow,
                        state_flow,
                        before_statement,
                        argument,
                    )
                })
            });
            if let Some(expected) = program.primitive_type_reference(parameter.type_reference) {
                scalar.push(argument.and_then(|argument| {
                    let (_, state) = builtin_bound_owner(program, owner, argument)?;
                    let lowered = crate::values::lower_unit_scalar_argument(
                        program,
                        operators,
                        state,
                        before_statement,
                        argument,
                        expected,
                    )
                    .or_else(|| {
                        crate::values::lower_state_scalar_expression(
                            program,
                            operators,
                            state,
                            before_statement,
                            argument,
                            expected,
                            exact_integer_casts,
                        )
                    })?;
                    scalar_evidence_is_crash_lane_lowerable(&lowered).then_some(lowered)
                }));
                values.push(argument.and_then(|argument| {
                    let folded = (|| {
                        let (machine, state) = builtin_bound_owner(program, owner, argument)?;
                        let lowered = crate::values::lower_unit_scalar_argument(
                            program,
                            operators,
                            state,
                            before_statement,
                            argument,
                            expected,
                        )?;
                        crate::values::evaluate_checked_scalar(
                            &lowered,
                            &mut crate::values::PlaceScalarValues {
                                program,
                                parameters: program.state_parameters(state),
                                symbols: &caller_symbols,
                                value_at_place:
                                    |place: &validation::CanonicalPlace, _observation| {
                                        if containing_statement.is_none_or(|statement| {
                                            crash_entry_values::statement_may_overwrite_place(
                                                program,
                                                machine.symbol,
                                                statement,
                                                place,
                                            )
                                        }) {
                                            return None;
                                        }
                                        crate::values::scalar_value_at_place(
                                            program,
                                            semantic,
                                            entry_contexts
                                                .iter()
                                                .map(|handle| semantic.contexts.get(*handle)),
                                            place,
                                        )
                                        .or_else(|| {
                                            crate::checks::contracts::boolean_value_at_place(
                                                program,
                                                operators,
                                                semantic,
                                                &entry_contexts,
                                                facts::ProgramPoint::Statement {
                                                    machine_symbol: machine.symbol,
                                                    state_symbol: state.symbol,
                                                    statement_index: before_statement,
                                                },
                                                place,
                                            )
                                            .map(facts::ScalarValue::Boolean)
                                        })
                                    },
                            },
                        )
                        .and_then(|value| match value {
                            facts::ScalarValue::Boolean(value) => {
                                Some(CrashPredicateExpression::Boolean(value))
                            }
                            facts::ScalarValue::Integer(value) => {
                                Some(CrashPredicateExpression::Integer(value.to_string()))
                            }
                            facts::ScalarValue::Unknown => None,
                        })
                    })();
                    folded
                        .or_else(|| {
                            if expected != typed_trees::types::PrimitiveType::Bool {
                                return None;
                            }
                            let (machine, state) = owner?;
                            let observed = boolean_view_observation(
                                program,
                                operators,
                                semantic,
                                machine,
                                state,
                                before_statement,
                                &entry_contexts,
                                &caller_symbols,
                                argument,
                                0,
                            );
                            observed.map(CrashPredicateExpression::Boolean)
                        })
                        .or_else(|| {
                            if !expected.accepts_float_literal() {
                                return None;
                            }
                            super::float_observations::FloatObservations::new(
                                program, operators, semantic, flow,
                            )
                            .call_argument(
                                state_flow,
                                call_flow,
                                argument_index - 1,
                                parameter.type_reference,
                            )
                            .map(CrashPredicateExpression::Float)
                        })
                }));
            } else {
                values.push(None);
            }
            identity.push(entry_identity);
        }
        CallArgumentSubstitution {
            identity,
            scalar,
            fields: StructuralArgumentSubstitution {
                program,
                target_parameters,
                flow,
                operands: crate::flow::captured_call_operands(
                    program,
                    flow.view(),
                    state_flow,
                    call_flow,
                ),
                machine: state_flow.machine_symbol,
                state: caller_state,
                before_statement,
            },
            values,
        }
    }

    /*
        Invocation-owned structural actuals. Each predicate leaf authenticates its
        complete projected observation, not the whole actual or a frozen-root flag.
        A sibling write therefore preserves an unaffected field, while an overlapping
        write (including another argument's effects) cannot donate caller-entry facts.
        Copies use their completion epoch; view contents use the final operand's
        prefix, before the receiving call runs. Both canonical keys and annotations
        consume this same retained provenance, not a scan of the enclosing statement.
    */
    #[derive(Clone)]
    pub(crate) struct StructuralArgumentSubstitution<'program> {
        program: &'program dyn ::validation::CheckingInput,
        target_parameters: &'program [StateParameter],
        flow: &'program checked_trees::FlowFacts,
        operands: Option<(&'program [checked_trees::FlowOperandFact], usize)>,
        machine: SymbolHandle,
        state: SymbolHandle,
        before_statement: usize,
    }

    impl StructuralArgumentSubstitution<'_> {
        pub(crate) fn entry_operand(
            &self,
            whole: &[Option<CrashPredicateExpression>],
            ordinal: u32,
            members: &[String],
        ) -> Option<CrashPredicateExpression> {
            let ordinal = ordinal as usize;
            let parameter = self.target_parameters.get(ordinal)?;
            let projection = crash_entry_values::formal_member_place_projection(
                self.program,
                parameter.type_reference,
                members,
            )?;
            let Some(place) = self.observation_place(ordinal, &projection) else {
                return members
                    .is_empty()
                    .then(|| whole.get(ordinal).cloned().flatten())
                    .flatten();
            };
            if members.is_empty()
                && let Some(value) = whole.get(ordinal).cloned().flatten()
            {
                return Some(value);
            }
            let mut expression = crash_entry_values::entry_place_before_statement(
                self.program,
                self.machine,
                self.state,
                self.before_statement,
                &place,
            )?;
            for expected in members.iter().rev() {
                let CrashPredicateExpression::Member { receiver, member } = expression else {
                    return None;
                };
                if member != *expected {
                    return None;
                }
                expression = *receiver;
            }
            Some(expression)
        }

        fn observation_place(
            &self,
            ordinal: usize,
            suffix: &[facts::PlaceSegment],
        ) -> Option<validation::CanonicalPlace> {
            let parameter = self.target_parameters.get(ordinal)?;
            let (operands, receiver_offset) = self.operands?;
            let operand = operands.get(ordinal.checked_add(receiver_offset)?)?;
            let (mut place, epoch) =
                if crate::flow::operand_binds_live_view(self.program, parameter.type_reference) {
                    let [referent] = self
                        .flow
                        .control
                        .operand_referents
                        .span(operand.referents)?
                    else {
                        return None;
                    };
                    (
                        validation::CanonicalPlace {
                            root: referent.root,
                            segments: self
                                .flow
                                .control
                                .operand_referent_segments
                                .span(referent.segments)?
                                .to_vec(),
                        },
                        operands.last()?,
                    )
                } else if crate::flow::operand_binds_stable_copy(
                    self.program,
                    parameter.type_reference,
                ) {
                    (
                        place_at(
                            self.program,
                            operand.state_symbol,
                            operand.statement_index,
                            operand.expression,
                        )?,
                        operand,
                    )
                } else {
                    return None;
                };
            place.segments.extend_from_slice(suffix);
            crate::flow::operand_entry_origin_is_stable(
                self.program,
                self.flow.view(),
                epoch,
                &place,
            )
            .then_some(place)
        }

        fn subject(
            &self,
            leaf: &checked_trees::CheckedStructuralParameterField,
        ) -> Option<checked_trees::CheckedStructuralParameterField> {
            let (_, suffix, _, _) = crate::values::resolve_structural_parameter_path(
                self.program,
                self.target_parameters,
                leaf.parameter_position,
                &leaf.path,
            )?;
            let place = self.observation_place(leaf.parameter_position as usize, &suffix)?;
            let (parameter_position, path) =
                crash_entry_values::entry_parameter_place_before_statement(
                    self.program,
                    self.machine,
                    self.state,
                    self.before_statement,
                    &place,
                )?;
            Some(checked_trees::CheckedStructuralParameterField {
                parameter_position,
                path,
            })
        }
    }

    /*
        A checked `bool` view of a byte already proved to be 0 or 1.

        The view records that scalar observation. A local copy, a call whose
        result is ensured equal to an argument, and a match whose arms agree all
        read the same observation. The view does not become a total integer
        representation view, and a byte outside `{0, 1}` establishes no Boolean.
    */
    fn boolean_view_observation(
        program: &dyn ::validation::CheckingInput,
        operators: &checked_trees::CheckedOperatorFacts,
        semantic: &facts::FactPlan,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        before_statement: usize,
        entry_contexts: &[facts::FactContextHandle],
        caller_symbols: &[SymbolHandle],
        expression: ExpressionHandle,
        depth: u32,
    ) -> Option<bool> {
        if depth >= 8 || !program.expression_table.expression_is_valid(expression) {
            return None;
        }
        match program.expression_table.expression(expression) {
            typed_trees::expression::ExpressionNode::Name(path) => {
                if !program.expression_table.name_path_is_single_member(path) {
                    return None;
                }
                let initial_value = crash_entry_values::local_initial_value_before_statement(
                    program,
                    machine.symbol,
                    state.symbol,
                    before_statement,
                    path.symbol,
                )?;
                boolean_view_observation(
                    program,
                    operators,
                    semantic,
                    machine,
                    state,
                    before_statement,
                    entry_contexts,
                    caller_symbols,
                    initial_value,
                    depth + 1,
                )
            }
            typed_trees::expression::ExpressionNode::Borrow(borrow)
                if borrow.access == language_core::ReferenceAccess::Shared =>
            {
                boolean_view_observation(
                    program,
                    operators,
                    semantic,
                    machine,
                    state,
                    before_statement,
                    entry_contexts,
                    caller_symbols,
                    borrow.target,
                    depth + 1,
                )
            }
            typed_trees::expression::ExpressionNode::Cast(cast) if cast.form.is_recast() => {
                if !recast_views_boolean(program, cast.target_type) {
                    return None;
                }
                let mut source = cast.value;
                if let typed_trees::expression::ExpressionNode::Borrow(borrow) =
                    program.expression_table.expression(source)
                    && borrow.access == language_core::ReferenceAccess::Shared
                {
                    source = borrow.target;
                }
                let source_type = program.primitive_type_reference(
                    validation::expression_result_type_reference(program, machine, state, source)
                        .unwrap_or_default(),
                )?;
                if !source_type.is_fixed_integer() {
                    return None;
                }
                let lowered = crate::values::lower_unit_scalar_argument(
                    program,
                    operators,
                    state,
                    before_statement,
                    source,
                    source_type,
                )?;
                let value = crate::values::evaluate_checked_scalar(
                    &lowered,
                    &mut crate::values::PlaceScalarValues {
                        program,
                        parameters: program.state_parameters(state),
                        symbols: caller_symbols,
                        value_at_place: |place: &validation::CanonicalPlace, _observation| {
                            if let Some(statement) = program
                                .statement_table
                                .statement_at(state, before_statement)
                                && crash_entry_values::statement_may_overwrite_place(
                                    program,
                                    machine.symbol,
                                    statement,
                                    place,
                                )
                            {
                                return None;
                            }
                            crate::values::scalar_value_at_place(
                                program,
                                semantic,
                                entry_contexts
                                    .iter()
                                    .map(|handle| semantic.contexts.get(*handle)),
                                place,
                            )
                        },
                    },
                )?;
                match value {
                    facts::ScalarValue::Integer(value)
                        if value == numerics::bignum::BigInt::from_u64(0) =>
                    {
                        Some(false)
                    }
                    facts::ScalarValue::Integer(value)
                        if value == numerics::bignum::BigInt::from_u64(1) =>
                    {
                        Some(true)
                    }
                    _ => None,
                }
            }
            typed_trees::expression::ExpressionNode::Call(call) => observation_carried_by_call(
                program,
                operators,
                semantic,
                machine,
                state,
                before_statement,
                entry_contexts,
                caller_symbols,
                call,
                depth,
            ),
            typed_trees::expression::ExpressionNode::Match(dispatch) => {
                let mut agreed = None;
                for arm in program.expression_table.match_arms(dispatch.arms) {
                    let observed = boolean_view_observation(
                        program,
                        operators,
                        semantic,
                        machine,
                        state,
                        before_statement,
                        entry_contexts,
                        caller_symbols,
                        arm.value,
                        depth + 1,
                    )?;
                    if agreed.is_some_and(|known| known != observed) {
                        return None;
                    }
                    agreed = Some(observed);
                }
                agreed
            }
            typed_trees::expression::ExpressionNode::Boolean(value) => Some(*value),
            _ => None,
        }
    }

    /*
        The observation a call result carries when the callee ensures that result
        equals one argument. The argument is read in the caller; a missing or
        disagreeing observation leaves the result unresolved.
    */
    fn observation_carried_by_call(
        program: &dyn ::validation::CheckingInput,
        operators: &checked_trees::CheckedOperatorFacts,
        semantic: &facts::FactPlan,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        before_statement: usize,
        entry_contexts: &[facts::FactContextHandle],
        caller_symbols: &[SymbolHandle],
        call: &typed_trees::expression::TableCallExpression,
        depth: u32,
    ) -> Option<bool> {
        let (callee, callee_state) = validation::find_machine_head(program, call.target_symbol)?;
        let parameters = program.state_parameters(callee_state);
        let arguments = program.expression_table.expression_handles(call.arguments);
        let scalar_parameters = parameters
            .iter()
            .filter(|parameter| crate::values::occupies_scalar_position(program, parameter))
            .collect::<Vec<_>>();
        let mut agreed = None;
        for contract in program
            .machine_contracts(callee)
            .iter()
            .chain(program.state_contracts(callee_state))
        {
            if contract.kind != typed_trees::signature::SignatureContractKind::Ensures {
                continue;
            }
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let Some(expression) = fact.as_expression().copied() else {
                    continue;
                };
                let Some(checked) = crate::values::lower_callable_scalar_contract_predicate(
                    program,
                    operators,
                    callee.symbol,
                    parameters,
                    callee_state.return_type,
                    expression,
                    true,
                    &mut 4096,
                ) else {
                    continue;
                };
                let checked_trees::CheckedBooleanExpression::Equal { left, right } = checked else {
                    continue;
                };
                for (result_term, input_term) in [
                    (left.as_ref(), right.as_ref()),
                    (right.as_ref(), left.as_ref()),
                ] {
                    let (
                        checked_trees::CheckedBooleanExpression::Parameter { position: output },
                        checked_trees::CheckedBooleanExpression::Parameter { position: input },
                    ) = (result_term, input_term)
                    else {
                        continue;
                    };
                    if *output != scalar_parameters.len() {
                        continue;
                    }
                    let Some(parameter) = scalar_parameters.get(*input) else {
                        continue;
                    };
                    let Some(position) = parameters
                        .iter()
                        .position(|candidate| candidate.symbol == parameter.symbol)
                    else {
                        continue;
                    };
                    let Some(actual) = arguments.get(position).copied() else {
                        continue;
                    };
                    let Some(observed) = boolean_view_observation(
                        program,
                        operators,
                        semantic,
                        machine,
                        state,
                        before_statement,
                        entry_contexts,
                        caller_symbols,
                        actual,
                        depth + 1,
                    ) else {
                        continue;
                    };
                    if agreed.is_some_and(|known| known != observed) {
                        return None;
                    }
                    agreed = Some(observed);
                }
            }
        }
        agreed
    }

    fn recast_views_boolean(
        program: &dyn ::validation::CheckingInput,
        target: typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        let peeled = program.type_reference_table.peeled_type_reference(target);
        let carrier = match program.type_reference_table.type_reference(peeled) {
            typed_trees::types::TypeReferenceNode::Reference { referee, .. } => *referee,
            _ => peeled,
        };
        program.primitive_type_reference(carrier) == Some(typed_trees::types::PrimitiveType::Bool)
    }

    /*
        A scalar annotation crossing the call boundary may only carry terms a
        crash lane can still lower once substituted. Byte observations —
        `StructuralParameterIndexedRead` and `StructuralSequenceLength` —
        resolve structural storage views neither crash-route lane carries, and the
        contract namespace already accepts them: inserting one would keep the
        guarded route but turn its conservative widening into a downstream
        rejection, so the annotation stays empty instead. `Local`, `StorageRead`,
        `IeeeFloatLiteral` leaves stay admissible — the
        contract namespace refuses them, so they widen rather than pin an
        unsupported term. A cast actual carries no entry identity, so its scalar
        can only discharge a closed-false guard in the missing-identity arm and is
        never kept where the route survives.
    */
    fn scalar_evidence_is_crash_lane_lowerable(
        expression: &checked_trees::CheckedScalarExpression,
    ) -> bool {
        use checked_trees::CheckedScalarExpression;
        match expression {
            CheckedScalarExpression::ConstantApplication { .. }
            | CheckedScalarExpression::StructuralSequenceLength { .. }
            | CheckedScalarExpression::StructuralParameterField {
                root:
                    checked_trees::CheckedStorageRoot::OwnedLocal { .. }
                    | checked_trees::CheckedStorageRoot::ViewLocal { .. },
                ..
            }
            | CheckedScalarExpression::StructuralParameterIndexedRead { .. } => false,
            CheckedScalarExpression::RepresentationView {
                primitive_type,
                operand,
            } => {
                primitive_type.is_fixed_integer()
                    && operand.primitive_type().is_some_and(|source| {
                        source.is_fixed_integer()
                            && source.scalar_byte_size() == primitive_type.scalar_byte_size()
                    })
                    && scalar_evidence_is_crash_lane_lowerable(operand)
            }
            CheckedScalarExpression::IntegerBinary { left, right, .. } => {
                scalar_evidence_is_crash_lane_lowerable(left)
                    && scalar_evidence_is_crash_lane_lowerable(right)
            }
            checked_trees::integer_widen_triple_pattern!(operand, ..) => {
                scalar_evidence_is_crash_lane_lowerable(operand)
            }
            CheckedScalarExpression::Boolean(expression) => {
                boolean_evidence_is_crash_lane_lowerable(expression)
            }
            CheckedScalarExpression::Parameter { .. }
            | CheckedScalarExpression::ErasedParameter { .. }
            | CheckedScalarExpression::Local { .. }
            | CheckedScalarExpression::StorageRead { .. }
            | CheckedScalarExpression::StructuralParameterField {
                root: checked_trees::CheckedStorageRoot::Parameter { .. },
                ..
            }
            | CheckedScalarExpression::IntegerLiteral { .. }
            | CheckedScalarExpression::IeeeFloatLiteral { .. }
            | CheckedScalarExpression::ImplementationSelection { .. } => true,
        }
    }

    fn boolean_evidence_is_crash_lane_lowerable(
        expression: &checked_trees::CheckedBooleanExpression,
    ) -> bool {
        use checked_trees::CheckedBooleanExpression;
        match expression {
            CheckedBooleanExpression::IntegerMathComparison { left, right, .. } => {
                [left, right].into_iter().all(|expression| {
                    expression.terms.iter().all(|(_, term)| match term {
                        checked_trees::CheckedIntegerMathTerm::Embed { operand, .. } => {
                            scalar_evidence_is_crash_lane_lowerable(operand)
                        }
                        _ => true,
                    })
                })
            }

            CheckedBooleanExpression::IntegerComparison { left, right, .. }
            | CheckedBooleanExpression::ScalarIeeeFloatComparison { left, right, .. } => {
                scalar_evidence_is_crash_lane_lowerable(left)
                    && scalar_evidence_is_crash_lane_lowerable(right)
            }
            CheckedBooleanExpression::Not(operand) => {
                boolean_evidence_is_crash_lane_lowerable(operand)
            }
            facts::checked_boolean_triple_pattern!(left, right, ..) => {
                boolean_evidence_is_crash_lane_lowerable(left)
                    && boolean_evidence_is_crash_lane_lowerable(right)
            }
            CheckedBooleanExpression::Constant(_)
            | CheckedBooleanExpression::Parameter { .. }
            | CheckedBooleanExpression::ErasedParameter { .. }
            | CheckedBooleanExpression::Local { .. }
            | CheckedBooleanExpression::StorageRead { .. }
            | CheckedBooleanExpression::StructuralParameterField { .. }
            | CheckedBooleanExpression::IeeeFloatComparison { .. }
            | CheckedBooleanExpression::StructuralIdentity { .. }
            | CheckedBooleanExpression::ByteSequenceEqual { .. }
            | CheckedBooleanExpression::ByteSequencePredicate { .. }
            | CheckedBooleanExpression::StructuralCaseMembership { .. } => true,
        }
    }

    pub(crate) fn substitute_checked_boolean_expression(
        expression: &checked_trees::CheckedBooleanExpression,
        arguments: &[Option<checked_trees::CheckedScalarExpression>],
        fields: &StructuralArgumentSubstitution<'_>,
    ) -> Option<checked_trees::CheckedBooleanExpression> {
        use checked_trees::{CheckedBooleanExpression, CheckedScalarExpression};

        Some(match expression {
            CheckedBooleanExpression::IntegerMathComparison { kind, left, right } => {
                let substitute = |expression: &checked_trees::CheckedIntegerMathExpression| {
                    let mut result = expression.clone();
                    for (handle, term) in expression.terms.iter() {
                        if let checked_trees::CheckedIntegerMathTerm::Embed {
                            primitive_type,
                            operand,
                        } = term
                        {
                            *result.terms.get_mut(handle) =
                                checked_trees::CheckedIntegerMathTerm::Embed {
                                    primitive_type: *primitive_type,
                                    operand: substitute_checked_scalar_expression(
                                        operand, arguments, fields,
                                    )?,
                                };
                        }
                    }
                    Some(result)
                };
                CheckedBooleanExpression::IntegerMathComparison {
                    kind: *kind,
                    left: substitute(left)?,
                    right: substitute(right)?,
                }
            }
            CheckedBooleanExpression::Constant(value) => CheckedBooleanExpression::Constant(*value),
            CheckedBooleanExpression::Parameter { position } => {
                let CheckedScalarExpression::Boolean(expression) =
                    arguments.get(*position)?.as_ref()?.clone()
                else {
                    return None;
                };
                *expression
            }
            CheckedBooleanExpression::ErasedParameter { .. } => return None,
            CheckedBooleanExpression::Local { .. }
            | CheckedBooleanExpression::StorageRead { .. } => {
                return None;
            }
            CheckedBooleanExpression::IeeeFloatComparison {
                kind,
                primitive_type,
                left,
                right,
            } => CheckedBooleanExpression::IeeeFloatComparison {
                kind: *kind,
                primitive_type: *primitive_type,
                left: substitute_structural_parameter_field(left, fields)?,
                right: substitute_structural_parameter_field(right, fields)?,
            },
            CheckedBooleanExpression::StructuralIdentity { left, right } => {
                CheckedBooleanExpression::StructuralIdentity {
                    left: substitute_structural_parameter_field(left, fields)?,
                    right: substitute_structural_parameter_field(right, fields)?,
                }
            }
            CheckedBooleanExpression::ByteSequenceEqual { left, right } => {
                CheckedBooleanExpression::ByteSequenceEqual {
                    left: substitute_structural_parameter_field(&left.parameter_field()?, fields)?
                        .into(),
                    right: substitute_structural_parameter_field(
                        &right.parameter_field()?,
                        fields,
                    )?
                    .into(),
                }
            }
            CheckedBooleanExpression::StructuralCaseMembership { subject, case } => {
                CheckedBooleanExpression::StructuralCaseMembership {
                    subject: substitute_structural_parameter_field(
                        &subject.parameter_field()?,
                        fields,
                    )?
                    .into(),
                    case: case.clone(),
                }
            }
            CheckedBooleanExpression::ByteSequencePredicate {
                predicate,
                root,
                path,
            } => {
                let checked_trees::CheckedStorageRoot::Parameter { index } = root else {
                    return None;
                };
                let leaf = substitute_structural_parameter_field(
                    &checked_trees::CheckedStructuralParameterField {
                        parameter_position: *index,
                        path: path.clone(),
                    },
                    fields,
                )?;
                CheckedBooleanExpression::ByteSequencePredicate {
                    predicate: *predicate,
                    root: checked_trees::CheckedStorageRoot::Parameter {
                        index: leaf.parameter_position,
                    },
                    path: leaf.path,
                }
            }
            CheckedBooleanExpression::StructuralParameterField {
                parameter_position,
                path,
                ..
            } => {
                let leaf = substitute_structural_parameter_field(
                    &checked_trees::CheckedStructuralParameterField {
                        parameter_position: *parameter_position,
                        path: path.clone(),
                    },
                    fields,
                )?;
                CheckedBooleanExpression::StructuralParameterField {
                    observation: Default::default(),
                    parameter_position: leaf.parameter_position,
                    path: leaf.path,
                }
            }
            CheckedBooleanExpression::Not(operand) => CheckedBooleanExpression::Not(Box::new(
                substitute_checked_boolean_expression(operand, arguments, fields)?,
            )),
            CheckedBooleanExpression::Equal { left, right } => CheckedBooleanExpression::Equal {
                left: Box::new(substitute_checked_boolean_expression(
                    left, arguments, fields,
                )?),
                right: Box::new(substitute_checked_boolean_expression(
                    right, arguments, fields,
                )?),
            },
            CheckedBooleanExpression::IntegerComparison { kind, left, right } => {
                CheckedBooleanExpression::IntegerComparison {
                    kind: *kind,
                    left: Box::new(substitute_checked_scalar_expression(
                        left, arguments, fields,
                    )?),
                    right: Box::new(substitute_checked_scalar_expression(
                        right, arguments, fields,
                    )?),
                }
            }
            CheckedBooleanExpression::ScalarIeeeFloatComparison { kind, left, right } => {
                CheckedBooleanExpression::ScalarIeeeFloatComparison {
                    kind: *kind,
                    left: Box::new(substitute_checked_scalar_expression(
                        left, arguments, fields,
                    )?),
                    right: Box::new(substitute_checked_scalar_expression(
                        right, arguments, fields,
                    )?),
                }
            }
            CheckedBooleanExpression::And { left, right } => CheckedBooleanExpression::And {
                left: Box::new(substitute_checked_boolean_expression(
                    left, arguments, fields,
                )?),
                right: Box::new(substitute_checked_boolean_expression(
                    right, arguments, fields,
                )?),
            },
            CheckedBooleanExpression::Or { left, right } => CheckedBooleanExpression::Or {
                left: Box::new(substitute_checked_boolean_expression(
                    left, arguments, fields,
                )?),
                right: Box::new(substitute_checked_boolean_expression(
                    right, arguments, fields,
                )?),
            },
        })
    }

    /*
        Re-root one structural leaf through the call: the callee parameter
        position binds the actual's caller root (`parameter_position` plus the
        actual's own member spine) and the leaf's authored path extends below it.
        `None` where the actual's root stayed unproven — the guard then loses its
        checked scalar form rather than naming storage the entry snapshot cannot
        describe.
    */
    fn substitute_structural_parameter_field(
        leaf: &checked_trees::CheckedStructuralParameterField,
        fields: &StructuralArgumentSubstitution<'_>,
    ) -> Option<checked_trees::CheckedStructuralParameterField> {
        fields.subject(leaf)
    }

    fn substitute_checked_scalar_expression(
        expression: &checked_trees::CheckedScalarExpression,
        arguments: &[Option<checked_trees::CheckedScalarExpression>],
        fields: &StructuralArgumentSubstitution<'_>,
    ) -> Option<checked_trees::CheckedScalarExpression> {
        use checked_trees::CheckedScalarExpression;

        Some(match expression {
            CheckedScalarExpression::ConstantApplication { .. } => expression.clone(),
            CheckedScalarExpression::Parameter {
                position,
                primitive_type,
            } => {
                let substituted = arguments.get(*position)?.as_ref()?.clone();
                (crate::values::scalar_expression_type(&substituted) == Some(*primitive_type))
                    .then_some(substituted)?
            }
            CheckedScalarExpression::ErasedParameter { .. } => return None,
            CheckedScalarExpression::Local { .. }
            | CheckedScalarExpression::StorageRead { .. }
            | CheckedScalarExpression::StructuralParameterField {
                root:
                    checked_trees::CheckedStorageRoot::OwnedLocal { .. }
                    | checked_trees::CheckedStorageRoot::ViewLocal { .. },
                ..
            }
            | CheckedScalarExpression::StructuralSequenceLength { .. }
            | CheckedScalarExpression::StructuralParameterIndexedRead { .. } => return None,
            CheckedScalarExpression::RepresentationView {
                primitive_type,
                operand,
            } => CheckedScalarExpression::RepresentationView {
                primitive_type: *primitive_type,
                operand: Box::new(substitute_checked_scalar_expression(
                    operand, arguments, fields,
                )?),
            },
            CheckedScalarExpression::StructuralParameterField {
                root:
                    checked_trees::CheckedStorageRoot::Parameter {
                        index: parameter_position,
                    },
                path,
                primitive_type,
                ..
            } => {
                let leaf = substitute_structural_parameter_field(
                    &checked_trees::CheckedStructuralParameterField {
                        parameter_position: *parameter_position,
                        path: path.clone(),
                    },
                    fields,
                )?;
                CheckedScalarExpression::StructuralParameterField {
                    observation: Default::default(),
                    root: checked_trees::CheckedStorageRoot::Parameter {
                        index: leaf.parameter_position,
                    },
                    path: leaf.path,
                    primitive_type: *primitive_type,
                }
            }
            CheckedScalarExpression::IntegerLiteral { literal } => {
                CheckedScalarExpression::IntegerLiteral {
                    literal: literal.clone(),
                }
            }
            CheckedScalarExpression::IeeeFloatLiteral { value } => {
                CheckedScalarExpression::IeeeFloatLiteral { value: *value }
            }
            CheckedScalarExpression::ImplementationSelection { .. } => expression.clone(),
            CheckedScalarExpression::IntegerBinary {
                kind,
                primitive_type,
                left,
                right,
                ..
            } => CheckedScalarExpression::IntegerBinary {
                kind: *kind,
                primitive_type: *primitive_type,
                left: Box::new(substitute_checked_scalar_expression(
                    left, arguments, fields,
                )?),
                right: Box::new(substitute_checked_scalar_expression(
                    right, arguments, fields,
                )?),
                formation_decided: false,
                trap_free: false,
            },
            CheckedScalarExpression::IntegerBitwiseNot {
                primitive_type,
                operand,
            } => CheckedScalarExpression::IntegerBitwiseNot {
                primitive_type: *primitive_type,
                operand: Box::new(substitute_checked_scalar_expression(
                    operand, arguments, fields,
                )?),
            },
            CheckedScalarExpression::IntegerWiden {
                primitive_type,
                operand,
            } => CheckedScalarExpression::IntegerWiden {
                primitive_type: *primitive_type,
                operand: Box::new(substitute_checked_scalar_expression(
                    operand, arguments, fields,
                )?),
            },
            CheckedScalarExpression::IntegerExactCast {
                primitive_type,
                operand,
                range,
                ..
            } => CheckedScalarExpression::IntegerExactCast {
                primitive_type: *primitive_type,
                operand: Box::new(substitute_checked_scalar_expression(
                    operand, arguments, fields,
                )?),
                range: range.clone(),
                formation_decided: false,
            },
            CheckedScalarExpression::Boolean(expression) => {
                CheckedScalarExpression::Boolean(Box::new(substitute_checked_boolean_expression(
                    expression, arguments, fields,
                )?))
            }
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn refine_published_crash_routes(
        program: &dyn ::validation::CheckingInput,
        operators: &checked_trees::CheckedOperatorFacts,
        exact_integer_casts: &[validation::ExactIntegerCastFact],
        semantic: &facts::FactPlan,
        flow: &checked_trees::FlowFacts,
        state_flow: &checked_trees::FlowStateFact,
        call_flow: &checked_trees::FlowCallFact,
        call_site: &validation::CallSite<'_>,
        target_state_symbol: SymbolHandle,
        target_parameters: &[typed_trees::signature::StateParameter],
        target_parameter_names: &[String],
        buckets: &[checked_trees::CrashRouteBucket],
        contracts: &[typed_trees::signature::SignatureContract],
        content_conservation: &[validation::ContentConservationSourcePlan],
    ) -> Vec<SummaryCrashBucket> {
        let route_expressions = crash_route_expressions_by_identity(
            program,
            contracts,
            target_parameter_names,
            content_conservation,
        );
        let arguments = crate::shared::call_formals::arguments_aligned_to_formals(
            program,
            call_flow.target_symbol,
            target_parameters.len(),
            validation::call_site_argument_expressions(program, call_site),
        );
        let substitution = call_argument_substitution(
            program,
            operators,
            semantic,
            flow,
            state_flow,
            call_flow,
            target_parameters,
            arguments,
            exact_integer_casts,
        );
        let mut intervals = crate::values::CapturedCallIntervals::new(
            program,
            operators,
            semantic,
            flow,
            state_flow,
            call_flow,
            target_parameters,
        );
        let mut surviving = Vec::new();
        let mut ordinary_arguments = arguments.iter().copied();
        let operands = target_parameters
            .iter()
            .map(|_parameter| ordinary_arguments.next().unwrap_or_default())
            .collect::<Vec<_>>();
        for bucket in buckets {
            let mut guards = Vec::new();
            for guard in bucket.alternative_guards() {
                match guard {
                    checked_trees::CrashRouteGuard::Truth => {
                        guards.push(RouteGuard::Truth);
                    }
                    checked_trees::CrashRouteGuard::Predicate(identity) => {
                        let expression = *route_expressions.get(identity).expect(
                            "a canonical published crash route retains its typed producer expression",
                        );
                        let builtin_meaning = identity
                            .scalar_expression()
                            .is_some_and(super::summary_predicates::executable_entry_predicate)
                            || !identity.expression().is_some_and(
                                CrashPredicateExpression::contains_structural_identity,
                            ) && program
                                .machines()
                                .iter()
                                .find_map(|machine| {
                                    program.state_in_machine(machine, target_state_symbol).map(
                                        |state| {
                                            validation::has_builtin_bound_expression_meaning(
                                                program,
                                                machine,
                                                Some(state),
                                                expression,
                                            )
                                        },
                                    )
                                })
                                .unwrap_or(false);
                        if builtin_meaning
                            && identity.scalar_expression().is_some_and(|guard| {
                                intervals.as_mut().is_some_and(|intervals| {
                                    validation::selected_boolean_decision(guard, intervals)
                                        == Some(false)
                                })
                            })
                        {
                            continue;
                        }
                        if builtin_meaning
                            && crate::published::facts::operator_crashes::statement_entry_route_is_false(
                                program,
                                flow,
                                semantic,
                                state_flow.machine_symbol,
                                state_flow.state_symbol,
                                call_flow.statement_index,
                                target_parameters,
                                &operands,
                                expression,
                            )
                        {
                            continue;
                        }
                        let Some(predicate) = identity.expression() else {
                            guards.push(RouteGuard::Truth);
                            continue;
                        };
                        let entry_predicate = crash_entry_values::substitute_entry_projected(
                            predicate,
                            &mut |ordinal, members| {
                                substitution.fields.entry_operand(
                                    &substitution.identity,
                                    ordinal,
                                    members,
                                )
                            },
                        );
                        let concrete_value = (builtin_meaning && entry_predicate.is_some())
                            .then(|| {
                                crate::checks::contracts::call_site_boolean_contract_expression_value(
                                    program,
                                    None,
                                    state_flow,
                                    call_flow,
                                    call_site,
                                    target_state_symbol,
                                    target_parameters,
                                    expression,
                                )
                            })
                            .flatten();
                        match concrete_value {
                            Some(false) => {}
                            Some(true) => guards.push(RouteGuard::Truth),
                            None => {
                                if let Some(value) = crash_entry_values::substitute_entry(
                                    predicate,
                                    &substitution.values,
                                )
                                .and_then(|substituted| {
                                    if builtin_meaning {
                                        summary_boolean_value(&substituted)
                                    } else {
                                        substituted
                                            .boolean_value()
                                            .or_else(|| summary_closed_float_relation(&substituted))
                                    }
                                }) {
                                    if value {
                                        guards.push(RouteGuard::Truth);
                                    }
                                    continue;
                                }
                                let scalar = identity.scalar_expression().and_then(|scalar| {
                                    substitute_checked_boolean_expression(
                                        scalar,
                                        &substitution.scalar,
                                        &substitution.fields,
                                    )
                                });
                                let Some(predicate) = entry_predicate else {
                                    if !scalar_guard_proves_false(builtin_meaning, scalar.as_ref())
                                    {
                                        guards.push(RouteGuard::Truth);
                                    }
                                    continue;
                                };
                                let folded = if builtin_meaning {
                                    summary_boolean_value(&predicate)
                                } else {
                                    predicate.boolean_value()
                                };
                                let value = folded.or_else(|| {
                                    (builtin_meaning
                                        && scalar
                                            .as_ref()
                                            .is_some_and(scalar_guard_is_integer_comparison))
                                    .then(|| scalar.as_ref().and_then(concrete_guard_scalar_value))
                                    .flatten()
                                });
                                match value {
                                    Some(false) => {}
                                    Some(true) => guards.push(RouteGuard::Truth),
                                    None => {
                                        guards.push(RouteGuard::Predicate(SummaryCrashPredicate {
                                            identity: predicate,
                                            builtin_meaning,
                                            scalar,
                                        }))
                                    }
                                }
                            }
                        }
                    }
                }
            }
            normalize_summary_guards(&mut guards);
            if !guards.is_empty() {
                surviving.push(SummaryCrashBucket {
                    cause: bucket.cause(),
                    alternative_guards: guards,
                });
            }
        }
        normalize_summary_buckets(surviving)
    }
}
mod summary_predicates {
    /*
        Summary crash predicates, guards, buckets and their normalization.
    */
    use crate::published::facts::crash_calls::route_substitution::substitute_checked_boolean_expression;
    use checked_trees::CrashPredicateExpression;

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub(crate) enum SummaryCrashRouteGuard {
        Truth,
        Predicate(SummaryCrashPredicate),
    }

    impl SummaryCrashRouteGuard {
        fn conjunction(&self, other: &Self) -> Self {
            let (left, right) = match (self, other) {
                (Self::Truth, other) | (other, Self::Truth) => return other.clone(),
                (Self::Predicate(left), Self::Predicate(right)) => (left, right),
            };
            if !left.builtin_meaning || !right.builtin_meaning {
                let retained = match (left.builtin_meaning, right.builtin_meaning) {
                    (true, false) => left,
                    (false, true) => right,
                    (false, false) => std::cmp::min(left, right),
                    (true, true) => unreachable!("both builtin meanings passed the gate"),
                };
                return Self::Predicate(retained.clone());
            }
            if left == right {
                let mut result = left.clone();
                if result.scalar.is_none() {
                    result.scalar.clone_from(&right.scalar);
                }
                return Self::Predicate(result);
            }
            let mut pending = vec![&left.identity, &right.identity];
            let mut conjuncts = Vec::new();
            while let Some(identity) = pending.pop() {
                if let CrashPredicateExpression::Binary {
                    operator,
                    left,
                    right,
                } = identity
                    && *operator == typed_trees::expression::BinaryOperator::And as u8
                {
                    pending.extend([left.as_ref(), right.as_ref()]);
                } else {
                    conjuncts.push(identity.clone());
                }
            }
            conjuncts.sort();
            conjuncts.dedup();
            let mut conjuncts = conjuncts.into_iter();
            let first = conjuncts.next().expect("two predicates supply a conjunct");
            let identity = conjuncts.fold(first, |left, right| CrashPredicateExpression::Binary {
                operator: typed_trees::expression::BinaryOperator::And as u8,
                left: Box::new(left),
                right: Box::new(right),
            });
            let scalar = left
                .scalar
                .as_ref()
                .zip(right.scalar.as_ref())
                .map(
                    |(left, right)| checked_trees::CheckedBooleanExpression::And {
                        left: Box::new(left.clone()),
                        right: Box::new(right.clone()),
                    },
                );
            Self::Predicate(SummaryCrashPredicate {
                identity,
                builtin_meaning: left.builtin_meaning && right.builtin_meaning,
                scalar,
            })
        }
    }

    #[derive(Debug, Clone)]
    pub(crate) struct SummaryCrashPredicate {
        pub(crate) identity: CrashPredicateExpression,
        pub(crate) builtin_meaning: bool,
        pub(crate) scalar: Option<checked_trees::CheckedBooleanExpression>,
    }

    impl PartialEq for SummaryCrashPredicate {
        fn eq(&self, other: &Self) -> bool {
            self.identity == other.identity && self.builtin_meaning == other.builtin_meaning
        }
    }

    impl Eq for SummaryCrashPredicate {}

    impl PartialOrd for SummaryCrashPredicate {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }

    impl Ord for SummaryCrashPredicate {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            self.identity
                .cmp(&other.identity)
                .then_with(|| self.builtin_meaning.cmp(&other.builtin_meaning))
        }
    }

    #[derive(Clone)]
    pub(crate) struct CallArgumentSubstitution<'program> {
        pub(crate) identity: Vec<Option<CrashPredicateExpression>>,
        pub(crate) scalar: Vec<Option<checked_trees::CheckedScalarExpression>>,
        /*
            Complete projected subjects are authenticated at the invocation's
            write boundary, including when an actual is mutable or local-bound.
        */
        pub(crate) fields: super::route_substitution::StructuralArgumentSubstitution<'program>,
        /*
            One literal operand per target parameter: the exact value this call's
            own entry contexts prove for its actual, `None` where the flow proves
            no single value. These decide a retained guard under its own fold
            rules; they never become caller entry identity and never re-read a
            mutable initializer.
        */
        pub(crate) values: Vec<Option<CrashPredicateExpression>>,
    }

    /*
        Reduce only closed proof-literal comparisons after entry substitution.
        The predicate identity is domain-free: arithmetic operands stay opaque
        here so no caller-selected meaning is silently replaced by builtin laws.
    */
    pub(crate) fn summary_boolean_value(expression: &CrashPredicateExpression) -> Option<bool> {
        use typed_trees::expression::{BinaryOperator, UnaryOperator};

        match expression {
            CrashPredicateExpression::Boolean(value) => Some(*value),
            CrashPredicateExpression::Unary { operator, operand }
                if *operator == UnaryOperator::LogicalNot as u8 =>
            {
                Some(!summary_boolean_value(operand)?)
            }
            CrashPredicateExpression::Binary {
                operator,
                left,
                right,
            } => {
                if *operator == BinaryOperator::And as u8 {
                    return Some(summary_boolean_value(left)? && summary_boolean_value(right)?);
                }
                if *operator == BinaryOperator::Or as u8 {
                    return Some(summary_boolean_value(left)? || summary_boolean_value(right)?);
                }
                if *operator == BinaryOperator::Equal as u8
                    || *operator == BinaryOperator::NotEqual as u8
                {
                    if let (
                        CrashPredicateExpression::Boolean(left_value),
                        CrashPredicateExpression::Boolean(right_value),
                    ) = (left.as_ref(), right.as_ref())
                    {
                        let equal = left_value == right_value;
                        return Some(if *operator == BinaryOperator::Equal as u8 {
                            equal
                        } else {
                            !equal
                        });
                    }
                    if let (
                        CrashPredicateExpression::Float(left_text),
                        CrashPredicateExpression::Float(right_text),
                    ) = (left.as_ref(), right.as_ref())
                    {
                        let equal = ieee_literal_equal(left_text, right_text)?;
                        return Some(if *operator == BinaryOperator::Equal as u8 {
                            equal
                        } else {
                            !equal
                        });
                    }
                }
                let left = summary_integer_literal(left)?;
                let right = summary_integer_literal(right)?;
                match *operator {
                    operator if operator == BinaryOperator::Equal as u8 => Some(left == right),
                    operator if operator == BinaryOperator::NotEqual as u8 => Some(left != right),
                    operator if operator == BinaryOperator::Less as u8 => Some(left < right),
                    operator if operator == BinaryOperator::LessOrEqual as u8 => {
                        Some(left <= right)
                    }
                    operator if operator == BinaryOperator::Greater as u8 => Some(left > right),
                    operator if operator == BinaryOperator::GreaterOrEqual as u8 => {
                        Some(left >= right)
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    pub(crate) fn summary_closed_float_relation(
        expression: &CrashPredicateExpression,
    ) -> Option<bool> {
        use typed_trees::expression::{BinaryOperator, UnaryOperator};

        match expression {
            CrashPredicateExpression::Boolean(value) => Some(*value),
            CrashPredicateExpression::Unary { operator, operand }
                if *operator == UnaryOperator::LogicalNot as u8 =>
            {
                Some(!summary_closed_float_relation(operand)?)
            }
            CrashPredicateExpression::Binary {
                operator,
                left,
                right,
            } if *operator == BinaryOperator::And as u8 => {
                Some(summary_closed_float_relation(left)? && summary_closed_float_relation(right)?)
            }
            CrashPredicateExpression::Binary {
                operator,
                left,
                right,
            } if *operator == BinaryOperator::Or as u8 => {
                Some(summary_closed_float_relation(left)? || summary_closed_float_relation(right)?)
            }
            CrashPredicateExpression::Binary {
                operator,
                left,
                right,
            } if *operator == BinaryOperator::Equal as u8
                || *operator == BinaryOperator::NotEqual as u8 =>
            {
                let (
                    CrashPredicateExpression::Float(left_text),
                    CrashPredicateExpression::Float(right_text),
                ) = (left.as_ref(), right.as_ref())
                else {
                    return None;
                };
                let equal = ieee_literal_equal(left_text, right_text)?;
                Some(if *operator == BinaryOperator::Equal as u8 {
                    equal
                } else {
                    !equal
                })
            }
            _ => None,
        }
    }

    fn ieee_literal_equal(left: &str, right: &str) -> Option<bool> {
        let left = numerics::literals::FloatLiteral::parse(left)?;
        let right = numerics::literals::FloatLiteral::parse(right)?;
        Some(left.value_f64() == right.value_f64())
    }

    fn summary_integer_literal(
        expression: &CrashPredicateExpression,
    ) -> Option<numerics::bignum::BigInt> {
        if let CrashPredicateExpression::IntegerWiden { operand, .. } = expression {
            return summary_integer_literal(operand);
        }
        if let CrashPredicateExpression::RepresentationView {
            source_type,
            target_type,
            operand,
        } = expression
        {
            let source = *source_type;
            let target = *target_type;
            if source.bits() != target.bits() {
                return None;
            }
            let value = summary_integer_literal(operand)?;
            let value = crate::values::admitted_integer(source, &value)?;
            let viewed = crate::values::representation_view_value(target, value)?;
            return Some(crate::values::integer_magnitude(viewed));
        }
        let CrashPredicateExpression::Integer(text) = expression else {
            return None;
        };
        let (negative, unsigned) = text
            .strip_prefix('-')
            .map_or((false, text.as_str()), |digits| (true, digits));
        let (base, digits) = if let Some(digits) = unsigned.strip_prefix("0x") {
            (16, digits)
        } else if let Some(digits) = unsigned.strip_prefix("0o") {
            (8, digits)
        } else if let Some(digits) = unsigned.strip_prefix("0b") {
            (2, digits)
        } else {
            (10, unsigned)
        };
        if digits.is_empty() || !digits.chars().all(|character| character.is_digit(base)) {
            return None;
        }
        let magnitude = numerics::bignum::BigInt::from_str_radix(digits, base)?;
        Some(if negative {
            magnitude.negate()
        } else {
            magnitude
        })
    }

    /*
        The checked scalar evidence for an arithmetic guard is a (possibly negated)
        integer comparison. It is the only annotation form the domain-free identity
        cannot fold itself, so it is the only form consulted for discharge.
    */
    pub(crate) fn scalar_guard_is_integer_comparison(
        expression: &checked_trees::CheckedBooleanExpression,
    ) -> bool {
        use checked_trees::CheckedBooleanExpression;
        match expression {
            CheckedBooleanExpression::IntegerComparison { .. } => true,
            CheckedBooleanExpression::Not(operand) => {
                matches!(
                    &**operand,
                    CheckedBooleanExpression::IntegerComparison { .. }
                )
            }
            _ => false,
        }
    }

    /*
        Evaluate a fully substituted checked scalar guard. `Some` is decided only
        from closed concrete operands under the selected domains; `None` leaves
        the route conservative and never establishes or erases a cause by itself.
    */
    pub(crate) fn concrete_guard_scalar_value(
        expression: &checked_trees::CheckedBooleanExpression,
    ) -> Option<bool> {
        match crate::values::evaluate_checked_scalar(
            &checked_trees::CheckedScalarExpression::Boolean(Box::new(expression.clone())),
            &mut |_| None,
        )? {
            facts::ScalarValue::Boolean(value) => Some(value),
            _ => None,
        }
    }

    /*
        Whether the substituted checked scalar annotation alone proves the guard
        false — the last discharge channel when entry custody cannot produce a
        caller-namespace predicate at all. The annotation is retained
        independently of entry custody, so a closed `false` still kills the
        alternative rather than widening its route to `Truth`; a true or
        undecided fold cannot narrow a missing identity and keeps the ceiling.
        The gate matches the identity path's: the route's own builtin meaning
        plus a (possibly negated) integer comparison, the one annotation shape
        the domain-free identity cannot fold itself.
    */
    pub(crate) fn scalar_guard_proves_false(
        builtin_meaning: bool,
        scalar: Option<&checked_trees::CheckedBooleanExpression>,
    ) -> bool {
        builtin_meaning
            && scalar.is_some_and(scalar_guard_is_integer_comparison)
            && scalar.and_then(concrete_guard_scalar_value) == Some(false)
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub(crate) struct SummaryCrashBucket {
        pub(crate) cause: checked_trees::CrashCause,
        pub(crate) alternative_guards: Vec<SummaryCrashRouteGuard>,
    }

    impl SummaryCrashBucket {
        pub(crate) fn unconditional(cause: checked_trees::CrashCause) -> Self {
            Self {
                cause,
                alternative_guards: vec![SummaryCrashRouteGuard::Truth],
            }
        }

        /*
            Restrict every possible route by this invocation's own stable path.
            Recursive widening happens before this operation, so it cannot erase
            a local guard that all executions of that dependency must observe.
        */
        pub(crate) fn on_path(mut self, path: &SummaryCrashRouteGuard) -> Self {
            self.alternative_guards = self
                .alternative_guards
                .iter()
                .map(|route| route.conjunction(path))
                .filter(|route| match route {
                    SummaryCrashRouteGuard::Truth => true,
                    SummaryCrashRouteGuard::Predicate(predicate) => {
                        let value = if predicate.builtin_meaning {
                            summary_boolean_value(&predicate.identity)
                        } else {
                            predicate.identity.boolean_value()
                        };
                        value != Some(false)
                    }
                })
                .collect();
            normalize_summary_guards(&mut self.alternative_guards);
            self
        }

        /*
            Transport each referenced projection through the retained invocation.
            Private fixed-point propagation uses the same custody boundary as a
            direct call: a sibling write does not erase an untouched field, and an
            annotation cannot manufacture a missing canonical identity.
        */
        pub(crate) fn substitute(&self, arguments: &CallArgumentSubstitution<'_>) -> Self {
            let mut guards = self
                .alternative_guards
                .iter()
                .filter_map(|guard| match guard {
                    SummaryCrashRouteGuard::Truth => Some(SummaryCrashRouteGuard::Truth),
                    SummaryCrashRouteGuard::Predicate(predicate) => {
                        if let Some(value) =
                            crate::published::facts::crash_entry_values::substitute_entry(
                                &predicate.identity,
                                &arguments.values,
                            )
                            .and_then(|substituted| {
                                if predicate.builtin_meaning {
                                    summary_boolean_value(&substituted)
                                } else {
                                    substituted.boolean_value()
                                }
                            })
                        {
                            return value.then_some(SummaryCrashRouteGuard::Truth);
                        }
                        let identity =
                            crate::published::facts::crash_entry_values::substitute_entry_projected(
                                &predicate.identity,
                                &mut |ordinal, members| {
                                    arguments.fields.entry_operand(
                                        &arguments.identity,
                                        ordinal,
                                        members,
                                    )
                                },
                            );
                        let scalar = predicate.scalar.as_ref().and_then(|scalar| {
                            substitute_checked_boolean_expression(
                                scalar,
                                &arguments.scalar,
                                &arguments.fields,
                            )
                        });
                        let Some(identity) = identity else {
                            return (!scalar_guard_proves_false(
                                predicate.builtin_meaning,
                                scalar.as_ref(),
                            ))
                            .then_some(SummaryCrashRouteGuard::Truth);
                        };
                        let folded = if predicate.builtin_meaning {
                            summary_boolean_value(&identity)
                        } else {
                            identity.boolean_value()
                        };
                        let value = folded.or_else(|| {
                            (predicate.builtin_meaning
                                && scalar
                                    .as_ref()
                                    .is_some_and(scalar_guard_is_integer_comparison))
                            .then(|| scalar.as_ref().and_then(concrete_guard_scalar_value))
                            .flatten()
                        });
                        match value {
                            Some(false) => None,
                            Some(true) => Some(SummaryCrashRouteGuard::Truth),
                            None => {
                                Some(SummaryCrashRouteGuard::Predicate(SummaryCrashPredicate {
                                    identity,
                                    builtin_meaning: predicate.builtin_meaning,
                                    scalar,
                                }))
                            }
                        }
                    }
                })
                .collect::<Vec<_>>();
            normalize_summary_guards(&mut guards);
            Self {
                cause: self.cause,
                alternative_guards: guards,
            }
        }

        pub(crate) fn into_checked(self) -> Option<checked_trees::CrashRouteBucket> {
            checked_trees::CrashRouteBucket::new(
                self.cause,
                self.alternative_guards
                    .into_iter()
                    .map(|guard| match guard {
                        SummaryCrashRouteGuard::Truth => checked_trees::CrashRouteGuard::Truth,
                        SummaryCrashRouteGuard::Predicate(predicate) => {
                            let identity = if let Some(scalar) = predicate.scalar {
                                checked_trees::CrashPredicateIdentity::from_expression_and_scalar(
                                    predicate.identity,
                                    scalar,
                                )
                            } else {
                                checked_trees::CrashPredicateIdentity::from_expression(
                                    predicate.identity,
                                )
                            };
                            checked_trees::CrashRouteGuard::Predicate(identity)
                        }
                    })
                    .collect(),
            )
        }
    }

    pub(crate) fn normalize_summary_guards(guards: &mut Vec<SummaryCrashRouteGuard>) {
        if guards.contains(&SummaryCrashRouteGuard::Truth) {
            guards.clear();
            guards.push(SummaryCrashRouteGuard::Truth);
            return;
        }
        guards.sort();
        let mut normalized = Vec::<SummaryCrashRouteGuard>::with_capacity(guards.len());
        for guard in guards.drain(..) {
            if let (
                Some(SummaryCrashRouteGuard::Predicate(existing)),
                SummaryCrashRouteGuard::Predicate(candidate),
            ) = (normalized.last_mut(), &guard)
                && existing == candidate
            {
                if existing.scalar.is_none() {
                    existing.scalar.clone_from(&candidate.scalar);
                }
                continue;
            }
            normalized.push(guard);
        }
        *guards = normalized;
    }

    pub(crate) fn normalize_summary_buckets(
        buckets: Vec<SummaryCrashBucket>,
    ) -> Vec<SummaryCrashBucket> {
        let mut grouped = std::collections::BTreeMap::<
            checked_trees::CrashCause,
            Vec<SummaryCrashRouteGuard>,
        >::new();
        for bucket in buckets {
            grouped
                .entry(bucket.cause)
                .or_default()
                .extend(bucket.alternative_guards);
        }
        grouped
            .into_iter()
            .filter_map(|(cause, mut alternative_guards)| {
                normalize_summary_guards(&mut alternative_guards);
                (!alternative_guards.is_empty()).then_some(SummaryCrashBucket {
                    cause,
                    alternative_guards,
                })
            })
            .collect()
    }

    /*
        A checked annotation can retain proof-only operands. Only the executable
        invocation-entry algebra may authorize identity folding; an annotation's
        presence alone supplies neither an executable value nor runtime custody.
    */
    pub(super) fn executable_entry_predicate(
        expression: &checked_trees::CheckedBooleanExpression,
    ) -> bool {
        use checked_trees::{
            CheckedBooleanExpression as Boolean, CheckedScalarExpression as Scalar,
            CheckedScalarReadProjection as Projection, CheckedStorageRoot as Root,
        };
        fn projections(path: &[Projection]) -> bool {
            path.iter().all(|step| match step {
                Projection::Static(_) => true,
                Projection::RuntimeIndex(selector) => scalar(&selector.expression),
            })
        }
        fn scalar(value: &Scalar) -> bool {
            match value {
                Scalar::Parameter { .. }
                | Scalar::StructuralParameterField {
                    root: checked_trees::CheckedStorageRoot::Parameter { .. },
                    ..
                }
                | Scalar::IntegerLiteral { .. }
                | Scalar::IeeeFloatLiteral { .. }
                | Scalar::ImplementationSelection { .. } => true,
                Scalar::StructuralParameterIndexedRead {
                    root,
                    index,
                    element_path,
                    ..
                } => {
                    matches!(root, Root::Parameter { .. })
                        && scalar(&index.expression)
                        && projections(element_path)
                }
                Scalar::StructuralSequenceLength { root, path } => {
                    matches!(root, Root::Parameter { .. }) && projections(path)
                }
                Scalar::IntegerBinary { left, right, .. } => scalar(left) && scalar(right),
                Scalar::IntegerBitwiseNot { operand, .. }
                | Scalar::IntegerWiden { operand, .. }
                | Scalar::IntegerExactCast { operand, .. }
                | Scalar::RepresentationView { operand, .. } => scalar(operand),
                Scalar::Boolean(value) => executable_entry_predicate(value),
                Scalar::ConstantApplication { .. }
                | Scalar::ErasedParameter { .. }
                | Scalar::Local { .. }
                | Scalar::StructuralParameterField {
                    root:
                        checked_trees::CheckedStorageRoot::OwnedLocal { .. }
                        | checked_trees::CheckedStorageRoot::ViewLocal { .. },
                    ..
                }
                | Scalar::StorageRead { .. } => false,
            }
        }
        match expression {
            Boolean::Constant(_)
            | Boolean::Parameter { .. }
            | Boolean::StructuralParameterField { .. }
            | Boolean::IeeeFloatComparison { .. } => true,
            Boolean::Not(value) => executable_entry_predicate(value),
            facts::checked_boolean_triple_pattern!(left, right, ..) => {
                executable_entry_predicate(left) && executable_entry_predicate(right)
            }
            Boolean::IntegerComparison { left, right, .. }
            | Boolean::ScalarIeeeFloatComparison { left, right, .. } => {
                scalar(left) && scalar(right)
            }
            Boolean::ErasedParameter { .. }
            | Boolean::Local { .. }
            | Boolean::StorageRead { .. }
            | Boolean::IntegerMathComparison { .. }
            | Boolean::StructuralIdentity { .. }
            | Boolean::ByteSequenceEqual { .. }
            | Boolean::ByteSequencePredicate { .. }
            | Boolean::StructuralCaseMembership { .. } => false,
        }
    }
}

mod entry_equations {
    /*
        Exact entry equations and comparison polarity refine authenticated operands.
        A current actual first crosses the shared capture/provenance boundary. Entry
        requirements substitute closed literal values and retain checked comparison
        polarity over those original operands, independently of subsequent writes.
        Unknown origins and unselected arithmetic supply no entry evidence.
    */
    use crate::published::facts::crash_entry_values::statement_may_overwrite_place;
    use crate::published::facts::crash_plan_facts::declared_parameter_names;
    use crate::published::places::place_at;
    use checked_trees::{
        CheckedBooleanExpression as Boolean, CheckedScalarExpression as Scalar,
        CrashPredicateExpression as Expression,
    };
    use typed_trees::expression::BinaryOperator;

    pub(super) fn refine(
        program: &dyn ::validation::CheckingInput,
        operators: &checked_trees::CheckedOperatorFacts,
        machine: &typed_trees::machine::Machine,
        buckets: &mut Vec<super::summary_predicates::SummaryCrashBucket>,
    ) {
        let Some(entry) = program.machine_states(machine).first() else {
            return;
        };
        let parameters = program.state_parameters(entry);
        let names = declared_parameter_names(parameters);
        let mut equations = Vec::new();
        let mut excluded = Vec::new();
        for contract in program
            .machine_contracts(machine)
            .iter()
            .chain(program.state_contracts(entry))
            .filter(|contract| {
                contract.kind == typed_trees::signature::SignatureContractKind::Requires
            })
        {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let Some(expression) = fact.as_expression() else {
                    continue;
                };
                let Some(checked) = crate::values::lower_machine_entry_crash_contract_expression(
                    program,
                    operators,
                    machine,
                    *expression,
                    &[],
                ) else {
                    continue;
                };
                let identity =
                    super::crash_predicate_from_expression(program, *expression, &names, None);
                collect(&checked, &identity, &mut equations);
                if super::summary_predicates::executable_entry_predicate(&checked) {
                    excluded.extend(excluded_comparisons(&checked, &identity, false, 0));
                }
            }
        }
        if equations.is_empty() && excluded.is_empty() {
            return;
        }
        for bucket in buckets.iter_mut() {
            bucket.alternative_guards.retain(|guard| {
                let super::summary_predicates::SummaryCrashRouteGuard::Predicate(predicate) = guard
                else {
                    return true;
                };
                if !predicate.builtin_meaning {
                    return true;
                }
                let substituted = substitute(&predicate.identity, &equations);
                !excluded.contains(&substituted)
                    && super::summary_predicates::summary_boolean_value(&substituted) != Some(false)
            });
        }
        buckets.retain(|bucket| !bucket.alternative_guards.is_empty());
    }

    /*
        Only checked fixed-integer comparisons admit ordered complementation.
        IEEE unordered relations, erased mathematics and selected authored operators
        never supply these keys. Conjuncts combine; disjuncts retain common keys.
        The keys refer to invocation-entry identities, not current storage.
    */
    fn excluded_comparisons(
        checked: &Boolean,
        identity: &Expression,
        negated: bool,
        depth: usize,
    ) -> Vec<Expression> {
        use typed_trees::expression::UnaryOperator;
        if depth >= 64 {
            return Vec::new();
        }
        match (checked, identity) {
            (Boolean::Not(inner), Expression::Unary { operator, operand })
                if *operator == UnaryOperator::LogicalNot as u8 =>
            {
                excluded_comparisons(inner, operand, !negated, depth + 1)
            }
            (
                Boolean::And { left, right } | Boolean::Or { left, right },
                Expression::Binary {
                    operator,
                    left: a,
                    right: b,
                },
            ) if (*operator == BinaryOperator::And as u8
                && matches!(checked, Boolean::And { .. }))
                || (*operator == BinaryOperator::Or as u8
                    && matches!(checked, Boolean::Or { .. })) =>
            {
                let mut left = excluded_comparisons(left, a, negated, depth + 1);
                let right = excluded_comparisons(right, b, negated, depth + 1);
                if matches!(checked, Boolean::And { .. }) != negated {
                    left.extend(right);
                } else {
                    left.retain(|key| right.contains(key));
                }
                left
            }
            (
                Boolean::IntegerComparison { .. },
                Expression::Binary {
                    operator,
                    left,
                    right,
                },
            ) => {
                let complement = [
                    (BinaryOperator::Equal, BinaryOperator::NotEqual),
                    (BinaryOperator::NotEqual, BinaryOperator::Equal),
                    (BinaryOperator::Less, BinaryOperator::GreaterOrEqual),
                    (BinaryOperator::LessOrEqual, BinaryOperator::Greater),
                    (BinaryOperator::Greater, BinaryOperator::LessOrEqual),
                    (BinaryOperator::GreaterOrEqual, BinaryOperator::Less),
                ]
                .into_iter()
                .find_map(|(original, complement)| {
                    (*operator == original as u8).then_some(if negated {
                        original
                    } else {
                        complement
                    })
                });
                complement
                    .map(|operator| {
                        vec![Expression::Binary {
                            operator: operator as u8,
                            left: left.clone(),
                            right: right.clone(),
                        }]
                    })
                    .unwrap_or_default()
            }
            _ => Vec::new(),
        }
    }

    fn collect(
        checked: &Boolean,
        identity: &Expression,
        equations: &mut Vec<(Expression, Expression)>,
    ) {
        match (checked, identity) {
            (
                Boolean::And { left, right },
                Expression::Binary {
                    operator,
                    left: a,
                    right: b,
                },
            ) if *operator == BinaryOperator::And as u8 => {
                collect(left, a, equations);
                collect(right, b, equations);
            }
            (
                Boolean::IntegerComparison {
                    kind: checked_trees::CheckedIntegerComparisonKind::Equal,
                    left,
                    right,
                },
                Expression::Binary {
                    operator,
                    left: a,
                    right: b,
                },
            ) if *operator == BinaryOperator::Equal as u8 => {
                for (value, subject, literal) in [
                    (left.as_ref(), b.as_ref(), a.as_ref()),
                    (right.as_ref(), a.as_ref(), b.as_ref()),
                ] {
                    if matches!(value, Scalar::IntegerLiteral { .. })
                        && matches!(literal, Expression::Integer(_))
                    {
                        equations.push((subject.clone(), literal.clone()));
                    }
                }
            }
            _ => {}
        }
    }

    fn substitute(value: &Expression, equations: &[(Expression, Expression)]) -> Expression {
        if let Some((_, replacement)) = equations.iter().find(|(subject, _)| subject == value) {
            return replacement.clone();
        }
        match value {
            Expression::Binary {
                operator,
                left,
                right,
            } => Expression::Binary {
                operator: *operator,
                left: Box::new(substitute(left, equations)),
                right: Box::new(substitute(right, equations)),
            },
            Expression::Unary { operator, operand } => Expression::Unary {
                operator: *operator,
                operand: Box::new(substitute(operand, equations)),
            },
            Expression::RepresentationView {
                source_type,
                target_type,
                operand,
            } => Expression::RepresentationView {
                source_type: *source_type,
                target_type: *target_type,
                operand: Box::new(substitute(operand, equations)),
            },
            Expression::IntegerWiden {
                source_type,
                target_type,
                domain,
                operand,
            } => Expression::IntegerWiden {
                source_type: *source_type,
                target_type: *target_type,
                domain: *domain,
                operand: Box::new(substitute(operand, equations)),
            },
            _ => value.clone(),
        }
    }

    /*
        A completed scalar result can inherit an entry operand from a surviving
        checked `result == formal` guarantee. The assignment pins the exact call;
        the guarantee pins normal completion and the captured formal, independently
        of the callee body or the caller's current source value.
    */
    pub(super) fn completed_operand(
        program: &dyn ::validation::CheckingInput,
        operators: &checked_trees::CheckedOperatorFacts,
        semantic: &facts::FactPlan,
        flow: &checked_trees::FlowFacts,
        caller: &checked_trees::FlowStateFact,
        statement: usize,
        contexts: &[facts::FactContextHandle],
        argument: typed_trees::expression::ExpressionHandle,
    ) -> Option<Expression> {
        let result = if matches!(
            program.expression_table.expression(argument),
            typed_trees::expression::ExpressionNode::Call(_)
        ) {
            argument
        } else {
            let place = place_at(program, caller.state_symbol, statement, argument)?;
            let (_, state) = validation::machine_state_by_symbols(
                program,
                caller.machine_symbol,
                caller.state_symbol,
            )?;
            let containing = program.statement_table.statement_at(state, statement)?;
            if statement_may_overwrite_place(program, caller.machine_symbol, containing, &place) {
                return None;
            }
            let captured = crate::checks::contracts::captured_guarantee_place(
                program, semantic, contexts, place,
            )?;
            let facts::PlaceRoot::Expression(result) = captured.root else {
                return None;
            };
            if !captured.segments.is_empty() {
                return None;
            }
            result
        };
        let mut agreed = None;
        for fact in contexts.iter().flat_map(|context| {
            semantic
                .context_view(semantic.contexts.get(*context))
                .facts()
        }) {
            let facts::FactPayload::ContractBooleanExpression {
                kind: facts::ContractFactKind::Ensures,
                expression,
                ..
            } = fact.payload
            else {
                continue;
            };
            let facts::ProgramPoint::CallEnsures {
                machine_symbol,
                state_symbol,
                statement_index,
                call_ordinal,
            } = fact.point
            else {
                continue;
            };
            if fact.origin != facts::FactOrigin::CallEnsures
                || machine_symbol != caller.machine_symbol
                || state_symbol != caller.state_symbol
                || statement_index > statement
            {
                continue;
            }
            let Some(site) = validation::find_call_site(
                program,
                machine_symbol,
                state_symbol,
                statement_index,
                call_ordinal,
            ) else {
                continue;
            };
            if !matches!(site, validation::CallSite::Expression { expression, .. } if expression == result)
            {
                continue;
            }
            let Some(call) = crate::flow::call_flows(&flow.control, caller.calls)
                .iter()
                .find(|call| {
                    call.statement_index == statement_index && call.call_ordinal == call_ordinal
                })
            else {
                continue;
            };
            let Some((machine, state)) = validation::find_machine_head(program, call.target_symbol)
            else {
                continue;
            };
            if !validation::has_builtin_bound_expression_meaning(
                program,
                machine,
                Some(state),
                expression,
            ) {
                continue;
            }
            let parameters = program.state_parameters(state);
            let Some(checked) = crate::values::lower_callable_scalar_contract_predicate(
                program,
                operators,
                machine.symbol,
                parameters,
                state.return_type,
                expression,
                true,
                &mut 4096,
            ) else {
                continue;
            };
            let Boolean::IntegerComparison {
                kind: checked_trees::CheckedIntegerComparisonKind::Equal,
                left,
                right,
            } = checked
            else {
                continue;
            };
            let scalar_parameters = parameters
                .iter()
                .filter(|parameter| crate::values::occupies_scalar_position(program, parameter))
                .collect::<Vec<_>>();
            for (result_term, input) in [
                (left.as_ref(), right.as_ref()),
                (right.as_ref(), left.as_ref()),
            ] {
                let (
                    Scalar::Parameter {
                        position: output,
                        primitive_type: output_type,
                    },
                    Scalar::Parameter {
                        position: input,
                        primitive_type: input_type,
                    },
                ) = (result_term, input)
                else {
                    continue;
                };
                if *output != scalar_parameters.len() || output_type != input_type {
                    continue;
                }
                let Some(parameter) = scalar_parameters.get(*input) else {
                    continue;
                };
                let Some(position) = parameters
                    .iter()
                    .position(|candidate| candidate.symbol == parameter.symbol)
                else {
                    continue;
                };
                let Some(actual) =
                    validation::call_site_argument_expressions(program, &site).get(position)
                else {
                    continue;
                };
                let Some((_, producer_state)) =
                    validation::machine_state_by_symbols(program, machine_symbol, state_symbol)
                else {
                    continue;
                };
                let Some(producer_statement) = program
                    .statement_table
                    .statement_at(producer_state, statement_index)
                else {
                    continue;
                };
                if let Some(actual_place) =
                    place_at(program, state_symbol, statement_index, *actual)
                    && statement_may_overwrite_place(
                        program,
                        machine_symbol,
                        producer_statement,
                        &actual_place,
                    )
                {
                    continue;
                }
                let Some(value) = crate::published::facts::crash_entry_values::entry_operand(
                    program,
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    *actual,
                ) else {
                    continue;
                };
                if agreed.as_ref().is_some_and(|known| *known != value) {
                    return None;
                }
                agreed = Some(value);
            }
        }
        agreed
    }
}
mod entry_paths {
    /*
        Crash possibility reads the entry-path receipt produced by evaluation.
        These historical conditions are independent of current-storage facts and
        the selecting caller. Unknown observations widen a route, never erase it.
    */
    use super::summary_predicates::{SummaryCrashPredicate, SummaryCrashRouteGuard as RouteGuard};
    use crate::flow::active_context_handles;
    use crate::published::facts::boolean_predicates::entry_path_necessary_condition;
    use crate::values::occupies_scalar_position;
    use checked_trees::{
        CheckedBooleanExpression as Boolean, CheckedScalarExpression as Scalar,
        CheckedStructuralPredicatePathSegment as Path, CrashPredicateExpression as Expression,
    };
    use symbols::SymbolHandle;
    use typed_trees::expression::{BinaryOperator, UnaryOperator};

    pub(super) struct EntryConditions<'a> {
        program: &'a dyn validation::CheckingInput,
        semantic: &'a facts::FactPlan,
        flow: &'a checked_trees::FlowFacts,
    }

    impl<'a> EntryConditions<'a> {
        pub(super) fn new(
            program: &'a dyn validation::CheckingInput,
            semantic: &'a facts::FactPlan,
            flow: &'a checked_trees::FlowFacts,
        ) -> Self {
            Self {
                program,
                semantic,
                flow,
            }
        }

        pub(super) fn for_machine(&self, machine: SymbolHandle) -> MachineEntryConditions<'_> {
            let scalars = self
                .program
                .machine_by_symbol(machine)
                .and_then(|machine| self.program.machine_states(machine).first())
                .and_then(|entry| {
                    self.program
                        .state_parameters(entry)
                        .iter()
                        .enumerate()
                        .filter(|(_, parameter)| occupies_scalar_position(self.program, parameter))
                        .map(|(position, _)| u32::try_from(position))
                        .collect::<Result<Vec<_>, _>>()
                        .ok()
                });
            MachineEntryConditions {
                query: self,
                machine,
                scalars,
            }
        }
    }

    pub(super) struct MachineEntryConditions<'a> {
        query: &'a EntryConditions<'a>,
        machine: SymbolHandle,
        /* Checked scalar positions are dense; route identity uses authored positions. */
        scalars: Option<Vec<u32>>,
    }

    impl MachineEntryConditions<'_> {
        pub(super) fn at_call(&self, call: &checked_trees::FlowCallFact) -> RouteGuard {
            self.under_contexts(call.entry_semantic_contexts)
        }

        pub(super) fn at_site(&self, location: checked_trees::CrashSiteLocation) -> RouteGuard {
            self.query
                .flow
                .control
                .state(self.machine, location.state())
                .and_then(|state| {
                    self.query
                        .flow
                        .control
                        .statements
                        .span_or_empty(state.statements)
                        .iter()
                        .find(|statement| {
                            statement.statement_index == location.statement_ordinal() as usize
                        })
                })
                .map_or(RouteGuard::Truth, |statement| {
                    self.under_contexts(statement.entry_semantic_contexts)
                })
        }

        fn under_contexts(
            &self,
            contexts: arena::HandleSpan<checked_trees::FlowSemanticContextRef>,
        ) -> RouteGuard {
            if self.scalars.is_none() {
                return RouteGuard::Truth;
            }
            let contexts = active_context_handles(&self.query.flow.contexts, contexts);
            let Some(condition) =
                entry_path_necessary_condition(self.query.semantic, &contexts, self.machine)
            else {
                return RouteGuard::Truth;
            };
            let Some(identity) = self.boolean(&condition, 0) else {
                return RouteGuard::Truth;
            };
            if condition == Boolean::Constant(true) {
                return RouteGuard::Truth;
            }
            RouteGuard::Predicate(SummaryCrashPredicate {
                identity,
                builtin_meaning: true,
                scalar: Some(condition),
            })
        }

        /*
            Only the checked executable comparison algebra has this identity
            projection. Unsupported or effectful scalar forms lose precision rather
            than masquerading as domain-free builtin source arithmetic.
        */
        fn boolean(&self, condition: &Boolean, depth: usize) -> Option<Expression> {
            if depth >= 64 {
                return None;
            }
            let binary = |operator: BinaryOperator, left: Expression, right: Expression| {
                Expression::Binary {
                    operator: operator as u8,
                    left: Box::new(left),
                    right: Box::new(right),
                }
            };
            Some(match condition {
                Boolean::Constant(value) => Expression::Boolean(*value),
                Boolean::Parameter { position } => {
                    Expression::Parameter(*self.scalars.as_ref()?.get(*position)?)
                }
                Boolean::StructuralParameterField {
                    parameter_position,
                    path,
                    ..
                } => self.project(*parameter_position, path)?,
                Boolean::Not(value) => Expression::Unary {
                    operator: UnaryOperator::LogicalNot as u8,
                    operand: Box::new(self.boolean(value, depth + 1)?),
                },
                Boolean::And { left, right } => binary(
                    BinaryOperator::And,
                    self.boolean(left, depth + 1)?,
                    self.boolean(right, depth + 1)?,
                ),
                Boolean::Or { left, right } => binary(
                    BinaryOperator::Or,
                    self.boolean(left, depth + 1)?,
                    self.boolean(right, depth + 1)?,
                ),
                Boolean::Equal { left, right } => binary(
                    BinaryOperator::Equal,
                    self.boolean(left, depth + 1)?,
                    self.boolean(right, depth + 1)?,
                ),
                Boolean::IntegerComparison { kind, left, right } => binary(
                    match kind {
                        checked_trees::CheckedIntegerComparisonKind::Equal => BinaryOperator::Equal,
                        checked_trees::CheckedIntegerComparisonKind::LessThan => {
                            BinaryOperator::Less
                        }
                        checked_trees::CheckedIntegerComparisonKind::LessOrEqual => {
                            BinaryOperator::LessOrEqual
                        }
                    },
                    self.scalar(left, depth + 1)?,
                    self.scalar(right, depth + 1)?,
                ),
                /*
                    An IEEE comparison keeps its own operator. The negated kinds
                    are not the reversed relations (`!(x < y)` holds for NaN while
                    `x >= y` does not), so they stay a logical negation.
                */
                Boolean::ScalarIeeeFloatComparison { kind, left, right } => {
                    use checked_trees::CheckedIeeeFloatComparisonKind as Kind;
                    let (operator, negated) = match kind {
                        Kind::Equal => (BinaryOperator::Equal, false),
                        Kind::NotEqual => (BinaryOperator::NotEqual, false),
                        Kind::LessThan => (BinaryOperator::Less, false),
                        Kind::LessOrEqual => (BinaryOperator::LessOrEqual, false),
                        Kind::NotLessThan => (BinaryOperator::Less, true),
                        Kind::NotLessOrEqual => (BinaryOperator::LessOrEqual, true),
                    };
                    let comparison = binary(
                        operator,
                        self.scalar(left, depth + 1)?,
                        self.scalar(right, depth + 1)?,
                    );
                    if negated {
                        Expression::Unary {
                            operator: UnaryOperator::LogicalNot as u8,
                            operand: Box::new(comparison),
                        }
                    } else {
                        comparison
                    }
                }
                _ => return None,
            })
        }

        fn scalar(&self, value: &Scalar, depth: usize) -> Option<Expression> {
            if depth >= 64 {
                return None;
            }
            match value {
                Scalar::Parameter { position, .. } => Some(Expression::Parameter(
                    *self.scalars.as_ref()?.get(*position)?,
                )),
                Scalar::StructuralParameterField {
                    root:
                        checked_trees::CheckedStorageRoot::Parameter {
                            index: parameter_position,
                        },
                    path,
                    ..
                } => self.project(*parameter_position, path),
                Scalar::IntegerLiteral { literal } => Some(Expression::Integer(
                    literal.decimal_value_bignum()?.to_string(),
                )),
                /*
                    Route identities spell a float literal as its source text. The
                    shortest round-trip spelling of the evaluated bits (`0.0`,
                    `2.5`) is how such literals are ordinarily written; any other
                    spelling of the same value only fails to match, which widens
                    the route rather than erasing it.
                */
                Scalar::IeeeFloatLiteral { value } => {
                    let (format, bits) = match *value {
                        semantic_vocabulary::IeeeFloatValue::Binary32(bits) => {
                            (numerics::literals::FloatFormat::F32, u64::from(bits))
                        }
                        semantic_vocabulary::IeeeFloatValue::Binary64(bits) => {
                            (numerics::literals::FloatFormat::F64, bits)
                        }
                    };
                    numerics::literals::FloatLiteral::from_bits(format, bits)
                        .map(|literal| Expression::Float(literal.text().to_owned()))
                }
                Scalar::Boolean(condition) => self.boolean(condition, depth + 1),
                _ => None,
            }
        }

        fn project(&self, parameter: u32, path: &[Path]) -> Option<Expression> {
            let mut expression = Expression::Parameter(parameter);
            for step in path {
                expression = match step {
                    Path::Field(member) if member.is_valid() => Expression::Member {
                        receiver: Box::new(expression),
                        member: member.spelling(),
                    },
                    Path::FixedIndex(index) => Expression::Indexed {
                        collection: Box::new(expression),
                        index: Box::new(Expression::Integer(index.to_string())),
                    },
                    _ => return None,
                };
            }
            Some(expression)
        }
    }
}
mod float_observations {
    /*
        IEEE observations of completed invocation operands. Copies keep their
        completion facts and reaching stores; views observe invocation backing.
        Replaying an initializer in later contexts would turn an earlier copy into
        a newer value. Source expressions classify literals and representation
        views only; they never supply missing storage provenance.
    */
    use crate::flow::state_statements;
    use crate::published::places::place_at;
    use arena::HandleSpan;
    use checked_trees::{
        FlowCallFact, FlowConstraintRef, FlowFacts, FlowOperandFact, FlowStateFact,
    };
    use facts::{FactContextHandle, FactPlan};
    use numerics::float_semantics::{FloatMeaning, FloatSemantics};
    use typed_trees::expression::ExpressionNode;
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

    mod operations {
        /*
            Normal results of exact sealed IEEE operations over completed operands.
            Selection, operand custody and result policy remain independent evidence.
        */
        use super::{FloatObservations, extend_values};
        use checked_trees::{
            CheckedArithmeticPolicyAdapter, CheckedValueOrigin, FlowOperandFact, FlowStateFact,
        };
        use numerics::float_semantics::{FloatFormat, FloatMeaning, FloatSemantics};
        use typed_trees::expression::{BinaryOperator, ExpressionHandle};

        impl FloatObservations<'_> {
            pub(super) fn operation(
                &self,
                state: &FlowStateFact,
                capture: &FlowOperandFact,
                expression: ExpressionHandle,
            ) -> Option<Vec<FloatMeaning>> {
                let mut selection = None;
                let mut ambiguous = false;
                self.operators.for_each_resolved_expression_selection(
                    expression,
                    |origin, selected, adapter, use_fact| {
                        if selection.is_some() {
                            ambiguous = true;
                        } else {
                            selection = Some((origin, selected, adapter, use_fact));
                        }
                    },
                );
                let (origin, selected, adapter, use_fact) = selection?;
                if ambiguous
                    || !matches!(origin, CheckedValueOrigin::StateStatement {
                        machine_symbol, state_symbol, statement_index, ..
                    } if machine_symbol == state.machine_symbol
                        && state_symbol == capture.state_symbol
                        && statement_index == capture.statement_index)
                {
                    return None;
                }
                let operator =
                    typed_trees::operator::declaration_by_symbol(self.program, selected)?;
                if !typed_trees::operator::is_sealed_toolchain_float_operator(
                    self.program,
                    operator,
                ) {
                    return None;
                }
                let (operation, format) = typed_trees::operator::primitive_float_binary_semantics(
                    self.program,
                    operator,
                )?;
                if !matches!(
                    operation,
                    BinaryOperator::Add
                        | BinaryOperator::Subtract
                        | BinaryOperator::Multiply
                        | BinaryOperator::Divide
                ) {
                    return None;
                }
                let parameters = self.program.operator_parameters(operator);
                let operands = if let Some(use_fact) = use_fact {
                    use_fact.operands(self.program)?
                } else {
                    crate::published::facts::operator_crashes::named_call_operands(
                        self.program,
                        self.program.expression_table.call_node(expression)?,
                        parameters,
                    )?
                };
                if parameters.len() != 2 || operands.len() != 2 {
                    return None;
                }
                let invocation = crate::checks::InvocationContexts::for_selected_expression_in_flow(
                    self.program,
                    self.operators,
                    self.flow,
                    self.semantic,
                    expression,
                    selected,
                    &operands,
                );
                let constraints = invocation.entry_constraints()?;
                let epoch = invocation.operand_at(1)?;
                let read = |position: usize| {
                    self.capture_values(
                        state,
                        invocation.operand_at(position)?,
                        parameters[position].type_reference,
                        constraints,
                        epoch,
                    )
                };
                let (left_values, right_values) = (read(0)?, read(1)?);
                let format = match format {
                    numerics::literals::FloatFormat::F32 => FloatFormat::BINARY32,
                    numerics::literals::FloatFormat::F64 => FloatFormat::BINARY64,
                };
                let mut results = Vec::new();
                for left in &left_values {
                    for right in &right_values {
                        let result = match operation {
                            BinaryOperator::Add => FloatSemantics::add(format, left, right),
                            BinaryOperator::Subtract => {
                                FloatSemantics::subtract(format, left, right)
                            }
                            BinaryOperator::Multiply => {
                                FloatSemantics::multiply(format, left, right)
                            }
                            BinaryOperator::Divide => FloatSemantics::divide(format, left, right),
                            _ => return None,
                        };
                        let result = match adapter {
                            CheckedArithmeticPolicyAdapter::None => result,
                            CheckedArithmeticPolicyAdapter::FloatTrappingNonFinite {
                                format: adapted,
                            } if adapted == format => {
                                FloatSemantics::apply_trapping_policy(result).ok()?
                            }
                            CheckedArithmeticPolicyAdapter::FloatSaturatingOverflowOnly {
                                format: adapted,
                            } if adapted == format => {
                                if operation == BinaryOperator::Divide {
                                    FloatSemantics::apply_saturating_divide_policy(
                                        format, left, right, result,
                                    )
                                } else {
                                    FloatSemantics::apply_saturating_policy(
                                        format,
                                        &[left, right],
                                        result,
                                    )
                                }
                            }
                            _ => return None,
                        };
                        extend_values(&mut results, [result]);
                    }
                }
                (!results.is_empty()).then_some(results)
            }
        }
    }

    pub(super) struct FloatObservations<'a> {
        program: &'a dyn validation::CheckingInput,
        operators: &'a checked_trees::CheckedOperatorFacts,
        semantic: &'a FactPlan,
        flow: &'a FlowFacts,
    }

    impl<'a> FloatObservations<'a> {
        pub(super) fn new(
            program: &'a dyn validation::CheckingInput,
            operators: &'a checked_trees::CheckedOperatorFacts,
            semantic: &'a FactPlan,
            flow: &'a FlowFacts,
        ) -> Self {
            Self {
                program,
                operators,
                semantic,
                flow,
            }
        }

        pub(super) fn call_argument(
            &self,
            state: &FlowStateFact,
            call: &FlowCallFact,
            position: usize,
            reference: TypeReferenceHandle,
        ) -> Option<String> {
            let (operands, offset) =
                crate::flow::captured_call_operands(self.program, self.flow.view(), state, call)?;
            let values = self.capture_values(
                state,
                operands.get(position + offset)?,
                reference,
                call.entry_constraints,
                operands.last()?,
            )?;
            observation_spelling(&values)
        }

        /*
            Preserve every possible completed meaning until the final observation.
            IEEE equality alone cannot distinguish the two signed zeros; arithmetic
            must carry both rather than treating a representative as exact input.
        */
        fn capture_values(
            &self,
            state: &FlowStateFact,
            operand: &FlowOperandFact,
            reference: TypeReferenceHandle,
            invocation: HandleSpan<FlowConstraintRef>,
            epoch: &FlowOperandFact,
        ) -> Option<Vec<FloatMeaning>> {
            let values = crate::flow::captured_operand_values(
                self.program,
                self.flow.view(),
                self.semantic,
                state,
                crate::flow::CapturedOperand::unrecorded(operand),
                reference,
                &[],
            )?;
            let values = crate::flow::observe_captured_values(
                self.program,
                self.flow,
                self.semantic,
                state,
                values,
                epoch,
            );
            let mut possibilities = Vec::new();
            for value in values {
                let observed = [
                    (
                        value.observation,
                        value.observation_type,
                        &value.observation_projection,
                        crate::flow::captured_observation_places(
                            self.program,
                            self.flow.view(),
                            &value,
                        ),
                        invocation,
                        epoch,
                    ),
                    (
                        value.operand,
                        value.type_reference,
                        &value.projection,
                        crate::flow::captured_value_places(self.program, self.flow.view(), &value),
                        value.producer_invocation(invocation),
                        value.producer_epoch.map_or(epoch, |source| source.row()),
                    ),
                ]
                .into_iter()
                .find_map(
                    |(capture, reference, projection, places, invocation, epoch)| {
                        let contexts = crate::flow::operand_contexts(
                            self.program,
                            &self.flow.contexts,
                            &self.flow.control,
                            self.semantic,
                            capture.row(),
                            reference,
                            invocation,
                        );
                        let recorded = places.and_then(|places| {
                            let mut possibilities = Vec::new();
                            for place in places {
                                let observed = match crate::values::ieee_equal_spelling_at_place(
                                    self.program,
                                    self.semantic,
                                    contexts
                                        .iter()
                                        .map(|handle| self.semantic.contexts.get(*handle)),
                                    &place.projected,
                                ) {
                                    Some(spelling) => FloatMeaning::from_f64(
                                        numerics::literals::FloatLiteral::parse(&spelling)?
                                            .landed_f64(),
                                    ),
                                    None => self.viewed_integer_bits(
                                        capture.row(),
                                        reference,
                                        &place.projected,
                                        &contexts,
                                    )?,
                                };
                                if observed.is_zero() {
                                    extend_values(
                                        &mut possibilities,
                                        [
                                            FloatMeaning::Zero { negative: false },
                                            FloatMeaning::Zero { negative: true },
                                        ],
                                    );
                                } else {
                                    extend_values(&mut possibilities, [observed]);
                                }
                            }
                            (!possibilities.is_empty()).then_some(possibilities)
                        });
                        recorded.or_else(|| {
                            if !projection.remaining.is_empty() {
                                return None;
                            }
                            self.produced(
                                state,
                                capture.row(),
                                projection.expression,
                                epoch,
                                &contexts,
                            )
                        })
                    },
                )?;
                extend_values(&mut possibilities, observed);
            }
            (!possibilities.is_empty()).then_some(possibilities)
        }

        fn produced(
            &self,
            state: &FlowStateFact,
            operand: &FlowOperandFact,
            expression: typed_trees::expression::ExpressionHandle,
            epoch: &FlowOperandFact,
            contexts: &[FactContextHandle],
        ) -> Option<Vec<FloatMeaning>> {
            if let Some(value) = self.operation(state, operand, expression) {
                return Some(value);
            }
            match self.program.expression_table.expression(expression) {
                ExpressionNode::Float(_) => {
                    let carrier = self
                        .program
                        .primitive_type_reference(operand.type_reference)?;
                    Some(vec![meaning(validation::closed_float_range_endpoint(
                        self.program,
                        expression,
                        carrier,
                    )?)])
                }
                ExpressionNode::Name(path) if path.symbol == path.head_symbol => {
                    self.constant(path.symbol).map(|value| vec![value])
                }
                ExpressionNode::UndeterminedNaN(format) => {
                    let expected = match format {
                        numerics::literals::FloatFormat::F32 => PrimitiveType::F32,
                        numerics::literals::FloatFormat::F64 => PrimitiveType::F64,
                    };
                    (self
                        .program
                        .primitive_type_reference(operand.type_reference)
                        == Some(expected))
                    .then_some(vec![FloatMeaning::NaN])
                }
                ExpressionNode::Call(authored) => {
                    let mut calls =
                        self.flow
                            .control
                            .calls
                            .span(state.calls)?
                            .iter()
                            .filter(|call| {
                                call.statement_index == operand.statement_index
                                    && operand.state_symbol == state.state_symbol
                                    && call.authored_expression == expression
                            });
                    let call = calls.next()?;
                    if calls.next().is_some()
                        || call.target_symbol
                            != validation::call_target_entry_symbol(
                                self.program,
                                validation::call_target_symbol(self.program, authored),
                            )
                    {
                        return None;
                    }
                    let (callee, entry) =
                        validation::find_machine_head(self.program, call.target_symbol)?;
                    if !callee.supplies_checked_body()
                        || !callee.body_is_present
                        || callee.target_family_interface
                    {
                        return None;
                    }
                    let returned = typed_trees::statement::direct_state_return_values(
                        &self.program.statement_table,
                        state_statements(self.program, entry.statement_nodes),
                    )?;
                    let mut formal = None;
                    for returned in returned {
                        let ExpressionNode::Name(path) =
                            self.program.expression_table.expression(returned)
                        else {
                            return None;
                        };
                        if !self
                            .program
                            .expression_table
                            .name_path_is_single_member(path)
                            || formal.is_some_and(|known| known != path.symbol)
                        {
                            return None;
                        }
                        formal = Some(path.symbol);
                    }
                    let formal = formal?;
                    let (position, parameter) = self
                        .program
                        .state_parameters(entry)
                        .iter()
                        .enumerate()
                        .find(|(_, parameter)| {
                            parameter.symbol == formal && !parameter.is_mutable
                        })?;
                    let carrier = self
                        .program
                        .primitive_type_reference(parameter.type_reference)?;
                    if !carrier.accepts_float_literal()
                        || self.program.primitive_type_reference(entry.return_type) != Some(carrier)
                    {
                        return None;
                    }
                    let (operands, offset) = crate::flow::captured_call_operands(
                        self.program,
                        self.flow.view(),
                        state,
                        call,
                    )?;
                    self.capture_values(
                        state,
                        operands.get(position + offset)?,
                        parameter.type_reference,
                        call.entry_constraints,
                        operands.last()?,
                    )
                }
                ExpressionNode::Borrow(borrow)
                    if borrow.access == language_core::ReferenceAccess::Shared =>
                {
                    self.produced(state, operand, borrow.target, epoch, contexts)
                }
                ExpressionNode::Cast(cast) if cast.form.is_recast() => self
                    .recast(state, operand, epoch, cast, contexts)
                    .map(|value| vec![value]),
                _ => None,
            }
        }

        /*
            Completed declaration meaning is independent of caller storage and its
            current guards. Semantic NaN supplies no chosen representation payload.
        */
        fn constant(&self, symbol: symbols::SymbolHandle) -> Option<FloatMeaning> {
            use language_core::constant_value::EvaluatedConstant;
            use language_semantics::const_value::{
                CanonicalConstIdentity, DecodedCanonicalConstValue,
            };
            let declaration = self.program.const_declaration_by_symbol(symbol)?;
            let carrier = self
                .program
                .primitive_type_reference(declaration.declared_type)?;
            let expected = match carrier {
                PrimitiveType::F32 => numerics::literals::FloatFormat::F32,
                PrimitiveType::F64 => numerics::literals::FloatFormat::F64,
                _ => return None,
            };
            match declaration.evaluated_value.as_ref()? {
                EvaluatedConstant::UndeterminedNaN(format) if *format == expected => {
                    Some(FloatMeaning::NaN)
                }
                EvaluatedConstant::Encoded(encoding) => {
                    let DecodedCanonicalConstValue::Float { format, bits } =
                        (CanonicalConstIdentity {
                            type_name: expected.name().to_owned(),
                            encoding: encoding.clone(),
                        })
                        .decode_declaration_encoding()?
                    else {
                        return None;
                    };
                    if format != expected {
                        return None;
                    }
                    Some(meaning(match format {
                        numerics::literals::FloatFormat::F32 => {
                            semantic_vocabulary::IeeeFloatValue::Binary32(u32::try_from(bits).ok()?)
                        }
                        numerics::literals::FloatFormat::F64 => {
                            semantic_vocabulary::IeeeFloatValue::Binary64(bits)
                        }
                    }))
                }
                _ => None,
            }
        }

        fn recast(
            &self,
            state: &FlowStateFact,
            operand: &FlowOperandFact,
            epoch: &FlowOperandFact,
            cast: &typed_trees::expression::TableCastExpression,
            contexts: &[FactContextHandle],
        ) -> Option<FloatMeaning> {
            let target = self
                .program
                .primitive_type_reference(observed_carrier(self.program, cast.target_type))?;
            if !target.accepts_float_literal() {
                return None;
            }
            let source = match self.program.expression_table.expression(cast.value) {
                ExpressionNode::Borrow(borrow)
                    if borrow.access == language_core::ReferenceAccess::Shared =>
                {
                    borrow.target
                }
                _ => cast.value,
            };
            let (machine, body) = validation::machine_state_by_symbols(
                self.program,
                state.machine_symbol,
                operand.state_symbol,
            )?;
            let source_type = self.program.primitive_type_reference(
                validation::expression_result_type_reference(self.program, machine, body, source)?,
            )?;
            let place = place_at(
                self.program,
                operand.state_symbol,
                operand.statement_index,
                source,
            )?;
            if !crate::flow::operand_entry_origin_is_stable(
                self.program,
                self.flow.view(),
                epoch,
                &place,
            ) {
                return None;
            }
            self.integer_bits(source_type, target, &place, contexts)
        }

        /*
            A captured view's place is its backing storage. When that storage is
            an equal-width fixed integer while the observation is a float, only a
            representation view can connect them, so the observation is the
            stored integer's bits. The place is the flow's own snapshot, read under
            the same contexts as a recorded IEEE equality would be.
        */
        fn viewed_integer_bits(
            &self,
            operand: &FlowOperandFact,
            reference: TypeReferenceHandle,
            place: &validation::CanonicalPlace,
            contexts: &[FactContextHandle],
        ) -> Option<FloatMeaning> {
            let target = self
                .program
                .primitive_type_reference(observed_carrier(self.program, reference))?;
            let storage = validation::canonical_place_type_reference(
                self.program,
                operand.state_symbol,
                operand.statement_index,
                place,
            )?;
            let source_type = self.program.primitive_type_reference(storage)?;
            self.integer_bits(source_type, target, place, contexts)
        }

        fn integer_bits(
            &self,
            source_type: PrimitiveType,
            target: PrimitiveType,
            place: &validation::CanonicalPlace,
            contexts: &[FactContextHandle],
        ) -> Option<FloatMeaning> {
            if !target.accepts_float_literal()
                || !source_type.is_fixed_integer()
                || source_type.scalar_byte_size() != target.scalar_byte_size()
            {
                return None;
            }
            let facts::ScalarValue::Integer(integer) = crate::values::scalar_value_at_place(
                self.program,
                self.semantic,
                contexts
                    .iter()
                    .map(|handle| self.semantic.contexts.get(*handle)),
                place,
            )?
            else {
                return None;
            };
            let admitted = crate::values::admitted_integer(
                crate::values::integer_type(source_type)?,
                &integer,
            )?;
            let bits = match admitted {
                semantic_vocabulary::IntegerValue::Signed(value) => value as u128,
                semantic_vocabulary::IntegerValue::Unsigned(value) => value,
            };
            Some(match target {
                PrimitiveType::F32 => FloatMeaning::from_f32(f32::from_bits(bits as u32)),
                PrimitiveType::F64 => FloatMeaning::from_f64(f64::from_bits(bits as u64)),
                _ => return None,
            })
        }
    }

    /* The scalar a value or a shared view of it observes. */
    fn observed_carrier(
        program: &dyn validation::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> TypeReferenceHandle {
        let peeled = program
            .type_reference_table
            .peeled_type_reference(reference);
        match program.type_reference_table.type_reference(peeled) {
            TypeReferenceNode::Reference { referee, .. } => *referee,
            _ => peeled,
        }
    }

    fn extend_values(
        values: &mut Vec<FloatMeaning>,
        additional: impl IntoIterator<Item = FloatMeaning>,
    ) {
        for value in additional {
            crate::shared::collections::push_unique(&mut *values, value);
        }
    }

    fn observation_spelling(values: &[FloatMeaning]) -> Option<String> {
        let first = values.first()?;
        if !values
            .iter()
            .all(|value| FloatSemantics::equal(first, value) || (first.is_nan() && value.is_nan()))
        {
            return None;
        }
        Some(
            numerics::literals::FloatLiteral::from_f64(first.to_f64())
                .text()
                .to_owned(),
        )
    }

    fn meaning(value: semantic_vocabulary::IeeeFloatValue) -> FloatMeaning {
        match value {
            semantic_vocabulary::IeeeFloatValue::Binary32(bits) => {
                FloatMeaning::from_f32(f32::from_bits(bits))
            }
            semantic_vocabulary::IeeeFloatValue::Binary64(bits) => {
                FloatMeaning::from_f64(f64::from_bits(bits))
            }
        }
    }
}

use crate::flow::expressions;
use crate::published::facts::crash_calls::private_summaries::{
    infer_private_body_summaries, requirement_signature,
};
use crate::published::facts::crash_calls::route_substitution::{
    SelectedTargetCrashRoutes, call_argument_substitution, refine_published_crash_routes,
};
use crate::published::facts::crash_calls::summary_predicates::{
    SummaryCrashBucket, normalize_summary_buckets,
};
use crate::published::facts::crash_plan_facts::declared_parameter_names;
use crate::shared::call_formals::arguments_aligned_to_formals;
use checked_trees::CrashPredicateExpression;
pub(crate) use private_summaries::{
    PreparedCrashSummaryGraph, prepare_crash_summary_graph, selected_invocation_target,
};
use symbols::SymbolHandle;

pub(super) fn crash_predicate_from_expression(
    program: &dyn ::validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
    parameter_names: &[String],
    content_conservation: Option<&[validation::ContentConservationSourcePlan]>,
) -> CrashPredicateExpression {
    predicate_expression(program, expression, parameter_names, content_conservation)
}

fn predicate_expression(
    program: &dyn ::validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
    parameter_names: &[String],
    content_conservation: Option<&[validation::ContentConservationSourcePlan]>,
) -> CrashPredicateExpression {
    use typed_trees::expression::ExpressionNode;

    if let Some(conservation) = content_conservation.and_then(|plans| {
        plans
            .iter()
            .find(|candidate| candidate.source_expression == expression)
    }) {
        return CrashPredicateExpression::ContentConservation(
            language_semantics::content::content_conservation_plan_bytes(&conservation.plan),
        );
    }
    if !expression.is_valid() {
        return CrashPredicateExpression::Invalid;
    }
    let lower =
        |operand| predicate_expression(program, operand, parameter_names, content_conservation);
    if program
        .expression_table
        .call_node(expression)
        .is_some_and(|call| {
            validation::mathematical_applications::is_mathematical_call(program, call)
        })
        && let Ok(identity) =
            validation::mathematical_applications::identity_application_at(program, expression)
    {
        return CrashPredicateExpression::structural_identity(
            program.normalized_type_identity(identity.carrier),
            &program.type_identities,
            lower(identity.subjects[0]),
            lower(identity.subjects[1]),
        )
        .unwrap_or(CrashPredicateExpression::Invalid);
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => CrashPredicateExpression::Binary {
            operator: binary.operator as u8,
            left: Box::new(lower(binary.left)),
            right: Box::new(lower(binary.right)),
        },
        ExpressionNode::Unary(unary) => CrashPredicateExpression::Unary {
            operator: unary.operator as u8,
            operand: Box::new(lower(unary.operand)),
        },
        ExpressionNode::Integer(value) => CrashPredicateExpression::Integer(value.to_string()),
        ExpressionNode::Float(value) => CrashPredicateExpression::Float(value.text().to_owned()),
        ExpressionNode::Boolean(value) => CrashPredicateExpression::Boolean(*value),
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            if let [single] = members
                && let Some(index) = parameter_names
                    .iter()
                    .position(|name| name == single.as_str())
            {
                return CrashPredicateExpression::Parameter(
                    u32::try_from(index).expect("parameter index fits u32"),
                );
            }
            CrashPredicateExpression::Name(
                members
                    .iter()
                    .map(|member| member.as_str().to_owned())
                    .collect(),
            )
        }
        ExpressionNode::Member(member) => CrashPredicateExpression::Member {
            receiver: Box::new(lower(member.receiver)),
            member: member_entry_name(
                program,
                member,
                validation::effective_member_symbol(program, member.receiver, member),
            ),
        },
        ExpressionNode::Indexed(indexed) => CrashPredicateExpression::Indexed {
            collection: Box::new(lower(indexed.collection)),
            index: Box::new(lower(indexed.index)),
        },
        ExpressionNode::Range(range) => CrashPredicateExpression::Range {
            start: Box::new(lower(range.start)),
            end: Box::new(lower(range.end)),
            end_inclusive: range.end_inclusive,
        },
        ExpressionNode::Call(call) => CrashPredicateExpression::Call {
            target: call.target.as_str().to_owned(),
            receiver: Box::new(lower(call.receiver)),
            arguments: expressions(program)
                .expression_handles(call.arguments)
                .iter()
                .map(|argument| lower(*argument))
                .collect(),
        },
        other => {
            let _ = other;
            CrashPredicateExpression::Opaque(program.expression_table.display_name(expression))
        }
    }
}

/*
    One completed private fixed point, shared by direct invocation attachment and
    universal callable refinement. Absence is unresolved; a present empty row is
    a proved negative guarantee. These implementation rows never enter authored
    contract fingerprints.
*/
pub(crate) struct CompletedBodyCrashSummaries {
    rows: Vec<(SymbolHandle, Vec<SummaryCrashBucket>)>,
}

impl CompletedBodyCrashSummaries {
    pub(crate) fn checked_buckets(
        &self,
        machine: SymbolHandle,
    ) -> Option<Vec<checked_trees::CrashRouteBucket>> {
        let mut rows = self.rows.iter().filter(|(owner, _)| *owner == machine);
        let (_, buckets) = rows.next()?;
        if rows.next().is_some() {
            return None;
        }
        Some(
            buckets
                .iter()
                .cloned()
                .filter_map(SummaryCrashBucket::into_checked)
                .collect(),
        )
    }
}

/*
    Materialize direct invocation-specific crash refinement while the typed
    expressions are still available. Selection uses a published ceiling when
    one exists and a conservative monotone checked-body summary for same-unit
    private machines. Recursive components close over their finite cause/scope
    buckets; a dependency outside the recognized local/capsule graph keeps its
    caller unexamined. The retained rows are entirely checked data: downstream
    propagation can distinguish a proved-crash-free call from an unexamined call
    without reopening source trees.
*/
pub(super) fn attach_checked_crash_calls(
    program: &dyn ::validation::CheckingInput,
    operators: &checked_trees::CheckedOperatorFacts,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
    semantic: &facts::FactPlan,
    flow: &checked_trees::FlowFacts,
    content_conservation: &[validation::ContentConservationSourcePlan],
    crash_capsules: &[checked_trees::CrashContractCapsule],
    plans: &mut [checked_trees::MachineContractPlan],
    summary_graph: &PreparedCrashSummaryGraph,
) -> CompletedBodyCrashSummaries {
    let inferred_body_summaries = CompletedBodyCrashSummaries {
        rows: infer_private_body_summaries(
            program,
            operators,
            exact_integer_casts,
            semantic,
            flow,
            content_conservation,
            crash_capsules,
            plans,
            summary_graph,
        ),
    };
    let mut plan_indices = symbols::SymbolKeyMap::<SymbolHandle, usize>::default();
    let mut plan_entry_parameter_names = Vec::with_capacity(plans.len());
    for (index, plan) in plans.iter().enumerate() {
        plan_indices.entry(plan.machine).or_insert(index);
        let entry_names = program
            .machine_by_symbol(plan.machine)
            .and_then(|machine| program.machine_states(machine).first())
            .map(|entry| declared_parameter_names(program.state_parameters(entry)))
            .unwrap_or_default();
        plan_entry_parameter_names.push(entry_names);
    }
    let mut capsule_by_target =
        symbols::SymbolKeyMap::<CapsuleTargetKey, &checked_trees::CrashContractCapsule>::default();
    for capsule in crash_capsules {
        capsule_by_target
            .entry(capsule_target_key(capsule))
            .or_insert(capsule);
    }
    let mut summary_by_machine =
        symbols::SymbolKeyMap::<SymbolHandle, &[SummaryCrashBucket]>::default();
    for (machine, summary) in &inferred_body_summaries.rows {
        summary_by_machine
            .entry(*machine)
            .or_insert(summary.as_slice());
    }
    let state_flows: Vec<&checked_trees::FlowStateFact> = flow
        .control
        .states
        .iter()
        .map(|(_, state_flow)| state_flow)
        .collect();
    let per_state_calls: Vec<Vec<(SymbolHandle, checked_trees::CheckedCrashCallSite)>> =
        validation::checking_thread_pool().install(|| {
            use rayon::prelude::*;
            state_flows
                .par_iter()
                .map(|&state_flow| {
                    let mut state_calls = Vec::new();
                    for call_flow in flow.control.calls.span_or_empty(state_flow.calls) {
                        let Some(call_site) = validation::find_call_site(
                            program,
                            state_flow.machine_symbol,
                            state_flow.state_symbol,
                            call_flow.statement_index,
                            call_flow.call_ordinal,
                        ) else {
                            continue;
                        };
                        let public_target = summary_graph
                            .requirement_contracts
                            .for_call(
                                state_flow.machine_symbol,
                                state_flow.state_symbol,
                                call_flow.statement_index,
                                call_flow.call_ordinal,
                            )
                            .map(|selected| {
                                (selected.capsule.target_machine(), selected.capsule.target_state())
                            });
                        let Some((target_machine_symbol, target_state_symbol)) = public_target
                            .or_else(|| {
                                private_summaries::selected_invocation_target(
                                    program,
                                    call_site,
                                    call_flow.target_symbol,
                                )
                            })
                        else {
                            continue;
                        };
                        let local_target = program.machine_by_symbol(target_machine_symbol);
                        let local_plan_index = plan_indices.get(&target_machine_symbol).copied();
                        let local_plan = local_plan_index.and_then(|index| plans.get(index));
                        let (
                            target_parameters,
                            target_parameter_names_owned,
                            target_parameter_names_ref,
                            target_routes,
                            target_contract_fingerprint,
                            target_contract_commitment,
                        ) = if let (Some(target_machine), Some(target_plan), Some(plan_index)) =
                            (local_target, local_plan, local_plan_index)
                        {
                            let Some(target_state) =
                                program.state_in_machine(target_machine, target_state_symbol)
                            else {
                                continue;
                            };
                            let target_routes = if target_machine.target_family_interface {
                                let Some(&summary) = summary_by_machine.get(&target_machine_symbol)
                                else {
                                    continue;
                                };
                                SelectedTargetCrashRoutes::Private(summary)
                            } else if !target_plan.crash.published().is_empty() {
                                SelectedTargetCrashRoutes::Published {
                                    buckets: target_plan.crash.published(),
                                    contracts: program.machine_contracts(target_machine),
                                }
                            } else if target_machine.supply_mode
                                == language_semantics::MachineSupplyMode::CheckedBody
                            {
                                let Some(&summary) = summary_by_machine.get(&target_machine_symbol)
                                else {
                                    continue;
                                };
                                SelectedTargetCrashRoutes::Private(summary)
                            } else {
                                SelectedTargetCrashRoutes::Empty
                            };
                            (
                                program.state_parameters(target_state),
                                Vec::new(),
                                Some(plan_entry_parameter_names[plan_index].as_slice()),
                                target_routes,
                                target_plan.report_fingerprint,
                                target_plan.commitment,
                            )
                        } else {
                            let selected = summary_graph.requirement_contracts.for_call(
                                state_flow.machine_symbol,
                                state_flow.state_symbol,
                                call_flow.statement_index,
                                call_flow.call_ordinal,
                            );
                            let (capsule, signature) = if let Some(selected) = selected {
                                (&selected.capsule, &selected.source.signature)
                            } else {
                                let Some(&capsule) = capsule_by_target
                                    .get(&(target_machine_symbol, target_state_symbol, call_site.runtime_call().map(|binding| binding.contract)))
                                else {
                                    continue;
                                };
                                let Some(signature) = requirement_signature(
                                    program,
                                    target_machine_symbol,
                                    target_state_symbol,
                                ) else {
                                    continue;
                                };
                                (capsule, signature)
                            };
                            // effects.md, "Static binder contract dependencies": a
                            // requirement that publishes no crash route defers to
                            // the selection (`crashes none` admits only crash-free
                            // members, so their routes are empty), and an
                            // application "receives ... the selected published
                            // routes". Where the call names its realization, the
                            // call keeps the requirement's identity and contract
                            // and takes that member's routes over its formals.
                            let deferred = capsule
                                .published_buckets()
                                .is_empty()
                                .then(|| call_site.static_requirement_dispatch())
                                .flatten()
                                .and_then(|dispatch| {
                                    let machine =
                                        program.machine_by_symbol(dispatch.realization_machine)?;
                                    let state = program
                                        .state_in_machine(machine, dispatch.realization_state)?;
                                    let index =
                                        *plan_indices.get(&dispatch.realization_machine)?;
                                    let plan = plans.get(index)?;
                                    let routes = if !machine.target_family_interface
                                        && !plan.crash.published().is_empty()
                                    {
                                        SelectedTargetCrashRoutes::Published {
                                            buckets: plan.crash.published(),
                                            contracts: program.machine_contracts(machine),
                                        }
                                    } else if machine.target_family_interface
                                        || machine.supply_mode
                                            == language_semantics::MachineSupplyMode::CheckedBody
                                    {
                                        SelectedTargetCrashRoutes::Private(
                                            summary_by_machine.get(&dispatch.realization_machine)?,
                                        )
                                    } else {
                                        SelectedTargetCrashRoutes::Empty
                                    };
                                    Some((
                                        program.state_parameters(state),
                                        plan_entry_parameter_names[index].as_slice(),
                                        routes,
                                    ))
                                });
                            if let Some((member_parameters, member_names, routes)) = deferred {
                                (
                                    member_parameters,
                                    Vec::new(),
                                    Some(member_names),
                                    routes,
                                    capsule.target_contract_report_fingerprint(),
                                    capsule.target_contract_commitment(),
                                )
                            } else {
                            let parameters = program.state_signature_parameters(signature);
                            (
                                parameters,
                                declared_parameter_names(parameters),
                                None,
                                if capsule.published_buckets().is_empty() {
                                    SelectedTargetCrashRoutes::Empty
                                } else {
                                    SelectedTargetCrashRoutes::Published {
                                        buckets: capsule.published_buckets(),
                                        contracts: program.state_signature_contracts(signature),
                                    }
                                },
                                capsule.target_contract_report_fingerprint(),
                                capsule.target_contract_commitment(),
                            )
                            }
                        };
                        if call_site.is_internal_state_transfer(program, state_flow.machine_symbol)
                        {
                            continue;
                        }
                        let target_parameter_names = target_parameter_names_ref
                            .unwrap_or(target_parameter_names_owned.as_slice());
                        let arguments = arguments_aligned_to_formals(
                            program,
                            call_flow.target_symbol,
                            target_parameters.len(),
                            validation::call_site_argument_expressions(program, &call_site),
                        );
                        let mut surviving_summary = match target_routes {
                            SelectedTargetCrashRoutes::Published { buckets, contracts } => {
                                refine_published_crash_routes(
                                    program,
                                    operators,
                                    exact_integer_casts,
                                    semantic,
                                    flow,
                                    state_flow,
                                    call_flow,
                                    &call_site,
                                    target_state_symbol,
                                    target_parameters,
                                    target_parameter_names,
                                    buckets,
                                    contracts,
                                    content_conservation,
                                )
                            }
                            SelectedTargetCrashRoutes::Private(summary) => {
                                let substitution = call_argument_substitution(
                                    program,
                                    operators,
                                    semantic,
                                    flow,
                                    state_flow,
                                    call_flow,
                                    target_parameters,
                                    arguments,
                                    exact_integer_casts,
                                );
                                normalize_summary_buckets(
                                    summary
                                        .iter()
                                        .map(|bucket| bucket.substitute(&substitution))
                                        .collect(),
                                )
                            }
                            SelectedTargetCrashRoutes::Empty => Vec::new(),
                        };
                        let entry_conditions = entry_paths::EntryConditions::new(program, semantic, flow);
                        let path = entry_conditions.for_machine(state_flow.machine_symbol).at_call(call_flow);
                        surviving_summary = surviving_summary.into_iter().map(|bucket| bucket.on_path(&path)).collect();
                        if let Some(caller) = program.machine_by_symbol(state_flow.machine_symbol) {
                            entry_equations::refine(
                                program,
                                operators,
                                caller,
                                &mut surviving_summary,
                            );
                        }
                        let surviving_buckets = surviving_summary
                            .into_iter()
                            .filter_map(SummaryCrashBucket::into_checked)
                            .collect::<Vec<_>>();
                        state_calls.push((
                            state_flow.machine_symbol,
                            checked_trees::CheckedCrashCallSite::new_with_commitment(
                                checked_trees::CrashCallSiteLocation::new(
                                    state_flow.state_symbol,
                                    u32::try_from(call_flow.statement_index).expect(
                                        "statement ordinal exceeds checked crash-call identity range",
                                    ),
                                    u32::try_from(call_flow.call_ordinal).expect(
                                        "call ordinal exceeds checked crash-call identity range",
                                    ),
                                ),
                                target_machine_symbol,
                                target_state_symbol,
                                target_contract_fingerprint,
                                target_contract_commitment,
                                surviving_buckets,
                            ),
                        ));
                    }
                    state_calls
                })
                .collect()
        });
    let mut calls_by_caller =
        symbols::SymbolKeyMap::<SymbolHandle, Vec<checked_trees::CheckedCrashCallSite>>::default();
    for state_calls in per_state_calls {
        for (caller_machine, site) in state_calls {
            calls_by_caller
                .entry(caller_machine)
                .or_default()
                .push(site);
        }
    }

    for plan in plans {
        let checked_calls = calls_by_caller.remove(&plan.machine).unwrap_or_default();
        let body_summary = program.machine_by_symbol(plan.machine).and_then(|machine| {
            summary_by_machine.get(&plan.machine).map(|buckets| {
                checked_trees::CheckedBodyCrashSummary::new(
                    machine.symbol,
                    machine.states,
                    buckets
                        .iter()
                        .cloned()
                        .filter_map(SummaryCrashBucket::into_checked)
                        .collect(),
                )
            })
        });
        plan.crash = plan
            .crash
            .clone()
            .with_checked_calls(checked_calls)
            .expect("one checked crash-call record occupies each invocation coordinate")
            .with_checked_body_summary(body_summary);
    }
    inferred_body_summaries
}

pub(crate) fn infer_checked_machine_crash_causes(
    program: &dyn ::validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    machine: SymbolHandle,
) -> Option<Vec<checked_trees::CrashCause>> {
    let mut machines = program
        .machines()
        .iter()
        .filter(|candidate| candidate.symbol == machine);
    let source = machines.next()?;
    if machines.next().is_some() {
        return None;
    }
    let mut plans = facts
        .contract_plans
        .machines()
        .iter()
        .filter(|plan| plan.machine == machine);
    let plan = plans.next()?;
    if plans.next().is_some() {
        return None;
    }
    let summary = plan.crash.checked_body_summary(machine, source.states)?;
    Some(body_summary_causes(summary))
}

pub(crate) fn infer_checked_crash_causes(
    program: &dyn ::validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
) -> Vec<(SymbolHandle, Vec<checked_trees::CrashCause>)> {
    let key = |symbol: SymbolHandle| (symbol.arena_index(), symbol.generation());
    let mut machines = program.machines().iter().collect::<Vec<_>>();
    machines.sort_unstable_by_key(|machine| key(machine.symbol));
    let mut plans = facts.contract_plans.machines().iter().collect::<Vec<_>>();
    plans.sort_unstable_by_key(|plan| key(plan.machine));
    plans
        .iter()
        .enumerate()
        .filter_map(|(position, plan)| {
            if position > 0 && plans[position - 1].machine == plan.machine
                || plans
                    .get(position + 1)
                    .is_some_and(|next| next.machine == plan.machine)
            {
                return None;
            }
            let position = machines
                .binary_search_by_key(&key(plan.machine), |machine| key(machine.symbol))
                .ok()?;
            let machine = machines[position];
            if position > 0 && machines[position - 1].symbol == machine.symbol
                || machines
                    .get(position + 1)
                    .is_some_and(|next| next.symbol == machine.symbol)
            {
                return None;
            }
            let summary = plan
                .crash
                .checked_body_summary(machine.symbol, machine.states)?;
            Some((machine.symbol, body_summary_causes(summary)))
        })
        .collect()
}

fn body_summary_causes(
    summary: &checked_trees::CheckedBodyCrashSummary,
) -> Vec<checked_trees::CrashCause> {
    let mut causes = summary
        .buckets()
        .iter()
        .map(checked_trees::CrashRouteBucket::cause)
        .collect::<Vec<_>>();
    causes.sort_unstable();
    causes.dedup();
    causes
}

/*
    The member name a produced entry operand carries. A case payload field
    keeps its `Variant::field` qualification — the retained `case_variant`
    spelling when present, else the field's declaring variant — so two
    same-named payload fields of different cases cannot collide in the caller
    namespace.
*/
pub(super) fn member_entry_name(
    program: &dyn ::validation::CheckingInput,
    member: &typed_trees::expression::TableMemberExpression,
    field_symbol: symbols::SymbolHandle,
) -> String {
    field_entry_name(
        program,
        field_symbol,
        member.member.as_str(),
        member.case_variant.as_ref().map(|case| case.as_str()),
    )
}

pub(super) fn field_entry_name(
    program: &dyn ::validation::CheckingInput,
    field_symbol: symbols::SymbolHandle,
    field_name: &str,
    authored_case: Option<&str>,
) -> String {
    let Some(variant) = facts::payload_variant_for_field(program, field_symbol) else {
        return field_name.to_owned();
    };
    let case = authored_case.unwrap_or_else(|| program.symbols.name(variant));
    format!("{case}::{field_name}")
}
