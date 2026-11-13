/*
Boundary service declarations say which calls a provider promises to implement.
We keep those contracts independent of target calling plans, retaining each exact
typed declaration and application for later provider selection.
*/

pub mod provider_declarations {
    use diagnostics::Diagnostic;
    use language_semantics::direct_calls::DirectCallDeclarations;
    use language_semantics::provider_declarations::SelectedTargetMachineOrigin;
    use language_semantics::trust_declarations::TrustDeclarations;
    use std::sync::{Arc, OnceLock};
    use symbols::SymbolHandle;

    pub fn closed_provider_requirements(
        typed: &crate::TypedTrees,
        template: &crate::machine::Machine,
    ) -> Vec<crate::type_identity::CallableIdentityId> {
        typed
            .machine_specializations
            .iter()
            .filter(|receipt| receipt.template == template.symbol)
            .filter_map(|receipt| typed.machine(receipt.instance))
            .filter(|instance| {
                instance.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
            })
            .map(|instance| typed.machine_callable_identity(instance))
            .filter(|identity| identity.is_valid())
            .collect()
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct NamedProviderApplication {
        pub declaration: symbols::SymbolHandle,
        pub capture: arena::Handle<crate::CapturedMachineApplication>,
    }

    pub enum ProviderApplicationView {
        None,
        PhysicalLeaf,
        External,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ProviderDeclarationOrigin {
        Authored(symbols::SymbolHandle),
        BuildNative {
            requirement: symbols::SymbolHandle,
            application: Option<arena::Handle<crate::product_operand::ProductOperandApplication>>,
        },
        Toolchain,
    }

    pub struct ProviderDeclarationRow<'identity> {
        pub requirement: symbols::SymbolHandle,
        pub origin: ProviderDeclarationOrigin,
        pub requirement_identity: &'identity str,
        pub declared_requirement_identity: &'identity str,
        pub application: ProviderApplicationView,
    }

    pub struct ProviderDeclarationRequest<'rows> {
        pub rows: &'rows [ProviderDeclarationRow<'rows>],
        pub named_application: Option<NamedProviderApplication>,
        pub provider_application_identity: Option<&'rows str>,
        pub installation_reach: bool,
    }

    pub fn closed_provider_leaf_applications(
        typed: &crate::TypedTrees,
        template: &str,
    ) -> Result<
        Vec<language_semantics::provider_declarations::ProviderRequirementApplication>,
        Vec<String>,
    > {
        let mut identities = Vec::new();
        let mut diagnostics = Vec::new();
        for application in typed
            .closed_requirement_applications()
            .filter(|application| {
                typed
                    .trait_machine_signature(application.template)
                    .is_some_and(|(owner, signature)| {
                        typed.trait_requirement_callable_identity_text(owner, signature) == template
                    })
            })
        {
            let Some(owner) = typed.trait_definition_by_symbol(application.declaring_trait) else {
                diagnostics.push("closed requirement application lost its declaring trait".into());
                continue;
            };
            let identity =
                typed.trait_requirement_callable_identity_text(owner, application.signature);
            if !identity.is_empty() && identity != template {
                let Some(receiving_application) = typed
                    .type_identity_text(application.portable_trait_application_identity)
                    .filter(|text| !text.is_empty())
                else {
                    diagnostics
                        .push("provider receiving application has no portable identity".into());
                    continue;
                };
                identities.push(
                    language_semantics::provider_declarations::ProviderRequirementApplication {
                        requirement: identity,
                        receiving_application,
                    },
                );
            }
        }
        if diagnostics.is_empty() {
            Ok(identities)
        } else {
            Err(diagnostics)
        }
    }

    pub fn requirement_reach(
        typed: &crate::TypedTrees,
        symbol: symbols::SymbolHandle,
    ) -> Option<(String, bool, language_semantics::ServiceReachRowId)> {
        if let Some((owner, requirement)) = typed.trait_machine_signature(symbol) {
            return Some((
                typed.trait_requirement_callable_identity_text(owner, requirement),
                requirement.service_reach_is_installation_bound,
                requirement.service_reach_row,
            ));
        }
        let requirement = typed.machine_by_symbol(symbol)?;
        if requirement.supply_mode != language_semantics::MachineSupplyMode::TopLevelRequirement {
            return None;
        }
        Some((
            typed.machine_callable_identity_text(requirement)?,
            requirement.service_reach_is_installation_bound,
            requirement.service_reach_row,
        ))
    }
    pub(crate) mod context {
        use crate::TypedTrees;
        use source::DependencyScope;
        use symbols::SymbolHandle;

        #[derive(Copy, Clone)]
        pub struct ProviderPlanningContext<'p> {
            pub typed: &'p TypedTrees,
            pub scope: DependencyScope,
        }

        impl<'p> ProviderPlanningContext<'p> {
            pub const fn new(typed: &'p TypedTrees, scope: DependencyScope) -> Self {
                Self { typed, scope }
            }

            pub fn contains(self, symbol: SymbolHandle) -> bool {
                self.typed
                    .symbols
                    .symbol_is_in_dependency_scope(symbol, self.scope)
            }
        }
    }
    pub use context::ProviderPlanningContext;

    /// Declaration catalogs share the exact immutable source owner. Preparing
    /// them grants no checking or admission authority: the product factory must
    /// separately retain the route's acceptance of that same source.
    pub struct PreparedProviderInventory<Owner: AsRef<crate::TypedTrees> = crate::FrozenTypedTrees> {
        owner: Arc<Owner>,
        origins: OnceLock<Vec<SelectedTargetMachineOrigin>>,
        direct_calls: OnceLock<Arc<DirectCallDeclarations<SymbolHandle>>>,
        trust: OnceLock<Result<TrustDeclarations, Diagnostic>>,
        services: Arc<super::PreparedServiceCatalog>,
    }

    impl<Owner: AsRef<crate::TypedTrees>> PreparedProviderInventory<Owner> {
        pub fn new(
            owner: Arc<Owner>,
            exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
        ) -> Self {
            let services = super::PreparedServiceCatalog::prepare(
                ProviderPlanningContext::new(
                    owner.as_ref().as_ref(),
                    source::DependencyScope::Product,
                ),
                exact_toolchain_sources,
            );
            Self {
                owner,
                origins: OnceLock::new(),
                direct_calls: OnceLock::new(),
                trust: OnceLock::new(),
                services: Arc::new(services),
            }
        }

        pub fn program(&self) -> &Arc<Owner> {
            &self.owner
        }

        pub fn retained_service_catalog(&self) -> Arc<super::PreparedServiceCatalog> {
            Arc::clone(&self.services)
        }

        pub fn service_declarations(
            &self,
        ) -> impl Iterator<Item = super::PreparedServiceDeclaration<'_>> {
            self.services.declared_traits()
        }

        pub fn services_for(&self, owner: &Owner) -> Result<super::ServiceCatalog<'_>, Diagnostic> {
            self.require_owner(owner)?;
            self.services.bind(ProviderPlanningContext::new(
                self.owner.as_ref().as_ref(),
                source::DependencyScope::Product,
            ))
        }

        pub fn direct_calls_for(
            &self,
            owner: &Owner,
        ) -> Result<&DirectCallDeclarations<SymbolHandle>, Diagnostic> {
            self.require_owner(owner)?;
            Ok(self.direct_calls())
        }

        pub fn direct_calls(&self) -> &DirectCallDeclarations<SymbolHandle> {
            self.direct_call_declarations()
        }

        pub fn retained_direct_calls_for(
            &self,
            owner: &Owner,
        ) -> Result<Arc<DirectCallDeclarations<SymbolHandle>>, Diagnostic> {
            self.require_owner(owner)?;
            Ok(Arc::clone(self.direct_call_declarations()))
        }

        fn direct_call_declarations(&self) -> &Arc<DirectCallDeclarations<SymbolHandle>> {
            self.direct_calls.get_or_init(|| {
                Arc::new(crate::direct_calls::collect(
                    self.owner.as_ref().as_ref(),
                    source::DependencyScope::Product,
                ))
            })
        }

        pub fn origins_for(
            &self,
            owner: &Owner,
        ) -> Result<&[SelectedTargetMachineOrigin], Diagnostic> {
            self.require_owner(owner)?;
            Ok(self.origins())
        }

        pub fn origins(&self) -> &[SelectedTargetMachineOrigin] {
            self.origins.get_or_init(|| {
                provider_target_declarations(
                    self.owner.as_ref().as_ref(),
                    source::DependencyScope::Product,
                )
            })
        }

        fn require_owner(&self, owner: &Owner) -> Result<(), Diagnostic> {
            if !std::ptr::eq(self.owner.as_ref(), owner) {
                return Err(Diagnostic::error(
                    "provider declarations belong to another source program",
                ));
            }
            Ok(())
        }

        /// The old checker's template and contract plans are not route evidence.
        /// Keep the complete source census and refuse when an admitted machine
        /// needs trust declarations the route does not yet publish. An empty
        /// census requires no such custody and makes no admission claim.
        pub fn trust_declarations(&self) -> Result<&TrustDeclarations, Diagnostic> {
            self.trust
                .get_or_init(|| {
                    let claims = self
                        .owner
                        .as_ref()
                        .as_ref()
                        .machines()
                        .iter()
                        .filter(|machine| {
                            machine.supply_mode
                                == language_semantics::MachineSupplyMode::AdmissionClaim
                        })
                        .map(|machine| machine.name.as_str())
                        .collect::<Vec<_>>();
                    if claims.is_empty() {
                        Ok(TrustDeclarations::default())
                    } else {
                        Err(Diagnostic::error(format!(
                            "no product route yet: admitted machines require route-owned \
                             template and contract trust declarations: {}",
                            claims.join(", ")
                        )))
                    }
                })
                .as_ref()
                .map_err(Clone::clone)
        }
    }

    pub fn independent_satisfactions<'program>(
        typed: &'program crate::TypedTrees,
        machine: &'program crate::machine::Machine,
    ) -> impl Iterator<Item = &'program crate::machine::TraitConformance> + 'program {
        let repeated = owned_requirement_clause(typed, machine);
        typed
            .machine_trait_conformances(machine)
            .iter()
            .enumerate()
            .filter(move |(ordinal, _)| Some(*ordinal) != repeated)
            .map(|(_, edge)| edge)
    }

    fn owned_requirement_clause(
        typed: &crate::TypedTrees,
        machine: &crate::machine::Machine,
    ) -> Option<usize> {
        use crate::trait_definition::{AppliedTraitScope, ConformanceRowSource};

        let (implementation, row) = typed.owned_conformance_realization(machine.symbol)?;
        if row.source != ConformanceRowSource::Inline {
            return None;
        }
        let expected = AppliedTraitScope::from_conformance(typed, implementation)?
            .declaring_scope(typed, row.declaring_trait)?;
        let mut matching = typed
            .machine_trait_conformances(machine)
            .iter()
            .enumerate()
            .filter(|(_, edge)| {
                edge.requirement_symbol == row.requirement
                    && AppliedTraitScope::from_machine_conformance(typed, machine, edge)
                        .is_some_and(|actual| expected.same_application(typed, &actual))
            });
        let (ordinal, _) = matching.next()?;
        matching.next().is_none().then_some(ordinal)
    }

    pub fn provider_target_declarations(
        typed: &crate::TypedTrees,
        scope: source::DependencyScope,
    ) -> Vec<language_semantics::provider_declarations::SelectedTargetMachineOrigin> {
        typed
            .machines()
            .iter()
            .filter(|machine| {
                typed
                    .symbols
                    .symbol_is_in_dependency_scope(machine.symbol, scope)
                    && (scope == source::DependencyScope::Product || machine.target.is_none())
            })
            .filter_map(|machine| {
                machine.authored_target.as_ref().map(|target| {
                    language_semantics::provider_declarations::SelectedTargetMachineOrigin {
                        machine: machine.symbol,
                        target: target.as_str().to_owned(),
                    }
                })
            })
            .collect()
    }

    pub fn satisfaction_provider_declaration(
        typed: &crate::TypedTrees,
        machine: &crate::machine::Machine,
    ) -> Option<symbols::SymbolHandle> {
        let owner = typed.symbols.get(machine.symbol).declaration_owner;
        if owner.is_valid() && typed.symbols.get(owner).kind == symbols::SymbolKind::Conformance {
            return typed
                .owned_conformance_realization(machine.symbol)
                .map(|(implementation, _)| implementation.symbol);
        }
        let attached = machine.attached_data.as_ref()?;
        let owner = typed.data_definition_by_symbol(machine.attached_data_symbol)?;
        (owner.name == *attached).then_some(owner.symbol)
    }

    pub fn boundary_satisfaction_identity(
        typed: &crate::TypedTrees,
        machine: &crate::machine::Machine,
        edge: &crate::machine::TraitConformance,
    ) -> Option<(symbols::SymbolHandle, String)> {
        let declaration = crate::machine::resolve_satisfied_declaration(typed, machine, edge)?;
        let symbol = declaration.symbol();
        if !edge.requirement_symbol.is_valid() || edge.requirement_symbol != symbol {
            return None;
        }
        let identity = match declaration {
            crate::machine::SatisfiedDeclaration::Trait {
                definition,
                requirement,
            } if definition.is_boundary => {
                typed.trait_requirement_callable_identity_text(definition, requirement)
            }
            crate::machine::SatisfiedDeclaration::TopLevelRequirement(requirement)
                if requirement.supply_mode
                    == language_semantics::MachineSupplyMode::TopLevelRequirement =>
            {
                typed.machine_callable_identity_text(requirement)?
            }
            crate::machine::SatisfiedDeclaration::Operator(operator) if operator.is_boundary => {
                crate::operator::boundary_operator_requirement_identity(typed, operator)
            }
            _ => return None,
        };
        (!identity.is_empty()).then_some((symbol, identity))
    }
    #[cfg(test)]
    mod inventory_tests {
        use super::PreparedProviderInventory;
        use crate::{FrozenTypedTrees, TypedTrees};
        use std::sync::Arc;

        #[test]
        fn catalogs_require_the_exact_immutable_owner_even_for_equal_source() {
            let owner = Arc::new(FrozenTypedTrees::from(TypedTrees::default()));
            let other = Arc::new(owner.as_ref().clone());
            let inventory = PreparedProviderInventory::new(Arc::clone(&owner), &[]);
            assert!(Arc::ptr_eq(inventory.program(), &owner));
            assert!(inventory.services_for(&owner).is_ok());
            assert!(inventory.direct_calls_for(&owner).is_ok());
            assert!(inventory.origins_for(&owner).is_ok());
            assert!(inventory.services_for(&other).is_err());
            assert!(inventory.direct_calls_for(&other).is_err());
            assert!(inventory.retained_direct_calls_for(&other).is_err());
            assert!(inventory.origins_for(&other).is_err());
        }

        #[test]
        fn retained_catalogs_and_empty_trust_are_shared_for_one_owner() {
            let owner = Arc::new(FrozenTypedTrees::from(TypedTrees::default()));
            let inventory = PreparedProviderInventory::new(Arc::clone(&owner), &[]);
            assert!(Arc::ptr_eq(
                &inventory.retained_service_catalog(),
                &inventory.retained_service_catalog()
            ));
            assert!(Arc::ptr_eq(
                &inventory.retained_direct_calls_for(&owner).unwrap(),
                &inventory.retained_direct_calls_for(&owner).unwrap()
            ));
            let trust = inventory.trust_declarations().unwrap();
            assert!(trust.machines.is_empty());
            assert!(std::ptr::eq(trust, inventory.trust_declarations().unwrap()));
        }

        #[test]
        fn admission_claims_cannot_recover_old_checker_trust() {
            let mut symbols = symbols::SymbolTableBuilder::new();
            let first = symbols.insert_root(
                symbols::SymbolKind::Machine,
                symbols::SymbolNameRef::Static("First"),
            );
            let second = symbols.insert_root(
                symbols::SymbolKind::Machine,
                symbols::SymbolNameRef::Static("Second"),
            );
            let mut typed = TypedTrees {
                symbols: symbols.finish(),
                ..Default::default()
            };
            for (symbol, name) in [(first, "First"), (second, "Second")] {
                typed.push_machine(crate::machine::Machine {
                    symbol,
                    name: name.into(),
                    supply_mode: language_semantics::MachineSupplyMode::AdmissionClaim,
                    ..Default::default()
                });
            }
            let inventory =
                PreparedProviderInventory::new(Arc::new(FrozenTypedTrees::from(typed)), &[]);
            let error = inventory.trust_declarations().unwrap_err();
            assert!(error.message.contains("no product route yet"));
            assert!(error.message.contains("First, Second"));
        }
    }
}

