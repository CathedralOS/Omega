//! Machine and selected call progress summaries.

use validation::call_site_argument_expressions;
use validation::call_target_parameters;
use validation::find_call_site;

use crate::checks::termination::progress::CheckedProgressSummary;
use crate::checks::termination::progress::fact_subjects::{
    fact_domain, fact_subject, subject_from_place,
};
use crate::checks::termination::progress::{lineage, origins};

use checked_trees::{BuildBoundProgressDemand, FlowCallFact, FlowFacts, FlowStateFact};
use language_semantics::{ProgressPremise, ProgressSubject, TerminationGuarantee};
use symbols::SymbolHandle;

pub(crate) fn derive_machine_summary(
    program: &dyn ::validation::CheckingInput,
    flow: &FlowFacts,
    semantic: &facts::FactPlan,
    cleanup_dependencies: &arena::Arena<flow_effects::CleanupDependency>,
    machine: &typed_trees::machine::Machine,
    summaries: &[CheckedProgressSummary],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    rank_arrivals: &checked_trees::CheckedRankArrivalPlan,
) -> Option<CheckedProgressSummary> {
    if !crate::checks::termination::retained_machine_local_summary(machine, rank_arrivals)
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
                // A call whose routes disagree yields one subject per route.
                // The demand holds only if it holds for every one, so each
                // resolves on its own and all of their premises are kept.
                let subjects = origins::at_call(
                    program,
                    flow,
                    machine,
                    state_flow,
                    call,
                    local_instance.subject.clone(),
                    call_frames,
                )?;
                let mut instances = Vec::new();
                for subject in subjects {
                    let mut entry_instance = local_instance.clone();
                    entry_instance.subject = subject;
                    instances.extend(lineage::resolve(
                        program,
                        flow,
                        machine,
                        entry_instance,
                        call_frames,
                    )?);
                }
                for instance in instances {
                    if !entry_parameter_roots.contains(&instance.subject.root) {
                        // Arbitrary local values cannot become caller
                        // premises; only entry parameters can.
                        return Some(no_guarantee(machine.symbol));
                    }
                    if !premises.contains(&instance) {
                        premises.push(instance);
                    }
                }
            }
        }
    }

    // Compiler-selected calls have no authored call ordinal. Use the same
    // prepared dependency graph as effect admission, and do not invent a
    // subject substitution for a premise-bearing cleanup contract.
    for (_, dependency) in cleanup_dependencies
        .iter()
        .filter(|(_, dependency)| dependency.caller_machine == machine.symbol)
    {
        let selected = selected_call_summary(program, dependency.target_machine, summaries)?;
        if !matches!(selected.guarantee,
            TerminationGuarantee::Terminates { premises } if premises.is_empty())
            || !selected.build_bound_demands.is_empty()
        {
            return Some(no_guarantee(machine.symbol));
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

static CLOSED_VALUE_TERMINATION: TerminationGuarantee = TerminationGuarantee::Terminates {
    premises: Vec::new(),
};

pub(crate) fn selected_call_summary<'a>(
    program: &'a dyn ::validation::CheckingInput,
    target_symbol: SymbolHandle,
    summaries: &'a [CheckedProgressSummary],
) -> Option<SelectedCallProgress<'a>> {
    // Integer embedding is a compiler-owned proof value term. Its own
    // formation gate checks the operand and forbids runtime occurrences;
    // it has no authored machine whose progress summary could supply this
    // meaning. The flow walk still checks every call in its operand, so this
    // total leaf cannot conceal a nonterminating computation.
    if program.symbols.builtin_function_for_symbol(target_symbol)
        == Some(symbols::BuiltinFunction::IntegerEmbed)
    {
        return Some(SelectedCallProgress {
            guarantee: &CLOSED_VALUE_TERMINATION,
            build_bound_demands: &[],
        });
    }
    if let Some((_, signature)) = program.machine_parameter_signature(target_symbol) {
        return Some(SelectedCallProgress {
            guarantee: &signature.termination_guarantee,
            build_bound_demands: &[],
        });
    }
    if let Some(signature) = program
        .trait_machine_signature(target_symbol)
        .map(|(_, signature)| signature)
    {
        return Some(SelectedCallProgress {
            guarantee: &signature.termination_guarantee,
            build_bound_demands: &[],
        });
    }
    // Exact compiler-owned value semantics independently establish totality.
    // Their generated boundary machine is an execution shell, not the owner
    // of this meaning. Ordinary public signatures above still consume only
    // their authored promise; a matching operator name supplies no authority.
    if typed_trees::operator::declaration_by_symbol(program, target_symbol).is_some_and(
        |operator| validation::selected_operator_has_closed_value_denotation(program, operator),
    ) {
        return Some(SelectedCallProgress {
            guarantee: &CLOSED_VALUE_TERMINATION,
            build_bound_demands: &[],
        });
    }
    let target_machine = program
        .machine_by_symbol(target_symbol)
        .or_else(|| validation::find_state(program, target_symbol).map(|(machine, _)| machine))?;
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

fn instantiate_call_premise(
    program: &dyn ::validation::CheckingInput,
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
        call_target_parameters(program, call.target_symbol)?,
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

/// The single subject a call argument names, or `None` when it names none or
/// when a dynamic selector leaves several — `call_argument_subjects`
/// is what reads that set.
pub(crate) fn call_argument_subject(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state_flow: &FlowStateFact,
    call: &FlowCallFact,
    parameters: &[typed_trees::signature::StateParameter],
    parameter_symbol: SymbolHandle,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<ProgressSubject> {
    let mut subjects = call_argument_subjects(
        program,
        machine,
        state_flow,
        call,
        parameters,
        parameter_symbol,
        call_frames,
    )?;
    (subjects.len() == 1).then(|| subjects.pop()).flatten()
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn call_argument_subjects(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state_flow: &FlowStateFact,
    call: &FlowCallFact,
    parameters: &[typed_trees::signature::StateParameter],
    parameter_symbol: SymbolHandle,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<ProgressSubject>> {
    let call_site = find_call_site(
        program,
        machine.symbol,
        state_flow.state_symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let parameter_index =
        typed_trees::signature::state_parameter_position(parameters, parameter_symbol)?;
    let arguments = call_site_argument_expressions(program, &call_site);
    let parameter = &parameters[parameter_index];
    let place = origins::call_argument_place(
        program,
        state_flow,
        call.statement_index,
        *arguments.get(parameter_index)?,
        parameter.type_reference,
        &[],
        call_frames,
    )?;
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
    if let Some(subject) = subject_from_place(place.root, &place.segments) {
        return Some(vec![subject]);
    }
    dynamic_selector_subjects(program, state_flow, call, &place, call_frames)
}

/// The subjects a demand behind a DYNAMIC selector names, when the selected
/// carrier is a closed array literal.
///
/// `boxes[i].view.scheduler` has no single origin: `i` is not a constant, so
/// the premise surface, which carries field projections and no selectors,
/// refuses the place. The array's length is closed and its elements are
/// spelled, though, so the demand names one of finitely many exact places --
/// the same shape as a callee whose routes disagree. Every element must name
/// one, and the caller conjoins a premise apiece; one it cannot name leaves
/// the whole demand unproven.
fn dynamic_selector_subjects(
    program: &dyn ::validation::CheckingInput,
    state_flow: &FlowStateFact,
    call: &FlowCallFact,
    place: &validation::CanonicalPlace,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<ProgressSubject>> {
    let selector = place
        .segments
        .iter()
        .position(|segment| matches!(segment, facts::PlaceSegment::Index { .. }))?;
    // Only one selector: a second would multiply two element sets together,
    // which is a different obligation than this conjunction.
    if place.segments[selector + 1..]
        .iter()
        .any(|segment| matches!(segment, facts::PlaceSegment::Index { .. }))
    {
        return None;
    }
    let root = place.root.as_symbol()?;
    let typed_state = program.state_by_symbol(state_flow.state_symbol)?;
    let (_, local) = program
        .state_local_declaration(typed_state, root)
        .filter(|&(ordinal, _)| ordinal < call.statement_index)?;
    let typed_trees::expression::ExpressionNode::ArrayLiteral(elements) =
        program.expression_table.expression(local.initial_value)
    else {
        return None;
    };
    let elements = program.expression_table.expression_handles(*elements);
    if elements.is_empty() {
        return None;
    }
    let relative = &place.segments[selector + 1..];
    let mut subjects = Vec::with_capacity(elements.len());
    for element in elements {
        let element_place = origins::call_argument_place(
            program,
            state_flow,
            call.statement_index,
            *element,
            local.type_reference,
            relative,
            call_frames,
        )?;
        let subject = subject_from_place(element_place.root, &element_place.segments)?;
        if !subjects.contains(&subject) {
            subjects.push(subject);
        }
    }
    Some(subjects)
}

fn is_local_state_transition(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &FlowStateFact,
    call: &FlowCallFact,
) -> bool {
    local_state_transition_target(program, machine, state, call).is_some()
}

pub(crate) fn local_state_transition_target<'program>(
    program: &'program dyn ::validation::CheckingInput,
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
    if !matches!(call_site, validation::CallSite::TransitionNamed { .. }) {
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
    program: &dyn ::validation::CheckingInput,
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
