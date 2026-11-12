/*
Provider choices must keep the declarations and instances that were checked,
even after source is no longer available. This module retains that correspondence
and the explicit trust declarations used by provider admission.
*/

pub mod provider_declarations {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct NamedProviderApplication {
        pub declaration: symbols::SymbolHandle,
        pub capture: arena::Handle<typed_trees::typed_trees::CapturedMachineApplication>,
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
            application:
                Option<arena::Handle<typed_trees::product_operand::ProductOperandApplication>>,
        },
        Toolchain,
    }

    pub struct ProviderDeclarationRow<'identity> {
        pub requirement: symbols::SymbolHandle,
        pub origin: ProviderDeclarationOrigin,
        pub requirement_identity: &'identity str,
        pub application: ProviderApplicationView,
    }

    pub struct ProviderDeclarationRequest<'rows> {
        pub rows: &'rows [ProviderDeclarationRow<'rows>],
        pub named_application: Option<NamedProviderApplication>,
        pub provider_application_identity: Option<&'rows str>,
        pub installation_reach: bool,
    }

    impl crate::CheckedTrees {
        pub fn provider_declarations(
            &self,
            request: ProviderDeclarationRequest<'_>,
        ) -> language_semantics::provider_declarations::ProviderDeclarations {
            use language_semantics::provider_declarations::{
                ProviderApplicationDeclarations, ProviderDeclarations, ProviderRowDeclarations,
            };
            let mut declarations = Vec::with_capacity(request.rows.len());
            for (row_index, row) in request.rows.iter().enumerate() {
                let reach = if request.installation_reach {
                    self.provider_reach(row)
                } else {
                    Ok(None)
                };
                let leaf_applications =
                    if matches!(row.application, ProviderApplicationView::PhysicalLeaf) {
                        closed_provider_leaf_applications(&self.typed, row.requirement_identity)
                    } else {
                        Ok(Vec::new())
                    };
                let has_closed_application =
                    matches!(row.application, ProviderApplicationView::External)
                        && self
                            .typed
                            .closed_requirement_applications()
                            .any(|application| {
                                self.typed
                                    .trait_machine_signature(application.template)
                                    .is_some_and(|(owner, signature)| {
                                        self.typed.trait_requirement_callable_identity_text(
                                            owner, signature,
                                        ) == row.requirement_identity
                                    })
                            });
                let applications = if matches!(row.application, ProviderApplicationView::External) {
                    self.provider_external_applications(
                        row,
                        row_index,
                        request.named_application,
                        request.provider_application_identity,
                    )
                } else {
                    Ok(Vec::new())
                };
                declarations.push(ProviderRowDeclarations {
                    reach,
                    leaf_applications,
                    external_applications: ProviderApplicationDeclarations {
                        has_closed_application,
                        applications,
                    },
                });
            }
            ProviderDeclarations { rows: declarations }
        }

        fn provider_reach(
            &self,
            row: &ProviderDeclarationRow<'_>,
        ) -> Result<
            Option<language_semantics::provider_declarations::ProviderReachDeclaration>,
            String,
        > {
            let typed = &self.typed;
            let facts = &self.facts;
            let (identity, bounded, bound) = requirement_reach(typed, row.requirement)
                .ok_or("installation reach has no exact declared requirement")?;
            if identity != row.requirement_identity {
                return Err("installation reach changed its selected requirement identity".into());
            }
            if !bounded {
                return Ok(None);
            }
            let realization = match row.origin {
            ProviderDeclarationOrigin::Authored(symbol) => typed.machine_by_symbol(symbol),
            ProviderDeclarationOrigin::BuildNative { .. }
            | ProviderDeclarationOrigin::Toolchain => None,
        }.ok_or_else(|| format!(
            "selected provider row `{identity}` has no authored realization for its installation-bound reach"
        ))?;
            let envelope = facts
                .contract_plans
                .realized_envelope(realization.symbol)
                .ok_or_else(|| {
                    format!(
                        "selected provider realization `{}` has no checked contract envelope",
                        realization.name
                    )
                })?;
            let dependencies = envelope.unresolved_installation_reaches.iter().map(|dependency| {
            requirement_reach(typed, dependency.requirement).map(|(identity, _, _)| identity)
                .ok_or_else(|| format!("selected provider realization `{}` lost an exact installation-bound requirement", realization.name))
        }).collect::<Result<Vec<_>, _>>()?;
            let concrete_reach = if dependencies.is_empty() {
                envelope.effective_service_reach.clone()
            } else {
                envelope.concrete_service_reach.clone()
            };
            Ok(Some(
                language_semantics::provider_declarations::ProviderReachDeclaration {
                    requirement_identity: identity,
                    realization_name: realization.name.as_str().to_owned(),
                    upper_bound: facts
                        .service_reaches
                        .rows
                        .services(bound)
                        .iter()
                        .filter_map(|service| facts.service_reaches.services.definition(*service))
                        .map(|definition| definition.name.clone())
                        .collect(),
                    concrete_reach,
                    dependencies,
                },
            ))
        }

        fn provider_external_applications(
            &self,
            row: &ProviderDeclarationRow<'_>,
            row_index: usize,
            named: Option<NamedProviderApplication>,
            provider_application_identity: Option<&str>,
        ) -> Result<Vec<String>, String> {
            use typed_trees::trait_definition::AppliedTraitScope;
            let typed = &self.typed;
            let Some((owner, signature)) = typed.trait_machine_signature(row.requirement) else {
                return Ok(Vec::new());
            };
            if typed.trait_requirement_callable_identity_text(owner, signature)
                != row.requirement_identity
            {
                return Err(
                    "selected Import template identity disagrees with its provenance".into(),
                );
            }
            let mut native_scope = None;
            let mut native_closed = None;
            let realization = match row.origin {
                ProviderDeclarationOrigin::Authored(symbol) => Some(
                    typed
                        .machine(symbol)
                        .ok_or("selected Import provenance lost its realization declaration")?,
                ),
                ProviderDeclarationOrigin::BuildNative {
                    requirement,
                    application,
                } => {
                    if !requirement.is_valid() || requirement != row.requirement {
                        return Err("native Import origin names another exact requirement".into());
                    }
                    if named.is_some() {
                        return Err(
                            "native Import origin cannot substitute a named implementation".into(),
                        );
                    }
                    if !owner.is_boundary {
                        return Err("native Import origin lost its boundary declaration".into());
                    }
                    if let Some(application) = application {
                        if !typed.product_operand_applications.is_valid(application) {
                            return Err(
                                "native Import origin has a stale Product application".into()
                            );
                        }
                        let prepared = typed.product_operand_applications.get(application);
                        if prepared.role != Some(language_semantics::declaration_selection::ProductOperandRole::Requirement) { return Err("native Import origin has another Product operand role".into()); }
                        let closed = typed.closed_requirement_application(prepared.declaration);
                        if closed
                            .as_ref()
                            .map_or(prepared.declaration, |closed| closed.template)
                            != row.requirement
                        {
                            return Err(
                                "native Import origin changed its exact requirement template"
                                    .into(),
                            );
                        }
                        let scope = prepared
                            .scope
                            .scope(typed)
                            .filter(|scope| scope.declaration.symbol == owner.symbol)
                            .ok_or("native Import origin lost its exact applied boundary scope")?;
                        let identity = match &closed {
                            Some(closed) => {
                                Some(typed.trait_requirement_callable_identity_text(
                                    owner,
                                    closed.signature,
                                ))
                            }
                            None => scope
                                .portable_application_identity(typed)
                                .and_then(|identity| typed.type_identity_text(identity)),
                        }
                        .ok_or(
                            "native Import origin lost its complete selected application identity",
                        )?;
                        if provider_application_identity != Some(identity.as_str()) {
                            return Err("native Import origin differs from the selected application identity".into());
                        }
                        native_scope = Some(scope);
                        native_closed = closed.map(|_| prepared.declaration);
                    } else if provider_application_identity.is_some()
                        || !typed.trait_type_parameters(owner).is_empty()
                        || !owner.lifetime_parameters.is_empty()
                    {
                        return Err(
                            "native Import origin lost its closed boundary declaration".into()
                        );
                    }
                    None
                }
                ProviderDeclarationOrigin::Toolchain => {
                    return Err("selected Import has no retained source owner".into());
                }
            };
            let scope = if let Some(realization) = realization {
                if let Some(named) = named {
                    let declaration = typed
                        .conformance_by_symbol(named.declaration)
                        .ok_or("selected Import lost its exact named implementation row")?;
                    declaration
                        .rows
                        .get(row_index)
                        .filter(|entry| {
                            entry.requirement == row.requirement
                                && entry.declaring_trait == owner.symbol
                        })
                        .ok_or("selected Import lost its exact named implementation row")?;
                    if named.capture.is_valid() {
                        if !typed.captured_machine_applications.is_valid(named.capture) {
                            return Err(
                                "selected Import lost its captured implementation application"
                                    .into(),
                            );
                        }
                        let capture = typed.captured_machine_applications.get(named.capture);
                        if capture.conformance_application.declaration != declaration.symbol {
                            return Err(
                                "selected Import captured application names another implementation"
                                    .into(),
                            );
                        }
                        capture.conformance_application.trait_scope.scope(typed)
                    } else {
                        AppliedTraitScope::from_conformance(typed, declaration)
                    }
                } else {
                    let mut edges =
                        typed
                            .machine_trait_conformances(realization)
                            .iter()
                            .filter(|edge| {
                                edge.requirement_symbol == row.requirement
                                    && edge.symbol == owner.symbol
                            });
                    let edge = edges
                        .next()
                        .filter(|_| edges.next().is_none())
                        .ok_or("selected Import lost its unique external satisfaction")?;
                    AppliedTraitScope::from_machine_conformance(typed, realization, edge)
                }
            } else {
                match row.origin {
                    ProviderDeclarationOrigin::BuildNative {
                        application: Some(_),
                        ..
                    } => native_scope,
                    ProviderDeclarationOrigin::BuildNative {
                        application: None, ..
                    } => Some(AppliedTraitScope::abstract_scope(owner)),
                    _ => None,
                }
            }
            .ok_or("selected Import lost its complete declaring application")?;
            let mut identities = Vec::new();
            for application in typed
                .closed_requirement_applications()
                .filter(|application| {
                    application.template == row.requirement
                        && native_closed
                            .is_none_or(|selected| application.signature.symbol == selected)
                        && typed.requirement_application_matches_scope(
                            application.signature.symbol,
                            &scope,
                        )
                })
            {
                let required = typed.state_signature_type_parameters(signature);
                let realization_matches = realization.is_none_or(|realization| {
                    let actual = typed.machine_type_parameters(realization);
                    required.len() == actual.len()
                        && required.iter().zip(actual).all(|(required, actual)| {
                            std::mem::discriminant(&required.kind)
                                == std::mem::discriminant(&actual.kind)
                        })
                });
                if application.local_arguments.len() != required.len() || !realization_matches {
                    return Err(
                    "selected Import closed application lost its complete declaration telescope"
                        .into(),
                );
                }
                identities.push(
                    typed.trait_requirement_callable_identity_text(owner, application.signature),
                );
            }
            Ok(identities)
        }
    }

    pub fn closed_provider_leaf_applications(
        typed: &typed_trees::TypedTrees,
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

    fn requirement_reach(
        typed: &typed_trees::TypedTrees,
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
}

pub mod provider_instances {
    use crate::CheckFacts;
    use language_semantics::MachineSupplyMode::TopLevelRequirement;
    use language_semantics::provider_declarations::{
        ClosedProviderInstance, ProviderInstanceApplication,
    };
    use semantic_vocabulary::PackageKeyIdentity;
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;
    use typed_trees::machine::Machine;
    use typed_trees::type_identity::CallableIdentityId;

    pub fn inherited_instances(
        typed: &TypedTrees,
        facts: &CheckFacts,
        requirement: &str,
        provider: &Machine,
        selecting_declaration: Option<SymbolHandle>,
        package: Option<PackageKeyIdentity>,
    ) -> Vec<ProviderInstanceApplication<CallableIdentityId>> {
        typed.closed_requirement_applications().filter(|application| {
        typed.trait_machine_signature(application.template).is_some_and(|(owner, signature)| {
            typed.trait_requirement_callable_identity_text(owner, signature) == requirement
        })
    }).map(|application| {
        let associations: Vec<_> = facts.provider_closed_requirements.iter().filter(|fact| {
            fact.template == application.template
                && fact.closed_requirement == application.signature.symbol
                && fact.receiving_application == application.trait_application_identity
                && fact.selecting_declaration == selecting_declaration.unwrap_or(fact.provider)
                && typed.requirement_realization_matches(
                    fact.closed_requirement, provider.symbol, fact.provider,
                )
        }).collect();
        let candidates = associations.iter().map(|fact| {
            let instance = typed.machine(fact.provider)
                .ok_or("closed provider fact lost its realization body")?;
            if !typed.machine_type_parameters(instance).is_empty()
                || typed.symbols.symbol_package_identity(instance.symbol) != package
            {
                return Err("closed provider realization has an open telescope or a different package".into());
            }
            let owner = typed.trait_definition_by_symbol(application.declaring_trait)
                .ok_or("closed requirement fact lost its declaring trait")?;
            let requirement = typed.trait_requirement_callable_identity(owner, application.signature);
            let realization = typed.machine_callable_identity(instance);
            if !requirement.is_valid() || !realization.is_valid() {
                return Err("closed provider coordinate has no normalized callable identity".into());
            }
            Ok(ClosedProviderInstance { requirement, realization })
        }).collect();
        let declaration_application = associations.first()
            .ok_or_else(|| "provider group has no checked declaration application".to_owned())
            .and_then(|first| {
                if associations.iter().any(|fact| fact.declaration_application != first.declaration_application) {
                    return Err("provider group has competing declaration applications".into());
                }
                typed.type_identity_text(first.declaration_application)
                    .filter(|text| !text.is_empty())
                    .ok_or_else(|| "provider declaration application has no portable identity".into())
            });
        ProviderInstanceApplication {
            declaration_application,
            receiving_application: typed
                .type_identity_text(application.portable_trait_application_identity)
                .filter(|text| !text.is_empty())
                .ok_or_else(|| "provider receiving application has no portable identity".into()),
            candidates,
        }
    }).collect()
    }

    pub fn generic_instances(
        typed: &TypedTrees,
        facts: &CheckFacts,
        provider: &Machine,
        template: &Machine,
        package: Option<PackageKeyIdentity>,
    ) -> Vec<Result<ClosedProviderInstance<CallableIdentityId>, String>> {
        typed.machine_specializations.iter().filter(|receipt| {
        receipt.template == provider.symbol && receipt.operator_realizations.iter()
            .any(|application| application.requirement_symbol == template.symbol)
    }).map(|receipt| {
        let instance = typed.machine(receipt.instance).ok_or_else(|| format!(
            "selected provider `{}` specialization retains no typed instance", provider.name,
        ))?;
        if !typed.machine_type_parameters(instance).is_empty() {
            return Err(format!(
                "selected provider specialization instance `{}` remains generic", instance.name,
            ));
        }
        if typed.symbols.symbol_package_identity(instance.symbol) != package {
            return Err(format!(
                "selected provider instance `{}` drifts from its template's exact package identity", instance.name,
            ));
        }
        let closed = satisfied_requirement(typed, facts, instance, template).ok_or_else(|| format!(
            "selected provider instance `{}` realizes requirement template `{}` without one exact closed requirement",
            instance.name, template.name,
        ))?;
        if closed.symbol == template.symbol {
            return Err(format!(
                "selected provider instance `{}` replays requirement template `{}` onto itself", instance.name, template.name,
            ));
        }
        let requirement = typed.machine_callable_identity(closed);
        let realization = typed.machine_callable_identity(instance);
        if !requirement.is_valid() || !realization.is_valid() {
            return Err(format!(
                "selected provider instance `{}` or closed requirement `{}` has no normalized identity", instance.name, closed.name,
            ));
        }
        Ok(ClosedProviderInstance { requirement, realization })
    }).collect()
    }

    pub fn closed_requirements(typed: &TypedTrees, template: &Machine) -> Vec<CallableIdentityId> {
        typed
            .machine_specializations
            .iter()
            .filter(|receipt| receipt.template == template.symbol)
            .filter_map(|receipt| typed.machine(receipt.instance))
            .filter(|instance| instance.supply_mode == TopLevelRequirement)
            .map(|instance| typed.machine_callable_identity(instance))
            .filter(|identity| identity.is_valid())
            .collect()
    }

    fn satisfied_requirement<'typed>(
        typed: &'typed TypedTrees,
        facts: &CheckFacts,
        provider: &Machine,
        requirement: &'typed Machine,
    ) -> Option<&'typed Machine> {
        if typed.machine_type_parameters(requirement).is_empty() {
            return Some(requirement);
        }
        let mut applications = facts.provider_closed_requirements.iter().filter(|row| {
            row.provider == provider.symbol
                && row.selecting_declaration == provider.symbol
                && row.template == requirement.symbol
        });
        let application = applications.next()?;
        if applications.next().is_some() {
            return None;
        }
        typed.machine(application.closed_requirement)
    }
}

