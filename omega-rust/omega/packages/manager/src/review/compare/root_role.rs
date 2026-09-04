//! Compatibility comparison for a package closure's root role.

use super::model::{
    ReviewOnlyRootRoleChange, ReviewOnlyRootRoleComparisonError, ReviewOnlyRootRoleContract,
};
use crate::declarations::BuildDeclarationKind;
use crate::resolution::graph::ResolvedPackageClosure;

pub(crate) fn compare_review_only_root_role_graphs(
    baseline: &ResolvedPackageClosure,
    candidate: &ResolvedPackageClosure,
) -> Result<Option<ReviewOnlyRootRoleChange>, ReviewOnlyRootRoleComparisonError> {
    if baseline.root() != candidate.root() {
        return Err(ReviewOnlyRootRoleComparisonError::RootIdentityMismatch {
            baseline: Box::new(baseline.root().clone()),
            candidate: Box::new(candidate.root().clone()),
        });
    }
    let baseline_role = baseline.root_role();
    let candidate_role = candidate.root_role();
    let broken_contract = match (baseline_role, candidate_role) {
        (BuildDeclarationKind::Package, BuildDeclarationKind::Application) => {
            ReviewOnlyRootRoleContract::DependencyCompatibility
        }
        (BuildDeclarationKind::Application, BuildDeclarationKind::Package) => {
            ReviewOnlyRootRoleContract::ApplicationActivation
        }
        _ => return Ok(None),
    };
    Ok(Some(ReviewOnlyRootRoleChange {
        root: baseline.root().clone(),
        baseline_role,
        candidate_role,
        broken_contract,
    }))
}
