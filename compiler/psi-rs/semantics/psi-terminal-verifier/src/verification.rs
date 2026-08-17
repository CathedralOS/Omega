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
mod integer_divide_remainder;
mod integer_multiply;

use integer_add_subtract::{exact_integer_add_obligation, exact_integer_subtract_obligation};
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

fn exact_integer_cast_obligation(
    source_type: psi_core::IntegerType,
    target_type: psi_core::IntegerType,
    operand: ScalarTerm,
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Proposition {
    if let Some(obligation) = exact_integer_cast_chain_obligation(
        source_type,
        target_type,
        operand.clone(),
        semantic_axioms,
        machine_parameter_values,
    ) {
        return obligation;
    }
    if let Some(obligation) = exact_integer_computed_prefix_cast_chain_obligation(
        source_type,
        target_type,
        operand.clone(),
        semantic_axioms,
        machine_parameter_values,
    ) {
        return obligation;
    }
    if let Some(obligation) = exact_integer_divide_remainder_chain_cast_obligation(
        source_type,
        target_type,
        operand.clone(),
        semantic_axioms,
        machine_parameter_values,
    ) {
        return obligation;
    }
    if let Some(obligation) = exact_integer_mixed_shift_chain_cast_obligation(
        source_type,
        target_type,
        operand.clone(),
        semantic_axioms,
        machine_parameter_values,
    ) {
        return obligation;
    }
    if let Some(obligation) = exact_integer_shift_right_chain_cast_obligation(
        source_type,
        target_type,
        operand.clone(),
        semantic_axioms,
        machine_parameter_values,
    ) {
        return obligation;
    }
    if let Some(obligation) = exact_integer_shift_left_chain_cast_obligation(
        source_type,
        target_type,
        operand.clone(),
        semantic_axioms,
        machine_parameter_values,
    ) {
        return obligation;
    }
    if let Some(obligation) = exact_integer_affine_chain_cast_obligation(
        source_type,
        target_type,
        operand.clone(),
        semantic_axioms,
        machine_parameter_values,
    ) {
        return obligation;
    }
    if let Some(obligation) = exact_integer_multiply_chain_cast_obligation(
        source_type,
        target_type,
        operand.clone(),
        semantic_axioms,
        machine_parameter_values,
    ) {
        return obligation;
    }
    if let Some(obligation) = exact_integer_signed_multiply_chain_cast_obligation(
        source_type,
        target_type,
        operand.clone(),
        semantic_axioms,
        machine_parameter_values,
    ) {
        return obligation;
    }
    if let Some(obligation) = exact_integer_signed_affine_chain_cast_obligation(
        source_type,
        target_type,
        operand.clone(),
        semantic_axioms,
        machine_parameter_values,
    ) {
        return obligation;
    }
    if let Some(obligation) = exact_integer_offset_chain_cast_obligation(
        source_type,
        target_type,
        operand.clone(),
        semantic_axioms,
        machine_parameter_values,
    ) {
        return obligation;
    }
    let roundtrip = {
        let mut current = &operand;
        let mut expected_widened_type = source_type;
        let mut prior_axiom_count = semantic_axioms.len();
        let mut established = false;
        for _ in 0..semantic_axioms.len() {
            let Some((definition_index, definition)) = semantic_axioms[..prior_axiom_count]
                .iter()
                .enumerate()
                .rev()
                .find_map(|(index, axiom)| match axiom {
                    Proposition::Equal(left, right) if left == current => Some((index, right)),
                    _ => None,
                })
            else {
                break;
            };
            let ScalarTerm::IntegerWiden {
                source_type: original_type,
                target_type: widened_type,
                operand: original_operand,
            } = definition
            else {
                break;
            };
            let ScalarTerm::Value {
                id: original_value,
                scalar_type: ScalarType::Integer(original_operand_type),
            } = original_operand.as_ref()
            else {
                break;
            };
            if *widened_type != expected_widened_type
                || *original_operand_type != *original_type
                || !original_type.can_widen_to(*widened_type)
            {
                break;
            }
            if *original_type == target_type {
                established = machine_parameter_values.contains(original_value);
                break;
            }
            current = original_operand;
            expected_widened_type = *original_type;
            prior_axiom_count = definition_index;
        }
        established
    };
    if roundtrip {
        return Proposition::Truth;
    }
    if let Some(obligation) = exact_integer_computed_prefix_mixed_conversion_chain_cast_obligation(
        source_type,
        target_type,
        operand.clone(),
        semantic_axioms,
        machine_parameter_values,
    ) {
        return obligation;
    }
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

fn exact_integer_cast_chain_obligation(
    source_type: psi_core::IntegerType,
    target_type: psi_core::IntegerType,
    mut operand: ScalarTerm,
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if !partial_fixed_native_integer_cast(source_type, target_type) {
        return None;
    }
    let source_interval = fixed_integer_type_interval(source_type)?;
    let target_interval = fixed_integer_type_interval(target_type)?;
    let mut interval = (
        source_interval.0.max(target_interval.0),
        source_interval.1.min(target_interval.1),
    );
    let mut expected_target = source_type;
    let mut prior_axiom_count = semantic_axioms.len();
    let mut followed_nested_cast = false;
    for _ in 0..prior_axiom_count {
        let Some((definition_index, definition)) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &operand => Some((index, right)),
                _ => None,
            })
        else {
            break;
        };
        let ScalarTerm::IntegerExactCast {
            source_type: nested_source,
            target_type: nested_target,
            operand: nested_operand,
        } = definition
        else {
            return None;
        };
        if *nested_target != expected_target
            || !partial_fixed_native_integer_cast(*nested_source, *nested_target)
        {
            return None;
        }
        let nested_interval = fixed_integer_type_interval(*nested_source)?;
        interval.0 = interval.0.max(nested_interval.0);
        interval.1 = interval.1.min(nested_interval.1);
        operand = (**nested_operand).clone();
        expected_target = *nested_source;
        prior_axiom_count = definition_index;
        followed_nested_cast = true;
    }
    if !followed_nested_cast
        || !matches!(
            &operand,
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == expected_target && machine_parameter_values.contains(id)
        )
    {
        return None;
    }
    if interval.0 > interval.1 {
        return Some(Proposition::Falsehood);
    }
    Some(exact_integer_source_interval_obligation(
        expected_target,
        operand,
        interval.0,
        interval.1,
    ))
}

fn exact_integer_computed_prefix_cast_chain_obligation(
    source_type: psi_core::IntegerType,
    target_type: psi_core::IntegerType,
    mut operand: ScalarTerm,
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if !partial_fixed_native_integer_cast(source_type, target_type) {
        return None;
    }
    let source_interval = fixed_integer_type_interval(source_type)?;
    let target_interval = fixed_integer_type_interval(target_type)?;
    let mut interval = (
        source_interval.0.max(target_interval.0),
        source_interval.1.min(target_interval.1),
    );
    let mut expected_target = source_type;
    let mut prior_axiom_count = semantic_axioms.len();
    let mut followed_nested_cast = false;
    for _ in 0..prior_axiom_count {
        let Some((definition_index, definition)) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &operand => Some((index, right)),
                _ => None,
            })
        else {
            break;
        };
        let ScalarTerm::IntegerExactCast {
            source_type: nested_source,
            target_type: nested_target,
            operand: nested_operand,
        } = definition
        else {
            break;
        };
        if *nested_target != expected_target
            || !partial_fixed_native_integer_cast(*nested_source, *nested_target)
        {
            return None;
        }
        let nested_interval = fixed_integer_type_interval(*nested_source)?;
        interval.0 = interval.0.max(nested_interval.0);
        interval.1 = interval.1.min(nested_interval.1);
        operand = (**nested_operand).clone();
        expected_target = *nested_source;
        prior_axiom_count = definition_index;
        followed_nested_cast = true;
    }
    if !followed_nested_cast {
        return None;
    }
    if interval.0 > interval.1 {
        return Some(Proposition::Falsehood);
    }
    let definition =
        semantic_axioms[..prior_axiom_count]
            .iter()
            .rev()
            .find_map(|axiom| match axiom {
                Proposition::Equal(left, right) if left == &operand => Some(right),
                _ => None,
            })?;
    match definition {
        ScalarTerm::ExactIntegerDivide { .. } | ScalarTerm::ExactIntegerRemainder { .. } => {
            let hull = exact_integer_divide_remainder_chain_hull(
                expected_target,
                operand,
                semantic_axioms,
                prior_axiom_count,
                machine_parameter_values,
            )?;
            (hull.0 >= interval.0 && hull.1 <= interval.1).then_some(Proposition::Truth)
        }
        ScalarTerm::ExactIntegerShiftLeft { .. } | ScalarTerm::ExactIntegerShiftRight { .. } => {
            exact_integer_shift_prefix_interval_obligation(
                expected_target,
                operand,
                interval,
                semantic_axioms,
                prior_axiom_count,
                machine_parameter_values,
            )
        }
        ScalarTerm::ExactIntegerAdd { .. } | ScalarTerm::ExactIntegerSubtract { .. } => {
            exact_integer_affine_prefix_interval_obligation(
                expected_target,
                operand,
                interval,
                semantic_axioms,
                prior_axiom_count,
                machine_parameter_values,
            )
        }
        ScalarTerm::ExactIntegerMultiply { .. } => exact_integer_affine_prefix_interval_obligation(
            expected_target,
            operand.clone(),
            interval,
            semantic_axioms,
            prior_axiom_count,
            machine_parameter_values,
        )
        .or_else(|| {
            exact_integer_signed_product_prefix_interval_obligation(
                expected_target,
                operand,
                interval,
                semantic_axioms,
                prior_axiom_count,
                machine_parameter_values,
            )
        }),
        _ => None,
    }
}

fn exact_integer_computed_prefix_cast_chain_interval_obligation(
    target_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    requested_interval: (i128, i128),
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    let mut cast_interval = fixed_integer_type_interval(target_type)?;
    let mut expected_target = target_type;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let mut cast_count = 0_usize;
    for _ in 0..=prior_axiom_count {
        let Some((definition_index, definition)) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })
        else {
            break;
        };
        let ScalarTerm::IntegerExactCast {
            source_type,
            target_type: cast_target,
            operand,
        } = definition
        else {
            break;
        };
        if *cast_target != expected_target
            || !partial_fixed_native_integer_cast(*source_type, *cast_target)
        {
            return None;
        }
        let source_interval = fixed_integer_type_interval(*source_type)?;
        cast_interval.0 = cast_interval.0.max(source_interval.0);
        cast_interval.1 = cast_interval.1.min(source_interval.1);
        value = (**operand).clone();
        expected_target = *source_type;
        prior_axiom_count = definition_index;
        cast_count += 1;
    }
    if cast_count < 2 {
        return None;
    }
    let interval = (
        requested_interval.0.max(cast_interval.0),
        requested_interval.1.min(cast_interval.1),
    );
    let definition =
        semantic_axioms[..prior_axiom_count]
            .iter()
            .rev()
            .find_map(|axiom| match axiom {
                Proposition::Equal(left, right) if left == &value => Some(right),
                _ => None,
            })?;
    if matches!(
        definition,
        ScalarTerm::ExactIntegerDivide { .. } | ScalarTerm::ExactIntegerRemainder { .. }
    ) {
        let hull = exact_integer_divide_remainder_chain_hull(
            expected_target,
            value,
            semantic_axioms,
            prior_axiom_count,
            machine_parameter_values,
        )?;
        if hull.0 < cast_interval.0 || hull.1 > cast_interval.1 {
            return None;
        }
        return exact_integer_carrier_total_hull_obligation(hull, interval);
    }
    exact_integer_computed_prefix_interval_obligation(
        expected_target,
        value,
        interval,
        semantic_axioms,
        prior_axiom_count,
        machine_parameter_values,
    )
}

fn exact_integer_computed_prefix_interval_obligation(
    value_type: psi_core::IntegerType,
    value: ScalarTerm,
    interval: (i128, i128),
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    let definition = semantic_axioms[..definition_axiom_count.min(semantic_axioms.len())]
        .iter()
        .rev()
        .find_map(|axiom| match axiom {
            Proposition::Equal(left, right) if left == &value => Some(right),
            _ => None,
        })?;
    match definition {
        ScalarTerm::ExactIntegerDivide { .. } | ScalarTerm::ExactIntegerRemainder { .. } => {
            let hull = exact_integer_divide_remainder_chain_hull(
                value_type,
                value,
                semantic_axioms,
                definition_axiom_count,
                machine_parameter_values,
            )?;
            exact_integer_carrier_total_hull_obligation(hull, interval)
        }
        ScalarTerm::ExactIntegerShiftLeft { .. } | ScalarTerm::ExactIntegerShiftRight { .. } => {
            exact_integer_shift_prefix_interval_obligation(
                value_type,
                value,
                interval,
                semantic_axioms,
                definition_axiom_count,
                machine_parameter_values,
            )
        }
        ScalarTerm::ExactIntegerAdd { .. } | ScalarTerm::ExactIntegerSubtract { .. } => {
            exact_integer_affine_prefix_interval_obligation(
                value_type,
                value,
                interval,
                semantic_axioms,
                definition_axiom_count,
                machine_parameter_values,
            )
        }
        ScalarTerm::ExactIntegerMultiply { .. } => exact_integer_affine_prefix_interval_obligation(
            value_type,
            value.clone(),
            interval,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
        .or_else(|| {
            exact_integer_signed_product_prefix_interval_obligation(
                value_type,
                value,
                interval,
                semantic_axioms,
                definition_axiom_count,
                machine_parameter_values,
            )
        }),
        _ => None,
    }
}

fn exact_integer_computed_prefix_widen_chain_interval_obligation(
    target_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    requested_interval: (i128, i128),
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    let mut expected_target = target_type;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let mut widen_count = 0_usize;
    for _ in 0..=prior_axiom_count {
        let Some((definition_index, definition)) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })
        else {
            break;
        };
        let ScalarTerm::IntegerWiden {
            source_type,
            target_type: widen_target,
            operand,
        } = definition
        else {
            break;
        };
        if *widen_target != expected_target || !source_type.can_widen_to(*widen_target) {
            return None;
        }
        value = (**operand).clone();
        expected_target = *source_type;
        prior_axiom_count = definition_index;
        widen_count += 1;
    }
    if widen_count == 0 {
        return None;
    }
    let source_interval = fixed_integer_type_interval(expected_target)?;
    let interval = (
        requested_interval.0.max(source_interval.0),
        requested_interval.1.min(source_interval.1),
    );
    exact_integer_computed_prefix_interval_obligation(
        expected_target,
        value,
        interval,
        semantic_axioms,
        prior_axiom_count,
        machine_parameter_values,
    )
}

fn exact_integer_computed_prefix_mixed_conversion_chain_cast_obligation(
    source_type: psi_core::IntegerType,
    target_type: psi_core::IntegerType,
    mut operand: ScalarTerm,
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if !partial_fixed_native_integer_cast(source_type, target_type) {
        return None;
    }
    let source_interval = fixed_integer_type_interval(source_type)?;
    let target_interval = fixed_integer_type_interval(target_type)?;
    let mut interval = (
        source_interval.0.max(target_interval.0),
        source_interval.1.min(target_interval.1),
    );
    let mut expected_target = source_type;
    let mut prior_axiom_count = semantic_axioms.len();
    let mut saw_widen = false;
    for _ in 0..=prior_axiom_count {
        let Some((definition_index, definition)) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &operand => Some((index, right)),
                _ => None,
            })
        else {
            break;
        };
        let (nested_source, nested_operand) = match definition {
            ScalarTerm::IntegerWiden {
                source_type: nested_source,
                target_type: nested_target,
                operand: nested_operand,
            } if *nested_target == expected_target
                && nested_source.can_widen_to(*nested_target) =>
            {
                saw_widen = true;
                (*nested_source, nested_operand)
            }
            ScalarTerm::IntegerExactCast {
                source_type: nested_source,
                target_type: nested_target,
                operand: nested_operand,
            } if *nested_target == expected_target
                && partial_fixed_native_integer_cast(*nested_source, *nested_target) =>
            {
                (*nested_source, nested_operand)
            }
            ScalarTerm::IntegerWiden { .. } | ScalarTerm::IntegerExactCast { .. } => return None,
            _ => break,
        };
        let nested_interval = fixed_integer_type_interval(nested_source)?;
        interval.0 = interval.0.max(nested_interval.0);
        interval.1 = interval.1.min(nested_interval.1);
        operand = (**nested_operand).clone();
        expected_target = nested_source;
        prior_axiom_count = definition_index;
    }
    if !saw_widen {
        return None;
    }
    let definition =
        semantic_axioms[..prior_axiom_count]
            .iter()
            .rev()
            .find_map(|axiom| match axiom {
                Proposition::Equal(left, right) if left == &operand => Some(right),
                _ => None,
            })?;
    if matches!(
        definition,
        ScalarTerm::ExactIntegerDivide { .. } | ScalarTerm::ExactIntegerRemainder { .. }
    ) {
        let hull = exact_integer_divide_remainder_chain_hull(
            expected_target,
            operand,
            semantic_axioms,
            prior_axiom_count,
            machine_parameter_values,
        )?;
        return (hull.0 >= interval.0 && hull.1 <= interval.1).then_some(Proposition::Truth);
    }
    exact_integer_computed_prefix_interval_obligation(
        expected_target,
        operand,
        interval,
        semantic_axioms,
        prior_axiom_count,
        machine_parameter_values,
    )
}

