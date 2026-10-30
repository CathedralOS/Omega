//! Whole-plan synchronous invocation graph, independent of source or execution storage.
//!
//! Source checking supplies checked call facts. Receiving admission supplies
//! retained executable edges for every installed candidate. Both use this same
//! exact-slot join and cycle rule; opaque rows retain their declared ceilings.

use effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow};

/// A complete checked application, or an explicitly abstract external ceiling.
/// Declaration names are canonical package-qualified export coordinates here,
/// not source spelling used to rediscover a call.
pub enum SynchronousBoundaryTarget {
    Application {
        declaration: String,
        package: Option<semantic_vocabulary::PackageKeyIdentity>,
        application: Option<String>,
    },
    /// Canonical installation already joined this exact whole plan. Do not
    /// erase that association and rediscover it from an optional package name.
    Selected {
        plan: effects::provider_plan::ProviderPlanDigest,
        label: String,
    },
}

impl SynchronousBoundaryTarget {
    fn matches(&self, target: &ProviderPlan) -> bool {
        let schema = &target.schema;
        match self {
            Self::Selected { plan, .. } => target.identity_digest() == *plan,
            Self::Application {
                package,
                application: Some(application),
                ..
            } => schema
                .declaration
                .receiver_applications
                .iter()
                .any(|receiver| {
                    schema.identity_text(receiver.application) == application
                        && receiver.trait_package_identity == *package
                }),
            Self::Application {
                declaration,
                package,
                application: None,
            } => {
                let matches = |name, owner| {
                    schema.identity_text(name) == declaration
                        && (package.is_none() || owner == *package)
                };
                matches(
                    schema.declaration.trait_name,
                    schema.declaration.trait_package_identity,
                ) || schema
                    .declaration
                    .receiver_applications
                    .iter()
                    .any(|receiver| matches(receiver.trait_name, receiver.trait_package_identity))
            }
        }
    }

    fn description(&self) -> &str {
        match self {
            Self::Application {
                declaration,
                application,
                ..
            } => application.as_deref().unwrap_or(declaration),
            Self::Selected { label, .. } => label,
        }
    }
}

