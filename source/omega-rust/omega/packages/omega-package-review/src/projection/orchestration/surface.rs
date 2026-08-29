use super::super::public_api::conformances::project_public_conformances;
use super::super::public_api::constants::project_public_consts;
use super::super::public_api::data::projection::project_public_data;
use super::super::public_api::domains::projection::project_public_domains;
use super::super::public_api::operators::project_public_operators;
use super::super::public_api::propositions::project_public_propositions;
use super::super::public_api::traits::project_public_traits;
use super::super::semantics::{project_representation_tcb, project_semantic_dependencies};
use crate::evidence::projection::{ProjectedReviewRow, ProjectedSemanticDependencyRow};
use crate::evidence::{
    PackageReviewConformanceShape, PackageReviewConstShape, PackageReviewDataShape,
    PackageReviewDomainShape, PackageReviewOperatorShape, PackageReviewPropositionShape,
    PackageReviewRepresentationTcb, PackageReviewTraitShape,
};
use omega_compiler::CheckedCompilation;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;

pub(super) struct ProjectedPackageSurface {
    pub(super) public_traits: Vec<ProjectedReviewRow<PackageReviewTraitShape>>,
    pub(super) public_conformances: Vec<ProjectedReviewRow<PackageReviewConformanceShape>>,
    pub(super) public_domains: Vec<ProjectedReviewRow<PackageReviewDomainShape>>,
    pub(super) public_propositions: Vec<ProjectedReviewRow<PackageReviewPropositionShape>>,
    pub(super) public_consts: Vec<ProjectedReviewRow<PackageReviewConstShape>>,
    pub(super) public_operators: Vec<ProjectedReviewRow<PackageReviewOperatorShape>>,
    pub(super) public_data: Vec<ProjectedReviewRow<PackageReviewDataShape>>,
    pub(super) representation_tcb: Vec<ProjectedReviewRow<PackageReviewRepresentationTcb>>,
    pub(super) semantic_dependencies: Vec<ProjectedSemanticDependencyRow>,
}

pub(super) fn project_package_surface(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<ProjectedPackageSurface, Vec<Diagnostic>> {
    Ok(ProjectedPackageSurface {
        public_traits: project_public_traits(compilation, package)?,
        public_conformances: project_public_conformances(compilation, package)?,
        public_domains: project_public_domains(compilation, package)?,
        public_propositions: project_public_propositions(compilation, package)?,
        public_consts: project_public_consts(compilation, package)?,
        public_operators: project_public_operators(compilation, package)?,
        public_data: project_public_data(compilation, package)?,
        representation_tcb: project_representation_tcb(compilation, package)?,
        semantic_dependencies: project_semantic_dependencies(compilation, package)?,
    })
}