fn exact_integer_computed_prefix_mixed_conversion_chain_interval_obligation(
    target_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    requested_interval: (i128, i128),
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    let mut interval = fixed_integer_type_interval(target_type)?;
    let mut expected_target = target_type;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let mut saw_widen = false;
    let mut saw_cast = false;
    for _ in 0..=prior_axiom_count {
        let Some((definition_index, definition)) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })
        else {
            break;
        };
        let (source_type, operand) = match definition {
            ScalarTerm::IntegerWiden {
                source_type,
                target_type: conversion_target,
                operand,
            } if *conversion_target == expected_target
                && source_type.can_widen_to(*conversion_target) =>
            {
                saw_widen = true;
                (*source_type, operand)
            }
            ScalarTerm::IntegerExactCast {
                source_type,
                target_type: conversion_target,
                operand,
            } if *conversion_target == expected_target
                && partial_fixed_native_integer_cast(*source_type, *conversion_target) =>
            {
                saw_cast = true;
                (*source_type, operand)
            }
            ScalarTerm::IntegerWiden { .. } | ScalarTerm::IntegerExactCast { .. } => return None,
            _ => break,
        };
        let source_interval = fixed_integer_type_interval(source_type)?;
        interval.0 = interval.0.max(source_interval.0);
        interval.1 = interval.1.min(source_interval.1);
        value = (**operand).clone();
        expected_target = source_type;
        prior_axiom_count = definition_index;
    }
    if !saw_widen || !saw_cast {
        return None;
    }
    let interval = (
        requested_interval.0.max(interval.0),
        requested_interval.1.min(interval.1),
    );
    exact_integer_computed_prefix_interval_obligation(
        expected_target,
        value,
        interval,
        semantic_axioms,
        prior_axiom_count,
        machine_parameter_values,
    )
}

fn exact_integer_computed_prefix_conversion_interval_obligation(
    target_type: psi_core::IntegerType,
    value: ScalarTerm,
    requested_interval: (i128, i128),
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    exact_integer_computed_prefix_cast_chain_interval_obligation(
        target_type,
        value.clone(),
        requested_interval,
        semantic_axioms,
        definition_axiom_count,
        machine_parameter_values,
    )
    .or_else(|| {
        exact_integer_computed_prefix_widen_chain_interval_obligation(
            target_type,
            value.clone(),
            requested_interval,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    })
    .or_else(|| {
        exact_integer_computed_prefix_mixed_conversion_chain_interval_obligation(
            target_type,
            value,
            requested_interval,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    })
}

fn exact_integer_affine_prefix_interval_obligation(
    value_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    interval: (i128, i128),
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    let mut coefficient = 1_u128;
    let mut offset = IntegerOffset::Nonnegative(0);
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let mut followed_definition = false;
    for _ in 0..=prior_axiom_count {
        if followed_definition
            && matches!(
                &value,
                ScalarTerm::Value {
                    id,
                    scalar_type: ScalarType::Integer(root_type),
                } if *root_type == value_type && machine_parameter_values.contains(id)
            )
        {
            return exact_integer_affine_preimage_obligation(
                value_type,
                value,
                coefficient,
                offset,
                interval,
            )
            .ok();
        }
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })?;
        let (left, right, nested_coefficient, nested_offset) = match definition {
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            } if *scalar_type == value_type => (
                left,
                right,
                1,
                IntegerOffset::from_value(landed_integer_constant_value(
                    value_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?),
            ),
            ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } if *scalar_type == value_type => (
                left,
                right,
                1,
                IntegerOffset::from_subtrahend(landed_integer_constant_value(
                    value_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?),
            ),
            ScalarTerm::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } if *scalar_type == value_type => (
                left,
                right,
                nonnegative_integer_factor(
                    value_type,
                    landed_integer_constant_value(
                        value_type,
                        right,
                        semantic_axioms,
                        definition_index,
                    )?,
                )?,
                IntegerOffset::Nonnegative(0),
            ),
            _ => return None,
        };
        if landed_integer_constant_value(value_type, left, semantic_axioms, definition_index)
            .is_some()
            || landed_integer_constant_value(value_type, right, semantic_axioms, definition_index)
                .is_none()
        {
            return None;
        }
        offset = nested_offset
            .checked_multiply(coefficient)
            .and_then(|nested| nested.checked_add(offset))?;
        coefficient = coefficient.checked_mul(nested_coefficient)?;
        value = (**left).clone();
        prior_axiom_count = definition_index;
        followed_definition = true;
    }
    None
}

fn exact_integer_shift_prefix_interval_obligation(
    value_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    mut interval: (i128, i128),
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let mut followed_definition = false;
    for _ in 0..=prior_axiom_count {
        if followed_definition
            && matches!(
                &value,
                ScalarTerm::Value {
                    id,
                    scalar_type: ScalarType::Integer(root_type),
                } if *root_type == value_type && machine_parameter_values.contains(id)
            )
        {
            return Some(exact_integer_source_interval_obligation(
                value_type, value, interval.0, interval.1,
            ));
        }
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })?;
        let (nested_value, count_type, count) = match definition {
            ScalarTerm::ExactIntegerShiftLeft {
                value_type: nested_value_type,
                count_type,
                value,
                count,
            }
            | ScalarTerm::ExactIntegerShiftRight {
                value_type: nested_value_type,
                count_type,
                value,
                count,
            } if *nested_value_type == value_type => (value, *count_type, count),
            _ => return None,
        };
        let count = landed_exact_shift_count(
            value_type,
            count_type,
            count,
            semantic_axioms,
            definition_index,
        )?;
        interval = match exact_integer_mixed_shift_preimage(value_type, interval, definition, count)
        {
            Ok(Some(interval)) => interval,
            Ok(None) => return Some(Proposition::Falsehood),
            Err(()) => return None,
        };
        value = (**nested_value).clone();
        prior_axiom_count = definition_index;
        followed_definition = true;
    }
    None
}

fn exact_integer_signed_product_prefix_interval_obligation(
    value_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    interval: (i128, i128),
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if value_type.sign() != IntegerSign::Signed {
        return None;
    }
    let mut product = Some(IntegerOffset::Nonnegative(1));
    let mut saw_negative = false;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let mut followed_definition = false;
    for _ in 0..=prior_axiom_count {
        if followed_definition
            && saw_negative
            && matches!(
                &value,
                ScalarTerm::Value {
                    id,
                    scalar_type: ScalarType::Integer(root_type),
                } if *root_type == value_type && machine_parameter_values.contains(id)
            )
        {
            return exact_integer_signed_product_interval_preimage_obligation(
                value_type, value, product?, interval,
            );
        }
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })?;
        let ScalarTerm::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
        } = definition
        else {
            return None;
        };
        if *scalar_type != value_type
            || landed_integer_constant_value(value_type, left, semantic_axioms, definition_index)
                .is_some()
        {
            return None;
        }
        let factor =
            landed_integer_constant_value(value_type, right, semantic_axioms, definition_index)?;
        let IntegerValue::Signed(factor_value) = factor else {
            return None;
        };
        product = checked_signed_integer_product(product, factor);
        saw_negative |= factor_value < 0;
        value = (**left).clone();
        prior_axiom_count = definition_index;
        followed_definition = true;
    }
    None
}

fn exact_integer_signed_product_interval_preimage_obligation(
    root_type: psi_core::IntegerType,
    root: ScalarTerm,
    product: IntegerOffset,
    interval: (i128, i128),
) -> Option<Proposition> {
    if product.magnitude() == 0 {
        return Some(if interval.0 <= 0 && 0 <= interval.1 {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        });
    }
    let magnitude = product.magnitude();
    let (minimum, maximum) = if magnitude > i128::MAX as u128 {
        (0, 0)
    } else {
        let magnitude = i128::try_from(magnitude).ok()?;
        let signed_product = match product {
            IntegerOffset::Nonnegative(_) => magnitude,
            IntegerOffset::Negative(_) => magnitude.checked_neg()?,
        };
        if signed_product > 0 {
            (
                checked_integer_ceil_division(interval.0, signed_product)?,
                checked_integer_floor_division(interval.1, signed_product)?,
            )
        } else {
            (
                checked_integer_ceil_division(interval.1, signed_product)?,
                checked_integer_floor_division(interval.0, signed_product)?,
            )
        }
    };
    Some(exact_integer_source_interval_obligation(
        root_type, root, minimum, maximum,
    ))
}

fn partial_fixed_native_integer_cast(
    source: psi_core::IntegerType,
    target: psi_core::IntegerType,
) -> bool {
    fixed_integer_type_interval(source).is_some()
        && fixed_integer_type_interval(target).is_some()
        && source != target
        && source.can_exact_cast_to(target)
        && !source.can_widen_to(target)
}

fn exact_integer_cast_chain_root_interval(
    target_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<(psi_core::IntegerType, ScalarTerm, (i128, i128))> {
    let mut interval = fixed_integer_type_interval(target_type)?;
    let mut expected_target_type = target_type;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let mut cast_count = 0_usize;
    for _ in 0..=prior_axiom_count {
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })?;
        let ScalarTerm::IntegerExactCast {
            source_type,
            target_type: cast_target_type,
            operand,
        } = definition
        else {
            return None;
        };
        if *cast_target_type != expected_target_type
            || !partial_fixed_native_integer_cast(*source_type, *cast_target_type)
        {
            return None;
        }
        let source_interval = fixed_integer_type_interval(*source_type)?;
        interval.0 = interval.0.max(source_interval.0);
        interval.1 = interval.1.min(source_interval.1);
        cast_count += 1;
        value = (**operand).clone();
        expected_target_type = *source_type;
        prior_axiom_count = definition_index;
        if matches!(
            &value,
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if cast_count >= 2
                && *root_type == expected_target_type
                && machine_parameter_values.contains(id)
        ) {
            return Some((expected_target_type, value, interval));
        }
    }
    None
}

fn exact_integer_mixed_shift_chain_cast_obligation(
    source_type: psi_core::IntegerType,
    target_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if source_type.is_address()
        || target_type.is_address()
        || !matches!(source_type.bits(), 8 | 16 | 32 | 64)
        || !matches!(target_type.bits(), 8 | 16 | 32 | 64)
        || source_type == target_type
        || source_type.can_widen_to(target_type)
        || !source_type.can_exact_cast_to(target_type)
    {
        return None;
    }
    let source_interval = fixed_integer_type_interval(source_type)?;
    let target_interval = fixed_integer_type_interval(target_type)?;
    let minimum = source_interval.0.max(target_interval.0);
    let maximum = source_interval.1.min(target_interval.1);
    if minimum > maximum {
        return Some(Proposition::Falsehood);
    }
    let mut interval = (minimum, maximum);
    let mut prior_axiom_count = semantic_axioms.len();
    let mut saw_left = false;
    let mut saw_right = false;
    for _ in 0..=prior_axiom_count {
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })?;
        let (nested_value, count_type, count) = match definition {
            ScalarTerm::ExactIntegerShiftLeft {
                value_type,
                count_type,
                value,
                count,
            } if *value_type == source_type => {
                saw_left = true;
                (value, *count_type, count)
            }
            ScalarTerm::ExactIntegerShiftRight {
                value_type,
                count_type,
                value,
                count,
            } if *value_type == source_type => {
                saw_right = true;
                (value, *count_type, count)
            }
            _ => return None,
        };
        let count = landed_exact_shift_count(
            source_type,
            count_type,
            count,
            semantic_axioms,
            definition_index,
        )?;
        interval =
            match exact_integer_mixed_shift_preimage(source_type, interval, definition, count) {
                Ok(Some(interval)) => interval,
                Ok(None) => return Some(Proposition::Falsehood),
                Err(()) => return None,
            };
        value = (**nested_value).clone();
        prior_axiom_count = definition_index;
        if saw_left
            && saw_right
            && matches!(
                &value,
                ScalarTerm::Value {
                    id,
                    scalar_type: ScalarType::Integer(root_type),
                } if *root_type == source_type && machine_parameter_values.contains(id)
            )
        {
            return Some(exact_integer_source_interval_obligation(
                source_type,
                value,
                interval.0,
                interval.1,
            ));
        }
    }
    None
}

fn exact_integer_shift_obligation(
    value_type: psi_core::IntegerType,
    count_type: psi_core::IntegerType,
    count: ScalarTerm,
    semantic_axioms: &[Proposition],
) -> Proposition {
    if let Some(count) = known_integer_term_value(count_type, &count, semantic_axioms) {
        let count = match count {
            IntegerValue::Signed(count) => u128::try_from(count).ok(),
            IntegerValue::Unsigned(count) => Some(count),
        };
        return if count.is_some_and(|count| count < u128::from(value_type.bits())) {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        };
    }
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
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Proposition {
    if let Some(count_value) = landed_exact_shift_count(
        value_type,
        count_type,
        &count,
        semantic_axioms,
        definition_axiom_count,
    ) {
        if let Some(obligation) = exact_integer_mixed_shift_chain_obligation(
            value_type,
            value.clone(),
            count_value,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        ) {
            return obligation;
        }
        if let Some(obligation) = exact_integer_shift_cast_shift_obligation(
            value_type,
            value.clone(),
            count_value,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        ) {
            return obligation;
        }
        if let Some(obligation) = exact_integer_affine_cast_shift_obligation(
            value_type,
            value.clone(),
            count_value,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        ) {
            return obligation;
        }
        if let Some(obligation) = exact_integer_divide_remainder_then_shift_obligation(
            value_type,
            value.clone(),
            count_value,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        ) {
            return obligation;
        }
        if let Some(obligation) = exact_integer_divide_remainder_cast_shift_obligation(
            value_type,
            value.clone(),
            count_value,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        ) {
            return obligation;
        }
        if let Some(obligation) = exact_integer_cast_chain_then_shift_suffix_obligation(
            value_type,
            value.clone(),
            count_value,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        ) {
            return obligation;
        }
        if let Some(obligation) = exact_integer_cast_then_mixed_shift_chain_obligation(
            value_type,
            value.clone(),
            count_value,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        ) {
            return obligation;
        }
        if let Some(obligation) = exact_integer_arithmetic_then_shift_chain_obligation(
            value_type,
            value.clone(),
            count_value,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        ) {
            return obligation;
        }
        if let Some(obligation) = exact_integer_cast_then_shift_left_chain_obligation(
            value_type,
            value.clone(),
            count_value,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        ) {
            return obligation;
        }
        if let Some(obligation) = exact_integer_shift_left_chain_obligation(
            value_type,
            value.clone(),
            count_value,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        ) {
            return obligation;
        }
    }
    if let Some(count) = exact_known_shift_count(value_type, count_type, &count, semantic_axioms) {
        let mut bounds = Vec::with_capacity(2);
        append_exact_shift_left_value_bounds(&mut bounds, value_type, value, count);
        return canonical_conjunction(bounds);
    }

    let count_bounds =
        exact_integer_shift_obligation(value_type, count_type, count.clone(), semantic_axioms);
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

fn exact_integer_cast_then_mixed_shift_chain_obligation(
    value_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    count: u128,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if value_type.is_address() || !matches!(value_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let Some(mut interval) = exact_integer_shift_left_input_interval(value_type, count) else {
        return Some(Proposition::Falsehood);
    };
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let mut saw_right = false;
    for _ in 0..=prior_axiom_count {
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })?;
        match definition {
            definition @ ScalarTerm::ExactIntegerShiftLeft {
                value_type: nested_value_type,
                count_type,
                value: nested_value,
                count: nested_count,
            }
            | definition @ ScalarTerm::ExactIntegerShiftRight {
                value_type: nested_value_type,
                count_type,
                value: nested_value,
                count: nested_count,
            } if *nested_value_type == value_type => {
                saw_right |= matches!(definition, ScalarTerm::ExactIntegerShiftRight { .. });
                let nested_count = landed_exact_shift_count(
                    value_type,
                    *count_type,
                    nested_count,
                    semantic_axioms,
                    definition_index,
                )?;
                interval = match exact_integer_mixed_shift_preimage(
                    value_type,
                    interval,
                    definition,
                    nested_count,
                ) {
                    Ok(Some(interval)) => interval,
                    Ok(None) => return Some(Proposition::Falsehood),
                    Err(()) => return None,
                };
                value = (**nested_value).clone();
                prior_axiom_count = definition_index;
            }
            ScalarTerm::IntegerExactCast {
                source_type,
                target_type,
                operand,
            } if saw_right
                && *target_type == value_type
                && !source_type.is_address()
                && matches!(source_type.bits(), 8 | 16 | 32 | 64)
                && *source_type != value_type
                && !source_type.can_widen_to(value_type)
                && source_type.can_exact_cast_to(value_type)
                && matches!(
                    operand.as_ref(),
                    ScalarTerm::Value {
                        id,
                        scalar_type: ScalarType::Integer(root_type),
                    } if *root_type == *source_type && machine_parameter_values.contains(id)
                ) =>
            {
                let source_interval = fixed_integer_type_interval(*source_type)?;
                let minimum = interval.0.max(source_interval.0);
                let maximum = interval.1.min(source_interval.1);
                if minimum > maximum {
                    return Some(Proposition::Falsehood);
                }
                return Some(exact_integer_source_interval_obligation(
                    *source_type,
                    (**operand).clone(),
                    minimum,
                    maximum,
                ));
            }
            _ => return None,
        }
    }
    None
}

fn exact_integer_cast_chain_then_shift_suffix_obligation(
    value_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    count: u128,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    let Some(mut interval) = exact_integer_shift_left_input_interval(value_type, count) else {
        return Some(Proposition::Falsehood);
    };
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    for _ in 0..=prior_axiom_count {
        if let Some((root_type, root, cast_interval)) = exact_integer_cast_chain_root_interval(
            value_type,
            value.clone(),
            semantic_axioms,
            prior_axiom_count,
            machine_parameter_values,
        ) {
            let minimum = interval.0.max(cast_interval.0);
            let maximum = interval.1.min(cast_interval.1);
            return Some(if minimum <= maximum {
                exact_integer_source_interval_obligation(root_type, root, minimum, maximum)
            } else {
                Proposition::Falsehood
            });
        }
        if let Some(obligation) = exact_integer_computed_prefix_conversion_interval_obligation(
            value_type,
            value.clone(),
            interval,
            semantic_axioms,
            prior_axiom_count,
            machine_parameter_values,
        ) {
            return Some(obligation);
        }
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })?;
        let (nested_value, count_type, nested_count) = match definition {
            definition @ ScalarTerm::ExactIntegerShiftLeft {
                value_type: nested_value_type,
                count_type,
                value,
                count,
            }
            | definition @ ScalarTerm::ExactIntegerShiftRight {
                value_type: nested_value_type,
                count_type,
                value,
                count,
            } if *nested_value_type == value_type => (value, *count_type, (definition, count)),
            _ => return None,
        };
        let count = landed_exact_shift_count(
            value_type,
            count_type,
            nested_count.1,
            semantic_axioms,
            definition_index,
        )?;
        interval =
            match exact_integer_mixed_shift_preimage(value_type, interval, nested_count.0, count) {
                Ok(Some(interval)) => interval,
                Ok(None) => return Some(Proposition::Falsehood),
                Err(()) => return None,
            };
        value = (**nested_value).clone();
        prior_axiom_count = definition_index;
    }
    None
}

