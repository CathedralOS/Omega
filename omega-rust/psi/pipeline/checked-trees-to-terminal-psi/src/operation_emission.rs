//! Terminal operation emission and proof finalization.

use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::nonzero_divisor_certificate::produce_checked_canonical_integer_proof;

const PARALLEL_PROOF_THRESHOLD: usize = 16;
const MAX_PROOF_WORKERS: usize = 8;

pub(super) fn finalize_operation_proofs(
    lowered: &mut LoweredTerminalPsi,
) -> Result<(), LoweringError> {
    let has_ranked_countdown = lowered
        .semantic_module
        .machines
        .iter()
        .any(|machine| machine.ranked_scc.is_some());
    let execution_validated = (!has_ranked_countdown)
        .then(|| terminal_verifier::validate_module(&lowered.semantic_module))
        .transpose()
        .map_err(LoweringError::InvalidTerminalModule)?;
    let interpretation_validated = has_ranked_countdown
        .then(|| terminal_verifier::validate_module_for_interpretation(&lowered.semantic_module))
        .transpose()
        .map_err(LoweringError::InvalidTerminalModule)?;
    let obligations = if let Some(validated) = interpretation_validated {
        terminal_verifier::reconstruct_interpretable_operation_obligations(validated)
    } else {
        reconstruct_operation_obligations(&lowered.semantic_module)
    }
    .map_err(LoweringError::InvalidTerminalModule)?;
    let existing = lowered
        .proof_bundle
        .evidence
        .iter()
        .map(|evidence| evidence.obligation)
        .collect::<BTreeSet<_>>();
    // Some closure builders have already supplied source-derived evidence for
    // contextual call/cleanup obligations. Reconstruct every site, but
    // synthesize only obligations that remain undispatched; the final verifier
    // still checks the retained evidence against the exact goal.
    let pending = obligations
        .into_iter()
        .filter(|site| !existing.contains(&site.obligation.id))
        .collect::<Vec<_>>();
    let owners = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| {
            machine.blocks.iter().flat_map(move |block| {
                block.operations.iter().filter_map(move |operation| {
                    proof_bearing_operation_obligation(&operation.kind)
                        .map(|obligation| (obligation, machine))
                })
            })
        })
        .collect::<BTreeMap<_, _>>();
    let produce = |site: &terminal_verifier::ReconstructedOperationObligation| {
        let owner = owners.get(&site.obligation.id).copied();
        let assumptions = owner
            .map(|machine| machine.contract.requires.as_slice())
            .unwrap_or_default();
        let proof = if let Some(machine) = owner
            && site.canonical_certificate
        {
            let context = if let Some(validated) = interpretation_validated {
                validated.value_context(machine)
            } else {
                execution_validated
                    .expect("one validation carrier is present")
                    .value_context(machine)
            }
            .map_err(LoweringError::InvalidTerminalModule)?;
            let machine_parameter_values = machine
                .parameters
                .iter()
                .map(|parameter| parameter.id)
                .collect();
            produce_checked_canonical_integer_proof(
                &context,
                &site.obligation.proposition,
                assumptions,
                &site.semantic_axioms,
                &machine_parameter_values,
            )
        } else {
            proof_from_available_facts(
                &site.obligation.proposition,
                assumptions,
                &site.semantic_axioms,
            )
        };
        let proof = proof.ok_or(LoweringError::OperationProofUnavailable(site.obligation.id))?;
        Ok::<_, LoweringError>(ObligationEvidence {
            obligation: site.obligation.id,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(site.obligation.id.get())
                    .expect("terminal obligations have nonzero identities"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof,
            }),
        })
    };
    let generated = if pending.len() < PARALLEL_PROOF_THRESHOLD {
        pending.iter().map(produce).collect::<Result<Vec<_>, _>>()?
    } else {
        // Certificate searches are independent and read only validated module
        // state. Bound the pool so one large compile can use the host without
        // turning ordinary concurrent test runs into nested fan-out.
        let worker_count = std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1)
            .min(MAX_PROOF_WORKERS)
            .min(pending.len());
        let next = AtomicUsize::new(0);
        std::thread::scope(|scope| {
            let produce = &produce;
            let pending = pending.as_slice();
            let mut workers = Vec::with_capacity(worker_count);
            for _ in 0..worker_count {
                let next = &next;
                workers.push(scope.spawn(move || {
                    let mut generated = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(site) = pending.get(index) else {
                            break;
                        };
                        generated.push((index, produce(site)));
                    }
                    generated
                }));
            }
            let mut generated = Vec::with_capacity(pending.len());
            for worker in workers {
                generated.extend(
                    worker
                        .join()
                        .expect("operation-proof synthesis worker does not panic"),
                );
            }
            generated.sort_by_key(|(index, _)| *index);
            generated
                .into_iter()
                .map(|(_, evidence)| evidence)
                .collect::<Result<Vec<_>, _>>()
        })?
    };
    lowered.proof_bundle.evidence.extend(generated);
    lowered
        .proof_bundle
        .evidence
        .sort_by_key(|evidence| evidence.obligation);
    Ok(())
}

