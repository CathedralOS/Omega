//! Finalizing const selections, declarations and argument selections.

use crate::constant::carrier;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionRecordError,
};
use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::SymbolKind;

/// Attach the authored const selection to the substituted expression. The
/// value remains fully erased; only declaration custody survives.
pub(crate) fn finalize_const_selections(
    program: &mut SymbolResolvedTrees,
    pending: &[crate::resolution::lowerer::PendingConstSelection],
) -> Result<(), Diagnostic> {
    let const_symbols = program
        .symbols
        .child_handles(program.symbols.root())
        .into_iter()
        .flatten()
        .filter(|symbol| program.symbols.get(*symbol).kind == SymbolKind::Const)
        .collect::<Vec<_>>();

    for selection in pending {
        let Some(symbol) = const_symbols.get(selection.declaration_ordinal).copied() else {
            return Err(Diagnostic::error(
                "failed to retain const declaration selection provenance",
            ));
        };
        carrier::retain_declared_carrier(
            program,
            selection.expression,
            symbol,
            selection.source_span,
        )?;
        let occurrence = program
            .record_resolved_authored_declaration_selection(
                selection.source_span,
                selection.exposure,
                AuthoredDeclarationSelectionKind::StaticPathSegment,
                symbol,
            )
            .map_err(const_selection_record_diagnostic)?;
        program
            .tables
            .bodies
            .expressions
            .attach_authored_selection_occurrences(selection.expression, [occurrence]);
    }

    Ok(())
}

/// Bind retained const declarations to the symbols minted from the parallel
/// pending declaration list. Value substitution is independent of this root:
/// only source identity and visibility survive here.
pub(crate) fn finalize_const_declarations(
    program: &mut SymbolResolvedTrees,
    pending: &[crate::resolution::lowerer::PendingConstDeclaration],
) -> Result<(), Diagnostic> {
    let const_symbols = program
        .symbols
        .child_handles(program.symbols.root())
        .into_iter()
        .flatten()
        .filter(|symbol| program.symbols.get(*symbol).kind == SymbolKind::Const)
        .collect::<Vec<_>>();
    if const_symbols.len() != pending.len()
        || program.roots.const_declarations.len() != pending.len()
    {
        return Err(Diagnostic::error(
            "failed to retain const declaration visibility provenance",
        ));
    }
    // Scoped module constants select their carrier independently of scalar
    // substitution or structural value encoding. The scope head resolves under
    // the declaring source's ordinary name law — a module-local carrier
    // outranks imported and unmoduled candidates, an exact import exposes a
    // foreign-module carrier — and the authored selection is retained before
    // the scope token is erased. A constant's own visibility cannot authorize
    // naming a private carrier in another package, and generic carriers still
    // need their complete normalization owners.
    for (declaration, constant_symbol) in pending.iter().zip(&const_symbols) {
        let module = program.symbols.symbol_module(*constant_symbol);
        if declaration.scope.as_str().is_empty() || !module.is_valid() {
            continue;
        }
        let carrier = program
            .symbols
            .lookup_top_level_by_name_and_kinds_from_source_matching(
                declaration.scope.as_str(),
                &[SymbolKind::Data],
                declaration.scope.source_span(),
                |_| true,
            );
        let carrier = match carrier {
            symbols::SymbolLookup::Unique(carrier) => carrier,
            symbols::SymbolLookup::Ambiguous { first, second } => {
                return Err(Diagnostic::error(format!(
                    "module type-scoped constant selects an ambiguous data carrier between `{}` and `{}`",
                    program.symbols.display_path(first, "::"),
                    program.symbols.display_path(second, "::"),
                ))
                .with_source_span(declaration.scope.source_span()));
            }
            symbols::SymbolLookup::NotFound => {
                return Err(Diagnostic::error(
                    "module type-scoped constants require an exact nongeneric data carrier selected in their declaring source",
                ).with_source_span(declaration.scope.source_span()));
            }
        };
        // The resolved carrier claims the scope name; ineligible candidates do
        // not shadow it away to a fallback. A generic carrier, or a private
        // declaration in another package, keeps the attachment unproved.
        if !program.data_definitions.iter().any(|definition| {
            definition.symbol == carrier
                && definition.type_parameters.is_empty()
                && (definition.is_public
                    || program
                        .symbols
                        .same_symbol_source_package(carrier, *constant_symbol))
        }) {
            return Err(Diagnostic::error(
                "module type-scoped constants require an exact nongeneric data carrier selected in their declaring source",
            ).with_source_span(declaration.scope.source_span()));
        }
        let exposure = if declaration.is_public {
            language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface
        } else {
            language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation
        };
        program
            .record_resolved_authored_declaration_selection(
                declaration.scope.source_span(),
                exposure,
                AuthoredDeclarationSelectionKind::TypeReference,
                carrier,
            )
            .map_err(const_selection_record_diagnostic)?;
    }
    let mut ordinal = 0usize;
    let mut visibility_drifted = false;
    program
        .roots
        .const_declarations
        .for_each_mut(|declaration| {
            declaration.symbol = const_symbols[ordinal];
            visibility_drifted |= declaration.is_public != pending[ordinal].is_public;
            ordinal += 1;
        });
    if visibility_drifted {
        return Err(Diagnostic::error(
            "const declaration visibility drifted before symbol assignment",
        ));
    }
    Ok(())
}

