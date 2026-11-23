use crate::SyntaxTrees;

impl SyntaxTrees {
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
