//! Machine and selected call progress summaries.

use crate::checks::termination::progress::CheckedProgressSummary;
use crate::checks::termination::progress::fact_subjects::{
    fact_domain, fact_subject, profile_label, subject_from_place,
};
use crate::checks::termination::progress::{lineage, origins};
use crate::semantic_calls::call_site_argument_expressions;
use crate::semantic_calls::call_target_parameters;
use crate::semantic_calls::find_call_site;
use checked_trees::{
    BuildBoundProgressDemand, FlowCallFact, FlowFacts, FlowStateFact, ProgressDemandCallSite,
};
use language_semantics::{ProgressPremise, ProgressSubject, TerminationGuarantee};
use symbols::SymbolHandle;

pub(crate) fn derive_machine_summary(
    program: &typed_trees::TypedTrees,
    flow: &FlowFacts,
    semantic: &facts::FactPlan,
    machine: &typed_trees::machine::Machine,
    summaries: &[CheckedProgressSummary],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<CheckedProgressSummary> {
    if !crate::checks::termination::infer_machine_checked_summary_with_call_frames(
        program,
        machine,
        call_frames,
    )
    .promises_termination()
    {
        return Some(no_guarantee(machine.symbol));
    }

    if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody {
        return Some(no_guarantee(machine.symbol));
    }

    let entry_parameter_roots = program
        .machine_states(machine)
        .first()
        .map(|state| {
            program
                .state_parameters(state)
                .iter()
                .map(|parameter| parameter.symbol)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut premises = Vec::new();
    let mut build_bound_demands: Vec<BuildBoundProgressDemand> = Vec::new();
    for (_, state_flow) in flow
        .control
        .states
        .iter()
        .filter(|(_, state)| state.machine_symbol == machine.symbol)
    {
        for call in flow.control.calls.span_or_empty(state_flow.calls) {
            if is_local_state_transition(program, machine, state_flow, call) {
                // Named transitions are edges within this activation, not
                // nested calls. Their argument correspondence is reconstructed
                // only when an invoked operation demands a progress premise.
                continue;
            }
            let selected = selected_call_summary(program, call.target_symbol, summaries)?;
            for demand in selected.build_bound_demands {
                if !build_bound_demands.contains(demand) {
                    build_bound_demands.push(demand.clone());
                }
            }
            let TerminationGuarantee::Terminates {
                premises: callee_premises,
            } = selected.guarantee
            else {
                return Some(no_guarantee(machine.symbol));
            };
            for callee_premise in callee_premises {
                let build_bound_requirement = provider_receiver_requirement(
                    program,
                    call.receiver_symbol,
                    call.target_symbol,
                    callee_premise,
                );
                let local_instance = instantiate_call_premise(
                    program,
                    machine,
                    state_flow,
                    call,
                    callee_premise,
                    call_frames,
                )?;
                if admitted_receipt_covers(
                    program,
                    flow,
                    semantic,
                    state_flow,
                    call,
                    &local_instance,
                ) {
                    continue;
                }
                if let Some((
                    provider_service_package_identity,
                    provider_service_identity,
                    requirement_owner_package_identity,
                    requirement_identity,
                )) = &build_bound_requirement
                {
                    // A requirement receiver remains build-bound even when a
                    // checked wrapper happens to hold it in one of its own
                    // parameters. Rewriting it into that caller parameter
                    // would lose the exact selected-provider subject class.
                    let demand = BuildBoundProgressDemand {
                        provider_service_identity: provider_service_identity.clone(),
                        provider_service_package_identity: *provider_service_package_identity,
                        requirement_identity: requirement_identity.clone(),
                        requirement_owner_package_identity: *requirement_owner_package_identity,
                        profile_identity: profile_label(program, callee_premise.profile),
                        subject_projections: callee_premise
                            .subject
                            .projections
                            .iter()
                            .map(|symbol| program.symbols.display_path(*symbol, "::"))
                            .collect(),
                        origin: ProgressDemandCallSite {
                            machine: machine.symbol,
                            state: state_flow.state_symbol,
                            statement_ordinal: call.statement_index,
                            call_ordinal: call.call_ordinal,
                        },
                    };
                    if !build_bound_demands.contains(&demand) {
                        build_bound_demands.push(demand);
                    }
                    continue;
                }
                let mut entry_instance = local_instance;
                entry_instance.subject = origins::at_call(
                    program,
                    flow,
                    machine,
                    state_flow,
                    call,
                    entry_instance.subject,
                    call_frames,
                )?;
                let instances =
                    lineage::resolve(program, flow, machine, entry_instance, call_frames)?;
                for instance in instances {
                    if !entry_parameter_roots.contains(&instance.subject.root) {
                        // Arbitrary local values still cannot become caller
                        // premises. Only the exact provider receiver handled
                        // above may defer its coverage to composition.
                        return Some(no_guarantee(machine.symbol));
                    }
                    if !premises.contains(&instance) {
                        premises.push(instance);
                    }
                }
            }
        }
    }

    Some(CheckedProgressSummary {
        machine: machine.symbol,
        guarantee: TerminationGuarantee::Terminates { premises },
        build_bound_demands,
    })
}

pub(crate) fn no_guarantee(machine: SymbolHandle) -> CheckedProgressSummary {
    CheckedProgressSummary {
        machine,
        guarantee: TerminationGuarantee::NoGuarantee,
        build_bound_demands: Vec::new(),
    }
}

pub(crate) struct SelectedCallProgress<'a> {
    pub(crate) guarantee: &'a TerminationGuarantee,
    build_bound_demands: &'a [BuildBoundProgressDemand],
}

pub(crate) fn selected_call_summary<'a>(
    program: &'a typed_trees::TypedTrees,
    target_symbol: SymbolHandle,
    summaries: &'a [CheckedProgressSummary],
) -> Option<SelectedCallProgress<'a>> {
    if let Some((_, signature)) = program.machine_parameter_signature(target_symbol) {
        return Some(SelectedCallProgress {
            guarantee: &signature.termination_guarantee,
            build_bound_demands: &[],
        });
    }
    if let Some(signature) = program.traits().iter().find_map(|trait_definition| {
        program
            .trait_machine_signatures(trait_definition)
            .iter()
            .find(|signature| signature.symbol == target_symbol)
    }) {
        return Some(SelectedCallProgress {
            guarantee: &signature.termination_guarantee,
            build_bound_demands: &[],
        });
    }
    let target_machine =
        crate::lookup::machine_by_symbol(program, target_symbol).or_else(|| {
            crate::semantic_calls::find_state_with_machine(program, target_symbol)
                .map(|(machine, _)| machine)
        })?;
    match &target_machine.termination_plan.interface {
        language_semantics::TerminationInterface::Published(guarantee) => {
            let build_bound_demands = summaries
                .iter()
                .find(|summary| summary.machine == target_machine.symbol)
                .map_or(&[] as &[BuildBoundProgressDemand], |summary| {
                    summary.build_bound_demands.as_slice()
                });
            Some(SelectedCallProgress {
                guarantee,
                build_bound_demands,
            })
        }
        language_semantics::TerminationInterface::InternalDerived => {
            let summary = summaries
                .iter()
                .find(|summary| summary.machine == target_machine.symbol)?;
            Some(SelectedCallProgress {
                guarantee: &summary.guarantee,
                build_bound_demands: &summary.build_bound_demands,
            })
        }
    }
}

