//! Build-time evaluation of source-authored placed-access policies.
//!
//! Source policy records are ordinary values. This module is the compiler
//! boundary that reflects a schema, evaluates an effect-free `Access::plan` or
//! `Placement::plan` machine, and converts the result into the sealed
//! normalized model consumed by admission and lowering.

use access_plans::{AccessPlan, ValidatedAccessPlan};
use checked_interpreter::BuildTimeValue;
use layout_plans::{LayoutPlanReport, layout_plan_reports_match_for_replay};
use typed_trees::TypedTrees;

use crate::BuildTimeAdmissionPlan;
use crate::layout_plans::{build_schema_value, schema_fields, validate_plan};

mod access_value;
mod layout_value;

pub use access_value::compute_placement_plan;
use access_value::validate_access_value;
use layout_value::build_layout_plan_value;

/// Evaluate and validate one source `Access::plan` machine against an already
/// validated layout for the same reflected schema.
pub fn compute_access_plan(
    typed: &TypedTrees,
    policy_machine: &str,
    schema_data: &str,
    layout: &LayoutPlanReport,
) -> Result<ValidatedAccessPlan, String> {
    let (schema_fields, schema_report_fingerprint) = schema_fields(typed, schema_data)?;
    if layout.schema_report_fingerprint != schema_report_fingerprint {
        return Err(format!(
            "validated layout schema report fingerprint {} does not match reflected schema `{schema_data}` report fingerprint {schema_report_fingerprint}",
            layout.schema_report_fingerprint
        ));
    }
    // Reject an aliased or malformed retained field-identity set before
    // canonical replay can replace numbered presentation names.
    AccessPlan::inaccessible(layout).map_err(|diagnostic| {
        format!(
            "layout supplied to access policy `{policy_machine}` is not a canonical field-identity set: {diagnostic}"
        )
    })?;
    let schema_value = build_schema_value(typed, schema_data, &schema_fields)?;
    let layout_value = build_layout_plan_value(layout, &schema_fields)?;
    let canonical_layout = validate_plan(
        &layout_value,
        &schema_fields,
        schema_report_fingerprint,
        policy_machine,
    )?;
    // Numbered source names are presentation. Acceptance still replays the
    // exact structure rather than trusting its compact report/cache hash.
    if !layout_plan_reports_match_for_replay(&canonical_layout, layout) {
        return Err(format!(
            "layout supplied to access policy `{policy_machine}` is not the canonical validated layout for schema `{schema_data}`"
        ));
    }
    let plan = evaluate_policy(
        typed,
        policy_machine,
        vec![schema_value, layout_value],
        "access",
    )?;
    validate_access_value(&plan, &schema_fields, &canonical_layout, policy_machine)
}

fn evaluate_policy(
    typed: &TypedTrees,
    policy_machine: &str,
    arguments: Vec<BuildTimeValue>,
    policy_kind: &str,
) -> Result<BuildTimeValue, String> {
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == policy_machine)
        .ok_or_else(|| format!("no machine named `{policy_machine}` exists"))?;
    BuildTimeAdmissionPlan::infer(typed).require_common_floor(typed, machine)?;
    checked_interpreter::evaluate_build_time_machine(typed, policy_machine, arguments).map_err(
        |reason| {
            format!(
                "build-time evaluation of {policy_kind} policy `{policy_machine}` failed: {reason}"
            )
        },
    )
}