pub mod projection {
    pub(crate) mod applied_receivers {
        use crate::trait_definition::AppliedTraitScope;
        use language_semantics::service_surface::ServiceReceiverApplication;
        use language_semantics::service_surface::identities::ProviderIdentityTable;

        pub(crate) fn collect_scopes<'program>(
            program: &'program crate::TypedTrees,
            root: AppliedTraitScope<'program>,
        ) -> Option<Vec<AppliedTraitScope<'program>>> {
            let mut scopes: Vec<AppliedTraitScope<'_>> = Vec::new();
            let mut ambiguous = false;
            root.visit(program, &mut |scope| {
                if let Some(existing) = scopes
                    .iter()
                    .find(|existing| existing.declaration.symbol == scope.declaration.symbol)
                {
                    ambiguous |= !existing.same_application(program, scope);
                } else {
                    scopes.push(scope.clone());
                }
            });
            (!ambiguous).then_some(scopes)
        }

        pub(crate) fn receiver_applications(
            program: &crate::TypedTrees,
            scopes: &[AppliedTraitScope<'_>],
            identities: &mut ProviderIdentityTable,
        ) -> Option<Vec<ServiceReceiverApplication>> {
            let mut applications = Vec::new();
            for scope in scopes.iter().filter(|scope| {
                scope.declaration.is_boundary
                    || !program
                        .trait_machine_signatures(scope.declaration)
                        .is_empty()
            }) {
                let mut argument_identities = Vec::new();
                for parameter in program.trait_type_parameters(scope.declaration) {
                    let Some(identity) =
                        scope.portable_parameter_identity(program, parameter.symbol)
                    else {
                        return Some(Vec::new());
                    };
                    argument_identities
                        .push(identities.intern(&program.type_identity_text(identity)?));
                }
                let Some(lifetimes) = scope.portable_lifetime_arguments(program) else {
                    return Some(Vec::new());
                };
                let lifetime_argument_identities = lifetimes
                    .iter()
                    .map(|identity| {
                        Some(identities.intern(&program.type_identity_text(*identity)?))
                    })
                    .collect::<Option<Vec<_>>>()?;
                let mut requirement_identities = Vec::new();
                scope.visit(program, &mut |declaring| {
                    for signature in program.trait_machine_signatures(declaring.declaration) {
                        let identity =
                            identities.intern(&program.trait_requirement_callable_identity_text(
                                declaring.declaration,
                                signature,
                            ));
                        if !requirement_identities.contains(&identity) {
                            requirement_identities.push(identity);
                        }
                    }
                });
                requirement_identities
                    .sort_by(|left, right| identities.text(*left).cmp(identities.text(*right)));
                let application = scope
                    .portable_application_identity(program)
                    .and_then(|identity| program.type_identity_text(identity))?;
                applications.push(ServiceReceiverApplication {
                    application: identities.intern(&application),
                    trait_name: identities
                        .intern(&program.trait_declaration_path(scope.declaration)),
                    trait_package_identity: program
                        .symbols
                        .symbol_package_identity(scope.declaration.symbol),
                    argument_identities,
                    lifetime_argument_identities,
                    requirement_identities,
                });
            }
            if let Some(ancestors) = applications.get_mut(1..) {
                ancestors.sort_by(|left, right| {
                    identities
                        .text(left.trait_name)
                        .cmp(identities.text(right.trait_name))
                        .then_with(|| {
                            left.trait_package_identity
                                .cmp(&right.trait_package_identity)
                        })
                });
            }
            Some(applications)
        }
    }
    pub(crate) mod correspondence {
        use super::{ProjectedServiceSurface, ProviderSchemaDeclaration};
        use crate::TypedTrees;
        use diagnostics::Diagnostic;

        pub(crate) fn validate(
            program: &TypedTrees,
            projection: &ProjectedServiceSurface,
        ) -> Result<(), Diagnostic> {
            if projection.declaration.methods.len() != projection.requirements.len() {
                return Err(Diagnostic::error(
                    "service row lost its ordered source correspondence",
                ));
            }
            if !matches!(
                projection.schema,
                ProviderSchemaDeclaration::BoundaryTrait(_)
            ) {
                return Ok(());
            }
            let mut pending = vec![projection.schema.symbol()];
            let mut visited = Vec::new();
            let mut declarations = Vec::new();
            while let Some(symbol) = pending.pop() {
                if visited.contains(&symbol) {
                    continue;
                }
                visited.push(symbol);
                let mut matches = program
                    .traits()
                    .iter()
                    .filter(|declaration| declaration.symbol == symbol);
                let Some(declaration) = matches.next() else {
                    return Err(Diagnostic::error(
                        "service inheritance has no exact declaration",
                    ));
                };
                if matches.next().is_some() {
                    return Err(Diagnostic::error(
                        "service inheritance has ambiguous declarations",
                    ));
                }
                declarations.push(declaration);
                pending.extend(
                    program
                        .trait_requirements(declaration)
                        .iter()
                        .map(|parent| parent.symbol),
                );
            }
            for (method, source) in projection
                .declaration
                .methods
                .iter()
                .zip(&projection.requirements)
            {
                let owner = program
                    .trait_definition_by_symbol(source.declaring_trait)
                    .ok_or_else(|| {
                        Diagnostic::error("service row lost its exact declaring trait")
                    })?;
                let signature = program
                    .trait_machine_signature_by_symbol(owner, source.symbol)
                    .ok_or_else(|| {
                        Diagnostic::error("service row lost its exact declared requirement")
                    })?;
                let identity = program.trait_requirement_callable_identity(owner, signature);
                let mut matches = declarations.iter().flat_map(|declaration| {
                    program
                        .trait_machine_signatures(declaration)
                        .iter()
                        .filter_map(move |candidate| {
                            (program.symbols.symbol_package_identity(declaration.symbol)
                                == method.requirement_owner_package_identity
                                && program.symbols.symbol_package_identity(candidate.symbol)
                                    == method.requirement_owner_package_identity
                                && program
                                    .trait_requirement_callable_identity(declaration, candidate)
                                    == identity)
                                .then_some(candidate.symbol)
                        })
                });
                if matches.next() != Some(source.symbol) || matches.next().is_some() {
                    return Err(Diagnostic::error(
                        "service row has no unique exact inherited requirement",
                    ));
                }
            }
            Ok(())
        }
    }

    use super::ProviderSchemaDeclaration;
    use crate::boundary_completions::boundary_requirement_completion;
    use crate::operator::boundary_operator_requirement_identity;
    use crate::types::DomainConstraint;
    use language_semantics::service_surface::identities::ProviderIdentityTable;
    use std::sync::Arc;

    pub struct ProjectedServiceSurface<'program> {
        program: &'program crate::TypedTrees,
        pub declaration: Arc<ServiceSurface>,
        pub schema: ProviderSchemaDeclaration,
        pub argument_owner: symbols::SymbolHandle,
        pub requirements: Vec<ServiceRequirementSource>,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub struct ServiceRequirementSource {
        pub declaring_trait: symbols::SymbolHandle,
        pub symbol: symbols::SymbolHandle,
    }

    impl ProjectedServiceSurface<'_> {
        pub(crate) fn validate_correspondence(&self) -> Result<(), diagnostics::Diagnostic> {
            correspondence::validate(self.program, self)
        }
    }

    use crate::provider_declarations::ProviderPlanningContext;
    use crate::trait_definition::{AppliedTraitScope, AppliedTypeReference};
    use language_semantics::service_surface::{
        ServiceEntryAuthorityFlow, ServiceEntryClaim, ServiceMethod,
        ServiceProgressEstablishmentRoute, ServiceProgressEstablishmentRouteKind,
        ServiceProgressPremise, ServiceProgressSubject, ServiceResultClaim, ServiceSurface,
    };

    pub fn is_product_declaration(
        program: &crate::TypedTrees,
        symbol: symbols::SymbolHandle,
    ) -> bool {
        ProviderPlanningContext::new(program, source::DependencyScope::Product).contains(symbol)
    }

    pub fn from_typed<'program>(
        context: ProviderPlanningContext<'program>,
        trait_definition: &'program crate::trait_definition::TraitDefinition,
    ) -> Option<ProjectedServiceSurface<'program>> {
        from_typed_application(context, AppliedTraitScope::abstract_scope(trait_definition))
    }

    pub fn from_typed_application<'program>(
        context: ProviderPlanningContext<'program>,
        root: AppliedTraitScope<'program>,
    ) -> Option<ProjectedServiceSurface<'program>> {
        let program = context.typed;
        let trait_definition = root.declaration;
        if !trait_definition.is_boundary || !context.contains(trait_definition.symbol) {
            return None;
        }
        let mut methods = Vec::new();
        let mut requirements = Vec::new();
        let mut identities = ProviderIdentityTable::default();
        let argument_owner = if program.trait_type_parameters(trait_definition).is_empty() {
            trait_definition.symbol
        } else {
            root.argument_owner
        };
        let scopes = applied_receivers::collect_scopes(program, root)?;
        for scope in scopes.iter().rev() {
            collect_service_methods(
                program,
                scope,
                &mut requirements,
                &mut methods,
                &mut identities,
            );
        }
        let receiver_applications =
            applied_receivers::receiver_applications(program, &scopes, &mut identities)?;
        Some(ProjectedServiceSurface {
            program,
            schema: ProviderSchemaDeclaration::BoundaryTrait(trait_definition.symbol),
            argument_owner,
            requirements,
            declaration: Arc::new(ServiceSurface {
                trait_name: identities.intern(&program.trait_declaration_path(trait_definition)),
                trait_package_identity: program
                    .symbols
                    .symbol_package_identity(trait_definition.symbol),
                identities,
                receiver_applications,
                methods,
            }),
        })
    }

    pub fn from_typed_operator<'program>(
        context: ProviderPlanningContext<'program>,
        operator: &crate::operator::OperatorDefinition,
    ) -> Option<ProjectedServiceSurface<'program>> {
        let program = context.typed;
        if !context.contains(operator.symbol) || !operator.is_boundary {
            return None;
        }
        let mut identities = ProviderIdentityTable::default();
        let requirement_identity =
            identities.intern(&boundary_operator_requirement_identity(program, operator));
        Some(ProjectedServiceSurface {
            program,
            schema: ProviderSchemaDeclaration::BoundaryOperator(operator.symbol),
            argument_owner: operator.symbol,
            requirements: vec![ServiceRequirementSource {
                declaring_trait: symbols::SymbolHandle::invalid(),
                symbol: operator.symbol,
            }],
            declaration: Arc::new(ServiceSurface {
                trait_name: requirement_identity,
                receiver_applications: Vec::new(),
                trait_package_identity: program.symbols.symbol_package_identity(operator.symbol),
                methods: vec![ServiceMethod {
                    completion: boundary_requirement_completion(program, operator.symbol),
                    name: "realize".to_owned(),
                    requirement_owner: boundary_operator_requirement_identity(program, operator),
                    requirement_owner_package_identity: program
                        .symbols
                        .symbol_package_identity(operator.symbol),
                    requirement_identity,
                    parameter_count: program.operator_parameters(operator).len(),
                    parameter_type_identities: program
                        .operator_parameters(operator)
                        .iter()
                        .map(|parameter| {
                            identities.intern(
                                &program.normalized_type_identity_text(parameter.type_reference),
                            )
                        })
                        .collect(),
                    entry_claims: Vec::new(),
                    has_result: operator.return_type.is_valid(),
                    result_type_identity: operator
                        .return_type
                        .is_valid()
                        .then(|| {
                            identities.intern(
                                &program.normalized_type_identity_text(operator.return_type),
                            )
                        })
                        .unwrap_or_default(),
                    result_claims: Vec::new(),
                    service_reach: Vec::new(),
                    synchronous_invocations: Vec::new(),
                    may_suspend: false,
                    may_block: false,
                    terminates_guarantee: false,
                    termination_premises: Vec::new(),
                }],
                identities,
            }),
        })
    }

    pub fn from_typed_boundary_requirement<'program>(
        context: ProviderPlanningContext<'program>,
        requirement: &crate::machine::Machine,
    ) -> Option<ProjectedServiceSurface<'program>> {
        if !context.contains(requirement.symbol)
            || !requirement.symbol.is_valid()
            || requirement.supply_mode != language_semantics::MachineSupplyMode::TopLevelRequirement
            || requirement.body_is_present
        {
            return None;
        }
        project_boundary_machine_schema(
            context,
            requirement,
            ProviderSchemaDeclaration::BoundaryRequirement(requirement.symbol),
        )
    }

    pub fn project_boundary_machine_schema<'program>(
        context: ProviderPlanningContext<'program>,
        requirement: &crate::machine::Machine,
        schema: ProviderSchemaDeclaration,
    ) -> Option<ProjectedServiceSurface<'program>> {
        let program = context.typed;
        let [entry] = program.machine_states(requirement) else {
            return None;
        };
        let requirement_path = program.symbols.display_path(requirement.symbol, "::");
        let (requirement_owner, method_name) = requirement_path.rsplit_once("::")?;
        if requirement_owner.is_empty() || method_name.is_empty() {
            return None;
        }
        let requirement_identity = program.machine_callable_identity_text(requirement)?;
        let parameters = program.state_parameters(entry);
        let published_termination = requirement.termination_plan.interface.published()?;
        let package_identity = program.symbols.symbol_package_identity(requirement.symbol);
        let mut identities = ProviderIdentityTable::default();
        let requirement_identity_id = identities.intern(&requirement_identity);
        let termination_premises = service_progress_premises_for_parameters(
            program,
            published_termination,
            parameters,
            &mut identities,
        );
        Some(ProjectedServiceSurface {
            program,
            schema,
            argument_owner: requirement.symbol,
            requirements: vec![ServiceRequirementSource {
                declaring_trait: symbols::SymbolHandle::invalid(),
                symbol: requirement.symbol,
            }],
            declaration: Arc::new(ServiceSurface {
                trait_name: if requirement.spelling.is_some() {
                    requirement_identity_id
                } else {
                    identities.intern(&requirement_path)
                },
                trait_package_identity: package_identity,
                receiver_applications: Vec::new(),
                methods: vec![ServiceMethod {
                    completion: boundary_requirement_completion(program, requirement.symbol),
                    name: method_name.to_owned(),
                    requirement_owner: requirement_owner.to_owned(),
                    requirement_owner_package_identity: package_identity,
                    requirement_identity: requirement_identity_id,
                    parameter_count: parameters.len(),
                    parameter_type_identities: parameters
                        .iter()
                        .map(|parameter| {
                            identities.intern(
                                &program.normalized_type_identity_text(parameter.type_reference),
                            )
                        })
                        .collect(),
                    entry_claims: Vec::new(),
                    has_result: entry.return_type.is_valid(),
                    result_type_identity: entry
                        .return_type
                        .is_valid()
                        .then(|| {
                            identities
                                .intern(&program.normalized_type_identity_text(entry.return_type))
                        })
                        .unwrap_or_default(),
                    result_claims: Vec::new(),
                    service_reach: service_reach_row_names(program, requirement.service_reach_row),
                    synchronous_invocations: machine_synchronous_invocation_names(
                        program,
                        requirement,
                    ),
                    may_suspend: requirement.suspends,
                    may_block: requirement.blocks,
                    terminates_guarantee: published_termination.promises_termination(),
                    termination_premises,
                }],
                identities,
            }),
        })
    }

    fn collect_service_methods(
        program: &crate::TypedTrees,
        scope: &AppliedTraitScope<'_>,
        requirements: &mut Vec<ServiceRequirementSource>,
        methods: &mut Vec<ServiceMethod>,
        identities: &mut ProviderIdentityTable,
    ) {
        let trait_definition = scope.declaration;
        for signature in program.trait_machine_signatures(trait_definition) {
            let requirement_identity = identities.intern(
                &program.trait_requirement_callable_identity_text(trait_definition, signature),
            );
            if methods
                .iter()
                .any(|method| method.requirement_identity == requirement_identity)
            {
                continue;
            }
            requirements.push(ServiceRequirementSource {
                declaring_trait: trait_definition.symbol,
                symbol: signature.symbol,
            });
            methods.push(ServiceMethod {
                completion: boundary_requirement_completion(program, signature.symbol),
                name: signature.name.as_str().to_owned(),
                requirement_owner: program.trait_declaration_path(trait_definition),
                requirement_owner_package_identity: program
                    .symbols
                    .symbol_package_identity(trait_definition.symbol),
                requirement_identity,
                parameter_count: program.state_signature_parameters(signature).iter().count(),
                parameter_type_identities: program
                    .state_signature_parameters(signature)
                    .iter()
                    .map(|parameter| {
                        identities.intern(
                            &program
                                .type_identity_text(
                                    scope.type_identity(program, parameter.type_reference),
                                )
                                .unwrap_or_default(),
                        )
                    })
                    .collect(),
                entry_claims: service_entry_claims(program, scope, signature, identities),
                has_result: signature.return_type.is_valid(),
                result_type_identity: signature
                    .return_type
                    .is_valid()
                    .then(|| {
                        identities.intern(
                            &program
                                .type_identity_text(
                                    scope.type_identity(program, signature.return_type),
                                )
                                .unwrap_or_default(),
                        )
                    })
                    .unwrap_or_default(),
                result_claims: service_result_claims(program, scope, signature),
                service_reach: service_reach_names(program, trait_definition, signature),
                synchronous_invocations: synchronous_invocation_names(program, signature),
                may_suspend: signature.suspends,
                may_block: signature.blocks,
                terminates_guarantee: signature.termination_guarantee.promises_termination(),
                termination_premises: service_progress_premises(program, signature, identities),
            });
        }
    }

    fn service_progress_premises(
        program: &crate::TypedTrees,
        signature: &crate::signature::StateSignature,
        identities: &mut ProviderIdentityTable,
    ) -> Vec<ServiceProgressPremise> {
        service_progress_premises_for_parameters(
            program,
            &signature.termination_guarantee,
            program.state_signature_parameters(signature),
            identities,
        )
    }

    fn service_progress_premises_for_parameters(
        program: &crate::TypedTrees,
        guarantee: &language_semantics::TerminationGuarantee,
        parameters: &[crate::signature::StateParameter],
        identities: &mut ProviderIdentityTable,
    ) -> Vec<ServiceProgressPremise> {
        let language_semantics::TerminationGuarantee::Terminates { premises } = guarantee else {
            return Vec::new();
        };
        premises
            .iter()
            .map(|premise| {
                let position =
                    crate::signature::state_parameter_position(parameters, premise.subject.root)
                        .expect("normalized public progress premise must be parameter-rooted");
                let subject = ServiceProgressSubject::Parameter(position);
                let profile = program
                    .semantic_domains
                    .name(premise.profile)
                    .expect("normalized progress premise must name a registered profile")
                    .to_owned();
                ServiceProgressPremise {
                    profile,
                    subject,
                    subject_projections: premise
                        .subject
                        .projections
                        .iter()
                        .map(|symbol| program.symbols.display_path(*symbol, "::"))
                        .collect(),
                    establishment_routes: service_progress_establishment_routes(
                        program,
                        premise.profile,
                        identities,
                    ),
                }
            })
            .collect()
    }

    fn service_progress_establishment_routes(
        program: &crate::TypedTrees,
        profile: language_semantics::SemanticDomainId,
        identities: &mut ProviderIdentityTable,
    ) -> Vec<ServiceProgressEstablishmentRoute> {
        let domain = program
            .domain_definitions()
            .iter()
            .find(|domain| domain.semantic_id == profile)
            .expect("normalized progress premise must name one declared profile domain");
        debug_assert_eq!(
            domain.classification,
            Some(language_semantics::DomainClassification::ProgressProfile)
        );
        let mut routes = domain
            .establishment_routes
            .iter()
            .filter(|route| {
                !matches!(
                    route,
                    language_semantics::DomainEstablishmentRoute::ExactMachine { .. }
                )
            })
            .map(|route| {
                let owner = program
                    .trait_definition_by_symbol(route.source_symbol())
                    .expect("normalized progress establishment route must name one trait owner");
                let requirement = program
                    .trait_machine_signature_by_symbol(owner, route.requirement_symbol())
                    .expect("normalized progress establishment route must name one requirement");
                let kind = match route {
                    language_semantics::DomainEstablishmentRoute::CheckedRequirement { .. } => {
                        ServiceProgressEstablishmentRouteKind::CheckedRequirement
                    }
                    language_semantics::DomainEstablishmentRoute::BoundaryRequirement {
                        ..
                    } => ServiceProgressEstablishmentRouteKind::BoundaryRequirement,
                    language_semantics::DomainEstablishmentRoute::ExactMachine { .. } => {
                        unreachable!("exact-machine routes are filtered above")
                    }
                };
                ServiceProgressEstablishmentRoute {
                    kind,
                    requirement_identity: identities.intern(
                        &program.trait_requirement_callable_identity_text(owner, requirement),
                    ),
                }
            })
            .collect::<Vec<_>>();
        routes.sort_by(|left, right| {
            left.kind.cmp(&right.kind).then_with(|| {
                identities
                    .text(left.requirement_identity)
                    .cmp(identities.text(right.requirement_identity))
            })
        });
        routes.dedup();
        routes
    }

    fn service_entry_claims(
        program: &crate::TypedTrees,
        scope: &AppliedTraitScope<'_>,
        signature: &crate::signature::StateSignature,
        identities: &mut ProviderIdentityTable,
    ) -> Vec<ServiceEntryClaim> {
        let mut claims = Vec::new();
        for (parameter_index, parameter) in program
            .state_signature_parameters(signature)
            .iter()
            .enumerate()
        {
            let reference = scope.type_reference(program, parameter.type_reference);
            if reference.multiplicity(program) != language_semantics::Multiplicity::Linear {
                continue;
            }
            append_routed_entry_claims(
                program,
                reference,
                parameter_index,
                scope.declaration.symbol,
                signature.symbol,
                identities,
                &mut claims,
            );
        }
        claims.sort_by(|left, right| {
            left.parameter_index
                .cmp(&right.parameter_index)
                .then_with(|| {
                    identities
                        .text(left.carrier_identity)
                        .cmp(identities.text(right.carrier_identity))
                })
                .then_with(|| left.domain.cmp(&right.domain))
        });
        claims.dedup_by(|left, right| {
            left.parameter_index == right.parameter_index
                && identities.text(left.carrier_identity) == identities.text(right.carrier_identity)
                && left.domain == right.domain
        });
        claims
    }

    fn service_result_claims(
        program: &crate::TypedTrees,
        scope: &AppliedTraitScope<'_>,
        signature: &crate::signature::StateSignature,
    ) -> Vec<ServiceResultClaim> {
        let reference = scope.type_reference(program, signature.return_type);
        if !reference.reference.is_valid()
            || reference.multiplicity(program) != language_semantics::Multiplicity::Linear
        {
            return Vec::new();
        }
        let mut claims = Vec::new();
        append_bodyless_result_claims(
            program,
            reference,
            scope.declaration.symbol,
            signature.symbol,
            &mut claims,
        );
        claims.sort_by(|left, right| left.domain.cmp(&right.domain));
        claims.dedup_by(|left, right| left.domain == right.domain);
        claims
    }

    fn for_each_establishing_domain<'scope, Visitor>(
        program: &crate::TypedTrees,
        reference: AppliedTypeReference<'scope>,
        boundary_trait: symbols::SymbolHandle,
        requirement: symbols::SymbolHandle,
        visit: &mut Visitor,
    ) where
        Visitor: FnMut(AppliedTypeReference<'scope>, &DomainConstraint),
    {
        use crate::types::{TypeConstraintNode, TypeReferenceNode};

        match program.type_reference_node(reference.reference) {
            TypeReferenceNode::Reference { referee, .. } => {
                for_each_establishing_domain(
                    program,
                    reference.child(program, *referee),
                    boundary_trait,
                    requirement,
                    visit,
                );
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let subject = reference.child(program, *base_type);
                for_each_establishing_domain(program, subject, boundary_trait, requirement, visit);
                for constraint in program.type_reference_table.constraints(*constraints) {
                    let TypeConstraintNode::Domain(domain) = constraint else {
                        continue;
                    };
                    if domain.symbol.is_valid() && domain.establishment_routes.iter().any(|route| {
                        matches!(
                            route,
                            language_semantics::DomainEstablishmentRoute::BoundaryRequirement {
                                boundary_trait: route_trait,
                                requirement: route_requirement,
                            } if *route_trait == boundary_trait && *route_requirement == requirement
                        )
                    }) {
                        visit(subject, domain);
                    }
                }
            }
            _ => {}
        }
    }

    fn domain_claim_name(program: &crate::TypedTrees, domain: &DomainConstraint) -> String {
        domain
            .semantic_id
            .is_valid()
            .then(|| program.semantic_domains.name(domain.semantic_id))
            .flatten()
            .unwrap_or_else(|| domain.name.as_str())
            .to_owned()
    }

    fn append_bodyless_result_claims(
        program: &crate::TypedTrees,
        reference: AppliedTypeReference<'_>,
        boundary_trait: symbols::SymbolHandle,
        requirement: symbols::SymbolHandle,
        claims: &mut Vec<ServiceResultClaim>,
    ) {
        for_each_establishing_domain(
            program,
            reference,
            boundary_trait,
            requirement,
            &mut |_, domain| {
                if domain.predicate_presence != language_semantics::DomainPredicatePresence::Absent
                {
                    return;
                }
                claims.push(ServiceResultClaim {
                    domain: domain_claim_name(program, domain),
                    effective_carry: language_semantics::CarryPolicy::STRICT,
                });
            },
        );
    }

    fn append_routed_entry_claims(
        program: &crate::TypedTrees,
        reference: AppliedTypeReference<'_>,
        parameter_index: usize,
        boundary_trait: symbols::SymbolHandle,
        requirement: symbols::SymbolHandle,
        identities: &mut ProviderIdentityTable,
        claims: &mut Vec<ServiceEntryClaim>,
    ) {
        for_each_establishing_domain(
            program,
            reference,
            boundary_trait,
            requirement,
            &mut |subject, domain| {
                claims.push(ServiceEntryClaim {
                    parameter_index,
                    carrier_identity: identities.intern(
                        &program
                            .type_identity_text(subject.type_identity(program))
                            .unwrap_or_default(),
                    ),
                    domain: domain_claim_name(program, domain),
                    predicate_presence: domain.predicate_presence,
                    effective_carry: language_semantics::CarryPolicy::STRICT,
                    authority_flow: ServiceEntryAuthorityFlow::Accepts,
                });
            },
        );
    }

    fn service_reach_names(
        program: &crate::TypedTrees,
        trait_definition: &crate::trait_definition::TraitDefinition,
        signature: &crate::signature::StateSignature,
    ) -> Vec<String> {
        let mut services = program
            .service_reach_rows
            .services(signature.service_reach_row)
            .to_vec();
        if trait_definition.is_boundary
            && let Some(service) = program
                .service_reaches
                .id_for_symbol(trait_definition.symbol)
        {
            program
                .service_reaches
                .extend_closure(service, &mut services);
        }
        services.sort_by_key(|service| service.0);
        services.dedup();
        let mut names = services
            .into_iter()
            .filter_map(|service| program.service_reaches.definition(service))
            .map(|definition| definition.name.clone())
            .collect::<Vec<_>>();
        names.sort_unstable();
        names.dedup();
        names
    }

    fn service_reach_row_names(
        program: &crate::TypedTrees,
        row: language_semantics::ServiceReachRowId,
    ) -> Vec<String> {
        let mut names = program
            .service_reach_rows
            .services(row)
            .iter()
            .filter_map(|service| program.service_reaches.definition(*service))
            .map(|definition| definition.name.clone())
            .collect::<Vec<_>>();
        names.sort_unstable();
        names.dedup();
        names
    }

    fn synchronous_invocation_names(
        program: &crate::TypedTrees,
        signature: &crate::signature::StateSignature,
    ) -> Vec<String> {
        let parameters = program
            .state_signature_parameters(signature)
            .iter()
            .collect::<Vec<_>>();
        boundary_invocation_names(
            program,
            crate::invocations::declared_signature_invocations(program, signature),
            &parameters,
        )
    }

    fn machine_synchronous_invocation_names(
        program: &crate::TypedTrees,
        machine: &crate::machine::Machine,
    ) -> Vec<String> {
        let Some(entry) = program.machine_states(machine).first() else {
            return Vec::new();
        };
        let parameters = program.state_parameters(entry).iter().collect::<Vec<_>>();
        boundary_invocation_names(
            program,
            crate::invocations::declared_machine_invocations(program, machine),
            &parameters,
        )
    }

    fn boundary_trait_name_for_type(
        program: &crate::TypedTrees,
        type_reference: crate::types::TypeReferenceHandle,
    ) -> Option<String> {
        let symbol = program
            .type_reference_node(type_reference)
            .type_symbol(&program.type_reference_table);
        program
            .traits()
            .iter()
            .find(|definition| definition.is_boundary && definition.symbol == symbol)
            .map(|definition| program.trait_declaration_path(definition))
    }

    fn boundary_invocation_names(
        program: &crate::TypedTrees,
        targets: Vec<flow_effects::InvocationTarget>,
        parameters: &[&crate::signature::StateParameter],
    ) -> Vec<String> {
        let mut names = targets
            .into_iter()
            .filter_map(|target| match target {
                flow_effects::InvocationTarget::Parameter(index) => parameters
                    .get(index as usize)
                    .map(|parameter| parameter.type_reference)
                    .and_then(|type_reference| {
                        boundary_trait_name_for_type(program, type_reference)
                    }),
                flow_effects::InvocationTarget::Service(symbol) => program
                    .traits()
                    .iter()
                    .find(|definition| definition.is_boundary && definition.symbol == symbol)
                    .map(|definition| program.trait_declaration_path(definition)),
            })
            .collect::<Vec<_>>();
        names.sort_unstable();
        names.dedup();
        names
    }
}
pub use projection::ProjectedServiceSurface;
pub(crate) mod catalog {
    use super::projection::{self, ServiceRequirementSource};
    use super::{ProjectedServiceSurface, ProviderSchemaDeclaration};
    use crate::ScalarAccessorDeclaration;
    use crate::provider_declarations::ProviderPlanningContext;
    use crate::trait_definition::AppliedTraitScope;
    use crate::type_identity::TypeIdentityId;
    use crate::types::TypeReferenceHandle;
    use arena::{Arena, Handle, HandleSpan};
    use diagnostics::Diagnostic;
    use language_semantics::service_surface::ServiceSurface;
    use semantic_vocabulary::PackageKeyIdentity;
    use std::sync::Arc;
    use symbols::SymbolHandle;

