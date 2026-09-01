use super::super::providers::selection::selected_provider_row_source;
use super::super::source::finalization::{
    finalize_dangerous_authority_rows, finalize_dangerous_authority_slack_rows,
    finalize_projected_rows, finalize_semantic_dependency_rows,
};
use super::super::source::locations::validate_canonical_row_source_limits;
use super::callables::ProjectedPackageCallables;
use super::providers::ProjectedProviders;
use super::surface::ProjectedPackageSurface;
use crate::capture::source::{
    ProjectedDangerousAuthorityRow, ProjectedDangerousAuthoritySlackRow, ProjectedReviewRow,
};
use crate::record::package::PackageReviewCanonicalRowSources;
use crate::record::{
    CheckedPackageReviewProjection, PackageReviewSourceLocationRole,
    PackageReviewSyntheticSourceKind, PackageReviewTerminalAuthorityPermission,
};
use omega_compiler::CheckedCompilation;
use omega_target::TargetProfile;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;

pub(super) struct PendingPackageReview {
    pub(super) package: PackageKeyIdentity,
    pub(super) target: TargetProfile,
    pub(super) surface: ProjectedPackageSurface,
    pub(super) callables: ProjectedPackageCallables,
    pub(super) dangerous_authorities: Vec<ProjectedDangerousAuthorityRow>,
    pub(super) dangerous_authority_slack: Vec<ProjectedDangerousAuthoritySlackRow>,
    pub(super) terminal_authority_permissions:
        Vec<ProjectedReviewRow<PackageReviewTerminalAuthorityPermission>>,
    pub(super) providers: ProjectedProviders,
}

impl PendingPackageReview {
    pub(super) fn finalize(
        self,
        compilation: &CheckedCompilation,
    ) -> Result<CheckedPackageReviewProjection, Vec<Diagnostic>> {
        let (public_traits, public_trait_sources) = finalize_projected_rows(
            compilation,
            self.surface.public_traits,
            PackageReviewSourceLocationRole::Declaration,
        )?;
        let (public_conformances, public_conformance_sources) = finalize_projected_rows(
            compilation,
            self.surface.public_conformances,
            PackageReviewSourceLocationRole::Declaration,
        )?;
        let (public_domains, public_domain_sources) = finalize_projected_rows(
            compilation,
            self.surface.public_domains,
            PackageReviewSourceLocationRole::Declaration,
        )?;
        let (public_propositions, public_proposition_sources) = finalize_projected_rows(
            compilation,
            self.surface.public_propositions,
            PackageReviewSourceLocationRole::Declaration,
        )?;
        let (public_consts, public_const_sources) = finalize_projected_rows(
            compilation,
            self.surface.public_consts,
            PackageReviewSourceLocationRole::Declaration,
        )?;
        let (public_operators, public_operator_sources) = finalize_projected_rows(
            compilation,
            self.surface.public_operators,
            PackageReviewSourceLocationRole::Declaration,
        )?;
        let (public_data, public_data_sources) = finalize_projected_rows(
            compilation,
            self.surface.public_data,
            PackageReviewSourceLocationRole::Declaration,
        )?;
        let (representation_tcb, representation_tcb_sources) = finalize_projected_rows(
            compilation,
            self.surface.representation_tcb,
            PackageReviewSourceLocationRole::Declaration,
        )?;
        let (semantic_dependencies, semantic_dependency_sources) =
            finalize_semantic_dependency_rows(compilation, self.surface.semantic_dependencies)?;
        let (callables, callable_sources) = finalize_projected_rows(
            compilation,
            self.callables.callables,
            PackageReviewSourceLocationRole::Declaration,
        )?;
        let (contract_entailment_open_obligations, contract_entailment_open_obligation_sources) =
            finalize_projected_rows(
                compilation,
                self.callables.contract_entailment_open_obligations,
                PackageReviewSourceLocationRole::Declaration,
            )?;
        let (external_executable_supply, external_executable_supply_sources) =
            finalize_projected_rows(
                compilation,
                self.callables.external_executable_supply,
                PackageReviewSourceLocationRole::Declaration,
            )?;
        let (dangerous_authorities, dangerous_authority_sources) =
            finalize_dangerous_authority_rows(compilation, self.dangerous_authorities)?;
        let (dangerous_authority_slack, dangerous_authority_slack_sources) =
            finalize_dangerous_authority_slack_rows(compilation, self.dangerous_authority_slack)?;
        let (terminal_authority_permissions, mut terminal_authority_permission_sources) =
            finalize_projected_rows(
                compilation,
                self.terminal_authority_permissions,
                PackageReviewSourceLocationRole::AuthorityDeclaration,
            )?;
        for source in &mut terminal_authority_permission_sources {
            source
                .compiler_derivations
                .push(PackageReviewSyntheticSourceKind::ConsumerTerminalAuthorityPermission);
        }
        let row_sources = PackageReviewCanonicalRowSources {
            public_traits: public_trait_sources,
            public_conformances: public_conformance_sources,
            public_domains: public_domain_sources,
            public_propositions: public_proposition_sources,
            public_consts: public_const_sources,
            public_operators: public_operator_sources,
            public_data: public_data_sources,
            representation_tcb: representation_tcb_sources,
            semantic_dependencies: semantic_dependency_sources,
            callables: callable_sources,
            contract_entailment_open_obligations: contract_entailment_open_obligation_sources,
            external_executable_supply: external_executable_supply_sources,
            dangerous_authorities: dangerous_authority_sources,
            dangerous_authority_slack: dangerous_authority_slack_sources,
            terminal_authority_permissions: terminal_authority_permission_sources,
            boundary_application_realizations: self.providers.application_realizations.sources,
            selected_provider_set: selected_provider_row_source(
                compilation,
                &self.providers.selected,
            )?,
        };
        validate_canonical_row_source_limits(&row_sources)?;

        Ok(CheckedPackageReviewProjection {
            package: self.package,
            target: self.target,
            public_traits,
            public_conformances,
            public_domains,
            public_propositions,
            public_consts,
            public_operators,
            public_data,
            representation_tcb,
            semantic_dependencies,
            callables,
            contract_entailment_open_obligations,
            external_executable_supply,
            dangerous_authorities,
            dangerous_authority_slack,
            terminal_authority_permissions,
            selected_providers: self.providers.selected,
            selected_provider_families: self.providers.families,
            boundary_application_realizations: self.providers.application_realizations.rows,
            row_sources,
        })
    }
}
