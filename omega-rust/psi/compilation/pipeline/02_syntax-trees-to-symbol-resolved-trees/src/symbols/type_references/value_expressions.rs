//! Type-bound value expressions use their declaration's ordinary value namespace.
//! Type/domain names retain declaration lookup; index expressions must resolve
//! lexical subjects before a later phase can establish that their value is static.

use arena::{Arena, Handle};
use symbol_resolved_trees::{
    expression::ExpressionTable,
    signature::StateParameter,
    types::{FixedArrayLength, TypeConstraint, TypeReference},
};
use symbols::{SymbolHandle, SymbolKind, SymbolTable};

use crate::symbols::{expressions::assign_statement_expression_symbols, scope::MachineScope};

/// A field's type can contain the same value expressions as a parameter's type.
/// Resolve them after declaration and type identities exist, before typing can
/// copy an unresolved call into a constant position. Data owns its generic and
/// field names but has no machine activation: no state parameters or locals are
/// in scope. Field references remain symbolic, not same-spelled global constants.
pub(in crate::symbols) fn assign_data_type_value_expression_symbols(
    program: &mut symbol_resolved_trees::SymbolResolvedTrees,
    symbols: &SymbolTable,
) {
    let attached_machines = crate::symbols::scope::attached_machines(program);
    let declarations = &mut program.tables.declarations;
    let mut field_updates = Vec::new();
    let mut payload_updates = Vec::new();
    for definition in program.roots.data_definitions.iter() {
        let members = declarations.data_members.span_or_empty(definition.members);
        let mut scope = MachineScope {
            symbol: definition.symbol,
            lexical_requirements: crate::symbols::scope::LexicalRequirements::none(),
            attached_machines: &attached_machines,
            resolution_indexes: &program.resolution_indexes,
            type_parameters: declarations
                .data_type_parameters
                .span_or_empty(definition.type_parameters),
            attached_data: Some(&definition.name),
            attached_data_symbol: definition.symbol,
            inherited_data_members: Some(members),
            owned_data: &[],
            prior_statements: &[],
            data_definitions: &program.roots.data_definitions,
            data_members: &declarations.data_members,
            data_payload_fields: &declarations.data_payload_fields,
            type_constraints: &program.tables.types.constraints,
        };
        for (member_offset, member) in members.iter().enumerate() {
            let (fields, payload) = match member {
                symbol_resolved_trees::data::DataMember::Field(field) => {
                    scope.symbol = definition.symbol;
                    (std::slice::from_ref(field), None)
                }
                symbol_resolved_trees::data::DataMember::Variant(variant) => {
                    // Payload subjects shadow common fields and global names.
                    // The symbol hierarchy retains the enclosing data owner.
                    scope.symbol = variant.symbol;
                    (
                        declarations
                            .data_payload_fields
                            .span_or_empty(variant.payload),
                        Some(variant.payload),
                    )
                }
            };
            for (field_offset, field) in fields.iter().enumerate() {
                let mut reference = field.type_reference.clone();
                assign_type_value_expression_symbols(
                    symbols,
                    &scope,
                    &[],
                    SymbolHandle::invalid(),
                    &mut program.tables.bodies.expressions,
                    &mut declarations.child_type_references,
                    &mut reference,
                );
                if let Some(payload) = payload {
                    payload_updates.push((
                        Handle::from_parts(
                            payload.start().arena_index() + field_offset as u32,
                            payload.start().generation(),
                        ),
                        reference,
                    ));
                } else {
                    field_updates.push((
                        Handle::from_parts(
                            definition.members.start().arena_index() + member_offset as u32,
                            definition.members.start().generation(),
                        ),
                        reference,
                    ));
                }
            }
        }
    }
    for (handle, reference) in field_updates {
        if let symbol_resolved_trees::data::DataMember::Field(field) =
            declarations.data_members.get_mut(handle)
        {
            field.type_reference = reference;
        }
    }
    for (handle, reference) in payload_updates {
        declarations
            .data_payload_fields
            .get_mut(handle)
            .type_reference = reference;
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::symbols) fn assign_type_value_expression_symbols(
    symbols: &SymbolTable,
    machine: &MachineScope<'_>,
    parameters: &[StateParameter],
    state_symbol: SymbolHandle,
    expressions: &mut ExpressionTable,
    child_types: &mut Arena<TypeReference>,
    type_reference: &mut TypeReference,
) {
    let mut child = |handle: Handle<TypeReference>| {
        let mut reference = child_types.get(handle).clone();
        assign_type_value_expression_symbols(
            symbols,
            machine,
            parameters,
            state_symbol,
            expressions,
            child_types,
            &mut reference,
        );
        *child_types.get_mut(handle) = reference;
    };
    match type_reference {
        TypeReference::Reference(reference) => child(reference.referee),
        TypeReference::FixedArray(array) => {
            child(array.element_type);
            if let FixedArrayLength::ConstParameter { symbol, name } = &mut array.length {
                // Type syntax retains the complete occurrence span, not
                // individual token spans. Preserve that source authority for
                // each segment rather than inventing offsets across whitespace.
                let members = name
                    .as_str()
                    .split("::")
                    .map(|member| {
                        symbol_resolved_trees::name::DiagnosticName::from_str(
                            member,
                            name.source_span(),
                        )
                    })
                    .collect::<Vec<_>>();
                let Some((head_name, suffix)) = members.split_first() else {
                    return;
                };
                let lexical = crate::symbols::expressions::lexical_value_head(
                    machine,
                    parameters,
                    head_name.as_str(),
                    !suffix.is_empty(),
                );
                let lexical = if lexical.is_valid() {
                    lexical
                } else {
                    machine
                        .type_parameters
                        .iter()
                        .find(|parameter| parameter.name.as_str() == head_name.as_str())
                        .map(|parameter| parameter.symbol)
                        .unwrap_or_else(SymbolHandle::invalid)
                };
                if lexical.is_valid() {
                    *symbol = crate::symbols::scoped_paths::resolve_path_members_from_head(
                        symbols, lexical, suffix,
                    )
                    .last()
                    .copied()
                    .unwrap_or_else(SymbolHandle::invalid);
                } else {
                    let (head, selected) =
                        crate::symbols::scoped_paths::resolve_state_scoped_members(
                            symbols,
                            machine.symbol,
                            SymbolHandle::invalid(),
                            std::slice::from_ref(head_name),
                        );
                    if suffix.is_empty() {
                        *symbol = selected;
                    } else if head.is_valid()
                        && !matches!(
                            symbols.get(head).kind,
                            SymbolKind::Module
                                | SymbolKind::Data
                                | SymbolKind::Machine
                                | SymbolKind::Trait
                                | SymbolKind::BuiltinType
                        )
                    {
                        *symbol = crate::symbols::scoped_paths::resolve_path_members_from_head(
                            symbols, head, suffix,
                        )
                        .last()
                        .copied()
                        .unwrap_or_else(SymbolHandle::invalid);
                    }
                }
            }
        }
        TypeReference::Slice(slice) => child(slice.element_type),
        TypeReference::Generic(generic) => {
            for offset in 0..generic.arguments.count() {
                let start = generic.arguments.start();
                child(Handle::from_parts(
                    start.arena_index() + offset,
                    start.generation(),
                ));
            }
        }
        TypeReference::Constrained(constrained) => {
            child(constrained.base_type);
            for constraint in machine
                .type_constraints
                .span_or_empty(constrained.constraints)
            {
                match constraint {
                    TypeConstraint::Range {
                        minimum, maximum, ..
                    } => {
                        for expression in [*minimum, *maximum] {
                            assign_statement_expression_symbols(
                                symbols,
                                machine,
                                parameters,
                                state_symbol,
                                expressions,
                                child_types,
                                expression,
                            );
                        }
                    }
                    TypeConstraint::Domain(domain) => {
                        for offset in 0..domain.arguments.count() {
                            let start = domain.arguments.start();
                            let handle = Handle::from_parts(
                                start.arena_index() + offset,
                                start.generation(),
                            );
                            let mut reference = child_types.get(handle).clone();
                            assign_type_value_expression_symbols(
                                symbols,
                                machine,
                                parameters,
                                state_symbol,
                                expressions,
                                child_types,
                                &mut reference,
                            );
                            *child_types.get_mut(handle) = reference;
                        }
                    }
                    TypeConstraint::Named(_) | TypeConstraint::ArithmeticDomain(_) => {}
                }
            }
        }
        TypeReference::ConstExpression(expression) => {
            // Resolve lexical subjects before determining whether an index is
            // closed. A runtime binding cannot become a same-named constant.
            assign_statement_expression_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expressions,
                child_types,
                *expression,
            );
        }
        TypeReference::Named { .. } | TypeReference::DynamicTrait { .. } | TypeReference::Unit => {}
    }
}