    pub struct PreparedServiceRequirement {
        pub source: ServiceRequirementSource,
        pub nominal_identity: Result<String, Diagnostic>,
    }

    impl Default for PreparedServiceRequirement {
        fn default() -> Self {
            Self {
                source: ServiceRequirementSource::default(),
                nominal_identity: Err(missing_declaration_owner()),
            }
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum ServiceOrigin {
        Declaration(ProviderSchemaDeclaration),
        Application {
            declaration: SymbolHandle,
            argument_owner: SymbolHandle,
            application: TypeIdentityId,
        },
        Named {
            declaration: SymbolHandle,
            capture: Handle<crate::CapturedMachineApplication>,
            receiver: SymbolHandle,
        },
    }

    struct ServiceRecord {
        origin: ServiceOrigin,
        schema: ProviderSchemaDeclaration,
        argument_owner: SymbolHandle,
        package: Option<PackageKeyIdentity>,
        path: String,
        nominal_identity: Result<String, Diagnostic>,
        declaration: Option<Arc<ServiceSurface>>,
        requirements: HandleSpan<PreparedServiceRequirement>,
        arguments: HandleSpan<TypeReferenceHandle>,
        correspondence: Result<(), Diagnostic>,
    }

    impl Default for ServiceRecord {
        fn default() -> Self {
            let schema = ProviderSchemaDeclaration::BoundaryTrait(SymbolHandle::invalid());
            Self {
                origin: ServiceOrigin::Declaration(schema),
                schema,
                argument_owner: SymbolHandle::invalid(),
                package: None,
                path: String::new(),
                nominal_identity: Err(missing_declaration_owner()),
                declaration: None,
                requirements: HandleSpan::default(),
                arguments: HandleSpan::default(),
                correspondence: Ok(()),
            }
        }
    }

    struct SealedDeclaration {
        symbol: SymbolHandle,
        declaration: Result<Option<ScalarAccessorDeclaration>, Diagnostic>,
    }

    impl Default for SealedDeclaration {
        fn default() -> Self {
            Self {
                symbol: SymbolHandle::invalid(),
                declaration: Ok(None),
            }
        }
    }

    pub struct PreparedServiceCatalog {
        owner_identity: u64,
        owner_address: usize,
        scope: source::DependencyScope,
        records: Arena<ServiceRecord>,
        requirements: Arena<PreparedServiceRequirement>,
        arguments: Arena<TypeReferenceHandle>,
        sealed: Arena<SealedDeclaration>,
        signatures: Arc<super::signatures::PreparedBoundarySignatures>,
    }

    #[derive(Clone, Copy)]
    pub struct ServiceCatalog<'catalog> {
        context: ProviderPlanningContext<'catalog>,
        catalog: &'catalog Arc<PreparedServiceCatalog>,
    }

