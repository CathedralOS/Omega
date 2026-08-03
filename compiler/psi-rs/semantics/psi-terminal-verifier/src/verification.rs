use std::collections::{BTreeMap, BTreeSet};

use psi_core::{ObligationId, Proposition, ScalarTerm, ScalarType, ValueId};
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
        let semantic_axioms = reconstruct_semantic_axioms(machine);
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
                &semantic_axioms,
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

/// Reconstruct only facts established on every return path. A conditional is
/// structural control, not a proposition source; arm-local operations and
/// bindings become usable only when the same fact survives every incoming
/// path to a merge and every eventual return.
fn reconstruct_semantic_axioms(machine: &TerminalMachine) -> Vec<Proposition> {
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
            Terminator::Return { .. } => Vec::new(),
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
                for (parameter, argument) in target_block.parameters.iter().zip(arguments) {
                    axioms.push(Proposition::Equal(
                        value_term(parameter.id),
                        value_term(*argument),
                    ));
                }
                incoming.entry(*target).or_default().push(axioms);
            }
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                for successor in [when_true, when_false] {
                    let target_block = blocks
                        .get(&successor.target)
                        .expect("validator requires conditional target");
                    let mut arm_axioms = axioms.clone();
                    for (parameter, argument) in
                        target_block.parameters.iter().zip(&successor.arguments)
                    {
                        arm_axioms.push(Proposition::Equal(
                            value_term(parameter.id),
                            value_term(*argument),
                        ));
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
        }
    }
    let mut exits = exits.into_iter();
    let mut guaranteed = exits
        .next()
        .expect("validated acyclic machine has at least one return");
    for exit in exits {
        guaranteed.retain(|fact| exit.contains(fact));
    }
    guaranteed
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
