use std::collections::{BTreeMap, BTreeSet};

use psi_core::{
    IntegerSign, IntegerValue, ObligationId, Proposition, ScalarTerm, ScalarType, ValueId,
};
use psi_proof_kernel::{
    AcceptedFact, AdmissionProfile, EvidenceError, EvidenceRoute, Obligation, ObligationClass,
    verify_obligation,
};
use psi_terminal::{OperationKind, TerminalMachine, TerminalModule, Terminator};

use crate::{ModuleError, ValidatedTerminalModule, validate_module};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObligationEvidence {
    pub obligation: ObligationId,
    pub route: EvidenceRoute,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProofBundle {
    pub evidence: Vec<ObligationEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconstructedOperationObligation {
    pub obligation: Obligation,
    pub semantic_axioms: Vec<Proposition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReconstructedMachineSemantics {
    operation_obligations: Vec<ReconstructedOperationObligation>,
    exit_axioms: Vec<Proposition>,
}

#[derive(Debug)]
pub struct VerifiedTerminalModule<'module> {
    validated: ValidatedTerminalModule<'module>,
    accepted_facts: Vec<AcceptedFact>,
}

impl<'module> VerifiedTerminalModule<'module> {
    pub const fn module(&self) -> &'module TerminalModule {
        self.validated.module()
    }

    pub fn accepted_facts(&self) -> &[AcceptedFact] {
        &self.accepted_facts
    }
}

pub fn verify_module<'module>(
    module: &'module TerminalModule,
    proof_bundle: &ProofBundle,
    profile: &AdmissionProfile,
) -> Result<VerifiedTerminalModule<'module>, VerificationError> {
    let validated = validate_module(module).map_err(VerificationError::Module)?;
    let mut evidence = BTreeMap::new();
    for entry in &proof_bundle.evidence {
        if evidence
            .insert(entry.obligation, entry.route.clone())
            .is_some()
        {
            return Err(VerificationError::DuplicateEvidence(entry.obligation));
        }
    }

    let mut accepted_facts = Vec::new();
    for machine in &module.machines {
        let context = validated
            .value_context(machine)
            .map_err(VerificationError::Module)?;
        let semantics = reconstruct_machine_semantics(machine);
        for site in &semantics.operation_obligations {
            let route = evidence
                .remove(&site.obligation.id)
                .ok_or(VerificationError::MissingEvidence(site.obligation.id))?;
            let accepted = verify_obligation(
                &context,
                &site.obligation,
                &machine.contract.requires,
                &site.semantic_axioms,
                route,
                profile,
            )
            .map_err(|error| VerificationError::RejectedEvidence {
                obligation: site.obligation.id,
                error,
            })?;
            accepted_facts.push(accepted);
        }
        for clause in &machine.contract.ensures {
            let route = evidence
                .remove(&clause.obligation)
                .ok_or(VerificationError::MissingEvidence(clause.obligation))?;
            let accepted = verify_obligation(
                &context,
                &Obligation {
                    id: clause.obligation,
                    proposition: clause.proposition.clone(),
                    class: ObligationClass::Derivable,
                },
                &machine.contract.requires,
                &semantics.exit_axioms,
                route,
                profile,
            )
            .map_err(|error| VerificationError::RejectedEvidence {
                obligation: clause.obligation,
                error,
            })?;
            accepted_facts.push(accepted);
        }
    }

    if let Some(obligation) = evidence.keys().next().copied() {
        return Err(VerificationError::UnknownEvidence(obligation));
    }
    Ok(VerifiedTerminalModule {
        validated,
        accepted_facts,
    })
}

/// Reconstruct proof obligations owned by executable operation sites. This is
/// exposed so a producer can build a certificate against exactly the same
/// source-independent obligations and axiom ordering that the verifier will
/// later replay. The module is validated before any reconstruction occurs.
pub fn reconstruct_operation_obligations(
    module: &TerminalModule,
) -> Result<Vec<ReconstructedOperationObligation>, ModuleError> {
    validate_module(module)?;
    Ok(module
        .machines
        .iter()
        .flat_map(|machine| reconstruct_machine_semantics(machine).operation_obligations)
        .collect())
}

