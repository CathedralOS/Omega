use psi_arena::{Arena, Handle, HandleSpan};
use psi_symbol_resolved_trees::SymbolResolvedTrees;
use psi_symbols::{SymbolHandle, SymbolKind, SymbolTable};

use crate::symbols::lookup::top_level_type_symbol;

pub(in crate::symbols) fn assign_type_reference_symbols(
    program: &mut SymbolResolvedTrees,
    symbols: &SymbolTable,
) {
    let type_constraints = &program.tables.types.constraints;
    let data_type_parameters = &mut program.tables.declarations.data_type_parameters;
    let data_members = &mut program.tables.declarations.data_members;
    let data_payload_fields = &mut program.tables.declarations.data_payload_fields;
    let child_type_references = &mut program.tables.declarations.child_type_references;
    program
        .roots
        .data_definitions
        .for_each_mut(|data_definition| {
            let type_parameters = data_type_parameters
                .span_or_empty(data_definition.type_parameters)
                .to_vec();
            assign_type_parameter_constraint_symbols(
                symbols,
                child_type_references,
                type_constraints,
                &type_parameters,
                data_type_parameters.span_mut_or_empty(data_definition.type_parameters),
            );
            if let Some(quotient) = &mut data_definition.quotient {
                assign_type_reference_symbol_with_locals_and_constraints(
                    symbols,
                    child_type_references,
                    type_constraints,
                    &type_parameters,
                    &mut quotient.carrier,
                );
                quotient.relation_symbol = symbols
                    .find_descendant_by_path(
                        symbols.root(),
                        quotient.relation.iter().map(|member| member.as_str()),
                    )
                    .unwrap_or_else(SymbolHandle::invalid);
            }
            for member in data_members.span_mut_or_empty(data_definition.members) {
                match member {
                    psi_symbol_resolved_trees::data::DataMember::Field(field) => {
                        assign_type_reference_symbol_with_locals_and_constraints(
                            symbols,
                            child_type_references,
                            type_constraints,
                            &type_parameters,
                            &mut field.type_reference,
                        );
                    }
                    // A payload-bearing variant's fields are stored out of band (the
                    // `data_payload_fields` arena); their type references need symbols
                    // too -- a struct-typed payload `case Wrap(p: Point)` otherwise
                    // fails in the layout builder ("non-primitive type `Point` is
                    // missing a resolved symbol"). Primitive/array payloads resolved
                    // anyway (no named symbol needed); only NAMED payload types broke.
                    psi_symbol_resolved_trees::data::DataMember::Variant(variant) => {
                        for field in data_payload_fields.span_mut_or_empty(variant.payload) {
                            assign_type_reference_symbol_with_locals_and_constraints(
                                symbols,
                                child_type_references,
                                type_constraints,
                                &type_parameters,
                                &mut field.type_reference,
                            );
                        }
                    }
                }
            }
        });

    program.roots.domain_definitions.for_each_mut(|domain| {
        let type_parameters = data_type_parameters
            .span_or_empty(domain.type_parameters)
            .to_vec();
        assign_type_parameter_constraint_symbols(
            symbols,
            child_type_references,
            type_constraints,
            &type_parameters,
            data_type_parameters.span_mut_or_empty(domain.type_parameters),
        );
        assign_type_reference_symbol_with_locals_and_constraints(
            symbols,
            child_type_references,
            type_constraints,
            &type_parameters,
            &mut domain.target_type,
        );
        assign_type_reference_argument_symbols_with_constraints(
            symbols,
            child_type_references,
            type_constraints,
            &type_parameters,
            SymbolHandle::invalid(),
            domain.index_arguments,
        );
    });

    program.roots.conformances.for_each_mut(|conformance| {
        assign_type_reference_argument_symbols_with_constraints(
            symbols,
            child_type_references,
            type_constraints,
            &[],
            SymbolHandle::invalid(),
            conformance.arguments,
        );
    });
}

pub(in crate::symbols) fn assign_type_reference_symbol_with_locals_and_self_type_and_constraints(
    symbols: &SymbolTable,
    child_type_references: &mut Arena<psi_symbol_resolved_trees::types::TypeReference>,
    type_constraints: &Arena<psi_symbol_resolved_trees::types::TypeConstraint>,
    local_type_parameters: &[psi_symbol_resolved_trees::data::TypeParameter],
    self_type_symbol: SymbolHandle,
    type_reference: &mut psi_symbol_resolved_trees::types::TypeReference,
) {
    assign_type_reference_symbol_with_context(
        symbols,
        child_type_references,
        type_constraints,
        local_type_parameters,
        self_type_symbol,
        type_reference,
    );
}

