//! Form compiler inputs from the capture, Build return, and package graph of one opened Product.

use super::PlannedProduct;
use build_declarations::BuildDeclaration;
use compiler::{NativeApplicationSettings, Program, SourceNativeTarget, StageSource};
use diagnostics::Diagnostic;
use installation_conditions::InstallationEnvironmentAssumption;
use optimization_requests::{OptimizationRollback, OptimizationRollbackReceipt};
use package_compilation::PackageDependencyClosure;
use semantic_vocabulary::source_selections::ProductTarget;
use source_compiler::evaluation::BuildTimeSelectionAuthority;
use source_compiler::evaluation::provider_declarations::RetainedProviderSelection;
use source_compiler::product_source::{
    NativeBindingInputs, PackageNativeBindings, ProductPolicyInputs, ProductSourceInputs,
};
use source_compiler::source_targets::SourceTarget;
use std::sync::Arc;
use target::TargetProfile;

/// The receiving declarations and deployment choices for one completed project Build.
pub struct ProductStage<'a> {
    pub(super) product: &'a PlannedProduct,
    rollback: Option<OptimizationRollbackReceipt>,
    targets: Vec<SourceTarget>,
    deployments: Vec<&'static str>,
    build_host: ProductTarget,
    scope: Option<Arc<PackageDependencyClosure>>,
    defaults: Vec<RetainedProviderSelection>,
    native: Vec<PackageNativeBindings<'a>>,
}

impl PlannedProduct {
    pub fn product_stage(
        &self,
        deployments: &[TargetProfile],
    ) -> Result<ProductStage<'_>, Vec<Diagnostic>> {
        let packages = self.package_inputs();
        let (defaults, native) = match packages {
            Some(inputs) => (
                inputs.admitted_package_provider_selections()?,
                inputs.product_native_bindings()?,
            ),
            None => (Vec::new(), Vec::new()),
        };
        let profile = self.build_profile().target_name();
        let build_host = ProductTarget::from_canonical_name(profile).ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "the Build host profile `{profile}` has no source target identity"
            ))]
        })?;
        Ok(ProductStage {
            product: self,
            rollback: None,
            targets: package_compilation::source_imports::source_targets(packages),
            deployments: deployments
                .iter()
                .map(|target| target.target_name())
                .collect(),
            build_host,
            scope: packages.map(|inputs| Arc::new(inputs.dependency_closure())),
            defaults,
            native,
        })
    }
}

impl ProductStage<'_> {
    pub fn apply_rollback(
        &mut self,
        request: &OptimizationRollback,
    ) -> Option<OptimizationRollbackReceipt> {
        self.rollback = request.reconcile(
            &self
                .product
                .evaluated()
                .configuration
                .config
                .settings
                .optimizations,
        );
        self.rollback.clone()
    }

    pub fn artifact_only(&self) -> bool {
        matches!(&self.product.evaluated().configuration.project_declaration,
            Some(BuildDeclaration::Application(application)) if application.artifact_only)
    }

    pub fn source(&self) -> StageSource<'_> {
        let source = self.product.source();
        let build = &self.product.evaluated().configuration;
        StageSource {
            files: Arc::clone(&source.sources),
            roots: source.root_sources.clone(),
            imports: source.source_scoped_top_level_bindings.clone(),
            declaration_authority: self.scope.as_deref().map(|scope| scope as &_),
            build_host: self.build_host,
            build_accounting: Some(&self.product.evaluated().source_completion_accounting),
            selections: ProductSourceInputs {
                targets: &self.targets,
                calling_targets: &self.deployments,
                subject: self.scope.as_ref().map(|scope| scope.root()),
                root_binding_requests: &build.root_binding_requests,
                policy: ProductPolicyInputs {
                    activation: build.selected_build_declaration.as_ref(),
                    declared: &build.declared_product_selections,
                    executed: &build.product_selection_requests,
                    native_bindings: &build.native_binding_requests,
                    exclusions: &build.behavior_exclusion_requests,
                },
                default_provider_selections: &self.defaults,
                native_bindings: NativeBindingInputs {
                    evaluation: build.config.native_binding_evaluation.as_ref(),
                    packages: &self.native,
                },
                layout_selection_authority: self
                    .scope
                    .as_ref()
                    .map(|scope| Arc::clone(scope) as Arc<dyn BuildTimeSelectionAuthority>),
                optimizations: self
                    .rollback
                    .as_ref()
                    .map_or(&build.config.settings.optimizations, |receipt| {
                        receipt.effective()
                    })
                    .project_psi()
                    .selections()
                    .clone(),
            },
        }
    }

    /// The chosen source deployment receives its own settings and reviewed installation conditions.
    pub fn program(
        &self,
        profile: TargetProfile,
        installation_assumptions: Vec<InstallationEnvironmentAssumption>,
    ) -> Result<Program<'_>, Vec<Diagnostic>> {
        Ok(Program::Source {
            source: self.source(),
            target: self.native_target(profile, installation_assumptions)?,
        })
    }

    pub(super) fn native_target(
        &self,
        profile: TargetProfile,
        installation_assumptions: Vec<InstallationEnvironmentAssumption>,
    ) -> Result<SourceNativeTarget<'_>, Vec<Diagnostic>> {
        let build = &self.product.evaluated().configuration;
        let settings = &build.config.settings;
        let name = match build.project_declaration.as_ref() {
            Some(BuildDeclaration::Application(application)) => {
                Some(application.name.as_str().to_owned())
            }
            _ => None,
        };
        let runtime_consent = self
            .product
            .runtime_consents
            .as_ref()
            .map(|consents| {
                consents
                    .iter()
                    .find(|consent| consent.target == profile)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(format!(
                            "the locked Product has no accepted runtime policy for {}",
                            profile.target_name()
                        ))]
                    })
            })
            .transpose()?;
        Ok(SourceNativeTarget {
            profile,
            runtime_consent,
            settings: NativeApplicationSettings {
                application: target::NativeApplicationMetadata {
                    name,
                    intent: settings.application_intent,
                    identifier: settings.application_identifier.clone(),
                    subsystem: settings.subsystem,
                    elf_interpreter: settings.elf_interpreters.effective(profile),
                },
                optimizations: self
                    .rollback
                    .as_ref()
                    .map_or(&settings.optimizations, |receipt| receipt.effective()),
                x86_scalar_fma_provider: settings
                    .x86_deployment_claim
                    .admit_for(Some(profile))
                    .map_err(|reason| vec![Diagnostic::error(reason)])?,
            },
            package_permissions: self
                .scope
                .as_ref()
                .map(|scope| scope.declaration_permissions()),
            semantic_bindings: self
                .product
                .package_inputs()
                .into_iter()
                .flat_map(|inputs| inputs.accepted_semantic_bindings().cloned())
                .collect(),
            installation_assumptions,
        })
    }
}
