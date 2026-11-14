//! A package hands the provider choices its Build made to the packages that
//! depend on it. A choice leaves the typing it was admitted in as source
//! anchors: each declaration as the source that declares it, beside the
//! identity it had there. A consumer types that source itself, so its symbols
//! and source numbers differ; it finds the same declarations again from the
//! anchors and admits the pair again among its own.
//!
//! A slot or provider applied to static arguments, `Reading<17>` or
//! `FirstReading<[Holder<u64>; 2], 17>`, leaves as its closed application as
//! well: the argument tuple as a graph whose names are anchored at their
//! declarations, the sources those anchors are in, and the application's
//! canonical identity. The consumer forms the application again from the
//! graph among its own declarations and must arrive at the same identity.
//! provider_selection.md, "Declaration and choice": "Plans and checked
//! adapters rejoin exact slot, realizing machine, provider value, schema, and
//! requirement owners. Leaf names, strings, normalized signatures, ordinals,
//! declaration order, and compact fingerprints cannot select overloads."

use super::provider_declarations::{
    ProviderPlanningContext, ProviderSelection, ProviderSelectionIdentity,
    ProviderSelectionSubject, RetainedProductApplication, RetainedProviderSelection, form,
    identity,
};
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{ProductDeclarationOperand, ProductOperandRole};
use std::sync::Arc;
use symbols::{RetainedDeclaration, SymbolHandle};
use typed_trees::TypedTrees;

/// The closed application `identity` carries, as a consumer can form it
/// again; `None` for a declaration that applies nothing.
fn captured(
    typed: &TypedTrees,
    identity: &ProviderSelectionIdentity,
) -> Result<Option<RetainedProductApplication>, String> {
    let Some(text) = &identity.application_identity else {
        return if identity.applied {
            Err("an applied provider choice has no application identity".to_owned())
        } else {
            Ok(None)
        };
    };
    if !typed
        .product_operand_applications
        .is_valid(identity.application)
    {
        return Err("an applied provider choice lost its prepared application".to_owned());
    }
    let application = typed.product_operand_applications.get(identity.application);
    let owner = typed
        .expression_table
        .static_machine_application(application.supplying_application)
        .lexical_owner;
    Ok(Some(RetainedProductApplication {
        graph: typed_trees::product_operand::capture_closed_operand(
            typed,
            &application.argument,
            owner,
        )?,
        sources: typed
            .symbols
            .source_map()
            .ok_or("an applied provider choice has no source map for its anchors")?,
        identity: text.clone(),
    }))
}

/// Forms a retained application again among `program`'s declarations and
/// returns it with the identity it has here.
///
/// The graph's declaration anchors are positions in the supplier's sources.
/// Each is moved to the same position of the same file as this program
/// numbers it; an anchor in a file this program did not load names nothing
/// here, and the application is refused.
fn applied(
    program: &mut TypedTrees,
    retained: &RetainedProductApplication,
    role: ProductOperandRole,
    declaration: SymbolHandle,
    selected: &mut ProviderSelectionIdentity,
) -> Result<(), Diagnostic> {
    let own = |span: source::SourceSpan| -> Option<source::SourceSpan> {
        let file = retained.sources.get(span.source_id)?;
        let here = program
            .symbols
            .source_files()
            .find(|own| own.path == file.path)?;
        Some(source::SourceSpan::new(here.source_id, span.span))
    };
    let mut graph = retained.graph.clone();
    let mut foreign = false;
    let mut anchor = |declaration: &mut Option<source::SourceSpan>| {
        if let Some(span) = *declaration {
            match own(span) {
                Some(here) => *declaration = Some(here),
                None => foreign = true,
            }
        }
    };
    graph
        .names
        .for_each_mut(|_, name| anchor(&mut name.declaration));
    graph
        .operands
        .for_each_mut(|_, operand| anchor(&mut operand.declaration));
    if foreign {
        return Err(Diagnostic::error(format!(
            "retained provider choice `{}` is applied to a declaration in a source this program did not load",
            selected.authored_path
        )));
    }
    let operand = ProductDeclarationOperand {
        path: selected.authored_path.clone(),
        source_span: program
            .symbols
            .symbol_provenance_source_span(declaration)
            .unwrap_or_default(),
        application: Some(Arc::new(graph)),
    };
    let application = typed_trees_to_typed_trees::prepare_retained_product_application(
        program,
        &operand,
        role,
        declaration,
    )
    .map_err(|mut diagnostics| diagnostics.remove(0))?;
    let here = program
        .product_operand_application_identity(application)
        .map_err(Diagnostic::error)?
        .and_then(|identity| program.type_identity_text(identity));
    if here.as_deref() != Some(retained.identity.as_str()) {
        return Err(Diagnostic::error(format!(
            "retained provider choice `{}` forms `{}` here and was chosen as `{}`",
            selected.authored_path,
            here.as_deref().unwrap_or("no application"),
            retained.identity,
        )));
    }
    program.retain_dependency_product_application(application);
    selected.application = application;
    selected.applied = true;
    selected.application_identity = here;
    Ok(())
}

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
            subject_application: captured(typed, subject)?,
            provider: anchor(provider.symbol).map_err(str::to_owned)?,
            provider_identity: typed.normalized_hermetic_symbol_identity(provider.symbol)?,
            provider_application: captured(typed, provider)?,
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
    /// found by its name; an application is formed again from its graph and
    /// must have the identity it was chosen with. Overlap with `_previous`
    /// is provider planning's to decide, as it is for the choices of the
    /// package itself.
    pub fn rejoin(
        &self,
        program: &mut TypedTrees,
        scope: source::DependencyScope,
        _previous: &[ProviderSelection],
        candidates: &mut symbols::SourceSpanCandidates,
    ) -> Result<ProviderSelection, Diagnostic> {
        let mut rejoined = |program: &TypedTrees, anchor: &RetainedDeclaration, expected: &str| {
            anchor
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
                .map_err(Diagnostic::error)
        };
        let subject = rejoined(program, &self.subject, &self.subject_identity)?;
        let provider = rejoined(program, &self.provider, &self.provider_identity)?;
        let named = |program: &TypedTrees, symbol| {
            identity(
                ProviderPlanningContext::new(program, scope),
                symbol,
                &program.symbols.display_path(symbol, "::"),
            )
        };
        let mut subject_identity = named(program, subject);
        let mut provider_identity = named(program, provider);
        if let Some(application) = &self.subject_application {
            applied(
                program,
                application,
                ProductOperandRole::BoundarySubject,
                subject,
                &mut subject_identity,
            )?;
        }
        if let Some(application) = &self.provider_application {
            applied(
                program,
                application,
                ProductOperandRole::Provider,
                provider,
                &mut provider_identity,
            )?;
        }
        form(
            ProviderPlanningContext::new(program, scope),
            subject_identity,
            provider_identity,
            self.origin.clone(),
            Some(self.target),
            self.composition_mode,
        )
    }
}