fn exact_integer_arithmetic_then_shift_chain_obligation(
    value_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    count: u128,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if value_type.is_address() || !matches!(value_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let Some(mut interval) = exact_integer_shift_left_input_interval(value_type, count) else {
        return Some(Proposition::Falsehood);
    };
    let mut coefficient = 1_u128;
    let mut offset = IntegerOffset::Nonnegative(0);
    let mut saw_arithmetic = false;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    for _ in 0..=prior_axiom_count {
        if saw_arithmetic
            && matches!(
                &value,
                ScalarTerm::Value {
                    id,
                    scalar_type: ScalarType::Integer(root_type),
                } if *root_type == value_type && machine_parameter_values.contains(id)
            )
        {
            return match exact_integer_affine_preimage_obligation(
                value_type,
                value,
                coefficient,
                offset,
                interval,
            ) {
                Ok(obligation) => Some(obligation),
                Err(()) => None,
            };
        }
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })?;
        match definition {
            definition @ ScalarTerm::ExactIntegerShiftLeft {
                value_type: nested_value_type,
                count_type,
                value: nested_value,
                count: nested_count,
            }
            | definition @ ScalarTerm::ExactIntegerShiftRight {
                value_type: nested_value_type,
                count_type,
                value: nested_value,
                count: nested_count,
            } if !saw_arithmetic && *nested_value_type == value_type => {
                let nested_count = landed_exact_shift_count(
                    value_type,
                    *count_type,
                    nested_count,
                    semantic_axioms,
                    definition_index,
                )?;
                interval = match exact_integer_mixed_shift_preimage(
                    value_type,
                    interval,
                    definition,
                    nested_count,
                ) {
                    Ok(Some(interval)) => interval,
                    Ok(None) => return Some(Proposition::Falsehood),
                    Err(()) => return None,
                };
                value = (**nested_value).clone();
                prior_axiom_count = definition_index;
            }
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } if *scalar_type == value_type => {
                if landed_integer_constant_value(
                    value_type,
                    left,
                    semantic_axioms,
                    definition_index,
                )
                .is_some()
                {
                    return None;
                }
                let constant = landed_integer_constant_value(
                    value_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?;
                let (nested_coefficient, nested_offset) = match definition {
                    ScalarTerm::ExactIntegerAdd { .. } => (1, IntegerOffset::from_value(constant)),
                    ScalarTerm::ExactIntegerSubtract { .. } => {
                        (1, IntegerOffset::from_subtrahend(constant))
                    }
                    ScalarTerm::ExactIntegerMultiply { .. } => (
                        nonnegative_integer_factor(value_type, constant)?,
                        IntegerOffset::Nonnegative(0),
                    ),
                    _ => unreachable!("matched one exact arithmetic definition"),
                };
                offset = nested_offset
                    .checked_multiply(coefficient)
                    .and_then(|nested| nested.checked_add(offset))?;
                coefficient = coefficient.checked_mul(nested_coefficient)?;
                value = (**left).clone();
                prior_axiom_count = definition_index;
                saw_arithmetic = true;
            }
            _ => return None,
        }
    }
    None
}

fn exact_integer_affine_preimage_obligation(
    root_type: psi_core::IntegerType,
    root: ScalarTerm,
    coefficient: u128,
    offset: IntegerOffset,
    interval: (i128, i128),
) -> Result<Proposition, ()> {
    let offset_as_i128 = |offset| match offset {
        IntegerOffset::Nonnegative(value) => i128::try_from(value).ok(),
        IntegerOffset::Negative(value) if value > (1_u128 << 127) => None,
        IntegerOffset::Negative(value) => signed_negative_magnitude(value),
    };
    if coefficient == 0 {
        let Some(constant) = offset_as_i128(offset) else {
            return Ok(Proposition::Falsehood);
        };
        return Ok(if interval.0 <= constant && constant <= interval.1 {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        });
    }
    let Some(interval) =
        exact_integer_affine_preimage_interval(root_type, coefficient, offset, interval)?
    else {
        return Ok(Proposition::Falsehood);
    };
    Ok(exact_integer_source_interval_obligation(
        root_type, root, interval.0, interval.1,
    ))
}

fn exact_integer_affine_preimage_interval(
    input_type: psi_core::IntegerType,
    coefficient: u128,
    offset: IntegerOffset,
    interval: (i128, i128),
) -> Result<Option<(i128, i128)>, ()> {
    debug_assert_ne!(coefficient, 0);
    let lower_numerator = IntegerOffset::from_value(IntegerValue::Signed(interval.0))
        .checked_add(offset.negated())
        .ok_or(())?;
    let upper_numerator = IntegerOffset::from_value(IntegerValue::Signed(interval.1))
        .checked_add(offset.negated())
        .ok_or(())?;
    let lower = integer_offset_ceil_div(lower_numerator, coefficient);
    let upper = integer_offset_floor_div(upper_numerator, coefficient);
    let lower = match lower {
        IntegerOffset::Nonnegative(value) => match i128::try_from(value) {
            Ok(value) => value,
            Err(_) => return Ok(None),
        },
        IntegerOffset::Negative(value) if value > (1_u128 << 127) => i128::MIN,
        IntegerOffset::Negative(value) => signed_negative_magnitude(value).ok_or(())?,
    };
    let upper = match upper {
        IntegerOffset::Nonnegative(value) => i128::try_from(value).unwrap_or(i128::MAX),
        IntegerOffset::Negative(value) if value > (1_u128 << 127) => {
            return Ok(None);
        }
        IntegerOffset::Negative(value) => signed_negative_magnitude(value).ok_or(())?,
    };
    let carrier = fixed_integer_type_interval(input_type).ok_or(())?;
    let lower = lower.max(carrier.0);
    let upper = upper.min(carrier.1);
    Ok((lower <= upper).then_some((lower, upper)))
}

fn exact_integer_shift_then_arithmetic_chain_obligation(
    value_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    initial_constant: IntegerValue,
    initial_operation: ExactIntegerAffineOperation,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if value_type.is_address() || !matches!(value_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let (mut coefficient, mut offset) = match initial_operation {
        ExactIntegerAffineOperation::Add => (1, IntegerOffset::from_value(initial_constant)),
        ExactIntegerAffineOperation::Subtract => {
            (1, IntegerOffset::from_subtrahend(initial_constant))
        }
        ExactIntegerAffineOperation::Multiply => (
            nonnegative_integer_factor(value_type, initial_constant)?,
            IntegerOffset::Nonnegative(0),
        ),
    };
    let carrier_interval = fixed_integer_type_interval(value_type)?;
    let mut shifted_interval: Option<(i128, i128)> = None;
    let mut constant_decision = None;
    let mut mathematical_empty = false;
    let mut saw_shift = false;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    for _ in 0..=prior_axiom_count {
        if saw_shift
            && matches!(
                &value,
                ScalarTerm::Value {
                    id,
                    scalar_type: ScalarType::Integer(root_type),
                } if *root_type == value_type && machine_parameter_values.contains(id)
            )
        {
            if let Some(decision) = constant_decision {
                return Some(decision);
            }
            if mathematical_empty {
                return Some(Proposition::Falsehood);
            }
            let interval = shifted_interval?;
            return Some(exact_integer_source_interval_obligation(
                value_type, value, interval.0, interval.1,
            ));
        }
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })?;
        match definition {
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } if !saw_shift && *scalar_type == value_type => {
                if landed_integer_constant_value(
                    value_type,
                    left,
                    semantic_axioms,
                    definition_index,
                )
                .is_some()
                {
                    return None;
                }
                let constant = landed_integer_constant_value(
                    value_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?;
                let (nested_coefficient, nested_offset) = match definition {
                    ScalarTerm::ExactIntegerAdd { .. } => (1, IntegerOffset::from_value(constant)),
                    ScalarTerm::ExactIntegerSubtract { .. } => {
                        (1, IntegerOffset::from_subtrahend(constant))
                    }
                    ScalarTerm::ExactIntegerMultiply { .. } => (
                        nonnegative_integer_factor(value_type, constant)?,
                        IntegerOffset::Nonnegative(0),
                    ),
                    _ => unreachable!("matched one exact arithmetic definition"),
                };
                offset = nested_offset
                    .checked_multiply(coefficient)
                    .and_then(|nested| nested.checked_add(offset))?;
                coefficient = coefficient.checked_mul(nested_coefficient)?;
                value = (**left).clone();
                prior_axiom_count = definition_index;
            }
            definition @ ScalarTerm::ExactIntegerShiftLeft {
                value_type: nested_value_type,
                count_type,
                value: nested_value,
                count,
            }
            | definition @ ScalarTerm::ExactIntegerShiftRight {
                value_type: nested_value_type,
                count_type,
                value: nested_value,
                count,
            } if *nested_value_type == value_type => {
                if !saw_shift {
                    if coefficient == 0 {
                        let decision = if offset.is_representable(value_type) {
                            Proposition::Truth
                        } else {
                            Proposition::Falsehood
                        };
                        constant_decision = Some(decision);
                    } else {
                        shifted_interval = match exact_integer_affine_preimage_interval(
                            value_type,
                            coefficient,
                            offset,
                            carrier_interval,
                        ) {
                            Ok(Some(interval)) => Some(interval),
                            Ok(None) => {
                                mathematical_empty = true;
                                None
                            }
                            Err(()) => return None,
                        };
                    }
                    saw_shift = true;
                }
                let count = landed_exact_shift_count(
                    value_type,
                    *count_type,
                    count,
                    semantic_axioms,
                    definition_index,
                )?;
                if constant_decision.is_none() && !mathematical_empty {
                    shifted_interval = match exact_integer_mixed_shift_preimage(
                        value_type,
                        shifted_interval?,
                        definition,
                        count,
                    ) {
                        Ok(Some(interval)) => Some(interval),
                        Ok(None) => {
                            mathematical_empty = true;
                            None
                        }
                        Err(()) => return None,
                    };
                }
                value = (**nested_value).clone();
                prior_axiom_count = definition_index;
            }
            _ => return None,
        }
    }
    None
}

fn exact_integer_shift_cast_shift_obligation(
    target_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    count: u128,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if target_type.is_address() || !matches!(target_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let mut interval = exact_integer_shift_left_input_interval(target_type, count)?;
    let mut mathematical_empty = false;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let (source_type, mut source_value) = loop {
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })?;
        match definition {
            definition @ ScalarTerm::ExactIntegerShiftLeft {
                value_type,
                count_type,
                value: nested_value,
                count,
            }
            | definition @ ScalarTerm::ExactIntegerShiftRight {
                value_type,
                count_type,
                value: nested_value,
                count,
            } if *value_type == target_type => {
                let count = landed_exact_shift_count(
                    target_type,
                    *count_type,
                    count,
                    semantic_axioms,
                    definition_index,
                )?;
                if !mathematical_empty {
                    interval = match exact_integer_mixed_shift_preimage(
                        target_type,
                        interval,
                        definition,
                        count,
                    ) {
                        Ok(Some(interval)) => interval,
                        Ok(None) => {
                            mathematical_empty = true;
                            interval
                        }
                        Err(()) => return None,
                    };
                }
                value = (**nested_value).clone();
                prior_axiom_count = definition_index;
            }
            ScalarTerm::IntegerExactCast {
                source_type,
                target_type: cast_target_type,
                operand,
            } if *cast_target_type == target_type
                && !source_type.is_address()
                && matches!(source_type.bits(), 8 | 16 | 32 | 64)
                && *source_type != target_type
                && !source_type.can_widen_to(target_type)
                && source_type.can_exact_cast_to(target_type) =>
            {
                if !mathematical_empty {
                    let source_interval = fixed_integer_type_interval(*source_type)?;
                    let minimum = interval.0.max(source_interval.0);
                    let maximum = interval.1.min(source_interval.1);
                    if minimum > maximum {
                        mathematical_empty = true;
                    } else {
                        interval = (minimum, maximum);
                    }
                }
                prior_axiom_count = definition_index;
                break (*source_type, (**operand).clone());
            }
            _ => return None,
        }
    };

    let mut followed_source_definition = false;
    for _ in 0..=prior_axiom_count {
        if matches!(
            &source_value,
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == source_type && machine_parameter_values.contains(id)
        ) {
            if !followed_source_definition {
                return None;
            }
            return Some(if mathematical_empty {
                Proposition::Falsehood
            } else {
                exact_integer_source_interval_obligation(
                    source_type,
                    source_value,
                    interval.0,
                    interval.1,
                )
            });
        }
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &source_value => Some((index, right)),
                _ => None,
            })?;
        let (nested_value, count_type, count) = match definition {
            ScalarTerm::ExactIntegerShiftLeft {
                value_type,
                count_type,
                value,
                count,
            }
            | ScalarTerm::ExactIntegerShiftRight {
                value_type,
                count_type,
                value,
                count,
            } if *value_type == source_type => (value, *count_type, count),
            _ => return None,
        };
        let count = landed_exact_shift_count(
            source_type,
            count_type,
            count,
            semantic_axioms,
            definition_index,
        )?;
        if !mathematical_empty {
            interval = match exact_integer_mixed_shift_preimage(
                source_type,
                interval,
                definition,
                count,
            ) {
                Ok(Some(interval)) => interval,
                Ok(None) => {
                    mathematical_empty = true;
                    interval
                }
                Err(()) => return None,
            };
        }
        source_value = (**nested_value).clone();
        prior_axiom_count = definition_index;
        followed_source_definition = true;
    }
    None
}