/// Reconstruct facts at each executable obligation site and facts established
/// on every return path. A true conditional edge establishes the predicate
/// computed by its condition operation; edge bindings rewrite those facts to
/// successor parameters. Merge and return facts remain intersection-only.
fn reconstruct_machine_semantics(machine: &TerminalMachine) -> ReconstructedMachineSemantics {
    let reconstruct_path_facts = machine.blocks.iter().any(|block| {
        block.operations.iter().any(|operation| {
            matches!(
                operation.kind,
                OperationKind::IntegerExactCast { .. }
                    | OperationKind::ExactIntegerShiftLeft { .. }
                    | OperationKind::ExactIntegerShiftRight { .. }
            )
        })
    });
    let value_types = machine
        .parameters
        .iter()
        .chain(std::iter::once(&machine.result))
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| block.parameters.iter()),
        )
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| block.operations.iter().map(|operation| &operation.result)),
        )
        .map(|declaration| (declaration.id, declaration.scalar_type))
        .collect::<BTreeMap<_, _>>();
    let blocks = machine
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    let value_term = |id: ValueId| {
        ScalarTerm::value(
            id,
            *value_types
                .get(&id)
                .expect("validated module contains every referenced value"),
        )
    };

    let mut base_axioms = machine
        .content_identity_reshuffles
        .iter()
        .flat_map(|reshuffle| reshuffle.inferred_propositions())
        .collect::<Vec<_>>();
    base_axioms.extend(
        machine
            .content_partition_compositions
            .iter()
            .map(|composition| composition.inferred_proposition()),
    );
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
            Terminator::Return { .. } | Terminator::Crash { .. } => Vec::new(),
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
            match operation.kind {
                OperationKind::IntegerConstant { value } => {
                    let ScalarType::Integer(integer_type) = operation.result.scalar_type else {
                        unreachable!("validator requires integer constant result type");
                    };
                    let literal = ScalarTerm::integer(integer_type, value)
                        .expect("validator requires representable integer constant");
                    axioms.push(Proposition::Equal(value_term(operation.result.id), literal));
                }
                OperationKind::BooleanConstant { value } => {
                    axioms.push(Proposition::Equal(
                        value_term(operation.result.id),
                        ScalarTerm::boolean(value),
                    ));
                }
                OperationKind::BooleanNot { operand } => {
                    let negated = ScalarTerm::boolean_not(value_term(operand))
                        .expect("validator requires a Boolean logical-not operand");
                    axioms.push(Proposition::Equal(value_term(operation.result.id), negated));
                }
                OperationKind::BooleanEqual { left, right } => {
                    let equal = ScalarTerm::boolean_equal(value_term(left), value_term(right))
                        .expect("validator requires Boolean equality operands");
                    axioms.push(Proposition::Equal(value_term(operation.result.id), equal));
                }
                OperationKind::IntegerEqual { left, right } => {
                    let ScalarType::Integer(integer_type) = value_term(left).scalar_type() else {
                        unreachable!("validator requires integer equality operands")
                    };
                    let equal = ScalarTerm::integer_equal(
                        integer_type,
                        value_term(left),
                        value_term(right),
                    )
                    .expect("validator requires exact integer equality operand types");
                    axioms.push(Proposition::Equal(value_term(operation.result.id), equal));
                }
                OperationKind::IntegerLessThan { left, right } => {
                    let ScalarType::Integer(integer_type) = value_term(left).scalar_type() else {
                        unreachable!("validator requires integer ordering operands")
                    };
                    let ordered = ScalarTerm::integer_less_than(
                        integer_type,
                        value_term(left),
                        value_term(right),
                    )
                    .expect("validator requires exact integer ordering operand types");
                    axioms.push(Proposition::Equal(value_term(operation.result.id), ordered));
                }
                OperationKind::IntegerLessOrEqual { left, right } => {
                    let ScalarType::Integer(integer_type) = value_term(left).scalar_type() else {
                        unreachable!("validator requires integer ordering operands")
                    };
                    let ordered = ScalarTerm::integer_less_or_equal(
                        integer_type,
                        value_term(left),
                        value_term(right),
                    )
                    .expect("validator requires exact integer ordering operand types");
                    axioms.push(Proposition::Equal(value_term(operation.result.id), ordered));
                }
                OperationKind::IntegerBitwiseNot { operand } => {
                    let ScalarType::Integer(integer_type) = operation.result.scalar_type else {
                        unreachable!("validator requires bitwise-not integer result type")
                    };
                    let result = ScalarTerm::integer_bitwise_not(integer_type, value_term(operand))
                        .expect("validator requires exact bitwise-not operand type");
                    axioms.push(Proposition::Equal(value_term(operation.result.id), result));
                }
                OperationKind::IntegerWiden { operand } => {
                    let ScalarType::Integer(source_type) = value_term(operand).scalar_type() else {
                        unreachable!("validator requires an integer widening operand")
                    };
                    let ScalarType::Integer(target_type) = operation.result.scalar_type else {
                        unreachable!("validator requires an integer widening result")
                    };
                    let result =
                        ScalarTerm::integer_widen(source_type, target_type, value_term(operand))
                            .expect("validator requires a universally total integer widening");
                    axioms.push(Proposition::Equal(value_term(operation.result.id), result));
                }
                OperationKind::IntegerExactCast {
                    operand,
                    obligation,
                } => {
                    let ScalarType::Integer(source_type) = value_term(operand).scalar_type() else {
                        unreachable!("validator requires an integer exact-cast operand")
                    };
                    let ScalarType::Integer(target_type) = operation.result.scalar_type else {
                        unreachable!("validator requires an integer exact-cast result")
                    };
                    operation_obligations.push(ReconstructedOperationObligation {
                        obligation: Obligation {
                            id: obligation,
                            proposition: exact_integer_cast_obligation(
                                source_type,
                                target_type,
                                value_term(operand),
                            ),
                            class: ObligationClass::Derivable,
                        },
                        semantic_axioms: axioms.clone(),
                    });
                    let result = ScalarTerm::integer_exact_cast(
                        source_type,
                        target_type,
                        value_term(operand),
                    )
                    .expect("validator requires a fixed-carrier exact integer cast");
                    axioms.push(Proposition::Equal(value_term(operation.result.id), result));
                }
                OperationKind::IntegerBitwiseAnd { left, right }
                | OperationKind::IntegerBitwiseOr { left, right }
                | OperationKind::IntegerBitwiseXor { left, right } => {
                    let ScalarType::Integer(integer_type) = operation.result.scalar_type else {
                        unreachable!("validator requires bitwise integer result type")
                    };
                    let result = match operation.kind {
                        OperationKind::IntegerBitwiseAnd { .. } => ScalarTerm::integer_bitwise_and(
                            integer_type,
                            value_term(left),
                            value_term(right),
                        ),
                        OperationKind::IntegerBitwiseOr { .. } => ScalarTerm::integer_bitwise_or(
                            integer_type,
                            value_term(left),
                            value_term(right),
                        ),
                        OperationKind::IntegerBitwiseXor { .. } => ScalarTerm::integer_bitwise_xor(
                            integer_type,
                            value_term(left),
                            value_term(right),
                        ),
                        _ => unreachable!(),
                    }
                    .expect("validator requires exact bitwise operand types");
                    axioms.push(Proposition::Equal(value_term(operation.result.id), result));
                }
                OperationKind::WrappingIntegerShiftLeft { value, count }
                | OperationKind::WrappingIntegerShiftRight { value, count } => {
                    let ScalarType::Integer(value_type) = operation.result.scalar_type else {
                        unreachable!("validator requires wrapping-shift integer result type")
                    };
                    let ScalarType::Integer(count_type) = value_term(count).scalar_type() else {
                        unreachable!("validator requires wrapping-shift integer count type")
                    };
                    let result = match operation.kind {
                        OperationKind::WrappingIntegerShiftLeft { .. } => {
                            ScalarTerm::wrapping_integer_shift_left(
                                value_type,
                                count_type,
                                value_term(value),
                                value_term(count),
                            )
                        }
                        OperationKind::WrappingIntegerShiftRight { .. } => {
                            ScalarTerm::wrapping_integer_shift_right(
                                value_type,
                                count_type,
                                value_term(value),
                                value_term(count),
                            )
                        }
                        _ => unreachable!(),
                    }
                    .expect("validator requires exact wrapping-shift operand types");
                    axioms.push(Proposition::Equal(value_term(operation.result.id), result));
                }
                OperationKind::ExactIntegerShiftRight {
                    value,
                    count,
                    obligation,
                } => {
                    let ScalarType::Integer(value_type) = operation.result.scalar_type else {
                        unreachable!("validator requires exact-shift integer result type")
                    };
                    let ScalarType::Integer(count_type) = value_term(count).scalar_type() else {
                        unreachable!("validator requires exact-shift integer count type")
                    };
                    operation_obligations.push(ReconstructedOperationObligation {
                        obligation: Obligation {
                            id: obligation,
                            proposition: exact_integer_shift_obligation(
                                value_type,
                                count_type,
                                value_term(count),
                            ),
                            class: ObligationClass::Derivable,
                        },
                        semantic_axioms: axioms.clone(),
                    });
                    let result = ScalarTerm::exact_integer_shift_right(
                        value_type,
                        count_type,
                        value_term(value),
                        value_term(count),
                    )
                    .expect("validator requires exact exact-shift operand types");
                    axioms.push(Proposition::Equal(value_term(operation.result.id), result));
                }
                OperationKind::ExactIntegerShiftLeft {
                    value,
                    count,
                    obligation,
                } => {
                    let ScalarType::Integer(value_type) = operation.result.scalar_type else {
                        unreachable!("validator requires exact-shift integer result type")
                    };
                    let ScalarType::Integer(count_type) = value_term(count).scalar_type() else {
                        unreachable!("validator requires exact-shift integer count type")
                    };
                    operation_obligations.push(ReconstructedOperationObligation {
                        obligation: Obligation {
                            id: obligation,
                            proposition: exact_integer_shift_left_obligation(
                                value_type,
                                count_type,
                                value_term(value),
                                value_term(count),
                                &axioms,
                            ),
                            class: ObligationClass::Derivable,
                        },
                        semantic_axioms: axioms.clone(),
                    });
                    let result = ScalarTerm::exact_integer_shift_left(
                        value_type,
                        count_type,
                        value_term(value),
                        value_term(count),
                    )
                    .expect("validator requires exact exact-shift operand types");
                    axioms.push(Proposition::Equal(value_term(operation.result.id), result));
                }
                OperationKind::WrappingIntegerAdd { left, right } => {
                    let ScalarType::Integer(integer_type) = operation.result.scalar_type else {
                        unreachable!("validator requires wrapping-add integer result type")
                    };
                    let sum = ScalarTerm::wrapping_integer_add(
                        integer_type,
                        value_term(left),
                        value_term(right),
                    )
                    .expect("validator requires exact wrapping-add operand types");
                    axioms.push(Proposition::Equal(value_term(operation.result.id), sum));
                }
                OperationKind::SaturatingIntegerAdd { left, right } => {
                    let ScalarType::Integer(integer_type) = operation.result.scalar_type else {
                        unreachable!("validator requires saturating-add integer result type")
                    };
                    let sum = ScalarTerm::saturating_integer_add(
                        integer_type,
                        value_term(left),
                        value_term(right),
                    )
                    .expect("validator requires exact saturating-add operand types");
                    axioms.push(Proposition::Equal(value_term(operation.result.id), sum));
                }
                OperationKind::WrappingIntegerSubtract { left, right } => {
                    let ScalarType::Integer(integer_type) = operation.result.scalar_type else {
                        unreachable!("validator requires wrapping-subtract integer result type")
                    };
                    let difference = ScalarTerm::wrapping_integer_subtract(
                        integer_type,
                        value_term(left),
                        value_term(right),
                    )
                    .expect("validator requires exact wrapping-subtract operand types");
                    axioms.push(Proposition::Equal(
                        value_term(operation.result.id),
                        difference,
                    ));
                }
                OperationKind::SaturatingIntegerSubtract { left, right } => {
                    let ScalarType::Integer(integer_type) = operation.result.scalar_type else {
                        unreachable!("validator requires saturating-subtract integer result type")
                    };
                    let difference = ScalarTerm::saturating_integer_subtract(
                        integer_type,
                        value_term(left),
                        value_term(right),
                    )
                    .expect("validator requires exact saturating-subtract operand types");
                    axioms.push(Proposition::Equal(
                        value_term(operation.result.id),
                        difference,
                    ));
                }
                OperationKind::WrappingIntegerMultiply { left, right } => {
                    let ScalarType::Integer(integer_type) = operation.result.scalar_type else {
                        unreachable!("validator requires wrapping-multiply integer result type")
                    };
                    let product = ScalarTerm::wrapping_integer_multiply(
                        integer_type,
                        value_term(left),
                        value_term(right),
                    )
                    .expect("validator requires exact wrapping-multiply operand types");
                    axioms.push(Proposition::Equal(value_term(operation.result.id), product));
                }
                OperationKind::SaturatingIntegerMultiply { left, right } => {
                    let ScalarType::Integer(integer_type) = operation.result.scalar_type else {
                        unreachable!("validator requires saturating-multiply integer result type")
                    };
                    let product = ScalarTerm::saturating_integer_multiply(
                        integer_type,
                        value_term(left),
                        value_term(right),
                    )
                    .expect("validator requires exact saturating-multiply operand types");
                    axioms.push(Proposition::Equal(value_term(operation.result.id), product));
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
            Terminator::Return { value, .. } => {
                axioms.push(Proposition::Equal(
                    value_term(machine.result.id),
                    value_term(*value),
                ));
                exits.push(axioms);
            }
            // A crash establishes no normal-return guarantee. Its explicit
            // frontier record is validated structurally before proof replay.
            Terminator::Crash { .. } => {}
        }
    }
    let mut exits = exits.into_iter();
    let Some(mut guaranteed) = exits.next() else {
        return ReconstructedMachineSemantics {
            operation_obligations,
            exit_axioms: Vec::new(),
        };
    };
    for exit in exits {
        guaranteed.retain(|fact| exit.contains(fact));
    }
    ReconstructedMachineSemantics {
        operation_obligations,
        exit_axioms: guaranteed,
    }
}

