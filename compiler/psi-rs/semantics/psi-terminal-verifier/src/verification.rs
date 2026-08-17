use std::collections::{BTreeMap, BTreeSet};

use psi_core::{
    CanonicalStructuralPathSegment, ContentConservation, ContentStructuralPlace, ContentTerm,
    EvidenceIdentity, EvidenceTermId, IntegerSign, IntegerValue, ObligationId, PlaceId,
    Proposition, ScalarTerm, ScalarType, ValueId,
};
use psi_proof_kernel::{
    AcceptedFact, AdmissionProfile, EvidenceError, EvidenceRoute, Obligation, ObligationClass,
    verify_obligation,
};
use psi_terminal::{OperationKind, TerminalMachine, TerminalModule, Terminator};

use crate::{ModuleError, ValidatedTerminalModule, validate_module};

mod affine_joins;
mod integer_add_subtract;
mod integer_affine;
mod integer_conversion;
mod integer_divide_remainder;
mod integer_multiply;
mod integer_shift;

use integer_add_subtract::{exact_integer_add_obligation, exact_integer_subtract_obligation};
#[cfg(test)]
use integer_affine::exact_integer_affine_interval_obligation;
use integer_affine::{
    exact_integer_affine_cast_affine_obligation, exact_integer_affine_chain_obligation,
    exact_integer_cast_chain_then_affine_suffix_obligation,
    exact_integer_cast_then_affine_chain_obligation,
    exact_integer_signed_affine_cast_affine_obligation,
    exact_integer_signed_affine_chain_obligation, exact_integer_signed_affine_initial_form,
    exact_integer_signed_affine_interval_obligation, exact_integer_signed_affine_preimage_interval,
    exact_integer_signed_affine_replay,
};
#[cfg(test)]
use integer_conversion::{
    exact_integer_affine_chain_cast_obligation, exact_integer_cast_chain_obligation,
    exact_integer_computed_prefix_cast_chain_interval_obligation,
    exact_integer_computed_prefix_cast_chain_obligation,
    exact_integer_computed_prefix_mixed_conversion_chain_cast_obligation,
    exact_integer_computed_prefix_mixed_conversion_chain_interval_obligation,
    exact_integer_computed_prefix_widen_chain_interval_obligation,
    exact_integer_signed_affine_chain_cast_obligation,
    exact_integer_signed_multiply_chain_cast_obligation,
};
use integer_conversion::{
    exact_integer_affine_preimage_interval, exact_integer_affine_preimage_obligation,
    exact_integer_cast_chain_root_interval, exact_integer_cast_obligation,
    exact_integer_cast_then_offset_obligation,
    exact_integer_computed_prefix_conversion_interval_obligation,
    exact_integer_divide_remainder_cast_affine_obligation,
    exact_integer_divide_remainder_chain_hull,
    exact_integer_divide_remainder_then_affine_obligation,
    exact_integer_signed_product_interval_obligation, partial_fixed_native_integer_cast,
};
#[cfg(test)]
use integer_divide_remainder::{
    exact_integer_divide_obligation, exact_integer_remainder_obligation,
};
use integer_divide_remainder::{
    exact_integer_divide_obligation_with_definitions,
    exact_integer_remainder_obligation_with_definitions, saturating_integer_divide_obligation,
    saturating_integer_remainder_obligation, wrapping_integer_divide_obligation,
    wrapping_integer_remainder_obligation,
};
use integer_multiply::exact_integer_multiply_obligation_with_definitions;
#[cfg(test)]
use integer_multiply::{
    exact_integer_cast_chain_then_signed_product_suffix_obligation,
    exact_integer_cast_then_signed_affine_chain_obligation,
    exact_integer_cast_then_signed_multiply_chain_obligation, exact_integer_multiply_obligation,
    exact_integer_signed_multiply_chain_obligation,
};
#[cfg(test)]
use integer_shift::{
    exact_integer_affine_cast_shift_obligation,
    exact_integer_arithmetic_then_shift_chain_obligation,
    exact_integer_cast_chain_then_shift_suffix_obligation,
    exact_integer_cast_then_mixed_shift_chain_obligation,
    exact_integer_cast_then_shift_left_chain_obligation,
    exact_integer_cumulative_shift_left_obligation,
    exact_integer_divide_remainder_cast_shift_obligation,
    exact_integer_divide_remainder_then_shift_obligation,
    exact_integer_mixed_shift_chain_cast_obligation, exact_integer_mixed_shift_chain_obligation,
    exact_integer_mixed_shift_preimage, exact_integer_shift_cast_shift_obligation,
    exact_integer_shift_left_chain_obligation,
    exact_integer_shift_right_chain_cast_interval_obligation,
};
use integer_shift::{
    exact_integer_shift_cast_affine_obligation, exact_integer_shift_left_obligation,
    exact_integer_shift_obligation, exact_integer_shift_then_arithmetic_chain_obligation,
};

#[cfg(test)]
use affine_joins::{
    exact_integer_affine_fork_join_obligation, exact_integer_affine_quadratic_range,
    exact_integer_distinct_root_affine_fork_join_obligation,
    exact_integer_distinct_root_affine_product_join_obligation,
    exact_integer_same_root_affine_divide_remainder_join_obligation,
    exact_integer_same_root_affine_product_join_obligation,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObligationEvidence {
    pub obligation: ObligationId,
    pub route: EvidenceRoute,
}

/// Checked provenance for one freshly introduced carrierless evidence term.
/// This belongs to the proof artifact, not terminal-Psi semantic identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvidenceProducerProvenance {
    pub id: EvidenceIdentity,
    pub term: EvidenceTermId,
    pub conformance_identity: String,
    pub evidence_trait_identity: String,
    pub rows: Vec<EvidenceProducerRealization>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvidenceProducerRealization {
    pub declaring_trait_identity: String,
    pub declaring_trait_arguments: Vec<String>,
    pub requirement_identity: String,
    pub realization_machine_identity: String,
    pub realization_state_identity: String,
    pub source: EvidenceProducerRowSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EvidenceProducerRowSource {
    Inline,
    Reference,
    TraitDefault,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProofBundle {
    pub evidence: Vec<ObligationEvidence>,
    pub evidence_producers: Vec<EvidenceProducerProvenance>,
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
    proof_bundle: ProofBundle,
    accepted_facts: Vec<AcceptedFact>,
}

impl<'module> VerifiedTerminalModule<'module> {
    pub const fn module(&self) -> &'module TerminalModule {
        self.validated.module()
    }

    pub fn accepted_facts(&self) -> &[AcceptedFact] {
        &self.accepted_facts
    }

    /// Exact artifact evidence accepted for this module. Retaining the bundle
    /// lets artifact consumers re-encode the verified semantic/proof pair
    /// without consulting producer state.
    pub const fn proof_bundle(&self) -> &ProofBundle {
        &self.proof_bundle
    }
}

pub fn verify_module<'module>(
    module: &'module TerminalModule,
    proof_bundle: &ProofBundle,
    profile: &AdmissionProfile,
) -> Result<VerifiedTerminalModule<'module>, VerificationError> {
    let validated = validate_module(module).map_err(VerificationError::Module)?;
    validate_evidence_producer_provenance(module, proof_bundle)?;
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
        let semantics = reconstruct_machine_semantics(module, machine);
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
        proof_bundle: proof_bundle.clone(),
        accepted_facts,
    })
}

