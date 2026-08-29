use super::super::semantics::declarations::nominal_identity;
use crate::evidence::PackageReviewMutation;
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;

pub(crate) fn project_mutation(
    compilation: &CheckedCompilation,
    plans: &[psi_checked_trees::StateWriteFramePlan],
) -> Result<Vec<PackageReviewMutation>, Vec<Diagnostic>> {
    let mut projected = plans
        .iter()
        .map(|plan| {
            Ok(PackageReviewMutation {
                state: nominal_identity(compilation, plan.state)?,
                completeness: plan.frame.completeness(),
                paths: plan.frame.paths().to_vec(),
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    projected.sort_by(|left, right| {
        left.state
            .cmp(&right.state)
            .then_with(|| {
                mutation_completeness_tag(left.completeness)
                    .cmp(&mutation_completeness_tag(right.completeness))
            })
            .then_with(|| left.paths.cmp(&right.paths))
    });
    projected.dedup();
    Ok(projected)
}

const fn mutation_completeness_tag(completeness: psi_facts::WriteFrameCompleteness) -> u8 {
    match completeness {
        psi_facts::WriteFrameCompleteness::Complete => 1,
        psi_facts::WriteFrameCompleteness::Opaque => 2,
    }
}
