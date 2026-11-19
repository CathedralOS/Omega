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

use super::{
    ProviderPlanningContext, ProviderSelection, ProviderSelectionIdentity,
    ProviderSelectionSubject, RetainedOperatorCoordinate, RetainedOperatorFamily,
    RetainedProductApplication, RetainedProviderSelection, RetainedProviderSubject, form, identity,
};
use crate::declaration_selection::ProductOperandRole;
use crate::source_state::TypedTrees;
use diagnostics::Diagnostic;
use source_admission::RetainedDeclaration;
use symbols::SymbolHandle;

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
    RetainedProductApplication::capture(
        typed,
        super::source_application(identity.application),
        text.clone(),
    )
    .map(Some)
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
    selected.application = super::application_id(retained.prepare(
        program,
        role,
        declaration,
        &selected.authored_path,
    )?);
    selected.applied = true;
    selected.application_identity = Some(retained.identity.clone());
    Ok(())
}

/// A dependency may supply defaults for declarations outside this Product's source closure.
/// Resolve each loaded subject and its provider against the receiving declarations.
pub(crate) fn rejoin_provider_defaults(
    typed: &mut TypedTrees,
    retained: &[RetainedProviderSelection],
) -> Result<Vec<ProviderSelection>, Vec<Diagnostic>> {
    let mut candidates = crate::source_symbols::SourceSpanCandidates::default();
    let mut selections = Vec::new();
    for choice in retained {
        let loaded = typed.symbols.source_files().any(|file| {
            file.dependency_scope == source_provenance::DependencyScope::Product
                && typed
                    .symbols
                    .source_map()
                    .is_none_or(|sources| sources.source_is_selected(file.source_id))
                && choice
                    .subject
                    .declarations()
                    .any(|member| file.path == member.source().path)
        });
        if loaded {
            let selection = choice
                .rejoin(
                    typed,
                    source_provenance::DependencyScope::Product,
                    &selections,
                    &mut candidates,
                )
                .map_err(|error| vec![error])?;
            selections.push(selection);
        }
    }
    Ok(selections)
}

