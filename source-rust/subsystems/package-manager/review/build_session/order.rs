// Dependency-first scheduling of the packages a review session builds.

use crate::declarations::PackageKey;
use crate::resolution::graph::ResolvedPackageSourceClosure;
use std::collections::BTreeSet;

pub(crate) fn dependency_first_package_order(
    closure: &ResolvedPackageSourceClosure,
) -> Vec<PackageKey> {
    fn visit(
        closure: &ResolvedPackageSourceClosure,
        key: &PackageKey,
        visited: &mut BTreeSet<PackageKey>,
        ordered: &mut Vec<PackageKey>,
    ) {
        if !visited.insert(key.clone()) {
            return;
        }
        let mut dependencies = closure
            .graph()
            .package(key)
            .expect("validated closure contains every traversed package")
            .dependencies()
            .iter()
            .map(|dependency| dependency.target().clone())
            .collect::<Vec<_>>();
        dependencies.sort();
        for dependency in dependencies {
            visit(closure, &dependency, visited, ordered);
        }
        ordered.push(key.clone());
    }

    let mut visited = BTreeSet::new();
    let mut ordered = Vec::with_capacity(closure.custodies().len());
    visit(closure, closure.graph().root(), &mut visited, &mut ordered);
    ordered
}
