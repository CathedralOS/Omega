//! Array elements finish in index order before construction commits. Completed
//! leaves survive later calls and selections in private scalar slots, outside
//! the fixed source namespace; those slots never become authored local bindings.

use super::*;

impl Evaluation {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn array_element(
        &mut self,
        checked: &CheckedTrees,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        statement: u32,
        element_ordinal: u32,
        element: &CheckedCallScalarArgument,
        source_value_count: usize,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
    ) -> Result<ValueDeclaration, LoweringError> {
        if source_value_count > values.len() {
            return unsupported("array source prefix exceeds its retained values");
        }
        let role = CheckedScalarExpressionRole::ArrayElement { element_ordinal };
        let source = crate::scalar_source_custody::locate(checked, state, statement, role)?;
        let scalar_type = terminal_scalar_type(source.primitive_type)?;
        let bindings = self
            .scalar_bindings
            .clone()
            .unwrap_or_else(|| crate::scalar_bindings::ScalarBindings::new(source_value_count))
            .with_primitive_storage(&self.primitive_storage)
            .with_structural_parameters(&self.structural_parameters)
            .with_resolved_structural_fields(&self.structural_fields);
        let source_types = values
            .iter()
            .map(|value| value.scalar_type)
            .collect::<Vec<_>>();
        if let CheckedCallScalarArgument::Pure(value) = element {
            let expression = bindings.expression(value)?;
            if expression.scalar_type() != scalar_type {
                return unsupported("array operand type differs from its destination carrier");
            }
            if !direct_expression_contains_short_circuit(&expression) {
                validate_direct_parameter_types(&expression, &source_types)?;
                return Ok(ValueDeclaration {
                    id: emit_direct_expression(&expression, values, next_value, operations),
                    scalar_type,
                });
            }
        }
        let mut expansion = crate::scalar_computations::Expansion::new(checked, machine, 1);
        let entry = match element {
            CheckedCallScalarArgument::Pure(_) => expansion.retained_pure_value(
                state,
                statement,
                role,
                &bindings,
                &source_types,
                scalar_type,
                0,
            )?,
            CheckedCallScalarArgument::Computation(_) => expansion.retained_value(
                state,
                statement,
                role,
                source.destination,
                &bindings,
                &source_types,
                scalar_type,
                0,
            )?,
        };
        let states = expansion.finish();
        let completed = self.complete_expansion(
            &states,
            entry,
            &[scalar_type],
            values,
            next_value,
            next_block,
            next_edge,
            operations,
            calls,
        )?;
        match completed.as_slice() {
            [value] => Ok(*value),
            _ => unsupported("array element has no single completed scalar value"),
        }
    }
}
