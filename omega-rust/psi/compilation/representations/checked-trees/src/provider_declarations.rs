//! Exact declaration associations supplied by provider derivation.
//!
//! Target realization is already selected by the caller. Source projection
//! reads the original named application; it never chooses a target member.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NamedProviderApplication {
    pub declaration: symbols::SymbolHandle,
    /// Zero denotes the ordinary declaration scope, not a captured application.
    pub capture: arena::Handle<typed_trees::typed_trees::CapturedMachineApplication>,
}

/// Requested declaration views, not a selected provider or execution grant.
pub enum ProviderApplicationView {
    None,
    PhysicalLeaf,
    External,
}

/// The selected row's source owner. A native Build binding supplies the exact
/// public requirement, not an authored realization or a new binder telescope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderDeclarationOrigin {
    Authored(symbols::SymbolHandle),
    BuildNative { requirement: symbols::SymbolHandle },
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
    pub installation_reach: bool,
}

impl crate::CheckedTrees {
    /// Project the selected source associations without selecting any provider,
    /// interpreting a target or reporting an inapplicable source failure early.
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
                                    self.typed
                                        .trait_requirement_callable_identity_text(owner, signature)
                                        == row.requirement_identity
                                })
                        });
            let applications = if matches!(row.application, ProviderApplicationView::External) {
                self.provider_external_applications(row, row_index, request.named_application)
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
    ) -> Result<Option<language_semantics::provider_declarations::ProviderReachDeclaration>, String>
    {
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
    ) -> Result<Vec<String>, String> {
        use typed_trees::trait_definition::AppliedTraitScope;
        let typed = &self.typed;
        let Some((owner, signature)) = typed.trait_machine_signature(row.requirement) else {
            return Ok(Vec::new());
        };
        if typed.trait_requirement_callable_identity_text(owner, signature)
            != row.requirement_identity
        {
            return Err("selected Import template identity disagrees with its provenance".into());
        }
        let realization = match row.origin {
            ProviderDeclarationOrigin::Authored(symbol) => Some(
                typed
                    .machine(symbol)
                    .ok_or("selected Import provenance lost its realization declaration")?,
            ),
            ProviderDeclarationOrigin::BuildNative { requirement } => {
                if !requirement.is_valid() || requirement != row.requirement {
                    return Err("native Import origin names another exact requirement".into());
                }
                if named.is_some() {
                    return Err(
                        "native Import origin cannot substitute a named implementation".into(),
                    );
                }
                // Native intent admission currently requires this closed outer
                // declaration. Local method operands remain in the complete
                // checked requirement applications below, including consts.
                if !owner.is_boundary
                    || !typed.trait_type_parameters(owner).is_empty()
                    || !owner.lifetime_parameters.is_empty()
                {
                    return Err("native Import origin lost its closed boundary declaration".into());
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
                // Derivation retains the row before resolving captures/target members.
                // This reader consumes that association, never repeats target selection.
                if named.capture.is_valid() {
                    if !typed.captured_machine_applications.is_valid(named.capture) {
                        return Err(
                            "selected Import lost its captured implementation application".into(),
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
            Some(AppliedTraitScope::abstract_scope(owner))
        }
        .ok_or("selected Import lost its complete declaring application")?;
        let mut identities = Vec::new();
        for application in typed
            .closed_requirement_applications()
            .filter(|application| {
                application.template == row.requirement
                    && typed
                        .requirement_application_matches_scope(application.signature.symbol, &scope)
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
            identities
                .push(typed.trait_requirement_callable_identity_text(owner, application.signature));
        }
        Ok(identities)
    }
}

pub fn closed_provider_leaf_applications(
    typed: &typed_trees::TypedTrees,
    template: &str,
) -> Result<Vec<String>, Vec<String>> {
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
        let identity = typed.trait_requirement_callable_identity_text(owner, application.signature);
        if !identity.is_empty() && identity != template {
            identities.push(identity);
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
