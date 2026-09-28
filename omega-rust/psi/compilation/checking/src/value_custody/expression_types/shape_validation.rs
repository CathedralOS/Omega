use super::operator_validation::{type_reference_is_array, type_reference_is_text_carrier};
use super::value_classification::{
    ValueClass, concrete_data_type_name, value_class, value_concrete_data_name,
};
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Whether a value's SHAPE is an array (`Some(true)`), a non-array scalar/struct
/// (`Some(false)`), or undeterminable here (`None` -> skipped): an array literal
/// vs a scalar literal, or a place resolved through `declared_place_type`. A
/// computed value (call, binary, indexed) is `None` so this never false-positives.
pub(super) fn value_shape_is_array(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: Option<&typed_trees::state::State>,
    value: ExpressionHandle,
) -> Option<bool> {
    match program.expression_table.expression(value) {
        ExpressionNode::ArrayLiteral(_) => Some(true),
        ExpressionNode::Integer(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::String(_)
        | ExpressionNode::StructLiteral(_) => Some(false),
        ExpressionNode::Borrow(inner) => {
            value_shape_is_array(program, machine, state, inner.target)
        }
        ExpressionNode::Name(_) | ExpressionNode::Member(_) => {
            crate::value_custody::places::declared_place_type(program, machine, state, value)
                .map(|type_reference| type_reference_is_array(program, type_reference))
        }
        _ => None,
    }
}

/// Reject binding a value of the wrong SHAPE to a target: an array into a
/// non-array slot (`let y: i32 = self.xs`, which silently read a ZII 0) or a
/// non-array value into an array slot (`let xs: [i32; 3] = 5`). Both sides must be
/// determinable; a computed value (a call result) is skipped. Complements the
/// scalar-CLASS and nominal-DATA checks, which both classify only scalar shapes.
pub(crate) fn report_array_scalar_shape_mismatch(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: Option<&typed_trees::state::State>,
    value: ExpressionHandle,
    target_type: TypeReferenceHandle,
    slot_context: &str,
    slot_noun: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    if crate::value_custody::literals::validate_constant_projection_destination(
        program,
        machine.symbol,
        value,
        target_type,
        diagnostics,
    ) {
        return true;
    }
    // TEXT is `&[u8]`-backed, so a `String`, a byte slice, and a `[u8; N]` are one
    // shape family and values flow between them freely (`write_line([u8])` takes a
    // String; a byte-slice value fills a String param). The array-vs-scalar
    // dichotomy does not apply -- skip when EITHER side is a text carrier (the
    // cross-class store gate still governs text-vs-numeric).
    if type_reference_is_text_carrier(program, target_type)
        || value_class(program, Some(machine), state, value) == Some(ValueClass::Text)
        || crate::value_custody::places::declared_place_type(program, machine, state, value)
            .is_some_and(|value_type| type_reference_is_text_carrier(program, value_type))
    {
        return false;
    }
    let Some(value_is_array) = value_shape_is_array(program, machine, state, value) else {
        return false;
    };
    if value_is_array == type_reference_is_array(program, target_type) {
        return false;
    }
    diagnostics.push(Diagnostic::error(if value_is_array {
        format!("{slot_context} binds an ARRAY value into a non-array {slot_noun}")
    } else {
        format!("{slot_context} binds a non-array value into an ARRAY {slot_noun}")
    }));
    true
}

/// Reject binding a SCALAR value into a DATA (struct/enum) target, or a DATA value
/// into a SCALAR target: `self.point = 5` (a scalar into a struct field) silently
/// clobbers the struct's leading bytes, and `let n: i32 = self.point` (a struct into
/// a scalar slot) silently reads a ZII `0`. This cross-shape case fell between the
/// two type gates: the scalar-CLASS gate needs a primitive TARGET (a struct target
/// has none, so it is skipped) and the nominal gate needs BOTH sides to resolve to
/// data names (a scalar does not). Fires only when one side is a PROVABLE scalar
/// (`value_class` / a primitive target) and the other a concrete data type; arrays
/// (owned by the array-shape check), text carriers, and unresolvable computed values
/// are left alone.
pub(crate) fn report_scalar_data_shape_mismatch(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: Option<&typed_trees::state::State>,
    value: ExpressionHandle,
    target_type: TypeReferenceHandle,
    slot_context: &str,
    slot_noun: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    // Scalar VALUE (a bool/number/text literal or a primitive-typed place) into a
    // DATA target.
    if let Some(target_name) = concrete_data_type_name(program, target_type)
        && let Some(value_scalar_class) = value_class(program, Some(machine), state, value)
    {
        diagnostics.push(Diagnostic::error(format!(
            "{slot_context} binds {} into the `{target_name}` data {slot_noun}; a scalar value \
             cannot fill a struct or enum slot",
            value_scalar_class.describe(),
        )));
        return true;
    }
    // DATA VALUE (a struct literal or a data-typed place) into a SCALAR target.
    if program.primitive_type_reference(target_type).is_some()
        && let Some(value_name) = value_concrete_data_name(program, machine, state, value)
    {
        diagnostics.push(Diagnostic::error(format!(
            "{slot_context} binds a `{value_name}` value into a scalar {slot_noun}; a struct or \
             enum value cannot fill a scalar slot",
        )));
        return true;
    }
    false
}

/// The slice-view element a parameter declares: `&[T]` under any
/// `Constrained`/`Reference` shells. Anything else has no element to check.
fn slice_view_element(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    let mut handle = type_reference;
    loop {
        match program.type_reference_table.type_reference(handle) {
            TypeReferenceNode::Constrained { base_type, .. } => handle = *base_type,
            TypeReferenceNode::Reference { referee, .. } => handle = *referee,
            TypeReferenceNode::Slice { element_type } => return Some(*element_type),
            _ => return None,
        }
    }
}

/// The element type a slice or fixed-array type reference supplies; a scalar,
/// data, or unresolvable type has none.
fn collection_element(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Slice { element_type }
        | TypeReferenceNode::FixedArray { element_type, .. } => Some(*element_type),
        _ => None,
    }
}

