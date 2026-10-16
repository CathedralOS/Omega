//! Task machine suspension, blocking and the selected runtime provider.

use crate::task_plans::start_selections::TaskStartSelection;
use diagnostics::Diagnostic;
use task_plans::{SelectedTaskRuntimeProviderFact, TaskRuntimeId, TaskStartOperation};

fn exact_task_machine_plan<P>(
    matches: impl Iterator<Item = (symbols::SymbolHandle, P)>,
    machine: symbols::SymbolHandle,
    machine_name: &str,
    plan_domain: &'static str,
) -> Result<P, Vec<Diagnostic>> {
    let mut matches = matches.filter(|(fact_machine, _)| *fact_machine == machine);
    let (_, plan) = matches.next().ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "task activation target `{machine_name}` has no checked {plan_domain} plan"
        ))]
    })?;
    if matches.next().is_some() {
        return Err(vec![Diagnostic::error(format!(
            "task activation target `{machine_name}` has duplicate exact checked {plan_domain} plans"
        ))]);
    }
    Ok(plan)
}

pub(crate) fn exact_task_machine_suspension(
    check_facts: &checked_trees::CheckFacts,
    machine: symbols::SymbolHandle,
    machine_name: &str,
) -> Result<language_semantics::SuspensionPlan, Vec<Diagnostic>> {
    exact_task_machine_plan(
        check_facts
            .suspensions
            .machines
            .iter()
            .map(|fact| (fact.machine, fact.plan)),
        machine,
        machine_name,
        "suspension",
    )
}

pub(crate) fn exact_task_machine_blocking(
    check_facts: &checked_trees::CheckFacts,
    machine: symbols::SymbolHandle,
    machine_name: &str,
) -> Result<language_semantics::BlockingPlan, Vec<Diagnostic>> {
    exact_task_machine_plan(
        check_facts
            .blocking
            .machines
            .iter()
            .map(|fact| (fact.machine, fact.plan)),
        machine,
        machine_name,
        "blocking",
    )
}

pub(crate) fn selected_task_runtime_provider(
    typed: &checked_trees::typed_trees::TypedTrees,
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
    selection: &TaskStartSelection,
) -> Result<
    (
        SelectedTaskRuntimeProviderFact,
        checked_trees::typed_trees::type_identity::CallableIdentityId,
    ),
    Vec<Diagnostic>,
> {
    let (requirement_owner, _, authored_requirement_identity) =
        exact_task_runtime_requirement(typed, selection)?;
    let requirement_owner_package = typed
        .symbols
        .symbol_package_identity(requirement_owner.symbol);
    let matches = selected_provider_plans
        .plans()
        .iter()
        .filter(|plan| {
            crate::service_schema::schema_binds_exact_boundary_trait(
                crate::ProviderPlanningContext::new(&typed, source::DependencyScope::Product),
                &plan.schema,
                requirement_owner,
            ) && plan.schema.trait_package_identity == requirement_owner_package
                && plan.schema.methods.iter().any(|method| {
                    method.requirement_owner == typed.trait_declaration_path(requirement_owner)
                        && method.requirement_owner_package_identity == requirement_owner_package
                        && typed.callable_identity_by_text(
                            plan.schema.identity_text(method.requirement_identity),
                        ) == authored_requirement_identity
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
            method.requirement_owner == typed.trait_declaration_path(requirement_owner)
                && method.requirement_owner_package_identity == requirement_owner_package
                && typed.callable_identity_by_text(
                    plan.schema.identity_text(method.requirement_identity),
                ) == authored_requirement_identity
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
    if typed.callable_identity_by_text(plan.schema.identity_text(method.requirement_identity))
        != authored_requirement_identity
    {
        return Err(vec![Diagnostic::error(format!(
            "selected TaskRuntime provider plan `{}` binds `{requirement_name}` as `{}`, but the activation call requires `{}`",
            plan.name,
            plan.schema.identity_text(method.requirement_identity),
            typed.callable_identity_text(authored_requirement_identity)
        ))]);
    }
    let covering_rows = plan
        .rows
        .iter()
        .filter(|row| plan.schema.row_binds_method(row, method, &plan.identities))
        .count();
    if covering_rows != 1 {
        return Err(vec![Diagnostic::error(format!(
            "selected TaskRuntime provider plan `{}` binds `{requirement_name}` {covering_rows} times; exactly one operation realization is required",
            plan.name
        ))]);
    }
    let runtime = TaskRuntimeId::from_normalized_identity(plan.report_fingerprint())
        .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
    // The local callable handle is used only to derive the specialization
    // commitment. It cannot travel in runtime evidence without its typed arena.
    Ok((
        SelectedTaskRuntimeProviderFact {
            runtime,
            provider_plan_name: plan.name.clone(),
        },
        authored_requirement_identity,
    ))
}

pub(crate) fn exact_task_runtime_requirement<'program>(
    typed: &'program checked_trees::typed_trees::TypedTrees,
    selection: &TaskStartSelection,
) -> Result<
    (
        &'program checked_trees::trait_definition::TraitDefinition,
        &'program checked_trees::signature::StateSignature,
        checked_trees::typed_trees::type_identity::CallableIdentityId,
    ),
    Vec<Diagnostic>,
> {
    let owner = typed
        .trait_definition_by_symbol(selection.requirement_owner)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "TaskRuntime activation requirement must name one exact retained trait owner",
            )]
        })?;
    if !owner.is_boundary {
        return Err(vec![Diagnostic::error(
            "TaskRuntime activation requirement owner must be an exact boundary trait",
        )]);
    }
    // A selected generic provider covers the authored slot. The call's closed
    // application remains on the selection and in its strong commitment;
    // it cannot be mistaken for another method in this provider schema.
    let application = typed
        .closed_requirement_application(selection.requirement)
        .filter(|application| application.declaring_trait == owner.symbol)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "TaskRuntime activation requirement must retain its exact closed application",
            )]
        })?;
    let signature = typed
        .trait_machine_signature_by_symbol(owner, application.template)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "TaskRuntime activation requirement must belong to its exact retained owner",
            )]
        })?;
    let identity = typed.trait_requirement_callable_identity(owner, signature);
    Ok((owner, signature, identity))
}