/// Reject a cycle in the direct synchronous graph realized by the concrete
/// provider selection. Reach closure is intentionally irrelevant. Checked
/// adapters contribute actual requirement applications; opaque supplies retain
/// their authored invocation ceilings. A missing selected target cannot
/// manufacture an edge. The caller owns checked-body edge production, not slot
/// matching or cycle policy.
pub fn validate_selected_synchronous_invocation_cycles<'plan>(
    selected_plans: impl IntoIterator<Item = &'plan ProviderPlan>,
    mut checked_targets: impl FnMut(
        &ProviderPlan,
        &effects::provider_plan::ProviderPlanRow,
    )
        -> Result<Vec<SynchronousBoundaryTarget>, diagnostics::Diagnostic>,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let selected = exact_selected_synchronous_plans(selected_plans)?;
    let mut edges = vec![Vec::<usize>::new(); selected.len()];
    let mut diagnostics = Vec::new();
    for (source_index, source) in selected.iter().enumerate() {
        // Only bound rows grant supply. Ordinary selections independently owe
        // full schema coverage; partial toolchain cohorts may leave a leaf
        // unbound, but no marker exempts a present row from its call edges.
        for binding in &source.rows {
            let methods = source
                .schema
                .declaration
                .methods
                .iter()
                .filter(|method| {
                    source
                        .schema
                        .row_binds_method(binding, method, &source.identities)
                })
                .collect::<Vec<_>>();
            let [method] = methods.as_slice() else {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "ProviderPlan `{}` synchronous row `{}` binds {} exact schema methods",
                    source.name,
                    binding.method,
                    methods.len(),
                )));
                continue;
            };
            let row = match exact_row_for_schema_method(source, method) {
                Ok(row) => row,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
            let target_names = match &row.binding {
                ProviderBinding::CheckedAdapter { .. } => checked_targets(source, row),
                _ => exact_authored_invocations(source, method).map(|targets| {
                    targets
                        .into_iter()
                        .map(|declaration| SynchronousBoundaryTarget::Application {
                            declaration,
                            package: None,
                            application: None,
                        })
                        .collect()
                }),
            };
            let target_names = match target_names {
                Ok(target_names) => target_names,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
            for invoked in target_names {
                let matching_targets = selected
                    .iter()
                    .enumerate()
                    .filter(|(_, target)| invoked.matches(target))
                    .collect::<Vec<_>>();
                match matching_targets.as_slice() {
                    [] => {}
                    [(target_index, _)] if !edges[source_index].contains(target_index) => {
                        edges[source_index].push(*target_index);
                    }
                    [_] => {}
                    _ => diagnostics.push(diagnostics::Diagnostic::error(format!(
                        "selected synchronous invocation `{}` from `{}::{}` is ambiguous across {} package-qualified boundary slots",
                        invoked.description(),
                        source.schema.identity_text(source.schema.declaration.trait_name),
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
                .map(|index| {
                    selected[*index]
                        .schema
                        .identity_text(selected[*index].schema.declaration.trait_name)
                })
                .chain(std::iter::once(selected[cycle[0]].schema.identity_text(
                    selected[cycle[0]].schema.declaration.trait_name,
                )))
                .collect::<Vec<_>>()
                .join(" -> ");
            return Err(vec![diagnostics::Diagnostic::error(format!(
                "selected providers realize a cyclic synchronous `invokes` graph: {names}; break one edge with a mailbox, queue, scheduler handoff, or other new activation",
            ))]);
        }
    }
    Ok(())
}

fn exact_selected_synchronous_plans<'plan>(
    selected_plans: impl IntoIterator<Item = &'plan ProviderPlan>,
) -> Result<Vec<&'plan ProviderPlan>, Vec<diagnostics::Diagnostic>> {
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
        if plan
            .schema
            .identity_text(plan.schema.declaration.trait_name)
            .is_empty()
        {
            diagnostics.push(diagnostics::Diagnostic::error(format!(
                "selected synchronous-invocation ProviderPlan `{}` has an empty exact schema identity",
                plan.name,
            )));
            continue;
        }
        let schema_identity = plan.schema.selection_slot_identity();
        if seen_schemas.contains(&schema_identity) {
            diagnostics.push(diagnostics::Diagnostic::error(format!(
                "selected synchronous-invocation schema `{}` is realized by more than one selected ProviderPlan",
                plan.schema.identity_text(plan.schema.declaration.trait_name),
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

pub fn exact_row_for_schema_method<'plan>(
    plan: &'plan ProviderPlan,
    method: &effects::provider_plan::ServiceMethod,
) -> Result<&'plan ProviderPlanRow, diagnostics::Diagnostic> {
    if plan
        .schema
        .identity_text(method.requirement_identity)
        .is_empty()
    {
        return Err(diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` schema method `{}` has no exact synchronous-invocation overload identity",
            plan.name, method.name,
        )));
    }
    let method_count = plan
        .schema
        .declaration
        .methods
        .iter()
        .filter(|candidate| candidate.requirement_identity == method.requirement_identity)
        .count();
    if method_count != 1 {
        return Err(diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` contains {method_count} schema methods for exact synchronous-invocation overload `{}`",
            plan.name,
            plan.schema.identity_text(method.requirement_identity),
        )));
    }
    let rows = plan
        .rows
        .iter()
        .filter(|row| plan.schema.row_binds_method(row, method, &plan.identities))
        .collect::<Vec<_>>();
    let [row] = rows.as_slice() else {
        return Err(diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` schema method `{}` / `{}` binds {} exact synchronous-invocation rows",
            plan.name,
            method.name,
            plan.schema.identity_text(method.requirement_identity),
            rows.len(),
        )));
    };
    Ok(*row)
}

fn exact_authored_invocations(
    plan: &ProviderPlan,
    method: &effects::provider_plan::ServiceMethod,
) -> Result<Vec<String>, diagnostics::Diagnostic> {
    if method
        .synchronous_invocations
        .iter()
        .any(|target| target.is_empty())
    {
        return Err(diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` exact overload `{}` has an empty synchronous-invocation identity",
            plan.name,
            plan.schema.identity_text(method.requirement_identity),
        )));
    }
    if method
        .synchronous_invocations
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(diagnostics::Diagnostic::error(format!(
            "ProviderPlan `{}` exact overload `{}` synchronous-invocation identities are not strictly increasing",
            plan.name,
            plan.schema.identity_text(method.requirement_identity),
        )));
    }
    Ok(method.synchronous_invocations.clone())
}
