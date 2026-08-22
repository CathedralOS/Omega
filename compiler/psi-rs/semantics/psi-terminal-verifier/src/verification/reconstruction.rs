//! Exact executable-site obligations and all-path fact reconstruction.

use std::collections::{BTreeMap, BTreeSet};

use psi_core::{Proposition, PropositionContext, ScalarTerm, ValueId};
use psi_proof_kernel::{
    IntegerCastChainWitness, Obligation, ObligationClass, check_integer_cast_bound_conversion,
    check_integer_cast_chain_witness,
};
use psi_terminal::{OperationKind, TerminalMachine, TerminalModule, Terminator};
use psi_terminal_semantics::{
    CanonicalScalarGoal, OperationSemanticError, OperationSemanticTag,
    goal_free_scalar_leaf_equation, proof_bearing_scalar_leaf_semantics,
    structural_effect_leaf_observation,
};

use crate::{ModuleError, validate_module};

use super::call_composition::compose_call_operation;
use super::substitution::{
    substitute_proposition_places, substitute_proposition_values, substitute_scalar_term_values,
};
use super::sufficient_reduction::reduce_proof_bearing_scalar_goal;

mod affine_custody;
mod affine_selection;
mod alias_transport;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconstructedOperationObligation {
    pub obligation: Obligation,
    pub semantic_axioms: Vec<Proposition>,
    /// The obligation is the operation schema's canonical kernel proposition,
    /// not a proposition selected by a trusted sufficient-form reducer.
    pub canonical_certificate: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReconstructedMachineSemantics {
    pub(super) operation_obligations: Vec<ReconstructedOperationObligation>,
    pub(super) exit_axioms: Vec<Proposition>,
}

/// Reconstruct proof obligations owned by executable operation sites. This is
/// exposed so a producer can build a certificate against exactly the same
/// source-independent obligations and axiom ordering that the verifier will
/// later replay. The module is validated before any reconstruction occurs.
pub fn reconstruct_operation_obligations(
    module: &TerminalModule,
) -> Result<Vec<ReconstructedOperationObligation>, ModuleError> {
    validate_module(module)?;
    let mut obligations = Vec::new();
    for machine in &module.machines {
        obligations.extend(reconstruct_machine_semantics(module, machine)?.operation_obligations);
    }
    Ok(obligations)
}

/// Reconstruct facts at each executable obligation site and facts established
/// on every return path. A true conditional edge establishes the predicate
/// computed by its condition operation; edge bindings rewrite those facts to
/// successor parameters. Merge and return facts remain intersection-only.
pub(super) fn reconstruct_machine_semantics(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Result<ReconstructedMachineSemantics, ModuleError> {
    let reconstruct_path_facts = machine.blocks.iter().any(|block| {
        block.operations.iter().any(|operation| {
            matches!(
                &operation.kind,
                OperationKind::Call { .. }
                    | OperationKind::CallUnit { .. }
                    | OperationKind::IntegerExactCast { .. }
                    | OperationKind::ExactIntegerShiftLeft { .. }
                    | OperationKind::ExactIntegerShiftRight { .. }
                    | OperationKind::ExactIntegerAdd { .. }
                    | OperationKind::ExactIntegerSubtract { .. }
                    | OperationKind::ExactIntegerMultiply { .. }
                    | OperationKind::ExactIntegerDivide { .. }
                    | OperationKind::ExactIntegerRemainder { .. }
                    | OperationKind::WrappingIntegerDivide { .. }
                    | OperationKind::WrappingIntegerRemainder { .. }
                    | OperationKind::SaturatingIntegerDivide { .. }
                    | OperationKind::SaturatingIntegerRemainder { .. }
            )
        })
    });
    let value_types = machine
        .parameters
        .iter()
        .chain(machine.result.scalar_ref())
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| block.parameters.iter()),
        )
        .chain(machine.blocks.iter().flat_map(|block| {
            block
                .operations
                .iter()
                .filter_map(|operation| operation.result.scalar_ref())
        }))
        .map(|declaration| (declaration.id, declaration.scalar_type))
        .collect::<BTreeMap<_, _>>();
    let proposition_context = PropositionContext::from_value_types(
        value_types
            .iter()
            .map(|(&id, &scalar_type)| (id, scalar_type)),
    )
    .map_err(ModuleError::MalformedProposition)?;
    let machine_parameter_values = machine
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .collect::<BTreeSet<_>>();
    let blocks = machine
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    let machines = module
        .machines
        .iter()
        .map(|machine| (machine.id, machine))
        .collect::<BTreeMap<_, _>>();
    let value_term = |id: ValueId| {
        ScalarTerm::value(
            id,
            *value_types
                .get(&id)
                .expect("validated module contains every referenced value"),
        )
    };

    // Result-content equalities become true only when an exact structural
    // return edge transfers the corresponding live claims.
    let base_axioms = Vec::new();
    let mut successors = BTreeMap::<_, Vec<_>>::new();
    let mut indegree = machine
        .blocks
        .iter()
        .map(|block| (block.id, 0usize))
        .collect::<BTreeMap<_, _>>();
    for block in &machine.blocks {
        let targets = match &block.terminator {
            Terminator::Jump { target, .. } => vec![*target],
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => vec![when_true.target, when_false.target],
            Terminator::Return { .. }
            | Terminator::ReturnUnit { .. }
            | Terminator::ReturnUnitPartialAffine { .. }
            | Terminator::ReturnUnitNominalAffine { .. }
            | Terminator::ReturnStructural { .. }
            | Terminator::Crash { .. } => Vec::new(),
        };
        for target in &targets {
            *indegree
                .get_mut(target)
                .expect("validated target has an indegree") += 1;
        }
        successors.insert(block.id, targets);
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(block, count)| (*count == 0).then_some(*block))
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(machine.blocks.len());
    while let Some(block) = ready.pop_first() {
        order.push(block);
        for target in &successors[&block] {
            let count = indegree
                .get_mut(target)
                .expect("validated target has an indegree");
            *count -= 1;
            if *count == 0 {
                ready.insert(*target);
            }
        }
    }

    let mut incoming = BTreeMap::<_, Vec<Vec<Proposition>>>::new();
    incoming.insert(machine.entry, vec![base_axioms]);
    let mut exits = Vec::<Vec<Proposition>>::new();
    let mut operation_obligations = Vec::new();
    for current in order {
        let block = blocks
            .get(&current)
            .expect("validated module contains every reached block");
        let paths = incoming
            .remove(&current)
            .expect("validated reachable block has incoming facts");
        let mut paths = paths.into_iter();
        let mut axioms = paths.next().expect("block has an incoming path");
        for path in paths {
            axioms.retain(|fact| path.contains(fact));
        }
        for operation in &block.operations {
            if let Some(equation) = goal_free_scalar_leaf_equation(operation, &value_types)
                .map_err(ModuleError::OperationSemanticSchema)?
            {
                axioms.push(equation);
                continue;
            }
            if let Some(semantics) = proof_bearing_scalar_leaf_semantics(operation, &value_types)
                .map_err(ModuleError::OperationSemanticSchema)?
            {
                let canonical_certificate = matches!(
                    semantics.tag(),
                    OperationSemanticTag::WrappingIntegerDivide
                        | OperationSemanticTag::WrappingIntegerRemainder
                        | OperationSemanticTag::SaturatingIntegerDivide
                        | OperationSemanticTag::SaturatingIntegerRemainder
                ) || exact_division_has_prior_certificate(
                    &proposition_context,
                    semantics.canonical_goal(),
                    &axioms,
                    &machine.contract.requires,
                );
                let proposition = if canonical_certificate {
                    semantics
                        .canonical_goal()
                        .kernel_proposition()
                        .map_err(ModuleError::OperationSemanticSchema)?
                        .ok_or(ModuleError::OperationSemanticSchema(
                            OperationSemanticError::ProofBearingScalarSchemaMismatch(
                                semantics.tag(),
                            ),
                        ))?
                } else {
                    reduce_proof_bearing_scalar_goal(
                        &semantics,
                        &axioms,
                        &machine.contract.requires,
                        &machine_parameter_values,
                    )
                };
                operation_obligations.push(ReconstructedOperationObligation {
                    obligation: Obligation {
                        id: semantics.obligation(),
                        proposition,
                        class: ObligationClass::Derivable,
                    },
                    semantic_axioms: axioms.clone(),
                    canonical_certificate,
                });
                axioms.push(semantics.result_equation().clone());
                continue;
            }
            if let Some(observation) = structural_effect_leaf_observation(operation)
                .map_err(ModuleError::OperationSemanticSchema)?
            {
                if let Some(equation) = observation.local_equation() {
                    axioms.push(equation.clone());
                }
                continue;
            }
            if compose_call_operation(
                module,
                machine,
                operation,
                &machines,
                &value_types,
                &mut axioms,
                &mut operation_obligations,
            )? {
                continue;
            }
            match operation.kind.clone() {
                OperationKind::IntegerExactCast { .. }
                | OperationKind::ExactIntegerShiftLeft { .. }
                | OperationKind::ExactIntegerShiftRight { .. }
                | OperationKind::ExactIntegerAdd { .. }
                | OperationKind::ExactIntegerSubtract { .. }
                | OperationKind::ExactIntegerMultiply { .. }
                | OperationKind::ExactIntegerDivide { .. }
                | OperationKind::ExactIntegerRemainder { .. }
                | OperationKind::WrappingIntegerDivide { .. }
                | OperationKind::WrappingIntegerRemainder { .. }
                | OperationKind::SaturatingIntegerDivide { .. }
                | OperationKind::SaturatingIntegerRemainder { .. } => {
                    unreachable!(
                        "proof-bearing scalar rows return before legacy reduction dispatch"
                    )
                }
                OperationKind::IntegerConstant { .. }
                | OperationKind::BooleanConstant { .. }
                | OperationKind::BooleanNot { .. }
                | OperationKind::BooleanEqual { .. }
                | OperationKind::IntegerEqual { .. }
                | OperationKind::IntegerLessThan { .. }
                | OperationKind::IntegerLessOrEqual { .. }
                | OperationKind::IntegerBitwiseNot { .. }
                | OperationKind::IntegerWiden { .. }
                | OperationKind::IntegerBitwiseAnd { .. }
                | OperationKind::IntegerBitwiseOr { .. }
                | OperationKind::IntegerBitwiseXor { .. }
                | OperationKind::WrappingIntegerShiftLeft { .. }
                | OperationKind::WrappingIntegerShiftRight { .. }
                | OperationKind::WrappingIntegerAdd { .. }
                | OperationKind::SaturatingIntegerAdd { .. }
                | OperationKind::WrappingIntegerSubtract { .. }
                | OperationKind::SaturatingIntegerSubtract { .. }
                | OperationKind::WrappingIntegerMultiply { .. }
                | OperationKind::SaturatingIntegerMultiply { .. } => {
                    unreachable!("goal-free scalar rows return before specialized reconstruction")
                }
                OperationKind::EstablishTrivialAffineLocal { .. }
                | OperationKind::PortWrite { .. }
                | OperationKind::BooleanStructuralField { .. } => {
                    unreachable!("structural/effect rows return before specialized reconstruction")
                }
                OperationKind::Call { .. }
                | OperationKind::CallUnit { .. }
                | OperationKind::BoundaryCall { .. } => {
                    unreachable!("call rows return before specialized reconstruction")
                }
            }
        }
        match &block.terminator {
            Terminator::Jump {
                target, arguments, ..
            } => {
                let target_block = blocks.get(target).expect("validator requires jump target");
                bind_successor_axioms(
                    &mut axioms,
                    target_block,
                    arguments,
                    &value_term,
                    reconstruct_path_facts,
                );
                incoming.entry(*target).or_default().push(axioms);
            }
            Terminator::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                let true_fact = true_condition_fact(*condition, &axioms, &value_term);
                for (successor, condition_fact) in
                    [(when_true, true_fact.as_ref()), (when_false, None)]
                {
                    let target_block = blocks
                        .get(&successor.target)
                        .expect("validator requires conditional target");
                    let mut arm_axioms = axioms.clone();
                    bind_successor_axioms(
                        &mut arm_axioms,
                        target_block,
                        &successor.arguments,
                        &value_term,
                        reconstruct_path_facts,
                    );
                    if reconstruct_path_facts && let Some(condition_fact) = condition_fact {
                        append_successor_fact(
                            &mut arm_axioms,
                            condition_fact,
                            target_block,
                            &successor.arguments,
                            &value_term,
                        );
                    }
                    incoming
                        .entry(successor.target)
                        .or_default()
                        .push(arm_axioms);
                }
            }
            Terminator::Return {
                value,
                cleanup_actions,
                ..
            } => {
                let result = machine
                    .result
                    .scalar()
                    .expect("validated scalar return has a scalar machine result");
                axioms.push(Proposition::Equal(
                    value_term(result.id),
                    value_term(*value),
                ));
                for cleanup in cleanup_actions.iter().filter_map(|action| match action {
                    psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                        Some(cleanup)
                    }
                    psi_terminal::TerminalAffineCleanupAction::DiscardRoot(_)
                    | psi_terminal::TerminalAffineCleanupAction::DiscardResidual(_) => None,
                }) {
                    let target = machines
                        .get(&cleanup.cleanup_machine)
                        .copied()
                        .expect("validated nominal cleanup target exists");
                    let receiver = cleanup
                        .cleanup_receiver
                        .map(|receiver| BTreeMap::from([(receiver, cleanup.place)]))
                        .unwrap_or_default();
                    for (required, obligation) in target
                        .contract
                        .requires
                        .iter()
                        .zip(&cleanup.requirement_obligations)
                    {
                        operation_obligations.push(ReconstructedOperationObligation {
                            obligation: Obligation {
                                id: *obligation,
                                proposition: substitute_proposition_places(required, &receiver),
                                class: ObligationClass::Derivable,
                            },
                            semantic_axioms: axioms.clone(),
                            canonical_certificate: false,
                        });
                    }
                }
                exits.push(axioms);
            }
            Terminator::ReturnUnitNominalAffine { cleanups, .. } => {
                for cleanup in cleanups {
                    let target = machines
                        .get(&cleanup.cleanup_machine)
                        .copied()
                        .expect("validated nominal cleanup target exists");
                    let receiver = cleanup
                        .cleanup_receiver
                        .map(|receiver| BTreeMap::from([(receiver, cleanup.place)]))
                        .unwrap_or_default();
                    for (required, obligation) in target
                        .contract
                        .requires
                        .iter()
                        .zip(&cleanup.requirement_obligations)
                    {
                        operation_obligations.push(ReconstructedOperationObligation {
                            obligation: Obligation {
                                id: *obligation,
                                proposition: substitute_proposition_places(required, &receiver),
                                class: ObligationClass::Derivable,
                            },
                            semantic_axioms: axioms.clone(),
                            canonical_certificate: false,
                        });
                    }
                }
                exits.push(axioms);
            }
            Terminator::ReturnUnit { .. } | Terminator::ReturnUnitPartialAffine { .. } => {
                exits.push(axioms)
            }
            Terminator::ReturnStructural {
                returned_claims, ..
            } => {
                axioms.extend(
                    machine
                        .content_identity_reshuffles
                        .iter()
                        .filter(|reshuffle| returned_claims.contains(&reshuffle.claim))
                        .flat_map(|reshuffle| reshuffle.inferred_propositions()),
                );
                exits.push(axioms);
            }
            // A crash establishes no normal-return guarantee. Its explicit
            // frontier record is validated structurally before proof replay.
            Terminator::Crash { .. } => {}
        }
    }
    let mut exits = exits.into_iter();
    let Some(mut guaranteed) = exits.next() else {
        return Ok(ReconstructedMachineSemantics {
            operation_obligations,
            exit_axioms: Vec::new(),
        });
    };
    for exit in exits {
        guaranteed.retain(|fact| exit.contains(fact));
    }
    Ok(ReconstructedMachineSemantics {
        operation_obligations,
        exit_axioms: guaranteed,
    })
}

