//! Caller-isolated local and aggregate classification for write-frame
//! inference.
//!
//! These queries decide whether an ordinary value is structurally incapable
//! of carrying caller-visible aliasing. They inspect only checked typed shapes.
//! Frame traversal and complete-or-opaque fallback remain in the parent.

use crate::struct_literals::construction_field_type;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::expression::TableStructLiteral;
use typed_trees::name::Identifier;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Lifetime applications retain borrow-region checking but do not require
/// type substitution to inspect their declared storage fields.
pub(super) fn concrete_nominal_type(
    reference: &TypeReferenceNode,
) -> Option<(SymbolHandle, &Identifier)> {
    match reference {
        TypeReferenceNode::Named { symbol, name } => Some((*symbol, name)),
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            arguments,
            ..
        } if arguments.is_empty() => Some((*base_symbol, base_name)),
        _ => None,
    }
}

/// Storage-origin transport compares concrete field structure, not borrow
/// regions. An elided nominal use and an explicit lifetime-only application
/// have the same fields; actual type arguments and constraints still require
/// their existing exact type identity. Lifetime validation remains separate.
pub(super) fn aggregate_storage_types_match(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    expected: TypeReferenceHandle,
) -> bool {
    if crate::type_references::type_references_match(program, actual, expected) {
        return true;
    }
    let Some((actual, _)) =
        concrete_nominal_type(program.type_reference_table.type_reference(actual))
    else {
        return false;
    };
    let Some((expected, _)) =
        concrete_nominal_type(program.type_reference_table.type_reference(expected))
    else {
        return false;
    };
    actual.is_valid()
        && actual == expected
        && program
            .data_definitions()
            .iter()
            .any(|definition| definition.symbol == actual && definition.type_parameters.is_empty())
}

pub(super) fn struct_literal_field_type(
    program: &TypedTrees,
    literal: &TableStructLiteral,
    field_name: &str,
) -> Option<TypeReferenceHandle> {
    let mut definitions = program
        .data_definitions()
        .iter()
        .filter(|definition| definition.name == literal.type_name);
    let definition = definitions.next()?;
    definitions.next().is_none().then_some(())?;
    construction_field_type(
        program,
        definition,
        literal.case_name.as_ref().map(|name| name.as_str()),
        field_name,
    )
}

pub(super) fn struct_literal_matches_expected_type(
    program: &TypedTrees,
    literal: &TableStructLiteral,
    expected_type: TypeReferenceHandle,
) -> bool {
    let Some(expected_type) = crate::places::unwrapped_type_reference(program, expected_type)
    else {
        return false;
    };
    let Some((symbol, name)) =
        concrete_nominal_type(program.type_reference_table.type_reference(expected_type))
    else {
        return false;
    };
    let mut definitions = program
        .data_definitions()
        .iter()
        .filter(|definition| definition.name == literal.type_name);
    let Some(definition) = definitions.next() else {
        return false;
    };
    definitions.next().is_none()
        && definition.type_parameters.is_empty()
        && if symbol.is_valid() {
            definition.symbol == symbol
        } else {
            definition.name == *name
        }
}