pub(crate) mod provider_inventory {
    use crate::CheckedTrees;
    use diagnostics::Diagnostic;
    use language_semantics::direct_calls::DirectCallDeclarations;
    use language_semantics::provider_declarations::SelectedTargetMachineOrigin;
    use source::DependencyScope::Product;
    use std::collections::HashSet;
    use std::sync::{Arc, OnceLock};
    use symbols::SymbolHandle;
    use typed_trees::provider_declarations::ProviderPlanningContext;
    use typed_trees::provider_declarations::provider_target_declarations;
    use typed_trees::service_declarations::ServiceCatalog;

    pub struct PreparedProviderInventory {
        checked: Arc<CheckedTrees>,
        uses: OnceLock<HashSet<SymbolHandle>>,
        origins: OnceLock<Vec<SelectedTargetMachineOrigin>>,
        direct_calls: OnceLock<Arc<DirectCallDeclarations<SymbolHandle>>>,
        trust: OnceLock<language_semantics::trust_declarations::TrustDeclarations>,
        services: Arc<typed_trees::service_declarations::PreparedServiceCatalog>,
    }

    impl PreparedProviderInventory {
        pub fn new(
            checked: Arc<CheckedTrees>,
            exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
        ) -> Self {
            let services =
                typed_trees::service_declarations::PreparedServiceCatalog::prepare_frozen(
                    &checked.typed,
                    Product,
                    exact_toolchain_sources,
                );
            Self {
                checked,
                uses: OnceLock::new(),
                origins: OnceLock::new(),
                direct_calls: OnceLock::new(),
                trust: OnceLock::new(),
                services: Arc::new(services),
            }
        }