fn proof_bearing_operation_obligation(kind: &OperationKind) -> Option<ObligationId> {
    let obligation = match kind {
        OperationKind::IntegerExactCast { obligation, .. }
        | OperationKind::ExactIntegerAdd { obligation, .. }
        | OperationKind::ExactIntegerSubtract { obligation, .. }
        | OperationKind::ExactIntegerMultiply { obligation, .. }
        | OperationKind::ExactIntegerShiftRight { obligation, .. }
        | OperationKind::ExactIntegerShiftLeft { obligation, .. }
        | OperationKind::ExactIntegerDivide { obligation, .. }
        | OperationKind::ExactIntegerRemainder { obligation, .. }
        | OperationKind::WrappingIntegerDivide { obligation, .. }
        | OperationKind::WrappingIntegerRemainder { obligation, .. }
        | OperationKind::SaturatingIntegerDivide { obligation, .. }
        | OperationKind::SaturatingIntegerRemainder { obligation, .. } => *obligation,
        _ => return None,
    };
    Some(obligation)
}

fn proof_from_available_facts(
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    if goal == &Proposition::Truth {
        return Some(ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
        });
    }
    if matches!(goal, Proposition::Equal(left, right) if left == right) {
        return Some(ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
        });
    }
    if let Some(index) = assumptions.iter().position(|assumption| assumption == goal) {
        return Some(ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::Assumption { index },
        });
    }
    if let Some(index) = semantic_axioms.iter().position(|axiom| axiom == goal) {
        return Some(ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::SemanticAxiom { index },
        });
    }
    let Proposition::Conjunction(conjuncts) = goal else {
        return None;
    };
    let proofs = conjuncts
        .iter()
        .map(|conjunct| proof_from_available_facts(conjunct, assumptions, semantic_axioms))
        .collect::<Option<Vec<_>>>()?;
    Some(ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::ConjunctionIntroduction(proofs),
    })
}

