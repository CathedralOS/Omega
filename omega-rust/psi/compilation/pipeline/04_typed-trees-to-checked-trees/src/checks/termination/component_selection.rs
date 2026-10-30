//! A source-complete local choice remains provisional until its required
//! outgoing calls and the whole recurrent component accept the same choice.
//! The continuation extends one lazy path; each ready member constrains it
//! before another body's alternatives are visited. No candidate products or
//! durable proof rows are built during search.

use super::ranking::{self, RankArrivalAdmission};

pub(super) struct SelectedComponent {
    pub positions: Vec<usize>,
    pub source_natural: Option<super::source_natural::Component>,
    pub bodies: Vec<(usize, RankArrivalAdmission)>,
    pub decision: Result<Option<validation::ValidatedRuntimeNaturalComponent>, &'static str>,
    pub source_outcomes: Vec<(usize, ranking::DecreaseOutcome)>,
}

pub(super) fn select(
    program: &dyn validation::CheckingInput,
    topology: &validation::RuntimeRankComponents,
    component: &[usize],
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> SelectedComponent {
    if let Err(reason) = topology.require_tail_component(program, component) {
        return SelectedComponent {
            positions: component.to_vec(),
            source_natural: None,
            bodies: Vec::new(),
            decision: Err(reason),
            source_outcomes: Vec::new(),
        };
    }
    let requires_ranking = component.iter().any(|position| {
        let plan = &program.machines()[*position].termination_plan;
        plan.implementation_witness.is_some()
            || plan
                .interface
                .published()
                .is_some_and(language_semantics::TerminationGuarantee::promises_termination)
    });
    if !requires_ranking {
        return SelectedComponent {
            positions: component.to_vec(),
            source_natural: None,
            bodies: Vec::new(),
            decision: Ok(None),
            source_outcomes: Vec::new(),
        };
    }
    if let Some(decision) = super::joint_natural::select(program, component, frames) {
        let admitted = decision.is_ok();
        return SelectedComponent {
            positions: component.to_vec(),
            source_natural: None,
            bodies: Vec::new(),
            decision: decision.map(Some),
            source_outcomes: if admitted {
                component
                    .iter()
                    .map(|position| (*position, ranking::DecreaseOutcome::Proven(None)))
                    .collect()
            } else {
                Vec::new()
            },
        };
    }
    if let Some(result) = super::source_natural::admit(program, component, frames) {
        return match result {
            Ok(source) => SelectedComponent {
                positions: component.to_vec(),
                source_natural: Some(source),
                bodies: Vec::new(),
                decision: Ok(None),
                source_outcomes: component
                    .iter()
                    .map(|position| (*position, ranking::DecreaseOutcome::Proven(None)))
                    .collect(),
            },
            Err(message) => SelectedComponent {
                positions: component.to_vec(),
                source_natural: None,
                bodies: Vec::new(),
                decision: Err("a source natural call edge lacks independent strict descent"),
                source_outcomes: component
                    .iter()
                    .map(|position| {
                        (
                            *position,
                            ranking::DecreaseOutcome::Rejected(message.clone()),
                        )
                    })
                    .collect(),
            },
        };
    }
    let mut bodies = Vec::new();
    let mut chosen = None;
    let mut failure = "a member lacks a source-complete ranking witness";
    let mut source_outcomes = Vec::new();
    search(
        program,
        topology,
        component,
        frames,
        &mut bodies,
        &mut chosen,
        &mut failure,
        &mut source_outcomes,
    );
    let (bodies, decision) = chosen.map_or_else(
        || (Vec::new(), Err(failure)),
        |(bodies, decision)| (bodies, Ok(decision)),
    );
    SelectedComponent {
        positions: component.to_vec(),
        source_natural: None,
        bodies,
        decision,
        source_outcomes,
    }
}

type CompleteSelection = (
    Vec<(usize, RankArrivalAdmission)>,
    Option<validation::ValidatedRuntimeNaturalComponent>,
);

fn search(
    program: &dyn validation::CheckingInput,
    topology: &validation::RuntimeRankComponents,
    component: &[usize],
    frames: Option<&validation::CallFrameResolver<'_>>,
    bodies: &mut Vec<(usize, RankArrivalAdmission)>,
    chosen: &mut Option<CompleteSelection>,
    failure: &mut &'static str,
    source_outcomes: &mut Vec<(usize, ranking::DecreaseOutcome)>,
) -> bool {
    let Some(position) = component.get(bodies.len()).copied() else {
        return false;
    };
    let machine = &program.machines()[position];
    let mut source_complete = false;
    let mut accept = |arrival: &RankArrivalAdmission| {
        source_complete = true;
        bodies.push((position, arrival.clone()));
        let witnesses = bodies
            .iter()
            .map(|(position, arrival)| {
                let machine = &program.machines()[*position];
                validation::RuntimeRankBodyWitness {
                    machine: machine.symbol,
                    body: machine.states,
                    states: &arrival.states,
                    edges: &arrival.edges,
                    entry_rank_formed: machine
                        .termination_plan
                        .implementation_witness
                        .as_ref()
                        .is_some_and(|witness| witness.rank_range.is_some()),
                }
            })
            .collect::<Vec<_>>();
        let decision = topology.judge(program, component, &witnesses, frames);
        let accepted = match decision {
            Err(reason) => {
                *failure = reason;
                false
            }
            Ok(natural) if bodies.len() == component.len() => {
                *chosen = Some((bodies.clone(), natural));
                true
            }
            Ok(_) => search(
                program,
                topology,
                component,
                frames,
                bodies,
                chosen,
                failure,
                source_outcomes,
            ),
        };
        bodies.pop();
        accepted
    };
    let outcome = ranking::machine_decrease_outcome_with(program, machine, frames, &mut accept);
    // Genuine non-relational orders have no coordinate alternatives. An
    // entry-only body can still contribute its exact declaration identity.
    if matches!(outcome, ranking::DecreaseOutcome::Proven(None))
        && let Some(identity) = RankArrivalAdmission::entry_only(program, machine)
    {
        return accept(&identity);
    }
    if !source_complete {
        // A source failure before any complete candidate keeps its original
        // owning diagnostic. Rejection by the continuation instead belongs
        // to the call-component decision, not a manufactured range failure.
        if let Some(previous) = source_outcomes
            .iter_mut()
            .find(|(member, _)| *member == position)
        {
            previous.1 = outcome;
        } else {
            source_outcomes.push((position, outcome));
        }
    }
    chosen.is_some()
}