fn exact_integer_cast_obligation(
    source_type: psi_core::IntegerType,
    target_type: psi_core::IntegerType,
    operand: ScalarTerm,
) -> Proposition {
    let mut bounds = Vec::with_capacity(2);
    let source_minimum = source_type.minimum_value();
    let target_minimum = target_type.minimum_value();
    if integer_value_cmp(target_minimum, source_minimum).is_gt() {
        let boundary = target_type
            .exact_cast_value_to(source_type, target_minimum)
            .expect("a stricter target minimum is representable by the source type");
        let boundary = ScalarTerm::integer(source_type, boundary)
            .expect("converted exact-cast minimum is admitted by its source type");
        bounds.push(Proposition::LessOrEqual(boundary, operand.clone()));
    }

    let source_maximum = source_type.maximum_value();
    let target_maximum = target_type.maximum_value();
    if integer_value_cmp(target_maximum, source_maximum).is_lt() {
        let boundary = target_type
            .exact_cast_value_to(source_type, target_maximum)
            .expect("a stricter target maximum is representable by the source type");
        let boundary = ScalarTerm::integer(source_type, boundary)
            .expect("converted exact-cast maximum is admitted by its source type");
        bounds.push(Proposition::LessOrEqual(operand, boundary));
    }

    match bounds.len() {
        0 => unreachable!("validator rejects exact casts whose source range already fits"),
        1 => bounds.pop().expect("one exact-cast bound exists"),
        _ => Proposition::Conjunction(bounds),
    }
}

