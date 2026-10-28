//! Task machine suspension, blocking and the selected runtime provider.

use crate::task_activation::start_selections::TaskStartSelection;
use diagnostics::Diagnostic;

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