    pub struct RetainedServiceSurface {
        catalog: Arc<PreparedServiceCatalog>,
        record: Handle<ServiceRecord>,
    }

    impl RetainedServiceSurface {
        pub fn surface(&self) -> Result<PreparedServiceSurface<'_>, Diagnostic> {
            self.catalog
                .view(self.catalog.records.get(self.record))
                .ok_or_else(|| {
                    Diagnostic::error("retained service declaration lost its prepared surface")
                })
        }
    }

    pub struct PreparedServiceDeclaration<'catalog> {
        pub schema: ProviderSchemaDeclaration,
        pub package: Option<PackageKeyIdentity>,
        pub path: &'catalog str,
        pub nominal_identity: &'catalog Result<String, Diagnostic>,
        pub surface: Option<PreparedServiceSurface<'catalog>>,
    }

    pub struct PreparedServiceSurface<'catalog> {
        pub schema: ProviderSchemaDeclaration,
        pub argument_owner: SymbolHandle,
        pub package: Option<PackageKeyIdentity>,
        pub path: &'catalog str,
        pub declaration: &'catalog Arc<ServiceSurface>,
        pub requirements: &'catalog [PreparedServiceRequirement],
        pub boundary_arguments: &'catalog [TypeReferenceHandle],
        correspondence: &'catalog Result<(), Diagnostic>,
    }

    impl PreparedServiceSurface<'_> {
        pub fn exact_requirements(&self) -> Result<Vec<SymbolHandle>, Diagnostic> {
            self.correspondence.as_ref().map_err(Clone::clone)?;
            Ok(self
                .requirements
                .iter()
                .map(|requirement| requirement.source.symbol)
                .collect())
        }
    }

    impl PreparedServiceCatalog {
        pub fn prepare(
            context: ProviderPlanningContext<'_>,
            exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
        ) -> Self {
            let signatures = Arc::new(super::signatures::PreparedBoundarySignatures::prepare(
                context.typed,
            ));
            Self::assemble(context, signatures, exact_toolchain_sources)
        }

        pub fn prepare_frozen(
            program: &crate::FrozenTypedTrees,
            scope: source::DependencyScope,
            exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
        ) -> Self {
            Self::assemble(
                ProviderPlanningContext::new(program, scope),
                Arc::clone(program.boundary_signatures()),
                exact_toolchain_sources,
            )
        }

        fn assemble(
            context: ProviderPlanningContext<'_>,
            signatures: Arc<super::signatures::PreparedBoundarySignatures>,
            exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
        ) -> Self {
            let program = context.typed;
            let mut catalog = Self {
                owner_identity: program.identity.get(),
                owner_address: std::ptr::from_ref(program) as usize,
                scope: context.scope,
                records: Arena::new(),
                requirements: Arena::new(),
                arguments: Arena::new(),
                sealed: Arena::new(),
                signatures,
            };

            for declaration in program.traits().iter().filter(|declaration| {
                declaration.is_boundary && context.contains(declaration.symbol)
            }) {
                let schema = ProviderSchemaDeclaration::BoundaryTrait(declaration.symbol);
                catalog.retain(
                    context,
                    exact_toolchain_sources,
                    ServiceOrigin::Declaration(schema),
                    schema,
                    projection::from_typed(context, declaration),
                    &[],
                );
            }

            for root in program.applied_trait_roots() {
                root.visit(program, &mut |scope| {
                    if !scope.declaration.is_boundary || !context.contains(scope.declaration.symbol)
                    {
                        return;
                    }
                    let schema = ProviderSchemaDeclaration::BoundaryTrait(scope.declaration.symbol);
                    let origin = ServiceOrigin::Application {
                        declaration: scope.declaration.symbol,
                        argument_owner: scope.argument_owner,
                        application: scope.application_identity(program).unwrap_or_default(),
                    };
                    if catalog.has_origin(origin) {
                        return;
                    }
                    catalog.retain(
                        context,
                        exact_toolchain_sources,
                        origin,
                        schema,
                        projection::from_typed_application(context, scope.clone()),
                        &[],
                    );
                });
            }

            for declaration in program
                .conformances()
                .iter()
                .filter(|declaration| context.contains(declaration.symbol))
            {
                catalog.retain_named(
                    context,
                    exact_toolchain_sources,
                    declaration.symbol,
                    Handle::default(),
                    AppliedTraitScope::from_conformance(program, declaration),
                    program.type_reference_handles(declaration.arguments),
                );
            }
            for (capture, application) in program.captured_machine_applications.iter() {
                let Some(declaration) = program
                    .conformance_by_symbol(application.conformance_application.declaration)
                    .filter(|declaration| context.contains(declaration.symbol))
                else {
                    continue;
                };
                catalog.retain_named(
                    context,
                    exact_toolchain_sources,
                    declaration.symbol,
                    capture,
                    application
                        .conformance_application
                        .trait_scope
                        .scope(program),
                    program.type_reference_handles(declaration.arguments),
                );
            }

            for operator in program
                .operators()
                .iter()
                .filter(|operator| operator.is_boundary && context.contains(operator.symbol))
            {
                let schema = ProviderSchemaDeclaration::BoundaryOperator(operator.symbol);
                catalog.retain(
                    context,
                    exact_toolchain_sources,
                    ServiceOrigin::Declaration(schema),
                    schema,
                    projection::from_typed_operator(context, operator),
                    &[],
                );
            }
            for machine in program.machines().iter().filter(|machine| {
                context.contains(machine.symbol)
                    && machine.supply_mode
                        == language_semantics::MachineSupplyMode::TopLevelRequirement
                    && !machine.body_is_present
            }) {
                let schema = ProviderSchemaDeclaration::BoundaryRequirement(machine.symbol);
                catalog.retain(
                    context,
                    exact_toolchain_sources,
                    ServiceOrigin::Declaration(schema),
                    schema,
                    projection::from_typed_boundary_requirement(context, machine),
                    &[],
                );
            }
            catalog.retain_sealed(context, exact_toolchain_sources);
            catalog
        }

        pub fn declared_traits(&self) -> impl Iterator<Item = PreparedServiceDeclaration<'_>> {
            self.records
                .iter()
                .filter(|(_, record)| {
                    matches!(
                        record.origin,
                        ServiceOrigin::Declaration(ProviderSchemaDeclaration::BoundaryTrait(_))
                    )
                })
                .map(move |(_, record)| PreparedServiceDeclaration {
                    schema: record.schema,
                    package: record.package,
                    path: &record.path,
                    nominal_identity: &record.nominal_identity,
                    surface: self.view(record),
                })
        }

        pub fn bind<'catalog>(
            self: &'catalog Arc<Self>,
            context: ProviderPlanningContext<'catalog>,
        ) -> Result<ServiceCatalog<'catalog>, Diagnostic> {
            if self.owner_identity != context.typed.identity.get()
                || self.owner_address != std::ptr::from_ref(context.typed) as usize
                || self.scope != context.scope
            {
                return Err(Diagnostic::error(
                    "service catalog belongs to another source snapshot or dependency purpose",
                ));
            }
            Ok(ServiceCatalog {
                context,
                catalog: self,
            })
        }

        fn record(&self, origin: ServiceOrigin) -> Handle<ServiceRecord> {
            let mut matching = self
                .records
                .iter()
                .filter(|(_, record)| record.origin == origin);
            let Some((record, _)) = matching.next() else {
                return Handle::default();
            };
            if matching.next().is_some() {
                return Handle::default();
            }
            record
        }

        fn lookup(&self, origin: ServiceOrigin) -> Option<PreparedServiceSurface<'_>> {
            self.view(self.records.get(self.record(origin)))
        }
    }

    impl<'catalog> ServiceCatalog<'catalog> {
        pub fn signatures(self) -> &'catalog super::signatures::PreparedBoundarySignatures {
            &self.catalog.signatures
        }

        pub fn context(self) -> ProviderPlanningContext<'catalog> {
            self.context
        }

        pub fn declaration(
            self,
            schema: ProviderSchemaDeclaration,
        ) -> Option<PreparedServiceSurface<'catalog>> {
            self.catalog.lookup(ServiceOrigin::Declaration(schema))
        }

        pub fn retain_declaration(
            self,
            schema: ProviderSchemaDeclaration,
        ) -> Option<RetainedServiceSurface> {
            let record = self.catalog.record(ServiceOrigin::Declaration(schema));
            self.catalog.view(self.catalog.records.get(record))?;
            Some(RetainedServiceSurface {
                catalog: Arc::clone(self.catalog),
                record,
            })
        }

        pub fn retain_application(
            self,
            scope: AppliedTraitScope<'_>,
        ) -> Option<RetainedServiceSurface> {
            let origin = ServiceOrigin::Application {
                declaration: scope.declaration.symbol,
                argument_owner: scope.argument_owner,
                application: scope.application_identity(self.context.typed)?,
            };
            let record = self.catalog.record(origin);
            self.catalog.view(self.catalog.records.get(record))?;
            Some(RetainedServiceSurface {
                catalog: Arc::clone(self.catalog),
                record,
            })
        }

        pub fn named_application(
            self,
            declaration: SymbolHandle,
            capture: Handle<crate::CapturedMachineApplication>,
            receiver: SymbolHandle,
        ) -> Option<PreparedServiceSurface<'catalog>> {
            self.catalog.lookup(ServiceOrigin::Named {
                declaration,
                capture,
                receiver,
            })
        }

        pub fn declared_traits(self) -> impl Iterator<Item = PreparedServiceDeclaration<'catalog>> {
            self.catalog.declared_traits()
        }

        // Calling must keep the exact declaration used to form the candidate.
        // Looking it up again by its authored name would discard applied
        // arguments. Only the same catalog-owned declaration can name its
        // retained source and requirement correspondence.
        pub fn retain_surface(
            self,
            declaration: &Arc<ServiceSurface>,
        ) -> Option<RetainedServiceSurface> {
            let mut matching = self.catalog.records.iter().filter(|(_, record)| {
                record
                    .declaration
                    .as_ref()
                    .is_some_and(|owned| Arc::ptr_eq(owned, declaration))
            });
            let (record, retained) = matching.next()?;
            if matching.next().is_some() {
                return None;
            }
            self.catalog.view(retained)?;
            Some(RetainedServiceSurface {
                catalog: Arc::clone(self.catalog),
                record,
            })
        }

        pub fn sealed_accessor(
            self,
            symbol: SymbolHandle,
        ) -> Result<Option<&'catalog ScalarAccessorDeclaration>, Diagnostic> {
            let Some((_, record)) = self
                .catalog
                .sealed
                .iter()
                .find(|(_, record)| record.symbol == symbol)
            else {
                return Ok(None);
            };
            record
                .declaration
                .as_ref()
                .map(Option::as_ref)
                .map_err(Clone::clone)
        }
    }

    impl PreparedServiceCatalog {
        fn view<'catalog>(
            &'catalog self,
            record: &'catalog ServiceRecord,
        ) -> Option<PreparedServiceSurface<'catalog>> {
            Some(PreparedServiceSurface {
                schema: record.schema,
                argument_owner: record.argument_owner,
                package: record.package,
                path: &record.path,
                declaration: record.declaration.as_ref()?,
                requirements: self.requirements.span_or_empty(record.requirements),
                boundary_arguments: self.arguments.span_or_empty(record.arguments),
                correspondence: &record.correspondence,
            })
        }

        fn has_origin(&self, origin: ServiceOrigin) -> bool {
            if matches!(origin, ServiceOrigin::Application { application, .. } if !application.is_valid())
            {
                return false;
            }
            self.records
                .iter()
                .any(|(_, record)| record.origin == origin)
        }

        fn retain(
            &mut self,
            context: ProviderPlanningContext<'_>,
            exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
            origin: ServiceOrigin,
            schema: ProviderSchemaDeclaration,
            projected: Option<ProjectedServiceSurface<'_>>,
            arguments: &[TypeReferenceHandle],
        ) {
            let mut record = ServiceRecord {
                origin,
                schema,
                package: context
                    .typed
                    .symbols
                    .symbol_package_identity(schema.symbol()),
                path: context.typed.symbols.display_path(schema.symbol(), "::"),
                nominal_identity: exact_identity(
                    context.typed,
                    schema.symbol(),
                    exact_toolchain_sources,
                ),
                arguments: self.arguments.insert_many(arguments.iter().copied()),
                ..ServiceRecord::default()
            };
            if let Some(projected) = projected {
                record.correspondence = projected.validate_correspondence();
                record.argument_owner = projected.argument_owner;
                record.requirements =
                    self.requirements
                        .insert_many(projected.requirements.into_iter().map(|source| {
                            PreparedServiceRequirement {
                                nominal_identity: exact_identity(
                                    context.typed,
                                    source.symbol,
                                    exact_toolchain_sources,
                                ),
                                source,
                            }
                        }));
                record.declaration = Some(projected.declaration);
            }
            self.records.insert(record);
        }

        fn retain_named<'program>(
            &mut self,
            context: ProviderPlanningContext<'program>,
            exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
            declaration: SymbolHandle,
            capture: Handle<crate::CapturedMachineApplication>,
            root: Option<AppliedTraitScope<'program>>,
            arguments: &[TypeReferenceHandle],
        ) {
            let Some(root) = root else {
                return;
            };
            let mut receivers = Vec::new();
            root.visit(context.typed, &mut |scope| {
                if scope.declaration.is_boundary
                    && context.contains(scope.declaration.symbol)
                    && !receivers.contains(&scope.declaration.symbol)
                {
                    receivers.push(scope.declaration.symbol);
                }
            });
            for receiver in receivers {
                let schema = ProviderSchemaDeclaration::BoundaryTrait(receiver);
                let projected = root
                    .declaring_scope(context.typed, receiver)
                    .and_then(|scope| projection::from_typed_application(context, scope));
                self.retain(
                    context,
                    exact_toolchain_sources,
                    ServiceOrigin::Named {
                        declaration,
                        capture,
                        receiver,
                    },
                    schema,
                    projected,
                    arguments,
                );
            }
        }

        fn retain_sealed(
            &mut self,
            context: ProviderPlanningContext<'_>,
            exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
        ) {
            let program = context.typed;
            let mut symbols = Vec::new();
            for view in &program.access_view_plans {
                for field in &view.fields {
                    for target in &field.accessor_targets {
                        for symbol in [target.machine_symbol, target.state_symbol] {
                            if !symbols.contains(&symbol) {
                                symbols.push(symbol);
                            }
                        }
                    }
                }
            }
            for symbol in symbols {
                let declaration =
                    ScalarAccessorDeclaration::authenticate(program, symbol).map(|declaration| {
                        declaration.filter(|declaration| context.contains(declaration.machine()))
                    });
                if let Ok(Some(declaration)) = &declaration {
                    let schema = ProviderSchemaDeclaration::SealedAccessor(declaration.machine());
                    if !self.has_origin(ServiceOrigin::Declaration(schema)) {
                        let projected =
                            program.machine(declaration.machine()).and_then(|machine| {
                                projection::project_boundary_machine_schema(
                                    context, machine, schema,
                                )
                            });
                        self.retain(
                            context,
                            exact_toolchain_sources,
                            ServiceOrigin::Declaration(schema),
                            schema,
                            projected,
                            &[],
                        );
                    }
                }
                self.sealed.insert(SealedDeclaration {
                    symbol,
                    declaration,
                });
            }
        }
    }

    pub fn exact_identity(
        typed: &crate::TypedTrees,
        symbol: SymbolHandle,
        sources: &[(source::SourceId, [u8; 32])],
    ) -> Result<String, Diagnostic> {
        typed
            .exact_owner_nominal_identity(symbol, sources)
            .and_then(|identity| typed.type_identity_text(identity))
            .ok_or_else(missing_declaration_owner)
    }

    fn missing_declaration_owner() -> Diagnostic {
        Diagnostic::error("service declaration has no exact package or toolchain owner")
    }
}
pub mod signatures {
    use crate::TypedTrees;
    use crate::provider_declarations::ProviderPlanningContext;
    use crate::types::TypeReferenceHandle;
    use arena::Arena;
    use language_semantics::callback_declarations::{
        BOUNDARY_NATIVE_PARAMETER_CAPACITY, BoundaryCallbackBinderDeclaration,
        BoundaryDirectCallbackDeclaration, BoundaryRuntimeFormal, BoundaryTelescope,
    };
    use semantic_vocabulary::{
        PackageKeyIdentity, callback_requirement_id, nominal_callback_binder_id,
        nominal_callback_native_parameter_id,
    };
    use symbols::SymbolHandle;

