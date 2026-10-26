//! Complete runtime component admission belongs to the completed-effects
//! window. Early call topology and proof recursion do not read this result.
//! Source admission constrains provisional body readings through this common
//! component judgment, then retains its decision beside the same witnesses. Progress and executable
//! rank projection read the retained rows without rebuilding the source proof.

use super::{TopLevelSymbols, build_machine_call_graph, runtime_ranking};
use crate::machine_calls::graph_components::strongly_connected_components;
use checked_trees::{
    CheckedRankArrivalPlan, CheckedRankCoordinate, CheckedRuntimeNaturalState,
    CheckedRuntimeRankComponent, CheckedRuntimeRankEdge, CheckedRuntimeRankMember,
};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

/// Invocation-local call topology, prepared once before source admission. It
/// names recurrent runtime bodies only; membership supplies no ranking proof.
pub struct RuntimeRankComponents {
    adjacency: Vec<Vec<usize>>,
    components: Vec<Vec<usize>>,
}

impl RuntimeRankComponents {
    /// Tail eligibility is an execution obligation; it supplies no ranking or
    /// eventual-return evidence. Proof-only recurrence remains separately owned.
    pub fn require_tail_component(
        &self,
        program: &dyn crate::CheckingInput,
        component: &[usize],
    ) -> Result<(), &'static str> {
        let proof_only = crate::proof_only_classification(program);
        if component
            .iter()
            .any(|position| proof_only.is_proof_machine(program, &program.machines()[*position]))
        {
            return Err("runtime and proof-only machines cannot share a recursive component");
        }
        runtime_ranking::require_tail_component(program, component)
    }
    pub fn requires_arrival_witness(&self, machine_position: usize) -> bool {
        self.components
            .iter()
            .any(|component| component.contains(&machine_position))
    }
    /// Exact recurrent member positions. Self and mutual machine calls share
    /// one source-complete call-component judgment. A private unpromised
    /// productive self loop supplies no termination evidence and needs none.
    pub fn joint_components<'program>(
        &'program self,
        program: &'program dyn crate::CheckingInput,
    ) -> impl Iterator<Item = &'program [usize]> {
        self.components
            .iter()
            .filter(move |component| {
                component.len() > 1
                    || component.first().is_some_and(|position| {
                        let machine = &program.machines()[*position];
                        machine.termination_plan.implementation_witness.is_some()
                            || machine.termination_plan.interface.published().is_some_and(
                                language_semantics::TerminationGuarantee::promises_termination,
                            )
                    })
            })
            .map(Vec::as_slice)
    }

    /// Constrain the currently source-complete members, including outgoing
    /// edges, before extending a candidate prefix. A full prefix additionally
    /// owes mixed endpoint conservation and every-cycle strictness.
    pub fn judge(
        &self,
        program: &dyn crate::CheckingInput,
        component: &[usize],
        witnesses: &[runtime_ranking::RuntimeRankBodyWitness<'_>],
        frames: Option<&crate::CallFrameResolver<'_>>,
    ) -> Result<Option<super::ValidatedRuntimeNaturalComponent>, &'static str> {
        let proof_only = crate::proof_only_classification(program);
        if component
            .iter()
            .any(|position| proof_only.is_proof_machine(program, &program.machines()[*position]))
        {
            return Err("runtime and proof-only machines cannot share a recursive component");
        }
        runtime_ranking::project_component(program, &self.adjacency, component, witnesses, frames)
    }
}

pub fn prepare_runtime_rank_components(
    program: &dyn crate::CheckingInput,
) -> Option<RuntimeRankComponents> {
    let mut diagnostics = Vec::new();
    let symbols = TopLevelSymbols::build(program, &mut diagnostics);
    if diagnostics.iter().any(Diagnostic::is_error) {
        return None;
    }
    let proof_only = crate::proof_only_classification(program);
    let graph = build_machine_call_graph(program, &symbols, &proof_only);
    let components = strongly_connected_components(&graph.edges)
        .into_iter()
        .filter(|component| {
            (component.len() > 1
                || component
                    .first()
                    .is_some_and(|position| graph.edges[*position].contains(position)))
                && !component.iter().all(|position| {
                    proof_only.is_proof_machine(program, &program.machines()[*position])
                })
        })
        .collect();
    Some(RuntimeRankComponents {
        adjacency: graph.edges,
        components,
    })
}