fn validate_evidence_producer_provenance(
    module: &TerminalModule,
    proof_bundle: &ProofBundle,
) -> Result<(), VerificationError> {
    use psi_terminal::EvidenceContractLaneKind;

    let terms = module
        .evidence_terms
        .iter()
        .map(|term| (term.id, term))
        .collect::<BTreeMap<_, _>>();
    let required = module
        .evidence_contract_lanes
        .iter()
        .filter(|lane| lane.kind == EvidenceContractLaneKind::Requires)
        .map(|lane| (lane.machine, lane.term))
        .collect::<BTreeSet<_>>();
    let mut unmatched_ensures = BTreeMap::<_, usize>::new();
    let package_outputs = module
        .evidence_package_invocations
        .iter()
        .flat_map(|invocation| invocation.outputs.iter().map(|output| output.output))
        .collect::<BTreeSet<_>>();
    for lane in &module.evidence_contract_lanes {
        if lane.kind == EvidenceContractLaneKind::Ensures
            && !required.contains(&(lane.machine, lane.term))
            && !package_outputs.contains(&lane.term)
        {
            *unmatched_ensures.entry(lane.term).or_default() += 1;
        }
    }
    for invocation in &module.evidence_package_invocations {
        for output in &invocation.outputs {
            unmatched_ensures.entry(output.callee_output).or_insert(1);
        }
    }

    let mut previous_id = None;
    let mut previous_term = None;
    let mut produced_terms = BTreeSet::new();
    for (index, provenance) in proof_bundle.evidence_producers.iter().enumerate() {
        let expected = EvidenceIdentity::new(
            u64::try_from(index)
                .expect("producer provenance count fits u64")
                .checked_add(1)
                .expect("one-based producer provenance identity fits u64"),
        )
        .expect("one-based producer provenance identity is nonzero");
        if provenance.id != expected {
            return Err(VerificationError::NonDenseEvidenceProducer {
                expected,
                actual: provenance.id,
            });
        }
        if previous_id.is_some_and(|previous| previous >= provenance.id) {
            return Err(VerificationError::NonCanonicalEvidenceProducerOrder);
        }
        previous_id = Some(provenance.id);
        if previous_term.is_some_and(|previous| previous >= provenance.term) {
            return Err(VerificationError::NonCanonicalEvidenceProducerOrder);
        }
        previous_term = Some(provenance.term);
        if !produced_terms.insert(provenance.term) {
            return Err(VerificationError::DuplicateEvidenceProducerTerm(
                provenance.term,
            ));
        }
        let term =
            terms
                .get(&provenance.term)
                .ok_or(VerificationError::UnknownEvidenceProducerTerm(
                    provenance.term,
                ))?;
        if unmatched_ensures.get(&provenance.term).copied() != Some(1) {
            return Err(VerificationError::UnusedEvidenceProducerTerm(
                provenance.term,
            ));
        }
        if provenance.conformance_identity.is_empty()
            || provenance.evidence_trait_identity.is_empty()
        {
            return Err(VerificationError::InvalidEvidenceProducer(provenance.id));
        }
        if provenance.evidence_trait_identity != term.interface.trait_identity {
            return Err(VerificationError::EvidenceProducerInterfaceMismatch(
                provenance.term,
            ));
        }
        let mut previous_row = None;
        for row in &provenance.rows {
            if row.declaring_trait_identity.is_empty()
                || row.declaring_trait_arguments.iter().any(String::is_empty)
                || row.requirement_identity.is_empty()
                || row.realization_machine_identity.is_empty()
                || row.realization_state_identity.is_empty()
            {
                return Err(VerificationError::InvalidEvidenceProducer(provenance.id));
            }
            if previous_row.is_some_and(|previous: &EvidenceProducerRealization| previous >= row) {
                return Err(VerificationError::NonCanonicalEvidenceProducerRows(
                    provenance.id,
                ));
            }
            previous_row = Some(row);
        }
        let mut realized_requirements = provenance
            .rows
            .iter()
            .map(|row| psi_terminal::EvidenceRequirementIdentity {
                declaring_trait_identity: row.declaring_trait_identity.clone(),
                declaring_trait_arguments: row.declaring_trait_arguments.clone(),
                requirement_identity: row.requirement_identity.clone(),
            })
            .collect::<Vec<_>>();
        realized_requirements.sort();
        if realized_requirements != term.interface.requirements {
            return Err(VerificationError::EvidenceProducerInterfaceMismatch(
                provenance.term,
            ));
        }
    }
    if let Some(term) = unmatched_ensures
        .keys()
        .find(|term| !produced_terms.contains(term))
        .copied()
    {
        return Err(VerificationError::MissingEvidenceProducer(term));
    }
    Ok(())
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
        .flat_map(|machine| reconstruct_machine_semantics(module, machine).operation_obligations)
        .collect())
}

