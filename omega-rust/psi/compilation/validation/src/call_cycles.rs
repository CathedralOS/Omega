/*
Call-graph cycle detection, runtime tail-call ranking, and recursive proof
component validation (`machine_calls::call_cycles`).
*/
use crate::frozen_program::expression_table;
use crate::machine_calls::graph_components::strongly_connected_components;
use crate::value_custody::places::exact_data_member_field;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use typed_trees::types::TypeReferenceHandle;

pub(crate) mod proof_components {
    use super::{
        ExactProofCallEdge, collect_exact_proof_call_edges_in_machines, proof_call_site_sort_key,
        strongly_connected_components,
    };
    use std::collections::BTreeSet;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::machine::Machine;
    use typed_trees::state::State;

    pub(crate) struct CompleteProofCallComponent {
        pub members: Vec<usize>,
        pub edges: Vec<ExactProofCallEdge>,
    }

    pub(crate) fn complete(
        program: &dyn crate::CheckingInput,
        proof_only: &typed_trees::proof_only::ProofOnlyClassification,
        adjacency: &[Vec<usize>],
        proof_dependencies: &[Vec<usize>],
    ) -> Vec<CompleteProofCallComponent> {
        complete_with_demand(program, proof_only, adjacency, proof_dependencies, None)
    }

    pub(crate) fn complete_for_machine(
        program: &dyn crate::CheckingInput,
        proof_only: &typed_trees::proof_only::ProofOnlyClassification,
        adjacency: &[Vec<usize>],
        proof_dependencies: &[Vec<usize>],
        machine_position: usize,
    ) -> Vec<CompleteProofCallComponent> {
        complete_with_demand(
            program,
            proof_only,
            adjacency,
            proof_dependencies,
            Some(machine_position),
        )
    }

    fn complete_with_demand(
        program: &dyn crate::CheckingInput,
        proof_only: &typed_trees::proof_only::ProofOnlyClassification,
        adjacency: &[Vec<usize>],
        proof_dependencies: &[Vec<usize>],
        demanded: Option<usize>,
    ) -> Vec<CompleteProofCallComponent> {
        let machines = program.machines();
        let components = strongly_connected_components(adjacency);
        let demanded_members = demanded
            .and_then(|position| {
                components
                    .iter()
                    .find(|members| members.contains(&position))
            })
            .cloned();
        if demanded.is_some() && demanded_members.is_none() {
            return Vec::new();
        }
        let exact_edges = collect_exact_proof_call_edges_in_machines(
            program,
            proof_only,
            demanded_members.as_deref(),
        );
        let exact_self_recursive_members = exact_edges
            .iter()
            .filter(|edge| edge.caller == edge.callee)
            .map(|edge| edge.caller)
            .collect::<BTreeSet<_>>();
        let mut components = components
            .into_iter()
            .filter(|members| demanded.is_none_or(|position| members.contains(&position)))
            .filter(|members| {
                (members.len() > 1
                    || members
                        .first()
                        .is_some_and(|member| exact_self_recursive_members.contains(member)))
                    && members
                        .iter()
                        .all(|member| proof_only.is_proof_machine(program, &machines[*member]))
            })
            .collect::<Vec<_>>();
        components.sort_by_key(|members| members.first().copied().unwrap_or(usize::MAX));

        components
            .into_iter()
            .filter_map(|members| {
                let member_set = members.iter().copied().collect::<BTreeSet<_>>();
                let mut component_edges = exact_edges
                    .iter()
                    .filter(|edge| {
                        member_set.contains(&edge.caller) && member_set.contains(&edge.callee)
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                component_edges.sort_by_key(|edge| {
                    (
                        machines[edge.caller].symbol.arena_index(),
                        proof_call_site_sort_key(edge.site),
                        machines[edge.callee].symbol.arena_index(),
                    )
                });
                if component_edges.is_empty() {
                    return None;
                }
                let mut certified_calls = component_edges
                    .iter()
                    .filter(|edge| edge.caller != edge.callee)
                    .map(|edge| (edge.caller, edge.callee))
                    .collect::<Vec<_>>();
                let mut resolved_calls = members
                    .iter()
                    .flat_map(|caller| {
                        proof_dependencies[*caller]
                            .iter()
                            .filter(|callee| member_set.contains(callee))
                            .map(|callee| (*caller, *callee))
                    })
                    .collect::<Vec<_>>();
                certified_calls.sort_unstable();
                resolved_calls.sort_unstable();
                if certified_calls != resolved_calls {
                    return None;
                }
                Some(CompleteProofCallComponent {
                    members,
                    edges: component_edges,
                })
            })
            .collect()
    }

    pub(crate) fn structural_return_descends(
        program: &dyn crate::CheckingInput,
        origin: &Machine,
        state: &State,
        statement: usize,
        expression: ExpressionHandle,
    ) -> bool {
        let Some((caller, _)) =
            crate::machine_calls::calls::machine_state_by_symbol(program, state.symbol)
        else {
            return false;
        };
        if !crate::machine_calls::calls::collect_call_sites_in_statement(program, caller, state, statement)
        .iter().any(|site| matches!(site, crate::CallSite::Expression { expression: observed, .. } if *observed == expression)) { return false; }
        let Some((rank, _, _)) = super::proof_rank_parameter(program, origin) else {
            return false;
        };
        let descent = structural_descent_components(program);
        let containing = |symbol| {
            let mut components: Vec<usize> = program
                .machine_positions_for_symbol(symbol)
                .into_iter()
                .flat_map(|position| descent.by_machine[position as usize].iter().copied())
                .collect();
            components.sort_unstable();
            components.dedup();
            components
        };
        let with_caller = containing(caller.symbol);
        containing(origin.symbol)
            .into_iter()
            .filter(|component| with_caller.binary_search(component).is_ok())
            .any(|component| {
                let component = &descent.components[component];
                component
                    .validated
                    .get_or_init(|| {
                        super::proof_ranking::validate_component(
                            program,
                            &component.members,
                            &component.edges,
                        )
                    })
                    .as_ref()
                    .is_some_and(|validated| {
                        validated.members.iter().any(|member| {
                            member.machine == origin.symbol && member.rank_parameter == rank
                        })
                    })
            })
    }

    pub(crate) struct StructuralDescentComponent {
        members: Vec<usize>,
        edges: Vec<ExactProofCallEdge>,
        validated: std::sync::OnceLock<Option<super::ValidatedProofRecursiveComponent>>,
    }

    pub(crate) struct StructuralDescentComponents {
        components: Vec<StructuralDescentComponent>,
        by_machine: Vec<Vec<usize>>,
    }

    fn structural_descent_components(
        program: &dyn crate::CheckingInput,
    ) -> std::sync::Arc<StructuralDescentComponents> {
        crate::frozen_program::frozen_memo(
            program,
            |memos| &memos.structural_descent_components,
            || {
                let classification = crate::proof_only_classification(program);
                let graph = super::build_machine_call_graph(program, &classification);
                let components: Vec<_> = complete(
                    program,
                    &classification,
                    &graph.edges,
                    &graph.proof_dependencies,
                )
                .into_iter()
                .map(|component| StructuralDescentComponent {
                    members: component.members,
                    edges: component.edges,
                    validated: std::sync::OnceLock::new(),
                })
                .collect();
                let mut by_machine = vec![Vec::new(); program.machines().len()];
                for (index, component) in components.iter().enumerate() {
                    for &member in &component.members {
                        if by_machine[member].last() != Some(&index) {
                            by_machine[member].push(index);
                        }
                    }
                }
                StructuralDescentComponents {
                    components,
                    by_machine,
                }
            },
        )
    }
}
pub(crate) use proof_components::{StructuralDescentComponents, structural_return_descends};
pub(crate) mod proof_intervals {
    use super::{
        ExactProofCallEdge, ValidatedProofRankInterval, ValidatedProofRecursiveCallSite,
        proof_rank_parameter, strongly_connected_components,
    };
    use crate::proof_contracts::contract_entailment::proof_interval_strict_decrease_judged;

    pub(crate) fn validate_component(
        program: &dyn crate::CheckingInput,
        members: &[usize],
        calls: &[ExactProofCallEdge],
    ) -> Result<Vec<ValidatedProofRankInterval>, &'static str> {
        let machines = program.machines();
        let mut ranks = Vec::new();
        for member in members {
            let machine = &machines[*member];
            let Some((symbol, _, identity)) = proof_rank_parameter(program, machine) else {
                if machine.termination_plan.implementation_witness.is_some() {
                    return Err("an authored proof rank is not an entry parameter");
                }
                continue;
            };
            let entry = program
                .machine_states(machine)
                .first()
                .ok_or("ranked body has no entry")?;
            let (position, parameter) = program
                .state_parameters(entry)
                .iter()
                .enumerate()
                .find(|(_, parameter)| parameter.symbol == symbol)
                .ok_or("rank parameter is absent from its entry")?;
            if parameter.is_mutable {
                return Err("a computed proof rank must retain its entry value");
            }
            ranks.push((*member, position, parameter, identity));
        }
        let Some((_, _, _, rank_identity)) = ranks.first() else {
            return Err("the proof component has no authored ranking entry");
        };
        if ranks
            .iter()
            .any(|(_, _, _, identity)| identity != rank_identity)
        {
            return Err("ranked entries do not share one well-founded carrier");
        }
        let is_ranked = |member| ranks.iter().any(|(candidate, ..)| *candidate == member);
        let mut unranked = vec![Vec::new(); machines.len()];
        for call in calls {
            if !is_ranked(call.caller) && !is_ranked(call.callee) {
                unranked[call.caller].push(call.callee);
            }
        }
        if strongly_connected_components(&unranked)
            .iter()
            .any(|component| {
                component.len() > 1
                    || component
                        .first()
                        .is_some_and(|member| unranked[*member].contains(member))
            })
        {
            return Err("a helper-only cycle can avoid every ranked entry");
        }
        let ranked_entries = ranks
            .iter()
            .map(|(member, ..)| machines[*member].symbol)
            .collect::<Vec<_>>();
        let mut intervals = Vec::new();
        let mut common_relation = symbols::SymbolHandle::invalid();
        for (origin, _, measure, _) in &ranks {
            let mut pending = vec![*origin];
            let mut visited = Vec::new();
            while let Some(caller) = pending.pop() {
                if visited.contains(&caller) {
                    continue;
                }
                visited.push(caller);
                for call in calls.iter().filter(|call| call.caller == caller) {
                    let Some((_, rank_position, target_rank, _)) =
                        ranks.iter().find(|(member, ..)| *member == call.callee)
                    else {
                        pending.push(call.callee);
                        continue;
                    };
                    let (state_symbol, statement_index) = match call.site {
                        ValidatedProofRecursiveCallSite::Statement {
                            state,
                            statement_index,
                        }
                        | ValidatedProofRecursiveCallSite::Expression {
                            state,
                            statement_index,
                            ..
                        }
                        | ValidatedProofRecursiveCallSite::Transition {
                            state,
                            statement_index,
                            ..
                        } => (state, statement_index),
                    };
                    let state = program
                        .state_in_machine(&machines[caller], state_symbol)
                        .ok_or("closing call is absent from its recorded body")?;
                    let argument = *call
                        .arguments
                        .get(*rank_position)
                        .ok_or("closing call has no actual for the selected rank")?;
                    let Some(relation) = proof_interval_strict_decrease_judged(
                        program,
                        &machines[*origin],
                        state,
                        statement_index,
                        argument,
                        measure,
                        &ranked_entries,
                    ) else {
                        return Err(
                            "a computed closing argument is not proved smaller on every incoming path",
                        );
                    };
                    if common_relation.is_valid() && relation != common_relation {
                        return Err("computed intervals select different order relations");
                    }
                    common_relation = relation;
                    intervals.push(ValidatedProofRankInterval {
                        origin: machines[*origin].symbol,
                        origin_rank: measure.symbol,
                        caller: machines[caller].symbol,
                        callee: machines[call.callee].symbol,
                        callee_rank: target_rank.symbol,
                        site: call.site,
                        argument,
                        relation,
                    });
                }
            }
        }
        if intervals.is_empty() {
            return Err("no checked interval closes at a ranked entry");
        }
        Ok(intervals)
    }
}
pub(crate) mod proof_ranking {
    use super::{
        ExactProofCallEdge, ValidatedProofRankingRelation, ValidatedProofRecursiveComponent,
        ValidatedProofRecursiveEdge, ValidatedProofRecursiveMember, expression_member_path,
        proof_rank_parameter, strongly_connected_components,
    };
    use symbols::SymbolHandle;
    use typed_trees::types::TypeReferenceHandle;

    #[derive(Clone)]
    pub(crate) struct RankParameter {
        position: usize,
        symbol: SymbolHandle,
        type_reference: TypeReferenceHandle,
    }

    pub(crate) struct ComponentEdge<'a> {
        source: usize,
        target: usize,
        call: &'a ExactProofCallEdge,
    }

    pub(crate) fn validate_component(
        program: &dyn crate::CheckingInput,
        members: &[usize],
        calls: &[ExactProofCallEdge],
    ) -> Option<ValidatedProofRecursiveComponent> {
        let machines = program.machines();
        let rank_type_identity = members
            .iter()
            .find_map(|member| proof_rank_parameter(program, &machines[*member]))?
            .2;
        let mut candidates = Vec::with_capacity(members.len());
        for member in members {
            let machine = &machines[*member];
            let authored = proof_rank_parameter(program, machine);
            if authored.is_none()
                && (machine.termination_plan.implementation_witness.is_some() || machine.is_public)
            {
                return None;
            }
            let entry = program.machine_states(machine).first()?;
            let choices = program
                .state_parameters(entry)
                .iter()
                .enumerate()
                .filter(|(_, parameter)| {
                    parameter.symbol.is_valid()
                        && !parameter.is_mutable
                        && authored
                            .as_ref()
                            .is_none_or(|rank| rank.0 == parameter.symbol)
                        && program.package_qualified_type_identity(parameter.type_reference)
                            == rank_type_identity
                })
                .map(|(position, parameter)| RankParameter {
                    position,
                    symbol: parameter.symbol,
                    type_reference: parameter.type_reference,
                })
                .collect::<Vec<_>>();
            if choices.is_empty() {
                return None;
            }
            candidates.push(choices);
        }
        let edges = calls
            .iter()
            .map(|call| {
                let callee_entry = program.machine_states(&machines[call.callee]).first()?;
                if call.arguments.len() != program.state_parameters(callee_entry).len() {
                    return None;
                }
                Some(ComponentEdge {
                    source: members.iter().position(|member| *member == call.caller)?,
                    target: members.iter().position(|member| *member == call.callee)?,
                    call,
                })
            })
            .collect::<Option<Vec<_>>>()?;

        loop {
            let mut changed = false;
            for edge in &edges {
                let source = candidates[edge.source].clone();
                let target = candidates[edge.target].clone();
                candidates[edge.source].retain(|rank| {
                    target
                        .iter()
                        .any(|next| member_path(program, edge.call, rank, next).is_some())
                });
                candidates[edge.target].retain(|rank| {
                    source
                        .iter()
                        .any(|previous| member_path(program, edge.call, previous, rank).is_some())
                });
                if candidates[edge.source].is_empty() || candidates[edge.target].is_empty() {
                    return None;
                }
                changed |= source.len() != candidates[edge.source].len()
                    || target.len() != candidates[edge.target].len();
            }
            if !changed {
                break;
            }
        }
        if candidates.iter().any(|choices| choices.len() != 1) {
            return None;
        }
        let mut preserving = vec![Vec::new(); members.len()];
        let mut witnesses = Vec::with_capacity(edges.len());
        for edge in edges {
            let source = &candidates[edge.source][0];
            let target = &candidates[edge.target][0];
            let path = member_path(program, edge.call, source, target)?;
            if path.is_empty() {
                preserving[edge.source].push(edge.target);
            }
            witnesses.push(ValidatedProofRecursiveEdge {
                caller: machines[edge.call.caller].symbol,
                callee: machines[edge.call.callee].symbol,
                site: edge.call.site,
                caller_rank_parameter: source.symbol,
                callee_rank_parameter: target.symbol,
                strict_member_path: path,
            });
        }
        if strongly_connected_components(&preserving)
            .iter()
            .any(|component| {
                component.len() > 1
                    || component
                        .first()
                        .is_some_and(|member| preserving[*member].contains(member))
            })
        {
            return None;
        }
        let ranking_relation = if preserving.iter().all(Vec::is_empty) {
            ValidatedProofRankingRelation::StructuralSubterm
        } else {
            ValidatedProofRankingRelation::StructuralSubtermCycles
        };
        Some(ValidatedProofRecursiveComponent {
            members: members
                .iter()
                .enumerate()
                .map(|(position, member)| ValidatedProofRecursiveMember {
                    machine: machines[*member].symbol,
                    rank_parameter: candidates[position][0].symbol,
                })
                .collect(),
            ranking_relation,
            rank_type_identity,
            edges: witnesses,
        })
    }

