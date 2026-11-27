//! Form compiler inputs from the capture, Build return, and package graph of one opened Product.

use super::PlannedProduct;
use build_declarations::BuildDeclaration;
use compiler::{NativeApplicationSettings, Program, SourceNativeTarget, StageSource};
use declaration_scope::PackageDeclarationScope;
use diagnostics::Diagnostic;
use installation_conditions::InstallationEnvironmentAssumption;
use optimization_requests::{OptimizationRollback, OptimizationRollbackReceipt};
use rayon::iter::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator};
use retained_product_selections::RetainedProviderSelection;
use semantic_vocabulary::source_selections::ProductTarget;
use source_compiler::compile_product::source_targets::SourceTarget;
use source_compiler::compile_product::{
    NativeBindingInputs, PackageNativeBindings, ProductPolicyInputs, ProductSourceInputs,
};
use source_declarations::declaration_selection::{
    DescribedProductEntry, ProductEntryBinding, RootBindingSelection,
};
use std::borrow::Cow;
use std::sync::Arc;
use target::TargetProfile;

/// The receiving declarations and deployment choices for one completed project Build.
pub struct ProductStage<'a> {
    pub(super) product: &'a PlannedProduct,
    rollback: Option<OptimizationRollbackReceipt>,
    targets: Vec<SourceTarget>,
    entries: Cow<'a, [ProductEntryBinding]>,
    deployments: Vec<&'static str>,
    build_host: ProductTarget,
    scope: Option<Arc<PackageDeclarationScope>>,
    defaults: Vec<RetainedProviderSelection>,
    native: Vec<PackageNativeBindings<'a>>,
    proof_index_expansion_terms: u64,
}

