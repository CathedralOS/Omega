//! Structural scalar-return lowering and nominal cleanup specialization.
//!
//! This module owns the structural-custody return cohort. General structural
//! returns, structural Unit control, and scalar-graph construction remain
//! separate producer responsibilities in the crate root.
use super::{
    BTreeSet, Block, CheckedNormalCleanupAction, CheckedScalarBindingValue,
    CheckedScalarExpressionRole, CheckedStructuralScalarReturnMachinePlan, CheckedTrees,
    CheckedUnitStructuralTypePlan, LoweredPsi, LoweringError, MachineContract, MachineId,
    Operation, PrimitiveType, ScalarType, StructuralPlaceDeclaration, StructuralPlaceKind,
    StructuralTypeDeclaration, TerminalAffineCleanupAction, TerminalMachine, TerminalMachineResult,
    TerminalModule, Terminator, ValueDeclaration, allocate_dense, block_id,
    boolean_decision_block_count, contains_short_circuit, contract_id, edge_id,
    emit_direct_expression, emit_inlined_boolean_value_blocks, lookup_type_id,
    lower_boolean_value_decision, lower_checked_scalar_expression_at,
    lower_checked_scalar_expression_at_with_parameters, lower_structural_type_plans,
    lower_unit_parameters, machine_id, terminal_scalar_type, unsupported,
    validate_boolean_parameter_types, validate_direct_parameter_types, value_id,
};
use crate::emission::boolean_control::LoweredBooleanDecisionExit;
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::returns::structural_scalar_return::expressions::{
    is_branch_free_structural_scalar_expression, is_structural_scalar_return_expression,
};

mod effects;
mod expressions;
mod nominal;
pub(crate) mod shared_convergence;
mod shared_types;
use nominal::lower_nominal_structural_scalar_return_machine;

pub(crate) fn lower_structural_scalar_return_machine(
    checked: &CheckedTrees,
    plan: &CheckedStructuralScalarReturnMachinePlan,
) -> Result<LoweredPsi, LoweringError> {
    if plan
        .cleanup_actions
        .iter()
        .any(|action| matches!(action, CheckedNormalCleanupAction::InvokeNominal(_)))
    {
        return lower_nominal_structural_scalar_return_machine(checked, plan);
    }
    lower_structural_scalar_return_machine_in_namespace(checked, plan, machine_id(1), 0, None)
}

/// The structural-scalar-return shapes one callee lowering may resolve: the
/// lane's published roster, restricted to the identities of a shared Unit
/// catalog when the callee is rejoined in a caller's namespace. A callee
/// lowered into a shared catalog can only name the shapes that catalog
/// declares, so the view narrows the roster instead of the caller trimming a
/// cloned copy of the checked trees.
#[derive(Clone, Copy)]
pub(crate) struct StructuralScalarReturnTypes<'a> {
    published: &'a [CheckedUnitStructuralTypePlan],
    shared: Option<&'a [StructuralTypeDeclaration]>,
}

impl<'a> StructuralScalarReturnTypes<'a> {
    pub(crate) fn published(checked: &'a CheckedTrees) -> Self {
        Self {
            published: &checked
                .facts
                .flow
                .terminal_structural_scalar_returns
                .structural_types,
            shared: None,
        }
    }

    fn shared_with(
        checked: &'a CheckedTrees,
        shared: Option<&'a [StructuralTypeDeclaration]>,
    ) -> Self {
        Self {
            shared,
            ..Self::published(checked)
        }
    }

    pub(crate) fn iter(self) -> impl Iterator<Item = &'a CheckedUnitStructuralTypePlan> {
        self.published.iter().filter(move |plan| {
            self.shared.is_none_or(|shared| {
                shared
                    .iter()
                    .any(|declaration| declaration.identity == plan.identity)
            })
        })
    }
}

