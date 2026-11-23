use crate::SyntaxTrees;

impl SyntaxTrees {
    pub fn copy_expression_handles_to_statement_table(
        &mut self,
        arguments: arena::HandleSpan<crate::expression::ExpressionHandle>,
    ) -> arena::HandleSpan<crate::expression::ExpressionHandle> {
        let mut copy = arena::HandleSpan::empty();

        let tables = &mut self.tables;
        for argument in tables.expressions.expression_handles(arguments) {
            copy.push_contiguous(tables.statements.append_expression_handle(*argument));
        }

        copy
    }

    pub fn expression_identifier_path(
        &self,
        expression: crate::expression::ExpressionHandle,
    ) -> Option<Vec<&crate::identifier::Identifier>> {
        use crate::expression::ExpressionNode;
        if !expression.is_valid() {
            return Some(Vec::new());
        }
        match self.expressions.expression(expression) {
            ExpressionNode::Name {
                members,
                application: None,
            } => Some(
                self.expressions
                    .identifier_path_members(*members)
                    .iter()
                    .collect(),
            ),
            ExpressionNode::Member(member) => {
                let mut path = self.expression_identifier_path(member.receiver)?;
                path.push(&member.member);
                Some(path)
            }
            _ => None,
        }
    }
}