fn provider_receiver_requirement(
    program: &typed_trees::TypedTrees,
    receiver_symbol: SymbolHandle,
    target_symbol: SymbolHandle,
    premise: &ProgressPremise,
) -> Option<(
    Option<semantic_vocabulary::PackageKeyIdentity>,
    String,
    Option<semantic_vocabulary::PackageKeyIdentity>,
    String,
)> {
    program.traits().iter().find_map(|owner| {
        program
            .trait_machine_signatures(owner)
            .iter()
            .find(|requirement| requirement.symbol == target_symbol)
            .and_then(|requirement| {
                program
                    .state_signature_parameters(requirement)
                    .iter()
                    .find(|parameter| parameter.symbol == premise.subject.root)
                    .filter(|parameter| parameter.is_self)
                    .map(|_| {
                        let provider_service =
                            crate::flow::symbol_type_symbol(program, receiver_symbol)
                                .and_then(|symbol| {
                                    program.traits().iter().find(|candidate| {
                                        candidate.is_boundary && candidate.symbol == symbol
                                    })
                                })
                                .unwrap_or(owner);
                        (
                            program
                                .symbols
                                .symbol_package_identity(provider_service.symbol),
                            provider_service.name.as_str().to_owned(),
                            program.symbols.symbol_package_identity(requirement.symbol),
                            program
                                .normalized_trait_requirement_overload_identity(owner, requirement)
                                .identity(),
                        )
                    })
            })
    })
}