pub(crate) fn lower_structural_scalar_return_machine_in_namespace(
    checked: &CheckedTrees,
    plan: &CheckedStructuralScalarReturnMachinePlan,
    terminal_machine: MachineId,
    identity_base: u64,
    shared_structural_types: Option<&[StructuralTypeDeclaration]>,
) -> Result<LoweredPsi, LoweringError> {
    let types = StructuralScalarReturnTypes::shared_with(checked, shared_structural_types);
    // Stage 04 owns signature, permission and complete statement coverage.
    // Rebuilding a primitive/slice signature here rejected checked fixed-array
    // borrows. Emit the retained parameter shapes, effects and result instead;
    // unsupported operations still refuse at their emission boundary.
    if plan
        .cleanup_actions
        .iter()
        .any(|action| matches!(action, CheckedNormalCleanupAction::InvokeNominal(_)))
    {
        return unsupported("namespaced structural scalar callees require direct root cleanup");
    }
    let (structural_types, type_ids) = if let Some(shared) = shared_structural_types {
        let structural_types = shared_types::reconstruct(types, shared, plan)?;
        let type_ids = structural_types
            .iter()
            .map(|declaration| (declaration.identity.clone(), declaration.id))
            .collect::<Vec<_>>();
        (structural_types, type_ids)
    } else {
        lower_structural_type_plans(types.published)?
    };
    let result_type = terminal_scalar_type(plan.result_type)?;

    let mut next_place = identity_base
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "structural place namespace overflows",
        ))?;
    let structural_parameters = lower_unit_parameters(
        &plan.structural_parameters,
        &type_ids,
        &structural_types,
        &[],
        &mut next_place,
    )?;
    // Expressions observe byte views through the authored source position;
    // pair each with its emitted place so `source.len` resolves exactly.
    let structural_namespace = plan
        .structural_parameters
        .iter()
        .zip(&structural_parameters)
        .map(|(source, emitted)| (source.position, emitted.clone()))
        .collect::<Vec<_>>();
    let cleanup = plan
        .cleanup_actions
        .iter()
        .map(|action| {
            let CheckedNormalCleanupAction::Discard(discard) = action else {
                return Err(LoweringError::Unsupported(
                    "structural scalar return trivial lane acquired a nominal cleanup",
                ));
            };
            if !discard.path.is_empty() {
                return Err(LoweringError::Unsupported(
                    "whole scalar return cleanup acquired a residual path",
                ));
            }
            let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index,
            } = discard.source
            else {
                return Err(LoweringError::Unsupported(
                    "scalar return parameter cleanup acquired another source",
                ));
            };
            let parameter_index = parameter_index as usize;
            structural_parameters
                .get(parameter_index)
                .map(|parameter| parameter.place)
                .ok_or(LoweringError::Unsupported(
                    "structural scalar return cleanup position has no terminal place",
                ))
        })
        .map(|place| place.map(TerminalAffineCleanupAction::DiscardRoot))
        .collect::<Result<Vec<_>, _>>()?;
    let mut operations = OperationBuffer::new(
        identity_base,
        std::sync::Arc::new(crate::unit::dynamic_dispatch::DynamicDispatchIndex::new(
            checked,
        )),
    );
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
                qualifications: Default::default(),
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
            || binding.destination != checked_trees::CheckedScalarBindingDestination::Immutable
            || binding.value != CheckedScalarBindingValue::Expression
        {
            return unsupported(
                "structural scalar return bindings are not a direct expression prefix",
            );
        }
        let expression = lower_checked_scalar_expression_at_with_parameters(
            checked,
            plan.state,
            statement_ordinal,
            CheckedScalarExpressionRole::LocalInitializer {
                binding_ordinal: statement_ordinal,
            },
            &structural_namespace,
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
            || binding.destination != checked_trees::CheckedScalarBindingDestination::Immutable
            || binding.value != CheckedScalarBindingValue::Expression
        {
            return unsupported(
                "structural scalar return bindings are not a direct expression prefix",
            );
        }
        let scalar_type = terminal_scalar_type(binding.primitive_type)?;
        let expression = lower_checked_scalar_expression_at_with_parameters(
            checked,
            plan.state,
            statement_ordinal,
            CheckedScalarExpressionRole::LocalInitializer {
                binding_ordinal: statement_ordinal,
            },
            &structural_namespace,
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
        scalar_values.push(ValueDeclaration {
            qualifications: Default::default(),
            id,
            scalar_type,
        });
    }
    effects::emit(
        plan,
        &structural_parameters,
        &structural_types,
        &scalar_values,
        &mut next_value,
        &mut operations,
    )?;
    let expression = lower_checked_scalar_expression_at_with_parameters(
        checked,
        plan.state,
        plan.return_statement_ordinal,
        CheckedScalarExpressionRole::Return,
        &structural_namespace,
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
                qualifications: Default::default(),
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
                let continuation_expression = lower_checked_scalar_expression_at_with_parameters(
                    checked,
                    plan.state,
                    statement_ordinal,
                    CheckedScalarExpressionRole::LocalInitializer {
                        binding_ordinal: statement_ordinal,
                    },
                    &structural_namespace,
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
                scalar_values.push(ValueDeclaration {
                    qualifications: Default::default(),
                    id,
                    scalar_type,
                });
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
                structural_parameters: Vec::new(),
                id: next_block,
                parameters: next_block_parameters,
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
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
            structural_parameters: Vec::new(),
            id: block_id(
                identity_base
                    .checked_add(1)
                    .expect("validated identity base"),
            ),
            parameters: Vec::new(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
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
        qualifications: Default::default(),
        id: value_id(next_value),
        scalar_type: result_type,
    };
    let machine = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: terminal_machine,
        attachment: plan
            .attachment_type_identity
            .as_deref()
            .map(|identity| lookup_type_id(&type_ids, identity))
            .transpose()?,
        nominal_cleanup_subject: None,
        parameters: scalar_parameters,
        structural_parameters: structural_parameters.clone(),
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(result),
        structural_places: structural_parameters
            .iter()
            .map(|parameter| StructuralPlaceDeclaration {
                id: parameter.place,
                kind: StructuralPlaceKind::Parameter {
                    position: parameter.position,
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
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    let entry = machine.id;
    let lowered = LoweredPsi {
        semantic_module: TerminalModule {
            structural_types,
            machines: vec![machine],
            ..TerminalModule::for_entry(entry)
        },
        debug_map: None,
        source_call_occurrences: Vec::new(),
        selected_ieee_float_comparison_occurrences: Vec::new(),
        selected_integer_comparison_occurrences: Vec::new(),
    };
    Ok(lowered)
}
