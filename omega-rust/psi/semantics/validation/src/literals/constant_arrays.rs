//! Substitution copies values, not permission to infer a new array carrier.
//!
//! Integer leaves retain numeric landings, but empty arrays have no leaf on
//! which to retain their element type. The exact authored constant selection
//! already leads to the declaration's complete type. Rejoin that type at every
//! destination before ordinary literal checks; refinements and borrowing still
//! owe their existing independent checks. No runtime constant value is retained.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Recover a substituted array value's exact declared type, not a storage
/// origin or permission to borrow it. Indexing and operator matching need the
/// complete type even when there are no literal leaves from which to infer it.
pub fn declared_constant_array_type(
    program: &TypedTrees,
    value: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    if !matches!(
        program.expression_table.expression(value),
        ExpressionNode::ArrayLiteral(_)
    ) {
        return None;
    }
    let mut selected_type = None;
    for occurrence in program
        .expression_table
        .authored_selection_occurrences(value)
    {
        let selection = program.authored_declaration_selections().get(occurrence)?;
        let AuthoredDeclarationSelectionTarget::Resolved(selected) = selection.target() else {
            continue;
        };
        let Some(declaration) = program
            .const_declarations()
            .iter()
            .find(|declaration| declaration.symbol == selected.selected_symbol())
        else {
            continue;
        };
        if !matches!(
            program
                .type_reference_table
                .type_reference(declaration.declared_type),
            TypeReferenceNode::FixedArray { .. }
        ) {
            return None;
        }
        if selected_type.is_some_and(|previous| previous != declaration.declared_type) {
            return None;
        }
        selected_type = Some(declaration.declared_type);
    }
    selected_type
}

/// Type of a fixed builtin projection from a selected constant value. This
/// checks operation meaning, not bounds or borrowing. Retaining the projection
/// type prevents a typed leaf from becoming an anonymous destination literal.
pub fn builtin_constant_array_projection_type(
    program: &TypedTrees,
    machine_symbol: symbols::SymbolHandle,
    mut expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let mut projections = Vec::new();
    while let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) {
        if !matches!(
            program.expression_table.expression(indexed.index),
            ExpressionNode::Integer(_)
        ) {
            return None;
        }
        projections.push(expression);
        expression = indexed.collection;
    }
    if projections.is_empty() {
        return None;
    }
    let mut reference = declared_constant_array_type(program, expression)?;
    for projection in projections.into_iter().rev() {
        let TypeReferenceNode::FixedArray { element_type, .. } =
            program.type_reference_table.type_reference(reference)
        else {
            return None;
        };
        // Literal selectors have no declaration type in the checked indexing
        // selector. A later numeric landing cannot retroactively exclude an
        // authored overload that still participates at that binding site.
        let operands = [Some(reference), None];
        if !typed_trees::operator::resolve_indexed_spelling_for_operands(
            program,
            language_core::OperatorSpelling::Index,
            &operands,
        )
        .is_empty()
            || !typed_trees::operator::has_builtin_spelled_expression_meaning(
                program,
                machine_symbol,
                projection,
                language_core::OperatorSpelling::Index,
                &operands,
            )
        {
            return None;
        }
        reference = *element_type;
    }
    Some(reference)
}

pub(super) fn validate_declared_array_destination(
    program: &TypedTrees,
    mut value: ExpressionHandle,
    destination: TypeReferenceHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    while let ExpressionNode::Borrow(borrow) = program.expression_table.expression(value) {
        value = borrow.target;
    }
    if !matches!(
        program.expression_table.expression(value),
        ExpressionNode::ArrayLiteral(_)
    ) {
        return;
    }
    let Some(destination) = crate::places::unwrapped_type_reference(program, destination) else {
        return;
    };
    for occurrence in program
        .expression_table
        .authored_selection_occurrences(value)
    {
        let Some(selection) = program.authored_declaration_selections().get(occurrence) else {
            continue;
        };
        let AuthoredDeclarationSelectionTarget::Resolved(selected) = selection.target() else {
            continue;
        };
        let Some(declaration) = program
            .const_declarations()
            .iter()
            .find(|declaration| declaration.symbol == selected.selected_symbol())
        else {
            continue;
        };
        if matches!(
            program
                .type_reference_table
                .type_reference(declaration.declared_type),
            TypeReferenceNode::FixedArray { .. }
        ) && !same_array_carrier(program, declaration.declared_type, destination)
        {
            diagnostics.push(Diagnostic::error(
                "constant array destination conflicts with its declared dimensions or element carrier"
            ).with_source_span(selection.source_span()));
        }
    }
}