fn exact_integer_shift_obligation(
    value_type: psi_core::IntegerType,
    count_type: psi_core::IntegerType,
    count: ScalarTerm,
) -> Proposition {
    let mut bounds = Vec::with_capacity(2);
    if count_type.minimum_value() != IntegerValue::Unsigned(0)
        && integer_value_cmp(count_type.minimum_value(), IntegerValue::Unsigned(0)).is_lt()
    {
        let zero = ScalarTerm::integer(
            count_type,
            match count_type.sign() {
                psi_core::IntegerSign::Signed => IntegerValue::Signed(0),
                psi_core::IntegerSign::Unsigned => IntegerValue::Unsigned(0),
            },
        )
        .expect("fixed integer count types admit zero");
        bounds.push(Proposition::LessOrEqual(zero, count.clone()));
    }
    let maximum = u128::from(value_type.bits() - 1);
    let maximum = match count_type.sign() {
        psi_core::IntegerSign::Signed => i128::try_from(maximum).ok().map(IntegerValue::Signed),
        psi_core::IntegerSign::Unsigned => Some(IntegerValue::Unsigned(maximum)),
    };
    if let Some(maximum) = maximum
        && count_type.admits(maximum)
        && integer_value_cmp(count_type.maximum_value(), maximum).is_gt()
    {
        let maximum = ScalarTerm::integer(count_type, maximum)
            .expect("admitted exact-shift maximum remains in the count type");
        bounds.push(Proposition::LessOrEqual(count, maximum));
    }
    canonical_conjunction(bounds)
}