/// The complete prior-fact exact divide/remainder families whose canonical
/// proofs need only exact citations, closed integer order, substitution, and
/// transitivity. No value-root custody or operation-definition authority
/// participates.
#[cfg(test)]
fn exact_division_has_closed_prior_certificate(
    goal: &CanonicalScalarGoal,
    semantic_axioms: &[Proposition],
    requirements: &[Proposition],
) -> bool {
    let Ok(Some(proposition)) = goal.kernel_proposition() else {
        return false;
    };
    retained_canonical_integer_proposition(None, &proposition, requirements, semantic_axioms)
}

fn exact_division_has_prior_certificate(
    context: &PropositionContext,
    goal: &CanonicalScalarGoal,
    semantic_axioms: &[Proposition],
    requirements: &[Proposition],
) -> bool {
    let Ok(Some(proposition)) = goal.kernel_proposition() else {
        return false;
    };
    retained_canonical_integer_proposition(
        Some(context),
        &proposition,
        requirements,
        semantic_axioms,
    )
}

fn retained_canonical_integer_proposition(
    context: Option<&PropositionContext>,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    if requirements
        .iter()
        .chain(semantic_axioms)
        .any(|fact| fact == goal)
    {
        return true;
    }
    match goal {
        Proposition::LessOrEqual(_, _) => {
            requirements
                .iter()
                .chain(semantic_axioms)
                .any(|fact| closed_transitive_integer_bound(goal, fact))
                || retained_literal_integer_bound(goal, requirements, semantic_axioms)
                || retained_two_fact_transitive_integer_bound(goal, requirements, semantic_axioms)
                || retained_equality_substituted_integer_bound(
                    context,
                    goal,
                    requirements,
                    semantic_axioms,
                )
                || context.is_some_and(|context| {
                    retained_cast_chain_bound(context, goal, requirements, semantic_axioms)
                })
                || context.is_some_and(|context| {
                    affine_selection::retained(context, goal, requirements, semantic_axioms)
                })
        }
        Proposition::Conjunction(conjuncts) => {
            !conjuncts.is_empty()
                && conjuncts.iter().all(|conjunct| {
                    retained_canonical_integer_proposition(
                        context,
                        conjunct,
                        requirements,
                        semantic_axioms,
                    )
                })
        }
        Proposition::Disjunction(disjuncts) => disjuncts.iter().any(|disjunct| {
            retained_canonical_integer_proposition(context, disjunct, requirements, semantic_axioms)
        }),
        _ => false,
    }
}

fn retained_cast_chain_bound(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    if !matches!(goal, Proposition::LessOrEqual(_, _)) {
        return false;
    }
    if requirements
        .iter()
        .chain(semantic_axioms)
        .filter_map(|root_bound| match root_bound {
            Proposition::LessOrEqual(left, right) => Some((root_bound, left, right)),
            _ => None,
        })
        .any(|(root_bound, root_left, root_right)| {
            [root_left, root_right]
                .into_iter()
                .filter(|root| matches!(root, ScalarTerm::Value { .. }))
                .any(|root| {
                    retained_cast_bound_from_root(context, goal, semantic_axioms, root, root_bound)
                })
        })
    {
        return true;
    }
    if requirements
        .iter()
        .chain(semantic_axioms)
        .filter_map(|equality| match equality {
            Proposition::Equal(left, right) => Some((left, right)),
            _ => None,
        })
        .any(|(left, right)| {
            [(left, right), (right, left)]
                .into_iter()
                .filter(|(root, literal)| {
                    matches!(root, ScalarTerm::Value { .. })
                        && literal.integer_value().is_some_and(|(integer_type, _)| {
                            root.scalar_type() == psi_core::ScalarType::Integer(integer_type)
                        })
                })
                .any(|(root, literal)| {
                    retained_cast_bound_from_landed_literal(
                        context,
                        goal,
                        semantic_axioms,
                        root,
                        literal,
                    )
                })
        })
    {
        return true;
    }
    retained_alias_substituted_cast_bound(context, goal, requirements, semantic_axioms)
}

fn retained_alias_substituted_cast_bound(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    if alias_transport::retained_one(requirements, semantic_axioms, |root, root_bound| {
        retained_cast_bound_from_root(context, goal, semantic_axioms, root, root_bound)
    }) {
        return true;
    }
    alias_transport::retained_stronger_cast(context, goal, requirements, semantic_axioms)
        || alias_transport::retained_landed_literal_cast(
            context,
            goal,
            requirements,
            semantic_axioms,
        )
        || retained_two_alias_substituted_cast_bound(context, goal, requirements, semantic_axioms)
}

fn retained_two_alias_substituted_cast_bound(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    alias_transport::retained_two(requirements, semantic_axioms, |root, root_bound| {
        retained_cast_bound_from_root(context, goal, semantic_axioms, root, root_bound)
    })
}

fn retained_cast_bound_from_landed_literal(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
    root: &ScalarTerm,
    landed_literal: &ScalarTerm,
) -> bool {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return false;
    };
    let psi_core::ScalarType::Integer(root_type) = root.scalar_type() else {
        return false;
    };
    [(goal_right, goal_left, 1), (goal_left, goal_right, 0)]
        .into_iter()
        .filter(|(target, _, _)| matches!(target, ScalarTerm::Value { .. }))
        .any(|(_, target_endpoint, endpoint)| {
            let Some(source_endpoint) = retained_remap_integer_literal(target_endpoint, root_type)
            else {
                return false;
            };
            let closed = if endpoint == 1 {
                closed_integer_less_or_equal(&source_endpoint, landed_literal)
            } else {
                closed_integer_less_or_equal(landed_literal, &source_endpoint)
            };
            if !closed {
                return false;
            }
            let root_bound = if endpoint == 1 {
                Proposition::LessOrEqual(source_endpoint, root.clone())
            } else {
                Proposition::LessOrEqual(root.clone(), source_endpoint)
            };
            retained_cast_bound_from_root(context, goal, semantic_axioms, root, &root_bound)
        })
}

fn retained_remap_integer_literal(
    literal: &ScalarTerm,
    target_type: psi_core::IntegerType,
) -> Option<ScalarTerm> {
    let (source_type, value) = literal.integer_value()?;
    let value = source_type.exact_cast_value_to(target_type, value)?;
    ScalarTerm::integer(target_type, value).ok()
}

fn retained_cast_bound_from_root(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
    root: &ScalarTerm,
    root_bound: &Proposition,
) -> bool {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return false;
    };
    [goal_left, goal_right]
        .into_iter()
        .filter(|target| matches!(target, ScalarTerm::Value { .. }))
        .any(|target| {
            let Some(definition_axioms) =
                retained_exact_cast_chain_axioms(root, target, semantic_axioms)
            else {
                return false;
            };
            check_integer_cast_chain_witness(
                context,
                semantic_axioms,
                &IntegerCastChainWitness {
                    root: root.clone(),
                    target: target.clone(),
                    definition_axioms,
                },
            )
            .is_ok_and(|chain| {
                check_integer_cast_bound_conversion(&chain, root_bound, goal).is_ok()
            })
        })
}

/// Independently reconstruct the unique exact-cast SSA definition spine.
///
/// This follows one definition per reached target and never explores alternate
/// paths or permutations. The proof-kernel witness checker still owns all cast
/// legality, continuity, and carrier validation.
fn retained_exact_cast_chain_axioms(
    root: &ScalarTerm,
    target: &ScalarTerm,
    semantic_axioms: &[Proposition],
) -> Option<Vec<usize>> {
    if root == target {
        return None;
    }
    let mut current = target.clone();
    let mut reversed = Vec::new();
    while &current != root {
        if reversed.len() >= semantic_axioms.len() {
            return None;
        }
        let mut definitions = semantic_axioms
            .iter()
            .enumerate()
            .filter_map(|(index, axiom)| {
                let Proposition::Equal(output, ScalarTerm::IntegerExactCast { operand, .. }) =
                    axiom
                else {
                    return None;
                };
                (output == &current).then(|| (index, operand.as_ref().clone()))
            });
        let (index, operand) = definitions.next()?;
        if definitions.next().is_some() || reversed.contains(&index) {
            return None;
        }
        reversed.push(index);
        current = operand;
    }
    reversed.reverse();
    reversed
        .windows(2)
        .all(|pair| pair[0] < pair[1])
        .then_some(reversed)
}

