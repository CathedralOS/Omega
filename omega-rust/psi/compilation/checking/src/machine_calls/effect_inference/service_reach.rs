//! Conservative service rows and finite nominal callback dependencies share
//! the checked call-component fixed point. Flattening a binder's bound loses
//! the difference between `reach(Step)` and `reach(Step) + Console`; subtracting
//! that bound afterwards also loses a real Console contribution. Retain fixed
//! services and exact nominal binder symbols separately from the beginning.
//! The ordinary rows still use requirement ceilings for universal body checks.

use arena::HandleSpan;
use language_semantics::{ServiceReachId, ServiceReachInterface};
use symbols::SymbolHandle;
use typed_trees::declaration_index::CallTargetIndex;

use flow_effects::{
    CallServiceReachInference, InstallationReachRequirement, MachineServiceReachInference,
    OperationalPlan, ServiceReachInferencePlan, StateServiceReachInference,
};

#[derive(Debug, Clone)]
struct MachineReachWork {
    symbol: SymbolHandle,
    family_members: Vec<FamilyMemberReach>,
    declared: Vec<ServiceReachId>,
    published: Vec<ServiceReachId>,
    checked_body: bool,
    uses_published: bool,
    concrete_declared: Vec<ServiceReachId>,
    direct: Vec<ServiceReachId>,
    transitive: Vec<ServiceReachId>,
    concrete_direct: Vec<ServiceReachId>,
    concrete_transitive: Vec<ServiceReachId>,
    concrete_effective: Vec<ServiceReachId>,
    unresolved_installation_reaches: Vec<InstallationReachRequirement>,
    dependency_direct: DependencyWork,
    dependency: DependencyWork,
    cleanup_contribution: Vec<ServiceReachId>,
    calls: Vec<MachineReachCall>,
}

#[derive(Debug, Clone)]
struct FamilyMemberReach {
    machine: SymbolHandle,
    /// Member telescopes have independent symbols under one family signature.
    parameters: Vec<(SymbolHandle, SymbolHandle)>,
}

#[derive(Debug, Clone)]
struct MachineReachCall {
    target: SymbolHandle,
    direct: DirectServiceReach,
    bindings: HandleSpan<flow_effects::StaticMachineCallBinding>,
    cleanup: CleanupContribution,
}

