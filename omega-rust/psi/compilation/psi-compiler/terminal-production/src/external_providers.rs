/*
    External provider declarations as retained data: the nominal identities,
    conformances and callback declarations that provider selection and package
    review read for a checked program.

    The projection that filled these tables walked checked trees inside the
    removed checked-tree lowering stage. Nothing projects them from the lowered
    route's published module yet, so both constructors refuse with
    `NoProductRoute` and the readers below stay as the data contract the
    replacement producer has to meet.
*/

use std::sync::{Arc, Weak};
use symbols::SymbolHandle;
use terminal_psi::{
    ExternalProviderConformance, NominalDeclarationIdentity, PreparedCallbackDeclarations,
    TerminalIdentityTable,
};
use typed_trees::FrozenTypedTrees;

/// The refusal a caller receives where the removed stage used to produce a value.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct NoProductRoute(&'static str);

impl std::fmt::Debug for NoProductRoute {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "no product route yet: {}", self.0)
    }
}

const NO_PROJECTION: NoProductRoute =
    NoProductRoute("external provider declarations have no producer");

#[derive(Debug, Clone, PartialEq, Eq)]
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

    pub fn has_requirement_schema(&self, symbol: SymbolHandle) -> bool {
        let Some(nominal) = self.nominal(symbol) else {
            return false;
        };
        self.conformances.iter().any(|candidate| {
            self.identities.text(candidate.requirement.identity()) == Some(nominal)
                && candidate.requirement_schema.is_some()
        })
    }

    pub fn nominal(&self, symbol: SymbolHandle) -> Option<&str> {
        let (_, declaration) = self
            .declarations
            .iter()
            .find(|(source, _)| *source == symbol)?;
        self.identities.text(declaration.as_ref().ok()?.identity())
    }
}

pub fn project_external_provider_declarations(
    _checked: &FrozenTypedTrees,
    _sources: &[(source::SourceId, [u8; 32])],
) -> Result<ExternalProviderDeclarations, NoProductRoute> {
    Err(NO_PROJECTION)
}

pub struct PreparedExternalProviderDeclarations {
    checked: Weak<FrozenTypedTrees>,
    sources: Arc<[(source::SourceId, [u8; 32])]>,
    declarations: ExternalProviderDeclarations,
    review: language_semantics::boundary_provider_review::RetainedBoundaryProviderReview,
}

impl std::fmt::Debug for PreparedExternalProviderDeclarations {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreparedExternalProviderDeclarations")
            .field("sources", &self.sources)
            .field("declarations", &self.declarations)
            .finish_non_exhaustive()
    }
}

impl PartialEq for PreparedExternalProviderDeclarations {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.checked, &other.checked)
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
    pub fn selected_callback_roots(
        &self,
        selected: impl IntoIterator<Item = terminal_psi::CallbackUseId>,
    ) -> terminal_psi::PreparedTerminalRoots {
        self.declarations
            .callbacks
            .selected_roots(Arc::clone(&self.declarations.identities), selected)
    }

    pub fn boundary_provider_review(
        &self,
    ) -> language_semantics::boundary_provider_review::BoundaryProviderReview<'_> {
        self.review.borrow()
    }

    pub fn prepare(
        _checked: Arc<FrozenTypedTrees>,
        _sources: &[(source::SourceId, [u8; 32])],
    ) -> Result<Self, NoProductRoute> {
        Err(NO_PROJECTION)
    }

    pub fn require_owner(
        &self,
        checked: &FrozenTypedTrees,
        sources: &[(source::SourceId, [u8; 32])],
    ) -> Result<(), NoProductRoute> {
        self.require_owner_address(checked, sources)
    }

    pub fn require_prepared_owner(
        &self,
        checked: &Weak<FrozenTypedTrees>,
        sources: &[(source::SourceId, [u8; 32])],
    ) -> Result<(), NoProductRoute> {
        self.require_owner_address(checked.as_ptr(), sources)
    }

    fn require_owner_address(
        &self,
        checked: *const FrozenTypedTrees,
        sources: &[(source::SourceId, [u8; 32])],
    ) -> Result<(), NoProductRoute> {
        if self.checked.as_ptr() != checked || self.sources.as_ref() != sources {
            return Err(NoProductRoute(
                "external declarations belong to another frozen source owner or source custody",
            ));
        }
        Ok(())
    }
}
