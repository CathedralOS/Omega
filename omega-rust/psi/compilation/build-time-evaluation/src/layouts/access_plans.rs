//! Build-time evaluation of source-authored placed-access policies.
//!
//! Source policy records are ordinary values. This module is the compiler
//! boundary that reflects a schema, evaluates an effect-free `Placement::plan`
//! machine, and converts the result into the sealed normalized model consumed
//! by admission and lowering.

use build_time_values::BuildTimeValue;
use checked_interpreter::BuildTimeOperationEvaluation;
use typed_trees::TypedTrees;

use crate::BuildTimeAdmissionPlan;

mod access_value;

pub(crate) use access_value::compute_selected_access_plan;

fn evaluate_selected_policy(
    typed: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    arguments: Vec<BuildTimeValue>,
    policy_kind: &str,
    accounting: &crate::evaluation_accounting::EvaluationAccounting,
) -> Result<BuildTimeValue, String> {
    let policy_machine = machine.name.as_str();
    let admission = BuildTimeAdmissionPlan::infer(typed, None);
    admission.require_common_floor(typed, machine)?;
    let mut request = super::policy_arguments::policy_request(typed, machine, arguments)?;
    admission.require_invocation_failures(
        crate::BuildTimeProgram::Typed(typed),
        machine,
        &mut request.arguments,
        None,
    )?;
    accounting
        .evaluate(
            checked_interpreter::BuildMachineProgram::Typed(typed),
            request,
        )
        .map(BuildTimeOperationEvaluation::into_value)
        .map_err(|reason| {
            format!(
                "build-time evaluation of {policy_kind} policy `{policy_machine}` failed: {reason}"
            )
        })
}
