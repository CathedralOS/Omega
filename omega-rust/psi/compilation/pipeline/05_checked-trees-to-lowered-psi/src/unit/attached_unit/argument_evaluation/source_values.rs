//! Retained scalar values share selective evaluation, whether they establish
//! a source local or one array leaf. Completed values survive later control
//! joins; the fixed source prefix excludes private argument and leaf slots.
//! Completion carries the produced value's full qualified type. Projecting
//! only its primitive here would discard an authored qualification after the
//! computation established it, or reject a correctly qualified call result.
use super::{
    CheckedCallScalarArgument, CheckedScalarExpressionRole, CheckedTrees, Evaluation,
    LoweringError, ScalarType, ValueDeclaration, direct_expression_contains_short_circuit,
    emit_direct_expression, scalar_carriers, terminal_scalar_type, unsupported,
    validate_direct_parameter_types,
};
use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;

impl Evaluation {
    /// Consume an already retained computation over one captured formal frame.
    /// A fresh Expansion owns completion caches for this activation only: the
    /// same observation root at entry and on another arrival has new actuals.
    pub(crate) fn retained_observation(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        root: &checked_trees::CheckedScalarComputationRoot,
        bindings: &crate::expression_preparation::bindings::ScalarBindings,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
    ) -> Result<ValueDeclaration, LoweringError> {
        let source_types = values
            .iter()
            .map(|value| value.value_type())
            .collect::<Vec<_>>();
        let result_type = crate::scalar_lowering::computations::computation_value_type(
            identities,
            checked,
            &self.shared_qualifications,
            root.root,
            bindings,
            &source_types,
        )?;
        let mut expansion = self.scalar_computation_expansion(
            identities,
            checked,
            &self.shared_qualifications,
            root.machine,
            operations,
        )?;
        let entry =
            expansion.retained_root(identities, root, bindings, &source_types, result_type, 0)?;
        let states = expansion.finish();
        self.single_completed_expansion(
            identities,
            checked,
            &states,
            entry,
            result_type,
            values,
            next_value,
            next_block,
            next_edge,
            operations,
            calls,
            "retained observation has no single completed value",
        )
    }

    /// A pure Boolean used only to choose an edge needs no joined value.
    /// Merging its outcomes before branching would discard predecessor bounds
    /// that the selected body's independent operation proofs still need.
    /// Effectful computations keep the existing selective value evaluator.
    pub(crate) fn branch_guard(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        state: symbols::SymbolHandle,
        statement: u32,
        values: &[ValueDeclaration],
        successor_reads_payloads: bool,
    ) -> Result<Option<LoweredBooleanReturnExpression>, LoweringError> {
        let role = CheckedScalarExpressionRole::Guard;
        if checked
            .facts
            .values
            .scalar_computations
            .roots
            .iter()
            .any(|(_, root)| {
                root.state == state && root.statement_ordinal == statement && root.role == role
            })
        {
            return Ok(None);
        }
        let (_, value) = checked
            .facts
            .values
            .scalar_expressions
            .bound_expression_at(state, statement, role)
            .ok_or(LoweringError::Unsupported(
                "guard has no retained source value",
            ))?;
        let bindings = self.contextual_bindings();
        let expression =
            bindings.expression(identities, &checked.facts.proven_scalar_indexes, value)?;
        // A short-circuit guard is always a planned decision. So is a whole-root
        // case test whose successor may read that case's payload: only the
        // decision's dispatch establishes the payload for it.
        if !direct_expression_contains_short_circuit(&expression)
            && !(successor_reads_payloads && direct_expression_tests_whole_root_case(&expression))
        {
            return Ok(None);
        }
        validate_direct_parameter_types(
            &expression,
            &values
                .iter()
                .map(|value| value.scalar_type)
                .collect::<Vec<_>>(),
        )?;
        let LoweredDirectExpression::Boolean { expression } = expression else {
            return unsupported("branch guard source value is not Boolean");
        };
        Ok(Some(*expression))
    }