    fn member_path(
        program: &dyn crate::CheckingInput,
        edge: &ExactProofCallEdge,
        source: &RankParameter,
        target: &RankParameter,
    ) -> Option<Vec<SymbolHandle>> {
        let (path, resulting_type) = expression_member_path(
            program,
            *edge.arguments.get(target.position)?,
            source.symbol,
            source.type_reference,
        )?;
        (program.package_qualified_type_identity(resulting_type)
            == program.package_qualified_type_identity(target.type_reference))
        .then_some(path)
    }
}
pub(crate) mod rank_components {
    use super::{build_machine_call_graph, runtime_ranking};
    use crate::machine_calls::graph_components::strongly_connected_components;
    use checked_trees::{
        CheckedRankArrivalPlan, CheckedRankCoordinate, CheckedRuntimeIntegerRank,
        CheckedRuntimeNaturalState, CheckedRuntimeRankComponent, CheckedRuntimeRankEdge,
        CheckedRuntimeRankMember,
    };
    use symbols::SymbolHandle;

    pub struct RuntimeRankComponents {
        adjacency: Vec<Vec<usize>>,
        components: Vec<Vec<usize>>,
    }

    impl RuntimeRankComponents {
        pub fn require_tail_component(
            &self,
            program: &dyn crate::CheckingInput,
            component: &[usize],
        ) -> Result<(), &'static str> {
            let proof_only = crate::proof_only_classification(program);
            if component.iter().any(|position| {
                proof_only.is_proof_machine(program, &program.machines()[*position])
            }) {
                return Err("runtime and proof-only machines cannot share a recursive component");
            }
            runtime_ranking::require_tail_component(program, component)
        }
        pub fn requires_arrival_witness(&self, machine_position: usize) -> bool {
            self.component_containing(machine_position).is_some()
        }
        pub fn component_containing(&self, machine_position: usize) -> Option<&[usize]> {
            self.components
                .iter()
                .find(|component| component.contains(&machine_position))
                .map(Vec::as_slice)
        }
        pub fn unranked_component_reached_from(
            &self,
            program: &dyn crate::CheckingInput,
            origin: usize,
        ) -> Option<&[usize]> {
            let ranked = |component: &[usize]| {
                component.iter().any(|position| {
                    let plan = &program.machines()[*position].termination_plan;
                    plan.implementation_witness.is_some()
                        || plan.interface.published().is_some_and(
                            language_semantics::TerminationGuarantee::promises_termination,
                        )
                })
            };
            let mut seen = vec![false; self.adjacency.len()];
            *seen.get_mut(origin)? = true;
            let mut pending = self.adjacency.get(origin)?.clone();
            while let Some(position) = pending.pop() {
                if std::mem::replace(seen.get_mut(position)?, true) {
                    continue;
                }
                if let Some(component) = self.component_containing(position) {
                    if !component.contains(&origin) && !ranked(component) {
                        return Some(component);
                    }
                    continue;
                }
                pending.extend(self.adjacency.get(position)?.iter().copied());
            }
            None
        }
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