fn retained_equality_substituted_integer_bound(
    context: Option<&PropositionContext>,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return false;
    };
    let facts = || requirements.iter().chain(semantic_axioms);
    if facts().any(|equality| {
        let Proposition::Equal(equality_left, equality_right) = equality else {
            return false;
        };
        [
            (equality_left, equality_right),
            (equality_right, equality_left),
        ]
        .into_iter()
        .any(|(old, replacement)| {
            let relation = if old == goal_left {
                Proposition::LessOrEqual(replacement.clone(), goal_right.clone())
            } else if old == goal_right {
                Proposition::LessOrEqual(goal_left.clone(), replacement.clone())
            } else {
                return false;
            };
            requirements
                .iter()
                .chain(semantic_axioms)
                .any(|fact| fact == &relation || closed_transitive_integer_bound(&relation, fact))
                || retained_two_fact_transitive_integer_bound(
                    &relation,
                    requirements,
                    semantic_axioms,
                )
                || context.is_some_and(|context| {
                    affine_selection::retained(context, &relation, requirements, semantic_axioms)
                })
        })
    }) {
        return true;
    }

    let Some(context) = context else {
        return false;
    };
    facts()
        .filter_map(|outer_equality| match outer_equality {
            Proposition::Equal(left, right) => Some((outer_equality, left, right)),
            _ => None,
        })
        .any(|(outer_equality, outer_left, outer_right)| {
            [(outer_left, outer_right), (outer_right, outer_left)]
                .into_iter()
                .filter_map(|(old, middle_alias)| {
                    let endpoint = if old == goal_left {
                        0
                    } else if old == goal_right {
                        1
                    } else {
                        return None;
                    };
                    (matches!(old, ScalarTerm::Value { .. })
                        && matches!(middle_alias, ScalarTerm::Value { .. })
                        && old != middle_alias
                        && old.scalar_type() == middle_alias.scalar_type())
                    .then_some((old, middle_alias, endpoint))
                })
                .any(|(old, middle_alias, endpoint)| {
                    facts()
                        .filter(|inner_equality| !std::ptr::eq(outer_equality, *inner_equality))
                        .filter_map(|inner_equality| match inner_equality {
                            Proposition::Equal(left, right) => Some((left, right)),
                            _ => None,
                        })
                        .any(|(inner_left, inner_right)| {
                            let target_alias = if inner_left == middle_alias {
                                inner_right
                            } else if inner_right == middle_alias {
                                inner_left
                            } else {
                                return false;
                            };
                            if !matches!(target_alias, ScalarTerm::Value { .. })
                                || target_alias == old
                                || target_alias == middle_alias
                                || target_alias.scalar_type() != old.scalar_type()
                            {
                                return false;
                            }
                            let relation = if endpoint == 0 {
                                Proposition::LessOrEqual(target_alias.clone(), goal_right.clone())
                            } else {
                                Proposition::LessOrEqual(goal_left.clone(), target_alias.clone())
                            };
                            affine_selection::retained(
                                context,
                                &relation,
                                requirements,
                                semantic_axioms,
                            )
                        })
                })
        })
}

fn retained_literal_integer_bound(
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    let Proposition::LessOrEqual(left, right) = goal else {
        return false;
    };
    if let Some((integer_type, left)) = left.integer_value() {
        return retained_integer_term_values(right, requirements, semantic_axioms).any(
            |(known_type, right)| {
                known_type == integer_type
                    && integer_type.admits(right)
                    && integer_type
                        .compare(left, right)
                        .is_some_and(|order| !order.is_gt())
            },
        );
    }
    if let Some((integer_type, right)) = right.integer_value() {
        return retained_integer_term_values(left, requirements, semantic_axioms).any(
            |(known_type, left)| {
                known_type == integer_type
                    && integer_type.admits(left)
                    && integer_type
                        .compare(left, right)
                        .is_some_and(|order| !order.is_gt())
            },
        );
    }
    false
}

fn retained_integer_term_values<'a>(
    term: &'a ScalarTerm,
    requirements: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> impl Iterator<Item = (psi_core::IntegerType, psi_core::IntegerValue)> + 'a {
    std::iter::once(term.integer_value()).flatten().chain(
        requirements
            .iter()
            .chain(semantic_axioms)
            .filter_map(move |fact| {
                let Proposition::Equal(left, right) = fact else {
                    return None;
                };
                if left == term {
                    right.integer_value()
                } else if right == term {
                    left.integer_value()
                } else {
                    None
                }
            }),
    )
}

fn retained_two_fact_transitive_integer_bound(
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return false;
    };
    requirements.iter().chain(semantic_axioms).any(|left_fact| {
        let Proposition::LessOrEqual(left, middle) = left_fact else {
            return false;
        };
        left == goal_left
            && requirements
                .iter()
                .chain(semantic_axioms)
                .any(|right_fact| {
                    matches!(
                        right_fact,
                        Proposition::LessOrEqual(right_middle, right)
                            if right_middle == middle && right == goal_right
                    )
                })
    })
}

fn closed_transitive_integer_bound(goal: &Proposition, retained: &Proposition) -> bool {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return false;
    };
    let Proposition::LessOrEqual(retained_left, retained_right) = retained else {
        return false;
    };
    (retained_right == goal_right && closed_integer_less_or_equal(goal_left, retained_left))
        || (retained_left == goal_left && closed_integer_less_or_equal(retained_right, goal_right))
}

fn closed_integer_less_or_equal(left: &ScalarTerm, right: &ScalarTerm) -> bool {
    let Some((left_type, left)) = left.integer_value() else {
        return false;
    };
    let Some((right_type, right)) = right.integer_value() else {
        return false;
    };
    left_type == right_type
        && left_type
            .compare(left, right)
            .is_some_and(|order| !order.is_gt())
}

fn true_condition_fact(
    condition: ValueId,
    axioms: &[Proposition],
    value_term: &impl Fn(ValueId) -> ScalarTerm,
) -> Option<Proposition> {
    let condition = value_term(condition);
    let predicate = axioms.iter().rev().find_map(|axiom| match axiom {
        Proposition::Equal(left, right) if left == &condition => Some(right),
        Proposition::Equal(left, right) if right == &condition => Some(left),
        _ => None,
    })?;
    let constants = axioms
        .iter()
        .filter_map(|axiom| match axiom {
            Proposition::Equal(ScalarTerm::Value { id, .. }, value)
                if matches!(value, ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. }) =>
            {
                Some((*id, value.clone()))
            }
            Proposition::Equal(value, ScalarTerm::Value { id, .. })
                if matches!(value, ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. }) =>
            {
                Some((*id, value.clone()))
            }
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let predicate = substitute_scalar_term_values(predicate, &constants);
    match &predicate {
        ScalarTerm::BooleanEqual { left, right } | ScalarTerm::IntegerEqual { left, right, .. } => {
            Some(Proposition::Equal((**left).clone(), (**right).clone()))
        }
        ScalarTerm::IntegerLessThan { left, right, .. } => {
            Some(Proposition::LessThan((**left).clone(), (**right).clone()))
        }
        ScalarTerm::IntegerLessOrEqual { left, right, .. } => Some(Proposition::LessOrEqual(
            (**left).clone(),
            (**right).clone(),
        )),
        _ => None,
    }
}

fn bind_successor_axioms(
    axioms: &mut Vec<Proposition>,
    target_block: &psi_terminal::Block,
    arguments: &[ValueId],
    value_term: &impl Fn(ValueId) -> ScalarTerm,
    rewrite_path_facts: bool,
) {
    let substitutions = target_block
        .parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| (*argument, value_term(parameter.id)))
        .collect::<BTreeMap<_, _>>();
    let established = axioms.clone();
    // Emit edge equalities before rewritten path facts so independently
    // reconstructed axiom indexes are deterministic.
    for (parameter, argument) in target_block.parameters.iter().zip(arguments) {
        axioms.push(Proposition::Equal(
            value_term(parameter.id),
            value_term(*argument),
        ));
    }
    if rewrite_path_facts {
        for proposition in &established {
            let rewritten = substitute_proposition_values(proposition, &substitutions);
            push_unique(axioms, rewritten);
        }
    }
}

fn append_successor_fact(
    axioms: &mut Vec<Proposition>,
    proposition: &Proposition,
    target_block: &psi_terminal::Block,
    arguments: &[ValueId],
    value_term: &impl Fn(ValueId) -> ScalarTerm,
) {
    let substitutions = target_block
        .parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| (*argument, value_term(parameter.id)))
        .collect::<BTreeMap<_, _>>();
    push_unique(axioms, proposition.clone());
    push_unique(
        axioms,
        substitute_proposition_values(proposition, &substitutions),
    );
}