pub(in crate::symbols) fn assign_type_reference_symbol_with_locals_and_constraints(
    symbols: &SymbolTable,
    child_type_references: &mut Arena<psi_symbol_resolved_trees::types::TypeReference>,
    type_constraints: &Arena<psi_symbol_resolved_trees::types::TypeConstraint>,
    local_type_parameters: &[psi_symbol_resolved_trees::data::TypeParameter],
    type_reference: &mut psi_symbol_resolved_trees::types::TypeReference,
) {
    assign_type_reference_symbol_with_context(
        symbols,
        child_type_references,
        type_constraints,
        local_type_parameters,
        SymbolHandle::invalid(),
        type_reference,
    );
}

fn assign_type_reference_symbol_with_context(
    symbols: &SymbolTable,
    child_type_references: &mut Arena<psi_symbol_resolved_trees::types::TypeReference>,
    type_constraints: &Arena<psi_symbol_resolved_trees::types::TypeConstraint>,
    local_type_parameters: &[psi_symbol_resolved_trees::data::TypeParameter],
    self_type_symbol: SymbolHandle,
    type_reference: &mut psi_symbol_resolved_trees::types::TypeReference,
) {
    match type_reference {
        psi_symbol_resolved_trees::types::TypeReference::Reference(reference) => {
            assign_type_reference_handle_symbol_with_context(
                symbols,
                child_type_references,
                type_constraints,
                local_type_parameters,
                self_type_symbol,
                reference.referee,
            );
        }
        psi_symbol_resolved_trees::types::TypeReference::Constrained(constrained) => {
            assign_type_reference_handle_symbol_with_context(
                symbols,
                child_type_references,
                type_constraints,
                local_type_parameters,
                self_type_symbol,
                constrained.base_type,
            );
            for constraint in type_constraints.span_or_empty(constrained.constraints) {
                let psi_symbol_resolved_trees::types::TypeConstraint::Domain(domain) = constraint
                else {
                    continue;
                };
                assign_type_reference_argument_symbols_with_constraints(
                    symbols,
                    child_type_references,
                    type_constraints,
                    local_type_parameters,
                    self_type_symbol,
                    domain.arguments,
                );
            }
        }
        psi_symbol_resolved_trees::types::TypeReference::FixedArray(fixed_array) => {
            assign_type_reference_handle_symbol_with_context(
                symbols,
                child_type_references,
                type_constraints,
                local_type_parameters,
                self_type_symbol,
                fixed_array.element_type,
            );
            assign_fixed_array_length_symbol(
                symbols,
                local_type_parameters,
                &mut fixed_array.length,
            );
        }
        psi_symbol_resolved_trees::types::TypeReference::Slice(slice) => {
            assign_type_reference_handle_symbol_with_context(
                symbols,
                child_type_references,
                type_constraints,
                local_type_parameters,
                self_type_symbol,
                slice.element_type,
            );
        }
        psi_symbol_resolved_trees::types::TypeReference::Generic(generic) => {
            generic.base_symbol =
                resolve_type_symbol(symbols, local_type_parameters, &generic.base_name);

            assign_type_reference_argument_symbols_with_constraints(
                symbols,
                child_type_references,
                type_constraints,
                local_type_parameters,
                self_type_symbol,
                generic.arguments,
            );
        }
        // PDI3 index expressions retain their lexical binder spellings here;
        // typed index normalization resolves them against the enclosing const
        // telescope and records the exact selected operation separately.
        psi_symbol_resolved_trees::types::TypeReference::ConstExpression(_) => {}
        psi_symbol_resolved_trees::types::TypeReference::DynamicTrait {
            symbol,
            name,
            conformance,
            conformance_carrier,
            conformance_name,
        } => {
            *symbol =
                crate::symbols::lookup::top_level_symbol(symbols, SymbolKind::Trait, name.as_str());
            if let (Some(data_name), Some(conformance_name)) =
                (conformance_carrier, conformance_name)
            {
                let carrier = crate::symbols::lookup::top_level_symbol(
                    symbols,
                    SymbolKind::Data,
                    data_name.as_str(),
                );
                let selected = crate::symbols::lookup::child_symbol_by_kinds(
                    symbols,
                    carrier,
                    &[SymbolKind::Conformance],
                    conformance_name.as_str(),
                );
                *conformance = selected.is_valid().then_some(selected);
            }
        }
        psi_symbol_resolved_trees::types::TypeReference::Named { symbol, name } => {
            *symbol = resolve_type_symbol(symbols, local_type_parameters, name);
        }
        psi_symbol_resolved_trees::types::TypeReference::SelfType { symbol } => {
            *symbol = self_type_symbol;
        }
        psi_symbol_resolved_trees::types::TypeReference::Unit => {}
    }
}

