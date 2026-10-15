//! Substitution copies values, not permission to infer a new array carrier.
//!
//! Numeric leaves retain their landings, but empty arrays have no leaf on
//! which to retain their element type. The exact authored constant selection
//! already leads to the declaration's complete type. Rejoin that type at every
//! destination before ordinary literal checks; refinements and borrowing still
//! owe their existing independent checks. No runtime constant value is retained.

use diagnostics::Diagnostic;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Peel arithmetic-policy shells from `reference`. `i32 in Wrapping`
/// qualifies arithmetic on the value, never its storage shape, so a
/// policy-constrained layer or element is still a closed primitive carrier;
/// any other constraint keeps its own owner and stops the peel.
pub fn peel_arithmetic_policy(
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
) -> TypeReferenceHandle {
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(reference)
    {
        if !program
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .all(|constraint| {
                matches!(
                    constraint,
                    typed_trees::types::TypeConstraintNode::ArithmeticDomain(_)
                )
            })
        {
            break;
        }
        reference = *base_type;
    }
    reference
}

/// Array-level qualification refines values, never the array's storage family.
/// Scalar element qualification retains its separate admission rules. This
/// shape query does not establish any predicate on the constructed value.
fn array_carrier_type(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> TypeReferenceHandle {
    let carrier = program
        .type_reference_table
        .unconstrained_type_reference(reference);
    if matches!(
        program.type_reference_table.type_reference(carrier),
        TypeReferenceNode::FixedArray { .. }
    ) {
        carrier
    } else {
        peel_arithmetic_policy(program, reference)
    }
}

/// The closed primitive array type a whole array literal stored into a place
/// of `reference` lands at. Array qualification keeps its proof owner while
/// construction uses the underlying exact array storage.
pub fn closed_array_store_type(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    let peeled = array_carrier_type(program, reference);
    is_closed_primitive_array_type(program, peeled).then_some(peeled)
}

/// A concrete primitive array shape supplies no qualification or ownership authority.
/// Validate its complete shape even below empty dimensions; operand evaluation
/// and effects remain separate expression obligations. Admitting IEEE storage
/// does not admit unselected floating arithmetic in its initializer.
pub fn is_closed_primitive_array_type(
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
) -> bool {
    let mut array = false;
    for _ in 0..program.type_reference_table.type_reference_count() {
        let peeled = array_carrier_type(program, reference);
        match program.type_reference_table.type_reference(peeled) {
            TypeReferenceNode::FixedArray {
                element_type,
                length: typed_trees::types::FixedArrayLength::Literal(_),
            } => {
                array = true;
                reference = *element_type;
            }
            TypeReferenceNode::Named { .. } => {
                return array
                    && matches!(
                        program.primitive_type_reference(peeled),
                        Some(
                            typed_trees::types::PrimitiveType::Bool
                                | typed_trees::types::PrimitiveType::I8
                                | typed_trees::types::PrimitiveType::I16
                                | typed_trees::types::PrimitiveType::I32
                                | typed_trees::types::PrimitiveType::I64
                                | typed_trees::types::PrimitiveType::U8
                                | typed_trees::types::PrimitiveType::U16
                                | typed_trees::types::PrimitiveType::U32
                                | typed_trees::types::PrimitiveType::U64
                                | typed_trees::types::PrimitiveType::F32
                                | typed_trees::types::PrimitiveType::F64
                        )
                    );
            }
            _ => return false,
        }
    }
    false
}

/// The packed scalar constructor retains whole-array memberships, but has no
/// projected membership roster. Nested qualified arrays use ordinary element
/// construction so each element keeps its checked evidence. Both constructors
/// still produce the same exact fixed-array carrier and native geometry.
pub fn scalar_array_preserves_memberships(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> bool {
    if !is_closed_primitive_array_type(program, reference) {
        return false;
    }
    let mut current = array_carrier_type(program, reference);
    for _ in 0..program.type_reference_table.type_reference_count() {
        match program.type_reference_table.type_reference(current) {
            TypeReferenceNode::FixedArray { element_type, .. } => {
                // Arithmetic policies add no standing predicate obligation.
                // Other nested constraints cannot disappear in flattening.
                current = peel_arithmetic_policy(program, *element_type);
            }
            TypeReferenceNode::Named { .. } => return true,
            _ => return false,
        }
    }
    false
}

/// Rejoin an exact declaration and every selected builtin index before
/// extracting row-major literals. Unselected siblings must also be closed:
/// projection may not suppress an evaluation, failure, or effect.
/// Each leaf retains its exact destination carrier so anonymous integer literals
/// can be rendered by the numeric landing owner without rewriting authored trees.
pub fn closed_constant_array_elements(
    program: &dyn crate::CheckingInput,
    machine: symbols::SymbolHandle,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
) -> Option<Vec<(ExpressionHandle, typed_trees::types::PrimitiveType)>> {
    if !scalar_array_preserves_memberships(program, expected) {
        return None;
    }
    let mut root = expression;
    let mut indices = Vec::new();
    let mut projections = Vec::new();
    while let Some(indexed) = program.expression_table.indexed_node(root) {
        if projections.contains(&root) {
            return None;
        }
        projections.push(root);
        indices.push(indexed.index);
        root = indexed.collection;
    }
    let mut reference = declared_constant_array_type(program, root).or_else(|| {
        (indices.is_empty()
            && program
                .expression_table
                .authored_selection_occurrences(root)
                .next()
                .is_none())
        .then_some(expected)
    })?;
    if !is_closed_primitive_array_type(program, reference) {
        return None;
    }
    closed_literal_array_elements(program, root, reference)?;
    if !indices.is_empty() {
        let projected = builtin_constant_array_projection_type(program, machine, expression)?;
        if program.normalized_type_identity(projected) != program.normalized_type_identity(expected)
        {
            return None;
        }
    }
    let mut selected = root;
    for index in indices.into_iter().rev() {
        let TypeReferenceNode::FixedArray {
            element_type,
            length: typed_trees::types::FixedArrayLength::Literal(length),
        } = program
            .type_reference_table
            .type_reference(array_carrier_type(program, reference))
        else {
            return None;
        };
        let literal = program.expression_table.integer_node(index)?;
        if literal.landing().is_some_and(|landing| {
            landing.domain != numerics::arithmetic::ArithmeticDomain::Exact
                || landing.landed_type == numerics::literals::LandedIntegerType::Addr
        }) {
            return None;
        }
        let index = usize::try_from(literal.value_u64()?).ok()?;
        if index >= *length {
            return None;
        }
        let elements = program.expression_table.array_literal_node(selected)?;
        selected = *program
            .expression_table
            .expression_handles(*elements)
            .get(index)?;
        reference = *element_type;
    }
    if program.normalized_type_identity(reference) != program.normalized_type_identity(expected) {
        return None;
    }
    closed_literal_array_elements(program, selected, reference)
}

/// Temporary source-shape result; expressions remain owned by the typed tree.
pub struct ScalarArrayElements {
    pub elements: Vec<(ExpressionHandle, typed_trees::types::PrimitiveType)>,
    /// Array-valued projections eliminated only after closed literal validation.
    /// Scalar-valued indexing remains with each ordinary scalar operand owner.
    pub projections: Vec<ExpressionHandle>,
}

/// Resolve the exact shape and authored row-major scalar operands of an array.
/// Direct constructors evaluate every leaf normally. Selecting a substituted
/// constant row still requires all unselected siblings to be closed literals;
/// this query never makes an effectful sibling disappear.
pub fn scalar_array_elements(
    program: &dyn crate::CheckingInput,
    machine: symbols::SymbolHandle,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
) -> Option<ScalarArrayElements> {
    if !scalar_array_preserves_memberships(program, expected) {
        return None;
    }
    let mut leaves = Vec::new();
    let mut projections = Vec::new();
    let mut active = Vec::new();
    let mut pending = vec![(expression, expected, false)];
    while let Some((expression, reference, exiting)) = pending.pop() {
        if exiting {
            active.pop();
            continue;
        }
        if active.contains(&expression) || !program.expression_table.expression_is_valid(expression)
        {
            return None;
        }
        active.push(expression);
        pending.push((expression, reference, true));
        let peeled = array_carrier_type(program, reference);
        match program.type_reference_table.type_reference(peeled) {
            TypeReferenceNode::FixedArray {
                element_type,
                length: typed_trees::types::FixedArrayLength::Literal(length),
            } => {
                if matches!(
                    program.expression_table.expression(expression),
                    ExpressionNode::Indexed(_)
                ) || program
                    .expression_table
                    .authored_selection_occurrences(expression)
                    .next()
                    .is_some()
                {
                    leaves.extend(closed_constant_array_elements(
                        program, machine, expression, reference,
                    )?);
                    let mut selected = expression;
                    let mut selected_projections = Vec::new();
                    while let Some(indexed) = program.expression_table.indexed_node(selected) {
                        selected_projections.push(selected);
                        selected = indexed.collection;
                    }
                    projections.extend(selected_projections.into_iter().rev());
                    continue;
                }
                let elements = program.expression_table.array_literal_node(expression)?;
                if !extend_fixed_array_elements(
                    program,
                    &mut pending,
                    elements,
                    element_type,
                    *length,
                ) {
                    return None;
                }
            }
            TypeReferenceNode::Named { .. } => {
                leaves.push((expression, program.primitive_type_reference(reference)?))
            }
            _ => return None,
        }
    }
    Some(ScalarArrayElements {
        elements: leaves,
        projections,
    })
}

/// Check closed literal payloads in their declared primitive carriers. Scalar
/// qualifications add no evaluation or storage; this does not establish their
/// predicates or routed authority. Constructor and destination checking retain
/// those obligations on the complete declared type before lowering.
pub fn closed_literal_array_elements(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
) -> Option<Vec<(ExpressionHandle, typed_trees::types::PrimitiveType)>> {
    let mut leaves = Vec::new();
    let mut pending = vec![(expression, reference, false)];
    let mut active = Vec::new();
    while let Some((expression, reference, exiting)) = pending.pop() {
        if exiting {
            active.pop();
            continue;
        }
        if active.contains(&expression) {
            return None;
        }
        active.push(expression);
        pending.push((expression, reference, true));
        match (
            program.expression_table.expression(expression),
            program
                .type_reference_table
                .type_reference(array_carrier_type(program, reference)),
        ) {
            (
                ExpressionNode::ArrayLiteral(elements),
                TypeReferenceNode::FixedArray {
                    element_type,
                    length: typed_trees::types::FixedArrayLength::Literal(length),
                },
            ) => {
                if !extend_fixed_array_elements(
                    program,
                    &mut pending,
                    elements,
                    element_type,
                    *length,
                ) {
                    return None;
                }
            }
            (
                ExpressionNode::Integer(literal),
                TypeReferenceNode::Named { .. } | TypeReferenceNode::Constrained { .. },
            ) => {
                let primitive = program.primitive_type_reference(reference)?;
                if let Some(landing) = literal.landing() {
                    if landing.domain != numerics::arithmetic::ArithmeticDomain::Exact
                        || Some(primitive)
                            != typed_trees::types::PrimitiveType::from_name(
                                landing.landed_type.name(),
                            )
                    {
                        return None;
                    }
                } else {
                    super::land_anonymous_integer_expression(
                        program,
                        expression,
                        primitive,
                        &[],
                        |_| false,
                    )?;
                }
                leaves.push((expression, primitive));
            }
            (
                ExpressionNode::Boolean(_),
                TypeReferenceNode::Named { .. } | TypeReferenceNode::Constrained { .. },
            ) if program.primitive_type_reference(reference)
                == Some(typed_trees::types::PrimitiveType::Bool) =>
            {
                leaves.push((expression, typed_trees::types::PrimitiveType::Bool))
            }
            (
                ExpressionNode::Float(literal),
                TypeReferenceNode::Named { .. } | TypeReferenceNode::Constrained { .. },
            ) => {
                let primitive = program.primitive_type_reference(reference)?;
                // A projection may discard siblings only after every leaf is
                // a closed value in its declared format, never a conversion.
                if !matches!(
                    (primitive, literal.landing()),
                    (
                        typed_trees::types::PrimitiveType::F32,
                        Some(numerics::literals::FloatFormat::F32)
                    ) | (
                        typed_trees::types::PrimitiveType::F64,
                        Some(numerics::literals::FloatFormat::F64)
                    )
                ) {
                    return None;
                }
                leaves.push((expression, primitive));
            }
            _ => return None,
        }
    }
    Some(leaves)
}

/// Recover a substituted array value's exact declared type, not a storage
/// origin or permission to borrow it. Indexing and operator matching need the
/// complete type even when there are no literal leaves from which to infer it.
pub fn declared_constant_array_type(
    program: &dyn crate::CheckingInput,
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
        // Substitution keeps inherited initializer selections as provenance.
        // Only this authored use supplies the receiving declaration's type;
        // an inherited alias must not compete via its separately stored type.
        if selection.source_span() != program.expression_table.source_span(value) {
            continue;
        }
        let target = selection.target();
        let Some(selected) = target.as_resolved() else {
            continue;
        };
        let Some(declaration) = program.const_declaration_by_symbol(selected.selected_symbol())
        else {
            continue;
        };
        if !matches!(
            program
                .type_reference_table
                .type_reference(array_carrier_type(program, declaration.declared_type)),
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
    program: &dyn crate::CheckingInput,
    machine_symbol: symbols::SymbolHandle,
    mut expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let mut projections = Vec::new();
    while let Some(indexed) = program.expression_table.indexed_node(expression) {
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
        let TypeReferenceNode::FixedArray { element_type, .. } = program
            .type_reference_table
            .type_reference(array_carrier_type(program, reference))
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
            program.expression_table.source_span(expression),
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
    program: &dyn crate::CheckingInput,
    mut value: ExpressionHandle,
    destination: TypeReferenceHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    value = program.expression_table.peeled_expression_handle(value);
    if !matches!(
        program.expression_table.expression(value),
        ExpressionNode::ArrayLiteral(_)
    ) {
        return;
    }
    let Some(destination) =
        crate::value_custody::places::unwrapped_type_reference(program, destination)
    else {
        return;
    };
    for occurrence in program
        .expression_table
        .authored_selection_occurrences(value)
    {
        let Some(selection) = program.authored_declaration_selections().get(occurrence) else {
            continue;
        };
        let target = selection.target();
        let Some(selected) = target.as_resolved() else {
            continue;
        };
        let Some(declaration) = program.const_declaration_by_symbol(selected.selected_symbol())
        else {
            continue;
        };
        if matches!(
            program
                .type_reference_table
                .type_reference(array_carrier_type(program, declaration.declared_type)),
            TypeReferenceNode::FixedArray { .. }
        ) && !same_array_carrier(program, declaration.declared_type, destination)
        {
            diagnostics.push(Diagnostic::error(
                "constant array destination conflicts with its declared dimensions or element carrier"
            ).with_source_span(selection.source_span()));
        }
    }
}

/// Check projected constant values before permissive computed-value shape
/// fallbacks. Empty rows still have exact element types, and nesting the read
/// in a new array literal cannot erase them. Operator selection supplies the
/// projected type; bounds and loan validation remain independent obligations.
/// Recurse for scalar projections too: value-position call arguments do not
/// otherwise run the statement path's per-element array validation.
pub(crate) fn validate_constant_projection_destination(
    program: &dyn crate::CheckingInput,
    machine_symbol: symbols::SymbolHandle,
    value: ExpressionHandle,
    destination: TypeReferenceHandle,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let mut pending = vec![(value, destination)];
    let mut conflict = false;
    while let Some((value, mut destination)) = pending.pop() {
        while let TypeReferenceNode::Constrained { base_type, .. } =
            program.type_reference_table.type_reference(destination)
        {
            destination = *base_type;
        }
        let shared_lending = matches!(
            program.type_reference_table.type_reference(destination),
            TypeReferenceNode::Reference {
                access: language_semantics::ReferenceAccess::Shared,
                ..
            }
        );
        let Some(destination) =
            crate::value_custody::places::unwrapped_type_reference(program, destination)
        else {
            continue;
        };
        if let Some(declared) =
            builtin_constant_array_projection_type(program, machine_symbol, value).or_else(|| {
                // A field read from a closed record remains its declared
                // scalar type, not an anonymous literal which can reland into
                // another width just because this particular payload fits.
                super::record_projection::closed_scalar_projection(program, value)
                    .map(|(_, reference)| reference)
            })
        {
            // Shared array-to-slice lending forgets only the outer extent.
            // Its element identity is still exact; this type comparison grants
            // neither a loan nor permission for the resulting view to escape.
            let matches = match (
                program.type_reference_table.type_reference(declared),
                program.type_reference_table.type_reference(destination),
            ) {
                (
                    TypeReferenceNode::FixedArray {
                        element_type: actual,
                        ..
                    },
                    TypeReferenceNode::Slice {
                        element_type: expected,
                    },
                ) if shared_lending => same_array_carrier(program, *actual, *expected),
                _ => same_array_carrier(program, declared, destination),
            };
            if !matches {
                diagnostics.push(Diagnostic::error(format!(
                    "constant array projection of type `{}` conflicts with destination `{}`; declared dimensions and element carriers must agree",
                    program.display_type_reference_with_constraints(declared),
                    program.display_type_reference_with_constraints(destination),
                )));
                conflict = true;
            }
        }
        if let ExpressionNode::ArrayLiteral(elements) = program.expression_table.expression(value)
            && let TypeReferenceNode::FixedArray { element_type, .. } =
                program.type_reference_table.type_reference(destination)
        {
            pending.extend(
                program
                    .expression_table
                    .expression_handles(*elements)
                    .iter()
                    .map(|element| (*element, *element_type)),
            );
        }
    }
    conflict
}

/// Push a fixed array's literal elements onto the pending walk when the
/// literal count matches the declared length; `false` rejects a mismatched
/// literal.
fn extend_fixed_array_elements(
    program: &dyn crate::CheckingInput,
    pending: &mut Vec<(ExpressionHandle, TypeReferenceHandle, bool)>,
    elements: &arena::HandleSpan<ExpressionHandle>,
    element_type: &TypeReferenceHandle,
    length: usize,
) -> bool {
    let elements = program.expression_table.expression_handles(*elements);
    if elements.len() != length {
        return false;
    }
    pending.extend(
        elements
            .iter()
            .rev()
            .map(|element| (*element, *element_type, false)),
    );
    true
}

fn same_array_carrier(
    program: &dyn crate::CheckingInput,
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
