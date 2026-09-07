use super::*;

impl Evaluator<'_> {
    /// Use the compiler's exact anonymous value at the actual destination.
    /// Typed operands, places, calls, and authored operators keep ordinary
    /// execution; a destination never changes their operation semantics.
    pub(super) fn eval_expression_with_destination(
        &mut self,
        expression: ExpressionHandle,
        destination: Option<PrimitiveType>,
        frame: &Frame,
    ) -> EvalResult<Value> {
        if let Some(value) = self.anonymous_integer_landing_value(expression, destination)? {
            return Ok(value);
        }
        self.eval_expression(expression, frame)
    }

    pub(super) fn anonymous_integer_landing_value(
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

    pub(super) fn eval_state_arguments(
        &mut self,
        machine: &Machine,
        state_name: &str,
        arguments: &[ExpressionHandle],
        frame: &Frame,
    ) -> EvalResult<Vec<EvaluatedArgument>> {
        let state = self.find_state(machine, state_name).ok_or_else(|| {
            Halt::Unsupported(format!("unknown argument destination `{state_name}`"))
        })?;
        let destinations: Vec<_> = self
            .program
            .state_parameters(state)
            .iter()
            .filter(|parameter| !parameter.is_self)
            .map(|parameter| {
                self.program
                    .primitive_type_reference(parameter.type_reference)
            })
            .collect();
        let mut evaluated = Vec::with_capacity(arguments.len());
        for (ordinal, argument) in arguments.iter().copied().enumerate() {
            evaluated.push(self.eval_state_argument(
                argument,
                destinations.get(ordinal).copied().flatten(),
                frame,
            )?);
        }
        Ok(evaluated)
    }
}