fn same_array_carrier(
    program: &TypedTrees,
    mut declared: TypeReferenceHandle,
    mut destination: TypeReferenceHandle,
) -> bool {
    // Only qualification shells are transparent here. A reference-valued
    // element must never become the scalar it points at, even in an empty array.
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(declared)
    {
        declared = *base_type;
    }
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(destination)
    {
        destination = *base_type;
    }
    match (
        program.type_reference_table.type_reference(declared),
        program.type_reference_table.type_reference(destination),
    ) {
        (
            TypeReferenceNode::FixedArray {
                element_type: left,
                length: left_length,
            },
            TypeReferenceNode::FixedArray {
                element_type: right,
                length: right_length,
            },
        ) => left_length == right_length && same_array_carrier(program, *left, *right),
        (
            TypeReferenceNode::Named { symbol: left, .. },
            TypeReferenceNode::Named { symbol: right, .. },
        ) => left.is_valid() && left == right,
        // Legacy root constants may already have generic nominal elements.
        // Preserve exact existing type identity without admitting new module
        // initializer forms or erasing reference access and generic arguments.
        _ => {
            program
                .type_reference_table
                .contains_type_reference(declared)
                && program
                    .type_reference_table
                    .contains_type_reference(destination)
                && program.normalized_type_identity(declared)
                    == program.normalized_type_identity(destination)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use typed_trees::types::FixedArrayLength;

    #[test]
    fn empty_array_comparison_peels_qualifications_but_not_reference_elements() {
        let mut program = TypedTrees::default();
        let element = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: symbols::SymbolHandle::from_parts(1, 1),
                name: typed_trees::name::Identifier::generated_static("u8"),
            });
        let source = program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: element,
                length: FixedArrayLength::Literal(0),
            });
        let qualified_element =
            program
                .type_reference_table
                .insert(TypeReferenceNode::Constrained {
                    base_type: element,
                    constraints: Default::default(),
                });
        let qualified = program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: qualified_element,
                length: FixedArrayLength::Literal(0),
            });
        assert!(same_array_carrier(&program, source, qualified));
        let reference = program
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: element,
                access: language_semantics::ReferenceAccess::Shared,
                lifetime: None,
            });
        let reference_array = program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: reference,
                length: FixedArrayLength::Literal(0),
            });
        assert!(!same_array_carrier(&program, source, reference_array));
        let mut generic_arrays = Vec::new();
        for argument in [element, element, reference] {
            let arguments = program
                .type_reference_table
                .insert_type_reference_handles([argument]);
            let generic = program
                .type_reference_table
                .insert(TypeReferenceNode::Generic {
                    base_symbol: symbols::SymbolHandle::from_parts(2, 1),
                    base_name: typed_trees::name::Identifier::generated_static("Holder"),
                    lifetime_arguments: vec![],
                    arguments,
                });
            generic_arrays.push(program.type_reference_table.insert(
                TypeReferenceNode::FixedArray {
                    element_type: generic,
                    length: FixedArrayLength::Literal(0),
                },
            ));
        }
        assert!(same_array_carrier(
            &program,
            generic_arrays[0],
            generic_arrays[1]
        ));
        assert!(!same_array_carrier(
            &program,
            generic_arrays[0],
            generic_arrays[2]
        ));
    }
}