pub(crate) fn const_selection_record_diagnostic(
    error: AuthoredDeclarationSelectionRecordError,
) -> Diagnostic {
    Diagnostic::error(format!(
        "failed to retain const declaration selection: {error:?}"
    ))
}

/// Bind exact selected declaration custody after ordinary symbol allocation.
pub(crate) fn finalize_const_argument_selections(
    program: &mut SymbolResolvedTrees,
    pending: &[crate::resolution::lowerer::PendingConstArgumentSelection],
    slots: &[crate::resolution::lowerer::PendingConstArgumentSlot],
) -> Result<(), Diagnostic> {
    for (selection_ordinal, selection) in pending.iter().enumerate() {
        let origin = &selection.origin;
        let mut declarations = program.const_declarations.iter().filter(|declaration| {
            program.symbols.symbol_source_span(declaration.symbol) == Some(origin.declaration)
        });
        let declaration = declarations.next().ok_or_else(|| {
            Diagnostic::error("normalized constant argument lost its exact selected declaration")
                .with_source_span(origin.reference)
        })?;
        if declarations.next().is_some()
            || declaration.initializer_source_span != origin.initializer
            || declaration.canonical_value_encoding.as_ref()
                != Some(&origin.canonical_value_encoding)
        {
            return Err(Diagnostic::error(
                "normalized constant argument declaration or value custody drifted before resolution",
            ).with_source_span(origin.reference));
        }
        let requires_nominal_slot =
            carrier::has_nominal_carrier(program, &declaration.declared_type)
                || carrier::encoding_has_nominal_carrier(&origin.canonical_value_encoding);
        let mut matched = false;
        for slot in slots
            .iter()
            .filter(|slot| slot.selection == selection_ordinal)
        {
            let parameters =
                carrier::receiving_parameters(program, slot.arguments).ok_or_else(|| {
                    Diagnostic::error("constant lost its actual receiving application")
                        .with_source_span(origin.reference)
                })?;
            let arguments = program.child_type_references(slot.arguments);
            if parameters.len() != arguments.len() {
                return Err(Diagnostic::error(
                    "constant receiving application has mismatched argument arity",
                )
                .with_source_span(origin.reference));
            }
            let argument = arguments.get(slot.ordinal).ok_or_else(|| {
                Diagnostic::error("constant lost its actual receiving argument")
                    .with_source_span(origin.reference)
            })?;
            let parameter = parameters.get(slot.ordinal).ok_or_else(|| {
                Diagnostic::error("constant lost its receiving generic slot")
                    .with_source_span(origin.reference)
            })?;
            let symbol_resolved_trees::data::TypeParameterKind::Const { type_reference } =
                &parameter.kind
            else {
                return Err(
                    Diagnostic::error("constant did not enter a const parameter")
                        .with_source_span(origin.reference),
                );
            };
            if requires_nominal_slot || carrier::has_nominal_carrier(program, type_reference) {
                if !carrier::same_resolved_carrier(
                    program,
                    &declaration.declared_type,
                    type_reference,
                ) {
                    return Err(Diagnostic::error("selected constant nominal carrier differs from its receiving generic parameter").with_source_span(origin.reference));
                }
                let symbol_resolved_trees::types::TypeReference::Named { symbol, name } = argument
                else {
                    return Err(Diagnostic::error(
                        "nominal constant receiving argument lost its canonical value",
                    )
                    .with_source_span(origin.reference));
                };
                if symbol.is_valid()
                    || !language_semantics::const_value::CanonicalConstValue::from_atom(
                        name.as_str(),
                    )
                    .is_some_and(|value| value.encoding == origin.canonical_value_encoding)
                {
                    return Err(Diagnostic::error(
                        "nominal constant receiving argument differs from its selected value",
                    )
                    .with_source_span(origin.reference));
                }
            }
            matched = true;
        }
        if requires_nominal_slot && !matched {
            return Err(Diagnostic::error(
                "nominal constant argument has no retained receiving generic slot",
            )
            .with_source_span(origin.reference));
        }
        let selected = declaration.symbol;
        program
            .record_resolved_authored_declaration_selection(
                origin.reference,
                selection.exposure,
                AuthoredDeclarationSelectionKind::StaticPathSegment,
                selected,
            )
            .map_err(const_selection_record_diagnostic)?;
    }
    Ok(())
}
