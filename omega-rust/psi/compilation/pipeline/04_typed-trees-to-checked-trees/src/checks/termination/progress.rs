//!
//! This file analyzes checked progress. `machine_summaries.rs` derives
//! machine and selected call summaries and `fact_subjects.rs` names fact
//! domains and subjects; the remaining files carry the lineage and receipt
//! vocabulary.

mod components;
mod fact_subjects;
mod lineage;
mod machine_summaries;
mod origins;

use checked_trees::{FlowCallFact, FlowFacts, FlowStateFact};
use diagnostics::Diagnostic;
use language_semantics::{ProgressPremise, ProgressSubject, TerminationGuarantee};
use symbols::SymbolHandle;

use crate::checks::termination::progress::fact_subjects::{profile_label, subject_label};

/// Check a derived demand against the entry's existing requirement telescope.
/// This does not add a caller premise or treat an exit guarantee, another
/// subject's qualification, or a later receipt as an entry assumption.
pub(crate) fn entry_covers_progress_premises(
    program: &dyn ::validation::CheckingInput,
    checked: &checked_trees::CheckFacts,
    machine: SymbolHandle,
    premises: &[ProgressPremise],
) -> bool {
    if premises.is_empty() {
        return true;
    }
    let Some(entry) = program
        .machine_by_symbol(machine)
        .and_then(|machine| program.machine_states(machine).first())
    else {
        return false;
    };
    let Some(state) = checked.flow.control.state(machine, entry.symbol) else {
        return false;
    };
    premises.iter().all(|premise| {
        checked
            .flow
            .semantic_constraint_contexts(state.entry_constraints)
            .any(|context| {
                checked
                    .semantic
                    .context_view(checked.semantic.contexts.get(context))
                    .facts()
                    .any(|fact| {
                        let authored_entry = match (fact.origin, fact.payload) {
                            (
                                facts::FactOrigin::MachineContract { machine_symbol },
                                facts::FactPayload::ContractDomainMembership {
                                    kind: facts::ContractFactKind::Requires,
                                    ..
                                },
                            ) => machine_symbol == machine,
                            (
                                facts::FactOrigin::StateContract {
                                    machine_symbol,
                                    state_symbol,
                                },
                                facts::FactPayload::ContractDomainMembership {
                                    kind: facts::ContractFactKind::Requires,
                                    ..
                                },
                            )
                            | (
                                facts::FactOrigin::StateParameterDomain {
                                    machine_symbol,
                                    state_symbol,
                                },
                                facts::FactPayload::DomainMembership { .. },
                            ) => machine_symbol == machine && state_symbol == entry.symbol,
                            _ => false,
                        };
                        authored_entry
                            && fact_subjects::fact_domain(program, fact.payload).is_some_and(
                                |symbol| {
                                    program.domain_definitions().iter().any(|domain| {
                                        domain.symbol == symbol
                                            && domain.semantic_id == premise.profile
                                    })
                                },
                            )
                            && fact_subjects::fact_subject(&checked.semantic, fact.place).as_ref()
                                == Some(&premise.subject)
                    })
            })
    })
}

/// A progress-profile precondition consumes the same exact subject transport
/// as termination analysis. It still needs an existing entry qualification or
/// an exact live receipt; a selected callee's promise establishes neither.
pub(crate) fn profile_requirement_covered(
    program: &dyn ::validation::CheckingInput,
    checked: &checked_trees::CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    required: &facts::Fact,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let Some((_, _, domain_symbol, instance)) = required.payload.domain_membership() else {
        return false;
    };
    let Some(domain) = program.domain_definition(domain_symbol).filter(|domain| {
        domain.classification == Some(language_semantics::DomainClassification::ProgressProfile)
            && typed_trees::domain::index_parameters(program, domain).is_empty()
            && (!instance.is_valid() || instance == domain.semantic_id)
    }) else {
        return false;
    };
    if !call.authored_source_custody_valid
        || crate::flow::captured_call_operands(program, &checked.flow, state, call).is_none()
    {
        return false;
    }
    let Some(handle) = required.place.as_place() else {
        return false;
    };
    let stored = checked.semantic.places.get(handle);
    let place = validation::CanonicalPlace {
        root: stored.root,
        segments: checked
            .semantic
            .place_segments
            .span_or_empty(stored.segments)
            .to_vec(),
    };
    let Some(machine) = program.machine_by_symbol(state.machine_symbol) else {
        return false;
    };
    let Some(places) = machine_summaries::selector_places(program, state, call, place) else {
        return false;
    };
    places.into_iter().all(|place| {
        machine_summaries::resolve_call_place_premises(
            program,
            &checked.flow,
            &checked.semantic,
            machine,
            state,
            call,
            place,
            domain.semantic_id,
            call_frames,
        )
        .is_some_and(|premises| {
            entry_covers_progress_premises(program, checked, machine.symbol, &premises)
        })
    })
}

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
}

pub(crate) fn analyze_checked_progress(
    program: &dyn ::validation::CheckingInput,
    flow: &FlowFacts,
    semantic: &facts::FactPlan,
    cleanup_dependencies: &arena::Arena<flow_effects::CleanupDependency>,
    proof_components: &[validation::ValidatedProofCallComponent],
    self_call_descents: &[validation::ValidatedSelfCallDescent],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    rank_arrivals: &checked_trees::CheckedRankArrivalPlan,
) -> Result<Vec<CheckedProgressSummary>, Vec<Diagnostic>> {
    let summaries = components::derive_summaries(
        program,
        flow,
        semantic,
        cleanup_dependencies,
        proof_components,
        self_call_descents,
        call_frames,
        rank_arrivals,
    );

    let mut diagnostics = Vec::new();
    for machine in program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody)
    {
        // Ranking diagnostics own local control-flow failure and can name the
        // missing or invalid witness precisely. Progress coverage begins only
        // after that independent obligation succeeds.
        if !super::retained_machine_local_summary(machine, rank_arrivals).promises_termination() {
            continue;
        }
        let Some(checked) = summaries
            .iter()
            .find(|summary| summary.machine == machine.symbol)
        else {
            continue;
        };
        let language_semantics::TerminationInterface::Published(published) =
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
