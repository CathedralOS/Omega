//! Collapse a prepared scalar chain without creating artificial control edges.
//!
//! This optional emission choice inspects the complete prepared graph before
//! touching the operation buffer. Source evaluation and selected operation
//! meaning stay with Expansion and the ordinary scalar binding emitter. A
//! selective, ownership-bearing or qualification-changing edge uses the existing
//! block emitter instead; no partial inline prefix escapes that decision.
use super::{
    Evaluation, LoweringError, QualifiedScalarType, ValueDeclaration,
    direct_expression_contains_short_circuit, emit_direct_expression, unsupported,
    validate_direct_parameter_types,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::emission::operation_emission::{LoweredScalarBinding, emit_scalar_binding};
use crate::scalar_lowering::branch_states::prepared::{
    LoweredScalarBranchState, LoweredScalarBranchTerminator,
};

impl Evaluation {
    pub(super) fn complete_straight_line(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &checked_trees::CheckedTrees,
        states: &[LoweredScalarBranchState],
        entry: usize,
        result_types: &[QualifiedScalarType],
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
    ) -> Result<Option<Vec<ValueDeclaration>>, LoweringError> {
        let source_count = values.len();
        let mut chain = Vec::new();
        let mut index = entry;
        let mut types = values
            .iter()
            .map(|value| value.value_type())
            .collect::<Vec<_>>();
        let proof_forwarding = self.proof_formal_forwarding();
        while index != 0 {
            if chain.contains(&index) {
                return Ok(None);
            }
            let state = states.get(index - 1).ok_or(LoweringError::Unsupported(
                "scalar computation chain lost its continuation",
            ))?;
            let LoweredScalarBranchTerminator::Jump {
                target,
                arguments,
                erased_arguments,
                erased_proof_arguments,
                structural_arguments,
                cleanup_actions,
            } = &state.terminator
            else {
                return Ok(None);
            };
            if !state.structural_parameters.is_empty()
                || !state.structural_effects.is_empty()
                || !state.erased_formal_types.is_empty()
                || !erased_arguments.is_empty()
                || *erased_proof_arguments != proof_forwarding
                || !structural_arguments.is_empty()
                || !cleanup_actions.is_empty()
                || state.parameter_types != types
            {
                return Ok(None);
            }
            for binding in &state.bindings {
                match binding {
                    LoweredScalarBinding::Expression(expression)
                        if !direct_expression_contains_short_circuit(expression) => {}
                    LoweredScalarBinding::SelectedComparison { left, right, .. }
                        if !direct_expression_contains_short_circuit(left)
                            && !direct_expression_contains_short_circuit(right) => {}
                    _ => return Ok(None),
                }
                types.push(binding.value_type(&types)?);
            }
            if arguments
                .iter()
                .any(direct_expression_contains_short_circuit)
            {
                return Ok(None);
            }
            types = arguments
                .iter()
                .map(|value| value.value_type(&types))
                .collect::<Result<Vec<_>, _>>()?;
            chain.push(index);
            index = *target;
        }
        let mut completion_types = values
            .iter()
            .map(|value| value.value_type())
            .collect::<Vec<_>>();
        completion_types.extend_from_slice(result_types);
        if types != completion_types {
            return unsupported("scalar computation chain changes its completion telescope");
        }
        let mut completed = values.clone();
        for index in chain {
            // Match the block emitter's observation boundary. Earlier caller
            // observations must not substitute for this activation's reads.
            operations.length_observations.clear();
            let state = &states[index - 1];
            for binding in &state.bindings {
                let value_type = binding.value_type(
                    &completed
                        .iter()
                        .map(|value| value.value_type())
                        .collect::<Vec<_>>(),
                )?;
                let id = emit_scalar_binding(
                    identities,
                    checked,
                    binding,
                    &completed,
                    &lowered_psi::LoweredSourceFrontierBindings::default(),
                    &self.erased_scalar_formals,
                    next_value,
                    operations,
                    calls,
                )?;
                completed.push(ValueDeclaration {
                    id,
                    scalar_type: value_type.scalar_type,
                    qualifications: value_type.qualifications,
                });
            }
            let LoweredScalarBranchTerminator::Jump { arguments, .. } = &state.terminator else {
                return unsupported("preflighted scalar chain ends in a continuation terminator");
            };
            completed = arguments
                .iter()
                .map(|expression| {
                    validate_direct_parameter_types(
                        expression,
                        &completed
                            .iter()
                            .map(|value| value.scalar_type)
                            .collect::<Vec<_>>(),
                    )?;
                    let value_type = expression.value_type(
                        &completed
                            .iter()
                            .map(|value| value.value_type())
                            .collect::<Vec<_>>(),
                    )?;
                    Ok(ValueDeclaration {
                        id: emit_direct_expression(expression, &completed, next_value, operations),
                        scalar_type: value_type.scalar_type,
                        qualifications: value_type.qualifications,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
        }
        let results = completed[source_count..].to_vec();
        *values = completed[..source_count].to_vec();
        operations.length_observations.clear();
        Ok(Some(results))
    }
}
