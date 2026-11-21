//! Project the accepted boundary declarations and checked provider selections.
//! A package may disclose its public requirement without installing a
//! realization.

use super::callables::reviewed_services;
use super::declarations::unavailable;
use super::input::PolicySource;
use super::provider_rows::ProviderChoice;
use diagnostics::Diagnostic;
use provider_plans::{
    ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod, ServiceSchema,
};
use review_evidence::record::{
    PackagePolicyCallable, PackagePolicyCallableRole, PackagePolicyCrash,
    PackagePolicyInferredCrash, PackagePolicyMutation, PackagePolicyProviderBinding,
    PackagePolicyProviderPlan, PackagePolicyProviderRow, PackagePolicyServiceAuthority,
    PackagePolicyServiceMethod, PackagePolicyServiceReachDependency, PackagePolicyServiceSignature,
    PackagePolicyTargetContext, PackagePolicyTermination, PackagePolicyTraitShape,
    PackageReviewCallableSupply, PackageReviewCheckedServiceReach, PackageReviewCrashInterface,
    PackageReviewInstallationReach, PackageReviewNominalIdentity, PackageReviewNominalOwner,
    PackageReviewSelectedInstallationReach, PackageReviewSignatureParameter,
    PackageReviewSynchronousInvocation, PackageReviewWriteFrameCompleteness,
};
use semantic_vocabulary::{BoundaryMachineId, MachineId, PackageKeyIdentity, ServiceId};
use source_compiler::compile_product::provider_declarations::ProviderSelectionSubject;
use source_compiler::representations::source_state::signature::StateSignature;
use source_compiler::representations::source_state::trait_definition::TraitDefinition;
use symbols::SymbolHandle;
use target::TargetProfile;
use terminal_psi::{
    BoundaryDeclarationRole, BoundaryMachineDeclaration, CheckedBoundaryExecution, TerminalMachine,
    UnoptimizedMachineBehavior,
};

pub(super) struct ReviewedProviders {
    pub slots: Vec<SymbolHandle>,
    pub callables: Vec<PackagePolicyCallable>,
    pub plans: Vec<PackagePolicyProviderPlan>,
    pub public_traits: Vec<PackagePolicyTraitShape>,
    pub applications: Vec<review_evidence::record::PackagePolicyBoundaryApplicationRealization>,
    pub semantic_dependencies: Vec<review_evidence::record::PackagePolicySemanticDependency>,
    boundaries: Vec<BoundaryMachineId>,
    services: Vec<ServiceId>,
}

impl ReviewedProviders {
    /// A package may publish a foreign service ceiling without installing a
    /// provider for it. Account for that declaration only when the completed
    /// Product findings name its one exact checked application.
    pub(super) fn validate_service_inventory(
        &self,
        declarations: &AcceptedDeclarations<'_>,
        callables: &[PackagePolicyCallable],
        traits: &[PackagePolicyTraitShape],
    ) -> Result<(), Vec<Diagnostic>> {
        use std::collections::{BTreeMap, BTreeSet};
        let signatures = traits
            .iter()
            .chain(&self.public_traits)
            .flat_map(|declaration| &declaration.requirements);
        let mut referenced: BTreeSet<_> = callables
            .iter()
            .chain(&self.callables)
            .filter(|callable| callable.role != PackagePolicyCallableRole::Build)
            .flat_map(super::callables::referenced_services)
            .collect();
        for signature in signatures {
            referenced.extend(signature.service_reach.iter().flatten());
            referenced.extend(
                signature
                    .synchronous_invocations
                    .iter()
                    .filter_map(PackageReviewSynchronousInvocation::service),
            );
        }
        let mut applications = BTreeMap::new();
        for (id, identity) in declarations.service_names {
            if referenced.contains(identity) {
                applications
                    .entry(identity)
                    .and_modify(|id| *id = None)
                    .or_insert(Some(*id));
            }
        }
        let cited: BTreeSet<_> = applications.into_values().flatten().collect();
        for &(service, owner) in declarations.service_origins {
            if self.services.binary_search(&service).is_err()
                && self
                    .slots
                    .binary_search_by_key(&symbol_key(owner), |slot| symbol_key(*slot))
                    .is_err()
                && !cited.contains(&service)
            {
                return Err(unavailable(format!(
                    "the checked service inventory for {}",
                    declarations
                        .accepted
                        .source()
                        .symbols
                        .display_path(owner, "::")
                )));
            }
        }
        Ok(())
    }
}

pub(super) fn symbol_key(symbol: SymbolHandle) -> (u32, u32) {
    (symbol.arena_index(), symbol.generation())
}

/// A pair from the complete publication roster; this includes default supplies
/// that the checked target selected from a dependency's Build.
struct AuthoredProvider {
    subject: SymbolHandle,
    provider: SymbolHandle,
    subject_owner: Option<PackageKeyIdentity>,
    provider_owner: Option<PackageKeyIdentity>,
    subject_path: String,
    provider_identity: String,
}

impl AuthoredProvider {
    fn key(
        &self,
    ) -> (
        Option<PackageKeyIdentity>,
        &str,
        Option<PackageKeyIdentity>,
        &str,
    ) {
        (
            self.provider_owner,
            &self.provider_identity,
            self.subject_owner,
            &self.subject_path,
        )
    }
}

pub(super) struct AcceptedDeclarations<'module> {
    pub(super) accepted: &'module source_compiler::publication::AcceptedTerminalSource,
    boundaries: Vec<&'module BoundaryMachineDeclaration>,
    bodies: Vec<&'module TerminalMachine>,
    behaviors: Vec<&'module UnoptimizedMachineBehavior>,
    authored_reach_owners: Vec<SymbolHandle>,
    service_origins: &'module [(ServiceId, SymbolHandle)],
    service_names: &'module [(ServiceId, PackageReviewNominalIdentity)],
}

