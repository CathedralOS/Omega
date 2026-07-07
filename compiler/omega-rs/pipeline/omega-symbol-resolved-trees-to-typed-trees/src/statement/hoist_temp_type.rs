//! Type inference for the synthetic `let __hoist_N = <indexed read>;` temps the
//! operand-hoisting normalization synthesizes (see the syntax -> symbol-resolved
//! `statement::hoist_operand_indexed_reads`). Those temps carry a `Unit`
//! declared type (the inference sentinel) because the language has no local type
//! inference and the element type is not knowable at the hoist site. Here, with
//! the resolved data-field types available, the temp's element type is derived
//! from its indexed-read initializer so the later domain/range/layout checks and
//! codegen see a concrete declared type that matches the element read.

use crate::lowerer::Lowerer;
use crate::type_reference::lower_element_applicable_constraints;
use crate::type_reference::lower_type_reference_into_table;
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees as resolved;
use omega_symbol_resolved_trees::types::{TypeConstraint, TypeReference};
use omega_typed_trees as typed;

use resolved::expression::{ExpressionHandle, ExpressionNode};

/// Infers the typed declared type for a hoist temp whose `initial_value` is a
/// runtime-indexed read. Returns `None` (the caller then keeps `Unit`) for any
/// shape it cannot resolve -- that preserves the pre-existing behavior for those
/// shapes rather than guessing.
pub(super) fn infer_hoist_temp_type(
    lowerer: &mut Lowerer,
    attached_data: Option<&resolved::name::DiagnosticName>,
    state: &resolved::state::State,
    initial_value: ExpressionHandle,
) -> Result<Option<typed::types::TypeReferenceHandle>, Diagnostic> {
    let expressions = &lowerer.source_trees.tables.bodies.expressions;

    // A pure-builtin guard subject the syntax->symbol-resolved lowering hoisted
    // (`let __hoist = min(self.a, self.b)`): the temp's type is its FIRST
    // argument's type. min/max/sqrt all return their operand's type (abs desugars
    // to `max(x, 0-x)`; clamp's outer call has a non-place first arg and is not
    // hoisted). Only a `self.<field>` first arg is typeable here, which is exactly
    // what the hoist predicate requires; any other call shape yields no type.
    let builtin_first_arg: Option<Option<ExpressionHandle>> =
        match expressions.expression(initial_value) {
            ExpressionNode::Call(call)
                if !call.receiver.is_valid()
                    && matches!(call.target.as_str(), "min" | "max" | "sqrt") =>
            {
                Some(expressions.expression_handles(call.arguments).first().copied())
            }
            ExpressionNode::Call(_) => Some(None),
            _ => None,
        };
    if let Some(first) = builtin_first_arg {
        let Some(first) = first else {
            return Ok(None);
        };
        let Some(place_type) = collection_type_reference(lowerer, attached_data, state, first)
        else {
            return Ok(None);
        };
        return Ok(Some(lower_type_reference_into_table(lowerer, &place_type)?));
    }

    // A member read hoisted through a SHARED reference-to-named param
    // (`let __hoist = table.con_out` with `table: &EfiSystemTable`): the temp's
    // type is the FIELD's declared type in the referee data definition. The
    // guard/assignment hoists synthesize exactly this shape so the read routes
    // through the pointee-deref path; the temp is slot-backed and must carry a
    // concrete type.
    if let ExpressionNode::Member(member) = expressions.expression(initial_value)
        && let ExpressionNode::Name(path) = expressions.expression(member.receiver)
        && let [only] = expressions.name_path_members(path.members)
        && let Some(TypeReference::Reference(reference)) =
            parameter_type(lowerer.source_trees, state, only.as_str())
        && !reference.is_mutable
        && let TypeReference::Named { name, .. } =
            lowerer.source_trees.child_type_reference(reference.referee)
    {
        let data_name = name.clone();
        let member_name = member.member.as_str().to_string();
        if let Some(field_type) =
            attached_field_type(lowerer.source_trees, &data_name, &member_name)
        {
            return Ok(Some(lower_type_reference_into_table(lowerer, &field_type)?));
        }
        return Ok(None);
    }

    let ExpressionNode::Indexed(indexed) = expressions.expression(initial_value) else {
        return Ok(None);
    };
    let collection = indexed.collection;

    let Some(collection_type) = collection_type_reference(lowerer, attached_data, state, collection)
    else {
        return Ok(None);
    };

    // The element type, plus the collection-level constraints to re-apply (so an
    // element read of `[i32; N] in Trapping` is `i32 in Trapping`).
    let Some((element_type, constraints)) =
        element_type_of(lowerer.source_trees, &collection_type)
    else {
        return Ok(None);
    };

    let element_handle = lower_type_reference_into_table(lowerer, &element_type)?;
    if constraints.is_empty() {
        return Ok(Some(element_handle));
    }

    // Re-apply only the collection constraints that characterize the ELEMENT
    // (arithmetic domains like `Trapping`); an encoding `Domain` (`[u8] in Utf8`)
    // is bound to the collection's storage type, not to `u8`, so it is dropped.
    let constraints = lower_element_applicable_constraints(
        lowerer.source_trees,
        &mut lowerer.typed_trees,
        constraints,
    )?;
    if constraints.is_empty() {
        return Ok(Some(element_handle));
    }
    Ok(Some(lowerer.typed_trees.type_reference_table.insert(
        typed::types::TypeReferenceNode::Constrained {
            base_type: element_handle,
            constraints,
        },
    )))
}