    pub(crate) mod abi_signatures {
        use super::{BoundarySignatureDeclaration, PreparedBoundarySignatures};
        use crate::TypedTrees;
        use crate::data::{TypeParameter, TypeParameterKind};
        use crate::types::TypeReferenceHandle;
        use language_semantics::callback_declarations::BoundaryTelescope;
        pub use semantic_vocabulary::CallingStaticOperand;
        use semantic_vocabulary::{CallingGenericFormal, CallingOperandKind, CallingSignatureId};
        use symbols::SymbolHandle;

        #[derive(Clone)]
        pub struct PreparedAbiStorage {
            pub parameters: Vec<TypeReferenceHandle>,
            pub result: TypeReferenceHandle,
            pub substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
        }

        pub struct PreparedBoundaryAbiSignature {
            pub id: CallingSignatureId,
            pub template: SymbolHandle,
            pub source_templates: Vec<SymbolHandle>,
            pub requirement_identity: String,
            pub declaring_application: String,
            pub operands: Vec<CallingStaticOperand>,
            pub formals: Vec<CallingGenericFormal>,
            pub default_eligible: bool,
            pub physical_owner: Result<semantic_vocabulary::CallingDeclarationOwner, String>,
            pub telescope: Result<BoundaryTelescope, String>,
            pub projections: Vec<Result<PreparedAbiStorage, String>>,
        }