impl<'module> AcceptedDeclarations<'module> {
    pub(super) fn new(
        accepted: &'module source_compiler::publication::AcceptedTerminalSource,
        service_origins: &'module [(ServiceId, SymbolHandle)],
        service_names: &'module [(ServiceId, PackageReviewNominalIdentity)],
        has_providers: bool,
    ) -> Self {
        let module = accepted.module();
        let mut boundaries = module
            .interfaces
            .boundary_machines
            .iter()
            .collect::<Vec<_>>();
        boundaries.sort_unstable_by_key(|boundary| boundary.id);
        let (bodies, behaviors) = if !has_providers {
            (Vec::new(), Vec::new())
        } else {
            let mut bodies = module.machines.iter().collect::<Vec<_>>();
            bodies.sort_unstable_by_key(|body| body.id);
            let mut behaviors = module
                .execution
                .unoptimized_behavior
                .iter()
                .collect::<Vec<_>>();
            behaviors.sort_unstable_by_key(|body| body.machine);
            (bodies, behaviors)
        };
        let mut authored_reach_owners = Vec::new();
        if !boundaries.is_empty() {
            authored_reach_owners.extend(
                accepted
                    .source()
                    .authored_service_reach_rows
                    .iter()
                    .map(|row| row.owner),
            );
        }
        authored_reach_owners.sort_unstable_by_key(|symbol| symbol_key(*symbol));
        authored_reach_owners.dedup();
        Self {
            accepted,
            boundaries,
            bodies,
            behaviors,
            authored_reach_owners,
            service_origins,
            service_names,
        }
    }

    fn has_authored_reach(&self, owner: SymbolHandle) -> bool {
        self.authored_reach_owners
            .binary_search_by_key(&symbol_key(owner), |symbol| symbol_key(*symbol))
            .is_ok()
    }

    fn source_service(&self, service: ServiceId) -> Result<&TraitDefinition, Vec<Diagnostic>> {
        let index = self
            .service_origins
            .binary_search_by_key(&service, |(id, _)| *id)
            .map_err(|_| unavailable("the selected service's exact source"))?;
        self.accepted
            .source()
            .trait_definition_by_symbol(self.service_origins[index].1)
            .ok_or_else(|| unavailable("the selected service's authored name"))
    }

    fn source_service_names<'source, Name: Ord>(
        &'source self,
        services: &[ServiceId],
        name: impl Fn(&'source TraitDefinition) -> Name,
    ) -> Result<Vec<Name>, Vec<Diagnostic>> {
        let mut names = services
            .iter()
            .map(|service| self.source_service(*service).map(&name))
            .collect::<Result<Vec<_>, _>>()?;
        names.sort_unstable();
        Ok(names)
    }
}

struct PreparedProviders {
    review: ReviewedProviders,
    choices: Vec<PreparedProvider>,
    numeric: Option<super::numeric::PreparedNumeric>,
    service_names: Vec<(ServiceId, PackageReviewNominalIdentity)>,
    service_ids: Vec<(String, ServiceId)>,
}

struct PreparedProvider {
    schema: PackageReviewNominalIdentity,
    provider: PackageReviewNominalIdentity,
    methods: Vec<PackagePolicyServiceMethod>,
    rows: Vec<PreparedRow>,
}

struct PreparedRow {
    canonical: String,
    requirement: PackageReviewNominalIdentity,
    realization: PackageReviewNominalIdentity,
    installation_bound: bool,
    upper_bound: Vec<ServiceId>,
}

impl PreparedProviders {
    fn finish(
        self,
        checked: &compiler::SelectedProviderResult,
        target: TargetProfile,
        authored_grants: &[symbols::root_grants::AuthoredRootGrant],
    ) -> Result<ReviewedProviders, Vec<Diagnostic>> {
        let selected = checked.plans();
        let plans = selected.plans();
        if plans.len() != self.choices.len() {
            return Err(unavailable("the complete prepared provider roster"));
        }
        let mut review = self.review;
        let grants = super::provider_grants::by_plan(checked, authored_grants)?;
        // Both rosters retain the order of the same checked target's plans.
        for ((plan, captured), grants) in plans.iter().zip(self.choices).zip(grants) {
            let mut rows = Vec::with_capacity(captured.rows.len());
            let mut declarations: Vec<_> = captured.rows.iter().collect();
            declarations.sort_unstable_by_key(|row| &row.canonical);
            for row in &plan.rows {
                let canonical = plan.identity_text(row.declared_requirement_identity);
                let i = declarations
                    .binary_search_by(|r| r.canonical.as_str().cmp(canonical))
                    .map_err(|_| unavailable("one selected row for the exact service method"))?;
                rows.push(declarations[i].finish(
                    selected,
                    plan,
                    row,
                    &self.service_ids,
                    &self.service_names,
                )?);
            }
            rows.sort_by(|a, b| a.requirement.cmp(&b.requirement));
            review.plans.push(PackagePolicyProviderPlan {
                plan_name: plan.name.clone(),
                realizing_package: plan.origin_package_identity,
                schema_declaration: captured.schema,
                provider_type: captured.provider.path().to_owned(),
                provider_type_declaration: Some(captured.provider),
                target: plan.target.clone(),
                methods: captured.methods,
                rows,
                grants,
            });
        }
        let mut applications = Vec::new();
        if let Some(numeric) = &self.numeric {
            for projected in numeric.finish(checked, target)? {
                review.semantic_dependencies.push(projected.dependency);
                if let Some((plan, application)) = projected.realization {
                    review.plans.push(plan);
                    applications.push(application);
                }
            }
        }
        review.semantic_dependencies.sort_unstable();
        review.semantic_dependencies.dedup();
        review
            .plans
            .sort_by(PackagePolicyProviderPlan::compare_canonical);
        review.applications = applications
            .into_iter()
            .map(|application| application.finish(&review.plans))
            .collect::<Result<_, _>>()?;
        review
            .applications
            .sort_by(|left, right| left.application_key().cmp(&right.application_key()));
        Ok(review)
    }
}