#[derive(Debug, Clone, Copy)]
enum CleanupContribution {
    // Selected callbacks retain their own contractual substitution. Their
    // bodies cannot add a second fixed contribution to the enclosing schema.
    Callback,
    Ordinary,
    SelectedCleanup,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct DependencyWork {
    concrete: Vec<ServiceReachId>,
    parameters: Vec<SymbolHandle>,
}

#[derive(Debug, Clone, Default)]
struct DirectServiceReach {
    services: Vec<ServiceReachId>,
    boundary_services: Vec<ServiceReachId>,
    concrete_services: Vec<ServiceReachId>,
    unresolved_installation_reaches: Vec<InstallationReachRequirement>,
}

// Reach inference repeats the same symbol scans at three scales: the
// operational summary per machine, the work row per call target inside the
// fixed point, and the whole program's boundary states / machine-parameter
// and trait signatures per call. The call-target index is owned by the
// program itself: `TypedTrees::with_call_target_index` rebuilds
// `CallTargetIndex` when any of the arena revisions it reads move, so no
// caller-side freshness key is needed and a dropped program can never serve
// a replacement its index.

fn with_call_target_index<R>(
    program: &dyn crate::CheckingInput,
    run: impl FnOnce(&CallTargetIndex) -> R,
) -> R {
    program.with_call_target_index(run)
}

pub fn infer_service_reaches(
    program: &dyn crate::CheckingInput,
    operational: &OperationalPlan,
) -> ServiceReachInferencePlan {
    super::prepared_effects::service_reach_plan(program, operational)
}

pub(crate) fn infer_service_reaches_uncached(
    program: &dyn crate::CheckingInput,
    operational: &OperationalPlan,
) -> ServiceReachInferencePlan {
    let mut work = Vec::new();
    for machine in program.machines() {
        let family_members = super::family_interface_members(program, machine)
            .iter()
            .map(|member| FamilyMemberReach {
                machine: member.machine,
                parameters: program
                    .machine_by_symbol(member.machine)
                    .map(|member| {
                        program
                            .machine_type_parameters(member)
                            .iter()
                            .zip(program.machine_type_parameters(machine))
                            .map(|(member, interface)| (member.symbol, interface.symbol))
                            .collect()
                    })
                    .unwrap_or_default(),
            })
            .collect::<Vec<_>>();
        let checked_body =
            machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody;
        let mut published = if family_members.is_empty() {
            program
                .service_reach_rows
                .services(machine.service_reach_row)
                .to_vec()
        } else {
            // The prototype's row has no authority over other target bodies.
            // The complete member union supplies this interface below.
            Vec::new()
        };
        let mut concrete_declared = if machine.service_reach_is_installation_bound {
            Vec::new()
        } else {
            published.clone()
        };
        // A top-level boundary's authored invocations survive replacement of
        // its abstract bound. Keep them in both the conservative declaration
        // and its concrete basis, not in inferred body calls: the declaration
        // has no body and the original row still bounds provider selection.
        let mut invoked = DirectServiceReach::default();
        if machine.supply_mode.is_boundary_declaration() {
            extend_machine_invoked_binding_services(program, machine, &mut invoked);
            extend_service_set(&mut published, &invoked.services);
            extend_service_set(&mut concrete_declared, &invoked.concrete_services);
        }
        let mut calls = Vec::new();
        let mut direct = Vec::new();
        let mut concrete_direct = Vec::new();
        let mut dependency_direct = DependencyWork {
            concrete: published.clone(),
            ..Default::default()
        };
        if let Some(summary) =
            with_operational_index(operational, |index| index.get(&machine.symbol).copied())
                .map(|index| &operational.machines()[index])
        {
            for state in operational.states.span_or_empty(summary.states) {
                for call in operational.calls.span_or_empty(state.calls) {
                    let call_reach = direct_service_reach_for_operational_call(program, call);
                    extend_service_set(&mut direct, &call_reach.services);
                    extend_service_set(&mut concrete_direct, &call_reach.concrete_services);
                    if checked_body
                        && !call.static_machine_parameter.is_valid()
                        && is_nominal_parameter(program, call.target_state_symbol)
                    {
                        extend_parameter_set(
                            &mut dependency_direct.parameters,
                            &[call.target_state_symbol],
                        );
                    } else if checked_body {
                        extend_service_set(&mut dependency_direct.concrete, &call_reach.services);
                    }
                    calls.push(MachineReachCall {
                        target: call.target_machine_symbol,
                        direct: call_reach,
                        bindings: call.static_machine_bindings,
                        cleanup: if call.static_machine_parameter.is_valid()
                            || is_nominal_parameter(program, call.target_state_symbol)
                        {
                            CleanupContribution::Callback
                        } else {
                            CleanupContribution::Ordinary
                        },
                    });
                }
            }
        }
        for (_, dependency) in operational
            .cleanup_dependencies
            .iter()
            .filter(|(_, dependency)| dependency.caller_machine == machine.symbol)
        {
            // Internal ownership-edge calls propagate the target's checked
            // closure; they do not manufacture a direct boundary invocation.
            calls.push(MachineReachCall {
                target: dependency.target_machine,
                direct: DirectServiceReach::default(),
                bindings: HandleSpan::empty(),
                cleanup: CleanupContribution::SelectedCleanup,
            });
        }
        work.push(MachineReachWork {
            symbol: machine.symbol,
            uses_published: family_members.is_empty()
                && (machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
                    || machine.is_public
                    || program
                        .authored_service_reach_rows_for(machine.symbol)
                        .next()
                        .is_some()
                    || !program
                        .service_reach_rows
                        .services(machine.service_reach_row)
                        .is_empty()),
            family_members,
            declared: published.clone(),
            published: published.clone(),
            checked_body,
            concrete_declared: concrete_declared.clone(),
            direct: direct.clone(),
            transitive: direct,
            concrete_direct: concrete_direct.clone(),
            concrete_transitive: concrete_direct,
            concrete_effective: concrete_declared,
            unresolved_installation_reaches: machine
                .service_reach_is_installation_bound
                .then_some(InstallationReachRequirement {
                    requirement: machine.symbol,
                    upper_bound: machine.service_reach_row,
                })
                .into_iter()
                .chain(invoked.unresolved_installation_reaches)
                .chain(
                    calls.iter().flat_map(|call| {
                        call.direct.unresolved_installation_reaches.iter().copied()
                    }),
                )
                .collect(),
            cleanup_contribution: Vec::new(),
            dependency: dependency_direct.clone(),
            dependency_direct,
            calls,
        });
        normalize_installation_reaches(
            &mut work
                .last_mut()
                .expect("machine work")
                .unresolved_installation_reaches,
        );
    }

    let work_index = work
        .iter()
        .enumerate()
        .map(|(index, machine)| (machine.symbol, index))
        .collect::<symbols::SymbolKeyMap<_, _>>();
    loop {
        let previous = work
            .iter()
            .map(|machine| {
                (
                    machine.transitive.clone(),
                    machine.concrete_transitive.clone(),
                    machine.unresolved_installation_reaches.clone(),
                    machine.published.clone(),
                    machine.concrete_effective.clone(),
                    machine.dependency.clone(),
                    machine.cleanup_contribution.clone(),
                )
            })
            .collect::<Vec<_>>();
        for machine_index in 0..work.len() {
            let mut transitive = work[machine_index].direct.clone();
            let mut concrete_transitive = work[machine_index].concrete_direct.clone();
            let mut unresolved = work[machine_index].unresolved_installation_reaches.clone();
            let mut dependency = work[machine_index].dependency_direct.clone();
            let mut cleanup_contribution = Vec::new();
            for member in &work[machine_index].family_members {
                let Some(target) = work_index.get(&member.machine).map(|index| &work[*index])
                else {
                    continue;
                };
                extend_service_set(&mut transitive, effective_services(target));
                extend_service_set(
                    &mut concrete_transitive,
                    concrete_effective_services(target),
                );
                extend_installation_reaches(
                    &mut unresolved,
                    &target.unresolved_installation_reaches,
                );
                extend_service_set(&mut dependency.concrete, &target.dependency.concrete);
                for parameter in &target.dependency.parameters {
                    let interface_parameter = member
                        .parameters
                        .iter()
                        .find(|(member_parameter, _)| member_parameter == parameter)
                        .map_or(*parameter, |(_, interface_parameter)| *interface_parameter);
                    extend_parameter_set(&mut dependency.parameters, &[interface_parameter]);
                }
                extend_service_set(&mut cleanup_contribution, &target.cleanup_contribution);
            }
            for call in &work[machine_index].calls {
                extend_service_set(&mut transitive, &call.direct.services);
                extend_service_set(&mut concrete_transitive, &call.direct.concrete_services);
                if let Some(target) = work_index.get(&call.target).map(|index| &work[*index]) {
                    if work[machine_index].checked_body {
                        substitute_dependency(
                            program,
                            operational,
                            &work,
                            &target.dependency,
                            call.bindings,
                            &mut dependency,
                        );
                        // Keep the instantiated custody contribution beside
                        // the original template's fixed/binder expression.
                        // This uses the same call-component fixed point and
                        // the hook's contract, never emitted-body subtraction.
                        match call.cleanup {
                            CleanupContribution::Callback => {}
                            CleanupContribution::Ordinary => extend_service_set(
                                &mut cleanup_contribution,
                                &target.cleanup_contribution,
                            ),
                            CleanupContribution::SelectedCleanup => extend_service_set(
                                &mut cleanup_contribution,
                                effective_services(target),
                            ),
                        }
                    }
                    extend_service_set(&mut transitive, effective_services(target));
                    extend_service_set(
                        &mut concrete_transitive,
                        concrete_effective_services(target),
                    );
                    extend_installation_reaches(
                        &mut unresolved,
                        &target.unresolved_installation_reaches,
                    );
                }
            }
            work[machine_index].transitive = transitive;
            work[machine_index].concrete_transitive = concrete_transitive;
            work[machine_index].unresolved_installation_reaches = unresolved;
            work[machine_index].dependency = dependency;
            work[machine_index].cleanup_contribution = cleanup_contribution;
            let machine = &mut work[machine_index];
            // A checked body's declaration contributes to its row; it does not
            // cap calls through private helpers or exported wrappers. Pinned
            // requirements keep their declared ceiling independently of bodies.
            if machine.checked_body {
                machine.published = machine.declared.clone();
                extend_service_set(&mut machine.published, &machine.transitive);
            }
            machine.concrete_effective = machine.concrete_declared.clone();
            if machine.checked_body || !machine.family_members.is_empty() {
                extend_service_set(
                    &mut machine.concrete_effective,
                    &machine.concrete_transitive,
                );
            }
        }
        if work.iter().zip(&previous).all(|(machine, previous)| {
            machine.transitive == previous.0
                && machine.concrete_transitive == previous.1
                && machine.unresolved_installation_reaches == previous.2
                && machine.published == previous.3
                && machine.concrete_effective == previous.4
                && machine.dependency == previous.5
                && machine.cleanup_contribution == previous.6
        }) {
            break;
        }
    }

    // A virtual head publishes its completed portable union once. Authored
    // members keep their own ceilings and concrete dependency summaries.
    for machine in &mut work {
        if !machine.family_members.is_empty() {
            machine.published = machine.transitive.clone();
            machine.uses_published = true;
        }
    }

    let mut plan = ServiceReachInferencePlan {
        rows: program.service_reach_rows.clone(),
        ..Default::default()
    };
    for machine in program.machines() {
        let Some(machine_work) = work_index.get(&machine.symbol).map(|index| &work[*index]) else {
            continue;
        };
        let mut states = HandleSpan::empty();
        if let Some(machine_summary) =
            with_operational_index(operational, |index| index.get(&machine.symbol).copied())
                .map(|index| &operational.machines()[index])
        {
            for state_summary in operational.states.span_or_empty(machine_summary.states) {
                let mut calls = HandleSpan::empty();
                let mut state_direct = Vec::new();
                let mut state_transitive = Vec::new();
                let mut state_concrete_direct = Vec::new();
                let mut state_concrete_transitive = Vec::new();
                let mut state_unresolved = Vec::new();
                for call_summary in operational.calls.span_or_empty(state_summary.calls) {
                    let call_direct =
                        direct_service_reach_for_operational_call(program, call_summary);
                    let mut call_transitive = call_direct.services.clone();
                    let mut call_concrete_transitive = call_direct.concrete_services.clone();
                    let mut call_unresolved = call_direct.unresolved_installation_reaches;
                    if let Some(target) = work_index
                        .get(&call_summary.target_machine_symbol)
                        .map(|index| &work[*index])
                    {
                        extend_service_set(&mut call_transitive, effective_services(target));
                        extend_service_set(
                            &mut call_concrete_transitive,
                            concrete_effective_services(target),
                        );
                        extend_installation_reaches(
                            &mut call_unresolved,
                            &target.unresolved_installation_reaches,
                        );
                    }
                    extend_service_set(&mut state_direct, &call_direct.services);
                    extend_service_set(&mut state_transitive, &call_transitive);
                    extend_service_set(&mut state_concrete_direct, &call_direct.concrete_services);
                    extend_service_set(&mut state_concrete_transitive, &call_concrete_transitive);
                    extend_installation_reaches(&mut state_unresolved, &call_unresolved);
                    let inferred_direct = plan.rows.intern(call_direct.services);
                    let inferred_transitive = plan.rows.intern(call_transitive);
                    let concrete_direct = plan.rows.intern(call_direct.concrete_services);
                    let concrete_transitive = plan.rows.intern(call_concrete_transitive);
                    plan.calls.append_to_span(
                        &mut calls,
                        CallServiceReachInference {
                            statement_index: call_summary.statement_index,
                            call_ordinal: call_summary.call_ordinal,
                            target_state: call_summary.target_state_symbol,
                            target_machine: call_summary.target_machine_symbol,
                            inferred_direct,
                            inferred_transitive,
                            concrete_direct,
                            concrete_transitive,
                            unresolved_installation_reaches: call_unresolved,
                        },
                    );
                }
                for (_, dependency) in
                    operational
                        .cleanup_dependencies
                        .iter()
                        .filter(|(_, dependency)| {
                            dependency.caller_machine == machine.symbol
                                && dependency.caller_state == state_summary.symbol
                        })
                {
                    if let Some(target) = work_index
                        .get(&dependency.target_machine)
                        .map(|index| &work[*index])
                    {
                        extend_service_set(&mut state_transitive, effective_services(target));
                        extend_service_set(
                            &mut state_concrete_transitive,
                            concrete_effective_services(target),
                        );
                        extend_installation_reaches(
                            &mut state_unresolved,
                            &target.unresolved_installation_reaches,
                        );
                    }
                }
                let inferred_direct = plan.rows.intern(state_direct);
                let inferred_transitive = plan.rows.intern(state_transitive);
                let concrete_direct = plan.rows.intern(state_concrete_direct);
                let concrete_transitive = plan.rows.intern(state_concrete_transitive);
                plan.states.append_to_span(
                    &mut states,
                    StateServiceReachInference {
                        state: state_summary.symbol,
                        inferred_direct,
                        inferred_transitive,
                        concrete_direct,
                        concrete_transitive,
                        unresolved_installation_reaches: state_unresolved,
                        calls,
                    },
                );
            }
        }

        let cleanup_contribution = plan.rows.intern(machine_work.cleanup_contribution.clone());
        let published = plan.rows.intern(machine_work.published.clone());
        let inferred_direct = plan.rows.intern(machine_work.direct.clone());
        let inferred_transitive = plan.rows.intern(machine_work.transitive.clone());
        let concrete_transitive = plan.rows.intern(machine_work.concrete_transitive.clone());
        let effective = plan.rows.intern(effective_services(machine_work).to_vec());
        let concrete_effective = plan
            .rows
            .intern(concrete_effective_services(machine_work).to_vec());
        let mut parameters = HandleSpan::empty();
        for parameter in &machine_work.dependency.parameters {
            plan.dependency_parameters
                .append_to_span(&mut parameters, *parameter);
        }
        let dependency = flow_effects::ServiceReachDependency {
            concrete: plan.rows.intern(machine_work.dependency.concrete.clone()),
            parameters,
        };
        plan.machines.append_to_span(
            &mut plan.root_machines,
            MachineServiceReachInference {
                machine: machine.symbol,
                dependency,
                cleanup_contribution,
                interface: if machine_work.uses_published {
                    ServiceReachInterface::PublishedCeiling(published)
                } else {
                    ServiceReachInterface::InternalInferred
                },
                published,
                inferred_direct,
                inferred_transitive,
                concrete_transitive,
                concrete_effective,
                unresolved_installation_reaches: machine_work
                    .unresolved_installation_reaches
                    .clone(),
                effective,
                states,
            },
        );
    }
    plan
}

fn effective_services(machine: &MachineReachWork) -> &[ServiceReachId] {
    if machine.uses_published {
        &machine.published
    } else {
        &machine.transitive
    }
}

fn concrete_effective_services(machine: &MachineReachWork) -> &[ServiceReachId] {
    &machine.concrete_effective
}

fn with_operational_index<R>(
    operational: &OperationalPlan,
    run: impl FnOnce(&symbols::SymbolKeyMap<SymbolHandle, usize>) -> R,
) -> R {
    // The machine-symbol index is owned by the plan itself; the arena
    // revision inside the slot is the freshness key.
    operational.with_machine_index(run)
}

pub(crate) fn required_boundary_services(
    program: &dyn crate::CheckingInput,
    operational: &OperationalPlan,
    machine: SymbolHandle,
) -> Vec<ServiceReachId> {
    let mut required = Vec::new();
    if let Some(summary) = with_operational_index(operational, |index| index.get(&machine).copied())
        .map(|index| &operational.machines()[index])
    {
        for state in operational.states.span_or_empty(summary.states) {
            for call in operational.calls.span_or_empty(state.calls) {
                let reach = direct_service_reach_for_operational_call(program, call);
                extend_service_set(&mut required, &reach.boundary_services);
            }
        }
    }
    required
}

fn indexed_contract_view<'program>(
    program: &'program dyn crate::CheckingInput,
    index: &CallTargetIndex,
    contract: &'program typed_trees::data::MachineParameterContract,
) -> Option<typed_trees::data::MachineParameterContractView<'program>> {
    match contract {
        typed_trees::data::MachineParameterContract::RequirementIdentity => None,
        typed_trees::data::MachineParameterContract::Structural(signature) => Some(
            typed_trees::data::MachineParameterContractView::Structural(signature),
        ),
        typed_trees::data::MachineParameterContract::Nominal {
            trait_definition,
            requirement,
            instantiated,
            ..
        } => {
            let trait_definition =
                &program.traits()[*index.trait_by_symbol.get(trait_definition)? as usize];
            let declared = &program.trait_machine_signatures(trait_definition)
                [index.trait_signature.get(requirement)?.1 as usize];
            let requirement = instantiated.as_deref().unwrap_or(declared);
            Some(typed_trees::data::MachineParameterContractView::Nominal {
                trait_definition,
                requirement,
            })
        }
    }
}

