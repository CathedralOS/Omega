use super::super::semantics::declarations::nominal_identity;
use crate::evidence::{PackageReviewMutation, PackageReviewWriteFrameCompleteness};
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
                completeness: project_write_frame_completeness(plan.frame.completeness()),
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

const fn project_write_frame_completeness(
    completeness: psi_facts::WriteFrameCompleteness,
) -> PackageReviewWriteFrameCompleteness {
    match completeness {
        psi_facts::WriteFrameCompleteness::Complete => {
            PackageReviewWriteFrameCompleteness::Complete
        }
        psi_facts::WriteFrameCompleteness::Opaque => PackageReviewWriteFrameCompleteness::Opaque,
    }
}

const fn mutation_completeness_tag(completeness: PackageReviewWriteFrameCompleteness) -> u8 {
    match completeness {
        PackageReviewWriteFrameCompleteness::Complete => 1,
        PackageReviewWriteFrameCompleteness::Opaque => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_frame_completeness_crosses_the_review_boundary_as_closed_evidence() {
        assert_eq!(
            project_write_frame_completeness(psi_facts::WriteFrameCompleteness::Complete),
            PackageReviewWriteFrameCompleteness::Complete,
        );
        assert_eq!(
            project_write_frame_completeness(psi_facts::WriteFrameCompleteness::Opaque),
            PackageReviewWriteFrameCompleteness::Opaque,
        );
    }
}