fn exact_integer_shift_left_obligation(
    value_type: psi_core::IntegerType,
    count_type: psi_core::IntegerType,
    value: ScalarTerm,
    count: ScalarTerm,
    semantic_axioms: &[Proposition],
) -> Proposition {
    if let Some(count) = exact_known_shift_count(value_type, count_type, &count, semantic_axioms) {
        let mut bounds = Vec::with_capacity(2);
        append_exact_shift_left_value_bounds(&mut bounds, value_type, value, count);
        return canonical_conjunction(bounds);
    }

    let count_bounds = exact_integer_shift_obligation(value_type, count_type, count.clone());
    let mut bounds = match count_bounds {
        Proposition::Truth => Vec::new(),
        Proposition::Conjunction(bounds) => bounds,
        bound => vec![bound],
    };
    let known_maximum = known_shift_count_maximum(value_type, count_type, &count, semantic_axioms);
    if let Some(maximum) = known_maximum {
        bounds
            .retain(|bound| !matches!(bound, Proposition::LessOrEqual(left, _) if left == &count));
        let maximum = ScalarTerm::integer(
            count_type,
            match count_type.sign() {
                IntegerSign::Signed => IntegerValue::Signed(i128::from(maximum)),
                IntegerSign::Unsigned => IntegerValue::Unsigned(u128::from(maximum)),
            },
        )
        .expect("known exact-shift maximum remains in its count carrier");
        bounds.push(Proposition::LessOrEqual(count, maximum));
    }
    let maximum_count = known_maximum.unwrap_or_else(|| u32::from(value_type.bits() - 1));
    append_exact_shift_left_value_bounds(&mut bounds, value_type, value, maximum_count);
    canonical_conjunction(bounds)
}

