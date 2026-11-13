/*
Source relation matching and declared termination queries retained while callers
move out of validation. Executable quotient operations belong to lowered checking.
*/
mod equivalence_selection {
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionNode;
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    pub(crate) fn exact_relation_application_matches(
        program: &dyn crate::CheckingInput,
        application: &typed_trees::proposition::PropositionApplication,
        relation_symbol: SymbolHandle,
        left_symbol: SymbolHandle,
        right_symbol: SymbolHandle,
        left_type: TypeReferenceHandle,
        right_type: TypeReferenceHandle,
    ) -> bool {
        let Some(relation) = program.proposition_definition_by_symbol(relation_symbol) else {
            return false;
        };
        if application.proposition != relation.symbol
            || !matches!(
                program.expression_table.expression_handles(application.arguments),
                [left_expression, right_expression]
                    if expression_is_symbol(program, *left_expression, left_symbol)
                        && expression_is_symbol(program, *right_expression, right_symbol)
            )
        {
            return false;
        }
        let Some(mut expected_binders) = exact_generic_argument_symbols(program, left_type) else {
            return false;
        };
        let Some(right_binders) = exact_generic_argument_symbols(program, right_type) else {
            return false;
        };
        expected_binders.extend(right_binders);
        let declared_binders = program.proposition_binders(relation);
        application.binder_arguments.len() == declared_binders.len()
            && expected_binders.len() == declared_binders.len()
            && application
                .binder_arguments
                .iter()
                .zip(declared_binders)
                .zip(expected_binders)
                .all(|((actual, declared), expected)| {
                    actual.symbol == expected
                        && matches!(
                            (actual.kind, &declared.kind),
                            (
                                typed_trees::proposition::PropositionBinderArgumentKind::Type,
                                typed_trees::proposition::PropositionBinderKind::Type,
                            ) | (
                                typed_trees::proposition::PropositionBinderArgumentKind::Const,
                                typed_trees::proposition::PropositionBinderKind::Const { .. },
                            ) | (
                                typed_trees::proposition::PropositionBinderArgumentKind::Machine,
                                typed_trees::proposition::PropositionBinderKind::Machine,
                            )
                        )
                })
    }

    fn exact_generic_argument_symbols(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<Vec<SymbolHandle>> {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Generic { arguments, .. } => program
                .type_reference_table
                .type_reference_handles(*arguments)
                .iter()
                .map(|argument| type_reference_symbol(program, *argument))
                .collect(),
            TypeReferenceNode::Reference { referee, .. } => {
                exact_generic_argument_symbols(program, *referee)
            }
            TypeReferenceNode::Constrained { base_type, .. } => {
                exact_generic_argument_symbols(program, *base_type)
            }
            TypeReferenceNode::Named { .. } => Some(Vec::new()),
            _ => None,
        }
    }

    fn expression_is_symbol(
        program: &dyn crate::CheckingInput,
        expression: typed_trees::expression::ExpressionHandle,
        symbol: SymbolHandle,
    ) -> bool {
        matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Name(path) if symbol.is_valid() && path.symbol == symbol
        )
    }

    fn type_reference_symbol(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<SymbolHandle> {
        program.type_reference_table.type_head_symbol(
            program
                .type_reference_table
                .peeled_type_reference(type_reference),
        )
    }
}

pub(crate) use equivalence_selection::exact_relation_application_matches;