        pub(crate) fn prepare(catalog: &mut PreparedBoundarySignatures, typed: &TypedTrees) {
            let mut rows = Vec::new();
            let mut boundaries = Vec::new();
            for declaration in catalog.declarations() {
                let (outer, open) = default_scope(typed, declaration);
                let projection = if open {
                    Err("declaration ABI requires its complete declaring and non-callback local application".into())
                } else {
                    declaration
                        .projection
                        .as_ref()
                        .map_err(Clone::clone)
                        .map(|prepared| PreparedAbiStorage {
                            parameters: prepared.parameter_types.clone(),
                            result: prepared.return_type,
                            substitutions: Vec::new(),
                        })
                };
                let id = insert(typed, &mut rows, declaration, outer, Vec::new(), projection);
                boundaries.push((declaration.symbol, Ok(id)));
            }

            for closed in typed.closed_requirement_applications() {
                let Some(declaration) = catalog
                    .declarations()
                    .find(|row| row.symbol == closed.template)
                else {
                    continue;
                };
                let prepared = (|| {
                    let outer = typed
                        .type_identity_text(closed.portable_trait_application_identity)
                        .filter(|identity| !identity.is_empty())
                        .ok_or("declaration ABI lost its portable declaring application")?
                        .to_owned();
                    let (_, signature) = typed
                        .trait_machine_signature(closed.template)
                        .ok_or("declaration ABI lost its exact template")?;
                    let parameters = typed.state_signature_type_parameters(signature);
                    let bindings = typed.static_machine_application_bindings(closed.application);
                    if parameters.len() != bindings.len()
                        || parameters.len() != closed.portable_local_arguments.len()
                        || parameters
                            .iter()
                            .zip(bindings)
                            .any(|(parameter, binding)| parameter.symbol != binding.parameter)
                    {
                        return Err(
                    "declaration ABI static operands differ from the complete declared telescope"
                        .to_owned(),
                );
                    }
                    let mut operands = Vec::new();
                    for (ordinal, ((parameter, identity), argument)) in parameters
                        .iter()
                        .zip(closed.portable_local_arguments)
                        .zip(closed.local_arguments)
                        .enumerate()
                    {
                        if callback_binder(declaration, parameter).is_some() {
                            continue;
                        }
                        operands.push(CallingStaticOperand {
                            ordinal: u32::try_from(ordinal)
                                .map_err(|_| "declaration ABI has too many static operands")?,
                            kind: operand_kind(&parameter.kind),
                            identity: typed
                                .requirement_argument_identity_text(*argument, *identity)
                                .filter(|identity| !identity.is_empty())
                                .ok_or("declaration ABI lost a portable static operand")?
                                .to_owned(),
                        });
                    }
                    let scope = closed
                        .trait_scope
                        .scope(typed)
                        .ok_or("declaration ABI lost its retained declaring scope")?;
                    let logical = typed
                        .applied_callable_signature(&scope, closed.template)
                        .or_else(|| {
                            (scope.substitutions.is_empty()
                                && scope.declaration.lifetime_parameters.is_empty())
                            .then_some(signature)
                        })
                        .ok_or("declaration ABI lost its callback-independent logical signature")?;
                    let substitutions = parameters
                        .iter()
                        .zip(closed.local_arguments)
                        .filter(|(parameter, _)| callback_binder(declaration, parameter).is_none())
                        .filter_map(|(parameter, actual)| {
                            actual.static_operand().map(|actual| (parameter, actual))
                        })
                        .map(|(parameter, actual)| {
                            let formal = typed
                                .applied_signature_parameter(signature, logical, parameter.symbol)
                                .ok_or(
                                    "declaration ABI lost a non-callback formal correspondence",
                                )?;
                            Ok((formal, actual))
                        })
                        .collect::<Result<Vec<_>, &str>>()?;
                    let runtime_projection = closed
                        .local_arguments
                        .iter()
                        .any(|argument| argument.static_operand().is_none());
                    Ok(insert(
                        typed,
                        &mut rows,
                        declaration,
                        outer,
                        operands,
                        Ok(PreparedAbiStorage {
                            parameters: typed
                                .state_signature_parameters(if runtime_projection {
                                    closed.signature
                                } else {
                                    logical
                                })
                                .iter()
                                .map(|parameter| parameter.type_reference)
                                .collect(),
                            result: if runtime_projection {
                                closed.signature.return_type
                            } else {
                                logical.return_type
                            },
                            substitutions: if runtime_projection {
                                Vec::new()
                            } else {
                                substitutions
                            },
                        }),
                    ))
                })();
                boundaries.push((closed.signature.symbol, prepared));
            }

            let roots = typed.applied_trait_roots().into_iter().chain(
                typed
                    .captured_machine_applications
                    .iter()
                    .filter_map(|(_, application)| {
                        application.conformance_application.trait_scope.scope(typed)
                    }),
            );
            for root in roots {
                root.visit(typed, &mut |scope| {
                    if !scope.declaration.is_boundary {
                        return;
                    }
                    for declared in typed.trait_machine_signatures(scope.declaration) {
                        let Some(declaration) = catalog
                            .declarations()
                            .find(|row| row.symbol == declared.symbol)
                        else {
                            continue;
                        };
                        if typed
                            .state_signature_type_parameters(declared)
                            .iter()
                            .any(|parameter| callback_binder(declaration, parameter).is_none())
                        {
                            continue;
                        }
                        let Some(outer) = scope
                            .portable_application_identity(typed)
                            .and_then(|identity| typed.type_identity_text(identity))
                            .filter(|identity| !identity.is_empty())
                        else {
                            continue;
                        };
                        let signature = typed
                            .applied_callable_signature(scope, declaration.symbol)
                            .or_else(|| {
                                (scope.substitutions.is_empty()
                                    && scope.declaration.lifetime_parameters.is_empty())
                                .then_some(declared)
                            });
                        let projection = signature
                            .map(|signature| PreparedAbiStorage {
                                parameters: typed
                                    .state_signature_parameters(signature)
                                    .iter()
                                    .map(|parameter| parameter.type_reference)
                                    .collect(),
                                result: signature.return_type,
                                substitutions: Vec::new(),
                            })
                            .ok_or_else(|| {
                                "declaration ABI lost its applied logical signature".to_owned()
                            });
                        insert(
                            typed,
                            &mut rows,
                            declaration,
                            outer.to_owned(),
                            Vec::new(),
                            projection,
                        );
                    }
                });
            }

            let governing_application = |machine: &crate::machine::Machine| {
                let mut applications = typed
                    .machine_trait_conformances(machine)
                    .iter()
                    .filter_map(|conformance| {
                        crate::machine::resolve_satisfied_declaration(typed, machine, conformance)
                    })
                    .filter_map(|declaration| {
                        boundaries
                            .iter()
                            .find(|(symbol, _)| *symbol == declaration.symbol())
                            .map(|(_, application)| application.clone())
                    });
                match (applications.next(), applications.next()) {
                    (Some(application), None) => Some(application),
                    _ => None,
                }
            };
            let mut inherited = Vec::new();
            for machine in typed.machines() {
                if machine.body_is_present
                    || boundaries
                        .iter()
                        .any(|(symbol, _)| *symbol == machine.symbol)
                {
                    continue;
                }
                let application = if machine.target_family_interface {
                    let members = typed
                        .target_machine_families
                        .iter()
                        .find(|family| family.canonical == machine.symbol)
                        .map_or(&[][..], |family| family.members.as_slice());
                    let mut applications = members
                        .iter()
                        .filter_map(|member| typed.machine_by_symbol(member.machine))
                        .map(governing_application);
                    match applications.next() {
                        Some(Some(first))
                            if applications.all(|other| other.as_ref() == Some(&first)) =>
                        {
                            Some(first)
                        }
                        _ => None,
                    }
                } else if matches!(
                    machine.supply_mode,
                    language_semantics::MachineSupplyMode::ExternalRealization { .. }
                        | language_semantics::MachineSupplyMode::Boundary
                ) {
                    governing_application(machine)
                } else {
                    None
                };
                if let Some(application) = application {
                    inherited.push((machine.symbol, application));
                }
            }
            boundaries.extend(inherited);

            let mut applications = Vec::new();
            for (handle, application) in catalog.calling.applications() {
                let prepared = (|| {
                    let declaration = catalog
                        .applied_declaration(application)
                        .ok_or("calling census lost its exact signature declaration")?;
                    let scope = application
                        .declaring_scope
                        .as_ref()
                        .and_then(|scope| scope.scope(typed))
                        .ok_or("calling census lost its exact declaring scope")?;
                    let outer = scope
                        .portable_application_identity(typed)
                        .and_then(|identity| typed.type_identity_text(identity))
                        .filter(|identity| !identity.is_empty())
                        .ok_or("calling census lost its portable declaring application")?
                        .to_owned();
                    let (_, declared) = typed
                        .trait_machine_signature(declaration.symbol)
                        .ok_or("calling census lost its exact template")?;
                    let locals = typed.state_signature_type_parameters(declared);
                    if locals
                        .iter()
                        .all(|parameter| callback_binder(declaration, parameter).is_some())
                    {
                        let signature = typed
                            .applied_callable_signature(&scope, declaration.symbol)
                            .or_else(|| {
                                (scope.substitutions.is_empty()
                                    && scope.declaration.lifetime_parameters.is_empty())
                                .then_some(declared)
                            });
                        let projection = signature
                            .map(|signature| PreparedAbiStorage {
                                parameters: typed
                                    .state_signature_parameters(signature)
                                    .iter()
                                    .map(|parameter| parameter.type_reference)
                                    .collect(),
                                result: signature.return_type,
                                substitutions: Vec::new(),
                            })
                            .ok_or_else(|| {
                                "declaration ABI lost its applied logical signature".to_owned()
                            });
                        let id =
                            insert(typed, &mut rows, declaration, outer, Vec::new(), projection);
                        return Ok(vec![id]);
                    }
                    Ok(rows
                        .iter()
                        .filter(|row| {
                            row.requirement_identity == declaration.requirement_identity
                                && row.declaring_application == outer
                                && !row.operands.is_empty()
                        })
                        .map(|row| row.id)
                        .collect())
                })();
                applications.push((handle, prepared));
            }
            for (application, ids) in applications {
                catalog.calling.set_abi_signatures(application, ids);
            }
            catalog.abi_signatures = rows;
            catalog.boundary_abis = boundaries;
        }

        fn default_scope(
            typed: &TypedTrees,
            declaration: &BoundarySignatureDeclaration,
        ) -> (String, bool) {
            let Some((owner, signature)) = typed.trait_machine_signature(declaration.symbol) else {
                return (declaration.requirement_identity.clone(), false);
            };
            let scope = crate::trait_definition::AppliedTraitScope::abstract_scope(owner);
            let outer = scope
                .portable_application_identity(typed)
                .and_then(|identity| typed.type_identity_text(identity))
                .unwrap_or_default()
                .to_owned();
            let open = !typed.trait_type_parameters(owner).is_empty()
                || !owner.lifetime_parameters.is_empty()
                || typed
                    .state_signature_type_parameters(signature)
                    .iter()
                    .any(|parameter| callback_binder(declaration, parameter).is_none());
            (outer, open)
        }

        fn callback_binder(
            declaration: &BoundarySignatureDeclaration,
            parameter: &TypeParameter,
        ) -> Option<semantic_vocabulary::StaticMachineBinderId> {
            let prepared = declaration.projection.as_ref().ok()?;
            prepared
                .callback_sources
                .iter()
                .zip(&prepared.telescope.binders)
                .find(|(source, _)| source.parameter_symbol == parameter.symbol)
                .map(|(_, binder)| binder.binder)
        }

        fn operand_kind(kind: &TypeParameterKind) -> CallingOperandKind {
            match kind {
                TypeParameterKind::Type => CallingOperandKind::Type,
                TypeParameterKind::Const { .. } => CallingOperandKind::Const,
                TypeParameterKind::Value { .. } => CallingOperandKind::Value,
                TypeParameterKind::Machine { .. } => CallingOperandKind::Machine,
                TypeParameterKind::Implementation => CallingOperandKind::Implementation,
                TypeParameterKind::Proposition { .. } => CallingOperandKind::Proposition,
            }
        }

        fn insert(
            typed: &TypedTrees,
            rows: &mut Vec<PreparedBoundaryAbiSignature>,
            declaration: &BoundarySignatureDeclaration,
            declaring_application: String,
            operands: Vec<CallingStaticOperand>,
            projection: Result<PreparedAbiStorage, String>,
        ) -> CallingSignatureId {
            let physical_owner = physical_owner(typed, declaration);
            let id = CallingSignatureId::for_application(
                &declaration.requirement_identity,
                &declaring_application,
                operands
                    .iter()
                    .map(|operand| (operand.ordinal, operand.kind, operand.identity.as_str())),
            );
            if let Some(row) = rows.iter_mut().find(|row| {
                row.id == id
                    && row.requirement_identity == declaration.requirement_identity
                    && row.declaring_application == declaring_application
                    && row.operands == operands
            }) {
                if !row.source_templates.contains(&declaration.symbol) {
                    row.source_templates.push(declaration.symbol);
                }
                row.projections.push(projection);
                row.default_eligible |= declaration.default_eligible;
                if row.physical_owner != physical_owner {
                    row.physical_owner = Err(
                        "declaration ABI copies disagree on their physical calling owner".into(),
                    );
                }
                return id;
            }
            let formals = typed
                .trait_machine_signature(declaration.symbol)
                .map(|(_, signature)| {
                    typed
                        .state_signature_type_parameters(signature)
                        .iter()
                        .enumerate()
                        .map(|(ordinal, parameter)| CallingGenericFormal {
                            ordinal: ordinal as u32,
                            kind: operand_kind(&parameter.kind),
                            callback: callback_binder(declaration, parameter),
                        })
                        .collect()
                })
                .unwrap_or_default();
            rows.push(PreparedBoundaryAbiSignature {
                id,
                template: declaration.symbol,
                source_templates: vec![declaration.symbol],
                requirement_identity: declaration.requirement_identity.clone(),
                declaring_application,
                operands,
                formals,
                default_eligible: declaration.default_eligible,
                physical_owner,
                telescope: declaration
                    .projection
                    .as_ref()
                    .map(|prepared| prepared.telescope.clone())
                    .map_err(Clone::clone),
                projections: vec![projection],
            });
            id
        }

        fn physical_owner(
            typed: &TypedTrees,
            declaration: &BoundarySignatureDeclaration,
        ) -> Result<semantic_vocabulary::CallingDeclarationOwner, String> {
            use semantic_vocabulary::CallingDeclarationOwner;
            if let Some((owner, _)) = typed.trait_machine_signature(declaration.symbol) {
                return if owner.is_boundary {
                    Ok(CallingDeclarationOwner::BoundaryTrait)
                } else {
                    Err(format!(
                        "selected boundary entry requirement owner `{}` is not an exact boundary trait",
                        typed.trait_declaration_path(owner)
                    ))
                };
            }
            if typed
                .machine_by_symbol(declaration.symbol)
                .is_some_and(|machine| {
                    machine.supply_mode
                        == language_semantics::MachineSupplyMode::TopLevelRequirement
                })
            {
                return Ok(CallingDeclarationOwner::TopLevelRequirement);
            }
            Err("declaration ABI has no exact physical calling owner".into())
        }
    }
    pub mod calling_applications {
        use super::BoundarySignatureDeclaration;
        use crate::TypedTrees;
        use crate::boundary_values::BoundaryTypeBinding;
        use crate::trait_definition::{
            AppliedTraitScope, TraitApplicationArguments, TraitApplicationEnvironment,
        };
        use crate::types::TypeReferenceHandle;
        use arena::{Arena, Handle, HandleSpan};
        use diagnostics::Diagnostic;
        use symbols::SymbolHandle;

        pub struct PreparedCallingRequest {
            pub boundary: SymbolHandle,
            pub name: String,
            pub span: source::SourceSpan,
            instances: Result<HandleSpan<PreparedCallingInstance>, Vec<Diagnostic>>,
        }

        impl Default for PreparedCallingRequest {
            fn default() -> Self {
                Self {
                    boundary: SymbolHandle::invalid(),
                    name: String::new(),
                    span: source::SourceSpan::default(),
                    instances: Ok(HandleSpan::default()),
                }
            }
        }

        pub struct PreparedCallingInstance {
            pub argument_owner: SymbolHandle,
            pub policy: Result<SymbolHandle, String>,
            arguments: HandleSpan<TypeReferenceHandle>,
            signatures: HandleSpan<PreparedAppliedBoundarySignature>,
        }

        impl Default for PreparedCallingInstance {
            fn default() -> Self {
                Self {
                    argument_owner: SymbolHandle::invalid(),
                    policy: Err("calling application is absent".into()),
                    arguments: HandleSpan::default(),
                    signatures: HandleSpan::default(),
                }
            }
        }

        #[derive(Default)]
        pub struct PreparedAppliedBoundarySignature {
            declaration: Handle<BoundarySignatureDeclaration>,
            bindings: HandleSpan<BoundaryTypeBinding>,
            pub declaring_scope: Option<TraitApplicationEnvironment>,
            pub abi_signatures: Vec<semantic_vocabulary::CallingSignatureId>,
            pub abi_refusal: Option<String>,
        }

        #[derive(Default)]
        pub struct PreparedCallingApplications {
            requests: Arena<PreparedCallingRequest>,
            instances: Arena<PreparedCallingInstance>,
            signatures: Arena<PreparedAppliedBoundarySignature>,
            arguments: Arena<TypeReferenceHandle>,
            bindings: Arena<BoundaryTypeBinding>,
        }

