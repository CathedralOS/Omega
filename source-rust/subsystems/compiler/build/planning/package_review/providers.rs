//! Project the accepted boundary declarations and checked provider selections.
//! A package may disclose its public requirement without installing a
//! realization.

mod structural_data;

use super::declarations::unavailable;
use super::input::PolicySource;
use super::provider_rows::{ChoiceKey, ProviderChoice};
use super::services::{ReviewedService, reviewed_services};
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
    PackageReviewSelectedInstallationReach, PackageReviewServiceIdentity,
    PackageReviewSignatureParameter, PackageReviewSynchronousInvocation,
    PackageReviewWriteFrameCompleteness,
};
use semantic_vocabulary::{BoundaryMachineId, MachineId, PackageKeyIdentity, ServiceId};
use source_compiler::compile_product::provider_declarations::ProviderSelectionSubject;
use source_compiler::representations::source_state::signature::StateSignature;
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
        for ReviewedService { id, identity, .. } in declarations.services {
            if referenced.contains(identity) {
                applications
                    .entry(identity)
                    .and_modify(|id| *id = None)
                    .or_insert(Some(*id));
            }
        }
        let cited: BTreeSet<_> = applications.into_values().flatten().collect();
        for ReviewedService {
            id: service,
            owner,
            identity,
        } in declarations.services
        {
            let declared_slot = identity.application.is_none()
                && self
                    .slots
                    .binary_search_by_key(&symbol_key(*owner), |slot| symbol_key(*slot))
                    .is_ok();
            if self.services.binary_search(service).is_err()
                && !declared_slot
                && !cited.contains(service)
            {
                return Err(unavailable(format!(
                    "the checked service inventory for {}",
                    declarations
                        .accepted
                        .source()
                        .symbols
                        .display_path(*owner, "::")
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
#[derive(Clone)]
struct AuthoredProvider {
    subject: SymbolHandle,
    provider: SymbolHandle,
    subject_owner: Option<PackageKeyIdentity>,
    provider_owner: Option<PackageKeyIdentity>,
    subject_path: String,
    subject_application: Option<String>,
    provider_identity: String,
}

impl AuthoredProvider {
    fn key(&self) -> ChoiceKey<'_> {
        ChoiceKey {
            provider_package: self.provider_owner,
            provider_identity: &self.provider_identity,
            service_package: self.subject_owner,
            service_declaration: &self.subject_path,
            service_application: self.subject_application.as_deref(),
        }
    }
}

pub(super) struct AcceptedDeclarations<'module> {
    pub(super) accepted: &'module source_compiler::publication::AcceptedTerminalSource,
    boundaries: Vec<&'module BoundaryMachineDeclaration>,
    bodies: Vec<&'module TerminalMachine>,
    behaviors: Vec<&'module UnoptimizedMachineBehavior>,
    authored_reach_owners: Vec<SymbolHandle>,
    services: &'module [ReviewedService],
}

impl<'module> AcceptedDeclarations<'module> {
    pub(super) fn new(
        accepted: &'module source_compiler::publication::AcceptedTerminalSource,
        services: &'module [ReviewedService],
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
            services,
        }
    }

    fn validate_parameter_domains(
        &self,
        signature: &StateSignature,
        requirement: &PackageReviewNominalIdentity,
        boundary: &BoundaryMachineDeclaration,
        contracts: &[review_evidence::record::PackageReviewCallableContract],
    ) -> Result<(), Vec<Diagnostic>> {
        use terminal_psi::{BoundaryParameterKind, StructuralDomainRequirement};
        use terminal_semantics::scalar_qualifications::{
            matches_arithmetic_policy, primitive_qualifications, qualification_domains,
            scalar_domain,
        };
        let mismatch = || {
            unavailable(format!(
                "the parameter domain applications of boundary requirement `{}`",
                requirement.path()
            ))
        };
        if !boundary.has_valid_parameter_order()
            || !StructuralDomainRequirement::declared_roots(&boundary.structural_parameters)
                .eq(boundary.requires.iter().cloned())
        {
            return Err(mismatch());
        }
        let module = self.accepted.module();
        let origins = &self.accepted.closure().elaboration().domain_sources;
        let scalar_positions: Vec<_> = boundary
            .parameter_positions(BoundaryParameterKind::Scalar)
            .collect();
        let structural_positions: Vec<_> = boundary
            .parameter_positions(BoundaryParameterKind::Structural)
            .collect();
        let mut actual = Vec::new();
        let mut scalar_requirements = Vec::new();
        for row in &boundary.scalar_domain_requirements {
            let position = *scalar_positions
                .get(row.argument_index as usize)
                .ok_or_else(mismatch)?;
            scalar_requirements.push((position, row.domain));
        }
        for formal in &boundary.structural_parameters {
            let position = *structural_positions
                .get(formal.position as usize)
                .ok_or_else(mismatch)?;
            let parameter = self
                .accepted
                .source()
                .state_signature_parameters(signature)
                .get(position)
                .ok_or_else(mismatch)?;
            if !structural_data::field_domains_retained(
                self.accepted,
                formal.structural_type,
                &formal.projected_qualifications,
                parameter.type_reference,
            ) {
                return Err(mismatch());
            }
            for &id in &formal.qualifications {
                let domain = module
                    .structural_domain(id)
                    .and_then(|d| origins.get(d.semantic_domain))
                    .ok_or_else(mismatch)?;
                actual.push((position, domain.application));
            }
            // A borrowed scalar carries its domains on the referent's primitive storage.
            let home = primitive_qualifications(module, formal.structural_type);
            for &id in qualification_domains(module, home).ok_or_else(mismatch)? {
                scalar_requirements.push((position, id));
            }
        }
        // Core arithmetic policies have no source domain declaration. The checked
        // formal carries the policy itself and its carrier.
        let mut policies = Vec::new();
        for (position, id) in scalar_requirements {
            let domain = scalar_domain(module, id).ok_or_else(mismatch)?;
            if let Some(source) = origins.get(domain.semantic_domain) {
                actual.push((position, source.application));
            } else {
                policies.push((position, domain));
            }
        }
        use review_evidence::record::{
            PackageReviewContractExpression, PackageReviewContractFact, PackageReviewContractKind,
        };
        let declared_membership = contracts.iter().any(|clause| {
            clause.kind == PackageReviewContractKind::Requires
                && matches!(
                    &clause.fact,
                    PackageReviewContractFact::Membership {
                        value: PackageReviewContractExpression::Parameter(_),
                        ..
                    }
                )
        });
        // Typing records each domain-qualified formal as a required membership.
        // If both sides have empty roots, unrelated unavailable metadata (for
        // example a generic result) is outside this comparison.
        let source = match self.accepted.requirement_review(signature.symbol) {
            Ok(metadata) => metadata,
            Err(_) if actual.is_empty() && policies.is_empty() && !declared_membership => {
                return Ok(());
            }
            Err(reason) => return Err(unavailable(reason)),
        };
        actual.sort_unstable_by_key(|(position, domain)| (*position, domain.coordinates()));
        policies.sort_unstable_by_key(|(position, _)| *position);
        if boundary.parameter_order.len() != source.parameter_types.len()
            || policies.len() != source.parameter_policies.len()
            || !policies.iter().zip(&source.parameter_policies).all(
                |(&(actual_position, domain), &(position, carrier, policy))| {
                    actual_position == position as usize
                        && matches_arithmetic_policy(module, domain, carrier, policy)
                },
            )
            || !structural_positions.iter().copied().eq(source
                .structural_parameter_positions
                .iter()
                .map(|&position| position as usize))
            || !actual.into_iter().eq(source
                .parameter_qualifications
                .iter()
                .map(|&(position, application)| (position as usize, application)))
        {
            return Err(mismatch());
        }
        Ok(())
    }

    fn validate_boundary_predicates(
        &self,
        signature: &StateSignature,
        requirement: &PackageReviewNominalIdentity,
        boundary: &BoundaryMachineDeclaration,
    ) -> Result<(), Vec<Diagnostic>> {
        self.accepted
            .boundary_predicate_signature(boundary.id, signature.symbol)
            .and_then(|source| source.validate_boundary_predicates(boundary))
            .map_err(|part| {
                unavailable(format!(
                    "{part} of boundary requirement `{}`",
                    requirement.path()
                ))
            })
    }

    fn has_authored_reach(&self, owner: SymbolHandle) -> bool {
        self.authored_reach_owners
            .binary_search_by_key(&symbol_key(owner), |symbol| symbol_key(*symbol))
            .is_ok()
    }

    fn service(&self, id: ServiceId) -> Result<&ReviewedService, Vec<Diagnostic>> {
        self.services
            .binary_search_by_key(&id, |row| row.id)
            .map(|index| &self.services[index])
            .map_err(|_| unavailable("the checked source and application of a published service"))
    }

    fn surface_service_name(&self, id: ServiceId) -> Result<String, Vec<Diagnostic>> {
        let service = self.service(id)?;
        if let Some(application) = &service.identity.application {
            return Ok(application.canonical.clone());
        }
        let source = self.accepted.source();
        let declaration = source
            .trait_definition_by_symbol(service.owner)
            .ok_or_else(|| unavailable("the service's authored declaration"))?;
        Ok(source.trait_declaration_path(declaration))
    }
}

struct PreparedProviders<'a> {
    review: ReviewedProviders,
    choices: Vec<PreparedProvider>,
    numeric: Option<super::numeric::PreparedNumeric>,
    services: &'a [ReviewedService],
    service_ids: Vec<(String, ServiceId)>,
}

struct PreparedProvider {
    schema: PackageReviewServiceIdentity,
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

impl PreparedProviders<'_> {
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
                    self.services,
                )?);
            }
            rows.sort_by(|a, b| a.requirement.cmp(&b.requirement));
            review.plans.push(PackagePolicyProviderPlan {
                plan_name: plan.name.clone(),
                realizing_package: plan.origin_package_identity,
                schema_declaration: captured.schema.declaration,
                schema_application: captured.schema.application,
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
        services: &[ReviewedService],
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
            requirement_application: (row.requirement_identity
                != row.declared_requirement_identity)
                .then(|| plan.identity_text(row.requirement_identity).to_owned()),
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
        services: &[ReviewedService],
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
                "the source declaration for every boundary in the accepted module",
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
        self.project_remaining_requirements(declarations, native_bindings, &mut review)?;
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
                "a reviewed source or authenticated primitive for every accepted boundary",
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
            services: declarations.services,
        }
        .finish(checked, target, self.declarations.root_grants())
    }

    fn project_remaining_requirements(
        &self,
        declarations: &AcceptedDeclarations<'_>,
        native_bindings: &[(PackagePolicyServiceMethod, PackagePolicyCallable)],
        review: &mut ReviewedProviders,
    ) -> Result<(), Vec<Diagnostic>> {
        let typed = self.typed();
        let originals = &declarations.accepted.closure().elaboration().boundaries;
        let mut reviewed = Vec::new();
        let mut primitives = Vec::new();
        for &boundary in &declarations.boundaries {
            if review.boundaries.binary_search(&boundary.id).is_ok() {
                continue;
            }
            // A hosted primitive executes a distinct, authenticated declaration
            // of an ordinary service requirement. The public requirement and
            // the receiver's terminal permission retain their own identities.
            if let Some(requirement) = terminal_semantics::closed_conformances::hosted_primitive_requirement(
                declarations.accepted.module(), boundary,
            ).map_err(unavailable)? {
                primitives.push((boundary.id, requirement));
                continue;
            }
            let Some(service) = boundary.owning_service else {
                continue;
            };
            let owner = declarations.service(service)?.owner;
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
        review.boundaries.sort_unstable();
        if primitives.iter().any(|(_, requirement)| review.boundaries.binary_search(requirement).is_err()) {
            return Err(unavailable("the public requirement realized by a checked hosted primitive"));
        }
        review.boundaries.extend(primitives.into_iter().map(|(primitive, _)| primitive));
        Ok(())
    }

    fn project_declared_services(
        &self,
        package: PackageKeyIdentity,
        review: &mut ReviewedProviders,
    ) -> Result<(), Vec<Diagnostic>> {
        let typed = self.typed();
        for declaration in typed.traits() {
            if !declaration.is_boundary || !self.owns(declaration.symbol, package)? {
                continue;
            }
            let signatures = typed.trait_machine_signatures(declaration);
            let mut requirements = Vec::new();
            for signature in signatures {
                let requirement = self.requirement(declaration, signature)?;
                review
                    .callables
                    .push(self.declared_boundary(declaration, &requirement)?);
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
    fn declared_boundary(
        &self,
        owner: &source_compiler::representations::source_state::trait_definition::TraitDefinition,
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
        // The callable is also published outside its trait. Its contract keeps
        // the same binder ordinals it had inside the trait and method scopes.
        let mut type_parameters = self.type_parameters(owner.symbol)?;
        type_parameters.extend_from_slice(&requirement.type_parameters);
        let mut conformance_bounds = self.conformance_bounds(
            owner.symbol,
            &owner.lifetime_parameters,
            &owner.conformance_bounds,
        )?;
        let parent_bounds = u32::try_from(conformance_bounds.len())
            .map_err(|_| unavailable("a boundary callable's conformance binder count"))?;
        for bound in &requirement.conformance_bounds {
            let mut bound = bound.clone();
            bound.binder_ordinal = parent_bounds
                .checked_add(bound.binder_ordinal)
                .ok_or_else(|| unavailable("a boundary callable's conformance binder ordinal"))?;
            conformance_bounds.push(bound);
        }
        Ok(PackagePolicyCallable {
            target_context: PackagePolicyTargetContext::TargetIndependent,
            role: PackagePolicyCallableRole::Boundary,
            identity: requirement.identity.clone(),
            supply: PackageReviewCallableSupply::Boundary,
            spelling: requirement.spelling,
            lifetime_parameter_count: owner
                .lifetime_parameters
                .len()
                .checked_add(requirement.lifetime_parameter_count)
                .ok_or_else(|| unavailable("a boundary callable's lifetime binder count"))?,
            type_parameters,
            conformance_bounds,
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
        let mut supplies = self
            .declarations
            .supplied_declarations(target.target_name())?;
        supplies
            .sort_by_key(|(slot, implementation)| (symbol_key(*slot), symbol_key(*implementation)));
        supplies.dedup();
        let mut explicit = Vec::new();
        let mut selected_applications = Vec::new();
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
            if provider.applied
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
            if subject.applied {
                let application = subject
                    .application_identity
                    .clone()
                    .ok_or_else(|| unavailable("the Build-selected service application"))?;
                selected_applications.push((
                    symbol_key(subject.symbol),
                    symbol_key(provider.symbol),
                    application,
                ));
            }
            explicit.push(subject.symbol);
        }
        selected_applications.sort();
        selected_applications.dedup();
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
            let coordinates = (symbol_key(subject), symbol_key(provider));
            let first = selected_applications.partition_point(|(s, p, _)| (*s, *p) < coordinates);
            let last = selected_applications.partition_point(|(s, p, _)| (*s, *p) <= coordinates);
            let declaration = AuthoredProvider {
                subject,
                provider,
                subject_owner: source.symbols.symbol_package_identity(subject),
                provider_owner: source.symbols.symbol_package_identity(provider),
                subject_path: source.symbols.display_path(subject, "::"),
                subject_application: None,
                provider_identity: source
                    .normalized_hermetic_symbol_identity(provider)
                    .map_err(unavailable)?,
            };
            if first == last {
                authored.push(declaration);
            } else {
                authored.extend(selected_applications[first..last].iter().map(
                    |(_, _, application)| AuthoredProvider {
                        subject_application: Some(application.clone()),
                        ..declaration.clone()
                    },
                ));
            }
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
        if plan
            .schema
            .declaration
            .receiver_applications
            .iter()
            .any(|receiver| !receiver.lifetime_argument_identities.is_empty())
        {
            return Err(unavailable(
                "the selected service's checked lifetime substitution",
            ));
        }
        let receiver = self.schema_effect_scope(choice.subject, &plan.schema)?;
        let receiver_identity = self.effect_identity(&receiver)?;
        let declaring_scopes = source_compiler::representations::source_state::service_declarations::projection::inherited_scopes(
            source, receiver,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        let mut signatures = std::collections::HashMap::new();
        for declaring in &declaring_scopes {
            for signature in source.trait_machine_signatures(declaring.declaration) {
                if signatures
                    .insert(signature.symbol, (declaring, signature))
                    .is_some()
                {
                    return Err(unavailable("the unique declared provider requirement"));
                }
            }
        }
        if receiver_identity
            .application
            .as_ref()
            .map(|term| term.canonical.as_str())
            != choice.subject_application.as_deref()
            || signatures.len() != plan.schema.declaration.methods.len()
            || signatures.len() != plan.method_count
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
            let application = source.closed_requirement_application(*original);
            let template = application.map_or(*original, |closed| closed.template);
            let (declaring, signature) = signatures
                .remove(&template)
                .ok_or_else(|| unavailable("one exact authored service requirement"))?;
            let owner = declaring.declaration;
            if source.trait_requirement_callable_identity_text(owner, signature) != canonical
                || source.trait_declaration_path(owner) != method.requirement_owner
                || source.symbols.symbol_package_identity(owner.symbol)
                    != method.requirement_owner_package_identity
                || declarations.service(service_id)?.identity != self.effect_identity(declaring)?
            {
                return Err(unavailable(
                    "the selected method's exact declaring receiver",
                ));
            }
            let checked_signature = if let Some(closed) = application {
                if closed.declaring_trait != owner.symbol
                    || !source.requirement_application_matches_contract_scope(*original, declaring)
                    || !closed.local_arguments.is_empty()
                {
                    return Err(unavailable("the selected provider's exact applied method"));
                }
                closed.signature
            } else if declaring.keeps_declared_signatures(source) {
                signature
            } else {
                return Err(unavailable(
                    "the selected provider's applied requirement signature",
                ));
            };
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
                checked_signature,
                boundary,
                method,
                &plan.schema,
                requirement,
                self.nominal(owner.symbol)?,
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
            schema: receiver_identity,
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
        let ceiling = &boundary.declaration.as_ref()
            .ok_or_else(|| unavailable("the selected provider's checked boundary declaration"))?.source;
        // Conditional boundary guards still need their complete review representation.
        // An unconditional route covers only the matching cause, including narrower body guards.
        let outside_crash = body.contract.crash_routes.iter()
            .chain(body.contract.declared_crash_routes.iter().flatten())
            .any(|route| !boundary.crash_routes.iter().any(|allowed| {
                route.cause == allowed.cause
                    && allowed.alternatives.as_slice() == [terminal_psi::CrashRouteGuard::Truth]
            }));
        if outside_reach || outside_invocations || outside_crash
            || ((may_suspend || body.contract.published_may_suspend) && !ceiling.published_may_suspend)
            || ((may_block || body.contract.published_may_block) && !ceiling.published_may_block)
        {
            return Err(unavailable("provider effects within the selected requirement's checked ceiling"));
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
        let published_owner = &declarations.service(owner)?.identity;
        let mut declaring_receivers = schema.declaration.declaring_receivers(method);
        let declaring = declaring_receivers.next();
        let exact_receiver = match (&published_owner.application, declaring) {
            (Some(term), Some(receiver)) => {
                schema.identity_text(receiver.application) == term.canonical
            }
            (None, Some(receiver)) => {
                receiver.argument_identities.is_empty()
                    && receiver.lifetime_argument_identities.is_empty()
            }
            (None, None) => schema.declaration.receiver_applications.is_empty(),
            _ => false,
        };
        if published_owner.declaration != requirement_owner
            || !exact_receiver
            || declaring_receivers.next().is_some()
        {
            return Err(unavailable(
                "the selected requirement's exact owning service",
            ));
        }

        let reach = &boundary.published_service_ceiling;
        let authority = reviewed_services(declarations.services, reach)?;
        let mut surface_reach = reach
            .iter()
            .map(|service| declarations.surface_service_name(*service))
            .collect::<Result<Vec<_>, _>>()?;
        surface_reach.sort_unstable();
        let ceiling_matches = surface_reach == method.service_reach;
        use service_surface::ServiceSynchronousInvocation;
        use terminal_psi::TerminalCallableInvocation;
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
                TerminalCallableInvocation::Service(service) => declarations
                    .surface_service_name(*service)
                    .map(ServiceSynchronousInvocation::Service),
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
        let (parameters, result) = if signature.symbol == declared_signature.symbol {
            (callable.parameters.clone(), callable.return_type.clone())
        } else {
            // An applied method has its own types in the selected provider row.
            // Its declaration's binders cannot stand in for applied lifetimes.
            if callable.lifetime_parameter_count != 0 {
                return Err(unavailable(
                    "the applied requirement's lifetime substitution",
                ));
            }
            let parameters = self.requirement_parameters(signature)?;
            let result = signature
                .return_type
                .is_valid()
                .then(|| {
                    self.type_identity_for_private_contract(signature.symbol, signature.return_type)
                })
                .transpose()?;
            (parameters, result)
        };
        let invocation_authority = callable
            .declared_synchronous_invocations
            .clone()
            .ok_or_else(|| unavailable("the selected requirement's synchronous entry ceiling"))?;
        let arguments = |receiver: Option<&service_surface::ServiceReceiverApplication>| {
            receiver
                .into_iter()
                .flat_map(|receiver| &receiver.argument_identities)
                .map(
                    |identity| review_evidence::record::PackageReviewTypeIdentity {
                        canonical: schema.identity_text(*identity).to_owned(),
                    },
                )
                .collect()
        };
        let selected = service_method_policy(
            method,
            schema,
            requirement,
            requirement_owner.clone(),
            PackagePolicyServiceSignature {
                schema_arguments: arguments(schema.declaration.receiver_applications.first()),
                requirement_arguments: arguments(declaring),
                schema_lifetime_parameter_count: 0,
                schema_lifetime_arguments: Vec::new(),
                application_lifetime_parameter_count: 0,
                requirement_lifetime_arguments: Vec::new(),
                requirement_lifetime_parameter_count: u32::try_from(
                    callable.lifetime_parameter_count,
                )
                .map_err(|_| unavailable("a portable requirement lifetime count"))?,
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
        let authored_routes = source.state_signature_contracts(signature).iter().any(|clause| {
            matches!(clause.kind, source_compiler::representations::source_state::signature::SignatureContractKind::Crashes { .. })
        });
        declarations.validate_parameter_domains(signature, &requirement, boundary, &contracts)?;
        declarations.validate_boundary_predicates(signature, &requirement, boundary)?;
        let mut fields = Vec::new();
        macro_rules! unretained {
            ($field:expr, $when:expr) => {
                if $when {
                    fields.push(stringify!($field));
                }
            };
            ($field:expr) => {
                unretained!($field, !$field.is_empty());
            };
        }
        unretained!(signature.conformance_bounds);
        unretained!(signature.spelling, signature.spelling.is_some());
        unretained!(signature.is_default, signature.is_default);
        unretained!(signature.native_callback_parameters);
        unretained!(
            signature.declares_no_crash_route,
            signature.declares_no_crash_route == authored_routes
        );
        unretained!(
            boundary.crash_routes,
            signature.declares_no_crash_route != boundary.crash_routes.is_empty()
        );
        unretained!(
            boundary.role,
            boundary.role != BoundaryDeclarationRole::Callable
        );
        unretained!(
            declaration.execution,
            declaration.execution != CheckedBoundaryExecution::Provider
        );
        for parameter in &boundary.structural_parameters {
            unretained!(
                parameter.multiplicity,
                !matches!(
                    parameter.multiplicity,
                    terminal_psi::nonlinear_multiplicity_pair_pattern!()
                )
            );
            unretained!(parameter.additional_input_qualifications);
        }
        unretained!(boundary.normal_qualifications);
        unretained!(boundary.reference_outputs);
        unretained!(boundary.program_local_root_introductions);
        unretained!(boundary.content_guarantees);
        unretained!(boundary.result_reference_regions);
        if let terminal_psi::BoundaryMachineResult::Structural(result) = &boundary.result {
            unretained!(
                result.multiplicity,
                completes_normally
                    && !structural_data::result_multiplicity_retained(
                        declarations.accepted,
                        result,
                        signature.return_type,
                    )
            );
            unretained!(result.qualifications);
            let fields_retained = if completes_normally {
                structural_data::field_domains_retained(
                    declarations.accepted,
                    result.structural_type,
                    &result.projected_qualifications,
                    signature.return_type,
                )
            } else {
                result.projected_qualifications.is_empty()
            };
            unretained!(result.projected_qualifications, !fields_retained);
        }
        if !fields.is_empty() {
            fields.sort_unstable();
            fields.dedup();
            return Err(unavailable(format!(
                "{} on boundary requirement `{}`",
                fields.join(", "),
                requirement.path()
            )));
        }
        let groups = terminal_semantics::references::boundary_result_lifetimes(
            &declarations.accepted.module().types.structural_types,
            boundary,
        )
        .map_err(unavailable)?;
        let metadata = declarations.accepted.requirement_review(signature.symbol);
        let scoped = !groups.is_empty()
            || !signature.lifetime_parameters.is_empty()
            || metadata
                .as_ref()
                .is_ok_and(|row| row.lifetime_parameter_count != 0);
        let (parameters, return_type, lifetime_parameter_count) = if scoped {
            let metadata = metadata.map_err(unavailable)?;
            if !groups.is_empty() {
                let result = metadata.result_type.as_deref().ok_or_else(|| {
                    unavailable("a declared result for the boundary lifetime groups")
                })?;
                let declared =
                    terminal_semantics::references::boundary_result_lifetimes_from_signature(
                        declarations.accepted.module(),
                        boundary,
                        metadata.lifetime_parameter_count,
                        &metadata.parameter_types,
                        result,
                    )
                    .map_err(unavailable)?;
                if declared != groups {
                    return Err(unavailable(
                        "boundary result lifetime groups matching the declared signature",
                    ));
                }
            }
            let parameters = self.scoped_requirement_parameters(
                    signature, &metadata.parameter_types,
                    review_evidence::record::PackageReviewSemanticDependencyExposure::PrivateImplementation,
                )?;
            let result = metadata.result_type.as_ref().map(|canonical| {
                review_evidence::record::PackageReviewTypeIdentity {
                    canonical: canonical.clone(),
                }
            });
            (parameters, result, metadata.lifetime_parameter_count)
        } else {
            let parameters = self.requirement_parameters(signature)?;
            let return_type = signature
                .return_type
                .is_valid()
                .then(|| {
                    self.type_identity_for_private_contract(signature.symbol, signature.return_type)
                })
                .transpose()?;
            (parameters, return_type, 0)
        };
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
                    },
                })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
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
        let declared_reach =
            reviewed_services(declarations.services, &boundary.published_service_ceiling)?;
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
            lifetime_parameter_count,
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
                declarations.services,
                declarations
                    .accepted
                    .module()
                    .execution
                    .callables
                    .boundary_envelope(boundary.id)
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
                published: super::callables::published_crash_routes(&boundary.crash_routes)?,
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