fn append_exact_shift_left_value_bounds(
    bounds: &mut Vec<Proposition>,
    value_type: psi_core::IntegerType,
    value: ScalarTerm,
    maximum_count: u32,
) {
    if maximum_count == 0 {
        return;
    }
    match value_type.sign() {
        IntegerSign::Unsigned => {
            let IntegerValue::Unsigned(maximum) = value_type.maximum_value() else {
                unreachable!("unsigned fixed integer type has an unsigned maximum")
            };
            let maximum =
                ScalarTerm::integer(value_type, IntegerValue::Unsigned(maximum >> maximum_count))
                    .expect("shifted unsigned maximum remains in its carrier");
            bounds.push(Proposition::LessOrEqual(value, maximum));
        }
        IntegerSign::Signed => {
            let (IntegerValue::Signed(minimum), IntegerValue::Signed(maximum)) =
                (value_type.minimum_value(), value_type.maximum_value())
            else {
                unreachable!("signed fixed integer type has signed bounds")
            };
            let minimum =
                ScalarTerm::integer(value_type, IntegerValue::Signed(minimum >> maximum_count))
                    .expect("shifted signed minimum remains in its carrier");
            let maximum =
                ScalarTerm::integer(value_type, IntegerValue::Signed(maximum >> maximum_count))
                    .expect("shifted signed maximum remains in its carrier");
            bounds.push(Proposition::LessOrEqual(minimum, value.clone()));
            bounds.push(Proposition::LessOrEqual(value, maximum));
        }
    }
}

fn canonical_conjunction(mut conjuncts: Vec<Proposition>) -> Proposition {
    match conjuncts.len() {
        0 => Proposition::Truth,
        1 => conjuncts.pop().expect("one conjunct exists"),
        _ => Proposition::Conjunction(conjuncts),
    }
}

