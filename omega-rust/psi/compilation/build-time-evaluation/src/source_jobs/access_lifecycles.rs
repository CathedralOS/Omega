//! Complete semantic policy requests of actual closed lifecycle calls before
//! source publication. Geometry remains an explicit selected-target demand.

use std::sync::Arc;

use diagnostics::Diagnostic;
use typed_trees::static_applications::StaticMachineBindingValue;
use typed_trees::{
    AccessContentShape, AccessExecutionDeclaration, AccessLifecyclePlan, TypedTrees,
};

pub fn evaluate(
    program: &mut TypedTrees,
    request: terminal_production::TerminalSourceCompilation<'_>,
    authority: Option<Arc<dyn crate::BuildTimeSelectionAuthority>>,
    consent: Option<Arc<dyn crate::InvocationSourceConsent>>,
    accounting: &mut crate::SourceCompletionAccounting,
) -> Result<(), Vec<Diagnostic>> {
    let demanded = super::constant_jobs::demanded_declarations(program, request)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let mut pending = Vec::new();
    for (application, occurrence) in program.expression_table.static_machine_applications() {
        if !demanded.contains(occurrence.lexical_owner) {
            continue;
        }
        // A retained template is a declaration dependency of its closed
        // specialization, not a request to evaluate policy for open P/T.
        let owner = program
            .machine_by_symbol(occurrence.lexical_owner)
            .or_else(|| program.machine_holding_state(occurrence.lexical_owner));
        if owner.is_some_and(|owner| !program.machine_type_parameters(owner).is_empty()) {
            continue;
        }
        let target = occurrence.binding_declaration;
        let Some(execution) = AccessExecutionDeclaration::authenticate(program, target)
            .map_err(|diagnostic| vec![diagnostic])?
        else {
            continue;
        };
        // A nonresident view has an existing selected Source/View contract;
        // it has no implicit policy/element pair to manufacture here.
        if execution.content != AccessContentShape::Run
            && execution.operation == typed_trees::AccessLifecycleOperation::View
        {
            continue;
        }
        let signature = program
            .bodyless_callable_signature(application, target)
            .map_err(|reason| vec![Diagnostic::error(reason)])?
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "placement demand lost its closed callable occurrence",
                )]
            })?;
        let bindings = program
            .expression_table
            .static_machine_application_bindings(application);
        if bindings.len()
            != execution
                .type_parameter_count()
                .map_err(|diagnostic| vec![diagnostic])?
        {
            return Err(vec![Diagnostic::error(
                "placement demand changed its complete static roster",
            )]);
        }
        let (
            StaticMachineBindingValue::Conformance(policy),
            StaticMachineBindingValue::Type(schema),
        ) = (&bindings[0].value, &bindings[1].value)
        else {
            return Err(vec![Diagnostic::error(
                "placement demand lost its policy value or element type",
            )]);
        };
        if !signature.type_parameters.is_empty() {
            return Err(vec![Diagnostic::error(
                "placement demand still has an open static binder",
            )]);
        }
        if program.access_lifecycle_plans.iter().any(|(_, plan)| {
            matches!(plan,
            AccessLifecyclePlan::Native { application: known, .. } if *known == application)
        }) {
            continue;
        }
        pending.push((
            application,
            target,
            occurrence.lexical_owner,
            execution.operation,
            policy.clone(),
            *schema,
        ));
    }
    if pending.is_empty() {
        return Ok(());
    }
    // Keep an immutable request owner while all selected computations run.
    // Inserting an earlier result cannot affect a later policy's source.
    let prepared = super::PreparedBuildMachineProgram::prepare_for(
        program,
        authority,
        request.purpose,
        consent,
    )?;
    let mut completed = Vec::with_capacity(pending.len());
    for (application, callable, requester, operation, policy_selection, schema) in pending {
        let (policy_machine, normalized) = super::domain_access_plans::evaluate_policy(
            program,
            &prepared,
            requester,
            &policy_selection,
            schema,
            accounting,
        )
        .map_err(|reason| vec![Diagnostic::error(format!("placement policy: {reason}"))])?;
        completed.push(AccessLifecyclePlan::Native {
            callable,
            application,
            operation,
            policy_selection,
            schema,
            policy_machine,
            normalized,
        });
    }
    for plan in completed {
        program.access_lifecycle_plans.insert(plan);
    }
    Ok(())
}
