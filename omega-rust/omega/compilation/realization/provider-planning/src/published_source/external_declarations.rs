//! Native provider review consumes declarations already retained by the
//! accepted module. A source `satisfies` spelling cannot manufacture one.

use crate::PublishedProviderSource;
use diagnostics::Diagnostic;
use language_semantics::boundary_provider_review::{
    BoundaryProviderReview, BoundaryRequirementReview, BoundaryRequirementUse,
    RetainedBoundaryProviderReview,
};
use std::sync::{Arc, Weak};
use symbols::SymbolHandle;
use terminal_psi::{
    ExternalProviderConformance, NominalDeclarationIdentity, PreparedCallbackDeclarations,
    TerminalIdentityTable,
};

#[derive(Debug, PartialEq, Eq)]
pub struct ExternalProviderDeclarations {
    pub identities: Arc<TerminalIdentityTable>,
    pub conformances: Vec<ExternalProviderConformance>,
    pub callbacks: PreparedCallbackDeclarations,
    declarations: Vec<(
        SymbolHandle,
        Result<NominalDeclarationIdentity, &'static str>,
    )>,
}
impl ExternalProviderDeclarations {
    pub fn correspondence(
        &self,
    ) -> &[(
        SymbolHandle,
        Result<NominalDeclarationIdentity, &'static str>,
    )] {
        &self.declarations
    }
    pub fn nominal(&self, symbol: SymbolHandle) -> Option<&str> {
        let mut rows = self
            .declarations
            .iter()
            .filter(|(source, _)| *source == symbol);
        let (Some((_, declaration)), None) = (rows.next(), rows.next()) else {
            return None;
        };
        self.identities.text(declaration.as_ref().ok()?.identity())
    }
    pub fn has_requirement_schema(&self, symbol: SymbolHandle) -> bool {
        self.nominal(symbol).is_some_and(|nominal| {
            self.conformances.iter().any(|row| {
                self.identities.text(row.requirement.identity()) == Some(nominal)
                    && row.requirement_schema.is_some()
            })
        })
    }
}

