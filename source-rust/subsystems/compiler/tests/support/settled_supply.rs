//! Exercise source choices through the canonical native entrance in tests.
use diagnostics::Diagnostic;
use provider_realization::target_supply::{NormalizedNativeSupplies, NormalizedNativeSupply};
use semantic_vocabulary::BoundaryMachineId;
use source_compiler::provider_choice_inputs::ProviderSelectionSubject;
use source_compiler::provider_supply::{SettledTargetSupply, SupplySelection};
use terminal_psi::TerminalModule;

pub trait SettledSupplyNative {
    fn native_bindings(
        &self,
        module: &TerminalModule,
    ) -> Result<NormalizedNativeSupplies, Vec<Diagnostic>>;
    fn realize_no_demand(
        &self,
        module: &TerminalModule,
    ) -> Result<provider_realization::target_supply::RealizedTargetSupply, Vec<Diagnostic>>;
    fn realize(
        &self,
        planning: SupplyPlanning<'_>,
    ) -> Result<provider_realization::target_supply::RealizedTargetSupply, Vec<Diagnostic>>;
    fn plans(
        &self,
        planning: SupplyPlanning<'_>,
    ) -> Result<Vec<effects::provider_plan::ProviderPlan>, Vec<Diagnostic>>;
}
impl SettledSupplyNative for SettledTargetSupply {
    /// Normalize only selected native leaves, retaining the complete Build
    /// receipt and its identity table. A row grants no execution permission.
    fn native_bindings(
        &self,
        module: &TerminalModule,
    ) -> Result<NormalizedNativeSupplies, Vec<Diagnostic>> {
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
        let mut rows = Vec::new();
        for requirement in self.requirements() {
            let SupplySelection::Native(supply) = &requirement.supply.selection else {
                continue;
            };
            let refuse = |reason| {
                vec![Diagnostic::error(format!(
                    "native supply for `{}`: {reason}",
                    requirement.reached.name
                ))]
            };
            let declaration = module
                .boundary_machine(requirement.reached.boundary)
                .and_then(|boundary| boundary.declaration.as_ref())
                .ok_or_else(|| {
                    refuse("the accepted module has no exact callable declaration".to_owned())
                })?;
            let callable = module
                .identities
                .text(declaration.source.callable)
                .filter(|identity| !identity.is_empty())
                .ok_or_else(|| {
                    refuse("the accepted requirement has no complete callable identity".to_owned())
                })?;
            let binding = provider_realization::native_bindings::normalize_native_binding(
                &supply.materialization(),
                callable,
                &declaration.service_declaration,
                &mut identities,
            )
            .map_err(refuse)?;
            if Some(binding.target().target_name()) != self.target() {
                return Err(vec![Diagnostic::error(
                    "normalized native binding changed its settled deployment",
                )]);
            }
            rows.push(NormalizedNativeSupply {
                boundary: requirement.reached.boundary,
                binding,
            });
        }
        Ok(NormalizedNativeSupplies {
            identities: identities.build(),
            rows,
        })
    }

    /// The demand walk established that no supply is needed. A nonempty
    /// choice needs the canonical row realization operation instead.
    fn realize_no_demand(
        &self,
        module: &TerminalModule,
    ) -> Result<provider_realization::target_supply::RealizedTargetSupply, Vec<Diagnostic>> {
        if !self.requirements().is_empty() {
            return Err(vec![Diagnostic::error(
                "nonempty provider supply needs its exact accepted module and execution inputs",
            )]);
        }
        let Some(target) = self.target() else {
            return provider_realization::target_supply::RealizedTargetSupply::unscoped(
                module,
                &[],
            )
            .map_err(|reason| vec![Diagnostic::error(reason)]);
        };
        provider_realization::target_supply::RealizedTargetSupply::from_checked(
            module,
            provider_realization::target_supply::TargetSupplyRealizationInput {
                target: target.to_owned(),
                reached: Vec::new(),
                plans: Vec::new(),
                direct_boundary_leaves: Vec::new(),
                calling_applications: Vec::new(),
                external_bindings: Vec::new(),
                evaluated_native_bindings: Vec::new(),
                evaluated_binding_identities: Default::default(),
                terminal_authority_permissions: Vec::new(),
                terminal_authority_permission_identities: Default::default(),
            },
        )
        .map_err(|reason| vec![Diagnostic::error(reason)])
    }