fn exact_integer_affine_cast_shift_obligation(
    target_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    count: u128,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if target_type.is_address() || !matches!(target_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let mut interval = exact_integer_shift_left_input_interval(target_type, count)?;
    let mut mathematical_empty = false;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let (source_type, mut source_value) = loop {
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })?;
        match definition {
            definition @ ScalarTerm::ExactIntegerShiftLeft {
                value_type,
                count_type,
                value: nested_value,
                count,
            }
            | definition @ ScalarTerm::ExactIntegerShiftRight {
                value_type,
                count_type,
                value: nested_value,
                count,
            } if *value_type == target_type => {
                let count = landed_exact_shift_count(
                    target_type,
                    *count_type,
                    count,
                    semantic_axioms,
                    definition_index,
                )?;
                if !mathematical_empty {
                    interval = match exact_integer_mixed_shift_preimage(
                        target_type,
                        interval,
                        definition,
                        count,
                    ) {
                        Ok(Some(interval)) => interval,
                        Ok(None) => {
                            mathematical_empty = true;
                            interval
                        }
                        Err(()) => return None,
                    };
                }
                value = (**nested_value).clone();
                prior_axiom_count = definition_index;
            }
            ScalarTerm::IntegerExactCast {
                source_type,
                target_type: cast_target_type,
                operand,
            } if *cast_target_type == target_type
                && !source_type.is_address()
                && matches!(source_type.bits(), 8 | 16 | 32 | 64)
                && *source_type != target_type
                && !source_type.can_widen_to(target_type)
                && source_type.can_exact_cast_to(target_type) =>
            {
                if !mathematical_empty {
                    let source_interval = fixed_integer_type_interval(*source_type)?;
                    let minimum = interval.0.max(source_interval.0);
                    let maximum = interval.1.min(source_interval.1);
                    if minimum > maximum {
                        mathematical_empty = true;
                    } else {
                        interval = (minimum, maximum);
                    }
                }
                prior_axiom_count = definition_index;
                break (*source_type, (**operand).clone());
            }
            _ => return None,
        }
    };

    let mut coefficient = 1_u128;
    let mut offset = IntegerOffset::Nonnegative(0);
    let mut followed_source_definition = false;
    for _ in 0..=prior_axiom_count {
        if matches!(
            &source_value,
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == source_type && machine_parameter_values.contains(id)
        ) {
            if !followed_source_definition {
                return None;
            }
            if mathematical_empty {
                return Some(Proposition::Falsehood);
            }
            return exact_integer_affine_preimage_obligation(
                source_type,
                source_value,
                coefficient,
                offset,
                interval,
            )
            .ok();
        }
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &source_value => Some((index, right)),
                _ => None,
            })?;
        let (left, right, nested_coefficient, nested_offset) = match definition {
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            } if *scalar_type == source_type => (
                left,
                right,
                1,
                IntegerOffset::from_value(landed_integer_constant_value(
                    source_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?),
            ),
            ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } if *scalar_type == source_type => (
                left,
                right,
                1,
                IntegerOffset::from_subtrahend(landed_integer_constant_value(
                    source_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?),
            ),
            ScalarTerm::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } if *scalar_type == source_type => (
                left,
                right,
                nonnegative_integer_factor(
                    source_type,
                    landed_integer_constant_value(
                        source_type,
                        right,
                        semantic_axioms,
                        definition_index,
                    )?,
                )?,
                IntegerOffset::Nonnegative(0),
            ),
            _ => return None,
        };
        if landed_integer_constant_value(source_type, left, semantic_axioms, definition_index)
            .is_some()
            || landed_integer_constant_value(source_type, right, semantic_axioms, definition_index)
                .is_none()
        {
            return None;
        }
        offset = nested_offset
            .checked_multiply(coefficient)
            .and_then(|nested| nested.checked_add(offset))?;
        coefficient = coefficient.checked_mul(nested_coefficient)?;
        source_value = (**left).clone();
        prior_axiom_count = definition_index;
        followed_source_definition = true;
    }
    None
}

fn exact_integer_shift_cast_affine_obligation(
    target_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    initial_constant: IntegerValue,
    initial_operation: ExactIntegerAffineOperation,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if target_type.is_address() || !matches!(target_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let (mut coefficient, mut offset) = match initial_operation {
        ExactIntegerAffineOperation::Add => (1, IntegerOffset::from_value(initial_constant)),
        ExactIntegerAffineOperation::Subtract => {
            (1, IntegerOffset::from_subtrahend(initial_constant))
        }
        ExactIntegerAffineOperation::Multiply => (
            nonnegative_integer_factor(target_type, initial_constant)?,
            IntegerOffset::Nonnegative(0),
        ),
    };
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let (source_type, mut source_value) = loop {
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })?;
        match definition {
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } if *scalar_type == target_type => {
                if landed_integer_constant_value(
                    target_type,
                    left,
                    semantic_axioms,
                    definition_index,
                )
                .is_some()
                {
                    return None;
                }
                let constant = landed_integer_constant_value(
                    target_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?;
                let (nested_coefficient, nested_offset) = match definition {
                    ScalarTerm::ExactIntegerAdd { .. } => (1, IntegerOffset::from_value(constant)),
                    ScalarTerm::ExactIntegerSubtract { .. } => {
                        (1, IntegerOffset::from_subtrahend(constant))
                    }
                    ScalarTerm::ExactIntegerMultiply { .. } => (
                        nonnegative_integer_factor(target_type, constant)?,
                        IntegerOffset::Nonnegative(0),
                    ),
                    _ => unreachable!("matched one exact affine definition"),
                };
                offset = nested_offset
                    .checked_multiply(coefficient)
                    .and_then(|nested| nested.checked_add(offset))?;
                coefficient = coefficient.checked_mul(nested_coefficient)?;
                value = (**left).clone();
                prior_axiom_count = definition_index;
            }
            ScalarTerm::IntegerExactCast {
                source_type,
                target_type: cast_target_type,
                operand,
            } if *cast_target_type == target_type
                && !source_type.is_address()
                && matches!(source_type.bits(), 8 | 16 | 32 | 64)
                && *source_type != target_type
                && !source_type.can_widen_to(target_type)
                && source_type.can_exact_cast_to(target_type) =>
            {
                prior_axiom_count = definition_index;
                break (*source_type, (**operand).clone());
            }
            _ => return None,
        }
    };

    let target_carrier = fixed_integer_type_interval(target_type)?;
    let mut constant_decision = None;
    let mut mathematical_empty = false;
    let mut interval = if coefficient == 0 {
        constant_decision = Some(if offset.is_representable(target_type) {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        });
        target_carrier
    } else {
        match exact_integer_affine_preimage_interval(
            target_type,
            coefficient,
            offset,
            target_carrier,
        ) {
            Ok(Some(interval)) => interval,
            Ok(None) => {
                mathematical_empty = true;
                target_carrier
            }
            Err(()) => return None,
        }
    };
    if constant_decision.is_none() && !mathematical_empty {
        let source_carrier = fixed_integer_type_interval(source_type)?;
        let minimum = interval.0.max(source_carrier.0);
        let maximum = interval.1.min(source_carrier.1);
        if minimum > maximum {
            mathematical_empty = true;
        } else {
            interval = (minimum, maximum);
        }
    }

    let mut followed_source_definition = false;
    for _ in 0..=prior_axiom_count {
        if matches!(
            &source_value,
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == source_type && machine_parameter_values.contains(id)
        ) {
            if !followed_source_definition {
                return None;
            }
            if let Some(decision) = constant_decision {
                return Some(decision);
            }
            if mathematical_empty {
                return Some(Proposition::Falsehood);
            }
            return Some(exact_integer_source_interval_obligation(
                source_type,
                source_value,
                interval.0,
                interval.1,
            ));
        }
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &source_value => Some((index, right)),
                _ => None,
            })?;
        let (nested_value, count_type, count) = match definition {
            ScalarTerm::ExactIntegerShiftLeft {
                value_type,
                count_type,
                value,
                count,
            }
            | ScalarTerm::ExactIntegerShiftRight {
                value_type,
                count_type,
                value,
                count,
            } if *value_type == source_type => (value, *count_type, count),
            _ => return None,
        };
        let count = landed_exact_shift_count(
            source_type,
            count_type,
            count,
            semantic_axioms,
            definition_index,
        )?;
        if constant_decision.is_none() && !mathematical_empty {
            interval = match exact_integer_mixed_shift_preimage(
                source_type,
                interval,
                definition,
                count,
            ) {
                Ok(Some(interval)) => interval,
                Ok(None) => {
                    mathematical_empty = true;
                    interval
                }
                Err(()) => return None,
            };
        }
        source_value = (**nested_value).clone();
        prior_axiom_count = definition_index;
        followed_source_definition = true;
    }
    None
}

fn exact_integer_divide_remainder_cast_shift_obligation(
    target_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    count: u128,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if target_type.is_address() || !matches!(target_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let mut interval = exact_integer_shift_left_input_interval(target_type, count)?;
    let mut mathematical_empty = false;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let (source_type, source_value) = loop {
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })?;
        match definition {
            definition @ ScalarTerm::ExactIntegerShiftLeft {
                value_type,
                count_type,
                value: nested_value,
                count,
            }
            | definition @ ScalarTerm::ExactIntegerShiftRight {
                value_type,
                count_type,
                value: nested_value,
                count,
            } if *value_type == target_type => {
                let count = landed_exact_shift_count(
                    target_type,
                    *count_type,
                    count,
                    semantic_axioms,
                    definition_index,
                )?;
                if !mathematical_empty {
                    interval = match exact_integer_mixed_shift_preimage(
                        target_type,
                        interval,
                        definition,
                        count,
                    ) {
                        Ok(Some(interval)) => interval,
                        Ok(None) => {
                            mathematical_empty = true;
                            interval
                        }
                        Err(()) => return None,
                    };
                }
                value = (**nested_value).clone();
                prior_axiom_count = definition_index;
            }
            ScalarTerm::IntegerExactCast {
                source_type,
                target_type: cast_target_type,
                operand,
            } if *cast_target_type == target_type
                && !source_type.is_address()
                && matches!(source_type.bits(), 8 | 16 | 32 | 64)
                && *source_type != target_type
                && !source_type.can_widen_to(target_type)
                && source_type.can_exact_cast_to(target_type) =>
            {
                prior_axiom_count = definition_index;
                break (*source_type, (**operand).clone());
            }
            _ => return None,
        }
    };
    let hull = exact_integer_divide_remainder_chain_hull(
        source_type,
        source_value,
        semantic_axioms,
        prior_axiom_count,
        machine_parameter_values,
    )?;
    let target_carrier = fixed_integer_type_interval(target_type)?;
    if hull.0 < target_carrier.0 || hull.1 > target_carrier.1 {
        return None;
    }
    if mathematical_empty {
        return Some(Proposition::Falsehood);
    }
    exact_integer_carrier_total_hull_obligation(hull, interval)
}

fn exact_integer_divide_remainder_then_shift_obligation(
    value_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    count: u128,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if value_type.is_address() || !matches!(value_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let mut interval = exact_integer_shift_left_input_interval(value_type, count)?;
    let mut mathematical_empty = false;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    for _ in 0..=prior_axiom_count {
        let Some((definition_index, definition)) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })
        else {
            break;
        };
        let (nested_value, count_type, nested_count) = match definition {
            ScalarTerm::ExactIntegerShiftLeft {
                value_type: nested_value_type,
                count_type,
                value,
                count,
            }
            | ScalarTerm::ExactIntegerShiftRight {
                value_type: nested_value_type,
                count_type,
                value,
                count,
            } if *nested_value_type == value_type => (value, *count_type, count),
            _ => break,
        };
        let nested_count = landed_exact_shift_count(
            value_type,
            count_type,
            nested_count,
            semantic_axioms,
            definition_index,
        )?;
        if !mathematical_empty {
            interval = match exact_integer_mixed_shift_preimage(
                value_type,
                interval,
                definition,
                nested_count,
            ) {
                Ok(Some(interval)) => interval,
                Ok(None) => {
                    mathematical_empty = true;
                    interval
                }
                Err(()) => return None,
            };
        }
        value = (**nested_value).clone();
        prior_axiom_count = definition_index;
    }
    let hull = exact_integer_divide_remainder_chain_hull(
        value_type,
        value,
        semantic_axioms,
        prior_axiom_count,
        machine_parameter_values,
    )?;
    if mathematical_empty {
        return Some(Proposition::Falsehood);
    }
    exact_integer_carrier_total_hull_obligation(hull, interval)
}

fn exact_integer_divide_remainder_cast_affine_obligation(
    target_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    initial_constant: IntegerValue,
    initial_operation: ExactIntegerAffineOperation,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if target_type.is_address() || !matches!(target_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let (mut coefficient, mut offset) = match initial_operation {
        ExactIntegerAffineOperation::Add => (1, IntegerOffset::from_value(initial_constant)),
        ExactIntegerAffineOperation::Subtract => {
            (1, IntegerOffset::from_subtrahend(initial_constant))
        }
        ExactIntegerAffineOperation::Multiply => (
            nonnegative_integer_factor(target_type, initial_constant)?,
            IntegerOffset::Nonnegative(0),
        ),
    };
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let (source_type, source_value) = loop {
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })?;
        match definition {
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } if *scalar_type == target_type => {
                if landed_integer_constant_value(
                    target_type,
                    left,
                    semantic_axioms,
                    definition_index,
                )
                .is_some()
                {
                    return None;
                }
                let constant = landed_integer_constant_value(
                    target_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?;
                let (nested_coefficient, nested_offset) = match definition {
                    ScalarTerm::ExactIntegerAdd { .. } => (1, IntegerOffset::from_value(constant)),
                    ScalarTerm::ExactIntegerSubtract { .. } => {
                        (1, IntegerOffset::from_subtrahend(constant))
                    }
                    ScalarTerm::ExactIntegerMultiply { .. } => (
                        nonnegative_integer_factor(target_type, constant)?,
                        IntegerOffset::Nonnegative(0),
                    ),
                    _ => unreachable!("matched one exact affine definition"),
                };
                offset = nested_offset
                    .checked_multiply(coefficient)
                    .and_then(|nested| nested.checked_add(offset))?;
                coefficient = coefficient.checked_mul(nested_coefficient)?;
                value = (**left).clone();
                prior_axiom_count = definition_index;
            }
            ScalarTerm::IntegerExactCast {
                source_type,
                target_type: cast_target_type,
                operand,
            } if *cast_target_type == target_type
                && !source_type.is_address()
                && matches!(source_type.bits(), 8 | 16 | 32 | 64)
                && *source_type != target_type
                && !source_type.can_widen_to(target_type)
                && source_type.can_exact_cast_to(target_type) =>
            {
                prior_axiom_count = definition_index;
                break (*source_type, (**operand).clone());
            }
            _ => return None,
        }
    };
    let hull = exact_integer_divide_remainder_chain_hull(
        source_type,
        source_value,
        semantic_axioms,
        prior_axiom_count,
        machine_parameter_values,
    )?;
    let target_carrier = fixed_integer_type_interval(target_type)?;
    if hull.0 < target_carrier.0 || hull.1 > target_carrier.1 {
        return None;
    }
    if coefficient == 0 {
        return Some(if offset.is_representable(target_type) {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        });
    }
    match exact_integer_affine_preimage_interval(target_type, coefficient, offset, target_carrier) {
        Ok(Some(interval)) => exact_integer_carrier_total_hull_obligation(hull, interval),
        Ok(None) => Some(Proposition::Falsehood),
        Err(()) => None,
    }
}

fn exact_integer_divide_remainder_then_affine_obligation(
    integer_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    initial_constant: IntegerValue,
    initial_operation: ExactIntegerAffineOperation,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if integer_type.is_address() || !matches!(integer_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let (mut coefficient, mut offset) = match initial_operation {
        ExactIntegerAffineOperation::Add => (1, IntegerOffset::from_value(initial_constant)),
        ExactIntegerAffineOperation::Subtract => {
            (1, IntegerOffset::from_subtrahend(initial_constant))
        }
        ExactIntegerAffineOperation::Multiply => (
            nonnegative_integer_factor(integer_type, initial_constant)?,
            IntegerOffset::Nonnegative(0),
        ),
    };
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    for _ in 0..=prior_axiom_count {
        let Some((definition_index, definition)) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })
        else {
            break;
        };
        let (left, right, nested_coefficient, nested_offset) = match definition {
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            } if *scalar_type == integer_type => (
                left,
                right,
                1,
                IntegerOffset::from_value(landed_integer_constant_value(
                    integer_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?),
            ),
            ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } if *scalar_type == integer_type => (
                left,
                right,
                1,
                IntegerOffset::from_subtrahend(landed_integer_constant_value(
                    integer_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?),
            ),
            ScalarTerm::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } if *scalar_type == integer_type => (
                left,
                right,
                nonnegative_integer_factor(
                    integer_type,
                    landed_integer_constant_value(
                        integer_type,
                        right,
                        semantic_axioms,
                        definition_index,
                    )?,
                )?,
                IntegerOffset::Nonnegative(0),
            ),
            _ => break,
        };
        if landed_integer_constant_value(integer_type, left, semantic_axioms, definition_index)
            .is_some()
            || landed_integer_constant_value(integer_type, right, semantic_axioms, definition_index)
                .is_none()
        {
            return None;
        }
        offset = nested_offset
            .checked_multiply(coefficient)
            .and_then(|nested| nested.checked_add(offset))?;
        coefficient = coefficient.checked_mul(nested_coefficient)?;
        value = (**left).clone();
        prior_axiom_count = definition_index;
    }
    let hull = exact_integer_divide_remainder_chain_hull(
        integer_type,
        value,
        semantic_axioms,
        prior_axiom_count,
        machine_parameter_values,
    )?;
    if coefficient == 0 {
        return Some(if offset.is_representable(integer_type) {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        });
    }
    let carrier = fixed_integer_type_interval(integer_type)?;
    let interval =
        match exact_integer_affine_preimage_interval(integer_type, coefficient, offset, carrier) {
            Ok(Some(interval)) => interval,
            Ok(None) => return Some(Proposition::Falsehood),
            Err(()) => return None,
        };
    exact_integer_carrier_total_hull_obligation(hull, interval)
}

