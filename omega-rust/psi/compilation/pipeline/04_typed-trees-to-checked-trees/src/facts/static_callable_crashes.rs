//! Universal crash refinement consumes the same completed private summary as
//! ordinary calls. An omitted authored private clause requests inference; it
//! cannot stand for an empty actual ceiling before that inference completes.
//! The retained obligation owns its exact applied requirement and selected
//! entry, including demands from target-family members without public rows.
//! No selecting caller's values or guards participate in this comparison.

use super::crash_calls::CompletedBodyCrashSummaries;
use checked_trees::{
    CheckedBooleanExpression as Boolean, CheckedOperatorFacts, CrashInterface, CrashRouteGuard,
    MachineContractPlans,
};
use diagnostics::Diagnostic;
use validation::StaticMachineContractObligation;

pub(super) fn validate(
    program: &dyn validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    obligations: &[StaticMachineContractObligation],
    plans: &MachineContractPlans,
    completed: &CompletedBodyCrashSummaries,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    for obligation in obligations {
        if let Err(reason) = validate_one(program, operators, obligation, plans, completed) {
            diagnostics.push(Diagnostic::error(format!(
                "{} does not refine its callable crash contract: {reason}",
                obligation.label()
            )));
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn validate_one(
    program: &dyn validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    obligation: &StaticMachineContractObligation,
    plans: &MachineContractPlans,
    completed: &CompletedBodyCrashSummaries,
) -> Result<(), String> {
    let requirement = obligation
        .required_signature(program)
        .map_err(str::to_owned)?;
    let required = super::crash_plan_facts::derive_authored_signature_crash_buckets(
        program,
        requirement,
        Some(operators),
    )
    .map_err(|error| format!("the exact required crash ceiling cannot be normalized: {error:?}"))?;
    let selected = program
        .machine_by_own_or_state_symbol(obligation.selected_entry())
        .ok_or_else(|| "the selected entry has no concrete callable owner".to_owned())?;
    let plan = plans
        .for_machine(selected.symbol)
        .ok_or_else(|| "the selected callable has no completed checked contract plan".to_owned())?;
    let actual = match plan.crash.interface() {
        CrashInterface::PublishedCeiling => {
            super::crash_plan_facts::derive_authored_machine_crash_buckets(
                program,
                selected,
                Some(operators),
            )
            .map_err(|error| {
                format!("the selected published ceiling cannot be normalized: {error:?}")
            })?
        }
        CrashInterface::InternalInferred => {
            completed.checked_buckets(selected.symbol).ok_or_else(|| {
                "the selected private callable has an unresolved crash summary".to_owned()
            })?
        }
    };
    let entry = program
        .machine_states(selected)
        .first()
        .ok_or_else(|| "the selected callable has no exact entry telescope".to_owned())?;
    let parameters = program.state_parameters(entry);
    // A callable refinement is universal over its required inputs. These are
    // the exact applied signature's premises, rebound to the actual entry;
    // no value or path from the selecting caller participates. A premise that
    // cannot be represented supplies no proof rather than an invented atom.
    let premises = program
        .state_signature_contracts(requirement)
        .iter()
        .filter(|contract| contract.kind == typed_trees::signature::SignatureContractKind::Requires)
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .filter_map(|fact| {
            let expression = fact.as_expression()?;
            let checked = crate::values::lower_signature_crash_contract_expression(
                program,
                operators,
                requirement,
                *expression,
            )?;
            super::boolean_predicates::bind_parameters(
                program,
                selected.symbol,
                entry.symbol,
                parameters,
                None,
                checked,
            )
        })
        .collect::<Vec<_>>();
    for bucket in &actual {
        let allowed = required
            .iter()
            .filter(|permitted| permitted.cause() == bucket.cause())
            .flat_map(|permitted| permitted.alternative_guards())
            .filter_map(checked_guard)
            .reduce(|left, right| Boolean::Or {
                left: Box::new(left),
                right: Box::new(right),
            })
            .unwrap_or(Boolean::Constant(false));
        for route in bucket.alternative_guards() {
            let under_required_inputs = checked_guard(route)
                .and_then(|actual| {
                    super::boolean_predicates::bind_parameters(
                        program,
                        selected.symbol,
                        entry.symbol,
                        parameters,
                        None,
                        Boolean::Or {
                            left: Box::new(Boolean::Not(Box::new(actual))),
                            right: Box::new(allowed.clone()),
                        },
                    )
                })
                .is_some_and(|goal| super::boolean_predicates::proves(&premises, &goal, |_| None));
            if !under_required_inputs
                && !required
                    .iter()
                    .filter(|permitted| permitted.cause() == bucket.cause())
                    .any(|permitted| {
                        permitted
                            .alternative_guards()
                            .iter()
                            .any(|guard| crate::checks::call_route_guard_covers(guard, route, &[]))
                    })
            {
                return Err(format!(
                    "its completed {:?} crash routes exceed the required ceiling",
                    bucket.cause()
                ));
            }
        }
    }
    Ok(())
}

/// A missing annotation remains unavailable; structural identity comparison
/// above can still authenticate an identical opaque route without reinterpreting it.
fn checked_guard(guard: &CrashRouteGuard) -> Option<Boolean> {
    match guard {
        CrashRouteGuard::Truth => Some(Boolean::Constant(true)),
        CrashRouteGuard::Predicate(predicate) => predicate.scalar_expression().cloned(),
    }
}
