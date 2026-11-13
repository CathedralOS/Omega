//! Run the actual type query and policy call under their shared retained source.
//! Source/code/arguments/results stay in existing invocation accounting; local
//! geometry rows compute an input and confer no field or backing authority.

use crate::{
    BuildTimeValue, InvocationTarget, PreparedBuildMachineEntry, PureEvaluationRecord,
    PureTerminalEvaluationReceipt, SourceCompletionAccounting,
};

pub(super) fn evaluate_policy<'account>(
    query: &PreparedBuildMachineEntry,
    policy: &PreparedBuildMachineEntry,
    target: InvocationTarget<'_>,
    other_arguments: &[BuildTimeValue],
    accounting: &'account mut SourceCompletionAccounting,
) -> Result<
    (
        &'account PureTerminalEvaluationReceipt,
        &'account PureTerminalEvaluationReceipt,
        layout_plans::LayoutPlanReport,
        Vec<terminal_psi::TargetLayoutFieldStorage>,
    ),
    String,
> {
    if !std::ptr::eq(query.source(), policy.source()) || query.requester() != policy.requester() {
        return Err("policy and query requests have different source or requester custody".into());
    }
    let (schema_position, schema_type) =
        super::schema_values::schema_parameter_in_source(policy.source(), policy.symbol())?;
    let machine = policy
        .source()
        .machine_by_symbol(policy.symbol())
        .ok_or("policy request lost its selected machine")?;
    let [entry] = policy.source().machine_states(machine) else {
        return Err("policy request has no unique formal signature".into());
    };
    if policy.source().state_parameters(entry).len() != other_arguments.len() + 1 {
        return Err("policy request lacks its complete actual non-Schema arguments".into());
    }
    let query_position = accounting.receipts().len();
    query
        .invoke_recorded(target, &[], accounting)
        .map_err(|reason| {
            format!(
                "{reason}; during the actual schema query: selected={:?}, requester={:?}",
                query.symbol(),
                query.requester()
            )
        })?;
    let query_receipt = invocation_at(accounting, query_position, query)?;
    let geometry = super::schema_geometry::schema_geometry(query_receipt, target)?;
    let schema_value =
        super::schema_values::schema_value(query_receipt, policy.source(), schema_type, &geometry)?;
    let mut arguments = other_arguments.to_vec();
    arguments.insert(schema_position, schema_value);
    let policy_position = accounting.receipts().len();
    policy
        .invoke_recorded(target, &arguments, accounting)
        .map_err(|reason| {
            format!(
                "{reason}; during the actual policy invocation: selected={:?}, requester={:?}",
                policy.symbol(),
                policy.requester()
            )
        })?;
    let query_receipt = invocation_at(accounting, query_position, query)?;
    let policy_receipt = invocation_at(accounting, policy_position, policy)?;
    // All coordinates below came from these two executions in this operation;
    // no caller can select an arbitrary later catalogue row or Schema tuple.
    let (report, storage) =
        super::schema_values::layout_value(query_receipt, policy_receipt, &geometry)?;
    Ok((query_receipt, policy_receipt, report, storage))
}

fn invocation_at<'account>(
    accounting: &'account SourceCompletionAccounting,
    position: usize,
    entry: &PreparedBuildMachineEntry,
) -> Result<&'account PureTerminalEvaluationReceipt, String> {
    let Some(PureEvaluationRecord::Invocation(receipt)) = accounting.receipts().get(position)
    else {
        return Err("policy evaluation did not retain its actual invocation receipt".into());
    };
    if receipt.machine() != entry.symbol() || receipt.requester() != entry.requester() {
        return Err("policy evaluation receipt belongs to another request".into());
    }
    receipt.validate_charge()?;
    Ok(receipt)
}

impl crate::PreparedBuildMachineEntry {
    /// Evaluate this actual type query and the policy under their retained
    /// source owner. The returned recipes describe placement; installing
    /// storage and field access requires ordinary consumer admission.
    /// Field IDs belong to the returned query receipt's module. A consumer
    /// retains that owner and rejoins its source application/member records
    /// before installing into a different module; equal dense IDs never join
    /// two modules. The policy receipt retains the whole ordinary AccessPlan.
    pub fn evaluate_layout_policy<'account>(
        &self,
        policy: &PreparedBuildMachineEntry,
        target: InvocationTarget<'_>,
        other_arguments: &[BuildTimeValue],
        accounting: &'account mut SourceCompletionAccounting,
    ) -> Result<
        (
            &'account PureTerminalEvaluationReceipt,
            &'account PureTerminalEvaluationReceipt,
            layout_plans::LayoutPlanReport,
            Vec<terminal_psi::TargetLayoutFieldStorage>,
        ),
        String,
    > {
        evaluate_policy(self, policy, target, other_arguments, accounting)
    }
}

#[cfg(test)]
#[path = "schema_customer_tests.rs"]
mod tests;