        pub fn program(&self) -> &Arc<CheckedTrees> {
            &self.checked
        }

        pub fn retained_service_catalog(
            &self,
        ) -> Arc<typed_trees::service_declarations::PreparedServiceCatalog> {
            Arc::clone(&self.services)
        }

        pub fn service_declarations(
            &self,
        ) -> impl Iterator<Item = typed_trees::service_declarations::PreparedServiceDeclaration<'_>>
        {
            self.services.declared_traits()
        }

        pub fn trust_declarations(
            &self,
        ) -> &language_semantics::trust_declarations::TrustDeclarations {
            self.trust
                .get_or_init(|| crate::trust_declarations::collect(&self.checked))
        }

        pub fn services_for(
            &self,
            checked: &CheckedTrees,
        ) -> Result<ServiceCatalog<'_>, Diagnostic> {
            self.require_owner(checked)?;
            self.services
                .bind(ProviderPlanningContext::new(&self.checked.typed, Product))
        }

        pub fn direct_calls_for(
            &self,
            checked: &CheckedTrees,
        ) -> Result<&DirectCallDeclarations<SymbolHandle>, Diagnostic> {
            self.require_owner(checked)?;
            Ok(self.direct_calls())
        }

        pub fn direct_calls(&self) -> &DirectCallDeclarations<SymbolHandle> {
            self.direct_call_declarations()
        }