impl PlannedProduct {
    pub fn product_stage(
        &self,
        deployments: &[TargetProfile],
    ) -> Result<ProductStage<'_>, Vec<Diagnostic>> {
        let packages = self.package_inputs();
        let (defaults, native) = match packages {
            Some(inputs) => (
                inputs.admitted_package_provider_selections(
                    build_declarations::DependencyPurpose::Product,
                )?,
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
        let entries = match self.source().default_product_entry.as_ref() {
            Some(main) => Cow::Owned(
                deployments
                    .iter()
                    .map(|profile| {
                        let contract = profile.program_entry_slot().source_contract();
                        let slot = format!("{}::{}", contract.owner, contract.slot_name);
                        ProductEntryBinding {
                            source_span: main.source().source_span(main.span()),
                            implementation: RootBindingSelection::Described(
                                DescribedProductEntry {
                                    declaration: main.clone(),
                                    machine_name: "main".to_owned(),
                                    slot: slot.clone(),
                                },
                            ),
                            slot,
                        }
                    })
                    .collect(),
            ),
            None => Cow::Borrowed(
                self.evaluated()
                    .configuration
                    .root_binding_requests
                    .as_slice(),
            ),
        };
        let mut bound_profiles = Vec::with_capacity(deployments.len());
        let mut include_profile = |profile: TargetProfile| {
            if !bound_profiles.contains(&profile) {
                bound_profiles.push(profile);
            }
        };
        for &profile in deployments {
            include_profile(profile);
        }
        for binding in entries.iter() {
            if let Some((owner, _)) = binding.slot.rsplit_once("::")
                && let Ok(profile) = TargetProfile::from_root_slot_owner(owner)
            {
                include_profile(profile);
            }
        }
        let configuration = &self.evaluated().configuration;
        for selection in &configuration.product_selection_requests {
            if selection.test_group.is_none()
                && let Some(target) = selection.target
                && let Ok(profile) =
                    TargetProfile::from_canonical_target_name(target.canonical_name())
            {
                include_profile(profile);
            }
        }
        for binding in &configuration.native_binding_requests {
            if let Ok(profile) =
                TargetProfile::from_canonical_target_name(binding.target.canonical_name())
            {
                include_profile(profile);
            }
        }
        Ok(ProductStage {
            product: self,
            entries,
            rollback: None,
            targets: package_compilation::source_imports::source_targets(
                packages,
                &bound_profiles,
            )?,
            deployments: deployments
                .iter()
                .map(|target| target.target_name())
                .collect(),
            build_host,
            scope: packages
                .map(|inputs| Arc::new(inputs.dependency_closure().declaration_permissions())),
            defaults,
            native,
            proof_index_expansion_terms: self
                .evaluated()
                .configuration
                .config
                .settings
                .proof_index_expansion_terms,
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

    pub fn produce_build_provider(
        self,
        profile: TargetProfile,
        declaration_path: &str,
    ) -> Result<source_compiler::publication::ProducedBuildProvider, Vec<Diagnostic>> {
        self.source()
            .produce_build_provider(profile.target_name(), declaration_path)
    }

    pub fn deployment_targets(&self) -> &[&'static str] {
        &self.deployments
    }

    pub fn check(
        &self,
        profile: TargetProfile,
        timings: &mut source_compiler::CompileTimings,
    ) -> Result<Vec<Diagnostic>, Vec<Diagnostic>> {
        let application = matches!(
            self.product.evaluated().configuration.project_declaration,
            Some(BuildDeclaration::Application(_))
        );
        if !application && !self.binds_entry(profile) {
            return compiler::check(self.source(), timings);
        }
        // "An explicit `--target` selection narrows realization only and
        // never narrows checking" (build/configuration.md, "Multi-target
        // compilation"). So every other target the Build binds an entry for
        // is checked too: its entry contract, its selected providers and its
        // demanded constants. A target with no address model (the Alpha
        // bootstrap) is checked only when requested, as its entry row is
        // (`TASKS.md`, **BUILD-OWNED-TARGET-SELECTION**).
        let mut checked = vec![profile];
        checked.extend(TargetProfile::ALL.into_iter().filter(|&other| {
            other != profile
                && self.binds_entry(other)
                && target::TargetSemantics::close_address_model(other).is_some()
        }));
        if checked.len() == 1 {
            return self.program(profile, Vec::new())?.check_target(timings);
        }
        let stage = ProductStage {
            rollback: self.rollback.clone(),
            ..self.product.product_stage(&checked)?
        };
        let names = checked
            .iter()
            .map(|target| target.target_name())
            .collect::<Vec<_>>();
        let mut advisories = Vec::new();
        let publications = source_compiler::compile_for_targets_with_timings(
            stage.source(),
            &names,
            Some(&mut advisories),
            timings,
        )?;
        // Each target's native preparation reads only its own publication and
        // the shared Product, so the targets prepare in parallel on the
        // invocation's workers. Results keep the checked order, so the first
        // failing target in that order is still the one reported.
        let enabled = timings.is_enabled();
        let prepared = timings.record_result(
            artifacts::compile_timings::CHECKED_TARGET_PREPARATION,
            || {
                Ok::<_, Vec<Diagnostic>>(worker_budget::within_budget(|| {
                    checked
                        .into_par_iter()
                        .zip(publications)
                        .map(|(target, publication)| {
                            let mut target_timings = if enabled {
                                source_compiler::CompileTimings::enabled()
                            } else {
                                source_compiler::CompileTimings::default()
                            };
                            let result = stage.check_publication(
                                target,
                                profile,
                                publication,
                                &mut target_timings,
                            );
                            (target_timings, result)
                        })
                        .collect::<Vec<_>>()
                }))
            },
        )?;
        for (target_timings, result) in prepared {
            timings.extend(&target_timings);
            advisories.extend(result?);
        }
        Ok(advisories)
    }

    /// Prepare `target`'s native inputs from its checked publication. A
    /// refusal on a target other than the requested `profile` names its Build row.
    fn check_publication(
        &self,
        target: TargetProfile,
        profile: TargetProfile,
        publication: source_compiler::PublishedTargetProgram,
        timings: &mut source_compiler::CompileTimings,
    ) -> Result<Vec<Diagnostic>, Vec<Diagnostic>> {
        let program = Program::CheckedSource {
            publication,
            suppliers: self.product,
            runtime_consent: self.runtime_consent(target)?,
            target: self.native_target(target, Vec::new())?,
        };
        program.check_target(timings).map_err(|mut diagnostics| {
            if target != profile {
                for diagnostic in &mut diagnostics {
                    diagnostic.message = format!(
                        "the `{}` Build row: {}",
                        target.target_name(),
                        diagnostic.message
                    );
                }
            }
            diagnostics
        })
    }

    /// Whether the Build binds a program entry for `profile`.
    fn binds_entry(&self, profile: TargetProfile) -> bool {
        let contract = profile.program_entry_slot().source_contract();
        self.entries
            .iter()
            .any(|entry| entry.slot.rsplit_once("::") == Some((contract.owner, contract.slot_name)))
    }

    pub fn source(&self) -> StageSource<'_> {
        let source = self.product.source();
        let build = &self.product.evaluated().configuration;
        StageSource {
            parsed: Box::new(source.parsed.clone()),
            product: ProductSourceInputs {
                reach_policy: {
                    let mut policy = self.product.package_inputs()
                        .map(|inputs| inputs.execution_reach_policy(build_declarations::DependencyPurpose::Product))
                        .unwrap_or_default();
                    policy.enabled_unsafe.insert(policy.root, build.enabled_unsafe_atoms.clone());
                    std::sync::Arc::new(policy)
                },
                declaration_scope: self.scope.clone(),
                build_host: self.build_host,
                build_accounting: Some(&self.product.evaluated().source_completion_accounting),
                targets: &self.targets,
                calling_targets: &self.deployments,
                subject: self.product.package_inputs().map(|inputs| inputs.root()),
                root_binding_requests: &self.entries,
                policy: ProductPolicyInputs {
                    activation_owner: source.build_owner(),
                    activation: build.selected_build_declaration(),
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
                optimizations: self
                    .rollback
                    .as_ref()
                    .map_or(&build.config.settings.optimizations, |receipt| {
                        receipt.effective()
                    })
                    .project_psi()
                    .selections()
                    .clone(),
                proof_index_expansion_terms: self.proof_index_expansion_terms,
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
            suppliers: self.product,
            runtime_consent: self.runtime_consent(profile)?,
            target: self.native_target(profile, installation_assumptions)?,
        })
    }

    pub(super) fn runtime_consent(
        &self,
        profile: TargetProfile,
    ) -> Result<
        Option<&review_evidence::permission_acceptance::TerminalPermissionAcceptance>,
        Vec<Diagnostic>,
    > {
        self.product
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
            .transpose()
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
        Ok(SourceNativeTarget {
            profile,
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
            isolated_execution_grant: None,
            package_permissions: self.scope.as_deref().cloned(),
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

impl compiler::ComponentSuppliers for PlannedProduct {
    fn publish(
        &self,
        package: semantic_vocabulary::PackageKeyIdentity,
        profile: TargetProfile,
    ) -> Result<compiler::ComponentSourcePublication, Vec<Diagnostic>> {
        let supplier = self.suppliers.get(&package).ok_or_else(|| {
            vec![Diagnostic::error(
                "independent provider is outside the receiving Product's captured package graph",
            )]
        })?;
        let mut stage = supplier.product_stage(&[profile])?;
        // Dependencies cannot increase the root Build's proof-work grant
        // (wiki/spec/build/configuration.md#proof-work-policy).
        let root_limit = self
            .evaluated()
            .configuration
            .config
            .settings
            .proof_index_expansion_terms;
        stage.proof_index_expansion_terms = stage.proof_index_expansion_terms.min(root_limit);
        let published =
            source_compiler::compile_for_target(stage.source(), profile.target_name(), None)?;
        if published.subject() != Some(package) {
            return Err(vec![Diagnostic::error(
                "independent supplier has a different package owner",
            )]);
        }
        let (artifact, providers, exclusions) = published.into_parts();
        let producer =
            psi_authorization::HostProducerTrust::accept("current dependency Product compilation");
        let authorization =
            psi_authorization::authorize_terminal_artifact(artifact, Some(&producer))
                .map_err(|reason| vec![Diagnostic::error(reason.to_string())])?;
        let choices = stage.native_target(profile, Vec::new())?;
        Ok(compiler::ComponentSourcePublication {
            package,
            authorization,
            providers,
            exclusions,
            optimizations: choices.settings.optimizations.clone(),
            x86_scalar_fma_provider: choices.settings.x86_scalar_fma_provider,
            package_permissions: choices.package_permissions,
            semantic_bindings: choices.semantic_bindings,
        })
    }
}
