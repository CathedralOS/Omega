//! Evaluate scalar call operands in authored order, then emit the exact callee.

use super::buffer::{OperationBuffer, SourceCallCoordinate};
use super::expressions::{LoweredDirectExpression, emit_direct_expression};
use crate::emission::boolean_control::{
    boolean_decision_block_count, emit_reserved_boolean_tuple_stage_blocks,
    lower_boolean_value_decision,
};
use crate::emission::expression_validation::{
    contains_short_circuit, direct_expression_contains_short_circuit,
};
use crate::lowering_error::LoweringError;
use crate::lowering_error::unsupported;
use crate::proofs::crash_routes::{lower_checked_crash_route_buckets, lowered_direct_scalar_term};
use crate::terminal_identities::block_id;
use crate::terminal_identities::edge_id;
use crate::terminal_identities::obligation_id;
use crate::terminal_identities::value_id;
use semantic_vocabulary::{BlockId, MachineId, ObligationId, QualifiedScalarType, ValueId};
use terminal_psi::{
    Block, Operation, OperationKind, StructuralArgument, Terminator, ValueDeclaration,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoweredDirectCallBinding {
    pub(crate) source_coordinate: SourceCallCoordinate,
    pub(crate) target_machine: symbols::SymbolHandle,
    pub(crate) result_type: QualifiedScalarType,
    pub(crate) arguments: Vec<LoweredDirectExpression>,
    /// Proof-only actuals in the callee's erased-formal order, lowered under
    /// the caller's retained/erased scalar namespaces.
    pub(crate) erased_arguments: Vec<LoweredDirectExpression>,
    pub(crate) structural_arguments: Vec<StructuralArgument>,
    pub(crate) uses_structural_frame: bool,
    pub(crate) crash_continuations: Vec<checked_trees::CrashRouteBucket>,
    pub(crate) parameter_relative_crash_routes: Vec<checked_trees::CrashRouteBucket>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScalarCallCrashScope {
    CallerValues,
    Arguments,
}

pub(crate) struct CallEmissionContext<'a> {
    pub(crate) machine_ids: &'a [(symbols::SymbolHandle, MachineId)],
    pub(crate) requirement_counts: &'a [(symbols::SymbolHandle, usize)],
    pub(crate) next_obligation_identity: u64,
    pub(crate) obligation_limit: u64,
}

impl CallEmissionContext<'_> {
    pub(crate) fn allocate_requirement(&mut self) -> Result<ObligationId, LoweringError> {
        if self.next_obligation_identity >= self.obligation_limit {
            return unsupported("terminal call obligations exceed their machine identity range");
        }
        let obligation = obligation_id(self.next_obligation_identity);
        self.next_obligation_identity = self
            .next_obligation_identity
            .checked_add(1)
            .expect("terminal call obligation identities advance");
        Ok(obligation)
    }
}