pub(super) fn emit_boolean_expression(
    expression: &LoweredBooleanReturnExpression,
    parameters: &[ValueDeclaration],
    next_value_identity: &mut u64,
    operations: &mut OperationBuffer,
) -> ValueId {
    match expression {
        LoweredBooleanReturnExpression::Constant { value } => {
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after a Boolean literal");
            let operation = operations.allocate();
            operations.push(Operation {
                id: operation,
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    id,
                    scalar_type: ScalarType::Boolean,
                }),
                kind: OperationKind::BooleanConstant { value: *value },
            });
            id
        }
        LoweredBooleanReturnExpression::IntegerComparison { kind, left, right } => {
            let left = emit_direct_expression(left, parameters, next_value_identity, operations);
            let right = emit_direct_expression(right, parameters, next_value_identity, operations);
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after integer comparison");
            let operation = operations.allocate();
            operations.push(Operation {
                id: operation,
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    id,
                    scalar_type: ScalarType::Boolean,
                }),
                kind: kind.operation(left, right),
            });
            id
        }
        LoweredBooleanReturnExpression::Parameter { position }
        | LoweredBooleanReturnExpression::Local { position } => parameters[*position].id,
        LoweredBooleanReturnExpression::StructuralField { source, field } => {
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after a structural Boolean load");
            let operation = operations.allocate();
            operations.push(Operation {
                id: operation,
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    id,
                    scalar_type: ScalarType::Boolean,
                }),
                kind: OperationKind::BooleanStructuralField {
                    source: *source,
                    field: *field,
                },
            });
            id
        }
        LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. } => {
            unreachable!("shared Boolean members resolve before terminal operation emission")
        }
        LoweredBooleanReturnExpression::Not { operand } => {
            let operand =
                emit_boolean_expression(operand, parameters, next_value_identity, operations);
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after Boolean negation");
            let operation = operations.allocate();
            operations.push(Operation {
                id: operation,
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    id,
                    scalar_type: ScalarType::Boolean,
                }),
                kind: OperationKind::BooleanNot { operand },
            });
            id
        }
        LoweredBooleanReturnExpression::Equal { left, right } => {
            let left = emit_boolean_expression(left, parameters, next_value_identity, operations);
            let right = emit_boolean_expression(right, parameters, next_value_identity, operations);
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after Boolean equality");
            let operation = operations.allocate();
            operations.push(Operation {
                id: operation,
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    id,
                    scalar_type: ScalarType::Boolean,
                }),
                kind: OperationKind::BooleanEqual { left, right },
            });
            id
        }
        LoweredBooleanReturnExpression::And { .. } | LoweredBooleanReturnExpression::Or { .. } => {
            unreachable!("short-circuit Boolean expressions lower through terminal control")
        }
    }
}

