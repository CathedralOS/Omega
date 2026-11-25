/*
    A type name that selects no declaration is an error in the program.

    `wiki/spec/language/modules.md`, "Names and imports", selects every name
    from the declarations visible where it is written. Resolution leaves a
    type name that selects nothing without a symbol, and lowering used to
    refuse `machine select(a: Nope) -> Nope { a }` as unsupported ("spelling
    type without a symbol"), as if the compiler could not judge a program
    that is simply wrong.

    Typing sees the program with every bundled library, so, as for an
    unknown call or value name (`DeferredRequest::UndeclaredCallName`,
    `UndeclaredValueName`), the missing type is recorded on the declaration
    that writes it, and the reader that demands that declaration rejects the
    program at the name. The rows cover the types a declaration writes:
    parameter and result types, local declarations, data fields and case
    payloads, through references, arrays, slices and constraints to the type
    each one holds.

    Only a name that certainly selects nothing is recorded: a written plain
    path (`Nope`, `Outer::Nope`) for which the same scoped lookup that
    resolves every top-level name finds no declaration of any kind. A name
    typing binds later, such as a domain written as a type, a generic
    argument that may be a value, or a package-qualified name a staged Build
    waits for, is left to its own stage.
*/

use crate::source_resolved as resolved;
use crate::source_state::TypedTrees;
use crate::source_state::deferred_refusals::DeferredRequest;
use crate::contracts::invocations::DECLARATION_KINDS;
use resolved::data::DataMember;
use resolved::name::DiagnosticName;
use resolved::statement::Statement;
use resolved::types::TypeReference;
use symbols::{SymbolHandle, SymbolKind};

pub(crate) fn defer_undeclared_type_names_in_cohort(
    source: &resolved::SymbolResolvedTrees,
    program: &mut TypedTrees,
    cohort: &crate::lowerer::TypingCohort<'_>,
) {
    let declarations = &source.tables.declarations;
    // Each row is the symbol the type belongs to and the type it writes.
    let parameters = declarations
        .state_parameters
        .iter_from(cohort.resolved_state_parameters)
        .map(|(_, parameter)| (parameter.symbol, &parameter.type_reference));
    let results = declarations
        .machine_states
        .iter_from(cohort.resolved_machine_states)
        .filter_map(|(_, state)| {
            state
                .return_type
                .as_ref()
                .map(|result| (state.symbol, result))
        });
    let locals = declarations
        .state_statements
        .iter_from(cohort.resolved_state_statements)
        .filter_map(|(_, statement)| match statement {
            Statement::LocalData(local) if !local.type_is_inferred => {
                Some((local.symbol, &local.type_reference))
            }
            _ => None,
        });
    let fields = declarations
        .data_members
        .iter_from(cohort.resolved_data_members)
        .filter_map(|(_, member)| match member {
            DataMember::Field(field) => Some((field.symbol, &field.type_reference)),
            _ => None,
        });
    let payloads = declarations
        .data_payload_fields
        .iter_from(cohort.resolved_data_payload_fields)
        .map(|(_, field)| (field.symbol, &field.type_reference));
    let mut rows = Vec::new();
    for (symbol, reference) in parameters
        .chain(results)
        .chain(locals)
        .chain(fields)
        .chain(payloads)
    {
        let Some(name) = unresolved_name(source, reference) else {
            continue;
        };
        let owner = declaration_of(source, symbol);
        if owner.is_valid() && names_nothing(source, name) {
            rows.push((owner, name));
        }
    }
    for (owner, name) in rows {
        program.record_deferred_refusal_with_detail(
            owner,
            DeferredRequest::UndeclaredTypeName,
            name.source_span(),
            format!("`{}`", name.as_str()),
        );
    }
}

/// The written name of the type `reference` holds, when resolution bound it
/// to nothing. Generic arguments are not entered.
fn unresolved_name<'p>(
    source: &'p resolved::SymbolResolvedTrees,
    reference: &'p TypeReference,
) -> Option<&'p DiagnosticName> {
    match reference {
        TypeReference::Named { symbol, name, .. } => (!symbol.is_valid()).then_some(name),
        TypeReference::Generic(application) => {
            (!application.base_symbol.is_valid()).then_some(&application.base_name)
        }
        TypeReference::Reference(reference) => {
            unresolved_name(source, source.child_type_reference(reference.referee))
        }
        TypeReference::FixedArray(array) => {
            unresolved_name(source, source.child_type_reference(array.element_type))
        }
        TypeReference::Slice(slice) => {
            unresolved_name(source, source.child_type_reference(slice.element_type))
        }
        TypeReference::Constrained(constrained) => {
            unresolved_name(source, source.child_type_reference(constrained.base_type))
        }
        TypeReference::AliasUse { .. }
        | TypeReference::ConstExpression(_)
        | TypeReference::Callable { .. }
        | TypeReference::Unit => None,
    }
}

/// The declaration a reader demands for a parameter, local, state, field or
/// payload field: the nearest enclosing state, machine or data declaration.
fn declaration_of(
    source: &resolved::SymbolResolvedTrees,
    mut symbol: SymbolHandle,
) -> SymbolHandle {
    while symbol.is_valid() && source.symbols.symbols().nodes().is_valid(symbol) {
        let row = source.symbols.get(symbol);
        if matches!(
            row.kind,
            SymbolKind::State | SymbolKind::Machine | SymbolKind::Data
        ) {
            return symbol;
        }
        symbol = row.parent;
    }
    SymbolHandle::invalid()
}

/// Whether a written plain path selects no top-level declaration of any
/// kind where it is written.
fn names_nothing(source: &resolved::SymbolResolvedTrees, name: &DiagnosticName) -> bool {
    let span = name.source_span();
    let spelled = name.as_str();
    let plain_path = !spelled.is_empty()
        && spelled.split("::").all(|segment| {
            !segment.is_empty()
                && segment
                    .chars()
                    .all(|character| character == '_' || character.is_alphanumeric())
        });
    span.span.start != span.span.end
        && plain_path
        && source
            .symbols
            .lookup_top_level_by_name_and_kinds_from_source_matching(
                spelled,
                &DECLARATION_KINDS,
                span,
                |_| true,
            )
            == crate::source_symbols::SymbolLookup::NotFound
}