/// Resolves the resolved `TypeReference` of an indexed read's COLLECTION.
///
/// Handles `self.<field>` (resolved against the machine's attached data) and a
/// bare state-parameter name `<param>` (the slice/array a `[i]` indexes). Other
/// shapes (locals, nested fields, deeper chains) return `None`.
fn collection_type_reference(
    lowerer: &Lowerer,
    attached_data: Option<&resolved::name::DiagnosticName>,
    state: &resolved::state::State,
    collection: ExpressionHandle,
) -> Option<TypeReference> {
    let expressions = &lowerer.source_trees.tables.bodies.expressions;
    match expressions.expression(collection) {
        // `self.<field>`
        ExpressionNode::Member(member) => {
            let receiver_is_self = matches!(
                expressions.expression(member.receiver),
                ExpressionNode::Name(path)
                    if expressions
                        .name_path_members(path.members)
                        .first()
                        .is_some_and(|name| name.as_str() == "self")
            );
            if !receiver_is_self {
                return None;
            }
            let data_name = attached_data?;
            attached_field_type(lowerer.source_trees, data_name, member.member.as_str())
        }
        // A bare `<name>` path -- a state parameter naming the indexed collection.
        ExpressionNode::Name(path) => {
            let members = expressions.name_path_members(path.members);
            let [only] = members else {
                return None;
            };
            parameter_type(lowerer.source_trees, state, only.as_str())
        }
        _ => None,
    }
}

/// The declared resolved type of `field_name` in the data definition named
/// `data_name`.
fn attached_field_type(
    source_trees: &resolved::SymbolResolvedTrees,
    data_name: &resolved::name::DiagnosticName,
    field_name: &str,
) -> Option<TypeReference> {
    let data = source_trees
        .data_definitions
        .iter()
        .find(|data| data.name.as_str() == data_name.as_str())?;
    source_trees
        .data_members(data.members)
        .iter()
        .find_map(|member| match member {
            resolved::data::DataMember::Field(field) if field.name.as_str() == field_name => {
                Some(field.type_reference.clone())
            }
            _ => None,
        })
}

/// The declared resolved type of state parameter `name`.
fn parameter_type(
    source_trees: &resolved::SymbolResolvedTrees,
    state: &resolved::state::State,
    name: &str,
) -> Option<TypeReference> {
    source_trees
        .state_parameters(state.parameters)
        .iter()
        .find(|parameter| parameter.name.as_str() == name)
        .map(|parameter| parameter.type_reference.clone())
}

/// The element type of an indexable collection type plus the COLLECTION-LEVEL
/// constraint span to re-apply to it.
///
/// `[T; N]` / `&[T]` / `[T]` yield `(T, empty)`. A leading reference (`&[T]`) is
/// looked through. A `Constrained{inner, [c..]}` collection (`[i32; N] in
/// Trapping`) yields the element type of `inner` paired with `[c..]`, so an
/// element read inherits the collection's arithmetic/encoding domain -- exactly
/// as a runtime read of one element does.
fn element_type_of(
    source_trees: &resolved::SymbolResolvedTrees,
    type_reference: &TypeReference,
) -> Option<(TypeReference, HandleSpan<TypeConstraint>)> {
    match type_reference {
        TypeReference::FixedArray(fixed_array) => Some((
            source_trees
                .child_type_reference(fixed_array.element_type)
                .clone(),
            HandleSpan::empty(),
        )),
        TypeReference::Slice(slice) => Some((
            source_trees.child_type_reference(slice.element_type).clone(),
            HandleSpan::empty(),
        )),
        TypeReference::Reference(reference) => element_type_of(
            source_trees,
            source_trees.child_type_reference(reference.referee),
        ),
        TypeReference::Constrained(constrained) => {
            let base = source_trees.child_type_reference(constrained.base_type);
            let (element, _inner_constraints) = element_type_of(source_trees, base)?;
            // The collection's own constraints (`in Trapping`) apply to each
            // element read; an inner element does not carry collection-level
            // constraints, so this span fully describes the element's domain.
            Some((element, constrained.constraints))
        }
        _ => None,
    }
}
