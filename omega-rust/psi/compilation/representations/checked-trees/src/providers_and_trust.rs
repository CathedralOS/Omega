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
        pub declared_requirement_identity: &'identity str,
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
                        if matches!(row.origin, ProviderDeclarationOrigin::BuildNative { .. }) {
                            self.native_requirement_scope(
                                row,
                                request.provider_application_identity,
                            )
                            .map_err(|reason| vec![reason])
                            .and_then(|(_, closed)| {
                                if closed.is_some() {
                                    Ok(Vec::new())
                                } else {
                                    closed_provider_leaf_applications(
                                        &self.typed,
                                        row.requirement_identity,
                                    )
                                }
                            })
                        } else {
                            closed_provider_leaf_applications(&self.typed, row.requirement_identity)
                        }
                    } else {
                        Ok(Vec::new())
                    };
                let has_closed_application =
                    matches!(row.application, ProviderApplicationView::External)
                        && row.requirement_identity == row.declared_requirement_identity
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
            if identity != row.declared_requirement_identity {
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
                    requirement_identity: row.requirement_identity.to_owned(),
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

        // Bindings retain one checked application even when several methods share
        // the same whole boundary plan. Authenticate that application before
        // publishing either direct rows or aliases for an open method.
        fn native_requirement_scope<'a>(
            &'a self,
            row: &ProviderDeclarationRow<'_>,
            provider_application_identity: Option<&str>,
        ) -> Result<
            (
                typed_trees::trait_definition::AppliedTraitScope<'a>,
                Option<symbols::SymbolHandle>,
            ),
            String,
        > {
            use typed_trees::trait_definition::AppliedTraitScope;
            let typed = &self.typed;
            let ProviderDeclarationOrigin::BuildNative {
                requirement,
                application,
            } = row.origin
            else {
                return Err("native binding lost its exact origin".into());
            };
            let (owner, signature) = typed
                .trait_machine_signature(row.requirement)
                .filter(|(owner, _)| owner.is_boundary)
                .ok_or("native binding lost its declared boundary method")?;
            if requirement != row.requirement
                || !requirement.is_valid()
                || typed.trait_requirement_callable_identity_text(owner, signature)
                    != row.declared_requirement_identity
            {
                return Err("native binding changed its exact requirement template".into());
            }
            let Some(handle) = application else {
                if provider_application_identity.is_some()
                    || row.requirement_identity != row.declared_requirement_identity
                    || !typed.trait_type_parameters(owner).is_empty()
                    || !owner.lifetime_parameters.is_empty()
                {
                    return Err("native binding lost its closed boundary declaration".into());
                }
                return Ok((AppliedTraitScope::abstract_scope(owner), None));
            };
            if !typed.product_operand_applications.is_valid(handle) {
                return Err("native binding has a stale Product application".into());
            }
            let prepared = typed.product_operand_applications.get(handle);
            if prepared.role
                != Some(language_semantics::declaration_selection::ProductOperandRole::Requirement)
            {
                return Err("native binding has another Product operand role".into());
            }
            let closed = typed.closed_requirement_application(prepared.declaration);
            if closed
                .as_ref()
                .map_or(prepared.declaration, |closed| closed.template)
                != requirement
            {
                return Err("native binding changed its exact requirement template".into());
            }
            let scope = prepared
                .scope
                .scope(typed)
                .filter(|scope| scope.declaration.symbol == owner.symbol)
                .ok_or("native binding lost its exact applied boundary scope")?;
            let outer = scope
                .portable_application_identity(typed)
                .and_then(|identity| typed.type_identity_text(identity))
                .ok_or("native binding lost its portable outer application")?;
            if provider_application_identity != Some(outer.as_str()) {
                return Err("native binding differs from the selected boundary application".into());
            }
            let selected = closed.as_ref().map_or_else(
                || row.declared_requirement_identity.to_owned(),
                |closed| typed.trait_requirement_callable_identity_text(owner, closed.signature),
            );
            if row.requirement_identity != selected {
                return Err("native binding differs from its selected method application".into());
            }
            Ok((scope, closed.map(|closed| closed.signature.symbol)))
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
                != row.declared_requirement_identity
            {
                return Err(
                    "selected Import template identity disagrees with its provenance".into(),
                );
            }
            let mut native_scope = None;
            let mut native_closed = None;
            let realization = match row.origin {
                ProviderDeclarationOrigin::Authored(symbol) => Some({
                    if row.requirement_identity != row.declared_requirement_identity {
                        return Err(
                            "authored Import substituted an unbound closed application".into()
                        );
                    }
                    typed
                        .machine(symbol)
                        .ok_or("selected Import provenance lost its realization declaration")?
                }),
                ProviderDeclarationOrigin::BuildNative { .. } => {
                    if named.is_some() {
                        return Err(
                            "native Import origin cannot substitute a named implementation".into(),
                        );
                    }
                    let (scope, closed) =
                        self.native_requirement_scope(row, provider_application_identity)?;
                    native_scope = Some(scope);
                    native_closed = closed;
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
            if native_closed.is_some() {
                return Ok(Vec::new());
            }
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
