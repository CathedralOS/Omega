use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_core::symbols::{SymbolHandle, SymbolTable};
use omega_symbol_resolved_trees::SymbolResolvedTrees;

use crate::symbols::lookup::top_level_type_symbol;

pub(in crate::symbols) fn assign_type_reference_symbols(
    program: &mut SymbolResolvedTrees,
    symbols: &SymbolTable,
) {
    let data_type_parameters = &program.tables.declarations.data_type_parameters;
    let data_members = &mut program.tables.declarations.data_members;
    let child_type_references = &mut program.tables.declarations.child_type_references;
    program
        .roots
        .data_definitions
        .for_each_mut(|data_definition| {
            let type_parameters =
                data_type_parameters.span_or_empty(data_definition.type_parameters);
            for member in data_members.span_mut_or_empty(data_definition.members) {
                if let omega_symbol_resolved_trees::data::DataMember::Field(field) = member {
                    assign_type_reference_symbol_with_locals(
                        symbols,
                        child_type_references,
                        type_parameters,
                        &mut field.type_reference,
                    );
                }
            }
        });

    program.roots.domain_definitions.for_each_mut(|domain| {
        assign_type_reference_symbol_with_locals(
            symbols,
            child_type_references,
            &[],
            &mut domain.target_type,
        );
    });
}

pub(in crate::symbols) fn assign_type_reference_symbol_with_self_type(
    symbols: &SymbolTable,
    child_type_references: &mut Arena<omega_symbol_resolved_trees::types::TypeReference>,
    self_type_symbol: SymbolHandle,
    type_reference: &mut omega_symbol_resolved_trees::types::TypeReference,
) {
    assign_type_reference_symbol_with_context(
        symbols,
        child_type_references,
        &[],
        self_type_symbol,
        type_reference,
    );
}

pub(in crate::symbols) fn assign_type_reference_symbol_with_locals_and_self_type(
    symbols: &SymbolTable,
    child_type_references: &mut Arena<omega_symbol_resolved_trees::types::TypeReference>,
    local_type_parameters: &[omega_symbol_resolved_trees::data::TypeParameter],
    self_type_symbol: SymbolHandle,
    type_reference: &mut omega_symbol_resolved_trees::types::TypeReference,
) {
    assign_type_reference_symbol_with_context(
        symbols,
        child_type_references,
        local_type_parameters,
        self_type_symbol,
        type_reference,
    );
}

pub(in crate::symbols) fn assign_type_reference_symbol_with_locals(
    symbols: &SymbolTable,
    child_type_references: &mut Arena<omega_symbol_resolved_trees::types::TypeReference>,
    local_type_parameters: &[omega_symbol_resolved_trees::data::TypeParameter],
    type_reference: &mut omega_symbol_resolved_trees::types::TypeReference,
) {
    assign_type_reference_symbol_with_context(
        symbols,
        child_type_references,
        local_type_parameters,
        SymbolHandle::invalid(),
        type_reference,
    );
}

fn assign_type_reference_symbol_with_context(
    symbols: &SymbolTable,
    child_type_references: &mut Arena<omega_symbol_resolved_trees::types::TypeReference>,
    local_type_parameters: &[omega_symbol_resolved_trees::data::TypeParameter],
    self_type_symbol: SymbolHandle,
    type_reference: &mut omega_symbol_resolved_trees::types::TypeReference,
) {
    match type_reference {
        omega_symbol_resolved_trees::types::TypeReference::Reference(reference) => {
            assign_type_reference_handle_symbol_with_context(
                symbols,
                child_type_references,
                local_type_parameters,
                self_type_symbol,
                reference.referee,
            );
        }
        omega_symbol_resolved_trees::types::TypeReference::Constrained(constrained) => {
            assign_type_reference_handle_symbol_with_context(
                symbols,
                child_type_references,
                local_type_parameters,
                self_type_symbol,
                constrained.base_type,
            );
        }
        omega_symbol_resolved_trees::types::TypeReference::FixedArray(fixed_array) => {
            assign_type_reference_handle_symbol_with_context(
                symbols,
                child_type_references,
                local_type_parameters,
                self_type_symbol,
                fixed_array.element_type,
            );
        }
        omega_symbol_resolved_trees::types::TypeReference::Slice(slice) => {
            assign_type_reference_handle_symbol_with_context(
                symbols,
                child_type_references,
                local_type_parameters,
                self_type_symbol,
                slice.element_type,
            );
        }
        omega_symbol_resolved_trees::types::TypeReference::Generic(generic) => {
            generic.base_symbol =
                resolve_type_symbol(symbols, local_type_parameters, &generic.base_name);

            assign_type_reference_argument_symbols(
                symbols,
                child_type_references,
                local_type_parameters,
                self_type_symbol,
                generic.arguments,
            );
        }
        omega_symbol_resolved_trees::types::TypeReference::Named { symbol, name } => {
            *symbol = resolve_type_symbol(symbols, local_type_parameters, name);
        }
        omega_symbol_resolved_trees::types::TypeReference::SelfType { symbol } => {
            *symbol = self_type_symbol;
        }
        omega_symbol_resolved_trees::types::TypeReference::Unit => {}
    }
}

fn assign_type_reference_handle_symbol_with_context(
    symbols: &SymbolTable,
    child_type_references: &mut Arena<omega_symbol_resolved_trees::types::TypeReference>,
    local_type_parameters: &[omega_symbol_resolved_trees::data::TypeParameter],
    self_type_symbol: SymbolHandle,
    handle: Handle<omega_symbol_resolved_trees::types::TypeReference>,
) {
    let mut type_reference = std::mem::take(child_type_references.get_mut(handle));
    assign_type_reference_symbol_with_context(
        symbols,
        child_type_references,
        local_type_parameters,
        self_type_symbol,
        &mut type_reference,
    );
    *child_type_references.get_mut(handle) = type_reference;
}

fn assign_type_reference_argument_symbols(
    symbols: &SymbolTable,
    child_type_references: &mut Arena<omega_symbol_resolved_trees::types::TypeReference>,
    local_type_parameters: &[omega_symbol_resolved_trees::data::TypeParameter],
    self_type_symbol: SymbolHandle,
    arguments: HandleSpan<omega_symbol_resolved_trees::types::TypeReference>,
) {
    let start = arguments.start();
    let generation = start.generation();

    for offset in 0..arguments.count() {
        let handle = Handle::from_parts(
            start
                .arena_index()
                .checked_add(offset)
                .expect("type reference argument handle overflow"),
            generation,
        );
        let mut argument = std::mem::take(child_type_references.get_mut(handle));
        assign_type_reference_symbol_with_context(
            symbols,
            child_type_references,
            local_type_parameters,
            self_type_symbol,
            &mut argument,
        );
        *child_type_references.get_mut(handle) = argument;
    }
}

fn resolve_type_symbol(
    symbols: &SymbolTable,
    local_type_parameters: &[omega_symbol_resolved_trees::data::TypeParameter],
    name: &omega_symbol_resolved_trees::name::DiagnosticName,
) -> SymbolHandle {
    local_type_parameters
        .iter()
        .find(|parameter| parameter.name.as_str() == name.as_str())
        .map(|parameter| parameter.symbol)
        .unwrap_or_else(|| top_level_type_symbol(symbols, name.as_str()))
}