pub(super) fn type_is_caller_isolated_local(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> bool {
    type_is_caller_isolated_local_inner(program, handle, &mut Vec::new(), false, &mut Vec::new())
}

/// Erased recursive proof values can have finite constructor terms without a
/// runtime layout. Follow the same storage-shape law as ordinary isolation,
/// but an inline back-edge is not an alias. References still fail closed.
pub(super) fn type_is_caller_isolated_proof_value(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> bool {
    type_is_caller_isolated_local_inner(program, handle, &mut Vec::new(), true, &mut Vec::new())
}

fn type_is_caller_isolated_local_inner(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
    visiting: &mut Vec<SymbolHandle>,
    proof_values: bool,
    isolated_parameters: &mut Vec<SymbolHandle>,
) -> bool {
    if program.primitive_type_reference(handle).is_some() {
        return true;
    }
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Constrained { base_type, .. } => type_is_caller_isolated_local_inner(
            program,
            *base_type,
            visiting,
            proof_values,
            isolated_parameters,
        ),
        TypeReferenceNode::FixedArray { element_type, .. } => type_is_caller_isolated_local_inner(
            program,
            *element_type,
            visiting,
            proof_values,
            isolated_parameters,
        ),
        TypeReferenceNode::Named { symbol, name } => {
            if proof_values && symbol.is_valid() && isolated_parameters.contains(symbol) {
                return true;
            }
            if proof_values
                && matches!(
                    program.symbols.builtin_type_atom(*symbol),
                    Some(symbols::BuiltinTypeAtom::UInt | symbols::BuiltinTypeAtom::Int)
                )
            {
                return true;
            }
            let mut definitions = program.data_definitions().iter().filter(|definition| {
                if symbol.is_valid() {
                    definition.symbol == *symbol
                } else {
                    definition.name == *name
                }
            });
            let Some(definition) = definitions.next() else {
                return false;
            };
            if definitions.next().is_some() {
                return false;
            }
            definition.type_parameters.is_empty()
                && data_definition_is_caller_isolated(
                    program,
                    definition,
                    visiting,
                    proof_values,
                    isolated_parameters,
                )
        }
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } if proof_values => {
            let Some(definition) = program
                .data_definitions()
                .iter()
                .find(|definition| base_symbol.is_valid() && definition.symbol == *base_symbol)
            else {
                return false;
            };
            let parameters = program.data_type_parameters(definition);
            let arguments = program
                .type_reference_table
                .type_reference_handles(*arguments);
            if parameters.len() != arguments.len()
                || parameters.iter().any(|parameter| {
                    !parameter.symbol.is_valid()
                        || program.symbols.get(parameter.symbol).kind != symbols::SymbolKind::TypeParameter
                        || program.symbols.get(parameter.symbol).parent != definition.symbol
                        || !matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
                })
                // Check actual arguments before consulting the nominal cycle
                // guard: Nest<T> -> Nest<&mut T> introduces authority even
                // though the data-definition symbol is already on the path.
                || !arguments.iter().all(|argument| {
                    type_is_caller_isolated_local_inner(
                        program, *argument, visiting, true, isolated_parameters,
                    )
                })
            {
                return false;
            }
            // Every actual argument has independently proved the same unary
            // property. Substituting these exact binders therefore preserves
            // isolation without allocating a second tree of substituted types.
            let parameter_count = isolated_parameters.len();
            isolated_parameters.extend(parameters.iter().map(|parameter| parameter.symbol));
            let isolated = data_definition_is_caller_isolated(
                program,
                definition,
                visiting,
                true,
                isolated_parameters,
            );
            isolated_parameters.truncate(parameter_count);
            isolated
        }
        TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => false,
    }
}

pub(super) fn struct_literal_type_is_caller_isolated(
    program: &TypedTrees,
    literal: &TableStructLiteral,
) -> bool {
    let mut definitions = program
        .data_definitions()
        .iter()
        .filter(|definition| definition.name == literal.type_name);
    let Some(definition) = definitions.next() else {
        return false;
    };
    let unique_shape = match literal.case_name.as_ref() {
        None => program
            .data_members(definition)
            .iter()
            .all(|member| matches!(member, DataMember::Field(_))),
        Some(case_name) => {
            let mut variants = program
                .data_members(definition)
                .iter()
                .filter_map(|member| match member {
                    DataMember::Variant(variant) if variant.name == *case_name => Some(variant),
                    _ => None,
                });
            variants.next().is_some() && variants.next().is_none()
        }
    };
    definitions.next().is_none()
        && unique_shape
        && data_definition_is_caller_isolated(
            program,
            definition,
            &mut Vec::new(),
            false,
            &mut Vec::new(),
        )
}

pub(super) fn data_definition_has_only_owned_storage(
    program: &TypedTrees,
    definition: &typed_trees::data::DataDefinition,
) -> bool {
    data_definition_is_caller_isolated(program, definition, &mut Vec::new(), false, &mut Vec::new())
}

fn data_definition_is_caller_isolated(
    program: &TypedTrees,
    definition: &typed_trees::data::DataDefinition,
    visiting: &mut Vec<SymbolHandle>,
    proof_values: bool,
    isolated_parameters: &mut Vec<SymbolHandle>,
) -> bool {
    if (!definition.type_parameters.is_empty()
        && (!proof_values
            || program
                .data_type_parameters(definition)
                .iter()
                .any(|parameter| !isolated_parameters.contains(&parameter.symbol))))
        || (proof_values
            && definition.supply_mode != language_semantics::DataSupplyMode::CheckedShape)
    {
        return false;
    }
    if visiting.contains(&definition.symbol) {
        return proof_values;
    }
    visiting.push(definition.symbol);
    let isolated = program
        .data_members(definition)
        .iter()
        .all(|member| match member {
            DataMember::Field(field) => type_is_caller_isolated_local_inner(
                program,
                field.type_reference,
                visiting,
                proof_values,
                isolated_parameters,
            ),
            DataMember::Variant(variant) => {
                program.data_payload_fields(variant).iter().all(|field| {
                    type_is_caller_isolated_local_inner(
                        program,
                        field.type_reference,
                        visiting,
                        proof_values,
                        isolated_parameters,
                    )
                })
            }
        });
    visiting.pop();
    isolated
}
