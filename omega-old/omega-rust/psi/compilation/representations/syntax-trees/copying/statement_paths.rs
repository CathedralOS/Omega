use crate::SyntaxTrees;

impl SyntaxTrees {
    pub fn copy_identifier_path_to_statement_table(
        &mut self,
        path: arena::HandleSpan<crate::identifier::Identifier>,
    ) -> arena::HandleSpan<crate::identifier::Identifier> {
        let mut copy = arena::HandleSpan::empty();

        let member_count = self.expressions.identifier_path_members(path).len();

        for index in 0..member_count {
            let member = self.expressions.identifier_path_members(path)[index].clone();
            copy.push_contiguous(self.statements.append_identifier_path_member(member));
        }

        copy
    }

    pub fn append_identifier_path_member_to_statement_table(
        &mut self,
        path: arena::HandleSpan<crate::identifier::Identifier>,
        member: crate::identifier::Identifier,
    ) -> arena::HandleSpan<crate::identifier::Identifier> {
        let handle = self.statements.append_identifier_path_member(member);

        let mut path = path;
        path.push_contiguous(handle);
        path
    }

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

    pub fn copy_expression_to_statement_identifier_path(
        &mut self,
        expression: crate::expression::ExpressionHandle,
    ) -> Option<arena::HandleSpan<crate::identifier::Identifier>> {
        use crate::expression::ExpressionNode;
        match self.expressions.expression(expression).clone() {
            ExpressionNode::Name {
                members: path,
                application: path_application,
            } if path_application.is_none() => {
                Some(self.copy_identifier_path_to_statement_table(path.to_owned()))
            }
            ExpressionNode::Member(member) => {
                let receiver =
                    self.copy_expression_to_statement_identifier_path(member.receiver)?;
                Some(self.append_identifier_path_member_to_statement_table(receiver, member.member))
            }
            _ => None,
        }
    }
}
