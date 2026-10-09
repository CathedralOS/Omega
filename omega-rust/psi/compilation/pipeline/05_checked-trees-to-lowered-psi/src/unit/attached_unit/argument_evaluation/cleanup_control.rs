//! Case-selected cleanup is ordinary control in the existing operand frame.
//! A discriminator is selected once before disposing that alternative; taking
//! one arm skips the other arms rather than inspecting already disposed storage.
//! Nested selection starts only inside its enclosing arm. Common/unconditional
//! regions keep their source order outside the dispatch. Scalar operands and
//! proof actuals cross every continuation exactly as they do for call cleanup.
//! This builds a linear decision tree, not a product of unrelated cases.

use super::{
    Evaluation, LoweringError, OperationBuffer, Terminator, ValueDeclaration, allocate_dense,
    block_id, declarations, edge_id, value_id,
};
use crate::unit::unit_cleanup::CleanupRegion;
use semantic_vocabulary::{BlockId, EdgeId, ScalarType};
use terminal_psi::{Block, OperationKind, OperationResult, SuccessorEdge};

impl Evaluation {
    pub(crate) fn continue_cleanup_regions(
        &mut self,
        mut regions: Vec<CleanupRegion>,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
    ) -> Result<(), LoweringError> {
        for region in &mut regions {
            for condition in &mut region.conditions {
                condition.place = self.current_structural_place(condition.place);
            }
        }
        self.cleanup_scope(
            &regions, 0, values, next_value, next_block, next_edge, operations,
        )
    }

    fn cleanup_scope(
        &mut self,
        regions: &[CleanupRegion],
        depth: usize,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
    ) -> Result<(), LoweringError> {
        let mut remaining = regions;
        while let Some(first) = remaining.first() {
            if first.conditions.len() < depth {
                return Err(LoweringError::Unsupported(
                    "cleanup lost its enclosing case scope",
                ));
            }
            if first.conditions.len() == depth {
                let count = remaining
                    .iter()
                    .take_while(|region| region.conditions.len() == depth)
                    .count();
                let actions = remaining[..count]
                    .iter()
                    .map(|region| region.action.clone())
                    .collect();
                self.continue_values(
                    actions, values, next_value, next_block, next_edge, operations,
                )?;
                remaining = &remaining[count..];
                continue;
            }
            let selected = &first.conditions[depth];
            let count = remaining
                .iter()
                .take_while(|region| {
                    region.conditions.get(depth).is_some_and(|condition| {
                        condition.place == selected.place
                            && condition.path == selected.path
                            && condition.structural_type == selected.structural_type
                    })
                })
                .count();
            let join = block_id(allocate_dense(next_block)?);
            let join_parameters = declarations(
                &values
                    .iter()
                    .map(|value| value.value_type())
                    .collect::<Vec<_>>(),
                next_value,
            )?;
            self.cleanup_cases(
                &remaining[..count],
                depth,
                join,
                &join_parameters,
                values,
                next_value,
                next_block,
                next_edge,
                operations,
            )?;
            remaining = &remaining[count..];
        }
        Ok(())
    }

    fn cleanup_cases(
        &mut self,
        regions: &[CleanupRegion],
        depth: usize,
        join: BlockId,
        join_parameters: &[ValueDeclaration],
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
    ) -> Result<(), LoweringError> {
        let mut remaining = regions;
        while let Some(first) = remaining.first() {
            let condition = &first.conditions[depth];
            let count = remaining
                .iter()
                .take_while(|region| region.conditions.get(depth) == Some(condition))
                .count();
            let when_true = block_id(allocate_dense(next_block)?);
            let when_true_parameters = declarations(
                &values
                    .iter()
                    .map(|value| value.value_type())
                    .collect::<Vec<_>>(),
                next_value,
            )?;
            let more_cases = count < remaining.len();
            let (when_false, when_false_parameters) = if more_cases {
                (
                    block_id(allocate_dense(next_block)?),
                    declarations(
                        &values
                            .iter()
                            .map(|value| value.value_type())
                            .collect::<Vec<_>>(),
                        next_value,
                    )?,
                )
            } else {
                (join, join_parameters.to_vec())
            };
            let observation = ValueDeclaration {
                id: value_id(allocate_dense(next_value)?),
                scalar_type: ScalarType::Boolean,
                qualifications: Default::default(),
            };
            operations.push_leaf(
                OperationResult::Scalar(observation.clone()),
                OperationKind::StructuralCaseMembership {
                    source: self.current_structural_place(condition.place),
                    path: condition.path.clone(),
                    case: condition.case,
                },
            );
            let proof_arguments = self
                .proof_formal_forwarding()
                .iter()
                .map(|term| {
                    crate::scalar_lowering::contracts::lowered_proof_term(
                        term,
                        values,
                        &self.erased_scalar_formals,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let successor = |edge, target| SuccessorEdge {
                edge,
                target,
                arguments: values.iter().map(|value| value.id).collect(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: proof_arguments.clone(),
                structural_arguments: Vec::new(),
                cleanup_actions: Vec::new(),
            };
            self.blocks.push(Block {
                id: self.current,
                parameters: std::mem::take(&mut self.parameters),
                structural_parameters: std::mem::take(&mut self.block_structural_parameters),
                erased_scalar_formals: Vec::new(),
                erased_proof_formals:
                    crate::scalar_lowering::contracts::erased_proof_formal_declarations(
                        &self.erased_proof_formals,
                    ),
                operations: operations[self.operation_start..].to_vec(),
                terminator: Terminator::Conditional {
                    condition: observation.id,
                    when_true: successor(edge_id(allocate_dense(next_edge)?), when_true),
                    when_false: successor(edge_id(allocate_dense(next_edge)?), when_false),
                },
            });
            self.current = when_true;
            self.parameters = when_true_parameters.clone();
            *values = when_true_parameters;
            self.operation_start = operations.len();
            self.cleanup_scope(
                &remaining[..count],
                depth + 1,
                values,
                next_value,
                next_block,
                next_edge,
                operations,
            )?;
            self.cleanup_jump(join, join_parameters, values, next_edge, operations)?;
            self.current = when_false;
            self.parameters = when_false_parameters.clone();
            *values = when_false_parameters;
            self.operation_start = operations.len();
            remaining = &remaining[count..];
        }
        Ok(())
    }

    fn cleanup_jump(
        &mut self,
        target: BlockId,
        parameters: &[ValueDeclaration],
        values: &mut Vec<ValueDeclaration>,
        next_edge: &mut u64,
        operations: &OperationBuffer,
    ) -> Result<EdgeId, LoweringError> {
        let edge = edge_id(allocate_dense(next_edge)?);
        self.blocks.push(Block {
            id: self.current,
            parameters: std::mem::take(&mut self.parameters),
            structural_parameters: std::mem::take(&mut self.block_structural_parameters),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals:
                crate::scalar_lowering::contracts::erased_proof_formal_declarations(
                    &self.erased_proof_formals,
                ),
            operations: operations[self.operation_start..].to_vec(),
            terminator: Terminator::Jump {
                edge,
                target,
                arguments: values.iter().map(|value| value.id).collect(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: self
                    .proof_formal_forwarding()
                    .iter()
                    .map(|term| {
                        crate::scalar_lowering::contracts::lowered_proof_term(
                            term,
                            values,
                            &self.erased_scalar_formals,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                structural_arguments: Vec::new(),
                cleanup_actions: Vec::new(),
            },
        });
        self.current = target;
        self.parameters = parameters.to_vec();
        *values = parameters.to_vec();
        self.operation_start = operations.len();
        Ok(edge)
    }
}