fn exact_integer_mixed_shift_chain_obligation(
    value_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    count: u128,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if value_type.is_address() || !matches!(value_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let Some(mut interval) = exact_integer_shift_left_input_interval(value_type, count) else {
        return Some(Proposition::Falsehood);
    };
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let mut saw_right = false;
    for _ in 0..=prior_axiom_count {
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })?;
        match definition {
            definition @ ScalarTerm::ExactIntegerShiftLeft {
                value_type: nested_value_type,
                count_type,
                value: nested_value,
                count: nested_count,
            } if *nested_value_type == value_type => {
                let nested_count = landed_exact_shift_count(
                    value_type,
                    *count_type,
                    nested_count,
                    semantic_axioms,
                    definition_index,
                )?;
                let mapped = match exact_integer_mixed_shift_preimage(
                    value_type,
                    interval,
                    definition,
                    nested_count,
                ) {
                    Ok(Some(mapped)) => mapped,
                    Ok(None) => return Some(Proposition::Falsehood),
                    Err(()) => return None,
                };
                interval = mapped;
                value = (**nested_value).clone();
                prior_axiom_count = definition_index;
            }
            definition @ ScalarTerm::ExactIntegerShiftRight {
                value_type: nested_value_type,
                count_type,
                value: nested_value,
                count: nested_count,
            } if *nested_value_type == value_type => {
                let nested_count = landed_exact_shift_count(
                    value_type,
                    *count_type,
                    nested_count,
                    semantic_axioms,
                    definition_index,
                )?;
                let mapped = match exact_integer_mixed_shift_preimage(
                    value_type,
                    interval,
                    definition,
                    nested_count,
                ) {
                    Ok(Some(mapped)) => mapped,
                    Ok(None) => return Some(Proposition::Falsehood),
                    Err(()) => return None,
                };
                interval = mapped;
                value = (**nested_value).clone();
                prior_axiom_count = definition_index;
                saw_right = true;
            }
            _ => return None,
        }
        if saw_right
            && matches!(
                &value,
                ScalarTerm::Value {
                    id,
                    scalar_type: ScalarType::Integer(root_type),
                } if *root_type == value_type && machine_parameter_values.contains(id)
            )
        {
            return Some(exact_integer_source_interval_obligation(
                value_type, value, interval.0, interval.1,
            ));
        }
    }
    None
}

fn exact_integer_shift_left_input_interval(
    value_type: psi_core::IntegerType,
    count: u128,
) -> Option<(i128, i128)> {
    let count = u32::try_from(count).ok()?;
    if count >= u32::from(value_type.bits()) {
        return None;
    }
    match value_type.sign() {
        IntegerSign::Unsigned => {
            let IntegerValue::Unsigned(maximum) = value_type.maximum_value() else {
                unreachable!("unsigned fixed integer type has an unsigned maximum")
            };
            Some((0, i128::try_from(maximum >> count).ok()?))
        }
        IntegerSign::Signed => {
            let (IntegerValue::Signed(minimum), IntegerValue::Signed(maximum)) =
                (value_type.minimum_value(), value_type.maximum_value())
            else {
                unreachable!("signed fixed integer type has signed bounds")
            };
            Some((minimum >> count, maximum >> count))
        }
    }
}

fn exact_integer_mixed_shift_preimage(
    value_type: psi_core::IntegerType,
    interval: (i128, i128),
    definition: &ScalarTerm,
    count: u128,
) -> Result<Option<(i128, i128)>, ()> {
    let count = u32::try_from(count).map_err(|_| ())?;
    if count >= u32::from(value_type.bits()) {
        return Err(());
    }
    let scale = 1_i128.checked_shl(count).ok_or(())?;
    let mapped = match definition {
        ScalarTerm::ExactIntegerShiftLeft { .. } => {
            let minimum =
                interval.0.div_euclid(scale) + i128::from(interval.0.rem_euclid(scale) != 0);
            let maximum = interval.1.div_euclid(scale);
            (minimum, maximum)
        }
        ScalarTerm::ExactIntegerShiftRight { .. } => match value_type.sign() {
            IntegerSign::Signed => (
                interval.0.checked_mul(scale).ok_or(())?,
                interval
                    .1
                    .checked_add(1)
                    .ok_or(())?
                    .checked_mul(scale)
                    .ok_or(())?
                    .checked_sub(1)
                    .ok_or(())?,
            ),
            IntegerSign::Unsigned => {
                let minimum = u128::try_from(interval.0)
                    .map_err(|_| ())?
                    .checked_mul(scale as u128)
                    .ok_or(())?;
                let maximum = u128::try_from(interval.1)
                    .map_err(|_| ())?
                    .checked_add(1)
                    .ok_or(())?
                    .checked_mul(scale as u128)
                    .ok_or(())?
                    .checked_sub(1)
                    .ok_or(())?;
                (
                    i128::try_from(minimum).map_err(|_| ())?,
                    i128::try_from(maximum).map_err(|_| ())?,
                )
            }
        },
        _ => return Err(()),
    };
    let carrier_minimum = integer_value_as_i128(value_type.minimum_value()).ok_or(())?;
    let carrier_maximum = integer_value_as_i128(value_type.maximum_value()).ok_or(())?;
    let minimum = mapped.0.max(carrier_minimum);
    let maximum = mapped.1.min(carrier_maximum);
    Ok((minimum <= maximum).then_some((minimum, maximum)))
}

fn exact_integer_cast_then_shift_left_chain_obligation(
    value_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    count: u128,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if value_type.is_address() || !matches!(value_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let mut cumulative_count = count;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    for _ in 0..=prior_axiom_count {
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })?;
        match definition {
            ScalarTerm::ExactIntegerShiftLeft {
                value_type: nested_value_type,
                count_type,
                value: nested_value,
                count: nested_count,
            } if *nested_value_type == value_type => {
                let nested_count = landed_exact_shift_count(
                    value_type,
                    *count_type,
                    nested_count,
                    semantic_axioms,
                    definition_index,
                )?;
                let Some(total) = cumulative_count.checked_add(nested_count) else {
                    return Some(Proposition::Falsehood);
                };
                cumulative_count = total;
                value = (**nested_value).clone();
                prior_axiom_count = definition_index;
            }
            ScalarTerm::IntegerExactCast {
                source_type,
                target_type,
                operand,
            } if *target_type == value_type
                && !source_type.is_address()
                && matches!(source_type.bits(), 8 | 16 | 32 | 64)
                && *source_type != value_type
                && !source_type.can_widen_to(value_type)
                && source_type.can_exact_cast_to(value_type)
                && matches!(
                    operand.as_ref(),
                    ScalarTerm::Value {
                        id,
                        scalar_type: ScalarType::Integer(root_type),
                    } if *root_type == *source_type && machine_parameter_values.contains(id)
                ) =>
            {
                return Some(exact_integer_shifted_value_interval_obligation(
                    *source_type,
                    value_type,
                    (**operand).clone(),
                    cumulative_count,
                ));
            }
            _ => return None,
        }
    }
    None
}

fn exact_integer_shifted_value_interval_obligation(
    root_type: psi_core::IntegerType,
    value_type: psi_core::IntegerType,
    root: ScalarTerm,
    cumulative_count: u128,
) -> Proposition {
    if cumulative_count == 0 {
        return Proposition::Truth;
    }
    if cumulative_count >= u128::from(value_type.bits()) {
        return exact_integer_source_interval_obligation(root_type, root, 0, 0);
    }
    let count = u32::try_from(cumulative_count).expect("count below native width fits u32");
    let (target_minimum, target_maximum) = match value_type.sign() {
        IntegerSign::Unsigned => {
            let IntegerValue::Unsigned(maximum) = value_type.maximum_value() else {
                unreachable!("unsigned fixed integer type has an unsigned maximum")
            };
            let Some(maximum) = i128::try_from(maximum >> count).ok() else {
                return Proposition::Falsehood;
            };
            (0, maximum)
        }
        IntegerSign::Signed => {
            let (IntegerValue::Signed(minimum), IntegerValue::Signed(maximum)) =
                (value_type.minimum_value(), value_type.maximum_value())
            else {
                unreachable!("signed fixed integer type has signed bounds")
            };
            (minimum >> count, maximum >> count)
        }
    };
    exact_integer_source_interval_obligation(root_type, root, target_minimum, target_maximum)
}

fn exact_integer_shift_left_chain_obligation(
    value_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    count: u128,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if value_type.is_address() || !matches!(value_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let mut cumulative_count = count;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let mut followed_definition = false;
    for _ in 0..prior_axiom_count {
        let Some((definition_index, definition)) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })
        else {
            break;
        };
        let ScalarTerm::ExactIntegerShiftLeft {
            value_type: nested_value_type,
            count_type,
            value: nested_value,
            count: nested_count,
        } = definition
        else {
            break;
        };
        if *nested_value_type != value_type {
            break;
        }
        let Some(nested_count) = landed_exact_shift_count(
            value_type,
            *count_type,
            nested_count,
            semantic_axioms,
            definition_index,
        ) else {
            break;
        };
        let Some(total) = cumulative_count.checked_add(nested_count) else {
            return Some(Proposition::Falsehood);
        };
        cumulative_count = total;
        value = (**nested_value).clone();
        prior_axiom_count = definition_index;
        followed_definition = true;
    }
    if !followed_definition
        || !matches!(
            &value,
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == value_type && machine_parameter_values.contains(id)
        )
    {
        return None;
    }
    Some(exact_integer_cumulative_shift_left_obligation(
        value_type,
        value,
        cumulative_count,
    ))
}

fn landed_exact_shift_count(
    value_type: psi_core::IntegerType,
    count_type: psi_core::IntegerType,
    count: &ScalarTerm,
    semantic_axioms: &[Proposition],
    prior_axiom_count: usize,
) -> Option<u128> {
    if count_type.is_address() || !matches!(count_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let count =
        landed_integer_constant_value(count_type, count, semantic_axioms, prior_axiom_count)?;
    let count = match count {
        IntegerValue::Signed(count) => u128::try_from(count).ok()?,
        IntegerValue::Unsigned(count) => count,
    };
    (count < u128::from(value_type.bits())).then_some(count)
}

fn exact_integer_cumulative_shift_left_obligation(
    value_type: psi_core::IntegerType,
    value: ScalarTerm,
    cumulative_count: u128,
) -> Proposition {
    if cumulative_count == 0 {
        return Proposition::Truth;
    }
    if cumulative_count < u128::from(value_type.bits()) {
        let mut bounds = Vec::with_capacity(2);
        append_exact_shift_left_value_bounds(
            &mut bounds,
            value_type,
            value,
            u32::try_from(cumulative_count).expect("count below native width fits u32"),
        );
        return canonical_conjunction(bounds);
    }
    let zero = ScalarTerm::integer(
        value_type,
        match value_type.sign() {
            IntegerSign::Signed => IntegerValue::Signed(0),
            IntegerSign::Unsigned => IntegerValue::Unsigned(0),
        },
    )
    .expect("fixed integer value types admit zero");
    match value_type.sign() {
        IntegerSign::Unsigned => Proposition::LessOrEqual(value, zero),
        IntegerSign::Signed => canonical_conjunction(vec![
            Proposition::LessOrEqual(zero.clone(), value.clone()),
            Proposition::LessOrEqual(value, zero),
        ]),
    }
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

fn exact_integer_divide_remainder_chain_cast_obligation(
    source_type: psi_core::IntegerType,
    target_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if source_type.is_address()
        || target_type.is_address()
        || !matches!(source_type.bits(), 8 | 16 | 32 | 64)
        || !matches!(target_type.bits(), 8 | 16 | 32 | 64)
        || source_type == target_type
        || source_type.can_widen_to(target_type)
        || !source_type.can_exact_cast_to(target_type)
    {
        return None;
    }
    let target_interval = fixed_integer_type_interval(target_type)?;
    let mut transfers = Vec::new();
    let mut prior_axiom_count = semantic_axioms.len();
    let mut followed_definition = false;
    for _ in 0..prior_axiom_count {
        let Some((definition_index, definition)) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })
        else {
            break;
        };
        let (left, right, transfer) = match definition {
            ScalarTerm::ExactIntegerDivide {
                scalar_type,
                left,
                right,
            } if *scalar_type == source_type => {
                (left, right, ExactIntegerDivideRemainderTransfer::Divide)
            }
            ScalarTerm::ExactIntegerRemainder {
                scalar_type,
                left,
                right,
            } if *scalar_type == source_type => {
                (left, right, ExactIntegerDivideRemainderTransfer::Remainder)
            }
            _ => break,
        };
        if landed_integer_constant_value(source_type, left, semantic_axioms, definition_index)
            .is_some()
        {
            break;
        }
        let divisor =
            landed_integer_constant_value(source_type, right, semantic_axioms, definition_index)
                .and_then(|value| fixed_integer_value(source_type, value))?;
        if divisor == 0 || divisor == -1 {
            break;
        }
        transfers.push((transfer, divisor));
        value = (**left).clone();
        prior_axiom_count = definition_index;
        followed_definition = true;
    }
    if !followed_definition
        || !matches!(
            &value,
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == source_type && machine_parameter_values.contains(id)
        )
    {
        return None;
    }
    let final_interval = transfers.into_iter().rev().try_fold(
        fixed_integer_type_interval(source_type)?,
        |interval, (transfer, divisor)| {
            exact_integer_divide_remainder_interval_transfer(interval, transfer, divisor)
        },
    )?;
    (final_interval.0 >= target_interval.0 && final_interval.1 <= target_interval.1)
        .then_some(Proposition::Truth)
}

fn exact_integer_divide_remainder_chain_hull(
    source_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<(i128, i128)> {
    if source_type.is_address() || !matches!(source_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let mut transfers = Vec::new();
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let mut followed_definition = false;
    for _ in 0..=prior_axiom_count {
        if matches!(
            &value,
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if followed_definition
                && *root_type == source_type
                && machine_parameter_values.contains(id)
        ) {
            return transfers.into_iter().rev().try_fold(
                fixed_integer_type_interval(source_type)?,
                |interval, (transfer, divisor)| {
                    exact_integer_divide_remainder_interval_transfer(interval, transfer, divisor)
                },
            );
        }
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })?;
        let (left, right, transfer) = match definition {
            ScalarTerm::ExactIntegerDivide {
                scalar_type,
                left,
                right,
            } if *scalar_type == source_type => {
                (left, right, ExactIntegerDivideRemainderTransfer::Divide)
            }
            ScalarTerm::ExactIntegerRemainder {
                scalar_type,
                left,
                right,
            } if *scalar_type == source_type => {
                (left, right, ExactIntegerDivideRemainderTransfer::Remainder)
            }
            _ => return None,
        };
        if landed_integer_constant_value(source_type, left, semantic_axioms, definition_index)
            .is_some()
        {
            return None;
        }
        let divisor =
            landed_integer_constant_value(source_type, right, semantic_axioms, definition_index)
                .and_then(|value| fixed_integer_value(source_type, value))?;
        if divisor == 0 || divisor == -1 {
            return None;
        }
        transfers.push((transfer, divisor));
        value = (**left).clone();
        prior_axiom_count = definition_index;
        followed_definition = true;
    }
    None
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

fn exact_integer_divide_remainder_interval_transfer(
    (minimum, maximum): (i128, i128),
    transfer: ExactIntegerDivideRemainderTransfer,
    divisor: i128,
) -> Option<(i128, i128)> {
    if divisor == 0 || divisor == -1 {
        return None;
    }
    match transfer {
        ExactIntegerDivideRemainderTransfer::Divide if divisor > 0 => {
            Some((minimum / divisor, maximum / divisor))
        }
        ExactIntegerDivideRemainderTransfer::Divide => Some((maximum / divisor, minimum / divisor)),
        ExactIntegerDivideRemainderTransfer::Remainder => {
            let magnitude = divisor.checked_abs()?;
            let remainder_maximum = magnitude.checked_sub(1)?;
            if minimum >= 0 {
                Some((0, maximum.min(remainder_maximum)))
            } else if maximum <= 0 {
                Some((minimum.max(-remainder_maximum), 0))
            } else {
                Some((
                    minimum.max(-remainder_maximum),
                    maximum.min(remainder_maximum),
                ))
            }
        }
    }
}

fn exact_integer_shift_right_chain_cast_obligation(
    source_type: psi_core::IntegerType,
    target_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if source_type.is_address()
        || target_type.is_address()
        || !matches!(source_type.bits(), 8 | 16 | 32 | 64)
        || !matches!(target_type.bits(), 8 | 16 | 32 | 64)
        || source_type == target_type
        || source_type.can_widen_to(target_type)
        || !source_type.can_exact_cast_to(target_type)
    {
        return None;
    }
    let mut cumulative_count = 0_u128;
    let mut prior_axiom_count = semantic_axioms.len();
    let mut followed_definition = false;
    for _ in 0..prior_axiom_count {
        let Some((definition_index, definition)) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })
        else {
            break;
        };
        let ScalarTerm::ExactIntegerShiftRight {
            value_type,
            count_type,
            value: nested_value,
            count,
        } = definition
        else {
            break;
        };
        if *value_type != source_type {
            break;
        }
        let Some(count) = landed_exact_shift_count(
            source_type,
            *count_type,
            count,
            semantic_axioms,
            definition_index,
        ) else {
            break;
        };
        let Some(total) = cumulative_count.checked_add(count) else {
            return Some(Proposition::Falsehood);
        };
        cumulative_count = total;
        value = (**nested_value).clone();
        prior_axiom_count = definition_index;
        followed_definition = true;
    }
    if !followed_definition
        || !matches!(
            &value,
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == source_type && machine_parameter_values.contains(id)
        )
    {
        return None;
    }
    Some(exact_integer_shift_right_chain_cast_interval_obligation(
        source_type,
        target_type,
        value,
        cumulative_count,
    ))
}

