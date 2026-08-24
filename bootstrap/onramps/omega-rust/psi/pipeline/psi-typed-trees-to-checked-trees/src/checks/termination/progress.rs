use psi_checked_trees::{
    BuildBoundProgressDemand, FlowCallFact, FlowFacts, FlowStateFact, ProgressDemandCallSite,
};
use psi_diagnostics::Diagnostic;
use psi_facts::{FactPayload, FactPlace, PlaceRoot, PlaceSegment};
use psi_language_semantics::{ProgressPremise, ProgressSubject, TerminationGuarantee};
use psi_symbols::SymbolHandle;

use crate::{call_site_argument_expressions, call_target_parameters, find_call_site};

/// Derive checked termination premises from exact selected call contracts.
///
/// Published operation contracts remain the caller-facing authority. Private
/// checked callees contribute their derived summary through a fixed point, so
/// mentioning a qualified value does nothing while actually invoking a
/// premise-bearing operation instantiates exactly that premise.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CheckedProgressSummary {
    pub(crate) machine: SymbolHandle,
    pub(crate) guarantee: TerminationGuarantee,
    pub(crate) build_bound_demands: Vec<BuildBoundProgressDemand>,
}

pub(crate) fn analyze_checked_progress(
    program: &psi_typed_trees::TypedTrees,
    flow: &FlowFacts,
    semantic: &psi_facts::FactPlan,
) -> Result<Vec<CheckedProgressSummary>, Vec<Diagnostic>> {
    let mut summaries = program
        .machines()
        .iter()
        .map(|machine| CheckedProgressSummary {
            machine: machine.symbol,
            guarantee: TerminationGuarantee::NoGuarantee,
            build_bound_demands: Vec::new(),
        })
        .collect::<Vec<_>>();

    loop {
        let previous = summaries.clone();
        for machine in program.machines() {
            let summary = derive_machine_summary(program, flow, semantic, machine, &previous)
                .unwrap_or_else(|| CheckedProgressSummary {
                    machine: machine.symbol,
                    guarantee: TerminationGuarantee::NoGuarantee,
                    build_bound_demands: Vec::new(),
                });
            if let Some(retained) = summaries
                .iter_mut()
                .find(|summary| summary.machine == machine.symbol)
            {
                *retained = summary;
            }
        }
        if summaries == previous {
            break;
        }
    }

    let mut diagnostics = Vec::new();
    for machine in program.machines().iter().filter(|machine| {
        machine.supply_mode == psi_language_semantics::MachineSupplyMode::CheckedBody
    }) {
        // Ranking diagnostics own local control-flow failure and can name the
        // missing or invalid witness precisely. Progress coverage begins only
        // after that independent obligation succeeds.
        if !super::infer_machine_checked_summary(program, machine).promises_termination() {
            continue;
        }
        let Some(checked) = summaries
            .iter()
            .find(|summary| summary.machine == machine.symbol)
        else {
            continue;
        };
        let psi_language_semantics::TerminationInterface::Published(published) =
            &machine.termination_plan.interface
        else {
            continue;
        };
        let TerminationGuarantee::Terminates {
            premises: published_premises,
        } = published
        else {
            continue;
        };
        let TerminationGuarantee::Terminates {
            premises: checked_premises,
        } = &checked.guarantee
        else {
            diagnostics.push(Diagnostic::error(format!(
                "cannot prove published termination for machine `{}`: its checked body reaches an operation without a usable termination guarantee",
                machine.name
            )));
            continue;
        };
        for premise in checked_premises {
            if !published_premises.contains(premise) {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` derives progress premise `{}` for `{}` from a selected call, but its published termination contract does not cover that exact subject",
                    machine.name,
                    profile_label(program, premise.profile),
                    subject_label(program, &premise.subject),
                )));
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(summaries)
    } else {
        Err(diagnostics)
    }
}

fn derive_machine_summary(
    program: &psi_typed_trees::TypedTrees,
    flow: &FlowFacts,
    semantic: &psi_facts::FactPlan,
    machine: &psi_typed_trees::machine::Machine,
    summaries: &[CheckedProgressSummary],
) -> Option<CheckedProgressSummary> {
    if !super::infer_machine_checked_summary(program, machine).promises_termination() {
        return Some(no_guarantee(machine.symbol));
    }

    if machine.supply_mode != psi_language_semantics::MachineSupplyMode::CheckedBody {
        return Some(no_guarantee(machine.symbol));
    }

    let parameter_lineage = state_parameter_lineage(program, flow, machine);
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
                // nested calls. Their argument correspondence is folded into
                // `parameter_lineage` above.
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
                let local_instance =
                    instantiate_call_premise(program, machine, state_flow, call, callee_premise)?;
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
                let instances = resolve_through_state_lineage(&parameter_lineage, local_instance)?;
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

fn no_guarantee(machine: SymbolHandle) -> CheckedProgressSummary {
    CheckedProgressSummary {
        machine,
        guarantee: TerminationGuarantee::NoGuarantee,
        build_bound_demands: Vec::new(),
    }
}

struct SelectedCallProgress<'a> {
    guarantee: &'a TerminationGuarantee,
    build_bound_demands: &'a [BuildBoundProgressDemand],
}

fn selected_call_summary<'a>(
    program: &'a psi_typed_trees::TypedTrees,
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
    let target_machine = program.machines().iter().find(|candidate| {
        candidate.symbol == target_symbol
            || program
                .machine_states(candidate)
                .iter()
                .any(|state| state.symbol == target_symbol)
    })?;
    match &target_machine.termination_plan.interface {
        psi_language_semantics::TerminationInterface::Published(guarantee) => {
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
        psi_language_semantics::TerminationInterface::InternalDerived => {
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
    program: &psi_typed_trees::TypedTrees,
    receiver_symbol: SymbolHandle,
    target_symbol: SymbolHandle,
    premise: &ProgressPremise,
) -> Option<(
    Option<psi_core::PackageKeyIdentity>,
    String,
    Option<psi_core::PackageKeyIdentity>,
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
    program: &psi_typed_trees::TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state_flow: &FlowStateFact,
    call: &FlowCallFact,
    premise: &ProgressPremise,
) -> Option<ProgressPremise> {
    let mut subject =
        call_argument_subject(program, machine, state_flow, call, premise.subject.root)?;
    subject
        .projections
        .extend(premise.subject.projections.iter().copied());
    Some(ProgressPremise {
        profile: premise.profile,
        subject,
    })
}

fn call_argument_subject(
    program: &psi_typed_trees::TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state_flow: &FlowStateFact,
    call: &FlowCallFact,
    parameter_symbol: SymbolHandle,
) -> Option<ProgressSubject> {
    let parameters = call_target_parameters(program, call.target_symbol)?;
    call_argument_subject_with_parameters(
        program,
        machine,
        state_flow,
        call,
        parameters,
        parameter_symbol,
    )
}

fn call_argument_subject_with_parameters(
    program: &psi_typed_trees::TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state_flow: &FlowStateFact,
    call: &FlowCallFact,
    parameters: &[psi_typed_trees::signature::StateParameter],
    parameter_symbol: SymbolHandle,
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
        crate::flow::canonical_receiver_place_for_call_site(
            program,
            machine.symbol,
            state_flow.state_symbol,
            &call_site,
        )?
    } else {
        let argument_index = if uses_receiver {
            parameters[..parameter_index]
                .iter()
                .filter(|parameter| !parameter.is_self)
                .count()
        } else {
            parameter_index
        };
        crate::flow::canonical_place_from_expression_in_state(
            program,
            state_flow.state_symbol,
            call.statement_index,
            *arguments.get(argument_index)?,
        )?
    };
    subject_from_place(place.root, &place.segments)
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ParameterLineage {
    Unseen,
    Exact(Vec<ProgressSubject>),
    Ambiguous,
}

fn state_parameter_lineage(
    program: &psi_typed_trees::TypedTrees,
    flow: &FlowFacts,
    machine: &psi_typed_trees::machine::Machine,
) -> Vec<(SymbolHandle, ParameterLineage)> {
    let states = program.machine_states(machine);
    let mut lineage = states
        .iter()
        .flat_map(|state| program.state_parameters(state))
        .map(|parameter| (parameter.symbol, ParameterLineage::Unseen))
        .collect::<Vec<_>>();
    if let Some(entry) = states.first() {
        for parameter in program.state_parameters(entry) {
            set_parameter_lineage(
                &mut lineage,
                parameter.symbol,
                ParameterLineage::Exact(vec![ProgressSubject {
                    root: parameter.symbol,
                    projections: Vec::new(),
                }]),
            );
        }
    }

    loop {
        let previous = lineage.clone();
        for (_, state_flow) in flow
            .control
            .states
            .iter()
            .filter(|(_, state)| state.machine_symbol == machine.symbol)
        {
            for call in flow.control.calls.span_or_empty(state_flow.calls) {
                let Some(target) =
                    local_state_transition_target(program, machine, state_flow, call)
                else {
                    continue;
                };
                let target_parameters = program.state_parameters(target);
                for parameter in target_parameters {
                    let incoming = call_argument_subject_with_parameters(
                        program,
                        machine,
                        state_flow,
                        call,
                        target_parameters,
                        parameter.symbol,
                    )
                    .map(|subject| resolve_subject_lineage(&previous, subject))
                    .unwrap_or(ParameterLineage::Ambiguous);
                    merge_parameter_lineage(&mut lineage, parameter.symbol, incoming);
                }
            }
        }
        if lineage == previous {
            break;
        }
    }

    lineage
}

fn is_local_state_transition(
    program: &psi_typed_trees::TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &FlowStateFact,
    call: &FlowCallFact,
) -> bool {
    local_state_transition_target(program, machine, state, call).is_some()
}

fn local_state_transition_target<'program>(
    program: &'program psi_typed_trees::TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &FlowStateFact,
    call: &FlowCallFact,
) -> Option<&'program psi_typed_trees::state::State> {
    let call_site = find_call_site(
        program,
        machine.symbol,
        state.state_symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    if !matches!(call_site, crate::CallSite::TransitionNamed { .. }) {
        return None;
    }
    let target_index =
        super::graph::named_transition_target_state_index(program, machine, call.target_symbol)?;
    program.machine_states(machine).get(target_index)
}

fn resolve_subject_lineage(
    lineage: &[(SymbolHandle, ParameterLineage)],
    subject: ProgressSubject,
) -> ParameterLineage {
    let Some((_, root)) = lineage.iter().find(|(symbol, _)| *symbol == subject.root) else {
        return ParameterLineage::Ambiguous;
    };
    match root {
        ParameterLineage::Unseen => ParameterLineage::Unseen,
        ParameterLineage::Ambiguous => ParameterLineage::Ambiguous,
        ParameterLineage::Exact(roots) => ParameterLineage::Exact(
            roots
                .iter()
                .map(|root| {
                    let mut resolved = root.clone();
                    resolved
                        .projections
                        .extend(subject.projections.iter().copied());
                    resolved
                })
                .collect(),
        ),
    }
}

fn resolve_through_state_lineage(
    lineage: &[(SymbolHandle, ParameterLineage)],
    premise: ProgressPremise,
) -> Option<Vec<ProgressPremise>> {
    let ParameterLineage::Exact(subjects) = resolve_subject_lineage(lineage, premise.subject)
    else {
        return None;
    };
    Some(
        subjects
            .into_iter()
            .map(|subject| ProgressPremise {
                profile: premise.profile,
                subject,
            })
            .collect(),
    )
}

fn set_parameter_lineage(
    lineage: &mut [(SymbolHandle, ParameterLineage)],
    symbol: SymbolHandle,
    value: ParameterLineage,
) {
    if let Some((_, retained)) = lineage
        .iter_mut()
        .find(|(candidate, _)| *candidate == symbol)
    {
        *retained = value;
    }
}

fn merge_parameter_lineage(
    lineage: &mut [(SymbolHandle, ParameterLineage)],
    symbol: SymbolHandle,
    incoming: ParameterLineage,
) {
    let Some((_, retained)) = lineage
        .iter_mut()
        .find(|(candidate, _)| *candidate == symbol)
    else {
        return;
    };
    match (&*retained, incoming) {
        (_, ParameterLineage::Unseen) => {}
        (ParameterLineage::Unseen, value) => *retained = value,
        (ParameterLineage::Exact(_), ParameterLineage::Exact(right)) => {
            let ParameterLineage::Exact(retained) = retained else {
                unreachable!()
            };
            for subject in right {
                if !retained.contains(&subject) {
                    retained.push(subject);
                }
            }
        }
        (ParameterLineage::Exact(_), ParameterLineage::Ambiguous) => {
            *retained = ParameterLineage::Ambiguous;
        }
        (ParameterLineage::Ambiguous, _) => {}
    }
}

fn admitted_receipt_covers(
    program: &psi_typed_trees::TypedTrees,
    flow: &FlowFacts,
    semantic: &psi_facts::FactPlan,
    state: &FlowStateFact,
    call: &FlowCallFact,
    premise: &ProgressPremise,
) -> bool {
    let Some(domain_symbol) = program
        .domain_definitions()
        .iter()
        .find(|domain| domain.semantic_id == premise.profile)
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
                    == psi_language_semantics::QualificationEvidenceOrigin::AdmittedReceipt
                    && fact_domain(fact.payload) == Some(domain_symbol)
                    && fact_subject(semantic, fact.place).as_ref() == Some(&premise.subject)
            })
    })
}

fn fact_domain(payload: FactPayload) -> Option<SymbolHandle> {
    match payload {
        FactPayload::DomainMembership { domain_symbol, .. }
        | FactPayload::ContractDomainMembership { domain_symbol, .. } => Some(domain_symbol),
        _ => None,
    }
}

fn fact_subject(semantic: &psi_facts::FactPlan, place: FactPlace) -> Option<ProgressSubject> {
    match place {
        FactPlace::Place(handle) => {
            let place = *semantic.places.get(handle);
            subject_from_place(
                place.root,
                semantic.place_segments.span_or_empty(place.segments),
            )
        }
        FactPlace::Symbol(symbol) => Some(ProgressSubject {
            root: symbol,
            projections: Vec::new(),
        }),
        _ => None,
    }
}

fn subject_from_place(root: PlaceRoot, segments: &[PlaceSegment]) -> Option<ProgressSubject> {
    let PlaceRoot::Symbol(root) = root else {
        return None;
    };
    let mut projections = Vec::new();
    for segment in segments {
        match segment {
            PlaceSegment::Field { symbol } => projections.push(*symbol),
            // The field symbol already carries exact variant identity, matching
            // authored member-path normalization.
            PlaceSegment::Case { .. } => {}
            PlaceSegment::FixedIndex { .. }
            | PlaceSegment::FixedRange { .. }
            | PlaceSegment::Index { .. } => return None,
        }
    }
    Some(ProgressSubject { root, projections })
}

fn profile_label(
    program: &psi_typed_trees::TypedTrees,
    profile: psi_language_semantics::SemanticDomainId,
) -> String {
    program
        .semantic_domains
        .name(profile)
        .unwrap_or("<unknown-progress-profile>")
        .to_owned()
}

fn subject_label(program: &psi_typed_trees::TypedTrees, subject: &ProgressSubject) -> String {
    let mut label = program.symbols.display_path(subject.root, "::");
    for projection in &subject.projections {
        label.push('.');
        label.push_str(&program.symbols.display_path(*projection, "::"));
    }
    label
}