fn retained_contract_view<'program>(
    program: &'program dyn crate::CheckingInput,
    index: &CallTargetIndex,
    symbol: SymbolHandle,
) -> Option<typed_trees::data::MachineParameterContractView<'program>> {
    if !symbol.is_valid() {
        return None;
    }
    let handle = index.parameter_contract.get(&symbol)?;
    let parameter = program.data_type_parameters.get(*handle);
    let typed_trees::data::TypeParameterKind::Machine { contract } = &parameter.kind else {
        return None;
    };
    indexed_contract_view(program, index, contract)
}

fn direct_service_reach_for_operational_call(
    program: &dyn crate::CheckingInput,
    call: &flow_effects::CallOperational,
) -> DirectServiceReach {
    let mut reach = direct_service_reach_for_call(program, call.target_state_symbol);
    if let Some(contract) = with_call_target_index(program, |index| {
        retained_contract_view(program, index, call.static_machine_parameter)
    }) {
        // Selection substitutes nominal reach only. Structural requirements
        // remain fixed even when the selected implementation has a smaller row.
        // A binder call is not a newly authored direct boundary invocation.
        reach.boundary_services.clear();
        if let typed_trees::data::MachineParameterContractView::Structural(signature) = contract {
            extend_signature_service_rows(program, signature, &mut reach);
            record_signature_reach(program, signature, &mut reach);
        }
    }
    reach
}

