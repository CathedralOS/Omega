//! Validation of selected synchronous invocation cycles.

use crate::provider_planning::provenance_replay::{
    exact_authored_invocations, exact_checked_adapter_invocations, exact_row_for_schema_method,
};
use effects::provider_plan::{ProviderBinding, ProviderPlan};
use typed_trees::TypedTrees;

/// Reject a cycle in the direct synchronous graph realized by the concrete
/// provider selection. Reach closure is intentionally irrelevant: only a
/// selected method's authored `invokes` edges participate, and a missing
/// selected target cannot manufacture an edge.
pub fn validate_selected_synchronous_invocation_cycles(
    typed: &TypedTrees,
    selected_plans: &[effects::provider_plan::ProviderPlan],
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let selected = exact_selected_synchronous_plans(selected_plans)?;
    let inferred = validation::infer_synchronous_invocations(typed);
    let mut edges = vec![Vec::<usize>::new(); selected.len()];
    let mut diagnostics = Vec::new();
    for (source_index, source) in selected.iter().enumerate() {
        for method in &source.schema.methods {
            let row = match exact_row_for_schema_method(source, method) {
                Ok(row) => row,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
            let target_names = match &row.binding {
                ProviderBinding::CheckedAdapter { .. } => {
                    exact_checked_adapter_invocations(typed, &inferred, source, method, row)
                }
                _ => exact_authored_invocations(source, method),
            };
            let target_names = match target_names {
                Ok(target_names) => target_names,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
            for target_name in target_names {
                let matching_targets = selected
                    .iter()
                    .enumerate()
                    .filter(|(_, target)| target.schema.trait_name == target_name)
                    .collect::<Vec<_>>();
                match matching_targets.as_slice() {
                    [] => {}
                    [(target_index, _)] if !edges[source_index].contains(target_index) => {
                        edges[source_index].push(*target_index);
                    }
                    [_] => {}
                    _ => diagnostics.push(diagnostics::Diagnostic::error(format!(
                        "selected synchronous invocation `{target_name}` from `{}::{}` is ambiguous across {} package-qualified boundary slots",
                        source.schema.trait_name,
                        method.name,
                        matching_targets.len(),
                    ))),
                }
            }
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let mut color = vec![0u8; selected.len()];
    let mut path = Vec::new();
    for start in 0..selected.len() {
        if color[start] == 0
            && let Some(cycle) = synchronous_cycle_from(start, &edges, &mut color, &mut path)
        {
            let names = cycle
                .iter()
                .map(|index| selected[*index].schema.trait_name.as_str())
                .chain(std::iter::once(
                    selected[cycle[0]].schema.trait_name.as_str(),
                ))
                .collect::<Vec<_>>()
                .join(" -> ");
            return Err(vec![diagnostics::Diagnostic::error(format!(
                "selected providers realize a cyclic synchronous `invokes` graph: {names}; break one edge with a mailbox, queue, scheduler handoff, or other new activation",
            ))]);
        }
    }
    Ok(())
}

fn exact_selected_synchronous_plans(
    selected_plans: &[ProviderPlan],
) -> Result<Vec<&ProviderPlan>, Vec<diagnostics::Diagnostic>> {
    let mut selected = Vec::new();
    let mut diagnostics = Vec::new();
    let mut seen_plans = Vec::new();
    for plan in selected_plans {
        if plan.name.is_empty() {
            diagnostics.push(diagnostics::Diagnostic::error(
                "selected synchronous-invocation ProviderPlan name is empty",
            ));
            continue;
        }
        if seen_plans.contains(&plan) {
            diagnostics.push(diagnostics::Diagnostic::error(format!(
                "selected synchronous-invocation ProviderPlan `{}` is listed more than once",
                plan.name,
            )));
            continue;
        }
        seen_plans.push(plan);
        selected.push(plan);
    }

    let mut seen_schemas = Vec::new();
    for plan in &selected {
        if plan.schema.trait_name.is_empty() {
            diagnostics.push(diagnostics::Diagnostic::error(format!(
                "selected synchronous-invocation ProviderPlan `{}` has an empty exact schema identity",
                plan.name,
            )));
            continue;
        }
        let schema_identity = (
            plan.schema.trait_package_identity,
            plan.schema.trait_name.as_str(),
        );
        if seen_schemas.contains(&schema_identity) {
            diagnostics.push(diagnostics::Diagnostic::error(format!(
                "selected synchronous-invocation schema `{}` is realized by more than one selected ProviderPlan",
                plan.schema.trait_name,
            )));
            continue;
        }
        seen_schemas.push(schema_identity);
    }

    if diagnostics.is_empty() {
        Ok(selected)
    } else {
        Err(diagnostics)
    }
}

fn synchronous_cycle_from(
    node: usize,
    edges: &[Vec<usize>],
    color: &mut [u8],
    path: &mut Vec<usize>,
) -> Option<Vec<usize>> {
    color[node] = 1;
    path.push(node);
    for target in &edges[node] {
        if color[*target] == 0 {
            if let Some(cycle) = synchronous_cycle_from(*target, edges, color, path) {
                return Some(cycle);
            }
        } else if color[*target] == 1 {
            let start = path.iter().position(|member| member == target)?;
            return Some(path[start..].to_vec());
        }
    }
    path.pop();
    color[node] = 2;
    None
}
