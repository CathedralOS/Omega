//! Machine and selected call progress summaries.

use validation::call_site_argument_expressions;
use validation::call_target_parameters;
use validation::find_call_site;

use crate::checks::termination::progress::CheckedProgressSummary;
use crate::checks::termination::progress::fact_subjects::{
    fact_domain, fact_subject, subject_from_place,
};
use crate::checks::termination::progress::{lineage, origins};

use checked_trees::{FlowCallFact, FlowFacts, FlowStateFact};
use language_semantics::{ProgressPremise, TerminationGuarantee};
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
    if machine.target_family_interface {
        return family_summary(program, machine, summaries);
    }
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
    for state_flow in flow.control.machine_states(machine.symbol) {
        for call in flow.control.calls.span_or_empty(state_flow.calls) {
            if is_local_state_transition(program, machine, state_flow, call) {
                // Named transitions are edges within this activation, not
                // nested calls. Their argument correspondence is reconstructed
                // only when an invoked operation demands a progress premise.
                continue;
            }
            let selected = selected_call_summary(program, call.target_symbol, summaries)?;
            let TerminationGuarantee::Terminates {
                premises: callee_premises,
            } = selected.guarantee
            else {
                return Some(no_guarantee(machine.symbol));
            };
            for callee_premise in callee_premises {
                let places = call_argument_places(
                    program,
                    machine,
                    state_flow,
                    call,
                    call_target_parameters(program, call.target_symbol)?,
                    callee_premise.subject.root,
                    &callee_premise.subject.projections,
                    call_frames,
                )?;
                for place in places {
                    for instance in resolve_call_place_premises(
                        program,
                        flow,
                        semantic,
                        machine,
                        state_flow,
                        call,
                        place,
                        callee_premise.profile,
                        call_frames,
                    )? {
                        if !entry_parameter_roots.contains(&instance.subject.root) {
                            return Some(no_guarantee(machine.symbol));
                        }
                        crate::collections::push_unique(&mut premises, instance);
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
        {
            return Some(no_guarantee(machine.symbol));
        }
    }

    Some(CheckedProgressSummary {
        machine: machine.symbol,
        guarantee: TerminationGuarantee::Terminates { premises },
    })
}

/// configuration.md's portable family head imports only guarantees shared by
/// every retained member. Join after checked member summaries exist, with
/// exact positional substitution into the head's independent telescope.
/// A lexical prototype or circular promise cannot seed this fixed point.
/// Canonical configuration.md blob ee6d981abab37d83e1ef92d92f9d267a0a5765bc:
/// "callers meet every member's premises and the joined conservative effect/crash
/// envelope."
fn family_summary(
    program: &dyn ::validation::CheckingInput,
    head: &typed_trees::machine::Machine,
    summaries: &[CheckedProgressSummary],
) -> Option<CheckedProgressSummary> {
    let mut families = program
        .target_machine_families
        .iter()
        .filter(|family| family.canonical == head.symbol);
    let family = families.next()?;
    if families.next().is_some() || family.members.is_empty() {
        return None;
    }
    let head_entry = program.machine_states(head).first()?;
    let parameters = program.state_parameters(head_entry);
    let mut premises = Vec::new();
    for member in &family.members {
        let machine = program.machine_by_symbol(member.machine)?;
        let language_semantics::TerminationInterface::Published(TerminationGuarantee::Terminates {
            premises: published,
        }) = &machine.termination_plan.interface
        else {
            return Some(no_guarantee(head.symbol));
        };
        let checked = summaries
            .iter()
            .find(|summary| summary.machine == machine.symbol)?;
        if !checked.guarantee.promises_termination() {
            return Some(no_guarantee(head.symbol));
        }
        let entry = program.machine_states(machine).first()?;
        let source = program.state_parameters(entry);
        if source.len() != parameters.len() {
            return None;
        }
        for premise in published {
            let position = source
                .iter()
                .position(|parameter| parameter.symbol == premise.subject.root)?;
            let mut premise = premise.clone();
            premise.subject.root = parameters[position].symbol;
            crate::collections::push_unique(&mut premises, premise);
        }
    }
    Some(CheckedProgressSummary {
        machine: head.symbol,
        guarantee: TerminationGuarantee::Terminates { premises },
    })
}

pub(crate) fn no_guarantee(machine: SymbolHandle) -> CheckedProgressSummary {
    CheckedProgressSummary {
        machine,
        guarantee: TerminationGuarantee::NoGuarantee,
    }
}

pub(crate) struct SelectedCallProgress<'a> {
    pub(crate) guarantee: &'a TerminationGuarantee,
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
        });
    }
    if let Some((_, signature)) = program.machine_parameter_signature(target_symbol) {
        return Some(SelectedCallProgress {
            guarantee: &signature.termination_guarantee,
        });
    }
    if let Some(signature) = program
        .trait_machine_signature(target_symbol)
        .map(|(_, signature)| signature)
    {
        return Some(SelectedCallProgress {
            guarantee: &signature.termination_guarantee,
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
        });
    }
    let target_machine = program
        .machine_by_symbol(target_symbol)
        .or_else(|| validation::find_state(program, target_symbol).map(|(machine, _)| machine))?;
    // A boundary requirement is a declaration with no body: a call to it runs
    // whichever satisfier the build selects. Source checking has no selection
    // row, so the requirement terminates unconditionally only when every
    // declared satisfier carries an unconditional checked summary. A satisfier
    // that suspends, blocks, crashes or is external keeps the requirement's own
    // published promise, which is the promise an unsatisfied boundary call has.
    if !target_machine.body_is_present
        && target_machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
    {
        let satisfiers = validation::requirement_satisfiers(program, target_machine.symbol);
        if !satisfiers.is_empty()
            && satisfiers.iter().all(|satisfier| {
                summaries.iter().any(|summary| {
                    summary.machine == *satisfier
                        && matches!(
                            &summary.guarantee,
                            TerminationGuarantee::Terminates { premises } if premises.is_empty()
                        )
                })
            })
        {
            return Some(SelectedCallProgress {
                guarantee: &CLOSED_VALUE_TERMINATION,
            });
        }
    }
    if target_machine.target_family_interface {
        let summary = program
            .machine_position_by_symbol(target_machine.symbol)
            .and_then(|index| summaries.get(index))
            .filter(|summary| summary.machine == target_machine.symbol)
            .or_else(|| {
                summaries
                    .iter()
                    .find(|summary| summary.machine == target_machine.symbol)
            })?;
        return Some(SelectedCallProgress {
            guarantee: &summary.guarantee,
        });
    }
    match &target_machine.termination_plan.interface {
        language_semantics::TerminationInterface::Published(guarantee) => {
            Some(SelectedCallProgress { guarantee })
        }
        language_semantics::TerminationInterface::InternalDerived => {
            let summary = program
                .machine_position_by_symbol(target_machine.symbol)
                .and_then(|index| summaries.get(index))
                .filter(|summary| summary.machine == target_machine.symbol)
                .or_else(|| {
                    summaries
                        .iter()
                        .find(|summary| summary.machine == target_machine.symbol)
                })?;
            Some(SelectedCallProgress {
                guarantee: &summary.guarantee,
            })
        }
    }
}