fn is_nominal_parameter(program: &dyn crate::CheckingInput, parameter: SymbolHandle) -> bool {
    with_call_target_index(program, |index| {
        matches!(
            retained_contract_view(program, index, parameter),
            Some(typed_trees::data::MachineParameterContractView::Nominal { .. })
        )
    })
}

fn extend_parameter_set(destination: &mut Vec<SymbolHandle>, source: &[SymbolHandle]) {
    destination.extend_from_slice(source);
    destination.sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
    destination.dedup();
}

/// Substitute only nominal row variables. Structural arguments contribute their
/// fixed requirement, and opaque selected machines contribute their published
/// bound. Nested static applications consume their own exact binding spans.
fn substitute_dependency(
    program: &dyn crate::CheckingInput,
    operational: &OperationalPlan,
    machines: &[MachineReachWork],
    dependency: &DependencyWork,
    bindings: HandleSpan<flow_effects::StaticMachineCallBinding>,
    result: &mut DependencyWork,
) {
    extend_service_set(&mut result.concrete, &dependency.concrete);
    for parameter in &dependency.parameters {
        let Some(binding) = operational
            .static_machine_bindings
            .span_or_empty(bindings)
            .iter()
            .find(|binding| binding.parameter == *parameter)
        else {
            // Preserve an unresolved identity rather than silently flattening
            // or erasing it. Public capture must rejoin every variable to the
            // declaring callable's nominal parameter scope before publication.
            extend_parameter_set(&mut result.parameters, &[*parameter]);
            continue;
        };
        if is_nominal_parameter(program, binding.selected) {
            extend_parameter_set(&mut result.parameters, &[binding.selected]);
        } else if let Some((selected, _)) =
            crate::declarations::transitions::resolved_transition_target_state(
                program,
                binding.selected,
            )
            && let Some(selected) = machines
                .iter()
                .find(|machine| machine.symbol == selected.symbol)
        {
            substitute_dependency(
                program,
                operational,
                machines,
                &selected.dependency,
                binding.arguments,
                result,
            );
        } else {
            extend_service_set(
                &mut result.concrete,
                &direct_service_reach_for_call(program, binding.selected).services,
            );
        }
    }
}

