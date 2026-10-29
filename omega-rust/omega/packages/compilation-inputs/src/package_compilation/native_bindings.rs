//! Relative import purpose, not producer purpose, controls native handoff supply.
use super::PackageBuildHandoff;
impl super::PackageCompilationInputs {
    pub fn product_dependency_native_handoffs(&self) -> impl Iterator<Item = &PackageBuildHandoff> {
        self.dependency_build_handoff_instances()
            .filter(|(scope, _)| *scope == build_declarations::DependencyPurpose::Product)
            .map(|(_, handoff)| handoff)
    }
}