        pub fn judge(
            &self,
            program: &dyn crate::CheckingInput,
            component: &[usize],
            witnesses: &[runtime_ranking::RuntimeRankBodyWitness<'_>],
            frames: Option<&crate::CallFrameResolver<'_>>,
        ) -> Result<Option<super::ValidatedRuntimeNaturalComponent>, &'static str> {
            let proof_only = crate::proof_only_classification(program);
            if component.iter().any(|position| {
                proof_only.is_proof_machine(program, &program.machines()[*position])
            }) {
                return Err("runtime and proof-only machines cannot share a recursive component");
            }
            runtime_ranking::project_component(
                program,
                &self.adjacency,
                component,
                witnesses,
                frames,
            )
        }
    }

    pub fn prepare_runtime_rank_components(
        program: &dyn crate::CheckingInput,
    ) -> Option<RuntimeRankComponents> {
        let proof_only = crate::proof_only_classification(program);
        let graph = build_machine_call_graph(program, &proof_only);
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
        let (states, edges) =
            if let Some(natural) = natural {
                let mut states = Vec::with_capacity(natural.states.len());
                for state in natural.states {
                    let mut values = Vec::with_capacity(state.values.len());
                    for value in state.values {
                        let path = admitted.path_segments.insert_many(value.coordinate.path);
                        let entry_indexes = admitted.entry_indexes.insert_many(value.entry_indexes);
                        let coordinate = admitted.coordinates.insert(CheckedRankCoordinate {
                            parameter: value.coordinate.parameter,
                            path,
                            entry_indexes,
                        });
                        let (
                            application_measure,
                            application_body,
                            application_casts,
                            application_calls,
                            application_dispatches,
                        ) = match value.application {
                            Some(application) => (
                                application.owner(),
                                application.body(),
                                admitted
                                    .application_casts
                                    .insert_many(application.casts().iter().cloned()),
                                admitted
                                    .application_calls
                                    .insert_many(application.calls().iter().copied()),
                                admitted
                                    .application_dispatches
                                    .insert_many(application.dispatches().iter().copied()),
                            ),
                            None => (
                                SymbolHandle::invalid(),
                                typed_trees::expression::ExpressionHandle::invalid(),
                                Default::default(),
                                Default::default(),
                                Default::default(),
                            ),
                        };
                        values.push(CheckedRuntimeIntegerRank {
                            application: checked_trees::CheckedRankApplicationFacts {
                                owner: application_measure,
                                definition_body: application_body,
                                casts: application_casts,
                                calls: application_calls,
                                dispatches: application_dispatches,
                            },
                            application_bindings: {
                                let bindings = value
                                    .application_bindings
                                    .iter()
                                    .map(|binding| binding.retain(admitted))
                                    .collect::<Vec<_>>();
                                admitted.bindings.insert_many(bindings)
                            },
                            input_type: value.input_type,
                            measure: value.measure,
                            coordinate,
                            parameter_position: value.parameter_position,
                            primitive_type: value.primitive_type,
                            observation: value.observation.retain(admitted, program),
                            function: value.function,
                        });
                    }
                    let values = admitted.runtime_values.insert_many(values);
                    states.push(CheckedRuntimeNaturalState {
                        state: state.state,
                        values,
                    });
                }
                let states = admitted.runtime_states.insert_many(states);
                let edges =
                    admitted
                        .runtime_edges
                        .insert_many(natural.edges.into_iter().map(|edge| {
                            CheckedRuntimeRankEdge {
                                source: edge.source,
                                target: edge.target,
                                statement_ordinal: edge.statement_ordinal,
                                strict: edge.strict,
                            }
                        }));
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
            .filter(|(_, row)| {
                row.diagnostic.is_empty() && (!row.states.is_empty() || !row.edges.is_empty())
            })
            .filter_map(|(_, row)| {
                let states =
                    admitted
                        .runtime_states
                        .span(row.states)?
                        .iter()
                        .map(|state| {
                            let values =
                                admitted
                                    .runtime_values
                                    .span(state.values)?
                                    .iter()
                                    .map(|value| {
                                        let coordinate = admitted.coordinates.get(value.coordinate);
                                        Some(super::ValidatedRuntimeIntegerRank {
                                application: if value.application.definition_body.is_valid() {
                                    if value.application.owner != value.measure {
                                        return None;
                                    }
                                    Some(crate::IntegerApplicationFormation::from_retained(
                                        value.application.owner,
                                        value.application.definition_body,
                                        admitted
                                            .application_casts
                                            .span(value.application.casts)?
                                            .to_vec(),
                                        admitted
                                            .application_calls
                                            .span(value.application.calls)?
                                            .to_vec(),
                                        admitted
                                            .application_dispatches
                                            .span(value.application.dispatches)?
                                            .to_vec(),
                                    )?)
                                } else {
                                    if value.application.owner.is_valid()
                                        || !value.application.casts.is_empty()
                                        || !value.application.calls.is_empty()
                                        || !value.application.dispatches.is_empty()
                                    {
                                        return None;
                                    }
                                    None
                                },
                                application_bindings: admitted
                                    .bindings
                                    .span(value.application_bindings)?
                                    .iter()
                                    .map(|binding| {
                                        super::RuntimeRankApplicationBinding::from_checked(
                                            admitted, binding,
                                        )
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                                input_type: value.input_type,
                                measure: value.measure,
                                parameter: coordinate.parameter,
                                coordinate: crate::RankSubjectCoordinate {
                                    parameter: coordinate.parameter,
                                    path: admitted.path_segments.span(coordinate.path)?.to_vec(),
                                },
                                entry_indexes: admitted
                                    .entry_indexes
                                    .span(coordinate.entry_indexes)?
                                    .to_vec(),
                                parameter_position: value.parameter_position,
                                primitive_type: value.primitive_type,
                                observation: super::RuntimeRankInput::from_checked(
                                    admitted,
                                    value.observation,
                                )?,
                                function: value.function,
                            })
                                    })
                                    .collect::<Option<Vec<_>>>()?;
                            Some(super::ValidatedRuntimeNaturalState {
                                state: state.state,
                                values,
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

    pub(crate) fn internal_rank_nonincrease(
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
                        && usize::try_from(edge.statement_ordinal).ok()
                            == Some(arrival.statement_index)
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
}
pub(crate) mod rank_relevance {
    use crate::CheckingInput;
    use crate::machine_calls::calls::{
        call_target_state, collect_call_sites_in_statement, find_state,
    };
    use crate::machine_calls::graph_components::strongly_connected_components;
    use symbols::SymbolHandle;
    use typed_trees::machine::SatisfiedDeclaration;

    pub struct RankRelevance {
        states: Vec<(SymbolHandle, bool)>,
    }

    impl RankRelevance {
        pub fn requires_rank(&self, state: SymbolHandle) -> bool {
            crate::queries::substitutions::actual_for(&self.states, state).unwrap_or(true)
        }
    }

    pub fn rank_relevance(
        program: &dyn CheckingInput,
        ranked_machines: &[SymbolHandle],
    ) -> Option<RankRelevance> {
        let mut states = Vec::<(&typed_trees::machine::Machine, &typed_trees::state::State)>::new();
        for symbol in ranked_machines {
            let machine = program.machine_by_symbol(*symbol)?;
            for state in program.machine_states(machine) {
                if !states
                    .iter()
                    .any(|(_, stored)| stored.symbol == state.symbol)
                {
                    states.push((machine, state));
                }
            }
        }
        let mut adjacency = Vec::<Vec<usize>>::new();
        let mut opaque = Vec::new();
        let mut position = 0;
        while let Some(&(machine, state)) = states.get(position) {
            let mut targets = Vec::new();
            let mut has_opaque_call = false;
            let callees = if machine.target_family_interface {
                match crate::machine_calls::calls::target_family_members(program, machine) {
                    Some(members) => members.iter().map(|member| member.machine).collect(),
                    None => {
                        has_opaque_call = true;
                        Vec::new()
                    }
                }
            } else {
                program
                    .statement_table
                    .indexed_statements(state.statement_nodes)
                    .flat_map(|(statement_index, _)| {
                        collect_call_sites_in_statement(program, machine, state, statement_index)
                    })
                    .map(|call| call.target_symbol(program))
                    .collect::<Vec<_>>()
            };
            for symbol in callees {
                if boundary_excludes_synchronous_entry(program, symbol) {
                    continue;
                }
                let Some(target) = call_target_state(program, symbol) else {
                    has_opaque_call = true;
                    continue;
                };
                let Some((owner, target)) = find_state(program, target.symbol) else {
                    has_opaque_call = true;
                    continue;
                };
                if !owner.body_is_present && !owner.target_family_interface {
                    has_opaque_call = true;
                    continue;
                }
                let target_position = states
                    .iter()
                    .position(|(_, stored)| stored.symbol == target.symbol)
                    .unwrap_or_else(|| {
                        let target_position = states.len();
                        states.push((owner, target));
                        target_position
                    });
                targets.push(target_position);
            }
            targets.sort_unstable();
            targets.dedup();
            adjacency.push(targets);
            opaque.push(has_opaque_call);
            position += 1;
        }
        let mut relevant = opaque;
        for component in strongly_connected_components(&adjacency) {
            let cyclic = component.len() > 1
                || component
                    .first()
                    .is_some_and(|position| adjacency[*position].contains(position));
            if cyclic
                && component
                    .iter()
                    .any(|position| ranked_machines.contains(&states[*position].0.symbol))
            {
                for position in component {
                    relevant[position] = true;
                }
            }
        }
        let mut predecessors = vec![Vec::new(); states.len()];
        for (source, targets) in adjacency.iter().enumerate() {
            for target in targets {
                predecessors[*target].push(source);
            }
        }
        let mut ready = relevant
            .iter()
            .enumerate()
            .filter_map(|(position, relevant)| relevant.then_some(position))
            .collect::<Vec<_>>();
        let mut position = 0;
        while let Some(&target) = ready.get(position) {
            for source in &predecessors[target] {
                if !relevant[*source] {
                    relevant[*source] = true;
                    ready.push(*source);
                }
            }
            position += 1;
        }
        Some(RankRelevance {
            states: states
                .into_iter()
                .zip(relevant)
                .map(|((_, state), relevant)| (state.symbol, relevant))
                .collect(),
        })
    }

    fn boundary_excludes_synchronous_entry(
        program: &dyn CheckingInput,
        target: SymbolHandle,
    ) -> bool {
        if let Some((owner, signature)) = program.trait_machine_signature(target) {
            return owner.is_boundary && signature.invokes.is_empty();
        }
        if let Some(operator) = program.operator_definition_by_symbol(target) {
            return operator.is_boundary;
        }
        let Some((machine, _)) = crate::machine_calls::calls::find_machine_head(program, target)
        else {
            return false;
        };
        if machine.body_is_present
            || !matches!(
                machine.supply_mode,
                language_semantics::MachineSupplyMode::Boundary
                    | language_semantics::MachineSupplyMode::TopLevelRequirement
                    | language_semantics::MachineSupplyMode::ExternalRealization { .. }
            )
            || !machine.invokes.is_empty()
        {
            return false;
        }
        let requirements = program.machine_trait_conformances(machine);
        (!matches!(
            machine.supply_mode,
            language_semantics::MachineSupplyMode::ExternalRealization { .. }
        ) || !requirements.is_empty())
            && requirements.iter().all(|conformance| {
                match typed_trees::machine::resolve_satisfied_declaration(
                    program.typed_program(),
                    machine,
                    conformance,
                ) {
                    Some(SatisfiedDeclaration::Trait {
                        definition,
                        requirement,
                    }) => definition.is_boundary && requirement.invokes.is_empty(),
                    Some(SatisfiedDeclaration::TopLevelRequirement(requirement)) => {
                        requirement.invokes.is_empty()
                    }
                    Some(SatisfiedDeclaration::Operator(operator)) => operator.is_boundary,
                    _ => false,
                }
            })
    }
}
pub use rank_components::{
    RuntimeRankComponents, prepare_runtime_rank_components, retain_runtime_rank_component,
    retained_internal_rank_nonincrease, retained_runtime_natural_components,
    retained_runtime_recursive_components,
};
pub(crate) mod runtime_ranking {
    pub(crate) mod meaning {
        use typed_trees::expression::{BinaryOperator, ExpressionHandle, TableBinaryExpression};
        use typed_trees::machine::Machine;
        use typed_trees::state::State;

        pub(crate) fn binary_is_inert(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            state: &State,
            expression: ExpressionHandle,
            binary: &TableBinaryExpression,
        ) -> bool {
            let spelling = match binary.operator {
                BinaryOperator::CaseMembership => {
                    return crate::proof_contracts::bound_expression_meaning::has_exact_case_membership_meaning(
                program, machine, Some(state), expression, binary,
            );
                }
                operator => {
                    let Some(spelling) = typed_trees::operator::binary_operator_spelling(operator)
                    else {
                        return true;
                    };
                    spelling
                }
            };
            let operand_types = [binary.left, binary.right].map(|operand| {
                crate::proof_contracts::bound_expression_meaning::operand_type(
                    program,
                    machine,
                    Some(state),
                    super::projection::unwrapped(program, operand),
                )
            });
            typed_trees::operator::has_builtin_spelled_expression_meaning(
                program,
                machine.symbol,
                expression,
                spelling,
                &operand_types,
            ) || crate::proof_contracts::bound_expression_meaning::sealed_float_comparison(
                program,
                machine,
                Some(state),
                expression,
                binary,
            )
        }
    }
    pub(crate) mod occurrences {
        use crate::machine_calls::calls::{
            collect_call_sites_in_statement, transition_call_target,
        };

        pub(crate) fn require_classified_tail_occurrences(
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            statement_ordinal: usize,
            component: &[usize],
            dispatched: &[(symbols::SymbolHandle, Vec<usize>)],
        ) -> Result<(), &'static str> {
            for (call_ordinal, call) in
                collect_call_sites_in_statement(program, machine, state, statement_ordinal)
                    .into_iter()
                    .enumerate()
            {
                let target = call.target_symbol(program);
                if let Some((_, implementations)) =
                    dispatched.iter().find(|(slot, _)| *slot == target)
                    && implementations
                        .iter()
                        .any(|callee| component.contains(callee))
                {
                    let Some(target_handle) = transition_call_target(
                        program,
                        machine,
                        state,
                        statement_ordinal,
                        call_ordinal,
                    ) else {
                        return Err("an internal call occurrence lacks a classified tail edge");
                    };
                    if !target_handle.is_valid()
                        || !call.completes_transition(program, target_handle)
                    {
                        return Err("an internal call occurrence lacks a classified tail edge");
                    }
                    continue;
                }
                let Some(callee) = super::target_machine(program, target) else {
                    continue;
                };
                if !component.contains(&callee)
                    || (program.machines()[callee].symbol == machine.symbol
                        && super::internal_state_index(program, machine, target).is_some())
                {
                    continue;
                }
                let Some(target_handle) = transition_call_target(
                    program,
                    machine,
                    state,
                    statement_ordinal,
                    call_ordinal,
                ) else {
                    return Err("an internal call occurrence lacks a classified tail edge");
                };
                if !target_handle.is_valid() {
                    return Err("an internal call occurrence lacks a classified tail edge");
                }
                let is_tail_root = call.completes_transition(program, target_handle);
                if !is_tail_root {
                    return Err("an internal call occurrence lacks a classified tail edge");
                }
            }
            Ok(())
        }
    }
    pub(crate) mod prefix {
        use crate::machine_calls::calls::CallFrameResolver;
        use crate::{
            RankPremiseInput, RankPremiseObservation, frame_preserves_rank_premises,
            protect_rank_premise, rank_protected_path,
        };
        use facts::NormalizedWriteFrame;
        use typed_trees::machine::Machine;
        use typed_trees::statement::StatementNode;

        pub(crate) fn preserves_rank(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            state: &typed_trees::state::State,
            premises: Option<&[RankPremiseInput]>,
            entry_parameters: &crate::RankStateBindings,
            statement: &StatementNode,
            frames: Option<&CallFrameResolver<'_>>,
        ) -> bool {
            let mut protected = Vec::new();
            if premises.is_none() {
                for parameter in program.state_parameters(state) {
                    let Some(carrier) = rank_protected_path(
                        program,
                        parameter.symbol,
                        &[],
                        RankPremiseObservation::Value,
                    ) else {
                        return false;
                    };
                    protect_rank_premise(&mut protected, carrier);
                }
            }
            for binding in &entry_parameters.coordinates {
                let whole = || {
                    vec![(
                        binding.destination.path.clone(),
                        RankPremiseObservation::Value,
                    )]
                };
                let readings: Vec<(Vec<facts::PlaceSegment>, RankPremiseObservation)> =
                    match premises {
                        None => whole(),
                        Some(premises) => premises
                            .iter()
                            .filter(|input| input.parameter == binding.entry.parameter)
                            .map(|input| {
                                if let Some(suffix) =
                                    input.path.strip_prefix(binding.entry.path.as_slice())
                                {
                                    let mut path = binding.destination.path.clone();
                                    path.extend_from_slice(suffix);
                                    (path, input.observation)
                                } else {
                                    (
                                        binding.destination.path.clone(),
                                        RankPremiseObservation::Value,
                                    )
                                }
                            })
                            .collect(),
                    };
                if program
                    .state_parameter_by_symbol(state, binding.destination.parameter)
                    .is_none()
                {
                    return false;
                }
                for (path, observation) in readings {
                    let Some(carrier) = rank_protected_path(
                        program,
                        binding.destination.parameter,
                        &path,
                        observation,
                    ) else {
                        return false;
                    };
                    protect_rank_premise(&mut protected, carrier);
                }
            }
            let Some(premises) =
                crate::ranking_range_state_premise_coordinates(program, machine, state)
            else {
                return false;
            };
            for premise in premises {
                let Some(carrier) = rank_protected_path(
                    program,
                    premise.coordinate.parameter,
                    &premise.coordinate.path,
                    premise.observation,
                ) else {
                    return false;
                };
                protect_rank_premise(&mut protected, carrier);
            }
            let preserves =
                |frame: NormalizedWriteFrame| frame_preserves_rank_premises(frame, &protected);
            match statement {
                StatementNode::Assignment(assignment) => {
                    if !operand_has_admitted_meaning(program, machine, state, assignment.target)
                        || !operand_has_admitted_meaning(program, machine, state, assignment.value)
                    {
                        return false;
                    }
                    frames.is_some_and(|frames| {
                        preserves(frames.assignment_write_frame(machine, statement))
                    })
                }
                StatementNode::Call(call) => {
                    program
                        .statement_table
                        .expression_handles(call.arguments)
                        .iter()
                        .all(|argument| {
                            operand_has_admitted_meaning(program, machine, state, *argument)
                        })
                        && frames.is_some_and(|frames| {
                            preserves(frames.may_write_frame(machine, call))
                                && preserves(frames.statement_value_write_frame(machine, statement))
                        })
                }
                StatementNode::LocalData(local) => {
                    operand_has_admitted_meaning(program, machine, state, local.initial_value)
                        && frames.is_some_and(|frames| {
                            preserves(frames.expression_write_frame(machine, local.initial_value))
                        })
                }
                StatementNode::Transition(_) => frames.is_some_and(|frames| {
                    preserves(frames.transition_operand_write_frame(machine, statement))
                }),
                StatementNode::Expression(expression) => {
                    operand_has_admitted_meaning(program, machine, state, *expression)
                        && frames.is_some_and(|frames| {
                            preserves(frames.statement_value_write_frame(machine, statement))
                        })
                }
                _ => false,
            }
        }

        pub(crate) fn operand_has_admitted_meaning(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            state: &typed_trees::state::State,
            expression: typed_trees::expression::ExpressionHandle,
        ) -> bool {
            super::expression_is_inert_or_calls(program, machine, state, expression, &|call| {
                call.target_symbol.is_valid()
            })
        }
    }
    pub(crate) mod projection {
        use crate::frozen_program::expression_table;
        use crate::{RankingRangeMeasure, ScalarViewComputation, declared_scalar_view};
        use language_semantics::RankingViewId;
        use symbols::SymbolHandle;
        use typed_trees::data::DataMember;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::machine::Machine;
        use typed_trees::signature::StateParameter;
        use typed_trees::state::State;
        use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

        #[derive(PartialEq, Eq)]
        pub(crate) enum RankOrder {
            Natural(PrimitiveType),
            IncreasingTo(PrimitiveType),
            BoundedDistance(PrimitiveType),
            SliceLength,
            DeclaredIdentity {
                measure: SymbolHandle,
                primitive: PrimitiveType,
            },
            DeclaredComputation {
                measure: SymbolHandle,
                input_type: TypeReferenceHandle,
                result_primitive: PrimitiveType,
                computation: ScalarViewComputation,
            },
            Lexicographic {
                measure_index: usize,
                data: SymbolHandle,
                fields: Vec<SymbolHandle>,
            },
        }

        pub(crate) struct RankProjection {
            pub(crate) order: RankOrder,
            pub(crate) argument_position: usize,
            pub(crate) subject: ExpressionHandle,
            pub(crate) paired_subject: ExpressionHandle,
            pub(crate) range: ExpressionHandle,
            pub(crate) measure: Option<RankingRangeMeasure>,
        }

        impl RankProjection {
            pub(crate) fn resolve(
                program: &dyn crate::CheckingInput,
                machine: &Machine,
            ) -> Option<Self> {
                let witness = machine.termination_plan.implementation_witness.as_ref()?;
                let custody = program.ranking_expression_custody_for(machine.symbol)?;
                if witness.subjects.len() != custody.subjects.len()
                    || witness.rank_range.is_some() != custody.rank_range.is_some()
                    || custody.rank_range.is_some_and(|range| !range.is_valid())
                {
                    return None;
                }
                let entry = program.machine_states(machine).first()?;
                if witness.ranking_view == RankingViewId::NAT_BOUNDED_DISTANCE
                    && Some(witness.view_path.as_str()) == witness.ranking_view.canonical_path()
                {
                    let [lower, upper] = custody.subjects.as_slice() else {
                        return None;
                    };
                    if !witness.view_arguments.is_empty() || !custody.view_arguments.is_empty() {
                        return None;
                    }
                    let (argument_position, parameter) = entry_parameter(program, entry, *lower)?;
                    let primitive =
                        integer_subject_carrier(program, machine, entry, *lower, parameter)?;
                    return Some(Self {
                        order: RankOrder::BoundedDistance(primitive),
                        argument_position,
                        subject: *lower,
                        paired_subject: *upper,
                        range: custody.rank_range.unwrap_or_default(),
                        measure: Some(RankingRangeMeasure::Distance {
                            lower: *lower,
                            upper: *upper,
                        }),
                    });
                }
                let [subject] = custody.subjects.as_slice() else {
                    return None;
                };
                let (argument_position, parameter) = entry_parameter(program, entry, *subject)?;
                if witness.ranking_view == RankingViewId::SLICE_LENGTH
                    && Some(witness.view_path.as_str()) == witness.ranking_view.canonical_path()
                {
                    if !witness.view_arguments.is_empty()
                        || !custody.view_arguments.is_empty()
                        || !slice_subject_carrier(program, machine, entry, *subject, parameter)
                    {
                        return None;
                    }
                    return Some(Self::for_subject(
                        *subject,
                        argument_position,
                        RankOrder::SliceLength,
                        RankingRangeMeasure::SliceLength(*subject),
                        custody.rank_range.unwrap_or_default(),
                    ));
                }
                let pending_natural_default = !witness.ranking_view.is_valid()
                    && witness.view_path.is_empty()
                    && witness.view_arguments.is_empty()
                    && custody.view_arguments.is_empty()
                    && member_subject(program, *subject)
                    && integer_subject_carrier(program, machine, entry, *subject, parameter)
                        .is_some_and(|primitive| {
                            matches!(
                                primitive,
                                typed_trees::unsigned_integer_primitive_pattern!()
                            )
                        });
                if pending_natural_default
                    || matches!(
                        witness.ranking_view,
                        RankingViewId::NAT_DESCENDING | RankingViewId::NAT_INCREASING_TO
                    ) && Some(witness.view_path.as_str())
                        == witness.ranking_view.canonical_path()
                {
                    let increasing = witness.ranking_view == RankingViewId::NAT_INCREASING_TO;
                    let mut limit = ExpressionHandle::invalid();
                    if increasing {
                        let [bound] = custody.view_arguments.as_slice() else {
                            return None;
                        };
                        if witness.view_arguments.len() != 1 || !bound.is_valid() {
                            return None;
                        }
                        limit = *bound;
                    } else if !witness.view_arguments.is_empty()
                        || !custody.view_arguments.is_empty()
                    {
                        return None;
                    }
                    let primitive =
                        integer_subject_carrier(program, machine, entry, *subject, parameter)?;
                    return Some(Self::for_subject(
                        *subject,
                        argument_position,
                        if increasing {
                            RankOrder::IncreasingTo(primitive)
                        } else {
                            RankOrder::Natural(primitive)
                        },
                        if increasing {
                            RankingRangeMeasure::IncreasingTo {
                                subject: *subject,
                                limit,
                            }
                        } else {
                            RankingRangeMeasure::Single(*subject)
                        },
                        custody.rank_range.unwrap_or_default(),
                    ));
                }
                if witness.ranking_view.is_valid()
                    || !witness.view_arguments.is_empty()
                    || !custody.view_arguments.is_empty()
                {
                    return None;
                }
                if let Some(view) =
                    declared_scalar_view(program, entry, *subject, &witness.view_path)
                {
                    let order = match view.computation {
                        None => RankOrder::DeclaredIdentity {
                            measure: view.measure,
                            primitive: integer_subject_carrier(
                                program, machine, entry, *subject, parameter,
                            )?,
                        },
                        Some(computation) => RankOrder::DeclaredComputation {
                            measure: view.measure,
                            input_type: view.input_type,
                            result_primitive: PrimitiveType::from_builtin_atom(
                                view.result_carrier,
                            )?,
                            computation,
                        },
                    };
                    let measure = match view.computation {
                        None => RankingRangeMeasure::Single(*subject),
                        Some(computation) => RankingRangeMeasure::Computed {
                            subject: *subject,
                            parameter: computation.parameter,
                            body: computation.body,
                            carrier: view.result_carrier,
                        },
                    };
                    return Some(Self::for_subject(
                        *subject,
                        argument_position,
                        order,
                        measure,
                        custody.rank_range.unwrap_or_default(),
                    ));
                }
                if custody.rank_range.is_some() || member_subject(program, *subject) {
                    return None;
                }
                let TypeReferenceNode::Named { symbol: data, .. } = program
                    .type_reference_table
                    .type_reference(parameter.type_reference)
                else {
                    return None;
                };
                if !data.is_valid() {
                    return None;
                }
                let definition =
                    crate::declarations::data_queries::data_definition_by_symbol(program, *data)?;

                let mut measures = program
                    .measures()
                    .iter()
                    .enumerate()
                    .filter(|(_, measure)| {
                        program
                            .measure_path_members(measure.name)
                            .iter()
                            .map(|member| member.as_str())
                            .eq(witness.view_path.split("::"))
                    });
                let (measure_index, measure) = measures.next()?;
                if measures.next().is_some()
                    || !measure.lexicographic
                    || measure.parameter.is_some()
                {
                    return None;
                }
                let (owner, _) = witness.view_path.rsplit_once("::")?;
                if owner != program.symbols.display_path(*data, "::") {
                    return None;
                }

                let components = expression_table(program).expression_handles(measure.body);
                if components.is_empty() {
                    return None;
                }
                let mut fields = Vec::with_capacity(components.len());
                for component in components {
                    let component = expression_table(program).name_path_node(*component)?;
                    let [name] = expression_table(program).name_path_members(component.members)
                    else {
                        return None;
                    };
                    let mut declarations =
                        program
                            .data_members(definition)
                            .iter()
                            .filter_map(|member| match member {
                                DataMember::Field(field)
                                    if field.name.as_str() == name.as_str() =>
                                {
                                    Some(field)
                                }
                                _ => None,
                            });
                    let field = declarations.next()?;
                    if declarations.next().is_some()
                        || !field.symbol.is_valid()
                        || fields.contains(&field.symbol)
                        || (component.symbol.is_valid() && component.symbol != field.symbol)
                        || !program
                            .primitive_type_reference(field.type_reference)
                            .is_some_and(|primitive| primitive.is_fixed_integer())
                    {
                        return None;
                    }
                    fields.push(field.symbol);
                }
                Some(Self {
                    order: RankOrder::Lexicographic {
                        measure_index,
                        data: *data,
                        fields,
                    },
                    argument_position,
                    subject: *subject,
                    paired_subject: ExpressionHandle::invalid(),
                    range: ExpressionHandle::invalid(),
                    measure: None,
                })
            }

            fn for_subject(
                subject: ExpressionHandle,
                argument_position: usize,
                order: RankOrder,
                measure: RankingRangeMeasure,
                range: ExpressionHandle,
            ) -> Self {
                Self {
                    order,
                    argument_position,
                    subject,
                    paired_subject: ExpressionHandle::invalid(),
                    range,
                    measure: Some(measure),
                }
            }

            pub(crate) fn captured_components(
                &self,
                program: &dyn crate::CheckingInput,
            ) -> Option<Vec<crate::proof_contracts::contract_entailment::CapturedRankComponent>>
            {
                use crate::proof_contracts::contract_entailment::CapturedRankComponent;
                if let RankOrder::Lexicographic { data, fields, .. } = &self.order {
                    let definition = program
                        .data_definitions()
                        .iter()
                        .find(|definition| definition.symbol == *data)?;
                    return fields
                        .iter()
                        .map(|symbol| {
                            let field =
                                program.data_members(definition).iter().find_map(|member| {
                                    match member {
                                        DataMember::Field(field) if field.symbol == *symbol => {
                                            Some(field)
                                        }
                                        _ => None,
                                    }
                                })?;
                            let primitive =
                                program.primitive_type_reference(field.type_reference)?;
                            Some(CapturedRankComponent {
                                subject: self.subject,
                                projection: vec![facts::PlaceSegment::Field { symbol: *symbol }],
                                primitive: Some(primitive),
                                minimum: numerics::bignum::BigInt::from_i128(
                                    primitive.integer_bounds()?.0,
                                ),
                                measure: SymbolHandle::invalid(),
                            })
                        })
                        .collect();
                }
                let (primitive, result_primitive, measure) = match self.order {
                    RankOrder::Natural(primitive) => {
                        (Some(primitive), primitive, SymbolHandle::invalid())
                    }
                    RankOrder::DeclaredIdentity { measure, primitive } => {
                        (Some(primitive), primitive, measure)
                    }
                    RankOrder::DeclaredComputation {
                        measure,
                        input_type,
                        result_primitive,
                        ..
                    } => (
                        program.primitive_type_reference(input_type),
                        result_primitive,
                        measure,
                    ),
                    _ => return None,
                };
                Some(vec![CapturedRankComponent {
                    subject: self.subject,
                    projection: Vec::new(),
                    primitive,
                    minimum: if measure.is_valid() {
                        numerics::bignum::BigInt::from_i128(result_primitive.integer_bounds()?.0)
                    } else {
                        numerics::bignum::BigInt::zero()
                    },
                    measure,
                }])
            }

            pub(crate) fn same_order(&self, other: &Self) -> bool {
                self.order == other.order
            }
        }

        fn entry_parameter<'program>(
            program: &'program dyn crate::CheckingInput,
            entry: &'program State,
            subject: ExpressionHandle,
        ) -> Option<(usize, &'program StateParameter)> {
            let place =
                crate::canonical_place_from_expression_in_state(program, entry.symbol, 0, subject)?;
            let symbol = place.root.as_symbol()?;
            let mut parameters = program
                .state_parameters(entry)
                .iter()
                .enumerate()
                .filter(|(_, parameter)| parameter.symbol == symbol);
            let found = parameters.next()?;
            if parameters.next().is_some() {
                return None;
            }
            Some(found)
        }

        fn member_subject(program: &dyn crate::CheckingInput, subject: ExpressionHandle) -> bool {
            matches!(
                expression_table(program).expression(unwrapped(program, subject)),
                ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
            )
        }

        fn slice_subject_carrier(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            entry: &State,
            subject: ExpressionHandle,
            parameter: &StateParameter,
        ) -> bool {
            let reference = if member_subject(program, subject) {
                let Some(reference) =
                    crate::expression_result_type_reference(program, machine, entry, subject)
                else {
                    return false;
                };
                reference
            } else {
                parameter.type_reference
            };
            crate::value_custody::type_shapes::has_slice_base(program, reference)
        }

        fn integer_subject_carrier(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            entry: &State,
            subject: ExpressionHandle,
            parameter: &StateParameter,
        ) -> Option<PrimitiveType> {
            let reference = if member_subject(program, subject) {
                crate::expression_result_type_reference(program, machine, entry, subject)?
            } else {
                parameter.type_reference
            };
            let reference = program
                .type_reference_table
                .unconstrained_type_reference(reference);
            crate::value_custody::recasts::exact_primitive_type(program, reference)
                .filter(|primitive| primitive.is_fixed_integer())
        }

        pub(crate) use crate::queries::expression_traversal::unwrapped;

        pub(crate) fn natural_component(
            program: &dyn crate::CheckingInput,
            component: &[usize],
            ranks: &[RankProjection],
            mappings: &[Vec<crate::RankStateBindings>],
            relevance: &super::super::RankRelevance,
        ) -> Option<super::ValidatedRuntimeNaturalComponent> {
            let mut states = Vec::new();
            for ((machine_position, rank), mappings) in component.iter().zip(ranks).zip(mappings) {
                let inputs = match &rank.order {
                    RankOrder::Natural(primitive) => {
                        let coordinates = crate::ranking_range_required_coordinates(
                            program,
                            &program.machines()[*machine_position],
                            rank.range,
                            rank.measure?,
                            crate::RankingRangePremises::RankInvariant,
                        )?;
                        let [coordinate] = coordinates.as_slice() else {
                            return None;
                        };
                        vec![(coordinate.clone(), *primitive)]
                    }
                    RankOrder::Lexicographic { .. } => rank
                        .captured_components(program)?
                        .into_iter()
                        .map(|component| {
                            let mut place =
                                crate::canonical_place_from_expression(program, component.subject)?;
                            place.segments.extend(component.projection);
                            Some((
                                crate::RankSubjectCoordinate {
                                    parameter: place.root.as_symbol()?,
                                    path: place.segments,
                                },
                                component.primitive?,
                            ))
                        })
                        .collect::<Option<Vec<_>>>()?,
                    _ => return None,
                };
                for (state, roles) in program
                    .machine_states(&program.machines()[*machine_position])
                    .iter()
                    .zip(mappings)
                {
                    if !relevance.requires_rank(state.symbol) {
                        continue;
                    }
                    let values = inputs
                        .iter()
                        .map(|(entry, primitive_type)| {
                            let coordinate = roles.destination(entry)?;
                            let (position, parameter) =
                                program.state_parameters(state).iter().enumerate().find(
                                    |(_, parameter)| parameter.symbol == coordinate.parameter,
                                )?;
                            let reference =
                                crate::ranking_coordinate_type(program, state, &coordinate)?;
                            if crate::value_custody::recasts::exact_primitive_type(
                                program, reference,
                            ) != Some(*primitive_type)
                            {
                                return None;
                            }
                            Some(super::ValidatedRuntimeIntegerRank {
                                application: None,
                                application_bindings: Vec::new(),
                                input_type: reference,
                                measure: SymbolHandle::invalid(),
                                parameter: parameter.symbol,
                                coordinate,
                                entry_indexes: Vec::new(),
                                parameter_position: u32::try_from(position).ok()?,
                                primitive_type: *primitive_type,
                                observation: crate::RankPremiseObservation::Value.into(),
                                function: checked_trees::CheckedNaturalRankFunction::Identity,
                            })
                        })
                        .collect::<Option<Vec<_>>>()?;
                    states.push(super::ValidatedRuntimeNaturalState {
                        state: state.symbol,
                        values,
                    });
                }
            }
            Some(super::ValidatedRuntimeNaturalComponent {
                states,
                edges: Vec::new(),
            })
        }

        pub(crate) fn retain_internal_transfer(
            program: &dyn crate::CheckingInput,
            component: &mut super::ValidatedRuntimeNaturalComponent,
            component_states: &[SymbolHandle],
            relevance: &super::super::RankRelevance,
            source: SymbolHandle,
            target: SymbolHandle,
            target_handle: typed_trees::statement::TransitionTargetHandle,
            statement_ordinal: usize,
            judgments: &[checked_trees::CheckedRankArrivalEdge],
        ) -> bool {
            let target = program
                .machine(target)
                .and_then(|machine| program.machine_states(machine).first())
                .map_or(target, |state| state.symbol);
            if !relevance.requires_rank(target) {
                return true;
            }
            if !component_states.contains(&source) || !component_states.contains(&target) {
                return false;
            }
            let mut matching = judgments.iter().filter(|edge| {
                edge.source == source
                    && edge.destination == target
                    && edge.target == target_handle
                    && usize::try_from(edge.statement_ordinal).ok() == Some(statement_ordinal)
            });
            let Some(edge) = matching.next() else {
                return false;
            };
            if matching.next().is_some() || !edge.non_increasing {
                return false;
            }
            component.edges.push(super::ValidatedRuntimeNaturalEdge {
                source,
                target,
                statement_ordinal: edge.statement_ordinal,
                strict: edge.strictly_decreases,
            });
            true
        }
    }

    use crate::CheckingInput;
    use crate::RankStateBindings;
    use crate::frozen_program::expression_table;
    use symbols::SymbolHandle;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::proof_only::ProofOnlyClassification;
    use typed_trees::signature::SignatureContractKind;
    use typed_trees::state::State;
    use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

    use crate::RankingRangeMeasure;
    use crate::machine_calls::graph_components::strongly_connected_components;
    use crate::proof_contracts::contract_entailment::{
        RankingRangeCallMember, RankingRangeCallProgress, RankingRangeCallSite,
        call_member_premise_symbols, prove_captured_rank_call, prove_ranking_range_call,
    };
    use projection::RankProjection;
    pub(crate) mod inputs {
        use symbols::SymbolHandle;
        use typed_trees::{expression::ExpressionHandle, types::PrimitiveType};

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum RuntimeRankInput {
            Value(crate::RankPremiseObservation),
            Distance {
                lower_observation: crate::RankPremiseObservation,
                lower_primitive: PrimitiveType,
                upper: RuntimeRankOperand,
            },
        }

        impl From<crate::RankPremiseObservation> for RuntimeRankInput {
            fn from(value: crate::RankPremiseObservation) -> Self {
                Self::Value(value)
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum RuntimeRankOperand {
            Coordinate {
                parameter: SymbolHandle,
                coordinate: crate::RankSubjectCoordinate,
                entry_indexes: Vec<checked_trees::CheckedEntryIndexBound>,
                primitive_type: PrimitiveType,
                observation: crate::RankPremiseObservation,
            },
            Expression {
                state: SymbolHandle,
                evaluator: SymbolHandle,
                expression: ExpressionHandle,
                primitive_type: PrimitiveType,
                application: Option<crate::IntegerApplicationFormation>,
                bindings: Vec<super::RuntimeRankApplicationBinding>,
            },
            Literal(u64),
        }

        impl RuntimeRankInput {
            pub fn retain(
                &self,
                plan: &mut checked_trees::CheckedRankArrivalPlan,
                program: &dyn crate::CheckingInput,
            ) -> checked_trees::CheckedRuntimeRankInput {
                use checked_trees::CheckedRuntimeRankInput as Input;
                match self {
                    Self::Value(observation) => Input::Value(*observation),
                    Self::Distance {
                        lower_observation,
                        lower_primitive,
                        upper,
                    } => Input::Distance {
                        lower_observation: *lower_observation,
                        lower_primitive: *lower_primitive,
                        upper: upper.retain(plan, program),
                    },
                }
            }

            pub fn from_checked(
                plan: &checked_trees::CheckedRankArrivalPlan,
                input: checked_trees::CheckedRuntimeRankInput,
            ) -> Option<Self> {
                use checked_trees::CheckedRuntimeRankInput as Input;
                Some(match input {
                    Input::Value(observation) => Self::Value(observation),
                    Input::Distance {
                        lower_observation,
                        lower_primitive,
                        upper,
                    } => Self::Distance {
                        lower_observation,
                        lower_primitive,
                        upper: RuntimeRankOperand::from_checked(plan, upper)?,
                    },
                })
            }
        }

        impl RuntimeRankOperand {
            pub fn matches_checked(
                &self,
                plan: &checked_trees::CheckedRankArrivalPlan,
                handle: arena::Handle<checked_trees::CheckedRankOperand>,
            ) -> bool {
                Self::from_checked(plan, handle).as_ref() == Some(self)
            }

            pub fn retain(
                &self,
                plan: &mut checked_trees::CheckedRankArrivalPlan,
                program: &dyn crate::CheckingInput,
            ) -> arena::Handle<checked_trees::CheckedRankOperand> {
                use checked_trees::CheckedRankOperand as Operand;
                let operand = match self {
                    Self::Coordinate {
                        parameter,
                        coordinate,
                        entry_indexes,
                        primitive_type,
                        observation,
                    } => {
                        let path = plan.path_segments.insert_many(coordinate.path.clone());
                        let entry_indexes = plan.entry_indexes.insert_many(entry_indexes.clone());
                        let coordinate =
                            plan.coordinates
                                .insert(checked_trees::CheckedRankCoordinate {
                                    parameter: *parameter,
                                    path,
                                    entry_indexes,
                                });
                        Operand::Coordinate {
                            coordinate,
                            primitive_type: *primitive_type,
                            observation: *observation,
                        }
                    }
                    Self::Expression {
                        state,
                        evaluator,
                        expression,
                        primitive_type,
                        application,
                        bindings,
                    } => {
                        let formation = application
                            .as_ref()
                            .and_then(|application| {
                                let (machine, _) = crate::find_state(program, *evaluator)?;
                                if application.owner() != *state
                                    || application.body() != *expression
                                {
                                    return None;
                                }
                                let rows = bindings
                                    .iter()
                                    .map(|binding| binding.retain(plan))
                                    .collect::<Vec<_>>();
                                let bindings = plan.bindings.insert_many(rows);
                                let casts = plan
                                    .application_casts
                                    .insert_many(application.casts().iter().cloned());
                                let calls = plan
                                    .application_calls
                                    .insert_many(application.calls().iter().copied());
                                Some(plan.application_formations.insert(
                                    checked_trees::CheckedRankApplicationFormation {
                                        machine: machine.symbol,
                                        machine_body: machine.states,
                                        state: *evaluator,
                                        application: checked_trees::CheckedRankApplicationFacts {
                                            owner: application.owner(),
                                            definition_body: application.body(),
                                            casts,
                                            calls,
                                            dispatches: plan.application_dispatches.insert_many(
                                                application.dispatches().iter().copied(),
                                            ),
                                        },
                                        bindings,
                                        ..Default::default()
                                    },
                                ))
                            })
                            .unwrap_or_default();
                        Operand::Expression {
                            state: *state,
                            expression: *expression,
                            primitive_type: *primitive_type,
                            formation,
                        }
                    }
                    Self::Literal(value) => Operand::Literal(*value),
                };
                plan.rank_operands.insert(operand)
            }

            fn from_checked(
                plan: &checked_trees::CheckedRankArrivalPlan,
                handle: arena::Handle<checked_trees::CheckedRankOperand>,
            ) -> Option<Self> {
                use checked_trees::CheckedRankOperand as Operand;
                Some(match plan.rank_operands.get(handle) {
                    Operand::Coordinate {
                        coordinate,
                        primitive_type,
                        observation,
                    } => {
                        let coordinate = plan.coordinates.get(*coordinate);
                        Self::Coordinate {
                            parameter: coordinate.parameter,
                            coordinate: crate::RankSubjectCoordinate {
                                parameter: coordinate.parameter,
                                path: plan.path_segments.span(coordinate.path)?.to_vec(),
                            },
                            entry_indexes: plan
                                .entry_indexes
                                .span(coordinate.entry_indexes)?
                                .to_vec(),
                            primitive_type: *primitive_type,
                            observation: *observation,
                        }
                    }
                    Operand::Expression {
                        state,
                        expression,
                        primitive_type,
                        formation,
                    } => {
                        let (application, bindings, evaluator) = if formation.is_valid() {
                            let row = plan.application_formations.get(*formation);
                            if row.application.owner != *state
                                || row.application.definition_body != *expression
                            {
                                return None;
                            }
                            let application = crate::IntegerApplicationFormation::from_retained(
                                row.application.owner,
                                row.application.definition_body,
                                plan.application_casts.span(row.application.casts)?.to_vec(),
                                plan.application_calls.span(row.application.calls)?.to_vec(),
                                plan.application_dispatches
                                    .span(row.application.dispatches)?
                                    .to_vec(),
                            )?;
                            let bindings = plan
                                .bindings
                                .span(row.bindings)?
                                .iter()
                                .map(|binding| {
                                    super::RuntimeRankApplicationBinding::from_checked(
                                        plan, binding,
                                    )
                                })
                                .collect::<Option<Vec<_>>>()?;
                            (Some(application), bindings, row.state)
                        } else {
                            (None, Vec::new(), *state)
                        };
                        Self::Expression {
                            state: *state,
                            evaluator,
                            expression: *expression,
                            primitive_type: *primitive_type,
                            application,
                            bindings,
                        }
                    }
                    Operand::Literal(value) => Self::Literal(*value),
                })
            }
        }
    }
    pub use inputs::{RuntimeRankInput, RuntimeRankOperand};
    pub(crate) mod application_bindings {
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct RuntimeRankApplicationBinding {
            pub calls: Vec<typed_trees::expression::ExpressionHandle>,
            pub formal: crate::RankSubjectCoordinate,
            pub actual: crate::RankSubjectCoordinate,
            pub entry_indexes: Vec<checked_trees::CheckedEntryIndexBound>,
        }

        impl RuntimeRankApplicationBinding {
            pub fn retain(
                &self,
                plan: &mut checked_trees::CheckedRankArrivalPlan,
            ) -> checked_trees::CheckedRankCoordinateBinding {
                let entry = plan
                    .coordinates
                    .insert(checked_trees::CheckedRankCoordinate {
                        parameter: self.formal.parameter,
                        path: plan
                            .path_segments
                            .insert_many(self.formal.path.iter().cloned()),
                        ..Default::default()
                    });
                let destination = plan
                    .coordinates
                    .insert(checked_trees::CheckedRankCoordinate {
                        parameter: self.actual.parameter,
                        path: plan
                            .path_segments
                            .insert_many(self.actual.path.iter().cloned()),
                        entry_indexes: plan
                            .entry_indexes
                            .insert_many(self.entry_indexes.iter().cloned()),
                    });
                checked_trees::CheckedRankCoordinateBinding {
                    application_context: plan
                        .application_contexts
                        .insert_many(self.calls.iter().copied()),
                    entry,
                    destination,
                    ..Default::default()
                }
            }

            pub fn from_checked(
                plan: &checked_trees::CheckedRankArrivalPlan,
                binding: &checked_trees::CheckedRankCoordinateBinding,
            ) -> Option<Self> {
                let entry = plan.coordinates.get(binding.entry);
                let destination = plan.coordinates.get(binding.destination);
                Some(Self {
                    calls: plan
                        .application_contexts
                        .span(binding.application_context)?
                        .to_vec(),
                    formal: crate::RankSubjectCoordinate {
                        parameter: entry.parameter,
                        path: plan.path_segments.span(entry.path)?.to_vec(),
                    },
                    actual: crate::RankSubjectCoordinate {
                        parameter: destination.parameter,
                        path: plan.path_segments.span(destination.path)?.to_vec(),
                    },
                    entry_indexes: plan.entry_indexes.span(destination.entry_indexes)?.to_vec(),
                })
            }
        }
    }
    pub use application_bindings::RuntimeRankApplicationBinding;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ValidatedRuntimeNaturalState {
        pub state: SymbolHandle,
        pub values: Vec<ValidatedRuntimeIntegerRank>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ValidatedRuntimeIntegerRank {
        pub application: Option<crate::IntegerApplicationFormation>,
        pub application_bindings: Vec<RuntimeRankApplicationBinding>,
        pub input_type: typed_trees::types::TypeReferenceHandle,
        pub measure: SymbolHandle,
        pub parameter: SymbolHandle,
        pub coordinate: crate::RankSubjectCoordinate,
        pub entry_indexes: Vec<checked_trees::CheckedEntryIndexBound>,
        pub parameter_position: u32,
        pub primitive_type: typed_trees::types::PrimitiveType,
        pub observation: RuntimeRankInput,
        pub function: checked_trees::CheckedNaturalRankFunction,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ValidatedRuntimeNaturalEdge {
        pub source: SymbolHandle,
        pub target: SymbolHandle,
        pub statement_ordinal: u32,
        pub strict: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ValidatedRuntimeNaturalComponent {
        pub states: Vec<ValidatedRuntimeNaturalState>,
        pub edges: Vec<ValidatedRuntimeNaturalEdge>,
    }

    pub(crate) fn require_tail_component(
        program: &dyn CheckingInput,
        component: &[usize],
    ) -> Result<(), &'static str> {
        let mut dispatched = dispatched_implementations(program);
        dispatched.extend(supplied_requirement_implementations(program));
        for position in component {
            let machine = &program.machines()[*position];
            for state in program.machine_states(machine) {
                for (ordinal, statement) in program
                    .statement_table
                    .indexed_statements(state.statement_nodes)
                {
                    if matches!(statement, StatementNode::AssemblyFact(_)) {
                        continue;
                    }
                    occurrences::require_classified_tail_occurrences(
                        program,
                        machine,
                        state,
                        ordinal,
                        component,
                        &dispatched,
                    )?;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn extend_runtime_adjacency(
        program: &dyn CheckingInput,
        proof_only: &ProofOnlyClassification,
        adjacency: &mut [Vec<usize>],
    ) -> Vec<usize> {
        let dispatched = dispatched_implementations(program);
        let supplied = supplied_requirement_implementations(program);
        let mut selector_self_recursion = Vec::new();
        for (index, machine) in program.machines().iter().enumerate() {
            if proof_only.is_proof_machine(program, machine) {
                continue;
            }
            let mut targets = Vec::new();
            for state in program.machine_states(machine) {
                for statement in program.statement_table.statements(state.statement_nodes) {
                    if !matches!(statement, StatementNode::AssemblyFact(_)) {
                        super::collect_statement_dependency_symbols(
                            program,
                            statement,
                            &mut targets,
                        );
                    }
                }
            }
            for target in targets {
                if let Some((_, implementations)) =
                    dispatched.iter().find(|(slot, _)| *slot == target)
                {
                    adjacency[index].extend(implementations.iter().copied());
                    if implementations.contains(&index) && !selector_self_recursion.contains(&index)
                    {
                        selector_self_recursion.push(index);
                    }
                } else if let Some((_, implementations)) = supplied
                    .iter()
                    .find(|(requirement, _)| *requirement == target)
                {
                    adjacency[index].extend(implementations.iter().copied());
                    if implementations.contains(&index) && !selector_self_recursion.contains(&index)
                    {
                        selector_self_recursion.push(index);
                    }
                } else if let Some(callee) = target_machine(program, target)
                    && (callee != index
                        || target == machine.symbol
                        || program
                            .machine_states(machine)
                            .first()
                            .is_some_and(|entry| target == entry.symbol))
                {
                    adjacency[index].push(callee);
                }
            }
            adjacency[index].sort_unstable();
            adjacency[index].dedup();
        }
        selector_self_recursion
    }

    fn supplied_requirement_implementations(
        program: &dyn CheckingInput,
    ) -> Vec<(SymbolHandle, Vec<usize>)> {
        let mut supplied: Vec<(SymbolHandle, Vec<usize>)> = Vec::new();
        for conformance in program.conformances() {
            let boundary = program
                .trait_definition_by_symbol(conformance.trait_symbol)
                .is_some_and(|definition| definition.is_boundary);
            if !boundary {
                continue;
            }
            for row in &conformance.rows {
                let Some(implementation) = target_machine(program, row.realization_machine) else {
                    continue;
                };
                match supplied
                    .iter_mut()
                    .find(|(requirement, _)| *requirement == row.requirement)
                {
                    Some((_, implementations)) => implementations.push(implementation),
                    None => supplied.push((row.requirement, vec![implementation])),
                }
            }
        }
        for (_, implementations) in &mut supplied {
            implementations.sort_unstable();
            implementations.dedup();
        }
        supplied
    }

    fn dispatched_implementations(program: &dyn CheckingInput) -> Vec<(SymbolHandle, Vec<usize>)> {
        if program.dynamic_slots.is_empty() {
            return Vec::new();
        }
        let selections =
            crate::collect_dynamic_implementation_selections(program).unwrap_or_default();
        program
            .dynamic_slots
            .iter()
            .map(|slot| {
                let mut implementations = selections
                    .iter()
                    .filter(|selection| {
                        program.normalized_type_identity(selection.selector_type)
                            == slot.interface_identity
                    })
                    .filter_map(|selection| program.conformance_by_symbol(selection.implementation))
                    .flat_map(|conformance| {
                        conformance
                            .rows
                            .iter()
                            .filter(|row| row.requirement == slot.requirement)
                            .filter_map(|row| target_machine(program, row.realization_machine))
                    })
                    .collect::<Vec<_>>();
                implementations.sort_unstable();
                implementations.dedup();
                (slot.symbol, implementations)
            })
            .collect()
    }

    pub struct RuntimeRankBodyWitness<'selection> {
        pub machine: SymbolHandle,
        pub body: arena::HandleSpan<State>,
        pub states: &'selection [RankStateBindings],
        pub edges: &'selection [checked_trees::CheckedRankArrivalEdge],
        pub entry_rank_formed: bool,
    }

    pub(crate) fn project_component(
        program: &dyn CheckingInput,
        adjacency: &[Vec<usize>],
        component: &[usize],
        witnesses: &[RuntimeRankBodyWitness<'_>],
        frames: Option<&crate::CallFrameResolver<'_>>,
    ) -> Result<Option<ValidatedRuntimeNaturalComponent>, &'static str> {
        let Some(ranks) = component
            .iter()
            .map(|index| RankProjection::resolve(program, &program.machines()[*index]))
            .collect::<Option<Vec<_>>>()
        else {
            return Err("a member lacks a supported exact ranking witness");
        };
        if !ranks.iter().all(|rank| rank.same_order(&ranks[0])) {
            return Err("members do not share the same ranking order");
        }

        let body_witnesses = component
            .iter()
            .map(|index| {
                let machine = &program.machines()[*index];
                let mut matches = witnesses.iter().filter(|witness| {
                    witness.machine == machine.symbol && witness.body == machine.states
                });
                let witness = matches.next();
                if matches.next().is_some() {
                    return Err("the component has duplicate source body candidates");
                }
                Ok(witness)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let complete = body_witnesses.iter().all(Option::is_some);
        for (rank, witness) in ranks.iter().zip(&body_witnesses) {
            if rank.range.is_valid() && witness.is_some_and(|witness| !witness.entry_rank_formed) {
                return Err("a member's initial rank range is unproven");
            }
        }
        let member_mappings = component
            .iter()
            .zip(&body_witnesses)
            .map(|(index, witness)| {
                let Some(witness) = witness else {
                    return Ok(None);
                };
                let machine = &program.machines()[*index];
                let rows = program
                    .machine_states(machine)
                    .iter()
                    .map(|state| {
                        let mut matching = witness
                            .states
                            .iter()
                            .filter(|row| row.state == state.symbol);
                        let row = matching.next()?;
                        matching.next().is_none().then(|| row.clone())
                    })
                    .collect::<Option<Vec<_>>>()
                    .ok_or("the ranking needs complete entry-to-state arrival evidence")?;
                Ok(Some(rows))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let ranked_machines = component
            .iter()
            .map(|position| program.machines()[*position].symbol)
            .collect::<Vec<_>>();
        let relevance = super::rank_relevance(program, &ranked_machines)
            .ok_or("the ranking needs exact state recurrence topology")?;
        for (index, witness) in component.iter().zip(&body_witnesses) {
            if let Some(witness) = witness
                && !super::rank_components::internal_rank_nonincrease(
                    program,
                    &program.machines()[*index],
                    witness.edges,
                )
            {
                return Err("an internal arrival lacks its source component nonincrease result");
            }
        }
        let mut natural = if complete {
            let mappings = member_mappings
                .iter()
                .map(|rows| rows.as_ref().unwrap().clone())
                .collect::<Vec<_>>();
            Some(
                projection::natural_component(program, component, &ranks, &mappings, &relevance)
                    .unwrap_or(ValidatedRuntimeNaturalComponent {
                        states: Vec::new(),
                        edges: Vec::new(),
                    }),
            )
        } else {
            None
        };
        let member_premises = ranks
            .iter()
            .zip(component)
            .map(|(rank, index)| {
                call_member_premise_symbols(
                    program,
                    &RankingRangeCallMember {
                        machine: &program.machines()[*index],
                        subject: rank.subject,
                        paired_subject: rank.paired_subject,
                        range: rank.range,
                    },
                )
            })
            .collect::<Vec<_>>();
        let component_states = component
            .iter()
            .flat_map(|position| program.machine_states(&program.machines()[*position]))
            .map(|state| state.symbol)
            .collect::<Vec<_>>();
        let mut weak_edges = vec![Vec::new(); component_states.len()];
        for witness in body_witnesses.iter().flatten() {
            for edge in witness.edges {
                if edge.strictly_decreases || !relevance.requires_rank(edge.destination) {
                    continue;
                }
                let source = component_states
                    .iter()
                    .position(|state| *state == edge.source)
                    .ok_or("an internal rank edge has no component source")?;
                let destination = component_states
                    .iter()
                    .position(|state| *state == edge.destination)
                    .ok_or("an internal rank edge has no component destination")?;
                weak_edges[source].push(destination);
            }
        }
        for (position, index) in component.iter().copied().enumerate() {
            let machine = &program.machines()[index];
            let states = program.machine_states(machine);
            if states.is_empty() {
                return Err("a member has no checked entry state");
            }
            let Some(mappings) = &member_mappings[position] else {
                continue;
            };
            let mut observed = Vec::new();
            for (state_position, state) in states.iter().enumerate() {
                let mut guards = Vec::new();
                if state_position != 0 {
                    for contract in program.state_contracts(state) {
                        if contract.kind != SignatureContractKind::Requires {
                            continue;
                        }
                        for fact in program.proof_facts.span_or_empty(contract.facts) {
                            if let Some(expression) = fact.as_expression() {
                                guards.push((*expression, true));
                            }
                        }
                    }
                }
                let mut entry_relative = true;
                let captured_components = states
                    .first()
                    .is_some_and(|entry| entry.symbol == state.symbol)
                    && ranks[position].captured_components(program).is_some();
                for (statement_ordinal, statement) in program
                    .statement_table
                    .indexed_statements(state.statement_nodes)
                {
                    if !relevance.requires_rank(state.symbol) {
                        continue;
                    }
                    let Some(transition) = statement.as_transition() else {
                        if captured_components {
                            continue;
                        }
                        if prefix::preserves_rank(
                            program,
                            machine,
                            state,
                            member_premises[position].as_deref(),
                            &mappings[state_position],
                            statement,
                            frames,
                        ) {
                            continue;
                        }
                        match statement {
                            StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => continue,
                            StatementNode::LocalData(local)
                                if expression_is_inert(
                                    program,
                                    machine,
                                    state,
                                    local.initial_value,
                                ) =>
                            {
                                continue;
                            }
                            StatementNode::Expression(expression)
                                if expression_is_inert(program, machine, state, *expression) =>
                            {
                                continue;
                            }
                            StatementNode::Assignment(assignment)
                                if expression_is_inert(
                                    program,
                                    machine,
                                    state,
                                    assignment.target,
                                ) && expression_is_inert(
                                    program,
                                    machine,
                                    state,
                                    assignment.value,
                                ) =>
                            {
                                entry_relative = false;
                                if let Some(retained) = &mut natural {
                                    retained.states.clear();
                                }
                                continue;
                            }
                            _ => {
                                return Err(
                                    "a write, call, or alias invalidates the entry-relative ranking",
                                );
                            }
                        }
                    };
                    if transition.continuation.is_valid() {
                        return Err("a non-tail call edge returns into a continuation");
                    }
                    let guard = match transition.guard {
                        TransitionGuardNode::Always => ExpressionHandle::invalid(),
                        TransitionGuardNode::When(guard)
                            if prefix::operand_has_admitted_meaning(
                                program, machine, state, guard,
                            ) =>
                        {
                            guard
                        }
                        TransitionGuardNode::When(_) => {
                            return Err("a guard has effects or unproven operator meaning");
                        }
                    };
                    let mut site_guards = guards.clone();
                    if !captured_components
                        && !prefix::preserves_rank(
                            program,
                            machine,
                            state,
                            member_premises[position].as_deref(),
                            &mappings[state_position],
                            statement,
                            frames,
                        )
                    {
                        return Err(
                            "call operands invalidate the exact entry-relative ranking frame",
                        );
                    }
                    if guard.is_valid() {
                        site_guards.push((guard, true));
                        guards.push((guard, false));
                    }
                    let target_node = program.statement_table.transition_target(transition.target);
                    let tail_arrival =
                        crate::completing_transition_invocation(program, transition.target)
                            .map(|transfer| (transfer.target, transfer.arguments));
                    match tail_arrival {
                        Some((target_symbol, arguments)) => {
                            let callee = target_machine(program, target_symbol);
                            if callee == Some(index)
                                && internal_state_index(program, machine, target_symbol).is_some()
                            {
                                if let Some(retained) = &mut natural
                                    && !projection::retain_internal_transfer(
                                        program,
                                        retained,
                                        &component_states,
                                        &relevance,
                                        state.symbol,
                                        target_symbol,
                                        transition.target,
                                        statement_ordinal,
                                        body_witnesses[position].unwrap().edges,
                                    )
                                {
                                    return Err(
                                        "an internal arrival lost its exact admitted comparison",
                                    );
                                }
                                continue;
                            }
                            if !arguments.iter().all(|argument| {
                                prefix::operand_has_admitted_meaning(
                                    program, machine, state, *argument,
                                )
                            }) {
                                return Err(
                                    "a call argument has effects or non-builtin operator meaning",
                                );
                            }
                            if program.dynamic_slot_by_symbol(target_symbol).is_some() {
                                let data_arguments = arguments
                                    .get(1..)
                                    .ok_or("a selector call lost its data operands")?;
                                let implementations = dispatched_implementations(program);
                                let Some((_, implementations)) = implementations
                                    .iter()
                                    .find(|(slot, _)| *slot == target_symbol)
                                else {
                                    return Err(
                                        "a selector call names a slot with no implementation roster",
                                    );
                                };
                                for callee in implementations {
                                    let Some(callee_position) =
                                        component.iter().position(|index| *index == *callee)
                                    else {
                                        continue;
                                    };
                                    admit_ranked_edge(
                                        program,
                                        &ranks,
                                        &component_states,
                                        index,
                                        position,
                                        machine,
                                        state,
                                        &mappings[state_position],
                                        *callee,
                                        callee_position,
                                        data_arguments,
                                        entry_relative,
                                        body_witnesses[position].map(|witness| witness.edges),
                                        statement_ordinal,
                                        transition.target,
                                        &site_guards,
                                        &mut observed,
                                        &mut weak_edges,
                                        &mut natural,
                                    )?;
                                }
                                continue;
                            }
                            let Some(callee) = callee else {
                                let signature = program
                                    .retained_static_machine_contract(target_symbol)
                                    .map(|contract| contract.signature())
                                    .or_else(|| {
                                        crate::selected_requirement_signature(
                                            program,
                                            target_symbol,
                                        )
                                        .map(|(_, signature)| signature)
                                    });
                                if !signature.is_some_and(|signature| matches!(
                                &signature.termination_guarantee,
                                language_semantics::TerminationGuarantee::Terminates { premises }
                                    if premises.is_empty()
                            )) {
                                return Err("an abstract call has no unconditional public termination contract");
                            }
                                continue;
                            };
                            let callee_machine = &program.machines()[callee];
                            let Some(callee_entry) = program.machine_states(callee_machine).first()
                            else {
                                return Err("a call target has no checked entry binding");
                            };
                            if target_symbol != callee_machine.symbol
                                && target_symbol != callee_entry.symbol
                            {
                                return Err(
                                    "a subordinate-state call needs its own arrival evidence",
                                );
                            }
                            let Some(callee_position) =
                                component.iter().position(|index| *index == callee)
                            else {
                                continue;
                            };
                            admit_ranked_edge(
                                program,
                                &ranks,
                                &component_states,
                                index,
                                position,
                                machine,
                                state,
                                &mappings[state_position],
                                callee,
                                callee_position,
                                arguments,
                                entry_relative,
                                body_witnesses[position].map(|witness| witness.edges),
                                statement_ordinal,
                                transition.target,
                                &site_guards,
                                &mut observed,
                                &mut weak_edges,
                                &mut natural,
                            )?;
                        }
                        None => match target_node {
                            TransitionTargetNode::Value(value)
                                if expression_is_inert(program, machine, state, *value) => {}
                            TransitionTargetNode::Terminal => {}
                            _ => {
                                return Err(
                                    "a non-tail call or unknown effect prevents ranking admission",
                                );
                            }
                        },
                    }
                }
            }
            if adjacency[index]
                .iter()
                .filter(|target| component.contains(target))
                .any(|target| !observed.contains(target))
            {
                return Err("an internal call occurrence lacks a classified tail edge");
            }
        }
        if weak_edges_are_acyclic(&weak_edges) {
            Ok(natural)
        } else {
            Err("a preserving cycle has no strict measure DECREASE")
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn admit_ranked_edge(
        program: &dyn CheckingInput,
        ranks: &[projection::RankProjection],
        component_states: &[SymbolHandle],
        caller_machine_index: usize,
        caller_rank_index: usize,
        caller: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        caller_bindings: &RankStateBindings,
        callee: usize,
        callee_position: usize,
        arguments: &[ExpressionHandle],
        entry_relative: bool,
        witness_edges: Option<&[checked_trees::CheckedRankArrivalEdge]>,
        statement_ordinal: usize,
        transition_target: typed_trees::statement::TransitionTargetHandle,
        site_guards: &[(ExpressionHandle, bool)],
        observed: &mut Vec<usize>,
        weak_edges: &mut [Vec<usize>],
        natural: &mut Option<ValidatedRuntimeNaturalComponent>,
    ) -> Result<(), &'static str> {
        let callee_machine = &program.machines()[callee];
        let Some(callee_entry) = program.machine_states(callee_machine).first() else {
            return Err("a call target has no checked entry binding");
        };
        let parameters = program.state_parameters(callee_entry);
        if arguments.len() != parameters.len() {
            return Err("call arguments do not match the exact entry parameters");
        }
        if arguments
            .get(ranks[callee_position].argument_position)
            .is_none()
        {
            return Err("the ranked parameter has no corresponding actual argument");
        }
        observed.push(callee);
        let strict;
        if let (Some(source), Some(destination)) = (
            ranks[caller_rank_index].captured_components(program),
            ranks[callee_position].captured_components(program),
        ) && program
            .machine_states(caller)
            .first()
            .is_some_and(|entry| entry.symbol == state.symbol)
        {
            let progress = prove_captured_rank_call(
                program,
                caller,
                state,
                source,
                callee_machine,
                destination,
                statement_ordinal,
                transition_target,
                arguments,
            )?;
            for (index, application) in progress.source_applications.into_iter().enumerate() {
                let Some(application) = application else {
                    continue;
                };
                let Some(component) = natural.as_mut() else {
                    continue;
                };
                if let Some(row) = component
                    .states
                    .iter_mut()
                    .find(|row| row.state == state.symbol)
                {
                    let row = row
                        .values
                        .get_mut(index)
                        .ok_or("the captured rank application lost its ordered component")?;
                    if row.measure != application.owner()
                        || row
                            .application
                            .as_ref()
                            .is_some_and(|previous| previous != &application)
                    {
                        return Err(
                            "the captured rank formation does not match its exact state application",
                        );
                    }
                    row.application = Some(application);
                }
            }
            strict = match progress.progress {
                RankingRangeCallProgress::Strict => true,
                RankingRangeCallProgress::NonIncreasing => false,
            };
        } else if !entry_relative {
            let proved = callee == caller_machine_index
                && witness_edges.is_some_and(|edges| {
                    edges.iter().any(|edge| {
                        edge.source == state.symbol
                            && edge.destination == callee_entry.symbol
                            && edge.target == transition_target
                            && edge.statement_ordinal as usize == statement_ordinal
                            && edge.strictly_decreases
                    })
                });
            if !proved {
                return Err("a write, call, or alias invalidates the entry-relative ranking");
            }
            strict = true;
        } else if matches!(
            ranks[caller_rank_index].order,
            projection::RankOrder::Natural(_)
                | projection::RankOrder::IncreasingTo(_)
                | projection::RankOrder::BoundedDistance(_)
                | projection::RankOrder::SliceLength
                | projection::RankOrder::DeclaredIdentity { .. }
                | projection::RankOrder::DeclaredComputation { .. }
        ) {
            let caller_rank = &ranks[caller_rank_index];
            match prove_ranking_range_call(
                program,
                RankingRangeCallMember {
                    machine: caller,
                    subject: caller_rank.subject,
                    paired_subject: caller_rank.paired_subject,
                    range: caller_rank.range,
                },
                &RankingRangeCallSite {
                    state,
                    entry_parameters: caller_bindings,
                },
                RankingRangeCallMember {
                    machine: callee_machine,
                    subject: ranks[callee_position].subject,
                    paired_subject: ranks[callee_position].paired_subject,
                    range: ranks[callee_position].range,
                },
                site_guards,
                arguments,
            ) {
                Some(RankingRangeCallProgress::Strict) => strict = true,
                Some(RankingRangeCallProgress::NonIncreasing) => strict = false,
                None => {
                    return Err(
                        "rank range membership, selected view bounds, or nonincrease is unproven at a call site",
                    );
                }
            }
        } else {
            return Err("ranked invocation has no complete captured value comparison");
        };
        if !strict {
            let source = component_states
                .iter()
                .position(|symbol| *symbol == state.symbol)
                .ok_or("a call rank edge has no component source")?;
            let destination = component_states
                .iter()
                .position(|symbol| *symbol == callee_entry.symbol)
                .ok_or("a call rank edge has no component destination")?;
            weak_edges[source].push(destination);
        }
        if let Some(retained) = natural {
            retained.edges.push(ValidatedRuntimeNaturalEdge {
                source: state.symbol,
                target: callee_entry.symbol,
                statement_ordinal: u32::try_from(statement_ordinal)
                    .map_err(|_| "a ranking statement ordinal exceeds u32")?,
                strict,
            });
        }
        Ok(())
    }

    fn weak_edges_are_acyclic(adjacency: &[Vec<usize>]) -> bool {
        strongly_connected_components(adjacency)
            .iter()
            .all(|component| {
                component.len() == 1 && !adjacency[component[0]].contains(&component[0])
            })
    }

    pub(crate) fn ranged_member_site_invariant(
        program: &dyn CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &State,
        admitted: &checked_trees::CheckedRankArrivalPlan,
    ) -> Option<(RankingRangeMeasure, ExpressionHandle, RankStateBindings)> {
        let rank = RankProjection::resolve(program, machine)?;
        let measure = rank.measure?;
        if !rank.range.is_valid() || program.machine_states(machine).first()?.symbol == state.symbol
        {
            return None;
        }
        let body = admitted.for_body(machine.symbol, machine.states)?;
        let mut matches = admitted
            .states
            .span(body.states)?
            .iter()
            .filter(|row| row.state == state.symbol);
        let row = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        Some((
            measure,
            rank.range,
            RankStateBindings::from_checked(admitted, row)?,
        ))
    }

    pub(crate) struct InternalArrival<'program> {
        pub source: usize,
        pub destination: usize,
        pub statement_index: usize,
        pub target: typed_trees::statement::TransitionTargetHandle,
        pub arguments: &'program [ExpressionHandle],
        pub recurrent: bool,
    }

    pub(crate) fn internal_arrival_sites<'program>(
        program: &'program dyn CheckingInput,
        machine: &typed_trees::machine::Machine,
    ) -> Vec<InternalArrival<'program>> {
        let states = program.machine_states(machine);
        let mut arrivals = Vec::new();
        for (source, state) in states.iter().enumerate() {
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                let Some(transition) = statement.as_transition() else {
                    continue;
                };
                for target in transition.targets() {
                    if let Some((path, arguments)) =
                        program.statement_table.named_transition_target(target)
                        && let Some(destination) =
                            internal_state_index(program, machine, path.symbol)
                    {
                        arrivals.push(InternalArrival {
                            source,
                            destination,
                            statement_index,
                            target,
                            arguments: program.statement_table.expression_handles(arguments),
                            recurrent: false,
                        });
                    }
                }
            }
        }
        let mut adjacency = vec![Vec::new(); states.len()];
        for arrival in &arrivals {
            adjacency[arrival.source].push(arrival.destination);
        }
        let components = strongly_connected_components(&adjacency);
        for arrival in &mut arrivals {
            arrival.recurrent = components.iter().any(|component| {
                component.contains(&arrival.source)
                    && component.contains(&arrival.destination)
                    && (component.len() > 1 || arrival.source == arrival.destination)
            });
        }
        arrivals
    }

    pub fn internal_state_index(
        program: &dyn CheckingInput,
        machine: &typed_trees::machine::Machine,
        target_symbol: SymbolHandle,
    ) -> Option<usize> {
        typed_trees::state::state_position(program.machine_states(machine), target_symbol)
            .filter(|position| *position != 0)
    }

    pub fn activation_target_state_index(
        program: &dyn CheckingInput,
        machine: &typed_trees::machine::Machine,
        target_symbol: SymbolHandle,
    ) -> Option<usize> {
        if target_symbol == machine.symbol {
            return (!program.machine_states(machine).is_empty()).then_some(0);
        }
        typed_trees::state::state_position(program.machine_states(machine), target_symbol)
    }

    pub(crate) fn target_machine(
        program: &dyn CheckingInput,
        symbol: SymbolHandle,
    ) -> Option<usize> {
        if !symbol.is_valid() {
            return None;
        }
        program.machine_position_by_symbol(symbol).or_else(|| {
            let holder = program.machine_holding_state(symbol)?;
            program.machine_position_by_symbol(holder.symbol)
        })
    }

    fn expression_is_inert(
        program: &dyn CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        expression: ExpressionHandle,
    ) -> bool {
        expression_is_inert_or_calls(program, machine, state, expression, &|_| false)
    }

    fn expression_is_inert_or_calls(
        program: &dyn CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        expression: ExpressionHandle,
        admit_call: &dyn Fn(&typed_trees::expression::TableCallExpression) -> bool,
    ) -> bool {
        use typed_trees::SubtreeVisit;
        let edges = typed_trees::SubtreeEdges {
            pattern_dependencies: true,
            ..typed_trees::SubtreeEdges::VALUES
        };
        !program.subtree_any(expression, edges, |handle, node| match node {
            ExpressionNode::Atomic(atomic)
                if crate::value_custody::atomic_operations::atomic_once_operands(
                    program, atomic,
                )
                .is_some() =>
            {
                SubtreeVisit::Found
            }
            ExpressionNode::Binary(binary)
                if !meaning::binary_is_inert(program, machine, state, handle, binary) =>
            {
                SubtreeVisit::Found
            }
            ExpressionNode::Indexed(indexed)
                if crate::value_custody::places::has_builtin_subslice_meaning(
                    program,
                    machine,
                    Some(state),
                    handle,
                ) || crate::value_custody::places::place_has_builtin_coordinates(
                    program,
                    machine,
                    Some(state),
                    handle,
                ) =>
            {
                let mut operands = vec![indexed.collection];
                match expression_table(program).expression(indexed.index) {
                    ExpressionNode::Range(range) => operands.extend([range.start, range.end]),
                    _ => operands.push(indexed.index),
                }
                SubtreeVisit::InsteadEach(operands)
            }
            ExpressionNode::Indexed(_) | ExpressionNode::Range(_) => SubtreeVisit::Found,
            ExpressionNode::Call(call) if !admit_call(call) => SubtreeVisit::Found,
            _ => SubtreeVisit::Descend,
        })
    }
}
pub use rank_relevance::{RankRelevance, rank_relevance};
pub use runtime_ranking::{
    RuntimeRankApplicationBinding, RuntimeRankBodyWitness, RuntimeRankInput, RuntimeRankOperand,
    ValidatedRuntimeIntegerRank, ValidatedRuntimeNaturalComponent, ValidatedRuntimeNaturalEdge,
    ValidatedRuntimeNaturalState, activation_target_state_index, internal_state_index,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ValidatedProofRankingRelation {
    StructuralSubterm,
    StructuralSubtermCycles,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidatedProofRecursiveCallSite {
    Statement {
        state: SymbolHandle,
        statement_index: usize,
    },
    Expression {
        state: SymbolHandle,
        statement_index: usize,
        expression_ordinal: usize,
    },
    Transition {
        state: SymbolHandle,
        statement_index: usize,
        lane: ValidatedProofRecursiveTransitionLane,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ValidatedProofRecursiveTransitionLane {
    Target,
    Continuation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedProofRecursiveMember {
    pub machine: SymbolHandle,
    pub rank_parameter: SymbolHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedProofRecursiveEdge {
    pub caller: SymbolHandle,
    pub callee: SymbolHandle,
    pub site: ValidatedProofRecursiveCallSite,
    pub caller_rank_parameter: SymbolHandle,
    pub callee_rank_parameter: SymbolHandle,
    pub strict_member_path: Vec<SymbolHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedProofRecursiveComponent {
    pub members: Vec<ValidatedProofRecursiveMember>,
    pub ranking_relation: ValidatedProofRankingRelation,
    pub rank_type_identity: typed_trees::type_identity::TypeIdentityId,
    pub edges: Vec<ValidatedProofRecursiveEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedProofCallComponent {
    pub members: Vec<SymbolHandle>,
    pub structural: Option<ValidatedProofRecursiveComponent>,
    pub intervals: Vec<ValidatedProofRankInterval>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedProofRankInterval {
    pub origin: SymbolHandle,
    pub origin_rank: SymbolHandle,
    pub caller: SymbolHandle,
    pub callee: SymbolHandle,
    pub callee_rank: SymbolHandle,
    pub site: ValidatedProofRecursiveCallSite,
    pub argument: ExpressionHandle,
    pub relation: SymbolHandle,
}

#[derive(Debug, Clone)]
pub(crate) struct ExactProofCallEdge {
    caller: usize,
    callee: usize,
    site: ValidatedProofRecursiveCallSite,
    arguments: Vec<ExpressionHandle>,
}

pub fn machine_call_dependency_symbols(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
) -> Vec<SymbolHandle> {
    let mut symbols = Vec::new();
    collect_contract_dependency_symbols(program, program.machine_contracts(machine), &mut symbols);
    for state in program.machine_states(machine) {
        collect_contract_dependency_symbols(program, program.state_contracts(state), &mut symbols);
        for statement in program.statement_table.statements(state.statement_nodes) {
            collect_statement_dependency_symbols(program, statement, &mut symbols);
        }
    }
    symbols.sort_unstable_by_key(|symbol| symbol.arena_index());
    symbols.dedup();
    symbols
}

pub(crate) fn declaration_call_dependencies(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Option<(SymbolHandle, Vec<SymbolHandle>)> {
    if program
        .symbols
        .builtin_function_for_symbol(symbol)
        .is_some()
        || symbols::FixedCoreDeclaration::ALL
            .into_iter()
            .any(|declaration| {
                program.symbols.fixed_core_declaration_symbol(declaration) == Some(symbol)
            })
    {
        return Some((symbol, Vec::new()));
    }
    let machine = program.machine_by_symbol(symbol).or_else(|| {
        crate::machine_calls::calls::machine_state_by_symbol(program, symbol)
            .map(|(machine, _)| machine)
    });
    if let Some(machine) = machine {
        let mut dependencies = machine_call_dependency_symbols(program, machine);
        dependencies.extend(
            crate::machine_calls::machine_parameters::specialization_selection_dependency_symbols(
                program,
                machine.symbol,
            )?,
        );
        if machine.target_family_interface {
            dependencies.extend(
                crate::machine_calls::calls::target_family_members(program, machine)?
                    .iter()
                    .map(|member| member.machine),
            );
        }
        return Some((machine.symbol, dependencies));
    }
    let mut dependencies = Vec::new();
    if let Some(requirement) = program
        .dynamic_slot_by_symbol(symbol)
        .and_then(|slot| program.dynamic_slot_requirement(slot))
    {
        collect_contract_dependency_symbols(
            program,
            program.state_signature_contracts(requirement),
            &mut dependencies,
        );
    } else if let Some(definition) = program
        .mathematical_definitions()
        .iter()
        .find(|definition| definition.symbol == symbol)
    {
        for root in crate::mathematical_applications::definition_roots(program, definition) {
            collect_expression_dependency_symbols(program, root, &mut dependencies);
        }
    } else if let Some(operator) = typed_trees::operator::declaration_by_symbol(program, symbol) {
        collect_contract_dependency_symbols(
            program,
            program.operator_contracts(operator),
            &mut dependencies,
        );
    } else if let Some(signature) = crate::selected_requirement_signature(program, symbol)
        .map(|(_, signature)| signature)
        .or_else(|| {
            program
                .machine_parameter_signature(symbol)
                .map(|(_, signature)| signature)
        })
        .or_else(|| {
            program
                .retained_static_machine_contract(symbol)
                .map(|contract| contract.signature())
        })
    {
        collect_contract_dependency_symbols(
            program,
            program.state_signature_contracts(signature),
            &mut dependencies,
        );
        dependencies.extend(requirement_backing_machines(program, symbol));
    } else {
        let proposition = program
            .propositions()
            .iter()
            .find(|proposition| proposition.symbol == symbol)?;
        if let typed_trees::proposition::PropositionBody::Transparent { proposition } =
            &proposition.body
        {
            match proposition {
                typed_trees::proposition::PropositionFormula::BooleanExpression(expression) => {
                    collect_expression_dependency_symbols(program, *expression, &mut dependencies);
                }
                typed_trees::proposition::PropositionFormula::Application(application) => {
                    collect_proposition_dependency_symbols(program, application, &mut dependencies);
                }
            }
        }
    }
    dependencies
        .sort_unstable_by_key(|dependency| (dependency.arena_index(), dependency.generation()));
    dependencies.dedup();
    Some((symbol, dependencies))
}

pub fn requirement_backing_machines(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Vec<SymbolHandle> {
    let declared = program
        .closed_requirement_application(symbol)
        .map_or(symbol, |application| application.template);
    if program.trait_machine_signature(declared).is_none() {
        return Vec::new();
    }
    let mut backing =
        crate::machine_calls::denotational_calls::requirement_satisfiers(program, declared);
    backing.extend(
        program
            .conformances()
            .iter()
            .flat_map(|conformance| conformance.rows.iter())
            .filter(|row| row.requirement == declared && row.realization_machine.is_valid())
            .map(|row| row.realization_machine),
    );
    backing
}

fn collect_contract_dependency_symbols(
    program: &dyn crate::CheckingInput,
    contracts: &[typed_trees::signature::SignatureContract],
    symbols: &mut Vec<SymbolHandle>,
) {
    for fact in contracts
        .iter()
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
    {
        match fact {
            typed_trees::domain::ProofFact::Expression(expression) => {
                collect_expression_dependency_symbols(program, *expression, symbols);
            }
            typed_trees::domain::ProofFact::Membership(membership) => {
                collect_expression_dependency_symbols(program, membership.value, symbols);
            }
            typed_trees::domain::ProofFact::Proposition(application) => {
                collect_proposition_dependency_symbols(program, application, symbols);
            }
        }
    }
}

fn collect_proposition_dependency_symbols(
    program: &dyn crate::CheckingInput,
    application: &typed_trees::proposition::PropositionApplication,
    symbols: &mut Vec<SymbolHandle>,
) {
    push_dependency_symbol(symbols, application.proposition);
    for argument in expression_table(program).expression_handles(application.arguments) {
        collect_expression_dependency_symbols(program, *argument, symbols);
    }
    for argument in &application.binder_arguments {
        if argument.kind == typed_trees::proposition::PropositionBinderArgumentKind::Machine {
            push_dependency_symbol(symbols, argument.symbol);
        }
    }
}

fn push_dependency_symbol(symbols: &mut Vec<SymbolHandle>, symbol: SymbolHandle) {
    if symbol.is_valid() {
        symbols.push(symbol);
    }
}

fn collect_statement_dependency_symbols(
    program: &dyn crate::CheckingInput,
    statement: &StatementNode,
    symbols: &mut Vec<SymbolHandle>,
) {
    match statement {
        StatementNode::Assembly(_) => {}
        StatementNode::AssemblyFact(fact) => {
            for expression in program.tables.proof_fact_expression_roots(fact.fact) {
                collect_expression_dependency_symbols(program, expression, symbols);
            }
        }
        StatementNode::Assignment(assignment) => {
            collect_expression_dependency_symbols(program, assignment.target, symbols);
            collect_expression_dependency_symbols(program, assignment.value, symbols);
        }
        StatementNode::Call(call) => {
            push_dependency_symbol(symbols, call.target_symbol);
            symbols.extend(
                crate::machine_calls::machine_parameters::call_selection_dependency_symbols(
                    program,
                    typed_trees::references::StaticArgumentCall::Statement(call),
                ),
            );
            for argument in program.statement_table.expression_handles(call.arguments) {
                collect_expression_dependency_symbols(program, *argument, symbols);
            }
        }
        StatementNode::Expression(handle) => {
            collect_expression_dependency_symbols(program, *handle, symbols)
        }
        StatementNode::LocalData(local) => {
            collect_expression_dependency_symbols(program, local.initial_value, symbols)
        }
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard {
                collect_expression_dependency_symbols(program, guard, symbols);
            }
            for target_handle in transition.targets() {
                match program.statement_table.transition_target(target_handle) {
                    TransitionTargetNode::Named {
                        path, arguments, ..
                    } => {
                        push_dependency_symbol(symbols, path.symbol);
                        for argument in program.statement_table.expression_handles(*arguments) {
                            collect_expression_dependency_symbols(program, *argument, symbols);
                        }
                    }
                    TransitionTargetNode::Value(handle) => {
                        collect_expression_dependency_symbols(program, *handle, symbols)
                    }
                    TransitionTargetNode::Terminal => {}
                }
            }
        }
    }
}

fn collect_expression_dependency_symbols(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    symbols: &mut Vec<SymbolHandle>,
) {
    if !expression.is_valid() {
        return;
    }
    let node = expression_table(program).expression(expression);
    if let ExpressionNode::Call(call) = node {
        push_dependency_symbol(symbols, call.target_symbol);
        symbols.extend(
            crate::machine_calls::machine_parameters::call_selection_dependency_symbols(
                program,
                typed_trees::references::StaticArgumentCall::Expression(expression, call),
            ),
        );
    }
    crate::queries::expression_traversal::executable_children(program, node, |child| {
        collect_expression_dependency_symbols(program, child, symbols);
    });
}

pub(crate) struct MachineCallGraph {
    edges: Vec<Vec<usize>>,
    proof_dependencies: Vec<Vec<usize>>,
    selector_self_recursion: Vec<usize>,
}

fn build_machine_call_graph(
    program: &dyn crate::CheckingInput,
    proof_only: &typed_trees::proof_only::ProofOnlyClassification,
) -> MachineCallGraph {
    let proof_dependencies = collect_proof_call_dependencies(program, proof_only);
    let mut edges = proof_dependencies.clone();
    for edge in collect_exact_proof_call_edges(program, proof_only) {
        edges[edge.caller].push(edge.callee);
    }
    for targets in &mut edges {
        targets.sort_unstable();
        targets.dedup();
    }
    let selector_self_recursion =
        runtime_ranking::extend_runtime_adjacency(program, proof_only, &mut edges);
    MachineCallGraph {
        edges,
        proof_dependencies,
        selector_self_recursion,
    }
}

/// Contracts may not form a cycle (`wiki/spec/language/state_contracts.md`):
/// a `requires` or `ensures` that calls a machine whose own contract, directly
/// or through other contracts, calls back to the first is rejected even when
/// every body is non-recursive. Each edge runs from a machine to a machine its
/// contracts (machine and state contracts, including calls reached through the
/// propositions, definitions and requirements a contract names) call; bodies
/// add no edges, so only contract-to-contract cycles are reported.
fn validate_contract_call_cycles(
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let machines = program.machines();
    let mut edges = vec![Vec::new(); machines.len()];
    for (caller, machine) in machines.iter().enumerate() {
        let mut pending = Vec::new();
        collect_contract_dependency_symbols(
            program,
            program.machine_contracts(machine),
            &mut pending,
        );
        for state in program.machine_states(machine) {
            collect_contract_dependency_symbols(
                program,
                program.state_contracts(state),
                &mut pending,
            );
        }
        let mut expanded = Vec::new();
        while let Some(symbol) = pending.pop() {
            if expanded.contains(&symbol) {
                continue;
            }
            expanded.push(symbol);
            if let Some(callee) = runtime_ranking::target_machine(program, symbol) {
                if callee != caller {
                    edges[caller].push(callee);
                }
            } else if let Some((_, dependencies)) = declaration_call_dependencies(program, symbol) {
                pending.extend(dependencies);
            }
        }
        edges[caller].sort_unstable();
        edges[caller].dedup();
    }
    for mut members in strongly_connected_components(&edges) {
        if members.len() < 2 {
            continue;
        }
        members.sort_unstable();
        let names = members
            .iter()
            .map(|member| machines[*member].name.as_str())
            .collect::<Vec<_>>()
            .join("`, `");
        diagnostics.push(Diagnostic::error(format!(
            "contract call cycle: the `requires`/`ensures` contracts of `{names}` call each \
             other; contracts may not form a cycle even when the bodies are not recursive; \
             state the fact directly instead"
        )));
    }
}

pub(crate) fn validate_machine_call_cycles(
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<ValidatedProofCallComponent> {
    validate_contract_call_cycles(program, diagnostics);
    let proof_only = crate::proof_only_classification(program);
    let machines = program.machines();
    let MachineCallGraph {
        edges,
        proof_dependencies,
        selector_self_recursion,
    } = build_machine_call_graph(program, &proof_only);
    let mut ranking_failures = Vec::new();
    let proof_components = build_validated_proof_recursive_components(
        program,
        &proof_only,
        &edges,
        &proof_dependencies,
        &mut ranking_failures,
        None,
    )
    .into_iter()
    .map(|(_, component)| component)
    .collect::<Vec<_>>();

    for members in strongly_connected_components(&edges) {
        if members.len() == 1 {
            let member = members[0];
            if selector_self_recursion.contains(&member) {
                let machine = &machines[member];
                let witnessed = machine.termination_plan.implementation_witness.is_some()
                    || machine.termination_plan.interface.published().is_some_and(
                        language_semantics::TerminationGuarantee::promises_termination,
                    );
                if !witnessed {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine call cycle: `{}` can re-enter itself through a selector call or \
                         boundary requirement call; termination across implementation selection \
                         is not yet ranked",
                        machine.name
                    )));
                }
            }
            continue;
        }
        let names = members
            .iter()
            .map(|member| machines[*member].name.as_str())
            .collect::<Vec<_>>()
            .join("`, `");
        if members
            .iter()
            .all(|member| proof_only.is_proof_machine(program, &machines[*member]))
            && !proof_components.iter().any(|component| {
                component.members.len() == members.len()
                    && members.iter().all(|member| {
                        component
                            .members
                            .iter()
                            .any(|validated| *validated == machines[*member].symbol)
                    })
            })
        {
            let reason = ranking_failures
                .iter()
                .find(|(failed, _)| failed == &members)
                .map(|(_, reason)| *reason)
                .unwrap_or("the exact call inventory is incomplete");
            diagnostics.push(Diagnostic::error(format!(
                "proof-only machine call cycle: `{names}` -- no complete structural ranking \
                     establishes strict descent on every cycle; exact arguments must preserve \
                     or descend from the ranking subject, and preserving edges must be acyclic; \
                     computed ranking: {reason}"
            )));
        }
    }
    proof_components
}

pub fn infer_checked_proof_recursive_machines(
    program: &dyn crate::CheckingInput,
    machine: SymbolHandle,
    body: arena::HandleSpan<typed_trees::state::State>,
) -> Vec<&typed_trees::machine::Machine> {
    let mut matching = program
        .machines()
        .iter()
        .enumerate()
        .filter(|(_, candidate)| candidate.symbol == machine && candidate.states == body);
    let Some((position, machine)) = matching.next() else {
        return Vec::new();
    };
    if matching.next().is_some() {
        return Vec::new();
    }
    let (proof_only, graph) = {
        let proof_only = crate::proof_only_classification(program);
        let graph = build_machine_call_graph(program, &proof_only);
        (proof_only, graph)
    };
    if !proof_only.is_proof_machine(program, machine) {
        return Vec::new();
    }
    build_validated_proof_recursive_components(
        program,
        &proof_only,
        &graph.edges,
        &graph.proof_dependencies,
        &mut Vec::new(),
        Some(position),
    )
    .into_iter()
    .flat_map(|(positions, _)| positions)
    .map(|position| &program.machines()[position])
    .collect()
}

fn build_validated_proof_recursive_components(
    program: &dyn crate::CheckingInput,
    proof_only: &typed_trees::proof_only::ProofOnlyClassification,
    adjacency: &[Vec<usize>],
    proof_dependencies: &[Vec<usize>],
    failures: &mut Vec<(Vec<usize>, &'static str)>,
    demanded: Option<usize>,
) -> Vec<(Vec<usize>, ValidatedProofCallComponent)> {
    let machines = program.machines();
    let components = match demanded {
        Some(position) => proof_components::complete_for_machine(
            program,
            proof_only,
            adjacency,
            proof_dependencies,
            position,
        ),
        None => proof_components::complete(program, proof_only, adjacency, proof_dependencies),
    };
    components
        .into_iter()
        .filter_map(|component| {
            let structural =
                proof_ranking::validate_component(program, &component.members, &component.edges);
            let intervals = if structural.is_some() {
                Vec::new()
            } else {
                match proof_intervals::validate_component(
                    program,
                    &component.members,
                    &component.edges,
                ) {
                    Ok(intervals) => intervals,
                    Err(reason) => {
                        failures.push((component.members, reason));
                        return None;
                    }
                }
            };
            let validated = ValidatedProofCallComponent {
                members: component
                    .members
                    .iter()
                    .map(|member| machines[*member].symbol)
                    .collect(),
                structural,
                intervals,
            };
            Some((component.members, validated))
        })
        .collect()
}

fn proof_call_site_sort_key(
    site: ValidatedProofRecursiveCallSite,
) -> (u8, u32, u32, usize, u32, u32) {
    match site {
        ValidatedProofRecursiveCallSite::Statement {
            state,
            statement_index,
        } => (
            1,
            state.arena_index(),
            state.generation(),
            statement_index,
            0,
            0,
        ),
        ValidatedProofRecursiveCallSite::Expression {
            state,
            statement_index,
            expression_ordinal,
        } => (
            2,
            state.arena_index(),
            state.generation(),
            statement_index,
            u32::try_from(expression_ordinal).unwrap_or(u32::MAX),
            0,
        ),
        ValidatedProofRecursiveCallSite::Transition {
            state,
            statement_index,
            lane,
        } => (
            3,
            state.arena_index(),
            state.generation(),
            statement_index,
            match lane {
                ValidatedProofRecursiveTransitionLane::Target => 1,
                ValidatedProofRecursiveTransitionLane::Continuation => 2,
            },
            0,
        ),
    }
}

fn collect_proof_call_dependencies(
    program: &dyn crate::CheckingInput,
    proof_only: &typed_trees::proof_only::ProofOnlyClassification,
) -> Vec<Vec<usize>> {
    let machines = program.machines();
    let mut dependencies = vec![Vec::new(); machines.len()];
    for (caller, machine) in machines.iter().enumerate() {
        if !proof_only.is_proof_machine(program, machine) {
            continue;
        }
        let mut targets = Vec::new();
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                if !matches!(statement, StatementNode::AssemblyFact(_)) {
                    collect_statement_dependency_symbols(program, statement, &mut targets);
                }
            }
        }
        for target in targets {
            if let Some(callee) = runtime_ranking::target_machine(program, target)
                && callee != caller
            {
                dependencies[caller].push(callee);
            }
        }
    }
    dependencies
}

fn collect_exact_proof_call_edges(
    program: &dyn crate::CheckingInput,
    proof_only: &typed_trees::proof_only::ProofOnlyClassification,
) -> Vec<ExactProofCallEdge> {
    collect_exact_proof_call_edges_in_machines(program, proof_only, None)
}

fn collect_exact_proof_call_edges_in_machines(
    program: &dyn crate::CheckingInput,
    proof_only: &typed_trees::proof_only::ProofOnlyClassification,
    demanded: Option<&[usize]>,
) -> Vec<ExactProofCallEdge> {
    let mut edges = Vec::new();
    for (caller, machine) in program.machines().iter().enumerate() {
        if demanded.is_some_and(|demanded| !demanded.contains(&caller))
            || !proof_only.is_proof_machine(program, machine)
        {
            continue;
        }
        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .indexed_statements(state.statement_nodes)
            {
                collect_exact_proof_statement_edges(
                    program,
                    state.symbol,
                    statement_index,
                    statement,
                    caller,
                    &mut edges,
                );
            }
        }
    }
    edges
}

#[allow(clippy::too_many_arguments)]
fn collect_exact_proof_statement_edges(
    program: &dyn crate::CheckingInput,
    state: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
    caller: usize,
    edges: &mut Vec<ExactProofCallEdge>,
) {
    let mut expression_ordinal = 0usize;
    macro_rules! collect_expression {
        ($expression:expr) => {
            collect_exact_proof_expression_edges(
                program,
                state,
                statement_index,
                $expression,
                caller,
                &mut expression_ordinal,
                edges,
            )
        };
    }
    match statement {
        StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => {}
        StatementNode::Call(call) => {
            if let Some(callee) = runtime_ranking::target_machine(program, call.target_symbol) {
                edges.push(ExactProofCallEdge {
                    caller,
                    callee,
                    site: ValidatedProofRecursiveCallSite::Statement {
                        state,
                        statement_index,
                    },
                    arguments: program
                        .statement_table
                        .expression_handles(call.arguments)
                        .to_vec(),
                });
            }
            for argument in program.statement_table.expression_handles(call.arguments) {
                collect_expression!(*argument);
            }
        }
        StatementNode::Assignment(assignment) => collect_expression!(assignment.value),
        StatementNode::Expression(expression) => collect_expression!(*expression),
        StatementNode::LocalData(local) => collect_expression!(local.initial_value),
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard {
                collect_expression!(guard);
            }
            for (target_handle, lane) in [
                (
                    transition.target,
                    ValidatedProofRecursiveTransitionLane::Target,
                ),
                (
                    transition.continuation,
                    ValidatedProofRecursiveTransitionLane::Continuation,
                ),
            ] {
                if !target_handle.is_valid() {
                    continue;
                }
                match program.statement_table.transition_target(target_handle) {
                    TransitionTargetNode::Named {
                        path, arguments, ..
                    } => {
                        if let Some(callee) = runtime_ranking::target_machine(program, path.symbol)
                        {
                            edges.push(ExactProofCallEdge {
                                caller,
                                callee,
                                site: ValidatedProofRecursiveCallSite::Transition {
                                    state,
                                    statement_index,
                                    lane,
                                },
                                arguments: program
                                    .statement_table
                                    .expression_handles(*arguments)
                                    .to_vec(),
                            });
                        }
                        for argument in program.statement_table.expression_handles(*arguments) {
                            collect_expression!(*argument);
                        }
                    }
                    TransitionTargetNode::Value(expression) => collect_expression!(*expression),
                    TransitionTargetNode::Terminal => {}
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_exact_proof_expression_edges(
    program: &dyn crate::CheckingInput,
    state: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    caller: usize,
    next_expression_ordinal: &mut usize,
    edges: &mut Vec<ExactProofCallEdge>,
) {
    if !expression.is_valid() {
        return;
    }
    let expression_ordinal = *next_expression_ordinal;
    *next_expression_ordinal = expression_ordinal.saturating_add(1);
    macro_rules! recurse {
        ($child:expr) => {
            collect_exact_proof_expression_edges(
                program,
                state,
                statement_index,
                $child,
                caller,
                next_expression_ordinal,
                edges,
            )
        };
    }
    match expression_table(program).expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            for child in
                crate::value_custody::atomic_operations::atomic_execution_children(program, atomic)
            {
                recurse!(child);
            }
        }
        ExpressionNode::Call(call) => {
            if let Some(callee) = runtime_ranking::target_machine(program, call.target_symbol) {
                edges.push(ExactProofCallEdge {
                    caller,
                    callee,
                    site: ValidatedProofRecursiveCallSite::Expression {
                        state,
                        statement_index,
                        expression_ordinal,
                    },
                    arguments: expression_table(program)
                        .expression_handles(call.arguments)
                        .to_vec(),
                });
            }
            recurse!(call.receiver);
            for argument in expression_table(program).expression_handles(call.arguments) {
                recurse!(*argument);
            }
        }
        other => crate::queries::expression_traversal::children(program, other, |child| {
            recurse!(child);
        }),
    }
}

fn proof_rank_parameter(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
) -> Option<(
    SymbolHandle,
    TypeReferenceHandle,
    typed_trees::type_identity::TypeIdentityId,
)> {
    let caller_witness = machine.termination_plan.implementation_witness.as_ref()?;
    let [caller_subject] = caller_witness.subjects.as_slice() else {
        return None;
    };
    let parameter = program.machine_states(machine).first().and_then(|entry| {
        program
            .state_parameters(entry)
            .iter()
            .find(|parameter| parameter.name.as_str() == caller_subject.as_str())
    })?;
    Some((
        parameter.symbol,
        parameter.type_reference,
        program.package_qualified_type_identity(parameter.type_reference),
    ))
}

fn expression_member_path(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    root_symbol: SymbolHandle,
    root_type: TypeReferenceHandle,
) -> Option<(Vec<SymbolHandle>, TypeReferenceHandle)> {
    match expression_table(program).expression(expression) {
        ExpressionNode::Member(member) => {
            let (mut path, receiver_type) =
                expression_member_path(program, member.receiver, root_symbol, root_type)?;
            let field = exact_member_field(
                program,
                receiver_type,
                member.member_symbol,
                member.member.as_str(),
                member.case_variant.as_ref().map(|variant| variant.as_str()),
            )?;
            path.push(field.symbol);
            Some((path, field.type_reference))
        }
        ExpressionNode::Name(path) => {
            (root_symbol.is_valid() && path.symbol == root_symbol).then(|| (Vec::new(), root_type))
        }
        _ => None,
    }
}

fn exact_member_field<'program>(
    program: &'program dyn crate::CheckingInput,
    receiver_type: TypeReferenceHandle,
    member_symbol: SymbolHandle,
    member_name: &str,
    case_variant: Option<&str>,
) -> Option<&'program typed_trees::data::DataField> {
    let data = crate::value_custody::places::data_definition_for_type(program, receiver_type)?;
    exact_data_member_field(program, data, member_symbol, member_name, case_variant)
}