#[derive(Debug)]
pub struct PreparedExternalProviderDeclarations {
    source: Weak<PublishedProviderSource>,
    sources: Arc<[(source::SourceId, [u8; 32])]>,
    declarations: ExternalProviderDeclarations,
    review: RetainedBoundaryProviderReview,
}
impl PartialEq for PreparedExternalProviderDeclarations {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.source, &other.source)
            && self.sources == other.sources
            && self.declarations == other.declarations
            && self.review == other.review
    }
}
impl Eq for PreparedExternalProviderDeclarations {}
impl std::ops::Deref for PreparedExternalProviderDeclarations {
    type Target = ExternalProviderDeclarations;
    fn deref(&self) -> &Self::Target {
        &self.declarations
    }
}
impl PreparedExternalProviderDeclarations {
    pub fn prepare(
        source: Arc<PublishedProviderSource>,
        sources: &[(source::SourceId, [u8; 32])],
    ) -> Result<Self, Vec<Diagnostic>> {
        let module = source.module();
        if !module.callback_applications.is_empty()
            || !module.installed_interrupt_registrations.is_empty()
        {
            return Err(vec![Diagnostic::error(
                "published callbacks need their complete requirement, entry-resource and occurrence receipt roster",
            )]);
        }
        if !module.boundary_application_demands.is_empty() {
            return Err(vec![Diagnostic::error(
                "published provider review needs exact applied boundary-operator receipt rows",
            )]);
        }
        let mut declarations = Vec::new();
        for (id, symbol) in source.correspondence().machines.iter() {
            let nominal = module
                .machine(id)
                .and_then(|machine| machine.declaration.as_ref())
                .map(|declaration| declaration.machine.clone())
                .ok_or("accepted machine lost its nominal declaration");
            declarations.push((*symbol, nominal));
        }
        for (id, symbol) in source.correspondence().boundaries.iter() {
            let nominal = module
                .boundary_machine(id)
                .and_then(|boundary| boundary.declaration.as_ref())
                .map(|declaration| declaration.source.machine.clone())
                .ok_or("accepted boundary lost its nominal declaration");
            declarations.push((*symbol, nominal));
        }
        // Native leaves have checked headers but no executable MachineId.
        // Rejoin their real source declaration by complete canonical identity
        // and target metadata, never by a readable path or the first match.
        for external in &module.external_providers {
            let expected = module
                .identities
                .text(external.realization.semantic_declaration)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "external realization lost its accepted semantic declaration",
                    )]
                })?;
            let nominal = module
                .identities
                .text(external.realization.machine.identity())
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "external realization lost its accepted nominal declaration",
                    )]
                })?;
            let callable = module
                .identities
                .text(external.realization.callable)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "external realization lost its accepted callable declaration",
                    )]
                })?;
            let mut matching = source.typed.machines().iter().filter(|machine| {
                machine
                    .target
                    .as_ref()
                    .or(machine.authored_target.as_ref())
                    .map(|target| target.as_str())
                    == external.target.as_deref()
                    && source
                        .typed
                        .normalized_hermetic_symbol_identity(machine.symbol)
                        .as_deref()
                        == Ok(expected)
                    && source
                        .typed
                        .exact_owner_nominal_identity(machine.symbol, sources)
                        .and_then(|identity| source.typed.type_identity_text(identity))
                        .as_deref()
                        == Some(nominal)
                    && source
                        .typed
                        .machine_callable_identity_text(machine)
                        .as_deref()
                        == Some(callable)
            });
            let (Some(machine), None) = (matching.next(), matching.next()) else {
                return Err(vec![Diagnostic::error(
                    "external realization has no unique exact source declaration and target",
                )]);
            };
            if source.typed.symbols.symbol_package_identity(machine.symbol)
                != external.realization_package
            {
                return Err(vec![Diagnostic::error(
                    "external realization changed its exact source package",
                )]);
            }
            declarations.push((machine.symbol, Ok(external.realization.machine.clone())));
            let provider = typed_trees::provider_declarations::satisfaction_provider_declaration(
                &source.typed,
                machine,
            );
            match (&external.provider, provider) {
                (None, None) => {}
                (Some(owner), Some(symbol)) => {
                    let semantic = source
                        .typed
                        .normalized_hermetic_symbol_identity(symbol)
                        .map_err(|reason| vec![Diagnostic::error(reason)])?;
                    let nominal = source
                        .typed
                        .exact_owner_nominal_identity(symbol, sources)
                        .and_then(|identity| source.typed.type_identity_text(identity))
                        .ok_or_else(|| {
                            vec![Diagnostic::error(
                                "native provider owner lost its exact nominal declaration",
                            )]
                        })?;
                    if module.identities.text(owner.identity) != Some(semantic.as_str())
                        || module.identities.text(owner.declaration.identity())
                            != Some(nominal.as_str())
                        || source.typed.symbols.symbol_package_identity(symbol) != owner.package
                    {
                        return Err(vec![Diagnostic::error(
                            "native provider owner differs from the accepted semantic declaration, nominal identity or package",
                        )]);
                    }
                    declarations.push((symbol, Ok(owner.declaration.clone())));
                }
                _ => {
                    return Err(vec![Diagnostic::error(
                        "native provider changed its exact declaring source owner",
                    )]);
                }
            }
        }
        declarations.sort_by_key(|(symbol, _)| (symbol.arena_index(), symbol.generation()));
        declarations.dedup();
        let mut uses = Vec::new();
        for machine in &module.machines {
            for call in machine.boundary_invocations() {
                let boundary = module.boundary_machine(call.boundary).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "provider review invocation has no accepted boundary",
                    )]
                })?;
                let symbol = source
                    .correspondence()
                    .boundaries
                    .get(call.boundary)
                    .copied()
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "provider review boundary has no exact source coordinate",
                        )]
                    })?;
                let schema = module.identities.text(boundary.identity).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "provider review boundary lost its callable identity",
                    )]
                })?;
                let numeric = boundary
                    .declaration
                    .as_ref()
                    .and_then(|declared| module.identities.text(declared.source.machine.identity()))
                    .ok_or("provider requirement has no checked nominal declaration");
                uses.push(BoundaryRequirementUse {
                    requirement: BoundaryRequirementReview {
                        schema_identity: schema,
                        package_identity: source.typed.symbols.symbol_package_identity(symbol),
                        numeric_requirement: numeric,
                    },
                    diagnostic_name: source.typed.symbols.name(symbol),
                });
            }
        }
        let review = BoundaryProviderReview {
            declarations: Vec::new(),
            operator_uses: Vec::new(),
            requirement_uses: uses,
        }
        .retain();
        Ok(Self {
            source: Arc::downgrade(&source),
            sources: sources.into(),
            review,
            declarations: ExternalProviderDeclarations {
                identities: Arc::new(module.identities.clone()),
                conformances: module.external_providers.clone(),
                // Absence is established by the accepted module's complete
                // callback/registration census above, not by missing facts.
                callbacks: PreparedCallbackDeclarations::default(),
                declarations,
            },
        })
    }
    pub fn require_owner(
        &self,
        source: &PublishedProviderSource,
        sources: &[(source::SourceId, [u8; 32])],
    ) -> Result<(), &'static str> {
        if !std::ptr::eq(self.source.as_ptr(), source) || self.sources.as_ref() != sources {
            return Err(
                "external provider review belongs to another published owner or source custody",
            );
        }
        Ok(())
    }
    pub fn require_prepared_owner(
        &self,
        source: &Weak<PublishedProviderSource>,
        sources: &[(source::SourceId, [u8; 32])],
    ) -> Result<(), &'static str> {
        if !Weak::ptr_eq(&self.source, source) || self.sources.as_ref() != sources {
            return Err(
                "external provider review belongs to another published owner or source custody",
            );
        }
        Ok(())
    }
    pub fn boundary_provider_review(&self) -> BoundaryProviderReview<'_> {
        self.review.borrow()
    }
    pub fn selected_callback_roots(
        &self,
        selected: impl IntoIterator<Item = terminal_psi::CallbackUseId>,
    ) -> terminal_psi::PreparedTerminalRoots {
        self.callbacks
            .selected_roots(Arc::clone(&self.identities), selected)
    }
}
