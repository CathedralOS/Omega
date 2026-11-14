//! A package hands the provider choices its Build made to the packages that
//! depend on it. A choice leaves the typing it was admitted in as source
//! anchors: each declaration as the source that declares it, beside the
//! identity it had there. A consumer types that source itself, so its symbols
//! and source numbers differ; it finds the same declarations again from the
//! anchors and admits the pair again among its own.

use super::provider_declarations::{
    ProviderPlanningContext, ProviderSelection, ProviderSelectionSubject,
    RetainedProviderSelection, form, identity,
};
use diagnostics::Diagnostic;
use symbols::RetainedDeclaration;
use typed_trees::TypedTrees;

/// What an anchor cannot carry yet, refused by name: an application of a
/// declaration to static arguments needs its prepared operand beside it.
const APPLIED: &str = "no product route yet: retained source provider selection: a slot or provider applied to static arguments";

impl RetainedProviderSelection {
    /// `selection` as the anchors a consumer can rejoin.
    pub fn capture(typed: &TypedTrees, selection: &ProviderSelection) -> Result<Self, String> {
        let subject = match &selection.subject {
            ProviderSelectionSubject::BoundaryTrait(identity)
            | ProviderSelectionSubject::BoundaryRequirement(identity) => identity,
            ProviderSelectionSubject::BoundaryOperatorFamily(_) => {
                return Err("no product route yet: retained source provider selection: a boundary-operator family as the slot".to_owned());
            }
        };
        let provider = &selection.provider_type;
        if subject.applied
            || provider.applied
            || subject.application_identity.is_some()
            || provider.application_identity.is_some()
        {
            return Err(APPLIED.to_owned());
        }
        // The anchor is kept free of this typing's source numbering.
        let anchor = |symbol| {
            let selected = RetainedDeclaration::capture(&typed.symbols, symbol)?;
            RetainedDeclaration::from_retained_parts(
                selected.source().clone(),
                selected.span(),
                selected.kind(),
            )
        };
        Ok(Self {
            subject: anchor(subject.symbol).map_err(str::to_owned)?,
            subject_identity: typed.normalized_hermetic_symbol_identity(subject.symbol)?,
            subject_application: None,
            provider: anchor(provider.symbol).map_err(str::to_owned)?,
            provider_identity: typed.normalized_hermetic_symbol_identity(provider.symbol)?,
            provider_application: None,
            target: selection
                .target
                .ok_or("an executed package provider choice has no target key")?,
            composition_mode: selection.composition_mode,
            origin: selection.origin.clone(),
        })
    }

    /// The same selection among the declarations of `program`. Each anchor
    /// rejoins the one declaration at its source that still has the identity
    /// it was captured with, so a declaration that moved or changed is not
    /// found by its name. Overlap with `_previous` is provider planning's to
    /// decide, as it is for the choices of the package itself.
    pub fn rejoin(
        &self,
        program: &mut TypedTrees,
        scope: source::DependencyScope,
        _previous: &[ProviderSelection],
        candidates: &mut symbols::SourceSpanCandidates,
    ) -> Result<ProviderSelection, Diagnostic> {
        if self.subject_application.is_some() || self.provider_application.is_some() {
            return Err(Diagnostic::error(APPLIED));
        }
        let program = &*program;
        let context = ProviderPlanningContext::new(program, scope);
        let mut rejoined = |anchor: &RetainedDeclaration, expected: &str| {
            let symbol = anchor
                .rejoin(
                    &program.symbols,
                    candidates
                        .at_span(&program.symbols, anchor.span())
                        .into_iter()
                        .filter(|symbol| {
                            program.symbols.get(*symbol).kind == anchor.kind()
                                && program
                                    .normalized_hermetic_symbol_identity(*symbol)
                                    .is_ok_and(|actual| actual == expected)
                        }),
                )
                .map_err(Diagnostic::error)?;
            Ok::<_, Diagnostic>(identity(
                context,
                symbol,
                &program.symbols.display_path(symbol, "::"),
            ))
        };
        let subject = rejoined(&self.subject, &self.subject_identity)?;
        let provider = rejoined(&self.provider, &self.provider_identity)?;
        form(
            context,
            subject,
            provider,
            self.origin.clone(),
            Some(self.target),
            self.composition_mode,
        )
    }
}