/// Reconstruct the fixed basis beside one exact boundary requirement's bound.
/// Nominal boundary services and synchronous invocations survive installation
/// substitution, even when they also occur in the requirement's upper bound.
/// Use the same source meaning as call inference; subtraction from its flattened
/// ceiling cannot recover overlapping fixed contributions.
/// Ordinary boundaries need no separate basis: their whole published ceiling
/// is already concrete. This projection must not reinterpret an ordinary
/// callable contract as a newly authored nominal boundary invocation.
pub fn fixed_installation_boundary_service_reach(
    program: &dyn crate::CheckingInput,
    target: SymbolHandle,
) -> Option<Vec<ServiceReachId>> {
    let trait_requirements = program
        .traits()
        .iter()
        .filter(|owner| owner.is_boundary)
        .flat_map(|owner| program.trait_machine_instances(owner))
        .filter(|signature| signature.symbol == target)
        .map(|signature| signature.service_reach_is_installation_bound);
    let machine_requirements = program
        .machines()
        .iter()
        .filter(|machine| {
            // Opaque external supply retains a callable contract even before
            // a target installs its physical binding. The fixed basis belongs
            // to that declaration, not to provider selection.
            !machine.supplies_checked_body()
                && typed_trees::state::state_by_symbol(program.machine_states(machine), target)
                    .is_some()
        })
        .map(|machine| machine.service_reach_is_installation_bound);
    // Legacy operators publish an empty concrete ceiling: their declaration
    // syntax has no installation-bound service row. Recognize the exact
    // requirement here so every consumer uses the same authored envelope.
    let operator_requirements = program
        .operators()
        .iter()
        .filter(|operator| operator.is_boundary && operator.symbol == target)
        .map(|_| false);
    let mut requirements = trait_requirements
        .chain(machine_requirements)
        .chain(operator_requirements);
    let installation_bound = requirements.next()?;
    if requirements.next().is_some() {
        return None;
    }
    if !installation_bound {
        return Some(Vec::new());
    }
    Some(direct_service_reach_for_call(program, target).concrete_services)
}

