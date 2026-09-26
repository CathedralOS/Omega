//! Task machine suspension, blocking and the selected runtime provider.

use crate::task_plans::start_selections::TaskStartSelection;
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use task_plans::{SelectedTaskRuntimeProviderFact, TaskRuntimeId, TaskStartOperation};

pub(crate) fn exact_task_machine_suspension(
    program: &CheckedTrees,
    machine: symbols::SymbolHandle,
    machine_name: &str,
) -> Result<language_semantics::SuspensionPlan, Vec<Diagnostic>> {
    let mut matches = program
        .facts
        .suspensions
        .machines
        .iter()
        .filter(|fact| fact.machine == machine);
    let plan = matches.next().map(|fact| fact.plan).ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "task activation target `{machine_name}` has no checked suspension plan"
        ))]
    })?;
    if matches.next().is_some() {
        return Err(vec![Diagnostic::error(format!(
            "task activation target `{machine_name}` has duplicate exact checked suspension plans"
        ))]);
    }
    Ok(plan)
}

pub(crate) fn exact_task_machine_blocking(
    program: &CheckedTrees,
    machine: symbols::SymbolHandle,
    machine_name: &str,
) -> Result<language_semantics::BlockingPlan, Vec<Diagnostic>> {
    let mut matches = program
        .facts
        .blocking
        .machines
        .iter()
        .filter(|fact| fact.machine == machine);
    let plan = matches.next().map(|fact| fact.plan).ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "task activation target `{machine_name}` has no checked blocking plan"
        ))]
    })?;
    if matches.next().is_some() {
        return Err(vec![Diagnostic::error(format!(
            "task activation target `{machine_name}` has duplicate exact checked blocking plans"
        ))]);
    }
    Ok(plan)
}

pub(crate) fn selected_task_runtime_provider(
    program: &CheckedTrees,
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
    selection: &TaskStartSelection,
) -> Result<SelectedTaskRuntimeProviderFact, Vec<Diagnostic>> {
    let (requirement_owner, _, authored_requirement_identity) =
        exact_task_runtime_requirement(program, selection)?;
    let requirement_owner_package = program
        .typed
        .symbols
        .symbol_package_identity(requirement_owner.symbol);
    let matches = selected_provider_plans
        .plans()
        .iter()
        .filter(|plan| {
            crate::service_schema::schema_binds_exact_boundary_trait(
                &program.typed,
                &plan.schema,
                requirement_owner,
            ) && plan.schema.trait_package_identity == requirement_owner_package
                && plan.schema.methods.iter().any(|method| {
                    method.requirement_owner
                        == program.typed.trait_declaration_path(requirement_owner)
                        && method.requirement_owner_package_identity == requirement_owner_package
                        && method.requirement_identity == authored_requirement_identity
                })
        })
        .collect::<Vec<_>>();
    let [plan] = matches.as_slice() else {
        return Err(vec![Diagnostic::error(match matches.len() {
            0 => "TaskRuntime boundary slot has no retained selected provider plan".to_owned(),
            count => format!(
                "TaskRuntime boundary slot matches {count} retained selected provider plans"
            ),
        })]);
    };
    let requirement_name = match selection.operation {
        TaskStartOperation::Start => "start",
        TaskStartOperation::TryStart => "try_start",
    };
    let requirements = plan
        .schema
        .methods
        .iter()
        .filter(|method| {
            method.requirement_owner == program.typed.trait_declaration_path(requirement_owner)
                && method.requirement_owner_package_identity == requirement_owner_package
                && method.requirement_identity == authored_requirement_identity
        })
        .collect::<Vec<_>>();
    let [method] = requirements.as_slice() else {
        return Err(vec![Diagnostic::error(match requirements.len() {
            0 => format!(
                "selected TaskRuntime provider plan `{}` has no `{requirement_name}` requirement",
                plan.name
            ),
            count => format!(
                "selected TaskRuntime provider plan `{}` has {count} `{requirement_name}` requirements; operation binding must be exact",
                plan.name
            ),
        })]);
    };
    if method.requirement_identity != authored_requirement_identity {
        return Err(vec![Diagnostic::error(format!(
            "selected TaskRuntime provider plan `{}` binds `{requirement_name}` as `{}`, but the activation call requires `{authored_requirement_identity}`",
            plan.name, method.requirement_identity
        ))]);
    }
    let covering_rows = plan
        .rows
        .iter()
        .filter(|row| plan.schema.row_binds_method(row, method))
        .count();
    if covering_rows != 1 {
        return Err(vec![Diagnostic::error(format!(
            "selected TaskRuntime provider plan `{}` binds `{requirement_name}` {covering_rows} times; exactly one operation realization is required",
            plan.name
        ))]);
    }
    let runtime = TaskRuntimeId::from_normalized_identity(plan.report_fingerprint())
        .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
    Ok(SelectedTaskRuntimeProviderFact {
        runtime,
        provider_plan_name: plan.name.clone(),
        requirement_identity: method.requirement_identity.clone(),
    })
}

pub(crate) fn exact_task_runtime_requirement<'program>(
    program: &'program CheckedTrees,
    selection: &TaskStartSelection,
) -> Result<
    (
        &'program checked_trees::trait_definition::TraitDefinition,
        &'program checked_trees::signature::StateSignature,
        String,
    ),
    Vec<Diagnostic>,
> {
    let mut owners = program
        .traits()
        .iter()
        .filter(|definition| definition.symbol == selection.requirement_owner);
    let owner = owners.next().ok_or_else(|| {
        vec![Diagnostic::error(
            "TaskRuntime activation requirement must name one exact retained trait owner",
        )]
    })?;
    if owners.next().is_some() {
        return Err(vec![Diagnostic::error(
            "TaskRuntime activation requirement owner must resolve uniquely",
        )]);
    }
    if !owner.is_boundary {
        return Err(vec![Diagnostic::error(
            "TaskRuntime activation requirement owner must be an exact boundary trait",
        )]);
    }
    let mut signatures = program
        .trait_machine_signatures(owner)
        .iter()
        .filter(|signature| signature.symbol == selection.requirement);
    let signature = signatures.next().ok_or_else(|| {
        vec![Diagnostic::error(
            "TaskRuntime activation requirement must belong to its exact retained owner",
        )]
    })?;
    if signatures.next().is_some() {
        return Err(vec![Diagnostic::error(
            "TaskRuntime activation requirement must resolve uniquely within its exact owner",
        )]);
    }
    let identity = program
        .normalized_trait_requirement_overload_identity(owner, signature)
        .identity();
    Ok((owner, signature, identity))
}
