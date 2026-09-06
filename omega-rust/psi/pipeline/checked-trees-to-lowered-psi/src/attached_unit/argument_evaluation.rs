//! Scalar operand evaluation within an existing machine's structural frontier.

use super::*;
use checked_trees::CheckedCallScalarArgument;

pub(crate) struct Evaluation {
    pub entry: BlockId,
    pub current: BlockId,
    pub parameters: Vec<ValueDeclaration>,
    pub operation_start: usize,
    pub blocks: Vec<Block>,
}

impl Evaluation {
    pub(crate) fn new(next_block: &mut u64) -> Result<Self, LoweringError> {
        let entry = block_id(allocate_dense(next_block)?);
        Ok(Self {
            entry,
            current: entry,
            parameters: Vec::new(),
            operation_start: 0,
            blocks: Vec::new(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn arguments(
        &mut self,
        checked: &CheckedTrees,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        operation: &CheckedUnitEffectOperationPlan,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
    ) -> Result<Option<Vec<ValueDeclaration>>, LoweringError> {
        let (coordinate, arguments, boundary) = match operation {
            CheckedUnitEffectOperationPlan::CallUnit {
                coordinate,
                scalar_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::ScalarCall {
                coordinate,
                scalar_arguments,
                ..
            } => (*coordinate, scalar_arguments, false),
            CheckedUnitEffectOperationPlan::BoundaryCall {
                coordinate,
                scalar_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                coordinate,
                scalar_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                coordinate,
                scalar_arguments,
                ..
            } => (*coordinate, scalar_arguments, true),
            _ => return Ok(None),
        };
        let source_types = values
            .iter()
            .map(|value| value.scalar_type)
            .collect::<Vec<_>>();
        let argument_types = arguments
            .iter()
            .map(|argument| match argument {
                CheckedCallScalarArgument::Pure(expression) => {
                    Ok(lower_checked_scalar_expression(expression)?.scalar_type())
                }
                CheckedCallScalarArgument::Computation(root) => {
                    let nodes = &checked.facts.values.scalar_computations.nodes;
                    if !nodes.is_valid(*root) {
                        return unsupported("call argument computation has no live root");
                    }
                    terminal_scalar_type(nodes.get(*root).primitive_type)
                }
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let needs_control = arguments.iter().any(|argument| match argument {
            CheckedCallScalarArgument::Computation(_) => true,
            CheckedCallScalarArgument::Pure(expression) => {
                lower_checked_scalar_expression(expression)
                    .is_ok_and(|expression| direct_expression_contains_short_circuit(&expression))
            }
        });
        if !needs_control {
            return arguments
                .iter()
                .zip(&argument_types)
                .map(|(argument, scalar_type)| {
                    let expression = lower_checked_scalar_expression(argument.as_pure().ok_or(
                        LoweringError::Unsupported(
                            "computed call argument requires ordered control",
                        ),
                    )?)?;
                    validate_direct_parameter_types(&expression, &source_types)?;
                    Ok(ValueDeclaration {
                        id: emit_direct_expression(&expression, values, next_value, operations),
                        scalar_type: *scalar_type,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()
                .map(Some);
        }

        let mut expansion = crate::scalar_computations::Expansion::new(checked, machine, 1);
        let entry_index = expansion.call_arguments(
            state,
            coordinate,
            boundary,
            arguments,
            &crate::scalar_bindings::ScalarBindings::new(values.len()),
            &source_types,
            0,
        )?;
        let states = expansion.finish();
        let mut completion_types = source_types.clone();
        completion_types.extend(argument_types);
        let completion_parameters = declarations(&completion_types, next_value)?;
        let completion = block_id(allocate_dense(next_block)?);
        let mut targets = vec![completion];
        let mut parameters = vec![completion_parameters.clone()];
        for state in &states {
            targets.push(block_id(allocate_dense(next_block)?));
            parameters.push(declarations(&state.parameter_types, next_value)?);
        }
        self.blocks.push(Block {
            id: self.current,
            parameters: std::mem::take(&mut self.parameters),
            operations: operations[self.operation_start..].to_vec(),
            terminator: Terminator::Jump {
                edge: edge_id(allocate_dense(next_edge)?),
                target: *targets.get(entry_index).ok_or(LoweringError::Unsupported(
                    "call computation entry is absent",
                ))?,
                arguments: values.iter().map(|value| value.id).collect(),
                trivial_affine_discards: Vec::new(),
            },
        });
        for (index, state) in states.iter().enumerate() {
            emit_state(
                state,
                targets[index + 1],
                &parameters[index + 1],
                &targets,
                next_value,
                next_block,
                next_edge,
                operations,
                calls,
                &mut self.blocks,
            )?;
        }
        let result = completion_parameters[source_types.len()..].to_vec();
        *values = completion_parameters[..source_types.len()].to_vec();
        self.current = completion;
        self.parameters = completion_parameters;
        self.operation_start = operations.len();
        Ok(Some(result))
    }
}

fn declarations(
    types: &[ScalarType],
    next_value: &mut u64,
) -> Result<Vec<ValueDeclaration>, LoweringError> {
    types
        .iter()
        .map(|scalar_type| {
            Ok(ValueDeclaration {
                id: value_id(allocate_dense(next_value)?),
                scalar_type: *scalar_type,
            })
        })
        .collect()
}

pub(crate) fn validated_values(
    values: Option<&[ValueDeclaration]>,
    types: &[ScalarType],
) -> Result<Vec<ValueDeclaration>, LoweringError> {
    let values = values.ok_or(LoweringError::Unsupported(
        "call has no completed scalar operands",
    ))?;
    if values.len() != types.len()
        || values
            .iter()
            .zip(types)
            .any(|(value, scalar_type)| value.scalar_type != *scalar_type)
    {
        return unsupported("completed call scalar operands disagree with the target signature");
    }
    Ok(values.to_vec())
}

#[allow(clippy::too_many_arguments)]
fn emit_state(
    state: &LoweredScalarBranchState,
    mut block: BlockId,
    parameters: &[ValueDeclaration],
    targets: &[BlockId],
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
    calls: &mut CallEmissionContext<'_>,
    blocks: &mut Vec<Block>,
) -> Result<(), LoweringError> {
    let mut values = parameters.to_vec();
    let mut block_parameters = parameters.to_vec();
    let mut operation_start = operations.len();
    for binding in &state.bindings {
        if let LoweredScalarBinding::Expression(LoweredDirectExpression::Boolean { expression }) =
            binding
            && contains_short_circuit(expression)
        {
            let decision = lower_boolean_value_decision(expression);
            let count = boolean_decision_block_count(&decision);
            let reserved_root = *next_block;
            *next_block = next_block
                .checked_add(u64::try_from(count).map_err(|_| {
                    LoweringError::Unsupported("call Boolean block count exceeds u64")
                })?)
                .ok_or(LoweringError::Unsupported(
                    "call Boolean block identities exhausted",
                ))?;
            let continuation = block_id(allocate_dense(next_block)?);
            let mut types = values
                .iter()
                .map(|value| value.scalar_type)
                .collect::<Vec<_>>();
            types.push(binding.scalar_type());
            let continuation_parameters = declarations(&types, next_value)?;
            let prefix = operations[operation_start..].to_vec();
            let mut decisions = Vec::new();
            emit_reserved_boolean_tuple_stage_blocks(
                &decision,
                &values,
                block_parameters,
                continuation,
                &values.iter().map(|value| value.id).collect::<Vec<_>>(),
                reserved_root,
                next_value,
                next_edge,
                operations,
                &mut decisions,
            );
            for (index, decision) in decisions.into_iter().enumerate() {
                let mut decision = decision.ok_or(LoweringError::Unsupported(
                    "call Boolean block is incomplete",
                ))?;
                if index == 0 {
                    decision.id = block;
                    decision.operations.splice(0..0, prefix.iter().cloned());
                }
                blocks.push(decision);
            }
            block = continuation;
            values = continuation_parameters.clone();
            block_parameters = continuation_parameters;
            operation_start = operations.len();
        } else {
            let id = emit_scalar_binding(binding, &values, next_value, operations, calls)?;
            values.push(ValueDeclaration {
                id,
                scalar_type: binding.scalar_type(),
            });
        }
    }
    let mut arguments =
        |expressions: &[LoweredDirectExpression]| -> Result<Vec<ValueId>, LoweringError> {
            expressions
                .iter()
                .map(|expression| {
                    validate_direct_parameter_types(
                        expression,
                        &values
                            .iter()
                            .map(|value| value.scalar_type)
                            .collect::<Vec<_>>(),
                    )?;
                    if direct_expression_contains_short_circuit(expression) {
                        return unsupported(
                            "call computation transfer retains unexpanded Boolean control",
                        );
                    }
                    Ok(emit_direct_expression(
                        expression, &values, next_value, operations,
                    ))
                })
                .collect()
        };
    let terminator = match &state.terminator {
        LoweredScalarBranchTerminator::Jump {
            target,
            arguments: outgoing,
        } => Terminator::Jump {
            edge: edge_id(allocate_dense(next_edge)?),
            target: *targets.get(*target).ok_or(LoweringError::Unsupported(
                "call computation target is absent",
            ))?,
            arguments: arguments(outgoing)?,
            trivial_affine_discards: Vec::new(),
        },
        LoweredScalarBranchTerminator::Conditional {
            condition,
            when_true_target,
            when_true_arguments,
            when_false_target,
            when_false_arguments,
        } => {
            let when_true_arguments = arguments(when_true_arguments)?;
            let when_false_arguments = arguments(when_false_arguments)?;
            let condition = emit_boolean_expression(condition, &values, next_value, operations);
            Terminator::Conditional {
                condition,
                when_true: SuccessorEdge {
                    edge: edge_id(allocate_dense(next_edge)?),
                    target: *targets
                        .get(*when_true_target)
                        .ok_or(LoweringError::Unsupported("call true target is absent"))?,
                    arguments: when_true_arguments,
                    trivial_affine_discards: Vec::new(),
                },
                when_false: SuccessorEdge {
                    edge: edge_id(allocate_dense(next_edge)?),
                    target: *targets
                        .get(*when_false_target)
                        .ok_or(LoweringError::Unsupported("call false target is absent"))?,
                    arguments: when_false_arguments,
                    trivial_affine_discards: Vec::new(),
                },
            }
        }
        _ => {
            return unsupported(
                "call operand computation cannot return from its enclosing machine",
            );
        }
    };
    blocks.push(Block {
        id: block,
        parameters: block_parameters,
        operations: operations[operation_start..].to_vec(),
        terminator,
    });
    Ok(())
}
