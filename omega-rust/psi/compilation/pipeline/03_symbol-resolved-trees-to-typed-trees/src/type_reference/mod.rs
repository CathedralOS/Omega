//! Typing of written type references.
//!
//! Two entries lower a resolved type reference into the typed type-reference
//! table. `lower_type_reference_into_table` takes a `TypeReference` value and
//! uses the `Lowerer`'s current exposure; declared types and signatures go
//! through it, and `direct` implements it.
//! `lower_type_reference_handle_from_table` takes a handle into the resolved
//! type-reference table and always records a private implementation;
//! statement lowering uses it, and `table` implements it. Both routes lower constraints through `constraints`, record authored
//! declaration selections through `retain_type_reference_selection`, and
//! retain computed result carriers through `computed_results`.
//! `retain_static_path_selection` and `retain_path_selection` record the same
//! selections for path spellings that are not type references.
//!
//! Two children run after the typed trees are complete, from
//! `lowerer::finish` and the seeded continuation: `domain_constraints` binds
//! and expands declared-domain constraints, and `generic_origins` supplies
//! `validate_range_arguments`, which completes the range equality checks that
//! structural type construction deferred for generic applications.
//! `domain_aliases` expands transparent domain aliases for domain
//! constraints, proof facts and domain membership expressions.

pub(crate) mod domain_aliases;
pub(crate) mod domain_constraints;

use crate::lowerer::Lowerer;
use diagnostics::Diagnostic;
use symbol_resolved_trees as resolved;
use typed_trees as typed;

mod arithmetic_results;
mod computed_results;
mod const_arguments;
mod constraints;
mod direct;
mod generic_origins;
mod table;

/// Type copying preserves a supplying declaration independently of the
/// surrounding signature's fresh invocation telescope.
pub(super) fn retain_supplied_operand_owner(
    source_trees: &resolved::SymbolResolvedTrees,
    typed_trees: &mut typed::TypedTrees,
    reference: typed::types::TypeReferenceHandle,
    owner: resolved::types::OperandLifetimeOwnerHandle,
    lexical_owner: symbols::SymbolHandle,
) -> Result<(), Diagnostic> {
    if lexical_owner.is_valid() {
        typed_trees
            .type_reference_table
            .retain_operand_lifetime_owner(reference, lexical_owner)
            .map_err(Diagnostic::error)?;
    }
    if !owner.is_valid() {
        return Ok(());
    }
    let declaration = source_trees
        .tables
        .types
        .operand_lifetime_owners
        .get(owner)
        .declaration;
    if !declaration.is_valid() {
        return Err(Diagnostic::error(
            "supplied operand has no resolved lifetime owner",
        ));
    }
    typed_trees
        .type_reference_table
        .retain_operand_lifetime_owner(reference, declaration)
        .map_err(Diagnostic::error)
}

pub(crate) use arithmetic_results::retain_arithmetic_result_type;
pub(crate) use computed_results::retain_computed_result_types;
pub(crate) use const_arguments::normalize_const_argument_carriers;
pub(crate) use generic_origins::validate_range_arguments;

use direct::lower_type_reference_handle_with_context;
use table::lower_type_reference_handle_from_table_with_context;

pub(crate) fn lower_type_reference_into_table(
    lowerer: &mut Lowerer,
    type_reference: &resolved::types::TypeReference,
) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
    lower_type_reference_into_trees_with_exposure(
        lowerer.source_trees,
        &mut lowerer.typed_trees,
        type_reference,
        lowerer.type_reference_exposure,
    )
}

/// An authored return type is optional in source and mandatory in typed trees:
/// an unwritten one types as the invalid handle rather than as an absent one.
pub(crate) fn lower_return_type(
    lowerer: &mut Lowerer,
    written: &Option<resolved::types::TypeReference>,
) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
    match written {
        Some(type_reference) => lower_type_reference_into_table(lowerer, type_reference),
        None => Ok(typed::types::TypeReferenceHandle::invalid()),
    }
}

pub(super) fn lower_type_reference_into_trees_with_exposure(
    source_trees: &resolved::SymbolResolvedTrees,
    typed_trees: &mut typed::TypedTrees,
    type_reference: &resolved::types::TypeReference,
    exposure: impl Into<
        Option<language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure>,
    >,
) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
    lower_type_reference_handle_with_context(
        source_trees,
        typed_trees,
        type_reference,
        exposure.into(),
    )
}

