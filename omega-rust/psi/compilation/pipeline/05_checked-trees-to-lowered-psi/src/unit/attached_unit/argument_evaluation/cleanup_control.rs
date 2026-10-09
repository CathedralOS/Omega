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
use semantic_vocabulary::{BlockId, EdgeId, ProofTerm, ScalarType};
use terminal_psi::{
    Block, OperationKind, OperationResult, SuccessorEdge, TerminalAffineCleanupAction,
};

/// Invocation-owned block construction shared by statement continuations and
/// scalar expansion. Storage coordinates are already captured and normalized.
/// The first block retains its incoming declarations; private continuations
/// carry scalar values and forward erased proof formals without reevaluation.
pub(crate) struct CleanupEmission<'a> {
    pub(crate) current: &'a mut BlockId,
    pub(crate) parameters: &'a mut Vec<ValueDeclaration>,
    pub(crate) structural_parameters: &'a mut Vec<terminal_psi::StructuralParameterDeclaration>,
    pub(crate) block_erased_scalar_formals: &'a mut Vec<ValueDeclaration>,
    pub(crate) erased_proof_formals: &'a [checked_trees::CheckedErasedProofParameterPlan],
    pub(crate) operation_start: &'a mut usize,
    pub(crate) blocks: &'a mut Vec<Block>,
}

impl Evaluation {
    /// Normal death disposes only the remaining partition of a selected
    /// complement. Early descriptor retirement does not retire its home or
    /// authorize a second whole-root disposal of the absent leaf.
    pub(crate) fn normal_discard_regions(
        &self,
        place: semantic_vocabulary::PlaceId,
        types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Result<Vec<CleanupRegion>, LoweringError> {
        let moved = self
            .selection_cleanups
            .iter()
            .flat_map(|cleanup| &cleanup.retired_reference_paths)
            .filter(|(source, _)| self.current_structural_place(*source) == place)
            .map(|(_, path)| path.clone())
            .collect::<std::collections::BTreeSet<_>>();
        if moved.is_empty() {
            return Ok(vec![CleanupRegion::unconditional(
                TerminalAffineCleanupAction::DiscardRoot(place),
            )]);
        }
        let root_type = self
            .structural_root_parameters()
            .find(|declaration| declaration.place == place)
            .map(|declaration| declaration.structural_type)
            .ok_or(LoweringError::Unsupported(
                "selected complement death lost its receiving declaration",
            ))?;
        let residuals =
            terminal_semantics::partial_affine_residuals(types, root_type, &moved, usize::MAX)
                .ok_or(LoweringError::Unsupported(
                    "selected complement death has no exact remaining partition",
                ))?;
        let mut regions = Vec::new();
        for (path, structural_type) in residuals {
            let mut conditions = Vec::new();
            for (position, segment) in path.iter().enumerate() {
                let terminal_psi::StructuralPathSegment::Case(case) = segment else {
                    continue;
                };
                let sum = terminal_semantics::runtime_structural_path_tip(
                    types.iter(),
                    root_type,
                    &path[..position],
                )
                .ok_or(LoweringError::Unsupported(
                    "selected complement remaining case lost its carrier",
                ))?;
                conditions.push(crate::unit::unit_cleanup::CleanupCaseCondition {
                    place,
                    path: path[..position].to_vec(),
                    structural_type: sum.id,
                    case: *case,
                });
            }
            regions.push(CleanupRegion {
                conditions,
                action: TerminalAffineCleanupAction::DiscardResidual(
                    terminal_psi::StructuralAffineDiscard {
                        place,
                        path,
                        structural_type,
                    },
                ),
            });
        }
        regions.push(CleanupRegion::unconditional(
            TerminalAffineCleanupAction::RetireHome(place),
        ));
        Ok(regions)
    }

    /// Bind outgoing structural operands before cleanup can retire their old
    /// homes. The later edge forwards these whole captures, not their old paths.
    pub(crate) fn capture_cleanup_arguments(
        &mut self,
        arguments: &mut Vec<terminal_psi::StructuralArgument>,
        declarations: &[terminal_psi::StructuralParameterDeclaration],
        values: &mut Vec<ValueDeclaration>,
        next_place: &mut u64,
        places: &mut Vec<terminal_psi::StructuralPlaceDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &OperationBuffer,
    ) -> Result<(), LoweringError> {
        CleanupEmission {
            current: &mut self.current,
            parameters: &mut self.parameters,
            structural_parameters: &mut self.block_structural_parameters,
            block_erased_scalar_formals: &mut Vec::new(),
            erased_proof_formals: &self.erased_proof_formals,
            operation_start: &mut self.operation_start,
            blocks: &mut self.blocks,
        }
        .capture_arguments(
            arguments,
            declarations,
            values,
            next_place,
            places,
            next_value,
            next_block,
            next_edge,
            operations,
        )
    }

    /// Flat schedules remain on the completion edge; selected schedules use
    /// the same private control blocks as statement and call continuations.
    pub(crate) fn emit_cleanup_regions(
        &mut self,
        regions: Vec<CleanupRegion>,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
    ) -> Result<Vec<TerminalAffineCleanupAction>, LoweringError> {
        if regions.iter().all(|region| region.conditions.is_empty()) {
            return Ok(regions.into_iter().map(|region| region.action).collect());
        }
        self.continue_cleanup_regions(
            regions, values, next_value, next_block, next_edge, operations,
        )?;
        Ok(Vec::new())
    }

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
            let place = match &mut region.action {
                TerminalAffineCleanupAction::DiscardRoot(place)
                | TerminalAffineCleanupAction::RetireHome(place) => place,
                TerminalAffineCleanupAction::DiscardResidual(discard) => &mut discard.place,
                TerminalAffineCleanupAction::InvokeNominal(cleanup) => &mut cleanup.place,
            };
            *place = self.current_structural_place(*place);
            for condition in &mut region.conditions {
                condition.place = self.current_structural_place(condition.place);
            }
        }
        CleanupEmission {
            current: &mut self.current,
            parameters: &mut self.parameters,
            structural_parameters: &mut self.block_structural_parameters,
            block_erased_scalar_formals: &mut Vec::new(),
            erased_proof_formals: &self.erased_proof_formals,
            operation_start: &mut self.operation_start,
            blocks: &mut self.blocks,
        }
        .emit(
            &regions, values, next_value, next_block, next_edge, operations,
        )
    }
}

