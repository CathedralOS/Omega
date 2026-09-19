//! One scalar-graph state: its bindings, structural effects and terminator
//! become one block, with short-circuit Boolean control inlined beneath it and
//! conditional or tuple targets reserved for `pending_blocks`.

use super::super::{
    Block, LoweringError, SuccessorEdge, Terminator, ValueDeclaration, block_id,
    boolean_decision_block_count, boolean_decision_test_count, build_scalar_conditional_target,
    contains_short_circuit, direct_expression_contains_short_circuit, edge_id,
    emit_boolean_expression, emit_direct_expression, emit_inlined_boolean_guard_blocks,
    emit_inlined_boolean_value_blocks, emit_scalar_binding, lower_boolean_control_decision,
    lower_boolean_value_decision, lower_checked_crash_predicates, scalar_source_block,
    staged_short_circuit_bindings_terminator, unsupported,
};
use super::qualifications;
use super::{GraphEmission, StateFrame};
use crate::emission::boolean_control::{LoweredBooleanDecision, LoweredBooleanDecisionExit};
use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::proofs::crash_routes::lowered_direct_scalar_term;
use crate::scalar_graph::scalar_graph_lowering::prepared_graph::LoweredScalarBranchTerminator;

impl GraphEmission<'_> {
    /// Emit the state at `index` in the graph's emission order.
    pub(super) fn emit_state(&mut self, index: usize) -> Result<(), LoweringError> {
        let states = self.states;
        let loop_plan = self.loop_plan;
        let state = &states[index];
        let operation_start = self.all_operations.len();
        let current_parameters = self.state_parameters[index].clone();
        let source_block = block_id(
            self.identity_base
                .checked_add(u64::try_from(index).expect("state index fits a semantic identity"))
                .expect("state index fits the machine identity namespace")
                .checked_add(1)
                .expect("block identity is nonzero"),
        );
        let source_block_parameters = if index == 0 && loop_plan.is_none() {
            Vec::new()
        } else {
            current_parameters.clone()
        };
        let staged_short_circuit_terminator =
            staged_short_circuit_bindings_terminator(&state.bindings, &state.terminator);
        if let Some((binding_plans, continuation_plan)) = staged_short_circuit_terminator {
            let erased_formals = self.state_erased_formals[index].clone();
            return self.emit_staged_state(
                StateFrame {
                    state,
                    source_block,
                    source_block_parameters,
                    current_parameters: &current_parameters,
                    erased_formals: &erased_formals,
                },
                binding_plans,
                continuation_plan,
            );
        }
        let mut current_values = current_parameters.clone();
        let mut current_value_types = state.parameter_types.clone();
        for binding in &state.bindings {
            let binding_type = binding.value_type(&current_value_types)?;
            let id = emit_scalar_binding(
                binding,
                &current_values,
                &self.state_erased_formals[index],
                &mut self.next_value_identity,
                &mut self.all_operations,
                &mut self.call_emission,
            )?;
            current_values.push(ValueDeclaration {
                id,
                scalar_type: binding_type.scalar_type,
                qualifications: binding_type.qualifications,
            });
            current_value_types.push(binding_type);
        }
        crate::scalar_graph::scalar_graph_effects::emit(
            &state.structural_effects,
            &current_values,
            &mut self.next_value_identity,
            &mut self.all_operations,
            &mut self.call_emission,
        )?;
        let terminator_operation_start = self.all_operations.len();
        let terminator = match &state.terminator {
            LoweredScalarBranchTerminator::Qualify {
                target,
                arguments,
                structural_arguments,
            } => qualifications::emit(
                *target,
                arguments,
                structural_arguments,
                &current_values,
                &self.state_parameters,
                self.terminal_machine,
                self.identity_base,
                &mut self.next_edge_identity,
                &mut self.scalar_qualifications,
            )?,
            LoweredScalarBranchTerminator::Jump {
                target,
                arguments,
                erased_arguments,
                structural_arguments,
                trivial_affine_discards,
            } => {
                if erased_arguments
                    .iter()
                    .any(direct_expression_contains_short_circuit)
                {
                    return unsupported("erased call operands require completed scalar operands");
                }
                if (!structural_arguments.is_empty() || !trivial_affine_discards.is_empty())
                    && arguments
                        .iter()
                        .any(direct_expression_contains_short_circuit)
                {
                    return unsupported(
                        "structural state transfer requires completed scalar operands",
                    );
                }
                if let [LoweredDirectExpression::Boolean { expression }] = arguments.as_slice()
                    && contains_short_circuit(expression)
                {
                    if !erased_arguments.is_empty() {
                        return unsupported(
                            "erased call operands do not stage through Boolean decisions",
                        );
                    }
                    let decision = lower_boolean_value_decision(expression);
                    let block_count = boolean_decision_block_count(&decision);
                    let first_synthetic_block = block_id(self.next_block_identity);
                    self.next_block_identity = self
                        .next_block_identity
                        .checked_add(
                            u64::try_from(block_count - 1)
                                .expect("Boolean binding child count fits a semantic identity"),
                        )
                        .expect("Boolean binding block identities advance");
                    let target = scalar_source_block(self.identity_base, *target);
                    let (root, children) = emit_inlined_boolean_value_blocks(
                        &decision,
                        &current_values,
                        source_block_parameters,
                        LoweredBooleanDecisionExit::Jump { target },
                        source_block,
                        first_synthetic_block,
                        &mut self.next_value_identity,
                        &mut self.next_edge_identity,
                        &mut self.all_operations,
                    );
                    let mut root = root;
                    root.operations.splice(
                        0..0,
                        self.all_operations[operation_start..terminator_operation_start]
                            .iter()
                            .cloned(),
                    );
                    self.blocks.push(root);
                    self.inlined_blocks.extend(children);
                    return Ok(());
                } else if arguments
                    .iter()
                    .any(direct_expression_contains_short_circuit)
                {
                    if !erased_arguments.is_empty() {
                        return unsupported(
                            "erased call operands do not stage through tuple entry",
                        );
                    }
                    let target = build_scalar_conditional_target(
                        *target,
                        arguments,
                        &current_values,
                        &current_value_types,
                        &mut self.next_block_identity,
                        &mut self.next_value_identity,
                        &mut self.pending_blocks,
                        self.identity_base,
                    )?;
                    let edge = edge_id(self.next_edge_identity);
                    self.next_edge_identity = self
                        .next_edge_identity
                        .checked_add(1)
                        .expect("mixed tuple entry edge identity advances");
                    Terminator::Jump {
                        structural_arguments: Vec::new(),
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
                                &current_values,
                                &mut self.next_value_identity,
                                &mut self.all_operations,
                            )
                        })
                        .collect();
                    let erased_arguments = erased_arguments
                        .iter()
                        .map(|argument| {
                            lowered_direct_scalar_term(
                                argument,
                                &current_values,
                                &self.state_erased_formals[index],
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let edge = edge_id(self.next_edge_identity);
                    self.next_edge_identity = self
                        .next_edge_identity
                        .checked_add(1)
                        .expect("scalar graph jump edge identities advance");
                    Terminator::Jump {
                        structural_arguments: structural_arguments.clone(),
                        edge,
                        target: scalar_source_block(self.identity_base, *target),
                        arguments,
                        erased_arguments,
                        residual_affine_discards: Vec::new(),
                        trivial_affine_discards: trivial_affine_discards.clone(),
                    }
                }
            }
            LoweredScalarBranchTerminator::Conditional {
                condition,
                when_true_target,
                when_true_arguments,
                when_true_erased_arguments,
                when_false_target,
                when_false_arguments,
                when_false_erased_arguments,
            } => {
                if when_true_erased_arguments
                    .iter()
                    .chain(when_false_erased_arguments.iter())
                    .any(direct_expression_contains_short_circuit)
                {
                    return unsupported("erased call operands require completed scalar operands");
                }
                if contains_short_circuit(condition) {
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
                                .expect("scalar graph guard child count fits a semantic identity"),
                        )
                        .expect("scalar graph guard block identities advance");
                    let when_true = build_scalar_conditional_target(
                        *when_true_target,
                        when_true_arguments,
                        &current_values,
                        &current_value_types,
                        &mut self.next_block_identity,
                        &mut self.next_value_identity,
                        &mut self.pending_blocks,
                        self.identity_base,
                    )?;
                    let when_false = build_scalar_conditional_target(
                        *when_false_target,
                        when_false_arguments,
                        &current_values,
                        &current_value_types,
                        &mut self.next_block_identity,
                        &mut self.next_value_identity,
                        &mut self.pending_blocks,
                        self.identity_base,
                    )?;
                    if !when_true_erased_arguments.is_empty()
                        || !when_false_erased_arguments.is_empty()
                    {
                        return unsupported(
                            "erased call operands do not stage through guard decisions",
                        );
                    }
                    let (root, children) = emit_inlined_boolean_guard_blocks(
                        &decision,
                        &current_values,
                        source_block_parameters,
                        &when_true,
                        &when_false,
                        source_block,
                        first_synthetic_block,
                        &mut self.next_value_identity,
                        &mut self.next_edge_identity,
                        &mut self.all_operations,
                    );
                    let mut root = root;
                    root.operations.splice(
                        0..0,
                        self.all_operations[operation_start..terminator_operation_start]
                            .iter()
                            .cloned(),
                    );
                    self.blocks.push(root);
                    self.inlined_blocks.extend(children);
                    return Ok(());
                } else {
                    let condition = emit_boolean_expression(
                        condition,
                        &current_values,
                        &mut self.next_value_identity,
                        &mut self.all_operations,
                    );
                    let when_true_edge = edge_id(self.next_edge_identity);
                    self.next_edge_identity = self
                        .next_edge_identity
                        .checked_add(1)
                        .expect("scalar graph edge identities advance");
                    let when_false_edge = edge_id(self.next_edge_identity);
                    self.next_edge_identity = self
                        .next_edge_identity
                        .checked_add(1)
                        .expect("scalar graph edge identities advance");
                    let when_true = build_scalar_conditional_target(
                        *when_true_target,
                        when_true_arguments,
                        &current_values,
                        &current_value_types,
                        &mut self.next_block_identity,
                        &mut self.next_value_identity,
                        &mut self.pending_blocks,
                        self.identity_base,
                    )?;
                    let when_false = build_scalar_conditional_target(
                        *when_false_target,
                        when_false_arguments,
                        &current_values,
                        &current_value_types,
                        &mut self.next_block_identity,
                        &mut self.next_value_identity,
                        &mut self.pending_blocks,
                        self.identity_base,
                    )?;
                    let erased_terms = |arguments: &[LoweredDirectExpression]| {
                        arguments
                            .iter()
                            .map(|argument| {
                                lowered_direct_scalar_term(
                                    argument,
                                    &current_values,
                                    &self.state_erased_formals[index],
                                )
                            })
                            .collect::<Result<Vec<_>, _>>()
                    };
                    let when_true_erased = if when_true.block
                        == scalar_source_block(self.identity_base, *when_true_target)
                    {
                        erased_terms(when_true_erased_arguments)?
                    } else {
                        if !when_true_erased_arguments.is_empty() {
                            return unsupported(
                                "erased call operands do not stage through tuple entry",
                            );
                        }
                        Vec::new()
                    };
                    let when_false_erased = if when_false.block
                        == scalar_source_block(self.identity_base, *when_false_target)
                    {
                        erased_terms(when_false_erased_arguments)?
                    } else {
                        if !when_false_erased_arguments.is_empty() {
                            return unsupported(
                                "erased call operands do not stage through tuple entry",
                            );
                        }
                        Vec::new()
                    };
                    Terminator::Conditional {
                        condition,
                        when_true: SuccessorEdge {
                            structural_arguments: Vec::new(),
                            edge: when_true_edge,
                            target: when_true.block,
                            arguments: when_true.arguments,
                            erased_arguments: when_true_erased,
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            structural_arguments: Vec::new(),
                            edge: when_false_edge,
                            target: when_false.block,
                            arguments: when_false.arguments,
                            erased_arguments: when_false_erased,
                            trivial_affine_discards: Vec::new(),
                        },
                    }
                }
            }
            LoweredScalarBranchTerminator::Return { expression } => {
                if let LoweredDirectExpression::Boolean { expression } = expression
                    && contains_short_circuit(expression)
                {
                    let decision = lower_boolean_value_decision(expression);
                    let block_count = boolean_decision_block_count(&decision);
                    let first_synthetic_block = block_id(self.next_block_identity);
                    self.next_block_identity = self
                        .next_block_identity
                        .checked_add(
                            u64::try_from(block_count - 1)
                                .expect("scalar return child count fits a semantic identity"),
                        )
                        .expect("scalar return block identities advance");
                    let (root, children) = emit_inlined_boolean_value_blocks(
                        &decision,
                        &current_values,
                        source_block_parameters,
                        LoweredBooleanDecisionExit::Return,
                        source_block,
                        first_synthetic_block,
                        &mut self.next_value_identity,
                        &mut self.next_edge_identity,
                        &mut self.all_operations,
                    );
                    let mut root = root;
                    root.operations.splice(
                        0..0,
                        self.all_operations[operation_start..terminator_operation_start]
                            .iter()
                            .cloned(),
                    );
                    self.blocks.push(root);
                    self.inlined_blocks.extend(children);
                    return Ok(());
                } else {
                    let value = emit_direct_expression(
                        expression,
                        &current_values,
                        &mut self.next_value_identity,
                        &mut self.all_operations,
                    );
                    let edge = edge_id(self.next_edge_identity);
                    self.next_edge_identity = self
                        .next_edge_identity
                        .checked_add(1)
                        .expect("scalar graph return edge identities advance");
                    Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge,
                        value,
                    }
                }
            }
            LoweredScalarBranchTerminator::Crash(crash) => {
                let edge = edge_id(self.next_edge_identity);
                self.next_edge_identity = self
                    .next_edge_identity
                    .checked_add(1)
                    .expect("nested crash edge identities advance");
                Terminator::Crash {
                    edge,
                    cause: crash.cause,
                    site_guard: lower_checked_crash_predicates(&crash.site_guard, self.parameters)?,
                    frontier_lower_bound: crash.frontier_lower_bound.clone(),
                }
            }
        };
        let erased_scalar_formals = if index == 0 && loop_plan.is_none() {
            Vec::new()
        } else {
            self.state_erased_formals[index].clone()
        };
        self.blocks.push(Block {
            structural_parameters: Vec::new(),
            id: source_block,
            parameters: source_block_parameters,
            erased_scalar_formals,
            operations: self.all_operations[operation_start..].to_vec(),
            terminator,
        });
        Ok(())
    }
}