/// Project an exact trait requirement's declared ceiling, including nominal
/// service and synchronous invocation contributions, without inventing a call.
/// Publication needs the same contract for called and uncalled coordinates.
pub fn boundary_signature_service_reach(
    program: &dyn crate::CheckingInput,
    signature: SymbolHandle,
) -> Option<Vec<ServiceReachId>> {
    let mut declarations = program
        .traits()
        .iter()
        .filter(|owner| owner.is_boundary)
        .flat_map(|owner| program.trait_machine_instances(owner))
        .filter(|declaration| declaration.symbol == signature);
    let signature = declarations.next()?;
    if declarations.next().is_some() {
        return None;
    }
    Some(signature_service_reach(program, signature))
}

/// The same requirement envelope bounds providers and contributes at calls.
/// Invocation permission alone adds nothing; exact synchronous callback
/// contracts remain part of the envelope, including applied requirements.
pub(crate) fn signature_service_reach(
    program: &dyn crate::CheckingInput,
    signature: &typed_trees::signature::StateSignature,
) -> Vec<ServiceReachId> {
    let mut reach = DirectServiceReach::default();
    extend_signature_service_rows(program, signature, &mut reach);
    record_signature_reach(program, signature, &mut reach);
    reach.services
}

fn direct_service_reach_for_call(
    program: &dyn crate::CheckingInput,
    target: SymbolHandle,
) -> DirectServiceReach {
    // Checked calls to an exact intrinsic realization retain the boundary
    // requirement's identity. Infer its nominal reach at the same boundary,
    // even when the realization's own declaration has no authored reach row.
    let intrinsic = crate::exact_compiler_intrinsic_boundary_requirement(program, target);
    // An evaluated-binding leaf is an ordinary bodyless machine, not a
    // boundary declaration. Its satisfied requirement supplies its published
    // reach (foreign bindings, operational coherence), so a call to it
    // propagates that reach like any machine call and is not newly authored
    // direct boundary use. The call plans as a boundary call on the same
    // requirement, so it must also publish the same reach as a field call on
    // that requirement, or the requirement keeps no single boundary plan.
    if intrinsic.is_none()
        && let Some((requirement, _)) =
            crate::exact_external_leaf_boundary_requirement(program, target)
    {
        let mut reach = direct_service_reach_for_call(program, requirement);
        reach.boundary_services.clear();
        return reach;
    }
    let target = intrinsic.map_or(target, |(requirement, _)| requirement);
    let mut reach = DirectServiceReach::default();
    if !target.is_valid() {
        return reach;
    }

    // Inline assembly is an unnameable builtin call whose instruction
    // contract reaches a canonical boundary-service identity. Resolve that
    // identity through the same symbol-backed table as authored `reaches`
    // rows; never consult the lowercase/u64 compatibility catalog here. Each
    // builtin slot names one function, so the target's own slot selects it.
    if let Some(function) = program.symbols.builtin_function_for_symbol(target)
        && symbols::BuiltinFunction::asm_intrinsics().contains(&function)
    {
        if let Some(service_name) = function.asm_intrinsic_service_name()
            && let Some(service) = program.service_reaches.id_for_name(service_name)
        {
            program
                .service_reaches
                .extend_closure(service, &mut reach.services);
            reach.services.sort_by_key(|service| service.0);
            reach.services.dedup();
            reach.concrete_services = reach.services.clone();
            reach.boundary_services = reach.services.clone();
        }
        return reach;
    }

    let parameter_signature = with_call_target_index(program, |index| {
        index
            .parameter_signature
            .get(&target)
            .and_then(|&(machine_index, parameter_index)| {
                let machine = &program.machines()[machine_index as usize];
                let parameter = &program.machine_type_parameters(machine)[parameter_index as usize];
                let typed_trees::data::TypeParameterKind::Machine { contract } = &parameter.kind
                else {
                    return None;
                };
                indexed_contract_view(program, index, contract)
                    .map(typed_trees::data::MachineParameterContractView::signature)
                    .map(|signature| (machine, signature))
            })
    });
    if let Some((_, signature)) = parameter_signature {
        extend_signature_service_rows(program, signature, &mut reach);
        record_signature_reach(program, signature, &mut reach);
        return reach;
    }

    if let Some((trait_definition, signature)) = program.trait_machine_signature(target) {
        // The provider interface identifies the requirement, not an implicit
        // effect. A raw OS interface can expose several portable capabilities,
        // and an empty requirement row remains empty even when it has a provider.
        extend_signature_service_rows(program, signature, &mut reach);
        record_signature_reach(program, signature, &mut reach);
        if trait_definition.is_boundary {
            reach.boundary_services = reach.services.clone();
        }
        return reach;
    }
    if let Some(machine) = with_call_target_index(program, |index| {
        index.boundary_machine_by_state.get(&target).copied()
    })
    .map(|machine_index| &program.machines()[machine_index as usize])
    {
        extend_service_set(
            &mut reach.services,
            program
                .service_reach_rows
                .services(machine.service_reach_row),
        );
        if !machine.service_reach_is_installation_bound {
            reach.concrete_services = reach.services.clone();
        }
        extend_machine_invoked_binding_services(program, machine, &mut reach);
        reach.boundary_services = reach.services.clone();
    }
    reach
}