fn exact_known_shift_count(
    value_type: psi_core::IntegerType,
    count_type: psi_core::IntegerType,
    count: &ScalarTerm,
    semantic_axioms: &[Proposition],
) -> Option<u32> {
    let (known_type, known_count) = count.integer_value().or_else(|| {
        semantic_axioms.iter().rev().find_map(|axiom| {
            let Proposition::Equal(left, right) = axiom else {
                return None;
            };
            if left == count {
                right.integer_value()
            } else if right == count {
                left.integer_value()
            } else {
                None
            }
        })
    })?;
    if known_type != count_type || !count_type.admits(known_count) {
        return None;
    }
    let count = match known_count {
        IntegerValue::Unsigned(count) => u32::try_from(count).ok()?,
        IntegerValue::Signed(count) => u32::try_from(count).ok()?,
    };
    (count < u32::from(value_type.bits())).then_some(count)
}

fn known_shift_count_maximum(
    value_type: psi_core::IntegerType,
    count_type: psi_core::IntegerType,
    count: &ScalarTerm,
    semantic_axioms: &[Proposition],
) -> Option<u32> {
    semantic_axioms
        .iter()
        .filter_map(|axiom| {
            let Proposition::LessOrEqual(left, right) = axiom else {
                return None;
            };
            if left != count {
                return None;
            }
            let (bound_type, bound) = right.integer_value()?;
            if bound_type != count_type || !count_type.admits(bound) {
                return None;
            }
            let bound = match bound {
                IntegerValue::Unsigned(bound) => u32::try_from(bound).ok()?,
                IntegerValue::Signed(bound) => u32::try_from(bound).ok()?,
            };
            (bound < u32::from(value_type.bits())).then_some(bound)
        })
        .min()
}

