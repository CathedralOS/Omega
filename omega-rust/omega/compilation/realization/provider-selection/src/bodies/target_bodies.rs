//! Target-body availability and reached direct calls over prepared declarations.
//!
//! Absence is negative evidence, not a realizability certificate. Entry reach
//! additionally follows selected authored providers. Neither operation walks
//! source or infers implicit operator, cleanup or dynamic edges.

use diagnostics::Diagnostic;
use language_semantics::direct_calls::{DependencyScope, DirectCallDeclarations};

pub struct DirectTargetBodyAbsences<'graph, Declaration> {
    graph: &'graph DirectCallDeclarations<Declaration>,
    absent: Vec<bool>,
}

impl<'graph, Declaration: Eq + std::hash::Hash> DirectTargetBodyAbsences<'graph, Declaration> {
    pub fn derive(graph: &'graph DirectCallDeclarations<Declaration>, target: &str) -> Self {
        let mut absent = vec![false; graph.machines.len()];
        let mut callers = vec![Vec::new(); graph.machines.len()];
        for (position, machine) in graph.machines.iter().enumerate() {
            if !realizes_provider_body(graph, position, target) {
                absent[position] = true;
                continue;
            }
            for state in &machine.states {
                for call in &state.calls {
                    let Some(declaration) = call.machine else {
                        continue;
                    };
                    let Some(realization) = provider_body(graph, declaration, target) else {
                        absent[position] = true;
                        continue;
                    };
                    callers[realization].push(position);
                }
            }
        }
        let mut frontier: Vec<_> = absent
            .iter()
            .enumerate()
            .filter_map(|(position, absent)| absent.then_some(position))
            .collect();
        while let Some(callee) = frontier.pop() {
            for &caller in &callers[callee] {
                if !std::mem::replace(&mut absent[caller], true) {
                    frontier.push(caller);
                }
            }
        }
        Self { graph, absent }
    }

    pub fn contains(&self, declaration: &Declaration) -> bool {
        self.graph
            .called_machine_position(declaration)
            .is_some_and(|position| {
                self.graph.machines[position].declaration == *declaration && self.absent[position]
            })
    }
}

/// Visit direct callees before selected authored realizations; preserve state
/// and edge order, duplicates, and stable caller-position diagnostic ordering.
pub fn reject_reached_inert_calls<Declaration: Eq + std::hash::Hash>(
    graph: &DirectCallDeclarations<Declaration>,
    target: &str,
    entry: Option<&Declaration>,
    provider_rows: &[(Declaration, Declaration)],
) -> Result<(), Vec<Diagnostic>> {
    let provider_rows = provider_rows
        .iter()
        .map(|(required, realization)| (required, graph.called_machine_position(realization)))
        .collect::<Vec<_>>();
    let mut reached = vec![false; graph.machines.len()];
    let mut frontier: Vec<_> = entry
        .and_then(|entry| graph.called_machine_position(entry))
        .and_then(|entry| explicit_body(graph, entry, target))
        .into_iter()
        .collect();
    for position in &frontier {
        reached[*position] = true;
    }
    let mut refusals = Vec::new();
    while let Some(position) = frontier.pop() {
        let caller = &graph.machines[position];
        for state in &caller.states {
            for call in &state.calls {
                let requirement_machine = call
                    .machine
                    .map(|callee| &graph.machines[callee].declaration);
                let realizations = provider_rows
                    .iter()
                    .filter(|(required, _)| {
                        *required == &call.declaration || Some(*required) == requirement_machine
                    })
                    .filter_map(|(_, realization)| *realization);
                for callee in call.machine.into_iter().chain(realizations) {
                    let Some(realization) = explicit_body(graph, callee, target) else {
                        refusals.push((
                            position,
                            Diagnostic::error(unrealized_call_message(
                                graph,
                                &caller.name,
                                &state.name,
                                callee,
                                target,
                            )),
                        ));
                        continue;
                    };
                    if !std::mem::replace(&mut reached[realization], true) {
                        frontier.push(realization);
                    }
                }
            }
        }
    }
    if refusals.is_empty() {
        return Ok(());
    }
    refusals.sort_by_key(|(position, _)| *position);
    Err(refusals
        .into_iter()
        .map(|(_, diagnostic)| diagnostic)
        .collect())
}

fn family_body<Declaration>(
    graph: &DirectCallDeclarations<Declaration>,
    position: usize,
    target: &str,
) -> Option<usize> {
    graph
        .families
        .iter()
        .find(|family| family.canonical == Some(position))?
        .members
        .iter()
        .find(|member| member.target == target)?
        .machine
}

/// Explicit member references stay exact. This is deliberately not the
/// Product/Build provider-eligibility rule below.
fn explicit_body<Declaration>(
    graph: &DirectCallDeclarations<Declaration>,
    position: usize,
    target: &str,
) -> Option<usize> {
    let machine = &graph.machines[position];
    if machine.family_interface {
        return family_body(graph, position, target);
    }
    machine
        .authored_target
        .as_deref()
        .is_none_or(|declared| declared == target)
        .then_some(position)
}

fn realizes_provider_body<Declaration>(
    graph: &DirectCallDeclarations<Declaration>,
    position: usize,
    target: &str,
) -> bool {
    let machine = &graph.machines[position];
    if machine.family_interface || !machine.in_scope {
        return false;
    }
    if graph.scope == DependencyScope::Build {
        return machine.target.is_none();
    }
    machine
        .source_target_context
        .as_deref()
        .is_none_or(|authored| authored == target)
}

fn provider_body<Declaration>(
    graph: &DirectCallDeclarations<Declaration>,
    position: usize,
    target: &str,
) -> Option<usize> {
    let position = if graph.machines[position].family_interface {
        family_body(graph, position, target)?
    } else {
        position
    };
    realizes_provider_body(graph, position, target).then_some(position)
}

fn unrealized_call_message<Declaration>(
    graph: &DirectCallDeclarations<Declaration>,
    caller: &str,
    state: &str,
    callee: usize,
    target: &str,
) -> String {
    let machine = &graph.machines[callee];
    let declared: Vec<_> = graph
        .families
        .iter()
        .find(|family| {
            family.canonical == Some(callee)
                || family
                    .members
                    .iter()
                    .any(|member| member.machine == Some(callee))
        })
        .map(|family| {
            family
                .members
                .iter()
                .map(|member| member.target.as_str())
                .collect()
        })
        .unwrap_or_else(|| machine.target.iter().map(String::as_str).collect());
    let callee = &machine.name;
    match declared.as_slice() {
        [only] => format!(
            "machine `{caller}` state `{state}` calls `{callee}`, which is \
             declared only for target `{only}`. This build realizes `{target}`, so that body \
             is an inert sibling and the call has no implementation; scope the caller to \
             `{only}` as well, or declare `{callee}` for the target being built."
        ),
        several => format!(
            "machine `{caller}` state `{state}` calls `{callee}`, which is \
             declared only for targets {}. This build realizes `{target}`, so those bodies \
             are inert siblings and the call has no implementation; scope the caller to one \
             of those targets, or declare `{callee}` for the target being built.",
            several
                .iter()
                .map(|target| format!("`{target}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}
