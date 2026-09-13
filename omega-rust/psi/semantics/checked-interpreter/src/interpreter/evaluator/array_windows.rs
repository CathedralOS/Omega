use super::*;

impl Evaluator<'_> {
    /// Replace a window's exact element footprint without rebinding its backing
    /// array or observing the previous element values. Source checking owns
    /// write permission and admissible range shapes; execution checks bounds
    /// again and never clamps an invalid replacement into a different window.
    pub(super) fn assign_array_window(
        &mut self,
        target: ExpressionHandle,
        value: ExpressionHandle,
        frame: &Frame,
    ) -> EvalResult<bool> {
        let ExpressionNode::Indexed(indexed) = self.program.expression_table.expression(target)
        else {
            return Ok(false);
        };
        let ExpressionNode::Range(range) = self.program.expression_table.expression(indexed.index)
        else {
            return Ok(false);
        };
        if !matches!(
            self.program.expression_table.expression(value),
            ExpressionNode::ArrayLiteral(_)
        ) {
            return Ok(false);
        }
        let collection = indexed.collection;
        let range = *range;
        let element_type = self
            .expression_type_reference(collection, frame)
            .and_then(|collection_type| self.collection_element_type(collection_type))
            .ok_or_else(|| {
                Halt::Unsupported("array window lost its element destination".to_owned())
            })?;
        let Value::Array(source) =
            self.eval_array_literal_at_element_type(value, element_type, frame)?
        else {
            return unsupported("array window replacement lost its literal elements");
        };
        // Match ordinary assignment's RHS-before-place evaluation. Materialize
        // all replacement values before touching any destination cell.
        let replacements = source
            .iter()
            .map(|cell| {
                cell.borrow()
                    .deep_clone_with(&|value| self.allocate_cell(value))
            })
            .collect::<EvalResult<Vec<_>>>()?;
        let target = self.resolve_place(collection, frame)?;
        let target = self.deref_cell(target);
        let target_length = match &*target.borrow() {
            Value::Array(elements) => elements.len(),
            Value::Str(text) => text.borrow().len(),
            _ => return unsupported("array window requires array or byte-carrier storage"),
        };
        let start = if range.start.is_valid() {
            self.eval_index(range.start, frame)?
        } else {
            0
        };
        let end = if range.end.is_valid() {
            self.eval_index(range.end, frame)?
        } else {
            target_length
        };
        let end = if range.end_inclusive {
            end.checked_add(1)
                .ok_or_else(|| Halt::Trap("inclusive array window bound overflow".to_owned()))?
        } else {
            end
        };
        if start > end || end > target_length {
            return trap("array window replacement is out of bounds");
        }
        if replacements.len() != end - start {
            return trap("array window replacement has a different element count");
        }
        match &*target.borrow() {
            Value::Array(targets) => {
                for (target, value) in targets[start..end].iter().zip(replacements) {
                    *target.borrow_mut() = value;
                }
            }
            Value::Str(text) => {
                let bytes = replacements
                    .into_iter()
                    .map(|value| {
                        value
                            .as_int()
                            .and_then(|integer| u8::try_from(integer).ok())
                            .ok_or_else(|| {
                                Halt::Trap("byte window replacement is not a byte".to_owned())
                            })
                    })
                    .collect::<EvalResult<Vec<_>>>()?;
                for (position, byte) in (start..end).zip(bytes) {
                    text.write_byte(position, byte).map_err(|_| {
                        Halt::Trap("byte window replacement is out of bounds".to_owned())
                    })?;
                }
            }
            _ => return unsupported("array window lost its backing storage"),
        }
        Ok(true)
    }
}