fn exact_integer_shift_right_chain_cast_interval_obligation(
    root_type: psi_core::IntegerType,
    target_type: psi_core::IntegerType,
    root: ScalarTerm,
    cumulative_count: u128,
) -> Proposition {
    if cumulative_count >= u128::from(root_type.bits()) {
        return match (root_type.sign(), target_type.sign()) {
            (IntegerSign::Signed, IntegerSign::Unsigned) => {
                exact_integer_source_interval_obligation(root_type, root, 0, i128::MAX)
            }
            _ => Proposition::Truth,
        };
    }
    let count = u32::try_from(cumulative_count).expect("count below native width fits u32");
    let target_minimum = match target_type.minimum_value() {
        IntegerValue::Signed(minimum) => minimum.checked_shl(count),
        IntegerValue::Unsigned(_) => Some(0),
    };
    let target_maximum = match target_type.maximum_value() {
        IntegerValue::Signed(maximum) => u128::try_from(maximum).ok(),
        IntegerValue::Unsigned(maximum) => Some(maximum),
    }
    .and_then(|maximum| maximum.checked_add(1))
    .and_then(|exclusive| exclusive.checked_shl(count))
    .and_then(|exclusive| exclusive.checked_sub(1))
    .and_then(|maximum| i128::try_from(maximum).ok());
    let (Some(target_minimum), Some(target_maximum)) = (target_minimum, target_maximum) else {
        return Proposition::Falsehood;
    };
    exact_integer_source_interval_obligation(root_type, root, target_minimum, target_maximum)
}

fn exact_integer_shift_left_chain_cast_obligation(
    source_type: psi_core::IntegerType,
    target_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if source_type.is_address()
        || target_type.is_address()
        || !matches!(source_type.bits(), 8 | 16 | 32 | 64)
        || !matches!(target_type.bits(), 8 | 16 | 32 | 64)
        || source_type == target_type
        || source_type.can_widen_to(target_type)
        || !source_type.can_exact_cast_to(target_type)
    {
        return None;
    }
    let mut cumulative_count = 0_u128;
    let mut prior_axiom_count = semantic_axioms.len();
    let mut followed_definition = false;
    for _ in 0..prior_axiom_count {
        let Some((definition_index, definition)) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })
        else {
            break;
        };
        let ScalarTerm::ExactIntegerShiftLeft {
            value_type,
            count_type,
            value: nested_value,
            count,
        } = definition
        else {
            break;
        };
        if *value_type != source_type {
            break;
        }
        let Some(count) = landed_exact_shift_count(
            source_type,
            *count_type,
            count,
            semantic_axioms,
            definition_index,
        ) else {
            break;
        };
        let Some(total) = cumulative_count.checked_add(count) else {
            return Some(Proposition::Falsehood);
        };
        cumulative_count = total;
        value = (**nested_value).clone();
        prior_axiom_count = definition_index;
        followed_definition = true;
    }
    if !followed_definition
        || !matches!(
            &value,
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == source_type && machine_parameter_values.contains(id)
        )
    {
        return None;
    }
    Some(exact_integer_shift_chain_cast_interval_obligation(
        source_type,
        target_type,
        value,
        cumulative_count,
    ))
}

fn exact_integer_shift_chain_cast_interval_obligation(
    root_type: psi_core::IntegerType,
    target_type: psi_core::IntegerType,
    root: ScalarTerm,
    cumulative_count: u128,
) -> Proposition {
    if cumulative_count >= u128::from(root_type.bits()) {
        return Proposition::Truth;
    }
    let count = u32::try_from(cumulative_count).expect("count below native width fits u32");
    let (target_minimum, target_maximum) = match target_type.sign() {
        IntegerSign::Unsigned => {
            let IntegerValue::Unsigned(maximum) = target_type.maximum_value() else {
                unreachable!("unsigned fixed integer type has an unsigned maximum")
            };
            let Some(maximum) = i128::try_from(maximum >> count).ok() else {
                return Proposition::Falsehood;
            };
            (0, maximum)
        }
        IntegerSign::Signed => {
            let (IntegerValue::Signed(minimum), IntegerValue::Signed(maximum)) =
                (target_type.minimum_value(), target_type.maximum_value())
            else {
                unreachable!("signed fixed integer type has signed bounds")
            };
            let Some(minimum) = signed_negative_magnitude(minimum.unsigned_abs() >> count) else {
                return Proposition::Falsehood;
            };
            (minimum, maximum >> count)
        }
    };
    exact_integer_source_interval_obligation(root_type, root, target_minimum, target_maximum)
}

fn exact_integer_multiply_chain_cast_obligation(
    source_type: psi_core::IntegerType,
    target_type: psi_core::IntegerType,
    mut variable: ScalarTerm,
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if source_type.is_address()
        || target_type.is_address()
        || !matches!(source_type.bits(), 8 | 16 | 32 | 64)
        || !matches!(target_type.bits(), 8 | 16 | 32 | 64)
        || source_type == target_type
        || source_type.can_widen_to(target_type)
        || !source_type.can_exact_cast_to(target_type)
    {
        return None;
    }
    let mut cumulative_factor = 1_u128;
    let mut prior_axiom_count = semantic_axioms.len();
    let mut followed_definition = false;
    for _ in 0..prior_axiom_count {
        let Some((definition_index, definition)) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &variable => Some((index, right)),
                _ => None,
            })
        else {
            break;
        };
        let ScalarTerm::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
        } = definition
        else {
            break;
        };
        if *scalar_type != source_type
            || landed_integer_constant_value(source_type, left, semantic_axioms, definition_index)
                .is_some()
        {
            break;
        }
        let Some(factor) =
            landed_integer_constant_value(source_type, right, semantic_axioms, definition_index)
                .and_then(|factor| nonnegative_integer_factor(source_type, factor))
        else {
            break;
        };
        let Some(product) = cumulative_factor.checked_mul(factor) else {
            return Some(Proposition::Falsehood);
        };
        cumulative_factor = product;
        variable = (**left).clone();
        prior_axiom_count = definition_index;
        followed_definition = true;
    }
    if !followed_definition
        || !matches!(
            &variable,
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == source_type && machine_parameter_values.contains(id)
        )
    {
        return None;
    }
    Some(exact_integer_product_cast_interval_obligation(
        source_type,
        target_type,
        variable,
        cumulative_factor,
    ))
}

fn exact_integer_signed_affine_chain_cast_obligation(
    source_type: psi_core::IntegerType,
    target_type: psi_core::IntegerType,
    operand: ScalarTerm,
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if !partial_fixed_native_integer_cast(source_type, target_type) {
        return None;
    }
    let (variable, coefficient, offset, _, saw_offset, saw_negative_factor) =
        exact_integer_signed_affine_replay(
            source_type,
            operand,
            IntegerOffset::Nonnegative(1),
            IntegerOffset::Nonnegative(0),
            false,
            false,
            semantic_axioms,
            semantic_axioms.len(),
        )?;
    if !saw_offset
        || !saw_negative_factor
        || !matches!(
            &variable,
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == source_type && machine_parameter_values.contains(id)
        )
    {
        return None;
    }
    exact_integer_signed_affine_interval_obligation(
        source_type,
        variable,
        coefficient,
        offset,
        fixed_integer_type_interval(target_type)?,
    )
}

fn exact_integer_signed_multiply_chain_cast_obligation(
    source_type: psi_core::IntegerType,
    target_type: psi_core::IntegerType,
    mut variable: ScalarTerm,
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if source_type.sign() != IntegerSign::Signed
        || source_type.is_address()
        || target_type.is_address()
        || !matches!(source_type.bits(), 8 | 16 | 32 | 64)
        || !matches!(target_type.bits(), 8 | 16 | 32 | 64)
        || source_type == target_type
        || source_type.can_widen_to(target_type)
        || !source_type.can_exact_cast_to(target_type)
    {
        return None;
    }
    let mut product = Some(IntegerOffset::Nonnegative(1));
    let mut saw_negative = false;
    let mut prior_axiom_count = semantic_axioms.len();
    let mut followed_definition = false;
    for _ in 0..prior_axiom_count {
        let Some((definition_index, definition)) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &variable => Some((index, right)),
                _ => None,
            })
        else {
            break;
        };
        let ScalarTerm::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
        } = definition
        else {
            break;
        };
        if *scalar_type != source_type
            || landed_integer_constant_value(source_type, left, semantic_axioms, definition_index)
                .is_some()
        {
            break;
        }
        let factor =
            landed_integer_constant_value(source_type, right, semantic_axioms, definition_index)?;
        let IntegerValue::Signed(factor_value) = factor else {
            return None;
        };
        product = checked_signed_integer_product(product, factor);
        saw_negative |= factor_value < 0;
        variable = (**left).clone();
        prior_axiom_count = definition_index;
        followed_definition = true;
    }
    if !followed_definition
        || !saw_negative
        || !matches!(
            &variable,
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == source_type && machine_parameter_values.contains(id)
        )
    {
        return None;
    }
    exact_integer_signed_product_interval_obligation(source_type, target_type, variable, product?)
}

fn exact_integer_signed_product_interval_obligation(
    root_type: psi_core::IntegerType,
    interval_type: psi_core::IntegerType,
    root: ScalarTerm,
    product: IntegerOffset,
) -> Option<Proposition> {
    if product.magnitude() == 0 {
        return Some(Proposition::Truth);
    }
    let interval_minimum = integer_value_as_i128(interval_type.minimum_value())?;
    let interval_maximum = integer_value_as_i128(interval_type.maximum_value())?;
    let magnitude = product.magnitude();
    let (minimum, maximum) = if magnitude > i128::MAX as u128 {
        (0, 0)
    } else {
        let magnitude = i128::try_from(magnitude).ok()?;
        let signed_product = match product {
            IntegerOffset::Nonnegative(_) => magnitude,
            IntegerOffset::Negative(_) => magnitude.checked_neg()?,
        };
        if signed_product > 0 {
            (
                checked_integer_ceil_division(interval_minimum, signed_product)?,
                checked_integer_floor_division(interval_maximum, signed_product)?,
            )
        } else {
            (
                checked_integer_ceil_division(interval_maximum, signed_product)?,
                checked_integer_floor_division(interval_minimum, signed_product)?,
            )
        }
    };
    Some(exact_integer_source_interval_obligation(
        root_type, root, minimum, maximum,
    ))
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

fn exact_integer_affine_chain_cast_obligation(
    source_type: psi_core::IntegerType,
    target_type: psi_core::IntegerType,
    mut variable: ScalarTerm,
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if source_type.is_address()
        || target_type.is_address()
        || !matches!(source_type.bits(), 8 | 16 | 32 | 64)
        || !matches!(target_type.bits(), 8 | 16 | 32 | 64)
        || source_type == target_type
        || source_type.can_widen_to(target_type)
        || !source_type.can_exact_cast_to(target_type)
    {
        return None;
    }
    let mut coefficient = 1_u128;
    let mut offset = IntegerOffset::Nonnegative(0);
    let mut saw_offset = false;
    let mut saw_multiply = false;
    let mut followed_definition = false;
    let mut prior_axiom_count = semantic_axioms.len();
    for _ in 0..prior_axiom_count {
        let Some((definition_index, definition)) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &variable => Some((index, right)),
                _ => None,
            })
        else {
            break;
        };
        let (left, right, nested_coefficient, nested_offset, operation) = match definition {
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            } if *scalar_type == source_type => (
                left,
                right,
                1,
                IntegerOffset::from_value(landed_integer_constant_value(
                    source_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?),
                ExactIntegerAffineOperation::Add,
            ),
            ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } if *scalar_type == source_type => (
                left,
                right,
                1,
                IntegerOffset::from_subtrahend(landed_integer_constant_value(
                    source_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?),
                ExactIntegerAffineOperation::Subtract,
            ),
            ScalarTerm::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } if *scalar_type == source_type => (
                left,
                right,
                nonnegative_integer_factor(
                    source_type,
                    landed_integer_constant_value(
                        source_type,
                        right,
                        semantic_axioms,
                        definition_index,
                    )?,
                )?,
                IntegerOffset::Nonnegative(0),
                ExactIntegerAffineOperation::Multiply,
            ),
            _ => break,
        };
        if landed_integer_constant_value(source_type, left, semantic_axioms, definition_index)
            .is_some()
            || landed_integer_constant_value(source_type, right, semantic_axioms, definition_index)
                .is_none()
        {
            break;
        }
        let Some(composed_offset) = nested_offset
            .checked_multiply(coefficient)
            .and_then(|nested| nested.checked_add(offset))
        else {
            return Some(Proposition::Falsehood);
        };
        let Some(composed_coefficient) = coefficient.checked_mul(nested_coefficient) else {
            return Some(Proposition::Falsehood);
        };
        coefficient = composed_coefficient;
        offset = composed_offset;
        variable = (**left).clone();
        prior_axiom_count = definition_index;
        followed_definition = true;
        saw_offset |= operation != ExactIntegerAffineOperation::Multiply;
        saw_multiply |= operation == ExactIntegerAffineOperation::Multiply;
    }
    if !followed_definition
        || !saw_offset
        || !saw_multiply
        || !matches!(
            &variable,
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == source_type && machine_parameter_values.contains(id)
        )
    {
        return None;
    }
    Some(exact_integer_affine_target_interval_obligation(
        source_type,
        target_type,
        variable,
        coefficient,
        offset,
    ))
}

fn exact_integer_product_cast_interval_obligation(
    root_type: psi_core::IntegerType,
    target_type: psi_core::IntegerType,
    root: ScalarTerm,
    cumulative_factor: u128,
) -> Proposition {
    if cumulative_factor == 0 {
        return Proposition::Truth;
    }
    let (target_minimum, target_maximum) = match target_type.sign() {
        IntegerSign::Unsigned => {
            let IntegerValue::Unsigned(maximum) = target_type.maximum_value() else {
                unreachable!("unsigned fixed integer type has an unsigned maximum")
            };
            let Some(maximum) = i128::try_from(maximum / cumulative_factor).ok() else {
                return Proposition::Falsehood;
            };
            (0, maximum)
        }
        IntegerSign::Signed => {
            let (IntegerValue::Signed(minimum), IntegerValue::Signed(maximum)) =
                (target_type.minimum_value(), target_type.maximum_value())
            else {
                unreachable!("signed fixed integer type has signed bounds")
            };
            let Some(minimum) =
                signed_negative_magnitude(minimum.unsigned_abs() / cumulative_factor)
            else {
                return Proposition::Falsehood;
            };
            let Some(maximum) = i128::try_from(maximum as u128 / cumulative_factor).ok() else {
                return Proposition::Falsehood;
            };
            (minimum, maximum)
        }
    };
    exact_integer_source_interval_obligation(root_type, root, target_minimum, target_maximum)
}

fn exact_integer_offset_chain_cast_obligation(
    source_type: psi_core::IntegerType,
    target_type: psi_core::IntegerType,
    mut variable: ScalarTerm,
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if source_type.is_address()
        || target_type.is_address()
        || !matches!(source_type.bits(), 8 | 16 | 32 | 64)
        || !matches!(target_type.bits(), 8 | 16 | 32 | 64)
        || source_type == target_type
        || source_type.can_widen_to(target_type)
        || !source_type.can_exact_cast_to(target_type)
    {
        return None;
    }

    let mut offset = IntegerOffset::Nonnegative(0);
    let mut prior_axiom_count = semantic_axioms.len();
    let mut followed_definition = false;
    for _ in 0..prior_axiom_count {
        let Some((definition_index, definition)) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &variable => Some((index, right)),
                _ => None,
            })
        else {
            break;
        };
        let (left, right, operation) = match definition {
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            } if *scalar_type == source_type => (left, right, ExactIntegerOffsetOperation::Add),
            ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } if *scalar_type == source_type => {
                (left, right, ExactIntegerOffsetOperation::Subtract)
            }
            _ => break,
        };
        if landed_integer_constant_value(source_type, left, semantic_axioms, definition_index)
            .is_some()
        {
            break;
        }
        let Some(constant) =
            landed_integer_constant_value(source_type, right, semantic_axioms, definition_index)
        else {
            break;
        };
        let nested_offset = match operation {
            ExactIntegerOffsetOperation::Add => IntegerOffset::from_value(constant),
            ExactIntegerOffsetOperation::Subtract => IntegerOffset::from_subtrahend(constant),
        };
        let Some(combined) = offset.checked_add(nested_offset) else {
            return Some(Proposition::Falsehood);
        };
        if combined.magnitude() > integer_type_span(source_type) {
            return Some(Proposition::Falsehood);
        }
        offset = combined;
        variable = (**left).clone();
        prior_axiom_count = definition_index;
        followed_definition = true;
    }
    if !followed_definition
        || !matches!(
            &variable,
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == source_type && machine_parameter_values.contains(id)
        )
    {
        return None;
    }

    Some(exact_integer_shifted_interval_obligation(
        source_type,
        target_type,
        variable,
        offset,
    ))
}

