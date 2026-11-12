/*
Saved source declaration identities and assumed-binding contracts for runtime
permission comparison (`PreparedRuntimePolicy`).

`PreparedRuntimePolicy::capture` snapshots nominal service and requirement
identities plus `PreparedAssumedContracts` from `CheckedProductSource` before
shared Terminal publication, holding only a `Weak<CheckedTrees>` pointer so the
checked AST is not kept alive. During `rows`, we verify that the weak owner
pointer and `CheckedSourceCustody` match `RuntimePermissionInput`, rejoin
accepted semantic bindings to their saved service schemas and requirement
identities, render assumed-binding contracts, and emit canonical
`PackagePolicyRow` items without reopening source.
*/

use super::SourceDeclarationInput;
use super::package_policy::assumed_bindings::PreparedAssumedContracts;
use super::semantics::declarations::{nominal_identity, provider_requirement_identity};
use super::terminal_authority_permissions::declarations::rejected;
use crate::record::{
    PackagePolicyTerminalPermission, PackageReviewNominalIdentity, PackageReviewNominalOwner,
};
use checked_trees::typed_trees::service_declarations::PreparedServiceCatalog;
use compilation_products::CheckedProductSource;
use diagnostics::Diagnostic;
use provider_planning::ProviderSchemaDeclaration;
use provider_planning::service_declarations::ServiceDeclarations;
use std::sync::{Arc, Weak};
use symbols::SymbolHandle;

struct ServiceIdentities {
    symbol: SymbolHandle,
    service: Result<PackageReviewNominalIdentity, Vec<Diagnostic>>,
    requirements: Vec<(
        SymbolHandle,
        Result<PackageReviewNominalIdentity, Vec<Diagnostic>>,
    )>,
}

pub struct PreparedRuntimePolicy {
    owner: Weak<checked_trees::CheckedTrees>,
    sources: Arc<package_compilation::CheckedSourceCustody>,
    catalog: Arc<PreparedServiceCatalog>,
    identities: Vec<ServiceIdentities>,
    contracts: PreparedAssumedContracts,
    evaluated_assumptions: usize,
}

