//! Project the accepted boundary declarations and checked provider selections.
//! A package may disclose its public requirement without installing a
//! realization.

use super::PackageReviewInput;
use super::callables::reviewed_services;
use super::declarations::unavailable;
use crate::record::{
    PackagePolicyCallable, PackagePolicyCallableRole, PackagePolicyCrash,
    PackagePolicyInferredCrash, PackagePolicyMutation, PackagePolicyNativeBinding,
    PackagePolicyProviderBinding, PackagePolicyProviderPlan, PackagePolicyProviderRow,
    PackagePolicyServiceAuthority, PackagePolicyServiceMethod, PackagePolicyServiceReachDependency,
    PackagePolicyServiceSignature, PackagePolicyTargetContext, PackagePolicyTermination,
    PackagePolicyTraitRequirement, PackagePolicyTraitShape, PackageReviewCallableSupply,
    PackageReviewCheckedServiceReach, PackageReviewCrashInterface, PackageReviewInstallationReach,
    PackageReviewNominalIdentity, PackageReviewNominalOwner,
    PackageReviewSelectedInstallationReach, PackageReviewSignatureParameter,
    PackageReviewWriteFrameCompleteness,
};
use backend_plan::SelectedCheckedInvocationRow;
use build_time_evaluation::provider_declarations::{
    ProviderSelectionIdentity, ProviderSelectionSubject,
};
use diagnostics::Diagnostic;
use effects::provider_plan::{
    ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod, ServiceSchema,
};
use semantic_vocabulary::{BoundaryMachineId, MachineId, PackageKeyIdentity, ServiceId};
use symbols::SymbolHandle;
use target::TargetProfile;
use terminal_psi::{
    BoundaryDeclarationRole, BoundaryMachineDeclaration, CheckedBoundaryExecution, TerminalMachine,
    UnoptimizedMachineBehavior,
};
use typed_trees::signature::StateSignature;
use typed_trees::trait_definition::TraitDefinition;

pub(super) struct ReviewedProviders {
    pub slots: Vec<SymbolHandle>,
    pub callables: Vec<PackagePolicyCallable>,
    pub plans: Vec<PackagePolicyProviderPlan>,
    pub public_traits: Vec<PackagePolicyTraitShape>,
    boundaries: Vec<BoundaryMachineId>,
    services: Vec<ServiceId>,
}

pub(super) fn symbol_key(symbol: SymbolHandle) -> (u32, u32) {
    (symbol.arena_index(), symbol.generation())
}

struct AuthoredProvider<'source> {
    subject: &'source ProviderSelectionIdentity,
    provider: &'source ProviderSelectionIdentity,
}

impl<'source> AuthoredProvider<'source> {
    fn key(&self) -> (Option<PackageKeyIdentity>, &'source str, &'source str) {
        (
            self.provider.package,
            &self.provider.canonical_path,
            &self.subject.canonical_path,
        )
    }
}

pub(super) struct AcceptedDeclarations<'module> {
    pub(super) accepted: &'module source_compiler::AcceptedTerminalSource,
    boundaries: Vec<&'module BoundaryMachineDeclaration>,
    bodies: Vec<&'module TerminalMachine>,
    behaviors: Vec<&'module UnoptimizedMachineBehavior>,
    authored_reach_owners: Vec<SymbolHandle>,
    service_names: &'module [(ServiceId, PackageReviewNominalIdentity)],
}

impl<'module> AcceptedDeclarations<'module> {
    pub(super) fn new(
        accepted: &'module source_compiler::AcceptedTerminalSource,
        service_names: &'module [(ServiceId, PackageReviewNominalIdentity)],
        plans: &[ProviderPlan],
    ) -> Self {
        let module = accepted.module();
        let mut boundaries = module.boundary_machines.iter().collect::<Vec<_>>();
        boundaries.sort_unstable_by_key(|boundary| boundary.id);
        let (bodies, behaviors) = if plans.is_empty() {
            (Vec::new(), Vec::new())
        } else {
            let mut bodies = module.machines.iter().collect::<Vec<_>>();
            bodies.sort_unstable_by_key(|body| body.id);
            let mut behaviors = module.unoptimized_behavior.iter().collect::<Vec<_>>();
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
            service_names,
        }
    }

    fn has_authored_reach(&self, owner: SymbolHandle) -> bool {
        self.authored_reach_owners
            .binary_search_by_key(&symbol_key(owner), |symbol| symbol_key(*symbol))
            .is_ok()
    }
}