fn instantiate_call_premise(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state_flow: &FlowStateFact,
    call: &FlowCallFact,
    premise: &ProgressPremise,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<ProgressPremise> {
    let mut subject = call_argument_subject(
        program,
        machine,
        state_flow,
        call,
        premise.subject.root,
        call_frames,
    )?;
    subject
        .projections
        .extend(premise.subject.projections.iter().copied());
    Some(ProgressPremise {
        profile: premise.profile,
        subject,
    })
}

fn call_argument_subject(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state_flow: &FlowStateFact,
    call: &FlowCallFact,
    parameter_symbol: SymbolHandle,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<ProgressSubject> {
    let parameters = call_target_parameters(program, call.target_symbol)?;
    call_argument_subject_with_parameters(
        program,
        machine,
        state_flow,
        call,
        parameters,
        parameter_symbol,
        call_frames,
    )
}

pub(crate) fn call_argument_subject_with_parameters(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state_flow: &FlowStateFact,
    call: &FlowCallFact,
    parameters: &[typed_trees::signature::StateParameter],
    parameter_symbol: SymbolHandle,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<ProgressSubject> {
    let call_site = find_call_site(
        program,
        machine.symbol,
        state_flow.state_symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let parameter_index = parameters
        .iter()
        .position(|parameter| parameter.symbol == parameter_symbol)?;
    let arguments = call_site_argument_expressions(program, &call_site);
    let non_self_count = parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .count();
    let uses_receiver =
        parameters.iter().any(|parameter| parameter.is_self) && arguments.len() == non_self_count;
    let parameter = &parameters[parameter_index];
    let place = if parameter.is_self && uses_receiver {
        let place = crate::flow::canonical_receiver_place_for_call_site(
            program,
            machine.symbol,
            state_flow.state_symbol,
            &call_site,
            call.statement_index,
        )?;
        // A receiver that is itself a value-call result has no storage to
        // demand; the same origin replay proves which exact caller input the
        // callee returned, carrying the receiver's own peeled projection.
        // What it cannot prove keeps no subject.
        match place.root {
            facts::PlaceRoot::Expression(expression) => origins::call_argument_place(
                program,
                state_flow,
                call.statement_index,
                expression,
                parameter.type_reference,
                &place.segments,
                call_frames,
            )?,
            _ => place,
        }
    } else {
        let argument_index = if uses_receiver {
            parameters[..parameter_index]
                .iter()
                .filter(|parameter| !parameter.is_self)
                .count()
        } else {
            parameter_index
        };
        // A spelled name or member path keeps its own place; an argument that
        // is itself a value-call result or a constructor has no storage to
        // demand, so the shared origin replay proves which exact caller input
        // supplied it. What it cannot prove keeps no subject.
        origins::call_argument_place(
            program,
            state_flow,
            call.statement_index,
            *arguments.get(argument_index)?,
            parameter.type_reference,
            &[],
            call_frames,
        )?
    };
    // A demanded operand spelled through a reference leaf names the
    // referent's storage, not the carrier's: resolve the leaf so the premise
    // surface never sees the index selectors it cannot carry.
    let place = origins::call_argument_boundary_place(
        program,
        machine,
        state_flow,
        call.statement_index,
        place,
        call_frames,
    )?;
    subject_from_place(place.root, &place.segments)
}

fn is_local_state_transition(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &FlowStateFact,
    call: &FlowCallFact,
) -> bool {
    local_state_transition_target(program, machine, state, call).is_some()
}

pub(crate) fn local_state_transition_target<'program>(
    program: &'program typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &FlowStateFact,
    call: &FlowCallFact,
) -> Option<&'program typed_trees::state::State> {
    let call_site = find_call_site(
        program,
        machine.symbol,
        state.state_symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    if !matches!(
        call_site,
        crate::semantic_calls::CallSite::TransitionNamed { .. }
    ) {
        return None;
    }
    let target_index = crate::checks::termination::graph::named_transition_target_state_index(
        program,
        machine,
        call.target_symbol,
    )?;
    program.machine_states(machine).get(target_index)
}

fn admitted_receipt_covers(
    program: &typed_trees::TypedTrees,
    flow: &FlowFacts,
    semantic: &facts::FactPlan,
    state: &FlowStateFact,
    call: &FlowCallFact,
    premise: &ProgressPremise,
) -> bool {
    let Some(domain_symbol) = program
        .domain_definitions()
        .iter()
        .find(|domain| domain.semantic_id == premise.profile)
        .filter(|domain| typed_trees::domain::index_parameters(program, domain).is_empty())
        .map(|domain| domain.symbol)
    else {
        return false;
    };
    flow.state_call_entry_semantic_contexts(
        state,
        call.statement_index,
        call.call_ordinal,
        call.target_symbol,
        call.receiver_symbol,
    )
    .any(|context| {
        semantic
            .context_view(semantic.contexts.get(context))
            .facts()
            .any(|fact| {
                fact.evidence.origin
                    == language_semantics::QualificationEvidenceOrigin::AdmittedReceipt
                    && fact_domain(program, fact.payload) == Some(domain_symbol)
                    && fact_subject(semantic, fact.place).as_ref() == Some(&premise.subject)
            })
    })
}