fn push_unique(propositions: &mut Vec<Proposition>, proposition: Proposition) {
    if !propositions.contains(&proposition) {
        propositions.push(proposition);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_core::{IntegerSign, IntegerType, IntegerValue, ScalarType};

    fn value(id: u64, integer_type: IntegerType) -> ScalarTerm {
        ScalarTerm::value(
            ValueId::new(id).expect("value id"),
            ScalarType::Integer(integer_type),
        )
    }

    #[test]
    fn exact_division_selects_canonical_certificate_only_for_complete_prior_facts() {
        let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let unsigned_right = value(2, unsigned);
        let unsigned_goal = CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: unsigned,
            left: value(1, unsigned),
            right: unsigned_right.clone(),
        };
        assert!(exact_division_has_closed_prior_certificate(
            &unsigned_goal,
            &[],
            &[Proposition::LessOrEqual(
                ScalarTerm::integer(unsigned, IntegerValue::Unsigned(1)).expect("u8 one"),
                unsigned_right.clone(),
            )],
        ));
        assert!(exact_division_has_closed_prior_certificate(
            &unsigned_goal,
            &[],
            &[Proposition::LessOrEqual(
                ScalarTerm::integer(unsigned, IntegerValue::Unsigned(2))
                    .expect("stronger u8 floor"),
                unsigned_right.clone(),
            )],
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &unsigned_goal,
            &[],
            &[Proposition::LessOrEqual(
                ScalarTerm::integer(unsigned, IntegerValue::Unsigned(0)).expect("weak u8 floor"),
                unsigned_right.clone(),
            )],
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &unsigned_goal,
            &[],
            &[Proposition::LessOrEqual(
                ScalarTerm::integer(unsigned, IntegerValue::Unsigned(2))
                    .expect("stronger u8 floor"),
                value(9, unsigned),
            )],
        ));
        assert!(exact_division_has_closed_prior_certificate(
            &unsigned_goal,
            &[Proposition::Equal(
                unsigned_right.clone(),
                ScalarTerm::integer(unsigned, IntegerValue::Unsigned(5)).expect("u8 literal"),
            )],
            &[],
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &unsigned_goal,
            &[Proposition::Equal(
                unsigned_right,
                ScalarTerm::integer(unsigned, IntegerValue::Unsigned(0)).expect("u8 literal"),
            )],
            &[],
        ));

        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        for (literal, expected) in [(-3, true), (1, true), (0, false), (-1, false)] {
            assert_eq!(
                exact_division_has_closed_prior_certificate(
                    &CanonicalScalarGoal::ExactDivisionDefined {
                        integer_type: signed,
                        left: value(3, signed),
                        right: value(4, signed),
                    },
                    &[Proposition::Equal(
                        value(4, signed),
                        ScalarTerm::integer(signed, IntegerValue::Signed(literal))
                            .expect("i8 literal"),
                    )],
                    &[],
                ),
                expected,
                "signed literal {literal}",
            );
        }
        assert!(!exact_division_has_closed_prior_certificate(
            &CanonicalScalarGoal::ExactDivisionDefined {
                integer_type: signed,
                left: value(3, signed),
                right: value(4, signed),
            },
            &[],
            &[],
        ));

        let negative_one_goal = CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: signed,
            left: value(5, signed),
            right: value(6, signed),
        };
        let negative_one = Proposition::Equal(
            value(6, signed),
            ScalarTerm::integer(signed, IntegerValue::Signed(-1)).expect("i8 literal"),
        );
        assert!(exact_division_has_closed_prior_certificate(
            &negative_one_goal,
            &[],
            &[Proposition::LessOrEqual(
                ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
                value(6, signed),
            )],
        ));
        assert!(exact_division_has_closed_prior_certificate(
            &negative_one_goal,
            &[],
            &[Proposition::LessOrEqual(
                ScalarTerm::integer(signed, IntegerValue::Signed(3))
                    .expect("stronger i8 positive floor"),
                value(6, signed),
            )],
        ));
        assert!(exact_division_has_closed_prior_certificate(
            &negative_one_goal,
            &[Proposition::LessOrEqual(
                value(6, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(-2)).expect("i8 -2"),
            )],
            &[],
        ));
        assert!(exact_division_has_closed_prior_certificate(
            &negative_one_goal,
            &[Proposition::LessOrEqual(
                value(6, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(-3))
                    .expect("stronger i8 negative ceiling"),
            )],
            &[],
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &negative_one_goal,
            &[Proposition::LessOrEqual(
                value(6, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(-1))
                    .expect("weak i8 negative ceiling"),
            )],
            &[],
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &negative_one_goal,
            &[],
            &[Proposition::LessOrEqual(
                ScalarTerm::integer(signed, IntegerValue::Signed(0)).expect("i8 zero"),
                value(6, signed),
            )],
        ));
        assert!(exact_division_has_closed_prior_certificate(
            &negative_one_goal,
            &[
                negative_one.clone(),
                Proposition::Equal(
                    value(5, signed),
                    ScalarTerm::integer(signed, IntegerValue::Signed(-7)).expect("i8 literal"),
                ),
            ],
            &[],
        ));
        assert!(exact_division_has_closed_prior_certificate(
            &negative_one_goal,
            std::slice::from_ref(&Proposition::Equal(
                value(6, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(-1)).expect("i8 -1"),
            )),
            &[Proposition::LessOrEqual(
                ScalarTerm::integer(signed, IntegerValue::Signed(-120))
                    .expect("stronger i8 lower bound"),
                value(5, signed),
            )],
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &negative_one_goal,
            std::slice::from_ref(&negative_one),
            &[],
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &negative_one_goal,
            &[
                negative_one,
                Proposition::Equal(
                    value(5, signed),
                    ScalarTerm::integer(signed, signed.minimum_value()).expect("i8 minimum"),
                ),
            ],
            &[],
        ));

        let exact_bound = Proposition::LessOrEqual(
            ScalarTerm::integer(signed, IntegerValue::Signed(-127)).expect("i8 minimum + 1"),
            value(5, signed),
        );
        assert!(exact_division_has_closed_prior_certificate(
            &negative_one_goal,
            std::slice::from_ref(&Proposition::Equal(
                value(6, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(-1)).expect("i8 -1"),
            )),
            std::slice::from_ref(&exact_bound),
        ));
        assert!(exact_division_has_closed_prior_certificate(
            &negative_one_goal,
            &[
                Proposition::Equal(
                    value(6, signed),
                    ScalarTerm::integer(signed, IntegerValue::Signed(-1)).expect("i8 -1"),
                ),
                exact_bound,
            ],
            &[],
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &negative_one_goal,
            std::slice::from_ref(&Proposition::Equal(
                value(6, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(-1)).expect("i8 -1"),
            )),
            &[Proposition::LessOrEqual(
                ScalarTerm::integer(signed, signed.minimum_value()).expect("i8 minimum"),
                value(5, signed),
            )],
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &negative_one_goal,
            std::slice::from_ref(&Proposition::Equal(
                value(6, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(-1)).expect("i8 -1"),
            )),
            &[Proposition::LessOrEqual(
                ScalarTerm::integer(signed, IntegerValue::Signed(-127)).expect("i8 minimum + 1"),
                value(9, signed),
            )],
        ));

        let i1 = IntegerType::new(IntegerSign::Signed, 1).expect("i1");
        let i1_goal = CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: i1,
            left: value(7, i1),
            right: value(8, i1),
        };
        assert!(!exact_division_has_closed_prior_certificate(
            &i1_goal,
            &[],
            &[Proposition::LessOrEqual(
                value(8, i1),
                ScalarTerm::integer(i1, IntegerValue::Signed(-1)).expect("i1 -1"),
            )],
        ));
        let i1_divisor_bound = Proposition::LessOrEqual(
            value(8, i1),
            ScalarTerm::integer(i1, IntegerValue::Signed(-1)).expect("i1 -1"),
        );
        let i1_dividend_bound = Proposition::LessOrEqual(
            ScalarTerm::integer(i1, IntegerValue::Signed(0)).expect("i1 zero"),
            value(7, i1),
        );
        assert!(exact_division_has_closed_prior_certificate(
            &i1_goal,
            &[],
            &[i1_divisor_bound.clone(), i1_dividend_bound.clone()],
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &i1_goal,
            &[],
            &[
                i1_divisor_bound,
                Proposition::LessOrEqual(
                    ScalarTerm::integer(i1, IntegerValue::Signed(0)).expect("i1 zero"),
                    value(9, i1),
                ),
            ],
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &i1_goal,
            &[],
            &[
                Proposition::LessOrEqual(
                    value(9, i1),
                    ScalarTerm::integer(i1, IntegerValue::Signed(-1)).expect("i1 -1"),
                ),
                i1_dividend_bound,
            ],
        ));
        assert!(exact_division_has_closed_prior_certificate(
            &i1_goal,
            &[
                Proposition::Equal(
                    value(8, i1),
                    ScalarTerm::integer(i1, IntegerValue::Signed(-1)).expect("i1 -1"),
                ),
                Proposition::Equal(
                    value(7, i1),
                    ScalarTerm::integer(i1, IntegerValue::Signed(0)).expect("i1 zero"),
                ),
            ],
            &[],
        ));
    }

    #[test]
    fn exact_division_selects_two_exact_transitive_safe_divisor_facts() {
        let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let unsigned_goal = CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: unsigned,
            left: value(1, unsigned),
            right: value(2, unsigned),
        };
        let unsigned_head = Proposition::LessOrEqual(
            ScalarTerm::integer(unsigned, IntegerValue::Unsigned(1)).expect("u8 one"),
            value(3, unsigned),
        );
        let unsigned_tail = Proposition::LessOrEqual(value(3, unsigned), value(2, unsigned));
        assert!(exact_division_has_closed_prior_certificate(
            &unsigned_goal,
            &[],
            &[unsigned_head.clone(), unsigned_tail.clone()],
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &unsigned_goal,
            &[],
            std::slice::from_ref(&unsigned_head),
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &unsigned_goal,
            &[],
            &[
                unsigned_head,
                Proposition::LessOrEqual(value(3, unsigned), value(4, unsigned)),
            ],
        ));

        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let signed_goal = CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: signed,
            left: value(1, signed),
            right: value(2, signed),
        };
        assert!(exact_division_has_closed_prior_certificate(
            &signed_goal,
            &[],
            &[
                Proposition::LessOrEqual(
                    ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
                    value(3, signed),
                ),
                Proposition::LessOrEqual(value(3, signed), value(2, signed)),
            ],
        ));
        assert!(exact_division_has_closed_prior_certificate(
            &signed_goal,
            &[],
            &[
                Proposition::LessOrEqual(value(2, signed), value(3, signed)),
                Proposition::LessOrEqual(
                    value(3, signed),
                    ScalarTerm::integer(signed, IntegerValue::Signed(-2)).expect("i8 -2"),
                ),
            ],
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &signed_goal,
            &[],
            &[
                Proposition::LessOrEqual(value(2, signed), value(3, signed)),
                Proposition::LessOrEqual(
                    value(4, signed),
                    ScalarTerm::integer(signed, IntegerValue::Signed(-2)).expect("i8 -2"),
                ),
            ],
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &signed_goal,
            &[],
            &[
                Proposition::LessOrEqual(
                    value(3, signed),
                    ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
                ),
                Proposition::LessOrEqual(value(2, signed), value(3, signed)),
            ],
        ));
    }

    #[test]
    fn exact_division_selects_complete_signed_joint_prior_bounds() {
        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let goal = CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: signed,
            left: value(1, signed),
            right: value(2, signed),
        };
        let divisor_bound = Proposition::LessOrEqual(
            value(2, signed),
            ScalarTerm::integer(signed, IntegerValue::Signed(-1)).expect("i8 -1"),
        );
        let dividend_bound = Proposition::LessOrEqual(
            ScalarTerm::integer(signed, IntegerValue::Signed(-127)).expect("i8 minimum + 1"),
            value(1, signed),
        );
        assert!(exact_division_has_closed_prior_certificate(
            &goal,
            &[],
            &[divisor_bound.clone(), dividend_bound.clone()],
        ));
        assert!(exact_division_has_closed_prior_certificate(
            &goal,
            &[Proposition::Equal(
                value(1, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(-7))
                    .expect("nonminimum i8 dividend"),
            )],
            std::slice::from_ref(&divisor_bound),
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &goal,
            &[Proposition::Equal(
                value(1, signed),
                ScalarTerm::integer(signed, signed.minimum_value()).expect("minimum i8 dividend"),
            )],
            std::slice::from_ref(&divisor_bound),
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &goal,
            &[Proposition::Equal(
                value(3, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(-7))
                    .expect("wrong nonminimum i8 dividend"),
            )],
            std::slice::from_ref(&divisor_bound),
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &goal,
            &[],
            std::slice::from_ref(&divisor_bound),
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &goal,
            &[],
            std::slice::from_ref(&dividend_bound),
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &goal,
            &[],
            &[
                divisor_bound.clone(),
                Proposition::LessOrEqual(
                    ScalarTerm::integer(signed, IntegerValue::Signed(-127))
                        .expect("i8 minimum + 1"),
                    value(3, signed),
                ),
            ],
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &goal,
            &[],
            &[
                Proposition::LessOrEqual(
                    value(3, signed),
                    ScalarTerm::integer(signed, IntegerValue::Signed(-1)).expect("i8 -1"),
                ),
                dividend_bound,
            ],
        ));
    }

    #[test]
    fn exact_division_selects_exact_retained_canonical_goal_or_arm() {
        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let goal = CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: signed,
            left: value(1, signed),
            right: value(2, signed),
        };
        let canonical = goal
            .kernel_proposition()
            .expect("exact goal projects")
            .expect("exact goal has a kernel proposition");
        assert!(exact_division_has_closed_prior_certificate(
            &goal,
            &[],
            std::slice::from_ref(&canonical),
        ));
        assert!(exact_division_has_closed_prior_certificate(
            &goal,
            std::slice::from_ref(&canonical),
            &[],
        ));
        let Proposition::Disjunction(disjuncts) = &canonical else {
            panic!("signed exact goal is an ordered disjunction")
        };
        let joint_arm = disjuncts[2].clone();
        assert!(exact_division_has_closed_prior_certificate(
            &goal,
            &[],
            std::slice::from_ref(&joint_arm),
        ));
        let Proposition::Conjunction(joint_conjuncts) = joint_arm else {
            panic!("signed exceptional arm is a conjunction")
        };
        assert!(!exact_division_has_closed_prior_certificate(
            &goal,
            &[],
            &[Proposition::Conjunction(vec![
                joint_conjuncts[1].clone(),
                joint_conjuncts[0].clone(),
            ])],
        ));
        let redirected = CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: signed,
            left: value(1, signed),
            right: value(3, signed),
        }
        .kernel_proposition()
        .expect("redirected exact goal projects")
        .expect("redirected exact goal has a kernel proposition");
        assert!(!exact_division_has_closed_prior_certificate(
            &goal,
            &[],
            &[redirected],
        ));
    }

    #[test]
    fn exact_division_selects_literal_equalities_from_requirements() {
        let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let unsigned_goal = CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: unsigned,
            left: value(1, unsigned),
            right: value(2, unsigned),
        };
        assert!(exact_division_has_closed_prior_certificate(
            &unsigned_goal,
            &[],
            &[Proposition::Equal(
                value(2, unsigned),
                ScalarTerm::integer(unsigned, IntegerValue::Unsigned(5)).expect("safe u8 divisor"),
            )],
        ));
        assert!(exact_division_has_closed_prior_certificate(
            &unsigned_goal,
            &[],
            &[
                Proposition::Equal(
                    value(2, unsigned),
                    ScalarTerm::integer(unsigned, IntegerValue::Unsigned(0))
                        .expect("stale zero u8 divisor"),
                ),
                Proposition::Equal(
                    value(2, unsigned),
                    ScalarTerm::integer(unsigned, IntegerValue::Unsigned(5))
                        .expect("safe u8 divisor"),
                ),
            ],
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &unsigned_goal,
            &[],
            &[Proposition::Equal(
                value(2, unsigned),
                ScalarTerm::integer(unsigned, IntegerValue::Unsigned(0)).expect("zero u8 divisor"),
            )],
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &unsigned_goal,
            &[],
            &[Proposition::Equal(
                value(3, unsigned),
                ScalarTerm::integer(unsigned, IntegerValue::Unsigned(5))
                    .expect("redirected u8 divisor"),
            )],
        ));

        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let signed_goal = CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: signed,
            left: value(1, signed),
            right: value(2, signed),
        };
        let divisor_bound = Proposition::LessOrEqual(
            value(2, signed),
            ScalarTerm::integer(signed, IntegerValue::Signed(-1)).expect("i8 -1"),
        );
        assert!(exact_division_has_closed_prior_certificate(
            &signed_goal,
            &[],
            &[
                divisor_bound.clone(),
                Proposition::Equal(
                    value(1, signed),
                    ScalarTerm::integer(signed, IntegerValue::Signed(-7))
                        .expect("safe i8 dividend"),
                ),
            ],
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &signed_goal,
            &[],
            &[
                divisor_bound,
                Proposition::Equal(
                    value(1, signed),
                    ScalarTerm::integer(signed, signed.minimum_value())
                        .expect("minimum i8 dividend"),
                ),
            ],
        ));
    }

    #[test]
    fn exact_division_selects_exact_endpoint_equality_transport() {
        let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let unsigned_goal = CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: unsigned,
            left: value(1, unsigned),
            right: value(2, unsigned),
        };
        let intermediate_bound = Proposition::LessOrEqual(
            ScalarTerm::integer(unsigned, IntegerValue::Unsigned(1)).expect("u8 one"),
            value(3, unsigned),
        );
        let divisor_equality = Proposition::Equal(value(3, unsigned), value(2, unsigned));
        assert!(exact_division_has_closed_prior_certificate(
            &unsigned_goal,
            &[],
            &[intermediate_bound.clone(), divisor_equality.clone()],
        ));
        assert!(exact_division_has_closed_prior_certificate(
            &unsigned_goal,
            &[],
            &[
                intermediate_bound.clone(),
                Proposition::Equal(value(2, unsigned), value(3, unsigned)),
            ],
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &unsigned_goal,
            &[],
            std::slice::from_ref(&divisor_equality),
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &unsigned_goal,
            &[],
            &[
                intermediate_bound,
                Proposition::Equal(value(3, unsigned), value(1, unsigned)),
            ],
        ));

        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let signed_goal = CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: signed,
            left: value(1, signed),
            right: value(2, signed),
        };
        assert!(exact_division_has_closed_prior_certificate(
            &signed_goal,
            &[],
            &[
                Proposition::LessOrEqual(
                    value(3, signed),
                    ScalarTerm::integer(signed, IntegerValue::Signed(-2)).expect("i8 -2"),
                ),
                Proposition::Equal(value(3, signed), value(2, signed)),
            ],
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &signed_goal,
            &[],
            &[
                Proposition::LessOrEqual(
                    value(3, signed),
                    ScalarTerm::integer(signed, IntegerValue::Signed(-1)).expect("i8 -1"),
                ),
                Proposition::Equal(value(3, signed), value(2, signed)),
            ],
        ));
        let signed_divisor_bound = Proposition::LessOrEqual(
            value(2, signed),
            ScalarTerm::integer(signed, IntegerValue::Signed(-1)).expect("i8 -1"),
        );
        let intermediate_dividend_bound = Proposition::LessOrEqual(
            ScalarTerm::integer(signed, IntegerValue::Signed(-127)).expect("i8 minimum + 1"),
            value(3, signed),
        );
        assert!(exact_division_has_closed_prior_certificate(
            &signed_goal,
            &[],
            &[
                signed_divisor_bound.clone(),
                intermediate_dividend_bound.clone(),
                Proposition::Equal(value(3, signed), value(1, signed)),
            ],
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &signed_goal,
            &[],
            &[
                intermediate_dividend_bound.clone(),
                Proposition::Equal(value(3, signed), value(1, signed)),
            ],
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &signed_goal,
            &[],
            &[
                signed_divisor_bound,
                intermediate_dividend_bound,
                Proposition::Equal(value(3, signed), value(2, signed)),
            ],
        ));
    }

    #[test]
    fn i1_exact_division_selects_transport_for_both_joint_endpoints() {
        let i1 = IntegerType::new(IntegerSign::Signed, 1).expect("i1");
        let goal = CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: i1,
            left: value(1, i1),
            right: value(2, i1),
        };
        let divisor_bound = Proposition::LessOrEqual(
            value(3, i1),
            ScalarTerm::integer(i1, IntegerValue::Signed(-1)).expect("i1 -1"),
        );
        let divisor_equality = Proposition::Equal(value(3, i1), value(2, i1));
        let dividend_bound = Proposition::LessOrEqual(
            ScalarTerm::integer(i1, IntegerValue::Signed(0)).expect("i1 zero"),
            value(4, i1),
        );
        let dividend_equality = Proposition::Equal(value(4, i1), value(1, i1));
        assert!(exact_division_has_closed_prior_certificate(
            &goal,
            &[],
            &[
                divisor_bound.clone(),
                divisor_equality.clone(),
                dividend_bound.clone(),
                dividend_equality.clone(),
            ],
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &goal,
            &[],
            &[
                divisor_bound.clone(),
                divisor_equality.clone(),
                dividend_bound.clone(),
            ],
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &goal,
            &[],
            &[
                divisor_bound,
                Proposition::Equal(value(3, i1), value(1, i1)),
                dividend_bound,
                Proposition::Equal(value(4, i1), value(2, i1)),
            ],
        ));
    }

    #[test]
    fn exact_division_selects_closed_transitivity_under_endpoint_transport() {
        let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let unsigned_goal = CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: unsigned,
            left: value(1, unsigned),
            right: value(2, unsigned),
        };
        assert!(exact_division_has_closed_prior_certificate(
            &unsigned_goal,
            &[],
            &[
                Proposition::LessOrEqual(
                    ScalarTerm::integer(unsigned, IntegerValue::Unsigned(2))
                        .expect("stronger u8 floor"),
                    value(3, unsigned),
                ),
                Proposition::Equal(value(3, unsigned), value(2, unsigned)),
            ],
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &unsigned_goal,
            &[],
            &[
                Proposition::LessOrEqual(
                    ScalarTerm::integer(unsigned, IntegerValue::Unsigned(0))
                        .expect("weak u8 floor"),
                    value(3, unsigned),
                ),
                Proposition::Equal(value(3, unsigned), value(2, unsigned)),
            ],
        ));

        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let signed_goal = CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: signed,
            left: value(1, signed),
            right: value(2, signed),
        };
        assert!(exact_division_has_closed_prior_certificate(
            &signed_goal,
            &[],
            &[
                Proposition::LessOrEqual(
                    value(3, signed),
                    ScalarTerm::integer(signed, IntegerValue::Signed(-3))
                        .expect("stronger i8 ceiling"),
                ),
                Proposition::Equal(value(3, signed), value(2, signed)),
            ],
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &signed_goal,
            &[],
            &[
                Proposition::LessOrEqual(
                    value(3, signed),
                    ScalarTerm::integer(signed, IntegerValue::Signed(-1)).expect("weak i8 ceiling"),
                ),
                Proposition::Equal(value(3, signed), value(2, signed)),
            ],
        ));
    }

    #[test]
    fn exact_division_selects_two_citation_transitivity_under_endpoint_transport() {
        let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let unsigned_goal = CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: unsigned,
            left: value(1, unsigned),
            right: value(2, unsigned),
        };
        assert!(exact_division_has_closed_prior_certificate(
            &unsigned_goal,
            &[],
            &[
                Proposition::LessOrEqual(
                    ScalarTerm::integer(unsigned, IntegerValue::Unsigned(1)).expect("u8 one"),
                    value(4, unsigned),
                ),
                Proposition::LessOrEqual(value(4, unsigned), value(3, unsigned)),
                Proposition::Equal(value(3, unsigned), value(2, unsigned)),
            ],
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &unsigned_goal,
            &[],
            &[
                Proposition::LessOrEqual(
                    ScalarTerm::integer(unsigned, IntegerValue::Unsigned(1)).expect("u8 one"),
                    value(4, unsigned),
                ),
                Proposition::LessOrEqual(value(1, unsigned), value(3, unsigned)),
                Proposition::Equal(value(3, unsigned), value(2, unsigned)),
            ],
        ));

        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let signed_goal = CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: signed,
            left: value(1, signed),
            right: value(2, signed),
        };
        assert!(exact_division_has_closed_prior_certificate(
            &signed_goal,
            &[],
            &[
                Proposition::LessOrEqual(value(3, signed), value(4, signed)),
                Proposition::LessOrEqual(
                    value(4, signed),
                    ScalarTerm::integer(signed, IntegerValue::Signed(-2)).expect("i8 -2"),
                ),
                Proposition::Equal(value(3, signed), value(2, signed)),
            ],
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &signed_goal,
            &[],
            &[
                Proposition::LessOrEqual(value(3, signed), value(4, signed)),
                Proposition::LessOrEqual(
                    value(4, signed),
                    ScalarTerm::integer(signed, IntegerValue::Signed(-1)).expect("i8 -1"),
                ),
                Proposition::Equal(value(3, signed), value(2, signed)),
            ],
        ));
    }

    #[test]
    fn exact_division_selects_two_citation_dividend_floor_under_endpoint_transport() {
        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let goal = CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: signed,
            left: value(1, signed),
            right: value(2, signed),
        };
        let divisor_bound = Proposition::LessOrEqual(
            value(2, signed),
            ScalarTerm::integer(signed, IntegerValue::Signed(-1)).expect("i8 -1"),
        );
        let dividend_floor = Proposition::LessOrEqual(
            ScalarTerm::integer(signed, IntegerValue::Signed(-127)).expect("i8 minimum + 1"),
            value(4, signed),
        );
        let middle_bound = Proposition::LessOrEqual(value(4, signed), value(3, signed));
        let dividend_equality = Proposition::Equal(value(3, signed), value(1, signed));
        assert!(exact_division_has_closed_prior_certificate(
            &goal,
            std::slice::from_ref(&divisor_bound),
            &[
                dividend_floor.clone(),
                middle_bound.clone(),
                dividend_equality.clone(),
            ],
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &goal,
            std::slice::from_ref(&divisor_bound),
            &[dividend_floor.clone(), dividend_equality.clone()],
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &goal,
            &[divisor_bound],
            &[
                dividend_floor,
                Proposition::LessOrEqual(value(4, signed), value(2, signed)),
                dividend_equality,
            ],
        ));
    }

    #[test]
    fn i1_exact_division_selects_two_citation_transport_for_both_endpoints() {
        let i1 = IntegerType::new(IntegerSign::Signed, 1).expect("i1");
        let goal = CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: i1,
            left: value(1, i1),
            right: value(2, i1),
        };
        let facts = [
            Proposition::LessOrEqual(value(3, i1), value(4, i1)),
            Proposition::LessOrEqual(
                value(4, i1),
                ScalarTerm::integer(i1, IntegerValue::Signed(-1)).expect("i1 -1"),
            ),
            Proposition::Equal(value(3, i1), value(2, i1)),
            Proposition::LessOrEqual(
                ScalarTerm::integer(i1, IntegerValue::Signed(0)).expect("i1 zero"),
                value(6, i1),
            ),
            Proposition::LessOrEqual(value(6, i1), value(5, i1)),
            Proposition::Equal(value(5, i1), value(1, i1)),
        ];
        assert!(exact_division_has_closed_prior_certificate(
            &goal,
            &facts[..3],
            &facts[3..],
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &goal,
            &facts[..3],
            &[facts[3].clone(), facts[5].clone()],
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &goal,
            &[facts[0].clone(), facts[2].clone()],
            &facts[3..],
        ));
    }

    #[test]
    fn exact_division_selects_two_citation_bounds_for_both_signed_joint_conjuncts() {
        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let goal = CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: signed,
            left: value(1, signed),
            right: value(2, signed),
        };
        let facts = [
            Proposition::LessOrEqual(value(2, signed), value(3, signed)),
            Proposition::LessOrEqual(
                value(3, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(-1)).expect("i8 -1"),
            ),
            Proposition::LessOrEqual(
                ScalarTerm::integer(signed, IntegerValue::Signed(-127)).expect("i8 minimum + 1"),
                value(4, signed),
            ),
            Proposition::LessOrEqual(value(4, signed), value(1, signed)),
        ];
        assert!(exact_division_has_closed_prior_certificate(
            &goal,
            &facts[..2],
            &facts[2..],
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &goal,
            std::slice::from_ref(&facts[0]),
            &facts[2..],
        ));
        assert!(!exact_division_has_closed_prior_certificate(
            &goal,
            &facts[..2],
            std::slice::from_ref(&facts[2]),
        ));
    }

    #[test]
    fn exact_division_selects_single_definition_affine_safe_divisor() {
        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let context = PropositionContext::from_value_types((1..=4).map(|id| {
            (
                ValueId::new(id).expect("value id"),
                ScalarType::Integer(signed),
            )
        }))
        .expect("four i8 values");
        let goal = CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: signed,
            left: value(1, signed),
            right: value(2, signed),
        };
        let root_bound = Proposition::LessOrEqual(
            ScalarTerm::integer(signed, IntegerValue::Signed(0)).expect("i8 zero"),
            value(3, signed),
        );
        let definition = Proposition::Equal(
            value(2, signed),
            ScalarTerm::exact_integer_add(
                signed,
                value(3, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
            )
            .expect("exact add"),
        );
        assert!(exact_division_has_prior_certificate(
            &context,
            &goal,
            std::slice::from_ref(&definition),
            std::slice::from_ref(&root_bound),
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            std::slice::from_ref(&definition),
            &[],
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            &[Proposition::Equal(
                value(4, signed),
                ScalarTerm::exact_integer_add(
                    signed,
                    value(3, signed),
                    ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
                )
                .expect("redirected exact add"),
            )],
            &[root_bound],
        ));
    }

    #[test]
    fn exact_division_selects_stronger_affine_endpoint_bounds() {
        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let context = PropositionContext::from_value_types((1..=3).map(|id| {
            (
                ValueId::new(id).expect("value id"),
                ScalarType::Integer(signed),
            )
        }))
        .expect("three i8 values");
        let goal = CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: signed,
            left: value(1, signed),
            right: value(2, signed),
        };
        let zero = ScalarTerm::integer(signed, IntegerValue::Signed(0)).expect("i8 zero");
        let positive_root_bound = Proposition::LessOrEqual(zero.clone(), value(3, signed));
        let positive_definition = Proposition::Equal(
            value(2, signed),
            ScalarTerm::exact_integer_add(
                signed,
                value(3, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(2)).expect("i8 two"),
            )
            .expect("exact add"),
        );
        assert!(exact_division_has_prior_certificate(
            &context,
            &goal,
            std::slice::from_ref(&positive_definition),
            std::slice::from_ref(&positive_root_bound),
        ));

        let negative_root_bound = Proposition::LessOrEqual(value(3, signed), zero);
        let negative_definition = Proposition::Equal(
            value(2, signed),
            ScalarTerm::exact_integer_subtract(
                signed,
                value(3, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(3)).expect("i8 three"),
            )
            .expect("exact subtract"),
        );
        assert!(exact_division_has_prior_certificate(
            &context,
            &goal,
            std::slice::from_ref(&negative_definition),
            std::slice::from_ref(&negative_root_bound),
        ));

        let weak_definition = Proposition::Equal(
            value(2, signed),
            ScalarTerm::exact_integer_add(
                signed,
                value(3, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(0)).expect("i8 zero"),
            )
            .expect("exact add zero"),
        );
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            &[weak_definition],
            &[positive_root_bound],
        ));
    }

    #[test]
    fn exact_division_selects_landed_literal_affine_root() {
        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let context = PropositionContext::from_value_types((1..=4).map(|id| {
            (
                ValueId::new(id).expect("value id"),
                ScalarType::Integer(signed),
            )
        }))
        .expect("four i8 values");
        let goal = CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: signed,
            left: value(1, signed),
            right: value(2, signed),
        };
        let zero = ScalarTerm::integer(signed, IntegerValue::Signed(0)).expect("i8 zero");
        let landed_root = Proposition::Equal(value(3, signed), zero.clone());
        let positive_definition = Proposition::Equal(
            value(2, signed),
            ScalarTerm::exact_integer_add(
                signed,
                value(3, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
            )
            .expect("exact add"),
        );
        assert!(exact_division_has_prior_certificate(
            &context,
            &goal,
            std::slice::from_ref(&positive_definition),
            std::slice::from_ref(&landed_root),
        ));

        let negative_definition = Proposition::Equal(
            value(2, signed),
            ScalarTerm::exact_integer_subtract(
                signed,
                value(3, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(2)).expect("i8 two"),
            )
            .expect("exact subtract"),
        );
        assert!(exact_division_has_prior_certificate(
            &context,
            &goal,
            std::slice::from_ref(&negative_definition),
            std::slice::from_ref(&landed_root),
        ));

        let unsafe_definition = Proposition::Equal(
            value(2, signed),
            ScalarTerm::exact_integer_add(signed, value(3, signed), zero).expect("exact add zero"),
        );
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            std::slice::from_ref(&unsafe_definition),
            std::slice::from_ref(&landed_root),
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            &[positive_definition],
            &[Proposition::Equal(
                value(4, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(0)).expect("i8 zero"),
            )],
        ));
    }

    #[test]
    fn exact_division_selects_checked_contiguous_cast_root_bound() {
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
        let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
        let context = PropositionContext::from_value_types([
            (
                ValueId::new(1).expect("dividend"),
                ScalarType::Integer(i8_type),
            ),
            (
                ValueId::new(2).expect("divisor"),
                ScalarType::Integer(i8_type),
            ),
            (
                ValueId::new(3).expect("root"),
                ScalarType::Integer(i16_type),
            ),
            (
                ValueId::new(4).expect("middle"),
                ScalarType::Integer(i16_type),
            ),
            (
                ValueId::new(5).expect("wide root"),
                ScalarType::Integer(i32_type),
            ),
            (
                ValueId::new(6).expect("redirected root"),
                ScalarType::Integer(i32_type),
            ),
            (
                ValueId::new(7).expect("redirected bound"),
                ScalarType::Integer(i32_type),
            ),
            (
                ValueId::new(8).expect("third alias"),
                ScalarType::Integer(i32_type),
            ),
        ])
        .expect("cast values");
        let goal = CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: i8_type,
            left: value(1, i8_type),
            right: value(2, i8_type),
        };
        let cast = Proposition::Equal(
            value(2, i8_type),
            ScalarTerm::integer_exact_cast(i16_type, i8_type, value(3, i16_type))
                .expect("partial exact cast"),
        );
        let positive_bound = Proposition::LessOrEqual(
            ScalarTerm::integer(i16_type, IntegerValue::Signed(1)).expect("i16 one"),
            value(3, i16_type),
        );
        assert!(exact_division_has_prior_certificate(
            &context,
            &goal,
            std::slice::from_ref(&cast),
            std::slice::from_ref(&positive_bound),
        ));
        let negative_bound = Proposition::LessOrEqual(
            value(3, i16_type),
            ScalarTerm::integer(i16_type, IntegerValue::Signed(-2)).expect("i16 -2"),
        );
        assert!(exact_division_has_prior_certificate(
            &context,
            &goal,
            std::slice::from_ref(&cast),
            std::slice::from_ref(&negative_bound),
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            std::slice::from_ref(&cast),
            &[],
        ));
        let wide_bound = Proposition::LessOrEqual(
            ScalarTerm::integer(i32_type, IntegerValue::Signed(1)).expect("i32 one"),
            value(5, i32_type),
        );
        let first_cast = Proposition::Equal(
            value(4, i16_type),
            ScalarTerm::integer_exact_cast(i32_type, i16_type, value(5, i32_type))
                .expect("first partial cast"),
        );
        let second_cast = Proposition::Equal(
            value(2, i8_type),
            ScalarTerm::integer_exact_cast(i16_type, i8_type, value(4, i16_type))
                .expect("second partial cast"),
        );
        assert!(exact_division_has_prior_certificate(
            &context,
            &goal,
            &[first_cast.clone(), second_cast.clone()],
            std::slice::from_ref(&wide_bound),
        ));
        assert!(exact_division_has_prior_certificate(
            &context,
            &goal,
            &[first_cast.clone(), second_cast.clone()],
            &[Proposition::Equal(
                value(5, i32_type),
                ScalarTerm::integer(i32_type, IntegerValue::Signed(1)).expect("i32 one"),
            )],
        ));
        assert!(exact_division_has_prior_certificate(
            &context,
            &goal,
            &[first_cast.clone(), second_cast.clone()],
            &[Proposition::Equal(
                value(5, i32_type),
                ScalarTerm::integer(i32_type, IntegerValue::Signed(-2)).expect("i32 -2"),
            )],
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            &[first_cast.clone(), second_cast.clone()],
            &[Proposition::Equal(
                value(6, i32_type),
                ScalarTerm::integer(i32_type, IntegerValue::Signed(1)).expect("i32 one"),
            )],
        ));
        assert!(exact_division_has_prior_certificate(
            &context,
            &goal,
            &[first_cast.clone(), second_cast.clone()],
            &[Proposition::Equal(
                value(5, i32_type),
                ScalarTerm::integer(i32_type, IntegerValue::Signed(2)).expect("i32 two"),
            )],
        ));
        assert!(exact_division_has_prior_certificate(
            &context,
            &goal,
            &[first_cast.clone(), second_cast.clone()],
            &[Proposition::Equal(
                value(5, i32_type),
                ScalarTerm::integer(i32_type, IntegerValue::Signed(-3)).expect("i32 -3"),
            )],
        ));
        for weak in [0, -1] {
            assert!(!exact_division_has_prior_certificate(
                &context,
                &goal,
                &[first_cast.clone(), second_cast.clone()],
                &[Proposition::Equal(
                    value(5, i32_type),
                    ScalarTerm::integer(i32_type, IntegerValue::Signed(weak))
                        .expect("weak i32 literal"),
                )],
            ));
        }
        let root_alias = Proposition::Equal(value(5, i32_type), value(6, i32_type));
        assert!(exact_division_has_prior_certificate(
            &context,
            &goal,
            &[first_cast.clone(), second_cast.clone()],
            &[
                Proposition::LessOrEqual(
                    ScalarTerm::integer(i32_type, IntegerValue::Signed(1)).expect("i32 one"),
                    value(6, i32_type),
                ),
                root_alias.clone(),
            ],
        ));
        assert!(exact_division_has_prior_certificate(
            &context,
            &goal,
            &[first_cast.clone(), second_cast.clone()],
            &[
                Proposition::LessOrEqual(
                    ScalarTerm::integer(i32_type, IntegerValue::Signed(2)).expect("i32 two"),
                    value(6, i32_type),
                ),
                root_alias.clone(),
            ],
        ));
        assert!(exact_division_has_prior_certificate(
            &context,
            &goal,
            &[first_cast.clone(), second_cast.clone()],
            &[
                Proposition::LessOrEqual(
                    value(6, i32_type),
                    ScalarTerm::integer(i32_type, IntegerValue::Signed(-3)).expect("i32 -3"),
                ),
                root_alias.clone(),
            ],
        ));
        assert!(exact_division_has_prior_certificate(
            &context,
            &goal,
            &[first_cast.clone(), second_cast.clone()],
            &[
                Proposition::LessOrEqual(
                    value(6, i32_type),
                    ScalarTerm::integer(i32_type, IntegerValue::Signed(-2)).expect("i32 -2"),
                ),
                root_alias.clone(),
            ],
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            &[first_cast.clone(), second_cast.clone()],
            &[Proposition::LessOrEqual(
                ScalarTerm::integer(i32_type, IntegerValue::Signed(1)).expect("i32 one"),
                value(6, i32_type),
            )],
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            &[first_cast.clone(), second_cast.clone()],
            &[
                Proposition::LessOrEqual(
                    ScalarTerm::integer(i32_type, IntegerValue::Signed(1)).expect("i32 one"),
                    value(7, i32_type),
                ),
                root_alias.clone(),
            ],
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            &[first_cast.clone(), second_cast.clone()],
            &[
                Proposition::LessOrEqual(
                    value(6, i32_type),
                    ScalarTerm::integer(i32_type, IntegerValue::Signed(-1)).expect("i32 -1"),
                ),
                root_alias.clone(),
            ],
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            &[first_cast.clone(), second_cast.clone()],
            &[
                Proposition::LessOrEqual(
                    ScalarTerm::integer(i32_type, IntegerValue::Signed(0)).expect("i32 zero"),
                    value(6, i32_type),
                ),
                root_alias.clone(),
            ],
        ));
        assert!(exact_division_has_prior_certificate(
            &context,
            &goal,
            &[first_cast.clone(), second_cast.clone()],
            &[
                root_alias.clone(),
                Proposition::Equal(
                    value(6, i32_type),
                    ScalarTerm::integer(i32_type, IntegerValue::Signed(2)).expect("i32 two"),
                ),
            ],
        ));
        assert!(exact_division_has_prior_certificate(
            &context,
            &goal,
            &[first_cast.clone(), second_cast.clone()],
            &[
                root_alias.clone(),
                Proposition::Equal(
                    value(6, i32_type),
                    ScalarTerm::integer(i32_type, IntegerValue::Signed(-3)).expect("i32 -3"),
                ),
            ],
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            &[first_cast.clone(), second_cast.clone()],
            &[Proposition::Equal(
                value(6, i32_type),
                ScalarTerm::integer(i32_type, IntegerValue::Signed(2)).expect("i32 two"),
            )],
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            &[first_cast.clone(), second_cast.clone()],
            &[
                root_alias.clone(),
                Proposition::Equal(
                    value(7, i32_type),
                    ScalarTerm::integer(i32_type, IntegerValue::Signed(2)).expect("i32 two"),
                ),
            ],
        ));
        for weak in [0, -1] {
            assert!(!exact_division_has_prior_certificate(
                &context,
                &goal,
                &[first_cast.clone(), second_cast.clone()],
                &[
                    root_alias.clone(),
                    Proposition::Equal(
                        value(6, i32_type),
                        ScalarTerm::integer(i32_type, IntegerValue::Signed(weak))
                            .expect("weak i32 literal"),
                    ),
                ],
            ));
        }
        let middle_alias = Proposition::Equal(value(6, i32_type), value(7, i32_type));
        let two_alias_bound = Proposition::LessOrEqual(
            ScalarTerm::integer(i32_type, IntegerValue::Signed(1)).expect("i32 one"),
            value(7, i32_type),
        );
        assert!(exact_division_has_prior_certificate(
            &context,
            &goal,
            &[first_cast.clone(), second_cast.clone()],
            &[
                two_alias_bound.clone(),
                middle_alias.clone(),
                root_alias.clone(),
            ],
        ));
        assert!(exact_division_has_prior_certificate(
            &context,
            &goal,
            &[first_cast.clone(), second_cast.clone()],
            &[
                Proposition::LessOrEqual(
                    value(7, i32_type),
                    ScalarTerm::integer(i32_type, IntegerValue::Signed(-2)).expect("i32 -2"),
                ),
                middle_alias.clone(),
                root_alias.clone(),
            ],
        ));
        for rejected in [
            vec![two_alias_bound.clone(), root_alias.clone()],
            vec![
                two_alias_bound.clone(),
                Proposition::Equal(value(6, i32_type), value(8, i32_type)),
                root_alias.clone(),
            ],
            vec![
                Proposition::LessOrEqual(
                    ScalarTerm::integer(i32_type, IntegerValue::Signed(0)).expect("i32 zero"),
                    value(7, i32_type),
                ),
                middle_alias.clone(),
                root_alias.clone(),
            ],
            vec![
                Proposition::LessOrEqual(
                    ScalarTerm::integer(i32_type, IntegerValue::Signed(1)).expect("i32 one"),
                    value(8, i32_type),
                ),
                Proposition::Equal(value(7, i32_type), value(8, i32_type)),
                middle_alias,
                root_alias.clone(),
            ],
        ] {
            assert!(!exact_division_has_prior_certificate(
                &context,
                &goal,
                &[first_cast.clone(), second_cast.clone()],
                &rejected,
            ));
        }
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            &[second_cast.clone(), first_cast.clone()],
            std::slice::from_ref(&wide_bound),
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            &[first_cast, second_cast.clone(), second_cast],
            std::slice::from_ref(&wide_bound),
        ));
    }

    #[test]
    fn exact_division_selects_affine_root_literal_through_one_alias() {
        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let context = PropositionContext::from_value_types((1..=5).map(|id| {
            (
                ValueId::new(id).expect("value id"),
                ScalarType::Integer(signed),
            )
        }))
        .expect("five i8 values");
        let goal = CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: signed,
            left: value(1, signed),
            right: value(2, signed),
        };
        let root_alias = Proposition::Equal(value(3, signed), value(4, signed));
        let landed_alias = Proposition::Equal(
            value(4, signed),
            ScalarTerm::integer(signed, IntegerValue::Signed(0)).expect("i8 zero"),
        );
        let positive_definition = Proposition::Equal(
            value(2, signed),
            ScalarTerm::exact_integer_add(
                signed,
                value(3, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
            )
            .expect("exact add"),
        );
        assert!(exact_division_has_prior_certificate(
            &context,
            &goal,
            std::slice::from_ref(&positive_definition),
            &[root_alias.clone(), landed_alias.clone()],
        ));
        let negative_definition = Proposition::Equal(
            value(2, signed),
            ScalarTerm::exact_integer_subtract(
                signed,
                value(3, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(2)).expect("i8 two"),
            )
            .expect("exact subtract"),
        );
        assert!(exact_division_has_prior_certificate(
            &context,
            &goal,
            std::slice::from_ref(&negative_definition),
            &[root_alias.clone(), landed_alias.clone()],
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            std::slice::from_ref(&positive_definition),
            std::slice::from_ref(&landed_alias),
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            std::slice::from_ref(&positive_definition),
            &[
                root_alias.clone(),
                Proposition::Equal(
                    value(5, signed),
                    ScalarTerm::integer(signed, IntegerValue::Signed(0)).expect("i8 zero"),
                ),
            ],
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            &[positive_definition],
            &[
                root_alias,
                Proposition::Equal(value(4, signed), value(5, signed)),
                Proposition::Equal(
                    value(5, signed),
                    ScalarTerm::integer(signed, IntegerValue::Signed(0)).expect("i8 zero"),
                ),
            ],
        ));
    }

    #[test]
    fn exact_division_selects_affine_bound_through_target_alias() {
        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let context = PropositionContext::from_value_types((1..=5).map(|id| {
            (
                ValueId::new(id).expect("value id"),
                ScalarType::Integer(signed),
            )
        }))
        .expect("five i8 values");
        let goal = CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: signed,
            left: value(1, signed),
            right: value(2, signed),
        };
        let target_alias = Proposition::Equal(value(4, signed), value(2, signed));
        let zero = ScalarTerm::integer(signed, IntegerValue::Signed(0)).expect("i8 zero");

        let positive_root_bound = Proposition::LessOrEqual(zero.clone(), value(3, signed));
        let positive_definition = Proposition::Equal(
            value(4, signed),
            ScalarTerm::exact_integer_add(
                signed,
                value(3, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
            )
            .expect("exact add"),
        );
        assert!(exact_division_has_prior_certificate(
            &context,
            &goal,
            std::slice::from_ref(&positive_definition),
            &[positive_root_bound.clone(), target_alias.clone()],
        ));

        let negative_root_bound = Proposition::LessOrEqual(value(3, signed), zero);
        let negative_definition = Proposition::Equal(
            value(4, signed),
            ScalarTerm::exact_integer_subtract(
                signed,
                value(3, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(2)).expect("i8 two"),
            )
            .expect("exact subtract"),
        );
        assert!(exact_division_has_prior_certificate(
            &context,
            &goal,
            std::slice::from_ref(&negative_definition),
            &[negative_root_bound, target_alias],
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            std::slice::from_ref(&positive_definition),
            std::slice::from_ref(&positive_root_bound),
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            &[positive_definition],
            &[
                positive_root_bound,
                Proposition::Equal(value(4, signed), value(5, signed)),
            ],
        ));
    }

    #[test]
    fn exact_division_selects_affine_bound_through_two_target_aliases() {
        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let context = PropositionContext::from_value_types((1..=6).map(|id| {
            (
                ValueId::new(id).expect("value id"),
                ScalarType::Integer(signed),
            )
        }))
        .expect("six i8 values");
        let goal = CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: signed,
            left: value(1, signed),
            right: value(2, signed),
        };
        let outer_alias = Proposition::Equal(value(2, signed), value(4, signed));
        let inner_alias = Proposition::Equal(value(4, signed), value(5, signed));
        let positive_root_bound = Proposition::LessOrEqual(
            ScalarTerm::integer(signed, IntegerValue::Signed(0)).expect("i8 zero"),
            value(3, signed),
        );
        let positive_definition = Proposition::Equal(
            value(5, signed),
            ScalarTerm::exact_integer_add(
                signed,
                value(3, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
            )
            .expect("exact add"),
        );
        assert!(exact_division_has_prior_certificate(
            &context,
            &goal,
            std::slice::from_ref(&positive_definition),
            &[
                positive_root_bound.clone(),
                outer_alias.clone(),
                inner_alias.clone(),
            ],
        ));
        let negative_root_bound = Proposition::LessOrEqual(
            value(3, signed),
            ScalarTerm::integer(signed, IntegerValue::Signed(0)).expect("i8 zero"),
        );
        let negative_definition = Proposition::Equal(
            value(5, signed),
            ScalarTerm::exact_integer_subtract(
                signed,
                value(3, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(2)).expect("i8 two"),
            )
            .expect("exact subtract"),
        );
        assert!(exact_division_has_prior_certificate(
            &context,
            &goal,
            std::slice::from_ref(&negative_definition),
            &[
                negative_root_bound,
                outer_alias.clone(),
                inner_alias.clone(),
            ],
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            std::slice::from_ref(&positive_definition),
            &[positive_root_bound.clone(), outer_alias.clone()],
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            std::slice::from_ref(&positive_definition),
            &[
                positive_root_bound.clone(),
                outer_alias.clone(),
                Proposition::Equal(value(4, signed), value(6, signed)),
            ],
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            &[Proposition::Equal(
                value(6, signed),
                ScalarTerm::exact_integer_add(
                    signed,
                    value(3, signed),
                    ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
                )
                .expect("exact add"),
            )],
            &[
                positive_root_bound,
                outer_alias,
                inner_alias,
                Proposition::Equal(value(5, signed), value(6, signed)),
            ],
        ));
    }

    #[test]
    fn exact_division_selects_alias_substituted_affine_root_bound() {
        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let context = PropositionContext::from_value_types((1..=5).map(|id| {
            (
                ValueId::new(id).expect("value id"),
                ScalarType::Integer(signed),
            )
        }))
        .expect("five i8 values");
        let goal = CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: signed,
            left: value(1, signed),
            right: value(2, signed),
        };
        let alias_equality = Proposition::Equal(value(3, signed), value(4, signed));
        let alias_bound = Proposition::LessOrEqual(
            ScalarTerm::integer(signed, IntegerValue::Signed(0)).expect("i8 zero"),
            value(4, signed),
        );
        let definition = Proposition::Equal(
            value(2, signed),
            ScalarTerm::exact_integer_add(
                signed,
                value(3, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
            )
            .expect("exact add"),
        );
        assert!(exact_division_has_prior_certificate(
            &context,
            &goal,
            std::slice::from_ref(&definition),
            &[alias_equality.clone(), alias_bound.clone()],
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            std::slice::from_ref(&definition),
            std::slice::from_ref(&alias_bound),
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            std::slice::from_ref(&definition),
            std::slice::from_ref(&alias_equality),
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            &[definition],
            &[
                Proposition::Equal(value(5, signed), value(4, signed)),
                alias_bound,
            ],
        ));
    }

    #[test]
    fn exact_division_selects_bound_through_two_affine_root_aliases() {
        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let context = PropositionContext::from_value_types((1..=7).map(|id| {
            (
                ValueId::new(id).expect("value id"),
                ScalarType::Integer(signed),
            )
        }))
        .expect("seven i8 values");
        let goal = CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: signed,
            left: value(1, signed),
            right: value(2, signed),
        };
        let root_to_middle_alias = Proposition::Equal(value(3, signed), value(4, signed));
        let middle_to_bound_alias = Proposition::Equal(value(4, signed), value(5, signed));
        let lower_bound = Proposition::LessOrEqual(
            ScalarTerm::integer(signed, IntegerValue::Signed(0)).expect("i8 zero"),
            value(5, signed),
        );
        let definition = Proposition::Equal(
            value(2, signed),
            ScalarTerm::exact_integer_add(
                signed,
                value(3, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
            )
            .expect("exact add"),
        );
        assert!(exact_division_has_prior_certificate(
            &context,
            &goal,
            std::slice::from_ref(&definition),
            &[
                root_to_middle_alias.clone(),
                middle_to_bound_alias.clone(),
                lower_bound.clone(),
            ],
        ));
        assert!(exact_division_has_prior_certificate(
            &context,
            &goal,
            std::slice::from_ref(&definition),
            &[
                root_to_middle_alias.clone(),
                middle_to_bound_alias.clone(),
                Proposition::LessOrEqual(
                    value(5, signed),
                    ScalarTerm::integer(signed, IntegerValue::Signed(-3)).expect("i8 -3"),
                ),
            ],
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            std::slice::from_ref(&definition),
            &[root_to_middle_alias.clone(), lower_bound.clone()],
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            std::slice::from_ref(&definition),
            &[
                root_to_middle_alias.clone(),
                Proposition::Equal(value(4, signed), value(6, signed)),
                lower_bound,
            ],
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            &[definition],
            &[
                root_to_middle_alias,
                middle_to_bound_alias,
                Proposition::Equal(value(5, signed), value(6, signed)),
                Proposition::LessOrEqual(
                    ScalarTerm::integer(signed, IntegerValue::Signed(0)).expect("i8 zero"),
                    value(6, signed),
                ),
            ],
        ));
    }

    #[test]
    fn exact_division_selects_transitive_bound_on_affine_root_alias() {
        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let context = PropositionContext::from_value_types((1..=6).map(|id| {
            (
                ValueId::new(id).expect("value id"),
                ScalarType::Integer(signed),
            )
        }))
        .expect("six i8 values");
        let goal = CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: signed,
            left: value(1, signed),
            right: value(2, signed),
        };
        let root_alias = Proposition::Equal(value(3, signed), value(4, signed));
        let lower_to_middle = Proposition::LessOrEqual(
            ScalarTerm::integer(signed, IntegerValue::Signed(0)).expect("i8 zero"),
            value(5, signed),
        );
        let middle_to_alias = Proposition::LessOrEqual(value(5, signed), value(4, signed));
        let definition = Proposition::Equal(
            value(2, signed),
            ScalarTerm::exact_integer_add(
                signed,
                value(3, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
            )
            .expect("exact add"),
        );
        assert!(exact_division_has_prior_certificate(
            &context,
            &goal,
            std::slice::from_ref(&definition),
            &[
                root_alias.clone(),
                lower_to_middle.clone(),
                middle_to_alias.clone(),
            ],
        ));

        let alias_to_middle = Proposition::LessOrEqual(value(4, signed), value(5, signed));
        let middle_to_ceiling = Proposition::LessOrEqual(
            value(5, signed),
            ScalarTerm::integer(signed, IntegerValue::Signed(-3)).expect("i8 -3"),
        );
        assert!(exact_division_has_prior_certificate(
            &context,
            &goal,
            std::slice::from_ref(&definition),
            &[root_alias.clone(), alias_to_middle, middle_to_ceiling,],
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            std::slice::from_ref(&definition),
            &[lower_to_middle.clone(), middle_to_alias.clone()],
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            std::slice::from_ref(&definition),
            &[
                root_alias.clone(),
                lower_to_middle.clone(),
                Proposition::LessOrEqual(value(6, signed), value(4, signed)),
            ],
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            &[definition],
            &[
                Proposition::Equal(value(3, signed), value(6, signed)),
                lower_to_middle,
                middle_to_alias,
            ],
        ));
    }

    #[test]
    fn exact_division_selects_transitively_reconstructed_affine_root_bound() {
        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let context = PropositionContext::from_value_types((1..=5).map(|id| {
            (
                ValueId::new(id).expect("value id"),
                ScalarType::Integer(signed),
            )
        }))
        .expect("five i8 values");
        let goal = CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: signed,
            left: value(1, signed),
            right: value(2, signed),
        };
        let lower_to_middle = Proposition::LessOrEqual(
            ScalarTerm::integer(signed, IntegerValue::Signed(0)).expect("i8 zero"),
            value(4, signed),
        );
        let middle_to_root = Proposition::LessOrEqual(value(4, signed), value(3, signed));
        let definition = Proposition::Equal(
            value(2, signed),
            ScalarTerm::exact_integer_add(
                signed,
                value(3, signed),
                ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
            )
            .expect("exact add"),
        );
        assert!(exact_division_has_prior_certificate(
            &context,
            &goal,
            std::slice::from_ref(&definition),
            &[lower_to_middle.clone(), middle_to_root.clone()],
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            std::slice::from_ref(&definition),
            std::slice::from_ref(&lower_to_middle),
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            std::slice::from_ref(&definition),
            std::slice::from_ref(&middle_to_root),
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            &[definition],
            &[
                lower_to_middle,
                Proposition::LessOrEqual(value(5, signed), value(3, signed)),
            ],
        ));
    }

    #[test]
    fn exact_division_selects_two_definition_affine_safe_divisor() {
        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let context = PropositionContext::from_value_types((1..=4).map(|id| {
            (
                ValueId::new(id).expect("value id"),
                ScalarType::Integer(signed),
            )
        }))
        .expect("four i8 values");
        let goal = CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: signed,
            left: value(1, signed),
            right: value(2, signed),
        };
        let root_bound = Proposition::LessOrEqual(
            ScalarTerm::integer(signed, IntegerValue::Signed(-1)).expect("i8 -1"),
            value(3, signed),
        );
        let definitions = [
            Proposition::Equal(
                value(4, signed),
                ScalarTerm::exact_integer_add(
                    signed,
                    value(3, signed),
                    ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
                )
                .expect("first exact add"),
            ),
            Proposition::Equal(
                value(2, signed),
                ScalarTerm::exact_integer_add(
                    signed,
                    value(4, signed),
                    ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
                )
                .expect("second exact add"),
            ),
        ];
        assert!(exact_division_has_prior_certificate(
            &context,
            &goal,
            &definitions,
            std::slice::from_ref(&root_bound),
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            &definitions[..1],
            std::slice::from_ref(&root_bound),
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &goal,
            &[definitions[1].clone(), definitions[0].clone()],
            &[root_bound],
        ));
    }

    #[test]
    fn exact_division_selects_three_and_four_definition_affine_safe_divisors() {
        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let context = PropositionContext::from_value_types((1..=6).map(|id| {
            (
                ValueId::new(id).expect("value id"),
                ScalarType::Integer(signed),
            )
        }))
        .expect("six i8 values");
        let three_step_goal = CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: signed,
            left: value(1, signed),
            right: value(6, signed),
        };
        let four_step_goal = CanonicalScalarGoal::ExactDivisionDefined {
            integer_type: signed,
            left: value(1, signed),
            right: value(2, signed),
        };
        let three_step_root_bound = Proposition::LessOrEqual(
            ScalarTerm::integer(signed, IntegerValue::Signed(-2)).expect("i8 -2"),
            value(3, signed),
        );
        let four_step_root_bound = Proposition::LessOrEqual(
            ScalarTerm::integer(signed, IntegerValue::Signed(-3)).expect("i8 -3"),
            value(3, signed),
        );
        let definitions = [
            Proposition::Equal(
                value(4, signed),
                ScalarTerm::exact_integer_add(
                    signed,
                    value(3, signed),
                    ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
                )
                .expect("first exact add"),
            ),
            Proposition::Equal(
                value(5, signed),
                ScalarTerm::exact_integer_add(
                    signed,
                    value(4, signed),
                    ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
                )
                .expect("second exact add"),
            ),
            Proposition::Equal(
                value(6, signed),
                ScalarTerm::exact_integer_add(
                    signed,
                    value(5, signed),
                    ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
                )
                .expect("third exact add"),
            ),
            Proposition::Equal(
                value(2, signed),
                ScalarTerm::exact_integer_add(
                    signed,
                    value(6, signed),
                    ScalarTerm::integer(signed, IntegerValue::Signed(1)).expect("i8 one"),
                )
                .expect("fourth exact add"),
            ),
        ];
        assert!(exact_division_has_prior_certificate(
            &context,
            &three_step_goal,
            &definitions,
            std::slice::from_ref(&three_step_root_bound),
        ));
        assert!(exact_division_has_prior_certificate(
            &context,
            &four_step_goal,
            &definitions,
            std::slice::from_ref(&four_step_root_bound),
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &four_step_goal,
            &definitions[..3],
            std::slice::from_ref(&four_step_root_bound),
        ));
        assert!(!exact_division_has_prior_certificate(
            &context,
            &four_step_goal,
            &[
                definitions[3].clone(),
                definitions[2].clone(),
                definitions[1].clone(),
                definitions[0].clone(),
            ],
            &[four_step_root_bound],
        ));
    }
}