        pub fn retained_direct_calls_for(
            &self,
            checked: &CheckedTrees,
        ) -> Result<Arc<DirectCallDeclarations<SymbolHandle>>, Diagnostic> {
            self.require_owner(checked)?;
            Ok(Arc::clone(self.direct_call_declarations()))
        }

        fn direct_call_declarations(&self) -> &Arc<DirectCallDeclarations<SymbolHandle>> {
            self.direct_calls.get_or_init(|| {
                Arc::new(typed_trees::direct_calls::collect(
                    &self.checked.typed,
                    Product,
                ))
            })
        }

        pub fn origins_for(
            &self,
            checked: &CheckedTrees,
        ) -> Result<&[SelectedTargetMachineOrigin], Diagnostic> {
            self.require_owner(checked)?;
            Ok(self.origins())
        }

        pub fn origins(&self) -> &[SelectedTargetMachineOrigin] {
            self.origins
                .get_or_init(|| provider_target_declarations(&self.checked.typed, Product))
        }

        fn require_owner(&self, checked: &CheckedTrees) -> Result<(), Diagnostic> {
            if !std::ptr::eq(self.checked.as_ref(), checked) {
                return Err(Diagnostic::error(
                    "provider declarations belong to another checked program",
                ));
            }
            Ok(())
        }

