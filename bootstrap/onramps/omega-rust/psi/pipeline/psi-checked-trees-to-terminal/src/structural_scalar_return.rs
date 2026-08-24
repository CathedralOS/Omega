//! Structural scalar-return lowering and nominal cleanup specialization.
//!
//! This module owns the structural-custody return cohort. General structural
//! returns, structural Unit control, and scalar-graph construction remain
//! separate producer responsibilities in the crate root.

use super::*;

mod expressions;
mod nominal;
use expressions::*;
use nominal::lower_nominal_structural_scalar_return_machine;

pub(super) fn lower_trait_operator_scalar_return_machine(
    checked: &CheckedTrees,
    plan: &psi_checked_trees::CheckedTraitOperatorScalarReturnMachinePlan,
) -> Result<LoweredTerminalPsi, LoweringError> {
    let realization_matches = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .machines
        .iter()
        .filter(|candidate| {
            candidate.machine == plan.realization_machine
                && candidate.state == plan.realization_state
        });
    let realization_matches = realization_matches.collect::<Vec<_>>();
    if realization_matches.len() > 1 {
        return unsupported("trait operator realization has duplicate checked scalar bodies");
    }
    let specialization = checked
        .machine_specializations
        .iter()
        .find(|specialization| specialization.instance == plan.machine)
        .ok_or(LoweringError::Unsupported(
            "trait operator caller lost its checked specialization",
        ))?;
    let application = specialization
        .conformance_applications
        .iter()
        .find(|application| {
            application.declaration == plan.conformance
                && application.fingerprint == plan.conformance_application_fingerprint
        })
        .ok_or(LoweringError::Unsupported(
            "trait operator caller lost its exact closed conformance application",
        ))?;
    let row = application
        .rows
        .iter()
        .find(|row| row.requirement == plan.requirement)
        .ok_or(LoweringError::Unsupported(
            "trait operator caller lost its selected requirement row",
        ))?;
    if row.realization_machine != plan.realization_machine
        || row.realization_state != plan.realization_state
    {
        return unsupported("trait operator realization no longer matches its selected row");
    }

    let synthesized_realization;
    let realization = if let [realization] = realization_matches.as_slice() {
        *realization
    } else {
        let structural_parameters = plan
            .argument_source_positions
            .iter()
            .enumerate()
            .map(|(target_position, source_position)| {
                let mut parameter = plan
                    .structural_parameters
                    .iter()
                    .find(|parameter| parameter.position == *source_position)
                    .cloned()
                    .ok_or(LoweringError::Unsupported(
                        "trait operator realization source parameter is absent",
                    ))?;
                parameter.position = u32::try_from(target_position).map_err(|_| {
                    LoweringError::Unsupported(
                        "trait operator realization parameter count exceeds u32",
                    )
                })?;
                Ok(parameter)
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let attachment_type_identity = structural_parameters
            .first()
            .map(|parameter| parameter.type_identity.clone())
            .ok_or(LoweringError::Unsupported(
                "trait operator realization has no structural carrier",
            ))?;
        synthesized_realization = CheckedStructuralScalarReturnMachinePlan {
            machine: plan.realization_machine,
            state: plan.realization_state,
            attachment_type_identity,
            structural_parameters,
            scalar_parameters: Vec::new(),
            bindings: Vec::new(),
            result_type: plan.result_type,
            return_statement_ordinal: 0,
            shared_boolean_convergence: None,
            caller_requirements: Vec::new(),
            scalar_requirements: Vec::new(),
            cleanup_actions: (0..plan.argument_source_positions.len())
                .rev()
                .map(|position| {
                    u32::try_from(position)
                        .map(CheckedStructuralScalarReturnCleanupAction::DiscardRoot)
                        .map_err(|_| {
                            LoweringError::Unsupported(
                                "trait operator cleanup parameter count exceeds u32",
                            )
                        })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?,
        };
        &synthesized_realization
    };

    let mut staged = checked.clone();
    staged.facts.values.scalar_expressions.expressions.push(
        psi_checked_trees::CheckedLocatedScalarExpression {
            state: plan.realization_state,
            statement_ordinal: 0,
            role: CheckedScalarExpressionRole::Return,
            expression: plan.realization_return_expression.clone(),
        },
    );
    let mut lowered = lower_structural_scalar_return_machine_in_namespace(
        &staged,
        realization,
        machine_id(2),
        TERMINAL_MACHINE_IDENTITY_STRIDE,
    )?;
    let type_ids = lowered
        .semantic_module
        .structural_types
        .iter()
        .map(|declaration| (declaration.identity.clone(), declaration.id))
        .collect::<Vec<_>>();
    let mut positions = BTreeSet::new();
    for parameter in &plan.structural_parameters {
        if parameter.is_self
            || parameter.multiplicity != Multiplicity::Affine
            || !parameter.qualifications.is_empty()
            || !positions.insert(parameter.position)
        {
            return unsupported("trait operator caller is not claim-free affine custody");
        }
        lookup_type_id(&type_ids, &parameter.type_identity)?;
    }
    if positions.len() != plan.structural_parameters.len()
        || positions
            .iter()
            .copied()
            .enumerate()
            .any(|(index, position)| u32::try_from(index).ok() != Some(position))
    {
        return unsupported("trait operator caller parameters are not dense source positions");
    }
    if plan.argument_source_positions.len() != plan.structural_parameters.len()
        || plan
            .argument_source_positions
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            != positions
    {
        return unsupported("trait operator operands are not an exact parameter permutation");
    }
    let result_type = terminal_scalar_type(plan.result_type)?;
    if result_type != terminal_scalar_type(realization.result_type)? {
        return unsupported("trait operator result disagrees with its realization");
    }
    let mut next_place = 1_u64;
    let structural_parameters =
        lower_unit_parameters(&plan.structural_parameters, &type_ids, &[], &mut next_place)?;
    let callee = lowered
        .semantic_module
        .machines
        .first()
        .ok_or(LoweringError::Unsupported(
            "trait operator realization produced no terminal machine",
        ))?;
    if callee.id != machine_id(2)
        || callee.parameters.len() != 0
        || callee.structural_parameters.len() != structural_parameters.len()
        || callee.result.scalar().map(|result| result.scalar_type) != Some(result_type)
    {
        return unsupported("trait operator realization has an incompatible terminal signature");
    }
    let structural_arguments = plan
        .argument_source_positions
        .iter()
        .zip(&callee.structural_parameters)
        .map(|(source_position, target)| {
            let source = plan
                .structural_parameters
                .iter()
                .position(|parameter| parameter.position == *source_position)
                .and_then(|index| structural_parameters.get(index))
                .ok_or(LoweringError::Unsupported(
                    "trait operator operand source is absent from the terminal signature",
                ))?;
            if source.structural_type != target.structural_type
                || source.multiplicity != target.multiplicity
                || source.access != target.access
                || source.qualifications != target.qualifications
            {
                return Err(LoweringError::Unsupported(
                    "trait operator operand disagrees with the realization parameter",
                ));
            }
            Ok(StructuralArgument {
                place: source.place,
                path: Vec::new(),
                access: target.access,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let call_result = ValueDeclaration {
        id: value_id(1),
        scalar_type: result_type,
    };
    let machine_result = ValueDeclaration {
        id: value_id(2),
        scalar_type: result_type,
    };
    let caller = TerminalMachine {
        id: machine_id(1),
        attachment: plan
            .attachment_type_identity
            .as_ref()
            .map(|identity| lookup_type_id(&type_ids, identity))
            .transpose()?,
        parameters: Vec::new(),
        structural_parameters: structural_parameters.clone(),
        result: TerminalMachineResult::Scalar(machine_result),
        structural_places: structural_parameters
            .iter()
            .map(|parameter| StructuralPlaceDeclaration {
                id: parameter.place,
                kind: StructuralPlaceKind::Parameter {
                    position: parameter.position,
                    is_self: parameter.is_self,
                },
            })
            .collect(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(1),
        blocks: vec![Block {
            id: block_id(1),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: operation_id(1),
                result: psi_terminal::OperationResult::Scalar(call_result),
                kind: OperationKind::CallStructuralScalar {
                    callee: machine_id(2),
                    structural_arguments,
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            }],
            terminator: Terminator::Return {
                edge: edge_id(1),
                value: call_result.id,
                cleanup_actions: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: contract_id(1),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
        },
    };
    lowered.semantic_module.entry = caller.id;
    lowered.semantic_module.machines.insert(0, caller);
    finalize_operation_proofs(&mut lowered)?;
    Ok(lowered)
}

pub(super) fn lower_structural_scalar_return_machine(
    checked: &CheckedTrees,
    plan: &CheckedStructuralScalarReturnMachinePlan,
) -> Result<LoweredTerminalPsi, LoweringError> {
    if plan.cleanup_actions.iter().any(|action| {
        matches!(
            action,
            CheckedStructuralScalarReturnCleanupAction::InvokeNominal(_)
        )
    }) {
        return lower_nominal_structural_scalar_return_machine(checked, plan);
    }
    lower_structural_scalar_return_machine_in_namespace(checked, plan, machine_id(1), 0)
}

fn lower_structural_scalar_return_machine_in_namespace(
    checked: &CheckedTrees,
    plan: &CheckedStructuralScalarReturnMachinePlan,
    terminal_machine: MachineId,
    identity_base: u64,
) -> Result<LoweredTerminalPsi, LoweringError> {
    if plan.cleanup_actions.iter().any(|action| {
        matches!(
            action,
            CheckedStructuralScalarReturnCleanupAction::InvokeNominal(_)
        )
    }) {
        return unsupported("namespaced structural scalar callees require direct root cleanup");
    }
    let (structural_types, type_ids) = lower_structural_type_plans(
        &checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .structural_types,
    )?;
    if plan.structural_parameters.is_empty() {
        return unsupported("structural scalar return has no structural parameters");
    }
    let mut positions = BTreeSet::new();
    for parameter in &plan.structural_parameters {
        if parameter.is_self
            || parameter.multiplicity != Multiplicity::Affine
            || !parameter.qualifications.is_empty()
            || !positions.insert(parameter.position)
        {
            return unsupported(
                "structural scalar return signature is not claim-free affine custody",
            );
        }
        lookup_type_id(&type_ids, &parameter.type_identity)?;
    }
    for parameter in &plan.scalar_parameters {
        if !positions.insert(parameter.source_position) {
            return unsupported(
                "structural scalar return parameter maps overlap or repeat a source position",
            );
        }
        terminal_scalar_type(parameter.primitive_type)?;
    }
    let parameter_count = plan
        .structural_parameters
        .len()
        .checked_add(plan.scalar_parameters.len())
        .ok_or(LoweringError::Unsupported(
            "structural scalar return parameter count exceeds usize",
        ))?;
    if positions.len() != parameter_count
        || positions
            .iter()
            .copied()
            .enumerate()
            .any(|(index, position)| u32::try_from(index).ok() != Some(position))
    {
        return unsupported(
            "structural scalar return parameter maps do not partition source positions",
        );
    }
    let expected_cleanup = plan
        .structural_parameters
        .iter()
        .rev()
        .map(|parameter| {
            CheckedStructuralScalarReturnCleanupAction::DiscardRoot(parameter.position)
        })
        .collect::<Vec<_>>();
    if plan.cleanup_actions != expected_cleanup {
        return unsupported("structural scalar return cleanup does not consume its exact frontier");
    }
    let expected_return_ordinal = u32::try_from(plan.bindings.len()).map_err(|_| {
        LoweringError::Unsupported("structural scalar return binding count exceeds u32")
    })?;
    if plan.return_statement_ordinal != expected_return_ordinal {
        return unsupported("structural scalar return coordinates are not a contiguous prefix");
    }
    let result_type = terminal_scalar_type(plan.result_type)?;

    let mut next_place = identity_base
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "structural place namespace overflows",
        ))?;
    let structural_parameters =
        lower_unit_parameters(&plan.structural_parameters, &type_ids, &[], &mut next_place)?;
    let cleanup = plan
        .cleanup_actions
        .iter()
        .map(|action| {
            let CheckedStructuralScalarReturnCleanupAction::DiscardRoot(position) = action else {
                return Err(LoweringError::Unsupported(
                    "structural scalar return trivial lane acquired a nominal cleanup",
                ));
            };
            let parameter_index = plan
                .structural_parameters
                .iter()
                .position(|parameter| parameter.position == *position)
                .ok_or(LoweringError::Unsupported(
                    "structural scalar return cleanup position is absent from its signature",
                ))?;
            structural_parameters
                .get(parameter_index)
                .map(|parameter| parameter.place)
                .ok_or(LoweringError::Unsupported(
                    "structural scalar return cleanup position has no terminal place",
                ))
        })
        .map(|place| place.map(TerminalAffineCleanupAction::DiscardRoot))
        .collect::<Result<Vec<_>, _>>()?;
    let mut operations = OperationBuffer::new(identity_base);
    let mut next_value = identity_base
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "structural value namespace overflows",
        ))?;
    let scalar_parameters = plan
        .scalar_parameters
        .iter()
        .map(|parameter| {
            let value = ValueDeclaration {
                id: value_id(allocate_dense(&mut next_value)?),
                scalar_type: terminal_scalar_type(parameter.primitive_type)?,
            };
            Ok(value)
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let scalar_parameter_count = scalar_parameters.len();
    let mut scalar_values = Vec::with_capacity(
        scalar_parameter_count
            .checked_add(plan.bindings.len())
            .ok_or(LoweringError::Unsupported(
                "structural scalar value namespace exceeds usize",
            ))?,
    );
    scalar_values.extend_from_slice(&scalar_parameters);
    let mut staged_short_circuit_bindings = Vec::new();
    for (binding_index, binding) in plan.bindings.iter().enumerate() {
        let statement_ordinal = u32::try_from(binding_index).map_err(|_| {
            LoweringError::Unsupported("structural scalar return binding index exceeds u32")
        })?;
        if binding.statement_ordinal != statement_ordinal
            || binding.value != CheckedScalarBindingValue::Expression
        {
            return unsupported(
                "structural scalar return bindings are not a direct expression prefix",
            );
        }
        let expression = lower_checked_scalar_expression_at(
            checked,
            plan.state,
            statement_ordinal,
            CheckedScalarExpressionRole::LocalInitializer {
                binding_ordinal: statement_ordinal,
            },
        )?;
        if let LoweredDirectExpression::Boolean { expression } = expression
            && contains_short_circuit(&expression)
        {
            if binding.primitive_type != PrimitiveType::Bool {
                return unsupported(
                    "structural scalar short-circuit binding has a non-Boolean carrier",
                );
            }
            staged_short_circuit_bindings.push((binding_index, *expression));
        }
    }
    for (binding_index, binding) in plan
        .bindings
        .iter()
        .enumerate()
        .filter(|(binding_index, _)| {
            staged_short_circuit_bindings
                .first()
                .is_none_or(|(staged_index, _)| binding_index < staged_index)
        })
    {
        let statement_ordinal = u32::try_from(binding_index).map_err(|_| {
            LoweringError::Unsupported("structural scalar return binding index exceeds u32")
        })?;
        if binding.statement_ordinal != statement_ordinal
            || binding.value != CheckedScalarBindingValue::Expression
        {
            return unsupported(
                "structural scalar return bindings are not a direct expression prefix",
            );
        }
        let scalar_type = terminal_scalar_type(binding.primitive_type)?;
        let expression = lower_checked_scalar_expression_at(
            checked,
            plan.state,
            statement_ordinal,
            CheckedScalarExpressionRole::LocalInitializer {
                binding_ordinal: statement_ordinal,
            },
        )?;
        if !is_branch_free_structural_scalar_expression(
            &expression,
            scalar_parameter_count,
            binding_index,
        ) {
            return unsupported(
                "structural scalar binding is not one branch-free local expression",
            );
        }
        if expression.scalar_type() != scalar_type {
            return unsupported(
                "structural scalar binding value does not match its checked local type",
            );
        }
        validate_direct_parameter_types(
            &expression,
            &scalar_values
                .iter()
                .map(|value: &ValueDeclaration| value.scalar_type)
                .collect::<Vec<_>>(),
        )?;
        let id = emit_direct_expression(
            &expression,
            &scalar_values,
            &mut next_value,
            &mut operations,
        );
        scalar_values.push(ValueDeclaration { id, scalar_type });
    }
    let expression = lower_checked_scalar_expression_at(
        checked,
        plan.state,
        plan.return_statement_ordinal,
        CheckedScalarExpressionRole::Return,
    )?;
    if !is_structural_scalar_return_expression(
        &expression,
        scalar_parameter_count,
        plan.bindings.len(),
    ) {
        return unsupported("structural scalar return is outside its checked value/control slice");
    }
    if expression.scalar_type() != result_type {
        return unsupported(
            "structural scalar return value does not match its checked result type",
        );
    }
    let blocks = if !staged_short_circuit_bindings.is_empty() {
        let mut next_edge = identity_base
            .checked_add(1)
            .ok_or(LoweringError::Unsupported(
                "structural edge namespace overflows",
            ))?;
        let mut next_block = block_id(identity_base.checked_add(1).ok_or(
            LoweringError::Unsupported("structural block namespace overflows"),
        )?);
        let mut next_block_parameters = Vec::new();
        let mut operation_start = 0;
        let mut blocks = Vec::new();
        for (stage_position, (staged_index, short_circuit_binding)) in
            staged_short_circuit_bindings.iter().enumerate()
        {
            validate_boolean_parameter_types(
                short_circuit_binding,
                &scalar_values
                    .iter()
                    .map(|value| value.scalar_type)
                    .collect::<Vec<_>>(),
            )?;
            let decision = lower_boolean_value_decision(short_circuit_binding);
            let decision_block_count = boolean_decision_block_count(&decision);
            let continuation = block_id(
                next_block
                    .get()
                    .checked_add(u64::try_from(decision_block_count).map_err(|_| {
                        LoweringError::Unsupported(
                            "structural scalar local decision block count exceeds u64",
                        )
                    })?)
                    .ok_or(LoweringError::Unsupported(
                        "structural scalar local continuation identity overflows",
                    ))?,
            );
            let decision_operation_start = operations.operations.len();
            let first_synthetic_block = block_id(next_block.get().checked_add(1).ok_or(
                LoweringError::Unsupported(
                    "structural scalar local decision block identity overflows",
                ),
            )?);
            let (mut root, mut children) = emit_inlined_boolean_value_blocks(
                &decision,
                &scalar_values,
                next_block_parameters,
                LoweredBooleanDecisionExit::Jump {
                    target: continuation,
                },
                next_block,
                first_synthetic_block,
                &mut next_value,
                &mut next_edge,
                &mut operations,
            );
            let mut root_operations =
                operations.operations[operation_start..decision_operation_start].to_vec();
            root_operations.extend(root.operations);
            root.operations = root_operations;
            blocks.push(root);
            blocks.append(&mut children);

            let local = ValueDeclaration {
                id: value_id(allocate_dense(&mut next_value)?),
                scalar_type: ScalarType::Boolean,
            };
            scalar_values.push(local);
            next_block = continuation;
            next_block_parameters = vec![local];
            operation_start = operations.operations.len();

            let next_staged_index = staged_short_circuit_bindings
                .get(stage_position + 1)
                .map_or(plan.bindings.len(), |(binding_index, _)| *binding_index);
            for binding_index in staged_index + 1..next_staged_index {
                let binding = &plan.bindings[binding_index];
                let statement_ordinal = u32::try_from(binding_index).map_err(|_| {
                    LoweringError::Unsupported("structural scalar return binding index exceeds u32")
                })?;
                let scalar_type = terminal_scalar_type(binding.primitive_type)?;
                let continuation_expression = lower_checked_scalar_expression_at(
                    checked,
                    plan.state,
                    statement_ordinal,
                    CheckedScalarExpressionRole::LocalInitializer {
                        binding_ordinal: statement_ordinal,
                    },
                )?;
                if !is_branch_free_structural_scalar_expression(
                    &continuation_expression,
                    scalar_parameter_count,
                    binding_index,
                ) {
                    return unsupported(
                        "structural scalar continuation binding is not branch-free",
                    );
                }
                if continuation_expression.scalar_type() != scalar_type {
                    return unsupported(
                        "structural scalar continuation binding does not match its checked local type",
                    );
                }
                validate_direct_parameter_types(
                    &continuation_expression,
                    &scalar_values
                        .iter()
                        .map(|value| value.scalar_type)
                        .collect::<Vec<_>>(),
                )?;
                let id = emit_direct_expression(
                    &continuation_expression,
                    &scalar_values,
                    &mut next_value,
                    &mut operations,
                );
                scalar_values.push(ValueDeclaration { id, scalar_type });
            }
        }
        if let LoweredDirectExpression::Boolean { expression } = &expression
            && contains_short_circuit(expression)
        {
            validate_boolean_parameter_types(
                expression,
                &scalar_values
                    .iter()
                    .map(|value| value.scalar_type)
                    .collect::<Vec<_>>(),
            )?;
            let decision_operation_start = operations.operations.len();
            let decision = lower_boolean_value_decision(expression);
            let first_synthetic_block = block_id(next_block.get().checked_add(1).ok_or(
                LoweringError::Unsupported(
                    "structural scalar return decision block identity overflows",
                ),
            )?);
            let (mut root, mut children) = emit_inlined_boolean_value_blocks(
                &decision,
                &scalar_values,
                next_block_parameters,
                LoweredBooleanDecisionExit::Return,
                next_block,
                first_synthetic_block,
                &mut next_value,
                &mut next_edge,
                &mut operations,
            );
            let mut root_operations =
                operations.operations[operation_start..decision_operation_start].to_vec();
            root_operations.extend(root.operations);
            root.operations = root_operations;
            let final_decision_start = blocks.len();
            blocks.push(root);
            blocks.append(&mut children);
            for block in &mut blocks[final_decision_start..] {
                if let Terminator::Return {
                    cleanup_actions, ..
                } = &mut block.terminator
                {
                    *cleanup_actions = cleanup.clone();
                }
            }
        } else {
            validate_direct_parameter_types(
                &expression,
                &scalar_values
                    .iter()
                    .map(|value| value.scalar_type)
                    .collect::<Vec<_>>(),
            )?;
            let value = emit_direct_expression(
                &expression,
                &scalar_values,
                &mut next_value,
                &mut operations,
            );
            blocks.push(Block {
                id: next_block,
                parameters: next_block_parameters,
                operations: operations.operations[operation_start..].to_vec(),
                terminator: Terminator::Return {
                    edge: edge_id(next_edge),
                    value,
                    cleanup_actions: cleanup,
                },
            });
        }
        blocks
    } else if let LoweredDirectExpression::Boolean { expression } = &expression
        && contains_short_circuit(expression)
    {
        validate_boolean_parameter_types(
            expression,
            &scalar_values
                .iter()
                .map(|value| value.scalar_type)
                .collect::<Vec<_>>(),
        )?;
        let entry_operation_count = operations.operations.len();
        let decision = lower_boolean_value_decision(expression);
        let mut next_edge = identity_base
            .checked_add(1)
            .ok_or(LoweringError::Unsupported(
                "structural edge namespace overflows",
            ))?;
        let (mut root, mut children) = emit_inlined_boolean_value_blocks(
            &decision,
            &scalar_values,
            Vec::new(),
            LoweredBooleanDecisionExit::Return,
            block_id(
                identity_base
                    .checked_add(1)
                    .expect("validated identity base"),
            ),
            block_id(
                identity_base
                    .checked_add(2)
                    .expect("validated identity base"),
            ),
            &mut next_value,
            &mut next_edge,
            &mut operations,
        );
        let mut entry_operations = operations.operations[..entry_operation_count].to_vec();
        entry_operations.extend(root.operations);
        root.operations = entry_operations;
        let mut blocks = Vec::with_capacity(1_usize.checked_add(children.len()).ok_or(
            LoweringError::Unsupported("structural scalar return block count exceeds usize"),
        )?);
        blocks.push(root);
        blocks.append(&mut children);
        for block in &mut blocks {
            if let Terminator::Return {
                cleanup_actions, ..
            } = &mut block.terminator
            {
                *cleanup_actions = cleanup.clone();
            }
        }
        blocks
    } else {
        validate_direct_parameter_types(
            &expression,
            &scalar_values
                .iter()
                .map(|value| value.scalar_type)
                .collect::<Vec<_>>(),
        )?;
        let value = emit_direct_expression(
            &expression,
            &scalar_values,
            &mut next_value,
            &mut operations,
        );
        vec![Block {
            id: block_id(
                identity_base
                    .checked_add(1)
                    .expect("validated identity base"),
            ),
            parameters: Vec::new(),
            operations: operations.operations,
            terminator: Terminator::Return {
                edge: edge_id(
                    identity_base
                        .checked_add(1)
                        .expect("validated identity base"),
                ),
                value,
                cleanup_actions: cleanup,
            },
        }]
    };
    let result = ValueDeclaration {
        id: value_id(next_value),
        scalar_type: result_type,
    };
    let machine = TerminalMachine {
        id: terminal_machine,
        attachment: Some(lookup_type_id(&type_ids, &plan.attachment_type_identity)?),
        parameters: scalar_parameters,
        structural_parameters: structural_parameters.clone(),
        result: TerminalMachineResult::Scalar(result),
        structural_places: structural_parameters
            .iter()
            .map(|parameter| StructuralPlaceDeclaration {
                id: parameter.place,
                kind: StructuralPlaceKind::Parameter {
                    position: parameter.position,
                    is_self: parameter.is_self,
                },
            })
            .collect(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(
            identity_base
                .checked_add(1)
                .expect("validated identity base"),
        ),
        blocks,
        contract: MachineContract {
            id: contract_id(
                identity_base
                    .checked_add(1)
                    .expect("validated identity base"),
            ),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
        },
    };
    let mut lowered = LoweredTerminalPsi {
        semantic_module: TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine.id,
            structural_types,
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            proof_output_calls: Vec::new(),
            closed_conformance_applications: Vec::new(),
            machines: vec![machine],
        },
        proof_bundle: ProofBundle::default(),
        debug_map: None,
    };
    finalize_operation_proofs(&mut lowered)?;
    Ok(lowered)
}

#[cfg(test)]
mod trait_operator_tests {
    use super::*;
    use psi_source_files_to_tokens::Lexer;
    use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use psi_tokens_to_syntax_trees::parse_syntax_trees;
    use psi_typed_trees_to_checked_trees::lower_typed_trees;

    #[test]
    fn exact_trait_operator_call_closes_into_terminal_structural_scalar_call() {
        let source = r#"
            trait Ranked {
                operator < before(left: Self, right: Self) -> bool;
            }

            data Card { active: bool; }

            Ascending: Card satisfies Ranked {
                machine before(left: Card, right: Card) -> bool {
                    true
                }
            }

            machine choose<Element, Order: Element satisfies Ranked>(
                left: Element,
                right: Element
            ) -> bool {
                left < right
            }

            machine caller(left: Card, right: Card) -> bool {
                choose<Card, Ascending>(left, right)
            }
        "#;
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let checked = lower_typed_trees(typed).expect("check");
        let [plan] = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .trait_operator_machines
            .as_slice()
        else {
            panic!("one exact trait operator call plan")
        };
        let name = checked
            .facts
            .flow
            .terminal_machines
            .machines
            .iter()
            .find(|selection| selection.machine == plan.machine)
            .expect("selected specialized caller")
            .name
            .clone();
        let lowered = lower_machine(&checked, &name).expect("lower exact operator closure");
        assert_eq!(lowered.semantic_module.machines.len(), 2);
        assert_eq!(
            lowered
                .semantic_module
                .closed_conformance_applications
                .len(),
            1
        );
        assert_ne!(
            lowered.semantic_module.closed_conformance_applications[0].fingerprint,
            0
        );
        let entry = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == lowered.semantic_module.entry)
            .expect("entry machine");
        let [operation] = entry.blocks[0].operations.as_slice() else {
            panic!("one direct structural scalar call")
        };
        let OperationKind::CallStructuralScalar {
            callee,
            structural_arguments,
            claim_transfers,
            ..
        } = &operation.kind
        else {
            panic!("closed structural scalar operation")
        };
        assert_eq!(*callee, machine_id(2));
        assert_eq!(
            structural_arguments
                .iter()
                .map(|argument| argument.place)
                .collect::<Vec<_>>(),
            entry
                .structural_parameters
                .iter()
                .map(|parameter| parameter.place)
                .collect::<Vec<_>>()
        );
        assert!(claim_transfers.is_empty());
        psi_terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &psi_proof_kernel::AdmissionProfile::default(),
        )
        .expect("verify exact operator closure");
        let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
        let proof_bytes = psi_terminal_codec::encode_proof_bundle(&lowered.proof_bundle)
            .expect("encode proof bundle");
        let structural_values = entry
            .structural_parameters
            .iter()
            .enumerate()
            .map(
                |(index, parameter)| psi_terminal_interpreter::TerminalStructuralValue {
                    opaque_identity: u64::try_from(index).expect("small index") + 1,
                    structural_type: parameter.structural_type,
                    qualifications: parameter.qualifications.clone(),
                    path: Vec::new(),
                },
            )
            .collect::<Vec<_>>();
        let mut execution =
            psi_terminal_interpreter::TerminalExecution::start_artifact_with_structural_arguments(
                &bytes,
                &proof_bytes,
                &psi_proof_kernel::AdmissionProfile::default(),
                &[],
                &structural_values,
            )
            .expect("start structural scalar call");
        let mut meter = psi_terminal_fuel::TerminalFuelMeter::unbounded();
        assert_eq!(
            execution
                .resume(&mut meter)
                .expect("execute exact operator call"),
            psi_terminal_interpreter::TerminalExecutionStatus::Complete(
                psi_terminal_interpreter::TerminalExecutionResult::Scalar(
                    psi_terminal_interpreter::TerminalScalarValue::Boolean(true)
                )
            )
        );
        assert_eq!(
            psi_terminal_codec::decode_module(&bytes),
            Ok(lowered.semantic_module)
        );
    }
}