fn exact_integer_cast_then_offset_obligation(
    target_type: psi_core::IntegerType,
    mut variable: ScalarTerm,
    initial_offset: IntegerOffset,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if target_type.is_address() || !matches!(target_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let mut offset = initial_offset;
    if offset.magnitude() > integer_type_span(target_type) {
        return Some(Proposition::Falsehood);
    }
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    for _ in 0..=prior_axiom_count {
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &variable => Some((index, right)),
                _ => None,
            })?;
        match definition {
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } if *scalar_type == target_type => {
                if landed_integer_constant_value(
                    target_type,
                    left,
                    semantic_axioms,
                    definition_index,
                )
                .is_some()
                {
                    return None;
                }
                let constant = landed_integer_constant_value(
                    target_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?;
                let nested_offset = match definition {
                    ScalarTerm::ExactIntegerAdd { .. } => IntegerOffset::from_value(constant),
                    ScalarTerm::ExactIntegerSubtract { .. } => {
                        IntegerOffset::from_subtrahend(constant)
                    }
                    _ => unreachable!("matched one exact offset definition"),
                };
                let Some(combined) = offset.checked_add(nested_offset) else {
                    return Some(Proposition::Falsehood);
                };
                if combined.magnitude() > integer_type_span(target_type) {
                    return Some(Proposition::Falsehood);
                }
                offset = combined;
                variable = (**left).clone();
                prior_axiom_count = definition_index;
            }
            ScalarTerm::IntegerExactCast {
                source_type,
                target_type: cast_target_type,
                operand,
            } if *cast_target_type == target_type
                && !source_type.is_address()
                && matches!(source_type.bits(), 8 | 16 | 32 | 64)
                && *source_type != target_type
                && !source_type.can_widen_to(target_type)
                && source_type.can_exact_cast_to(target_type)
                && matches!(
                    operand.as_ref(),
                    ScalarTerm::Value {
                        id,
                        scalar_type: ScalarType::Integer(root_type),
                    } if *root_type == *source_type && machine_parameter_values.contains(id)
                ) =>
            {
                return Some(exact_integer_shifted_interval_obligation(
                    *source_type,
                    target_type,
                    (**operand).clone(),
                    offset,
                ));
            }
            _ => return None,
        }
    }
    None
}

fn exact_integer_shifted_interval_obligation(
    root_type: psi_core::IntegerType,
    interval_type: psi_core::IntegerType,
    root: ScalarTerm,
    offset: IntegerOffset,
) -> Proposition {
    let translate_target_boundary = |boundary: i128| match offset {
        IntegerOffset::Nonnegative(magnitude) => {
            boundary.checked_sub(i128::try_from(magnitude).ok()?)
        }
        IntegerOffset::Negative(magnitude) => boundary.checked_add(i128::try_from(magnitude).ok()?),
    };
    let Some(target_minimum) =
        integer_value_as_i128(interval_type.minimum_value()).and_then(translate_target_boundary)
    else {
        return Proposition::Falsehood;
    };
    let Some(target_maximum) =
        integer_value_as_i128(interval_type.maximum_value()).and_then(translate_target_boundary)
    else {
        return Proposition::Falsehood;
    };
    exact_integer_source_interval_obligation(root_type, root, target_minimum, target_maximum)
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

fn exact_integer_signed_affine_initial_form(
    constant: IntegerValue,
    operation: ExactIntegerAffineOperation,
) -> Option<(IntegerOffset, IntegerOffset, bool, bool)> {
    let IntegerValue::Signed(constant_value) = constant else {
        return None;
    };
    let constant = IntegerOffset::from_value(constant);
    Some(match operation {
        ExactIntegerAffineOperation::Add => (IntegerOffset::Nonnegative(1), constant, true, false),
        ExactIntegerAffineOperation::Subtract => (
            IntegerOffset::Nonnegative(1),
            constant.negated(),
            true,
            false,
        ),
        ExactIntegerAffineOperation::Multiply => (
            constant,
            IntegerOffset::Nonnegative(0),
            false,
            constant_value < 0,
        ),
    })
}

fn exact_integer_signed_affine_replay(
    integer_type: psi_core::IntegerType,
    mut variable: ScalarTerm,
    mut coefficient: IntegerOffset,
    mut offset: IntegerOffset,
    mut saw_offset: bool,
    mut saw_negative_factor: bool,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
) -> Option<(ScalarTerm, IntegerOffset, IntegerOffset, usize, bool, bool)> {
    if integer_type.sign() != IntegerSign::Signed
        || integer_type.is_address()
        || !matches!(integer_type.bits(), 8 | 16 | 32 | 64)
    {
        return None;
    }
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    for _ in 0..prior_axiom_count {
        let Some((definition_index, definition)) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &variable => Some((index, right)),
                _ => None,
            })
        else {
            break;
        };
        let (left, right, nested_coefficient, nested_offset, operation) = match definition {
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            } if *scalar_type == integer_type => (
                left,
                right,
                IntegerOffset::Nonnegative(1),
                IntegerOffset::from_value(landed_integer_constant_value(
                    integer_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?),
                ExactIntegerAffineOperation::Add,
            ),
            ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } if *scalar_type == integer_type => (
                left,
                right,
                IntegerOffset::Nonnegative(1),
                IntegerOffset::from_subtrahend(landed_integer_constant_value(
                    integer_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?),
                ExactIntegerAffineOperation::Subtract,
            ),
            ScalarTerm::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } if *scalar_type == integer_type => {
                let factor = landed_integer_constant_value(
                    integer_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?;
                let IntegerValue::Signed(_) = factor else {
                    return None;
                };
                (
                    left,
                    right,
                    IntegerOffset::from_value(factor),
                    IntegerOffset::Nonnegative(0),
                    ExactIntegerAffineOperation::Multiply,
                )
            }
            _ => break,
        };
        if landed_integer_constant_value(integer_type, left, semantic_axioms, definition_index)
            .is_some()
            || landed_integer_constant_value(integer_type, right, semantic_axioms, definition_index)
                .is_none()
        {
            return None;
        }
        offset = nested_offset
            .checked_multiply_offset(coefficient)
            .and_then(|nested| nested.checked_add(offset))?;
        coefficient = coefficient.checked_multiply_offset(nested_coefficient)?;
        if operation == ExactIntegerAffineOperation::Multiply {
            saw_negative_factor |= matches!(nested_coefficient, IntegerOffset::Negative(_));
        } else {
            saw_offset = true;
        }
        variable = (**left).clone();
        prior_axiom_count = definition_index;
    }
    Some((
        variable,
        coefficient,
        offset,
        prior_axiom_count,
        saw_offset,
        saw_negative_factor,
    ))
}

fn exact_integer_signed_affine_preimage_interval(
    coefficient: IntegerOffset,
    offset: IntegerOffset,
    interval: (i128, i128),
) -> Option<ExactIntegerIntervalPreimage> {
    if interval.0 > interval.1 {
        return Some(ExactIntegerIntervalPreimage::Empty);
    }
    if coefficient.magnitude() == 0 {
        let constant = match offset {
            IntegerOffset::Nonnegative(value) => i128::try_from(value).ok(),
            IntegerOffset::Negative(value) => signed_negative_magnitude(value),
        };
        return Some(
            if constant.is_some_and(|value| interval.0 <= value && value <= interval.1) {
                ExactIntegerIntervalPreimage::Interval((i128::MIN, i128::MAX))
            } else {
                ExactIntegerIntervalPreimage::Empty
            },
        );
    }
    let minimum = IntegerOffset::from_value(IntegerValue::Signed(interval.0));
    let maximum = IntegerOffset::from_value(IntegerValue::Signed(interval.1));
    let (lower_numerator, upper_numerator) = match coefficient {
        IntegerOffset::Nonnegative(_) => (
            minimum.checked_add(offset.negated())?,
            maximum.checked_add(offset.negated())?,
        ),
        IntegerOffset::Negative(_) => (
            offset.checked_add(maximum.negated())?,
            offset.checked_add(minimum.negated())?,
        ),
    };
    let lower = integer_offset_ceil_div(lower_numerator, coefficient.magnitude());
    let upper = integer_offset_floor_div(upper_numerator, coefficient.magnitude());
    let lower = match lower {
        IntegerOffset::Nonnegative(value) => match i128::try_from(value) {
            Ok(value) => value,
            Err(_) => return Some(ExactIntegerIntervalPreimage::Empty),
        },
        IntegerOffset::Negative(value) if value > (1_u128 << 127) => i128::MIN,
        IntegerOffset::Negative(value) => signed_negative_magnitude(value)?,
    };
    let upper = match upper {
        IntegerOffset::Nonnegative(value) => i128::try_from(value).unwrap_or(i128::MAX),
        IntegerOffset::Negative(value) if value > (1_u128 << 127) => {
            return Some(ExactIntegerIntervalPreimage::Empty);
        }
        IntegerOffset::Negative(value) => signed_negative_magnitude(value)?,
    };
    Some(if lower > upper {
        ExactIntegerIntervalPreimage::Empty
    } else {
        ExactIntegerIntervalPreimage::Interval((lower, upper))
    })
}

fn exact_integer_signed_affine_interval_obligation(
    root_type: psi_core::IntegerType,
    root: ScalarTerm,
    coefficient: IntegerOffset,
    offset: IntegerOffset,
    interval: (i128, i128),
) -> Option<Proposition> {
    Some(
        match exact_integer_signed_affine_preimage_interval(coefficient, offset, interval)? {
            ExactIntegerIntervalPreimage::Interval((minimum, maximum)) => {
                exact_integer_source_interval_obligation(root_type, root, minimum, maximum)
            }
            ExactIntegerIntervalPreimage::Empty => Proposition::Falsehood,
        },
    )
}

fn exact_integer_signed_affine_chain_obligation(
    integer_type: psi_core::IntegerType,
    variable: ScalarTerm,
    initial_constant: IntegerValue,
    initial_operation: ExactIntegerAffineOperation,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    let (coefficient, offset, saw_offset, saw_negative_factor) =
        exact_integer_signed_affine_initial_form(initial_constant, initial_operation)?;
    let (variable, coefficient, offset, _, saw_offset, saw_negative_factor) =
        exact_integer_signed_affine_replay(
            integer_type,
            variable,
            coefficient,
            offset,
            saw_offset,
            saw_negative_factor,
            semantic_axioms,
            definition_axiom_count,
        )?;
    if !saw_offset
        || !saw_negative_factor
        || !matches!(
            &variable,
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == integer_type && machine_parameter_values.contains(id)
        )
    {
        return None;
    }
    exact_integer_signed_affine_interval_obligation(
        integer_type,
        variable,
        coefficient,
        offset,
        fixed_integer_type_interval(integer_type)?,
    )
}

fn exact_integer_affine_chain_obligation(
    integer_type: psi_core::IntegerType,
    mut variable: ScalarTerm,
    initial_constant: IntegerValue,
    initial_operation: ExactIntegerAffineOperation,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if integer_type.is_address() || !matches!(integer_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let (mut coefficient, mut offset) = match initial_operation {
        ExactIntegerAffineOperation::Add => (1, IntegerOffset::from_value(initial_constant)),
        ExactIntegerAffineOperation::Subtract => {
            (1, IntegerOffset::from_subtrahend(initial_constant))
        }
        ExactIntegerAffineOperation::Multiply => (
            nonnegative_integer_factor(integer_type, initial_constant)?,
            IntegerOffset::Nonnegative(0),
        ),
    };
    let mut saw_offset = initial_operation != ExactIntegerAffineOperation::Multiply;
    let mut saw_multiply = initial_operation == ExactIntegerAffineOperation::Multiply;
    let mut followed_definition = false;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    for _ in 0..prior_axiom_count {
        let Some((definition_index, definition)) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &variable => Some((index, right)),
                _ => None,
            })
        else {
            break;
        };
        let (left, right, nested_coefficient, nested_offset, operation) = match definition {
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            } if *scalar_type == integer_type => (
                left,
                right,
                1,
                IntegerOffset::from_value(landed_integer_constant_value(
                    integer_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?),
                ExactIntegerAffineOperation::Add,
            ),
            ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } if *scalar_type == integer_type => (
                left,
                right,
                1,
                IntegerOffset::from_subtrahend(landed_integer_constant_value(
                    integer_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?),
                ExactIntegerAffineOperation::Subtract,
            ),
            ScalarTerm::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } if *scalar_type == integer_type => (
                left,
                right,
                nonnegative_integer_factor(
                    integer_type,
                    landed_integer_constant_value(
                        integer_type,
                        right,
                        semantic_axioms,
                        definition_index,
                    )?,
                )?,
                IntegerOffset::Nonnegative(0),
                ExactIntegerAffineOperation::Multiply,
            ),
            _ => break,
        };
        if landed_integer_constant_value(integer_type, left, semantic_axioms, definition_index)
            .is_some()
            || landed_integer_constant_value(integer_type, right, semantic_axioms, definition_index)
                .is_none()
        {
            break;
        }
        let Some(composed_offset) = nested_offset
            .checked_multiply(coefficient)
            .and_then(|nested| nested.checked_add(offset))
        else {
            return Some(Proposition::Falsehood);
        };
        let Some(composed_coefficient) = coefficient.checked_mul(nested_coefficient) else {
            return Some(Proposition::Falsehood);
        };
        coefficient = composed_coefficient;
        offset = composed_offset;
        variable = (**left).clone();
        prior_axiom_count = definition_index;
        followed_definition = true;
        saw_offset |= operation != ExactIntegerAffineOperation::Multiply;
        saw_multiply |= operation == ExactIntegerAffineOperation::Multiply;
    }
    if !followed_definition
        || !saw_offset
        || !saw_multiply
        || !matches!(
            &variable,
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == integer_type && machine_parameter_values.contains(id)
        )
    {
        return None;
    }
    Some(exact_integer_affine_interval_obligation(
        integer_type,
        variable,
        coefficient,
        offset,
    ))
}