/// Publish the complete search's decision; no source edge is judged here.
pub fn retain_runtime_rank_component(
    program: &dyn crate::CheckingInput,
    component: &[usize],
    decision: Result<Option<super::ValidatedRuntimeNaturalComponent>, &'static str>,
    admitted: &mut CheckedRankArrivalPlan,
) {
    let names = component
        .iter()
        .map(|position| program.machines()[*position].name.as_str())
        .collect::<Vec<_>>()
        .join("`, `");
    let (natural, diagnostic) = match decision {
        Ok(natural) => (natural, String::new()),
        Err(reason) => (
            None,
            format!(
                "machine call cycle: `{names}` -- {reason}; runtime calls must be tail; \
                 a termination promise additionally requires strict descent on every complete cycle"
            ),
        ),
    };
    let members = admitted
        .runtime_members
        .insert_many(component.iter().map(|position| {
            let machine = &program.machines()[*position];
            CheckedRuntimeRankMember {
                machine: machine.symbol,
                body: machine.states,
            }
        }));
    let (states, edges) = if let Some(natural) = natural {
        let states = natural
            .states
            .into_iter()
            .map(|state| {
                let path = admitted.path_segments.insert_many(state.coordinate.path);
                let coordinate = admitted.coordinates.insert(CheckedRankCoordinate {
                    parameter: state.coordinate.parameter,
                    path,
                });
                CheckedRuntimeNaturalState {
                    state: state.state,
                    coordinate,
                    parameter_position: state.parameter_position,
                    primitive_type: state.primitive_type,
                }
            })
            .collect::<Vec<_>>();
        let states = admitted.runtime_states.insert_many(states);
        let edges = admitted
            .runtime_edges
            .insert_many(
                natural
                    .edges
                    .into_iter()
                    .map(|edge| CheckedRuntimeRankEdge {
                        source: edge.source,
                        target: edge.target,
                        statement_ordinal: edge.statement_ordinal,
                        strict: edge.strict,
                    }),
            );
        (states, edges)
    } else {
        (Default::default(), Default::default())
    };
    admitted
        .runtime_components
        .insert(CheckedRuntimeRankComponent {
            members,
            states,
            edges,
            diagnostic,
        });
}

pub fn retained_runtime_recursive_components(
    admitted: &CheckedRankArrivalPlan,
) -> Vec<Vec<SymbolHandle>> {
    admitted
        .runtime_components
        .iter()
        .filter(|(_, row)| row.diagnostic.is_empty())
        .map(|(_, row)| {
            admitted
                .runtime_members
                .span_or_empty(row.members)
                .iter()
                .map(|member| member.machine)
                .collect()
        })
        .collect()
}

pub fn retained_runtime_natural_components(
    admitted: &CheckedRankArrivalPlan,
) -> Vec<super::ValidatedRuntimeNaturalComponent> {
    admitted
        .runtime_components
        .iter()
        .filter(|(_, row)| row.diagnostic.is_empty() && !row.states.is_empty())
        .filter_map(|(_, row)| {
            let states = admitted
                .runtime_states
                .span(row.states)?
                .iter()
                .map(|state| {
                    let coordinate = admitted.coordinates.get(state.coordinate);
                    Some(super::ValidatedRuntimeNaturalState {
                        state: state.state,
                        parameter: coordinate.parameter,
                        coordinate: crate::RankSubjectCoordinate {
                            parameter: coordinate.parameter,
                            path: admitted.path_segments.span(coordinate.path)?.to_vec(),
                        },
                        parameter_position: state.parameter_position,
                        primitive_type: state.primitive_type,
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            let edges = admitted
                .runtime_edges
                .span(row.edges)?
                .iter()
                .map(|edge| super::ValidatedRuntimeNaturalEdge {
                    source: edge.source,
                    target: edge.target,
                    statement_ordinal: edge.statement_ordinal,
                    strict: edge.strict,
                })
                .collect();
            Some(super::ValidatedRuntimeNaturalComponent { states, edges })
        })
        .collect()
}

/// Closing a runtime recursive call premise also consumes each local transfer
/// on its recurrent prefix. Site-local strict call descent cannot reset the
/// invocation's rank through an increasing acyclic hop. The complete body
/// admission already judged these exact occurrences; this query only reads.
pub fn retained_internal_rank_nonincrease(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    admitted: &CheckedRankArrivalPlan,
) -> bool {
    let Some(witness) = admitted.for_body(machine.symbol, machine.states) else {
        return false;
    };
    let Some(judgments) = admitted.arrival_edges.span(witness.edges) else {
        return false;
    };
    internal_rank_nonincrease(program, machine, judgments)
}

pub(super) fn internal_rank_nonincrease(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    judgments: &[checked_trees::CheckedRankArrivalEdge],
) -> bool {
    let check = || -> Option<()> {
        let relevance = super::rank_relevance(program, &[machine.symbol])?;
        let states = program.machine_states(machine);
        for arrival in runtime_ranking::internal_arrival_sites(program, machine) {
            let source = states.get(arrival.source)?.symbol;
            let destination = states.get(arrival.destination)?.symbol;
            if !relevance.requires_rank(destination) {
                continue;
            }
            let mut rows = judgments.iter().filter(|edge| {
                edge.source == source
                    && edge.destination == destination
                    && usize::try_from(edge.statement_ordinal).ok() == Some(arrival.statement_index)
                    && edge.target == arrival.target
            });
            if !rows.next()?.non_increasing || rows.next().is_some() {
                return None;
            }
        }
        Some(())
    };
    check().is_some()
}