/// Reconstruct facts at each executable obligation site and facts established
/// on every return path. A true conditional edge establishes the predicate
/// computed by its condition operation; edge bindings rewrite those facts to
/// successor parameters. Merge and return facts remain intersection-only.
fn reconstruct_machine_semantics(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> ReconstructedMachineSemantics {
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
            match operation.kind.clone() {
                OperationKind::EstablishTrivialAffineLocal { .. } => {}
                OperationKind::CallUnit {
                    callee,
                    structural_arguments,
                    requirement_obligations,
                    ..
                } => {
                    let callee = machines
                        .get(&callee)
                        .copied()
                        .expect("validated unit-call target exists");
                    let substitutions = callee
                        .structural_parameters
                        .iter()
                        .zip(&structural_arguments)
                        .map(|(parameter, argument)| {
                            (
                                parameter.place,
                                (
                                    argument.place,
                                    crate::validation::structural_argument_canonical_prefix(
                                        module, machine, argument,
                                    )
                                    .expect("validated structural argument has a canonical path"),
                                ),
                            )
                        })
                        .collect::<BTreeMap<_, _>>();
                    for (required, obligation) in
                        callee.contract.requires.iter().zip(requirement_obligations)
                    {
                        operation_obligations.push(ReconstructedOperationObligation {
                            obligation: Obligation {
                                id: obligation,
                                proposition: substitute_proposition_structural_places(
                                    required,
                                    &substitutions,
                                ),
                                class: ObligationClass::Derivable,
                            },
                            semantic_axioms: axioms.clone(),
                        });
                    }
                    for guarantee in &callee.contract.ensures {
                        push_unique(
                            &mut axioms,
                            substitute_proposition_structural_places(
                                &guarantee.proposition,
                                &substitutions,
                            ),
                        );
                    }
                }
                OperationKind::BoundaryCall { .. } | OperationKind::PortWrite { .. } => {}
                OperationKind::Call {
                    callee,
                    arguments,
                    requirement_obligations,
                    ..
                } => {
                    let callee = machines
                        .get(&callee)
                        .copied()
                        .expect("validated call target exists");
                    let mut substitutions = callee
                        .parameters
                        .iter()
                        .zip(&arguments)
                        .map(|(parameter, argument)| (parameter.id, value_term(*argument)))
                        .collect::<BTreeMap<_, _>>();
                    substitutions.insert(
                        callee
                            .result
                            .scalar()
                            .expect("validated call target has a scalar result")
                            .id,
                        value_term(operation.result.expect_scalar().id),
                    );
                    for (required, obligation) in
                        callee.contract.requires.iter().zip(requirement_obligations)
                    {
                        operation_obligations.push(ReconstructedOperationObligation {
                            obligation: Obligation {
                                id: obligation,
                                proposition: substitute_proposition_values(
                                    required,
                                    &substitutions,
                                ),
                                class: ObligationClass::Derivable,
                            },
                            semantic_axioms: axioms.clone(),
                        });
                    }
                    for guarantee in &callee.contract.ensures {
                        push_unique(
                            &mut axioms,
                            substitute_proposition_values(&guarantee.proposition, &substitutions),
                        );
                    }
                }
                OperationKind::IntegerConstant { value } => {
                    let ScalarType::Integer(integer_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        unreachable!("validator requires integer constant result type");
                    };
                    let literal = ScalarTerm::integer(integer_type, value)
                        .expect("validator requires representable integer constant");
                    axioms.push(Proposition::Equal(
                        value_term(operation.result.expect_scalar().id),
                        literal,
                    ));
                }
                OperationKind::BooleanConstant { value } => {
                    axioms.push(Proposition::Equal(
                        value_term(operation.result.expect_scalar().id),
                        ScalarTerm::boolean(value),
                    ));
                }
                OperationKind::BooleanStructuralField { source, field } => {
                    axioms.push(Proposition::Equal(
                        value_term(operation.result.expect_scalar().id),
                        ScalarTerm::boolean_field(source, field),
                    ));
                }
                OperationKind::BooleanNot { operand } => {
                    let negated = ScalarTerm::boolean_not(value_term(operand))
                        .expect("validator requires a Boolean logical-not operand");
                    axioms.push(Proposition::Equal(
                        value_term(operation.result.expect_scalar().id),
                        negated,
                    ));
                }
                OperationKind::BooleanEqual { left, right } => {
                    let equal = ScalarTerm::boolean_equal(value_term(left), value_term(right))
                        .expect("validator requires Boolean equality operands");
                    axioms.push(Proposition::Equal(
                        value_term(operation.result.expect_scalar().id),
                        equal,
                    ));
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
                    axioms.push(Proposition::Equal(
                        value_term(operation.result.expect_scalar().id),
                        equal,
                    ));
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
                    axioms.push(Proposition::Equal(
                        value_term(operation.result.expect_scalar().id),
                        ordered,
                    ));
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
                    axioms.push(Proposition::Equal(
                        value_term(operation.result.expect_scalar().id),
                        ordered,
                    ));
                }
                OperationKind::IntegerBitwiseNot { operand } => {
                    let ScalarType::Integer(integer_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        unreachable!("validator requires bitwise-not integer result type")
                    };
                    let result = ScalarTerm::integer_bitwise_not(integer_type, value_term(operand))
                        .expect("validator requires exact bitwise-not operand type");
                    axioms.push(Proposition::Equal(
                        value_term(operation.result.expect_scalar().id),
                        result,
                    ));
                }
                OperationKind::IntegerWiden { operand } => {
                    let ScalarType::Integer(source_type) = value_term(operand).scalar_type() else {
                        unreachable!("validator requires an integer widening operand")
                    };
                    let ScalarType::Integer(target_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        unreachable!("validator requires an integer widening result")
                    };
                    let result =
                        ScalarTerm::integer_widen(source_type, target_type, value_term(operand))
                            .expect("validator requires a universally total integer widening");
                    axioms.push(Proposition::Equal(
                        value_term(operation.result.expect_scalar().id),
                        result,
                    ));
                }
                OperationKind::IntegerExactCast {
                    operand,
                    obligation,
                } => {
                    let ScalarType::Integer(source_type) = value_term(operand).scalar_type() else {
                        unreachable!("validator requires an integer exact-cast operand")
                    };
                    let ScalarType::Integer(target_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        unreachable!("validator requires an integer exact-cast result")
                    };
                    operation_obligations.push(ReconstructedOperationObligation {
                        obligation: Obligation {
                            id: obligation,
                            proposition: exact_integer_cast_obligation(
                                source_type,
                                target_type,
                                value_term(operand),
                                &axioms,
                                &machine_parameter_values,
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
                    axioms.push(Proposition::Equal(
                        value_term(operation.result.expect_scalar().id),
                        result,
                    ));
                }
                OperationKind::IntegerBitwiseAnd { left, right }
                | OperationKind::IntegerBitwiseOr { left, right }
                | OperationKind::IntegerBitwiseXor { left, right } => {
                    let ScalarType::Integer(integer_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        unreachable!("validator requires bitwise integer result type")
                    };
                    let result = match operation.kind.clone() {
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
                    axioms.push(Proposition::Equal(
                        value_term(operation.result.expect_scalar().id),
                        result,
                    ));
                }
                OperationKind::WrappingIntegerShiftLeft { value, count }
                | OperationKind::WrappingIntegerShiftRight { value, count } => {
                    let ScalarType::Integer(value_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        unreachable!("validator requires wrapping-shift integer result type")
                    };
                    let ScalarType::Integer(count_type) = value_term(count).scalar_type() else {
                        unreachable!("validator requires wrapping-shift integer count type")
                    };
                    let result = match operation.kind.clone() {
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
                    axioms.push(Proposition::Equal(
                        value_term(operation.result.expect_scalar().id),
                        result,
                    ));
                }
                OperationKind::ExactIntegerShiftRight {
                    value,
                    count,
                    obligation,
                } => {
                    let ScalarType::Integer(value_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
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
                                &axioms,
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
                    axioms.push(Proposition::Equal(
                        value_term(operation.result.expect_scalar().id),
                        result,
                    ));
                }
                OperationKind::ExactIntegerShiftLeft {
                    value,
                    count,
                    obligation,
                } => {
                    let ScalarType::Integer(value_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        unreachable!("validator requires exact-shift integer result type")
                    };
                    let ScalarType::Integer(count_type) = value_term(count).scalar_type() else {
                        unreachable!("validator requires exact-shift integer count type")
                    };
                    let definition_axiom_count = axioms.len();
                    let mut available_bounds = axioms.clone();
                    available_bounds.extend(machine.contract.requires.iter().cloned());
                    operation_obligations.push(ReconstructedOperationObligation {
                        obligation: Obligation {
                            id: obligation,
                            proposition: exact_integer_shift_left_obligation(
                                value_type,
                                count_type,
                                value_term(value),
                                value_term(count),
                                &available_bounds,
                                definition_axiom_count,
                                &machine_parameter_values,
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
                    axioms.push(Proposition::Equal(
                        value_term(operation.result.expect_scalar().id),
                        result,
                    ));
                }
                OperationKind::ExactIntegerAdd {
                    left,
                    right,
                    obligation,
                } => {
                    let ScalarType::Integer(integer_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        unreachable!("validator requires exact-add integer result type")
                    };
                    let definition_axiom_count = axioms.len();
                    let mut available_bounds = axioms.clone();
                    available_bounds.extend(machine.contract.requires.iter().cloned());
                    operation_obligations.push(ReconstructedOperationObligation {
                        obligation: Obligation {
                            id: obligation,
                            proposition: exact_integer_add_obligation(
                                integer_type,
                                value_term(left),
                                value_term(right),
                                &available_bounds,
                                definition_axiom_count,
                                &machine_parameter_values,
                            ),
                            class: ObligationClass::Derivable,
                        },
                        semantic_axioms: axioms.clone(),
                    });
                    let result = ScalarTerm::exact_integer_add(
                        integer_type,
                        value_term(left),
                        value_term(right),
                    )
                    .expect("validator requires exact exact-add operand types");
                    axioms.push(Proposition::Equal(
                        value_term(operation.result.expect_scalar().id),
                        result,
                    ));
                }
                OperationKind::ExactIntegerSubtract {
                    left,
                    right,
                    obligation,
                } => {
                    let ScalarType::Integer(integer_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        unreachable!("validator requires exact-subtract integer result type")
                    };
                    let definition_axiom_count = axioms.len();
                    let mut available_bounds = axioms.clone();
                    available_bounds.extend(machine.contract.requires.iter().cloned());
                    operation_obligations.push(ReconstructedOperationObligation {
                        obligation: Obligation {
                            id: obligation,
                            proposition: exact_integer_subtract_obligation(
                                integer_type,
                                value_term(left),
                                value_term(right),
                                &available_bounds,
                                definition_axiom_count,
                                &machine_parameter_values,
                            ),
                            class: ObligationClass::Derivable,
                        },
                        semantic_axioms: axioms.clone(),
                    });
                    let result = ScalarTerm::exact_integer_subtract(
                        integer_type,
                        value_term(left),
                        value_term(right),
                    )
                    .expect("validator requires exact exact-subtract operand types");
                    axioms.push(Proposition::Equal(
                        value_term(operation.result.expect_scalar().id),
                        result,
                    ));
                }
                OperationKind::ExactIntegerMultiply {
                    left,
                    right,
                    obligation,
                } => {
                    let ScalarType::Integer(integer_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        unreachable!("validator requires exact-multiply integer result type")
                    };
                    let definition_axiom_count = axioms.len();
                    let mut available_bounds = axioms.clone();
                    available_bounds.extend(machine.contract.requires.iter().cloned());
                    operation_obligations.push(ReconstructedOperationObligation {
                        obligation: Obligation {
                            id: obligation,
                            proposition: exact_integer_multiply_obligation_with_definitions(
                                integer_type,
                                value_term(left),
                                value_term(right),
                                &available_bounds,
                                definition_axiom_count,
                                &machine_parameter_values,
                            ),
                            class: ObligationClass::Derivable,
                        },
                        semantic_axioms: axioms.clone(),
                    });
                    let result = ScalarTerm::exact_integer_multiply(
                        integer_type,
                        value_term(left),
                        value_term(right),
                    )
                    .expect("validator requires exact exact-multiply operand types");
                    axioms.push(Proposition::Equal(
                        value_term(operation.result.expect_scalar().id),
                        result,
                    ));
                }
                OperationKind::ExactIntegerDivide {
                    left,
                    right,
                    obligation,
                } => {
                    let ScalarType::Integer(integer_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        unreachable!("validator requires exact-divide integer result type")
                    };
                    let definition_axiom_count = axioms.len();
                    let mut available_bounds = axioms.clone();
                    available_bounds.extend(machine.contract.requires.iter().cloned());
                    operation_obligations.push(ReconstructedOperationObligation {
                        obligation: Obligation {
                            id: obligation,
                            proposition: exact_integer_divide_obligation_with_definitions(
                                integer_type,
                                value_term(left),
                                value_term(right),
                                &available_bounds,
                                definition_axiom_count,
                                &machine_parameter_values,
                            ),
                            class: ObligationClass::Derivable,
                        },
                        semantic_axioms: axioms.clone(),
                    });
                    let result = ScalarTerm::exact_integer_divide(
                        integer_type,
                        value_term(left),
                        value_term(right),
                    )
                    .expect("validator requires exact exact-divide operand types");
                    axioms.push(Proposition::Equal(
                        value_term(operation.result.expect_scalar().id),
                        result,
                    ));
                }
                OperationKind::ExactIntegerRemainder {
                    left,
                    right,
                    obligation,
                } => {
                    let ScalarType::Integer(integer_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        unreachable!("validator requires exact-remainder integer result type")
                    };
                    let definition_axiom_count = axioms.len();
                    let mut available_bounds = axioms.clone();
                    available_bounds.extend(machine.contract.requires.iter().cloned());
                    operation_obligations.push(ReconstructedOperationObligation {
                        obligation: Obligation {
                            id: obligation,
                            proposition: exact_integer_remainder_obligation_with_definitions(
                                integer_type,
                                value_term(left),
                                value_term(right),
                                &available_bounds,
                                definition_axiom_count,
                                &machine_parameter_values,
                            ),
                            class: ObligationClass::Derivable,
                        },
                        semantic_axioms: axioms.clone(),
                    });
                    let result = ScalarTerm::exact_integer_remainder(
                        integer_type,
                        value_term(left),
                        value_term(right),
                    )
                    .expect("validator requires exact exact-remainder operand types");
                    axioms.push(Proposition::Equal(
                        value_term(operation.result.expect_scalar().id),
                        result,
                    ));
                }
                OperationKind::WrappingIntegerDivide {
                    left,
                    right,
                    obligation,
                } => {
                    let ScalarType::Integer(integer_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        unreachable!("validator requires wrapping-divide integer result type")
                    };
                    operation_obligations.push(ReconstructedOperationObligation {
                        obligation: Obligation {
                            id: obligation,
                            proposition: wrapping_integer_divide_obligation(
                                integer_type,
                                value_term(left),
                                value_term(right),
                                &axioms,
                            ),
                            class: ObligationClass::Derivable,
                        },
                        semantic_axioms: axioms.clone(),
                    });
                    let result = ScalarTerm::wrapping_integer_divide(
                        integer_type,
                        value_term(left),
                        value_term(right),
                    )
                    .expect("validator requires exact wrapping-divide operand types");
                    axioms.push(Proposition::Equal(
                        value_term(operation.result.expect_scalar().id),
                        result,
                    ));
                }
                OperationKind::WrappingIntegerRemainder {
                    left,
                    right,
                    obligation,
                } => {
                    let ScalarType::Integer(integer_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        unreachable!("validator requires wrapping-remainder integer result type")
                    };
                    operation_obligations.push(ReconstructedOperationObligation {
                        obligation: Obligation {
                            id: obligation,
                            proposition: wrapping_integer_remainder_obligation(
                                integer_type,
                                value_term(left),
                                value_term(right),
                                &axioms,
                            ),
                            class: ObligationClass::Derivable,
                        },
                        semantic_axioms: axioms.clone(),
                    });
                    let result = ScalarTerm::wrapping_integer_remainder(
                        integer_type,
                        value_term(left),
                        value_term(right),
                    )
                    .expect("validator requires matching wrapping-remainder operand types");
                    axioms.push(Proposition::Equal(
                        value_term(operation.result.expect_scalar().id),
                        result,
                    ));
                }
                OperationKind::SaturatingIntegerDivide {
                    left,
                    right,
                    obligation,
                } => {
                    let ScalarType::Integer(integer_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        unreachable!("validator requires saturating-divide integer result type")
                    };
                    operation_obligations.push(ReconstructedOperationObligation {
                        obligation: Obligation {
                            id: obligation,
                            proposition: saturating_integer_divide_obligation(
                                integer_type,
                                value_term(left),
                                value_term(right),
                                &axioms,
                            ),
                            class: ObligationClass::Derivable,
                        },
                        semantic_axioms: axioms.clone(),
                    });
                    let result = ScalarTerm::saturating_integer_divide(
                        integer_type,
                        value_term(left),
                        value_term(right),
                    )
                    .expect("validator requires matching saturating-divide operand types");
                    axioms.push(Proposition::Equal(
                        value_term(operation.result.expect_scalar().id),
                        result,
                    ));
                }
                OperationKind::SaturatingIntegerRemainder {
                    left,
                    right,
                    obligation,
                } => {
                    let ScalarType::Integer(integer_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        unreachable!("validator requires saturating-remainder integer result type")
                    };
                    operation_obligations.push(ReconstructedOperationObligation {
                        obligation: Obligation {
                            id: obligation,
                            proposition: saturating_integer_remainder_obligation(
                                integer_type,
                                value_term(left),
                                value_term(right),
                                &axioms,
                            ),
                            class: ObligationClass::Derivable,
                        },
                        semantic_axioms: axioms.clone(),
                    });
                    let result = ScalarTerm::saturating_integer_remainder(
                        integer_type,
                        value_term(left),
                        value_term(right),
                    )
                    .expect("validator requires matching saturating-remainder operand types");
                    axioms.push(Proposition::Equal(
                        value_term(operation.result.expect_scalar().id),
                        result,
                    ));
                }
                OperationKind::WrappingIntegerAdd { left, right } => {
                    let ScalarType::Integer(integer_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        unreachable!("validator requires wrapping-add integer result type")
                    };
                    let sum = ScalarTerm::wrapping_integer_add(
                        integer_type,
                        value_term(left),
                        value_term(right),
                    )
                    .expect("validator requires exact wrapping-add operand types");
                    axioms.push(Proposition::Equal(
                        value_term(operation.result.expect_scalar().id),
                        sum,
                    ));
                }
                OperationKind::SaturatingIntegerAdd { left, right } => {
                    let ScalarType::Integer(integer_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        unreachable!("validator requires saturating-add integer result type")
                    };
                    let sum = ScalarTerm::saturating_integer_add(
                        integer_type,
                        value_term(left),
                        value_term(right),
                    )
                    .expect("validator requires exact saturating-add operand types");
                    axioms.push(Proposition::Equal(
                        value_term(operation.result.expect_scalar().id),
                        sum,
                    ));
                }
                OperationKind::WrappingIntegerSubtract { left, right } => {
                    let ScalarType::Integer(integer_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        unreachable!("validator requires wrapping-subtract integer result type")
                    };
                    let difference = ScalarTerm::wrapping_integer_subtract(
                        integer_type,
                        value_term(left),
                        value_term(right),
                    )
                    .expect("validator requires exact wrapping-subtract operand types");
                    axioms.push(Proposition::Equal(
                        value_term(operation.result.expect_scalar().id),
                        difference,
                    ));
                }
                OperationKind::SaturatingIntegerSubtract { left, right } => {
                    let ScalarType::Integer(integer_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        unreachable!("validator requires saturating-subtract integer result type")
                    };
                    let difference = ScalarTerm::saturating_integer_subtract(
                        integer_type,
                        value_term(left),
                        value_term(right),
                    )
                    .expect("validator requires exact saturating-subtract operand types");
                    axioms.push(Proposition::Equal(
                        value_term(operation.result.expect_scalar().id),
                        difference,
                    ));
                }
                OperationKind::WrappingIntegerMultiply { left, right } => {
                    let ScalarType::Integer(integer_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        unreachable!("validator requires wrapping-multiply integer result type")
                    };
                    let product = ScalarTerm::wrapping_integer_multiply(
                        integer_type,
                        value_term(left),
                        value_term(right),
                    )
                    .expect("validator requires exact wrapping-multiply operand types");
                    axioms.push(Proposition::Equal(
                        value_term(operation.result.expect_scalar().id),
                        product,
                    ));
                }
                OperationKind::SaturatingIntegerMultiply { left, right } => {
                    let ScalarType::Integer(integer_type) =
                        operation.result.expect_scalar().scalar_type
                    else {
                        unreachable!("validator requires saturating-multiply integer result type")
                    };
                    let product = ScalarTerm::saturating_integer_multiply(
                        integer_type,
                        value_term(left),
                        value_term(right),
                    )
                    .expect("validator requires exact saturating-multiply operand types");
                    axioms.push(Proposition::Equal(
                        value_term(operation.result.expect_scalar().id),
                        product,
                    ));
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

fn canonical_conjunction(mut conjuncts: Vec<Proposition>) -> Proposition {
    match conjuncts.len() {
        0 => Proposition::Truth,
        1 => conjuncts.pop().expect("one conjunct exists"),
        _ => Proposition::Conjunction(conjuncts),
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IntegerOffset {
    Nonnegative(u128),
    Negative(u128),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExactIntegerOffsetOperation {
    Add,
    Subtract,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExactIntegerAffineOperation {
    Add,
    Subtract,
    Multiply,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExactIntegerDivideRemainderTransfer {
    Divide,
    Remainder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExactIntegerIntervalPreimage {
    Interval((i128, i128)),
    Empty,
}

impl IntegerOffset {
    fn from_value(value: IntegerValue) -> Self {
        match value {
            IntegerValue::Unsigned(value) => Self::Nonnegative(value),
            IntegerValue::Signed(value) if value < 0 => Self::Negative(value.unsigned_abs()),
            IntegerValue::Signed(value) => Self::Nonnegative(value as u128),
        }
    }

    fn from_subtrahend(value: IntegerValue) -> Self {
        match Self::from_value(value) {
            Self::Nonnegative(value) => Self::Negative(value),
            Self::Negative(value) => Self::Nonnegative(value),
        }
    }

    fn checked_add(self, right: Self) -> Option<Self> {
        match (self, right) {
            (Self::Nonnegative(left), Self::Nonnegative(right)) => {
                left.checked_add(right).map(Self::Nonnegative)
            }
            (Self::Negative(left), Self::Negative(right)) => {
                left.checked_add(right).map(Self::Negative)
            }
            (Self::Nonnegative(left), Self::Negative(right)) => Some(if left >= right {
                Self::Nonnegative(left - right)
            } else {
                Self::Negative(right - left)
            }),
            (Self::Negative(left), Self::Nonnegative(right)) => Some(if right >= left {
                Self::Nonnegative(right - left)
            } else {
                Self::Negative(left - right)
            }),
        }
    }

    fn checked_multiply(self, factor: u128) -> Option<Self> {
        if factor == 0 {
            return Some(Self::Nonnegative(0));
        }
        match self {
            Self::Nonnegative(value) => value.checked_mul(factor).map(Self::Nonnegative),
            Self::Negative(value) => value.checked_mul(factor).map(Self::Negative),
        }
    }

    fn checked_multiply_value(self, factor: IntegerValue) -> Option<Self> {
        let factor = Self::from_value(factor);
        self.checked_multiply_offset(factor)
    }

    fn checked_multiply_offset(self, factor: Self) -> Option<Self> {
        let product = self.checked_multiply(factor.magnitude())?;
        Some(match factor {
            Self::Negative(_) => product.negated(),
            Self::Nonnegative(_) => product,
        })
    }

    const fn negated(self) -> Self {
        match self {
            Self::Nonnegative(0) | Self::Negative(0) => Self::Nonnegative(0),
            Self::Nonnegative(value) => Self::Negative(value),
            Self::Negative(value) => Self::Nonnegative(value),
        }
    }

    const fn magnitude(self) -> u128 {
        match self {
            Self::Nonnegative(value) | Self::Negative(value) => value,
        }
    }

    fn is_representable(self, integer_type: psi_core::IntegerType) -> bool {
        match (integer_type.sign(), self) {
            (IntegerSign::Unsigned, Self::Nonnegative(value)) => {
                integer_type.admits(IntegerValue::Unsigned(value))
            }
            (IntegerSign::Unsigned, Self::Negative(_)) => false,
            (IntegerSign::Signed, Self::Nonnegative(value)) => i128::try_from(value)
                .ok()
                .is_some_and(|value| integer_type.admits(IntegerValue::Signed(value))),
            (IntegerSign::Signed, Self::Negative(value)) => signed_negative_magnitude(value)
                .is_some_and(|value| integer_type.admits(IntegerValue::Signed(value))),
        }
    }
}

fn checked_signed_integer_product(
    product: Option<IntegerOffset>,
    factor: IntegerValue,
) -> Option<IntegerOffset> {
    if IntegerOffset::from_value(factor).magnitude() == 0 {
        return Some(IntegerOffset::Nonnegative(0));
    }
    product?.checked_multiply_value(factor)
}

fn exact_integer_carrier_total_hull_obligation(
    hull: (i128, i128),
    interval: (i128, i128),
) -> Option<Proposition> {
    if hull.0 >= interval.0 && hull.1 <= interval.1 {
        Some(Proposition::Truth)
    } else if hull.1 < interval.0 || hull.0 > interval.1 {
        Some(Proposition::Falsehood)
    } else {
        None
    }
}

fn fixed_integer_type_interval(integer_type: psi_core::IntegerType) -> Option<(i128, i128)> {
    if integer_type.is_address() || !matches!(integer_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    Some((
        fixed_integer_value(integer_type, integer_type.minimum_value())?,
        fixed_integer_value(integer_type, integer_type.maximum_value())?,
    ))
}

fn fixed_integer_value(integer_type: psi_core::IntegerType, value: IntegerValue) -> Option<i128> {
    match (integer_type.sign(), value) {
        (IntegerSign::Signed, IntegerValue::Signed(value)) => Some(value),
        (IntegerSign::Unsigned, IntegerValue::Unsigned(value)) => i128::try_from(value).ok(),
        _ => None,
    }
}

fn checked_integer_floor_division(dividend: i128, divisor: i128) -> Option<i128> {
    let quotient = dividend.checked_div(divisor)?;
    let remainder = dividend.checked_rem(divisor)?;
    Some(if remainder != 0 && (remainder < 0) != (divisor < 0) {
        quotient.checked_sub(1)?
    } else {
        quotient
    })
}

fn checked_integer_ceil_division(dividend: i128, divisor: i128) -> Option<i128> {
    let quotient = dividend.checked_div(divisor)?;
    let remainder = dividend.checked_rem(divisor)?;
    Some(if remainder != 0 && (remainder < 0) == (divisor < 0) {
        quotient.checked_add(1)?
    } else {
        quotient
    })
}

fn exact_integer_source_interval_obligation(
    root_type: psi_core::IntegerType,
    root: ScalarTerm,
    target_minimum: i128,
    target_maximum: i128,
) -> Proposition {
    let Some(root_minimum) = integer_value_as_i128(root_type.minimum_value()) else {
        return Proposition::Falsehood;
    };
    let Some(root_maximum) = integer_value_as_i128(root_type.maximum_value()) else {
        return Proposition::Falsehood;
    };
    if target_minimum > root_maximum || target_maximum < root_minimum {
        return Proposition::Falsehood;
    }

    let root_boundary = |boundary: i128| {
        let value = match root_type.sign() {
            IntegerSign::Signed => IntegerValue::Signed(boundary),
            IntegerSign::Unsigned => IntegerValue::Unsigned(u128::try_from(boundary).ok()?),
        };
        ScalarTerm::integer(root_type, value).ok()
    };
    let mut bounds = Vec::with_capacity(2);
    if target_minimum > root_minimum {
        let Some(boundary) = root_boundary(target_minimum) else {
            return Proposition::Falsehood;
        };
        bounds.push(Proposition::LessOrEqual(boundary, root.clone()));
    }
    if target_maximum < root_maximum {
        let Some(boundary) = root_boundary(target_maximum) else {
            return Proposition::Falsehood;
        };
        bounds.push(Proposition::LessOrEqual(root, boundary));
    }
    match bounds.len() {
        0 => Proposition::Truth,
        1 => bounds
            .pop()
            .expect("one translated exact-cast bound exists"),
        _ => canonical_conjunction(bounds),
    }
}

fn integer_value_as_i128(value: IntegerValue) -> Option<i128> {
    match value {
        IntegerValue::Signed(value) => Some(value),
        IntegerValue::Unsigned(value) => i128::try_from(value).ok(),
    }
}

fn signed_negative_magnitude(magnitude: u128) -> Option<i128> {
    if magnitude == 1_u128 << 127 {
        Some(i128::MIN)
    } else {
        i128::try_from(magnitude).ok().and_then(i128::checked_neg)
    }
}

fn integer_type_span(integer_type: psi_core::IntegerType) -> u128 {
    if integer_type.bits() == 128 {
        u128::MAX
    } else {
        (1_u128 << integer_type.bits()) - 1
    }
}

fn landed_integer_constant_value(
    integer_type: psi_core::IntegerType,
    term: &ScalarTerm,
    semantic_axioms: &[Proposition],
    prior_axiom_count: usize,
) -> Option<IntegerValue> {
    let (known_type, value) = term.integer_value().or_else(|| {
        semantic_axioms[..prior_axiom_count.min(semantic_axioms.len())]
            .iter()
            .rev()
            .find_map(|axiom| match axiom {
                Proposition::Equal(left, right) if left == term => right.integer_value(),
                _ => None,
            })
    })?;
    (known_type == integer_type && integer_type.admits(value)).then_some(value)
}

fn nonnegative_integer_factor(
    integer_type: psi_core::IntegerType,
    factor: IntegerValue,
) -> Option<u128> {
    match (integer_type.sign(), factor) {
        (IntegerSign::Unsigned, IntegerValue::Unsigned(factor)) => Some(factor),
        (IntegerSign::Signed, IntegerValue::Signed(factor)) => u128::try_from(factor).ok(),
        _ => None,
    }
}

fn known_integer_term_value(
    integer_type: psi_core::IntegerType,
    term: &ScalarTerm,
    semantic_axioms: &[Proposition],
) -> Option<IntegerValue> {
    let (known_type, value) = term.integer_value().or_else(|| {
        semantic_axioms.iter().rev().find_map(|axiom| {
            let Proposition::Equal(left, right) = axiom else {
                return None;
            };
            if left == term {
                right.integer_value()
            } else if right == term {
                left.integer_value()
            } else {
                None
            }
        })
    })?;
    (known_type == integer_type && integer_type.admits(value)).then_some(value)
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

pub(crate) fn substitute_proposition_values(
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
        Proposition::Disjunction(disjuncts) => Proposition::Disjunction(
            disjuncts
                .iter()
                .map(|disjunct| substitute_proposition_values(disjunct, substitutions))
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

pub(crate) fn substitute_proposition_places(
    proposition: &Proposition,
    substitutions: &BTreeMap<PlaceId, PlaceId>,
) -> Proposition {
    let substitutions = substitutions
        .iter()
        .map(|(source, target)| (*source, (*target, Vec::new())))
        .collect::<BTreeMap<_, _>>();
    substitute_proposition_structural_places(proposition, &substitutions)
}

pub(crate) fn substitute_proposition_structural_places(
    proposition: &Proposition,
    substitutions: &BTreeMap<PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
) -> Proposition {
    match proposition {
        Proposition::Truth => Proposition::Truth,
        Proposition::Falsehood => Proposition::Falsehood,
        Proposition::Atom(atom) => Proposition::Atom(*atom),
        Proposition::Equal(left, right) => Proposition::Equal(
            substitute_scalar_term_places(left, substitutions),
            substitute_scalar_term_places(right, substitutions),
        ),
        Proposition::LessThan(left, right) => Proposition::LessThan(
            substitute_scalar_term_places(left, substitutions),
            substitute_scalar_term_places(right, substitutions),
        ),
        Proposition::LessOrEqual(left, right) => Proposition::LessOrEqual(
            substitute_scalar_term_places(left, substitutions),
            substitute_scalar_term_places(right, substitutions),
        ),
        Proposition::Conjunction(conjuncts) => Proposition::Conjunction(
            conjuncts
                .iter()
                .map(|conjunct| substitute_proposition_structural_places(conjunct, substitutions))
                .collect(),
        ),
        Proposition::Disjunction(disjuncts) => Proposition::Disjunction(
            disjuncts
                .iter()
                .map(|disjunct| substitute_proposition_structural_places(disjunct, substitutions))
                .collect(),
        ),
        Proposition::Implication {
            premise,
            conclusion,
        } => Proposition::Implication {
            premise: Box::new(substitute_proposition_structural_places(
                premise,
                substitutions,
            )),
            conclusion: Box::new(substitute_proposition_structural_places(
                conclusion,
                substitutions,
            )),
        },
        Proposition::ContentConservation(conservation) => {
            Proposition::ContentConservation(ContentConservation::new(
                conservation.algebra().clone(),
                substitute_content_term_places(conservation.left(), substitutions),
                substitute_content_term_places(conservation.right(), substitutions),
            ))
        }
    }
}

fn substitute_scalar_term_places(
    term: &ScalarTerm,
    substitutions: &BTreeMap<PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
) -> ScalarTerm {
    let mut term = term.clone();
    fn substitute(
        term: &mut ScalarTerm,
        substitutions: &BTreeMap<PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
    ) {
        match term {
            ScalarTerm::BooleanField { root, path }
            | ScalarTerm::IntegerField { root, path, .. } => {
                if let Some((replacement, prefix)) = substitutions.get(root) {
                    *root = *replacement;
                    if !prefix.is_empty() {
                        let mut rebased = Vec::with_capacity(prefix.len() + path.len());
                        rebased.extend(prefix);
                        rebased.append(path);
                        *path = rebased;
                    }
                }
            }
            ScalarTerm::BooleanNot { operand }
            | ScalarTerm::IntegerBitwiseNot { operand, .. }
            | ScalarTerm::IntegerWiden { operand, .. }
            | ScalarTerm::IntegerExactCast { operand, .. } => substitute(operand, substitutions),
            ScalarTerm::BooleanEqual { left, right }
            | ScalarTerm::IntegerEqual { left, right, .. }
            | ScalarTerm::IntegerLessThan { left, right, .. }
            | ScalarTerm::IntegerLessOrEqual { left, right, .. }
            | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
            | ScalarTerm::IntegerBitwiseOr { left, right, .. }
            | ScalarTerm::IntegerBitwiseXor { left, right, .. }
            | ScalarTerm::ExactIntegerAdd { left, right, .. }
            | ScalarTerm::ExactIntegerSubtract { left, right, .. }
            | ScalarTerm::ExactIntegerMultiply { left, right, .. }
            | ScalarTerm::ExactIntegerDivide { left, right, .. }
            | ScalarTerm::ExactIntegerRemainder { left, right, .. }
            | ScalarTerm::WrappingIntegerDivide { left, right, .. }
            | ScalarTerm::WrappingIntegerRemainder { left, right, .. }
            | ScalarTerm::SaturatingIntegerDivide { left, right, .. }
            | ScalarTerm::SaturatingIntegerRemainder { left, right, .. }
            | ScalarTerm::WrappingIntegerAdd { left, right, .. }
            | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
            | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
            | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
            | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
            | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
                substitute(left, substitutions);
                substitute(right, substitutions);
            }
            ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
            | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
            | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
            | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
                substitute(value, substitutions);
                substitute(count, substitutions);
            }
            ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => {}
        }
    }
    substitute(&mut term, substitutions);
    term
}

fn substitute_content_term_places(
    term: &ContentTerm,
    substitutions: &BTreeMap<PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
) -> ContentTerm {
    match term {
        ContentTerm::Projection {
            projection,
            subject,
        } => ContentTerm::Projection {
            projection: *projection,
            subject: ContentStructuralPlace {
                version: subject.version,
                root: substitutions
                    .get(&subject.root)
                    .map(|(root, _)| *root)
                    .unwrap_or(subject.root),
                segments: subject.segments.clone(),
            },
        },
        ContentTerm::Separate(terms) => ContentTerm::Separate(
            terms
                .iter()
                .map(|term| substitute_content_term_places(term, substitutions))
                .collect(),
        ),
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
        ScalarTerm::BooleanField { .. }
        | ScalarTerm::IntegerField { .. }
        | ScalarTerm::Boolean(_)
        | ScalarTerm::Integer { .. } => term.clone(),
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
        ScalarTerm::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        } => ScalarTerm::ExactIntegerAdd {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
        } => ScalarTerm::ExactIntegerSubtract {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
        } => ScalarTerm::ExactIntegerMultiply {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::ExactIntegerDivide {
            scalar_type,
            left,
            right,
        } => ScalarTerm::ExactIntegerDivide {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::ExactIntegerRemainder {
            scalar_type,
            left,
            right,
        } => ScalarTerm::ExactIntegerRemainder {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::WrappingIntegerDivide {
            scalar_type,
            left,
            right,
        } => ScalarTerm::WrappingIntegerDivide {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::WrappingIntegerRemainder {
            scalar_type,
            left,
            right,
        } => ScalarTerm::WrappingIntegerRemainder {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::SaturatingIntegerDivide {
            scalar_type,
            left,
            right,
        } => ScalarTerm::SaturatingIntegerDivide {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::SaturatingIntegerRemainder {
            scalar_type,
            left,
            right,
        } => ScalarTerm::SaturatingIntegerRemainder {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
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
    NonDenseEvidenceProducer {
        expected: EvidenceIdentity,
        actual: EvidenceIdentity,
    },
    NonCanonicalEvidenceProducerOrder,
    DuplicateEvidenceProducerTerm(EvidenceTermId),
    UnknownEvidenceProducerTerm(EvidenceTermId),
    UnusedEvidenceProducerTerm(EvidenceTermId),
    MissingEvidenceProducer(EvidenceTermId),
    InvalidEvidenceProducer(EvidenceIdentity),
    EvidenceProducerInterfaceMismatch(EvidenceTermId),
    NonCanonicalEvidenceProducerRows(EvidenceIdentity),
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

#[cfg(test)]
mod tests;