        impl PreparedCallingApplications {
            pub(crate) fn prepare(
                typed: &TypedTrees,
                declarations: &Arena<BoundarySignatureDeclaration>,
            ) -> Self {
                let vocabularies = crate::calling_vocabulary::standard_calling_vocabularies(typed);
                let mut prepared = Self::default();
                if vocabularies.is_empty() {
                    return prepared;
                }
                for boundary in typed
                    .traits()
                    .iter()
                    .filter(|definition| definition.is_boundary)
                {
                    let relationships = typed
                        .trait_requirements(boundary)
                        .iter()
                        .filter(|requirement| {
                            vocabularies
                                .iter()
                                .any(|(calling, _)| requirement.symbol == calling.symbol)
                        })
                        .collect::<Vec<_>>();
                    let Some(relationship) = relationships.first() else {
                        continue;
                    };
                    let span = relationship.source_span;
                    let instances = (|| {
                        if relationships.len() != 1 {
                            return Err(vec![Diagnostic::error(format!(
                        "boundary trait `{}` selects {} Calling<Policy> policies; exactly one concrete policy may define a boundary contract",
                        boundary.name, relationships.len(),
                    )).with_source_span(span)]);
                        }
                        let arguments = typed
                            .type_reference_table
                            .type_reference_handles(relationship.arguments);
                        if arguments.len() != 1 {
                            return Err(vec![Diagnostic::error(format!(
                        "boundary trait `{}` has a Calling relationship with {} policy arguments; expected one",
                        boundary.name, arguments.len(),
                    )).with_source_span(span)]);
                        }
                        let Some((calling, policy)) = vocabularies
                            .iter()
                            .find(|(calling, _)| calling.symbol == relationship.symbol)
                            .copied()
                        else {
                            return Err(vec![
                                Diagnostic::error("calling relationship lost its exact vocabulary")
                                    .with_source_span(span),
                            ]);
                        };
                        let instances =
                            boundary_policy_instances(typed, boundary).map_err(|reason| {
                                vec![
                        Diagnostic::error(format!(
                            "boundary trait `{}` cannot evaluate Calling<Policy>: {reason}",
                            boundary.name
                        ))
                        .with_source_span(span),
                    ]
                            })?;
                        let mut retained = Vec::with_capacity(instances.len());
                        for instance in instances {
                            let policy = prepared_policy_machine(
                                typed,
                                boundary,
                                &instance.arguments,
                                instance.argument_owner,
                                calling,
                                policy,
                            )
                            .map(|machine| machine.symbol);
                            let mut signatures = Vec::new();
                            collect_boundary_signatures(
                                typed,
                                boundary,
                                &instance.arguments,
                                instance.argument_owner,
                                &mut signatures,
                            )
                            .map_err(|reason| {
                                vec![Diagnostic::error(reason).with_source_span(span)]
                            })?;
                            let signatures = signatures
                                .into_iter()
                                .map(|signature| {
                                    let declaration = declarations
                                        .iter()
                                        .find(|(_, declaration)| {
                                            declaration.symbol == signature.signature.symbol
                                                && declaration.requirement_identity
                                                    == signature.requirement_identity
                                        })
                                        .map(|(handle, _)| handle)
                                        .unwrap_or_default();
                                    let bindings =
                                        prepared.bindings.insert_many(signature.bindings);
                                    PreparedAppliedBoundarySignature {
                                        declaration,
                                        bindings,
                                        declaring_scope: Some(signature.scope),
                                        abi_signatures: Vec::new(),
                                        abi_refusal: None,
                                    }
                                })
                                .collect::<Vec<_>>();
                            let signatures = prepared.signatures.insert_many(signatures);
                            let arguments = prepared.arguments.insert_many(instance.arguments);
                            retained.push(PreparedCallingInstance {
                                argument_owner: instance.argument_owner,
                                policy,
                                arguments,
                                signatures,
                            });
                        }
                        Ok(prepared.instances.insert_many(retained))
                    })();
                    prepared.requests.insert(PreparedCallingRequest {
                        boundary: boundary.symbol,
                        name: boundary.name.as_str().to_owned(),
                        span,
                        instances,
                    });
                }
                prepared
            }

            pub fn requests(&self) -> impl Iterator<Item = &PreparedCallingRequest> {
                self.requests.iter().map(|(_, request)| request)
            }

            pub fn instances(
                &self,
                request: &PreparedCallingRequest,
            ) -> Result<&[PreparedCallingInstance], Vec<Diagnostic>> {
                let instances = request.instances.as_ref().map_err(Clone::clone)?;
                Ok(self.instances.span_or_empty(*instances))
            }

            pub fn arguments(&self, instance: &PreparedCallingInstance) -> &[TypeReferenceHandle] {
                self.arguments.span_or_empty(instance.arguments)
            }

            pub fn signatures(
                &self,
                instance: &PreparedCallingInstance,
            ) -> &[PreparedAppliedBoundarySignature] {
                self.signatures.span_or_empty(instance.signatures)
            }

            pub fn bindings(
                &self,
                signature: &PreparedAppliedBoundarySignature,
            ) -> &[BoundaryTypeBinding] {
                self.bindings.span_or_empty(signature.bindings)
            }

            pub(crate) fn declaration(
                &self,
                signature: &PreparedAppliedBoundarySignature,
            ) -> Handle<BoundarySignatureDeclaration> {
                signature.declaration
            }

            pub(crate) fn applications(
                &self,
            ) -> impl Iterator<
                Item = (
                    Handle<PreparedAppliedBoundarySignature>,
                    &PreparedAppliedBoundarySignature,
                ),
            > {
                self.signatures.iter()
            }

            pub(crate) fn set_abi_signatures(
                &mut self,
                application: Handle<PreparedAppliedBoundarySignature>,
                ids: Result<Vec<semantic_vocabulary::CallingSignatureId>, String>,
            ) {
                let application = self.signatures.get_mut(application);
                match ids {
                    Ok(ids) => application.abi_signatures = ids,
                    Err(reason) => application.abi_refusal = Some(reason),
                }
            }
        }

        struct BoundarySignatureInstance<'input> {
            signature: &'input crate::signature::StateSignature,
            requirement_identity: String,
            bindings: Vec<BoundaryTypeBinding>,
            scope: TraitApplicationEnvironment,
        }

        struct BoundaryPolicyInstance {
            pub arguments: Vec<TypeReferenceHandle>,
            pub argument_owner: symbols::SymbolHandle,
        }

        fn boundary_policy_instances(
            typed: &TypedTrees,
            boundary: &crate::trait_definition::TraitDefinition,
        ) -> Result<Vec<BoundaryPolicyInstance>, String> {
            let parameters = typed.trait_type_parameters(boundary);
            if parameters.is_empty() {
                return Ok(vec![BoundaryPolicyInstance {
                    arguments: Vec::new(),
                    argument_owner: boundary.symbol,
                }]);
            }

            let mut instances = Vec::new();
            for conformance in typed
                .conformances()
                .iter()
                .filter(|conformance| conformance.trait_symbol == boundary.symbol)
            {
                let arguments = typed
                    .type_reference_table
                    .type_reference_handles(conformance.arguments)
                    .to_vec();
                if arguments.len() != parameters.len() {
                    return Err(format!(
                        "conformance `{} satisfies {}` supplies {} argument(s), expected {}",
                        typed.symbols.display_path(conformance.symbol, "::"),
                        conformance.trait_name,
                        arguments.len(),
                        parameters.len()
                    ));
                }
                instances.push(BoundaryPolicyInstance {
                    arguments,
                    argument_owner: conformance.symbol,
                });
            }

            // A closed application the program names directly, as in
            // `Owner<Arguments>::member(...)`, may be supplied by a native
            // Build binding instead of a named implementation. It is then an
            // application of this boundary that no conformance covers, and
            // its declared policy must be evaluated for it all the same, so
            // each such application is an instance too. An application some
            // conformance already covers keeps that conformance's instance.
            // These are the applied roots the service catalog also reads, so
            // an application has an instance exactly when it has a surface.
            let portable = |scope: &crate::trait_definition::AppliedTraitScope<'_>| {
                scope
                    .portable_application_identity(typed)
                    .and_then(|identity| typed.type_identity_text(identity))
            };
            let mut covered = typed
                .conformances()
                .iter()
                .filter(|conformance| conformance.trait_symbol == boundary.symbol)
                .filter_map(|conformance| {
                    crate::trait_definition::AppliedTraitScope::from_conformance(typed, conformance)
                        .and_then(|scope| scope.declaring_scope(typed, boundary.symbol))
                        .and_then(|scope| portable(&scope))
                })
                .collect::<Vec<_>>();
            for root in typed.applied_trait_roots() {
                root.visit(typed, &mut |scope| {
                    // Only a use that applies the boundary by itself: its one
                    // frame of substitutions is then the boundary's arguments.
                    if scope.declaration.symbol != boundary.symbol
                        || scope.substitution_scope_starts != [0]
                        || scope.substitutions.len() != parameters.len()
                    {
                        return;
                    }
                    let Some(identity) = portable(scope) else {
                        return;
                    };
                    if covered.contains(&identity) {
                        return;
                    }
                    covered.push(identity);
                    instances.push(BoundaryPolicyInstance {
                        arguments: scope
                            .substitutions
                            .iter()
                            .map(|(_, argument)| *argument)
                            .collect(),
                        argument_owner: scope.argument_owner,
                    });
                });
            }

