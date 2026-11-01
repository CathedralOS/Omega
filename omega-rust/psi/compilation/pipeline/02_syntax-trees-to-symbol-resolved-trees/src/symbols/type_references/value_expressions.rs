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

use crate::symbols::expressions::lexical_value_head;
use crate::symbols::scope::MachineScope;
use symbol_resolved_trees::data::DataMember;
use symbol_resolved_trees::name::DiagnosticName;

/// Constant and field types contain the same value expressions as parameter types.
/// Resolve them after declaration and type identities exist, before typing can
/// copy an unresolved call into a constant position. Data owns its generic and
/// field names but has no machine activation: no state parameters or locals are
/// in scope. Field references remain symbolic, not same-spelled global constants.
pub(in crate::symbols) fn assign_declaration_type_value_expression_symbols(
    program: &mut symbol_resolved_trees::SymbolResolvedTrees,
    symbols: &SymbolTable,
) {
    let attached_machines = crate::symbols::scope::attached_machines(program);
    let declarations = &mut program.tables.declarations;
    // Declaration-owned static applications have no statement whose operand
    // walk could resolve their recipes. Give them the same exact header scope
    // as other type-bound expressions, without a runtime subject or activation.
    // Otherwise an unresolved const name can look closed to later tuple work.
    let applications = program
        .roots
        .data_definitions
        .iter()
        .filter_map(|declaration| {
            declaration.cleanup_selection.as_ref().map(|selection| {
                (
                    declaration.symbol,
                    declaration.type_parameters,
                    selection.clone(),
                )
            })
        })
        .chain(
            program
                .roots
                .domain_definitions
                .iter()
                .filter_map(|declaration| {
                    declaration.cleanup_selection.as_ref().map(|selection| {
                        (
                            declaration.symbol,
                            declaration.type_parameters,
                            selection.clone(),
                        )
                    })
                }),
        )
        .collect::<Vec<_>>();
    for (owner, parameters, selection) in applications {
        let scope = MachineScope {
            symbol: owner,
            lexical_requirements: crate::symbols::scope::LexicalRequirements::none(),
            attached_machines: &attached_machines,
            resolution_indexes: &program.resolution_indexes,
            type_parameters: declarations.data_type_parameters.span_or_empty(parameters),
            attached_data_symbol: SymbolHandle::invalid(),
            inherited_data_members: None,
            owned_data: &[],
            prior_statements: &[],
            data_definitions: &program.roots.data_definitions,
            type_aliases: &program.roots.type_aliases,
            conformances: &program.roots.conformances,
            operand_owners: &program.tables.types.operand_lifetime_owners,
            data_members: &declarations.data_members,
            data_type_parameters: &declarations.data_type_parameters,
            data_payload_fields: &declarations.data_payload_fields,
            type_constraints: &program.tables.types.constraints,
        };
        crate::symbols::expressions::assign_static_arguments_type_symbols(
            symbols,
            &scope,
            &[],
            SymbolHandle::invalid(),
            &mut program.tables.bodies.expressions,
            &mut declarations.child_type_references,
            std::slice::from_ref(&selection),
        );
    }
    // A conformance's trait arguments are source-owned types too. In
    // particular, a computed alias extent must bind this declaration's Const
    // before a default member is copied into a separate invocation telescope.
    for conformance in program.roots.conformances.iter() {
        let scope = MachineScope {
            symbol: conformance.symbol,
            lexical_requirements: crate::symbols::scope::LexicalRequirements::none(),
            attached_machines: &attached_machines,
            resolution_indexes: &program.resolution_indexes,
            type_parameters: declarations
                .data_type_parameters
                .span_or_empty(conformance.type_parameters),
            attached_data_symbol: SymbolHandle::invalid(),
            inherited_data_members: None,
            owned_data: &[],
            prior_statements: &[],
            data_definitions: &program.roots.data_definitions,
            type_aliases: &program.roots.type_aliases,
            conformances: &program.roots.conformances,
            operand_owners: &program.tables.types.operand_lifetime_owners,
            data_members: &declarations.data_members,
            data_type_parameters: &declarations.data_type_parameters,
            data_payload_fields: &declarations.data_payload_fields,
            type_constraints: &program.tables.types.constraints,
        };
        for argument in conformance.arguments.handles() {
            let mut reference = declarations.child_type_references.get(argument).clone();
            assign_type_value_expression_symbols(
                symbols,
                &scope,
                &[],
                SymbolHandle::invalid(),
                &mut program.tables.bodies.expressions,
                &mut declarations.child_type_references,
                &mut reference,
            );
            *declarations.child_type_references.get_mut(argument) = reference;
        }
    }
    // Constants own type-bound expressions too, including unused declarations.
    // Resolve under their exact source/module owner before typed endpoint
    // evaluation; no machine parameter or data field is in this scope.
    program
        .roots
        .const_declarations
        .for_each_mut(|declaration| {
            let scope = MachineScope {
                symbol: declaration.symbol,
                lexical_requirements: crate::symbols::scope::LexicalRequirements::none(),
                attached_machines: &attached_machines,
                resolution_indexes: &program.resolution_indexes,
                type_parameters: &[],
                attached_data_symbol: SymbolHandle::invalid(),
                inherited_data_members: None,
                owned_data: &[],
                prior_statements: &[],
                data_definitions: &program.roots.data_definitions,
                type_aliases: &program.roots.type_aliases,
                conformances: &program.roots.conformances,
                operand_owners: &program.tables.types.operand_lifetime_owners,
                data_members: &declarations.data_members,
                data_type_parameters: &declarations.data_type_parameters,
                data_payload_fields: &declarations.data_payload_fields,
                type_constraints: &program.tables.types.constraints,
            };
            assign_type_value_expression_symbols(
                symbols,
                &scope,
                &[],
                SymbolHandle::invalid(),
                &mut program.tables.bodies.expressions,
                &mut declarations.child_type_references,
                &mut declaration.declared_type,
            );
        });
    // Headers remain readable while roots are resolved. Only the expansion
    // changes here; publish its updates after the immutable roster traversal.
    let mut alias_expansions = Vec::new();
    for alias in program.roots.type_aliases.iter() {
        let parameters = declarations
            .data_type_parameters
            .span_or_empty(alias.type_parameters)
            .to_vec();
        let scope = MachineScope {
            symbol: alias.symbol,
            lexical_requirements: crate::symbols::scope::LexicalRequirements::none(),
            attached_machines: &attached_machines,
            resolution_indexes: &program.resolution_indexes,
            type_parameters: &parameters,
            attached_data_symbol: SymbolHandle::invalid(),
            inherited_data_members: None,
            owned_data: &[],
            prior_statements: &[],
            data_definitions: &program.roots.data_definitions,
            type_aliases: &program.roots.type_aliases,
            conformances: &program.roots.conformances,
            operand_owners: &program.tables.types.operand_lifetime_owners,
            data_members: &declarations.data_members,
            data_type_parameters: &declarations.data_type_parameters,
            data_payload_fields: &declarations.data_payload_fields,
            type_constraints: &program.tables.types.constraints,
        };
        let mut expansion = alias.expansion.clone();
        assign_type_value_expression_symbols(
            symbols,
            &scope,
            &[],
            SymbolHandle::invalid(),
            &mut program.tables.bodies.expressions,
            &mut declarations.child_type_references,
            &mut expansion,
        );
        alias_expansions.push(expansion);
    }
    let mut alias_expansions = alias_expansions.into_iter();
    program.roots.type_aliases.for_each_mut(|alias| {
        alias.expansion = alias_expansions.next().expect("unchanged alias roster");
    });
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
            attached_data_symbol: definition.symbol,
            inherited_data_members: Some(members),
            owned_data: &[],
            prior_statements: &[],
            data_definitions: &program.roots.data_definitions,
            type_aliases: &program.roots.type_aliases,
            conformances: &program.roots.conformances,
            operand_owners: &program.tables.types.operand_lifetime_owners,
            data_members: &declarations.data_members,
            data_type_parameters: &declarations.data_type_parameters,
            data_payload_fields: &declarations.data_payload_fields,
            type_constraints: &program.tables.types.constraints,
        };
        for (member_offset, member) in members.iter().enumerate() {
            let (fields, payload) = match member {
                DataMember::Field(field) => {
                    scope.symbol = definition.symbol;
                    (std::slice::from_ref(field), None)
                }
                DataMember::Variant(variant) => {
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
                    payload_updates.push((payload.handle_at(field_offset as u32), reference));
                } else {
                    field_updates.push((
                        definition.members.handle_at(member_offset as u32),
                        reference,
                    ));
                }
            }
        }
    }
    for (handle, reference) in field_updates {
        if let DataMember::Field(field) = declarations.data_members.get_mut(handle) {
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
    assign_type_value_expressions_with_declarations(
        symbols,
        machine,
        parameters,
        state_symbol,
        expressions,
        child_types,
        type_reference,
        &[],
    );
}

#[allow(clippy::too_many_arguments)]
fn assign_type_value_expressions_with_declarations(
    symbols: &SymbolTable,
    machine: &MachineScope<'_>,
    parameters: &[StateParameter],
    state_symbol: SymbolHandle,
    expressions: &mut ExpressionTable,
    child_types: &mut Arena<TypeReference>,
    type_reference: &mut TypeReference,
    declaration_references: &[source::SourceSpan],
) {
    let owner = type_reference.supplied_operand_owner();
    let supplied_scope = owner
        .is_valid()
        .then(|| machine.supplied_scope(owner))
        .flatten();
    // Lost/omitted custody supplies no receiving-name fallback. The ordinary
    // owner/header checks retain their refusal; do not bind this subtree in a
    // same-spelled caller telescope in the meantime.
    if owner.is_valid() && supplied_scope.is_none() {
        return;
    }
    let (machine, parameters, state_symbol) = supplied_scope
        .as_ref()
        .map_or((machine, parameters, state_symbol), |scope| {
            (scope, &[][..], SymbolHandle::invalid())
        });
    if supplied_scope.is_some() {
        // Types and value leaves share their exact supplying telescope.
        // Definition-owned alias references retain their independent lookup.
        super::assignment::assign_type_reference_with_definitions(
            symbols,
            child_types,
            machine.type_constraints,
            machine.type_parameters,
            type_reference,
            declaration_references,
        );
    }
    let mut child = |handle: Handle<TypeReference>| {
        let mut reference = child_types.get(handle).clone();
        assign_type_value_expressions_with_declarations(
            symbols,
            machine,
            parameters,
            state_symbol,
            expressions,
            child_types,
            &mut reference,
            declaration_references,
        );
        *child_types.get_mut(handle) = reference;
    };
    match type_reference {
        TypeReference::Reference(reference) => child(reference.referee),
        TypeReference::FixedArray(array) => {
            child(array.element_type);
            if let FixedArrayLength::Expression(value) = array.length {
                child(value);
            }
            if let FixedArrayLength::ConstParameter { symbol, name } = &mut array.length {
                if let Some(selected) = resolve_index_subject(symbols, machine, parameters, name) {
                    *symbol = selected;
                }
            }
        }
        TypeReference::Slice(slice) => child(slice.element_type),
        TypeReference::Generic(generic) => {
            for argument in generic.arguments.handles() {
                child(argument);
            }
            // Value-index operands inhabit the surrounding value scope. Their
            // exact declaration survives typing; a spelling is not a static
            // value and cannot become an application identity by itself.
            let telescope = machine
                .data_definitions
                .iter()
                .find(|data| data.symbol == generic.base_symbol)
                .map(|data| {
                    machine
                        .data_type_parameters
                        .span_or_empty(data.type_parameters)
                })
                .or_else(|| {
                    if symbols.get(generic.base_symbol).kind != SymbolKind::TypeAlias {
                        return None;
                    }
                    let mut aliases = machine
                        .type_aliases
                        .iter()
                        .filter(|alias| alias.symbol == generic.base_symbol);
                    let alias = aliases.next()?;
                    if aliases.next().is_some() {
                        return None;
                    }
                    Some(
                        machine
                            .data_type_parameters
                            .span_or_empty(alias.type_parameters),
                    )
                });
            if let Some(telescope) = telescope {
                for (parameter, argument) in telescope.iter().zip(generic.arguments.handles()) {
                    if !matches!(
                        parameter.kind,
                        symbol_resolved_trees::data::TypeParameterKind::Value { .. }
                            | symbol_resolved_trees::data::TypeParameterKind::Const { .. }
                    ) {
                        continue;
                    }
                    if let TypeReference::Named { symbol, name, .. } = child_types.get_mut(argument)
                    {
                        if let Some(selected) =
                            resolve_index_subject(symbols, machine, parameters, name)
                        {
                            *symbol = selected;
                        }
                    }
                }
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
                            crate::symbols::expressions::assign_expression_with_declaration_references(
                                symbols,
                                machine,
                                parameters,
                                state_symbol,
                                expressions,
                                child_types,
                                expression,
                                declaration_references,
                            );
                        }
                    }
                    TypeConstraint::Domain(domain) => {
                        for handle in domain.arguments.handles() {
                            let mut reference = child_types.get(handle).clone();
                            assign_type_value_expressions_with_declarations(
                                symbols,
                                machine,
                                parameters,
                                state_symbol,
                                expressions,
                                child_types,
                                &mut reference,
                                declaration_references,
                            );
                            *child_types.get_mut(handle) = reference;
                        }
                    }
                    TypeConstraint::Named(_) | TypeConstraint::ArithmeticDomain(_) => {}
                }
            }
        }
        TypeReference::ConstExpression(expression) => {
            if !expression.supplied_operand_owner.is_valid() && !expression.lexical_owner.is_valid()
            {
                expression.lexical_owner = machine.symbol;
            }
            // Resolve lexical subjects before determining whether an index is
            // closed. A runtime binding cannot become a same-named constant.
            crate::symbols::expressions::assign_expression_with_declaration_references(
                symbols,
                machine,
                parameters,
                state_symbol,
                expressions,
                child_types,
                expression.expression,
                declaration_references,
            );
        }
        TypeReference::AliasUse {
            expansion,
            application,
            definition_references,
            ..
        } => {
            assign_type_value_expressions_with_declarations(
                symbols,
                machine,
                parameters,
                state_symbol,
                expressions,
                child_types,
                application,
                declaration_references,
            );
            let mut references = declaration_references.to_vec();
            references.extend_from_slice(definition_references);
            assign_type_value_expressions_with_declarations(
                symbols,
                machine,
                parameters,
                state_symbol,
                expressions,
                child_types,
                expansion,
                &references,
            );
        }
        TypeReference::Named { .. } | TypeReference::DynamicTrait { .. } | TypeReference::Unit => {}
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