pub(super) fn emit_scalar_call_binding(
    call: &LoweredDirectCallBinding,
    parameters: &[ValueDeclaration],
    caller_erased_formals: &[ValueDeclaration],
    next_value_identity: &mut u64,
    operations: &mut OperationBuffer,
    call_emission: &mut CallEmissionContext<'_>,
) -> Result<ValueId, LoweringError> {
    let parameter_types = parameters
        .iter()
        .map(|parameter| QualifiedScalarType {
            scalar_type: parameter.scalar_type,
            qualifications: parameter.qualifications,
        })
        .collect::<Vec<_>>();
    let arguments = call
        .arguments
        .iter()
        .map(|argument| {
            let value_type = argument.value_type(&parameter_types)?;
            Ok(ValueDeclaration {
                qualifications: value_type.qualifications,
                id: emit_direct_expression(argument, parameters, next_value_identity, operations),
                scalar_type: value_type.scalar_type,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    emit_direct_call_operation(
        call,
        // Terminal calls retain the callee's published routes substituted onto
        // evaluated argument values. Source-refined surviving buckets can fold
        // a literal guard to Truth or remove it, which is not that exact value
        // substitution and disagrees with the independently verified callee.
        &call.parameter_relative_crash_routes,
        &arguments,
        parameters,
        caller_erased_formals,
        &arguments,
        next_value_identity,
        operations,
        call_emission,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_staged_scalar_call_binding(
    call: &LoweredDirectCallBinding,
    stage_parameters: &[ValueDeclaration],
    stage_parameter_types: &[QualifiedScalarType],
    stage_block_parameters: Vec<ValueDeclaration>,
    stage_block: BlockId,
    caller_erased_formals: &[ValueDeclaration],
    next_block_identity: &mut u64,
    next_value_identity: &mut u64,
    next_edge_identity: &mut u64,
    operations: &mut OperationBuffer,
    call_emission: &mut CallEmissionContext<'_>,
) -> Result<(BlockId, Vec<Block>), LoweringError> {
    debug_assert!(
        call.arguments
            .iter()
            .any(direct_expression_contains_short_circuit)
    );
    let caller_value_count = stage_parameters.len();
    let mut current_block = stage_block;
    let mut current_parameters = stage_parameters.to_vec();
    let mut current_block_parameters = stage_block_parameters;
    let mut blocks = Vec::new();

    for (argument_index, argument) in call.arguments.iter().enumerate() {
        let mut next_stage_types = stage_parameter_types.to_vec();
        next_stage_types.extend(
            call.arguments[..=argument_index]
                .iter()
                .map(|argument| argument.value_type(stage_parameter_types))
                .collect::<Result<Vec<_>, _>>()?,
        );
        let next_stage_parameters = next_stage_types
            .into_iter()
            .map(|scalar_type| {
                let parameter = ValueDeclaration {
                    qualifications: scalar_type.qualifications,
                    id: value_id(*next_value_identity),
                    scalar_type: scalar_type.scalar_type,
                };
                *next_value_identity = next_value_identity
                    .checked_add(1)
                    .expect("staged call-argument parameter identities advance");
                parameter
            })
            .collect::<Vec<_>>();
        let carried_arguments = current_parameters
            .iter()
            .map(|parameter| parameter.id)
            .collect::<Vec<_>>();

        let next_stage = if let LoweredDirectExpression::Boolean { expression } = argument
            && contains_short_circuit(expression)
        {
            let decision = lower_boolean_value_decision(expression);
            let decision_block_count = boolean_decision_block_count(&decision);
            let first_child_identity = *next_block_identity;
            let next_stage = block_id(
                first_child_identity
                    .checked_add(
                        u64::try_from(decision_block_count - 1)
                            .expect("staged call decision count fits a semantic identity"),
                    )
                    .expect("staged call decision block identities advance"),
            );
            *next_block_identity = next_stage
                .get()
                .checked_add(1)
                .expect("staged call argument blocks advance");
            let first_reserved_identity = first_child_identity
                .checked_sub(1)
                .expect("staged call decision blocks follow their root");
            let mut decision_blocks = Vec::with_capacity(decision_block_count);
            let entry = emit_reserved_boolean_tuple_stage_blocks(
                &decision,
                &current_parameters,
                current_block_parameters,
                next_stage,
                &carried_arguments,
                first_reserved_identity,
                next_value_identity,
                next_edge_identity,
                operations,
                &mut decision_blocks,
            );
            assert_eq!(entry.get(), first_reserved_identity);
            let mut decision_blocks = decision_blocks
                .into_iter()
                .map(|block| block.expect("every staged call decision block is finalized"));
            let mut root = decision_blocks
                .next()
                .expect("a short-circuit call argument has a decision root");
            root.id = current_block;
            blocks.push(root);
            blocks.extend(decision_blocks);
            next_stage
        } else {
            let next_stage = block_id(*next_block_identity);
            *next_block_identity = next_block_identity
                .checked_add(1)
                .expect("staged direct call-argument blocks advance");
            let operation_start = operations.len();
            let value = emit_direct_expression(
                argument,
                &current_parameters,
                next_value_identity,
                operations,
            );
            let mut arguments = carried_arguments;
            arguments.push(value);
            let edge = edge_id(*next_edge_identity);
            *next_edge_identity = next_edge_identity
                .checked_add(1)
                .expect("staged direct call-argument edge identities advance");
            blocks.push(Block {
                structural_parameters: Vec::new(),
                id: current_block,
                parameters: current_block_parameters,
                erased_scalar_formals: Vec::new(),
                operations: operations[operation_start..].to_vec(),
                terminator: Terminator::Jump {
                    structural_arguments: Vec::new(),
                    edge,
                    erased_arguments: Vec::new(),
                    target: next_stage,
                    arguments,
                    residual_affine_discards: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
            });
            next_stage
        };
        current_block = next_stage;
        current_parameters = next_stage_parameters;
        current_block_parameters = current_parameters.clone();
    }

    let continuation = block_id(*next_block_identity);
    *next_block_identity = next_block_identity
        .checked_add(1)
        .expect("staged call continuation block identities advance");
    let operation_start = operations.len();
    let arguments = current_parameters[caller_value_count..].to_vec();
    let result = emit_direct_call_operation(
        call,
        &call.parameter_relative_crash_routes,
        &arguments,
        &current_parameters[..caller_value_count],
        caller_erased_formals,
        &arguments,
        next_value_identity,
        operations,
        call_emission,
    )?;
    let mut continuation_arguments = current_parameters[..caller_value_count]
        .iter()
        .map(|parameter| parameter.id)
        .collect::<Vec<_>>();
    continuation_arguments.push(result);
    let edge = edge_id(*next_edge_identity);
    *next_edge_identity = next_edge_identity
        .checked_add(1)
        .expect("staged call continuation edge identities advance");
    blocks.push(Block {
        structural_parameters: Vec::new(),
        id: current_block,
        parameters: current_block_parameters,
        erased_scalar_formals: Vec::new(),
        operations: operations[operation_start..].to_vec(),
        terminator: Terminator::Jump {
            structural_arguments: Vec::new(),
            edge,
            target: continuation,
            arguments: continuation_arguments,
            erased_arguments: Vec::new(),
            residual_affine_discards: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    });
    Ok((continuation, blocks))
}

fn emit_direct_call_operation(
    call: &LoweredDirectCallBinding,
    crash_routes: &[checked_trees::CrashRouteBucket],
    crash_values: &[ValueDeclaration],
    source_values_before_call: &[ValueDeclaration],
    caller_erased_formals: &[ValueDeclaration],
    arguments: &[ValueDeclaration],
    next_value_identity: &mut u64,
    operations: &mut OperationBuffer,
    call_emission: &mut CallEmissionContext<'_>,
) -> Result<ValueId, LoweringError> {
    let callee = call_emission
        .machine_ids
        .iter()
        .find_map(|(source, terminal)| (*source == call.target_machine).then_some(*terminal))
        .ok_or(LoweringError::Unsupported(
            "direct scalar call target is absent from the terminal closure",
        ))?;
    let crash_continuations = lower_checked_crash_route_buckets(crash_routes, crash_values)?;
    let requirement_count = call_emission
        .requirement_counts
        .iter()
        .find_map(|(source, count)| (*source == call.target_machine).then_some(*count))
        .ok_or(LoweringError::Unsupported(
            "direct scalar call target has no prepared contract",
        ))?;
    let requirement_obligations = (0..requirement_count)
        .map(|_| call_emission.allocate_requirement())
        .collect::<Result<Vec<_>, _>>()?;
    let result = value_id(*next_value_identity);
    *next_value_identity = next_value_identity
        .checked_add(1)
        .expect("generated value identity advances after a direct call");
    let operation = operations.allocate();
    operations.record_source_call_with_values(
        call.source_coordinate,
        None,
        operation,
        call.target_machine,
        source_values_before_call,
    )?;
    let erased_arguments = call
        .erased_arguments
        .iter()
        .map(|argument| {
            lowered_direct_scalar_term(argument, source_values_before_call, caller_erased_formals)
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    operations.push(Operation {
        static_reach_binding: None,
        id: operation,
        result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
            qualifications: call.result_type.qualifications,
            id: result,
            scalar_type: call.result_type.scalar_type,
        }),
        kind: if call.structural_arguments.is_empty() && !call.uses_structural_frame {
            OperationKind::Call {
                callee,
                arguments: arguments.iter().map(|argument| argument.id).collect(),
                erased_arguments,
                requirement_obligations,
                crash_continuations,
            }
        } else {
            OperationKind::CallStructuralScalar {
                callee,
                arguments: arguments.iter().map(|argument| argument.id).collect(),
                erased_arguments,
                structural_arguments: call.structural_arguments.clone(),
                claim_transfers: Vec::new(),
                requirement_obligations,
                crash_continuations,
            }
        },
    });
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{CallEmissionContext, LoweringError};

    #[test]
    fn call_requirements_cannot_escape_their_reserved_identity_range() {
        let mut context = CallEmissionContext {
            machine_ids: &[],
            requirement_counts: &[],
            next_obligation_identity: 7,
            obligation_limit: 9,
        };
        assert_eq!(context.allocate_requirement().unwrap().get(), 7);
        assert_eq!(context.allocate_requirement().unwrap().get(), 8);
        for _ in 0..2 {
            assert!(matches!(
                context.allocate_requirement(),
                Err(LoweringError::Unsupported(
                    "terminal call obligations exceed their machine identity range"
                ))
            ));
            assert_eq!(context.next_obligation_identity, 9);
        }
    }
}