impl PackageReviewInput<'_> {
    pub(super) fn selected_providers(
        &self,
        package: PackageKeyIdentity,
        target: TargetProfile,
        service_origins: &[(ServiceId, SymbolHandle)],
        declarations: &AcceptedDeclarations<'_>,
        native_bindings: &[(PackagePolicyNativeBinding, PackagePolicyCallable)],
    ) -> Result<ReviewedProviders, Vec<Diagnostic>> {
        let checked = self.compilation();
        let selected = checked.execution().plans();
        let accepted = declarations.accepted;
        let module = accepted.module();
        let mut review = ReviewedProviders {
            slots: Vec::new(),
            callables: Vec::new(),
            plans: Vec::new(),
            public_traits: Vec::new(),
            boundaries: Vec::new(),
            services: Vec::new(),
        };
        if module.services.is_empty()
            && module.boundary_machines.is_empty()
            && module.provider_candidates.is_empty()
            && selected.plans().is_empty()
            && checked.build().config.provider_selections.is_empty()
        {
            self.project_memberless_services(package, &mut review)?;
            return Ok(review);
        }
        super::callables::validate_body_effect_inventory(module)?;
        if accepted.closure().elaboration.boundaries.len() != declarations.boundaries.len() {
            return Err(unavailable(
                "the accepted module's complete boundary inventory",
            ));
        }
        let (authored, mut supplied_slots) = self.authored_providers(target)?;
        let provider_rows =
            backend_plan::selected_checked_invocation_rows(selected.plans(), module)
                .map_err(|reason| unavailable(&reason))?;
        let mut remaining = provider_rows.as_slice();
        for plan in selected.plans() {
            if plan.target != target.target_name() || plan.origin_package_identity != Some(package)
            {
                return Err(unavailable(
                    "the selected provider's exact package and deployment",
                ));
            }
            let key = (
                plan.origin_package_identity,
                plan.provider_type.as_str(),
                plan.schema
                    .identity_text(plan.schema.declaration.trait_name),
            );
            let first = authored.partition_point(|choice| choice.key() < key);
            let last = authored.partition_point(|choice| choice.key() <= key);
            let [choice] = &authored[first..last] else {
                return Err(unavailable(
                    "the plan's exact authored provider and service selection",
                ));
            };
            // The backend returns each plan's checked rows contiguously.
            let count = remaining
                .iter()
                .take_while(|row| std::ptr::eq(row.plan, plan))
                .count();
            let (plan_rows, rest) = remaining.split_at(count);
            remaining = rest;
            let projected =
                self.project_provider(declarations, plan, choice, plan_rows, &mut review)?;
            review.slots.push(choice.subject.symbol);
            review.plans.push(projected);
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
        self.project_remaining_requirements(
            package,
            declarations,
            service_origins,
            native_bindings,
            &mut review,
        )?;
        self.project_memberless_services(package, &mut review)?;
        review.boundaries.sort_unstable();
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
            || module.services.iter().any(|row| !row.parents.is_empty())
            || service_origins.iter().any(|(service, owner)| {
                review.services.binary_search(service).is_err()
                    && review
                        .slots
                        .binary_search_by_key(&symbol_key(*owner), |slot| symbol_key(*slot))
                        .is_err()
            })
        {
            return Err(unavailable(
                "the accepted module's complete boundary/service inventory",
            ));
        }
        review
            .plans
            .sort_by(PackagePolicyProviderPlan::compare_canonical);
        Ok(review)
    }

    fn project_remaining_requirements(
        &self,
        package: PackageKeyIdentity,
        declarations: &AcceptedDeclarations<'_>,
        service_origins: &[(ServiceId, SymbolHandle)],
        native_bindings: &[(PackagePolicyNativeBinding, PackagePolicyCallable)],
        review: &mut ReviewedProviders,
    ) -> Result<(), Vec<Diagnostic>> {
        let typed = self.typed();
        let original_boundaries = &declarations.accepted.closure().elaboration.boundaries;
        // The checked roster names each requirement even without a provider.
        // Join its source declarations by the service and signature identities.
        let mut unsupplied = Vec::new();
        for &boundary in &declarations.boundaries {
            if review.boundaries.binary_search(&boundary.id).is_ok() {
                continue;
            }
            let Some(service) = boundary.owning_service else {
                continue;
            };
            let original = *original_boundaries
                .get(boundary.id)
                .ok_or_else(|| unavailable("the accepted requirement's source declaration"))?;
            unsupplied.push((service, original, boundary));
        }
        unsupplied.sort_unstable_by_key(|(service, original, _)| (*service, symbol_key(*original)));

        for rows in unsupplied.chunk_by(|left, right| left.0 == right.0) {
            let service = rows[0].0;
            let index = service_origins
                .binary_search_by_key(&service, |(service, _)| *service)
                .map_err(|_| unavailable("the accepted requirement's owning service"))?;
            let owner = service_origins[index].1;
            let Some(trait_declaration) = typed.trait_definition_by_symbol(owner) else {
                continue;
            };
            if (!trait_declaration.is_public && native_bindings.is_empty())
                || !closed_service(typed, trait_declaration)
                || !self.owns(owner, package)?
            {
                continue;
            }
            let signatures = typed.trait_machine_signatures(trait_declaration);
            if signatures.len() != rows.len() || rows.windows(2).any(|pair| pair[0].1 == pair[1].1)
            {
                return Err(unavailable("the complete public requirement roster"));
            }
            let mut callables = Vec::with_capacity(rows.len());
            for signature in signatures {
                let index = rows
                    .binary_search_by_key(&symbol_key(signature.symbol), |(_, original, _)| {
                        symbol_key(*original)
                    })
                    .map_err(|_| unavailable("one exact public service requirement"))?;
                let boundary = rows[index].2;
                let identity = self.nominal(signature.symbol)?;
                let callable = match native_bindings
                    .binary_search_by(|(binding, _)| binding.method.requirement.cmp(&identity))
                {
                    Ok(index) => native_bindings[index].1.clone(),
                    Err(_) if trait_declaration.is_public => {
                        self.checked_requirement(signature, boundary, identity, declarations)?
                    }
                    Err(_) => break,
                };
                callables.push((boundary.id, callable));
            }
            // A private declaration is retained only with its complete
            // authored native supply. An unrelated private row stays refused.
            if callables.len() != rows.len() {
                continue;
            }
            if trait_declaration.is_public {
                let requirements = callables
                    .iter()
                    .map(|(_, callable)| service_requirement(callable))
                    .collect();
                review
                    .public_traits
                    .push(service_trait(self.nominal(owner)?, requirements));
            }
            for (boundary, callable) in callables {
                review.boundaries.push(boundary);
                review.callables.push(callable);
            }
            review.slots.push(owner);
            review.services.push(service);
        }
        Ok(())
    }

    fn project_memberless_services(
        &self,
        package: PackageKeyIdentity,
        review: &mut ReviewedProviders,
    ) -> Result<(), Vec<Diagnostic>> {
        let typed = self.typed();
        for declaration in typed.traits() {
            if !closed_service(typed, declaration)
                || !typed.trait_machine_signatures(declaration).is_empty()
                || !self.owns(declaration.symbol, package)?
            {
                continue;
            }
            review.slots.push(declaration.symbol);
            if declaration.is_public {
                review
                    .public_traits
                    .push(service_trait(self.nominal(declaration.symbol)?, Vec::new()));
            }
        }
        review
            .slots
            .sort_unstable_by_key(|symbol| symbol_key(*symbol));
        review
            .public_traits
            .sort_by(|left, right| left.identity.cmp(&right.identity));
        Ok(())
    }

    fn authored_providers(
        &self,
        target: TargetProfile,
    ) -> Result<(Vec<AuthoredProvider<'_>>, Vec<SymbolHandle>), Vec<Diagnostic>> {
        let compilation = self.compilation();
        let roster = compilation.terminal_publication().roster()?;
        let mut supplies = roster
            .supplies
            .iter()
            .map(|supply| (symbol_key(supply.slot), symbol_key(supply.provider)))
            .collect::<Vec<_>>();
        supplies.sort_unstable();
        let mut authored = Vec::new();
        let mut supplied_slots = Vec::new();
        for selection in &compilation.build().config.provider_selections {
            let ProviderSelectionSubject::BoundaryTrait(subject) = &selection.subject else {
                return Err(unavailable("the complete Build provider selection"));
            };
            let provider = &selection.provider_type;
            if subject.applied
                || provider.applied
                || supplies
                    .binary_search(&(symbol_key(subject.symbol), symbol_key(provider.symbol)))
                    .is_err()
            {
                return Err(unavailable("the exact Build provider selection custody"));
            }
            supplied_slots.push(subject.symbol);
            if selection
                .target
                .is_none_or(|selected| selected.canonical_name() == target.target_name())
            {
                authored.push(AuthoredProvider { subject, provider });
            }
        }
        authored.sort_unstable_by_key(AuthoredProvider::key);
        Ok((authored, supplied_slots))
    }

    fn project_provider(
        &self,
        declarations: &AcceptedDeclarations<'_>,
        plan: &ProviderPlan,
        choice: &AuthoredProvider<'_>,
        provider_rows: &[SelectedCheckedInvocationRow<'_, '_>],
        review: &mut ReviewedProviders,
    ) -> Result<PackagePolicyProviderPlan, Vec<Diagnostic>> {
        let source = self.typed();
        if provider_rows.len() != plan.rows.len() {
            return Err(unavailable(
                "one complete checked installation for the selected provider",
            ));
        }
        let service = source
            .trait_definition_by_symbol(choice.subject.symbol)
            .filter(|service| service.is_boundary)
            .ok_or_else(|| unavailable("the selected provider's declared service"))?;
        let methods = source.trait_machine_signatures(service);
        let package = plan
            .origin_package_identity
            .ok_or_else(|| unavailable("the selected provider's package"))?;
        let schema_declaration = self.nominal(service.symbol)?;
        let service_package = choice
            .subject
            .package
            .ok_or_else(|| unavailable("the selected requirement's defining package"))?;
        let local_service = service_package == package;
        if !closed_service(source, service)
            || methods.len() != plan.schema.declaration.methods.len()
            || methods.len() != plan.rows.len()
            || schema_declaration.owner != PackageReviewNominalOwner::Package(service_package)
            || !self.owns(service.symbol, service_package)?
            || (!local_service && !service.is_public)
        {
            return Err(unavailable(
                "the selected provider's complete service application and source custody",
            ));
        }
        let provider_type_declaration = self.nominal(choice.provider.symbol)?;
        if provider_type_declaration.owner != PackageReviewNominalOwner::Package(package)
            || !self.owns(choice.provider.symbol, package)?
        {
            return Err(unavailable(
                "the selected provider's local source declaration",
            ));
        }
        let declaration_symbols = &declarations.accepted.closure().elaboration.boundaries;
        let mut signatures = methods.iter().collect::<Vec<_>>();
        signatures.sort_unstable_by_key(|signature| symbol_key(signature.symbol));
        let row_identity = |row: &SelectedCheckedInvocationRow<'_, '_>| {
            plan.identity_text(row.row.declared_requirement_identity)
        };
        let mut provider_rows = provider_rows.iter().collect::<Vec<_>>();
        provider_rows.sort_unstable_by_key(|row| row_identity(row));
        if provider_rows
            .windows(2)
            .any(|pair| row_identity(pair[0]) == row_identity(pair[1]))
        {
            return Err(unavailable("one selected row for the exact service method"));
        }
        let mut projected_methods = Vec::new();
        let mut public_requirements = Vec::new();
        let mut rows = Vec::new();
        for method in &plan.schema.declaration.methods {
            let canonical = plan.schema.identity_text(method.requirement_identity);
            let index = provider_rows
                .binary_search_by_key(&canonical, |row| row_identity(row))
                .map_err(|_| unavailable("one selected row for the exact service method"))?;
            let selected = provider_rows[index];
            let [candidate] = selected.candidates.as_slice() else {
                return Err(unavailable(
                    "one complete checked installation for the selected provider",
                ));
            };
            let candidate = candidate.conformance;
            let index = declarations
                .boundaries
                .binary_search_by_key(&candidate.boundary, |boundary| boundary.id)
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
            let row = self.checked_row(
                declarations,
                plan,
                selected.row,
                candidate.candidate,
                boundary,
                requirement.clone(),
            )?;
            let (projected, callable) = self.checked_method(
                service,
                signature,
                boundary,
                method,
                &plan.schema,
                requirement,
                schema_declaration.clone(),
                declarations,
            )?;
            // The plan refers to the exact requirement. The defining
            // package's review publishes that requirement as a callable.
            if local_service {
                if service.is_public {
                    public_requirements.push(service_requirement(&callable));
                }
                review.callables.push(callable);
            }
            projected_methods.push(projected);
            rows.push(row);
            review.boundaries.push(boundary.id);
            review.services.push(service_id);
        }
        if local_service && service.is_public {
            review.public_traits.push(service_trait(
                schema_declaration.clone(),
                public_requirements,
            ));
        }
        rows.sort_by(|left, right| left.requirement.cmp(&right.requirement));
        Ok(PackagePolicyProviderPlan {
            plan_name: plan.name.clone(),
            realizing_package: plan.origin_package_identity,
            schema_declaration,
            provider_type: plan.provider_type.clone(),
            provider_type_declaration: Some(provider_type_declaration),
            target: plan.target.clone(),
            methods: projected_methods,
            rows,
            grants: Vec::new(),
        })
    }

    fn checked_installation_reach(
        &self,
        declarations: &AcceptedDeclarations<'_>,
        plan: &ProviderPlan,
        row: &ProviderPlanRow,
        boundary: &BoundaryMachineDeclaration,
    ) -> Result<Option<PackageReviewSelectedInstallationReach>, Vec<Diagnostic>> {
        let source = declarations
            .accepted
            .closure()
            .elaboration
            .boundaries
            .get(boundary.id)
            .and_then(|symbol| self.typed().trait_machine_signature(*symbol))
            .ok_or_else(|| unavailable("the installation reach requirement declaration"))?;
        let selected = self.compilation().execution().plans();
        let matches = selected
            .installation_reach_resolutions()
            .iter()
            .filter(|reach| {
                reach.provider_plan_report_identity == plan.report_fingerprint()
                    && selected.identity_text(reach.requirement_identity)
                        == plan.identity_text(row.requirement_identity)
            })
            .collect::<Vec<_>>();
        let reach = match (
            source.1.service_reach_is_installation_bound,
            matches.as_slice(),
        ) {
            (false, []) => return Ok(None),
            (true, [reach]) => *reach,
            _ => return Err(unavailable("the exact selected installation reach row")),
        };
        let module = declarations.accepted.module();
        let project = |names: &[String]| -> Result<_, Vec<Diagnostic>> {
            let mut ids = Vec::with_capacity(names.len());
            for name in names {
                let mut matches = module.services.iter().filter(|service| {
                    module.identities.text(service.identity) == Some(name.as_str())
                });
                let service = matches
                    .next()
                    .filter(|_| matches.next().is_none())
                    .ok_or_else(|| unavailable("the selected reach's exact service declaration"))?;
                ids.push(service.id);
            }
            reviewed_services(declarations.service_names, &ids)
        };
        let upper_bound = project(&reach.upper_bound)?;
        if upper_bound
            != reviewed_services(
                declarations.service_names,
                &boundary.published_service_ceiling,
            )?
        {
            return Err(unavailable("the selected reach's published upper bound"));
        }
        Ok(Some(PackageReviewSelectedInstallationReach {
            upper_bound,
            resolved: project(&reach.resolved_row)?,
        }))
    }

    fn checked_row(
        &self,
        declarations: &AcceptedDeclarations<'_>,
        plan: &ProviderPlan,
        row: &ProviderPlanRow,
        candidate: MachineId,
        boundary: &BoundaryMachineDeclaration,
        requirement: PackageReviewNominalIdentity,
    ) -> Result<PackagePolicyProviderRow, Vec<Diagnostic>> {
        let ProviderBinding::CheckedAdapter {
            machine_identity,
            machine_package_identity,
        } = row.binding
        else {
            return Err(unavailable("the selected provider's checked realization"));
        };
        let callable = plan.identity_text(machine_identity);
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
            declarations.accepted,
            body,
            declarations.behaviors[index],
        )?;
        let realization = *declarations
            .accepted
            .closure()
            .elaboration
            .machines
            .get(candidate)
            .ok_or_else(|| unavailable("the selected provider's original declaration"))?;
        // A checked realization with no authored reach clause can carry
        // its governing requirement's ceiling in the execution contract.
        // The method row already publishes that ceiling, and the checked
        // body still owes its own empty reach in this projection.
        let installation_reach =
            self.checked_installation_reach(declarations, plan, row, boundary)?;
        let unrepresented_reach = if installation_reach.is_some() {
            reach.iter().any(|service| {
                boundary
                    .published_service_ceiling
                    .binary_search(service)
                    .is_err()
            }) || body
                .contract
                .published_service_ceiling
                .iter()
                .any(|service| {
                    boundary
                        .published_service_ceiling
                        .binary_search(service)
                        .is_err()
                })
        } else {
            !reach.is_empty()
                || (!body.contract.published_service_ceiling.is_empty()
                    && (body.contract.published_service_ceiling
                        != boundary.published_service_ceiling
                        || declarations.has_authored_reach(realization)))
        };
        if unrepresented_reach
            || !invocations.is_empty()
            || may_suspend
            || may_block
            || !body.contract.published_synchronous_entries.is_empty()
            || !body.contract.crash_routes.is_empty()
            || body.contract.published_may_block
            || body.contract.published_may_suspend
            || row.calling_signature.is_some()
        {
            return Err(unavailable(
                "the selected provider's complete effect or calling policy",
            ));
        }
        Ok(PackagePolicyProviderRow {
            method: row.method.clone(),
            requirement,
            requirement_application: None,
            realization: Some(self.nominal(realization)?),
            native_build: None,
            requirement_lifetime_partition: row.requirement_lifetime_partition.clone(),
            admitted_same_stack_bytes: row.admitted_same_stack_bytes,
            binding: PackagePolicyProviderBinding::CheckedAdapter {
                machine_identity: callable.to_owned(),
                machine_package_identity,
            },
            compiler_intrinsic_execution: None,
            installation_reach,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn checked_method(
        &self,
        service: &TraitDefinition,
        signature: &StateSignature,
        boundary: &BoundaryMachineDeclaration,
        method: &ServiceMethod,
        schema: &ServiceSchema,
        requirement: PackageReviewNominalIdentity,
        requirement_owner: PackageReviewNominalIdentity,
        declarations: &AcceptedDeclarations<'_>,
    ) -> Result<(PackagePolicyServiceMethod, PackagePolicyCallable), Vec<Diagnostic>> {
        let source = self.typed();
        let declared_service = source
            .service_reaches
            .id_for_symbol(service.symbol)
            .and_then(|service| source.service_reaches.definition(service))
            .ok_or_else(|| unavailable("the selected requirement's own service"))?;
        let mut authority = vec![requirement_owner.clone()];
        let ceiling_matches = if signature.service_reach_is_installation_bound {
            authority.extend(reviewed_services(
                declarations.service_names,
                &boundary.published_service_ceiling,
            )?);
            authority.sort();
            authority.dedup();
            let mut names = authority
                .iter()
                .map(PackageReviewNominalIdentity::path)
                .collect::<Vec<_>>();
            names.sort_unstable();
            names
                == method
                    .service_reach
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
        } else {
            method.service_reach.as_slice() == [declared_service.name.as_str()]
                && boundary
                    .published_service_ceiling
                    .iter()
                    .all(|id| Some(*id) == boundary.owning_service)
        };
        if method.completion != boundary.completion
            || method.terminates_guarantee != boundary.unconditional_termination
            || !ceiling_matches
            || !method.entry_claims.is_empty()
            || !method.result_claims.is_empty()
            || !method.synchronous_invocations.is_empty()
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
        let callable =
            self.checked_requirement(signature, boundary, requirement.clone(), declarations)?;
        let selected = PackagePolicyServiceMethod {
            completion: method.completion,
            name: method.name.clone(),
            requirement_owner: requirement_owner.clone(),
            requirement,
            signature: PackagePolicyServiceSignature {
                schema_arguments: Vec::new(),
                schema_lifetime_parameter_count: 0,
                schema_lifetime_arguments: Vec::new(),
                application_lifetime_parameter_count: 0,
                requirement_arguments: Vec::new(),
                requirement_lifetime_arguments: Vec::new(),
                requirement_lifetime_parameter_count: 0,
                static_parameters: Vec::new(),
                parameters: callable.parameters.clone(),
                result: callable.return_type.clone(),
            },
            authority: PackagePolicyServiceAuthority {
                service_reach: authority,
                synchronous_invocations: Vec::new(),
                progress_premises: Vec::new(),
            },
            parameter_count: method.parameter_count,
            parameter_type_identities: method
                .parameter_type_identities
                .iter()
                .map(|identity| schema.identity_text(*identity).to_owned())
                .collect(),
            entry_claims: Vec::new(),
            has_result: method.has_result,
            result_type_identity: method
                .has_result
                .then(|| schema.identity_text(method.result_type_identity).to_owned()),
            result_claims: Vec::new(),
            service_reach: method.service_reach.clone(),
            synchronous_invocations: Vec::new(),
            may_suspend: method.may_suspend,
            may_block: method.may_block,
            terminates_guarantee: method.terminates_guarantee,
            termination_premises: Vec::new(),
            identities: schema.declaration.identities.clone(),
            calling: Vec::new(),
        };
        Ok((selected, callable))
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
        if !signature.lifetime_parameters.is_empty()
            || !signature.type_parameters.is_empty()
            || !signature.conformance_bounds.is_empty()
            || signature.spelling.is_some()
            || signature.is_default
            || !signature.native_callback_parameters.is_empty()
            || !source.state_signature_contracts(signature).is_empty()
            || !signature.declares_no_crash_route
            || !boundary.crash_routes.is_empty()
            || boundary.role != BoundaryDeclarationRole::Callable
            || declaration.execution != CheckedBoundaryExecution::Provider
            || !declaration.method_arguments.is_empty()
            || !boundary.published_synchronous_entries.is_empty()
            || boundary.structural_parameters.iter().any(|parameter| {
                !parameter.is_unqualified_root()
                    || !parameter.additional_input_qualifications.is_empty()
            })
            || !boundary.requires.is_empty()
            || !boundary.normal_qualifications.is_empty()
            || !boundary.scalar_domain_requirements.is_empty()
            || !boundary.scalar_requires.is_empty()
            || !boundary.scalar_ensures.is_empty()
            || !boundary.reference_outputs.is_empty()
            || !boundary.program_local_root_introductions.is_empty()
            || !boundary.content_guarantees.is_empty()
            || !boundary.result_reference_regions.is_empty()
            || matches!(&boundary.result, terminal_psi::BoundaryMachineResult::Structural(result)
                if result.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
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
        let parameters = source
            .state_signature_parameters(signature)
            .iter()
            .map(|parameter| {
                Ok(PackageReviewSignatureParameter {
                    name: parameter.name.as_str().to_owned(),
                    type_identity: self
                        .type_identity_for_private_contract(parameter.type_reference)?,
                    is_const: parameter.is_const,
                    is_mutable: parameter.is_mutable,
                })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
        let return_type = signature
            .return_type
            .is_valid()
            .then(|| self.type_identity_for_private_contract(signature.return_type))
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
            type_parameters: Vec::new(),
            conformance_bounds: Vec::new(),
            parameters,
            return_type,
            conformances: Vec::new(),
            operator_realizations: Vec::new(),
            requirement_realizations: Vec::new(),
            contracts: Vec::new(),
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
            declared_synchronous_invocations: Some(Vec::new()),
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

fn closed_service(source: &typed_trees::TypedTrees, service: &TraitDefinition) -> bool {
    service.is_boundary
        && service.lifetime_parameters.is_empty()
        && service.conformance_bounds.is_empty()
        && source.trait_type_parameters(service).is_empty()
        && source.trait_requirements(service).is_empty()
        && service.refines.is_none()
        && service.refinement_clauses.is_empty()
}

fn service_trait(
    identity: PackageReviewNominalIdentity,
    requirements: Vec<PackagePolicyTraitRequirement>,
) -> PackagePolicyTraitShape {
    PackagePolicyTraitShape {
        identity,
        is_boundary: true,
        lifetime_parameter_count: 0,
        type_parameters: Vec::new(),
        conformance_bounds: Vec::new(),
        parents: Vec::new(),
        requirements,
    }
}

fn service_requirement(callable: &PackagePolicyCallable) -> PackagePolicyTraitRequirement {
    PackagePolicyTraitRequirement {
        identity: callable.identity.clone(),
        spelling: callable.spelling,
        has_default_realization: false,
        lifetime_parameter_count: callable.lifetime_parameter_count,
        type_parameters: callable.type_parameters.clone(),
        conformance_bounds: callable.conformance_bounds.clone(),
        parameters: callable.parameters.clone(),
        return_type: callable.return_type.clone(),
        contracts: callable.contracts.clone(),
        published_crash: callable.checked_crash.published.clone(),
        service_reach: callable.declared_service_reach.clone().unwrap_or_default(),
        service_reach_is_installation_bound: false,
        synchronous_invocations: callable
            .declared_synchronous_invocations
            .clone()
            .unwrap_or_default(),
        suspends: callable.declared_may_suspend.unwrap_or_default(),
        blocks: callable.declared_may_block.unwrap_or_default(),
        termination: callable.checked_termination.clone(),
        returned_borrow_origins: Vec::new(),
        parameter_qualification_preconditions: Vec::new(),
    }
}
