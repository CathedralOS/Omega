//! Name the actual calls and incoming dependency registration established
//! during Product admission in the matching checked module.
use super::super::{declarations::unavailable, input::PolicySource};
use diagnostics::Diagnostic;
use review_evidence::record::PackageReviewNominalIdentity;
use semantic_vocabulary::{PackageKeyIdentity, TargetMachineCallable as Callable};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewedUnsafePath {
    pub admission: ReviewedUnsafeAdmission,
    pub calls: Vec<PackageReviewNominalIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewedUnsafeAdmission {
    Product,
    Dependency {
        requester: Option<PackageKeyIdentity>,
        target: PackageKeyIdentity,
        alias: String,
    },
}

pub(super) fn paths(
    input: &PolicySource<'_>,
    selected: &compiler::SelectedProviderResult,
) -> Result<Vec<(Callable, ReviewedUnsafePath)>, Vec<Diagnostic>> {
    let Some(review) = selected.package_review() else {
        return Ok(Vec::new());
    };
    let elaboration = input.accepted().closure().elaboration();
    let mut result = Vec::with_capacity(review.admitted_paths.len());
    for path in &review.admitted_paths {
        let &origin = path
            .calls
            .last()
            .ok_or_else(|| unavailable("the origin of an admitted Unsafe path"))?;
        let calls = path
            .calls
            .iter()
            .map(|call| {
                let symbol = match *call {
                    Callable::Machine(id) => elaboration.machines.get(id),
                    Callable::Boundary(id) => elaboration.boundaries.get(id),
                }
                .ok_or_else(|| unavailable("a callable on the admitted Unsafe path"))?;
                input.nominal(*symbol)
            })
            .collect::<Result<_, _>>()?;
        let admission = match &path.dependency {
            Some(edge) => ReviewedUnsafeAdmission::Dependency {
                requester: edge.requester,
                target: edge.target,
                alias: edge.alias.clone(),
            },
            None => ReviewedUnsafeAdmission::Product,
        };
        result.push((origin, ReviewedUnsafePath { admission, calls }));
    }
    result.sort_by_key(|(origin, _)| *origin);
    Ok(result)
}

pub(super) fn for_callable(
    paths: &[(Callable, ReviewedUnsafePath)],
    callable: Callable,
) -> Vec<ReviewedUnsafePath> {
    let start = paths.partition_point(|(origin, _)| *origin < callable);
    paths[start..]
        .iter()
        .take_while(|(origin, _)| *origin == callable)
        .map(|(_, path)| path.clone())
        .collect()
}
