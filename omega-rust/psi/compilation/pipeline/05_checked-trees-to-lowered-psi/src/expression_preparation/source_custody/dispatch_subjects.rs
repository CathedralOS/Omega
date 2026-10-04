//! Closed-case dispatch subjects as the parser spells them, re-read from
//! source independently of the checker's plan.
//!
//! A transition dispatch over a case sum is parsed into one guarded arm per
//! case. Guards may name a case classifier or compare a payload-free case
//! value. When the subject is a place, every arm names that place. When it is
//! a call, the parser copies the call into every guard and every payload read
//! (`copy.field` qualified by the arm's case). The copies are separate
//! expression nodes that repeat the same call, operands and authored
//! selection occurrence. The language evaluates that call once, at the first
//! arm (wiki/spec/language/expressions.md), so every other copy reads that
//! one result.
use checked_trees::CheckedTrees;
use checked_trees::data::DataMember;
use checked_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use checked_trees::statement::{StatementNode, TransitionGuardNode};

/// The subject and variant a case guard tests, looking through the parser's
/// `true == ..` wrapping.
pub(crate) fn case_test(
    checked: &CheckedTrees,
    expression: ExpressionHandle,
) -> Option<(ExpressionHandle, symbols::SymbolHandle)> {
    let ExpressionNode::Binary(binary) = checked.expression_table.expression(expression) else {
        return None;
    };
    let authored_membership = checked.expression_table.authored_selection_occurrences(expression)
        .any(|occurrence| checked.authored_declaration_selections().get(occurrence).is_some_and(|selection| {
            selection.kind() == language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::CaseMembership
        }));
    if binary.operator == BinaryOperator::CaseMembership || authored_membership {
        // Membership observes only its left operand. Its right operand is a
        // classifier, not a constructed value that equality may commute.
        // Authored `in` retains this meaning in its selection roster even
        // when typing spells the tag comparison as BinaryOperator::Equal.
        if !matches!(
            binary.operator,
            BinaryOperator::Equal | BinaryOperator::CaseMembership
        ) {
            return None;
        }
        let ExpressionNode::Name(classifier) = checked.expression_table.expression(binary.right)
        else {
            return None;
        };
        let owner = validation::exact_case_reference_owner(&checked.typed, binary.right)?;
        if authored_membership {
            use language_semantics::declaration_selection::{
                AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionTarget,
            };
            let mut has_owner = false;
            let mut has_case = false;
            for occurrence in checked
                .expression_table
                .authored_selection_occurrences(expression)
            {
                let selection = checked.authored_declaration_selections().get(occurrence)?;
                let AuthoredDeclarationSelectionTarget::Resolved(selected) = selection.target()
                else {
                    return None;
                };
                match selection.kind() {
                    AuthoredDeclarationSelectionKind::CaseReference
                        if selected.selected_symbol() == owner.symbol && !has_owner =>
                    {
                        has_owner = true
                    }
                    AuthoredDeclarationSelectionKind::CaseMembership
                        if selected.selected_symbol() == classifier.symbol && !has_case =>
                    {
                        has_case = true
                    }
                    _ => return None,
                }
            }
            if !has_owner || !has_case {
                return None;
            }
        }
        return Some((binary.left, classifier.symbol));
    }
    if binary.operator != BinaryOperator::Equal {
        return None;
    }
    if matches!(
        checked.expression_table.expression(binary.left),
        ExpressionNode::Boolean(true)
    ) {
        return case_test(checked, binary.right);
    }
    if matches!(
        checked.expression_table.expression(binary.right),
        ExpressionNode::Boolean(true)
    ) {
        return case_test(checked, binary.left);
    }
    for (subject, candidate) in [(binary.left, binary.right), (binary.right, binary.left)] {
        let case = match checked.expression_table.expression(candidate) {
            ExpressionNode::Name(path) if declared_case(checked, path.symbol) => path.symbol,
            ExpressionNode::StructLiteral(literal) => {
                let case = literal.case_symbol?;
                let definition = checked
                    .data_definitions()
                    .iter()
                    .find(|definition| definition.symbol == literal.type_symbol)?;
                let members = checked.data_members(definition);
                let variant = members.iter().find_map(|member| match member {
                    DataMember::Variant(variant) if variant.symbol == case => Some(variant),
                    _ => None,
                })?;
                // Structural equality also observes common fields and the
                // active payload. Only a field-free case value is a tag test.
                if literal.fields.count() != 0
                    || !checked
                        .expression_table
                        .struct_fields(literal.fields)
                        .is_empty()
                    || !checked.data_payload_fields(variant).is_empty()
                    || members
                        .iter()
                        .any(|member| matches!(member, DataMember::Field(_)))
                {
                    return None;
                }
                case
            }
            _ => continue,
        };
        return Some((subject, case));
    }
    None
}

fn declared_case(checked: &CheckedTrees, symbol: symbols::SymbolHandle) -> bool {
    checked.data_definitions().iter().any(|definition| {
        checked.data_members(definition).iter().any(
            |member| matches!(member, DataMember::Variant(variant) if variant.symbol == symbol),
        )
    })
}

/// The call a case guard tests. Its operands use ordinary call preparation;
/// copied guards and payload receivers read the completed subject instead of
/// evaluating those operands again. Source custody and the argument schedule
/// still account for each producer before this subject becomes available.
pub(crate) fn computed_case_subject(
    checked: &CheckedTrees,
    guard: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let (subject, _) = case_test(checked, guard)?;
    let table = &checked.expression_table;
    let ExpressionNode::Call(call) = table.expression(subject) else {
        return None;
    };
    let operands = table.expression_handles(call.arguments);
    if !repeats_subject(checked, subject, subject)
        || operands.len() != call.arguments.count() as usize
    {
        return None;
    }
    Some(subject)
}

/// Whether `copy` repeats the computed `subject`: the same call and operands
/// carrying exactly the subject's nonempty authored selection occurrence.
/// Source spans are diagnostic only, so a separately authored call of the
/// same shape keeps its own occurrence and stays a separate evaluation.
pub(crate) fn repeats_subject(
    checked: &CheckedTrees,
    subject: ExpressionHandle,
    copy: ExpressionHandle,
) -> bool {
    let table = &checked.expression_table;
    if !table.expression_is_valid(subject)
        || !table.expression_is_valid(copy)
        || !matches!(table.expression(subject), ExpressionNode::Call(_))
        || !matches!(table.expression(copy), ExpressionNode::Call(_))
        || !table.expressions_structurally_equal(subject, copy)
    {
        return false;
    }
    let mut occurrences = table.authored_selection_occurrences(subject).peekable();
    occurrences.peek().is_some() && occurrences.eq(table.authored_selection_occurrences(copy))
}

/// The computed subject statement `index` evaluates: the subject of its
/// guard when no earlier transition of `statements` tests a copy of it.
pub(crate) fn evaluated_case_subject(
    checked: &CheckedTrees,
    statements: &[StatementNode],
    index: usize,
) -> Option<ExpressionHandle> {
    let tested = |statement: &StatementNode| {
        let StatementNode::Transition(transition) = statement else {
            return None;
        };
        let TransitionGuardNode::When(guard) = transition.guard else {
            return None;
        };
        computed_case_subject(checked, guard)
    };
    let subject = tested(statements.get(index)?)?;
    (!statements[..index].iter().any(|statement| {
        tested(statement).is_some_and(|earlier| repeats_subject(checked, subject, earlier))
    }))
    .then_some(subject)
}