            Ok(instances)
        }

        fn prepared_policy_machine<'program>(
            typed: &'program TypedTrees,
            boundary: &crate::trait_definition::TraitDefinition,
            boundary_arguments: &[TypeReferenceHandle],
            argument_owner: symbols::SymbolHandle,
            calling_trait: &crate::trait_definition::TraitDefinition,
            calling_policy: &crate::trait_definition::TraitDefinition,
        ) -> Result<&'program crate::machine::Machine, String> {
            use crate::trait_definition::{AppliedTraitScope, TraitApplicationArguments};
            let boundary_scope = AppliedTraitScope::new(
                typed,
                boundary,
                TraitApplicationArguments {
                    lexical_owner: argument_owner,
                    enclosing_data_applications: &[],
                    arguments: boundary_arguments,
                    lifetimes: &[],
                },
            )
            .ok_or("calling policy has no complete boundary application")?;
            let calling_scope = boundary_scope
                .declaring_scope(typed, calling_trait.symbol)
                .ok_or("calling policy has no unique exact Calling application")?;
            let selections = typed
                .trait_implementation_applications
                .iter()
                .filter(|(_, selected)| {
                    selected.source_scope.scope(typed).is_some_and(|scope| {
                        scope.argument_owner == argument_owner
                            && scope.same_application(typed, &calling_scope)
                    })
                })
                .map(|(_, selected)| selected)
                .collect::<Vec<_>>();
            let [selected] = selections.as_slice() else {
                return Err("calling-policy implementation application has no uniquely prepared closed source selection; unresolved static binders cannot execute".to_owned());
            };
            let Some(conformance) = typed.conformance_by_symbol(selected.application.declaration)
            else {
                return Err(format!(
                    "static argument `{}` does not name a package-scoped conformance",
                    typed
                        .symbols
                        .display_path(selected.application.declaration, "::")
                ));
            };
            let trait_definition = typed
                .trait_definition_by_symbol(conformance.trait_symbol)
                .ok_or_else(|| {
                    format!(
                        "conformance `{}` names unresolved trait `{}`",
                        conformance
                            .alias
                            .as_ref()
                            .map_or("<unnamed-conformance>", |name| name.as_str()),
                        conformance.trait_name
                    )
                })?;
            if trait_definition.symbol != calling_policy.symbol || !conformance.arguments.is_empty()
            {
                return Err(
                    "calling policy must explicitly select a complete CallingPolicy implementation"
                        .to_owned(),
                );
            }
            let requirements = typed.trait_machine_signatures(calling_policy);
            let [requirement] = requirements else {
                return Err(
                    "CallingPolicy must declare exactly one canonical plan requirement".to_owned(),
                );
            };
            if requirement.name.as_str() != "plan" {
                return Err("CallingPolicy has no canonical plan requirement".to_owned());
            }
            let rows = conformance
                .rows
                .iter()
                .filter(|row| {
                    row.declaring_trait == calling_policy.symbol
                        && row.requirement == requirement.symbol
                })
                .collect::<Vec<_>>();
            let [row] = rows.as_slice() else {
                return Err(
                    "calling policy implementation must retain exactly one CallingPolicy::plan row"
                        .to_owned(),
                );
            };
            let machine = typed
                .machine(row.realization_machine)
                .ok_or_else(|| "calling policy row has no exact realization machine".to_owned())?;
            if typed
                .machine_states(machine)
                .first()
                .is_none_or(|state| state.symbol != row.realization_state)
            {
                return Err(
                    "calling policy row must select its realization machine's entry state"
                        .to_owned(),
                );
            }
            let machine = if typed.declaration_scope_parent(machine.symbol) == conformance.symbol
                && (!typed.conformance_type_parameters(conformance).is_empty()
                    || !conformance.lifetime_parameters.is_empty())
            {
                let captures = typed
                    .captured_machine_applications
                    .iter()
                    .filter(|(_, capture)| {
                        capture.template == machine.symbol
                            && capture.conformance_application == selected.application
                            && typed
                                .expression_table
                                .static_machine_application(capture.supplying_application)
                                .lexical_owner
                                == argument_owner
                    })
                    .map(|(_, capture)| capture)
                    .collect::<Vec<_>>();
                let [capture] = captures.as_slice() else {
                    return Err(
                        "calling policy has no unique checked realization of its exact map capture"
                            .to_owned(),
                    );
                };
                let entry = typed
                    .captured_machine_states
                    .span_or_empty(capture.states)
                    .iter()
                    .filter(|state| state.template == row.realization_state)
                    .collect::<Vec<_>>();
                if entry.len() != 1 {
                    return Err(
                        "calling policy capture lost its exact entry correspondence".to_owned()
                    );
                }
                let realized = typed
                    .machine(capture.instance)
                    .ok_or("calling policy capture has no executable body")?;
                if typed
                    .machine_states(realized)
                    .first()
                    .is_none_or(|state| state.symbol != entry[0].instance)
                {
                    return Err(
                        "calling policy capture does not select its checked entry state".to_owned(),
                    );
                }
                realized
            } else {
                machine
            };
            if !typed.machine_type_parameters(machine).is_empty()
                || !machine.conformance_bounds.is_empty()
            {
                return Err("generic calling-policy realization requires an explicitly specialized machine before policy evaluation".to_owned());
            }
            Ok(machine)
        }

        fn collect_boundary_signatures<'a>(
            typed: &'a TypedTrees,
            definition: &'a crate::trait_definition::TraitDefinition,
            arguments: &[TypeReferenceHandle],
            argument_owner: SymbolHandle,
            signatures: &mut Vec<BoundarySignatureInstance<'a>>,
        ) -> Result<(), String> {
            let root = AppliedTraitScope::new(
                typed,
                definition,
                TraitApplicationArguments {
                    lexical_owner: argument_owner,
                    enclosing_data_applications: &[],
                    arguments,
                    lifetimes: &[],
                },
            )
            .ok_or("calling census lost its complete boundary application")?;
            let mut seen = Vec::new();
            let mut refusal = None;
            root.visit_parents_first(typed, &mut |scope| {
                if !scope.declaration.is_boundary || refusal.is_some() {
                    return;
                }
                let Some(application) = scope.portable_application_identity(typed) else {
                    refusal = Some(
                        "calling census cannot retain its complete declaring application"
                            .to_owned(),
                    );
                    return;
                };
                for signature in typed.trait_machine_signatures(scope.declaration) {
                    if seen.contains(&(signature.symbol, application)) {
                        continue;
                    }
                    seen.push((signature.symbol, application));
                    let bindings = scope
                        .substitutions
                        .iter()
                        .map(|(parameter, actual)| BoundaryTypeBinding {
                            parameter_symbol: *parameter,
                            parameter_name: typed.symbols.name(*parameter).to_owned(),
                            actual: *actual,
                        })
                        .collect();
                    signatures.push(BoundarySignatureInstance {
                        signature,
                        requirement_identity: typed
                            .trait_requirement_callable_identity_text(scope.declaration, signature),
                        bindings,
                        scope: scope.environment(),
                    });
                }
            });
            refusal.map_or(Ok(()), Err)
        }
    }
    pub use abi_signatures::{CallingStaticOperand, PreparedBoundaryAbiSignature};

    pub struct BoundaryCallbackSource {
        pub parameter_symbol: SymbolHandle,
        pub requirement_trait: SymbolHandle,
        pub requirement_machine: SymbolHandle,
    }

    pub struct PreparedBoundarySignature {
        pub telescope: BoundaryTelescope,
        pub callback_sources: Vec<BoundaryCallbackSource>,
        pub parameter_types: Vec<TypeReferenceHandle>,
        pub return_type: TypeReferenceHandle,
    }

    pub struct BoundarySignatureDeclaration {
        pub default_eligible: bool,
        pub symbol: SymbolHandle,
        pub package: Option<PackageKeyIdentity>,
        pub path: String,
        pub method: String,
        pub requirement_identity: String,
        pub projection: Result<PreparedBoundarySignature, String>,
    }

    impl Default for BoundarySignatureDeclaration {
        fn default() -> Self {
            Self {
                default_eligible: false,
                symbol: SymbolHandle::invalid(),
                package: None,
                path: String::new(),
                method: String::new(),
                requirement_identity: String::new(),
                projection: Err("boundary signature census has an absent declaration".into()),
            }
        }
    }

    pub struct PreparedBoundarySignatures {
        declarations: Arena<BoundarySignatureDeclaration>,
        calling: calling_applications::PreparedCallingApplications,
        abi_signatures: Vec<PreparedBoundaryAbiSignature>,
        boundary_abis: Vec<(
            SymbolHandle,
            Result<semantic_vocabulary::CallingSignatureId, String>,
        )>,
    }

    impl PreparedBoundarySignatures {
        pub(crate) fn prepare(typed: &TypedTrees) -> Self {
            let product = ProviderPlanningContext::new(typed, source::DependencyScope::Product);
            let mut declarations = Arena::new();
            for definition in typed.traits() {
                for signature in typed.trait_machine_signatures(definition) {
                    let requirement_identity =
                        typed.trait_requirement_callable_identity_text(definition, signature);
                    let projection =
                        prepare_trait_signature(typed, signature, &requirement_identity);
                    declarations.insert(BoundarySignatureDeclaration {
                        default_eligible: product.contains(definition.symbol),
                        symbol: signature.symbol,
                        package: typed.symbols.symbol_package_identity(definition.symbol),
                        path: typed.trait_declaration_path(definition),
                        method: signature.name.as_str().to_owned(),
                        requirement_identity,
                        projection,
                    });
                }
            }
            for machine in typed.machines().iter().filter(|machine| {
                machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
            }) {
                let [entry] = typed.machine_states(machine) else {
                    continue;
                };
                let requirement_identity = typed
                    .machine_callable_identity_text(machine)
                    .unwrap_or_default();
                let projection =
                    prepare_top_level_signature(typed, machine, entry, &requirement_identity);
                declarations.insert(BoundarySignatureDeclaration {
                    default_eligible: product.contains(machine.symbol),
                    symbol: machine.symbol,
                    package: typed.symbols.symbol_package_identity(machine.symbol),
                    path: typed.symbols.display_path(machine.symbol, "::"),
                    method: entry.name.as_str().to_owned(),
                    requirement_identity,
                    projection,
                });
            }
            let calling =
                calling_applications::PreparedCallingApplications::prepare(typed, &declarations);
            let mut prepared = Self {
                declarations,
                calling,
                abi_signatures: Vec::new(),
                boundary_abis: Vec::new(),
            };
            abi_signatures::prepare(&mut prepared, typed);
            prepared
        }

        pub fn declarations(&self) -> impl Iterator<Item = &BoundarySignatureDeclaration> {
            self.declarations.iter().map(|(_, declaration)| declaration)
        }

        pub fn calling(&self) -> &calling_applications::PreparedCallingApplications {
            &self.calling
        }

        pub fn abi_signatures(&self) -> &[PreparedBoundaryAbiSignature] {
            &self.abi_signatures
        }

        pub fn abi_for_boundary(
            &self,
            source: SymbolHandle,
        ) -> Option<&Result<semantic_vocabulary::CallingSignatureId, String>> {
            self.boundary_abis
                .iter()
                .find(|(owner, _)| *owner == source)
                .map(|(_, application)| application)
        }

        pub fn applied_declaration(
            &self,
            signature: &calling_applications::PreparedAppliedBoundarySignature,
        ) -> Option<&BoundarySignatureDeclaration> {
            let declaration = self.calling.declaration(signature);
            declaration
                .is_valid()
                .then(|| self.declarations.get(declaration))
        }
    }

    fn prepare_top_level_signature(
        typed: &TypedTrees,
        machine: &crate::machine::Machine,
        entry: &crate::state::State,
        owner: &str,
    ) -> Result<PreparedBoundarySignature, String> {
        if machine.body_is_present
            || !machine.lifetime_parameters.is_empty()
            || !typed.machine_type_parameters(machine).is_empty()
        {
            return Err("compatibility calling plans require one nongeneric bodyless top-level boundary requirement".into());
        }
        let parameters = typed.state_parameters(entry);
        if parameters.len() > BOUNDARY_NATIVE_PARAMETER_CAPACITY {
            return Err(format!(
                "top-level boundary requirement has {} native parameters; calling policies currently support at most {BOUNDARY_NATIVE_PARAMETER_CAPACITY}",
                parameters.len()
            ));
        }
        let parameters = parameters
            .iter()
            .enumerate()
            .map(|(ordinal, parameter)| {
                Ok(BoundaryRuntimeFormal {
                    identity: nominal_callback_native_parameter_id(owner, parameter.name.as_str()),
                    native_ordinal: u32::try_from(ordinal).map_err(
                        |_| "top-level boundary requirement has too many runtime parameters",
                    )?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(PreparedBoundarySignature {
            telescope: BoundaryTelescope {
                parameters,
                ..Default::default()
            },
            callback_sources: Vec::new(),
            parameter_types: typed
                .state_parameters(entry)
                .iter()
                .map(|parameter| parameter.type_reference)
                .collect(),
            return_type: entry.return_type,
        })
    }

    fn prepare_trait_signature(
        typed: &TypedTrees,
        signature: &crate::signature::StateSignature,
        owner: &str,
    ) -> Result<PreparedBoundarySignature, String> {
        let mut binders = Vec::new();
        let mut callback_sources = Vec::new();
        let mut static_machine_ordinal = 0u32;
        let invocations = crate::invocations::declared_signature_invocations(typed, signature);
        for parameter in typed.state_signature_type_parameters(signature) {
            let crate::data::TypeParameterKind::Machine { contract } = &parameter.kind else {
                continue;
            };
            let ordinal = static_machine_ordinal;
            static_machine_ordinal = static_machine_ordinal.checked_add(1).ok_or_else(|| {
                "boundary signature has too many static machine binders".to_owned()
            })?;
            let crate::data::MachineParameterContract::Nominal {
                trait_definition,
                requirement,
                ..
            } = contract
            else {
                continue;
            };
            let owner_trait = typed
                .trait_definition_by_symbol(*trait_definition)
                .ok_or_else(|| "nominal callback binder lost its declaring trait".to_owned())?;
            let requirement_row = typed
                .trait_machine_signature_by_symbol(owner_trait, *requirement)
                .ok_or_else(|| "nominal callback binder lost its exact requirement".to_owned())?;
            let requirement_identity =
                typed.trait_requirement_callable_identity_text(owner_trait, requirement_row);
            binders.push(BoundaryCallbackBinderDeclaration {
                binder: nominal_callback_binder_id(owner, ordinal, parameter.name.as_str()),
                requirement: callback_requirement_id(&requirement_identity),
                static_machine_ordinal: ordinal,
                requirement_identity,
                invokes_synchronously: invocations
                    .contains(&flow_effects::InvocationTarget::Service(*trait_definition)),
            });
            callback_sources.push(BoundaryCallbackSource {
                parameter_symbol: parameter.symbol,
                requirement_trait: *trait_definition,
                requirement_machine: *requirement,
            });
        }
        let runtime_parameters = typed.state_signature_parameters(signature);
        let total = runtime_parameters
            .len()
            .checked_add(signature.native_callback_parameters.len())
            .ok_or_else(|| "boundary native parameter telescope length overflow".to_owned())?;
        if total > BOUNDARY_NATIVE_PARAMETER_CAPACITY {
            return Err(format!(
                "boundary signature has {total} native parameters; calling policies currently support at most {BOUNDARY_NATIVE_PARAMETER_CAPACITY}"
            ));
        }
        let mut direct_callbacks = Vec::new();
        for direct in &signature.native_callback_parameters {
            let ordinal = usize::try_from(direct.native_ordinal)
                .map_err(|_| "native callback parameter ordinal exceeds usize".to_owned())?;
            if ordinal >= total {
                return Err(format!(
                    "native callback parameter `{}` has telescope position {}, outside {} declared native parameters",
                    direct.name, direct.native_ordinal, total
                ));
            }
            if direct_callbacks
                .iter()
                .any(|prior: &BoundaryDirectCallbackDeclaration| {
                    prior.native_ordinal == direct.native_ordinal
                        || prior.name == direct.name.as_str()
                })
                || runtime_parameters
                    .iter()
                    .any(|runtime| runtime.name.as_str() == direct.name.as_str())
            {
                return Err(format!(
                    "native callback parameter `{}` repeats a native telescope position or declared parameter name",
                    direct.name
                ));
            }
            let parameter = typed
                .state_signature_type_parameters(signature)
                .iter()
                .find(|parameter| parameter.name.as_str() == direct.binder.as_str())
                .ok_or_else(|| {
                    format!(
                        "native callback parameter `{}` names unknown machine binder `{}`",
                        direct.name, direct.binder
                    )
                })?;
            let binder = callback_sources.iter().position(|source| source.parameter_symbol == parameter.symbol)
            .and_then(|position| binders.get(position))
            .ok_or_else(|| format!("native callback parameter `{}` binder `{}` does not have one exact nominal callback requirement", direct.name, direct.binder))?;
            if direct_callbacks
                .iter()
                .any(|prior| prior.binder == binder.binder)
            {
                return Err(format!(
                    "native callback binder `{}` is assigned to more than one declared native callback parameter",
                    direct.binder
                ));
            }
            direct_callbacks.push(BoundaryDirectCallbackDeclaration {
                name: direct.name.as_str().to_owned(),
                identity: nominal_callback_native_parameter_id(owner, direct.name.as_str()),
                native_ordinal: direct.native_ordinal,
                binder: binder.binder,
                requirement: binder.requirement,
            });
        }
        direct_callbacks.sort_unstable_by_key(|parameter| parameter.native_ordinal);
        let mut native_ordinal = 0u32;
        let mut parameters = Vec::with_capacity(runtime_parameters.len());
        for parameter in runtime_parameters {
            while direct_callbacks
                .iter()
                .any(|direct| direct.native_ordinal == native_ordinal)
            {
                native_ordinal = native_ordinal
                    .checked_add(1)
                    .ok_or_else(|| "boundary native parameter ordinal overflow".to_owned())?;
            }
            parameters.push(BoundaryRuntimeFormal {
                identity: nominal_callback_native_parameter_id(owner, parameter.name.as_str()),
                native_ordinal,
            });
            native_ordinal = native_ordinal
                .checked_add(1)
                .ok_or_else(|| "boundary native parameter ordinal overflow".to_owned())?;
        }
        Ok(PreparedBoundarySignature {
            telescope: BoundaryTelescope {
                parameters,
                binders,
                direct_callbacks,
            },
            callback_sources,
            parameter_types: runtime_parameters
                .iter()
                .map(|parameter| parameter.type_reference)
                .collect(),
            return_type: signature.return_type,
        })
    }
}
pub use catalog::{
    PreparedServiceCatalog, PreparedServiceDeclaration, PreparedServiceRequirement,
    PreparedServiceSurface, RetainedServiceSurface, ServiceCatalog, exact_identity,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderSchemaDeclaration {
    BoundaryTrait(symbols::SymbolHandle),
    BoundaryRequirement(symbols::SymbolHandle),
    BoundaryOperator(symbols::SymbolHandle),
    SealedAccessor(symbols::SymbolHandle),
}

impl ProviderSchemaDeclaration {
    pub const fn symbol(self) -> symbols::SymbolHandle {
        match self {
            Self::BoundaryTrait(symbol)
            | Self::BoundaryRequirement(symbol)
            | Self::BoundaryOperator(symbol)
            | Self::SealedAccessor(symbol) => symbol,
        }
    }
}
