/*
Anonymous numeric and array literal evaluation and landing (`literals`)
alongside struct and variant constructor validation (`struct_literals`).
*/
pub mod literals {
    use diagnostics::Diagnostic;
    use numerics::literals::LandedIntegerType;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::signature::StateParameter;
    use typed_trees::statement::StatementNode;
    use typed_trees::types::PrimitiveType;

    pub(crate) fn primitive_landing_type(primitive: PrimitiveType) -> Option<LandedIntegerType> {
        primitive.landed_landing()
    }

    fn named_transition_arguments<'program>(
        program: &'program dyn crate::CheckingInput,
        target_node: &typed_trees::statement::TransitionTargetNode,
    ) -> Option<Vec<(&'program StateParameter, ExpressionHandle)>> {
        let typed_trees::statement::TransitionTargetNode::Named {
            path, arguments, ..
        } = target_node
        else {
            return None;
        };
        let parameters = crate::call_target_parameters(program, path.symbol)?;
        Some(
            parameters
                .iter()
                .zip(
                    program
                        .statement_table
                        .expression_handles(*arguments)
                        .iter()
                        .copied(),
                )
                .collect(),
        )
    }

    pub(crate) fn validate_suffix_magnitudes(
        program: &dyn crate::CheckingInput,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for (_, node) in program.expression_table.expression_entries() {
            let ExpressionNode::Integer(literal) = node else {
                continue;
            };
            let Some(landing) = literal.landing() else {
                continue;
            };
            let landed = landing.landed_type;
            let width = landed.bit_width();
            let fits = if landed.is_signed() {
                literal.value_i64().is_some_and(|value| {
                    if width == 64 {
                        true
                    } else {
                        let min = -(1i64 << (width - 1));
                        let max = (1i64 << (width - 1)) - 1;
                        (min..=max).contains(&value)
                    }
                })
            } else {
                !literal.is_negative()
                    && literal.value_u64().is_some_and(|value| {
                        if width == 64 {
                            true
                        } else {
                            value < (1u64 << width)
                        }
                    })
            };
            if !fits {
                diagnostics.push(Diagnostic::error(format!(
                    "literal `{}` does not fit its `{}` suffix -- a width suffix chooses the \
                 literal's type at the spelling, and the spelled value must fit that type's \
                 range (suffixes read VALUES, not bit patterns: spell `-1i8`, not `0xFFi8`)",
                    literal,
                    landed.name(),
                )));
            }
        }
    }

    pub(crate) mod float_landing {
        pub(crate) use typed_trees::source_numeric::validate_numeric_tree_destination;
    }
    pub(crate) mod constant_arrays {
        use crate::frozen_program::expression_table;
        use diagnostics::Diagnostic;
        use numerics::literals::FloatFormat;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::types::PrimitiveType;
        use typed_trees::types::TypeConstraintNode;
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

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
                    .all(|constraint| matches!(constraint, TypeConstraintNode::ArithmeticDomain(_)))
                {
                    break;
                }
                reference = *base_type;
            }
            reference
        }

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

        pub fn closed_array_store_type(
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
        ) -> Option<TypeReferenceHandle> {
            let peeled = array_carrier_type(program, reference);
            is_closed_primitive_array_type(program, peeled).then_some(peeled)
        }

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
                                    PrimitiveType::Bool
                                        | PrimitiveType::I8
                                        | PrimitiveType::I16
                                        | PrimitiveType::I32
                                        | PrimitiveType::I64
                                        | PrimitiveType::U8
                                        | PrimitiveType::U16
                                        | PrimitiveType::U32
                                        | PrimitiveType::U64
                                        | PrimitiveType::F32
                                        | PrimitiveType::F64
                                )
                            );
                    }
                    _ => return false,
                }
            }
            false
        }

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
                        current = peel_arithmetic_policy(program, *element_type);
                    }
                    TypeReferenceNode::Named { .. } => return true,
                    _ => return false,
                }
            }
            false
        }

        /*
            A constant's array value is read under that constant's declared type,
            so a literal that may stand for a constant must find its declaration
            before its leaves are trusted. The authored selections retained on the
            literal say which case applies: `expression_const_declaration` names a
            constant through a selection that covers the literal's own span and
            resolves to a constant declaration. A literal materialized from
            anything else keeps other selections, and is an ordinary literal of
            the type its context expects. The evaluated result of the call in
            `const C: [u64; SIZE] = make();` is one: it retains the callee, and
            `SIZE` at the extent's span, but no name of its own. An occurrence
            whose selection is missing, or one at the literal's span that is not
            yet resolved, cannot be ruled out, so it still counts as a possible
            constant name and keeps the stricter reading.
        */
        fn may_name_declared_constant(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
        ) -> bool {
            let source_span = expression_table(program).source_span(expression);
            expression_table(program)
                .authored_selection_occurrences(expression)
                .any(|occurrence| {
                    program
                        .authored_declaration_selections()
                        .get(occurrence)
                        .is_none_or(|selection| {
                            selection.source_span() == source_span
                                && selection.target().as_resolved().is_none_or(|selected| {
                                    program
                                        .const_declaration_by_symbol(selected.selected_symbol())
                                        .is_some()
                                })
                        })
                })
        }

        pub fn closed_constant_array_elements(
            program: &dyn crate::CheckingInput,
            machine: symbols::SymbolHandle,
            expression: ExpressionHandle,
            expected: TypeReferenceHandle,
        ) -> Option<Vec<(ExpressionHandle, PrimitiveType)>> {
            if !scalar_array_preserves_memberships(program, expected) {
                return None;
            }
            let mut root = expression;
            let mut indices = Vec::new();
            let mut projections = Vec::new();
            while let Some(indexed) = expression_table(program).indexed_node(root) {
                if projections.contains(&root) {
                    return None;
                }
                projections.push(root);
                indices.push(indexed.index);
                root = indexed.collection;
            }
            let mut reference = declared_constant_array_type(program, root).or_else(|| {
                (indices.is_empty() && !may_name_declared_constant(program, root))
                    .then_some(expected)
            })?;
            if !is_closed_primitive_array_type(program, reference) {
                return None;
            }
            closed_literal_array_elements(program, root, reference)?;
            if !indices.is_empty() {
                let projected =
                    builtin_constant_array_projection_type(program, machine, expression)?;
                if program.normalized_type_identity(projected)
                    != program.normalized_type_identity(expected)
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
                let literal = expression_table(program).integer_node(index)?;
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
                let elements = expression_table(program).array_literal_node(selected)?;
                selected = *expression_table(program)
                    .expression_handles(*elements)
                    .get(index)?;
                reference = *element_type;
            }
            if program.normalized_type_identity(reference)
                != program.normalized_type_identity(expected)
            {
                return None;
            }
            closed_literal_array_elements(program, selected, reference)
        }

        pub enum ScalarArrayElement {
            Authored {
                expression: ExpressionHandle,
                primitive_type: PrimitiveType,
            },
            Projected {
                row: ExpressionHandle,
                indices: Vec<u32>,
                leaf_type: TypeReferenceHandle,
                primitive_type: PrimitiveType,
            },
        }

        pub struct ScalarArrayElements {
            pub elements: Vec<ScalarArrayElement>,
            pub projections: Vec<ExpressionHandle>,
        }

        impl ScalarArrayElements {
            pub fn all_authored(&self) -> bool {
                self.elements
                    .iter()
                    .all(|element| matches!(element, ScalarArrayElement::Authored { .. }))
            }
        }

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
            let mut pending = vec![(expression, expected, false, false)];
            while let Some((expression, reference, exiting, is_row)) = pending.pop() {
                if exiting {
                    active.pop();
                    continue;
                }
                if active.contains(&expression)
                    || !expression_table(program).expression_is_valid(expression)
                {
                    return None;
                }
                active.push(expression);
                pending.push((expression, reference, true, is_row));
                let peeled = array_carrier_type(program, reference);
                match program.type_reference_table.type_reference(peeled) {
                    TypeReferenceNode::FixedArray {
                        element_type,
                        length: typed_trees::types::FixedArrayLength::Literal(length),
                    } => {
                        let indexed_row = matches!(
                            expression_table(program).expression(expression),
                            ExpressionNode::Indexed(_)
                        );
                        if indexed_row || may_name_declared_constant(program, expression) {
                            let Some(closed) = closed_constant_array_elements(
                                program, machine, expression, reference,
                            ) else {
                                if is_row
                                    && (indexed_row
                                        || matches!(
                                            expression_table(program).expression(expression),
                                            ExpressionNode::Member(_)
                                        ))
                                {
                                    projected_row_leaves(
                                        program,
                                        expression,
                                        reference,
                                        &mut Vec::new(),
                                        &mut leaves,
                                    )?;
                                    let mut selected = expression;
                                    let mut selected_projections = Vec::new();
                                    while let Some(indexed) =
                                        expression_table(program).indexed_node(selected)
                                    {
                                        selected_projections.push(selected);
                                        selected = indexed.collection;
                                    }
                                    projections.extend(selected_projections.into_iter().rev());
                                    continue;
                                }
                                return None;
                            };
                            leaves.extend(closed.into_iter().map(
                                |(expression, primitive_type)| ScalarArrayElement::Authored {
                                    expression,
                                    primitive_type,
                                },
                            ));
                            let mut selected = expression;
                            let mut selected_projections = Vec::new();
                            while let Some(indexed) =
                                expression_table(program).indexed_node(selected)
                            {
                                selected_projections.push(selected);
                                selected = indexed.collection;
                            }
                            projections.extend(selected_projections.into_iter().rev());
                            continue;
                        }
                        match expression_table(program).array_literal_node(expression) {
                            Some(elements) => {
                                if !extend_fixed_array_elements(
                                    program,
                                    &mut pending,
                                    elements,
                                    element_type,
                                    *length,
                                    true,
                                ) {
                                    return None;
                                }
                            }
                            None if is_row => {
                                projected_row_leaves(
                                    program,
                                    expression,
                                    reference,
                                    &mut Vec::new(),
                                    &mut leaves,
                                )?;
                            }
                            None => return None,
                        }
                    }
                    TypeReferenceNode::Named { .. } => leaves.push(ScalarArrayElement::Authored {
                        expression,
                        primitive_type: program.primitive_type_reference(reference)?,
                    }),
                    _ => return None,
                }
            }
            Some(ScalarArrayElements {
                elements: leaves,
                projections,
            })
        }

        pub fn closed_literal_array_elements(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            reference: TypeReferenceHandle,
        ) -> Option<Vec<(ExpressionHandle, PrimitiveType)>> {
            let mut leaves = Vec::new();
            let mut pending = vec![(expression, reference, false, false)];
            let mut active = Vec::new();
            while let Some((expression, reference, exiting, _)) = pending.pop() {
                if exiting {
                    active.pop();
                    continue;
                }
                if active.contains(&expression) {
                    return None;
                }
                active.push(expression);
                pending.push((expression, reference, true, false));
                match (
                    expression_table(program).expression(expression),
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
                            false,
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
                                    != PrimitiveType::from_name(landing.landed_type.name())
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
                        == Some(PrimitiveType::Bool) =>
                    {
                        leaves.push((expression, PrimitiveType::Bool))
                    }
                    (
                        ExpressionNode::Float(literal),
                        TypeReferenceNode::Named { .. } | TypeReferenceNode::Constrained { .. },
                    ) => {
                        let primitive = program.primitive_type_reference(reference)?;
                        if !matches!(
                            (primitive, literal.landing()),
                            (PrimitiveType::F32, Some(FloatFormat::F32))
                                | (PrimitiveType::F64, Some(FloatFormat::F64))
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

        pub fn declared_constant_array_type(
            program: &dyn crate::CheckingInput,
            value: ExpressionHandle,
        ) -> Option<TypeReferenceHandle> {
            if !matches!(
                expression_table(program).expression(value),
                ExpressionNode::ArrayLiteral(_)
            ) {
                return None;
            }
            let declaration = program.expression_const_declaration(value)?;
            matches!(
                program
                    .type_reference_table
                    .type_reference(array_carrier_type(program, declaration.declared_type)),
                TypeReferenceNode::FixedArray { .. }
            )
            .then_some(declaration.declared_type)
        }

        pub fn builtin_constant_array_projection_type(
            program: &dyn crate::CheckingInput,
            machine_symbol: symbols::SymbolHandle,
            mut expression: ExpressionHandle,
        ) -> Option<TypeReferenceHandle> {
            let mut projections = Vec::new();
            while let Some(indexed) = expression_table(program).indexed_node(expression) {
                if !matches!(
                    expression_table(program).expression(indexed.index),
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
                let operands = [Some(reference), None];
                if !typed_trees::operator::resolve_indexed_spelling_for_operands(
                    program,
                    language_core::OperatorSpelling::Index,
                    &operands,
                    expression_table(program).source_span(expression),
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

        pub(crate) fn validate_declared_array_destination(
            program: &dyn crate::CheckingInput,
            mut value: ExpressionHandle,
            destination: TypeReferenceHandle,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            value = expression_table(program).peeled_expression_handle(value);
            if !matches!(
                expression_table(program).expression(value),
                ExpressionNode::ArrayLiteral(_)
            ) {
                return;
            }
            let Some(destination) =
                crate::value_custody::places::unwrapped_type_reference(program, destination)
            else {
                return;
            };
            for occurrence in expression_table(program).authored_selection_occurrences(value) {
                let Some(selection) = program.authored_declaration_selections().get(occurrence)
                else {
                    continue;
                };
                let target = selection.target();
                let Some(selected) = target.as_resolved() else {
                    continue;
                };
                let Some(declaration) =
                    program.const_declaration_by_symbol(selected.selected_symbol())
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

        pub(crate) fn validate_constant_projection_destination(
            program: &dyn crate::CheckingInput,
            machine_symbol: symbols::SymbolHandle,
            value: ExpressionHandle,
            destination: TypeReferenceHandle,
            diagnostics: &mut Vec<Diagnostic>,
        ) -> bool {
            let mut pending = vec![(value, destination)];
            let mut conflict = false;
            while let Some((value, destination)) = pending.pop() {
                let destination = program
                    .type_reference_table
                    .unconstrained_type_reference(destination);
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
                    builtin_constant_array_projection_type(program, machine_symbol, value).or_else(
                        || {
                            super::record_projection::closed_scalar_projection(program, value)
                                .map(|(_, reference)| reference)
                        },
                    )
                {
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
                if let ExpressionNode::ArrayLiteral(elements) =
                    expression_table(program).expression(value)
                    && let TypeReferenceNode::FixedArray { element_type, .. } =
                        program.type_reference_table.type_reference(destination)
                {
                    pending.extend(
                        expression_table(program)
                            .expression_handles(*elements)
                            .iter()
                            .map(|element| (*element, *element_type)),
                    );
                }
            }
            conflict
        }

        fn projected_row_leaves(
            program: &dyn crate::CheckingInput,
            row: ExpressionHandle,
            reference: TypeReferenceHandle,
            indices: &mut Vec<u32>,
            leaves: &mut Vec<ScalarArrayElement>,
        ) -> Option<()> {
            match program
                .type_reference_table
                .type_reference(array_carrier_type(program, reference))
            {
                TypeReferenceNode::FixedArray {
                    element_type,
                    length: typed_trees::types::FixedArrayLength::Literal(length),
                } => {
                    for index in 0..*length {
                        indices.push(u32::try_from(index).ok()?);
                        projected_row_leaves(program, row, *element_type, indices, leaves)?;
                        indices.pop();
                    }
                    Some(())
                }
                TypeReferenceNode::Named { .. } => {
                    let primitive_type = program.primitive_type_reference(reference)?;
                    leaves.push(ScalarArrayElement::Projected {
                        row,
                        indices: indices.clone(),
                        leaf_type: reference,
                        primitive_type,
                    });
                    Some(())
                }
                _ => None,
            }
        }

        fn extend_fixed_array_elements(
            program: &dyn crate::CheckingInput,
            pending: &mut Vec<(ExpressionHandle, TypeReferenceHandle, bool, bool)>,
            elements: &arena::HandleSpan<ExpressionHandle>,
            element_type: &TypeReferenceHandle,
            length: usize,
            is_row: bool,
        ) -> bool {
            let elements = expression_table(program).expression_handles(*elements);
            if elements.len() != length {
                return false;
            }
            pending.extend(
                elements
                    .iter()
                    .rev()
                    .map(|element| (*element, *element_type, false, is_row)),
            );
            true
        }

        fn same_array_carrier(
            program: &dyn crate::CheckingInput,
            mut declared: TypeReferenceHandle,
            mut destination: TypeReferenceHandle,
        ) -> bool {
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
    }
    pub(crate) mod record_projection {
        use crate::frozen_program::expression_table;
        use numerics::arithmetic::ArithmeticDomain;
        use symbols::SymbolKind;
        use typed_trees::data::{DataDefinition, DataField, DataMember};
        use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableStructLiteral};
        use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

        pub fn closed_record_scalar_projection(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
        ) -> Option<(ExpressionHandle, PrimitiveType)> {
            let (selected, selected_type) = closed_scalar_projection(program, expression)?;
            Some((selected, program.primitive_type_reference(selected_type)?))
        }

        pub fn closed_scalar_place_projection(
            program: &dyn crate::CheckingInput,
            value: ExpressionHandle,
            reference: TypeReferenceHandle,
            path: &[facts::PlaceSegment],
        ) -> Option<(ExpressionHandle, TypeReferenceHandle)> {
            if !closed_value(program, value, reference, &mut Vec::new()) {
                return None;
            }
            let mut selected = value;
            let mut selected_type = reference;
            for segment in path {
                match segment {
                    facts::PlaceSegment::Field { symbol } => {
                        let (literal, definition) = record_constructor(program, selected)?;
                        let fields = constructor_fields(program, literal, definition)?;
                        let (field, _) = fields
                            .into_iter()
                            .find(|(field, _)| field.symbol == *symbol)?;
                        let actual = expression_table(program)
                            .struct_fields(literal.fields)
                            .iter()
                            .find(|actual| actual.field_symbol == field.symbol)?;
                        selected = actual.value;
                        selected_type = field.type_reference;
                    }
                    facts::PlaceSegment::Case { variant } => {
                        let (literal, _) = record_constructor(program, selected)?;
                        if literal.case_symbol != Some(*variant) {
                            return None;
                        }
                    }
                    facts::PlaceSegment::FixedIndex { index } => {
                        let TypeReferenceNode::FixedArray {
                            element_type,
                            length: typed_trees::types::FixedArrayLength::Literal(length),
                        } = program.type_reference_table.type_reference(selected_type)
                        else {
                            return None;
                        };
                        if index >= length {
                            return None;
                        }
                        let elements = expression_table(program).array_literal_node(selected)?;
                        selected = *expression_table(program)
                            .expression_handles(*elements)
                            .get(*index)?;
                        selected_type = *element_type;
                    }
                    _ => return None,
                }
            }
            program.primitive_type_reference(selected_type)?;
            Some((selected, selected_type))
        }

        pub(crate) fn closed_record_integer_projection(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
        ) -> Option<typed_trees::closed_numeric::ClosedIntegerValue> {
            let (selected, reference) = closed_scalar_projection(program, expression)?;
            let mut carrier = reference;
            for _ in 0..program.type_reference_table.type_reference_count() {
                match program.type_reference_table.type_reference(carrier) {
                    TypeReferenceNode::Constrained { base_type, .. } => carrier = *base_type,
                    _ => break,
                }
            }
            let symbol = &program
                .type_reference_table
                .type_reference(carrier)
                .as_named_symbol()?;
            program.symbols.builtin_type_atom(*symbol)?;
            let primitive = program.primitive_type_reference(reference)?;
            let value =
                program.closed_integer_value_in(selected, symbols::SymbolHandle::invalid())?;
            if value.primitive.is_some_and(|landed| landed != primitive) {
                return None;
            }
            typed_trees::closed_numeric::land_integer(&value.value, primitive)?;
            Some(typed_trees::closed_numeric::ClosedIntegerValue {
                value: value.value,
                primitive: Some(primitive),
                type_reference: Some(reference),
            })
        }

        pub(crate) fn closed_scalar_projection(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
        ) -> Option<(ExpressionHandle, TypeReferenceHandle)> {
            let mut projections = Vec::new();
            let mut root = expression;
            let mut has_field = false;
            loop {
                if projections.len() >= expression_table(program).expression_count() {
                    return None;
                }
                let receiver = match expression_table(program).expression(root) {
                    ExpressionNode::Member(member) if member.member_symbol.is_valid() => {
                        has_field = true;
                        member.receiver
                    }
                    ExpressionNode::Indexed(indexed) => indexed.collection,
                    _ => break,
                };
                projections.push(root);
                root = receiver;
            }
            if projections.is_empty() || !has_field {
                return None;
            }
            let root_type = super::declared_constant_array_type(program, root);
            let closed = match root_type {
                Some(reference) => closed_value(program, root, reference, &mut Vec::new()),
                None => closed_record(program, root, &mut Vec::new()),
            };
            if !closed {
                return None;
            }
            let mut selected = root;
            let mut selected_type = root_type.unwrap_or_default();
            for projection in projections.into_iter().rev() {
                match expression_table(program).expression(projection) {
                    ExpressionNode::Member(member) => {
                        let (literal, definition) = record_constructor(program, selected)?;
                        if member.case_variant.is_some() && member.case_variant != literal.case_name
                        {
                            return None;
                        }
                        let field = crate::value_custody::places::exact_data_member_field(
                            program,
                            definition,
                            member.member_symbol,
                            member.member.as_str(),
                            member.case_variant.as_ref().map(|variant| variant.as_str()),
                        )?;
                        let actual = expression_table(program)
                            .struct_fields(literal.fields)
                            .iter()
                            .find(|actual| actual.field_symbol == field.symbol)?;
                        selected = actual.value;
                        selected_type = field.type_reference;
                    }
                    ExpressionNode::Indexed(indexed) => {
                        let TypeReferenceNode::FixedArray {
                            element_type,
                            length: typed_trees::types::FixedArrayLength::Literal(length),
                        } = program.type_reference_table.type_reference(selected_type)
                        else {
                            return None;
                        };
                        let elements = expression_table(program).array_literal_node(selected)?;
                        let ordinal = expression_table(program).integer_node(indexed.index)?;
                        if ordinal.landing().is_some_and(|landing| {
                            landing.domain != ArithmeticDomain::Exact
                                || landing.landed_type
                                    == numerics::literals::LandedIntegerType::Addr
                        }) || !builtin_index(program, projection, selected_type)
                        {
                            return None;
                        }
                        let ordinal = usize::try_from(ordinal.value_u64()?).ok()?;
                        if ordinal >= *length {
                            return None;
                        }
                        selected = *expression_table(program)
                            .expression_handles(*elements)
                            .get(ordinal)?;
                        selected_type = *element_type;
                    }
                    _ => return None,
                }
            }
            program.primitive_type_reference(selected_type)?;
            if program.arithmetic_domain_for_type_reference(selected_type)
                != ArithmeticDomain::Exact
            {
                return None;
            }
            Some((selected, selected_type))
        }

        fn builtin_index(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            collection: TypeReferenceHandle,
        ) -> bool {
            let spelling = language_core::OperatorSpelling::Index;
            let operands = [Some(collection), None];
            typed_trees::operator::resolve_indexed_spelling_for_operands(
                program,
                spelling,
                &operands,
                expression_table(program).source_span(expression),
            )
            .is_empty()
                && program
                    .machine_specializations
                    .iter()
                    .all(|specialization| {
                        typed_trees::operator::selected_trait_operator_meanings(
                            program,
                            specialization.instance,
                            spelling,
                            &operands,
                        )
                        .is_empty()
                    })
                && typed_trees::operator::has_builtin_spelled_expression_meaning(
                    program,
                    symbols::SymbolHandle::invalid(),
                    expression,
                    spelling,
                    &operands,
                )
        }

        fn record_constructor(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
        ) -> Option<(&TableStructLiteral, &DataDefinition)> {
            if !expression_table(program).expression_is_valid(expression) {
                return None;
            }
            let literal = expression_table(program).struct_literal_node(expression)?;
            if !literal.type_symbol.is_valid()
                || program.symbols.get(literal.type_symbol).kind != SymbolKind::Data
            {
                return None;
            }
            let definition = program.data_definition_by_symbol(literal.type_symbol)?;
            if !program.data_type_parameters(definition).is_empty()
                || definition.quotient.is_some()
                || definition.cleanup_selection.is_some()
                || definition.properties.multiplicity == language_semantics::Multiplicity::Linear
            {
                return None;
            }
            Some((literal, definition))
        }

        fn constructor_fields<'program>(
            program: &'program dyn crate::CheckingInput,
            literal: &TableStructLiteral,
            definition: &'program DataDefinition,
        ) -> Option<Vec<(&'program DataField, symbols::SymbolHandle)>> {
            let mut fields = Vec::new();
            let mut selected = false;
            let mut has_cases = false;
            let members = program.data_members(definition);
            if members.len() != definition.members.count() as usize {
                return None;
            }
            for member in members {
                match member {
                    DataMember::Field(field) => fields.push((field, definition.symbol)),
                    DataMember::Variant(variant) => {
                        has_cases = true;
                        if literal.case_symbol == Some(variant.symbol)
                            && literal.case_name.as_ref() == Some(&variant.name)
                        {
                            if selected {
                                return None;
                            }
                            selected = true;
                            fields.extend(
                                program
                                    .data_payload_fields(variant)
                                    .iter()
                                    .map(|field| (field, variant.symbol)),
                            );
                        }
                    }
                }
            }
            if has_cases != selected
                || (!has_cases && (literal.case_name.is_some() || literal.case_symbol.is_some()))
            {
                return None;
            }
            Some(fields)
        }

        fn closed_record(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            active: &mut Vec<ExpressionHandle>,
        ) -> bool {
            if active.contains(&expression) {
                return false;
            }
            let Some((literal, definition)) = record_constructor(program, expression) else {
                return false;
            };
            let actuals = expression_table(program).struct_fields(literal.fields);
            let Some(fields) = constructor_fields(program, literal, definition) else {
                return false;
            };
            if actuals.len() != fields.len() || actuals.len() != literal.fields.count() as usize {
                return false;
            }
            active.push(expression);
            let valid = fields.iter().enumerate().all(|(ordinal, (field, owner))| {
                if !field.symbol.is_valid()
                    || program.symbols.get(field.symbol).kind != SymbolKind::Field
                    || program.symbols.get(field.symbol).parent != *owner
                    || field.relevance.is_erased()
                    || fields[..ordinal]
                        .iter()
                        .any(|(prior, _)| prior.symbol == field.symbol || prior.name == field.name)
                {
                    return false;
                }
                let mut matching = actuals.iter().filter(|actual| {
                    actual.field_symbol == field.symbol && actual.name == field.name
                });
                let Some(actual) = matching.next() else {
                    return false;
                };
                if matching.next().is_some()
                    || !expression_table(program).expression_is_valid(actual.value)
                {
                    return false;
                }
                closed_value(program, actual.value, field.type_reference, active)
            });
            active.pop();
            valid
        }

        fn closed_value(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            reference: TypeReferenceHandle,
            active: &mut Vec<ExpressionHandle>,
        ) -> bool {
            if active.contains(&expression)
                || !expression_table(program).expression_is_valid(expression)
            {
                return false;
            }
            match (
                expression_table(program).expression(expression),
                program.type_reference_table.type_reference(reference),
            ) {
                (
                    ExpressionNode::StructLiteral(literal),
                    TypeReferenceNode::Named { symbol, .. },
                ) => *symbol == literal.type_symbol && closed_record(program, expression, active),
                (
                    ExpressionNode::ArrayLiteral(elements),
                    TypeReferenceNode::FixedArray {
                        element_type,
                        length: typed_trees::types::FixedArrayLength::Literal(length),
                    },
                ) => {
                    let actuals = expression_table(program).expression_handles(*elements);
                    if actuals.len() != *length || actuals.len() != elements.count() as usize {
                        return false;
                    }
                    active.push(expression);
                    let valid = actuals
                        .iter()
                        .all(|actual| closed_value(program, *actual, *element_type, active));
                    active.pop();
                    valid
                }
                _ => super::closed_literal_array_elements(program, expression, reference).is_some(),
            }
        }
    }
    pub use constant_arrays::builtin_constant_array_projection_type;
    pub use constant_arrays::declared_constant_array_type;
    pub(crate) use constant_arrays::validate_constant_projection_destination;
    pub use constant_arrays::{
        ScalarArrayElement, ScalarArrayElements, closed_array_store_type,
        closed_constant_array_elements, closed_literal_array_elements,
        is_closed_primitive_array_type, peel_arithmetic_policy, scalar_array_elements,
        scalar_array_preserves_memberships,
    };
    pub(crate) use record_projection::closed_record_integer_projection;
    pub use record_projection::{closed_record_scalar_projection, closed_scalar_place_projection};
    pub(crate) mod integer_landing {
        use diagnostics::Diagnostic;
        use numerics::{
            arithmetic::ArithmeticDomain,
            bignum::{BigInt, BigRational},
            literals::{IntegerLanding, IntegerLiteral, IntegerRadix},
        };
        use typed_trees::{
            expression::{BinaryOperator, ExpressionHandle, ExpressionNode},
            types::PrimitiveType,
        };

        pub(crate) mod destinations {
            use super::{
                BinaryOperator, ExpressionHandle, ExpressionNode, PrimitiveType,
                anonymous_numeric_value, has_anonymous_operator_meaning, implicit_landing_error,
                land_integer_value,
            };
            use crate::CheckingInput;
            use crate::frozen_program::expression_table;

            use diagnostics::Diagnostic;
            use typed_trees::statement::{StatementNode, TransitionTargetNode};
            use typed_trees::types::TypeReferenceHandle;

            pub(crate) mod result_values {
                use typed_trees::expression::{ExpressionHandle, ExpressionNode};
                use typed_trees::machine::Machine;
                use typed_trees::state::State;
                use typed_trees::types::TypeReferenceHandle;

                pub(crate) fn visit_assignment_value(
                    program: &dyn crate::CheckingInput,
                    machine: &Machine,
                    state: &State,
                    target: ExpressionHandle,
                    value: ExpressionHandle,
                    visit: &mut impl FnMut(TypeReferenceHandle, ExpressionHandle),
                ) {
                    if let Some(indexed) = program.expression_table.indexed_node(target)
                        && matches!(
                            program.expression_table.expression(indexed.index),
                            ExpressionNode::Range(_)
                        )
                        && let Some(collection_type) =
                            crate::value_custody::places::declared_place_type(
                                program,
                                machine,
                                Some(state),
                                indexed.collection,
                            )
                        && let Some((element_type, _)) =
                            crate::value_custody::type_shapes::collection_element_and_length(
                                program,
                                collection_type,
                            )
                        && let Some(elements) = program.expression_table.array_literal_node(value)
                    {
                        for element in program.expression_table.expression_handles(*elements) {
                            visit(element_type, *element);
                        }
                        return;
                    }
                    if let Some(destination) =
                        crate::value_custody::places::declared_place_type_raw(
                            program,
                            machine,
                            Some(state),
                            target,
                        )
                        .or_else(|| {
                            crate::value_custody::places::declared_indexed_projection_type_raw(
                                program,
                                machine,
                                Some(state),
                                target,
                            )
                        })
                    {
                        visit(
                            crate::value_custody::places::assignment_value_type(
                                program,
                                destination,
                            ),
                            value,
                        );
                    }
                }

                pub(crate) fn visit_result_values(
                    program: &dyn crate::CheckingInput,
                    machine: &Machine,
                    state: &State,
                    destination: TypeReferenceHandle,
                    expression: ExpressionHandle,
                    visit: &mut impl FnMut(TypeReferenceHandle, ExpressionHandle),
                ) {
                    visit_result_values_inner(
                        program,
                        machine,
                        state,
                        destination,
                        expression,
                        visit,
                        &mut Vec::new(),
                    );
                }

                fn visit_result_values_inner(
                    program: &dyn crate::CheckingInput,
                    machine: &Machine,
                    state: &State,
                    destination: TypeReferenceHandle,
                    expression: ExpressionHandle,
                    visit: &mut impl FnMut(TypeReferenceHandle, ExpressionHandle),
                    active: &mut Vec<ExpressionHandle>,
                ) {
                    use typed_trees::types::TypeReferenceNode;
                    if !program.expression_table.expression_is_valid(expression)
                        || active.contains(&expression)
                    {
                        return;
                    }
                    if let Some(dispatch) = program.expression_table.match_node(expression) {
                        let destination =
                            match crate::value_custody::expression_types::expression_result_type_reference(
                program, machine, state, expression,
                            ) {
                Some(retained) => retained,
                None if matches!(
                    program.type_reference_table.type_reference(
                        crate::value_custody::places::peel_declared_constraints(
                            program,
                            destination
                        ),
                    ),
                    TypeReferenceNode::FixedArray { .. }
                ) || crate::value_custody::literals::has_anonymous_numeric_results(
                    program, expression,
                ) =>
                {
                    destination
                }
                None => return,
                            };
                        active.push(expression);
                        for arm in program.expression_table.match_arms(dispatch.arms) {
                            visit_result_values_inner(
                                program,
                                machine,
                                state,
                                destination,
                                arm.value,
                                visit,
                                active,
                            );
                        }
                        active.pop();
                    } else if let ExpressionNode::ArrayLiteral(elements) =
                        program.expression_table.expression(expression)
                        && let TypeReferenceNode::FixedArray { element_type, .. } =
                            program.type_reference_table.type_reference(
                                crate::value_custody::places::peel_declared_constraints(
                                    program,
                                    destination,
                                ),
                            )
                    {
                        active.push(expression);
                        for element in program.expression_table.expression_handles(*elements) {
                            visit_result_values_inner(
                                program,
                                machine,
                                state,
                                *element_type,
                                *element,
                                visit,
                                active,
                            );
                        }
                        active.pop();
                    } else {
                        visit(destination, expression);
                    }
                }
            }

            /* Explicit integer landing (numeric_values.md#fractional-intermediate-diagnostics):
            an anonymous calculation whose exact history holds a fractional
            intermediate may land in an integer only through `as` around the
            complete anonymous expression. Every other integer destination this
            visitor reaches -- locals, fields, parameters, returns, transfers,
            match joins and typed-operation peers -- is an implicit landing and
            rejects. A proof-level `Int` peer has no cast spelling, so it keeps
            the warning until OWNER_QUESTIONS.md settles its acknowledgement
            form ("Explicit landing at a proof-level Int peer"). A cast acknowledges exactly its own
            operand (and the arms that operand joins), never a sibling or an
            enclosing call argument; final integrality, range and division by
            zero are still decided by the cast's ordinary landing check. */
            pub(crate) fn anonymous_integer_landing_errors(
                program: &dyn CheckingInput,
            ) -> Vec<Diagnostic> {
                let has_potential_fractional_origin = expression_table(program)
                    .iter_expressions()
                    .any(|(_, node)| match node {
                        ExpressionNode::Float(literal) => literal.landing().is_none(),
                        ExpressionNode::Binary(binary) => binary.operator == BinaryOperator::Divide,
                        _ => false,
                    });
                if !has_potential_fractional_origin {
                    return Vec::new();
                }
                let mut warnings = Vec::new();
                let mut warned = explicitly_landed_operands(program);
                visit_destination_trees(program, |destination, expression| {
                    if let Some(primitive) = program.primitive_type_reference(destination) {
                        append_integer_landing_warning(
                            program,
                            Some(primitive),
                            expression,
                            &mut warned,
                            &mut warnings,
                        );
                    }
                });
                for (expression, node) in expression_table(program).iter_expressions() {
                    let ExpressionNode::Binary(binary) = node else {
                        continue;
                    };
                    let has_candidate_operand =
                        [binary.left, binary.right].into_iter().any(|operand| {
                            matches!(
                                expression_table(program).expression(operand),
                                ExpressionNode::Float(_)
                                    | ExpressionNode::Binary(_)
                                    | ExpressionNode::Match(_)
                            )
                        });
                    if has_candidate_operand && crate::proof_integer_expression(program, expression)
                    {
                        for operand in [binary.left, binary.right] {
                            append_integer_landing_warning(
                                program,
                                None,
                                operand,
                                &mut warned,
                                &mut warnings,
                            );
                        }
                    }
                }
                warnings
            }

            /* The operands an ordinary integer cast lands explicitly. A Match
            operand contributes the arm results it joins, because the cast is
            their landing too; nothing below an arithmetic operand is added, so
            a typed operation inside the cast keeps its own implicit landing. */
            fn explicitly_landed_operands(program: &dyn CheckingInput) -> Vec<ExpressionHandle> {
                let mut acknowledged = Vec::new();
                for (_, node) in expression_table(program).iter_expressions() {
                    let ExpressionNode::Cast(cast) = node else {
                        continue;
                    };
                    if cast.form.is_recast()
                        || !cast.semantic_domain.is_empty()
                        || !program
                            .primitive_type_reference(cast.target_type)
                            .is_some_and(|primitive| primitive.is_integer())
                    {
                        continue;
                    }
                    let mut pending = vec![cast.value];
                    while let Some(expression) = pending.pop() {
                        if acknowledged.contains(&expression) {
                            continue;
                        }
                        acknowledged.push(expression);
                        if let Some(dispatch) = program.expression_table.match_node(expression) {
                            pending.extend(
                                program
                                    .expression_table
                                    .match_arms(dispatch.arms)
                                    .iter()
                                    .map(|arm| arm.value),
                            );
                        }
                    }
                }
                acknowledged
            }

            fn append_integer_landing_warning(
                program: &dyn CheckingInput,
                primitive: Option<PrimitiveType>,
                expression: ExpressionHandle,
                warned: &mut Vec<ExpressionHandle>,
                warnings: &mut Vec<Diagnostic>,
            ) {
                if !matches!(
                    expression_table(program).expression(expression),
                    ExpressionNode::Float(_) | ExpressionNode::Binary(_) | ExpressionNode::Match(_)
                ) || warned.contains(&expression)
                {
                    return;
                }
                let mut builtin = |expression| has_anonymous_operator_meaning(program, expression);
                let Some(evaluated) = anonymous_numeric_value(program, expression, &mut builtin)
                else {
                    return;
                };
                if !evaluated.fractional_origin.is_valid() {
                    return;
                }
                let Some(integer) = evaluated.value.to_integer_exact() else {
                    return;
                };
                if primitive
                    .is_some_and(|primitive| land_integer_value(&integer, primitive).is_none())
                {
                    return;
                }
                let Some(warning) = implicit_landing_error(
                    program,
                    &evaluated,
                    &integer,
                    primitive,
                    &[],
                    &mut builtin,
                ) else {
                    return;
                };
                warnings.push(warning);
                warned.push(expression);
            }

            fn visit_destination_trees(
                program: &dyn CheckingInput,
                mut visit: impl FnMut(TypeReferenceHandle, ExpressionHandle),
            ) {
                for machine in program.machines() {
                    for state in program.machine_states(machine) {
                        let mut visit_result = |destination, expression| {
                            result_values::visit_result_values(
                                program,
                                machine,
                                state,
                                destination,
                                expression,
                                &mut visit,
                            );
                        };
                        for statement in program.statement_table.statements(state.statement_nodes) {
                            match statement {
                                StatementNode::Expression(expression) => {
                                    visit_result(state.return_type, *expression);
                                }
                                StatementNode::Transition(transition) => {
                                    for target in transition.targets() {
                                        match program.statement_table.transition_target(target) {
                                TransitionTargetNode::Value(expression)
                                    if transition.exit
                                        == typed_trees::statement::TransitionExit::Ordinary =>
                                {
                                    visit_result(state.return_type, *expression);
                                }
                                TransitionTargetNode::Named {
                                    path,
                                    arguments,
                                    evidence_arguments,
                                    authored_call_selection,
                                    ..
                                } => {
                                    let arguments =
                                        program.statement_table.expression_handles(*arguments);
                                    if transition.exit == typed_trees::statement::TransitionExit::Ordinary
                                        && evidence_arguments.is_empty()
                                        && authored_call_selection.is_none_or(|occurrence| {
                                            use language_semantics::declaration_selection::{AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionTarget};
                                            program.authored_declaration_selections().get(occurrence).is_some_and(|selection| {
                                                selection.kind() == AuthoredDeclarationSelectionKind::Call
                                                    && matches!(selection.target(), AuthoredDeclarationSelectionTarget::Resolved(selected) if selected.selected_symbol() == path.symbol)
                                            })
                                        })
                                    {
                                        visit_call_arguments(program, path.symbol, arguments, &mut visit_result);
                                    }
                                }
                                _ => {}
                            }
                                    }
                                }
                                StatementNode::LocalData(local) => {
                                    visit_result(local.type_reference, local.initial_value);
                                }
                                StatementNode::Assignment(assignment) => {
                                    result_values::visit_assignment_value(
                                        program,
                                        machine,
                                        state,
                                        assignment.target,
                                        assignment.value,
                                        &mut visit_result,
                                    );
                                }
                                StatementNode::Call(call)
                                    if call.static_requirement_dispatch.is_none()
                                        && call.machine_arguments.is_empty()
                                        && call.evidence_arguments.is_empty() =>
                                {
                                    visit_call_arguments(
                                        program,
                                        call.target_symbol,
                                        program.statement_table.expression_handles(call.arguments),
                                        &mut visit_result,
                                    );
                                }
                                _ => {}
                            }
                        }
                        let mut pending = Vec::new();
                        for statement in program.statement_table.statements(state.statement_nodes) {
                            pending.extend(
                                crate::machine_calls::calls::statement_value_expression_roots(
                                    program, statement,
                                ),
                            );
                        }
                        crate::queries::expression_traversal::visit_pre_order(
                            program,
                            pending,
                            |expression, node| {
                                match node {
                        ExpressionNode::StructLiteral(literal) => {
                            for field in expression_table(program).struct_fields(literal.fields) {
                                if let Some(destination) = crate::value_custody::struct_literals::selected_construction_field_type(program, expression, literal, field.field_symbol) {
                                    visit_result(destination, field.value);
                                }
                            }
                        }
                        ExpressionNode::Call(call)
                            if call.static_requirement_dispatch.is_none()
                                && call.machine_arguments.is_empty()
                                && call.evidence_arguments.is_empty()
                                && call.quotient_operation.is_none()
                                && call.private_layout_operation.is_none() =>
                        {
                            visit_call_arguments(
                                program, call.target_symbol,
                                expression_table(program).expression_handles(call.arguments),
                                &mut visit_result,
                            );
                        }
                        ExpressionNode::Match(_) => {
                            if let Some(destination) = crate::value_custody::expression_types::expression_result_type_reference(program, machine, state, expression) {
                                visit_result(destination, expression);
                            }
                        }
                        ExpressionNode::Unary(unary)
                            if unary.operator == typed_trees::expression::UnaryOperator::BitwiseNot =>
                        {
                            if let Some(destination) = crate::value_custody::expression_types::expression_result_type_reference(program, machine, state, expression) {
                                visit_result(destination, unary.operand);
                            }
                        }
                        ExpressionNode::Cast(cast) if !cast.form.is_recast() && cast.semantic_domain.is_empty() => {
                            visit_result(cast.target_type, cast.value);
                        }
                        ExpressionNode::Binary(binary)
                            if !matches!(binary.operator, BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight)
                                && crate::proof_contracts::bound_expression_meaning::has_builtin_binary_expression_meaning(program, machine, Some(state), expression) =>
                        {
                            for (operand, peer) in [(binary.left, binary.right), (binary.right, binary.left)] {
                                if let Some(destination) = crate::value_custody::expression_types::expression_result_type_reference(program, machine, state, peer)
                                    .and_then(|reference| crate::value_custody::places::unwrapped_type_reference(program, reference))
                                {
                                    visit_result(destination, operand);
                                }
                            }
                        }
                        _ => {}
                    }
                                false
                            },
                        );
                    }
                }
            }

            fn visit_call_arguments(
                program: &dyn CheckingInput,
                target: symbols::SymbolHandle,
                arguments: &[ExpressionHandle],
                visit: &mut impl FnMut(TypeReferenceHandle, ExpressionHandle),
            ) {
                if let Some(destinations) =
                    crate::machine_calls::calls::signature_queries::call_argument_destinations(
                        program,
                        target,
                        arguments.len(),
                    )
                {
                    for (destination, argument) in destinations.into_iter().zip(arguments) {
                        visit(destination, *argument);
                    }
                }
            }
        }
        pub(crate) use destinations::anonymous_integer_landing_errors;

        /* Lands a complete anonymous expression. The returned diagnostic is
        the implicit-landing error for a fractional history; a caller that
        lands the operand of an explicit `as` discards it, every other caller
        must report it. */
        pub fn land_anonymous_integer_expression(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            destination: PrimitiveType,
            selected_arms: &[(ExpressionHandle, ExpressionHandle)],
            mut builtin: impl FnMut(ExpressionHandle) -> bool,
        ) -> Option<(IntegerLiteral, Option<Diagnostic>)> {
            let value =
                evaluate_anonymous_value::<true>(program, expression, selected_arms, &mut builtin)?;
            let integer = value.value.to_integer_exact()?;
            let literal = land_integer_value(&integer, destination)?;
            let error = implicit_landing_error(
                program,
                &value,
                &integer,
                Some(destination),
                selected_arms,
                &mut builtin,
            );
            Some((literal, error))
        }

        pub fn evaluate_anonymous_numeric_comparison(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            selected_arms: &[(ExpressionHandle, ExpressionHandle)],
            mut builtin: impl FnMut(ExpressionHandle) -> bool,
        ) -> Option<bool> {
            use std::cmp::Ordering;

            if !program.expression_table.expression_is_valid(expression) {
                return None;
            }
            let binary = program.expression_table.binary_node(expression)?;
            if !matches!(
                binary.operator,
                BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
            ) || !builtin(expression)
            {
                return None;
            }
            let left = evaluate_anonymous_value::<true>(
                program,
                binary.left,
                selected_arms,
                &mut builtin,
            )?;
            let right = evaluate_anonymous_value::<true>(
                program,
                binary.right,
                selected_arms,
                &mut builtin,
            )?;
            let ordering = left.value.cmp_value(&right.value);
            Some(match binary.operator {
                BinaryOperator::Equal => ordering == Ordering::Equal,
                BinaryOperator::NotEqual => ordering != Ordering::Equal,
                BinaryOperator::Less => ordering == Ordering::Less,
                BinaryOperator::LessOrEqual => ordering != Ordering::Greater,
                BinaryOperator::Greater => ordering == Ordering::Greater,
                BinaryOperator::GreaterOrEqual => ordering != Ordering::Less,
                _ => return None,
            })
        }

        pub(crate) fn evaluate_anonymous_numeric_equality(
            program: &dyn crate::CheckingInput,
            left: ExpressionHandle,
            right: ExpressionHandle,
            mut builtin: impl FnMut(ExpressionHandle) -> bool,
        ) -> Option<bool> {
            let left = anonymous_numeric_value(program, left, &mut builtin)?;
            let right = anonymous_numeric_value(program, right, &mut builtin)?;
            Some(left.value.cmp_value(&right.value).is_eq())
        }

        pub fn evaluate_anonymous_numeric_expression(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            selected_arms: &[(ExpressionHandle, ExpressionHandle)],
            mut builtin: impl FnMut(ExpressionHandle) -> bool,
        ) -> Option<BigRational> {
            evaluate_anonymous_value::<true>(program, expression, selected_arms, &mut builtin)
                .map(|evaluated| evaluated.value)
        }

        pub fn select_anonymous_numeric_match_arm(
            program: &dyn crate::CheckingInput,
            dispatch: &typed_trees::expression::TableMatchExpression,
            mut builtin: impl FnMut(ExpressionHandle) -> bool,
        ) -> Option<ExpressionHandle> {
            let subject = anonymous_numeric_value(program, dispatch.subject, &mut builtin)?;
            let arms = program.expression_table.match_arms(dispatch.arms);
            if arms.len() != dispatch.arms.len() {
                return None;
            }
            for arm in arms {
                match arm.pattern {
                    typed_trees::expression::MatchPattern::Destructure(_) => return None,
                    typed_trees::expression::MatchPattern::Wildcard => return Some(arm.value),
                    typed_trees::expression::MatchPattern::Value(pattern) => {
                        let pattern = anonymous_numeric_value(program, pattern, &mut builtin)?;
                        if subject.value.cmp_value(&pattern.value).is_eq() {
                            return Some(arm.value);
                        }
                    }
                }
            }
            None
        }

        fn implicit_landing_error(
            program: &dyn crate::CheckingInput,
            evaluated: &AnonymousNumericValue,
            integer: &BigInt,
            destination: Option<PrimitiveType>,
            selected_arms: &[(ExpressionHandle, ExpressionHandle)],
            builtin: &mut impl FnMut(ExpressionHandle) -> bool,
        ) -> Option<Diagnostic> {
            if !evaluated.fractional_origin.is_valid() {
                return None;
            }
            let fractional = evaluate_anonymous_value::<true>(
                program,
                evaluated.fractional_origin,
                selected_arms,
                builtin,
            )?;
            let span = program
                .expression_table
                .source_span(evaluated.fractional_origin);
            let Some(destination) = destination else {
                // A proof-level `Int` peer: see anonymous_integer_landing_errors.
                return Some(Diagnostic::warning(format!(
                    "anonymous arithmetic preserves the exact fractional intermediate `{}` before landing as proof integer `{integer}`",
                    fractional.value,
                )).with_source_span(span));
            };
            Some(Diagnostic::error(format!(
                "anonymous arithmetic with the exact fractional intermediate `{}` cannot land implicitly as integer `{integer}` in `{}`; cast the complete anonymous expression with `as` to land its exact value, or type an operand if typed integer division was intended",
                fractional.value,
                destination.name(),
            )).with_source_span(span))
        }

        pub fn land_integer_value(
            value: &BigInt,
            destination: PrimitiveType,
        ) -> Option<IntegerLiteral> {
            let landed_type = destination.landed_landing_fixed()?;
            let width = usize::try_from(landed_type.bit_width()).ok()?;
            let bound = BigInt::from_u64(1).shl_bits(width - usize::from(landed_type.is_signed()));
            let minimum = if landed_type.is_signed() {
                bound.negate()
            } else {
                BigInt::zero()
            };
            let maximum = bound.sub(&BigInt::from_u64(1));
            if value < &minimum || value > &maximum {
                return None;
            }
            Some(
                IntegerLiteral::from_parts(
                    value.is_negative(),
                    IntegerRadix::Decimal,
                    &value.abs().to_string(),
                )
                .ok()?
                .with_landing(IntegerLanding {
                    landed_type,
                    domain: ArithmeticDomain::Exact,
                }),
            )
        }

        pub(crate) use typed_trees::closed_numeric::AnonymousNumericValue;

        pub(crate) fn anonymous_numeric_value(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            builtin: &mut impl FnMut(ExpressionHandle) -> bool,
        ) -> Option<AnonymousNumericValue> {
            evaluate_anonymous_value::<true>(program, expression, &[], builtin)
        }

        pub use typed_trees::closed_numeric::has_anonymous_numeric_results;

        pub(crate) fn anonymous_integer_literal_tree_value(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            builtin: &mut impl FnMut(ExpressionHandle) -> bool,
        ) -> Option<AnonymousNumericValue> {
            evaluate_anonymous_value::<false>(program, expression, &[], builtin)
        }

        fn evaluate_anonymous_value<const ALLOW_DECIMAL_LITERALS: bool>(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            selected_arms: &[(ExpressionHandle, ExpressionHandle)],
            builtin: &mut impl FnMut(ExpressionHandle) -> bool,
        ) -> Option<AnonymousNumericValue> {
            typed_trees::closed_numeric::evaluate_anonymous_value::<ALLOW_DECIMAL_LITERALS>(
                program,
                expression,
                selected_arms,
                builtin,
            )
        }

        pub use typed_trees::closed_numeric::has_anonymous_operator_meaning;
        pub(crate) use typed_trees::closed_numeric::has_builtin_anonymous_operands;
    }
    pub(crate) use integer_landing::anonymous_integer_landing_errors;
    pub(crate) use integer_landing::{
        anonymous_numeric_value, evaluate_anonymous_numeric_equality,
    };
    pub use integer_landing::{
        evaluate_anonymous_numeric_comparison, evaluate_anonymous_numeric_expression,
        has_anonymous_numeric_results, has_anonymous_operator_meaning,
        land_anonymous_integer_expression, land_integer_value, select_anonymous_numeric_match_arm,
    };

    pub(crate) fn validate_suffix_landing(
        program: &dyn crate::CheckingInput,
        value: ExpressionHandle,
        declared: typed_trees::types::TypeReferenceHandle,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if let Some(dispatch) = program.expression_table.match_node(value) {
            for arm in program.expression_table.match_arms(dispatch.arms) {
                validate_suffix_landing(program, arm.value, declared, diagnostics);
            }
            return;
        }
        constant_arrays::validate_declared_array_destination(program, value, declared, diagnostics);

        let literal_landing =
            |expression: ExpressionHandle| -> Option<(ExpressionHandle, LandedIntegerType)> {
                let mut current = expression;
                loop {
                    match program.expression_table.expression(current) {
                        ExpressionNode::Borrow(inner) => current = inner.target,
                        ExpressionNode::Integer(literal) => {
                            return literal
                                .landing()
                                .map(|landing| (current, landing.landed_type));
                        }
                        _ => return None,
                    }
                }
            };

        let float_landing = |expression: ExpressionHandle| -> Option<(
        ExpressionHandle,
        numerics::literals::FloatFormat,
    )> {
        let mut current = expression;
        loop {
            match program.expression_table.expression(current) {
                ExpressionNode::Borrow(inner) => current = inner.target,
                ExpressionNode::Float(literal) => {
                    return literal.landing().map(|landing| (current, landing));
                }
                _ => return None,
            }
        }
    };

        if program.static_type_context_is_unreachable(value) {
            return;
        }
        let declared = program.statically_refined_type_reference(value, declared);
        let Some(unwrapped) =
            crate::value_custody::places::unwrapped_type_reference(program, declared)
        else {
            return;
        };
        if let typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. } =
            program.type_reference_table.type_reference(unwrapped)
            && let ExpressionNode::ArrayLiteral(elements) =
                program.expression_table.expression(value)
        {
            for element in program.expression_table.expression_handles(*elements) {
                validate_suffix_landing(program, *element, *element_type, diagnostics);
            }
            return;
        }
        let Some(primitive) = program.primitive_type_reference(unwrapped) else {
            return;
        };
        float_landing::validate_numeric_tree_destination(program, value, primitive, diagnostics);
        if let Some((literal_handle, suffix_type)) = literal_landing(value) {
            let Some(declared_type) = primitive_landing_type(primitive) else {
                return;
            };
            if declared_type != suffix_type {
                let literal = program.expression_table.display_name(literal_handle);
                diagnostics.push(Diagnostic::error(format!(
                "literal `{literal}` is suffixed `{suffix}` but lands in a `{declared}` place -- \
                 a width suffix chooses the literal's type at the spelling, so it must agree \
                 with the destination's declared type (drop the suffix or fix one side)",
                suffix = suffix_type.name(),
                declared = primitive.name(),
            )));
            }
            return;
        }
        if let Some((literal_handle, suffix_format)) = float_landing(value) {
            use numerics::literals::FloatFormat;
            let declared_format = match primitive {
                PrimitiveType::F32 => FloatFormat::F32,
                PrimitiveType::F64 => FloatFormat::F64,
                _ => return,
            };
            if declared_format != suffix_format {
                let literal = program.expression_table.display_name(literal_handle);
                diagnostics.push(Diagnostic::error(format!(
                "literal `{literal}` is suffixed `{suffix}` but lands in a `{declared}` place -- \
                 a width suffix chooses the literal's format at the spelling, so it must agree \
                 with the destination's declared type (drop the suffix or fix one side)",
                suffix = suffix_format.name(),
                declared = primitive.name(),
            )));
            }
        }
    }

    pub(crate) fn validate_suffix_landings(
        program: &dyn crate::CheckingInput,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for (expression, node) in program.expression_table.expression_entries() {
            let ExpressionNode::StructLiteral(literal) = node else {
                continue;
            };
            crate::value_custody::struct_literals::each_selected_literal_field(
                program,
                expression,
                literal,
                |value, field_type| {
                    validate_suffix_landing(program, value, field_type, diagnostics)
                },
            );
        }

        for machine in program.machines() {
            for state in program.machine_states(machine) {
                for statement in program.statement_table.statements(state.statement_nodes) {
                    match statement {
                        StatementNode::Expression(value) if state.return_type.is_valid() => {
                            validate_suffix_landing(
                                program,
                                *value,
                                state.return_type,
                                diagnostics,
                            );
                        }
                        StatementNode::Assignment(assignment) => {
                            if let Some(declared) =
                                crate::value_custody::places::declared_place_type_raw(
                                    program,
                                    machine,
                                    Some(state),
                                    assignment.target,
                                )
                            {
                                if let Some(event) = crate::value_custody::atomic_operations::atomic_assignment_carrier(
                                program, assignment,
                            ) {
                                let resident =
                                    crate::value_custody::atomic_operations::atomic_resident_type_reference(
                                        program, declared,
                                    )
                                    .unwrap_or(declared);
                                for &operand in &event.operands {
                                    validate_suffix_landing(
                                        program,
                                        operand,
                                        resident,
                                        diagnostics,
                                    );
                                }
                            } else {
                                validate_suffix_landing(
                                    program,
                                    assignment.value,
                                    declared,
                                    diagnostics,
                                );
                            }
                            }
                        }
                        StatementNode::LocalData(local)
                            if local.initial_value.is_valid()
                                && local.type_reference.is_valid() =>
                        {
                            validate_suffix_landing(
                                program,
                                local.initial_value,
                                local.type_reference,
                                diagnostics,
                            );
                        }
                        StatementNode::Transition(transition) if state.return_type.is_valid() => {
                            for target in transition.targets() {
                                if let typed_trees::statement::TransitionTargetNode::Value(value) =
                                    program.statement_table.transition_target(target)
                                {
                                    validate_suffix_landing(
                                        program,
                                        *value,
                                        state.return_type,
                                        diagnostics,
                                    );
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    pub(crate) mod owned_bytes {
        use diagnostics::Diagnostic;
        use typed_trees::{
            TypedTrees,
            expression::{ExpressionHandle, ExpressionNode},
            types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode},
        };

        pub fn land_owned_fixed_byte_literal(
            program: &mut TypedTrees,
            value: ExpressionHandle,
            destination: TypeReferenceHandle,
        ) -> Result<(), Diagnostic> {
            let ExpressionNode::String(bytes) = program.expression_table.expression(value).clone()
            else {
                return Ok(());
            };
            let mut carrier = destination;
            while let TypeReferenceNode::Constrained { base_type, .. } =
                program.type_reference_table.type_reference(carrier)
            {
                carrier = *base_type;
            }
            let TypeReferenceNode::FixedArray {
                element_type,
                length: _,
            } = program.type_reference_table.type_reference(carrier).clone()
            else {
                return Ok(());
            };

            if program.primitive_type_reference(element_type) != Some(PrimitiveType::U8) {
                return Err(Diagnostic::error(format!(
                    "quoted byte literal cannot initialize owned fixed array `{}`: its element type must be `u8`",
                    program.display_type_reference(destination),
                )));
            }
            let Some(width) = program.closed_fixed_array_length(carrier) else {
                return Err(Diagnostic::error(format!(
                    "quoted byte literal cannot initialize owned fixed byte array `{}`: its width must be a compile-known resolved integer literal",
                    program.display_type_reference(destination),
                )));
            };
            let actual = bytes.len();
            if actual != width {
                return Err(Diagnostic::error(format!(
                    "quoted byte literal has {actual} source byte(s), but owned fixed byte array `{}` requires exactly {width}; fixed-array literal copying neither truncates nor pads",
                    program.display_type_reference(destination),
                )));
            }

            let elements = bytes
                .iter()
                .map(|byte| {
                    program.expression_table.insert(ExpressionNode::Integer(
                        numerics::literals::IntegerLiteral::from_value(i64::from(*byte)),
                    ))
                })
                .collect::<Vec<_>>();
            let elements = program.expression_table.insert_expression_handles(elements);
            *program.expression_table.expression_mut(value) =
                ExpressionNode::ArrayLiteral(elements);
            Ok(())
        }
    }
    pub use owned_bytes::land_owned_fixed_byte_literal;
}

pub mod struct_literals {
    use typed_trees::data::{DataDefinition, DataMember};

    pub(crate) mod construction_bounds {
        use crate::frozen_program::expression_table;
        use language_semantics::declaration_selection::CollectionMeasure;
        use typed_trees::data::DataDefinition;
        use typed_trees::data::DataMember;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::types::PrimitiveType;

        #[derive(Clone, Copy)]
        pub(crate) struct Bounds {
            interval: crate::proof_contracts::arithmetic_domains::Interval,
            length: Option<i64>,
            capacity: Option<i64>,
            float_primitive: Option<PrimitiveType>,
            float_interval: Option<(f64, f64)>,
        }

        impl Bounds {
            const UNKNOWN: Bounds = Bounds {
                interval: crate::proof_contracts::arithmetic_domains::Interval::UNBOUNDED,
                length: None,
                capacity: None,
                float_primitive: None,
                float_interval: None,
            };
            fn point(value: i64) -> Bounds {
                Bounds {
                    interval: crate::proof_contracts::arithmetic_domains::Interval::constant(value),
                    ..Bounds::UNKNOWN
                }
            }
            fn floating(primitive: PrimitiveType, interval: Option<(f64, f64)>) -> Bounds {
                Bounds {
                    float_primitive: Some(primitive),
                    float_interval: interval,
                    ..Bounds::UNKNOWN
                }
            }

            fn zero(primitive: Option<PrimitiveType>) -> Bounds {
                match primitive {
                    Some(primitive @ (PrimitiveType::F32 | PrimitiveType::F64)) => {
                        Bounds::floating(primitive, Some((0.0, 0.0)))
                    }
                    Some(primitive)
                        if primitive.is_fixed_integer() || primitive == PrimitiveType::Bool =>
                    {
                        Bounds::point(0)
                    }
                    _ => Bounds::UNKNOWN,
                }
            }
        }

        pub(crate) enum Truth {
            True,
            False,
            Unknown,
        }

        pub(crate) fn bounds_fold(
            program: &dyn crate::CheckingInput,
            owner: symbols::SymbolHandle,
            valuation: &[(symbols::SymbolHandle, Bounds)],
            declared: &[symbols::SymbolHandle],
            expression: ExpressionHandle,
        ) -> Truth {
            let bounds = bounds_eval(program, owner, valuation, declared, expression);
            match (bounds.interval.low, bounds.interval.high) {
                (Some(low), _) if low >= 1 => Truth::True,
                (_, Some(high)) if high <= 0 => Truth::False,
                _ => Truth::Unknown,
            }
        }

        pub fn case_predicates_admit_zero(
            program: &dyn crate::CheckingInput,
            definition: &DataDefinition,
            variant: &typed_trees::data::DataVariant,
        ) -> bool {
            let common =
                program
                    .data_members(definition)
                    .iter()
                    .filter_map(|member| match member {
                        DataMember::Field(field) => Some(field),
                        DataMember::Variant(_) => None,
                    });
            let fields = common.chain(program.data_payload_fields(variant));
            let mut declared = Vec::new();
            let mut valuation = Vec::new();
            for field in fields {
                declared.push(field.symbol);
                let bounds = Bounds::zero(program.primitive_type_reference(field.type_reference));
                valuation.push((field.symbol, bounds));
            }
            program
                .proof_facts
                .span_or_empty(variant.requires_facts)
                .iter()
                .all(|fact| {
                    fact.as_expression().is_some_and(|expression| {
                        matches!(
                            bounds_fold(
                                program,
                                definition.symbol,
                                &valuation,
                                &declared,
                                *expression
                            ),
                            Truth::True
                        )
                    })
                })
        }

        fn bounds_eval(
            program: &dyn crate::CheckingInput,
            owner: symbols::SymbolHandle,
            valuation: &[(symbols::SymbolHandle, Bounds)],
            declared: &[symbols::SymbolHandle],
            expression: ExpressionHandle,
        ) -> Bounds {
            use typed_trees::expression::BinaryOperator;
            if let Some((_, argument)) =
                crate::proof_contracts::proof_embeddings::integer_embedding_argument(
                    program, expression,
                )
            {
                return bounds_eval(program, owner, valuation, declared, argument);
            }
            match expression_table(program).expression(expression) {
                ExpressionNode::Name(path) => {
                    if !path.symbol.is_valid() {
                        return Bounds::UNKNOWN;
                    }
                    valuation
                        .iter()
                        .find(|(symbol, _)| *symbol == path.symbol)
                        .map(|(_, bounds)| *bounds)
                        .unwrap_or_else(|| {
                            if declared.contains(&path.symbol) {
                                Bounds::point(0)
                            } else {
                                Bounds::UNKNOWN
                            }
                        })
                }
                ExpressionNode::Boolean(value) => Bounds::point(i64::from(*value)),
                ExpressionNode::Float(literal) => {
                    let primitive = match literal.landing() {
                        Some(numerics::literals::FloatFormat::F32) => PrimitiveType::F32,
                        Some(numerics::literals::FloatFormat::F64) => PrimitiveType::F64,
                        None => return Bounds::UNKNOWN,
                    };
                    let value = literal.landed_f64();
                    Bounds::floating(primitive, (!value.is_nan()).then_some((value, value)))
                }
                ExpressionNode::Integer(value) => value
                    .decimal_value_i128()
                    .and_then(|value| i64::try_from(value).ok())
                    .map(Bounds::point)
                    .unwrap_or(Bounds::UNKNOWN),
                ExpressionNode::Member(member)
                    if CollectionMeasure::from_authored_spelling(member.member.as_str())
                        .is_some() =>
                {
                    let measure = match expression_table(program).expression(member.receiver) {
                        ExpressionNode::Name(path) if path.symbol.is_valid() => {
                            match valuation.iter().find(|(field, _)| *field == path.symbol) {
                                Some((_, bounds)) => {
                                    match CollectionMeasure::from_authored_spelling(
                                        member.member.as_str(),
                                    ) {
                                        Some(CollectionMeasure::Length) => bounds.length,
                                        Some(CollectionMeasure::Capacity) => bounds.capacity,
                                        None => None,
                                    }
                                }
                                None => declared.contains(&path.symbol).then_some(0),
                            }
                        }
                        _ => None,
                    };
                    measure.map(Bounds::point).unwrap_or(Bounds::UNKNOWN)
                }
                ExpressionNode::Binary(binary) => {
                    let operand_type = |expression| {
                        crate::value_custody::expression_types::declaration_expression_result_type_reference(
                            program,
                            owner,
                            None,
                            &|expression| {
                                crate::proof_contracts::proof_embeddings::expression_type_reference(
                                    program, expression,
                                )
                            },
                            expression,
                        )
                    };
                    let references = [binary.left, binary.right].map(operand_type);
                    let left = bounds_eval(program, owner, valuation, declared, binary.left);
                    let right = bounds_eval(program, owner, valuation, declared, binary.right);
                    if left.float_primitive.is_some() || right.float_primitive.is_some() {
                        if left.float_primitive != right.float_primitive
                            || !crate::has_builtin_or_sealed_float_comparison_meaning(
                                program,
                                owner,
                                expression,
                                binary.operator,
                                &references,
                            )
                        {
                            return Bounds::UNKNOWN;
                        }
                        return tri(float_comparison(left, right, binary.operator));
                    }
                    if let Some(spelling) =
                        crate::proof_contracts::parameter_expression_meaning::spelled_binary_operator(
                            binary.operator,
                        )
                        && !typed_trees::operator::has_builtin_spelled_expression_meaning(
                            program,
                            owner,
                            expression,
                            spelling,
                            &references,
                        )
                    {
                        return Bounds::UNKNOWN;
                    }
                    match binary.operator {
                        BinaryOperator::Add
                        | BinaryOperator::Subtract
                        | BinaryOperator::Multiply => {
                            let interval = if crate::proof_integer_expression(program, expression)
                                || crate::anonymous_integer_math_literal(program, expression)
                                    .is_some()
                            {
                                match binary.operator {
                                    BinaryOperator::Add => left.interval.add(right.interval),
                                    BinaryOperator::Subtract => {
                                        left.interval.subtract(right.interval)
                                    }
                                    _ => left.interval.multiply(right.interval),
                                }
                            } else {
                                let Some(reference) = operand_type(expression) else {
                                    return Bounds::UNKNOWN;
                                };
                                let Some(primitive) = program.primitive_type_reference(reference)
                                else {
                                    return Bounds::UNKNOWN;
                                };
                                let policy =
                                    program.arithmetic_domain_for_type_reference(reference);
                                let Some(kind) =
                                    crate::checked_integer_binary_kind(binary.operator, policy)
                                else {
                                    return Bounds::UNKNOWN;
                                };
                                let range = |bounds: Bounds| {
                                    Some(facts::IntegerRange {
                                        minimum: numerics::bignum::BigInt::from_i64(
                                            bounds.interval.low?,
                                        ),
                                        maximum: numerics::bignum::BigInt::from_i64(
                                            bounds.interval.high?,
                                        ),
                                    })
                                };
                                let Some((left, right)) = range(left).zip(range(right)) else {
                                    return Bounds::UNKNOWN;
                                };
                                let Some(result) = crate::proof_contracts::arithmetic_domains::selected_integer_binary_interval(kind, primitive, primitive, left, right) else { return Bounds::UNKNOWN; };
                                crate::proof_contracts::arithmetic_domains::Interval {
                                    low: result.minimum.to_i64(),
                                    high: result.maximum.to_i64(),
                                }
                            };
                            Bounds {
                                interval,
                                ..Bounds::UNKNOWN
                            }
                        }
                        BinaryOperator::LessOrEqual => tri(compare(left, right, |a, b| a <= b)),
                        BinaryOperator::Less => tri(compare(left, right, |a, b| a < b)),
                        BinaryOperator::GreaterOrEqual => tri(compare(right, left, |a, b| a <= b)),
                        BinaryOperator::Greater => tri(compare(right, left, |a, b| a < b)),
                        BinaryOperator::Equal => tri(equality(left, right, true)),
                        BinaryOperator::NotEqual => tri(equality(left, right, false)),
                        BinaryOperator::And => tri(truth_and(to_truth(left), to_truth(right))),
                        BinaryOperator::Or => tri(truth_or(to_truth(left), to_truth(right))),
                        _ => Bounds::UNKNOWN,
                    }
                }
                ExpressionNode::Borrow(inner) => {
                    bounds_eval(program, owner, valuation, declared, inner.target)
                }
                _ => Bounds::UNKNOWN,
            }
        }

        fn float_comparison(
            left: Bounds,
            right: Bounds,
            operator: typed_trees::expression::BinaryOperator,
        ) -> Truth {
            use typed_trees::expression::BinaryOperator;
            let Some(((left_low, left_high), (right_low, right_high))) =
                left.float_interval.zip(right.float_interval)
            else {
                return Truth::Unknown;
            };
            let relation = |true_for_all: bool, false_for_all: bool| {
                if true_for_all {
                    Truth::True
                } else if false_for_all {
                    Truth::False
                } else {
                    Truth::Unknown
                }
            };
            match operator {
                BinaryOperator::LessOrEqual => {
                    relation(left_high <= right_low, left_low > right_high)
                }
                BinaryOperator::Less => relation(left_high < right_low, left_low >= right_high),
                BinaryOperator::GreaterOrEqual => {
                    relation(left_low >= right_high, left_high < right_low)
                }
                BinaryOperator::Greater => relation(left_low > right_high, left_high <= right_low),
                BinaryOperator::Equal | BinaryOperator::NotEqual => {
                    let equal =
                        left_low == left_high && right_low == right_high && left_low == right_low;
                    let disjoint = left_high < right_low || right_high < left_low;
                    if operator == BinaryOperator::Equal {
                        relation(equal, disjoint)
                    } else {
                        relation(disjoint, equal)
                    }
                }
                _ => Truth::Unknown,
            }
        }

        fn compare(left: Bounds, right: Bounds, relates: fn(i64, i64) -> bool) -> Truth {
            if let (Some(left_high), Some(right_low)) = (left.interval.high, right.interval.low)
                && relates(left_high, right_low)
            {
                return Truth::True;
            }
            if let (Some(left_low), Some(right_high)) = (left.interval.low, right.interval.high)
                && !relates(left_low, right_high)
            {
                return Truth::False;
            }
            Truth::Unknown
        }

        fn equality(left: Bounds, right: Bounds, wants_equal: bool) -> Truth {
            let same_point = left.interval.low == left.interval.high
                && right.interval.low == right.interval.high
                && left.interval.low.is_some()
                && left.interval.low == right.interval.low;
            let disjoint = matches!((left.interval.high, right.interval.low), (Some(a), Some(b)) if a < b)
                || matches!((right.interval.high, left.interval.low), (Some(a), Some(b)) if a < b);
            match (same_point, disjoint, wants_equal) {
                (true, _, true) | (_, true, false) => Truth::True,
                (true, _, false) | (_, true, true) => Truth::False,
                _ => Truth::Unknown,
            }
        }

        fn to_truth(bounds: Bounds) -> Truth {
            match (bounds.interval.low, bounds.interval.high) {
                (Some(low), _) if low >= 1 => Truth::True,
                (_, Some(high)) if high <= 0 => Truth::False,
                _ => Truth::Unknown,
            }
        }

        fn truth_and(left: Truth, right: Truth) -> Truth {
            match (left, right) {
                (Truth::False, _) | (_, Truth::False) => Truth::False,
                (Truth::True, Truth::True) => Truth::True,
                _ => Truth::Unknown,
            }
        }

        fn truth_or(left: Truth, right: Truth) -> Truth {
            match (left, right) {
                (Truth::True, _) | (_, Truth::True) => Truth::True,
                (Truth::False, Truth::False) => Truth::False,
                _ => Truth::Unknown,
            }
        }

        fn tri(truth: Truth) -> Bounds {
            match truth {
                Truth::True => Bounds::point(1),
                Truth::False => Bounds::point(0),
                Truth::Unknown => Bounds {
                    interval: crate::proof_contracts::arithmetic_domains::Interval {
                        low: Some(0),
                        high: Some(1),
                    },
                    ..Bounds::UNKNOWN
                },
            }
        }
    }
    pub(crate) mod field_obligations {
        use crate::CheckingInput;
        use crate::frozen_program::expression_table;
        use typed_trees::data::{DataDefinition, DataMember};
        use typed_trees::expression::{ExpressionHandle, TableStructLiteral};
        use typed_trees::types::TypeReferenceHandle;

        pub(crate) fn construction_field_type(
            program: &dyn CheckingInput,
            data_definition: &DataDefinition,
            case_name: Option<&str>,
            field_name: &str,
        ) -> Option<typed_trees::types::TypeReferenceHandle> {
            crate::value_custody::field_domains::construction_field_type(
                program,
                data_definition,
                case_name,
                field_name,
            )
        }

        pub(crate) fn construction_application(
            program: &dyn CheckingInput,
            expression: ExpressionHandle,
        ) -> TypeReferenceHandle {
            let required =
                expression_table(program).construction_required_type_reference(expression);
            let constructed =
                expression_table(program).constructed_value_type_reference(expression);
            if required.is_valid() {
                return required;
            }
            if constructed.is_valid() {
                return constructed;
            }
            for occurrence in expression_table(program).authored_selection_occurrences(expression) {
                if let Some(selection) = program.authored_declaration_selections().get(occurrence) {
                    if selection.source_span() != expression_table(program).source_span(expression)
                    {
                        continue;
                    }
                    if let Some(selected) = selection.target().as_resolved()
                        && let Some(declaration) =
                            program.const_declaration_by_symbol(selected.selected_symbol())
                    {
                        return declaration.declared_type;
                    }
                }
            }
            TypeReferenceHandle::invalid()
        }

        pub fn selected_construction_definition<'a>(
            program: &'a dyn CheckingInput,
            expression: ExpressionHandle,
            literal: &TableStructLiteral,
        ) -> Option<&'a DataDefinition> {
            let reference = construction_application(program, expression);
            if reference.is_valid() {
                return program.data_constructor_definition(reference, literal.type_symbol);
            }
            let definition = program.unique_data_definition_by_symbol(literal.type_symbol)?;
            construction_members_are_concrete(program, definition).then_some(definition)
        }

        pub fn selected_construction_case<'a>(
            program: &'a dyn CheckingInput,
            expression: ExpressionHandle,
            literal: &TableStructLiteral,
            definition: &'a DataDefinition,
        ) -> Option<&'a typed_trees::data::DataVariant> {
            let selected = literal.case_symbol?;
            let receiving = construction_application(program, expression);
            if receiving.is_valid() {
                return program.data_constructor_case(receiving, literal.type_symbol, selected);
            }
            program
                .data_members(definition)
                .iter()
                .find_map(|member| match member {
                    DataMember::Variant(case) if case.symbol == selected => Some(case),
                    _ => None,
                })
        }

        pub(crate) fn each_selected_literal_field(
            program: &dyn CheckingInput,
            expression: ExpressionHandle,
            literal: &TableStructLiteral,
            mut each: impl FnMut(ExpressionHandle, TypeReferenceHandle),
        ) {
            for field in expression_table(program).struct_fields(literal.fields) {
                let Some(field_type) = selected_construction_field_type(
                    program,
                    expression,
                    literal,
                    field.field_symbol,
                ) else {
                    continue;
                };
                each(field.value, field_type);
            }
        }

        pub fn selected_construction_field_type(
            program: &dyn CheckingInput,
            expression: ExpressionHandle,
            literal: &TableStructLiteral,
            field: symbols::SymbolHandle,
        ) -> Option<TypeReferenceHandle> {
            selected_construction_field(program, expression, literal, field)
                .map(|field| field.type_reference)
        }

        pub fn selected_construction_field<'a>(
            program: &'a dyn CheckingInput,
            expression: ExpressionHandle,
            literal: &TableStructLiteral,
            field: symbols::SymbolHandle,
        ) -> Option<&'a typed_trees::data::DataField> {
            if !literal.type_symbol.is_valid() || !field.is_valid() {
                return None;
            }
            let receiving = construction_application(program, expression);
            if receiving.is_valid() {
                let selected =
                    program.data_constructor_field(receiving, literal.type_symbol, field)?;
                let parent = program.symbols.get(selected.symbol).parent;
                if program.symbols.get(parent).kind == symbols::SymbolKind::Variant {
                    let active = program.data_constructor_case(
                        receiving,
                        literal.type_symbol,
                        literal.case_symbol?,
                    )?;
                    if active.symbol != parent {
                        return None;
                    }
                }
                return Some(selected);
            }
            let definition = selected_construction_definition(program, expression, literal)?;
            let case =
                if let Some(case_symbol) = literal.case_symbol.filter(|symbol| symbol.is_valid()) {
                    let mut cases = program
                        .data_members(definition)
                        .iter()
                        .filter_map(|member| match member {
                            DataMember::Variant(case) if case.symbol == case_symbol => Some(case),
                            _ => None,
                        });
                    let case = cases.next()?;
                    if cases.next().is_some() {
                        return None;
                    }
                    Some(case)
                } else {
                    if literal.case_name.is_some() {
                        return None;
                    }
                    None
                };
            let mut fields = program
                .data_members(definition)
                .iter()
                .filter_map(|member| match member {
                    DataMember::Field(candidate) => Some(candidate),
                    _ => None,
                })
                .chain(
                    case.into_iter()
                        .flat_map(|case| program.data_payload_fields(case)),
                )
                .filter(|candidate| candidate.symbol == field);
            let selected = fields.next()?;
            let reference = selected.type_reference;
            if fields.next().is_some() {
                return None;
            }
            program
                .type_reference_table
                .contains_type_reference(reference)
                .then_some(selected)
        }
    }
    pub(crate) mod window_elements {
        use crate::frozen_program::expression_table;
        use typed_trees::expression::ExpressionHandle;
        use typed_trees::machine::Machine;
        use typed_trees::state::State;

        pub struct ArrayWindowShape {
            pub element_type: typed_trees::types::TypeReferenceHandle,
            pub start: Option<usize>,
            pub end: Option<usize>,
        }

        impl ArrayWindowShape {
            pub fn length(&self) -> Option<usize> {
                self.start
                    .zip(self.end)
                    .and_then(|(start, end)| end.checked_sub(start))
            }
        }

        pub fn array_window_shape(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            state: &State,
            target: ExpressionHandle,
            bound_lookup: &mut crate::proof_contracts::immutable_integer_bounds::ImmutableBoundLookup<'_>,
        ) -> Option<ArrayWindowShape> {
            let indexed = expression_table(program).indexed_node(target)?;
            let range = expression_table(program).range_node(indexed.index)?;
            let collection_type = crate::value_custody::places::declared_place_type_raw(
                program,
                machine,
                Some(state),
                indexed.collection,
            )
            .or_else(|| {
                crate::value_custody::places::declared_indexed_projection_type_raw(
                    program,
                    machine,
                    Some(state),
                    indexed.collection,
                )
            })?;
            let shape =
                crate::value_custody::places::unwrapped_type_reference(program, collection_type)?;
            let (_, collection_length) =
                crate::value_custody::type_shapes::collection_element_and_length(program, shape)?;
            let element_type = crate::value_custody::places::collection_element_type_reference(
                program,
                collection_type,
            )
            .unwrap_or_default();
            let start = if range.start.is_valid() {
                crate::normalize_immutable_integer_bound_to_usize(
                    program,
                    bound_lookup,
                    range.start,
                )
            } else {
                Some(0)
            };
            let end = if range.end.is_valid() {
                crate::normalize_immutable_integer_bound_to_usize(program, bound_lookup, range.end)
                    .and_then(|end| {
                        if range.end_inclusive {
                            end.checked_add(1)
                        } else {
                            Some(end)
                        }
                    })
            } else {
                collection_length
            };
            Some(ArrayWindowShape {
                element_type,
                start,
                end,
            })
        }
    }

    pub use construction_bounds::case_predicates_admit_zero;
    pub(crate) use field_obligations::{
        construction_application, construction_field_type, each_selected_literal_field,
    };
    pub use field_obligations::{
        selected_construction_case, selected_construction_definition, selected_construction_field,
        selected_construction_field_type,
    };
    pub use window_elements::{ArrayWindowShape, array_window_shape};

    pub(crate) fn data_declares_field(
        program: &dyn crate::CheckingInput,
        data_definition: &DataDefinition,
        field_name: &str,
    ) -> bool {
        program.data_members(data_definition).iter().any(
        |member| matches!(member, DataMember::Field(field) if field.name.as_str() == field_name),
    )
    }
}
