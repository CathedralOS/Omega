use psi_checked_trees::{FlowCallFact, FlowFacts, FlowStateFact};
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
pub(crate) fn analyze_checked_progress(
    program: &psi_typed_trees::TypedTrees,
    flow: &FlowFacts,
    semantic: &psi_facts::FactPlan,
) -> Result<Vec<(SymbolHandle, TerminationGuarantee)>, Vec<Diagnostic>> {
    let mut summaries = program
        .machines()
        .iter()
        .map(|machine| (machine.symbol, TerminationGuarantee::NoGuarantee))
        .collect::<Vec<_>>();

    loop {
        let previous = summaries.clone();
        for machine in program.machines() {
            let summary = derive_machine_summary(program, flow, semantic, machine, &previous)
                .unwrap_or(TerminationGuarantee::NoGuarantee);
            if let Some((_, retained)) = summaries
                .iter_mut()
                .find(|(symbol, _)| *symbol == machine.symbol)
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
        let Some((_, checked)) = summaries
            .iter()
            .find(|(symbol, _)| *symbol == machine.symbol)
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
        } = checked
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
    summaries: &[(SymbolHandle, TerminationGuarantee)],
) -> Option<TerminationGuarantee> {
    if !super::infer_machine_checked_summary(program, machine).promises_termination() {
        return Some(TerminationGuarantee::NoGuarantee);
    }

    if machine.supply_mode != psi_language_semantics::MachineSupplyMode::CheckedBody {
        return Some(TerminationGuarantee::NoGuarantee);
    }

    let mut premises = Vec::new();
    for (_, state_flow) in flow
        .control
        .states
        .iter()
        .filter(|(_, state)| state.machine_symbol == machine.symbol)
    {
        for call in flow.control.calls.span_or_empty(state_flow.calls) {
            let guarantee = selected_call_guarantee(program, call.target_symbol, summaries)?;
            let TerminationGuarantee::Terminates {
                premises: callee_premises,
            } = guarantee
            else {
                return Some(TerminationGuarantee::NoGuarantee);
            };
            for callee_premise in callee_premises {
                let instance =
                    instantiate_call_premise(program, machine, state_flow, call, callee_premise)?;
                if admitted_receipt_covers(program, flow, semantic, state_flow, call, &instance) {
                    continue;
                }
                if !machine_parameter_roots(program, machine).contains(&instance.subject.root) {
                    // A private local/build-bound subject cannot silently
                    // become a caller premise. Local admitted receipts are
                    // discharged above; manifest-bound provider discharge is
                    // the remaining TPR6 slice.
                    return Some(TerminationGuarantee::NoGuarantee);
                }
                if !premises.contains(&instance) {
                    premises.push(instance);
                }
            }
        }
    }

    Some(TerminationGuarantee::Terminates { premises })
}

fn selected_call_guarantee<'a>(
    program: &'a psi_typed_trees::TypedTrees,
    target_symbol: SymbolHandle,
    summaries: &'a [(SymbolHandle, TerminationGuarantee)],
) -> Option<&'a TerminationGuarantee> {
    if let Some((_, signature)) = program.machine_parameter_signature(target_symbol) {
        return Some(&signature.termination_guarantee);
    }
    if let Some(signature) = program.traits().iter().find_map(|trait_definition| {
        program
            .trait_machine_signatures(trait_definition)
            .iter()
            .find(|signature| signature.symbol == target_symbol)
    }) {
        return Some(&signature.termination_guarantee);
    }
    let target_machine = program.machines().iter().find(|candidate| {
        candidate.symbol == target_symbol
            || program
                .machine_states(candidate)
                .iter()
                .any(|state| state.symbol == target_symbol)
    })?;
    match &target_machine.termination_plan.interface {
        psi_language_semantics::TerminationInterface::Published(guarantee) => Some(guarantee),
        psi_language_semantics::TerminationInterface::InternalDerived => summaries
            .iter()
            .find(|(symbol, _)| *symbol == target_machine.symbol)
            .map(|(_, guarantee)| guarantee),
    }
}

fn instantiate_call_premise(
    program: &psi_typed_trees::TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state_flow: &FlowStateFact,
    call: &FlowCallFact,
    premise: &ProgressPremise,
) -> Option<ProgressPremise> {
    let call_site = find_call_site(
        program,
        machine.symbol,
        state_flow.state_symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let parameters = call_target_parameters(program, call.target_symbol)?;
    let parameter_index = parameters
        .iter()
        .position(|parameter| parameter.symbol == premise.subject.root)?;
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
    let mut subject = subject_from_place(place.root, &place.segments)?;
    subject
        .projections
        .extend(premise.subject.projections.iter().copied());
    Some(ProgressPremise {
        profile: premise.profile,
        subject,
    })
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
            PlaceSegment::FixedIndex { .. } | PlaceSegment::Index { .. } => return None,
        }
    }
    Some(ProgressSubject { root, projections })
}

fn machine_parameter_roots(
    program: &psi_typed_trees::TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
) -> Vec<SymbolHandle> {
    program
        .machine_states(machine)
        .iter()
        .flat_map(|state| program.state_parameters(state))
        .map(|parameter| parameter.symbol)
        .collect()
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
