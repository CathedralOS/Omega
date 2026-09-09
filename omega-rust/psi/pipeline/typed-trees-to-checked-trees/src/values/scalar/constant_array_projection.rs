//! A fixed projection of a closed constant array produces its existing scalar
//! leaf. The original typed indexing and declaration selections remain intact
//! for checking and source custody; this creates neither a place nor a borrow.
//! Every leaf must be a literal, since selecting one element cannot erase the
//! evaluation of an effectful or failing sibling constructor expression.

use super::structural_fields;
use checked_trees::CheckedOperatorFacts;
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

pub(super) fn selected_leaf(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let mut projections = Vec::new();
    let mut root = expression;
    while let ExpressionNode::Indexed(indexed) = program.expression_table.expression(root) {
        projections.push((root, indexed.index));
        root = indexed.collection;
    }
    if projections.is_empty() {
        return None;
    }
    let mut reference = validation::declared_constant_array_type(program, root)?;
    if !closed_literal_array(program, root, reference) {
        return None;
    }
    let mut selected = root;
    for (projection, index) in projections.into_iter().rev() {
        let TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } = program.type_reference_table.type_reference(reference)
        else {
            return None;
        };
        let ExpressionNode::ArrayLiteral(elements) = program.expression_table.expression(selected)
        else {
            return None;
        };
        let ExpressionNode::Integer(literal) = program.expression_table.expression(index) else {
            return None;
        };
        if literal.landing().is_some_and(|landing| {
            landing.landed_type == numerics::literals::LandedIntegerType::Addr
                || landing.domain != ArithmeticDomain::Exact
        }) || !structural_fields::indexed_read_is_builtin(
            program, operators, parameters, projection, reference, index,
        ) {
            return None;
        }
        let element_index = usize::try_from(literal.value_u64()?).ok()?;
        if element_index >= *length {
            return None;
        }
        selected = *program
            .expression_table
            .expression_handles(*elements)
            .get(element_index)?;
        reference = *element_type;
    }
    matches!(
        program.expression_table.expression(selected),
        ExpressionNode::Integer(_) | ExpressionNode::Boolean(_)
    )
    .then_some(selected)
}

fn closed_literal_array(
    program: &TypedTrees,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
) -> bool {
    validation::closed_literal_array_elements(program, expression, reference).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use numerics::literals::IntegerLiteral;

    fn fixture(carrier: &str, initializer: &str) -> (TypedTrees, ExpressionHandle) {
        let source = format!(
            "data Sizes {{}} const Sizes::VALUES: {carrier} = {initializer};
             machine read() -> u8 {{ let values: {carrier} = Sizes::VALUES; 0u8 }}"
        );
        let tokens = source_files_to_tokens::Lexer::new(&source)
            .tokenize()
            .expect("tokenize");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
            .expect("resolve array constant without projection");
        let program = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("type array constant");
        let root = program
            .expression_table
            .expression_entries()
            .find_map(|(expression, _)| {
                validation::declared_constant_array_type(&program, expression).map(|_| expression)
            })
            .expect("retained array constant root");
        (program, root)
    }

    fn project(
        program: &mut TypedTrees,
        collection: ExpressionHandle,
        index: i64,
    ) -> ExpressionHandle {
        let index = program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(index)));
        program.expression_table.insert(ExpressionNode::Indexed(
            typed_trees::expression::TableIndexedExpression { collection, index },
        ))
    }

    #[test]
    fn fixed_constant_projection_lowers_existing_integer_and_boolean_leaves() {
        let (mut program, root) = fixture("[u8; 2]", "[7, 9]");
        let projection = project(&mut program, root, 1);
        let original = program.expression_table.expression(projection).clone();
        let operators = CheckedOperatorFacts::default();
        let (lowered, _) = super::super::lower_scalar_expression(
            &program,
            &operators,
            projection,
            &[],
            &[],
            &[],
            &[],
            &[],
        )
        .expect("fixed integer projection lowers");
        let checked_trees::CheckedScalarExpression::IntegerLiteral { literal } = lowered else {
            panic!("integer leaf");
        };
        assert_eq!(literal.value_u64(), Some(9));
        assert_eq!(
            literal.landing().expect("declared landing").landed_type,
            numerics::literals::LandedIntegerType::U8
        );
        assert_eq!(program.expression_table.expression(projection), &original);

        let (mut program, root) = fixture("[[bool; 2]; 2]", "[[true, false], [false, true]]");
        let inner = project(&mut program, root, 1);
        let projection = project(&mut program, inner, 1);
        assert!(matches!(
            super::super::lower_boolean_expression(
                &program,
                &operators,
                projection,
                &[],
                &[],
                &[],
                &[],
                &[],
            ),
            Some(checked_trees::CheckedBooleanExpression::Constant(true))
        ));
    }

    #[test]
    fn projection_rejects_invalid_selectors_and_unselected_nonliteral_siblings() {
        let (mut program, root) = fixture("[u8; 2]", "[7, 9]");
        let operators = CheckedOperatorFacts::default();
        for index in [-1, 2] {
            let projection = project(&mut program, root, index);
            assert!(selected_leaf(&program, &operators, &[], projection).is_none());
        }
        let projection = project(&mut program, root, 0);
        let ExpressionNode::Indexed(indexed) = program.expression_table.expression(projection)
        else {
            panic!("indexing");
        };
        let index = indexed.index;
        *program.expression_table.expression_mut(index) = ExpressionNode::Boolean(false);
        assert!(selected_leaf(&program, &operators, &[], projection).is_none());
        *program.expression_table.expression_mut(index) =
            ExpressionNode::Integer(IntegerLiteral::from_value(0));
        let ExpressionNode::ArrayLiteral(elements) = program.expression_table.expression(root)
        else {
            panic!("array");
        };
        let sibling = program.expression_table.expression_handles(*elements)[1];
        *program.expression_table.expression_mut(sibling) =
            ExpressionNode::Name(Default::default());
        assert!(selected_leaf(&program, &operators, &[], projection).is_none());

        let (mut empty, root) = fixture("[u8; 0]", "[]");
        let projection = project(&mut empty, root, 0);
        assert!(selected_leaf(&empty, &operators, &[], projection).is_none());
    }

    #[test]
    fn nested_projection_requires_builtin_indexing_at_every_level() {
        let (mut program, root) = fixture("[[u8; 1]; 1]", "[[7]]");
        let inner = project(&mut program, root, 0);
        let outer = project(&mut program, inner, 0);
        assert!(selected_leaf(&program, &CheckedOperatorFacts::default(), &[], outer).is_some());
        for expression in [inner, outer] {
            let mut uses = arena::Arena::new();
            uses.append(checked_trees::CheckedOperatorUseFact {
                expression,
                spelling: language_core::OperatorSpelling::Index,
                status: checked_trees::CheckedOperatorResolutionStatus::Resolved,
                selected_operator_symbol: symbols::SymbolHandle::from_parts(1, 1),
                ..Default::default()
            });
            let operators =
                CheckedOperatorFacts::with_roots(uses, arena::Arena::new(), arena::Arena::new());
            assert!(selected_leaf(&program, &operators, &[], outer).is_none());
        }
    }
}
