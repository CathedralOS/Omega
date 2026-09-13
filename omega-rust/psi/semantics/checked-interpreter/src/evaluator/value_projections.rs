//! Read projections retain one selected cell, whether their receiver is stored
//! or produced by an expression. Arguments and reference destinations retain
//! its Ref wrapper; scalar observations may dereference it. This avoids both
//! replaying an effectful selector after a speculative place lookup and losing
//! the original referent when a temporary carrier dies. Writable place
//! resolution stays separate: materializing a value grants no source-place
//! assignment or borrowing authority.

use super::*;

impl Evaluator<'_> {
    pub(super) fn eval_read_cell(
        &mut self,
        expression: ExpressionHandle,
        frame: &Frame,
    ) -> EvalResult<Cell> {
        match self.program.expression_table.expression(expression).clone() {
            ExpressionNode::Name(path) => {
                let members = self
                    .program
                    .expression_table
                    .name_path_members(path.members);
                // Resolution may retain a field path in one Name rather than
                // nested Members. Observe its recast layout too, but preserve
                // a bare reference binding instead of snapshotting its target.
                if members.len() > 1
                    && let Some(value) =
                        self.read_mutable_record_recast_target(expression, frame)?
                {
                    return self.allocate_cell(value);
                }
                if matches!(members, [name] if matches!(name.as_str(), "true" | "false")) {
                    let value = self.eval_name(&path, frame)?;
                    return self.allocate_cell(value);
                }
                if let Some(value) = self.enum_value_from_path(&path)? {
                    return self.allocate_cell(value);
                }
                self.resolve_place(expression, frame)
            }
            ExpressionNode::Member(member) => {
                if let Some(value) = self.read_mutable_record_recast_target(expression, frame)? {
                    return self.allocate_cell(value);
                }
                // Destructuring reuses the subject already observed by its
                // guard; a copied call expression must not invoke it again.
                if member.case_variant.is_some() {
                    let observed = frame
                        .guard_call_results
                        .borrow()
                        .iter()
                        .find(|(subject, _)| {
                            self.program
                                .expression_table
                                .expressions_structurally_equal(*subject, member.receiver)
                        })
                        .map(|(_, value)| value.clone());
                    if let Some(observed) = observed {
                        let receiver = self.allocate_cell(observed)?;
                        return self.field_cell(&receiver, member.member.as_str());
                    }
                }
                let receiver = self.eval_read_cell(member.receiver, frame)?;
                self.field_cell(&receiver, member.member.as_str())
            }
            ExpressionNode::Indexed(indexed) => {
                if let Some(value) = self.read_mutable_record_recast_target(expression, frame)? {
                    return self.allocate_cell(value);
                }
                if let ExpressionNode::Range(range) =
                    self.program.expression_table.expression(indexed.index)
                {
                    let range = *range;
                    let value = self.eval_subslice(indexed.collection, &range, frame)?;
                    return self.allocate_cell(value);
                }
                self.require_builtin_index(expression, &indexed, frame)?;
                // Establish the collection before evaluating the selector.
                // Neither a selection failure nor a later consumer retries it.
                let collection = self.eval_read_cell(indexed.collection, frame)?;
                let collection = self.deref_cell(collection);
                let index = self.eval_index(indexed.index, frame)?;
                if let Value::Str(text) = &*collection.borrow() {
                    let byte =
                        text.borrow().get(index).copied().ok_or_else(|| {
                            Halt::Trap(format!("string index {index} out of bounds"))
                        })?;
                    return self.allocate_cell(Value::Int(i64::from(byte)));
                }
                self.element_cell(&collection, index)
            }
            _ => {
                let value = self.eval_expression(expression, frame)?;
                self.allocate_cell(value)
            }
        }
    }

    fn require_builtin_index(
        &self,
        expression: ExpressionHandle,
        indexed: &typed_trees::expression::TableIndexedExpression,
        frame: &Frame,
    ) -> EvalResult<()> {
        let operands = [
            self.expression_type_reference(indexed.collection, frame),
            // A literal has no declared selector type. Its eventual numeric
            // landing cannot retroactively remove an authored index candidate.
            self.expression_type_reference(indexed.index, frame),
        ];
        if !typed_trees::operator::resolve_indexed_spelling_for_operands(
            self.program,
            language_core::OperatorSpelling::Index,
            &operands,
        )
        .is_empty()
            || !typed_trees::operator::has_builtin_spelled_expression_meaning(
                self.program,
                frame.machine_symbol,
                expression,
                language_core::OperatorSpelling::Index,
                &operands,
            )
        {
            return unsupported("array value projection has no exact builtin indexing meaning");
        }
        Ok(())
    }
}
