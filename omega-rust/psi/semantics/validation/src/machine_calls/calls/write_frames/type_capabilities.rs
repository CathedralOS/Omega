//! Type capability queries used by caller-visible write-frame inference.
//!
//! These queries classify constrained references and whether a parameter can
//! carry a caller-visible write. They do not traverse expressions, resolve
//! calls, or summarize frames.

#[cfg(test)]
mod tests;

use super::isolation::type_is_caller_isolated_local_in;
use super::type_instantiation::{
    TypeBindings, push_generic_application_bindings, substituted_head,
};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::signature::StateParameter;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(super) fn type_reference_is_reference(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { .. } => true,
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_is_reference(program, *base_type)
        }
        _ => false,
    }
}

/// Parameter roots use the same storage classification as call arguments and
/// local bindings. Their position in a transition cycle adds no authority.
pub(super) fn parameter_may_carry_write(program: &TypedTrees, parameter: &StateParameter) -> bool {
    type_may_carry_write(program, parameter.type_reference)
}

/// A caller-isolated storage shape with no opaque boundary data anywhere in
/// its reachable structure. Isolation fails closed on references, dynamic
/// traits, unbound generic parameters and recursive runtime shapes. Opaque
/// boundary data has no declared members to inspect, so the second walk
/// excludes it rather than trusting an empty field roster. Slice carriers
/// keep their conservative classification below.
fn type_is_reference_free_value(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
    bindings: &[(SymbolHandle, TypeReferenceHandle)],
) -> bool {
    type_is_caller_isolated_local_in(program, handle, bindings)
        && !type_reaches_opaque_data(program, handle, &mut Vec::new(), &mut bindings.to_vec())
}

/// `visiting` holds the data definitions on the current path; a recursive
/// re-entry reports no opaque data of its own because the outer visit is
/// still inspecting that definition's other members. `bindings` substitute a
/// generic application's actuals for its definition's parameters exactly as
/// the isolation walk does, so `Box<u64>` inspects `value: u64` while an
/// unbound parameter stays unknown.
fn type_reaches_opaque_data(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
    visiting: &mut Vec<SymbolHandle>,
    bindings: &mut TypeBindings,
) -> bool {
    let handle = substituted_head(program, handle, bindings);
    if program.primitive_type_reference(handle).is_some() {
        return false;
    }
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Constrained { base_type, .. }
        | TypeReferenceNode::FixedArray {
            element_type: base_type,
            ..
        } => type_reaches_opaque_data(program, *base_type, visiting, bindings),
        TypeReferenceNode::Unit | TypeReferenceNode::ConstExpression(_) => false,
        // Isolation has already rejected these shapes; treat them as opaque.
        TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::DynamicTrait { .. } => true,
        TypeReferenceNode::Named { symbol, name } => {
            // The unbounded integer atoms are values with no members to
            // reach, exactly like the primitive atoms above.
            if matches!(
                program.symbols.builtin_type_atom(*symbol),
                Some(symbols::BuiltinTypeAtom::UInt | symbols::BuiltinTypeAtom::Int)
            ) {
                return false;
            }
            let mut definitions = program.data_definitions().iter().filter(|definition| {
                if symbol.is_valid() {
                    definition.symbol == *symbol
                } else {
                    definition.name == *name
                }
            });
            // A type parameter or an unknown nominal is not a data definition
            // this walk can vouch for.
            let Some(definition) = definitions.next() else {
                return true;
            };
            if definitions.next().is_some() || !definition.type_parameters.is_empty() {
                return true;
            }
            data_definition_reaches_opaque_data(program, definition, visiting, bindings)
        }
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            let arguments = program
                .type_reference_table
                .type_reference_handles(*arguments)
                .to_vec();
            if arguments
                .iter()
                .any(|argument| type_reaches_opaque_data(program, *argument, visiting, bindings))
            {
                return true;
            }
            let mark = bindings.len();
            let Some(definition) =
                push_generic_application_bindings(program, *base_symbol, &arguments, bindings)
            else {
                return true;
            };
            let reaches =
                data_definition_reaches_opaque_data(program, definition, visiting, bindings);
            bindings.truncate(mark);
            reaches
        }
    }
}

fn data_definition_reaches_opaque_data(
    program: &TypedTrees,
    definition: &typed_trees::data::DataDefinition,
    visiting: &mut Vec<SymbolHandle>,
    bindings: &mut TypeBindings,
) -> bool {
    if definition.supply_mode != language_semantics::DataSupplyMode::CheckedShape {
        return true;
    }
    if visiting.contains(&definition.symbol) {
        return false;
    }
    visiting.push(definition.symbol);
    let reaches = program
        .data_members(definition)
        .iter()
        .any(|member| match member {
            DataMember::Field(field) => {
                type_reaches_opaque_data(program, field.type_reference, visiting, bindings)
            }
            DataMember::Variant(variant) => {
                program.data_payload_fields(variant).iter().any(|field| {
                    type_reaches_opaque_data(program, field.type_reference, visiting, bindings)
                })
            }
        });
    visiting.pop();
    reaches
}

pub(super) fn type_may_carry_write_in(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
    bindings: &[(SymbolHandle, TypeReferenceHandle)],
) -> bool {
    let handle = substituted_head(program, handle, bindings);
    if program.primitive_type_reference(handle).is_some() {
        return false;
    }

    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { access, .. } if !access.is_exclusive() => false,
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_may_carry_write_in(program, *base_type, bindings)
        }
        TypeReferenceNode::Unit | TypeReferenceNode::ConstExpression(_) => false,
        // A by-value record, sum or fixed array without references owns its
        // storage. It cannot add another possible caller referent when a
        // boundary returns a loan from its exclusive arguments. Apply this
        // same law at every query site, not only transition parameters.
        // Opaque, recursive-unproven and unresolved shapes remain unknown;
        // initializer effects and exact origins are checked by the callers.
        TypeReferenceNode::Named { .. }
        | TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::FixedArray { .. } => {
            !type_is_reference_free_value(program, handle, bindings)
        }
        TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::DynamicTrait { .. } => true,
    }
}

pub(super) fn type_may_carry_write(program: &TypedTrees, handle: TypeReferenceHandle) -> bool {
    type_may_carry_write_in(program, handle, &[])
}