impl CleanupEmission<'_> {
    pub(crate) fn capture_arguments(
        &mut self,
        arguments: &mut Vec<terminal_psi::StructuralArgument>,
        declarations: &[terminal_psi::StructuralParameterDeclaration],
        values: &mut Vec<ValueDeclaration>,
        next_place: &mut u64,
        places: &mut Vec<terminal_psi::StructuralPlaceDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &OperationBuffer,
    ) -> Result<(), LoweringError> {
        if arguments.len() != declarations.len() {
            return Err(LoweringError::Unsupported(
                "cleanup capture has no exact structural roster",
            ));
        }
        if arguments.is_empty() {
            return Ok(());
        }
        let target = block_id(allocate_dense(next_block)?);
        let parameters = super::declarations(
            &values
                .iter()
                .map(|value| value.value_type())
                .collect::<Vec<_>>(),
            next_value,
        )?;
        let mut captures = Vec::with_capacity(arguments.len());
        let mut forwarded = Vec::with_capacity(arguments.len());
        for (ordinal, (argument, declaration)) in arguments.iter().zip(declarations).enumerate() {
            let position = u32::try_from(ordinal).map_err(|_| {
                LoweringError::Unsupported("cleanup capture structural roster exceeds u32")
            })?;
            let place = crate::terminal_identities::place_id(allocate_dense(next_place)?);
            places.push(terminal_psi::StructuralPlaceDeclaration {
                id: place,
                kind: semantic_vocabulary::StructuralPlaceKind::BlockParameter {
                    block: target,
                    position,
                },
            });
            let mut capture = declaration.clone();
            capture.place = place;
            capture.position = position;
            capture.access = argument.access;
            captures.push(capture);
            forwarded.push(terminal_psi::StructuralArgument {
                place,
                path: Vec::new(),
                access: argument.access,
            });
        }
        self.cleanup_jump(
            target,
            &parameters,
            Vec::new(),
            std::mem::take(arguments),
            values,
            next_edge,
            operations,
        )?;
        *self.structural_parameters = captures;
        *arguments = forwarded;
        Ok(())
    }

    pub(crate) fn emit(
        &mut self,
        regions: &[CleanupRegion],
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
    ) -> Result<(), LoweringError> {
        self.cleanup_scope(
            regions, 0, values, next_value, next_block, next_edge, operations,
        )
    }

    fn proof_arguments(&self) -> Result<Vec<ProofTerm>, LoweringError> {
        (0..self.erased_proof_formals.len())
            .map(|position| {
                Ok(ProofTerm::Formal {
                    position: u32::try_from(position).map_err(|_| {
                        LoweringError::Unsupported("cleanup erased-proof roster exceeds u32")
                    })?,
                })
            })
            .collect()
    }

    pub(crate) fn continue_actions(
        &mut self,
        actions: Vec<TerminalAffineCleanupAction>,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &OperationBuffer,
    ) -> Result<EdgeId, LoweringError> {
        let continuation = block_id(allocate_dense(next_block)?);
        let parameters = declarations(
            &values
                .iter()
                .map(|value| value.value_type())
                .collect::<Vec<_>>(),
            next_value,
        )?;
        self.cleanup_jump(
            continuation,
            &parameters,
            actions,
            Vec::new(),
            values,
            next_edge,
            operations,
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
                self.continue_actions(
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
                    source: condition.place,
                    path: condition.path.clone(),
                    case: condition.case,
                },
            );
            let proof_arguments = self.proof_arguments()?;
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
                id: *self.current,
                parameters: std::mem::take(self.parameters),
                structural_parameters: std::mem::take(self.structural_parameters),
                erased_scalar_formals: std::mem::take(self.block_erased_scalar_formals),
                erased_proof_formals:
                    crate::scalar_lowering::contracts::erased_proof_formal_declarations(
                        self.erased_proof_formals,
                    ),
                operations: operations[*self.operation_start..].to_vec(),
                terminator: Terminator::Conditional {
                    condition: observation.id,
                    when_true: successor(edge_id(allocate_dense(next_edge)?), when_true),
                    when_false: successor(edge_id(allocate_dense(next_edge)?), when_false),
                },
            });
            *self.current = when_true;
            *self.parameters = when_true_parameters.clone();
            *values = when_true_parameters;
            *self.operation_start = operations.len();
            self.cleanup_scope(
                &remaining[..count],
                depth + 1,
                values,
                next_value,
                next_block,
                next_edge,
                operations,
            )?;
            self.cleanup_jump(
                join,
                join_parameters,
                Vec::new(),
                Vec::new(),
                values,
                next_edge,
                operations,
            )?;
            *self.current = when_false;
            *self.parameters = when_false_parameters.clone();
            *values = when_false_parameters;
            *self.operation_start = operations.len();
            remaining = &remaining[count..];
        }
        Ok(())
    }

    fn cleanup_jump(
        &mut self,
        target: BlockId,
        parameters: &[ValueDeclaration],
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
        structural_arguments: Vec<terminal_psi::StructuralArgument>,
        values: &mut Vec<ValueDeclaration>,
        next_edge: &mut u64,
        operations: &OperationBuffer,
    ) -> Result<EdgeId, LoweringError> {
        let edge = edge_id(allocate_dense(next_edge)?);
        self.blocks.push(Block {
            id: *self.current,
            parameters: std::mem::take(self.parameters),
            structural_parameters: std::mem::take(self.structural_parameters),
            erased_scalar_formals: std::mem::take(self.block_erased_scalar_formals),
            erased_proof_formals:
                crate::scalar_lowering::contracts::erased_proof_formal_declarations(
                    self.erased_proof_formals,
                ),
            operations: operations[*self.operation_start..].to_vec(),
            terminator: Terminator::Jump {
                edge,
                target,
                arguments: values.iter().map(|value| value.id).collect(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: self.proof_arguments()?,
                structural_arguments,
                cleanup_actions,
            },
        });
        *self.current = target;
        *self.parameters = parameters.to_vec();
        *values = parameters.to_vec();
        *self.operation_start = operations.len();
        Ok(edge)
    }
}
