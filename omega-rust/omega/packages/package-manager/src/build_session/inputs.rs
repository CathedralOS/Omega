//! Reconcile complete named Build-input assignments before execution.

use crate::declarations::DependencyPurpose;
use crate::lock::PackageOccurrenceRoster;
use crate::resolution::graph::ExactTargetPackageSourceClosure;
use diagnostics::Diagnostic;

fn purpose_slot(purpose: DependencyPurpose) -> usize {
    match purpose {
        DependencyPurpose::Product => 0,
        DependencyPurpose::Build => 1,
    }
}

pub(crate) struct OccurrenceBuildInputs {
    incoming: package_compilation::BuildDependencyOccurrence,
    pub(crate) snapshot: build_evaluation::BuildSnapshotRequest,
}

/// The spec's scheduling node is package/purpose/profiles, not import alias.
/// Equal edge assignments can reuse that node; incompatible complete maps
/// reject before execution rather than granting their union or multiplying
/// package identity. Product edges propagate their requester's purposes, as
/// in PackageOccurrenceRoster; build edges always select the build purpose.
pub(crate) fn reconcile_build_inputs(
    target_closure: &ExactTargetPackageSourceClosure<'_>,
    roster: &PackageOccurrenceRoster,
    request: Option<&build_evaluation::BuildSnapshotRequest>,
) -> Result<Vec<[Option<OccurrenceBuildInputs>; 2]>, Vec<Diagnostic>> {
    use build_evaluation::BuildSnapshotRequest;
    use package_compilation::BuildDependencyOccurrence;

    let graph = target_closure.source_closure().graph();
    let mut assigned: Vec<[Option<OccurrenceBuildInputs>; 2]> =
        graph.packages().iter().map(|_| [None, None]).collect();
    let Some(request) = request else {
        return Ok(assigned);
    };
    for (occurrence, _) in request.dependency_inputs() {
        let exists = graph.packages().iter().any(|node| {
            node.source().key().identity() == occurrence.requester()
                && node.dependencies().iter().any(|edge| {
                    edge.purpose() == occurrence.purpose()
                        && edge.alias().as_str() == occurrence.alias()
                        && edge.target().identity() == occurrence.target()
                })
        });
        if !exists {
            return Err(vec![Diagnostic::error(format!(
                "named build inputs are assigned to a dependency occurrence that does not exist: {occurrence:?}"
            ))]);
        }
    }
    let empty = BuildSnapshotRequest::new(std::iter::empty::<Vec<u8>>());
    for node in graph.packages() {
        let requester = node.source().key();
        for edge in node.dependencies() {
            let occurrence = BuildDependencyOccurrence::new(
                requester.identity(),
                edge.purpose(),
                edge.alias().as_str(),
                edge.target().identity(),
            );
            let slots = request
                .dependency_input_slots(&occurrence)
                .unwrap_or_else(|| empty.inputs());
            let position = graph
                .package_position(edge.target())
                .expect("validated graph edge target");
            for &requester_purpose in roster
                .purposes(requester)
                .expect("closed occurrence roster")
            {
                let purpose = if edge.purpose().is_product() {
                    requester_purpose
                } else {
                    DependencyPurpose::Build
                };
                let assignment = &mut assigned[position][purpose_slot(purpose)];
                if let Some(previous) = assignment {
                    if previous.snapshot.inputs() != slots {
                        return Err(vec![Diagnostic::error(format!(
                            "conflicting named build input assignments for shared {:?} activation of `{}`: {:?} and {:?}; assignments must match exactly, including unassigned inputs",
                            purpose,
                            edge.target().name().as_str(),
                            previous.incoming,
                            occurrence,
                        ))]);
                    }
                } else {
                    *assignment = Some(OccurrenceBuildInputs {
                        incoming: occurrence.clone(),
                        snapshot: empty.clone().with_inputs(
                            slots
                                .iter()
                                .map(|(name, input)| (name.clone(), input.clone())),
                        )?,
                    });
                }
            }
        }
    }
    Ok(assigned)
}