/// The handles a `HandleSpan<TypeReference>` covers in a child arena — the
/// manual `from_parts` walk reassign sites repeat.
pub(in crate::symbols) fn each_type_reference_handle(
    span: arena::HandleSpan<TypeReference>,
    mut visit: impl FnMut(Handle<TypeReference>),
) {
    for offset in 0..span.count() {
        let start = span.start();
        visit(Handle::from_parts(
            start.arena_index() + offset,
            start.generation(),
        ));
    }
}

/// Re-assign symbols and value expressions on a cloned child type reference,
/// then write it back — the tail every Cast/argument reassign site repeats.
pub(in crate::symbols) fn reassign_type_reference_symbols_and_values(
    symbols: &SymbolTable,
    machine: &MachineScope<'_>,
    parameters: &[StateParameter],
    state_symbol: SymbolHandle,
    expressions: &mut ExpressionTable,
    child_types: &mut Arena<TypeReference>,
    handle: Handle<TypeReference>,
) {
    let mut type_reference = child_types.get(handle).clone();
    crate::symbols::type_references::assign_type_reference_symbol_with_locals(
        symbols,
        child_types,
        machine.type_parameters,
        &mut type_reference,
    );
    assign_type_value_expression_symbols(
        symbols,
        machine,
        parameters,
        state_symbol,
        expressions,
        child_types,
        &mut type_reference,
    );
    *child_types.get_mut(handle) = type_reference;
}

/// Re-assign symbols on a cloned child type reference, then write it back —
/// the tail every symbol-only reassign site repeats.
pub(in crate::symbols) fn reassign_type_reference_symbols(
    symbols: &SymbolTable,
    child_types: &mut Arena<TypeReference>,
    local_type_parameters: &[symbol_resolved_trees::data::TypeParameter],
    handle: Handle<TypeReference>,
) {
    let mut type_reference = child_types.get(handle).clone();
    crate::symbols::type_references::assign_type_reference_symbol_with_locals(
        symbols,
        child_types,
        local_type_parameters,
        &mut type_reference,
    );
    *child_types.get_mut(handle) = type_reference;
}