fn integer_value_cmp(left: IntegerValue, right: IntegerValue) -> std::cmp::Ordering {
    match (left, right) {
        (IntegerValue::Signed(left), IntegerValue::Signed(right)) => left.cmp(&right),
        (IntegerValue::Unsigned(left), IntegerValue::Unsigned(right)) => left.cmp(&right),
        (IntegerValue::Signed(left), IntegerValue::Unsigned(right)) => {
            if left < 0 {
                std::cmp::Ordering::Less
            } else {
                (left as u128).cmp(&right)
            }
        }
        (IntegerValue::Unsigned(left), IntegerValue::Signed(right)) => {
            if right < 0 {
                std::cmp::Ordering::Greater
            } else {
                left.cmp(&(right as u128))
            }
        }
    }
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
    // Keep the pre-V28 edge-equality prefix stable so archived certificates
    // retain their semantic-axiom indexes. Rewritten path facts are appended.
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

fn substitute_proposition_values(
    proposition: &Proposition,
    substitutions: &BTreeMap<ValueId, ScalarTerm>,
) -> Proposition {
    match proposition {
        Proposition::Truth => Proposition::Truth,
        Proposition::Falsehood => Proposition::Falsehood,
        Proposition::Atom(atom) => Proposition::Atom(*atom),
        Proposition::Equal(left, right) => Proposition::Equal(
            substitute_scalar_term_values(left, substitutions),
            substitute_scalar_term_values(right, substitutions),
        ),
        Proposition::LessThan(left, right) => Proposition::LessThan(
            substitute_scalar_term_values(left, substitutions),
            substitute_scalar_term_values(right, substitutions),
        ),
        Proposition::LessOrEqual(left, right) => Proposition::LessOrEqual(
            substitute_scalar_term_values(left, substitutions),
            substitute_scalar_term_values(right, substitutions),
        ),
        Proposition::Conjunction(conjuncts) => Proposition::Conjunction(
            conjuncts
                .iter()
                .map(|conjunct| substitute_proposition_values(conjunct, substitutions))
                .collect(),
        ),
        Proposition::Implication {
            premise,
            conclusion,
        } => Proposition::Implication {
            premise: Box::new(substitute_proposition_values(premise, substitutions)),
            conclusion: Box::new(substitute_proposition_values(conclusion, substitutions)),
        },
        Proposition::ContentConservation(conservation) => {
            Proposition::ContentConservation(conservation.clone())
        }
    }
}

fn substitute_scalar_term_values(
    term: &ScalarTerm,
    substitutions: &BTreeMap<ValueId, ScalarTerm>,
) -> ScalarTerm {
    let recurse = |term: &ScalarTerm| substitute_scalar_term_values(term, substitutions);
    match term {
        ScalarTerm::Value { id, .. } => substitutions
            .get(id)
            .cloned()
            .unwrap_or_else(|| term.clone()),
        ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => term.clone(),
        ScalarTerm::BooleanNot { operand } => ScalarTerm::BooleanNot {
            operand: Box::new(recurse(operand)),
        },
        ScalarTerm::IntegerBitwiseNot {
            scalar_type,
            operand,
        } => ScalarTerm::IntegerBitwiseNot {
            scalar_type: *scalar_type,
            operand: Box::new(recurse(operand)),
        },
        ScalarTerm::IntegerWiden {
            source_type,
            target_type,
            operand,
        } => ScalarTerm::IntegerWiden {
            source_type: *source_type,
            target_type: *target_type,
            operand: Box::new(recurse(operand)),
        },
        ScalarTerm::IntegerExactCast {
            source_type,
            target_type,
            operand,
        } => ScalarTerm::IntegerExactCast {
            source_type: *source_type,
            target_type: *target_type,
            operand: Box::new(recurse(operand)),
        },
        ScalarTerm::BooleanEqual { left, right } => ScalarTerm::BooleanEqual {
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::IntegerEqual {
            scalar_type,
            left,
            right,
        } => ScalarTerm::IntegerEqual {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::IntegerLessThan {
            scalar_type,
            left,
            right,
        } => ScalarTerm::IntegerLessThan {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::IntegerLessOrEqual {
            scalar_type,
            left,
            right,
        } => ScalarTerm::IntegerLessOrEqual {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::IntegerBitwiseAnd {
            scalar_type,
            left,
            right,
        } => ScalarTerm::IntegerBitwiseAnd {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::IntegerBitwiseOr {
            scalar_type,
            left,
            right,
        } => ScalarTerm::IntegerBitwiseOr {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::IntegerBitwiseXor {
            scalar_type,
            left,
            right,
        } => ScalarTerm::IntegerBitwiseXor {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::WrappingIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
        } => ScalarTerm::WrappingIntegerShiftLeft {
            value_type: *value_type,
            count_type: *count_type,
            value: Box::new(recurse(value)),
            count: Box::new(recurse(count)),
        },
        ScalarTerm::WrappingIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
        } => ScalarTerm::WrappingIntegerShiftRight {
            value_type: *value_type,
            count_type: *count_type,
            value: Box::new(recurse(value)),
            count: Box::new(recurse(count)),
        },
        ScalarTerm::ExactIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
        } => ScalarTerm::ExactIntegerShiftRight {
            value_type: *value_type,
            count_type: *count_type,
            value: Box::new(recurse(value)),
            count: Box::new(recurse(count)),
        },
        ScalarTerm::ExactIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
        } => ScalarTerm::ExactIntegerShiftLeft {
            value_type: *value_type,
            count_type: *count_type,
            value: Box::new(recurse(value)),
            count: Box::new(recurse(count)),
        },
        ScalarTerm::WrappingIntegerAdd {
            scalar_type,
            left,
            right,
        } => ScalarTerm::WrappingIntegerAdd {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::SaturatingIntegerAdd {
            scalar_type,
            left,
            right,
        } => ScalarTerm::SaturatingIntegerAdd {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::WrappingIntegerSubtract {
            scalar_type,
            left,
            right,
        } => ScalarTerm::WrappingIntegerSubtract {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::SaturatingIntegerSubtract {
            scalar_type,
            left,
            right,
        } => ScalarTerm::SaturatingIntegerSubtract {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::WrappingIntegerMultiply {
            scalar_type,
            left,
            right,
        } => ScalarTerm::WrappingIntegerMultiply {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::SaturatingIntegerMultiply {
            scalar_type,
            left,
            right,
        } => ScalarTerm::SaturatingIntegerMultiply {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationError {
    Module(ModuleError),
    DuplicateEvidence(ObligationId),
    MissingEvidence(ObligationId),
    UnknownEvidence(ObligationId),
    RejectedEvidence {
        obligation: ObligationId,
        error: EvidenceError,
    },
}

impl std::fmt::Display for VerificationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for VerificationError {}