/// Retain every selected place until its captured origin is resolved. A
/// selector slot is not itself a public progress subject, and resolving its
/// source twice could replace a captured value with a later storage version.
#[allow(clippy::too_many_arguments)]
pub(crate) fn call_argument_places(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &FlowStateFact,
    call: &FlowCallFact,
    parameters: &[typed_trees::signature::StateParameter],
    parameter_symbol: SymbolHandle,
    projections: &[SymbolHandle],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<validation::CanonicalPlace>> {
    let call_site = find_call_site(
        program,
        machine.symbol,
        state.state_symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let position = typed_trees::signature::state_parameter_position(parameters, parameter_symbol)?;
    let arguments = call_site_argument_expressions(program, &call_site);
    let mut relative = Vec::new();
    for projection in projections {
        validation::push_field_place_segments(program, &mut relative, *projection);
    }
    let place = origins::call_argument_place(
        program,
        state,
        call.statement_index,
        *arguments.get(position)?,
        parameters[position].type_reference,
        &relative,
        call_frames,
    )?;
    selector_places(program, state, call, place)
}

/// Closed geometry supplies alternatives, never their contents. Each fixed
/// slot still owes the ordinary capture, binding and write-preservation walk.
/// Nested selectors consume one dimension at a time without a shape budget.
pub(super) fn selector_places(
    program: &dyn ::validation::CheckingInput,
    state: &FlowStateFact,
    call: &FlowCallFact,
    place: validation::CanonicalPlace,
) -> Option<Vec<validation::CanonicalPlace>> {
    let mut pending = vec![place];
    let mut places = Vec::new();
    while let Some(mut selected) = pending.pop() {
        if let Some(selector) = selected
            .segments
            .iter()
            .position(|segment| matches!(segment, facts::PlaceSegment::Index { .. }))
        {
            let collection = validation::CanonicalPlace {
                root: selected.root,
                segments: selected.segments[..selector].to_vec(),
            };
            let reference = validation::canonical_place_type_reference(
                program,
                state.state_symbol,
                call.statement_index,
                &collection,
            )?;
            let length = validation::fixed_array_type_length(program, reference)?;
            if length == 0 {
                return None;
            }
            for index in 0..length {
                selected.segments[selector] = facts::PlaceSegment::FixedIndex { index };
                pending.push(selected.clone());
            }
        } else if !places.contains(&selected) {
            places.push(selected);
        }
    }
    (!places.is_empty()).then_some(places)
}

/// Local receipts and entry lineage are separate sources of coverage. A route
/// catalogue or a callee requirement is neither. This shared transport is used
/// both by summary derivation and by the consuming call's ordinary admission.
#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_call_place_premises(
    program: &dyn ::validation::CheckingInput,
    flow: &FlowFacts,
    semantic: &facts::FactPlan,
    machine: &typed_trees::machine::Machine,
    state: &FlowStateFact,
    call: &FlowCallFact,
    place: validation::CanonicalPlace,
    profile: language_semantics::SemanticDomainId,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<ProgressPremise>> {
    let local = origins::call_argument_boundary_place(
        program,
        machine,
        state,
        call.statement_index,
        place.clone(),
        call_frames,
    )?;
    let mut owned_frames = None;
    let frames = crate::flow::shared_call_frames_or(call_frames, program, &mut owned_frames)?;
    // A live receipt does not excuse writes during later operand evaluation.
    // Check both the captured slot and its referent before using that receipt.
    let preserved = [&place, &local].iter().all(|place| {
        crate::flow::local_reference_storage_at_call(
            program,
            frames,
            machine,
            flow,
            state,
            call,
            (*place).clone(),
            crate::flow::ValueObservation::Contents,
        )
        .is_some()
    });
    if preserved && let Some(subject) = subject_from_place(local.root, &local.segments) {
        let premise = ProgressPremise { profile, subject };
        if admitted_receipt_covers(program, flow, semantic, state, call, &premise) {
            return Some(Vec::new());
        }
    }
    let mut premises = Vec::new();
    for subject in origins::at_place(
        program,
        flow,
        semantic,
        machine,
        state,
        call,
        place,
        call_frames,
    )? {
        for premise in lineage::resolve(
            program,
            flow,
            semantic,
            machine,
            ProgressPremise { profile, subject },
            call_frames,
        )? {
            crate::collections::push_unique(&mut premises, premise);
        }
    }
    Some(premises)
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
