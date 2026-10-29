//! Rejoin a package's executed provider choice in a receiving Product world.
//!
//! Declaration anchors transfer the selection already made by the supplier;
//! they neither re-resolve its private names under the consumer nor transfer
//! visibility. Role and conformance admission still run on the receiving trees.

use super::{
    ProviderPlanningContext, ProviderSelection, ProviderSelectionIdentity,
    ProviderSelectionSubject, admission,
};
use language_semantics::declaration_selection::{
    BuildSelectionOrigin, CompositionMode, ProductTarget,
};
use symbols::RetainedDeclaration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedProviderSelection {
    pub subject: RetainedDeclaration,
    pub subject_identity: String,
    pub provider: RetainedDeclaration,
    pub provider_identity: String,
    pub target: ProductTarget,
    pub composition_mode: CompositionMode,
    pub origin: BuildSelectionOrigin,
}

impl RetainedProviderSelection {
    pub fn capture(
        typed: &typed_trees::TypedTrees,
        selection: &ProviderSelection,
    ) -> Result<Self, String> {
        let subject = match &selection.subject {
            ProviderSelectionSubject::BoundaryTrait(identity)
            | ProviderSelectionSubject::BoundaryRequirement(identity) => identity.symbol,
            ProviderSelectionSubject::BoundaryOperatorFamily(family) => {
                family
                    .coordinates()
                    .first()
                    .ok_or("retained provider family has no declaration")?
                    .symbol
            }
        };
        let capture = |symbol| {
            let selected = RetainedDeclaration::capture(&typed.symbols, symbol)?;
            RetainedDeclaration::from_retained_parts(
                selected.source().clone(),
                selected.span(),
                selected.kind(),
            )
        };
        Ok(Self {
            subject: capture(subject).map_err(str::to_owned)?,
            subject_identity: typed.normalized_hermetic_symbol_identity(subject)?,
            provider: capture(selection.provider_type.symbol).map_err(str::to_owned)?,
            provider_identity: typed
                .normalized_hermetic_symbol_identity(selection.provider_type.symbol)?,
            target: selection
                .target
                .ok_or("an executed package provider choice has no target key")?,
            composition_mode: selection.composition_mode,
            origin: selection.origin.clone(),
        })
    }

    pub fn rejoin(
        &self,
        context: ProviderPlanningContext<'_>,
        previous: &[ProviderSelection],
    ) -> Result<ProviderSelection, diagnostics::Diagnostic> {
        let identity = |anchor: &RetainedDeclaration, expected: &str| {
            let program = context.typed;
            let symbol = anchor
                .rejoin(
                    &program.symbols,
                    program
                        .symbols
                        .symbols()
                        .nodes()
                        .iter()
                        .map(|(symbol, _)| symbol)
                        .filter(|symbol| {
                            if program.symbols.get(*symbol).kind != anchor.kind()
                                || program
                                    .symbols
                                    .symbol_source_span(*symbol)
                                    .is_none_or(|span| span.span != anchor.span())
                            {
                                return false;
                            }
                            program
                                .normalized_hermetic_symbol_identity(*symbol)
                                .is_ok_and(|actual| actual == expected)
                        }),
                )
                .map_err(diagnostics::Diagnostic::error)?;
            Ok::<_, diagnostics::Diagnostic>(ProviderSelectionIdentity {
                symbol,
                package: program.symbols.symbol_package_identity(symbol),
                canonical_path: program.symbols.display_path(symbol, "::"),
                authored_path: program.symbols.display_path(symbol, "::"),
            })
        };
        admission::admit(
            context,
            identity(&self.subject, &self.subject_identity)?,
            identity(&self.provider, &self.provider_identity)?,
            self.composition_mode,
            Some(self.target),
            self.origin.clone(),
            previous,
        )
    }
}