impl PreparedRow {
    fn finish(
        &self,
        selected: &selected_provider_plans::SelectedProviderPlanFacts,
        plan: &ProviderPlan,
        row: &ProviderPlanRow,
        service_ids: &[(String, ServiceId)],
        services: &[(ServiceId, PackageReviewNominalIdentity)],
    ) -> Result<PackagePolicyProviderRow, Vec<Diagnostic>> {
        let ProviderBinding::CheckedAdapter {
            machine_identity,
            machine_package_identity,
        } = row.binding
        else {
            return Err(unavailable("the selected provider's checked realization"));
        };
        let installation_reach =
            self.installation_reach(selected, plan, row, service_ids, services)?;
        if row.calling_signature.is_some() {
            return Err(unavailable(
                "the selected provider's complete effect or calling policy",
            ));
        }
        Ok(PackagePolicyProviderRow {
            method: row.method.clone(),
            requirement: self.requirement.clone(),
            requirement_application: None,
            realization: Some(self.realization.clone()),
            native_build: None,
            requirement_lifetime_partition: row.requirement_lifetime_partition.clone(),
            admitted_same_stack_bytes: row.admitted_same_stack_bytes,
            binding: PackagePolicyProviderBinding::CheckedAdapter {
                machine_identity: plan.identity_text(machine_identity).to_owned(),
                machine_package_identity,
            },
            compiler_intrinsic_execution: None,
            installation_reach,
        })
    }
    fn installation_reach(
        &self,
        selected: &selected_provider_plans::SelectedProviderPlanFacts,
        plan: &ProviderPlan,
        row: &ProviderPlanRow,
        service_ids: &[(String, ServiceId)],
        services: &[(ServiceId, PackageReviewNominalIdentity)],
    ) -> Result<Option<PackageReviewSelectedInstallationReach>, Vec<Diagnostic>> {
        let matches = selected
            .installation_reach_resolutions()
            .iter()
            .filter(|reach| {
                reach.provider_plan_report_identity == plan.report_fingerprint()
                    && selected.identity_text(reach.requirement_identity)
                        == plan.identity_text(row.requirement_identity)
            })
            .collect::<Vec<_>>();
        let reach = match (self.installation_bound, matches.as_slice()) {
            (false, []) => return Ok(None),
            (true, [reach]) => *reach,
            _ => return Err(unavailable("the exact selected installation reach row")),
        };
        let project = |names: &[String]| -> Result<_, Vec<Diagnostic>> {
            let mut ids = Vec::with_capacity(names.len());
            for name in names {
                let first = service_ids.partition_point(|(canonical, _)| canonical < name);
                let last = service_ids.partition_point(|(canonical, _)| canonical <= name);
                let [(_, id)] = &service_ids[first..last] else {
                    return Err(unavailable(
                        "the selected reach's exact service declaration",
                    ));
                };
                ids.push(*id);
            }
            reviewed_services(services, &ids)
        };
        let upper_bound = project(&reach.upper_bound)?;
        if upper_bound != reviewed_services(services, &self.upper_bound)? {
            return Err(unavailable("the selected reach's published upper bound"));
        }
        Ok(Some(PackageReviewSelectedInstallationReach {
            upper_bound,
            resolved: project(&reach.resolved_row)?,
        }))
    }
}

