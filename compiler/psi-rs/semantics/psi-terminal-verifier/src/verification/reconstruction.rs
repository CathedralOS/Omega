//! Exact executable-site obligations and all-path fact reconstruction.

use std::collections::{BTreeMap, BTreeSet};

use psi_core::{Proposition, ScalarTerm, ValueId};
use psi_proof_kernel::{Obligation, ObligationClass};
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
                ) || exact_division_has_closed_prior_certificate(
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

/// The complete closed landed-literal exact divide/remainder families. A
/// directly landed unsigned nonzero divisor or signed divisor other than zero
/// and -1 makes definedness independent of the dividend. A signed -1 divisor
/// is also complete when the dividend is independently landed above the
/// carrier minimum or its exact canonical minimum-plus-one bound is retained
/// as a prior proposition. A retained stronger literal lower bound is complete
/// through one closed transitivity step. Their canonical proofs need only
/// prior citations and closed integer order; no value-root custody or
/// operation-definition authority participates.
fn exact_division_has_closed_prior_certificate(
    goal: &CanonicalScalarGoal,
    semantic_axioms: &[Proposition],
    requirements: &[Proposition],
) -> bool {
    let CanonicalScalarGoal::ExactDivisionDefined {
        integer_type,
        left,
        right,
    } = goal
    else {
        return false;
    };
    if retained_exact_division_safe_divisor_bound(goal, semantic_axioms, requirements) {
        return true;
    }
    let Some(value) = super::known_integer_term_value(*integer_type, right, semantic_axioms) else {
        return false;
    };
    match (integer_type.sign(), value) {
        (psi_core::IntegerSign::Unsigned, psi_core::IntegerValue::Unsigned(value)) => value != 0,
        (psi_core::IntegerSign::Signed, psi_core::IntegerValue::Signed(value)) if value == -1 => {
            super::known_integer_term_value(*integer_type, left, semantic_axioms)
                .is_some_and(|left| left != integer_type.minimum_value())
                || retained_exact_division_exception_bound(goal, semantic_axioms, requirements)
        }
        (psi_core::IntegerSign::Signed, psi_core::IntegerValue::Signed(value)) => value != 0,
        _ => false,
    }
}

fn retained_exact_division_safe_divisor_bound(
    goal: &CanonicalScalarGoal,
    semantic_axioms: &[Proposition],
    requirements: &[Proposition],
) -> bool {
    let Ok(Some(proposition)) = goal.kernel_proposition() else {
        return false;
    };
    let retained = |candidate: &Proposition| {
        requirements
            .iter()
            .chain(semantic_axioms)
            .any(|fact| fact == candidate || closed_transitive_integer_bound(candidate, fact))
    };
    match &proposition {
        Proposition::LessOrEqual(_, _) => retained(&proposition),
        Proposition::Disjunction(disjuncts) => {
            disjuncts
                .first()
                .is_some_and(|candidate| retained(candidate))
                || disjuncts
                    .get(1)
                    .is_some_and(|candidate| retained(candidate))
        }
        Proposition::Conjunction(conjuncts) => {
            !conjuncts.is_empty() && conjuncts.iter().all(retained)
        }
        _ => false,
    }
}

fn retained_exact_division_exception_bound(
    goal: &CanonicalScalarGoal,
    semantic_axioms: &[Proposition],
    requirements: &[Proposition],
) -> bool {
    let Ok(Some(proposition)) = goal.kernel_proposition() else {
        return false;
    };
    let bound = match &proposition {
        Proposition::Disjunction(disjuncts) => {
            let Some(Proposition::Conjunction(exception)) = disjuncts.get(2) else {
                return false;
            };
            let Some(bound) = exception.get(1) else {
                return false;
            };
            bound
        }
        Proposition::Conjunction(conjuncts) => {
            let Some(bound) = conjuncts.get(1) else {
                return false;
            };
            bound
        }
        _ => return false,
    };
    requirements
        .iter()
        .chain(semantic_axioms)
        .any(|fact| fact == bound || closed_transitive_integer_bound(bound, fact))
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
}