fn assign_fixed_array_length_symbol(
    symbols: &SymbolTable,
    local_type_parameters: &[psi_symbol_resolved_trees::data::TypeParameter],
    length: &mut psi_symbol_resolved_trees::types::FixedArrayLength,
) {
    let psi_symbol_resolved_trees::types::FixedArrayLength::ConstParameter { symbol, name } =
        length
    else {
        return;
    };
    *symbol = resolve_type_symbol(symbols, local_type_parameters, name);
}

fn assign_type_parameter_constraint_symbols(
    symbols: &SymbolTable,
    child_type_references: &mut Arena<psi_symbol_resolved_trees::types::TypeReference>,
    type_constraints: &Arena<psi_symbol_resolved_trees::types::TypeConstraint>,
    local_type_parameters: &[psi_symbol_resolved_trees::data::TypeParameter],
    type_parameters: &mut [psi_symbol_resolved_trees::data::TypeParameter],
) {
    for parameter in type_parameters {
        let psi_symbol_resolved_trees::data::TypeParameterKind::Const { type_reference } =
            &mut parameter.kind
        else {
            continue;
        };
        assign_type_reference_symbol_with_locals_and_constraints(
            symbols,
            child_type_references,
            type_constraints,
            local_type_parameters,
            type_reference,
        );
    }
}

fn assign_type_reference_handle_symbol_with_context(
    symbols: &SymbolTable,
    child_type_references: &mut Arena<psi_symbol_resolved_trees::types::TypeReference>,
    type_constraints: &Arena<psi_symbol_resolved_trees::types::TypeConstraint>,
    local_type_parameters: &[psi_symbol_resolved_trees::data::TypeParameter],
    self_type_symbol: SymbolHandle,
    handle: Handle<psi_symbol_resolved_trees::types::TypeReference>,
) {
    let mut type_reference = std::mem::take(child_type_references.get_mut(handle));
    assign_type_reference_symbol_with_context(
        symbols,
        child_type_references,
        type_constraints,
        local_type_parameters,
        self_type_symbol,
        &mut type_reference,
    );
    *child_type_references.get_mut(handle) = type_reference;
}

pub(in crate::symbols) fn assign_type_reference_argument_symbols_with_constraints(
    symbols: &SymbolTable,
    child_type_references: &mut Arena<psi_symbol_resolved_trees::types::TypeReference>,
    type_constraints: &Arena<psi_symbol_resolved_trees::types::TypeConstraint>,
    local_type_parameters: &[psi_symbol_resolved_trees::data::TypeParameter],
    self_type_symbol: SymbolHandle,
    arguments: HandleSpan<psi_symbol_resolved_trees::types::TypeReference>,
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
            type_constraints,
            local_type_parameters,
            self_type_symbol,
            &mut argument,
        );
        *child_type_references.get_mut(handle) = argument;
    }
}

// Cast/zero-value target leaves use a separate semantic-domain field and do
// not own declaration constraint spans. Signature/declaration traversal uses
// the `_and_constraints` variants above so indexed arguments resolve in their
// lexical generic scope.
pub(in crate::symbols) fn assign_type_reference_symbol_with_locals_and_self_type(
    symbols: &SymbolTable,
    child_type_references: &mut Arena<psi_symbol_resolved_trees::types::TypeReference>,
    local_type_parameters: &[psi_symbol_resolved_trees::data::TypeParameter],
    self_type_symbol: SymbolHandle,
    type_reference: &mut psi_symbol_resolved_trees::types::TypeReference,
) {
    let constraints = Arena::new();
    assign_type_reference_symbol_with_locals_and_self_type_and_constraints(
        symbols,
        child_type_references,
        &constraints,
        local_type_parameters,
        self_type_symbol,
        type_reference,
    );
}

fn resolve_type_symbol(
    symbols: &SymbolTable,
    local_type_parameters: &[psi_symbol_resolved_trees::data::TypeParameter],
    name: &psi_symbol_resolved_trees::name::DiagnosticName,
) -> SymbolHandle {
    local_type_parameters
        .iter()
        .find(|parameter| parameter.name.as_str() == name.as_str())
        .map(|parameter| parameter.symbol)
        .unwrap_or_else(|| top_level_type_symbol(symbols, name.as_str()))
}