impl PolicySource<'_> {
    pub(super) fn providers(
        &self,
        checked: &compiler::SelectedProviderResult,
        package: PackageKeyIdentity,
        target: TargetProfile,
        declarations: &AcceptedDeclarations<'_>,
        native_bindings: &[(PackagePolicyServiceMethod, PackagePolicyCallable)],
        choices: Vec<ProviderChoice>,
    ) -> Result<ReviewedProviders, Vec<Diagnostic>> {
        let service_origins = declarations.service_origins;
        let accepted = declarations.accepted;
        let module = accepted.module();
        let mut review = ReviewedProviders {
            slots: Vec::new(),
            callables: Vec::new(),
            plans: Vec::new(),
            public_traits: Vec::new(),
            applications: Vec::new(),
            semantic_dependencies: Vec::new(),
            boundaries: Vec::new(),
            services: Vec::new(),
        };
        if module.interfaces.services.is_empty()
            && module.interfaces.boundary_machines.is_empty()
            && module.interfaces.provider_candidates.is_empty()
            && choices.is_empty()
            && self.declarations.explicit_providers().is_empty()
        {
            self.project_declared_services(package, &mut review)?;
            return Ok(review);
        }
        super::callables::validate_body_effect_inventory(module)?;
        if accepted.closure().elaboration().boundaries.len() != declarations.boundaries.len() {
            return Err(unavailable(
                "the accepted module's complete boundary inventory",
            ));
        }
        let (authored, mut supplied_slots) = self.authored_providers(target)?;
        let mut providers = Vec::with_capacity(choices.len());
        let mut provider_bodies = Vec::new();
        for selected in choices {
            if selected.target != target.target_name() {
                return Err(unavailable(
                    "the selected provider's exact package and deployment",
                ));
            }
            let key = selected.key();
            let first = authored.partition_point(|choice| choice.key() < key);
            let last = authored.partition_point(|choice| choice.key() <= key);
            let [choice] = &authored[first..last] else {
                return Err(unavailable(
                    "the plan's exact authored provider and service selection",
                ));
            };
            provider_bodies.extend(
                selected
                    .rows
                    .iter()
                    .flat_map(|row| row.candidates.iter().map(|&(_, body)| body)),
            );
            providers.push(self.prepare_provider(declarations, selected, choice, &mut review)?);
            review.slots.push(choice.subject);
        }
        review.boundaries.sort_unstable();
        review
            .slots
            .sort_unstable_by_key(|symbol| symbol_key(*symbol));
        supplied_slots.sort_unstable_by_key(|symbol| symbol_key(*symbol));
        supplied_slots.dedup();
        // A declared service can be reviewed without an installation. It
        // cannot account for a provider choice made by the actual Build.
        if supplied_slots.iter().any(|slot| {
            review
                .slots
                .binary_search_by_key(&symbol_key(*slot), |symbol| symbol_key(*symbol))
                .is_err()
        }) {
            return Err(unavailable("the exact Build provider selection custody"));
        }
        // Canonical numeric declarations obtain execution from the target,
        // independently of the Build's authored provider selections.
        let numeric = super::numeric::prepare(self, target, provider_bodies)?;
        review.boundaries.extend(numeric.boundaries());
        review.boundaries.sort_unstable();
        self.project_remaining_requirements(
            declarations,
            service_origins,
            native_bindings,
            &mut review,
        )?;
        self.project_declared_services(package, &mut review)?;
        review.boundaries.sort_unstable();
        // Source publication has already judged each parent application. The
        // public trait carries its authored parents, and normalized callable
        // ceilings name the concrete inherited authorities.
        review.services.extend(
            module
                .interfaces
                .services
                .iter()
                .flat_map(|service| service.parents.iter().copied()),
        );
        review.services.sort_unstable();
        review.services.dedup();
        // Unrelated declarations and unreviewed authority still refuse. A
        // Product can keep alternate checked bodies for other deployments;
        // their exact Build selections travel in the handoff separately.
        if !review
            .boundaries
            .iter()
            .copied()
            .eq(declarations.boundaries.iter().map(|boundary| boundary.id))
            || review.boundaries.windows(2).any(|pair| pair[0] == pair[1])
        {
            return Err(unavailable(
                "the accepted module's complete boundary inventory",
            ));
        }
        let mut service_ids = Vec::new();
        if providers
            .iter()
            .any(|provider| provider.rows.iter().any(|row| row.installation_bound))
        {
            service_ids.extend(module.interfaces.services.iter().filter_map(|service| {
                module
                    .identities
                    .text(service.identity)
                    .map(|name| (name.to_owned(), service.id))
            }));
            service_ids.sort_unstable();
        }
        PreparedProviders {
            service_ids,
            review,
            choices: providers,
            numeric: Some(numeric),
            service_names: declarations.service_names.to_vec(),
        }
        .finish(checked, target, self.declarations.root_grants())
    }

    fn project_remaining_requirements(
        &self,
        declarations: &AcceptedDeclarations<'_>,
        service_origins: &[(ServiceId, SymbolHandle)],
        native_bindings: &[(PackagePolicyServiceMethod, PackagePolicyCallable)],
        review: &mut ReviewedProviders,
    ) -> Result<(), Vec<Diagnostic>> {
        let typed = self.typed();
        let originals = &declarations.accepted.closure().elaboration().boundaries;
        let mut reviewed = Vec::new();
        for &boundary in &declarations.boundaries {
            if review.boundaries.binary_search(&boundary.id).is_ok() {
                continue;
            }
            let Some(service) = boundary.owning_service else {
                continue;
            };
            let index = service_origins
                .binary_search_by_key(&service, |(id, _)| *id)
                .map_err(|_| unavailable("the exact service of an active boundary"))?;
            let owner = service_origins[index].1;
            let original = *originals
                .get(boundary.id)
                .ok_or_else(|| unavailable("the declaration of an active boundary"))?;
            let template = typed
                .closed_requirement_application(original)
                .map_or(original, |closed| closed.template);
            let (_, signature) = typed
                .trait_machine_signature(template)
                .filter(|(slot, _)| slot.symbol == owner)
                .ok_or_else(|| unavailable("the original signature of an active boundary"))?;
            let identity = self.nominal(template)?;
            if native_bindings
                .binary_search_by(|(method, _)| method.requirement.cmp(&identity))
                .is_err()
            {
                self.checked_requirement(signature, boundary, identity, declarations)?;
            }
            reviewed.push(boundary.id);
            review.services.push(service);
            review.services.extend(&boundary.published_service_ceiling);
            review
                .services
                .extend(&boundary.published_synchronous_entries);
        }
        review.boundaries.extend(reviewed);
        Ok(())
    }

    fn project_declared_services(
        &self,
        package: PackageKeyIdentity,
        review: &mut ReviewedProviders,
    ) -> Result<(), Vec<Diagnostic>> {
        let typed = self.typed();
        for declaration in typed.traits() {
            if !closed_service(typed, declaration) || !self.owns(declaration.symbol, package)? {
                continue;
            }
            let signatures = typed.trait_machine_signatures(declaration);
            let mut requirements = Vec::new();
            for signature in signatures {
                let requirement = self.requirement(declaration, signature)?;
                review.callables.push(self.declared_boundary(&requirement)?);
                requirements.push(requirement);
            }
            requirements.sort_by(|left, right| left.identity.cmp(&right.identity));
            review.slots.push(declaration.symbol);
            if declaration.is_public {
                review
                    .public_traits
                    .push(self.trait_shape(declaration, requirements)?);
            }
        }
        review
            .slots
            .sort_unstable_by_key(|symbol| symbol_key(*symbol));
        review.slots.dedup();
        review
            .public_traits
            .sort_by(|left, right| left.identity.cmp(&right.identity));
        Ok(())
    }

    /// A boundary requirement's declaration is reviewable even when no body calls it.
    pub(super) fn declared_boundary(
        &self,
        requirement: &review_evidence::record::PackagePolicyTraitRequirement,
    ) -> Result<PackagePolicyCallable, Vec<Diagnostic>> {
        let reach = requirement
            .service_reach
            .clone()
            .ok_or_else(|| unavailable("a boundary requirement's explicit service ceiling"))?;
        let crashes = requirement
            .published_crash
            .clone()
            .ok_or_else(|| unavailable("a boundary requirement's explicit crash ceiling"))?;
        let installation = requirement.service_reach_is_installation_bound;
        Ok(PackagePolicyCallable {
            target_context: PackagePolicyTargetContext::TargetIndependent,
            role: PackagePolicyCallableRole::Boundary,
            identity: requirement.identity.clone(),
            supply: PackageReviewCallableSupply::Boundary,
            spelling: requirement.spelling,
            lifetime_parameter_count: requirement.lifetime_parameter_count,
            type_parameters: requirement.type_parameters.clone(),
            conformance_bounds: requirement.conformance_bounds.clone(),
            parameters: requirement.parameters.clone(),
            return_type: requirement.return_type.clone(),
            conformances: Vec::new(),
            operator_realizations: Vec::new(),
            requirement_realizations: Vec::new(),
            contracts: requirement.contracts.clone(),
            declared_service_reach: Some(reach.clone()),
            service_reach_dependency: PackagePolicyServiceReachDependency {
                concrete: if installation {
                    Vec::new()
                } else {
                    reach.clone()
                },
                parameters: Vec::new(),
            },
            checked_service_reach: PackageReviewCheckedServiceReach::NoCheckedBody,
            unresolved_installation_reaches: installation
                .then(|| PackageReviewInstallationReach {
                    requirement: requirement.identity.clone(),
                    upper_bound: reach,
                })
                .into_iter()
                .collect(),
            declared_synchronous_invocations: Some(requirement.synchronous_invocations.clone()),
            realized_synchronous_invocations: Vec::new(),
            capability_flows: Vec::new(),
            reachable_capability_flows: Vec::new(),
            checked_may_suspend: false,
            checked_may_block: false,
            declared_may_suspend: Some(requirement.suspends),
            declared_may_block: Some(requirement.blocks),
            declared_termination: Some(requirement.termination.clone()),
            checked_termination: requirement.termination.clone(),
            checked_crash: PackagePolicyCrash {
                interface: PackageReviewCrashInterface::PublishedCeiling,
                published: crashes,
                structural_runtime_requirements: None,
                inferred: PackagePolicyInferredCrash::Unknown,
            },
            mutation: PackagePolicyMutation {
                completeness: PackageReviewWriteFrameCompleteness::Opaque,
                paths: Vec::new(),
            },
        })
    }

    fn authored_providers(
        &self,
        target: TargetProfile,
    ) -> Result<(Vec<AuthoredProvider>, Vec<SymbolHandle>), Vec<Diagnostic>> {
        let source = self.typed();
        let mut supplies: Vec<_> = self.declarations.supplied_declarations()?.collect();
        supplies
            .sort_by_key(|(slot, implementation)| (symbol_key(*slot), symbol_key(*implementation)));
        supplies.dedup();
        let mut explicit = Vec::new();
        for selection in self
            .declarations
            .explicit_providers()
            .iter()
            .filter(|selection| {
                selection
                    .target
                    .is_none_or(|profile| profile.canonical_name() == target.target_name())
            })
        {
            let ProviderSelectionSubject::BoundaryTrait(subject) = &selection.subject else {
                return Err(unavailable("the complete Build provider selection"));
            };
            let provider = &selection.provider_type;
            if subject.applied
                || provider.applied
                || supplies
                    .binary_search_by_key(
                        &(symbol_key(subject.symbol), symbol_key(provider.symbol)),
                        |(slot, implementation)| (symbol_key(*slot), symbol_key(*implementation)),
                    )
                    .is_err()
            {
                return Err(unavailable(
                    "the exact active Build provider declaration and supply",
                ));
            }
            for selected in [subject, provider] {
                if source.symbols.symbol_package_identity(selected.symbol) != selected.package
                    || source.symbols.display_path(selected.symbol, "::") != selected.canonical_path
                {
                    return Err(unavailable("the exact Build provider selection custody"));
                }
            }
            explicit.push(subject.symbol);
        }
        let mut authored = Vec::new();
        for (subject, provider) in supplies {
            // Individual source requirements and numeric operators have different
            // settlement paths; only a schema supply can match this plan key.
            if source
                .trait_definition_by_symbol(subject)
                .is_none_or(|d| !d.is_boundary)
            {
                continue;
            }
            authored.push(AuthoredProvider {
                subject,
                provider,
                subject_owner: source.symbols.symbol_package_identity(subject),
                provider_owner: source.symbols.symbol_package_identity(provider),
                subject_path: source.symbols.display_path(subject, "::"),
                provider_identity: source
                    .normalized_hermetic_symbol_identity(provider)
                    .map_err(unavailable)?,
            });
        }
        authored.sort_unstable_by(|left, right| left.key().cmp(&right.key()));
        Ok((authored, explicit))
    }

    fn prepare_provider(
        &self,
        declarations: &AcceptedDeclarations<'_>,
        plan: ProviderChoice,
        choice: &AuthoredProvider,
        review: &mut ReviewedProviders,
    ) -> Result<PreparedProvider, Vec<Diagnostic>> {
        let source = self.typed();
        if plan.rows.len() != plan.method_count {
            return Err(unavailable(
                "one complete checked installation for the selected provider",
            ));
        }
        let service = source
            .trait_definition_by_symbol(choice.subject)
            .filter(|service| service.is_boundary)
            .ok_or_else(|| unavailable("the selected provider's declared service"))?;
        let methods = source.trait_machine_signatures(service);
        let package = plan
            .package
            .ok_or_else(|| unavailable("the selected provider's package"))?;
        let schema_declaration = self.nominal(service.symbol)?;
        let local_service = choice.subject_owner == Some(package);
        let owned = match (choice.subject_owner, &schema_declaration.owner) {
            (Some(package), PackageReviewNominalOwner::Package(owner)) if *owner == package => {
                self.owns(service.symbol, package)?
            }
            (None, PackageReviewNominalOwner::ToolchainSource(_)) => true,
            _ => false,
        };
        if !closed_service(source, service)
            || methods.len() != plan.schema.declaration.methods.len()
            || methods.len() != plan.method_count
            || !owned
            || (!local_service && !service.is_public)
        {
            return Err(unavailable(
                "the selected provider's complete service application and source custody",
            ));
        }
        let provider_type_declaration = self.nominal(choice.provider)?;
        if provider_type_declaration.owner != PackageReviewNominalOwner::Package(package)
            || !self.owns(choice.provider, package)?
        {
            return Err(unavailable(
                "the selected provider's local source declaration",
            ));
        }
        let declaration_symbols = &declarations.accepted.closure().elaboration().boundaries;
        let mut signatures = methods.iter().collect::<Vec<_>>();
        signatures.sort_unstable_by_key(|signature| symbol_key(signature.symbol));
        let provider_rows = &plan.rows;
        if provider_rows
            .windows(2)
            .any(|pair| pair[0].requirement == pair[1].requirement)
        {
            return Err(unavailable("one selected row for the exact service method"));
        }
        let mut projected_methods = Vec::new();
        let mut rows = Vec::new();
        for method in &plan.schema.declaration.methods {
            let canonical = plan.schema.identity_text(method.requirement_identity);
            let index = provider_rows
                .binary_search_by(|row| row.requirement.as_str().cmp(canonical))
                .map_err(|_| unavailable("one selected row for the exact service method"))?;
            let selected = &provider_rows[index];
            let &[(boundary_id, candidate)] = selected.candidates.as_slice() else {
                return Err(unavailable(
                    "one complete checked installation for the selected provider",
                ));
            };
            let index = declarations
                .boundaries
                .binary_search_by_key(&boundary_id, |boundary| boundary.id)
                .map_err(|_| unavailable("the accepted service requirement contract"))?;
            let boundary = declarations.boundaries[index];
            let service_id = boundary
                .owning_service
                .ok_or_else(|| unavailable("the accepted requirement's service"))?;
            let original = declaration_symbols
                .get(boundary.id)
                .ok_or_else(|| unavailable("the accepted requirement's source declaration"))?;
            let index = signatures
                .binary_search_by_key(&symbol_key(*original), |signature| {
                    symbol_key(signature.symbol)
                })
                .map_err(|_| unavailable("one exact authored service requirement"))?;
            let signature = signatures[index];
            if source.trait_requirement_callable_identity_text(service, signature) != canonical {
                return Err(unavailable("one exact authored service requirement"));
            }
            let requirement = self.nominal(signature.symbol)?;
            let row = self.prepare_row(
                declarations,
                canonical,
                signature.service_reach_is_installation_bound,
                candidate,
                boundary,
                requirement.clone(),
            )?;
            let (projected, _) = self.checked_method(
                signature,
                boundary,
                method,
                &plan.schema,
                requirement,
                schema_declaration.clone(),
                declarations,
            )?;
            projected_methods.push(projected);
            rows.push(row);
            review.boundaries.push(boundary.id);
            review.services.push(service_id);
            review.services.extend(&boundary.published_service_ceiling);
            review
                .services
                .extend(&boundary.published_synchronous_entries);
        }
        Ok(PreparedProvider {
            schema: schema_declaration,
            provider: provider_type_declaration,
            methods: projected_methods,
            rows,
        })
    }

    fn prepare_row(
        &self,
        declarations: &AcceptedDeclarations<'_>,
        canonical: &str,
        installation_bound: bool,
        candidate: MachineId,
        boundary: &BoundaryMachineDeclaration,
        requirement: PackageReviewNominalIdentity,
    ) -> Result<PreparedRow, Vec<Diagnostic>> {
        let index = declarations
            .bodies
            .binary_search_by_key(&candidate, |body| body.id)
            .map_err(|_| unavailable("the selected provider's checked body"))?;
        let body = declarations.bodies[index];
        let index = declarations
            .behaviors
            .binary_search_by_key(&candidate, |body| body.machine)
            .map_err(|_| {
                unavailable("the selected provider's original checked behavior inventory")
            })?;
        let ([reach, invocations], [may_suspend, may_block]) = super::callables::body_effects(
            declarations.accepted.module(),
            body,
            declarations.behaviors[index],
        )?;
        let realization = *declarations
            .accepted
            .closure()
            .elaboration()
            .machines
            .get(candidate)
            .ok_or_else(|| unavailable("the selected provider's original declaration"))?;
        // The selected method publishes both ceilings. The exact checked
        // realization may use a subset of each, regardless of whether the
        // reach ceiling is fixed or settled at installation.
        let outside_reach = reach
            .iter()
            .chain(&body.contract.published_service_ceiling)
            .any(|service| {
                boundary
                    .published_service_ceiling
                    .binary_search(service)
                    .is_err()
            });
        let outside_invocations = invocations
            .iter()
            .chain(&body.contract.published_synchronous_entries)
            .any(|service| {
                boundary
                    .published_synchronous_entries
                    .binary_search(service)
                    .is_err()
            });
        if outside_reach
            || outside_invocations
            || may_suspend
            || may_block
            || !body.contract.crash_routes.is_empty()
            || body.contract.published_may_block
            || body.contract.published_may_suspend
        {
            return Err(unavailable(
                "the selected provider's complete effect or calling policy",
            ));
        }
        Ok(PreparedRow {
            canonical: canonical.to_owned(),
            requirement,
            realization: self.nominal(realization)?,
            installation_bound,
            upper_bound: boundary.published_service_ceiling.clone(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn checked_method(
        &self,
        signature: &StateSignature,
        boundary: &BoundaryMachineDeclaration,
        method: &ServiceMethod,
        schema: &ServiceSchema,
        requirement: PackageReviewNominalIdentity,
        requirement_owner: PackageReviewNominalIdentity,
        declarations: &AcceptedDeclarations<'_>,
    ) -> Result<(PackagePolicyServiceMethod, PackagePolicyCallable), Vec<Diagnostic>> {
        let owner = boundary
            .owning_service
            .ok_or_else(|| unavailable("the selected requirement's owning service"))?;
        let owner_index = declarations
            .service_names
            .binary_search_by_key(&owner, |(service, _)| *service)
            .map_err(|_| unavailable("the selected requirement's own service"))?;
        if declarations.service_names[owner_index].1 != requirement_owner {
            return Err(unavailable(
                "the selected requirement's exact owning service",
            ));
        }

        // Owning a provider slot identifies the method. Its published reach is
        // the independently declared ceiling, including any authored parents.
        let reach = &boundary.published_service_ceiling;
        let authority = reviewed_services(declarations.service_names, reach)?;
        // The plan stores source names. Resolve those from the same checked
        // coordinates, even when the nominal identities have qualified paths.
        let ceiling_matches = declarations
            .source_service_names(reach, |declaration| declaration.name.as_str())?
            .into_iter()
            .eq(method.service_reach.iter().map(String::as_str));
        use service_surface::ServiceSynchronousInvocation;
        use terminal_psi::TerminalCallableInvocation;
        let source = declarations.accepted.source();
        let invocation_contract = declarations
            .accepted
            .module()
            .execution
            .callables
            .boundary_envelope(boundary.id)
            .ok_or_else(|| unavailable("the selected requirement's invocation contract"))?;
        let mut invocations = invocation_contract
            .invocations
            .iter()
            .map(|entry| match entry {
                TerminalCallableInvocation::Parameter(position) => {
                    Ok(ServiceSynchronousInvocation::Parameter(*position))
                }
                TerminalCallableInvocation::Service(service) => {
                    declarations.source_service(*service).map(|declaration| {
                        ServiceSynchronousInvocation::Service(
                            source.trait_declaration_path(declaration),
                        )
                    })
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        invocations.sort_unstable();
        let invocations_match = invocations == method.synchronous_invocations;
        if method.completion != boundary.completion
            || method.terminates_guarantee != boundary.unconditional_termination
            || !ceiling_matches
            || !method.entry_claims.is_empty()
            || !method.result_claims.is_empty()
            || !invocations_match
            || boundary.declaration.as_ref().is_none_or(|declaration| {
                method.may_suspend != declaration.source.published_may_suspend
                    || method.may_block != declaration.source.published_may_block
            })
            || !method.termination_premises.is_empty()
        {
            return Err(unavailable(
                "the selected requirement's complete published contract",
            ));
        }
        let declared_signature = if let Some(application) = self
            .typed()
            .closed_requirement_application(signature.symbol)
        {
            self.typed()
                .trait_machine_signature(application.template)
                .map(|(_, signature)| signature)
                .ok_or_else(|| unavailable("the closed requirement's original signature"))?
        } else {
            signature
        };
        let callable = self.checked_requirement(
            declared_signature,
            boundary,
            requirement.clone(),
            declarations,
        )?;
        let parameters = self.requirement_parameters(signature)?;
        let result = signature
            .return_type
            .is_valid()
            .then(|| {
                self.type_identity_for_private_contract(signature.symbol, signature.return_type)
            })
            .transpose()?;
        let invocation_authority = callable
            .declared_synchronous_invocations
            .clone()
            .ok_or_else(|| unavailable("the selected requirement's synchronous entry ceiling"))?;
        let selected = service_method_policy(
            method,
            schema,
            requirement,
            requirement_owner.clone(),
            PackagePolicyServiceSignature {
                schema_arguments: Vec::new(),
                schema_lifetime_parameter_count: 0,
                schema_lifetime_arguments: Vec::new(),
                application_lifetime_parameter_count: 0,
                requirement_arguments: Vec::new(),
                requirement_lifetime_arguments: Vec::new(),
                requirement_lifetime_parameter_count: 0,
                static_parameters: callable.type_parameters.clone(),
                parameters,
                result,
            },
            PackagePolicyServiceAuthority {
                service_reach: authority,
                synchronous_invocations: invocation_authority,
                progress_premises: Vec::new(),
            },
        );
        Ok((selected, callable))
    }

    fn requirement_parameters(
        &self,
        signature: &StateSignature,
    ) -> Result<Vec<PackageReviewSignatureParameter>, Vec<Diagnostic>> {
        self.typed()
            .state_signature_parameters(signature)
            .iter()
            .map(|parameter| {
                Ok(PackageReviewSignatureParameter {
                    name: parameter.name.as_str().to_owned(),
                    type_identity: self.type_identity_for_private_contract(
                        signature.symbol,
                        parameter.type_reference,
                    )?,
                    is_const: parameter.is_const,
                    is_mutable: parameter.is_mutable,
                })
            })
            .collect()
    }

    fn checked_requirement(
        &self,
        signature: &StateSignature,
        boundary: &BoundaryMachineDeclaration,
        requirement: PackageReviewNominalIdentity,
        declarations: &AcceptedDeclarations<'_>,
    ) -> Result<PackagePolicyCallable, Vec<Diagnostic>> {
        let source = self.typed();
        let declaration = boundary
            .declaration
            .as_ref()
            .ok_or_else(|| unavailable("the exact checked boundary declaration"))?;
        // An external completion retains its source result type for review,
        // but cannot deliver storage to the caller.
        let completes_normally =
            boundary.completion == semantic_vocabulary::BoundaryCompletion::NormalReturn;
        let contracts = self.signature_contracts(
            source.state_signature_contracts(signature).iter(),
            super::declarations::ContractScope {
                owner: signature.symbol,
                subject: None,
                statics: source.state_signature_type_parameters(signature),
                parameters: source.state_signature_parameters(signature),
                result: None,
            },
            signature.return_type,
        )?;
        if !declaration.method_arguments.is_empty() {
            let abi = direct_boundary_bindings::DirectBoundaryLeaf::declared_calling_abi(
                declarations.accepted.module(),
                boundary.id,
            )
            .map_err(|reason| unavailable(&reason))?;
            let module = declarations.accepted.module();
            let (owner, _) = source
                .trait_machine_signature(signature.symbol)
                .ok_or_else(|| unavailable("the boundary requirement's original source"))?;
            if module.identities.text(abi.requirement_identity)
                != Some(
                    source
                        .trait_requirement_callable_identity_text(owner, signature)
                        .as_str(),
                )
            {
                return Err(unavailable(
                    "the boundary requirement's exact original calling method",
                ));
            }
        }
        if !signature.lifetime_parameters.is_empty()
            || !signature.conformance_bounds.is_empty()
            || signature.spelling.is_some()
            || signature.is_default
            || !signature.native_callback_parameters.is_empty()
            || !signature.declares_no_crash_route
            || !boundary.crash_routes.is_empty()
            || boundary.role != BoundaryDeclarationRole::Callable
            || declaration.execution != CheckedBoundaryExecution::Provider
            || boundary.structural_parameters.iter().any(|parameter| {
                !parameter.is_unqualified_root()
                    || !parameter.additional_input_qualifications.is_empty()
            })
            || ((!boundary.requires.is_empty() || !boundary.scalar_requires.is_empty())
                && !contracts.iter().any(|clause| {
                    clause.kind == review_evidence::record::PackageReviewContractKind::Requires
                }))
            || (!boundary.scalar_ensures.is_empty()
                && !contracts.iter().any(|clause| {
                    clause.kind == review_evidence::record::PackageReviewContractKind::Ensures
                }))
            || !boundary.normal_qualifications.is_empty()
            || !boundary.scalar_domain_requirements.is_empty()
            || !boundary.reference_outputs.is_empty()
            || !boundary.program_local_root_introductions.is_empty()
            || !boundary.content_guarantees.is_empty()
            || !boundary.result_reference_regions.is_empty()
            || matches!(&boundary.result, terminal_psi::BoundaryMachineResult::Structural(result)
                if (completes_normally && result.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted)
                    || !result.qualifications.is_empty()
                    || !result.projected_qualifications.is_empty())
            || boundary
                .result_lifetimes
                .as_ref()
                .is_some_and(|rows| !rows.is_empty())
        {
            return Err(unavailable(
                "the boundary requirement's complete published contract",
            ));
        }
        let parameters = self.requirement_parameters(signature)?;
        let type_parameters = source
            .state_signature_type_parameters(signature)
            .iter()
            .map(|parameter| {
                use review_evidence::record::PackagePolicyTypeParameterKind as Kind;
                use source_compiler::representations::source_state::data::TypeParameterKind as Source;
                let kind = match parameter.kind {
                    Source::Type => Kind::Type,
                    Source::Const { type_reference } => {
                        Kind::Const(self.type_identity_for_private_contract(signature.symbol, type_reference)?)
                    }
                    Source::Value { type_reference } => {
                        Kind::Value(self.type_identity_for_private_contract(signature.symbol, type_reference)?)
                    }
                    _ => {
                        return Err(unavailable(
                            "the requirement's complete static parameter contract",
                        ));
                    }
                };
                Ok(review_evidence::record::PackagePolicyTypeParameter {
                    kind,
                    bounds: review_evidence::record::PackageReviewDataProperties {
                        multiplicity: parameter.bounds.multiplicity,
                        carry: parameter.bounds.carry,
                    },
                })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
        let return_type = signature
            .return_type
            .is_valid()
            .then(|| {
                self.type_identity_for_private_contract(signature.symbol, signature.return_type)
            })
            .transpose()?;
        let termination = if boundary.unconditional_termination {
            PackagePolicyTermination::Terminates {
                premises: Vec::new(),
            }
        } else {
            PackagePolicyTermination::NoGuarantee
        };
        if !declarations.has_authored_reach(signature.symbol) {
            return Err(unavailable(
                "the boundary requirement's authored service ceiling",
            ));
        }
        let declared_reach = reviewed_services(
            declarations.service_names,
            &boundary.published_service_ceiling,
        )?;
        let unresolved_installation_reaches = signature
            .service_reach_is_installation_bound
            .then(|| PackageReviewInstallationReach {
                requirement: requirement.clone(),
                upper_bound: declared_reach.clone(),
            })
            .into_iter()
            .collect();
        Ok(PackagePolicyCallable {
            target_context: PackagePolicyTargetContext::TargetIndependent,
            role: PackagePolicyCallableRole::Boundary,
            identity: requirement,
            supply: PackageReviewCallableSupply::Boundary,
            spelling: signature.spelling,
            lifetime_parameter_count: 0,
            type_parameters,
            conformance_bounds: Vec::new(),
            parameters,
            return_type,
            conformances: Vec::new(),
            operator_realizations: Vec::new(),
            requirement_realizations: Vec::new(),
            contracts,
            declared_service_reach: Some(declared_reach.clone()),
            service_reach_dependency: PackagePolicyServiceReachDependency {
                concrete: if signature.service_reach_is_installation_bound {
                    Vec::new()
                } else {
                    declared_reach
                },
                parameters: Vec::new(),
            },
            checked_service_reach: PackageReviewCheckedServiceReach::NoCheckedBody,
            unresolved_installation_reaches,
            declared_synchronous_invocations: Some(super::callables::reviewed_invocations(
                declarations.service_names,
                declarations.accepted.module().execution.callables.boundary_envelope(boundary.id)
                    .ok_or_else(|| unavailable("the boundary requirement's invocation contract"))?,
            )?),
            realized_synchronous_invocations: Vec::new(),
            capability_flows: Vec::new(),
            reachable_capability_flows: Vec::new(),
            checked_may_suspend: false,
            checked_may_block: false,
            declared_may_suspend: Some(declaration.source.published_may_suspend),
            declared_may_block: Some(declaration.source.published_may_block),
            declared_termination: Some(termination.clone()),
            checked_termination: termination,
            checked_crash: PackagePolicyCrash {
                interface: PackageReviewCrashInterface::PublishedCeiling,
                published: Vec::new(),
                structural_runtime_requirements: None,
                inferred: PackagePolicyInferredCrash::Unknown,
            },
            mutation: PackagePolicyMutation {
                completeness: PackageReviewWriteFrameCompleteness::Opaque,
                paths: Vec::new(),
            },
        })
    }
}

fn closed_service(
    source: &source_compiler::representations::source_state::TypedTrees,
    service: &TraitDefinition,
) -> bool {
    service.is_boundary
        && service.lifetime_parameters.is_empty()
        && service.conformance_bounds.is_empty()
        && source.trait_type_parameters(service).is_empty()
        && service.refines.is_none()
        && service.refinement_clauses.is_empty()
}

/// Project the complete schema once the caller has joined its declaration and
/// authority. Authored providers and target primitives retain the same fields.
pub(super) fn service_method_policy(
    method: &ServiceMethod,
    schema: &ServiceSchema,
    requirement: PackageReviewNominalIdentity,
    requirement_owner: PackageReviewNominalIdentity,
    signature: PackagePolicyServiceSignature,
    authority: PackagePolicyServiceAuthority,
) -> PackagePolicyServiceMethod {
    PackagePolicyServiceMethod {
        completion: method.completion,
        name: method.name.clone(),
        requirement_owner,
        requirement,
        signature,
        authority,
        parameter_count: method.parameter_count,
        parameter_type_identities: method
            .parameter_type_identities
            .iter()
            .map(|identity| schema.identity_text(*identity).to_owned())
            .collect(),
        entry_claims: method.entry_claims.clone(),
        has_result: method.has_result,
        result_type_identity: method
            .has_result
            .then(|| schema.identity_text(method.result_type_identity).to_owned()),
        normal_result_memberships: method.normal_result_memberships.clone(),
        result_claims: method.result_claims.clone(),
        service_reach: method.service_reach.clone(),
        synchronous_invocations: method.synchronous_invocations.clone(),
        may_suspend: method.may_suspend,
        may_block: method.may_block,
        terminates_guarantee: method.terminates_guarantee,
        termination_premises: method.termination_premises.clone(),
        identities: schema.declaration.identities.clone(),
        calling: Vec::new(),
    }
}