/// Type-bound indices use the declaration's value scope. Active payload
/// binders precede enclosing generic names. A common descriptor synthesized
/// from a value binder keeps that binder identity for application substitution;
/// ordinary common fields still resolve through declaration lookup. Machine
/// parameters and preceding locals retain their normal frontier. Namespace-headed paths
/// keep the declaration identity already selected by ordinary type resolution.
fn resolve_index_subject(
    symbols: &SymbolTable,
    machine: &MachineScope<'_>,
    parameters: &[StateParameter],
    name: &DiagnosticName,
) -> Option<SymbolHandle> {
    // The type occurrence owns one span, so every path segment retains that
    // span rather than manufacturing source offsets across whitespace.
    let members = name
        .as_str()
        .split("::")
        .map(|member| DiagnosticName::from_str(member, name.source_span()))
        .collect::<Vec<_>>();
    let Some((head_name, suffix)) = members.split_first() else {
        return None;
    };
    let (_, declared) = crate::symbols::scoped_paths::resolve_state_scoped_members(
        symbols,
        machine.symbol,
        SymbolHandle::invalid(),
        std::slice::from_ref(head_name),
        None,
    );
    let payload_subject = symbols.get(machine.symbol).kind == SymbolKind::Variant
        && declared.is_valid()
        && symbols.get(declared).kind == SymbolKind::Field
        && symbols.get(declared).parent == machine.symbol;
    let lexical = if payload_subject {
        declared
    } else {
        let frontier =
            lexical_value_head(machine, parameters, head_name.as_str(), !suffix.is_empty());
        if frontier.is_valid() {
            frontier
        } else {
            machine
                .type_parameters
                .iter()
                .find(|parameter| parameter.name.as_str() == head_name.as_str())
                .map(|parameter| parameter.symbol)
                .unwrap_or_else(SymbolHandle::invalid)
        }
    };
    if lexical.is_valid() {
        return Some(
            crate::symbols::scoped_paths::resolve_path_members_from_head(symbols, lexical, suffix)
                .last()
                .copied()
                .unwrap_or_else(SymbolHandle::invalid),
        );
    }
    if suffix.is_empty() {
        return Some(declared);
    }
    if declared.is_valid()
        && !matches!(
            symbols.get(declared).kind,
            SymbolKind::Module
                | SymbolKind::Data
                | SymbolKind::Machine
                | SymbolKind::Trait
                | SymbolKind::BuiltinType
        )
    {
        return Some(
            crate::symbols::scoped_paths::resolve_path_members_from_head(symbols, declared, suffix)
                .last()
                .copied()
                .unwrap_or_else(SymbolHandle::invalid),
        );
    }
    None
}

#[cfg(test)]
mod subject_binding_tests;
