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

pub(crate) use access_value::compute_selected_placement_plan;

fn evaluate_selected_policy(
    typed: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    arguments: Vec<BuildTimeValue>,
    policy_kind: &str,
) -> Result<BuildTimeValue, String> {
    let policy_machine = machine.name.as_str();
    BuildTimeAdmissionPlan::infer(typed, None).require_common_floor(typed, machine)?;
    checked_interpreter::evaluate_build_time_machine(
        typed,
        super::policy_arguments::policy_request(typed, machine, arguments)?,
    )
    .map(BuildTimeOperationEvaluation::into_value)
    .map_err(|reason| {
        format!("build-time evaluation of {policy_kind} policy `{policy_machine}` failed: {reason}")
    })
}