fn exact_integer_affine_cast_affine_obligation(
    target_type: psi_core::IntegerType,
    mut variable: ScalarTerm,
    initial_constant: IntegerValue,
    initial_operation: ExactIntegerAffineOperation,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if target_type.is_address() || !matches!(target_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let (mut target_coefficient, mut target_offset) = match initial_operation {
        ExactIntegerAffineOperation::Add => (1, IntegerOffset::from_value(initial_constant)),
        ExactIntegerAffineOperation::Subtract => {
            (1, IntegerOffset::from_subtrahend(initial_constant))
        }
        ExactIntegerAffineOperation::Multiply => (
            nonnegative_integer_factor(target_type, initial_constant)?,
            IntegerOffset::Nonnegative(0),
        ),
    };
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let (source_type, mut source_variable) = loop {
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &variable => Some((index, right)),
                _ => None,
            })?;
        match definition {
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } if *scalar_type == target_type => {
                if landed_integer_constant_value(
                    target_type,
                    left,
                    semantic_axioms,
                    definition_index,
                )
                .is_some()
                {
                    return None;
                }
                let constant = landed_integer_constant_value(
                    target_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?;
                let (nested_coefficient, nested_offset) = match definition {
                    ScalarTerm::ExactIntegerAdd { .. } => (1, IntegerOffset::from_value(constant)),
                    ScalarTerm::ExactIntegerSubtract { .. } => {
                        (1, IntegerOffset::from_subtrahend(constant))
                    }
                    ScalarTerm::ExactIntegerMultiply { .. } => (
                        nonnegative_integer_factor(target_type, constant)?,
                        IntegerOffset::Nonnegative(0),
                    ),
                    _ => unreachable!("matched one exact affine definition"),
                };
                target_offset = nested_offset
                    .checked_multiply(target_coefficient)
                    .and_then(|nested| nested.checked_add(target_offset))?;
                target_coefficient = target_coefficient.checked_mul(nested_coefficient)?;
                variable = (**left).clone();
                prior_axiom_count = definition_index;
            }
            ScalarTerm::IntegerExactCast {
                source_type,
                target_type: cast_target_type,
                operand,
            } if *cast_target_type == target_type
                && !source_type.is_address()
                && matches!(source_type.bits(), 8 | 16 | 32 | 64)
                && *source_type != target_type
                && !source_type.can_widen_to(target_type)
                && source_type.can_exact_cast_to(target_type) =>
            {
                prior_axiom_count = definition_index;
                break (*source_type, (**operand).clone());
            }
            _ => return None,
        }
    };

    let target_carrier = fixed_integer_type_interval(target_type)?;
    let target_preimage = if target_coefficient == 0 {
        None
    } else {
        match exact_integer_affine_preimage_interval(
            target_type,
            target_coefficient,
            target_offset,
            target_carrier,
        ) {
            Ok(interval) => interval,
            Err(()) => return None,
        }
    };
    let mut source_coefficient = 1_u128;
    let mut source_offset = IntegerOffset::Nonnegative(0);
    let mut followed_source_definition = false;
    for _ in 0..=prior_axiom_count {
        if matches!(
            &source_variable,
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == source_type && machine_parameter_values.contains(id)
        ) {
            if !followed_source_definition {
                return None;
            }
            if target_coefficient == 0 {
                return Some(if target_offset.is_representable(target_type) {
                    Proposition::Truth
                } else {
                    Proposition::Falsehood
                });
            }
            let Some((target_lower, target_upper)) = target_preimage else {
                return Some(Proposition::Falsehood);
            };
            let source_carrier = fixed_integer_type_interval(source_type)?;
            let lower = target_lower.max(source_carrier.0);
            let upper = target_upper.min(source_carrier.1);
            if lower > upper {
                return Some(Proposition::Falsehood);
            }
            return exact_integer_affine_preimage_obligation(
                source_type,
                source_variable,
                source_coefficient,
                source_offset,
                (lower, upper),
            )
            .ok();
        }
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &source_variable => Some((index, right)),
                _ => None,
            })?;
        let (left, right, nested_coefficient, nested_offset) = match definition {
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            } if *scalar_type == source_type => (
                left,
                right,
                1,
                IntegerOffset::from_value(landed_integer_constant_value(
                    source_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?),
            ),
            ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } if *scalar_type == source_type => (
                left,
                right,
                1,
                IntegerOffset::from_subtrahend(landed_integer_constant_value(
                    source_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?),
            ),
            ScalarTerm::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } if *scalar_type == source_type => (
                left,
                right,
                nonnegative_integer_factor(
                    source_type,
                    landed_integer_constant_value(
                        source_type,
                        right,
                        semantic_axioms,
                        definition_index,
                    )?,
                )?,
                IntegerOffset::Nonnegative(0),
            ),
            _ => return None,
        };
        if landed_integer_constant_value(source_type, left, semantic_axioms, definition_index)
            .is_some()
            || landed_integer_constant_value(source_type, right, semantic_axioms, definition_index)
                .is_none()
        {
            return None;
        }
        source_offset = nested_offset
            .checked_multiply(source_coefficient)
            .and_then(|nested| nested.checked_add(source_offset))?;
        source_coefficient = source_coefficient.checked_mul(nested_coefficient)?;
        source_variable = (**left).clone();
        prior_axiom_count = definition_index;
        followed_source_definition = true;
    }
    None
}

fn exact_integer_signed_affine_cast_affine_obligation(
    target_type: psi_core::IntegerType,
    variable: ScalarTerm,
    initial_constant: IntegerValue,
    initial_operation: ExactIntegerAffineOperation,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    let (target_coefficient, target_offset, target_saw_offset, target_saw_negative_factor) =
        exact_integer_signed_affine_initial_form(initial_constant, initial_operation)?;
    let (
        cast_value,
        target_coefficient,
        target_offset,
        cast_definition_count,
        target_saw_offset,
        target_saw_negative_factor,
    ) = exact_integer_signed_affine_replay(
        target_type,
        variable,
        target_coefficient,
        target_offset,
        target_saw_offset,
        target_saw_negative_factor,
        semantic_axioms,
        definition_axiom_count,
    )?;
    let (cast_definition_index, cast_definition) = semantic_axioms[..cast_definition_count]
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, axiom)| match axiom {
            Proposition::Equal(left, right) if left == &cast_value => Some((index, right)),
            _ => None,
        })?;
    let ScalarTerm::IntegerExactCast {
        source_type,
        target_type: cast_target_type,
        operand,
    } = cast_definition
    else {
        return None;
    };
    if *cast_target_type != target_type
        || source_type.sign() != IntegerSign::Signed
        || !partial_fixed_native_integer_cast(*source_type, target_type)
    {
        return None;
    }
    let (
        source_root,
        source_coefficient,
        source_offset,
        source_definition_count,
        source_saw_offset,
        source_saw_negative_factor,
    ) = exact_integer_signed_affine_replay(
        *source_type,
        (**operand).clone(),
        IntegerOffset::Nonnegative(1),
        IntegerOffset::Nonnegative(0),
        false,
        false,
        semantic_axioms,
        cast_definition_index,
    )?;
    let source_followed_definition = source_definition_count < cast_definition_index;
    if !source_followed_definition
        || !matches!(
            &source_root,
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == *source_type && machine_parameter_values.contains(id)
        )
        || !((source_saw_offset && source_saw_negative_factor)
            || (!source_saw_negative_factor && target_saw_offset && target_saw_negative_factor))
    {
        return None;
    }
    if target_coefficient.magnitude() == 0 {
        return Some(if target_offset.is_representable(target_type) {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        });
    }
    let target_preimage = match exact_integer_signed_affine_preimage_interval(
        target_coefficient,
        target_offset,
        fixed_integer_type_interval(target_type)?,
    )? {
        ExactIntegerIntervalPreimage::Interval(interval) => interval,
        ExactIntegerIntervalPreimage::Empty => return Some(Proposition::Falsehood),
    };
    let source_carrier = fixed_integer_type_interval(*source_type)?;
    let target_carrier = fixed_integer_type_interval(target_type)?;
    let lower = target_preimage
        .0
        .max(source_carrier.0)
        .max(target_carrier.0);
    let upper = target_preimage
        .1
        .min(source_carrier.1)
        .min(target_carrier.1);
    if lower > upper {
        return Some(Proposition::Falsehood);
    }
    exact_integer_signed_affine_interval_obligation(
        *source_type,
        source_root,
        source_coefficient,
        source_offset,
        (lower, upper),
    )
}

fn exact_integer_cast_then_affine_chain_obligation(
    target_type: psi_core::IntegerType,
    mut variable: ScalarTerm,
    initial_constant: IntegerValue,
    initial_operation: ExactIntegerAffineOperation,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if target_type.is_address() || !matches!(target_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let (mut coefficient, mut offset) = match initial_operation {
        ExactIntegerAffineOperation::Add => (1, IntegerOffset::from_value(initial_constant)),
        ExactIntegerAffineOperation::Subtract => {
            (1, IntegerOffset::from_subtrahend(initial_constant))
        }
        ExactIntegerAffineOperation::Multiply => (
            nonnegative_integer_factor(target_type, initial_constant)?,
            IntegerOffset::Nonnegative(0),
        ),
    };
    let mut saw_offset = initial_operation != ExactIntegerAffineOperation::Multiply;
    let mut saw_multiply = initial_operation == ExactIntegerAffineOperation::Multiply;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    for _ in 0..=prior_axiom_count {
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &variable => Some((index, right)),
                _ => None,
            })?;
        let (left, right, nested_coefficient, nested_offset, operation) = match definition {
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            } if *scalar_type == target_type => (
                left,
                right,
                1,
                IntegerOffset::from_value(landed_integer_constant_value(
                    target_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?),
                ExactIntegerAffineOperation::Add,
            ),
            ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } if *scalar_type == target_type => (
                left,
                right,
                1,
                IntegerOffset::from_subtrahend(landed_integer_constant_value(
                    target_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?),
                ExactIntegerAffineOperation::Subtract,
            ),
            ScalarTerm::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } if *scalar_type == target_type => (
                left,
                right,
                nonnegative_integer_factor(
                    target_type,
                    landed_integer_constant_value(
                        target_type,
                        right,
                        semantic_axioms,
                        definition_index,
                    )?,
                )?,
                IntegerOffset::Nonnegative(0),
                ExactIntegerAffineOperation::Multiply,
            ),
            ScalarTerm::IntegerExactCast {
                source_type,
                target_type: cast_target_type,
                operand,
            } if saw_offset
                && saw_multiply
                && *cast_target_type == target_type
                && !source_type.is_address()
                && matches!(source_type.bits(), 8 | 16 | 32 | 64)
                && *source_type != target_type
                && !source_type.can_widen_to(target_type)
                && source_type.can_exact_cast_to(target_type)
                && matches!(
                    operand.as_ref(),
                    ScalarTerm::Value {
                        id,
                        scalar_type: ScalarType::Integer(root_type),
                    } if *root_type == *source_type && machine_parameter_values.contains(id)
                ) =>
            {
                return Some(exact_integer_affine_target_interval_obligation(
                    *source_type,
                    target_type,
                    (**operand).clone(),
                    coefficient,
                    offset,
                ));
            }
            _ => return None,
        };
        if landed_integer_constant_value(target_type, left, semantic_axioms, definition_index)
            .is_some()
            || landed_integer_constant_value(target_type, right, semantic_axioms, definition_index)
                .is_none()
        {
            return None;
        }
        let Some(composed_offset) = nested_offset
            .checked_multiply(coefficient)
            .and_then(|nested| nested.checked_add(offset))
        else {
            return Some(Proposition::Falsehood);
        };
        let Some(composed_coefficient) = coefficient.checked_mul(nested_coefficient) else {
            return Some(Proposition::Falsehood);
        };
        coefficient = composed_coefficient;
        offset = composed_offset;
        variable = (**left).clone();
        prior_axiom_count = definition_index;
        saw_offset |= operation != ExactIntegerAffineOperation::Multiply;
        saw_multiply |= operation == ExactIntegerAffineOperation::Multiply;
    }
    None
}

fn exact_integer_cast_chain_then_affine_suffix_obligation(
    target_type: psi_core::IntegerType,
    mut variable: ScalarTerm,
    initial_constant: IntegerValue,
    initial_operation: ExactIntegerAffineOperation,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    let (mut coefficient, mut offset) = match initial_operation {
        ExactIntegerAffineOperation::Add => (1, IntegerOffset::from_value(initial_constant)),
        ExactIntegerAffineOperation::Subtract => {
            (1, IntegerOffset::from_subtrahend(initial_constant))
        }
        ExactIntegerAffineOperation::Multiply => (
            nonnegative_integer_factor(target_type, initial_constant)?,
            IntegerOffset::Nonnegative(0),
        ),
    };
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    for _ in 0..=prior_axiom_count {
        if let Some((root_type, root, cast_interval)) = exact_integer_cast_chain_root_interval(
            target_type,
            variable.clone(),
            semantic_axioms,
            prior_axiom_count,
            machine_parameter_values,
        ) {
            if coefficient == 0 {
                return Some(if offset.is_representable(target_type) {
                    Proposition::Truth
                } else {
                    Proposition::Falsehood
                });
            }
            let target_interval = fixed_integer_type_interval(target_type)?;
            let preimage = match exact_integer_affine_preimage_interval(
                target_type,
                coefficient,
                offset,
                target_interval,
            ) {
                Ok(Some(interval)) => interval,
                Ok(None) => return Some(Proposition::Falsehood),
                Err(()) => return None,
            };
            let minimum = preimage.0.max(cast_interval.0);
            let maximum = preimage.1.min(cast_interval.1);
            return Some(if minimum <= maximum {
                exact_integer_source_interval_obligation(root_type, root, minimum, maximum)
            } else {
                Proposition::Falsehood
            });
        }
        let target_interval = fixed_integer_type_interval(target_type)?;
        if coefficient == 0 {
            if exact_integer_computed_prefix_conversion_interval_obligation(
                target_type,
                variable.clone(),
                target_interval,
                semantic_axioms,
                prior_axiom_count,
                machine_parameter_values,
            )
            .is_some()
            {
                return Some(if offset.is_representable(target_type) {
                    Proposition::Truth
                } else {
                    Proposition::Falsehood
                });
            }
        } else {
            match exact_integer_affine_preimage_interval(
                target_type,
                coefficient,
                offset,
                target_interval,
            ) {
                Ok(Some(interval)) => {
                    if let Some(obligation) =
                        exact_integer_computed_prefix_conversion_interval_obligation(
                            target_type,
                            variable.clone(),
                            interval,
                            semantic_axioms,
                            prior_axiom_count,
                            machine_parameter_values,
                        )
                    {
                        return Some(obligation);
                    }
                }
                Ok(None) => {
                    if exact_integer_computed_prefix_conversion_interval_obligation(
                        target_type,
                        variable.clone(),
                        target_interval,
                        semantic_axioms,
                        prior_axiom_count,
                        machine_parameter_values,
                    )
                    .is_some()
                    {
                        return Some(Proposition::Falsehood);
                    }
                }
                Err(()) => return None,
            }
        }
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &variable => Some((index, right)),
                _ => None,
            })?;
        let (left, right, nested_coefficient, nested_offset) = match definition {
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            } if *scalar_type == target_type => (
                left,
                right,
                1,
                IntegerOffset::from_value(landed_integer_constant_value(
                    target_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?),
            ),
            ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } if *scalar_type == target_type => (
                left,
                right,
                1,
                IntegerOffset::from_subtrahend(landed_integer_constant_value(
                    target_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?),
            ),
            ScalarTerm::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } if *scalar_type == target_type => (
                left,
                right,
                nonnegative_integer_factor(
                    target_type,
                    landed_integer_constant_value(
                        target_type,
                        right,
                        semantic_axioms,
                        definition_index,
                    )?,
                )?,
                IntegerOffset::Nonnegative(0),
            ),
            _ => return None,
        };
        if landed_integer_constant_value(target_type, left, semantic_axioms, definition_index)
            .is_some()
            || landed_integer_constant_value(target_type, right, semantic_axioms, definition_index)
                .is_none()
        {
            return None;
        }
        offset = nested_offset
            .checked_multiply(coefficient)
            .and_then(|nested| nested.checked_add(offset))?;
        coefficient = coefficient.checked_mul(nested_coefficient)?;
        variable = (**left).clone();
        prior_axiom_count = definition_index;
    }
    None
}

fn exact_integer_affine_interval_obligation(
    integer_type: psi_core::IntegerType,
    root: ScalarTerm,
    coefficient: u128,
    offset: IntegerOffset,
) -> Proposition {
    exact_integer_affine_target_interval_obligation(
        integer_type,
        integer_type,
        root,
        coefficient,
        offset,
    )
}

fn exact_integer_affine_target_interval_obligation(
    root_type: psi_core::IntegerType,
    interval_type: psi_core::IntegerType,
    root: ScalarTerm,
    coefficient: u128,
    offset: IntegerOffset,
) -> Proposition {
    if coefficient == 0 {
        return if offset.is_representable(interval_type) {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        };
    }
    let minimum = IntegerOffset::from_value(interval_type.minimum_value());
    let maximum = IntegerOffset::from_value(interval_type.maximum_value());
    let Some(lower_numerator) = minimum.checked_add(offset.negated()) else {
        return Proposition::Falsehood;
    };
    let Some(upper_numerator) = maximum.checked_add(offset.negated()) else {
        return Proposition::Falsehood;
    };
    let lower = integer_offset_ceil_div(lower_numerator, coefficient);
    let upper = integer_offset_floor_div(upper_numerator, coefficient);
    let lower = match lower {
        IntegerOffset::Nonnegative(value) => match i128::try_from(value) {
            Ok(value) => value,
            Err(_) => return Proposition::Falsehood,
        },
        IntegerOffset::Negative(value) if value > (1_u128 << 127) => i128::MIN,
        IntegerOffset::Negative(value) => match signed_negative_magnitude(value) {
            Some(value) => value,
            None => return Proposition::Falsehood,
        },
    };
    let upper = match upper {
        IntegerOffset::Nonnegative(value) => i128::try_from(value).unwrap_or(i128::MAX),
        IntegerOffset::Negative(value) if value > (1_u128 << 127) => {
            return Proposition::Falsehood;
        }
        IntegerOffset::Negative(value) => match signed_negative_magnitude(value) {
            Some(value) => value,
            None => return Proposition::Falsehood,
        },
    };
    exact_integer_source_interval_obligation(root_type, root, lower, upper)
}

fn integer_offset_floor_div(value: IntegerOffset, divisor: u128) -> IntegerOffset {
    debug_assert_ne!(divisor, 0);
    match value {
        IntegerOffset::Nonnegative(value) => IntegerOffset::Nonnegative(value / divisor),
        IntegerOffset::Negative(value) => {
            let quotient = value / divisor;
            let magnitude = quotient + u128::from(value % divisor != 0);
            if magnitude == 0 {
                IntegerOffset::Nonnegative(0)
            } else {
                IntegerOffset::Negative(magnitude)
            }
        }
    }
}

fn integer_offset_ceil_div(value: IntegerOffset, divisor: u128) -> IntegerOffset {
    debug_assert_ne!(divisor, 0);
    match value {
        IntegerOffset::Nonnegative(value) => {
            let quotient = value / divisor;
            IntegerOffset::Nonnegative(quotient + u128::from(value % divisor != 0))
        }
        IntegerOffset::Negative(value) => {
            let magnitude = value / divisor;
            if magnitude == 0 {
                IntegerOffset::Nonnegative(0)
            } else {
                IntegerOffset::Negative(magnitude)
            }
        }
    }
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