    /// Compatibility source choices can only request the plans already chosen
    /// and proved in this module. No choice or candidate is inferred here.
    fn realize(
        &self,
        planning: SupplyPlanning<'_>,
    ) -> Result<provider_realization::target_supply::RealizedTargetSupply, Vec<Diagnostic>> {
        if self.requirements().is_empty() {
            return self.realize_no_demand(planning.module);
        }
        let target = self.target().ok_or_else(|| {
            vec![Diagnostic::error(
                "provider settlement lost its deployment target",
            )]
        })?;
        let module = planning.module;
        let installation_requirements = planning.installation_requirements;
        let refuse = |reason: &str| {
            vec![Diagnostic::error(format!(
                "provider settlement for `{target}`: {reason}",
            ))]
        };
        let mut reached = Vec::with_capacity(self.requirements().len());
        let mut diagnostic_choices = Vec::with_capacity(self.requirements().len());
        for requirement in self.requirements() {
            let Some(selection) = requirement.supply.checked() else {
                reached.push(requirement.reached.boundary);
                continue;
            };
            let machine = requirement.realization.ok_or_else(|| {
                refuse(&format!(
                    "`{}` is supplied by `{}`, which the accepted module does not hold as a judged provider",
                    requirement.reached.name, selection.provider_type.authored_path,
                ))
            })?;
            let ProviderSelectionSubject::BoundaryTrait(slot) = &selection.subject else {
                return Err(refuse(
                    "provider plan needs its exact whole boundary trait selection",
                ));
            };
            let mut schemas = planning.schemas.iter().filter(|schema| {
                schema.target == target
                    && schema.slot == slot.symbol
                    && slot.application_identity.as_ref().is_none_or(|identity| {
                        schema.surface.root_application_identity() == Some(identity.as_str())
                    })
            });
            let (Some(source), None) = (schemas.next(), schemas.next()) else {
                return Err(refuse(
                    "selected slot has no unique complete schema correspondence",
                ));
            };
            let mut schemas = module.service_schemas.iter().filter(|schema| {
                module.identities.text(schema.declaration) == Some(source.declaration.as_str())
                    && module.identities.text(schema.application)
                        == source.surface.root_application_identity()
                    && schema.surface == source.surface
            });
            let (Some(schema), None) = (schemas.next(), schemas.next()) else {
                return Err(refuse(
                    "selected slot schema has no unique published declaration application",
                ));
            };
            module
                .service_schema(schema.declaration, schema.application)
                .map_err(|reason| refuse(&reason))?;
            if slot.applied && slot.application_identity.is_none()
                || slot.application_identity.as_ref().is_some_and(|identity| {
                    module.identities.text(schema.application) != Some(identity.as_str())
                })
            {
                return Err(refuse(
                    "selected slot application differs from its published schema",
                ));
            }
            if module
                .boundary_machine(requirement.reached.boundary)
                .is_none()
                || module.machine(machine).is_none()
            {
                return Err(refuse(
                    "selected provider lost its actual boundary or machine",
                ));
            }
            let mut candidates = module.provider_candidates.iter().filter(|candidate| {
                candidate.boundary == requirement.reached.boundary && candidate.candidate == machine
            });
            let (Some(candidate), None) = (candidates.next(), candidates.next()) else {
                return Err(refuse(
                    "selected provider realization has no unique exact checked candidate",
                ));
            };
            let mut choices = module.provider_selections.iter().filter(|choice| {
                choice.target == target
                    && choice.schema_declaration == schema.declaration
                    && choice.schema_application == schema.application
                    && choice.provider_declaration == candidate.provider_declaration.identity()
                    && choice.provider_identity == candidate.provider_identity
            });
            let (Some(choice), None) = (choices.next(), choices.next()) else {
                return Err(refuse(
                    "selected provider has no unique prepublished target choice",
                ));
            };
            if choice.composition_mode != selection.composition_mode
                || choice.provider_package != selection.provider_type.package
                || selection.provider_type.applied
                    && selection.provider_type.application_identity.is_none()
                || choice
                    .provider_application
                    .and_then(|identity| module.identities.text(identity))
                    != selection.provider_type.application_identity.as_deref()
            {
                return Err(refuse(
                    "selected provider application, package or composition differs from its published choice",
                ));
            }
            diagnostic_choices.push((choice, selection.provider_type.authored_path.as_str()));
            reached.push(requirement.reached.boundary);
        }
        reached.sort_unstable();
        reached.dedup();
        let logical = terminal_semantics::logical_providers::ClosedLogicalProviders::select(
            module,
            terminal_codec::terminal_psi_identity(module)
                .map_err(|error| refuse(&format!("{error:?}")))?,
            target,
            &reached,
        )
        .map_err(|error| refuse(&realization_diagnostic(error, module, &diagnostic_choices)))?;
        let realization = provider_realization::target_supply::realize_target_supply(
            provider_realization::target_supply::TargetSupplyRealizationRequest::from_execution_inputs(
                module,
                &logical,
                planning.execution,
            ),
        )
        .map_err(|error| refuse(&realization_diagnostic(error, module, &diagnostic_choices)))?;
        realization
            .resolve_installation_reach(module, installation_requirements)
            .map_err(|reason| refuse(&reason))
    }