/// The signature's declared service row joined into reach's nominal set —
/// and its concrete set unless the reach is installation-bound.
fn extend_signature_service_rows(
    program: &dyn crate::CheckingInput,
    signature: &typed_trees::signature::StateSignature,
    reach: &mut DirectServiceReach,
) {
    extend_service_set(
        &mut reach.services,
        program
            .service_reach_rows
            .services(signature.service_reach_row),
    );
    if !signature.service_reach_is_installation_bound {
        extend_service_set(
            &mut reach.concrete_services,
            program
                .service_reach_rows
                .services(signature.service_reach_row),
        );
    }
}

/// The signature's installation reach plus the services its invoked bindings
/// themselves declare.
fn record_signature_reach(
    program: &dyn crate::CheckingInput,
    signature: &typed_trees::signature::StateSignature,
    reach: &mut DirectServiceReach,
) {
    record_installation_reach(signature, reach);
    extend_invoked_binding_services(program, signature, reach);
}

fn record_installation_reach(
    signature: &typed_trees::signature::StateSignature,
    reach: &mut DirectServiceReach,
) {
    if signature.service_reach_is_installation_bound {
        reach
            .unresolved_installation_reaches
            .push(InstallationReachRequirement {
                requirement: signature.symbol,
                upper_bound: signature.service_reach_row,
            });
    }
}

