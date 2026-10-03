//! Expected types determine both scalar landing and operand ownership. Capture
//! owned operands before proceeding to the next authored expression; borrowed
//! destinations and Ref values keep their existing storage relationships.

use crate::interpreter::evaluator::{
    EvalResult, EvaluatedArgument, Evaluator, ExpressionHandle, ExpressionNode, Frame,
    PrimitiveType, State, TypeReferenceHandle, TypeReferenceNode, Value, unsupported,
};
impl Evaluator<'_> {
    /// Preserve aggregate destinations until each literal element reaches its
    /// own scalar type, and reference destinations until their alias is retained.
    /// A scalar observation must not replace a reference-valued field or result.
    pub(in crate::interpreter::evaluator) fn eval_expression_at_type(
        &mut self,
        expression: ExpressionHandle,
        destination: TypeReferenceHandle,
        frame: &mut Frame,
    ) -> EvalResult<Value> {
        if let ExpressionNode::Match(dispatch) =
            self.program.expression_table.expression(expression).clone()
        {
            self.tick()?;
            let selected = self.select_match_arm(expression, &dispatch, frame)?;
            return self.eval_expression_at_type(selected, destination, frame);
        }
        if matches!(
            self.program
                .type_reference_table
                .type_reference(destination),
            TypeReferenceNode::Reference { .. }
        ) {
            // A reference result/field retains the selected Ref, not a scalar
            // observation of its referent. The same selection serves arguments.
            let cell = self.eval_read_cell(expression, frame)?;
            return Ok(cell.borrow().clone());
        }
        if let ExpressionNode::ArrayLiteral(_) =
            self.program.expression_table.expression(expression)
            && let TypeReferenceNode::FixedArray { element_type, .. } = self
                .program
                .type_reference_table
                .type_reference(destination)
        {
            return self.eval_array_literal_at_element_type(expression, *element_type, frame);
        }
        let primitive = self.program.primitive_type_reference(destination);
        self.eval_expression_with_destination(expression, primitive, frame)
    }

    pub(in crate::interpreter::evaluator) fn eval_array_literal_at_element_type(
        &mut self,
        expression: ExpressionHandle,
        element_type: TypeReferenceHandle,
        frame: &mut Frame,
    ) -> EvalResult<Value> {
        let ExpressionNode::ArrayLiteral(elements) =
            self.program.expression_table.expression(expression)
        else {
            return unsupported("array element destination requires an array literal");
        };
        let elements = *elements;
        self.tick()?;
        let mut values = Vec::new();
        for element in self.program.expression_table.expression_handles(elements) {
            let value = self.eval_expression_at_type(*element, element_type, frame)?;
            let value = self.capture_value_operand(value, element_type)?;
            values.push(self.allocate_cell(value)?);
        }
        Ok(Value::Array(values))
    }

    /// Establish each value operand before the next operand can mutate
    /// its source. Waiting for the enclosing assignment's value copy is too
    /// late: ordinary Value clones retain the original aggregate cells.
    /// Calls and record, case, and array construction share this rule regardless of the
    /// expression that produced the operand. References retain their referent;
    /// an Array carrier is owned only at a fixed-array destination, not a view.
    pub(in crate::interpreter::evaluator) fn capture_value_operand(
        &self,
        value: Value,
        destination: TypeReferenceHandle,
    ) -> EvalResult<Value> {
        match self
            .program
            .type_reference_table
            .type_reference(destination)
        {
            TypeReferenceNode::Constrained { base_type, .. } => {
                return self.capture_value_operand(value, *base_type);
            }
            // Implicit shared lending may carry the aggregate without a Ref
            // wrapper. Its declared access still forbids taking an owned copy.
            TypeReferenceNode::Reference { .. } => return Ok(value),
            _ => {}
        }
        let owned_aggregate = matches!(value, Value::Struct { .. } | Value::Enum { .. })
            || (matches!(value, Value::Array(_)) && self.declared_type_is_fixed_array(destination));
        if owned_aggregate {
            value.deep_clone_with(&|value| self.allocate_cell(value))
        } else {
            Ok(value)
        }
    }

    /// Use the compiler's exact anonymous value at the actual destination.
    /// Typed operands, places, calls, and authored operators keep ordinary
    /// execution; a destination never changes their operation semantics.
    pub(in crate::interpreter::evaluator) fn eval_expression_with_destination(
        &mut self,
        expression: ExpressionHandle,
        destination: Option<PrimitiveType>,
        frame: &mut Frame,
    ) -> EvalResult<Value> {
        self.eval_expression_with_destination_and_carrier(expression, destination, frame)
            .map(|(value, _)| value)
    }

    /// Casts need the source carrier as well as its bits. A destination supplies
    /// that carrier only when anonymous landing actually succeeds; typed values
    /// retain their own signedness. Select match arms once, before recovering
    /// their carrier, so anonymous arms use the same landing path as literals.
    pub(in crate::interpreter::evaluator) fn eval_expression_with_destination_and_carrier(
        &mut self,
        expression: ExpressionHandle,
        destination: Option<PrimitiveType>,
        frame: &mut Frame,
    ) -> EvalResult<(Value, Option<PrimitiveType>)> {
        if let ExpressionNode::Match(dispatch) =
            self.program.expression_table.expression(expression).clone()
        {
            self.tick()?;
            let selected = self.select_match_arm(expression, &dispatch, frame)?;
            return self.eval_expression_with_destination_and_carrier(selected, destination, frame);
        }
        if let Some(value) = self.anonymous_integer_landing_value(expression, destination)? {
            return Ok((value, destination));
        }
        let source = self
            .expression_scalar_type(expression, frame)
            .map(|(primitive, _)| primitive)
            .or_else(|| {
                self.expression_type_reference(expression, frame)
                    .and_then(|reference| self.program.primitive_type_reference(reference))
            });
        self.eval_expression(expression, frame)
            .map(|value| (value, source))
    }

    pub(in crate::interpreter::evaluator) fn anonymous_integer_landing_value(
        &mut self,
        expression: ExpressionHandle,
        destination: Option<PrimitiveType>,
    ) -> EvalResult<Option<Value>> {
        if let Some(destination) = destination.filter(|primitive| {
            matches!(
                primitive,
                PrimitiveType::I8
                    | PrimitiveType::I16
                    | PrimitiveType::I32
                    | PrimitiveType::I64
                    | PrimitiveType::U8
                    | PrimitiveType::U16
                    | PrimitiveType::U32
                    | PrimitiveType::U64
            )
        }) && let Some(literal) = validation::land_anonymous_integer_expression(
            self.program,
            expression,
            destination,
            |expression| match self
                .operator_facts
                .and_then(|facts| facts.expression_use(expression))
            {
                Some(operator) => {
                    operator.status
                        == checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
                }
                None => validation::has_anonymous_operator_meaning(self.program, expression),
            },
        ) && let Some(bits) = literal.bits_u64()
        {
            // Preserve source-expression fuel accounting even though the
            // exact calculation is shared with compile-time landing.
            let mut pending = vec![expression];
            while let Some(expression) = pending.pop() {
                self.tick()?;
                if let ExpressionNode::Binary(binary) =
                    self.program.expression_table.expression(expression)
                {
                    pending.push(binary.right);
                    pending.push(binary.left);
                }
            }
            return Ok(Some(Value::Int(bits as i64)));
        }
        Ok(None)
    }

    pub(in crate::interpreter::evaluator) fn eval_state_arguments(
        &mut self,
        state: &State,
        arguments: &[ExpressionHandle],
        frame: &mut Frame,
    ) -> EvalResult<Vec<EvaluatedArgument>> {
        self.check_state_argument_count(state, arguments.len())?;
        let mut evaluated = Vec::with_capacity(arguments.len());
        for (ordinal, argument) in arguments.iter().copied().enumerate() {
            let destination = self.program.state_parameters(state)[ordinal].type_reference;
            let argument = self.eval_state_argument(argument, destination, frame)?;
            evaluated.push(argument);
        }
        Ok(evaluated)
    }

    /// Evaluation and frame binding consume the same exact formal roster.
    /// No parameter may be omitted or supplied from ambient instance storage.
    pub(in crate::interpreter::evaluator) fn check_state_argument_count(
        &self,
        state: &State,
        argument_count: usize,
    ) -> EvalResult<()> {
        let parameters = self.program.state_parameters(state);
        if argument_count == parameters.len() {
            return Ok(());
        }
        unsupported(format!(
            "call to `{}` supplies {argument_count} arguments for {} declared parameters",
            state.name.as_str(),
            parameters.len(),
        ))
    }
}
