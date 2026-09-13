//! Retained scalar values share selective evaluation, whether they establish
//! a source local or one array leaf. Completed values survive later control
//! joins; the fixed source prefix excludes private argument and leaf slots.

use super::*;

impl Evaluation {
    /// Guards share the same selective source evaluator as initializers and
    /// arguments. Selection chooses retained evidence, never a Boolean spelling.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn guard_value(
        &mut self,
        checked: &CheckedTrees,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        statement: u32,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
    ) -> Result<ValueDeclaration, LoweringError> {
        let role = CheckedScalarExpressionRole::Guard;
        let mut computations = checked
            .facts
            .values
            .scalar_computations
            .roots
            .iter()
            .map(|(_, root)| root)
            .filter(|root| {
                root.state == state && root.statement_ordinal == statement && root.role == role
            });
        let value = if let Some(root) = computations.next() {
            if computations.next().is_some() || root.machine != machine {
                return unsupported("guard computation has no unique source owner");
            }
            CheckedCallScalarArgument::Computation(root.root)
        } else {
            let (_, value) = checked
                .facts
                .values
                .scalar_expressions
                .bound_expression_at(state, statement, role)
                .ok_or(LoweringError::Unsupported(
                    "guard has no retained source value",
                ))?;
            CheckedCallScalarArgument::Pure(value.clone())
        };
        let value = self.source_value(
            checked,
            machine,
            state,
            statement,
            role,
            &value,
            values.len(),
            values,
            next_value,
            next_block,
            next_edge,
            operations,
            calls,
        )?;
        if value.scalar_type != ScalarType::Boolean {
            return unsupported("guard source value is not Boolean");
        }
        Ok(value)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn source_value(
        &mut self,
        checked: &CheckedTrees,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        statement: u32,
        role: CheckedScalarExpressionRole,
        value: &CheckedCallScalarArgument,
        source_value_count: usize,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
    ) -> Result<ValueDeclaration, LoweringError> {
        if source_value_count > values.len() {
            return unsupported("scalar source prefix exceeds its retained values");
        }
        let source = crate::scalar_source_custody::locate(checked, state, statement, role)?;
        let scalar_type = terminal_scalar_type(source.primitive_type)?;
        if matches!(
            role,
            CheckedScalarExpressionRole::ReturnCaseField { .. }
                | CheckedScalarExpressionRole::StructuralValueField { .. }
        ) {
            crate::scalar_source_custody::value_correspondence::validate(
                checked,
                state,
                statement,
                source.expression,
                source.primitive_type,
                value,
            )?;
        }
        match value {
            CheckedCallScalarArgument::Pure(value) => {
                let (binding, retained) = checked
                    .facts
                    .values
                    .scalar_expressions
                    .bound_expression_at(state, statement, role)
                    .ok_or(LoweringError::Unsupported(
                        "scalar value lost its pure source binding",
                    ))?;
                if retained != value {
                    return unsupported("scalar value differs from its retained source binding");
                }
                crate::scalar_source_custody::validate_pure(checked, binding, scalar_type)?;
            }
            CheckedCallScalarArgument::Computation(handle) => {
                let mut roots = checked
                    .facts
                    .values
                    .scalar_computations
                    .roots
                    .iter()
                    .map(|(_, root)| root)
                    .filter(|root| {
                        root.state == state
                            && root.statement_ordinal == statement
                            && root.role == role
                    });
                let root = roots.next().ok_or(LoweringError::Unsupported(
                    "scalar value lost its computation root",
                ))?;
                if roots.next().is_some() || root.machine != machine || root.root != *handle {
                    return unsupported("scalar value differs from its computation root");
                }
            }
        }
        let bindings = self
            .scalar_bindings
            .clone()
            .unwrap_or_else(|| crate::scalar_bindings::ScalarBindings::new(source_value_count))
            .with_primitive_storage(&self.primitive_storage)
            .with_local_cases(&self.local_cases)
            .with_structural_locals(&self.structural_locals)
            .with_structural_parameters(&self.structural_parameters)
            .with_resolved_structural_observations(&self.structural_fields, &self.structural_cases);
        let qualifications = prepare_shared_qualifications(checked, machine, values)?;
        let source_types = values
            .iter()
            .map(|value| value.value_type())
            .collect::<Vec<_>>();
        if let CheckedCallScalarArgument::Pure(value) = value {
            let expression = bindings.expression(value)?;
            if expression.scalar_type() != scalar_type {
                return unsupported("scalar value type differs from its destination carrier");
            }
            if !direct_expression_contains_short_circuit(&expression) {
                validate_direct_parameter_types(&expression, &scalar_carriers(&source_types))?;
                return Ok(ValueDeclaration {
                    qualifications: Default::default(),
                    id: emit_direct_expression(&expression, values, next_value, operations),
                    scalar_type,
                });
            }
        }
        let mut expansion =
            crate::scalar_computations::Expansion::new(checked, &qualifications, machine, 1)
                .with_arrays(&self.arrays)
                .with_cases(&self.cases)
                .with_fields(&self.record_fields);
        let entry = match value {
            CheckedCallScalarArgument::Pure(_) => expansion.retained_pure_value(
                state,
                statement,
                role,
                &bindings,
                &source_types,
                scalar_type.into(),
                0,
            )?,
            CheckedCallScalarArgument::Computation(_) => expansion.retained_value(
                state,
                statement,
                role,
                if matches!(role, CheckedScalarExpressionRole::LocalInitializer { .. }) {
                    symbols::SymbolHandle::invalid()
                } else {
                    source.destination
                },
                &bindings,
                &source_types,
                scalar_type.into(),
                0,
            )?,
        };
        let states = expansion.finish();
        let completed = self.complete_expansion(
            &states,
            entry,
            &[scalar_type.into()],
            values,
            next_value,
            next_block,
            next_edge,
            operations,
            calls,
        )?;
        match completed.as_slice() {
            [value] => Ok(*value),
            _ => unsupported("scalar source has no single completed value"),
        }
    }
}
