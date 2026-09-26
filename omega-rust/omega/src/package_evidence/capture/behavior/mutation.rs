use super::super::semantics::declarations::nominal_identity;
use crate::package_evidence::capture::PackageReviewInput;
use crate::package_evidence::record::{PackageReviewMutation, PackageReviewWriteFrameCompleteness};
use diagnostics::Diagnostic;

pub(crate) fn project_mutation(
    compilation: &PackageReviewInput<'_>,
    plans: &[typed_trees_to_checked_trees::checked_trees::StateWriteFramePlan],
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
    completeness: typed_trees_to_checked_trees::fact_plan::WriteFrameCompleteness,
) -> PackageReviewWriteFrameCompleteness {
    match completeness {
        typed_trees_to_checked_trees::fact_plan::WriteFrameCompleteness::Complete => {
            PackageReviewWriteFrameCompleteness::Complete
        }
        typed_trees_to_checked_trees::fact_plan::WriteFrameCompleteness::Opaque => {
            PackageReviewWriteFrameCompleteness::Opaque
        }
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
    use crate::package_evidence::capture::behavior::mutation::project_write_frame_completeness;
    use crate::package_evidence::record::PackageReviewWriteFrameCompleteness;

    #[test]
    fn write_frame_completeness_crosses_the_review_boundary_as_closed_evidence() {
        assert_eq!(
            project_write_frame_completeness(
                typed_trees_to_checked_trees::fact_plan::WriteFrameCompleteness::Complete
            ),
            PackageReviewWriteFrameCompleteness::Complete,
        );
        assert_eq!(
            project_write_frame_completeness(
                typed_trees_to_checked_trees::fact_plan::WriteFrameCompleteness::Opaque
            ),
            PackageReviewWriteFrameCompleteness::Opaque,
        );
    }
}
