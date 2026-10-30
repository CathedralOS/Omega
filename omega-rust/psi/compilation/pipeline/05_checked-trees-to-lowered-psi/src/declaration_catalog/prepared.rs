//! One immutable checked owner supplies both source consumers and retention.

use super::{DeclarationCatalog, project_declaration_catalog};
use checked_trees::CheckedTrees;
use std::sync::Arc;
use terminal_psi::{
    NominalDeclarationIdentity, NumericRealization, TerminalIdentityIndex,
    TerminalIdentityTableBuilder, TerminalModule,
};

pub struct PreparedDeclarationCatalog {
    checked: Arc<CheckedTrees>,
    sources: Arc<[(source::SourceId, [u8; 32])]>,
    declarations: DeclarationCatalog,
}

impl std::fmt::Debug for PreparedDeclarationCatalog {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreparedDeclarationCatalog")
            .field("sources", &self.sources)
            .field("declarations", &self.declarations)
            .finish_non_exhaustive()
    }
}

impl PartialEq for PreparedDeclarationCatalog {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.checked, &other.checked)
            && self.sources == other.sources
            && self.declarations == other.declarations
    }
}

impl Eq for PreparedDeclarationCatalog {}

impl std::ops::Deref for PreparedDeclarationCatalog {
    type Target = DeclarationCatalog;
    fn deref(&self) -> &Self::Target {
        &self.declarations
    }
}

impl PreparedDeclarationCatalog {
    /// Retain the already-projected exact bodies selected by Omega policy.
    /// The identity owner and diagnostic associations come from this product,
    /// never from another table or a second source-signature projection.
    pub fn selected_callback_roots(
        &self,
        selected: impl IntoIterator<Item = terminal_psi::CallbackUseId>,
    ) -> crate::PreparedTerminalRoots {
        self.declarations
            .callbacks
            .selected_roots(Arc::clone(&self.declarations.identities), selected)
    }

    /// Project complete declaration eligibility independently of portable demand.
    /// Unavailable evidence remains deferred until selected policy requires it.
    pub fn boundary_provider_review(
        &self,
    ) -> language_semantics::boundary_provider_review::BoundaryProviderReview<'_> {
        super::boundary_review::prepare(&self.checked.facts, &self.declarations)
    }

    /// Prepare source declarations without selecting a target or granting execution.
    pub fn prepare(
        checked: Arc<CheckedTrees>,
        sources: &[(source::SourceId, [u8; 32])],
    ) -> Result<Self, crate::LoweringError> {
        let declarations = project_declaration_catalog(&checked, sources)?;
        Ok(Self {
            checked,
            sources: sources.into(),
            declarations,
        })
    }

    /// Require the immutable source owner used to prepare this declaration product.
    /// Equal-looking trees from another constant closure are not this owner.
    pub fn require_owner(
        &self,
        checked: &CheckedTrees,
        sources: &[(source::SourceId, [u8; 32])],
    ) -> Result<(), crate::LoweringError> {
        if !std::ptr::eq(self.checked.as_ref(), checked) || self.sources.as_ref() != sources {
            return Err(crate::LoweringError::Unsupported(
                "external declarations belong to another checked program or source custody",
            ));
        }
        Ok(())
    }

    /// Demand filtering belongs to the completed module. Unused local owners
    /// remain valid source-review inputs but cannot enter portable publication.
    pub(crate) fn retain(
        &self,
        checked: &CheckedTrees,
        sources: &[(source::SourceId, [u8; 32])],
        identities: &mut TerminalIdentityTableBuilder,
        module: &mut TerminalModule,
    ) -> Result<(), crate::LoweringError> {
        self.require_owner(checked, sources)?;
        let mut retained = Vec::new();
        for candidate in &self.declarations.conformances {
            let callable = self
                .declarations
                .identities
                .text(candidate.requirement_identity)
                .ok_or(crate::LoweringError::Unsupported(
                    "external declaration lost its requirement identity",
                ))?;
            if !module
                .boundary_machines
                .iter()
                .any(|boundary| identities.text(boundary.identity) == Some(callable))
                && !module
                    .boundary_application_demands
                    .iter()
                    .any(|demand| demand.requirement().overload() == callable)
            {
                continue;
            }
            let mut candidate = candidate.clone();
            remap(&mut candidate, |identity| {
                let text = self
                    .declarations
                    .identities
                    .text(identity)
                    .ok_or("external declaration identity is absent from its owner")?;
                if self.declarations.identities.is_invocation_local(identity) {
                    identities.intern_invocation_local(text)
                } else {
                    Ok(identities.intern(text))
                }
            })
            .map_err(crate::LoweringError::Unsupported)?;
            retained.push(candidate);
        }
        retained.sort();
        retained.dedup();
        module.numeric_realizations = retained;
        Ok(())
    }
}

fn remap(
    candidate: &mut NumericRealization,
    mut map: impl FnMut(TerminalIdentityIndex) -> Result<TerminalIdentityIndex, &'static str>,
) -> Result<(), &'static str> {
    candidate.requirement = NominalDeclarationIdentity::new(map(candidate.requirement.identity())?);
    candidate.requirement_identity = map(candidate.requirement_identity)?;
    if let Some(schema) = &mut candidate.requirement_schema {
        schema.identity = map(schema.identity)?;
        schema.method = map(schema.method)?;
        schema.owner = map(schema.owner)?;
    }
    if let Some(provider) = &mut candidate.provider {
        provider.declaration =
            NominalDeclarationIdentity::new(map(provider.declaration.identity())?);
        provider.identity = map(provider.identity)?;
    }
    let declaration = &mut candidate.realization;
    declaration.machine = NominalDeclarationIdentity::new(map(declaration.machine.identity())?);
    declaration.state = NominalDeclarationIdentity::new(map(declaration.state.identity())?);
    declaration.semantic_declaration = map(declaration.semantic_declaration)?;
    declaration.callable = map(declaration.callable)?;
    declaration.result_type = map(declaration.result_type)?;
    for parameter in &mut declaration.parameter_types {
        *parameter = map(*parameter)?;
    }
    if let Some(specialization) = &mut declaration.specialization {
        specialization.template =
            NominalDeclarationIdentity::new(map(specialization.template.identity())?);
        specialization.template_callable = map(specialization.template_callable)?;
        specialization.template_semantic_declaration =
            map(specialization.template_semantic_declaration)?;
        if let Some(capture) = &mut specialization.capture {
            capture.template_identity = map(capture.template_identity)?;
        }
    }
    Ok(())
}
