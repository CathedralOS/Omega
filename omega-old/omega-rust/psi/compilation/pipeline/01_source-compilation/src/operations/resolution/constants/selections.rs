/* Finalizing const selections, declarations and argument selections. */

use crate::declaration_selection::{
    AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
};
use crate::resolution::constants::carrier;
#[cfg(test)]
use crate::resolution::constants::receiving_applications::FINALIZATION_WORK;
use crate::resolution::constants::receiving_applications::{self, ConstArgumentInputs};
use crate::source_resolved::SymbolResolvedTrees;
use crate::source_resolved::data::TypeParameterKind;
use crate::source_resolved::name::DiagnosticName;
use crate::source_resolved::types::TypeReference;
use crate::source_selections::AuthoredDeclarationSelectionRecordError;
use application_identity::const_value::CanonicalConstValue;
use carrier::retain_declared_carrier;
use carrier::same_resolved_carrier;
use diagnostics::Diagnostic;
use symbols::SymbolKind;

/*
    Attach the authored const selection to the substituted expression. The
    value remains fully erased; only declaration custody survives.
*/
pub(crate) fn finalize_const_selections(
    program: &mut SymbolResolvedTrees,
    pending: &[crate::resolution::lowering::lowerer::PendingConstSelection],
) -> Result<(), Diagnostic> {
    for selection in pending {
        let symbol = selection.declaration;
        retain_declared_carrier(program, selection.expression, symbol, selection.source_span)?;
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

/*
    Finalize const attachment and visibility using the identity its allocation
    already published. Historical symbol-table roots are not this declaration
    roster and must never be zipped back onto it.
*/
pub(crate) fn finalize_const_declarations(
    syntax_trees: &syntax_trees::SyntaxTrees,
    program: &mut SymbolResolvedTrees,
    pending: &[crate::resolution::lowering::lowerer::PendingConstDeclaration],
) -> Result<(), Diagnostic> {
    for declaration in pending {
        if !program
            .roots
            .const_declarations
            .contains(declaration.declaration)
        {
            return Err(Diagnostic::error(
                "pending constant lost its owned declaration",
            ));
        }
        let constant = program
            .roots
            .const_declarations
            .get(declaration.declaration);
        let constant_symbol = constant.symbol;
        let is_public = constant.is_public;
        if !constant_symbol.is_valid()
            || program.symbols.get(constant_symbol).kind != SymbolKind::Const
            || declaration.is_public != is_public
        {
            return Err(Diagnostic::error(
                "const declaration identity or visibility drifted before finalization",
            ));
        }
        let module = program.symbols.symbol_module(constant_symbol);
        if declaration.scope.as_str(syntax_trees).is_empty() || !module.is_valid() {
            continue;
        }
        let carrier = program
            .symbols
            .lookup_top_level_by_name_and_kinds_from_source_matching(
                declaration.scope.as_str(syntax_trees),
                &[SymbolKind::Data],
                declaration.scope.source_span(),
                |_| true,
            );
        let carrier = match carrier {
            crate::source_symbols::SymbolLookup::Unique(carrier) => carrier,
            crate::source_symbols::SymbolLookup::Ambiguous { first, second } => {
                return Err(Diagnostic::error(format!(
                "module type-scoped constant selects an ambiguous data carrier between `{}` and `{}`",
                program.symbols.display_path(first, "::"),
                program.symbols.display_path(second, "::"),
            ))
            .with_source_span(declaration.scope.source_span()));
            }
            crate::source_symbols::SymbolLookup::NotFound => {
                return Err(Diagnostic::error(
                "module type-scoped constants require an exact accessible data carrier selected in their declaring source",
            ).with_source_span(declaration.scope.source_span()));
            }
        };
        if !program.data_definitions.iter().any(|definition| {
            definition.symbol == carrier
                && (definition.is_public
                    || program
                        .symbols
                        .same_symbol_source_package(carrier, constant_symbol))
        }) {
            return Err(Diagnostic::error(
            "module type-scoped constants require an exact accessible data carrier selected in their declaring source",
        ).with_source_span(declaration.scope.source_span()));
        }
        let exposure = if declaration.is_public {
            AuthoredDeclarationSelectionExposure::PublicInterface
        } else {
            AuthoredDeclarationSelectionExposure::PrivateImplementation
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
    Ok(())
}

pub(crate) fn const_selection_record_diagnostic(
    error: AuthoredDeclarationSelectionRecordError,
) -> Diagnostic {
    Diagnostic::error(format!(
        "failed to retain const declaration selection: {error:?}"
    ))
}

/* Bind exact selected declaration custody after ordinary symbol allocation. */
pub(crate) fn finalize_const_argument_selections(
    program: &mut SymbolResolvedTrees,
    pending: &[crate::resolution::lowering::lowerer::PendingConstArgumentSelection],
    slots: &[crate::resolution::lowering::lowerer::PendingConstArgumentSlot],
) -> Result<(), Diagnostic> {
    let inputs = ConstArgumentInputs::prepare(program, pending, slots)?;
    let mut selected_arguments = Vec::new();
    for (selection_ordinal, selection) in pending.iter().enumerate() {
        let origin = &selection.origin;
        #[cfg(test)]
        FINALIZATION_WORK.with(|work| {
            let (preparation, declarations, slots) = work.get();
            work.set((preparation, declarations + 1, slots));
        });
        let declaration = inputs.declarations[selection_ordinal].ok_or_else(|| {
            Diagnostic::error("normalized constant argument lost its exact selected declaration")
                .with_source_span(origin.reference)
        })?;
        let declaration = program.const_declarations[declaration].clone();
        if inputs.duplicate_declarations[selection_ordinal]
            || declaration.initializer_source_span != origin.initializer
            || declaration.evaluated_value.as_ref() != Some(&origin.evaluated_value)
        {
            return Err(Diagnostic::error(
            "normalized constant argument declaration or value custody drifted before resolution",
        ).with_source_span(origin.reference));
        }
        let requires_nominal_slot = selection.is_argument_value
            && (carrier::has_nominal_carrier(program, &declaration.declared_type)
                || origin
                    .evaluated_value
                    .encoding()
                    .is_some_and(carrier::encoding_has_nominal_carrier));
        let structured_argument = selection.is_argument_value
            && origin.evaluated_value.encoding().is_some_and(|encoding| {
                use application_identity::const_value::{
                    CanonicalConstValue, DecodedCanonicalConstValue,
                };
                matches!(
                    CanonicalConstValue::new("", encoding, "").decode_encoding(),
                    Some(
                        DecodedCanonicalConstValue::Array { .. }
                            | DecodedCanonicalConstValue::Record { .. }
                            | DecodedCanonicalConstValue::Variant { .. }
                    )
                )
            });
        let mut matched = false;
        for slot in &inputs.slots[inputs.slot_runs[selection_ordinal].clone()] {
            #[cfg(test)]
            FINALIZATION_WORK.with(|work| {
                let (preparation, declarations, slots) = work.get();
                work.set((preparation, declarations, slots + 1));
            });
            let parameters = receiving_applications::receiving_parameters(program, slot)
                .map_err(|diagnostic| diagnostic.with_source_span(origin.reference))?;
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
            let TypeParameterKind::Const { type_reference } = &parameter.kind else {
                return Err(
                    Diagnostic::error("constant did not enter a const parameter")
                        .with_source_span(origin.reference),
                );
            };
            if selection.is_argument_value
                && (structured_argument
                    || requires_nominal_slot
                    || carrier::has_nominal_carrier(program, type_reference))
            {
                if (requires_nominal_slot || carrier::has_nominal_carrier(program, type_reference))
                    && !same_resolved_carrier(program, &declaration.declared_type, type_reference)
                {
                    return Err(Diagnostic::error("selected constant nominal carrier differs from its receiving generic parameter").with_source_span(origin.reference));
                }
                let TypeReference::Named { symbol, name, .. } = argument else {
                    return Err(Diagnostic::error(
                        "nominal constant receiving argument lost its canonical value",
                    )
                    .with_source_span(origin.reference));
                };
                if (symbol.is_valid() && *symbol != declaration.symbol)
                    || !CanonicalConstValue::from_atom(name.as_str()).is_some_and(|value| {
                        Some(value.encoding.as_str()) == origin.evaluated_value.encoding()
                    })
                {
                    return Err(Diagnostic::error(
                        "nominal constant receiving argument differs from its selected value",
                    )
                    .with_source_span(origin.reference));
                }
                selected_arguments.push((
                    slot.arguments,
                    slot.ordinal,
                    declaration.symbol,
                    DiagnosticName::from_str(
                        program.symbols.name(declaration.symbol),
                        origin.reference,
                    ),
                ));
            }
            matched = true;
        }
        if requires_nominal_slot && !matched {
            return Err(Diagnostic::error(
                "nominal constant argument has no retained receiving generic slot",
            )
            .with_source_span(origin.reference));
        }
        if let Some(exposure) = selection.exposure {
            program
                .record_resolved_authored_declaration_selection(
                    origin.reference,
                    exposure,
                    AuthoredDeclarationSelectionKind::StaticPathSegment,
                    declaration.symbol,
                )
                .map_err(const_selection_record_diagnostic)?;
        }
    }
    for (arguments, ordinal, selected, selected_name) in selected_arguments {
        let argument = &mut program
            .tables
            .declarations
            .child_type_references
            .span_mut_or_empty(arguments)[ordinal];
        if let TypeReference::Named { symbol, name, .. } = argument {
            *symbol = selected;
            *name = selected_name;
        }
    }
    Ok(())
}