    fn plans(
        &self,
        planning: SupplyPlanning<'_>,
    ) -> Result<Vec<effects::provider_plan::ProviderPlan>, Vec<Diagnostic>> {
        if self
            .requirements()
            .iter()
            .all(|requirement| requirement.supply.checked().is_none())
        {
            self.native_bindings(planning.module)?;
            return Ok(Vec::new());
        }
        self.realize(planning)
            .map(|realized| realized.provider_declarations().to_vec())
    }
}

fn realization_diagnostic(
    error: provider_realization::target_supply::TargetSupplyRealizationError,
    module: &TerminalModule,
    choices: &[(&terminal_psi::TerminalProviderSelection, &str)],
) -> String {
    use provider_realization::target_supply::TargetSupplyRealizationError;

    let TargetSupplyRealizationError::MissingCheckedRow {
        provider_declaration,
        schema_declaration,
        schema_application,
        requirement_identity,
        ..
    } = &error
    else {
        return error.diagnostic(module);
    };
    let mut labels = choices.iter().filter(|(choice, _)| {
        choice.provider_declaration == *provider_declaration
            && choice.schema_declaration == *schema_declaration
            && choice.schema_application == *schema_application
    });
    let Some((_, provider_path)) = labels.next() else {
        return error.diagnostic(module);
    };
    if provider_path.is_empty() || labels.any(|(_, other)| other != provider_path) {
        return error.diagnostic(module);
    }
    let Ok(schema) = module.service_schema(*schema_declaration, *schema_application) else {
        return error.diagnostic(module);
    };
    let mut methods = schema.surface.methods.iter().filter(|method| {
        schema.surface.identity_text(method.requirement_identity) == requirement_identity.as_str()
    });
    let (Some(method), None) = (methods.next(), methods.next()) else {
        return error.diagnostic(module);
    };
    format!(
        "`{provider_path}` supplies `{}::{}` with no checked machine the accepted module states",
        method.requirement_owner, method.name,
    )
}

/// Exact source schema correspondence; the full surface must already be in
/// the accepted module. The source slot is never used as executable identity.
#[derive(Debug, Clone)]
pub struct SlotSchema {
    pub target: String,
    pub slot: symbols::SymbolHandle,
    pub declaration: String,
    pub surface: std::sync::Arc<effects::provider_plan::ServiceSurface>,
}

pub struct SupplyPlanning<'a> {
    pub module: &'a TerminalModule,
    pub schemas: &'a [SlotSchema],
    pub execution: provider_realization::target_supply::TargetSupplyExecutionInputs,
    pub installation_requirements: &'a [BoundaryMachineId],
}

/// Project each accepted target's exact declared application using the same
/// frozen source and application environment retained by the portable producer.
pub fn published_slot_schemas(
    publication: &source_compiler::SourcePublication,
) -> Result<Vec<SlotSchema>, Vec<Diagnostic>> {
    let typed = publication.accepted()?.source();
    publication
        .roster()?
        .normalization
        .providers
        .iter()
        .map(|selected| {
            let scope = selected.schema.scope(typed).ok_or_else(|| {
                vec![Diagnostic::error("selected provider lost its source schema application")]
            })?;
            let slot = scope.declaration.symbol;
            let declaration = typed
                .normalized_hermetic_symbol_identity(slot)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            let projected = source_compiler::source_state::service_declarations::projection::from_typed_application(
                source_compiler::source_state::provider_declarations::ProviderPlanningContext::new(
                    typed,
                    declaration_contracts::source_identity::DependencyScope::Product,
                ),
                scope,
            )
            .map_err(|diagnostic| vec![diagnostic])?;
            projected.validate_correspondence().map_err(|diagnostic| vec![diagnostic])?;
            Ok(SlotSchema {
                target: selected.target.clone(),
                slot,
                declaration,
                surface: projected.declaration,
            })
        })
        .collect()
}