    /// Guards share the same selective source evaluator as initializers and
    /// arguments. Selection chooses retained evidence, never a Boolean spelling.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn guard_value(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
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
            identities,
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
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
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
        // The value's own plan carries its carrier: a pure expression names
        // its primitive type, and a computation root its node's.
        let primitive_type = match value {
            CheckedCallScalarArgument::Pure(expression) => expression.primitive_type(),
            CheckedCallScalarArgument::Computation(handle) => {
                let nodes = &checked.facts.values.scalar_computations.nodes;
                nodes
                    .is_valid(*handle)
                    .then(|| nodes.get(*handle).primitive_type)
            }
        }
        .ok_or(LoweringError::Unsupported(
            "scalar value plan has no primitive carrier",
        ))?;
        let scalar_type = terminal_scalar_type(primitive_type)?;
        match value {
            CheckedCallScalarArgument::Pure(_) => {
                if checked
                    .facts
                    .values
                    .scalar_expressions
                    .bound_expression_at(state, statement, role)
                    .is_none()
                {
                    return unsupported("scalar value lost its pure source binding");
                }
            }
            CheckedCallScalarArgument::Computation(_) => {
                if !checked
                    .facts
                    .values
                    .scalar_computations
                    .roots
                    .iter()
                    .any(|(_, root)| {
                        root.state == state
                            && root.statement_ordinal == statement
                            && root.role == role
                    })
                {
                    return unsupported("scalar value lost its computation root");
                }
            }
        }
        let bindings = self
            .scalar_bindings
            .clone()
            .with_primitive_storage(&self.primitive_storage)
            .with_structural_locals(&self.structural_locals)
            .with_view_locals(&self.view_locals)
            .with_element_views(&self.element_views)
            .with_structural_parameters(&self.structural_parameters)
            .with_resolved_structural_observations(&self.structural_fields, &self.structural_cases)
            .with_place_observations(&self.place_observations);
        let qualifications = &self.shared_qualifications;
        let source_types = values
            .iter()
            .map(|value| value.value_type())
            .collect::<Vec<_>>();
        let (pure_expression, result_type) = match value {
            CheckedCallScalarArgument::Pure(value) => {
                let expression =
                    bindings.expression(identities, &checked.facts.proven_scalar_indexes, value)?;
                let result_type = expression.value_type(&source_types)?;
                (Some(expression), result_type)
            }
            CheckedCallScalarArgument::Computation(handle) => (
                None,
                crate::scalar_lowering::computations::computation_value_type(
                    identities,
                    checked,
                    qualifications,
                    *handle,
                    &bindings,
                    &source_types,
                )?,
            ),
        };
        if result_type.scalar_type != scalar_type {
            return unsupported("scalar source value disagrees with its retained carrier");
        }
        if let Some(expression) = pure_expression
            && !direct_expression_contains_short_circuit(&expression)
        {
            validate_direct_parameter_types(&expression, &scalar_carriers(&source_types))?;
            return Ok(ValueDeclaration {
                qualifications: result_type.qualifications,
                id: emit_direct_expression(&expression, values, next_value, operations),
                scalar_type,
            });
        }
        let mut expansion = self.scalar_computation_expansion(
            identities,
            checked,
            qualifications,
            machine,
            operations,
        )?;
        let entry = match value {
            CheckedCallScalarArgument::Pure(_) => expansion.retained_pure_value(
                identities,
                state,
                statement,
                role,
                &bindings,
                &source_types,
                result_type,
                0,
            )?,
            CheckedCallScalarArgument::Computation(_) => expansion.retained_value(
                identities,
                state,
                statement,
                role,
                &bindings,
                &source_types,
                result_type,
                0,
            )?,
        };
        let states = expansion.finish();
        self.single_completed_expansion(
            identities,
            checked,
            &states,
            entry,
            result_type,
            values,
            next_value,
            next_block,
            next_edge,
            operations,
            calls,
            "scalar source has no single completed value",
        )
    }
}

/// Whether a Boolean guard tests a case of a whole structural root anywhere.
fn direct_expression_tests_whole_root_case(expression: &LoweredDirectExpression) -> bool {
    fn tests(expression: &LoweredBooleanReturnExpression) -> bool {
        match expression {
            LoweredBooleanReturnExpression::StructuralCaseMembership { path, .. } => {
                path.is_empty()
            }
            LoweredBooleanReturnExpression::Not { operand } => tests(operand),
            LoweredBooleanReturnExpression::Equal { left, right } => tests(left) || tests(right),
            LoweredBooleanReturnExpression::And { left, right }
            | LoweredBooleanReturnExpression::Or { left, right } => tests(left) || tests(right),
            _ => false,
        }
    }
    matches!(expression, LoweredDirectExpression::Boolean { expression } if tests(expression))
}
