//! A state whose bindings stage short-circuit Boolean control: each staged
//! binding gets its own block chain, then the continuation terminator is
//! emitted from the final stage.

use super::super::{
    Block, LoweringError, SuccessorEdge, Terminator, ValueDeclaration, block_id,
    boolean_decision_block_count, boolean_decision_test_count, build_scalar_conditional_target,
    contains_short_circuit, direct_expression_contains_short_circuit, edge_id,
    emit_boolean_expression, emit_direct_expression, emit_inlined_boolean_guard_blocks,
    emit_inlined_boolean_value_blocks, emit_reserved_boolean_tuple_stage_blocks,
    emit_scalar_binding, emit_staged_scalar_call_binding, lower_boolean_control_decision,
    lower_boolean_value_decision, lower_checked_crash_predicates, scalar_source_block, unsupported,
    value_id,
};
use super::qualifications;
use super::{GraphEmission, StateFrame};
use crate::emission::boolean_control::{LoweredBooleanDecision, LoweredBooleanDecisionExit};
use crate::emission::operation_emission::LoweredScalarBinding;
use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::scalar_graph::scalar_graph_lowering::prepared_graph::LoweredScalarBranchTerminator;

impl GraphEmission<'_> {
    /// Emit one state through its staged bindings and continuation.
    pub(super) fn emit_staged_state(
        &mut self,
        frame: StateFrame<'_>,
        binding_plans: Vec<LoweredScalarBinding>,
        continuation_plan: LoweredScalarBranchTerminator,
    ) -> Result<(), LoweringError> {
        let StateFrame {
            state,
            source_block,
            source_block_parameters,
            current_parameters,
            erased_formals,
        } = frame;
        if !state.structural_effects.is_empty() {
            return unsupported(
                "structural effects require their own completed scalar evaluation state",
            );
        }
        let mut stage_block = source_block;
        let mut stage_parameters = current_parameters.clone();
        let mut stage_parameter_types = state.parameter_types.clone();
        let mut stage_block_parameters = source_block_parameters;
        for (binding_index, binding) in binding_plans.iter().enumerate() {
            let mut next_stage_types = stage_parameter_types.clone();
            next_stage_types.push(binding.value_type(&stage_parameter_types)?);
            let next_stage_parameters = next_stage_types
                .iter()
                .copied()
                .map(|scalar_type| {
                    let parameter = ValueDeclaration {
                        id: value_id(self.next_value_identity),
                        scalar_type: scalar_type.scalar_type,
                        qualifications: scalar_type.qualifications,
                    };
                    self.next_value_identity = self
                        .next_value_identity
                        .checked_add(1)
                        .expect("staged local parameter identities advance");
                    parameter
                })
                .collect::<Vec<_>>();
            let next_stage =
                if let LoweredScalarBinding::Expression(LoweredDirectExpression::Boolean {
                    expression,
                }) = binding
                    && contains_short_circuit(expression)
                {
                    let decision = lower_boolean_value_decision(expression);
                    let decision_block_count = boolean_decision_block_count(&decision);
                    let first_child_identity = self.next_block_identity;
                    let next_stage = block_id(
                        self.next_block_identity
                            .checked_add(
                                u64::try_from(decision_block_count - 1)
                                    .expect("staged Boolean child count fits a semantic identity"),
                            )
                            .expect("staged Boolean continuation identity advances"),
                    );
                    self.next_block_identity = next_stage
                        .get()
                        .checked_add(1)
                        .expect("staged Boolean block identities advance");
                    let carried_arguments = stage_parameters
                        .iter()
                        .map(|parameter| parameter.id)
                        .collect::<Vec<_>>();
                    let first_reserved_identity = if binding_index == 0 {
                        first_child_identity
                            .checked_sub(1)
                            .expect("staged Boolean blocks follow source blocks")
                    } else {
                        stage_block.get()
                    };
                    let mut decision_blocks = Vec::with_capacity(decision_block_count);
                    let entry = emit_reserved_boolean_tuple_stage_blocks(
                        &decision,
                        &stage_parameters,
                        stage_block_parameters,
                        next_stage,
                        &carried_arguments,
                        first_reserved_identity,
                        &mut self.next_value_identity,
                        &mut self.next_edge_identity,
                        &mut self.all_operations,
                        &mut decision_blocks,
                    );
                    assert_eq!(entry.get(), first_reserved_identity);
                    let mut decision_blocks = decision_blocks
                        .into_iter()
                        .map(|block| block.expect("every staged Boolean block is finalized"));
                    let mut root = decision_blocks
                        .next()
                        .expect("staged short-circuit Boolean has a decision root");
                    if binding_index == 0 {
                        root.id = source_block;
                        self.blocks.push(root);
                    } else {
                        self.inlined_blocks.push(root);
                    }
                    self.inlined_blocks.extend(decision_blocks);
                    next_stage
                } else if let LoweredScalarBinding::DirectCall(call) = binding
                    && call
                        .arguments
                        .iter()
                        .any(direct_expression_contains_short_circuit)
                {
                    let (next_stage, mut call_blocks) = emit_staged_scalar_call_binding(
                        call,
                        &stage_parameters,
                        &stage_parameter_types,
                        stage_block_parameters,
                        stage_block,
                        erased_formals,
                        &mut self.next_block_identity,
                        &mut self.next_value_identity,
                        &mut self.next_edge_identity,
                        &mut self.all_operations,
                        &mut self.call_emission,
                    )?;
                    let root = call_blocks
                        .drain(..1)
                        .next()
                        .expect("a staged scalar call has an argument root");
                    if binding_index == 0 {
                        self.blocks.push(root);
                    } else {
                        self.inlined_blocks.push(root);
                    }
                    self.inlined_blocks.extend(call_blocks);
                    next_stage
                } else {
                    let next_stage = block_id(self.next_block_identity);
                    self.next_block_identity = self
                        .next_block_identity
                        .checked_add(1)
                        .expect("staged direct-local block identities advance");
                    let stage_operation_start = self.all_operations.len();
                    let value = emit_scalar_binding(
                        binding,
                        &stage_parameters,
                        erased_formals,
                        &mut self.next_value_identity,
                        &mut self.all_operations,
                        &mut self.call_emission,
                    )?;
                    let mut arguments = stage_parameters
                        .iter()
                        .map(|parameter| parameter.id)
                        .collect::<Vec<_>>();
                    arguments.push(value);
                    let edge = edge_id(self.next_edge_identity);
                    self.next_edge_identity = self
                        .next_edge_identity
                        .checked_add(1)
                        .expect("staged direct-local edge identity advances");
                    let block = Block {
                        structural_parameters: Vec::new(),
                        id: stage_block,
                        parameters: stage_block_parameters,
                        erased_scalar_formals: Vec::new(),
                        operations: self.all_operations[stage_operation_start..].to_vec(),
                        terminator: Terminator::Jump {
                            structural_arguments: Vec::new(),
                            edge,
                            target: next_stage,
                            arguments,
                            erased_arguments: Vec::new(),
                            residual_affine_discards: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    };
                    if binding_index == 0 {
                        self.blocks.push(block);
                    } else {
                        self.inlined_blocks.push(block);
                    }
                    next_stage
                };
            stage_block = next_stage;
            stage_parameters = next_stage_parameters;
            stage_parameter_types = next_stage_types;
            stage_block_parameters = stage_parameters.clone();
        }

        if let LoweredScalarBranchTerminator::Return {
            expression: LoweredDirectExpression::Boolean { expression },
        } = &continuation_plan
            && contains_short_circuit(expression)
        {
            let decision = lower_boolean_value_decision(expression);
            let block_count = boolean_decision_block_count(&decision);
            let first_synthetic_block = block_id(self.next_block_identity);
            self.next_block_identity = self
                .next_block_identity
                .checked_add(
                    u64::try_from(block_count - 1)
                        .expect("staged Boolean return child count fits a semantic identity"),
                )
                .expect("staged Boolean return block identities advance");
            let (root, children) = emit_inlined_boolean_value_blocks(
                &decision,
                &stage_parameters,
                stage_parameters.clone(),
                LoweredBooleanDecisionExit::Return,
                stage_block,
                first_synthetic_block,
                &mut self.next_value_identity,
                &mut self.next_edge_identity,
                &mut self.all_operations,
            );
            self.inlined_blocks.push(root);
            self.inlined_blocks.extend(children);
            return Ok(());
        }
        if let LoweredScalarBranchTerminator::Jump {
            target,
            arguments,
            structural_arguments,
            trivial_affine_discards,
            ..
        } = &continuation_plan
            && structural_arguments.is_empty()
            && trivial_affine_discards.is_empty()
            && let [LoweredDirectExpression::Boolean { expression }] = arguments.as_slice()
            && contains_short_circuit(expression)
        {
            let decision = lower_boolean_value_decision(expression);
            let block_count = boolean_decision_block_count(&decision);
            let first_synthetic_block = block_id(self.next_block_identity);
            self.next_block_identity = self
                .next_block_identity
                .checked_add(
                    u64::try_from(block_count - 1)
                        .expect("staged Boolean jump child count fits a semantic identity"),
                )
                .expect("staged Boolean jump block identities advance");
            let target = scalar_source_block(self.identity_base, *target);
            let (root, children) = emit_inlined_boolean_value_blocks(
                &decision,
                &stage_parameters,
                stage_parameters.clone(),
                LoweredBooleanDecisionExit::Jump { target },
                stage_block,
                first_synthetic_block,
                &mut self.next_value_identity,
                &mut self.next_edge_identity,
                &mut self.all_operations,
            );
            self.inlined_blocks.push(root);
            self.inlined_blocks.extend(children);
            return Ok(());
        }
        if let LoweredScalarBranchTerminator::Conditional {
            condition,
            when_true_target,
            when_true_arguments,
            when_false_target,
            when_false_arguments,
            ..
        } = &continuation_plan
            && contains_short_circuit(condition)
        {
            let decision = lower_boolean_control_decision(
                condition,
                LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant {
                    value: true,
                }),
                LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant {
                    value: false,
                }),
            );
            let decision_block_count = boolean_decision_test_count(&decision);
            debug_assert!(decision_block_count > 0);
            let first_synthetic_block = block_id(self.next_block_identity);
            self.next_block_identity = self
                .next_block_identity
                .checked_add(
                    u64::try_from(decision_block_count - 1)
                        .expect("staged Boolean guard child count fits a semantic identity"),
                )
                .expect("staged Boolean guard block identities advance");
            let when_true = build_scalar_conditional_target(
                *when_true_target,
                when_true_arguments,
                &stage_parameters,
                &stage_parameter_types,
                &mut self.next_block_identity,
                &mut self.next_value_identity,
                &mut self.pending_blocks,
                self.identity_base,
            )?;
            let when_false = build_scalar_conditional_target(
                *when_false_target,
                when_false_arguments,
                &stage_parameters,
                &stage_parameter_types,
                &mut self.next_block_identity,
                &mut self.next_value_identity,
                &mut self.pending_blocks,
                self.identity_base,
            )?;
            let (root, children) = emit_inlined_boolean_guard_blocks(
                &decision,
                &stage_parameters,
                stage_parameters.clone(),
                &when_true,
                &when_false,
                stage_block,
                first_synthetic_block,
                &mut self.next_value_identity,
                &mut self.next_edge_identity,
                &mut self.all_operations,
            );
            self.inlined_blocks.push(root);
            self.inlined_blocks.extend(children);
            return Ok(());
        }

        let operation_start = self.all_operations.len();
        let terminator = match continuation_plan {
            LoweredScalarBranchTerminator::Qualify {
                target,
                arguments,
                structural_arguments,
            } => qualifications::emit(
                target,
                &arguments,
                &structural_arguments,
                &stage_parameters,
                &self.state_parameters,
                self.terminal_machine,
                self.identity_base,
                &mut self.next_edge_identity,
                &mut self.scalar_qualifications,
            )?,
            LoweredScalarBranchTerminator::Return { expression } => {
                let value = emit_direct_expression(
                    &expression,
                    &stage_parameters,
                    &mut self.next_value_identity,
                    &mut self.all_operations,
                );
                let edge = edge_id(self.next_edge_identity);
                self.next_edge_identity = self
                    .next_edge_identity
                    .checked_add(1)
                    .expect("carried Boolean return edge identity advances");
                Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge,
                    value,
                }
            }
            LoweredScalarBranchTerminator::Conditional {
                condition,
                when_true_target,
                when_true_arguments,
                when_false_target,
                when_false_arguments,
                ..
            } => {
                let condition = emit_boolean_expression(
                    &condition,
                    &stage_parameters,
                    &mut self.next_value_identity,
                    &mut self.all_operations,
                );
                let when_true = build_scalar_conditional_target(
                    when_true_target,
                    &when_true_arguments,
                    &stage_parameters,
                    &stage_parameter_types,
                    &mut self.next_block_identity,
                    &mut self.next_value_identity,
                    &mut self.pending_blocks,
                    self.identity_base,
                )?;
                let when_false = build_scalar_conditional_target(
                    when_false_target,
                    &when_false_arguments,
                    &stage_parameters,
                    &stage_parameter_types,
                    &mut self.next_block_identity,
                    &mut self.next_value_identity,
                    &mut self.pending_blocks,
                    self.identity_base,
                )?;
                let when_true_edge = edge_id(self.next_edge_identity);
                self.next_edge_identity = self
                    .next_edge_identity
                    .checked_add(1)
                    .expect("carried Boolean true edge identity advances");
                let when_false_edge = edge_id(self.next_edge_identity);
                self.next_edge_identity = self
                    .next_edge_identity
                    .checked_add(1)
                    .expect("carried Boolean false edge identity advances");
                Terminator::Conditional {
                    condition,
                    when_true: SuccessorEdge {
                        structural_arguments: Vec::new(),
                        edge: when_true_edge,
                        target: when_true.block,
                        arguments: when_true.arguments,
                        erased_arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                    when_false: SuccessorEdge {
                        structural_arguments: Vec::new(),
                        edge: when_false_edge,
                        target: when_false.block,
                        arguments: when_false.arguments,
                        erased_arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                }
            }
            LoweredScalarBranchTerminator::Jump {
                target,
                arguments,
                structural_arguments,
                trivial_affine_discards,
                ..
            } => {
                if (!structural_arguments.is_empty() || !trivial_affine_discards.is_empty())
                    && arguments
                        .iter()
                        .any(direct_expression_contains_short_circuit)
                {
                    return unsupported(
                        "structural state transfer requires completed scalar operands",
                    );
                }
                let edge = edge_id(self.next_edge_identity);
                self.next_edge_identity = self
                    .next_edge_identity
                    .checked_add(1)
                    .expect("staged local jump edge identity advances");
                if arguments
                    .iter()
                    .any(direct_expression_contains_short_circuit)
                {
                    let target = build_scalar_conditional_target(
                        target,
                        &arguments,
                        &stage_parameters,
                        &stage_parameter_types,
                        &mut self.next_block_identity,
                        &mut self.next_value_identity,
                        &mut self.pending_blocks,
                        self.identity_base,
                    )?;
                    Terminator::Jump {
                        structural_arguments: structural_arguments.clone(),
                        edge,
                        target: target.block,
                        arguments: target.arguments,
                        erased_arguments: Vec::new(),
                        residual_affine_discards: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    }
                } else {
                    let arguments = arguments
                        .iter()
                        .map(|argument| {
                            emit_direct_expression(
                                argument,
                                &stage_parameters,
                                &mut self.next_value_identity,
                                &mut self.all_operations,
                            )
                        })
                        .collect();
                    Terminator::Jump {
                        structural_arguments,
                        edge,
                        target: scalar_source_block(self.identity_base, target),
                        arguments,
                        erased_arguments: Vec::new(),
                        residual_affine_discards: Vec::new(),
                        trivial_affine_discards,
                    }
                }
            }
            LoweredScalarBranchTerminator::Crash(crash) => {
                let edge = edge_id(self.next_edge_identity);
                self.next_edge_identity = self
                    .next_edge_identity
                    .checked_add(1)
                    .expect("staged local crash edge identity advances");
                Terminator::Crash {
                    edge,
                    cause: crash.cause,
                    site_guard: lower_checked_crash_predicates(&crash.site_guard, self.parameters)?,
                    frontier_lower_bound: crash.frontier_lower_bound,
                }
            }
        };
        self.inlined_blocks.push(Block {
            structural_parameters: Vec::new(),
            id: stage_block,
            parameters: stage_parameters,
            erased_scalar_formals: Vec::new(),
            operations: self.all_operations[operation_start..].to_vec(),
            terminator,
        });
        Ok(())
    }
}