pub(crate) fn lower_type_reference_handle_from_table(
    lowerer: &mut Lowerer,
    type_reference: resolved::types::TypeReferenceHandle,
) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
    lower_type_reference_handle_from_table_with_context(
        lowerer.source_trees,
        &mut lowerer.typed_trees,
        type_reference,
        Some(language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation),
    )
}

/// Issue selection custody from a real original source type token before
/// Product substitution. This reuses ordinary retention for resolved names;
/// an unresolved Product recipe remains late-bound until immutable checking.
/// Binder/local tokens never acquire the supplying actual's declaration ID.
pub(crate) fn retain_type_reference_recipe_selection(
    source: &resolved::SymbolResolvedTrees,
    typed: &mut typed::TypedTrees,
    name: &resolved::name::DiagnosticName,
    symbol: symbols::SymbolHandle,
    exposure: Option<
        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
    >,
    kind: language_semantics::declaration_selection::AuthoredDeclarationSelectionKind,
) -> Result<
    Option<language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId>,
    Diagnostic,
> {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionCoordinateError as CoordinateError,
        AuthoredDeclarationSelectionLateBinding as Binding,
    };
    let Some(exposure) = exposure else {
        return Ok(None);
    };
    if !name.is_source_backed() {
        return Ok(None);
    }
    if symbol.is_valid()
        && (source.symbols.builtin_type_atom(symbol).is_some()
            || matches!(
                source.symbols.get(symbol).kind,
                symbols::SymbolKind::Parameter
                    | symbols::SymbolKind::TypeParameter
                    | symbols::SymbolKind::MachineParameter
                    | symbols::SymbolKind::ConformanceParameter
                    | symbols::SymbolKind::PropositionParameter
                    | symbols::SymbolKind::PropositionMachineParameter
                    | symbols::SymbolKind::Local
            ))
    {
        return Ok(None);
    }
    if symbol.is_valid() {
        retain_type_reference_selection(source, typed, name, symbol, Some(exposure), kind)?;
    }
    match typed
        .authored_declaration_selections()
        .source_coordinate_occurrence(name.source_span(), exposure, kind)
    {
        Ok(occurrence) => Ok(Some(occurrence)),
        Err(CoordinateError::Missing) if !symbol.is_valid() => typed
            .record_late_bound_authored_declaration_selection(
                name.source_span(),
                exposure,
                kind,
                Binding::CheckedStaticPathSegment,
            )
            .map(Some)
            .map_err(|reason| {
                Diagnostic::error(format!(
                    "failed to retain original static type recipe selection: {reason:?}"
                ))
                .with_source_span(name.source_span())
            }),
        Err(reason) => Err(Diagnostic::error(format!(
            "original static type recipe selection has no unique source custody: {reason}"
        ))
        .with_source_span(name.source_span())),
    }
}

pub(super) fn retain_type_reference_selection(
    source_trees: &resolved::SymbolResolvedTrees,
    typed_trees: &mut typed::TypedTrees,
    name: &resolved::name::DiagnosticName,
    symbol: symbols::SymbolHandle,
    exposure: impl Into<
        Option<language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure>,
    >,
    kind: language_semantics::declaration_selection::AuthoredDeclarationSelectionKind,
) -> Result<(), Diagnostic> {
    let Some(exposure) = exposure.into() else {
        return Ok(());
    };
    if !name.is_source_backed()
        || !symbol.is_valid()
        || resolved::types::PrimitiveType::from_name(name.as_str()).is_some()
    {
        return Ok(());
    }
    if let Some(application) = generic_origins::application(source_trees, None, name, symbol)? {
        lower_type_reference_handle_with_context(
            source_trees,
            typed_trees,
            application,
            Some(exposure),
        )?;
        return Ok(());
    }
    if matches!(
        source_trees.symbols.get(symbol).kind,
        symbols::SymbolKind::Parameter
            | symbols::SymbolKind::TypeParameter
            | symbols::SymbolKind::MachineParameter
            | symbols::SymbolKind::ConformanceParameter
            | symbols::SymbolKind::PropositionParameter
            | symbols::SymbolKind::PropositionMachineParameter
            | symbols::SymbolKind::Local
    ) {
        return Ok(());
    }
    typed_trees
        .record_resolved_authored_declaration_selection_once(
            name.source_span(),
            exposure,
            kind,
            symbol,
        )
        .map(|_| ())
        .map_err(|error| {
            Diagnostic::error(format!(
                "failed to retain authored type-reference selection: {error:?}"
            ))
            .with_source_span(name.source_span())
        })
}