/// The element type an ARGUMENT's declared collection type supplies: a view
/// place (`s: &[i32]`), an owned array place (`self.arr: [u8; 4]`), a borrowed
/// one (`&self.arr`), a range subslice (`s[1..]` — its type is the source
/// collection's), or a call result declared with a collection return.
/// Literals, element projections (`s[0]`), match arms, and anything that does
/// not resolve to a concrete collection type return `None` and stay with plan
/// admission, which already arbitrates them.
fn argument_collection_element(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: Option<&typed_trees::state::State>,
    argument: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let mut handle = argument;
    while let ExpressionNode::Borrow(inner) = program.expression_table.expression(handle) {
        handle = inner.target;
    }
    if let ExpressionNode::Indexed(indexed) = program.expression_table.expression(handle)
        && matches!(
            program.expression_table.expression(indexed.index),
            ExpressionNode::Range(_)
        )
    {
        return crate::value_custody::places::declared_place_type(
            program,
            machine,
            state,
            indexed.collection,
        )
        .and_then(|type_reference| collection_element(program, type_reference));
    }
    crate::value_custody::places::declared_place_type(program, machine, state, handle)
        .and_then(|type_reference| collection_element(program, type_reference))
}

/// An element type both sides can compare: concrete under `Constrained`
/// shells. A type parameter, generic slot, or dynamic trait stays open for
/// inference, so the call keeps its existing acceptance shape.
fn element_is_concrete(program: &TypedTrees, element: TypeReferenceHandle) -> bool {
    let mut handle = element;
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(handle)
    {
        handle = *base_type;
    }
    !matches!(
        program.type_reference_table.type_reference(handle),
        TypeReferenceNode::Named { symbol, .. }
            if program.symbols.get(*symbol).kind == symbols::SymbolKind::TypeParameter
    ) && !matches!(
        program.type_reference_table.type_reference(handle),
        TypeReferenceNode::Generic { .. } | TypeReferenceNode::DynamicTrait { .. }
    )
}

/// Reject a call ARGUMENT whose declared collection element provably differs
/// from the callee parameter's slice-view element: `let n =
/// self.walker.walk(s)` binds `s: &[i32]` into a `&[u8]` parameter, and value
/// calls skip the reference shape gate (text/byte/`addr` spellings make it a
/// false-positive minefield), so the argument bound to nothing and the callee
/// lowered with no admitted body. The element comparison is the same
/// `normalized_type_identity` rule the argument binders apply: a fixed array
/// lends its elements to a slice of the same element type, and every other
/// shape only admits an exact element match. Both sides must resolve to a
/// concrete element; a generic formal or an argument whose type cannot be
/// resolved keeps the existing permissive shape.
pub(crate) fn report_view_element_argument_mismatch(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: Option<&typed_trees::state::State>,
    argument: ExpressionHandle,
    target_type: TypeReferenceHandle,
    slot_context: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let Some(required_element) = slice_view_element(program, target_type) else {
        return false;
    };
    let Some(actual_element) = argument_collection_element(program, machine, state, argument)
    else {
        return false;
    };
    if !element_is_concrete(program, required_element)
        || !element_is_concrete(program, actual_element)
        || program.normalized_type_identity(actual_element)
            == program.normalized_type_identity(required_element)
    {
        return false;
    }
    diagnostics.push(Diagnostic::error(format!(
        "{slot_context} expects element type `{}`, but the argument supplies `{}`",
        program.display_type_reference_with_constraints(required_element),
        program.display_type_reference_with_constraints(actual_element),
    )));
    true
}
