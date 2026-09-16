//! Type capability queries used by caller-visible write-frame inference.
//!
//! These queries classify constrained references and whether a parameter can
//! carry a caller-visible write. They do not traverse expressions, resolve
//! calls, or summarize frames.

use super::type_instantiation::substituted_head;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
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

pub(super) fn parameter_may_carry_write(program: &TypedTrees, parameter: &StateParameter) -> bool {
    type_may_carry_write(program, parameter.type_reference)
}

/// Under an active substitution a bound parameter's actual answers the
/// capability question; an unbound parameter keeps the named-leaf result.
pub(super) fn type_may_carry_write_in(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
    bindings: &[(SymbolHandle, TypeReferenceHandle)],
) -> bool {
    type_may_carry_write(program, substituted_head(program, handle, bindings))
}

pub(super) fn type_may_carry_write(program: &TypedTrees, handle: TypeReferenceHandle) -> bool {
    if program.primitive_type_reference(handle).is_some() {
        return false;
    }

    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { access, .. } if !access.is_exclusive() => false,
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_may_carry_write(program, *base_type)
        }
        TypeReferenceNode::Unit | TypeReferenceNode::ConstExpression(_) => false,
        TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::Named { .. }
        | TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::DynamicTrait { .. } => true,
    }
}