        pub fn uses_for(
            &self,
            checked: &CheckedTrees,
        ) -> Result<&HashSet<SymbolHandle>, Diagnostic> {
            self.require_owner(checked)?;
            Ok(self.uses.get_or_init(|| {
                let template = |symbol| {
                    self.checked
                        .typed
                        .closed_requirement_application(symbol)
                        .map_or(symbol, |application| application.template)
                };
                let mut uses =
                    crate::typed_trees::references::collect_program_references(&self.checked.typed)
                        .callable_uses
                        .into_iter()
                        .map(template)
                        .collect::<HashSet<_>>();
                uses.extend(
                    self.checked
                        .facts
                        .operators
                        .boundary_applications
                        .iter()
                        .map(|application| template(application.requirement_symbol)),
                );
                uses.extend(
                    self.checked
                        .facts
                        .operators
                        .symbolic_boundary_applications
                        .iter()
                        .map(|application| template(application.requirement_symbol)),
                );
                uses.extend(
                    self.checked
                        .facts
                        .provider_closed_requirements
                        .iter()
                        .map(|row| row.template),
                );
                uses
            }))
        }
    }
}

pub(crate) mod trust_declarations {
    use crate::CheckedTrees;
    use language_semantics::trust_declarations::{
        AcceptedContractDeclaration, AcceptedMachineDeclaration, AcceptedTemplateDeclaration,
        TrustDeclarationCardinality, TrustDeclarations,
    };