fn extend_installation_reaches(
    destination: &mut Vec<InstallationReachRequirement>,
    source: &[InstallationReachRequirement],
) {
    destination.extend_from_slice(source);
    normalize_installation_reaches(destination);
}

fn normalize_installation_reaches(rows: &mut Vec<InstallationReachRequirement>) {
    rows.sort_by_key(|row| {
        (
            row.requirement.arena_index(),
            row.requirement.generation(),
            row.upper_bound.0,
        )
    });
    rows.dedup();
}

fn extend_invoked_binding_services(
    program: &dyn crate::CheckingInput,
    signature: &typed_trees::signature::StateSignature,
    reach: &mut DirectServiceReach,
) {
    extend_invoked_callback_services(
        program,
        super::callback_requirements::opaque_signature_parameters(program, signature),
        &super::declared_signature_invocations(program, signature),
        reach,
    );
}

fn extend_machine_invoked_binding_services(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    reach: &mut DirectServiceReach,
) {
    if !machine.has_bodyless_signature() {
        return;
    }
    extend_invoked_callback_services(
        program,
        super::callback_requirements::opaque_machine_parameters(program, machine),
        &super::declared_machine_invocations(program, machine),
        reach,
    );
}

/// An invocation ceiling permits entry into a binding, not every effect of
/// every operation it offers. Internal provider families contribute nothing
/// here: their exact called requirements carry their reach independently.
/// An opaque registrar can, however, invoke a supplied nominal callback. Its
/// telescope pins that callback's exact requirement, whose row must survive
/// even though no checked registrar body exposes the call.
fn extend_invoked_callback_services(
    program: &dyn crate::CheckingInput,
    parameters: &[typed_trees::data::TypeParameter],
    invocations: &[flow_effects::InvocationTarget],
    reach: &mut DirectServiceReach,
) {
    super::callback_requirements::visit(program, parameters, invocations, |signature| {
        extend_signature_service_rows(program, signature, reach);
        record_installation_reach(signature, reach);
    });
}

fn extend_service_set(destination: &mut Vec<ServiceReachId>, source: &[ServiceReachId]) {
    destination.extend_from_slice(source);
    destination.sort_by_key(|service| service.0);
    destination.dedup();
}