impl RetainedProviderSelection {
    /// `selection` as the anchors a consumer can rejoin.
    pub fn capture(typed: &TypedTrees, selection: &ProviderSelection) -> Result<Self, String> {
        let provider = &selection.provider_type;
        // The anchor is kept free of this typing's source numbering.
        let anchor = |symbol| {
            let selected = crate::source_symbols::retain_declaration(&typed.symbols, symbol)?;
            RetainedDeclaration::from_retained_parts(
                selected.source().clone(),
                selected.span(),
                selected.kind(),
            )
        };
        let subject = match &selection.subject {
            ProviderSelectionSubject::BoundaryTrait(identity)
            | ProviderSelectionSubject::BoundaryRequirement(identity) => {
                RetainedProviderSubject::Declaration {
                    declaration: anchor(identity.symbol).map_err(str::to_owned)?,
                    identity: typed.normalized_hermetic_symbol_identity(identity.symbol)?,
                    application: captured(typed, identity)?,
                }
            }
            ProviderSelectionSubject::BoundaryOperatorFamily(family) => {
                let first = family
                    .coordinates()
                    .first()
                    .ok_or("selected operator family is empty")?;
                if !typed.symbols.symbols().nodes().is_valid(first.symbol) {
                    return Err("selected operator family has a stale representative".into());
                }
                let context = ProviderPlanningContext::new(
                    typed,
                    source_provenance::DependencyScope::Product,
                );
                let current = super::operator_family(
                    context,
                    identity(context, first.symbol, &family.authored_path),
                )
                .map_err(|error| error.message)?;
                if &current != family {
                    return Err(
                        "selected operator family differs from its complete source roster".into(),
                    );
                }
                let members = family
                    .coordinates()
                    .iter()
                    .map(|member| {
                        Ok(RetainedOperatorCoordinate {
                            declaration: anchor(member.symbol)?,
                            requirement_identity: member.requirement_identity.clone(),
                            static_parameter_count: member.static_parameter_count,
                        })
                    })
                    .collect::<Result<Vec<_>, &'static str>>()?;
                RetainedProviderSubject::OperatorFamily(RetainedOperatorFamily::new(
                    family.canonical_path.clone(),
                    members,
                )?)
            }
        };
        Ok(Self {
            subject,
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
        scope: source_provenance::DependencyScope,
        _previous: &[ProviderSelection],
        candidates: &mut crate::source_symbols::SourceSpanCandidates,
    ) -> Result<ProviderSelection, Diagnostic> {
        let mut rejoined = |program: &TypedTrees, anchor: &RetainedDeclaration, expected: &str| {
            crate::source_symbols::rejoin_declaration(
                anchor,
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
        let provider = rejoined(program, &self.provider, &self.provider_identity)?;
        let named = |program: &TypedTrees, symbol| {
            identity(
                ProviderPlanningContext::new(program, scope),
                symbol,
                &program.symbols.display_path(symbol, "::"),
            )
        };
        let mut provider_identity = named(program, provider);
        let subject_identity = match &self.subject {
            RetainedProviderSubject::Declaration {
                declaration,
                identity,
                application,
            } => {
                let subject = rejoined(program, declaration, identity)?;
                let mut subject_identity = named(program, subject);
                if let Some(application) = application {
                    applied(
                        program,
                        application,
                        ProductOperandRole::BoundarySubject,
                        subject,
                        &mut subject_identity,
                    )?;
                }
                subject_identity
            }
            RetainedProviderSubject::OperatorFamily(family) => {
                let first = &family.coordinates()[0];
                let matching = candidates
                    .at_span(&program.symbols, first.declaration.span())
                    .into_iter()
                    .filter(|&symbol| {
                        crate::source_state::operator::declaration_by_symbol(program, symbol)
                            .filter(|operator| operator.is_boundary)
                            .is_some_and(|operator| {
                                let coordinate = program
                                    .boundary_operator_requirement_callable_identity(operator);
                                coordinate.is_valid()
                                    && program.callable_identity_text(coordinate)
                                        == first.requirement_identity
                            })
                    });
                let symbol = crate::source_symbols::rejoin_declaration(
                    &first.declaration,
                    &program.symbols,
                    matching,
                )
                .map_err(Diagnostic::error)?;
                named(program, symbol)
            }
        };
        if let Some(application) = &self.provider_application {
            applied(
                program,
                application,
                ProductOperandRole::Provider,
                provider,
                &mut provider_identity,
            )?;
        }
        let selection = form(
            ProviderPlanningContext::new(program, scope),
            subject_identity,
            provider_identity,
            self.origin.clone(),
            Some(self.target),
            self.composition_mode,
        )?;
        match (&self.subject, &selection.subject) {
            (
                RetainedProviderSubject::OperatorFamily(retained),
                ProviderSelectionSubject::BoundaryOperatorFamily(current),
            ) => {
                if retained.canonical_path != current.canonical_path
                    || retained.coordinates().len() != current.coordinates().len()
                {
                    return Err(Diagnostic::error(
                        "retained provider family changed its complete source roster",
                    ));
                }
                for (expected, actual) in retained.coordinates().iter().zip(current.coordinates()) {
                    crate::source_symbols::rejoin_declaration(
                        &expected.declaration,
                        &program.symbols,
                        [actual.symbol],
                    )
                    .map_err(Diagnostic::error)?;
                    if expected.requirement_identity != actual.requirement_identity
                        || expected.static_parameter_count != actual.static_parameter_count
                    {
                        return Err(Diagnostic::error(
                            "retained provider family changed an overload or its static arity",
                        ));
                    }
                }
            }
            (
                RetainedProviderSubject::Declaration { .. },
                ProviderSelectionSubject::BoundaryTrait(_)
                | ProviderSelectionSubject::BoundaryRequirement(_),
            ) => {}
            _ => {
                return Err(Diagnostic::error(
                    "retained provider choice changed its declaration role",
                ));
            }
        }
        Ok(selection)
    }
}