impl PreparedRuntimePolicy {
    pub fn capture(source: &CheckedProductSource) -> Self {
        let mathematical_calls =
            super::contracts::mathematical_calls::MathematicalCallQueries::new(&*source.program);
        let input = SourceDeclarationInput {
            mathematical_calls: Some(&mathematical_calls),
            original_mathematical_calls: Some(&mathematical_calls),
            ..SourceDeclarationInput::checked(&source.program, &source.sources)
        };
        let catalog = source.provider_inventory.retained_service_catalog();
        let identities = catalog
            .declared_traits()
            .map(|declaration| {
                let symbol = declaration.schema.symbol();
                ServiceIdentities {
                    symbol,
                    service: nominal_identity(&input, symbol),
                    requirements: declaration
                        .surface
                        .as_ref()
                        .map(|surface| {
                            surface
                                .requirements
                                .iter()
                                .map(|requirement| {
                                    let requirement = requirement.source.symbol;
                                    (
                                        requirement,
                                        provider_requirement_identity(
                                            &input,
                                            ProviderSchemaDeclaration::BoundaryTrait(symbol),
                                            requirement,
                                        ),
                                    )
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                }
            })
            .collect();
        let requirements = source
            .computed_build_config
            .config
            .assumed_bindings
            .iter()
            .flat_map(|row| [row.required, row.supplied])
            .collect::<Vec<_>>();
        Self {
            evaluated_assumptions: source.computed_build_config.config.assumed_bindings.len(),
            owner: Arc::downgrade(&source.program),
            sources: Arc::clone(&source.sources),
            catalog,
            identities,
            contracts: PreparedAssumedContracts::capture(&input, &requirements),
        }
    }

    pub(crate) fn evaluated_assumptions(&self) -> usize {
        self.evaluated_assumptions
    }

    pub(crate) fn rows(
        &self,
        input: &crate::permission_acceptance::RuntimePermissionInput<'_>,
        target: target::TargetProfile,
        package: semantic_vocabulary::PackageKeyIdentity,
    ) -> Result<Vec<crate::record::PackagePolicyRow>, Vec<Diagnostic>> {
        if !Weak::ptr_eq(&self.owner, &input.providers.calling_program.source_owner)
            || self.sources.as_ref() != input.sources
        {
            return Err(rejected(
                "saved permission declarations belong to another checked program",
            ));
        }
        if input.sources.package_identity() != Some(package)
            || input.execution.selected_target_profile != Some(target)
            || input.execution.selected_native_target() != Some(target.native_target())
        {
            return Err(rejected(
                "policy package or target differs from the checked root activation",
            ));
        }
        let services = ServiceDeclarations::prepare(
            self.catalog.declared_traits(),
            &input.providers.boundary_calling_plan_realizations,
        );
        let mut permissions = Vec::new();
        for accepted in input.execution.accepted_semantic_bindings() {
            let resolved = input
                .execution
                .resolved_semantic_binding(accepted.role())
                .ok_or_else(|| rejected("accepted binding lost its exact checked declaration"))?;
            if resolved.accepted() != accepted {
                return Err(rejected(
                    "accepted binding differs from its retained policy",
                ));
            }
            let declaration = services
                .rejoin(resolved.declaration())
                .map_err(|error| vec![error])?;
            let schema = &declaration.declaration.schema;
            let digest =
                package_compilation::accepted_service_schema_digest(accepted.role(), schema);
            if digest != accepted.normalized_schema_digest() {
                return Err(rejected(
                    "accepted binding changed its normalized service schema",
                ));
            }
            let saved = self
                .identities
                .iter()
                .find(|saved| saved.symbol == declaration.symbol)
                .ok_or_else(|| {
                    rejected("accepted service lost its saved declaration identities")
                })?;
            let service = saved.service.as_ref().map_err(Clone::clone)?;
            let expected_owner = match accepted.role() {
                package_compilation::AcceptedSemanticBindingRole::ProcessExitExitProcessI32 => {
                    matches!(
                        service.owner(),
                        PackageReviewNominalOwner::ToolchainSource(_)
                    )
                }
                _ => service.owner() == PackageReviewNominalOwner::Package(accepted.package()),
            };
            if !expected_owner {
                return Err(rejected("accepted service changed its exact package owner"));
            }
            for supplied in accepted.terminal_authority_permissions() {
                if supplied.service_schema() != digest {
                    return Err(rejected(
                        "permission names a different accepted service schema",
                    ));
                }
                let matching = schema
                    .declaration
                    .methods
                    .iter()
                    .zip(declaration.requirements)
                    .filter(|(method, _)| {
                        accepted
                            .terminal_authority_permission_identities()
                            .text(supplied.requirement_identity())
                            == Some(schema.identity_text(method.requirement_identity))
                    })
                    .collect::<Vec<_>>();
                let [(_, symbol)] = matching.as_slice() else {
                    return Err(rejected(
                        "permission does not name one exact service requirement",
                    ));
                };
                let requirement = saved
                    .requirements
                    .iter()
                    .find(|(candidate, _)| candidate == *symbol)
                    .ok_or_else(|| rejected("permission lost its saved requirement identity"))?
                    .1
                    .as_ref()
                    .map_err(Clone::clone)?;
                permissions.push((
                    service.clone(),
                    PackagePolicyTerminalPermission {
                        requirement: requirement.clone(),
                        permitted: supplied.permitted().clone(),
                    },
                ));
            }
        }
        let assumed = self.contracts.render(
            input.execution.plans(),
            input.sources,
            input.execution.selected_target_profile,
        )?;
        let components = crate::record::PackagePolicyComponentAssumption::project(input.providers);
        crate::encoding::runtime_permission_rows(
            package,
            target,
            &permissions,
            &assumed,
            &components,
        )
        .map_err(|error| vec![Diagnostic::error(error.to_string())])
    }
}
