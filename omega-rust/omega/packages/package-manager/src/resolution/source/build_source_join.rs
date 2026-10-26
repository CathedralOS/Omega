//! Retain the exact acquired dependency roster through application execution.
//! Both executed discovery and prepared-graph consumers use this same join.

use crate::declarations::PackageKey;
use crate::declarations::dependencies::read::DependencyProjections;
use crate::resolution::graph::ResolvedPackageSourceClosure;
use package_compilation::{
    BuildDependencySelection, BuildDependencySourceJoin, PackageCompilationInputs,
    PackageCompilationSourceInputs,
};
use std::sync::Arc;

struct AcquiredBuildSourceJoin {
    source: Arc<PackageCompilationSourceInputs>,
    requests: DependencyProjections,
    selections: Vec<BuildDependencySelection>,
}

impl BuildDependencySourceJoin for AcquiredBuildSourceJoin {
    fn resolve_issued_requests(
        &self,
        inputs: &PackageCompilationInputs,
        requests: &[&build_declarations::BuildRequest],
    ) -> Result<Vec<BuildDependencySelection>, String> {
        if inputs.source_inputs().as_ref() != self.source.as_ref() {
            return Err("Build dependency join belongs to another acquired source graph".into());
        }
        let requests = DependencyProjections::from_executed_requests(requests.iter().copied())?;
        if requests != self.requests {
            return Err("Build source requests differ from the acquired source ledger".into());
        }
        Ok(self.selections.clone())
    }
}

pub(crate) fn prepare(
    closure: &ResolvedPackageSourceClosure,
    root: &PackageKey,
    inputs: &PackageCompilationInputs,
) -> Result<Arc<dyn BuildDependencySourceJoin>, String> {
    let custody = closure
        .custody(root)
        .ok_or("Build does not belong to this acquired source graph")?;
    if inputs.root() != root.identity() {
        return Err("Build source join has another compilation root".into());
    }
    let node = closure
        .graph()
        .package(root)
        .ok_or("acquired source has no graph node")?;
    let selections = node
        .dependencies()
        .iter()
        .map(|dependency| BuildDependencySelection {
            purpose: dependency.purpose(),
            request_ordinal: dependency.dependency_index(),
            package: dependency.target().identity(),
        })
        .collect();
    Ok(Arc::new(AcquiredBuildSourceJoin {
        source: inputs.source_inputs(),
        requests: custody.dependency_projections().clone(),
        selections,
    }))
}