pub(super) fn emit_scalar_binding(
    binding: &LoweredScalarBinding,
    parameters: &[ValueDeclaration],
    next_value_identity: &mut u64,
    operations: &mut OperationBuffer,
    call_emission: &mut CallEmissionContext<'_>,
) -> Result<ValueId, LoweringError> {
    let LoweredScalarBinding::DirectCall(call) = binding else {
        let LoweredScalarBinding::Expression(expression) = binding else {
            unreachable!()
        };
        return Ok(emit_direct_expression(
            expression,
            parameters,
            next_value_identity,
            operations,
        ));
    };
    let arguments = call
        .arguments
        .iter()
        .map(|argument| ValueDeclaration {
            id: emit_direct_expression(argument, parameters, next_value_identity, operations),
            scalar_type: argument.scalar_type(),
        })
        .collect::<Vec<_>>();
    emit_direct_call_operation(
        call,
        &call.crash_continuations,
        match call.crash_scope {
            ScalarCallCrashScope::CallerValues => parameters,
            ScalarCallCrashScope::Arguments => &arguments,
        },
        parameters,
        &arguments,
        next_value_identity,
        operations,
        call_emission,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_staged_scalar_call_binding(
    call: &LoweredDirectCallBinding,
    stage_parameters: &[ValueDeclaration],
    stage_parameter_types: &[ScalarType],
    stage_block_parameters: Vec<ValueDeclaration>,
    stage_block: BlockId,
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
                .map(LoweredDirectExpression::scalar_type),
        );
        let next_stage_parameters = next_stage_types
            .into_iter()
            .map(|scalar_type| {
                let parameter = ValueDeclaration {
                    id: value_id(*next_value_identity),
                    scalar_type,
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
                id: current_block,
                parameters: current_block_parameters,
                operations: operations[operation_start..].to_vec(),
                terminator: Terminator::Jump {
                    edge,
                    target: next_stage,
                    arguments,
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
        id: current_block,
        parameters: current_block_parameters,
        operations: operations[operation_start..].to_vec(),
        terminator: Terminator::Jump {
            edge,
            target: continuation,
            arguments: continuation_arguments,
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
    operations.push(Operation {
        id: operation,
        result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
            id: result,
            scalar_type: call.result_type,
        }),
        kind: OperationKind::Call {
            callee,
            arguments: arguments.iter().map(|argument| argument.id).collect(),
            requirement_obligations,
            crash_continuations,
        },
    });
    Ok(result)
}

pub(super) fn emit_direct_expression(
    expression: &LoweredDirectExpression,
    parameters: &[ValueDeclaration],
    next_value_identity: &mut u64,
    operations: &mut OperationBuffer,
) -> ValueId {
    match expression {
        LoweredDirectExpression::Parameter { position, .. }
        | LoweredDirectExpression::Local { position, .. } => parameters[*position].id,
        LoweredDirectExpression::IntegerLiteral { value, scalar_type } => {
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after a literal");
            let operation = operations.allocate();
            operations.push(Operation {
                id: operation,
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    id,
                    scalar_type: *scalar_type,
                }),
                kind: OperationKind::IntegerConstant { value: *value },
            });
            id
        }
        LoweredDirectExpression::IeeeFloatLiteral { value } => {
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after an IEEE float literal");
            let operation = operations.allocate();
            operations.push(Operation {
                id: operation,
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    id,
                    scalar_type: ScalarType::IeeeFloat(value.format()),
                }),
                kind: OperationKind::IeeeFloatConstant { value: *value },
            });
            id
        }
        LoweredDirectExpression::IntegerBinary {
            kind,
            scalar_type,
            left,
            right,
        } => {
            let left = emit_direct_expression(left, parameters, next_value_identity, operations);
            let right = emit_direct_expression(right, parameters, next_value_identity, operations);
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after a binary operation");
            let operation = operations.allocate();
            operations.push(Operation {
                id: operation,
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    id,
                    scalar_type: *scalar_type,
                }),
                kind: kind.operation(operation, left, right),
            });
            id
        }
        LoweredDirectExpression::IntegerBitwiseNot {
            scalar_type,
            operand,
        } => {
            let operand =
                emit_direct_expression(operand, parameters, next_value_identity, operations);
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after bitwise complement");
            let operation = operations.allocate();
            operations.push(Operation {
                id: operation,
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    id,
                    scalar_type: *scalar_type,
                }),
                kind: OperationKind::IntegerBitwiseNot { operand },
            });
            id
        }
        LoweredDirectExpression::IntegerWiden {
            scalar_type,
            operand,
        } => {
            let operand =
                emit_direct_expression(operand, parameters, next_value_identity, operations);
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after integer widening");
            let operation = operations.allocate();
            operations.push(Operation {
                id: operation,
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    id,
                    scalar_type: *scalar_type,
                }),
                kind: OperationKind::IntegerWiden { operand },
            });
            id
        }
        LoweredDirectExpression::IntegerExactCast {
            scalar_type,
            operand,
        } => {
            let operand =
                emit_direct_expression(operand, parameters, next_value_identity, operations);
            let id = value_id(*next_value_identity);
            *next_value_identity = next_value_identity
                .checked_add(1)
                .expect("generated value identity advances after an exact integer cast");
            let operation = operations.allocate();
            operations.push(Operation {
                id: operation,
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    id,
                    scalar_type: *scalar_type,
                }),
                kind: OperationKind::IntegerExactCast {
                    operand,
                    obligation: obligation_id(
                        operation
                            .get()
                            .checked_add(1)
                            .expect("exact-cast obligation follows its operation identity"),
                    ),
                },
            });
            id
        }
        LoweredDirectExpression::Boolean { expression } => {
            emit_boolean_expression(expression, parameters, next_value_identity, operations)
        }
    }
}
