use crate::BackendReportInput;
use omega_machine_bytes::EncodedMachineBoundarySummary;

pub(super) fn write_artifact_semantic_spine(
    output: &mut String,
    backend_plan: &BackendReportInput<'_>,
) {
    output.push_str("## Artifact Semantic Spine\n");
    output.push_str(&format!(
        "values: {}\n",
        backend_plan.value_summary().values.len()
    ));
    output.push_str(&format!(
        "moves: {}\n",
        backend_plan.ownership_summary().moves.len()
    ));
    output.push_str(&format!(
        "drops: {}\n",
        backend_plan.ownership_summary().drops.len()
    ));
    write_boundary_policy_checks(output, backend_plan.boundary_summary());
    output.push('\n');
}

fn write_boundary_policy_checks(output: &mut String, boundaries: &EncodedMachineBoundarySummary) {
    output.push_str(&format!(
        "boundary policy checks: {}\n",
        boundaries.policy_checks.len()
    ));
    if boundaries.policy_checks.is_empty() {
        output.push_str("none\n");
        return;
    }

    for (_, check) in boundaries.policy_checks.iter() {
        output.push_str(&format!(
            "- {:?} `{}` operation {:?}\n",
            check.verdict, check.boundary_policy, check.operation_key
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_abstract_operations::{AbstractBoundaryPolicyCheck, AbstractBoundaryPolicyVerdict};
    use omega_machine_bytes::EncodedMachineBoundarySummary;
    use std::sync::Arc;

    #[test]
    fn writes_boundary_policy_checks_from_preserved_semantic_spine() {
        let mut boundaries = EncodedMachineBoundarySummary::default();
        boundaries
            .policy_checks
            .insert(AbstractBoundaryPolicyCheck {
                boundary_policy: Arc::from("omega::core::Slice::Index"),
                verdict: AbstractBoundaryPolicyVerdict::DisallowedBoundaryPolicy,
                ..AbstractBoundaryPolicyCheck::default()
            });

        let mut output = String::new();
        write_boundary_policy_checks(&mut output, &boundaries);

        assert!(output.contains("boundary policy checks: 1"));
        assert!(output.contains("DisallowedBoundaryPolicy"));
        assert!(output.contains("omega::core::Slice::Index"));
    }
}
