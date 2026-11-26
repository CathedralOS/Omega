//! A package saves the source, span and identity of each declaration selected by
//! Build. The receiving compiler finds those declarations among its own symbols
//! and checks the selection again.
//!
//! If the selection has static arguments, the package also saves their expressions
//! and where their names were declared. The receiver forms the application again
//! and requires the same identity.

use super::{
    ProviderPlanningContext, ProviderSelection, ProviderSelectionIdentity,
    ProviderSelectionSubject, form, identity,
};
use crate::compile_product::retained_application;
use crate::declaration_selection::ProductOperandRole;
use crate::source_state::TypedTrees;
use diagnostics::Diagnostic;
use retained_product_selections::{
    RetainedOperatorCoordinate, RetainedOperatorFamily, RetainedProductApplication,
    RetainedProviderSelection, RetainedProviderSubject,
};
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
    retained_application::capture(
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
    scope: source_provenance::DependencyScope,
) -> Result<(), Diagnostic> {
    selected.application = super::application_id(retained_application::prepare(
        retained,
        program,
        role,
        declaration,
        &selected.authored_path,
        scope,
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
    rejoin_in_scope(
        typed,
        retained,
        source_provenance::DependencyScope::Product,
        true,
    )
}

/// A dependency may publish defaults for schemas the receiver never loaded.
/// Explicit choices must rejoin their designated declarations.
pub(crate) fn rejoin_in_scope(
    typed: &mut TypedTrees,
    retained: &[RetainedProviderSelection],
    scope: source_provenance::DependencyScope,
    defaults: bool,
) -> Result<Vec<ProviderSelection>, Vec<Diagnostic>> {
    let mut candidates = crate::source_symbols::SourceSpanCandidates::default();
    let mut selections = Vec::new();
    for choice in retained {
        if defaults {
            let sources = typed.symbols.source_map();
            let loaded = sources.as_ref().is_some_and(|sources| {
                choice.subject.declarations().any(|member| {
                    sources.selected_files().any(|file| {
                        file.dependency_scope == scope && file.path == member.source().path
                    })
                })
            });
            if !loaded {
                continue;
            }
        }
        let selection = rejoin_provider_selection(choice, typed, scope, &mut candidates)
            .map_err(|error| vec![error])?;
        selections.push(selection);
    }
    Ok(selections)
}

/// Record the exact source declarations and closed arguments selected by Build.
pub(crate) fn capture_provider_selection(
    typed: &TypedTrees,
    selection: &ProviderSelection,
) -> Result<RetainedProviderSelection, String> {
    let provider = &selection.provider_type;
    // Each compiler run assigns its own source IDs. Keep the declaration in package coordinates.
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
            let context =
                ProviderPlanningContext::new(typed, source_provenance::DependencyScope::Product);
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
    Ok(RetainedProviderSelection {
        subject,
        provider: anchor(provider.symbol).map_err(str::to_owned)?,
        provider_identity: typed.normalized_hermetic_symbol_identity(provider.symbol)?,
        provider_application: captured(typed, provider)?,
        target: selection.target,
        composition_mode: selection.composition_mode,
        origin: selection.origin.clone(),
    })
}

/// Find the selection in the receiving program using its original source and identity.
fn rejoin_provider_selection(
    retained: &RetainedProviderSelection,
    program: &mut TypedTrees,
    scope: source_provenance::DependencyScope,
    candidates: &mut crate::source_symbols::SourceSpanCandidates,
) -> Result<ProviderSelection, Diagnostic> {
    let anchor_in_scope = |program: &TypedTrees, anchor: &RetainedDeclaration| {
        let sources = program.symbols.source_map().ok_or_else(|| {
            Diagnostic::error("provider handoff needs the receiving source inventory")
        })?;
        let file = retained_application::receiving_file(&sources, anchor.source(), scope)
            .ok_or_else(|| Diagnostic::error(format!(
                "provider declaration in `{}` is absent or changed in the receiving {:?} sources",
                anchor.source().path.display(), scope,
            )))?;
        Ok::<_, Diagnostic>(RetainedDeclaration::from_source_occurrence(
            file.clone(),
            anchor.span(),
            anchor.kind(),
        ))
    };
    let mut rejoined = |program: &TypedTrees, anchor: &RetainedDeclaration, expected: &str| {
        crate::source_symbols::rejoin_declaration(
            &anchor_in_scope(program, anchor)?,
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
    let provider = rejoined(program, &retained.provider, &retained.provider_identity)?;
    let named = |program: &TypedTrees, symbol| {
        identity(
            ProviderPlanningContext::new(program, scope),
            symbol,
            &program.symbols.display_path(symbol, "::"),
        )
    };
    let mut provider_identity = named(program, provider);
    let subject_identity = match &retained.subject {
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
                    scope,
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
                            let coordinate =
                                program.boundary_operator_requirement_callable_identity(operator);
                            coordinate.is_valid()
                                && program.callable_identity_text(coordinate)
                                    == first.requirement_identity
                        })
                });
            let symbol = crate::source_symbols::rejoin_declaration(
                &anchor_in_scope(program, &first.declaration)?,
                &program.symbols,
                matching,
            )
            .map_err(Diagnostic::error)?;
            named(program, symbol)
        }
    };
    if let Some(application) = &retained.provider_application {
        applied(
            program,
            application,
            ProductOperandRole::Provider,
            provider,
            &mut provider_identity,
            scope,
        )?;
    }
    let selection = form(
        ProviderPlanningContext::new(program, scope),
        subject_identity,
        provider_identity,
        retained.origin.clone(),
        retained.target,
        retained.composition_mode,
    )?;
    match (&retained.subject, &selection.subject) {
        (
            RetainedProviderSubject::OperatorFamily(family),
            ProviderSelectionSubject::BoundaryOperatorFamily(current),
        ) => {
            if family.canonical_path != current.canonical_path
                || family.coordinates().len() != current.coordinates().len()
            {
                return Err(Diagnostic::error(
                    "retained provider family changed its complete source roster",
                ));
            }
            for (expected, actual) in family.coordinates().iter().zip(current.coordinates()) {
                crate::source_symbols::rejoin_declaration(
                    &anchor_in_scope(program, &expected.declaration)?,
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
