//! Scalar and sequence-length observations share declaration geometry with pure reads.
//! Each selector is an ordinary computation operand of a storage observation.
//! Its collection carrier is retained beside it so lowering can capture that
//! collection before running the selector, including at nested boundaries.
//! The indexing occurrence still owns its source-checked bound; an operand
//! result supplies only bits, never a substitute geometry-only evaluation.
use super::{
    ArithmeticDomain, Builder, CheckedScalarComputationHandle, CheckedScalarComputationKind,
    CheckedScalarExpression, ExpressionHandle, PrimitiveType,
};
use crate::values::scalar::expression_facts::{is_integer, scalar_expression_type};

impl Builder<'_, '_> {
    pub(super) fn indexed_read(
        &mut self,
        expression: ExpressionHandle,
    ) -> Option<(
        CheckedScalarComputationHandle,
        ArithmeticDomain,
        PrimitiveType,
    )> {
        let program = self.program;
        let operators = self.operators;
        let parameters = self.authored_parameters;
        let mut operands = Vec::new();
        let mut collections = Vec::new();
        let mut selector = |_, selector, collection| {
            let operand = self.integer_operand(selector, Some(PrimitiveType::U64))?;
            let primitive_type = scalar_expression_type(&operand.value)?;
            if !is_integer(primitive_type) || primitive_type == PrimitiveType::Addr {
                return None;
            }
            let position = operands.len();
            operands.push(self.materialize_integer(operand)?);
            collections.push(collection);
            Some(CheckedScalarExpression::Parameter {
                position,
                primitive_type,
            })
        };
        let (template, domain) = if program
            .expression_table
            .authored_length_receiver(expression)
            .is_some()
        {
            (
                crate::values::scalar::structural_fields::structural_sequence_length(
                    program,
                    operators,
                    parameters,
                    expression,
                    &mut selector,
                )?,
                ArithmeticDomain::Exact,
            )
        } else {
            crate::values::scalar::scalar_lowering::scalar_storage_read_with_selectors(
                program, operators, expression, parameters, selector,
            )?
        };
        let root = match &template {
            CheckedScalarExpression::StructuralParameterIndexedRead { root, .. }
            | CheckedScalarExpression::StructuralSequenceLength { root, .. } => Some(*root),
            _ => None,
        };
        let root_type = match root {
            Some(checked_trees::CheckedStorageRoot::Parameter { index }) => {
                self.authored_parameters.get(index as usize)?.type_reference
            }
            Some(
                checked_trees::CheckedStorageRoot::OwnedLocal { symbol }
                | checked_trees::CheckedStorageRoot::ViewLocal { symbol },
            ) => {
                let positions = program.local_data_positions_for_symbol(symbol);
                let [position] = positions.as_slice() else {
                    return None;
                };
                let machine = &program.machines()[position.0 as usize];
                let state = &program.machine_states(machine)[position.1 as usize];
                program
                    .statement_table
                    .statements(state.statement_nodes)
                    .get(position.2 as usize)?
                    .as_local_data()?
                    .type_reference
            }
            None => typed_trees::types::TypeReferenceHandle::invalid(),
        };
        let primitive_type = scalar_expression_type(&template)?;
        let operands = self.plans.operands.insert_many(operands);
        let collections = self.plans.observation_collections.insert_many(collections);
        let computation = self.insert(
            primitive_type,
            CheckedScalarComputationKind::StorageObservation {
                root_type,
                source_expression: expression,
                expression: template,
                operands,
                collections,
            },
        );
        Some((computation, domain, primitive_type))
    }
}