    pub(crate) fn collect(checked: &CheckedTrees) -> TrustDeclarations {
        let machines = checked
            .typed
            .machines()
            .iter()
            .filter(|machine| {
                machine.supply_mode == language_semantics::MachineSupplyMode::AdmissionClaim
                    && !checked
                        .typed
                        .machine_specializations
                        .iter()
                        .any(|specialization| {
                            specialization.accepted_template_commitment.is_some()
                                && specialization.instance == machine.symbol
                                && specialization.instance != specialization.template
                        })
            })
            .map(|machine| {
                let mut templates = checked
                    .facts
                    .accepted_template_identities
                    .rows()
                    .iter()
                    .filter(|row| row.machine == machine.symbol);
                let template = match (templates.next(), templates.next()) {
                    (None, _) => Err(TrustDeclarationCardinality::Missing),
                    (Some(_), Some(_)) => Err(TrustDeclarationCardinality::Duplicate),
                    (Some(row), None) => {
                        Ok(row.identity.map(|identity| AcceptedTemplateDeclaration {
                            report_fingerprint: identity.report_fingerprint,
                            commitment: identity.commitment.as_bytes(),
                        }))
                    }
                };
                let mut contracts = checked
                    .facts
                    .contract_plans
                    .machines()
                    .iter()
                    .filter(|plan| plan.machine == machine.symbol);
                let contract = match (contracts.next(), contracts.next()) {
                    (None, _) => Err(TrustDeclarationCardinality::Missing),
                    (Some(_), Some(_)) => Err(TrustDeclarationCardinality::Duplicate),
                    (Some(plan), None) => Ok(AcceptedContractDeclaration {
                        report_fingerprint: plan.report_fingerprint,
                        commitment: plan.commitment.as_bytes(),
                    }),
                };
                AcceptedMachineDeclaration {
                    machine: machine.symbol,
                    name: machine.name.as_str().to_owned(),
                    template,
                    contract,
                }
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        TrustDeclarations { machines }
    }

    #[cfg(test)]
    pub(crate) mod tests {
        use super::collect;
        use crate::{
            CheckFacts, CheckedAcceptedTemplateClassification, CheckedAcceptedTemplateIdentities,
            CheckedAcceptedTemplateIdentity, CheckedTrees, MachineContractCommitment,
            MachineContractPlan, PreparedProviderInventory,
        };
        use language_semantics::trust_declarations::TrustDeclarationCardinality;
        use std::sync::Arc;
        use symbols::SymbolHandle;
        use typed_trees::TypedTrees;
        use typed_trees::machine::Machine;
        use typed_trees::typed_trees::{MachineSpecialization, MachineTemplateCommitment};

        fn machine(index: u32, name: &str) -> Machine {
            Machine {
                symbol: SymbolHandle::from_arena_index(index),
                name: name.into(),
                supply_mode: language_semantics::MachineSupplyMode::AdmissionClaim,
                ..Default::default()
            }
        }

        #[test]
        fn complete_census_retains_exact_evidence_and_excludes_fresh_specialized_instances() {
            let mut typed = TypedTrees::default();
            typed.push_machine(machine(1, "Template"));
            typed.push_machine(machine(2, "Instance"));
            typed.push_machine(Machine {
                supply_mode: language_semantics::MachineSupplyMode::CheckedBody,
                ..machine(3, "Ordinary")
            });
            typed.push_machine(machine(4, "ReusedTemplate"));
            for (template, instance) in [(1, 2), (4, 4)] {
                typed.machine_specializations.push(MachineSpecialization {
                    template: SymbolHandle::from_arena_index(template),
                    instance: SymbolHandle::from_arena_index(instance),
                    accepted_template_commitment: Some("retained source commitment".to_owned()),
                    ..Default::default()
                });
            }
            let mut checked = CheckedTrees::with_roots(typed, CheckFacts::default());
            checked.facts.accepted_template_identities =
                CheckedAcceptedTemplateIdentities::with_rows(vec![
                    CheckedAcceptedTemplateClassification {
                        machine: SymbolHandle::from_arena_index(1),
                        identity: Some(CheckedAcceptedTemplateIdentity {
                            report_fingerprint: 71,
                            commitment: MachineTemplateCommitment::from_digest([11; 32]),
                        }),
                    },
                    CheckedAcceptedTemplateClassification {
                        machine: SymbolHandle::from_arena_index(4),
                        identity: None,
                    },
                ]);
            checked
                .facts
                .contract_plans
                .machines_mut()
                .push(MachineContractPlan {
                    machine: SymbolHandle::from_arena_index(1),
                    closed_scalar_values: Default::default(),
                    restored_declaration_predicates: Vec::new(),
                    crash: Default::default(),
                    report_fingerprint: 37,
                    commitment: MachineContractCommitment::from_digest([7; 32]),
                });
            let declarations = collect(&checked);
            assert_eq!(declarations.machines.len(), 2);
            assert_eq!(
                declarations.machines[0].machine,
                SymbolHandle::from_arena_index(1)
            );
            let template = declarations.machines[0].template.unwrap().unwrap();
            assert_eq!(template.report_fingerprint, 71);
            assert_eq!(template.commitment, [11; 32]);
            let contract = declarations.machines[0].contract.unwrap();
            assert_eq!(contract.report_fingerprint, 37);
            assert_eq!(contract.commitment, [7; 32]);
            assert_eq!(
                declarations.machines[1].machine,
                SymbolHandle::from_arena_index(4)
            );
            assert_eq!(declarations.machines[1].template, Ok(None));
            assert_eq!(
                declarations.machines[1].contract,
                Err(TrustDeclarationCardinality::Missing)
            );
        }

        #[test]
        fn duplicate_classifications_and_contracts_are_deferred_without_selecting_the_first() {
            let mut typed = TypedTrees::default();
            typed.push_machine(machine(1, "Claim"));
            typed.push_machine(machine(2, "Unclassified"));
            let mut checked = CheckedTrees::with_roots(typed, CheckFacts::default());
            let classification = CheckedAcceptedTemplateClassification {
                machine: SymbolHandle::from_arena_index(1),
                identity: None,
            };
            checked.facts.accepted_template_identities =
                CheckedAcceptedTemplateIdentities::with_rows(vec![
                    classification.clone(),
                    classification,
                ]);
            let contract = MachineContractPlan {
                machine: SymbolHandle::from_arena_index(1),
                closed_scalar_values: Default::default(),
                restored_declaration_predicates: Vec::new(),
                crash: Default::default(),
                report_fingerprint: 37,
                commitment: MachineContractCommitment::from_digest([7; 32]),
            };
            checked
                .facts
                .contract_plans
                .machines_mut()
                .extend([contract.clone(), contract]);
            let declarations = collect(&checked);
            assert_eq!(
                declarations.machines[0].template,
                Err(TrustDeclarationCardinality::Duplicate)
            );
            assert_eq!(
                declarations.machines[0].contract,
                Err(TrustDeclarationCardinality::Duplicate)
            );
            assert_eq!(
                declarations.machines[1].template,
                Err(TrustDeclarationCardinality::Missing)
            );
            assert_eq!(
                declarations.machines[1].contract,
                Err(TrustDeclarationCardinality::Missing)
            );
        }

        #[test]
        fn an_inventory_publishes_one_trust_roster_for_its_exact_immutable_owner() {
            let checked = Arc::new(CheckedTrees::default());
            let inventory = PreparedProviderInventory::new(Arc::clone(&checked), &[]);
            assert!(std::ptr::eq(
                inventory.trust_declarations(),
                inventory.trust_declarations()
            ));
            assert!(Arc::ptr_eq(inventory.program(), &checked));
        }
    }
}