/// The `DynamicTrait` node shared by the direct and table-backed type
/// reference lowerings, once each has lowered its own trait arguments.
pub(super) fn lower_dynamic_trait_type_reference(
    source_trees: &resolved::SymbolResolvedTrees,
    typed_trees: &mut typed::TypedTrees,
    symbol: symbols::SymbolHandle,
    name: &resolved::name::DiagnosticName,
    arguments: arena::HandleSpan<typed::types::TypeReferenceHandle>,
    exposure: Option<
        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
    >,
) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
    retain_type_reference_selection(
        source_trees,
        typed_trees,
        name,
        symbol,
        exposure,
        language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::TypeReference,
    )?;
    Ok(typed_trees
        .type_reference_table
        .insert(typed::types::TypeReferenceNode::DynamicTrait {
            symbol,
            name: crate::lowerer::name::lower_name(name),
            arguments,
        }))
}

/// The `Named` leaf shared by the direct and table-backed type reference
/// lowerings.
pub(super) fn lower_named_type_reference(
    source_trees: &resolved::SymbolResolvedTrees,
    typed_trees: &mut typed::TypedTrees,
    symbol: symbols::SymbolHandle,
    name: &resolved::name::DiagnosticName,
    exposure: Option<
        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
    >,
) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
    retain_type_reference_selection(
        source_trees,
        typed_trees,
        name,
        symbol,
        exposure,
        language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::TypeReference,
    )?;
    let reference =
        typed_trees
            .type_reference_table
            .insert(typed::types::TypeReferenceNode::Named {
                symbol,
                name: crate::lowerer::name::lower_name(name),
            });
    if source_trees.const_declaration(symbol).is_some() {
        typed_trees
            .type_reference_table
            .attach_constant_origins(
                reference,
                [typed::types::TypeConstantOrigin {
                    declaration: symbol,
                    reference: name.source_span(),
                }],
            )
            .map_err(Diagnostic::error)?;
    }
    Ok(reference)
}

pub(crate) fn retain_static_path_selection(
    typed_trees: &mut typed::TypedTrees,
    path: &[resolved::name::DiagnosticName],
    symbol: symbols::SymbolHandle,
    exposure: impl Into<
        Option<language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure>,
    >,
    context: &str,
) -> Result<(), Diagnostic> {
    retain_path_selection(
        typed_trees,
        path,
        symbol,
        exposure,
        language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::StaticPathSegment,
        context,
    )
}

pub(crate) fn retain_path_selection(
    typed_trees: &mut typed::TypedTrees,
    path: &[resolved::name::DiagnosticName],
    symbol: symbols::SymbolHandle,
    exposure: impl Into<
        Option<language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure>,
    >,
    kind: language_semantics::declaration_selection::AuthoredDeclarationSelectionKind,
    context: &str,
) -> Result<(), Diagnostic> {
    retain_path_selection_occurrence(typed_trees, path, symbol, exposure, kind, context).map(|_| ())
}

pub(crate) fn retain_path_selection_occurrence(
    typed_trees: &mut typed::TypedTrees,
    path: &[resolved::name::DiagnosticName],
    symbol: symbols::SymbolHandle,
    exposure: impl Into<
        Option<language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure>,
    >,
    kind: language_semantics::declaration_selection::AuthoredDeclarationSelectionKind,
    context: &str,
) -> Result<
    Option<language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId>,
    Diagnostic,
> {
    let Some(exposure) = exposure.into() else {
        return Ok(None);
    };
    if !symbol.is_valid() {
        return Ok(None);
    }
    let authored = path
        .iter()
        .filter(|member| member.is_source_backed())
        .collect::<Vec<_>>();
    let (Some(first), Some(last)) = (authored.first(), authored.last()) else {
        return Ok(None);
    };
    if first.source_span().source_id != last.source_span().source_id {
        return Err(Diagnostic::error(format!(
            "authored {context} path spans more than one source file"
        ))
        .with_source_span(first.source_span()));
    }
    let source_span = source::SourceSpan::new(
        first.source_span().source_id,
        source::Span::new(first.source_span().span.start, last.source_span().span.end),
    );
    typed_trees
        .record_resolved_authored_declaration_selection_once(source_span, exposure, kind, symbol)
        .map(Some)
        .map_err(|error| {
            Diagnostic::error(format!(
                "failed to retain authored {context} selection: {error:?}"
            ))
            .with_source_span(source_span)
        })
}
