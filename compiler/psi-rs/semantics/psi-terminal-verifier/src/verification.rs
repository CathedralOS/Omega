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
                    let mut available_bounds = axioms.clone();
                    available_bounds.extend(machine.contract.requires.iter().cloned());
                    operation_obligations.push(ReconstructedOperationObligation {
                        obligation: Obligation {
                            id: obligation,
                            proposition: exact_integer_divide_obligation(
                                integer_type,
                                value_term(left),
                                value_term(right),
                                &available_bounds,
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
                    let mut available_bounds = axioms.clone();
                    available_bounds.extend(machine.contract.requires.iter().cloned());
                    operation_obligations.push(ReconstructedOperationObligation {
                        obligation: Obligation {
                            id: obligation,
                            proposition: exact_integer_remainder_obligation(
                                integer_type,
                                value_term(left),
                                value_term(right),
                                &available_bounds,
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExactIntegerAffineForkBranch {
    root: ScalarTerm,
    coefficient: IntegerOffset,
    offset: IntegerOffset,
    definition_indices: BTreeSet<usize>,
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

fn exact_integer_offset_obligation(
    integer_type: psi_core::IntegerType,
    variable: ScalarTerm,
    offset: IntegerOffset,
) -> Proposition {
    if offset.magnitude() > integer_type_span(integer_type) {
        return Proposition::Falsehood;
    }
    match (integer_type.sign(), offset) {
        (_, IntegerOffset::Nonnegative(0)) | (_, IntegerOffset::Negative(0)) => Proposition::Truth,
        (IntegerSign::Unsigned, IntegerOffset::Nonnegative(offset)) => {
            let IntegerValue::Unsigned(maximum) = integer_type.maximum_value() else {
                unreachable!("unsigned type has unsigned maximum")
            };
            let boundary = ScalarTerm::integer(
                integer_type,
                IntegerValue::Unsigned(maximum.checked_sub(offset).expect("offset fits span")),
            )
            .expect("exact-add unsigned boundary remains in the carrier");
            Proposition::LessOrEqual(variable, boundary)
        }
        (IntegerSign::Unsigned, IntegerOffset::Negative(offset)) => {
            let boundary = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(offset))
                .expect("exact-subtract unsigned boundary remains in the carrier");
            Proposition::LessOrEqual(boundary, variable)
        }
        (IntegerSign::Signed, IntegerOffset::Nonnegative(offset)) => {
            let IntegerValue::Signed(maximum) = integer_type.maximum_value() else {
                unreachable!("signed type has signed maximum")
            };
            let boundary = if offset <= maximum as u128 {
                maximum - offset as i128
            } else {
                signed_negative_magnitude(offset - maximum as u128)
                    .expect("offset within the carrier span has a signed boundary")
            };
            Proposition::LessOrEqual(
                variable,
                ScalarTerm::integer(integer_type, IntegerValue::Signed(boundary))
                    .expect("exact-add signed upper boundary remains in the carrier"),
            )
        }
        (IntegerSign::Signed, IntegerOffset::Negative(offset)) => {
            let IntegerValue::Signed(minimum) = integer_type.minimum_value() else {
                unreachable!("signed type has signed minimum")
            };
            let minimum_magnitude = minimum.unsigned_abs();
            let boundary = if offset < minimum_magnitude {
                signed_negative_magnitude(minimum_magnitude - offset)
                    .expect("offset within the carrier span has a signed boundary")
            } else {
                i128::try_from(offset - minimum_magnitude)
                    .expect("offset within the carrier span has a signed boundary")
            };
            Proposition::LessOrEqual(
                ScalarTerm::integer(integer_type, IntegerValue::Signed(boundary))
                    .expect("exact-add signed lower boundary remains in the carrier"),
                variable,
            )
        }
    }
}

fn exact_integer_add_obligation(
    integer_type: psi_core::IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Proposition {
    let known_left = known_integer_term_value(integer_type, &left, semantic_axioms);
    let known_right = known_integer_term_value(integer_type, &right, semantic_axioms);
    if let (Some(left), Some(right)) = (known_left, known_right) {
        return if integer_type.exact_add(left, right).is_some() {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        };
    }
    if known_left.is_none()
        && known_right.is_none()
        && let Some(obligation) = exact_integer_affine_fork_join_obligation(
            integer_type,
            left.clone(),
            right.clone(),
            ExactIntegerOffsetOperation::Add,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && known_right.is_none()
        && let Some(obligation) = exact_integer_distinct_root_affine_fork_join_obligation(
            integer_type,
            left.clone(),
            right.clone(),
            ExactIntegerOffsetOperation::Add,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && let Some(constant) = known_right
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_shift_then_arithmetic_chain_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Add,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && let Some(constant) = known_right
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_shift_cast_affine_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Add,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && let Some(constant) = known_right
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_signed_affine_cast_affine_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Add,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && let Some(constant) = known_right
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_divide_remainder_then_affine_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Add,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && let Some(constant) = known_right
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_divide_remainder_cast_affine_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Add,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && let Some(constant) = known_right
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_affine_cast_affine_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Add,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && let Some(constant) = known_right
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_cast_chain_then_affine_suffix_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Add,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && let Some(constant) = known_right
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_cast_then_affine_chain_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Add,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && let Some(constant) = known_right
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_cast_then_signed_affine_chain_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Add,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && let Some(constant) = known_right
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_cast_then_offset_obligation(
            integer_type,
            left.clone(),
            IntegerOffset::from_value(constant),
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && let Some(constant) = known_right
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_affine_chain_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Add,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && let Some(constant) = known_right
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_signed_affine_chain_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Add,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && let Some(constant) = known_right
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_mixed_add_subtract_chain_obligation(
            integer_type,
            left.clone(),
            IntegerOffset::from_value(constant),
            ExactIntegerOffsetOperation::Add,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    let (mut variable, constant, constant_term) = match (known_left, known_right) {
        (Some(constant), None) => (right, constant, left),
        (None, Some(constant)) => (left, constant, right),
        (None, None) => {
            if integer_type.sign() == IntegerSign::Unsigned {
                if let Some(bound) = semantic_axioms.iter().rev().find(|axiom| match axiom {
                    Proposition::LessOrEqual(bound_left, bound_right) => {
                        (bound_left == &left
                            && is_maximum_minus(integer_type, bound_right, &right, semantic_axioms))
                            || (bound_left == &right
                                && is_maximum_minus(
                                    integer_type,
                                    bound_right,
                                    &left,
                                    semantic_axioms,
                                ))
                    }
                    _ => false,
                }) {
                    return bound.clone();
                }
            }
            if integer_type.sign() == IntegerSign::Signed {
                let zero = ScalarTerm::integer(integer_type, IntegerValue::Signed(0))
                    .expect("zero belongs to every signed carrier");
                for (variable, addend) in [(&left, &right), (&right, &left)] {
                    let nonnegative = Proposition::LessOrEqual(zero.clone(), addend.clone());
                    if !semantic_axioms.contains(&nonnegative) {
                        continue;
                    }
                    if let Some(bound) = semantic_axioms.iter().rev().find(|axiom| match axiom {
                        Proposition::LessOrEqual(bound_left, bound_right) => {
                            bound_left == variable
                                && is_maximum_minus(
                                    integer_type,
                                    bound_right,
                                    addend,
                                    semantic_axioms,
                                )
                        }
                        _ => false,
                    }) {
                        return canonical_conjunction(vec![nonnegative, bound.clone()]);
                    }
                }
            }
            if integer_type.sign() == IntegerSign::Signed {
                let zero = ScalarTerm::integer(integer_type, IntegerValue::Signed(0))
                    .expect("zero belongs to every signed carrier");
                for (variable, addend) in [(&left, &right), (&right, &left)] {
                    let nonpositive = Proposition::LessOrEqual(addend.clone(), zero.clone());
                    if !semantic_axioms.contains(&nonpositive) {
                        continue;
                    }
                    if let Some(bound) = semantic_axioms.iter().rev().find(|axiom| match axiom {
                        Proposition::LessOrEqual(bound_left, bound_right) => {
                            bound_right == variable
                                && is_minimum_minus(
                                    integer_type,
                                    bound_left,
                                    addend,
                                    semantic_axioms,
                                )
                        }
                        _ => false,
                    }) {
                        return canonical_conjunction(vec![nonpositive, bound.clone()]);
                    }
                }
            }
            return Proposition::Falsehood;
        }
        (Some(_), Some(_)) => unreachable!("known exact-add operands returned above"),
    };
    let original_variable = variable.clone();
    let original_offset = IntegerOffset::from_value(constant);
    let mut offset = original_offset;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let may_follow_chain = landed_integer_constant_value(
        integer_type,
        &constant_term,
        semantic_axioms,
        prior_axiom_count,
    ) == Some(constant);
    let mut followed_definition = false;
    if may_follow_chain {
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
            let ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            } = definition
            else {
                break;
            };
            if *scalar_type != integer_type {
                break;
            }
            let known_left = landed_integer_constant_value(
                integer_type,
                left,
                semantic_axioms,
                definition_index,
            );
            let known_right = landed_integer_constant_value(
                integer_type,
                right,
                semantic_axioms,
                definition_index,
            );
            let (nested_variable, nested_constant) = match (known_left, known_right) {
                (Some(left), None) => ((**right).clone(), left),
                (None, Some(right)) => ((**left).clone(), right),
                (Some(left), Some(right)) => {
                    let Some(base) = integer_type.exact_add(left, right) else {
                        return Proposition::Falsehood;
                    };
                    let Some(total) = offset.checked_add(IntegerOffset::from_value(base)) else {
                        return Proposition::Falsehood;
                    };
                    return if total.is_representable(integer_type) {
                        Proposition::Truth
                    } else {
                        Proposition::Falsehood
                    };
                }
                (None, None) => break,
            };
            let Some(combined) = offset.checked_add(IntegerOffset::from_value(nested_constant))
            else {
                return Proposition::Falsehood;
            };
            if combined.magnitude() > integer_type_span(integer_type) {
                return Proposition::Falsehood;
            }
            offset = combined;
            variable = nested_variable;
            prior_axiom_count = definition_index;
            followed_definition = true;
        }
    }
    if followed_definition
        && !matches!(
            &variable,
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == integer_type && machine_parameter_values.contains(id)
        )
    {
        variable = original_variable;
        offset = original_offset;
    }
    exact_integer_offset_obligation(integer_type, variable, offset)
}

fn is_maximum_minus(
    integer_type: psi_core::IntegerType,
    term: &ScalarTerm,
    subtrahend: &ScalarTerm,
    semantic_axioms: &[Proposition],
) -> bool {
    let definition = semantic_axioms
        .iter()
        .rev()
        .find_map(|axiom| match axiom {
            Proposition::Equal(left, right) if left == term => Some(right),
            Proposition::Equal(left, right) if right == term => Some(left),
            _ => None,
        })
        .unwrap_or(term);
    let ScalarTerm::ExactIntegerSubtract {
        scalar_type,
        left,
        right,
    } = definition
    else {
        return false;
    };
    *scalar_type == integer_type
        && right.as_ref() == subtrahend
        && known_integer_term_value(integer_type, left, semantic_axioms)
            == Some(integer_type.maximum_value())
}

fn is_minimum_minus(
    integer_type: psi_core::IntegerType,
    term: &ScalarTerm,
    subtrahend: &ScalarTerm,
    semantic_axioms: &[Proposition],
) -> bool {
    let definition = semantic_axioms
        .iter()
        .rev()
        .find_map(|axiom| match axiom {
            Proposition::Equal(left, right) if left == term => Some(right),
            Proposition::Equal(left, right) if right == term => Some(left),
            _ => None,
        })
        .unwrap_or(term);
    let ScalarTerm::ExactIntegerSubtract {
        scalar_type,
        left,
        right,
    } = definition
    else {
        return false;
    };
    *scalar_type == integer_type
        && right.as_ref() == subtrahend
        && known_integer_term_value(integer_type, left, semantic_axioms)
            == Some(integer_type.minimum_value())
}

fn exact_integer_subtract_obligation(
    integer_type: psi_core::IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Proposition {
    let known_left = known_integer_term_value(integer_type, &left, semantic_axioms);
    let known_right = known_integer_term_value(integer_type, &right, semantic_axioms);
    if let (Some(left), Some(right)) = (known_left, known_right) {
        return if integer_type.exact_sub(left, right).is_some() {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        };
    }
    let Some(constant) = known_right else {
        if let Some(obligation) = exact_integer_affine_fork_join_obligation(
            integer_type,
            left.clone(),
            right.clone(),
            ExactIntegerOffsetOperation::Subtract,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        ) {
            return obligation;
        }
        if let Some(obligation) = exact_integer_distinct_root_affine_fork_join_obligation(
            integer_type,
            left.clone(),
            right.clone(),
            ExactIntegerOffsetOperation::Subtract,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        ) {
            return obligation;
        }
        if integer_type.sign() == IntegerSign::Unsigned {
            let bound = Proposition::LessOrEqual(right.clone(), left.clone());
            if semantic_axioms.contains(&bound) {
                return bound;
            }
        }
        if integer_type.sign() == IntegerSign::Signed {
            let zero = ScalarTerm::integer(integer_type, IntegerValue::Signed(0))
                .expect("zero belongs to every signed carrier");
            let nonnegative = Proposition::LessOrEqual(zero, right.clone());
            if semantic_axioms.contains(&nonnegative)
                && let Some(bound) = semantic_axioms.iter().rev().find(|axiom| match axiom {
                    Proposition::LessOrEqual(bound_left, bound_right) => {
                        bound_right == &left
                            && is_minimum_plus(integer_type, bound_left, &right, semantic_axioms)
                    }
                    _ => false,
                })
            {
                return canonical_conjunction(vec![nonnegative, bound.clone()]);
            }
        }
        if integer_type.sign() == IntegerSign::Signed {
            let zero = ScalarTerm::integer(integer_type, IntegerValue::Signed(0))
                .expect("zero belongs to every signed carrier");
            let nonpositive = Proposition::LessOrEqual(right.clone(), zero);
            if semantic_axioms.contains(&nonpositive)
                && let Some(bound) = semantic_axioms.iter().rev().find(|axiom| match axiom {
                    Proposition::LessOrEqual(bound_left, bound_right) => {
                        bound_left == &left
                            && is_maximum_plus(integer_type, bound_right, &right, semantic_axioms)
                    }
                    _ => false,
                })
            {
                return canonical_conjunction(vec![nonpositive, bound.clone()]);
            }
        }
        if let (IntegerSign::Unsigned, Some(IntegerValue::Unsigned(constant))) =
            (integer_type.sign(), known_left)
        {
            if IntegerValue::Unsigned(constant) == integer_type.maximum_value() {
                return Proposition::Truth;
            }
            let boundary = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(constant))
                .expect("known unsigned minuend belongs to its carrier");
            return Proposition::LessOrEqual(right, boundary);
        }
        if integer_type.sign() == IntegerSign::Signed
            && known_left == Some(integer_type.maximum_value())
        {
            let zero = ScalarTerm::integer(integer_type, IntegerValue::Signed(0))
                .expect("zero belongs to every signed carrier");
            let nonnegative = Proposition::LessOrEqual(zero, right.clone());
            if semantic_axioms.contains(&nonnegative) {
                return nonnegative;
            }
        }
        if integer_type.sign() == IntegerSign::Signed
            && known_left == Some(integer_type.minimum_value())
        {
            let zero = ScalarTerm::integer(integer_type, IntegerValue::Signed(0))
                .expect("zero belongs to every signed carrier");
            let nonpositive = Proposition::LessOrEqual(right, zero);
            if semantic_axioms.contains(&nonpositive) {
                return nonpositive;
            }
        }
        return Proposition::Falsehood;
    };
    if known_left.is_none()
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_shift_then_arithmetic_chain_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Subtract,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_signed_affine_cast_affine_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Subtract,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_shift_cast_affine_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Subtract,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_divide_remainder_then_affine_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Subtract,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_divide_remainder_cast_affine_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Subtract,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_affine_cast_affine_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Subtract,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_cast_chain_then_affine_suffix_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Subtract,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_cast_then_affine_chain_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Subtract,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_cast_then_signed_affine_chain_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Subtract,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && let Some(constant) = known_right
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_cast_then_offset_obligation(
            integer_type,
            left.clone(),
            IntegerOffset::from_subtrahend(constant),
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_affine_chain_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Subtract,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_signed_affine_chain_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Subtract,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_mixed_add_subtract_chain_obligation(
            integer_type,
            left.clone(),
            IntegerOffset::from_subtrahend(constant),
            ExactIntegerOffsetOperation::Subtract,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    let original_variable = left.clone();
    let original_offset = IntegerOffset::from_subtrahend(constant);
    let mut variable = left;
    let mut offset = original_offset;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let may_follow_chain =
        landed_integer_constant_value(integer_type, &right, semantic_axioms, prior_axiom_count)
            == Some(constant);
    let mut followed_definition = false;
    if may_follow_chain {
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
            let ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } = definition
            else {
                break;
            };
            if *scalar_type != integer_type {
                break;
            }
            let known_left = landed_integer_constant_value(
                integer_type,
                left,
                semantic_axioms,
                definition_index,
            );
            let Some(nested_constant) = landed_integer_constant_value(
                integer_type,
                right,
                semantic_axioms,
                definition_index,
            ) else {
                break;
            };
            if let Some(base) = known_left {
                let Some(base) = integer_type.exact_sub(base, nested_constant) else {
                    return Proposition::Falsehood;
                };
                let Some(total) = offset.checked_add(IntegerOffset::from_value(base)) else {
                    return Proposition::Falsehood;
                };
                return if total.is_representable(integer_type) {
                    Proposition::Truth
                } else {
                    Proposition::Falsehood
                };
            }
            let Some(combined) =
                offset.checked_add(IntegerOffset::from_subtrahend(nested_constant))
            else {
                return Proposition::Falsehood;
            };
            if combined.magnitude() > integer_type_span(integer_type) {
                return Proposition::Falsehood;
            }
            offset = combined;
            variable = (**left).clone();
            prior_axiom_count = definition_index;
            followed_definition = true;
        }
    }
    if followed_definition
        && !matches!(
            &variable,
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == integer_type && machine_parameter_values.contains(id)
        )
    {
        variable = original_variable;
        offset = original_offset;
    }
    exact_integer_offset_obligation(integer_type, variable, offset)
}

fn exact_integer_mixed_add_subtract_chain_obligation(
    integer_type: psi_core::IntegerType,
    mut variable: ScalarTerm,
    initial_offset: IntegerOffset,
    initial_operation: ExactIntegerOffsetOperation,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if integer_type.is_address() || !matches!(integer_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let mut offset = initial_offset;
    let mut saw_add = initial_operation == ExactIntegerOffsetOperation::Add;
    let mut saw_subtract = initial_operation == ExactIntegerOffsetOperation::Subtract;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
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
            } if *scalar_type == integer_type => (left, right, ExactIntegerOffsetOperation::Add),
            ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } if *scalar_type == integer_type => {
                (left, right, ExactIntegerOffsetOperation::Subtract)
            }
            _ => break,
        };
        if landed_integer_constant_value(integer_type, left, semantic_axioms, definition_index)
            .is_some()
        {
            break;
        }
        let Some(constant) =
            landed_integer_constant_value(integer_type, right, semantic_axioms, definition_index)
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
        if combined.magnitude() > integer_type_span(integer_type) {
            return Some(Proposition::Falsehood);
        }
        offset = combined;
        variable = (**left).clone();
        prior_axiom_count = definition_index;
        followed_definition = true;
        saw_add |= operation == ExactIntegerOffsetOperation::Add;
        saw_subtract |= operation == ExactIntegerOffsetOperation::Subtract;
    }
    if !followed_definition
        || !saw_add
        || !saw_subtract
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
    Some(exact_integer_offset_obligation(
        integer_type,
        variable,
        offset,
    ))
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

fn exact_integer_affine_fork_branch(
    integer_type: psi_core::IntegerType,
    mut variable: ScalarTerm,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<ExactIntegerAffineForkBranch> {
    fixed_integer_type_interval(integer_type)?;
    let mut coefficient = IntegerOffset::Nonnegative(1);
    let mut offset = IntegerOffset::Nonnegative(0);
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let mut definition_indices = BTreeSet::new();
    for _ in 0..=prior_axiom_count {
        if !definition_indices.is_empty()
            && matches!(
                &variable,
                ScalarTerm::Value {
                    id,
                    scalar_type: ScalarType::Integer(root_type),
                } if *root_type == integer_type && machine_parameter_values.contains(id)
            )
        {
            return Some(ExactIntegerAffineForkBranch {
                root: variable,
                coefficient,
                offset,
                definition_indices,
            });
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
            ),
            ScalarTerm::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } if *scalar_type == integer_type => (
                left,
                right,
                IntegerOffset::from_value(landed_integer_constant_value(
                    integer_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?),
                IntegerOffset::Nonnegative(0),
            ),
            _ => return None,
        };
        if landed_integer_constant_value(integer_type, left, semantic_axioms, definition_index)
            .is_some()
            || landed_integer_constant_value(integer_type, right, semantic_axioms, definition_index)
                .is_none()
            || !definition_indices.insert(definition_index)
        {
            return None;
        }
        offset = nested_offset
            .checked_multiply_offset(coefficient)
            .and_then(|nested| nested.checked_add(offset))?;
        coefficient = coefficient.checked_multiply_offset(nested_coefficient)?;
        variable = (**left).clone();
        prior_axiom_count = definition_index;
    }
    None
}

fn exact_integer_affine_fork_join_obligation(
    integer_type: psi_core::IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
    operation: ExactIntegerOffsetOperation,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    let left = exact_integer_affine_fork_branch(
        integer_type,
        left,
        semantic_axioms,
        definition_axiom_count,
        machine_parameter_values,
    )?;
    let mut right = exact_integer_affine_fork_branch(
        integer_type,
        right,
        semantic_axioms,
        definition_axiom_count,
        machine_parameter_values,
    )?;
    if left.root != right.root
        || !left
            .definition_indices
            .is_disjoint(&right.definition_indices)
        || left.definition_indices.iter().next_back()? >= right.definition_indices.iter().next()?
    {
        return None;
    }
    if operation == ExactIntegerOffsetOperation::Subtract {
        right.coefficient = right.coefficient.negated();
        right.offset = right.offset.negated();
    }
    let coefficient = left.coefficient.checked_add(right.coefficient)?;
    let offset = left.offset.checked_add(right.offset)?;
    exact_integer_signed_affine_interval_obligation(
        integer_type,
        left.root,
        coefficient,
        offset,
        fixed_integer_type_interval(integer_type)?,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExactIntegerSignatureInterval {
    interval: (i128, i128),
    selected_bounds: Vec<Proposition>,
}

fn exact_integer_signature_interval(
    integer_type: psi_core::IntegerType,
    root: &ScalarTerm,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
) -> Option<ExactIntegerSignatureInterval> {
    let (mut minimum, mut maximum) = fixed_integer_type_interval(integer_type)?;
    let mut lower_bound = None;
    let mut upper_bound = None;
    let constant = |term: &ScalarTerm| {
        let (constant_type, value) = term.integer_value()?;
        (constant_type == integer_type)
            .then(|| integer_value_as_i128(value))
            .flatten()
    };
    for axiom in &semantic_axioms[definition_axiom_count.min(semantic_axioms.len())..] {
        let Proposition::LessOrEqual(left, right) = axiom else {
            continue;
        };
        if right == root
            && let Some(candidate) = constant(left)
            && candidate > minimum
        {
            minimum = candidate;
            lower_bound = Some(axiom.clone());
        }
        if left == root
            && let Some(candidate) = constant(right)
            && candidate < maximum
        {
            maximum = candidate;
            upper_bound = Some(axiom.clone());
        }
    }
    if minimum > maximum {
        return None;
    }
    let mut selected_bounds = Vec::with_capacity(2);
    selected_bounds.extend(lower_bound);
    selected_bounds.extend(upper_bound);
    Some(ExactIntegerSignatureInterval {
        interval: (minimum, maximum),
        selected_bounds,
    })
}

fn integer_offset_as_i128(value: IntegerOffset) -> Option<i128> {
    match value {
        IntegerOffset::Nonnegative(value) => i128::try_from(value).ok(),
        IntegerOffset::Negative(value) => signed_negative_magnitude(value),
    }
}

fn exact_integer_affine_forward_interval(
    coefficient: IntegerOffset,
    offset: IntegerOffset,
    interval: (i128, i128),
) -> Option<(i128, i128)> {
    let apply = |value| {
        IntegerOffset::from_value(IntegerValue::Signed(value))
            .checked_multiply_offset(coefficient)?
            .checked_add(offset)
            .and_then(integer_offset_as_i128)
    };
    let (lower_input, upper_input) = match coefficient {
        IntegerOffset::Negative(_) => (interval.1, interval.0),
        IntegerOffset::Nonnegative(_) => interval,
    };
    Some((apply(lower_input)?, apply(upper_input)?))
}

fn exact_integer_distinct_root_affine_fork_join_obligation(
    integer_type: psi_core::IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
    operation: ExactIntegerOffsetOperation,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    let left = exact_integer_affine_fork_branch(
        integer_type,
        left,
        semantic_axioms,
        definition_axiom_count,
        machine_parameter_values,
    )?;
    let right = exact_integer_affine_fork_branch(
        integer_type,
        right,
        semantic_axioms,
        definition_axiom_count,
        machine_parameter_values,
    )?;
    if left.root == right.root
        || !left
            .definition_indices
            .is_disjoint(&right.definition_indices)
        || left.definition_indices.iter().next_back()? >= right.definition_indices.iter().next()?
    {
        return None;
    }
    let mut left_signature = exact_integer_signature_interval(
        integer_type,
        &left.root,
        semantic_axioms,
        definition_axiom_count,
    )?;
    let right_signature = exact_integer_signature_interval(
        integer_type,
        &right.root,
        semantic_axioms,
        definition_axiom_count,
    )?;
    let left_interval = exact_integer_affine_forward_interval(
        left.coefficient,
        left.offset,
        left_signature.interval,
    )?;
    let right_interval = exact_integer_affine_forward_interval(
        right.coefficient,
        right.offset,
        right_signature.interval,
    )?;
    let joined = match operation {
        ExactIntegerOffsetOperation::Add => (
            left_interval.0.checked_add(right_interval.0)?,
            left_interval.1.checked_add(right_interval.1)?,
        ),
        ExactIntegerOffsetOperation::Subtract => (
            left_interval.0.checked_sub(right_interval.1)?,
            left_interval.1.checked_sub(right_interval.0)?,
        ),
    };
    let carrier = fixed_integer_type_interval(integer_type)?;
    if joined.1 < carrier.0 || joined.0 > carrier.1 {
        return Some(Proposition::Falsehood);
    }
    if joined.0 < carrier.0 || joined.1 > carrier.1 {
        return None;
    }
    left_signature
        .selected_bounds
        .extend(right_signature.selected_bounds);
    Some(canonical_conjunction(left_signature.selected_bounds))
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

fn is_minimum_plus(
    integer_type: psi_core::IntegerType,
    term: &ScalarTerm,
    addend: &ScalarTerm,
    semantic_axioms: &[Proposition],
) -> bool {
    let definition = semantic_axioms
        .iter()
        .rev()
        .find_map(|axiom| match axiom {
            Proposition::Equal(left, right) if left == term => Some(right),
            Proposition::Equal(left, right) if right == term => Some(left),
            _ => None,
        })
        .unwrap_or(term);
    let ScalarTerm::ExactIntegerAdd {
        scalar_type,
        left,
        right,
    } = definition
    else {
        return false;
    };
    *scalar_type == integer_type
        && ((right.as_ref() == addend
            && known_integer_term_value(integer_type, left, semantic_axioms)
                == Some(integer_type.minimum_value()))
            || (left.as_ref() == addend
                && known_integer_term_value(integer_type, right, semantic_axioms)
                    == Some(integer_type.minimum_value())))
}

fn is_maximum_plus(
    integer_type: psi_core::IntegerType,
    term: &ScalarTerm,
    addend: &ScalarTerm,
    semantic_axioms: &[Proposition],
) -> bool {
    let definition = semantic_axioms
        .iter()
        .rev()
        .find_map(|axiom| match axiom {
            Proposition::Equal(left, right) if left == term => Some(right),
            Proposition::Equal(left, right) if right == term => Some(left),
            _ => None,
        })
        .unwrap_or(term);
    let ScalarTerm::ExactIntegerAdd {
        scalar_type,
        left,
        right,
    } = definition
    else {
        return false;
    };
    *scalar_type == integer_type
        && ((right.as_ref() == addend
            && known_integer_term_value(integer_type, left, semantic_axioms)
                == Some(integer_type.maximum_value()))
            || (left.as_ref() == addend
                && known_integer_term_value(integer_type, right, semantic_axioms)
                    == Some(integer_type.maximum_value())))
}

fn exact_integer_multiply_obligation_with_definitions(
    integer_type: psi_core::IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Proposition {
    let known_left = known_integer_term_value(integer_type, &left, semantic_axioms);
    let known_right = known_integer_term_value(integer_type, &right, semantic_axioms);
    if let (Some(left), Some(right)) = (known_left, known_right) {
        return if integer_type.exact_mul(left, right).is_some() {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        };
    }
    let (variable, constant, constant_term, chain_orientation) = match (known_left, known_right) {
        (Some(constant), None) => (right, constant, left, false),
        (None, Some(constant)) => (left, constant, right, true),
        (None, None) => {
            if integer_type.sign() == IntegerSign::Unsigned {
                let one = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(1))
                    .expect("one belongs to every unsigned carrier");
                for (variable, factor) in [(&left, &right), (&right, &left)] {
                    let positive = Proposition::LessOrEqual(one.clone(), factor.clone());
                    if !semantic_axioms.contains(&positive) {
                        continue;
                    }
                    if let Some(bound) = semantic_axioms.iter().rev().find(|axiom| match axiom {
                        Proposition::LessOrEqual(bound_left, bound_right) => {
                            bound_left == variable
                                && is_maximum_divide(
                                    integer_type,
                                    bound_right,
                                    factor,
                                    semantic_axioms,
                                )
                        }
                        _ => false,
                    }) {
                        return canonical_conjunction(vec![positive, bound.clone()]);
                    }
                }
            }
            if integer_type.sign() == IntegerSign::Signed {
                let one = ScalarTerm::integer(integer_type, IntegerValue::Signed(1))
                    .expect("one belongs to every signed carrier");
                let negative_two = ScalarTerm::integer(integer_type, IntegerValue::Signed(-2))
                    .expect("negative two belongs to every signed carrier");
                for (variable, factor) in [(&left, &right), (&right, &left)] {
                    let positive = Proposition::LessOrEqual(one.clone(), factor.clone());
                    if semantic_axioms.contains(&positive) {
                        let lower = semantic_axioms.iter().rev().find(|axiom| match axiom {
                            Proposition::LessOrEqual(bound, bound_variable) => {
                                bound_variable == variable
                                    && is_minimum_divide(
                                        integer_type,
                                        bound,
                                        factor,
                                        semantic_axioms,
                                    )
                            }
                            _ => false,
                        });
                        let upper = semantic_axioms.iter().rev().find(|axiom| match axiom {
                            Proposition::LessOrEqual(bound_variable, bound) => {
                                bound_variable == variable
                                    && is_maximum_divide(
                                        integer_type,
                                        bound,
                                        factor,
                                        semantic_axioms,
                                    )
                            }
                            _ => false,
                        });
                        if let (Some(lower), Some(upper)) = (lower, upper) {
                            return canonical_conjunction(vec![
                                positive,
                                lower.clone(),
                                upper.clone(),
                            ]);
                        }
                    }

                    let negative = Proposition::LessOrEqual(factor.clone(), negative_two.clone());
                    if !semantic_axioms.contains(&negative) {
                        continue;
                    }
                    let lower = semantic_axioms.iter().rev().find(|axiom| match axiom {
                        Proposition::LessOrEqual(bound, bound_variable) => {
                            bound_variable == variable
                                && is_maximum_divide(integer_type, bound, factor, semantic_axioms)
                        }
                        _ => false,
                    });
                    let upper = semantic_axioms.iter().rev().find(|axiom| match axiom {
                        Proposition::LessOrEqual(bound_variable, bound) => {
                            bound_variable == variable
                                && is_minimum_divide(integer_type, bound, factor, semantic_axioms)
                        }
                        _ => false,
                    });
                    if let (Some(lower), Some(upper)) = (lower, upper) {
                        return canonical_conjunction(vec![negative, lower.clone(), upper.clone()]);
                    }
                }
            }
            return Proposition::Falsehood;
        }
        (Some(_), Some(_)) => unreachable!("known exact-multiply operands returned above"),
    };
    if chain_orientation
        && landed_integer_constant_value(
            integer_type,
            &constant_term,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_shift_then_arithmetic_chain_obligation(
            integer_type,
            variable.clone(),
            constant,
            ExactIntegerAffineOperation::Multiply,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if chain_orientation
        && landed_integer_constant_value(
            integer_type,
            &constant_term,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_signed_affine_cast_affine_obligation(
            integer_type,
            variable.clone(),
            constant,
            ExactIntegerAffineOperation::Multiply,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if chain_orientation
        && landed_integer_constant_value(
            integer_type,
            &constant_term,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_cast_chain_then_signed_product_suffix_obligation(
            integer_type,
            variable.clone(),
            constant,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if chain_orientation
        && landed_integer_constant_value(
            integer_type,
            &constant_term,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_cast_then_signed_multiply_chain_obligation(
            integer_type,
            variable.clone(),
            constant,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if chain_orientation
        && landed_integer_constant_value(
            integer_type,
            &constant_term,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_cast_then_signed_affine_chain_obligation(
            integer_type,
            variable.clone(),
            constant,
            ExactIntegerAffineOperation::Multiply,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if chain_orientation
        && landed_integer_constant_value(
            integer_type,
            &constant_term,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_shift_cast_affine_obligation(
            integer_type,
            variable.clone(),
            constant,
            ExactIntegerAffineOperation::Multiply,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if chain_orientation
        && landed_integer_constant_value(
            integer_type,
            &constant_term,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_signed_multiply_chain_obligation(
            integer_type,
            variable.clone(),
            constant,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if chain_orientation
        && landed_integer_constant_value(
            integer_type,
            &constant_term,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_signed_affine_chain_obligation(
            integer_type,
            variable.clone(),
            constant,
            ExactIntegerAffineOperation::Multiply,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if chain_orientation
        && landed_integer_constant_value(
            integer_type,
            &constant_term,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_divide_remainder_then_affine_obligation(
            integer_type,
            variable.clone(),
            constant,
            ExactIntegerAffineOperation::Multiply,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if chain_orientation
        && landed_integer_constant_value(
            integer_type,
            &constant_term,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_divide_remainder_cast_affine_obligation(
            integer_type,
            variable.clone(),
            constant,
            ExactIntegerAffineOperation::Multiply,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if chain_orientation
        && landed_integer_constant_value(
            integer_type,
            &constant_term,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_affine_cast_affine_obligation(
            integer_type,
            variable.clone(),
            constant,
            ExactIntegerAffineOperation::Multiply,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if chain_orientation
        && landed_integer_constant_value(
            integer_type,
            &constant_term,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_cast_chain_then_affine_suffix_obligation(
            integer_type,
            variable.clone(),
            constant,
            ExactIntegerAffineOperation::Multiply,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if chain_orientation
        && landed_integer_constant_value(
            integer_type,
            &constant_term,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_cast_then_affine_chain_obligation(
            integer_type,
            variable.clone(),
            constant,
            ExactIntegerAffineOperation::Multiply,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if chain_orientation
        && landed_integer_constant_value(
            integer_type,
            &constant_term,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_affine_chain_obligation(
            integer_type,
            variable.clone(),
            constant,
            ExactIntegerAffineOperation::Multiply,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if chain_orientation
        && landed_integer_constant_value(
            integer_type,
            &constant_term,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_cast_then_multiply_chain_obligation(
            integer_type,
            variable.clone(),
            constant,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if chain_orientation
        && landed_integer_constant_value(
            integer_type,
            &constant_term,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_multiply_chain_obligation(
            integer_type,
            variable.clone(),
            constant,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    match (integer_type.sign(), constant) {
        (IntegerSign::Unsigned, IntegerValue::Unsigned(0))
        | (IntegerSign::Unsigned, IntegerValue::Unsigned(1))
        | (IntegerSign::Signed, IntegerValue::Signed(0))
        | (IntegerSign::Signed, IntegerValue::Signed(1)) => Proposition::Truth,
        (IntegerSign::Unsigned, IntegerValue::Unsigned(constant)) => {
            let IntegerValue::Unsigned(maximum) = integer_type.maximum_value() else {
                unreachable!("unsigned type has unsigned maximum")
            };
            let boundary =
                ScalarTerm::integer(integer_type, IntegerValue::Unsigned(maximum / constant))
                    .expect("exact-multiply unsigned upper boundary remains in the carrier");
            Proposition::LessOrEqual(variable, boundary)
        }
        (IntegerSign::Signed, IntegerValue::Signed(-1)) => {
            let IntegerValue::Signed(minimum) = integer_type.minimum_value() else {
                unreachable!("signed type has signed minimum")
            };
            let boundary = ScalarTerm::integer(
                integer_type,
                IntegerValue::Signed(
                    minimum
                        .checked_add(1)
                        .expect("fixed signed minimum is below maximum"),
                ),
            )
            .expect("exact-multiply negation boundary remains in the carrier");
            Proposition::LessOrEqual(boundary, variable)
        }
        (IntegerSign::Signed, IntegerValue::Signed(constant)) if constant > 1 => {
            let (IntegerValue::Signed(minimum), IntegerValue::Signed(maximum)) =
                (integer_type.minimum_value(), integer_type.maximum_value())
            else {
                unreachable!("signed type has signed bounds")
            };
            let lower = ScalarTerm::integer(integer_type, IntegerValue::Signed(minimum / constant))
                .expect("exact-multiply signed lower boundary remains in the carrier");
            let upper = ScalarTerm::integer(integer_type, IntegerValue::Signed(maximum / constant))
                .expect("exact-multiply signed upper boundary remains in the carrier");
            canonical_conjunction(vec![
                Proposition::LessOrEqual(lower, variable.clone()),
                Proposition::LessOrEqual(variable, upper),
            ])
        }
        (IntegerSign::Signed, IntegerValue::Signed(constant)) => {
            let (IntegerValue::Signed(minimum), IntegerValue::Signed(maximum)) =
                (integer_type.minimum_value(), integer_type.maximum_value())
            else {
                unreachable!("signed type has signed bounds")
            };
            let lower = ScalarTerm::integer(integer_type, IntegerValue::Signed(maximum / constant))
                .expect("exact-multiply negative signed lower boundary remains in the carrier");
            let upper = ScalarTerm::integer(integer_type, IntegerValue::Signed(minimum / constant))
                .expect("exact-multiply negative signed upper boundary remains in the carrier");
            canonical_conjunction(vec![
                Proposition::LessOrEqual(lower, variable.clone()),
                Proposition::LessOrEqual(variable, upper),
            ])
        }
        _ => Proposition::Falsehood,
    }
}

fn exact_integer_cast_then_multiply_chain_obligation(
    target_type: psi_core::IntegerType,
    mut variable: ScalarTerm,
    factor: IntegerValue,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if target_type.is_address() || !matches!(target_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let mut cumulative_factor = nonnegative_integer_factor(target_type, factor)?;
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
            ScalarTerm::ExactIntegerMultiply {
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
                let nested_factor = landed_integer_constant_value(
                    target_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )
                .and_then(|factor| nonnegative_integer_factor(target_type, factor))?;
                let Some(product) = cumulative_factor.checked_mul(nested_factor) else {
                    return Some(Proposition::Falsehood);
                };
                cumulative_factor = product;
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
                return Some(exact_integer_scaled_interval_obligation(
                    *source_type,
                    target_type,
                    (**operand).clone(),
                    cumulative_factor,
                ));
            }
            _ => return None,
        }
    }
    None
}

fn exact_integer_cast_then_signed_affine_chain_obligation(
    target_type: psi_core::IntegerType,
    variable: ScalarTerm,
    initial_constant: IntegerValue,
    initial_operation: ExactIntegerAffineOperation,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    let (coefficient, offset, saw_offset, saw_negative_factor) =
        exact_integer_signed_affine_initial_form(initial_constant, initial_operation)?;
    let (variable, coefficient, offset, prior_axiom_count, saw_offset, saw_negative_factor) =
        exact_integer_signed_affine_replay(
            target_type,
            variable,
            coefficient,
            offset,
            saw_offset,
            saw_negative_factor,
            semantic_axioms,
            definition_axiom_count,
        )?;
    if !saw_offset || !saw_negative_factor {
        return None;
    }
    let (_, definition) = semantic_axioms[..prior_axiom_count]
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, axiom)| match axiom {
            Proposition::Equal(left, right) if left == &variable => Some((index, right)),
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
    if *cast_target_type != target_type
        || !partial_fixed_native_integer_cast(*source_type, target_type)
        || !matches!(
            operand.as_ref(),
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == *source_type && machine_parameter_values.contains(id)
        )
    {
        return None;
    }
    let interval = match exact_integer_signed_affine_preimage_interval(
        coefficient,
        offset,
        fixed_integer_type_interval(target_type)?,
    )? {
        ExactIntegerIntervalPreimage::Interval(interval) => interval,
        ExactIntegerIntervalPreimage::Empty => return Some(Proposition::Falsehood),
    };
    Some(exact_integer_source_interval_obligation(
        *source_type,
        (**operand).clone(),
        interval.0,
        interval.1,
    ))
}

fn exact_integer_cast_then_signed_multiply_chain_obligation(
    target_type: psi_core::IntegerType,
    mut variable: ScalarTerm,
    factor: IntegerValue,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if target_type.sign() != IntegerSign::Signed
        || target_type.is_address()
        || !matches!(target_type.bits(), 8 | 16 | 32 | 64)
    {
        return None;
    }
    let IntegerValue::Signed(factor_value) = factor else {
        return None;
    };
    let mut product = checked_signed_integer_product(Some(IntegerOffset::Nonnegative(1)), factor);
    let mut saw_negative = factor_value < 0;
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
            ScalarTerm::ExactIntegerMultiply {
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
                let nested_factor = landed_integer_constant_value(
                    target_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?;
                let IntegerValue::Signed(nested_value) = nested_factor else {
                    return None;
                };
                product = checked_signed_integer_product(product, nested_factor);
                saw_negative |= nested_value < 0;
                variable = (**left).clone();
                prior_axiom_count = definition_index;
            }
            ScalarTerm::IntegerExactCast {
                source_type,
                target_type: cast_target_type,
                operand,
            } if saw_negative
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
                return exact_integer_signed_product_interval_obligation(
                    *source_type,
                    target_type,
                    (**operand).clone(),
                    product?,
                );
            }
            _ => return None,
        }
    }
    None
}

fn exact_integer_cast_chain_then_signed_product_suffix_obligation(
    target_type: psi_core::IntegerType,
    mut variable: ScalarTerm,
    factor: IntegerValue,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if target_type.sign() != IntegerSign::Signed {
        return None;
    }
    let IntegerValue::Signed(factor_value) = factor else {
        return None;
    };
    let mut product = checked_signed_integer_product(Some(IntegerOffset::Nonnegative(1)), factor);
    let mut saw_negative = factor_value < 0;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    for _ in 0..=prior_axiom_count {
        if saw_negative
            && let Some((root_type, root, cast_interval)) = exact_integer_cast_chain_root_interval(
                target_type,
                variable.clone(),
                semantic_axioms,
                prior_axiom_count,
                machine_parameter_values,
            )
        {
            let product = product?;
            if product.magnitude() == 0 {
                return Some(Proposition::Truth);
            }
            let target_interval = fixed_integer_type_interval(target_type)?;
            let magnitude = product.magnitude();
            let preimage = if magnitude > i128::MAX as u128 {
                (0, 0)
            } else {
                let magnitude = i128::try_from(magnitude).ok()?;
                let signed_product = match product {
                    IntegerOffset::Nonnegative(_) => magnitude,
                    IntegerOffset::Negative(_) => magnitude.checked_neg()?,
                };
                if signed_product > 0 {
                    (
                        checked_integer_ceil_division(target_interval.0, signed_product)?,
                        checked_integer_floor_division(target_interval.1, signed_product)?,
                    )
                } else {
                    (
                        checked_integer_ceil_division(target_interval.1, signed_product)?,
                        checked_integer_floor_division(target_interval.0, signed_product)?,
                    )
                }
            };
            let minimum = preimage.0.max(cast_interval.0);
            let maximum = preimage.1.min(cast_interval.1);
            return Some(if minimum <= maximum {
                exact_integer_source_interval_obligation(root_type, root, minimum, maximum)
            } else {
                Proposition::Falsehood
            });
        }
        if saw_negative {
            let product = product?;
            let target_interval = fixed_integer_type_interval(target_type)?;
            if product.magnitude() == 0 {
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
                    return Some(Proposition::Truth);
                }
            } else {
                let magnitude = product.magnitude();
                let preimage = if magnitude > i128::MAX as u128 {
                    (0, 0)
                } else {
                    let magnitude = i128::try_from(magnitude).ok()?;
                    let signed_product = match product {
                        IntegerOffset::Nonnegative(_) => magnitude,
                        IntegerOffset::Negative(_) => magnitude.checked_neg()?,
                    };
                    if signed_product > 0 {
                        (
                            checked_integer_ceil_division(target_interval.0, signed_product)?,
                            checked_integer_floor_division(target_interval.1, signed_product)?,
                        )
                    } else {
                        (
                            checked_integer_ceil_division(target_interval.1, signed_product)?,
                            checked_integer_floor_division(target_interval.0, signed_product)?,
                        )
                    }
                };
                if let Some(obligation) =
                    exact_integer_computed_prefix_conversion_interval_obligation(
                        target_type,
                        variable.clone(),
                        preimage,
                        semantic_axioms,
                        prior_axiom_count,
                        machine_parameter_values,
                    )
                {
                    return Some(obligation);
                }
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
        let ScalarTerm::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
        } = definition
        else {
            return None;
        };
        if *scalar_type != target_type
            || landed_integer_constant_value(target_type, left, semantic_axioms, definition_index)
                .is_some()
        {
            return None;
        }
        let nested_factor =
            landed_integer_constant_value(target_type, right, semantic_axioms, definition_index)?;
        let IntegerValue::Signed(nested_value) = nested_factor else {
            return None;
        };
        product = checked_signed_integer_product(product, nested_factor);
        saw_negative |= nested_value < 0;
        variable = (**left).clone();
        prior_axiom_count = definition_index;
    }
    None
}

fn exact_integer_scaled_interval_obligation(
    root_type: psi_core::IntegerType,
    interval_type: psi_core::IntegerType,
    root: ScalarTerm,
    cumulative_factor: u128,
) -> Proposition {
    if cumulative_factor <= 1 {
        return Proposition::Truth;
    }
    let (target_minimum, target_maximum) = match interval_type.sign() {
        IntegerSign::Unsigned => {
            let IntegerValue::Unsigned(maximum) = interval_type.maximum_value() else {
                unreachable!("unsigned fixed integer type has an unsigned maximum")
            };
            let Some(maximum) = i128::try_from(maximum / cumulative_factor).ok() else {
                return Proposition::Falsehood;
            };
            (0, maximum)
        }
        IntegerSign::Signed => {
            let (IntegerValue::Signed(minimum), IntegerValue::Signed(maximum)) =
                (interval_type.minimum_value(), interval_type.maximum_value())
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

#[cfg(test)]
fn exact_integer_multiply_obligation(
    integer_type: psi_core::IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
    semantic_axioms: &[Proposition],
) -> Proposition {
    exact_integer_multiply_obligation_with_definitions(
        integer_type,
        left,
        right,
        semantic_axioms,
        0,
        &BTreeSet::new(),
    )
}

fn exact_integer_multiply_chain_obligation(
    integer_type: psi_core::IntegerType,
    mut variable: ScalarTerm,
    factor: IntegerValue,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    let mut cumulative_factor = nonnegative_integer_factor(integer_type, factor)?;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
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
        if *scalar_type != integer_type
            || landed_integer_constant_value(integer_type, left, semantic_axioms, definition_index)
                .is_some()
        {
            break;
        }
        let Some(nested_factor) =
            landed_integer_constant_value(integer_type, right, semantic_axioms, definition_index)
                .and_then(|factor| nonnegative_integer_factor(integer_type, factor))
        else {
            break;
        };
        let Some(product) = cumulative_factor.checked_mul(nested_factor) else {
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
            } if *root_type == integer_type && machine_parameter_values.contains(id)
        )
    {
        return None;
    }
    Some(exact_integer_cumulative_multiply_obligation(
        integer_type,
        variable,
        cumulative_factor,
    ))
}

fn exact_integer_signed_multiply_chain_obligation(
    integer_type: psi_core::IntegerType,
    mut variable: ScalarTerm,
    factor: IntegerValue,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if integer_type.sign() != IntegerSign::Signed {
        return None;
    }
    let IntegerValue::Signed(factor_value) = factor else {
        return None;
    };
    let mut product = checked_signed_integer_product(Some(IntegerOffset::Nonnegative(1)), factor);
    let mut saw_negative = factor_value < 0;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
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
        if *scalar_type != integer_type
            || landed_integer_constant_value(integer_type, left, semantic_axioms, definition_index)
                .is_some()
        {
            break;
        }
        let nested_factor =
            landed_integer_constant_value(integer_type, right, semantic_axioms, definition_index)?;
        let IntegerValue::Signed(nested_value) = nested_factor else {
            return None;
        };
        product = checked_signed_integer_product(product, nested_factor);
        saw_negative |= nested_value < 0;
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
            } if *root_type == integer_type && machine_parameter_values.contains(id)
        )
    {
        return None;
    }
    exact_integer_signed_product_interval_obligation(integer_type, integer_type, variable, product?)
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

fn exact_integer_cumulative_multiply_obligation(
    integer_type: psi_core::IntegerType,
    variable: ScalarTerm,
    cumulative_factor: u128,
) -> Proposition {
    if cumulative_factor <= 1 {
        return Proposition::Truth;
    }
    match integer_type.sign() {
        IntegerSign::Unsigned => {
            let IntegerValue::Unsigned(maximum) = integer_type.maximum_value() else {
                unreachable!("unsigned type has unsigned maximum")
            };
            let boundary = ScalarTerm::integer(
                integer_type,
                IntegerValue::Unsigned(maximum / cumulative_factor),
            )
            .expect("cumulative exact-multiply upper boundary remains in the carrier");
            Proposition::LessOrEqual(variable, boundary)
        }
        IntegerSign::Signed => {
            let (IntegerValue::Signed(minimum), IntegerValue::Signed(maximum)) =
                (integer_type.minimum_value(), integer_type.maximum_value())
            else {
                unreachable!("signed type has signed bounds")
            };
            let lower = signed_negative_magnitude(minimum.unsigned_abs() / cumulative_factor)
                .expect("cumulative exact-multiply lower boundary remains signed");
            let upper = i128::try_from(maximum as u128 / cumulative_factor)
                .expect("cumulative exact-multiply upper boundary remains signed");
            canonical_conjunction(vec![
                Proposition::LessOrEqual(
                    ScalarTerm::integer(integer_type, IntegerValue::Signed(lower))
                        .expect("cumulative exact-multiply lower boundary remains in the carrier"),
                    variable.clone(),
                ),
                Proposition::LessOrEqual(
                    variable,
                    ScalarTerm::integer(integer_type, IntegerValue::Signed(upper))
                        .expect("cumulative exact-multiply upper boundary remains in the carrier"),
                ),
            ])
        }
    }
}

fn is_maximum_divide(
    integer_type: psi_core::IntegerType,
    term: &ScalarTerm,
    divisor: &ScalarTerm,
    semantic_axioms: &[Proposition],
) -> bool {
    let definition = semantic_axioms
        .iter()
        .rev()
        .find_map(|axiom| match axiom {
            Proposition::Equal(left, right) if left == term => Some(right),
            Proposition::Equal(left, right) if right == term => Some(left),
            _ => None,
        })
        .unwrap_or(term);
    let ScalarTerm::ExactIntegerDivide {
        scalar_type,
        left,
        right,
    } = definition
    else {
        return false;
    };
    *scalar_type == integer_type
        && right.as_ref() == divisor
        && known_integer_term_value(integer_type, left, semantic_axioms)
            == Some(integer_type.maximum_value())
}

fn is_minimum_divide(
    integer_type: psi_core::IntegerType,
    term: &ScalarTerm,
    divisor: &ScalarTerm,
    semantic_axioms: &[Proposition],
) -> bool {
    let definition = semantic_axioms
        .iter()
        .rev()
        .find_map(|axiom| match axiom {
            Proposition::Equal(left, right) if left == term => Some(right),
            Proposition::Equal(left, right) if right == term => Some(left),
            _ => None,
        })
        .unwrap_or(term);
    let ScalarTerm::ExactIntegerDivide {
        scalar_type,
        left,
        right,
    } = definition
    else {
        return false;
    };
    *scalar_type == integer_type
        && right.as_ref() == divisor
        && known_integer_term_value(integer_type, left, semantic_axioms)
            == Some(integer_type.minimum_value())
}

fn exact_integer_divide_obligation(
    integer_type: psi_core::IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
    semantic_axioms: &[Proposition],
) -> Proposition {
    let known_left = known_integer_term_value(integer_type, &left, semantic_axioms);
    let known_right = known_integer_term_value(integer_type, &right, semantic_axioms);
    if let (Some(left), Some(right)) = (known_left, known_right) {
        return if integer_type.exact_div(left, right).is_some() {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        };
    }
    match (integer_type.sign(), known_right) {
        (IntegerSign::Unsigned, Some(IntegerValue::Unsigned(0)))
        | (IntegerSign::Signed, Some(IntegerValue::Signed(0))) => Proposition::Falsehood,
        (IntegerSign::Unsigned, Some(IntegerValue::Unsigned(_))) => Proposition::Truth,
        (IntegerSign::Signed, Some(IntegerValue::Signed(-1))) => {
            let IntegerValue::Signed(minimum) = integer_type.minimum_value() else {
                unreachable!("signed type has signed minimum")
            };
            let boundary = ScalarTerm::integer(
                integer_type,
                IntegerValue::Signed(
                    minimum
                        .checked_add(1)
                        .expect("fixed signed minimum is below maximum"),
                ),
            )
            .expect("exact-divide negation boundary remains in the carrier");
            Proposition::LessOrEqual(boundary, left)
        }
        (IntegerSign::Signed, Some(IntegerValue::Signed(_))) => Proposition::Truth,
        _ => runtime_divisor_obligation(integer_type, left, right, semantic_axioms, false),
    }
}

fn exact_integer_remainder_obligation(
    integer_type: psi_core::IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
    semantic_axioms: &[Proposition],
) -> Proposition {
    let known_left = known_integer_term_value(integer_type, &left, semantic_axioms);
    let known_right = known_integer_term_value(integer_type, &right, semantic_axioms);
    if let (Some(left), Some(right)) = (known_left, known_right) {
        return if integer_type.exact_rem(left, right).is_some() {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        };
    }
    match (integer_type.sign(), known_right) {
        (IntegerSign::Unsigned, Some(IntegerValue::Unsigned(0)))
        | (IntegerSign::Signed, Some(IntegerValue::Signed(0))) => Proposition::Falsehood,
        (IntegerSign::Unsigned, Some(IntegerValue::Unsigned(_))) => Proposition::Truth,
        (IntegerSign::Signed, Some(IntegerValue::Signed(-1))) => {
            let IntegerValue::Signed(minimum) = integer_type.minimum_value() else {
                unreachable!("signed type has signed minimum")
            };
            let boundary = ScalarTerm::integer(
                integer_type,
                IntegerValue::Signed(minimum.checked_add(1).expect("minimum has a successor")),
            )
            .expect("exact-remainder boundary remains in the carrier");
            Proposition::LessOrEqual(boundary, left)
        }
        (IntegerSign::Signed, Some(IntegerValue::Signed(_))) => Proposition::Truth,
        _ => runtime_divisor_obligation(integer_type, left, right, semantic_axioms, false),
    }
}

fn wrapping_integer_divide_obligation(
    integer_type: psi_core::IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
    semantic_axioms: &[Proposition],
) -> Proposition {
    let known_left = known_integer_term_value(integer_type, &left, semantic_axioms);
    let known_right = known_integer_term_value(integer_type, &right, semantic_axioms);
    if let (Some(left), Some(right)) = (known_left, known_right) {
        return if integer_type.wrapping_div(left, right).is_some() {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        };
    }
    match (integer_type.sign(), known_right) {
        (IntegerSign::Unsigned, Some(IntegerValue::Unsigned(0)))
        | (IntegerSign::Signed, Some(IntegerValue::Signed(0))) => Proposition::Falsehood,
        (IntegerSign::Unsigned, Some(IntegerValue::Unsigned(_)))
        | (IntegerSign::Signed, Some(IntegerValue::Signed(_))) => Proposition::Truth,
        _ => runtime_divisor_obligation(integer_type, left, right, semantic_axioms, true),
    }
}

fn wrapping_integer_remainder_obligation(
    integer_type: psi_core::IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
    semantic_axioms: &[Proposition],
) -> Proposition {
    let known_left = known_integer_term_value(integer_type, &left, semantic_axioms);
    let known_right = known_integer_term_value(integer_type, &right, semantic_axioms);
    if let (Some(left), Some(right)) = (known_left, known_right) {
        return if integer_type.wrapping_rem(left, right).is_some() {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        };
    }
    match (integer_type.sign(), known_right) {
        (IntegerSign::Unsigned, Some(IntegerValue::Unsigned(0)))
        | (IntegerSign::Signed, Some(IntegerValue::Signed(0))) => Proposition::Falsehood,
        (IntegerSign::Unsigned, Some(IntegerValue::Unsigned(_)))
        | (IntegerSign::Signed, Some(IntegerValue::Signed(_))) => Proposition::Truth,
        _ => runtime_divisor_obligation(integer_type, left, right, semantic_axioms, true),
    }
}

fn saturating_integer_divide_obligation(
    integer_type: psi_core::IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
    semantic_axioms: &[Proposition],
) -> Proposition {
    let known_left = known_integer_term_value(integer_type, &left, semantic_axioms);
    let known_right = known_integer_term_value(integer_type, &right, semantic_axioms);
    if let (Some(left), Some(right)) = (known_left, known_right) {
        return if integer_type.saturating_div(left, right).is_some() {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        };
    }
    match (integer_type.sign(), known_right) {
        (IntegerSign::Unsigned, Some(IntegerValue::Unsigned(0)))
        | (IntegerSign::Signed, Some(IntegerValue::Signed(0))) => Proposition::Falsehood,
        (IntegerSign::Unsigned, Some(IntegerValue::Unsigned(_)))
        | (IntegerSign::Signed, Some(IntegerValue::Signed(_))) => Proposition::Truth,
        _ => runtime_divisor_obligation(integer_type, left, right, semantic_axioms, true),
    }
}

fn saturating_integer_remainder_obligation(
    integer_type: psi_core::IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
    semantic_axioms: &[Proposition],
) -> Proposition {
    let known_left = known_integer_term_value(integer_type, &left, semantic_axioms);
    let known_right = known_integer_term_value(integer_type, &right, semantic_axioms);
    if let (Some(left), Some(right)) = (known_left, known_right) {
        return if integer_type.saturating_rem(left, right).is_some() {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        };
    }
    match (integer_type.sign(), known_right) {
        (IntegerSign::Unsigned, Some(IntegerValue::Unsigned(0)))
        | (IntegerSign::Signed, Some(IntegerValue::Signed(0))) => Proposition::Falsehood,
        (IntegerSign::Unsigned, Some(IntegerValue::Unsigned(_)))
        | (IntegerSign::Signed, Some(IntegerValue::Signed(_))) => Proposition::Truth,
        _ => runtime_divisor_obligation(integer_type, left, right, semantic_axioms, true),
    }
}

fn runtime_divisor_obligation(
    integer_type: psi_core::IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
    semantic_axioms: &[Proposition],
    negative_one_is_total: bool,
) -> Proposition {
    if integer_type.sign() == IntegerSign::Signed {
        let negative_one = ScalarTerm::integer(integer_type, IntegerValue::Signed(-1))
            .expect("every signed fixed integer carrier admits negative one");
        let negative_one_bound = Proposition::LessOrEqual(right.clone(), negative_one);
        if semantic_axioms.contains(&negative_one_bound) {
            if negative_one_is_total {
                return negative_one_bound;
            }
            let IntegerValue::Signed(minimum) = integer_type.minimum_value() else {
                unreachable!("signed fixed integer has a signed minimum")
            };
            if let Ok(minimum_plus_one) = ScalarTerm::integer(
                integer_type,
                IntegerValue::Signed(minimum.checked_add(1).expect("minimum has a successor")),
            ) {
                let dividend_bound = Proposition::LessOrEqual(minimum_plus_one, left);
                if semantic_axioms.contains(&dividend_bound) {
                    return canonical_conjunction(vec![negative_one_bound, dividend_bound]);
                }
            }
        }
    }
    if integer_type.sign() == IntegerSign::Signed {
        if let Ok(negative_two) = ScalarTerm::integer(integer_type, IntegerValue::Signed(-2)) {
            let negative_bound = Proposition::LessOrEqual(right.clone(), negative_two);
            if semantic_axioms.contains(&negative_bound) {
                return negative_bound;
            }
        }
    }
    let one = match integer_type.sign() {
        IntegerSign::Unsigned => IntegerValue::Unsigned(1),
        IntegerSign::Signed => IntegerValue::Signed(1),
    };
    let Ok(boundary) = ScalarTerm::integer(integer_type, one) else {
        return Proposition::Falsehood;
    };
    Proposition::LessOrEqual(boundary, right)
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
mod tests {
    use super::*;
    use psi_core::{IntegerType, ScalarType, ValueId};

    #[test]
    fn reconstructs_widen_then_exact_narrow_roundtrip_as_self_proving() {
        let narrow_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let wide_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
        let input_id = ValueId::new(1).expect("input");
        let input = ScalarTerm::value(input_id, ScalarType::Integer(narrow_type));
        let widened = ScalarTerm::value(
            ValueId::new(2).expect("widened"),
            ScalarType::Integer(wide_type),
        );
        let definition = Proposition::Equal(
            widened.clone(),
            ScalarTerm::integer_widen(narrow_type, wide_type, input).expect("u8 to u16 widening"),
        );
        assert_eq!(
            exact_integer_cast_obligation(
                wide_type,
                narrow_type,
                widened,
                std::slice::from_ref(&definition),
                &BTreeSet::from([input_id]),
            ),
            Proposition::Truth
        );
    }

    #[test]
    fn reconstructs_multiply_chain_exact_cast_bounds_and_rejects_broken_definitions() {
        let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let root_id = ValueId::new(201).expect("root");
        let root = ScalarTerm::value(root_id, ScalarType::Integer(u16_type));
        let first = ScalarTerm::value(
            ValueId::new(202).expect("first"),
            ScalarType::Integer(u16_type),
        );
        let second = ScalarTerm::value(
            ValueId::new(203).expect("second"),
            ScalarType::Integer(u16_type),
        );
        let first_definition = Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_multiply(
                u16_type,
                root.clone(),
                ScalarTerm::integer(u16_type, IntegerValue::Unsigned(2)).expect("2u16"),
            )
            .expect("root * 2u16"),
        );
        let second_definition = Proposition::Equal(
            second.clone(),
            ScalarTerm::exact_integer_multiply(
                u16_type,
                first.clone(),
                ScalarTerm::integer(u16_type, IntegerValue::Unsigned(3)).expect("3u16"),
            )
            .expect("first * 3u16"),
        );
        let parameters = BTreeSet::from([root_id]);
        let expected = Proposition::LessOrEqual(
            root.clone(),
            ScalarTerm::integer(u16_type, IntegerValue::Unsigned(42)).expect("42u16"),
        );
        let reconstruct = |axioms: &[Proposition]| {
            exact_integer_cast_obligation(u16_type, u8_type, second.clone(), axioms, &parameters)
        };
        assert_eq!(
            exact_integer_cast_obligation(
                u16_type,
                u8_type,
                first.clone(),
                std::slice::from_ref(&first_definition),
                &parameters,
            ),
            Proposition::LessOrEqual(
                root.clone(),
                ScalarTerm::integer(u16_type, IntegerValue::Unsigned(127)).expect("127u16"),
            )
        );
        assert_eq!(
            reconstruct(&[first_definition.clone(), second_definition.clone()]),
            expected
        );
        let one = ScalarTerm::value(
            ValueId::new(204).expect("one"),
            ScalarType::Integer(u16_type),
        );
        let one_definition = Proposition::Equal(
            one.clone(),
            ScalarTerm::exact_integer_multiply(
                u16_type,
                root.clone(),
                ScalarTerm::integer(u16_type, IntegerValue::Unsigned(1)).expect("1u16"),
            )
            .expect("root * 1u16"),
        );
        assert_eq!(
            exact_integer_cast_obligation(
                u16_type,
                u8_type,
                one,
                std::slice::from_ref(&one_definition),
                &parameters,
            ),
            Proposition::LessOrEqual(
                root.clone(),
                ScalarTerm::integer(u16_type, IntegerValue::Unsigned(255)).expect("255u16"),
            )
        );
        let zero = ScalarTerm::value(
            ValueId::new(205).expect("zero"),
            ScalarType::Integer(u16_type),
        );
        let zero_definition = Proposition::Equal(
            zero.clone(),
            ScalarTerm::exact_integer_multiply(
                u16_type,
                first.clone(),
                ScalarTerm::integer(u16_type, IntegerValue::Unsigned(0)).expect("0u16"),
            )
            .expect("first * 0u16"),
        );
        assert_eq!(
            exact_integer_cast_obligation(
                u16_type,
                u8_type,
                zero,
                &[first_definition.clone(), zero_definition],
                &parameters,
            ),
            Proposition::Truth,
            "a zero cumulative product discharges only the cast; the prefix multiply keeps its own proof"
        );

        assert_ne!(
            reconstruct(&[second_definition.clone(), first_definition.clone()]),
            expected
        );
        let reversed_second = match second_definition.clone() {
            Proposition::Equal(left, right) => Proposition::Equal(right, left),
            _ => unreachable!("definition is an equality"),
        };
        assert_ne!(
            reconstruct(&[first_definition.clone(), reversed_second]),
            expected
        );
        let literal_left_first = Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_multiply(
                u16_type,
                ScalarTerm::integer(u16_type, IntegerValue::Unsigned(2)).expect("2u16"),
                root.clone(),
            )
            .expect("2u16 * root"),
        );
        assert_ne!(
            reconstruct(&[literal_left_first, second_definition.clone()]),
            expected
        );
        let runtime_factor = Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_multiply(u16_type, root.clone(), root.clone())
                .expect("root * root"),
        );
        assert_ne!(
            reconstruct(&[runtime_factor, second_definition.clone()]),
            expected
        );
        assert_ne!(
            reconstruct(&[first_definition.clone(), second_definition.clone()]),
            exact_integer_cast_obligation(
                u16_type,
                u8_type,
                second.clone(),
                &[first_definition.clone(), second_definition.clone()],
                &BTreeSet::new(),
            )
        );

        let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let signed_root_id = ValueId::new(211).expect("signed root");
        let signed_root = ScalarTerm::value(signed_root_id, ScalarType::Integer(i16_type));
        let signed_first = ScalarTerm::value(
            ValueId::new(212).expect("signed first"),
            ScalarType::Integer(i16_type),
        );
        let signed_second = ScalarTerm::value(
            ValueId::new(213).expect("signed second"),
            ScalarType::Integer(i16_type),
        );
        let signed_definitions = [
            Proposition::Equal(
                signed_first.clone(),
                ScalarTerm::exact_integer_multiply(
                    i16_type,
                    signed_root.clone(),
                    ScalarTerm::integer(i16_type, IntegerValue::Signed(2)).expect("2i16"),
                )
                .expect("signed root * 2i16"),
            ),
            Proposition::Equal(
                signed_second.clone(),
                ScalarTerm::exact_integer_multiply(
                    i16_type,
                    signed_first,
                    ScalarTerm::integer(i16_type, IntegerValue::Signed(3)).expect("3i16"),
                )
                .expect("signed first * 3i16"),
            ),
        ];
        assert_eq!(
            exact_integer_cast_obligation(
                i16_type,
                i8_type,
                signed_second,
                &signed_definitions,
                &BTreeSet::from([signed_root_id]),
            ),
            canonical_conjunction(vec![
                Proposition::LessOrEqual(
                    ScalarTerm::integer(i16_type, IntegerValue::Signed(-21)).expect("-21i16"),
                    signed_root.clone(),
                ),
                Proposition::LessOrEqual(
                    signed_root,
                    ScalarTerm::integer(i16_type, IntegerValue::Signed(21)).expect("21i16"),
                ),
            ])
        );

        let cross_signed_root_id = ValueId::new(221).expect("signed cross root");
        let cross_signed_root =
            ScalarTerm::value(cross_signed_root_id, ScalarType::Integer(i8_type));
        let cross_signed_product = ScalarTerm::value(
            ValueId::new(222).expect("signed cross product"),
            ScalarType::Integer(i8_type),
        );
        let cross_signed_definition = Proposition::Equal(
            cross_signed_product.clone(),
            ScalarTerm::exact_integer_multiply(
                i8_type,
                cross_signed_root.clone(),
                ScalarTerm::integer(i8_type, IntegerValue::Signed(2)).expect("2i8"),
            )
            .expect("signed cross root * 2i8"),
        );
        assert_eq!(
            exact_integer_cast_obligation(
                i8_type,
                u8_type,
                cross_signed_product,
                std::slice::from_ref(&cross_signed_definition),
                &BTreeSet::from([cross_signed_root_id]),
            ),
            Proposition::LessOrEqual(
                ScalarTerm::integer(i8_type, IntegerValue::Signed(0)).expect("0i8"),
                cross_signed_root,
            )
        );

        let cross_unsigned_root_id = ValueId::new(231).expect("unsigned cross root");
        let cross_unsigned_root =
            ScalarTerm::value(cross_unsigned_root_id, ScalarType::Integer(u8_type));
        let cross_unsigned_product = ScalarTerm::value(
            ValueId::new(232).expect("unsigned cross product"),
            ScalarType::Integer(u8_type),
        );
        let cross_unsigned_definition = Proposition::Equal(
            cross_unsigned_product.clone(),
            ScalarTerm::exact_integer_multiply(
                u8_type,
                cross_unsigned_root.clone(),
                ScalarTerm::integer(u8_type, IntegerValue::Unsigned(2)).expect("2u8"),
            )
            .expect("unsigned cross root * 2u8"),
        );
        assert_eq!(
            exact_integer_cast_obligation(
                u8_type,
                i8_type,
                cross_unsigned_product,
                std::slice::from_ref(&cross_unsigned_definition),
                &BTreeSet::from([cross_unsigned_root_id]),
            ),
            Proposition::LessOrEqual(
                cross_unsigned_root,
                ScalarTerm::integer(u8_type, IntegerValue::Unsigned(63)).expect("63u8"),
            )
        );

        let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
        let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
        let overflow_root_id = ValueId::new(241).expect("overflow root");
        let overflow_root = ScalarTerm::value(overflow_root_id, ScalarType::Integer(u64_type));
        let overflow_first = ScalarTerm::value(
            ValueId::new(242).expect("overflow first"),
            ScalarType::Integer(u64_type),
        );
        let overflow_second = ScalarTerm::value(
            ValueId::new(243).expect("overflow second"),
            ScalarType::Integer(u64_type),
        );
        let overflow_third = ScalarTerm::value(
            ValueId::new(244).expect("overflow third"),
            ScalarType::Integer(u64_type),
        );
        let maximum = ScalarTerm::integer(u64_type, IntegerValue::Unsigned(u64::MAX as u128))
            .expect("u64::MAX");
        let overflow_definitions = [
            Proposition::Equal(
                overflow_first.clone(),
                ScalarTerm::exact_integer_multiply(u64_type, overflow_root, maximum.clone())
                    .expect("overflow first"),
            ),
            Proposition::Equal(
                overflow_second.clone(),
                ScalarTerm::exact_integer_multiply(u64_type, overflow_first, maximum.clone())
                    .expect("overflow second"),
            ),
            Proposition::Equal(
                overflow_third.clone(),
                ScalarTerm::exact_integer_multiply(u64_type, overflow_second, maximum)
                    .expect("overflow third"),
            ),
        ];
        assert_eq!(
            exact_integer_cast_obligation(
                u64_type,
                i64_type,
                overflow_third,
                &overflow_definitions,
                &BTreeSet::from([overflow_root_id]),
            ),
            Proposition::Falsehood
        );
    }

    #[test]
    fn reconstructs_affine_chain_exact_cast_bounds_zero_collapse_and_fences() {
        let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let root_id = ValueId::new(251).expect("root");
        let value = |id| {
            ScalarTerm::value(
                ValueId::new(id).expect("value"),
                ScalarType::Integer(u16_type),
            )
        };
        let root = ScalarTerm::value(root_id, ScalarType::Integer(u16_type));
        let first = value(252);
        let second = value(253);
        let third = value(254);
        let definitions = [
            Proposition::Equal(
                first.clone(),
                ScalarTerm::exact_integer_add(
                    u16_type,
                    root.clone(),
                    ScalarTerm::integer(u16_type, IntegerValue::Unsigned(3)).expect("3u16"),
                )
                .expect("root + 3"),
            ),
            Proposition::Equal(
                second.clone(),
                ScalarTerm::exact_integer_multiply(
                    u16_type,
                    first.clone(),
                    ScalarTerm::integer(u16_type, IntegerValue::Unsigned(2)).expect("2u16"),
                )
                .expect("first * 2"),
            ),
            Proposition::Equal(
                third.clone(),
                ScalarTerm::exact_integer_subtract(
                    u16_type,
                    second.clone(),
                    ScalarTerm::integer(u16_type, IntegerValue::Unsigned(1)).expect("1u16"),
                )
                .expect("second - 1"),
            ),
        ];
        let parameters = BTreeSet::from([root_id]);
        let expected = Proposition::LessOrEqual(
            root.clone(),
            ScalarTerm::integer(u16_type, IntegerValue::Unsigned(125)).expect("125u16"),
        );
        let reconstruct = |axioms: &[Proposition]| {
            exact_integer_cast_obligation(u16_type, u8_type, third.clone(), axioms, &parameters)
        };
        assert_eq!(reconstruct(&definitions), expected);
        assert_ne!(
            reconstruct(&[
                definitions[1].clone(),
                definitions[0].clone(),
                definitions[2].clone(),
            ]),
            expected,
        );
        assert_eq!(
            exact_integer_affine_chain_cast_obligation(
                u16_type,
                u8_type,
                first.clone(),
                &definitions[..1],
                &parameters,
            ),
            None,
            "homogeneous offset chains remain on the narrower cast path",
        );

        let zero = value(255);
        let constant = value(256);
        let zero_definitions = [
            definitions[0].clone(),
            Proposition::Equal(
                zero.clone(),
                ScalarTerm::exact_integer_multiply(
                    u16_type,
                    first.clone(),
                    ScalarTerm::integer(u16_type, IntegerValue::Unsigned(0)).expect("0u16"),
                )
                .expect("first * 0"),
            ),
            Proposition::Equal(
                constant.clone(),
                ScalarTerm::exact_integer_add(
                    u16_type,
                    zero.clone(),
                    ScalarTerm::integer(u16_type, IntegerValue::Unsigned(255)).expect("255u16"),
                )
                .expect("zero + 255"),
            ),
        ];
        assert_eq!(
            exact_integer_cast_obligation(
                u16_type,
                u8_type,
                constant,
                &zero_definitions,
                &parameters,
            ),
            Proposition::Truth,
            "a zero coefficient discharges only the cast when its offset fits the target",
        );
        let outside = value(257);
        let outside_definition = Proposition::Equal(
            outside.clone(),
            ScalarTerm::exact_integer_add(
                u16_type,
                zero,
                ScalarTerm::integer(u16_type, IntegerValue::Unsigned(256)).expect("256u16"),
            )
            .expect("zero + 256"),
        );
        assert_eq!(
            exact_integer_cast_obligation(
                u16_type,
                u8_type,
                outside,
                &[
                    zero_definitions[0].clone(),
                    zero_definitions[1].clone(),
                    outside_definition,
                ],
                &parameters,
            ),
            Proposition::Falsehood,
            "a zero coefficient still checks target representability of its offset",
        );

        let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
        let signed_root_id = ValueId::new(261).expect("signed root");
        let signed_value = |id| {
            ScalarTerm::value(
                ValueId::new(id).expect("signed value"),
                ScalarType::Integer(i16_type),
            )
        };
        let signed_root = ScalarTerm::value(signed_root_id, ScalarType::Integer(i16_type));
        let signed_first = signed_value(262);
        let signed_second = signed_value(263);
        let signed_third = signed_value(264);
        let signed_definitions = [
            Proposition::Equal(
                signed_first.clone(),
                ScalarTerm::exact_integer_subtract(
                    i16_type,
                    signed_root.clone(),
                    ScalarTerm::integer(i16_type, IntegerValue::Signed(3)).expect("3i16"),
                )
                .expect("root - 3"),
            ),
            Proposition::Equal(
                signed_second.clone(),
                ScalarTerm::exact_integer_multiply(
                    i16_type,
                    signed_first,
                    ScalarTerm::integer(i16_type, IntegerValue::Signed(2)).expect("2i16"),
                )
                .expect("first * 2"),
            ),
            Proposition::Equal(
                signed_third.clone(),
                ScalarTerm::exact_integer_add(
                    i16_type,
                    signed_second,
                    ScalarTerm::integer(i16_type, IntegerValue::Signed(1)).expect("1i16"),
                )
                .expect("second + 1"),
            ),
        ];
        assert_eq!(
            exact_integer_cast_obligation(
                i16_type,
                u8_type,
                signed_third,
                &signed_definitions,
                &BTreeSet::from([signed_root_id]),
            ),
            canonical_conjunction(vec![
                Proposition::LessOrEqual(
                    ScalarTerm::integer(i16_type, IntegerValue::Signed(3)).expect("3i16"),
                    signed_root.clone(),
                ),
                Proposition::LessOrEqual(
                    signed_root,
                    ScalarTerm::integer(i16_type, IntegerValue::Signed(130)).expect("130i16"),
                ),
            ]),
        );

        let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
        let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
        let overflow_root_id = ValueId::new(271).expect("overflow root");
        let overflow_value = |id| {
            ScalarTerm::value(
                ValueId::new(id).expect("overflow value"),
                ScalarType::Integer(u64_type),
            )
        };
        let overflow_root = ScalarTerm::value(overflow_root_id, ScalarType::Integer(u64_type));
        let overflow_offset = overflow_value(272);
        let overflow_first = overflow_value(273);
        let overflow_second = overflow_value(274);
        let overflow_third = overflow_value(275);
        let maximum = ScalarTerm::integer(u64_type, IntegerValue::Unsigned(u128::from(u64::MAX)))
            .expect("u64::MAX");
        let overflow_definitions = [
            Proposition::Equal(
                overflow_offset.clone(),
                ScalarTerm::exact_integer_add(
                    u64_type,
                    overflow_root,
                    ScalarTerm::integer(u64_type, IntegerValue::Unsigned(1)).expect("1u64"),
                )
                .expect("root + 1"),
            ),
            Proposition::Equal(
                overflow_first.clone(),
                ScalarTerm::exact_integer_multiply(u64_type, overflow_offset, maximum.clone())
                    .expect("offset * MAX"),
            ),
            Proposition::Equal(
                overflow_second.clone(),
                ScalarTerm::exact_integer_multiply(u64_type, overflow_first, maximum.clone())
                    .expect("first * MAX"),
            ),
            Proposition::Equal(
                overflow_third.clone(),
                ScalarTerm::exact_integer_multiply(u64_type, overflow_second, maximum)
                    .expect("second * MAX"),
            ),
        ];
        assert_eq!(
            exact_integer_cast_obligation(
                u64_type,
                i64_type,
                overflow_third,
                &overflow_definitions,
                &BTreeSet::from([overflow_root_id]),
            ),
            Proposition::Falsehood,
            "coefficient or offset composition overflow fails closed",
        );
    }

    #[test]
    fn reconstructs_shift_left_chain_exact_cast_bounds_and_rejects_broken_definitions() {
        let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
        let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
        let root_id = ValueId::new(301).expect("root");
        let root = ScalarTerm::value(root_id, ScalarType::Integer(u16_type));
        let first = ScalarTerm::value(
            ValueId::new(302).expect("first"),
            ScalarType::Integer(u16_type),
        );
        let second = ScalarTerm::value(
            ValueId::new(303).expect("second"),
            ScalarType::Integer(u16_type),
        );
        let first_definition = Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_shift_left(
                u16_type,
                i8_type,
                root.clone(),
                ScalarTerm::integer(i8_type, IntegerValue::Signed(1)).expect("1i8"),
            )
            .expect("root << 1i8"),
        );
        let second_definition = Proposition::Equal(
            second.clone(),
            ScalarTerm::exact_integer_shift_left(
                u16_type,
                i32_type,
                first.clone(),
                ScalarTerm::integer(i32_type, IntegerValue::Signed(2)).expect("2i32"),
            )
            .expect("first << 2i32"),
        );
        let parameters = BTreeSet::from([root_id]);
        let expected = Proposition::LessOrEqual(
            root.clone(),
            ScalarTerm::integer(u16_type, IntegerValue::Unsigned(31)).expect("31u16"),
        );
        let reconstruct = |axioms: &[Proposition]| {
            exact_integer_cast_obligation(u16_type, u8_type, second.clone(), axioms, &parameters)
        };
        assert_eq!(
            exact_integer_cast_obligation(
                u16_type,
                u8_type,
                first.clone(),
                std::slice::from_ref(&first_definition),
                &parameters,
            ),
            Proposition::LessOrEqual(
                root.clone(),
                ScalarTerm::integer(u16_type, IntegerValue::Unsigned(127)).expect("127u16"),
            )
        );
        assert_eq!(
            reconstruct(&[first_definition.clone(), second_definition.clone()]),
            expected
        );
        let identity = ScalarTerm::value(
            ValueId::new(304).expect("identity"),
            ScalarType::Integer(u16_type),
        );
        let identity_definition = Proposition::Equal(
            identity.clone(),
            ScalarTerm::exact_integer_shift_left(
                u16_type,
                u8_type,
                root.clone(),
                ScalarTerm::integer(u8_type, IntegerValue::Unsigned(0)).expect("0u8"),
            )
            .expect("root << 0u8"),
        );
        assert_eq!(
            exact_integer_cast_obligation(
                u16_type,
                u8_type,
                identity,
                std::slice::from_ref(&identity_definition),
                &parameters,
            ),
            Proposition::LessOrEqual(
                root.clone(),
                ScalarTerm::integer(u16_type, IntegerValue::Unsigned(255)).expect("255u16"),
            )
        );

        assert_ne!(
            reconstruct(&[second_definition.clone(), first_definition.clone()]),
            expected
        );
        let reversed_second = match second_definition.clone() {
            Proposition::Equal(left, right) => Proposition::Equal(right, left),
            _ => unreachable!("definition is an equality"),
        };
        assert_ne!(
            reconstruct(&[first_definition.clone(), reversed_second]),
            expected
        );
        let runtime_count = Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_shift_left(u16_type, u16_type, root.clone(), root.clone())
                .expect("root << root"),
        );
        assert_ne!(
            reconstruct(&[runtime_count, second_definition.clone()]),
            expected
        );
        let negative_count = Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_shift_left(
                u16_type,
                i8_type,
                root.clone(),
                ScalarTerm::integer(i8_type, IntegerValue::Signed(-1)).expect("-1i8"),
            )
            .expect("root << -1i8 shape"),
        );
        assert_ne!(
            reconstruct(&[negative_count, second_definition.clone()]),
            expected
        );
        let width_count = Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_shift_left(
                u16_type,
                u8_type,
                root.clone(),
                ScalarTerm::integer(u8_type, IntegerValue::Unsigned(16)).expect("16u8"),
            )
            .expect("root << 16u8 shape"),
        );
        assert_ne!(
            reconstruct(&[width_count, second_definition.clone()]),
            expected
        );
        assert_ne!(
            exact_integer_cast_obligation(
                u16_type,
                u8_type,
                second.clone(),
                &[first_definition.clone(), second_definition.clone()],
                &BTreeSet::new(),
            ),
            expected
        );

        let signed_root_id = ValueId::new(311).expect("signed root");
        let signed_root = ScalarTerm::value(signed_root_id, ScalarType::Integer(i16_type));
        let signed_first = ScalarTerm::value(
            ValueId::new(312).expect("signed first"),
            ScalarType::Integer(i16_type),
        );
        let signed_second = ScalarTerm::value(
            ValueId::new(313).expect("signed second"),
            ScalarType::Integer(i16_type),
        );
        let signed_definitions = [
            Proposition::Equal(
                signed_first.clone(),
                ScalarTerm::exact_integer_shift_left(
                    i16_type,
                    u8_type,
                    signed_root.clone(),
                    ScalarTerm::integer(u8_type, IntegerValue::Unsigned(1)).expect("1u8"),
                )
                .expect("signed root << 1u8"),
            ),
            Proposition::Equal(
                signed_second.clone(),
                ScalarTerm::exact_integer_shift_left(
                    i16_type,
                    i32_type,
                    signed_first,
                    ScalarTerm::integer(i32_type, IntegerValue::Signed(2)).expect("2i32"),
                )
                .expect("signed first << 2i32"),
            ),
        ];
        assert_eq!(
            exact_integer_cast_obligation(
                i16_type,
                i8_type,
                signed_second,
                &signed_definitions,
                &BTreeSet::from([signed_root_id]),
            ),
            canonical_conjunction(vec![
                Proposition::LessOrEqual(
                    ScalarTerm::integer(i16_type, IntegerValue::Signed(-16)).expect("-16i16"),
                    signed_root.clone(),
                ),
                Proposition::LessOrEqual(
                    signed_root,
                    ScalarTerm::integer(i16_type, IntegerValue::Signed(15)).expect("15i16"),
                ),
            ])
        );

        let signed_cross_root_id = ValueId::new(321).expect("signed cross root");
        let signed_cross_root =
            ScalarTerm::value(signed_cross_root_id, ScalarType::Integer(i8_type));
        let signed_cross_result = ScalarTerm::value(
            ValueId::new(322).expect("signed cross result"),
            ScalarType::Integer(i8_type),
        );
        let signed_cross_definition = Proposition::Equal(
            signed_cross_result.clone(),
            ScalarTerm::exact_integer_shift_left(
                i8_type,
                u16_type,
                signed_cross_root.clone(),
                ScalarTerm::integer(u16_type, IntegerValue::Unsigned(1)).expect("1u16"),
            )
            .expect("signed cross root << 1u16"),
        );
        assert_eq!(
            exact_integer_cast_obligation(
                i8_type,
                u8_type,
                signed_cross_result,
                std::slice::from_ref(&signed_cross_definition),
                &BTreeSet::from([signed_cross_root_id]),
            ),
            Proposition::LessOrEqual(
                ScalarTerm::integer(i8_type, IntegerValue::Signed(0)).expect("0i8"),
                signed_cross_root,
            )
        );

        let unsigned_cross_root_id = ValueId::new(331).expect("unsigned cross root");
        let unsigned_cross_root =
            ScalarTerm::value(unsigned_cross_root_id, ScalarType::Integer(u8_type));
        let unsigned_cross_result = ScalarTerm::value(
            ValueId::new(332).expect("unsigned cross result"),
            ScalarType::Integer(u8_type),
        );
        let unsigned_cross_definition = Proposition::Equal(
            unsigned_cross_result.clone(),
            ScalarTerm::exact_integer_shift_left(
                u8_type,
                i16_type,
                unsigned_cross_root.clone(),
                ScalarTerm::integer(i16_type, IntegerValue::Signed(1)).expect("1i16"),
            )
            .expect("unsigned cross root << 1i16"),
        );
        assert_eq!(
            exact_integer_cast_obligation(
                u8_type,
                i8_type,
                unsigned_cross_result,
                std::slice::from_ref(&unsigned_cross_definition),
                &BTreeSet::from([unsigned_cross_root_id]),
            ),
            Proposition::LessOrEqual(
                unsigned_cross_root,
                ScalarTerm::integer(u8_type, IntegerValue::Unsigned(63)).expect("63u8"),
            )
        );

        let narrow_zero_root_id = ValueId::new(341).expect("narrow zero root");
        let narrow_zero_root =
            ScalarTerm::value(narrow_zero_root_id, ScalarType::Integer(u16_type));
        let narrow_zero_result = ScalarTerm::value(
            ValueId::new(342).expect("narrow zero result"),
            ScalarType::Integer(u16_type),
        );
        let narrow_zero_definition = Proposition::Equal(
            narrow_zero_result.clone(),
            ScalarTerm::exact_integer_shift_left(
                u16_type,
                u8_type,
                narrow_zero_root.clone(),
                ScalarTerm::integer(u8_type, IntegerValue::Unsigned(8)).expect("8u8"),
            )
            .expect("narrow zero root << 8u8"),
        );
        assert_eq!(
            exact_integer_cast_obligation(
                u16_type,
                i8_type,
                narrow_zero_result,
                std::slice::from_ref(&narrow_zero_definition),
                &BTreeSet::from([narrow_zero_root_id]),
            ),
            Proposition::LessOrEqual(
                narrow_zero_root,
                ScalarTerm::integer(u16_type, IntegerValue::Unsigned(0)).expect("0u16"),
            ),
            "a sub-source-width shift beyond the target width independently narrows the cast interval to zero"
        );

        let width_root_id = ValueId::new(351).expect("width root");
        let width_root = ScalarTerm::value(width_root_id, ScalarType::Integer(u8_type));
        let width_first = ScalarTerm::value(
            ValueId::new(352).expect("width first"),
            ScalarType::Integer(u8_type),
        );
        let width_second = ScalarTerm::value(
            ValueId::new(353).expect("width second"),
            ScalarType::Integer(u8_type),
        );
        let width_definitions = [
            Proposition::Equal(
                width_first.clone(),
                ScalarTerm::exact_integer_shift_left(
                    u8_type,
                    u8_type,
                    width_root,
                    ScalarTerm::integer(u8_type, IntegerValue::Unsigned(4)).expect("4u8"),
                )
                .expect("width root << 4u8"),
            ),
            Proposition::Equal(
                width_second.clone(),
                ScalarTerm::exact_integer_shift_left(
                    u8_type,
                    i16_type,
                    width_first,
                    ScalarTerm::integer(i16_type, IntegerValue::Signed(4)).expect("4i16"),
                )
                .expect("width first << 4i16"),
            ),
        ];
        assert_eq!(
            exact_integer_cast_obligation(
                u8_type,
                i8_type,
                width_second,
                &width_definitions,
                &BTreeSet::from([width_root_id]),
            ),
            Proposition::Truth,
            "the cast of a successfully produced source-width exact shift result is zero-valued without importing either shift proof"
        );
    }

    #[test]
    fn reconstructs_shift_right_chain_exact_cast_preimages_and_saturation() {
        let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let root_id = ValueId::new(401).expect("root");
        let root = ScalarTerm::value(root_id, ScalarType::Integer(u16_type));
        let first = ScalarTerm::value(
            ValueId::new(402).expect("first"),
            ScalarType::Integer(u16_type),
        );
        let second = ScalarTerm::value(
            ValueId::new(403).expect("second"),
            ScalarType::Integer(u16_type),
        );
        let first_definition = Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_shift_right(
                u16_type,
                i8_type,
                root.clone(),
                ScalarTerm::integer(i8_type, IntegerValue::Signed(1)).expect("1i8"),
            )
            .expect("root >> 1i8"),
        );
        let second_definition = Proposition::Equal(
            second.clone(),
            ScalarTerm::exact_integer_shift_right(
                u16_type,
                u16_type,
                first.clone(),
                ScalarTerm::integer(u16_type, IntegerValue::Unsigned(2)).expect("2u16"),
            )
            .expect("first >> 2u16"),
        );
        let parameters = BTreeSet::from([root_id]);
        let expected = Proposition::LessOrEqual(
            root.clone(),
            ScalarTerm::integer(u16_type, IntegerValue::Unsigned(2047)).expect("2047u16"),
        );
        assert_eq!(
            exact_integer_cast_obligation(
                u16_type,
                u8_type,
                second.clone(),
                &[first_definition.clone(), second_definition.clone()],
                &parameters,
            ),
            expected
        );
        assert_ne!(
            exact_integer_cast_obligation(
                u16_type,
                u8_type,
                second.clone(),
                &[second_definition.clone(), first_definition.clone()],
                &parameters,
            ),
            expected
        );
        let runtime_count = Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_shift_right(u16_type, u16_type, root.clone(), root.clone())
                .expect("runtime count shape"),
        );
        assert_ne!(
            exact_integer_cast_obligation(
                u16_type,
                u8_type,
                second,
                &[runtime_count, second_definition],
                &parameters,
            ),
            expected
        );

        let signed_root = ScalarTerm::value(
            ValueId::new(411).expect("signed root"),
            ScalarType::Integer(i16_type),
        );
        assert_eq!(
            exact_integer_shift_right_chain_cast_interval_obligation(
                i16_type,
                i8_type,
                signed_root.clone(),
                3,
            ),
            canonical_conjunction(vec![
                Proposition::LessOrEqual(
                    ScalarTerm::integer(i16_type, IntegerValue::Signed(-1024)).expect("-1024i16"),
                    signed_root.clone(),
                ),
                Proposition::LessOrEqual(
                    signed_root.clone(),
                    ScalarTerm::integer(i16_type, IntegerValue::Signed(1023)).expect("1023i16"),
                ),
            ])
        );
        assert_eq!(
            exact_integer_shift_right_chain_cast_interval_obligation(
                i16_type,
                u8_type,
                signed_root.clone(),
                16,
            ),
            Proposition::LessOrEqual(
                ScalarTerm::integer(i16_type, IntegerValue::Signed(0)).expect("0i16"),
                signed_root,
            ),
            "signed saturation leaves -1 or 0, and an unsigned cast admits only the nonnegative root"
        );
        assert_eq!(
            exact_integer_shift_right_chain_cast_interval_obligation(u16_type, u8_type, root, 16,),
            Proposition::Truth,
            "unsigned saturation yields zero"
        );
        let unsigned_cross_root = ScalarTerm::value(
            ValueId::new(421).expect("unsigned cross root"),
            ScalarType::Integer(u8_type),
        );
        assert_eq!(
            exact_integer_shift_right_chain_cast_interval_obligation(
                u8_type,
                i8_type,
                unsigned_cross_root,
                1,
            ),
            Proposition::Truth,
            "one zero-fill shift makes every u8 result fit i8"
        );
    }

    #[test]
    fn reconstructs_carrier_total_divide_remainder_chain_exact_casts() {
        let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let root_id = ValueId::new(451).expect("root");
        let root = ScalarTerm::value(root_id, ScalarType::Integer(u16_type));
        let first = ScalarTerm::value(
            ValueId::new(452).expect("first"),
            ScalarType::Integer(u16_type),
        );
        let second = ScalarTerm::value(
            ValueId::new(453).expect("second"),
            ScalarType::Integer(u16_type),
        );
        let first_definition = Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_divide(
                u16_type,
                root.clone(),
                ScalarTerm::integer(u16_type, IntegerValue::Unsigned(2)).expect("2u16"),
            )
            .expect("root / 2u16"),
        );
        let second_definition = Proposition::Equal(
            second.clone(),
            ScalarTerm::exact_integer_remainder(
                u16_type,
                first.clone(),
                ScalarTerm::integer(u16_type, IntegerValue::Unsigned(3)).expect("3u16"),
            )
            .expect("first % 3u16"),
        );
        let parameters = BTreeSet::from([root_id]);
        assert_eq!(
            exact_integer_cast_obligation(
                u16_type,
                u8_type,
                second.clone(),
                &[first_definition.clone(), second_definition.clone()],
                &parameters,
            ),
            Proposition::Truth,
        );
        assert_ne!(
            exact_integer_cast_obligation(
                u16_type,
                u8_type,
                second,
                &[second_definition, first_definition.clone()],
                &parameters,
            ),
            Proposition::Truth,
            "out-of-order definitions do not authorize the cast",
        );
        assert_ne!(
            exact_integer_cast_obligation(
                u16_type,
                u8_type,
                first,
                std::slice::from_ref(&first_definition),
                &parameters,
            ),
            Proposition::Truth,
            "a noncontained quotient hull stays outside the carrier-total family",
        );

        let signed_root_id = ValueId::new(461).expect("signed root");
        let signed_root = ScalarTerm::value(signed_root_id, ScalarType::Integer(i16_type));
        let signed_result = ScalarTerm::value(
            ValueId::new(462).expect("signed result"),
            ScalarType::Integer(i16_type),
        );
        let signed_definition = Proposition::Equal(
            signed_result.clone(),
            ScalarTerm::exact_integer_remainder(
                i16_type,
                signed_root,
                ScalarTerm::integer(i16_type, IntegerValue::Signed(-3)).expect("-3i16"),
            )
            .expect("signed root % -3i16"),
        );
        assert_eq!(
            exact_integer_cast_obligation(
                i16_type,
                i8_type,
                signed_result,
                std::slice::from_ref(&signed_definition),
                &BTreeSet::from([signed_root_id]),
            ),
            Proposition::Truth,
        );
        assert_eq!(
            exact_integer_cast_obligation(
                u16_type,
                i8_type,
                ScalarTerm::value(
                    ValueId::new(463).expect("cross result"),
                    ScalarType::Integer(u16_type),
                ),
                &[Proposition::Equal(
                    ScalarTerm::value(
                        ValueId::new(463).expect("cross result"),
                        ScalarType::Integer(u16_type),
                    ),
                    ScalarTerm::exact_integer_remainder(
                        u16_type,
                        root,
                        ScalarTerm::integer(u16_type, IntegerValue::Unsigned(3)).expect("3u16"),
                    )
                    .expect("root % 3u16"),
                )],
                &parameters,
            ),
            Proposition::Truth,
            "a nonnegative remainder hull may cross to a signed target",
        );
    }

    #[test]
    fn reconstructs_offset_chain_exact_cast_bounds_and_rejects_broken_definitions() {
        let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let root_id = ValueId::new(101).expect("root");
        let root = ScalarTerm::value(root_id, ScalarType::Integer(u16_type));
        let first = ScalarTerm::value(
            ValueId::new(102).expect("first"),
            ScalarType::Integer(u16_type),
        );
        let second = ScalarTerm::value(
            ValueId::new(103).expect("second"),
            ScalarType::Integer(u16_type),
        );
        let first_definition = Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_add(
                u16_type,
                root.clone(),
                ScalarTerm::integer(u16_type, IntegerValue::Unsigned(5)).expect("5u16"),
            )
            .expect("root + 5u16"),
        );
        let second_definition = Proposition::Equal(
            second.clone(),
            ScalarTerm::exact_integer_subtract(
                u16_type,
                first.clone(),
                ScalarTerm::integer(u16_type, IntegerValue::Unsigned(3)).expect("3u16"),
            )
            .expect("first - 3u16"),
        );
        let parameters = BTreeSet::from([root_id]);
        let expected = Proposition::LessOrEqual(
            root.clone(),
            ScalarTerm::integer(u16_type, IntegerValue::Unsigned(253)).expect("253u16"),
        );
        let reconstruct = |axioms: &[Proposition]| {
            exact_integer_cast_obligation(u16_type, u8_type, second.clone(), axioms, &parameters)
        };
        assert_eq!(
            reconstruct(&[first_definition.clone(), second_definition.clone()]),
            expected
        );
        assert_ne!(
            reconstruct(&[second_definition.clone(), first_definition.clone()]),
            expected
        );
        let reversed_second = match second_definition.clone() {
            Proposition::Equal(left, right) => Proposition::Equal(right, left),
            _ => unreachable!("definition is an equality"),
        };
        assert_ne!(
            reconstruct(&[first_definition.clone(), reversed_second]),
            expected
        );
        let redirected_second = Proposition::Equal(
            second.clone(),
            ScalarTerm::integer(u16_type, IntegerValue::Unsigned(2)).expect("2u16"),
        );
        assert_ne!(
            reconstruct(&[first_definition.clone(), redirected_second]),
            expected
        );
        let cyclic_first = Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_add(
                u16_type,
                second.clone(),
                ScalarTerm::integer(u16_type, IntegerValue::Unsigned(5)).expect("5u16"),
            )
            .expect("second + 5u16"),
        );
        assert_ne!(
            reconstruct(&[cyclic_first, second_definition.clone()]),
            expected
        );
        let literal_left_first = Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_add(
                u16_type,
                ScalarTerm::integer(u16_type, IntegerValue::Unsigned(5)).expect("5u16"),
                root.clone(),
            )
            .expect("5u16 + root"),
        );
        assert_ne!(
            reconstruct(&[literal_left_first, second_definition.clone()]),
            expected
        );
        assert_ne!(
            exact_integer_cast_obligation(
                u16_type,
                u8_type,
                second.clone(),
                &[first_definition.clone(), second_definition.clone()],
                &BTreeSet::new(),
            ),
            expected
        );

        let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let signed_root_id = ValueId::new(111).expect("signed root");
        let signed_root = ScalarTerm::value(signed_root_id, ScalarType::Integer(i16_type));
        let signed_first = ScalarTerm::value(
            ValueId::new(112).expect("signed first"),
            ScalarType::Integer(i16_type),
        );
        let signed_second = ScalarTerm::value(
            ValueId::new(113).expect("signed second"),
            ScalarType::Integer(i16_type),
        );
        let signed_definitions = [
            Proposition::Equal(
                signed_first.clone(),
                ScalarTerm::exact_integer_add(
                    i16_type,
                    signed_root.clone(),
                    ScalarTerm::integer(i16_type, IntegerValue::Signed(5)).expect("5i16"),
                )
                .expect("signed root + 5i16"),
            ),
            Proposition::Equal(
                signed_second.clone(),
                ScalarTerm::exact_integer_subtract(
                    i16_type,
                    signed_first,
                    ScalarTerm::integer(i16_type, IntegerValue::Signed(3)).expect("3i16"),
                )
                .expect("signed first - 3i16"),
            ),
        ];
        assert_eq!(
            exact_integer_cast_obligation(
                i16_type,
                i8_type,
                signed_second,
                &signed_definitions,
                &BTreeSet::from([signed_root_id]),
            ),
            canonical_conjunction(vec![
                Proposition::LessOrEqual(
                    ScalarTerm::integer(i16_type, IntegerValue::Signed(-130)).expect("-130i16"),
                    signed_root.clone(),
                ),
                Proposition::LessOrEqual(
                    signed_root,
                    ScalarTerm::integer(i16_type, IntegerValue::Signed(125)).expect("125i16"),
                ),
            ])
        );

        let i8_root_id = ValueId::new(121).expect("i8 root");
        let i8_root = ScalarTerm::value(i8_root_id, ScalarType::Integer(i8_type));
        let i8_offset = ScalarTerm::value(
            ValueId::new(122).expect("i8 offset"),
            ScalarType::Integer(i8_type),
        );
        let i8_definition = Proposition::Equal(
            i8_offset.clone(),
            ScalarTerm::exact_integer_subtract(
                i8_type,
                i8_root.clone(),
                ScalarTerm::integer(i8_type, IntegerValue::Signed(1)).expect("1i8"),
            )
            .expect("i8 root - 1i8"),
        );
        assert_eq!(
            exact_integer_cast_obligation(
                i8_type,
                u8_type,
                i8_offset,
                std::slice::from_ref(&i8_definition),
                &BTreeSet::from([i8_root_id]),
            ),
            Proposition::LessOrEqual(
                ScalarTerm::integer(i8_type, IntegerValue::Signed(1)).expect("1i8"),
                i8_root,
            )
        );

        let u8_root_id = ValueId::new(131).expect("u8 root");
        let u8_root = ScalarTerm::value(u8_root_id, ScalarType::Integer(u8_type));
        let impossible = ScalarTerm::value(
            ValueId::new(132).expect("impossible"),
            ScalarType::Integer(u8_type),
        );
        let impossible_definition = Proposition::Equal(
            impossible.clone(),
            ScalarTerm::exact_integer_add(
                u8_type,
                u8_root,
                ScalarTerm::integer(u8_type, IntegerValue::Unsigned(128)).expect("128u8"),
            )
            .expect("u8 root + 128u8"),
        );
        assert_eq!(
            exact_integer_cast_obligation(
                u8_type,
                i8_type,
                impossible,
                std::slice::from_ref(&impossible_definition),
                &BTreeSet::from([u8_root_id]),
            ),
            Proposition::Falsehood
        );

        let truth_root_id = ValueId::new(141).expect("truth root");
        let truth_root = ScalarTerm::value(truth_root_id, ScalarType::Integer(i8_type));
        let truth_first = ScalarTerm::value(
            ValueId::new(142).expect("truth first"),
            ScalarType::Integer(i8_type),
        );
        let truth_second = ScalarTerm::value(
            ValueId::new(143).expect("truth second"),
            ScalarType::Integer(i8_type),
        );
        let truth_definitions = [
            Proposition::Equal(
                truth_first.clone(),
                ScalarTerm::exact_integer_add(
                    i8_type,
                    truth_root,
                    ScalarTerm::integer(i8_type, IntegerValue::Signed(127)).expect("127i8"),
                )
                .expect("truth root + 127i8"),
            ),
            Proposition::Equal(
                truth_second.clone(),
                ScalarTerm::exact_integer_add(
                    i8_type,
                    truth_first,
                    ScalarTerm::integer(i8_type, IntegerValue::Signed(1)).expect("1i8"),
                )
                .expect("truth first + 1i8"),
            ),
        ];
        assert_eq!(
            exact_integer_cast_obligation(
                i8_type,
                u8_type,
                truth_second,
                &truth_definitions,
                &BTreeSet::from([truth_root_id]),
            ),
            Proposition::Truth,
            "the shifted target interval may cover the complete source carrier"
        );

        let overflow_root_id = ValueId::new(151).expect("overflow root");
        let overflow_root = ScalarTerm::value(overflow_root_id, ScalarType::Integer(u8_type));
        let overflow_first = ScalarTerm::value(
            ValueId::new(152).expect("overflow first"),
            ScalarType::Integer(u8_type),
        );
        let overflow_second = ScalarTerm::value(
            ValueId::new(153).expect("overflow second"),
            ScalarType::Integer(u8_type),
        );
        let overflow_definitions = [
            Proposition::Equal(
                overflow_first.clone(),
                ScalarTerm::exact_integer_add(
                    u8_type,
                    overflow_root,
                    ScalarTerm::integer(u8_type, IntegerValue::Unsigned(200)).expect("200u8"),
                )
                .expect("overflow root + 200u8"),
            ),
            Proposition::Equal(
                overflow_second.clone(),
                ScalarTerm::exact_integer_add(
                    u8_type,
                    overflow_first,
                    ScalarTerm::integer(u8_type, IntegerValue::Unsigned(100)).expect("100u8"),
                )
                .expect("overflow first + 100u8"),
            ),
        ];
        assert_eq!(
            exact_integer_cast_obligation(
                u8_type,
                i8_type,
                overflow_second,
                &overflow_definitions,
                &BTreeSet::from([overflow_root_id]),
            ),
            Proposition::Falsehood
        );
    }

    #[test]
    fn reconstructs_a_finite_exact_offset_chain_after_a_direct_partial_cast() {
        let source_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
        let target_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let root_id = ValueId::new(161).expect("root");
        let root = ScalarTerm::value(root_id, ScalarType::Integer(source_type));
        let cast = ScalarTerm::value(
            ValueId::new(162).expect("cast"),
            ScalarType::Integer(target_type),
        );
        let cast_definition = Proposition::Equal(
            cast.clone(),
            ScalarTerm::integer_exact_cast(source_type, target_type, root.clone())
                .expect("u16 to u8 exact cast"),
        );
        let parameters = BTreeSet::from([root_id]);
        assert_eq!(
            exact_integer_cast_obligation(source_type, target_type, root.clone(), &[], &parameters,),
            Proposition::LessOrEqual(
                root.clone(),
                ScalarTerm::integer(source_type, IntegerValue::Unsigned(255)).expect("255u16"),
            )
        );
        let five = ScalarTerm::integer(target_type, IntegerValue::Unsigned(5)).expect("5u8");
        let expected_add = Proposition::LessOrEqual(
            root.clone(),
            ScalarTerm::integer(source_type, IntegerValue::Unsigned(250)).expect("250u16"),
        );
        assert_eq!(
            exact_integer_add_obligation(
                target_type,
                cast.clone(),
                five.clone(),
                std::slice::from_ref(&cast_definition),
                1,
                &parameters,
            ),
            expected_add
        );
        let expected_subtract = canonical_conjunction(vec![
            Proposition::LessOrEqual(
                ScalarTerm::integer(source_type, IntegerValue::Unsigned(5)).expect("5u16"),
                root.clone(),
            ),
            Proposition::LessOrEqual(
                root.clone(),
                ScalarTerm::integer(source_type, IntegerValue::Unsigned(260)).expect("260u16"),
            ),
        ]);
        assert_eq!(
            exact_integer_subtract_obligation(
                target_type,
                cast.clone(),
                five.clone(),
                std::slice::from_ref(&cast_definition),
                1,
                &parameters,
            ),
            expected_subtract
        );

        let first = ScalarTerm::value(
            ValueId::new(163).expect("first offset"),
            ScalarType::Integer(target_type),
        );
        let first_definition = Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_add(target_type, cast.clone(), five.clone())
                .expect("cast + 5u8"),
        );
        let three = ScalarTerm::integer(target_type, IntegerValue::Unsigned(3)).expect("3u8");
        let second = ScalarTerm::value(
            ValueId::new(164).expect("second offset"),
            ScalarType::Integer(target_type),
        );
        let second_definition = Proposition::Equal(
            second.clone(),
            ScalarTerm::exact_integer_subtract(target_type, first.clone(), three.clone())
                .expect("(cast + 5u8) - 3u8"),
        );
        let two = ScalarTerm::integer(target_type, IntegerValue::Unsigned(2)).expect("2u8");
        let definitions = [
            cast_definition.clone(),
            first_definition.clone(),
            second_definition.clone(),
        ];
        assert_eq!(
            exact_integer_subtract_obligation(
                target_type,
                first.clone(),
                three,
                &definitions[..2],
                2,
                &parameters,
            ),
            Proposition::LessOrEqual(
                root.clone(),
                ScalarTerm::integer(source_type, IntegerValue::Unsigned(253)).expect("253u16"),
            ),
            "the second prefix reconstructs cumulative offset +2"
        );
        assert_eq!(
            exact_integer_add_obligation(target_type, second, two, &definitions, 3, &parameters,),
            Proposition::LessOrEqual(
                root.clone(),
                ScalarTerm::integer(source_type, IntegerValue::Unsigned(251)).expect("251u16"),
            ),
            "the third prefix reconstructs cumulative offset +4"
        );
        assert_eq!(
            exact_integer_subtract_obligation(
                target_type,
                first.clone(),
                five.clone(),
                &[cast_definition.clone(), first_definition.clone()],
                2,
                &parameters,
            ),
            Proposition::LessOrEqual(
                root.clone(),
                ScalarTerm::integer(source_type, IntegerValue::Unsigned(255)).expect("255u16"),
            ),
            "later cancellation reconstructs its own zero-offset bound without replacing the first-prefix proof"
        );
        assert_ne!(
            exact_integer_add_obligation(
                target_type,
                first.clone(),
                ScalarTerm::integer(target_type, IntegerValue::Unsigned(2)).expect("2u8"),
                &[first_definition.clone(), cast_definition.clone()],
                2,
                &parameters,
            ),
            Proposition::LessOrEqual(
                root.clone(),
                ScalarTerm::integer(source_type, IntegerValue::Unsigned(248)).expect("248u16"),
            ),
            "definitions must precede the operation that consumes them"
        );

        let two_hundred =
            ScalarTerm::integer(target_type, IntegerValue::Unsigned(200)).expect("200u8");
        let large_first = ScalarTerm::value(
            ValueId::new(165).expect("large first offset"),
            ScalarType::Integer(target_type),
        );
        let large_first_definition = Proposition::Equal(
            large_first.clone(),
            ScalarTerm::exact_integer_add(target_type, cast.clone(), two_hundred)
                .expect("cast + 200u8"),
        );
        assert_eq!(
            exact_integer_add_obligation(
                target_type,
                large_first,
                ScalarTerm::integer(target_type, IntegerValue::Unsigned(100)).expect("100u8"),
                &[cast_definition.clone(), large_first_definition],
                2,
                &parameters,
            ),
            Proposition::Falsehood,
            "a cumulative offset wider than the target carrier fails closed"
        );

        let reversed_definition = match cast_definition.clone() {
            Proposition::Equal(left, right) => Proposition::Equal(right, left),
            _ => unreachable!("cast definition is an equality"),
        };
        assert_ne!(
            exact_integer_add_obligation(
                target_type,
                cast.clone(),
                five.clone(),
                std::slice::from_ref(&reversed_definition),
                1,
                &parameters,
            ),
            expected_add
        );
        let redirected_definition = Proposition::Equal(
            cast.clone(),
            ScalarTerm::integer(target_type, IntegerValue::Unsigned(1)).expect("1u8"),
        );
        assert_ne!(
            exact_integer_add_obligation(
                target_type,
                cast.clone(),
                five.clone(),
                std::slice::from_ref(&redirected_definition),
                1,
                &parameters,
            ),
            expected_add
        );
        assert_ne!(
            exact_integer_add_obligation(
                target_type,
                cast.clone(),
                five.clone(),
                std::slice::from_ref(&cast_definition),
                1,
                &BTreeSet::new(),
            ),
            expected_add
        );
        assert_ne!(
            exact_integer_add_obligation(
                target_type,
                five.clone(),
                cast,
                std::slice::from_ref(&cast_definition),
                1,
                &parameters,
            ),
            expected_add,
            "literal-left addition is outside the canonical composition"
        );

        let signed_source = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
        let signed_target = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let signed_root_id = ValueId::new(171).expect("signed root");
        let signed_root = ScalarTerm::value(signed_root_id, ScalarType::Integer(signed_source));
        let signed_cast = ScalarTerm::value(
            ValueId::new(172).expect("signed cast"),
            ScalarType::Integer(signed_target),
        );
        let signed_definition = Proposition::Equal(
            signed_cast.clone(),
            ScalarTerm::integer_exact_cast(signed_source, signed_target, signed_root.clone())
                .expect("i16 to i8 exact cast"),
        );
        let negative_five =
            ScalarTerm::integer(signed_target, IntegerValue::Signed(-5)).expect("-5i8");
        assert_eq!(
            exact_integer_add_obligation(
                signed_target,
                signed_cast,
                negative_five,
                std::slice::from_ref(&signed_definition),
                1,
                &BTreeSet::from([signed_root_id]),
            ),
            canonical_conjunction(vec![
                Proposition::LessOrEqual(
                    ScalarTerm::integer(signed_source, IntegerValue::Signed(-123))
                        .expect("-123i16"),
                    signed_root.clone(),
                ),
                Proposition::LessOrEqual(
                    signed_root,
                    ScalarTerm::integer(signed_source, IntegerValue::Signed(132)).expect("132i16"),
                ),
            ])
        );

        let cross_source = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let cross_target = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let cross_root_id = ValueId::new(181).expect("cross root");
        let cross_root = ScalarTerm::value(cross_root_id, ScalarType::Integer(cross_source));
        let cross_cast = ScalarTerm::value(
            ValueId::new(182).expect("cross cast"),
            ScalarType::Integer(cross_target),
        );
        let cross_definition = Proposition::Equal(
            cross_cast.clone(),
            ScalarTerm::integer_exact_cast(cross_source, cross_target, cross_root.clone())
                .expect("i8 to u8 exact cast"),
        );
        let cross_parameters = BTreeSet::from([cross_root_id]);
        assert_eq!(
            exact_integer_cast_obligation(
                cross_source,
                cross_target,
                cross_root.clone(),
                &[],
                &cross_parameters,
            ),
            Proposition::LessOrEqual(
                ScalarTerm::integer(cross_source, IntegerValue::Signed(0)).expect("0i8"),
                cross_root.clone(),
            )
        );
        assert_eq!(
            exact_integer_add_obligation(
                cross_target,
                cross_cast,
                ScalarTerm::integer(cross_target, IntegerValue::Unsigned(1)).expect("1u8"),
                std::slice::from_ref(&cross_definition),
                1,
                &cross_parameters,
            ),
            Proposition::LessOrEqual(
                ScalarTerm::integer(cross_source, IntegerValue::Signed(-1)).expect("-1i8"),
                cross_root,
            )
        );
    }

    #[test]
    fn reconstructs_a_finite_ordered_widening_chain_and_rejects_broken_chains() {
        let narrow_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let middle_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
        let wide_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
        let deep_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
        let input_id = ValueId::new(1).expect("input");
        let input = ScalarTerm::value(input_id, ScalarType::Integer(narrow_type));
        let middle = ScalarTerm::value(
            ValueId::new(2).expect("middle"),
            ScalarType::Integer(middle_type),
        );
        let widened = ScalarTerm::value(
            ValueId::new(3).expect("widened"),
            ScalarType::Integer(wide_type),
        );
        let deeply_widened = ScalarTerm::value(
            ValueId::new(4).expect("deeply widened"),
            ScalarType::Integer(deep_type),
        );
        let middle_definition = Proposition::Equal(
            middle.clone(),
            ScalarTerm::integer_widen(narrow_type, middle_type, input).expect("u8 to u16 widening"),
        );
        let wide_definition = Proposition::Equal(
            widened.clone(),
            ScalarTerm::integer_widen(middle_type, wide_type, middle.clone())
                .expect("u16 to u32 widening"),
        );
        let deep_definition = Proposition::Equal(
            deeply_widened.clone(),
            ScalarTerm::integer_widen(wide_type, deep_type, widened.clone())
                .expect("u32 to u64 widening"),
        );
        let machine_parameter_values = BTreeSet::from([input_id]);
        assert_eq!(
            exact_integer_cast_obligation(
                deep_type,
                narrow_type,
                deeply_widened.clone(),
                &[
                    middle_definition.clone(),
                    wide_definition.clone(),
                    deep_definition.clone(),
                ],
                &machine_parameter_values,
            ),
            Proposition::Truth
        );
        // A symmetric equality fact is not the verifier-owned operation
        // definition orientation.
        let reversed_deep_definition = match deep_definition.clone() {
            Proposition::Equal(left, right) => Proposition::Equal(right, left),
            _ => unreachable!("widen definition is an equality"),
        };
        assert_ne!(
            exact_integer_cast_obligation(
                deep_type,
                narrow_type,
                deeply_widened.clone(),
                &[
                    middle_definition.clone(),
                    wide_definition.clone(),
                    reversed_deep_definition,
                ],
                &machine_parameter_values,
            ),
            Proposition::Truth
        );
        // Redirecting one result definition to a non-widening value breaks the
        // chain even though its surrounding carrier remains unchanged.
        let redirected_wide_definition = Proposition::Equal(
            widened.clone(),
            ScalarTerm::integer(wide_type, IntegerValue::Unsigned(0)).expect("0u32"),
        );
        assert_ne!(
            exact_integer_cast_obligation(
                deep_type,
                narrow_type,
                deeply_widened.clone(),
                &[
                    middle_definition.clone(),
                    redirected_wide_definition,
                    deep_definition.clone(),
                ],
                &machine_parameter_values,
            ),
            Proposition::Truth
        );
        assert_ne!(
            exact_integer_cast_obligation(
                deep_type,
                narrow_type,
                deeply_widened.clone(),
                &[middle_definition.clone(), deep_definition.clone()],
                &machine_parameter_values,
            ),
            Proposition::Truth
        );
        assert_ne!(
            exact_integer_cast_obligation(
                deep_type,
                narrow_type,
                deeply_widened.clone(),
                &[
                    deep_definition.clone(),
                    middle_definition.clone(),
                    wide_definition.clone(),
                ],
                &machine_parameter_values,
            ),
            Proposition::Truth
        );
        // A cycle cannot manufacture an origin: operation order decreases at
        // each step, and the malformed back-edge is also type-inconsistent.
        let cyclic_middle_definition = Proposition::Equal(
            middle,
            ScalarTerm::IntegerWiden {
                source_type: narrow_type,
                target_type: middle_type,
                operand: Box::new(deeply_widened.clone()),
            },
        );
        assert_ne!(
            exact_integer_cast_obligation(
                deep_type,
                narrow_type,
                deeply_widened.clone(),
                &[
                    cyclic_middle_definition,
                    wide_definition.clone(),
                    deep_definition.clone(),
                ],
                &machine_parameter_values,
            ),
            Proposition::Truth
        );
        assert_ne!(
            exact_integer_cast_obligation(
                deep_type,
                narrow_type,
                deeply_widened,
                &[middle_definition, wide_definition, deep_definition],
                &BTreeSet::new(),
            ),
            Proposition::Truth
        );
    }

    #[test]
    fn reconstructs_unsigned_joint_exact_add_bounds() {
        let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let left = ScalarTerm::value(
            ValueId::new(1).expect("left"),
            ScalarType::Integer(integer_type),
        );
        let right = ScalarTerm::value(
            ValueId::new(2).expect("right"),
            ScalarType::Integer(integer_type),
        );
        let maximum =
            ScalarTerm::integer(integer_type, IntegerValue::Unsigned(255)).expect("255u8");
        let remainder = ScalarTerm::exact_integer_subtract(integer_type, maximum, right.clone())
            .expect("255 - right");
        let bound = Proposition::LessOrEqual(left.clone(), remainder);
        assert_eq!(
            exact_integer_add_obligation(
                integer_type,
                left.clone(),
                right.clone(),
                std::slice::from_ref(&bound),
                0,
                &BTreeSet::new(),
            ),
            bound.clone()
        );
    }

    #[test]
    fn reconstructs_one_nested_exact_add_from_the_inner_result_definition() {
        let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let input = ScalarTerm::value(
            ValueId::new(1).expect("input"),
            ScalarType::Integer(integer_type),
        );
        let inner_result = ScalarTerm::value(
            ValueId::new(2).expect("inner result"),
            ScalarType::Integer(integer_type),
        );
        let one = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(1)).expect("1u8");
        let inner_definition = Proposition::Equal(
            inner_result.clone(),
            ScalarTerm::exact_integer_add(integer_type, input.clone(), one.clone())
                .expect("u8 exact add term"),
        );
        let machine_parameters = BTreeSet::from([ValueId::new(1).expect("input")]);
        assert_eq!(
            exact_integer_add_obligation(
                integer_type,
                inner_result,
                one,
                std::slice::from_ref(&inner_definition),
                1,
                &machine_parameters,
            ),
            Proposition::LessOrEqual(
                input,
                ScalarTerm::integer(integer_type, IntegerValue::Unsigned(253)).expect("253u8"),
            )
        );
    }

    #[test]
    fn reconstructs_a_finite_ordered_exact_add_chain_and_rejects_broken_definitions() {
        let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let input_id = ValueId::new(1).expect("input");
        let input = ScalarTerm::value(input_id, ScalarType::Integer(integer_type));
        let first = ScalarTerm::value(
            ValueId::new(2).expect("first"),
            ScalarType::Integer(integer_type),
        );
        let second = ScalarTerm::value(
            ValueId::new(3).expect("second"),
            ScalarType::Integer(integer_type),
        );
        let one = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(1)).expect("1u8");
        let first_definition = Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_add(integer_type, input.clone(), one.clone())
                .expect("first exact add"),
        );
        let second_definition = Proposition::Equal(
            second.clone(),
            ScalarTerm::exact_integer_add(integer_type, first.clone(), one.clone())
                .expect("second exact add"),
        );
        let parameters = BTreeSet::from([input_id]);
        let expected = Proposition::LessOrEqual(
            input,
            ScalarTerm::integer(integer_type, IntegerValue::Unsigned(252)).expect("252u8"),
        );
        let reconstruct = |axioms: &[Proposition], parameters: &BTreeSet<ValueId>| {
            exact_integer_add_obligation(
                integer_type,
                second.clone(),
                one.clone(),
                axioms,
                axioms.len(),
                parameters,
            )
        };
        assert_eq!(
            reconstruct(
                &[first_definition.clone(), second_definition.clone()],
                &parameters
            ),
            expected
        );
        assert_ne!(
            reconstruct(std::slice::from_ref(&second_definition), &parameters),
            expected
        );
        assert_ne!(
            reconstruct(
                &[second_definition.clone(), first_definition.clone()],
                &parameters
            ),
            expected
        );
        let reversed_second = match second_definition.clone() {
            Proposition::Equal(left, right) => Proposition::Equal(right, left),
            _ => unreachable!("exact-add definition is an equality"),
        };
        assert_ne!(
            reconstruct(&[first_definition.clone(), reversed_second], &parameters),
            expected
        );
        let redirected_second = Proposition::Equal(
            second.clone(),
            ScalarTerm::integer(integer_type, IntegerValue::Unsigned(2)).expect("2u8"),
        );
        assert_ne!(
            reconstruct(&[first_definition.clone(), redirected_second], &parameters),
            expected
        );
        let cyclic_first = Proposition::Equal(
            first,
            ScalarTerm::exact_integer_add(integer_type, second.clone(), one.clone())
                .expect("cyclic exact add"),
        );
        assert_ne!(
            reconstruct(&[cyclic_first, second_definition.clone()], &parameters),
            expected
        );
        assert_ne!(
            reconstruct(&[first_definition, second_definition], &BTreeSet::new(),),
            expected
        );
    }

    #[test]
    fn reconstructs_wide_signed_offsets_cancellation_and_magnitude_overflow() {
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let input_id = ValueId::new(1).expect("input");
        let input = ScalarTerm::value(input_id, ScalarType::Integer(i8_type));
        let first = ScalarTerm::value(
            ValueId::new(2).expect("first"),
            ScalarType::Integer(i8_type),
        );
        let positive = ScalarTerm::integer(i8_type, IntegerValue::Signed(127)).expect("127i8");
        let negative = ScalarTerm::integer(i8_type, IntegerValue::Signed(-127)).expect("-127i8");
        let first_definition = Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_add(i8_type, input.clone(), positive.clone())
                .expect("first signed exact add"),
        );
        let parameters = BTreeSet::from([input_id]);
        assert_eq!(
            exact_integer_add_obligation(
                i8_type,
                first.clone(),
                positive,
                std::slice::from_ref(&first_definition),
                1,
                &parameters,
            ),
            Proposition::LessOrEqual(
                input,
                ScalarTerm::integer(i8_type, IntegerValue::Signed(-127)).expect("-127i8"),
            )
        );
        assert_eq!(
            exact_integer_add_obligation(
                i8_type,
                first,
                negative,
                std::slice::from_ref(&first_definition),
                1,
                &parameters,
            ),
            Proposition::Truth
        );

        let i128_type = IntegerType::new(IntegerSign::Signed, 128).expect("i128");
        let wide_input_id = ValueId::new(11).expect("wide input");
        let wide_input = ScalarTerm::value(wide_input_id, ScalarType::Integer(i128_type));
        let wide_first = ScalarTerm::value(
            ValueId::new(12).expect("wide first"),
            ScalarType::Integer(i128_type),
        );
        let wide_second = ScalarTerm::value(
            ValueId::new(13).expect("wide second"),
            ScalarType::Integer(i128_type),
        );
        let maximum =
            ScalarTerm::integer(i128_type, IntegerValue::Signed(i128::MAX)).expect("i128 maximum");
        let wide_first_definition = Proposition::Equal(
            wide_first.clone(),
            ScalarTerm::exact_integer_add(i128_type, wide_input, maximum.clone())
                .expect("wide first exact add"),
        );
        let wide_second_definition = Proposition::Equal(
            wide_second.clone(),
            ScalarTerm::exact_integer_add(i128_type, wide_first, maximum.clone())
                .expect("wide second exact add"),
        );
        assert_eq!(
            exact_integer_add_obligation(
                i128_type,
                wide_second,
                maximum,
                &[wide_first_definition, wide_second_definition],
                2,
                &BTreeSet::from([wide_input_id]),
            ),
            Proposition::Falsehood
        );
    }

    #[test]
    fn reconstructs_a_finite_ordered_exact_subtract_chain_and_rejects_broken_definitions() {
        let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let input_id = ValueId::new(21).expect("input");
        let input = ScalarTerm::value(input_id, ScalarType::Integer(integer_type));
        let first = ScalarTerm::value(
            ValueId::new(22).expect("first"),
            ScalarType::Integer(integer_type),
        );
        let second = ScalarTerm::value(
            ValueId::new(23).expect("second"),
            ScalarType::Integer(integer_type),
        );
        let one = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(1)).expect("1u8");
        let first_definition = Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_subtract(integer_type, input.clone(), one.clone())
                .expect("first exact subtract"),
        );
        let second_definition = Proposition::Equal(
            second.clone(),
            ScalarTerm::exact_integer_subtract(integer_type, first.clone(), one.clone())
                .expect("second exact subtract"),
        );
        let parameters = BTreeSet::from([input_id]);
        let expected = Proposition::LessOrEqual(
            ScalarTerm::integer(integer_type, IntegerValue::Unsigned(3)).expect("3u8"),
            input.clone(),
        );
        let reconstruct = |axioms: &[Proposition], parameters: &BTreeSet<ValueId>| {
            exact_integer_subtract_obligation(
                integer_type,
                second.clone(),
                one.clone(),
                axioms,
                axioms.len(),
                parameters,
            )
        };
        assert_eq!(
            reconstruct(
                &[first_definition.clone(), second_definition.clone()],
                &parameters,
            ),
            expected
        );
        assert_ne!(
            reconstruct(std::slice::from_ref(&second_definition), &parameters),
            expected
        );
        assert_ne!(
            reconstruct(
                &[second_definition.clone(), first_definition.clone()],
                &parameters,
            ),
            expected
        );
        let reversed_second = match second_definition.clone() {
            Proposition::Equal(left, right) => Proposition::Equal(right, left),
            _ => unreachable!("exact-subtract definition is an equality"),
        };
        assert_ne!(
            reconstruct(&[first_definition.clone(), reversed_second], &parameters),
            expected
        );
        let redirected_second = Proposition::Equal(
            second.clone(),
            ScalarTerm::integer(integer_type, IntegerValue::Unsigned(2)).expect("2u8"),
        );
        assert_ne!(
            reconstruct(&[first_definition.clone(), redirected_second], &parameters),
            expected
        );
        let reversed_operand_definition = Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_subtract(integer_type, one.clone(), input)
                .expect("reversed exact subtract"),
        );
        assert_ne!(
            reconstruct(
                &[reversed_operand_definition, second_definition.clone()],
                &parameters,
            ),
            expected
        );
        let cyclic_first = Proposition::Equal(
            first,
            ScalarTerm::exact_integer_subtract(integer_type, second.clone(), one.clone())
                .expect("cyclic exact subtract"),
        );
        assert_ne!(
            reconstruct(&[cyclic_first, second_definition.clone()], &parameters),
            expected
        );
        assert_ne!(
            reconstruct(&[first_definition, second_definition], &BTreeSet::new()),
            expected
        );
    }

    #[test]
    fn reconstructs_wide_signed_subtract_offsets_cancellation_and_magnitude_overflow() {
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let input_id = ValueId::new(31).expect("input");
        let input = ScalarTerm::value(input_id, ScalarType::Integer(i8_type));
        let first = ScalarTerm::value(
            ValueId::new(32).expect("first"),
            ScalarType::Integer(i8_type),
        );
        let positive = ScalarTerm::integer(i8_type, IntegerValue::Signed(127)).expect("127i8");
        let negative = ScalarTerm::integer(i8_type, IntegerValue::Signed(-127)).expect("-127i8");
        let first_definition = Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_subtract(i8_type, input.clone(), positive.clone())
                .expect("first signed exact subtract"),
        );
        let parameters = BTreeSet::from([input_id]);
        assert_eq!(
            exact_integer_subtract_obligation(
                i8_type,
                first.clone(),
                positive,
                std::slice::from_ref(&first_definition),
                1,
                &parameters,
            ),
            Proposition::LessOrEqual(
                ScalarTerm::integer(i8_type, IntegerValue::Signed(126)).expect("126i8"),
                input,
            )
        );
        assert_eq!(
            exact_integer_subtract_obligation(
                i8_type,
                first,
                negative,
                std::slice::from_ref(&first_definition),
                1,
                &parameters,
            ),
            Proposition::Truth
        );

        let i128_type = IntegerType::new(IntegerSign::Signed, 128).expect("i128");
        let wide_input_id = ValueId::new(41).expect("wide input");
        let wide_input = ScalarTerm::value(wide_input_id, ScalarType::Integer(i128_type));
        let wide_first = ScalarTerm::value(
            ValueId::new(42).expect("wide first"),
            ScalarType::Integer(i128_type),
        );
        let wide_second = ScalarTerm::value(
            ValueId::new(43).expect("wide second"),
            ScalarType::Integer(i128_type),
        );
        let maximum =
            ScalarTerm::integer(i128_type, IntegerValue::Signed(i128::MAX)).expect("i128 maximum");
        let wide_first_definition = Proposition::Equal(
            wide_first.clone(),
            ScalarTerm::exact_integer_subtract(i128_type, wide_input, maximum.clone())
                .expect("wide first exact subtract"),
        );
        let wide_second_definition = Proposition::Equal(
            wide_second.clone(),
            ScalarTerm::exact_integer_subtract(i128_type, wide_first, maximum.clone())
                .expect("wide second exact subtract"),
        );
        assert_eq!(
            exact_integer_subtract_obligation(
                i128_type,
                wide_second,
                maximum,
                &[wide_first_definition, wide_second_definition],
                2,
                &BTreeSet::from([wide_input_id]),
            ),
            Proposition::Falsehood
        );
    }

    #[test]
    fn reconstructs_mixed_exact_add_subtract_offsets_and_rejects_broken_chains() {
        let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let input_id = ValueId::new(51).expect("input");
        let input = ScalarTerm::value(input_id, ScalarType::Integer(integer_type));
        let first = ScalarTerm::value(
            ValueId::new(52).expect("first"),
            ScalarType::Integer(integer_type),
        );
        let second = ScalarTerm::value(
            ValueId::new(53).expect("second"),
            ScalarType::Integer(integer_type),
        );
        let five = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(5)).expect("5u8");
        let three = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(3)).expect("3u8");
        let two = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(2)).expect("2u8");
        let first_definition = Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_add(integer_type, input.clone(), five).expect("input + 5u8"),
        );
        let second_definition = Proposition::Equal(
            second.clone(),
            ScalarTerm::exact_integer_subtract(integer_type, first.clone(), three)
                .expect("first - 3u8"),
        );
        let parameters = BTreeSet::from([input_id]);
        assert_eq!(
            exact_integer_subtract_obligation(
                integer_type,
                first.clone(),
                ScalarTerm::integer(integer_type, IntegerValue::Unsigned(3)).expect("3u8"),
                std::slice::from_ref(&first_definition),
                1,
                &parameters,
            ),
            Proposition::LessOrEqual(
                input.clone(),
                ScalarTerm::integer(integer_type, IntegerValue::Unsigned(253)).expect("253u8"),
            ),
            "the mixed second prefix is reconstructed from the direct root"
        );
        assert_eq!(
            exact_integer_subtract_obligation(
                integer_type,
                first.clone(),
                ScalarTerm::integer(integer_type, IntegerValue::Unsigned(5)).expect("5u8"),
                std::slice::from_ref(&first_definition),
                1,
                &parameters,
            ),
            Proposition::Truth,
            "cancellation is total only after the earlier prefix keeps its own proof"
        );
        let expected = Proposition::LessOrEqual(
            input.clone(),
            ScalarTerm::integer(integer_type, IntegerValue::Unsigned(251)).expect("251u8"),
        );
        let reconstruct = |axioms: &[Proposition], parameters: &BTreeSet<ValueId>| {
            exact_integer_add_obligation(
                integer_type,
                second.clone(),
                two.clone(),
                axioms,
                axioms.len(),
                parameters,
            )
        };
        assert_eq!(
            reconstruct(
                &[first_definition.clone(), second_definition.clone()],
                &parameters,
            ),
            expected
        );
        assert_ne!(
            reconstruct(
                &[second_definition.clone(), first_definition.clone()],
                &parameters,
            ),
            expected
        );
        let reversed_second = match second_definition.clone() {
            Proposition::Equal(left, right) => Proposition::Equal(right, left),
            _ => unreachable!("mixed definition is an equality"),
        };
        assert_ne!(
            reconstruct(&[first_definition.clone(), reversed_second], &parameters),
            expected
        );
        let redirected_second = Proposition::Equal(
            second.clone(),
            ScalarTerm::integer(integer_type, IntegerValue::Unsigned(2)).expect("2u8"),
        );
        assert_ne!(
            reconstruct(&[first_definition.clone(), redirected_second], &parameters),
            expected
        );
        let right_associated_first = Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_add(
                integer_type,
                ScalarTerm::integer(integer_type, IntegerValue::Unsigned(5)).expect("5u8"),
                input.clone(),
            )
            .expect("5u8 + input"),
        );
        assert_ne!(
            reconstruct(
                &[right_associated_first, second_definition.clone()],
                &parameters,
            ),
            expected
        );
        let cyclic_first = Proposition::Equal(
            first,
            ScalarTerm::exact_integer_add(integer_type, second.clone(), two.clone())
                .expect("second + 2u8"),
        );
        assert_ne!(
            reconstruct(&[cyclic_first, second_definition.clone()], &parameters),
            expected
        );
        assert_ne!(
            reconstruct(&[first_definition, second_definition], &BTreeSet::new()),
            expected
        );

        let signed_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let signed_input_id = ValueId::new(61).expect("signed input");
        let signed_input = ScalarTerm::value(signed_input_id, ScalarType::Integer(signed_type));
        let signed_first = ScalarTerm::value(
            ValueId::new(62).expect("signed first"),
            ScalarType::Integer(signed_type),
        );
        let signed_definition = Proposition::Equal(
            signed_first.clone(),
            ScalarTerm::exact_integer_subtract(
                signed_type,
                signed_input.clone(),
                ScalarTerm::integer(signed_type, IntegerValue::Signed(-3)).expect("-3i8"),
            )
            .expect("signed input - -3i8"),
        );
        assert_eq!(
            exact_integer_add_obligation(
                signed_type,
                signed_first,
                ScalarTerm::integer(signed_type, IntegerValue::Signed(-5)).expect("-5i8"),
                std::slice::from_ref(&signed_definition),
                1,
                &BTreeSet::from([signed_input_id]),
            ),
            Proposition::LessOrEqual(
                ScalarTerm::integer(signed_type, IntegerValue::Signed(-126)).expect("-126i8"),
                signed_input,
            )
        );

        let overflow_input_id = ValueId::new(71).expect("overflow input");
        let overflow_input =
            ScalarTerm::value(overflow_input_id, ScalarType::Integer(integer_type));
        let overflow_first = ScalarTerm::value(
            ValueId::new(72).expect("overflow first"),
            ScalarType::Integer(integer_type),
        );
        let overflow_second = ScalarTerm::value(
            ValueId::new(73).expect("overflow second"),
            ScalarType::Integer(integer_type),
        );
        let subtract_zero = Proposition::Equal(
            overflow_first.clone(),
            ScalarTerm::exact_integer_subtract(
                integer_type,
                overflow_input,
                ScalarTerm::integer(integer_type, IntegerValue::Unsigned(0)).expect("0u8"),
            )
            .expect("input - 0u8"),
        );
        let add_maximum = Proposition::Equal(
            overflow_second.clone(),
            ScalarTerm::exact_integer_add(
                integer_type,
                overflow_first,
                ScalarTerm::integer(integer_type, IntegerValue::Unsigned(255)).expect("255u8"),
            )
            .expect("first + 255u8"),
        );
        assert_eq!(
            exact_integer_add_obligation(
                integer_type,
                overflow_second,
                ScalarTerm::integer(integer_type, IntegerValue::Unsigned(1)).expect("1u8"),
                &[subtract_zero, add_maximum],
                2,
                &BTreeSet::from([overflow_input_id]),
            ),
            Proposition::Falsehood
        );
    }

    #[test]
    fn reconstructs_signed_nonnegative_joint_exact_add_bounds() {
        let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let left = ScalarTerm::value(
            ValueId::new(4).expect("left"),
            ScalarType::Integer(integer_type),
        );
        let right = ScalarTerm::value(
            ValueId::new(5).expect("right"),
            ScalarType::Integer(integer_type),
        );
        let zero = ScalarTerm::integer(integer_type, IntegerValue::Signed(0)).expect("0i8");
        let maximum = ScalarTerm::integer(integer_type, IntegerValue::Signed(127)).expect("127i8");
        let remainder = ScalarTerm::exact_integer_subtract(integer_type, maximum, right.clone())
            .expect("127 - right");
        let nonnegative = Proposition::LessOrEqual(zero, right.clone());
        let bound = Proposition::LessOrEqual(left.clone(), remainder);
        let axioms = vec![nonnegative.clone(), bound.clone()];
        assert_eq!(
            exact_integer_add_obligation(
                integer_type,
                left,
                right.clone(),
                &axioms,
                0,
                &BTreeSet::new(),
            ),
            canonical_conjunction(vec![nonnegative.clone(), bound])
        );
        assert_eq!(
            exact_integer_subtract_obligation(
                integer_type,
                ScalarTerm::integer(integer_type, IntegerValue::Signed(127)).expect("127i8"),
                right,
                std::slice::from_ref(&nonnegative),
                0,
                &BTreeSet::new(),
            ),
            nonnegative
        );
    }

    #[test]
    fn reconstructs_signed_nonpositive_joint_exact_add_bounds() {
        let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let left = ScalarTerm::value(
            ValueId::new(6).expect("left"),
            ScalarType::Integer(integer_type),
        );
        let right = ScalarTerm::value(
            ValueId::new(7).expect("right"),
            ScalarType::Integer(integer_type),
        );
        let zero = ScalarTerm::integer(integer_type, IntegerValue::Signed(0)).expect("0i8");
        let minimum =
            ScalarTerm::integer(integer_type, IntegerValue::Signed(-128)).expect("-128i8");
        let remainder = ScalarTerm::exact_integer_subtract(integer_type, minimum, right.clone())
            .expect("-128 - right");
        let nonpositive = Proposition::LessOrEqual(right.clone(), zero);
        let bound = Proposition::LessOrEqual(remainder, left.clone());
        let axioms = vec![nonpositive.clone(), bound.clone()];
        assert_eq!(
            exact_integer_add_obligation(
                integer_type,
                left.clone(),
                right.clone(),
                &axioms,
                0,
                &BTreeSet::new(),
            ),
            canonical_conjunction(vec![nonpositive.clone(), bound])
        );
        assert_eq!(
            exact_integer_subtract_obligation(
                integer_type,
                ScalarTerm::integer(integer_type, IntegerValue::Signed(-128)).expect("-128i8"),
                right.clone(),
                std::slice::from_ref(&nonpositive),
                0,
                &BTreeSet::new(),
            ),
            nonpositive
        );
        assert_eq!(
            exact_integer_subtract_obligation(
                integer_type,
                ScalarTerm::integer(integer_type, IntegerValue::Signed(-128)).expect("-128i8"),
                right,
                &[axioms[1].clone()],
                0,
                &BTreeSet::new(),
            ),
            Proposition::Falsehood
        );
    }

    #[test]
    fn reconstructs_unsigned_joint_exact_subtract_bounds() {
        let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let left = ScalarTerm::value(
            ValueId::new(8).expect("left"),
            ScalarType::Integer(integer_type),
        );
        let right = ScalarTerm::value(
            ValueId::new(9).expect("right"),
            ScalarType::Integer(integer_type),
        );
        let bound = Proposition::LessOrEqual(right.clone(), left.clone());
        assert_eq!(
            exact_integer_subtract_obligation(
                integer_type,
                left.clone(),
                right.clone(),
                std::slice::from_ref(&bound),
                0,
                &BTreeSet::new(),
            ),
            bound.clone()
        );
        assert_eq!(
            exact_integer_subtract_obligation(integer_type, left, right, &[], 0, &BTreeSet::new(),),
            Proposition::Falsehood
        );
    }

    #[test]
    fn reconstructs_signed_nonnegative_joint_exact_subtract_bounds() {
        let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let left = ScalarTerm::value(
            ValueId::new(10).expect("left"),
            ScalarType::Integer(integer_type),
        );
        let right = ScalarTerm::value(
            ValueId::new(11).expect("right"),
            ScalarType::Integer(integer_type),
        );
        let zero = ScalarTerm::integer(integer_type, IntegerValue::Signed(0)).expect("0i8");
        let minimum =
            ScalarTerm::integer(integer_type, IntegerValue::Signed(-128)).expect("-128i8");
        let lower = ScalarTerm::exact_integer_add(integer_type, minimum, right.clone())
            .expect("-128 + right");
        let nonnegative = Proposition::LessOrEqual(zero, right.clone());
        let bound = Proposition::LessOrEqual(lower, left.clone());
        let axioms = vec![nonnegative.clone(), bound.clone()];
        assert_eq!(
            exact_integer_subtract_obligation(
                integer_type,
                left.clone(),
                right.clone(),
                &axioms,
                0,
                &BTreeSet::new(),
            ),
            canonical_conjunction(vec![nonnegative.clone(), bound.clone()])
        );
        assert_eq!(
            exact_integer_subtract_obligation(
                integer_type,
                left,
                right,
                &axioms[1..],
                0,
                &BTreeSet::new(),
            ),
            Proposition::Falsehood
        );
    }

    #[test]
    fn reconstructs_signed_nonpositive_joint_exact_subtract_bounds() {
        let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let left = ScalarTerm::value(
            ValueId::new(12).expect("left"),
            ScalarType::Integer(integer_type),
        );
        let right = ScalarTerm::value(
            ValueId::new(13).expect("right"),
            ScalarType::Integer(integer_type),
        );
        let zero = ScalarTerm::integer(integer_type, IntegerValue::Signed(0)).expect("0i8");
        let maximum = ScalarTerm::integer(integer_type, IntegerValue::Signed(127)).expect("127i8");
        let upper = ScalarTerm::exact_integer_add(integer_type, maximum, right.clone())
            .expect("127 + right");
        let nonpositive = Proposition::LessOrEqual(right.clone(), zero);
        let bound = Proposition::LessOrEqual(left.clone(), upper);
        let axioms = vec![nonpositive.clone(), bound.clone()];
        assert_eq!(
            exact_integer_subtract_obligation(
                integer_type,
                left.clone(),
                right.clone(),
                &axioms,
                0,
                &BTreeSet::new(),
            ),
            canonical_conjunction(vec![nonpositive.clone(), bound.clone()])
        );
        assert_eq!(
            exact_integer_subtract_obligation(
                integer_type,
                left,
                right,
                &axioms[1..],
                0,
                &BTreeSet::new(),
            ),
            Proposition::Falsehood
        );
    }

    #[test]
    fn exact_subtract_reconstructs_carrier_tight_known_right_bounds() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let unsigned_left = ScalarTerm::value(
            ValueId::new(1).expect("value"),
            ScalarType::Integer(u8_type),
        );
        let unsigned_five = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(5)).expect("5u8");
        assert_eq!(
            exact_integer_subtract_obligation(
                u8_type,
                unsigned_left.clone(),
                unsigned_five.clone(),
                &[],
                0,
                &BTreeSet::new(),
            ),
            Proposition::LessOrEqual(unsigned_five, unsigned_left)
        );

        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let signed_left = ScalarTerm::value(
            ValueId::new(2).expect("value"),
            ScalarType::Integer(i8_type),
        );
        let positive = ScalarTerm::integer(i8_type, IntegerValue::Signed(8)).expect("8i8");
        let lower = ScalarTerm::integer(i8_type, IntegerValue::Signed(-120)).expect("-120i8");
        assert_eq!(
            exact_integer_subtract_obligation(
                i8_type,
                signed_left.clone(),
                positive,
                &[],
                0,
                &BTreeSet::new(),
            ),
            Proposition::LessOrEqual(lower, signed_left.clone())
        );

        let negative = ScalarTerm::integer(i8_type, IntegerValue::Signed(-7)).expect("-7i8");
        let upper = ScalarTerm::integer(i8_type, IntegerValue::Signed(120)).expect("120i8");
        assert_eq!(
            exact_integer_subtract_obligation(
                i8_type,
                signed_left.clone(),
                negative,
                &[],
                0,
                &BTreeSet::new(),
            ),
            Proposition::LessOrEqual(signed_left, upper)
        );
    }

    #[test]
    fn exact_subtract_fails_closed_without_a_known_right_operand() {
        let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let value = |id| {
            ScalarTerm::value(
                ValueId::new(id).expect("value"),
                ScalarType::Integer(integer_type),
            )
        };
        assert_eq!(
            exact_integer_subtract_obligation(
                integer_type,
                value(1),
                value(2),
                &[],
                0,
                &BTreeSet::new(),
            ),
            Proposition::Falsehood
        );

        let four = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(4)).expect("4u8");
        let five = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(5)).expect("5u8");
        assert_eq!(
            exact_integer_subtract_obligation(integer_type, four, five, &[], 0, &BTreeSet::new(),),
            Proposition::Falsehood
        );
    }

    #[test]
    fn reconstructs_known_unsigned_minuend_bounds() {
        let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let right = ScalarTerm::value(
            ValueId::new(3).expect("right"),
            ScalarType::Integer(integer_type),
        );
        let maximum =
            ScalarTerm::integer(integer_type, IntegerValue::Unsigned(255)).expect("255u8");
        assert_eq!(
            exact_integer_subtract_obligation(
                integer_type,
                maximum.clone(),
                right.clone(),
                &[],
                0,
                &BTreeSet::new(),
            ),
            Proposition::Truth
        );
    }

    #[test]
    fn exact_multiply_reconstructs_carrier_tight_known_factor_bounds() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let unsigned_value = ScalarTerm::value(
            ValueId::new(3).expect("value"),
            ScalarType::Integer(u8_type),
        );
        let unsigned_five = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(5)).expect("5u8");
        let unsigned_one = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(1)).expect("1u8");
        let unsigned_maximum =
            ScalarTerm::integer(u8_type, IntegerValue::Unsigned(51)).expect("51u8");
        assert_eq!(
            exact_integer_multiply_obligation(u8_type, unsigned_value.clone(), unsigned_one, &[],),
            Proposition::Truth
        );
        assert_eq!(
            exact_integer_multiply_obligation(
                u8_type,
                unsigned_value.clone(),
                unsigned_five.clone(),
                &[],
            ),
            Proposition::LessOrEqual(unsigned_value.clone(), unsigned_maximum.clone())
        );
        assert_eq!(
            exact_integer_multiply_obligation(u8_type, unsigned_five, unsigned_value.clone(), &[],),
            Proposition::LessOrEqual(unsigned_value, unsigned_maximum)
        );

        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let signed_value = ScalarTerm::value(
            ValueId::new(4).expect("value"),
            ScalarType::Integer(i8_type),
        );
        let signed_three = ScalarTerm::integer(i8_type, IntegerValue::Signed(3)).expect("3i8");
        let negative_three = ScalarTerm::integer(i8_type, IntegerValue::Signed(-3)).expect("-3i8");
        let negative_42 = ScalarTerm::integer(i8_type, IntegerValue::Signed(-42)).expect("-42i8");
        let positive_42 = ScalarTerm::integer(i8_type, IntegerValue::Signed(42)).expect("42i8");
        let expected = Proposition::Conjunction(vec![
            Proposition::LessOrEqual(negative_42.clone(), signed_value.clone()),
            Proposition::LessOrEqual(signed_value.clone(), positive_42.clone()),
        ]);
        assert_eq!(
            exact_integer_multiply_obligation(i8_type, signed_value.clone(), signed_three, &[],),
            expected.clone()
        );
        assert_eq!(
            exact_integer_multiply_obligation(i8_type, signed_value, negative_three, &[],),
            expected
        );

        let negative_one = ScalarTerm::integer(i8_type, IntegerValue::Signed(-1)).expect("-1i8");
        let minimum_plus_one =
            ScalarTerm::integer(i8_type, IntegerValue::Signed(-127)).expect("-127i8");
        let signed_value = ScalarTerm::value(
            ValueId::new(4).expect("value"),
            ScalarType::Integer(i8_type),
        );
        assert_eq!(
            exact_integer_multiply_obligation(i8_type, signed_value.clone(), negative_one, &[],),
            Proposition::LessOrEqual(minimum_plus_one, signed_value)
        );
    }

    #[test]
    fn exact_multiply_chain_reconstructs_cumulative_parameter_bounds() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let u8_value = |id| {
            ScalarTerm::value(
                ValueId::new(id).expect("value"),
                ScalarType::Integer(u8_type),
            )
        };
        let root = u8_value(1);
        let first = u8_value(2);
        let second = u8_value(3);
        let two = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(2)).expect("2u8");
        let three = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(3)).expect("3u8");
        let first_definition = Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_multiply(u8_type, root.clone(), two.clone())
                .expect("root * 2"),
        );
        let second_definition = Proposition::Equal(
            second.clone(),
            ScalarTerm::exact_integer_multiply(u8_type, first, three).expect("first * 3"),
        );
        let axioms = vec![first_definition, second_definition];
        let parameters = BTreeSet::from([ValueId::new(1).expect("root")]);
        let twenty_one = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(21)).expect("21u8");
        assert_eq!(
            exact_integer_multiply_obligation_with_definitions(
                u8_type,
                second,
                two,
                &axioms,
                axioms.len(),
                &parameters,
            ),
            Proposition::LessOrEqual(root, twenty_one)
        );

        let reversed_factor = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(3)).expect("3u8");
        let direct_boundary =
            ScalarTerm::integer(u8_type, IntegerValue::Unsigned(85)).expect("85u8");
        let reversed_left = axioms
            .last()
            .and_then(|axiom| match axiom {
                Proposition::Equal(left, _) => Some(left.clone()),
                _ => None,
            })
            .expect("second result");
        assert_eq!(
            exact_integer_multiply_obligation_with_definitions(
                u8_type,
                reversed_factor,
                reversed_left.clone(),
                &axioms,
                axioms.len(),
                &parameters,
            ),
            Proposition::LessOrEqual(reversed_left, direct_boundary),
            "a reversed outer factor does not gain chain-definition authority"
        );

        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let signed_root = ScalarTerm::value(
            ValueId::new(4).expect("signed root"),
            ScalarType::Integer(i8_type),
        );
        let signed_first = ScalarTerm::value(
            ValueId::new(5).expect("signed first"),
            ScalarType::Integer(i8_type),
        );
        let signed_two = ScalarTerm::integer(i8_type, IntegerValue::Signed(2)).expect("2i8");
        let signed_three = ScalarTerm::integer(i8_type, IntegerValue::Signed(3)).expect("3i8");
        let signed_axioms = vec![Proposition::Equal(
            signed_first.clone(),
            ScalarTerm::exact_integer_multiply(i8_type, signed_root.clone(), signed_two)
                .expect("signed root * 2"),
        )];
        let signed_parameters = BTreeSet::from([ValueId::new(4).expect("signed root")]);
        let negative_twenty_one =
            ScalarTerm::integer(i8_type, IntegerValue::Signed(-21)).expect("-21i8");
        let positive_twenty_one =
            ScalarTerm::integer(i8_type, IntegerValue::Signed(21)).expect("21i8");
        assert_eq!(
            exact_integer_multiply_obligation_with_definitions(
                i8_type,
                signed_first,
                signed_three,
                &signed_axioms,
                signed_axioms.len(),
                &signed_parameters,
            ),
            canonical_conjunction(vec![
                Proposition::LessOrEqual(negative_twenty_one, signed_root.clone()),
                Proposition::LessOrEqual(signed_root, positive_twenty_one),
            ])
        );
    }

    #[test]
    fn signed_multiply_chains_reverse_preimages_and_preserve_zero_and_minimum() {
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 type");
        let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16 type");
        let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 type");
        let root_id = ValueId::new(1601).expect("signed-product root");
        let root = ScalarTerm::value(root_id, ScalarType::Integer(i8_type));
        let inner = ScalarTerm::value(
            ValueId::new(1602).expect("signed-product inner"),
            ScalarType::Integer(i8_type),
        );
        let definitions = vec![Proposition::Equal(
            inner.clone(),
            ScalarTerm::exact_integer_multiply(
                i8_type,
                root.clone(),
                ScalarTerm::integer(i8_type, IntegerValue::Signed(-2)).expect("-2i8"),
            )
            .expect("root * -2"),
        )];
        let expected = canonical_conjunction(vec![
            Proposition::LessOrEqual(
                ScalarTerm::integer(i8_type, IntegerValue::Signed(-21)).expect("-21i8"),
                root.clone(),
            ),
            Proposition::LessOrEqual(
                root.clone(),
                ScalarTerm::integer(i8_type, IntegerValue::Signed(21)).expect("21i8"),
            ),
        ]);
        assert_eq!(
            exact_integer_signed_multiply_chain_obligation(
                i8_type,
                inner.clone(),
                IntegerValue::Signed(3),
                &definitions,
                definitions.len(),
                &BTreeSet::from([root_id]),
            ),
            Some(expected.clone()),
            "a negative cumulative product reverses the carrier preimage",
        );
        assert_eq!(
            exact_integer_signed_multiply_chain_obligation(
                i8_type,
                inner.clone(),
                IntegerValue::Signed(-3),
                &definitions,
                definitions.len(),
                &BTreeSet::from([root_id]),
            ),
            Some(expected),
            "two negative factors restore the positive carrier preimage",
        );
        assert_eq!(
            exact_integer_signed_multiply_chain_obligation(
                i8_type,
                inner,
                IntegerValue::Signed(0),
                &definitions,
                definitions.len(),
                &BTreeSet::from([root_id]),
            ),
            Some(Proposition::Truth),
            "zero decides only the current prefix",
        );

        let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64 type");
        let minimum_root_id = ValueId::new(1607).expect("minimum-factor root");
        let minimum_root = ScalarTerm::value(minimum_root_id, ScalarType::Integer(i64_type));
        let minimum_product = ScalarTerm::value(
            ValueId::new(1608).expect("minimum-factor product"),
            ScalarType::Integer(i64_type),
        );
        let minimum_definitions = vec![Proposition::Equal(
            minimum_product.clone(),
            ScalarTerm::exact_integer_multiply(
                i64_type,
                minimum_root.clone(),
                ScalarTerm::integer(i64_type, IntegerValue::Signed(i64::MIN.into()))
                    .expect("MIN i64"),
            )
            .expect("root * MIN"),
        )];
        assert_eq!(
            exact_integer_signed_multiply_chain_obligation(
                i64_type,
                minimum_product,
                IntegerValue::Signed(1),
                &minimum_definitions,
                minimum_definitions.len(),
                &BTreeSet::from([minimum_root_id]),
            ),
            Some(exact_integer_source_interval_obligation(
                i64_type,
                minimum_root,
                0,
                1,
            )),
            "signed MIN is accumulated by magnitude without host negation",
        );

        let wide_root_id = ValueId::new(1603).expect("wide root");
        let wide_root = ScalarTerm::value(wide_root_id, ScalarType::Integer(i16_type));
        let wide_product = ScalarTerm::value(
            ValueId::new(1604).expect("wide product"),
            ScalarType::Integer(i16_type),
        );
        let wide_definitions = vec![Proposition::Equal(
            wide_product.clone(),
            ScalarTerm::exact_integer_multiply(
                i16_type,
                wide_root.clone(),
                ScalarTerm::integer(i16_type, IntegerValue::Signed(-512)).expect("-512i16"),
            )
            .expect("wide root * -512"),
        )];
        assert_eq!(
            exact_integer_signed_multiply_chain_cast_obligation(
                i16_type,
                i8_type,
                wide_product,
                &wide_definitions,
                &BTreeSet::from([wide_root_id]),
            ),
            Some(exact_integer_source_interval_obligation(
                i16_type, wide_root, 0, 0,
            )),
            "the negative pre-cast product reverses the target interval",
        );

        let unsigned_root_id = ValueId::new(1605).expect("unsigned root");
        let unsigned_root = ScalarTerm::value(unsigned_root_id, ScalarType::Integer(u16_type));
        let cast = ScalarTerm::value(
            ValueId::new(1606).expect("signed cast"),
            ScalarType::Integer(i8_type),
        );
        let cast_definitions = vec![Proposition::Equal(
            cast.clone(),
            ScalarTerm::integer_exact_cast(u16_type, i8_type, unsigned_root.clone())
                .expect("u16 to i8 cast"),
        )];
        assert_eq!(
            exact_integer_cast_then_signed_multiply_chain_obligation(
                i8_type,
                cast,
                IntegerValue::Signed(-2),
                &cast_definitions,
                cast_definitions.len(),
                &BTreeSet::from([unsigned_root_id]),
            ),
            Some(exact_integer_source_interval_obligation(
                u16_type,
                unsigned_root,
                0,
                64,
            )),
            "post-cast negative multiplication intersects its reversed preimage with the source",
        );
    }

    #[test]
    fn signed_affine_three_placement_replays_negative_coefficients_without_importing_prefix_proofs()
    {
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 type");
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 type");
        let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16 type");
        let root_id = ValueId::new(1661).expect("signed-affine root");
        let root = ScalarTerm::value(root_id, ScalarType::Integer(i8_type));
        let offset = ScalarTerm::value(
            ValueId::new(1662).expect("signed-affine offset"),
            ScalarType::Integer(i8_type),
        );
        let negative = ScalarTerm::value(
            ValueId::new(1663).expect("signed-affine negative"),
            ScalarType::Integer(i8_type),
        );
        let definitions = vec![
            Proposition::Equal(
                offset.clone(),
                ScalarTerm::exact_integer_add(
                    i8_type,
                    root.clone(),
                    ScalarTerm::integer(i8_type, IntegerValue::Signed(3)).expect("3i8"),
                )
                .expect("root + 3"),
            ),
            Proposition::Equal(
                negative.clone(),
                ScalarTerm::exact_integer_multiply(
                    i8_type,
                    offset,
                    ScalarTerm::integer(i8_type, IntegerValue::Signed(-2)).expect("-2i8"),
                )
                .expect("offset * -2"),
            ),
        ];
        let parameters = BTreeSet::from([root_id]);
        assert_eq!(
            exact_integer_signed_affine_chain_obligation(
                i8_type,
                negative.clone(),
                IntegerValue::Signed(1),
                ExactIntegerAffineOperation::Subtract,
                &definitions,
                definitions.len(),
                &parameters,
            ),
            Some(exact_integer_source_interval_obligation(
                i8_type,
                root.clone(),
                -67,
                60,
            )),
            "a negative coefficient reverses the complete offset preimage",
        );
        assert_eq!(
            exact_integer_signed_affine_chain_cast_obligation(
                i8_type,
                u8_type,
                negative,
                &definitions,
                &parameters,
            ),
            Some(exact_integer_source_interval_obligation(
                i8_type,
                root.clone(),
                -128,
                -3,
            )),
            "the pre-cast obligation replays the signed source form independently",
        );

        let wide_root_id = ValueId::new(1664).expect("post-cast root");
        let wide_root = ScalarTerm::value(wide_root_id, ScalarType::Integer(i16_type));
        let cast = ScalarTerm::value(
            ValueId::new(1665).expect("post-cast value"),
            ScalarType::Integer(i8_type),
        );
        let cast_offset = ScalarTerm::value(
            ValueId::new(1666).expect("post-cast offset"),
            ScalarType::Integer(i8_type),
        );
        let post_definitions = vec![
            Proposition::Equal(
                cast.clone(),
                ScalarTerm::integer_exact_cast(i16_type, i8_type, wide_root.clone())
                    .expect("i16 to i8 cast"),
            ),
            Proposition::Equal(
                cast_offset.clone(),
                ScalarTerm::exact_integer_add(
                    i8_type,
                    cast,
                    ScalarTerm::integer(i8_type, IntegerValue::Signed(3)).expect("3i8"),
                )
                .expect("cast + 3"),
            ),
        ];
        assert_eq!(
            exact_integer_cast_then_signed_affine_chain_obligation(
                i8_type,
                cast_offset,
                IntegerValue::Signed(-2),
                ExactIntegerAffineOperation::Multiply,
                &post_definitions,
                post_definitions.len(),
                &BTreeSet::from([wide_root_id]),
            ),
            Some(exact_integer_source_interval_obligation(
                i16_type, wide_root, -66, 61,
            )),
            "the post-cast prefix intersects the reversed target preimage with the source carrier",
        );

        let zero = ScalarTerm::value(
            ValueId::new(1667).expect("zero result"),
            ScalarType::Integer(i8_type),
        );
        let zero_definitions = definitions
            .iter()
            .cloned()
            .chain([Proposition::Equal(
                zero.clone(),
                ScalarTerm::exact_integer_multiply(
                    i8_type,
                    ScalarTerm::value(
                        ValueId::new(1663).expect("signed-affine negative"),
                        ScalarType::Integer(i8_type),
                    ),
                    ScalarTerm::integer(i8_type, IntegerValue::Signed(0)).expect("0i8"),
                )
                .expect("negative * 0"),
            )])
            .collect::<Vec<_>>();
        assert_eq!(
            exact_integer_signed_affine_chain_obligation(
                i8_type,
                zero,
                IntegerValue::Signed(-128),
                ExactIntegerAffineOperation::Subtract,
                &zero_definitions,
                zero_definitions.len(),
                &parameters,
            ),
            Some(Proposition::Falsehood),
            "a constant outside the carrier is mathematical falsehood, not checked failure",
        );

        let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64 type");
        let minimum_root_id = ValueId::new(1668).expect("MIN affine root");
        let minimum_root = ScalarTerm::value(minimum_root_id, ScalarType::Integer(i64_type));
        let minimum_offset = ScalarTerm::value(
            ValueId::new(1669).expect("MIN affine offset"),
            ScalarType::Integer(i64_type),
        );
        let minimum_definitions = vec![Proposition::Equal(
            minimum_offset.clone(),
            ScalarTerm::exact_integer_add(
                i64_type,
                minimum_root.clone(),
                ScalarTerm::integer(i64_type, IntegerValue::Signed(0)).expect("0i64"),
            )
            .expect("root + 0"),
        )];
        assert_eq!(
            exact_integer_signed_affine_chain_obligation(
                i64_type,
                minimum_offset,
                IntegerValue::Signed(i64::MIN.into()),
                ExactIntegerAffineOperation::Multiply,
                &minimum_definitions,
                minimum_definitions.len(),
                &BTreeSet::from([minimum_root_id]),
            ),
            Some(exact_integer_source_interval_obligation(
                i64_type,
                minimum_root,
                0,
                1,
            )),
            "MIN is handled by magnitude without host negation",
        );

        let overflow_first = ScalarTerm::value(
            ValueId::new(1670).expect("overflow first"),
            ScalarType::Integer(i64_type),
        );
        let overflow_second = ScalarTerm::value(
            ValueId::new(1671).expect("overflow second"),
            ScalarType::Integer(i64_type),
        );
        let overflow_third = ScalarTerm::value(
            ValueId::new(1672).expect("overflow third"),
            ScalarType::Integer(i64_type),
        );
        let overflow_definitions = vec![
            Proposition::Equal(
                overflow_first.clone(),
                ScalarTerm::exact_integer_add(
                    i64_type,
                    ScalarTerm::value(minimum_root_id, ScalarType::Integer(i64_type)),
                    ScalarTerm::integer(i64_type, IntegerValue::Signed(1)).expect("1i64"),
                )
                .expect("root + 1"),
            ),
            Proposition::Equal(
                overflow_second.clone(),
                ScalarTerm::exact_integer_multiply(
                    i64_type,
                    overflow_first,
                    ScalarTerm::integer(i64_type, IntegerValue::Signed(i64::MIN.into()))
                        .expect("MIN i64"),
                )
                .expect("offset * MIN"),
            ),
            Proposition::Equal(
                overflow_third.clone(),
                ScalarTerm::exact_integer_multiply(
                    i64_type,
                    overflow_second,
                    ScalarTerm::integer(i64_type, IntegerValue::Signed(i64::MIN.into()))
                        .expect("MIN i64"),
                )
                .expect("MIN product * MIN"),
            ),
        ];
        assert_eq!(
            exact_integer_signed_affine_chain_obligation(
                i64_type,
                overflow_third,
                IntegerValue::Signed(4),
                ExactIntegerAffineOperation::Multiply,
                &overflow_definitions,
                overflow_definitions.len(),
                &BTreeSet::from([minimum_root_id]),
            ),
            None,
            "checked coefficient or offset overflow admits no family",
        );
        assert_eq!(
            exact_integer_signed_affine_chain_obligation(
                i8_type,
                ScalarTerm::value(
                    ValueId::new(1673).expect("stale value"),
                    ScalarType::Integer(i8_type),
                ),
                IntegerValue::Signed(-2),
                ExactIntegerAffineOperation::Multiply,
                &definitions,
                definitions.len(),
                &parameters,
            ),
            None,
            "a stale definition cannot borrow the valid chain's evidence",
        );
    }

    #[test]
    fn exact_cast_chain_intersects_every_carrier_without_importing_prefix_proofs() {
        let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
        let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
        let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let root_id = ValueId::new(1701).expect("cast-chain root");
        let root = ScalarTerm::value(root_id, ScalarType::Integer(i64_type));
        let first = ScalarTerm::value(
            ValueId::new(1702).expect("first cast"),
            ScalarType::Integer(u64_type),
        );
        let second = ScalarTerm::value(
            ValueId::new(1703).expect("second cast"),
            ScalarType::Integer(i32_type),
        );
        let first_definition = Proposition::Equal(
            first.clone(),
            ScalarTerm::integer_exact_cast(i64_type, u64_type, root.clone()).expect("i64 to u64"),
        );
        let second_definition = Proposition::Equal(
            second.clone(),
            ScalarTerm::integer_exact_cast(u64_type, i32_type, first.clone()).expect("u64 to i32"),
        );
        let definitions = vec![first_definition.clone(), second_definition.clone()];
        let parameters = BTreeSet::from([root_id]);

        assert_eq!(
            exact_integer_cast_chain_obligation(
                u64_type,
                i32_type,
                first.clone(),
                std::slice::from_ref(&first_definition),
                &parameters,
            ),
            Some(exact_integer_source_interval_obligation(
                i64_type,
                root.clone(),
                0,
                i32::MAX.into(),
            )),
            "the second cast independently reconstructs the first two carrier intersections",
        );
        assert_eq!(
            exact_integer_cast_chain_obligation(
                i32_type,
                u8_type,
                second.clone(),
                &definitions,
                &parameters,
            ),
            Some(exact_integer_source_interval_obligation(
                i64_type,
                root.clone(),
                0,
                u8::MAX.into(),
            )),
            "the third cast independently intersects every prior carrier",
        );

        let reordered = vec![second_definition, first_definition];
        assert_eq!(
            exact_integer_cast_chain_obligation(i32_type, u8_type, second, &reordered, &parameters,),
            None,
            "definition order is proof structure",
        );
        let widened = ScalarTerm::value(
            ValueId::new(1704).expect("widened cast"),
            ScalarType::Integer(i32_type),
        );
        let widening_definition = Proposition::Equal(
            widened.clone(),
            ScalarTerm::integer_exact_cast(
                u8_type,
                i32_type,
                ScalarTerm::value(root_id, ScalarType::Integer(u8_type)),
            )
            .expect("core term permits fixed exact casts"),
        );
        assert_eq!(
            exact_integer_cast_chain_obligation(
                i32_type,
                u8_type,
                widened,
                &[widening_definition],
                &BTreeSet::from([root_id]),
            ),
            None,
            "a nested widening-shaped cast is not admitted",
        );
    }

    #[test]
    fn computed_prefix_cast_chain_replays_each_existing_source_algebra_independently() {
        let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
        let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
        let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
        let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
        let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 count");
        let root_id = ValueId::new(1801).expect("computed cast-chain root");
        let root = ScalarTerm::value(root_id, ScalarType::Integer(i64_type));
        let product = ScalarTerm::value(
            ValueId::new(1802).expect("product"),
            ScalarType::Integer(i64_type),
        );
        let affine = ScalarTerm::value(
            ValueId::new(1803).expect("affine"),
            ScalarType::Integer(i64_type),
        );
        let first = ScalarTerm::value(
            ValueId::new(1804).expect("first cast"),
            ScalarType::Integer(u64_type),
        );
        let second = ScalarTerm::value(
            ValueId::new(1805).expect("second cast"),
            ScalarType::Integer(i32_type),
        );
        let two = ScalarTerm::integer(i64_type, IntegerValue::Signed(2)).expect("2i64");
        let one = ScalarTerm::integer(i64_type, IntegerValue::Signed(1)).expect("1i64");
        let affine_definitions = vec![
            Proposition::Equal(
                product.clone(),
                ScalarTerm::exact_integer_multiply(i64_type, root.clone(), two).expect("root * 2"),
            ),
            Proposition::Equal(
                affine.clone(),
                ScalarTerm::exact_integer_add(i64_type, product, one).expect("product + 1"),
            ),
            Proposition::Equal(
                first.clone(),
                ScalarTerm::integer_exact_cast(i64_type, u64_type, affine).expect("i64 to u64"),
            ),
            Proposition::Equal(
                second.clone(),
                ScalarTerm::integer_exact_cast(u64_type, i32_type, first).expect("u64 to i32"),
            ),
        ];
        let parameters = BTreeSet::from([root_id]);
        assert_eq!(
            exact_integer_computed_prefix_cast_chain_obligation(
                i32_type,
                u8_type,
                second,
                &affine_definitions,
                &parameters,
            ),
            Some(exact_integer_source_interval_obligation(
                i64_type,
                root.clone(),
                0,
                127,
            )),
            "the outer cast maps the intersection of every cast carrier through A*x+B",
        );

        let negative_product = ScalarTerm::value(
            ValueId::new(1811).expect("negative product"),
            ScalarType::Integer(i64_type),
        );
        let negative_first = ScalarTerm::value(
            ValueId::new(1812).expect("negative first cast"),
            ScalarType::Integer(u64_type),
        );
        let negative_second = ScalarTerm::value(
            ValueId::new(1813).expect("negative second cast"),
            ScalarType::Integer(i32_type),
        );
        let negative_definitions = vec![
            Proposition::Equal(
                negative_product.clone(),
                ScalarTerm::exact_integer_multiply(
                    i64_type,
                    root.clone(),
                    ScalarTerm::integer(i64_type, IntegerValue::Signed(-2)).expect("-2i64"),
                )
                .expect("root * -2"),
            ),
            Proposition::Equal(
                negative_first.clone(),
                ScalarTerm::integer_exact_cast(i64_type, u64_type, negative_product)
                    .expect("negative product to u64"),
            ),
            Proposition::Equal(
                negative_second.clone(),
                ScalarTerm::integer_exact_cast(u64_type, i32_type, negative_first)
                    .expect("u64 to i32"),
            ),
        ];
        assert_eq!(
            exact_integer_computed_prefix_cast_chain_obligation(
                i32_type,
                u8_type,
                negative_second,
                &negative_definitions,
                &parameters,
            ),
            Some(exact_integer_source_interval_obligation(
                i64_type,
                root.clone(),
                -127,
                0,
            )),
            "negative products reverse the complete cast-carrier intersection",
        );

        let shifted_left = ScalarTerm::value(
            ValueId::new(1821).expect("left shift"),
            ScalarType::Integer(i64_type),
        );
        let shifted_right = ScalarTerm::value(
            ValueId::new(1822).expect("right shift"),
            ScalarType::Integer(i64_type),
        );
        let shift_first = ScalarTerm::value(
            ValueId::new(1823).expect("shift first cast"),
            ScalarType::Integer(u64_type),
        );
        let shift_second = ScalarTerm::value(
            ValueId::new(1824).expect("shift second cast"),
            ScalarType::Integer(i32_type),
        );
        let shift_count = ScalarTerm::integer(u16_type, IntegerValue::Unsigned(1)).expect("1u16");
        let shift_definitions = vec![
            Proposition::Equal(
                shifted_left.clone(),
                ScalarTerm::exact_integer_shift_left(
                    i64_type,
                    u16_type,
                    root.clone(),
                    shift_count.clone(),
                )
                .expect("root << 1"),
            ),
            Proposition::Equal(
                shifted_right.clone(),
                ScalarTerm::exact_integer_shift_right(
                    i64_type,
                    u16_type,
                    shifted_left,
                    shift_count,
                )
                .expect("shifted << 1 then >> 1"),
            ),
            Proposition::Equal(
                shift_first.clone(),
                ScalarTerm::integer_exact_cast(i64_type, u64_type, shifted_right)
                    .expect("shift result to u64"),
            ),
            Proposition::Equal(
                shift_second.clone(),
                ScalarTerm::integer_exact_cast(u64_type, i32_type, shift_first)
                    .expect("u64 to i32"),
            ),
        ];
        assert_eq!(
            exact_integer_computed_prefix_cast_chain_obligation(
                i32_type,
                u8_type,
                shift_second,
                &shift_definitions,
                &parameters,
            ),
            Some(exact_integer_source_interval_obligation(
                i64_type,
                root.clone(),
                0,
                255,
            )),
            "mixed shifts replay from the complete cast-carrier intersection",
        );

        let unsigned_root_id = ValueId::new(1831).expect("unsigned DQ root");
        let unsigned_root = ScalarTerm::value(unsigned_root_id, ScalarType::Integer(u64_type));
        let divided = ScalarTerm::value(
            ValueId::new(1832).expect("divided"),
            ScalarType::Integer(u64_type),
        );
        let remainder = ScalarTerm::value(
            ValueId::new(1833).expect("remainder"),
            ScalarType::Integer(u64_type),
        );
        let dq_first = ScalarTerm::value(
            ValueId::new(1834).expect("DQ first cast"),
            ScalarType::Integer(i64_type),
        );
        let dq_second = ScalarTerm::value(
            ValueId::new(1835).expect("DQ second cast"),
            ScalarType::Integer(u32_type),
        );
        let dq_definitions = vec![
            Proposition::Equal(
                divided.clone(),
                ScalarTerm::exact_integer_divide(
                    u64_type,
                    unsigned_root,
                    ScalarTerm::integer(u64_type, IntegerValue::Unsigned(2)).expect("2u64"),
                )
                .expect("root / 2"),
            ),
            Proposition::Equal(
                remainder.clone(),
                ScalarTerm::exact_integer_remainder(
                    u64_type,
                    divided,
                    ScalarTerm::integer(u64_type, IntegerValue::Unsigned(3)).expect("3u64"),
                )
                .expect("divided % 3"),
            ),
            Proposition::Equal(
                dq_first.clone(),
                ScalarTerm::integer_exact_cast(u64_type, i64_type, remainder)
                    .expect("DQ result to i64"),
            ),
            Proposition::Equal(
                dq_second.clone(),
                ScalarTerm::integer_exact_cast(i64_type, u32_type, dq_first).expect("i64 to u32"),
            ),
        ];
        assert_eq!(
            exact_integer_computed_prefix_cast_chain_obligation(
                u32_type,
                i16_type,
                dq_second,
                &dq_definitions,
                &BTreeSet::from([unsigned_root_id]),
            ),
            Some(Proposition::Truth),
            "the complete DQ hull fits every cast carrier",
        );

        let zero_product = ScalarTerm::value(
            ValueId::new(1841).expect("zero product"),
            ScalarType::Integer(i64_type),
        );
        let negative_constant = ScalarTerm::value(
            ValueId::new(1842).expect("negative constant"),
            ScalarType::Integer(i64_type),
        );
        let empty_first = ScalarTerm::value(
            ValueId::new(1843).expect("empty first cast"),
            ScalarType::Integer(u64_type),
        );
        let empty_second = ScalarTerm::value(
            ValueId::new(1844).expect("empty second cast"),
            ScalarType::Integer(i32_type),
        );
        let empty_definitions = vec![
            Proposition::Equal(
                zero_product.clone(),
                ScalarTerm::exact_integer_multiply(
                    i64_type,
                    root.clone(),
                    ScalarTerm::integer(i64_type, IntegerValue::Signed(0)).expect("0i64"),
                )
                .expect("root * 0"),
            ),
            Proposition::Equal(
                negative_constant.clone(),
                ScalarTerm::exact_integer_add(
                    i64_type,
                    zero_product,
                    ScalarTerm::integer(i64_type, IntegerValue::Signed(-1)).expect("-1i64"),
                )
                .expect("zero product - 1"),
            ),
            Proposition::Equal(
                empty_first.clone(),
                ScalarTerm::integer_exact_cast(i64_type, u64_type, negative_constant)
                    .expect("negative constant to u64"),
            ),
            Proposition::Equal(
                empty_second.clone(),
                ScalarTerm::integer_exact_cast(u64_type, i32_type, empty_first)
                    .expect("u64 to i32"),
            ),
        ];
        assert_eq!(
            exact_integer_computed_prefix_cast_chain_obligation(
                i32_type,
                u8_type,
                empty_second,
                &empty_definitions,
                &parameters,
            ),
            Some(Proposition::Falsehood),
            "a mathematical empty preimage is canonical falsehood",
        );

        let mut stale_definitions = affine_definitions;
        stale_definitions.swap(0, 1);
        let stale_second = match &stale_definitions[3] {
            Proposition::Equal(left, _) => left.clone(),
            _ => unreachable!("cast definition is an equality"),
        };
        assert_eq!(
            exact_integer_computed_prefix_cast_chain_obligation(
                i32_type,
                u8_type,
                stale_second,
                &stale_definitions,
                &parameters,
            ),
            None,
            "reordered source definitions remain fail-closed",
        );
    }

    #[test]
    fn cast_chain_then_computed_suffix_replays_each_target_algebra_independently() {
        let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
        let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
        let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
        let root_id = ValueId::new(1901).expect("cast-chain suffix root");
        let root = ScalarTerm::value(root_id, ScalarType::Integer(i64_type));
        let first = ScalarTerm::value(
            ValueId::new(1902).expect("first cast"),
            ScalarType::Integer(u64_type),
        );
        let second = ScalarTerm::value(
            ValueId::new(1903).expect("second cast"),
            ScalarType::Integer(i32_type),
        );
        let definitions = vec![
            Proposition::Equal(
                first.clone(),
                ScalarTerm::integer_exact_cast(i64_type, u64_type, root.clone())
                    .expect("i64 to u64"),
            ),
            Proposition::Equal(
                second.clone(),
                ScalarTerm::integer_exact_cast(u64_type, i32_type, first).expect("u64 to i32"),
            ),
        ];
        let parameters = BTreeSet::from([root_id]);
        assert_eq!(
            exact_integer_cast_chain_then_affine_suffix_obligation(
                i32_type,
                second.clone(),
                IntegerValue::Signed(1),
                ExactIntegerAffineOperation::Add,
                &definitions,
                definitions.len(),
                &parameters,
            ),
            Some(exact_integer_source_interval_obligation(
                i64_type,
                root.clone(),
                0,
                2_147_483_646,
            )),
            "affine inversion intersects every cast carrier",
        );
        assert_eq!(
            exact_integer_cast_chain_then_signed_product_suffix_obligation(
                i32_type,
                second.clone(),
                IntegerValue::Signed(-2),
                &definitions,
                definitions.len(),
                &parameters,
            ),
            Some(exact_integer_source_interval_obligation(
                i64_type,
                root.clone(),
                0,
                1_073_741_824,
            )),
            "negative products reverse the target preimage before carrier intersection",
        );
        assert_eq!(
            exact_integer_cast_chain_then_shift_suffix_obligation(
                i32_type,
                second.clone(),
                1,
                &definitions,
                definitions.len(),
                &parameters,
            ),
            Some(exact_integer_source_interval_obligation(
                i64_type,
                root,
                0,
                1_073_741_823,
            )),
            "left-shift replay intersects the full cast-chain carrier hull",
        );
        assert_eq!(
            exact_integer_cast_chain_then_affine_suffix_obligation(
                i32_type,
                second.clone(),
                IntegerValue::Signed(0),
                ExactIntegerAffineOperation::Multiply,
                &definitions,
                definitions.len(),
                &parameters,
            ),
            Some(Proposition::Truth),
            "zero decides only its current suffix after full cast-chain validation",
        );

        let mut stale = definitions;
        stale.swap(0, 1);
        assert_eq!(
            exact_integer_cast_chain_then_affine_suffix_obligation(
                i32_type,
                second,
                IntegerValue::Signed(1),
                ExactIntegerAffineOperation::Add,
                &stale,
                stale.len(),
                &parameters,
            ),
            None,
            "reordered cast definitions remain fail-closed",
        );
    }

    #[test]
    fn computed_prefix_cast_chain_computed_suffix_composes_existing_interval_algebras() {
        let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
        let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
        let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
        let root_id = ValueId::new(2001).expect("sandwich root");
        let root = ScalarTerm::value(root_id, ScalarType::Integer(i64_type));
        let source = ScalarTerm::value(
            ValueId::new(2002).expect("source affine"),
            ScalarType::Integer(i64_type),
        );
        let first = ScalarTerm::value(
            ValueId::new(2003).expect("first cast"),
            ScalarType::Integer(u64_type),
        );
        let second = ScalarTerm::value(
            ValueId::new(2004).expect("second cast"),
            ScalarType::Integer(i32_type),
        );
        let definitions = vec![
            Proposition::Equal(
                source.clone(),
                ScalarTerm::exact_integer_add(
                    i64_type,
                    root.clone(),
                    ScalarTerm::integer(i64_type, IntegerValue::Signed(1)).expect("1i64"),
                )
                .expect("root + 1"),
            ),
            Proposition::Equal(
                first.clone(),
                ScalarTerm::integer_exact_cast(i64_type, u64_type, source).expect("i64 to u64"),
            ),
            Proposition::Equal(
                second.clone(),
                ScalarTerm::integer_exact_cast(u64_type, i32_type, first).expect("u64 to i32"),
            ),
        ];
        let parameters = BTreeSet::from([root_id]);
        assert_eq!(
            exact_integer_computed_prefix_cast_chain_interval_obligation(
                i32_type,
                second.clone(),
                (0, 100),
                &definitions,
                definitions.len(),
                &parameters,
            ),
            Some(exact_integer_source_interval_obligation(
                i64_type,
                root.clone(),
                -1,
                99,
            )),
            "target intervals cross every cast carrier before affine source inversion",
        );
        assert!(
            exact_integer_cast_chain_then_affine_suffix_obligation(
                i32_type,
                second.clone(),
                IntegerValue::Signed(1),
                ExactIntegerAffineOperation::Add,
                &definitions,
                definitions.len(),
                &parameters,
            )
            .is_some(),
            "affine target prefixes compose through the computed source",
        );
        assert!(
            exact_integer_cast_chain_then_signed_product_suffix_obligation(
                i32_type,
                second.clone(),
                IntegerValue::Signed(-2),
                &definitions,
                definitions.len(),
                &parameters,
            )
            .is_some(),
            "signed target products compose through the computed source",
        );
        assert!(
            exact_integer_cast_chain_then_shift_suffix_obligation(
                i32_type,
                second.clone(),
                1,
                &definitions,
                definitions.len(),
                &parameters,
            )
            .is_some(),
            "target left shifts compose through the computed source",
        );
        assert_eq!(
            exact_integer_cast_chain_then_affine_suffix_obligation(
                i32_type,
                second.clone(),
                IntegerValue::Signed(0),
                ExactIntegerAffineOperation::Multiply,
                &definitions,
                definitions.len(),
                &parameters,
            ),
            Some(Proposition::Truth),
            "zero decides only its target prefix after full sandwich validation",
        );

        let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let divide_root_id = ValueId::new(2011).expect("divide root");
        let divide_root = ScalarTerm::value(divide_root_id, ScalarType::Integer(u32_type));
        let remainder = ScalarTerm::value(
            ValueId::new(2012).expect("remainder"),
            ScalarType::Integer(u32_type),
        );
        let narrow = ScalarTerm::value(
            ValueId::new(2013).expect("narrow"),
            ScalarType::Integer(u8_type),
        );
        let signed = ScalarTerm::value(
            ValueId::new(2014).expect("signed"),
            ScalarType::Integer(i8_type),
        );
        let divide_definitions = vec![
            Proposition::Equal(
                remainder.clone(),
                ScalarTerm::exact_integer_remainder(
                    u32_type,
                    divide_root,
                    ScalarTerm::integer(u32_type, IntegerValue::Unsigned(3)).expect("3u32"),
                )
                .expect("root % 3"),
            ),
            Proposition::Equal(
                narrow.clone(),
                ScalarTerm::integer_exact_cast(u32_type, u8_type, remainder).expect("u32 to u8"),
            ),
            Proposition::Equal(
                signed.clone(),
                ScalarTerm::integer_exact_cast(u8_type, i8_type, narrow).expect("u8 to i8"),
            ),
        ];
        let divide_parameters = BTreeSet::from([divide_root_id]);
        assert_eq!(
            exact_integer_computed_prefix_cast_chain_interval_obligation(
                i8_type,
                signed.clone(),
                (0, 2),
                &divide_definitions,
                divide_definitions.len(),
                &divide_parameters,
            ),
            Some(Proposition::Truth),
            "a contained carrier-total hull is true",
        );
        assert_eq!(
            exact_integer_computed_prefix_cast_chain_interval_obligation(
                i8_type,
                signed.clone(),
                (3, 4),
                &divide_definitions,
                divide_definitions.len(),
                &divide_parameters,
            ),
            Some(Proposition::Falsehood),
            "a disjoint carrier-total hull is falsehood",
        );
        assert_eq!(
            exact_integer_computed_prefix_cast_chain_interval_obligation(
                i8_type,
                signed,
                (0, 1),
                &divide_definitions,
                divide_definitions.len(),
                &divide_parameters,
            ),
            None,
            "a partial hull overlap remains unadmitted",
        );

        let mut stale = definitions;
        stale.swap(1, 2);
        assert_eq!(
            exact_integer_cast_chain_then_affine_suffix_obligation(
                i32_type,
                second,
                IntegerValue::Signed(1),
                ExactIntegerAffineOperation::Add,
                &stale,
                stale.len(),
                &parameters,
            ),
            None,
            "reordered cast definitions remain fail-closed",
        );
    }

    #[test]
    fn computed_prefix_widen_chain_computed_suffix_composes_existing_interval_algebras() {
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
        let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
        let root_id = ValueId::new(2101).expect("widen sandwich root");
        let root = ScalarTerm::value(root_id, ScalarType::Integer(i8_type));
        let source = ScalarTerm::value(
            ValueId::new(2102).expect("source affine"),
            ScalarType::Integer(i8_type),
        );
        let first = ScalarTerm::value(
            ValueId::new(2103).expect("first widen"),
            ScalarType::Integer(i16_type),
        );
        let second = ScalarTerm::value(
            ValueId::new(2104).expect("second widen"),
            ScalarType::Integer(i32_type),
        );
        let definitions = vec![
            Proposition::Equal(
                source.clone(),
                ScalarTerm::exact_integer_add(
                    i8_type,
                    root.clone(),
                    ScalarTerm::integer(i8_type, IntegerValue::Signed(1)).expect("1i8"),
                )
                .expect("root + 1"),
            ),
            Proposition::Equal(
                first.clone(),
                ScalarTerm::integer_widen(i8_type, i16_type, source).expect("i8 to i16"),
            ),
            Proposition::Equal(
                second.clone(),
                ScalarTerm::integer_widen(i16_type, i32_type, first).expect("i16 to i32"),
            ),
        ];
        let parameters = BTreeSet::from([root_id]);
        assert_eq!(
            exact_integer_computed_prefix_widen_chain_interval_obligation(
                i32_type,
                second.clone(),
                (0, 100),
                &definitions,
                definitions.len(),
                &parameters,
            ),
            Some(exact_integer_source_interval_obligation(
                i8_type,
                root.clone(),
                -1,
                99,
            )),
            "target intervals cross each strict widening before source inversion",
        );
        assert!(
            exact_integer_cast_chain_then_affine_suffix_obligation(
                i32_type,
                second.clone(),
                IntegerValue::Signed(1),
                ExactIntegerAffineOperation::Add,
                &definitions,
                definitions.len(),
                &parameters,
            )
            .is_some(),
            "target affine prefixes compose across the widening seam",
        );
        assert!(
            exact_integer_cast_chain_then_signed_product_suffix_obligation(
                i32_type,
                second.clone(),
                IntegerValue::Signed(-2),
                &definitions,
                definitions.len(),
                &parameters,
            )
            .is_some(),
            "target signed products compose across the widening seam",
        );
        assert!(
            exact_integer_cast_chain_then_shift_suffix_obligation(
                i32_type,
                second.clone(),
                1,
                &definitions,
                definitions.len(),
                &parameters,
            )
            .is_some(),
            "target left shifts compose across the widening seam",
        );
        assert_eq!(
            exact_integer_cast_chain_then_affine_suffix_obligation(
                i32_type,
                second.clone(),
                IntegerValue::Signed(0),
                ExactIntegerAffineOperation::Multiply,
                &definitions,
                definitions.len(),
                &parameters,
            ),
            Some(Proposition::Truth),
            "zero decides only its target prefix after full widening-shape validation",
        );

        let product_root_id = ValueId::new(2111).expect("product root");
        let product_root = ScalarTerm::value(product_root_id, ScalarType::Integer(i8_type));
        let product = ScalarTerm::value(
            ValueId::new(2112).expect("signed product"),
            ScalarType::Integer(i8_type),
        );
        let product_widened = ScalarTerm::value(
            ValueId::new(2113).expect("product widened"),
            ScalarType::Integer(i16_type),
        );
        let product_definitions = vec![
            Proposition::Equal(
                product.clone(),
                ScalarTerm::exact_integer_multiply(
                    i8_type,
                    product_root,
                    ScalarTerm::integer(i8_type, IntegerValue::Signed(-2)).expect("-2i8"),
                )
                .expect("root * -2"),
            ),
            Proposition::Equal(
                product_widened.clone(),
                ScalarTerm::integer_widen(i8_type, i16_type, product).expect("i8 to i16"),
            ),
        ];
        assert!(
            exact_integer_computed_prefix_widen_chain_interval_obligation(
                i16_type,
                product_widened,
                (-100, 100),
                &product_definitions,
                product_definitions.len(),
                &BTreeSet::from([product_root_id]),
            )
            .is_some(),
            "negative source products retain reversed inverse replay",
        );

        let shift_root_id = ValueId::new(2121).expect("shift root");
        let shift_root = ScalarTerm::value(shift_root_id, ScalarType::Integer(u8_type));
        let shifted = ScalarTerm::value(
            ValueId::new(2122).expect("shifted"),
            ScalarType::Integer(u8_type),
        );
        let shift_widened = ScalarTerm::value(
            ValueId::new(2123).expect("shift widened"),
            ScalarType::Integer(i16_type),
        );
        let shift_definitions = vec![
            Proposition::Equal(
                shifted.clone(),
                ScalarTerm::exact_integer_shift_right(
                    u8_type,
                    u8_type,
                    shift_root,
                    ScalarTerm::integer(u8_type, IntegerValue::Unsigned(1)).expect("1u8"),
                )
                .expect("root >> 1"),
            ),
            Proposition::Equal(
                shift_widened.clone(),
                ScalarTerm::integer_widen(u8_type, i16_type, shifted).expect("u8 to i16"),
            ),
        ];
        assert!(
            exact_integer_computed_prefix_widen_chain_interval_obligation(
                i16_type,
                shift_widened,
                (0, 100),
                &shift_definitions,
                shift_definitions.len(),
                &BTreeSet::from([shift_root_id]),
            )
            .is_some(),
            "source shifts retain their ordered inverse replay",
        );

        let divide_root_id = ValueId::new(2131).expect("divide root");
        let divide_root = ScalarTerm::value(divide_root_id, ScalarType::Integer(u8_type));
        let remainder = ScalarTerm::value(
            ValueId::new(2132).expect("remainder"),
            ScalarType::Integer(u8_type),
        );
        let remainder_widened = ScalarTerm::value(
            ValueId::new(2133).expect("remainder widened"),
            ScalarType::Integer(i16_type),
        );
        let divide_definitions = vec![
            Proposition::Equal(
                remainder.clone(),
                ScalarTerm::exact_integer_remainder(
                    u8_type,
                    divide_root,
                    ScalarTerm::integer(u8_type, IntegerValue::Unsigned(3)).expect("3u8"),
                )
                .expect("root % 3"),
            ),
            Proposition::Equal(
                remainder_widened.clone(),
                ScalarTerm::integer_widen(u8_type, i16_type, remainder).expect("u8 to i16"),
            ),
        ];
        let divide_parameters = BTreeSet::from([divide_root_id]);
        for (interval, expected) in [
            ((0, 2), Some(Proposition::Truth)),
            ((3, 4), Some(Proposition::Falsehood)),
            ((0, 1), None),
        ] {
            assert_eq!(
                exact_integer_computed_prefix_widen_chain_interval_obligation(
                    i16_type,
                    remainder_widened.clone(),
                    interval,
                    &divide_definitions,
                    divide_definitions.len(),
                    &divide_parameters,
                ),
                expected,
            );
        }

        let mut stale = definitions;
        stale.swap(1, 2);
        assert_eq!(
            exact_integer_cast_chain_then_affine_suffix_obligation(
                i32_type,
                second,
                IntegerValue::Signed(1),
                ExactIntegerAffineOperation::Add,
                &stale,
                stale.len(),
                &parameters,
            ),
            None,
            "reordered widening definitions remain fail-closed",
        );
    }

    #[test]
    fn computed_prefix_mixed_conversion_chain_computed_suffix_replays_every_edge_independently() {
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
        let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
        let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
        let root_id = ValueId::new(2141).expect("mixed conversion root");
        let root = ScalarTerm::value(root_id, ScalarType::Integer(i16_type));
        let source = ScalarTerm::value(
            ValueId::new(2142).expect("source affine"),
            ScalarType::Integer(i16_type),
        );
        let widened = ScalarTerm::value(
            ValueId::new(2143).expect("widened"),
            ScalarType::Integer(i32_type),
        );
        let narrowed = ScalarTerm::value(
            ValueId::new(2144).expect("narrowed"),
            ScalarType::Integer(i16_type),
        );
        let definitions = vec![
            Proposition::Equal(
                source.clone(),
                ScalarTerm::exact_integer_add(
                    i16_type,
                    root.clone(),
                    ScalarTerm::integer(i16_type, IntegerValue::Signed(1)).expect("1i16"),
                )
                .expect("root + 1"),
            ),
            Proposition::Equal(
                widened.clone(),
                ScalarTerm::integer_widen(i16_type, i32_type, source).expect("i16 to i32"),
            ),
            Proposition::Equal(
                narrowed.clone(),
                ScalarTerm::integer_exact_cast(i32_type, i16_type, widened.clone())
                    .expect("i32 to i16"),
            ),
        ];
        let parameters = BTreeSet::from([root_id]);
        assert_eq!(
            exact_integer_computed_prefix_mixed_conversion_chain_cast_obligation(
                i32_type,
                i16_type,
                widened,
                &definitions[..2],
                &parameters,
            ),
            Some(exact_integer_source_interval_obligation(
                i16_type,
                root.clone(),
                i16::MIN.into(),
                (i16::MAX - 1).into(),
            )),
            "the partial cast replays the prior widen and source affine independently",
        );
        assert_eq!(
            exact_integer_computed_prefix_mixed_conversion_chain_interval_obligation(
                i16_type,
                narrowed.clone(),
                (0, 100),
                &definitions,
                definitions.len(),
                &parameters,
            ),
            Some(exact_integer_source_interval_obligation(
                i16_type,
                root.clone(),
                -1,
                99,
            )),
        );
        assert!(
            exact_integer_cast_chain_then_affine_suffix_obligation(
                i16_type,
                narrowed.clone(),
                IntegerValue::Signed(1),
                ExactIntegerAffineOperation::Add,
                &definitions,
                definitions.len(),
                &parameters,
            )
            .is_some(),
            "target affine algebra composes with the heterogeneous conversion spine",
        );
        assert!(
            exact_integer_cast_chain_then_signed_product_suffix_obligation(
                i16_type,
                narrowed.clone(),
                IntegerValue::Signed(-2),
                &definitions,
                definitions.len(),
                &parameters,
            )
            .is_some(),
            "target signed-product algebra composes with the heterogeneous conversion spine",
        );
        assert!(
            exact_integer_cast_chain_then_shift_suffix_obligation(
                i16_type,
                narrowed,
                1,
                &definitions,
                definitions.len(),
                &parameters,
            )
            .is_some(),
            "target shift algebra composes with the heterogeneous conversion spine",
        );

        let alternating_root_id = ValueId::new(2151).expect("alternating root");
        let alternating_root = ScalarTerm::value(alternating_root_id, ScalarType::Integer(i8_type));
        let alternating_source = ScalarTerm::value(
            ValueId::new(2152).expect("alternating source"),
            ScalarType::Integer(i8_type),
        );
        let alternating_first = ScalarTerm::value(
            ValueId::new(2153).expect("alternating first widen"),
            ScalarType::Integer(i16_type),
        );
        let alternating_second = ScalarTerm::value(
            ValueId::new(2154).expect("alternating cast"),
            ScalarType::Integer(u8_type),
        );
        let alternating_third = ScalarTerm::value(
            ValueId::new(2155).expect("alternating second widen"),
            ScalarType::Integer(i16_type),
        );
        let alternating_fourth = ScalarTerm::value(
            ValueId::new(2156).expect("alternating second cast"),
            ScalarType::Integer(u8_type),
        );
        let alternating_definitions = vec![
            Proposition::Equal(
                alternating_source.clone(),
                ScalarTerm::exact_integer_add(
                    i8_type,
                    alternating_root,
                    ScalarTerm::integer(i8_type, IntegerValue::Signed(1)).expect("1i8"),
                )
                .expect("root + 1"),
            ),
            Proposition::Equal(
                alternating_first.clone(),
                ScalarTerm::integer_widen(i8_type, i16_type, alternating_source)
                    .expect("i8 to i16"),
            ),
            Proposition::Equal(
                alternating_second.clone(),
                ScalarTerm::integer_exact_cast(i16_type, u8_type, alternating_first)
                    .expect("i16 to u8"),
            ),
            Proposition::Equal(
                alternating_third.clone(),
                ScalarTerm::integer_widen(u8_type, i16_type, alternating_second)
                    .expect("u8 to i16"),
            ),
            Proposition::Equal(
                alternating_fourth.clone(),
                ScalarTerm::integer_exact_cast(i16_type, u8_type, alternating_third.clone())
                    .expect("i16 to u8"),
            ),
        ];
        assert!(
            exact_integer_computed_prefix_mixed_conversion_chain_cast_obligation(
                i16_type,
                u8_type,
                alternating_third,
                &alternating_definitions[..4],
                &BTreeSet::from([alternating_root_id]),
            )
            .is_some(),
            "each later cast replays every preceding alternating edge independently",
        );
        assert!(
            exact_integer_computed_prefix_mixed_conversion_chain_interval_obligation(
                u8_type,
                alternating_fourth.clone(),
                (0, 100),
                &alternating_definitions,
                alternating_definitions.len(),
                &BTreeSet::from([alternating_root_id]),
            )
            .is_some(),
            "an alternating widen-cast-widen word replays as one ordered spine",
        );

        let divide_root_id = ValueId::new(2161).expect("divide root");
        let divide_root = ScalarTerm::value(divide_root_id, ScalarType::Integer(u16_type));
        let remainder = ScalarTerm::value(
            ValueId::new(2162).expect("remainder"),
            ScalarType::Integer(u16_type),
        );
        let remainder_cast = ScalarTerm::value(
            ValueId::new(2163).expect("remainder cast"),
            ScalarType::Integer(i16_type),
        );
        let remainder_widened = ScalarTerm::value(
            ValueId::new(2164).expect("remainder widened"),
            ScalarType::Integer(i32_type),
        );
        let divide_definitions = vec![
            Proposition::Equal(
                remainder.clone(),
                ScalarTerm::exact_integer_remainder(
                    u16_type,
                    divide_root,
                    ScalarTerm::integer(u16_type, IntegerValue::Unsigned(3)).expect("3u16"),
                )
                .expect("root % 3"),
            ),
            Proposition::Equal(
                remainder_cast.clone(),
                ScalarTerm::integer_exact_cast(u16_type, i16_type, remainder).expect("u16 to i16"),
            ),
            Proposition::Equal(
                remainder_widened.clone(),
                ScalarTerm::integer_widen(i16_type, i32_type, remainder_cast).expect("i16 to i32"),
            ),
        ];
        let divide_parameters = BTreeSet::from([divide_root_id]);
        for (requested, expected) in [
            ((0, 2), Some(Proposition::Truth)),
            ((3, 4), Some(Proposition::Falsehood)),
            ((0, 1), None),
        ] {
            assert_eq!(
                exact_integer_computed_prefix_mixed_conversion_chain_interval_obligation(
                    i32_type,
                    remainder_widened.clone(),
                    requested,
                    &divide_definitions,
                    divide_definitions.len(),
                    &divide_parameters,
                ),
                expected,
            );
        }

        let wide_divide_root_id = ValueId::new(2171).expect("wide divide root");
        let wide_divide_root =
            ScalarTerm::value(wide_divide_root_id, ScalarType::Integer(i16_type));
        let wide_divide = ScalarTerm::value(
            ValueId::new(2172).expect("wide divide"),
            ScalarType::Integer(i16_type),
        );
        let wide_divide_widened = ScalarTerm::value(
            ValueId::new(2173).expect("wide divide widened"),
            ScalarType::Integer(i32_type),
        );
        let wide_divide_definitions = vec![
            Proposition::Equal(
                wide_divide.clone(),
                ScalarTerm::exact_integer_divide(
                    i16_type,
                    wide_divide_root,
                    ScalarTerm::integer(i16_type, IntegerValue::Signed(2)).expect("2i16"),
                )
                .expect("root / 2"),
            ),
            Proposition::Equal(
                wide_divide_widened.clone(),
                ScalarTerm::integer_widen(i16_type, i32_type, wide_divide).expect("i16 to i32"),
            ),
        ];
        assert_eq!(
            exact_integer_computed_prefix_mixed_conversion_chain_cast_obligation(
                i32_type,
                i8_type,
                wide_divide_widened,
                &wide_divide_definitions,
                &BTreeSet::from([wide_divide_root_id]),
            ),
            None,
            "a partial D hull never becomes cast authority or falsehood",
        );

        let mut stale = alternating_definitions;
        stale.swap(1, 2);
        assert_eq!(
            exact_integer_computed_prefix_mixed_conversion_chain_interval_obligation(
                u8_type,
                alternating_fourth,
                (0, 100),
                &stale,
                stale.len(),
                &BTreeSet::from([alternating_root_id]),
            ),
            None,
            "reordered conversion definitions remain fail-closed",
        );
    }

    #[test]
    fn mixed_affine_chain_reconstructs_every_prefix_and_constant_collapse() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let root_id = ValueId::new(601).expect("root");
        let root = ScalarTerm::value(root_id, ScalarType::Integer(u8_type));
        let added = ScalarTerm::value(
            ValueId::new(602).expect("added"),
            ScalarType::Integer(u8_type),
        );
        let multiplied = ScalarTerm::value(
            ValueId::new(603).expect("multiplied"),
            ScalarType::Integer(u8_type),
        );
        let three = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(3)).expect("3u8");
        let two = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(2)).expect("2u8");
        let definitions = vec![
            Proposition::Equal(
                added.clone(),
                ScalarTerm::exact_integer_add(u8_type, root.clone(), three).expect("root + 3"),
            ),
            Proposition::Equal(
                multiplied.clone(),
                ScalarTerm::exact_integer_multiply(u8_type, added.clone(), two).expect("added * 2"),
            ),
        ];
        let parameters = BTreeSet::from([root_id]);
        assert_eq!(
            exact_integer_affine_chain_obligation(
                u8_type,
                added.clone(),
                IntegerValue::Unsigned(2),
                ExactIntegerAffineOperation::Multiply,
                &definitions,
                definitions.len(),
                &parameters,
            ),
            Some(Proposition::LessOrEqual(
                root.clone(),
                ScalarTerm::integer(u8_type, IntegerValue::Unsigned(124)).expect("124u8"),
            )),
            "the multiply prefix independently reconstructs 2*p + 6"
        );
        assert_eq!(
            exact_integer_affine_chain_obligation(
                u8_type,
                multiplied.clone(),
                IntegerValue::Unsigned(1),
                ExactIntegerAffineOperation::Subtract,
                &definitions,
                definitions.len(),
                &parameters,
            ),
            Some(Proposition::LessOrEqual(
                root.clone(),
                ScalarTerm::integer(u8_type, IntegerValue::Unsigned(125)).expect("125u8"),
            )),
            "the subtract prefix independently reconstructs 2*p + 5"
        );
        assert_eq!(
            exact_integer_affine_chain_obligation(
                u8_type,
                added.clone(),
                IntegerValue::Unsigned(0),
                ExactIntegerAffineOperation::Multiply,
                &definitions,
                1,
                &parameters,
            ),
            Some(Proposition::Truth),
            "a zero factor makes only the current prefix constant while the add proof remains"
        );
        assert_eq!(
            exact_integer_affine_chain_obligation(
                u8_type,
                multiplied,
                IntegerValue::Unsigned(1),
                ExactIntegerAffineOperation::Subtract,
                &[definitions[1].clone(), definitions[0].clone()],
                definitions.len(),
                &parameters,
            ),
            None,
            "reordered definitions cannot authorize the affine walk"
        );

        let signed_root_id = ValueId::new(611).expect("signed root");
        let signed_root = ScalarTerm::value(signed_root_id, ScalarType::Integer(i8_type));
        assert_eq!(
            exact_integer_affine_interval_obligation(
                i8_type,
                signed_root.clone(),
                2,
                IntegerOffset::Nonnegative(7),
            ),
            canonical_conjunction(vec![
                Proposition::LessOrEqual(
                    ScalarTerm::integer(i8_type, IntegerValue::Signed(-67)).expect("-67i8"),
                    signed_root.clone(),
                ),
                Proposition::LessOrEqual(
                    signed_root,
                    ScalarTerm::integer(i8_type, IntegerValue::Signed(60)).expect("60i8"),
                ),
            ]),
            "signed affine inversion uses ceiling for the lower and floor for the upper bound"
        );
    }

    #[test]
    fn exact_multiply_chain_handles_zero_one_and_accumulator_overflow() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let value = |id| {
            ScalarTerm::value(
                ValueId::new(id).expect("value"),
                ScalarType::Integer(u8_type),
            )
        };
        let root = value(1);
        let first = value(2);
        let second = value(3);
        let one = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(1)).expect("1u8");
        let two = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(2)).expect("2u8");
        let zero = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(0)).expect("0u8");
        let seven = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(7)).expect("7u8");
        let identity_axioms = vec![Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_multiply(u8_type, root.clone(), one.clone())
                .expect("root * 1"),
        )];
        assert_eq!(
            exact_integer_multiply_obligation_with_definitions(
                u8_type,
                first.clone(),
                one,
                &identity_axioms,
                identity_axioms.len(),
                &BTreeSet::from([ValueId::new(1).expect("root")]),
            ),
            Proposition::Truth
        );
        let axioms = vec![
            Proposition::Equal(
                first.clone(),
                ScalarTerm::exact_integer_multiply(u8_type, root, two).expect("root * 2"),
            ),
            Proposition::Equal(
                second.clone(),
                ScalarTerm::exact_integer_multiply(u8_type, first, zero).expect("first * 0"),
            ),
        ];
        assert_eq!(
            exact_integer_multiply_obligation_with_definitions(
                u8_type,
                second,
                seven,
                &axioms,
                axioms.len(),
                &BTreeSet::from([ValueId::new(1).expect("root")]),
            ),
            Proposition::Truth
        );

        let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
        let value = |id| {
            ScalarTerm::value(
                ValueId::new(id).expect("value"),
                ScalarType::Integer(u64_type),
            )
        };
        let root = value(10);
        let first = value(11);
        let second = value(12);
        let maximum = ScalarTerm::integer(u64_type, IntegerValue::Unsigned(u64::MAX.into()))
            .expect("u64::MAX");
        let axioms = vec![
            Proposition::Equal(
                first.clone(),
                ScalarTerm::exact_integer_multiply(u64_type, root, maximum.clone())
                    .expect("root * MAX"),
            ),
            Proposition::Equal(
                second.clone(),
                ScalarTerm::exact_integer_multiply(u64_type, first, maximum.clone())
                    .expect("first * MAX"),
            ),
        ];
        assert_eq!(
            exact_integer_multiply_obligation_with_definitions(
                u64_type,
                second,
                maximum,
                &axioms,
                axioms.len(),
                &BTreeSet::from([ValueId::new(10).expect("root")]),
            ),
            Proposition::Falsehood
        );
    }

    #[test]
    fn exact_multiply_chain_after_partial_cast_reconstructs_every_scaled_source_interval() {
        let source_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
        let target_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let root_id = ValueId::new(201).expect("root");
        let root = ScalarTerm::value(root_id, ScalarType::Integer(source_type));
        let cast = ScalarTerm::value(
            ValueId::new(202).expect("cast"),
            ScalarType::Integer(target_type),
        );
        let first = ScalarTerm::value(
            ValueId::new(203).expect("first product"),
            ScalarType::Integer(target_type),
        );
        let second = ScalarTerm::value(
            ValueId::new(204).expect("second product"),
            ScalarType::Integer(target_type),
        );
        let cast_definition = Proposition::Equal(
            cast.clone(),
            ScalarTerm::integer_exact_cast(source_type, target_type, root.clone())
                .expect("u16 to u8 exact cast"),
        );
        let two = ScalarTerm::integer(target_type, IntegerValue::Unsigned(2)).expect("2u8");
        let three = ScalarTerm::integer(target_type, IntegerValue::Unsigned(3)).expect("3u8");
        let first_definition = Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_multiply(target_type, cast.clone(), two.clone())
                .expect("cast * 2u8"),
        );
        let second_definition = Proposition::Equal(
            second.clone(),
            ScalarTerm::exact_integer_multiply(target_type, first.clone(), three.clone())
                .expect("(cast * 2u8) * 3u8"),
        );
        let parameters = BTreeSet::from([root_id]);
        assert_eq!(
            exact_integer_multiply_obligation_with_definitions(
                target_type,
                cast.clone(),
                two.clone(),
                std::slice::from_ref(&cast_definition),
                1,
                &parameters,
            ),
            Proposition::LessOrEqual(
                root.clone(),
                ScalarTerm::integer(source_type, IntegerValue::Unsigned(127)).expect("127u16"),
            )
        );
        assert_eq!(
            exact_integer_multiply_obligation_with_definitions(
                target_type,
                first.clone(),
                three,
                &[cast_definition.clone(), first_definition.clone()],
                2,
                &parameters,
            ),
            Proposition::LessOrEqual(
                root.clone(),
                ScalarTerm::integer(source_type, IntegerValue::Unsigned(42)).expect("42u16"),
            )
        );
        assert_eq!(
            exact_integer_multiply_obligation_with_definitions(
                target_type,
                second,
                ScalarTerm::integer(target_type, IntegerValue::Unsigned(0)).expect("0u8"),
                &[
                    cast_definition.clone(),
                    first_definition.clone(),
                    second_definition,
                ],
                3,
                &parameters,
            ),
            Proposition::Truth,
            "a zero factor makes only the current prefix total"
        );

        let signed_source = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
        let signed_target = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let signed_root_id = ValueId::new(211).expect("signed root");
        let signed_root = ScalarTerm::value(signed_root_id, ScalarType::Integer(signed_source));
        let signed_cast = ScalarTerm::value(
            ValueId::new(212).expect("signed cast"),
            ScalarType::Integer(signed_target),
        );
        let signed_first = ScalarTerm::value(
            ValueId::new(213).expect("signed first product"),
            ScalarType::Integer(signed_target),
        );
        let signed_cast_definition = Proposition::Equal(
            signed_cast.clone(),
            ScalarTerm::integer_exact_cast(signed_source, signed_target, signed_root.clone())
                .expect("i16 to i8 exact cast"),
        );
        let signed_first_definition = Proposition::Equal(
            signed_first.clone(),
            ScalarTerm::exact_integer_multiply(
                signed_target,
                signed_cast,
                ScalarTerm::integer(signed_target, IntegerValue::Signed(2)).expect("2i8"),
            )
            .expect("signed cast * 2i8"),
        );
        assert_eq!(
            exact_integer_multiply_obligation_with_definitions(
                signed_target,
                signed_first,
                ScalarTerm::integer(signed_target, IntegerValue::Signed(3)).expect("3i8"),
                &[signed_cast_definition, signed_first_definition],
                2,
                &BTreeSet::from([signed_root_id]),
            ),
            canonical_conjunction(vec![
                Proposition::LessOrEqual(
                    ScalarTerm::integer(signed_source, IntegerValue::Signed(-21)).expect("-21i16"),
                    signed_root.clone(),
                ),
                Proposition::LessOrEqual(
                    signed_root,
                    ScalarTerm::integer(signed_source, IntegerValue::Signed(21)).expect("21i16"),
                ),
            ])
        );

        let cross_source = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let cross_target = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let cross_root_id = ValueId::new(221).expect("cross root");
        let cross_root = ScalarTerm::value(cross_root_id, ScalarType::Integer(cross_source));
        let cross_cast = ScalarTerm::value(
            ValueId::new(222).expect("cross cast"),
            ScalarType::Integer(cross_target),
        );
        let cross_definition = Proposition::Equal(
            cross_cast.clone(),
            ScalarTerm::integer_exact_cast(cross_source, cross_target, cross_root.clone())
                .expect("i8 to u8 exact cast"),
        );
        assert_eq!(
            exact_integer_multiply_obligation_with_definitions(
                cross_target,
                cross_cast,
                ScalarTerm::integer(cross_target, IntegerValue::Unsigned(2)).expect("2u8"),
                std::slice::from_ref(&cross_definition),
                1,
                &BTreeSet::from([cross_root_id]),
            ),
            Proposition::LessOrEqual(
                ScalarTerm::integer(cross_source, IntegerValue::Signed(0)).expect("0i8"),
                cross_root,
            )
        );

        let reversed_cast_definition = match cast_definition.clone() {
            Proposition::Equal(left, right) => Proposition::Equal(right, left),
            _ => unreachable!("cast definition is an equality"),
        };
        assert_ne!(
            exact_integer_multiply_obligation_with_definitions(
                target_type,
                cast.clone(),
                two.clone(),
                std::slice::from_ref(&reversed_cast_definition),
                1,
                &parameters,
            ),
            Proposition::LessOrEqual(
                root.clone(),
                ScalarTerm::integer(source_type, IntegerValue::Unsigned(127)).expect("127u16"),
            )
        );
        assert_ne!(
            exact_integer_multiply_obligation_with_definitions(
                target_type,
                two,
                cast,
                std::slice::from_ref(&cast_definition),
                1,
                &parameters,
            ),
            Proposition::LessOrEqual(
                root,
                ScalarTerm::integer(source_type, IntegerValue::Unsigned(127)).expect("127u16"),
            ),
            "literal-left multiplication does not gain definition authority"
        );

        let wide_source = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
        let wide_target = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
        let wide_root_id = ValueId::new(231).expect("wide root");
        let wide_root = ScalarTerm::value(wide_root_id, ScalarType::Integer(wide_source));
        let wide_cast = ScalarTerm::value(
            ValueId::new(232).expect("wide cast"),
            ScalarType::Integer(wide_target),
        );
        let wide_first = ScalarTerm::value(
            ValueId::new(233).expect("wide first product"),
            ScalarType::Integer(wide_target),
        );
        let wide_second = ScalarTerm::value(
            ValueId::new(234).expect("wide second product"),
            ScalarType::Integer(wide_target),
        );
        let maximum =
            ScalarTerm::integer(wide_target, IntegerValue::Unsigned(u128::from(u64::MAX)))
                .expect("u64::MAX");
        let wide_definitions = [
            Proposition::Equal(
                wide_cast.clone(),
                ScalarTerm::integer_exact_cast(wide_source, wide_target, wide_root)
                    .expect("i64 to u64 exact cast"),
            ),
            Proposition::Equal(
                wide_first.clone(),
                ScalarTerm::exact_integer_multiply(wide_target, wide_cast, maximum.clone())
                    .expect("wide cast * MAX"),
            ),
            Proposition::Equal(
                wide_second.clone(),
                ScalarTerm::exact_integer_multiply(wide_target, wide_first, maximum.clone())
                    .expect("wide first * MAX"),
            ),
        ];
        assert_eq!(
            exact_integer_multiply_obligation_with_definitions(
                wide_target,
                wide_second,
                maximum,
                &wide_definitions,
                wide_definitions.len(),
                &BTreeSet::from([wide_root_id]),
            ),
            Proposition::Falsehood,
            "cumulative product overflow fails closed"
        );
    }

    #[test]
    fn mixed_affine_chain_after_partial_cast_reconstructs_each_prefix_independently() {
        let source_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
        let target_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let root_id = ValueId::new(701).expect("root");
        let root = ScalarTerm::value(root_id, ScalarType::Integer(source_type));
        let value = |id| {
            ScalarTerm::value(
                ValueId::new(id).expect("value"),
                ScalarType::Integer(target_type),
            )
        };
        let cast = value(702);
        let added = value(703);
        let multiplied = value(704);
        let definitions = vec![
            Proposition::Equal(
                cast.clone(),
                ScalarTerm::integer_exact_cast(source_type, target_type, root.clone())
                    .expect("u16 to u8 exact cast"),
            ),
            Proposition::Equal(
                added.clone(),
                ScalarTerm::exact_integer_add(
                    target_type,
                    cast.clone(),
                    ScalarTerm::integer(target_type, IntegerValue::Unsigned(3)).expect("3u8"),
                )
                .expect("cast + 3"),
            ),
            Proposition::Equal(
                multiplied.clone(),
                ScalarTerm::exact_integer_multiply(
                    target_type,
                    added.clone(),
                    ScalarTerm::integer(target_type, IntegerValue::Unsigned(2)).expect("2u8"),
                )
                .expect("added * 2"),
            ),
        ];
        let parameters = BTreeSet::from([root_id]);
        assert_eq!(
            exact_integer_multiply_obligation_with_definitions(
                target_type,
                added.clone(),
                ScalarTerm::integer(target_type, IntegerValue::Unsigned(2)).expect("2u8"),
                &definitions[..2],
                2,
                &parameters,
            ),
            Proposition::LessOrEqual(
                root.clone(),
                ScalarTerm::integer(source_type, IntegerValue::Unsigned(124)).expect("124u16"),
            ),
        );
        assert_eq!(
            exact_integer_subtract_obligation(
                target_type,
                multiplied,
                ScalarTerm::integer(target_type, IntegerValue::Unsigned(1)).expect("1u8"),
                &definitions,
                definitions.len(),
                &parameters,
            ),
            Proposition::LessOrEqual(
                root.clone(),
                ScalarTerm::integer(source_type, IntegerValue::Unsigned(125)).expect("125u16"),
            ),
        );
        assert_eq!(
            exact_integer_cast_then_affine_chain_obligation(
                target_type,
                added,
                IntegerValue::Unsigned(0),
                ExactIntegerAffineOperation::Multiply,
                &definitions[..2],
                2,
                &parameters,
            ),
            Some(Proposition::Truth),
            "zero collapse discharges only the current arithmetic prefix",
        );
        assert_eq!(
            exact_integer_cast_then_affine_chain_obligation(
                target_type,
                value(705),
                IntegerValue::Unsigned(1),
                ExactIntegerAffineOperation::Subtract,
                &[definitions[1].clone(), definitions[0].clone()],
                2,
                &parameters,
            ),
            None,
            "reordered or stale definitions cannot authorize the walk",
        );
    }

    #[test]
    fn affine_cast_affine_reconstructs_both_sides_without_importing_any_prefix_proof() {
        let source_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
        let target_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let root_id = ValueId::new(711).expect("root");
        let root = ScalarTerm::value(root_id, ScalarType::Integer(source_type));
        let source_add = ScalarTerm::value(
            ValueId::new(712).expect("source add"),
            ScalarType::Integer(source_type),
        );
        let source_multiply = ScalarTerm::value(
            ValueId::new(713).expect("source multiply"),
            ScalarType::Integer(source_type),
        );
        let cast = ScalarTerm::value(
            ValueId::new(714).expect("cast"),
            ScalarType::Integer(target_type),
        );
        let target_subtract = ScalarTerm::value(
            ValueId::new(715).expect("target subtract"),
            ScalarType::Integer(target_type),
        );
        let definitions = vec![
            Proposition::Equal(
                source_add.clone(),
                ScalarTerm::exact_integer_add(
                    source_type,
                    root.clone(),
                    ScalarTerm::integer(source_type, IntegerValue::Unsigned(3)).expect("3u16"),
                )
                .expect("root + 3"),
            ),
            Proposition::Equal(
                source_multiply.clone(),
                ScalarTerm::exact_integer_multiply(
                    source_type,
                    source_add,
                    ScalarTerm::integer(source_type, IntegerValue::Unsigned(2)).expect("2u16"),
                )
                .expect("(root + 3) * 2"),
            ),
            Proposition::Equal(
                cast.clone(),
                ScalarTerm::integer_exact_cast(source_type, target_type, source_multiply)
                    .expect("affine u16 to u8 cast"),
            ),
            Proposition::Equal(
                target_subtract.clone(),
                ScalarTerm::exact_integer_subtract(
                    target_type,
                    cast,
                    ScalarTerm::integer(target_type, IntegerValue::Unsigned(1)).expect("1u8"),
                )
                .expect("cast - 1"),
            ),
        ];
        assert_eq!(
            exact_integer_affine_cast_affine_obligation(
                target_type,
                target_subtract.clone(),
                IntegerValue::Unsigned(2),
                ExactIntegerAffineOperation::Multiply,
                &definitions,
                definitions.len(),
                &BTreeSet::from([root_id]),
            ),
            Some(Proposition::LessOrEqual(
                root.clone(),
                ScalarTerm::integer(source_type, IntegerValue::Unsigned(61)).expect("61u16"),
            )),
        );
        assert_eq!(
            exact_integer_affine_cast_affine_obligation(
                target_type,
                target_subtract,
                IntegerValue::Unsigned(2),
                ExactIntegerAffineOperation::Multiply,
                &[Proposition::Truth],
                1,
                &BTreeSet::from([root_id]),
            ),
            None,
            "stale definitions cannot authorize either side of the cast sandwich",
        );

        let source_zero = ScalarTerm::value(
            ValueId::new(716).expect("source zero"),
            ScalarType::Integer(source_type),
        );
        let zero_cast = ScalarTerm::value(
            ValueId::new(717).expect("zero cast"),
            ScalarType::Integer(target_type),
        );
        let source_zero_definitions = vec![
            Proposition::Equal(
                source_zero.clone(),
                ScalarTerm::exact_integer_multiply(
                    source_type,
                    root.clone(),
                    ScalarTerm::integer(source_type, IntegerValue::Unsigned(0)).expect("0u16"),
                )
                .expect("root * 0"),
            ),
            Proposition::Equal(
                zero_cast.clone(),
                ScalarTerm::integer_exact_cast(source_type, target_type, source_zero)
                    .expect("zero u16 to u8 cast"),
            ),
        ];
        assert_eq!(
            exact_integer_affine_cast_affine_obligation(
                target_type,
                zero_cast,
                IntegerValue::Unsigned(1),
                ExactIntegerAffineOperation::Add,
                &source_zero_definitions,
                source_zero_definitions.len(),
                &BTreeSet::from([root_id]),
            ),
            Some(Proposition::Truth),
            "a source-side zero decides only the current post-cast obligation",
        );

        let post_zero = ScalarTerm::value(
            ValueId::new(718).expect("post zero"),
            ScalarType::Integer(target_type),
        );
        let mut post_zero_definitions = definitions[..3].to_vec();
        post_zero_definitions.push(Proposition::Equal(
            post_zero.clone(),
            ScalarTerm::exact_integer_multiply(
                target_type,
                ScalarTerm::value(
                    ValueId::new(714).expect("cast"),
                    ScalarType::Integer(target_type),
                ),
                ScalarTerm::integer(target_type, IntegerValue::Unsigned(0)).expect("0u8"),
            )
            .expect("cast * 0"),
        ));
        assert_eq!(
            exact_integer_affine_cast_affine_obligation(
                target_type,
                post_zero,
                IntegerValue::Unsigned(255),
                ExactIntegerAffineOperation::Add,
                &post_zero_definitions,
                post_zero_definitions.len(),
                &BTreeSet::from([root_id]),
            ),
            Some(Proposition::Truth),
            "a target-side zero decides only the current obligation after full source validation",
        );
    }

    #[test]
    fn signed_affine_cast_affine_reverses_each_side_and_keeps_zero_local() {
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 type");
        let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16 type");
        let root_id = ValueId::new(1761).expect("source-negative root");
        let root = ScalarTerm::value(root_id, ScalarType::Integer(i16_type));
        let source_add = ScalarTerm::value(
            ValueId::new(1762).expect("source-negative add"),
            ScalarType::Integer(i16_type),
        );
        let source_negative = ScalarTerm::value(
            ValueId::new(1763).expect("source-negative product"),
            ScalarType::Integer(i16_type),
        );
        let cast = ScalarTerm::value(
            ValueId::new(1764).expect("source-negative cast"),
            ScalarType::Integer(i8_type),
        );
        let target_add = ScalarTerm::value(
            ValueId::new(1765).expect("source-negative target add"),
            ScalarType::Integer(i8_type),
        );
        let source_negative_definitions = vec![
            Proposition::Equal(
                source_add.clone(),
                ScalarTerm::exact_integer_add(
                    i16_type,
                    root.clone(),
                    ScalarTerm::integer(i16_type, IntegerValue::Signed(3)).expect("3i16"),
                )
                .expect("root + 3"),
            ),
            Proposition::Equal(
                source_negative.clone(),
                ScalarTerm::exact_integer_multiply(
                    i16_type,
                    source_add,
                    ScalarTerm::integer(i16_type, IntegerValue::Signed(-2)).expect("-2i16"),
                )
                .expect("source add * -2"),
            ),
            Proposition::Equal(
                cast.clone(),
                ScalarTerm::integer_exact_cast(i16_type, i8_type, source_negative)
                    .expect("signed affine i16 to i8 cast"),
            ),
            Proposition::Equal(
                target_add.clone(),
                ScalarTerm::exact_integer_add(
                    i8_type,
                    cast.clone(),
                    ScalarTerm::integer(i8_type, IntegerValue::Signed(1)).expect("1i8"),
                )
                .expect("cast + 1"),
            ),
        ];
        let source_negative_parameters = BTreeSet::from([root_id]);
        assert_eq!(
            exact_integer_signed_affine_cast_affine_obligation(
                i8_type,
                target_add.clone(),
                IntegerValue::Signed(2),
                ExactIntegerAffineOperation::Multiply,
                &source_negative_definitions,
                source_negative_definitions.len(),
                &source_negative_parameters,
            ),
            Some(exact_integer_source_interval_obligation(
                i16_type,
                root.clone(),
                -34,
                29,
            )),
            "a source-negative form reverses the target interval after cast intersection",
        );

        let target_negative_root_id = ValueId::new(1771).expect("target-negative root");
        let target_negative_root =
            ScalarTerm::value(target_negative_root_id, ScalarType::Integer(i16_type));
        let source_positive_add = ScalarTerm::value(
            ValueId::new(1772).expect("source-positive add"),
            ScalarType::Integer(i16_type),
        );
        let source_positive = ScalarTerm::value(
            ValueId::new(1773).expect("source-positive product"),
            ScalarType::Integer(i16_type),
        );
        let target_negative_cast = ScalarTerm::value(
            ValueId::new(1774).expect("target-negative cast"),
            ScalarType::Integer(i8_type),
        );
        let target_negative_add = ScalarTerm::value(
            ValueId::new(1775).expect("target-negative add"),
            ScalarType::Integer(i8_type),
        );
        let target_negative_definitions = vec![
            Proposition::Equal(
                source_positive_add.clone(),
                ScalarTerm::exact_integer_add(
                    i16_type,
                    target_negative_root.clone(),
                    ScalarTerm::integer(i16_type, IntegerValue::Signed(3)).expect("3i16"),
                )
                .expect("root + 3"),
            ),
            Proposition::Equal(
                source_positive.clone(),
                ScalarTerm::exact_integer_multiply(
                    i16_type,
                    source_positive_add,
                    ScalarTerm::integer(i16_type, IntegerValue::Signed(2)).expect("2i16"),
                )
                .expect("source add * 2"),
            ),
            Proposition::Equal(
                target_negative_cast.clone(),
                ScalarTerm::integer_exact_cast(i16_type, i8_type, source_positive)
                    .expect("positive affine i16 to i8 cast"),
            ),
            Proposition::Equal(
                target_negative_add.clone(),
                ScalarTerm::exact_integer_add(
                    i8_type,
                    target_negative_cast,
                    ScalarTerm::integer(i8_type, IntegerValue::Signed(3)).expect("3i8"),
                )
                .expect("cast + 3"),
            ),
        ];
        assert_eq!(
            exact_integer_signed_affine_cast_affine_obligation(
                i8_type,
                target_negative_add,
                IntegerValue::Signed(-2),
                ExactIntegerAffineOperation::Multiply,
                &target_negative_definitions,
                target_negative_definitions.len(),
                &BTreeSet::from([target_negative_root_id]),
            ),
            Some(exact_integer_source_interval_obligation(
                i16_type,
                target_negative_root,
                -36,
                27,
            )),
            "a target-negative form reverses before the source-positive inverse",
        );

        let target_zero = ScalarTerm::value(
            ValueId::new(1766).expect("target zero"),
            ScalarType::Integer(i8_type),
        );
        let mut zero_definitions = source_negative_definitions.clone();
        zero_definitions.push(Proposition::Equal(
            target_zero.clone(),
            ScalarTerm::exact_integer_multiply(
                i8_type,
                cast,
                ScalarTerm::integer(i8_type, IntegerValue::Signed(0)).expect("0i8"),
            )
            .expect("cast * 0"),
        ));
        assert_eq!(
            exact_integer_signed_affine_cast_affine_obligation(
                i8_type,
                target_zero.clone(),
                IntegerValue::Signed(127),
                ExactIntegerAffineOperation::Add,
                &zero_definitions,
                zero_definitions.len(),
                &source_negative_parameters,
            ),
            Some(Proposition::Truth),
            "a target zero decides only its current representable constant",
        );
        assert_eq!(
            exact_integer_signed_affine_cast_affine_obligation(
                i8_type,
                target_zero,
                IntegerValue::Signed(-128),
                ExactIntegerAffineOperation::Subtract,
                &zero_definitions,
                zero_definitions.len(),
                &source_negative_parameters,
            ),
            Some(Proposition::Falsehood),
            "an unrepresentable target constant is mathematical falsehood",
        );
        assert_eq!(
            exact_integer_signed_affine_cast_affine_obligation(
                i8_type,
                target_add,
                IntegerValue::Signed(2),
                ExactIntegerAffineOperation::Multiply,
                &[Proposition::Truth],
                1,
                &source_negative_parameters,
            ),
            None,
            "stale definitions cannot authorize either sign-reversing side",
        );

        let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64 type");
        let minimum_root_id = ValueId::new(1781).expect("MIN sandwich root");
        let minimum_root = ScalarTerm::value(minimum_root_id, ScalarType::Integer(i64_type));
        let minimum_offset = ScalarTerm::value(
            ValueId::new(1782).expect("MIN sandwich offset"),
            ScalarType::Integer(i64_type),
        );
        let minimum_product = ScalarTerm::value(
            ValueId::new(1783).expect("MIN sandwich product"),
            ScalarType::Integer(i64_type),
        );
        let minimum_cast = ScalarTerm::value(
            ValueId::new(1784).expect("MIN sandwich cast"),
            ScalarType::Integer(i8_type),
        );
        let minimum_definitions = vec![
            Proposition::Equal(
                minimum_offset.clone(),
                ScalarTerm::exact_integer_add(
                    i64_type,
                    minimum_root.clone(),
                    ScalarTerm::integer(i64_type, IntegerValue::Signed(0)).expect("0i64"),
                )
                .expect("MIN root + 0"),
            ),
            Proposition::Equal(
                minimum_product.clone(),
                ScalarTerm::exact_integer_multiply(
                    i64_type,
                    minimum_offset,
                    ScalarTerm::integer(i64_type, IntegerValue::Signed(i64::MIN.into()))
                        .expect("MIN i64"),
                )
                .expect("root * MIN"),
            ),
            Proposition::Equal(
                minimum_cast.clone(),
                ScalarTerm::integer_exact_cast(i64_type, i8_type, minimum_product)
                    .expect("MIN product to i8 cast"),
            ),
        ];
        assert_eq!(
            exact_integer_signed_affine_cast_affine_obligation(
                i8_type,
                minimum_cast,
                IntegerValue::Signed(0),
                ExactIntegerAffineOperation::Add,
                &minimum_definitions,
                minimum_definitions.len(),
                &BTreeSet::from([minimum_root_id]),
            ),
            Some(exact_integer_source_interval_obligation(
                i64_type,
                minimum_root,
                0,
                0,
            )),
            "MIN is accumulated by magnitude without host negation",
        );

        let overflow_first = ScalarTerm::value(
            ValueId::new(1785).expect("overflow first"),
            ScalarType::Integer(i64_type),
        );
        let overflow_second = ScalarTerm::value(
            ValueId::new(1786).expect("overflow second"),
            ScalarType::Integer(i64_type),
        );
        let overflow_third = ScalarTerm::value(
            ValueId::new(1787).expect("overflow third"),
            ScalarType::Integer(i64_type),
        );
        let overflow_cast = ScalarTerm::value(
            ValueId::new(1788).expect("overflow cast"),
            ScalarType::Integer(i8_type),
        );
        let mut overflow_definitions = minimum_definitions[..1].to_vec();
        overflow_definitions.extend([
            Proposition::Equal(
                overflow_first.clone(),
                ScalarTerm::exact_integer_multiply(
                    i64_type,
                    ScalarTerm::value(
                        ValueId::new(1782).expect("MIN sandwich offset"),
                        ScalarType::Integer(i64_type),
                    ),
                    ScalarTerm::integer(i64_type, IntegerValue::Signed(i64::MIN.into()))
                        .expect("MIN i64"),
                )
                .expect("offset * MIN"),
            ),
            Proposition::Equal(
                overflow_second.clone(),
                ScalarTerm::exact_integer_multiply(
                    i64_type,
                    overflow_first,
                    ScalarTerm::integer(i64_type, IntegerValue::Signed(i64::MIN.into()))
                        .expect("MIN i64"),
                )
                .expect("first * MIN"),
            ),
            Proposition::Equal(
                overflow_third.clone(),
                ScalarTerm::exact_integer_multiply(
                    i64_type,
                    overflow_second,
                    ScalarTerm::integer(i64_type, IntegerValue::Signed(4)).expect("4i64"),
                )
                .expect("second * 4"),
            ),
            Proposition::Equal(
                overflow_cast.clone(),
                ScalarTerm::integer_exact_cast(i64_type, i8_type, overflow_third)
                    .expect("overflow product to i8 cast"),
            ),
        ]);
        assert_eq!(
            exact_integer_signed_affine_cast_affine_obligation(
                i8_type,
                overflow_cast,
                IntegerValue::Signed(1),
                ExactIntegerAffineOperation::Add,
                &overflow_definitions,
                overflow_definitions.len(),
                &BTreeSet::from([minimum_root_id]),
            ),
            None,
            "checked coefficient overflow admits no sandwich family",
        );
    }

    #[test]
    fn affine_fork_join_replays_correlated_branches_without_importing_prefix_proofs() {
        let integer_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16 type");
        let root_id = ValueId::new(1791).expect("fork root");
        let root = ScalarTerm::value(root_id, ScalarType::Integer(integer_type));
        let value = |id| {
            ScalarTerm::value(
                ValueId::new(id).expect("fork value"),
                ScalarType::Integer(integer_type),
            )
        };
        let literal = |value| {
            ScalarTerm::integer(integer_type, IntegerValue::Signed(value)).expect("fork literal")
        };
        let left_offset = value(1792);
        let left_product = value(1793);
        let right_offset = value(1794);
        let right_product = value(1795);
        let definitions = vec![
            Proposition::Equal(
                left_offset.clone(),
                ScalarTerm::exact_integer_add(integer_type, root.clone(), literal(1))
                    .expect("root + 1"),
            ),
            Proposition::Equal(
                left_product.clone(),
                ScalarTerm::exact_integer_multiply(integer_type, left_offset, literal(2))
                    .expect("left * 2"),
            ),
            Proposition::Equal(
                right_offset.clone(),
                ScalarTerm::exact_integer_subtract(integer_type, root.clone(), literal(1))
                    .expect("root - 1"),
            ),
            Proposition::Equal(
                right_product.clone(),
                ScalarTerm::exact_integer_multiply(integer_type, right_offset, literal(3))
                    .expect("right * 3"),
            ),
        ];
        let parameters = BTreeSet::from([root_id]);
        let expected =
            exact_integer_source_interval_obligation(integer_type, root.clone(), -6553, 6553);
        assert_eq!(
            exact_integer_affine_fork_join_obligation(
                integer_type,
                left_product.clone(),
                right_product.clone(),
                ExactIntegerOffsetOperation::Add,
                &definitions,
                definitions.len(),
                &parameters,
            ),
            Some(expected.clone()),
            "2 * (x + 1) + 3 * (x - 1) replays as 5 * x - 1",
        );
        assert_eq!(
            exact_integer_add_obligation(
                integer_type,
                left_product.clone(),
                right_product.clone(),
                &definitions,
                definitions.len(),
                &parameters,
            ),
            expected,
            "the ordinary exact-add dispatch selects the correlated fork",
        );

        let cancel_left_offset = value(1796);
        let cancel_left_product = value(1797);
        let cancel_right_offset = value(1798);
        let cancel_right_product = value(1799);
        let cancellation_definitions = vec![
            Proposition::Equal(
                cancel_left_offset.clone(),
                ScalarTerm::exact_integer_add(integer_type, root.clone(), literal(3))
                    .expect("root + 3"),
            ),
            Proposition::Equal(
                cancel_left_product.clone(),
                ScalarTerm::exact_integer_multiply(integer_type, cancel_left_offset, literal(-2))
                    .expect("left * -2"),
            ),
            Proposition::Equal(
                cancel_right_offset.clone(),
                ScalarTerm::exact_integer_subtract(integer_type, root.clone(), literal(4))
                    .expect("root - 4"),
            ),
            Proposition::Equal(
                cancel_right_product.clone(),
                ScalarTerm::exact_integer_multiply(integer_type, cancel_right_offset, literal(-2))
                    .expect("right * -2"),
            ),
        ];
        assert_eq!(
            exact_integer_subtract_obligation(
                integer_type,
                cancel_left_product.clone(),
                cancel_right_product.clone(),
                &cancellation_definitions,
                cancellation_definitions.len(),
                &parameters,
            ),
            Proposition::Truth,
            "-2 * (x + 3) - -2 * (x - 4) is the join-local constant -14",
        );

        assert_eq!(
            exact_integer_affine_fork_join_obligation(
                integer_type,
                left_product.clone(),
                left_product,
                ExactIntegerOffsetOperation::Add,
                &definitions,
                definitions.len(),
                &parameters,
            ),
            None,
            "the two branch definition walks must be disjoint",
        );
        let reordered_definitions = vec![
            definitions[2].clone(),
            definitions[3].clone(),
            definitions[0].clone(),
            definitions[1].clone(),
        ];
        assert_eq!(
            exact_integer_affine_fork_join_obligation(
                integer_type,
                value(1793),
                value(1795),
                ExactIntegerOffsetOperation::Add,
                &reordered_definitions,
                reordered_definitions.len(),
                &parameters,
            ),
            None,
            "right-branch definitions cannot precede the ordered left branch",
        );
        assert_eq!(
            exact_integer_affine_fork_join_obligation(
                integer_type,
                cancel_left_product,
                cancel_right_product,
                ExactIntegerOffsetOperation::Subtract,
                &cancellation_definitions,
                cancellation_definitions.len(),
                &BTreeSet::new(),
            ),
            None,
            "a local or stale root cannot authorize the fork",
        );
    }

    #[test]
    fn distinct_root_affine_fork_join_uses_only_canonical_signature_rectangles() {
        let integer_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16 type");
        let left_root_id = ValueId::new(1801).expect("left root");
        let right_root_id = ValueId::new(1802).expect("right root");
        let left_root = ScalarTerm::value(left_root_id, ScalarType::Integer(integer_type));
        let right_root = ScalarTerm::value(right_root_id, ScalarType::Integer(integer_type));
        let value = |id| {
            ScalarTerm::value(
                ValueId::new(id).expect("fork value"),
                ScalarType::Integer(integer_type),
            )
        };
        let literal = |value| {
            ScalarTerm::integer(integer_type, IntegerValue::Signed(value)).expect("fork literal")
        };
        let left_offset = value(1803);
        let left_product = value(1804);
        let right_offset = value(1805);
        let right_product = value(1806);
        let definitions = vec![
            Proposition::Equal(
                left_offset.clone(),
                ScalarTerm::exact_integer_add(integer_type, left_root.clone(), literal(1))
                    .expect("left root + 1"),
            ),
            Proposition::Equal(
                left_product.clone(),
                ScalarTerm::exact_integer_multiply(integer_type, left_offset, literal(2))
                    .expect("left branch * 2"),
            ),
            Proposition::Equal(
                right_offset.clone(),
                ScalarTerm::exact_integer_subtract(integer_type, right_root.clone(), literal(1))
                    .expect("right root - 1"),
            ),
            Proposition::Equal(
                right_product.clone(),
                ScalarTerm::exact_integer_multiply(integer_type, right_offset, literal(3))
                    .expect("right branch * 3"),
            ),
        ];
        let parameters = BTreeSet::from([left_root_id, right_root_id]);
        let loose_left = Proposition::LessOrEqual(literal(-200), left_root.clone());
        let left_lower = Proposition::LessOrEqual(literal(-100), left_root.clone());
        let left_upper = Proposition::LessOrEqual(left_root.clone(), literal(100));
        let right_lower = Proposition::LessOrEqual(literal(-100), right_root.clone());
        let right_upper = Proposition::LessOrEqual(right_root.clone(), literal(100));
        let mut bounded = definitions.clone();
        bounded.extend([
            loose_left,
            left_lower.clone(),
            left_upper.clone(),
            right_lower.clone(),
            right_upper.clone(),
        ]);
        let expected =
            canonical_conjunction(vec![left_lower, left_upper, right_lower, right_upper]);
        assert_eq!(
            exact_integer_distinct_root_affine_fork_join_obligation(
                integer_type,
                left_product.clone(),
                right_product.clone(),
                ExactIntegerOffsetOperation::Add,
                &bounded,
                definitions.len(),
                &parameters,
            ),
            Some(expected.clone()),
            "the tightest unary signature rectangle proves the bivariate sum",
        );
        assert_eq!(
            exact_integer_add_obligation(
                integer_type,
                left_product.clone(),
                right_product.clone(),
                &bounded,
                definitions.len(),
                &parameters,
            ),
            expected,
            "ordinary dispatch selects the distinct-root rectangle after same-root priority",
        );
        assert!(
            exact_integer_distinct_root_affine_fork_join_obligation(
                integer_type,
                left_product.clone(),
                right_product.clone(),
                ExactIntegerOffsetOperation::Subtract,
                &bounded,
                definitions.len(),
                &parameters,
            )
            .is_some(),
            "Minkowski subtraction reverses the right interval endpoints",
        );

        assert_eq!(
            exact_integer_distinct_root_affine_fork_join_obligation(
                integer_type,
                left_product.clone(),
                right_product.clone(),
                ExactIntegerOffsetOperation::Add,
                &definitions,
                definitions.len(),
                &parameters,
            ),
            None,
            "carrier defaults alone leave this output rectangle partially unsafe",
        );
        let relational = Proposition::LessOrEqual(
            left_root.clone(),
            ScalarTerm::exact_integer_subtract(
                integer_type,
                literal(i128::from(i16::MAX)),
                right_root.clone(),
            )
            .expect("MAX - right root"),
        );
        let mut relational_only = definitions.clone();
        relational_only.push(relational);
        assert_eq!(
            exact_integer_distinct_root_affine_fork_join_obligation(
                integer_type,
                left_product.clone(),
                right_product.clone(),
                ExactIntegerOffsetOperation::Add,
                &relational_only,
                definitions.len(),
                &parameters,
            ),
            None,
            "cross-root relational premises require a future polyhedral family",
        );
        assert_eq!(
            exact_integer_distinct_root_affine_fork_join_obligation(
                integer_type,
                left_product.clone(),
                left_product,
                ExactIntegerOffsetOperation::Add,
                &bounded,
                definitions.len(),
                &parameters,
            ),
            None,
            "same-root and overlapping walks retain the existing family priority",
        );
        let reordered = vec![
            definitions[2].clone(),
            definitions[3].clone(),
            definitions[0].clone(),
            definitions[1].clone(),
        ];
        assert_eq!(
            exact_integer_distinct_root_affine_fork_join_obligation(
                integer_type,
                value(1804),
                value(1806),
                ExactIntegerOffsetOperation::Add,
                &reordered,
                reordered.len(),
                &parameters,
            ),
            None,
            "right-branch definitions cannot precede the left branch",
        );
    }

    #[test]
    fn exact_multiply_fails_closed_without_a_known_factor() {
        let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let value = |id| {
            ScalarTerm::value(
                ValueId::new(id).expect("value"),
                ScalarType::Integer(integer_type),
            )
        };
        assert_eq!(
            exact_integer_multiply_obligation(integer_type, value(1), value(2), &[],),
            Proposition::Falsehood
        );
        let fifty_one =
            ScalarTerm::integer(integer_type, IntegerValue::Unsigned(51)).expect("51u8");
        let five = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(5)).expect("5u8");
        assert_eq!(
            exact_integer_multiply_obligation(integer_type, fifty_one, five, &[],),
            Proposition::Truth
        );
    }

    #[test]
    fn reconstructs_unsigned_joint_exact_multiply_bounds() {
        let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let left = ScalarTerm::value(
            ValueId::new(14).expect("left"),
            ScalarType::Integer(integer_type),
        );
        let right = ScalarTerm::value(
            ValueId::new(15).expect("right"),
            ScalarType::Integer(integer_type),
        );
        let one = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(1)).expect("1u8");
        let maximum =
            ScalarTerm::integer(integer_type, IntegerValue::Unsigned(255)).expect("255u8");
        let upper = ScalarTerm::exact_integer_divide(integer_type, maximum, right.clone())
            .expect("255 / right");
        let positive = Proposition::LessOrEqual(one, right.clone());
        let bound = Proposition::LessOrEqual(left.clone(), upper);
        let axioms = vec![positive.clone(), bound.clone()];
        assert_eq!(
            exact_integer_multiply_obligation(integer_type, left.clone(), right.clone(), &axioms,),
            canonical_conjunction(vec![positive.clone(), bound.clone()])
        );
        assert_eq!(
            exact_integer_multiply_obligation(integer_type, left, right, &axioms[1..],),
            Proposition::Falsehood
        );
    }

    #[test]
    fn reconstructs_signed_positive_joint_exact_multiply_bounds() {
        let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let left = ScalarTerm::value(
            ValueId::new(16).expect("left"),
            ScalarType::Integer(integer_type),
        );
        let right = ScalarTerm::value(
            ValueId::new(17).expect("right"),
            ScalarType::Integer(integer_type),
        );
        let one = ScalarTerm::integer(integer_type, IntegerValue::Signed(1)).expect("1i8");
        let minimum =
            ScalarTerm::integer(integer_type, IntegerValue::Signed(-128)).expect("-128i8");
        let maximum = ScalarTerm::integer(integer_type, IntegerValue::Signed(127)).expect("127i8");
        let lower = ScalarTerm::exact_integer_divide(integer_type, minimum, right.clone())
            .expect("-128 / right");
        let upper = ScalarTerm::exact_integer_divide(integer_type, maximum, right.clone())
            .expect("127 / right");
        let positive = Proposition::LessOrEqual(one, right.clone());
        let lower_bound = Proposition::LessOrEqual(lower, left.clone());
        let upper_bound = Proposition::LessOrEqual(left.clone(), upper);
        let axioms = vec![positive.clone(), lower_bound.clone(), upper_bound.clone()];
        assert_eq!(
            exact_integer_multiply_obligation(integer_type, left.clone(), right.clone(), &axioms,),
            canonical_conjunction(vec![
                positive.clone(),
                lower_bound.clone(),
                upper_bound.clone(),
            ])
        );
        assert_eq!(
            exact_integer_multiply_obligation(integer_type, left, right, &axioms[..2],),
            Proposition::Falsehood
        );
    }

    #[test]
    fn reconstructs_signed_negative_joint_exact_multiply_bounds() {
        let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let left = ScalarTerm::value(
            ValueId::new(18).expect("left"),
            ScalarType::Integer(integer_type),
        );
        let right = ScalarTerm::value(
            ValueId::new(19).expect("right"),
            ScalarType::Integer(integer_type),
        );
        let negative_two =
            ScalarTerm::integer(integer_type, IntegerValue::Signed(-2)).expect("-2i8");
        let minimum =
            ScalarTerm::integer(integer_type, IntegerValue::Signed(-128)).expect("-128i8");
        let maximum = ScalarTerm::integer(integer_type, IntegerValue::Signed(127)).expect("127i8");
        let lower = ScalarTerm::exact_integer_divide(integer_type, maximum, right.clone())
            .expect("127 / right");
        let upper = ScalarTerm::exact_integer_divide(integer_type, minimum, right.clone())
            .expect("-128 / right");
        let negative = Proposition::LessOrEqual(right.clone(), negative_two);
        let lower_bound = Proposition::LessOrEqual(lower, left.clone());
        let upper_bound = Proposition::LessOrEqual(left.clone(), upper);
        let axioms = vec![negative.clone(), lower_bound.clone(), upper_bound.clone()];
        assert_eq!(
            exact_integer_multiply_obligation(integer_type, left.clone(), right.clone(), &axioms,),
            canonical_conjunction(vec![
                negative.clone(),
                lower_bound.clone(),
                upper_bound.clone(),
            ])
        );
        assert_eq!(
            exact_integer_multiply_obligation(integer_type, left, right, &axioms[..2],),
            Proposition::Falsehood
        );
    }

    #[test]
    fn exact_divide_reconstructs_known_divisor_safety() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let value = ScalarTerm::value(
            ValueId::new(1).expect("value"),
            ScalarType::Integer(u8_type),
        );
        let five = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(5)).expect("5u8");
        assert_eq!(
            exact_integer_divide_obligation(u8_type, value.clone(), five, &[]),
            Proposition::Truth
        );
        let zero = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(0)).expect("0u8");
        assert_eq!(
            exact_integer_divide_obligation(u8_type, value, zero, &[]),
            Proposition::Falsehood
        );

        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let value = ScalarTerm::value(
            ValueId::new(2).expect("value"),
            ScalarType::Integer(i8_type),
        );
        let negative_one = ScalarTerm::integer(i8_type, IntegerValue::Signed(-1)).expect("-1i8");
        let minimum_plus_one =
            ScalarTerm::integer(i8_type, IntegerValue::Signed(-127)).expect("-127i8");
        assert_eq!(
            exact_integer_divide_obligation(i8_type, value.clone(), negative_one, &[]),
            Proposition::LessOrEqual(minimum_plus_one, value)
        );
        let unknown = ScalarTerm::value(
            ValueId::new(3).expect("divisor"),
            ScalarType::Integer(i8_type),
        );
        let one = ScalarTerm::integer(i8_type, IntegerValue::Signed(1)).unwrap();
        assert_eq!(
            exact_integer_divide_obligation(i8_type, one.clone(), unknown.clone(), &[]),
            Proposition::LessOrEqual(one, unknown)
        );
    }

    #[test]
    fn exact_remainder_reconstructs_known_divisor_safety() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let value = ScalarTerm::value(
            ValueId::new(1).expect("value"),
            ScalarType::Integer(u8_type),
        );
        let five = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(5)).expect("5u8");
        assert_eq!(
            exact_integer_remainder_obligation(u8_type, value.clone(), five, &[]),
            Proposition::Truth
        );
        let zero = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(0)).expect("0u8");
        assert_eq!(
            exact_integer_remainder_obligation(u8_type, value, zero, &[]),
            Proposition::Falsehood
        );

        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let value = ScalarTerm::value(
            ValueId::new(2).expect("value"),
            ScalarType::Integer(i8_type),
        );
        let negative_one = ScalarTerm::integer(i8_type, IntegerValue::Signed(-1)).expect("-1i8");
        let minimum_plus_one =
            ScalarTerm::integer(i8_type, IntegerValue::Signed(-127)).expect("-127i8");
        assert_eq!(
            exact_integer_remainder_obligation(i8_type, value.clone(), negative_one, &[]),
            Proposition::LessOrEqual(minimum_plus_one, value)
        );
        let unknown = ScalarTerm::value(
            ValueId::new(3).expect("divisor"),
            ScalarType::Integer(i8_type),
        );
        let one = ScalarTerm::integer(i8_type, IntegerValue::Signed(1)).unwrap();
        assert_eq!(
            exact_integer_remainder_obligation(i8_type, one.clone(), unknown.clone(), &[]),
            Proposition::LessOrEqual(one, unknown)
        );
    }

    #[test]
    fn mixed_exact_divide_remainder_chain_reconstructs_each_safe_divisor_independently() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let root = ScalarTerm::value(ValueId::new(1).expect("root"), ScalarType::Integer(u8_type));
        let two = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(2)).expect("2u8");
        let three = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(3)).expect("3u8");
        let inner =
            ScalarTerm::exact_integer_divide(u8_type, root, two.clone()).expect("root / 2u8");
        assert_eq!(
            exact_integer_remainder_obligation(u8_type, inner.clone(), three.clone(), &[]),
            Proposition::Truth
        );
        let middle =
            ScalarTerm::exact_integer_remainder(u8_type, inner, three).expect("(root / 2u8) % 3u8");
        assert_eq!(
            exact_integer_divide_obligation(u8_type, middle, two, &[]),
            Proposition::Truth
        );
    }

    #[test]
    fn runtime_divisor_chains_reconstruct_each_parameter_guard_independently() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let root = ScalarTerm::value(
            ValueId::new(21).expect("root"),
            ScalarType::Integer(u8_type),
        );
        let positive_divisor = ScalarTerm::value(
            ValueId::new(22).expect("positive divisor"),
            ScalarType::Integer(u8_type),
        );
        let two = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(2)).expect("2u8");
        let positive_bound = Proposition::LessOrEqual(
            ScalarTerm::integer(u8_type, IntegerValue::Unsigned(1)).expect("1u8"),
            positive_divisor.clone(),
        );
        let first = ScalarTerm::exact_integer_divide(u8_type, root, positive_divisor.clone())
            .expect("root / divisor");
        assert_eq!(
            exact_integer_divide_obligation(
                u8_type,
                first.clone(),
                positive_divisor,
                std::slice::from_ref(&positive_bound),
            ),
            positive_bound,
            "the nested dividend supplies no authority beyond the direct divisor guard",
        );
        assert_eq!(
            exact_integer_remainder_obligation(u8_type, first, two, &[]),
            Proposition::Truth,
        );

        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let signed_root = ScalarTerm::value(
            ValueId::new(23).expect("signed root"),
            ScalarType::Integer(i8_type),
        );
        let negative_divisor = ScalarTerm::value(
            ValueId::new(24).expect("negative divisor"),
            ScalarType::Integer(i8_type),
        );
        let negative_bound = Proposition::LessOrEqual(
            negative_divisor.clone(),
            ScalarTerm::integer(i8_type, IntegerValue::Signed(-2)).expect("-2i8"),
        );
        let signed_first = ScalarTerm::exact_integer_remainder(
            i8_type,
            signed_root.clone(),
            negative_divisor.clone(),
        )
        .expect("signed root % negative divisor");
        assert_eq!(
            exact_integer_remainder_obligation(
                i8_type,
                signed_first,
                negative_divisor.clone(),
                std::slice::from_ref(&negative_bound),
            ),
            negative_bound,
        );

        let negative_one_bound = Proposition::LessOrEqual(
            negative_divisor.clone(),
            ScalarTerm::integer(i8_type, IntegerValue::Signed(-1)).expect("-1i8"),
        );
        let minimum_plus_one_bound = Proposition::LessOrEqual(
            ScalarTerm::integer(i8_type, IntegerValue::Signed(-127)).expect("-127i8"),
            signed_root.clone(),
        );
        assert_eq!(
            exact_integer_divide_obligation(
                i8_type,
                signed_root.clone(),
                negative_divisor.clone(),
                &[negative_one_bound.clone(), minimum_plus_one_bound.clone()],
            ),
            canonical_conjunction(vec![
                negative_one_bound.clone(),
                minimum_plus_one_bound.clone(),
            ]),
            "the joint -1 exception remains valid for the independently bounded direct root",
        );
        let computed_dividend = ScalarTerm::exact_integer_divide(
            i8_type,
            signed_root,
            ScalarTerm::integer(i8_type, IntegerValue::Signed(2)).expect("2i8"),
        )
        .expect("computed dividend");
        let fallback_positive_requirement = Proposition::LessOrEqual(
            ScalarTerm::integer(i8_type, IntegerValue::Signed(1)).expect("1i8"),
            negative_divisor.clone(),
        );
        assert_eq!(
            exact_integer_divide_obligation(
                i8_type,
                computed_dividend,
                negative_divisor,
                &[negative_one_bound, minimum_plus_one_bound],
            ),
            fallback_positive_requirement,
            "a direct-root bound is never imported as a computed-prefix dividend bound",
        );
    }

    #[test]
    fn exact_shift_right_chain_counts_reconstruct_without_value_definitions() {
        let value_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 value");
        let signed_count_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 count");
        let unsigned_count_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 count");
        let one = ScalarTerm::integer(signed_count_type, IntegerValue::Signed(1)).expect("1i8");
        let two =
            ScalarTerm::integer(unsigned_count_type, IntegerValue::Unsigned(2)).expect("2u16");
        assert_eq!(
            exact_integer_shift_obligation(value_type, signed_count_type, one, &[]),
            Proposition::Truth
        );
        assert_eq!(
            exact_integer_shift_obligation(value_type, unsigned_count_type, two, &[]),
            Proposition::Truth
        );
        let negative_one =
            ScalarTerm::integer(signed_count_type, IntegerValue::Signed(-1)).expect("-1i8");
        let eight =
            ScalarTerm::integer(unsigned_count_type, IntegerValue::Unsigned(8)).expect("8u16");
        assert_eq!(
            exact_integer_shift_obligation(value_type, signed_count_type, negative_one, &[]),
            Proposition::Falsehood
        );
        assert_eq!(
            exact_integer_shift_obligation(value_type, unsigned_count_type, eight, &[]),
            Proposition::Falsehood
        );
    }

    #[test]
    fn exact_mixed_shift_chain_reconstructs_alternating_prefixes_from_ordered_definitions() {
        let value_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 value");
        let signed_count_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 count");
        let unsigned_count_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 count");
        let root_id = ValueId::new(301).expect("root");
        let root = ScalarTerm::value(root_id, ScalarType::Integer(value_type));
        let left_one = ScalarTerm::value(
            ValueId::new(302).expect("left one"),
            ScalarType::Integer(value_type),
        );
        let right_two = ScalarTerm::value(
            ValueId::new(303).expect("right two"),
            ScalarType::Integer(value_type),
        );
        let left_three = ScalarTerm::value(
            ValueId::new(304).expect("left three"),
            ScalarType::Integer(value_type),
        );
        let one = ScalarTerm::integer(signed_count_type, IntegerValue::Signed(1)).expect("1i8");
        let two =
            ScalarTerm::integer(unsigned_count_type, IntegerValue::Unsigned(2)).expect("2u16");
        let three = ScalarTerm::integer(signed_count_type, IntegerValue::Signed(3)).expect("3i8");
        let definitions = vec![
            Proposition::Equal(
                left_one.clone(),
                ScalarTerm::exact_integer_shift_left(
                    value_type,
                    signed_count_type,
                    root.clone(),
                    one.clone(),
                )
                .expect("root << 1"),
            ),
            Proposition::Equal(
                right_two.clone(),
                ScalarTerm::exact_integer_shift_right(
                    value_type,
                    unsigned_count_type,
                    left_one,
                    two,
                )
                .expect("(root << 1) >> 2"),
            ),
            Proposition::Equal(
                left_three.clone(),
                ScalarTerm::exact_integer_shift_left(
                    value_type,
                    signed_count_type,
                    right_two,
                    three,
                )
                .expect("((root << 1) >> 2) << 3"),
            ),
        ];
        let maximum = ScalarTerm::integer(value_type, IntegerValue::Unsigned(31)).expect("31u8");
        assert_eq!(
            exact_integer_mixed_shift_chain_obligation(
                value_type,
                left_three,
                1,
                &definitions,
                definitions.len(),
                &BTreeSet::from([root_id]),
            ),
            Some(Proposition::LessOrEqual(root.clone(), maximum)),
            "each alternating definition is replayed backward before the final left prefix",
        );

        let right = ScalarTerm::value(
            ValueId::new(305).expect("right"),
            ScalarType::Integer(value_type),
        );
        let one_right_definition = vec![Proposition::Equal(
            right.clone(),
            ScalarTerm::exact_integer_shift_right(
                value_type,
                signed_count_type,
                root.clone(),
                one.clone(),
            )
            .expect("root >> 1"),
        )];
        let maximum = ScalarTerm::integer(value_type, IntegerValue::Unsigned(31)).expect("31u8");
        assert_eq!(
            exact_integer_mixed_shift_chain_obligation(
                value_type,
                right.clone(),
                4,
                &one_right_definition,
                one_right_definition.len(),
                &BTreeSet::from([root_id]),
            ),
            Some(Proposition::LessOrEqual(root.clone(), maximum)),
        );
        assert_eq!(
            exact_integer_mixed_shift_chain_obligation(
                value_type,
                right,
                4,
                &one_right_definition,
                one_right_definition.len(),
                &BTreeSet::new(),
            ),
            None,
            "a local or unregistered root cannot acquire machine-parameter bounds",
        );
    }

    #[test]
    fn exact_mixed_shift_chain_handles_signed_preimages_and_stale_definitions() {
        let signed_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let count_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 count");
        let root_id = ValueId::new(311).expect("signed root");
        let signed_root = ScalarTerm::value(root_id, ScalarType::Integer(signed_type));
        let right = ScalarTerm::value(
            ValueId::new(312).expect("signed right"),
            ScalarType::Integer(signed_type),
        );
        let one = ScalarTerm::integer(count_type, IntegerValue::Unsigned(1)).expect("1u8");
        let definitions = vec![Proposition::Equal(
            right.clone(),
            ScalarTerm::exact_integer_shift_right(
                signed_type,
                count_type,
                signed_root.clone(),
                one,
            )
            .expect("signed root >> 1"),
        )];
        let minimum = ScalarTerm::integer(signed_type, IntegerValue::Signed(-32)).expect("-32i8");
        let maximum = ScalarTerm::integer(signed_type, IntegerValue::Signed(31)).expect("31i8");
        assert_eq!(
            exact_integer_mixed_shift_chain_obligation(
                signed_type,
                right.clone(),
                3,
                &definitions,
                definitions.len(),
                &BTreeSet::from([root_id]),
            ),
            Some(canonical_conjunction(vec![
                Proposition::LessOrEqual(minimum, signed_root.clone()),
                Proposition::LessOrEqual(signed_root.clone(), maximum),
            ])),
        );
        assert_eq!(
            exact_integer_mixed_shift_chain_obligation(
                signed_type,
                right,
                3,
                &[Proposition::Truth],
                1,
                &BTreeSet::from([root_id]),
            ),
            None,
            "a stale or redirected definition cannot authorize the mixed prefix",
        );

        let unsigned_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        assert_eq!(
            exact_integer_mixed_shift_preimage(
                unsigned_type,
                (0, 15),
                &ScalarTerm::exact_integer_shift_right(
                    unsigned_type,
                    count_type,
                    ScalarTerm::integer(unsigned_type, IntegerValue::Unsigned(0)).expect("0u8"),
                    ScalarTerm::integer(count_type, IntegerValue::Unsigned(4)).expect("4u8"),
                )
                .expect("unsigned right shape"),
                4,
            ),
            Ok(Some((0, 255))),
            "a right-shift preimage clips to the source carrier",
        );
    }

    #[test]
    fn exact_mixed_shift_chain_cast_reconstructs_unsigned_and_cross_sign_preimages() {
        let source_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 source");
        let target_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 target");
        let i8_count = IntegerType::new(IntegerSign::Signed, 8).expect("i8 count");
        let u16_count = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 count");
        let i32_count = IntegerType::new(IntegerSign::Signed, 32).expect("i32 count");
        let root_id = ValueId::new(321).expect("root");
        let root = ScalarTerm::value(root_id, ScalarType::Integer(source_type));
        let left_one = ScalarTerm::value(
            ValueId::new(322).expect("left one"),
            ScalarType::Integer(source_type),
        );
        let right_two = ScalarTerm::value(
            ValueId::new(323).expect("right two"),
            ScalarType::Integer(source_type),
        );
        let left_three = ScalarTerm::value(
            ValueId::new(324).expect("left three"),
            ScalarType::Integer(source_type),
        );
        let one = ScalarTerm::integer(i8_count, IntegerValue::Signed(1)).expect("1i8");
        let two = ScalarTerm::integer(u16_count, IntegerValue::Unsigned(2)).expect("2u16");
        let three = ScalarTerm::integer(i32_count, IntegerValue::Signed(3)).expect("3i32");
        let definitions = vec![
            Proposition::Equal(
                left_one.clone(),
                ScalarTerm::exact_integer_shift_left(source_type, i8_count, root.clone(), one)
                    .expect("root << 1"),
            ),
            Proposition::Equal(
                right_two.clone(),
                ScalarTerm::exact_integer_shift_right(source_type, u16_count, left_one, two)
                    .expect("(root << 1) >> 2"),
            ),
            Proposition::Equal(
                left_three.clone(),
                ScalarTerm::exact_integer_shift_left(source_type, i32_count, right_two, three)
                    .expect("((root << 1) >> 2) << 3"),
            ),
        ];
        let maximum = ScalarTerm::integer(source_type, IntegerValue::Unsigned(63)).expect("63u16");
        assert_eq!(
            exact_integer_mixed_shift_chain_cast_obligation(
                source_type,
                target_type,
                left_three,
                &definitions,
                &BTreeSet::from([root_id]),
            ),
            Some(Proposition::LessOrEqual(root.clone(), maximum)),
        );

        let signed_source = IntegerType::new(IntegerSign::Signed, 16).expect("i16 source");
        let signed_root_id = ValueId::new(325).expect("signed root");
        let signed_root = ScalarTerm::value(signed_root_id, ScalarType::Integer(signed_source));
        let signed_right = ScalarTerm::value(
            ValueId::new(326).expect("signed right"),
            ScalarType::Integer(signed_source),
        );
        let signed_left = ScalarTerm::value(
            ValueId::new(327).expect("signed left"),
            ScalarType::Integer(signed_source),
        );
        let signed_definitions = vec![
            Proposition::Equal(
                signed_right.clone(),
                ScalarTerm::exact_integer_shift_right(
                    signed_source,
                    i8_count,
                    signed_root.clone(),
                    ScalarTerm::integer(i8_count, IntegerValue::Signed(1)).expect("1i8"),
                )
                .expect("signed root >> 1"),
            ),
            Proposition::Equal(
                signed_left.clone(),
                ScalarTerm::exact_integer_shift_left(
                    signed_source,
                    u16_count,
                    signed_right,
                    ScalarTerm::integer(u16_count, IntegerValue::Unsigned(2)).expect("2u16"),
                )
                .expect("(signed root >> 1) << 2"),
            ),
        ];
        let minimum = ScalarTerm::integer(signed_source, IntegerValue::Signed(0)).expect("0i16");
        let maximum =
            ScalarTerm::integer(signed_source, IntegerValue::Signed(127)).expect("127i16");
        assert_eq!(
            exact_integer_mixed_shift_chain_cast_obligation(
                signed_source,
                target_type,
                signed_left,
                &signed_definitions,
                &BTreeSet::from([signed_root_id]),
            ),
            Some(canonical_conjunction(vec![
                Proposition::LessOrEqual(minimum, signed_root.clone()),
                Proposition::LessOrEqual(signed_root, maximum),
            ])),
        );
    }

    #[test]
    fn exact_cast_then_mixed_shift_chain_reconstructs_each_left_prefix_from_source_root() {
        let source_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16 source");
        let target_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 target");
        let i8_count = IntegerType::new(IntegerSign::Signed, 8).expect("i8 count");
        let u16_count = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 count");
        let root_id = ValueId::new(331).expect("root");
        let root = ScalarTerm::value(root_id, ScalarType::Integer(source_type));
        let cast = ScalarTerm::value(
            ValueId::new(332).expect("cast"),
            ScalarType::Integer(target_type),
        );
        let left = ScalarTerm::value(
            ValueId::new(333).expect("left"),
            ScalarType::Integer(target_type),
        );
        let right = ScalarTerm::value(
            ValueId::new(334).expect("right"),
            ScalarType::Integer(target_type),
        );
        let definitions = vec![
            Proposition::Equal(
                cast.clone(),
                ScalarTerm::integer_exact_cast(source_type, target_type, root.clone())
                    .expect("i16 to u8 cast"),
            ),
            Proposition::Equal(
                left.clone(),
                ScalarTerm::exact_integer_shift_left(
                    target_type,
                    i8_count,
                    cast,
                    ScalarTerm::integer(i8_count, IntegerValue::Signed(1)).expect("1i8"),
                )
                .expect("cast << 1"),
            ),
            Proposition::Equal(
                right.clone(),
                ScalarTerm::exact_integer_shift_right(
                    target_type,
                    u16_count,
                    left,
                    ScalarTerm::integer(u16_count, IntegerValue::Unsigned(2)).expect("2u16"),
                )
                .expect("(cast << 1) >> 2"),
            ),
        ];
        let minimum = ScalarTerm::integer(source_type, IntegerValue::Signed(0)).expect("0i16");
        let maximum = ScalarTerm::integer(source_type, IntegerValue::Signed(63)).expect("63i16");
        assert_eq!(
            exact_integer_cast_then_mixed_shift_chain_obligation(
                target_type,
                right.clone(),
                3,
                &definitions,
                definitions.len(),
                &BTreeSet::from([root_id]),
            ),
            Some(canonical_conjunction(vec![
                Proposition::LessOrEqual(minimum, root.clone()),
                Proposition::LessOrEqual(root.clone(), maximum),
            ])),
        );
        assert_eq!(
            exact_integer_cast_then_mixed_shift_chain_obligation(
                target_type,
                right,
                3,
                &[Proposition::Truth],
                1,
                &BTreeSet::from([root_id]),
            ),
            None,
            "a stale definition cannot authorize a post-cast mixed prefix",
        );

        let cross_source = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 source");
        let cross_target = IntegerType::new(IntegerSign::Signed, 8).expect("i8 target");
        let cross_root_id = ValueId::new(335).expect("cross root");
        let cross_root = ScalarTerm::value(cross_root_id, ScalarType::Integer(cross_source));
        let cross_cast = ScalarTerm::value(
            ValueId::new(336).expect("cross cast"),
            ScalarType::Integer(cross_target),
        );
        let cross_right = ScalarTerm::value(
            ValueId::new(337).expect("cross right"),
            ScalarType::Integer(cross_target),
        );
        let cross_definitions = vec![
            Proposition::Equal(
                cross_cast.clone(),
                ScalarTerm::integer_exact_cast(cross_source, cross_target, cross_root.clone())
                    .expect("u16 to i8 cast"),
            ),
            Proposition::Equal(
                cross_right.clone(),
                ScalarTerm::exact_integer_shift_right(
                    cross_target,
                    i8_count,
                    cross_cast,
                    ScalarTerm::integer(i8_count, IntegerValue::Signed(1)).expect("1i8"),
                )
                .expect("cast >> 1"),
            ),
        ];
        assert_eq!(
            exact_integer_cast_then_mixed_shift_chain_obligation(
                cross_target,
                cross_right,
                2,
                &cross_definitions,
                cross_definitions.len(),
                &BTreeSet::from([cross_root_id]),
            ),
            Some(Proposition::LessOrEqual(
                cross_root,
                ScalarTerm::integer(cross_source, IntegerValue::Unsigned(63)).expect("63u16"),
            )),
        );
    }

    #[test]
    fn exact_shift_cast_shift_reconstructs_both_sides_without_importing_prefix_proofs() {
        let source_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 source");
        let target_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 target");
        let i8_count = IntegerType::new(IntegerSign::Signed, 8).expect("i8 count");
        let u16_count = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 count");
        let i32_count = IntegerType::new(IntegerSign::Signed, 32).expect("i32 count");
        let root_id = ValueId::new(338).expect("root");
        let root = ScalarTerm::value(root_id, ScalarType::Integer(source_type));
        let source_right = ScalarTerm::value(
            ValueId::new(339).expect("source right"),
            ScalarType::Integer(source_type),
        );
        let source_left = ScalarTerm::value(
            ValueId::new(340).expect("source left"),
            ScalarType::Integer(source_type),
        );
        let cast = ScalarTerm::value(
            ValueId::new(1341).expect("cast"),
            ScalarType::Integer(target_type),
        );
        let target_right = ScalarTerm::value(
            ValueId::new(1342).expect("target right"),
            ScalarType::Integer(target_type),
        );
        let definitions = vec![
            Proposition::Equal(
                source_right.clone(),
                ScalarTerm::exact_integer_shift_right(
                    source_type,
                    i8_count,
                    root.clone(),
                    ScalarTerm::integer(i8_count, IntegerValue::Signed(1)).expect("1i8"),
                )
                .expect("root >> 1"),
            ),
            Proposition::Equal(
                source_left.clone(),
                ScalarTerm::exact_integer_shift_left(
                    source_type,
                    u16_count,
                    source_right,
                    ScalarTerm::integer(u16_count, IntegerValue::Unsigned(2)).expect("2u16"),
                )
                .expect("(root >> 1) << 2"),
            ),
            Proposition::Equal(
                cast.clone(),
                ScalarTerm::integer_exact_cast(source_type, target_type, source_left)
                    .expect("u16 to u8 cast"),
            ),
            Proposition::Equal(
                target_right.clone(),
                ScalarTerm::exact_integer_shift_right(
                    target_type,
                    i32_count,
                    cast,
                    ScalarTerm::integer(i32_count, IntegerValue::Signed(1)).expect("1i32"),
                )
                .expect("cast >> 1"),
            ),
        ];
        let maximum = ScalarTerm::integer(source_type, IntegerValue::Unsigned(63)).expect("63u16");
        assert_eq!(
            exact_integer_shift_cast_shift_obligation(
                target_type,
                target_right.clone(),
                2,
                &definitions,
                definitions.len(),
                &BTreeSet::from([root_id]),
            ),
            Some(Proposition::LessOrEqual(root.clone(), maximum)),
            "the target prefix, cast, and source prefix are replayed from canonical definitions",
        );
        assert_eq!(
            exact_integer_shift_cast_shift_obligation(
                target_type,
                target_right,
                2,
                &[Proposition::Truth],
                1,
                &BTreeSet::from([root_id]),
            ),
            None,
            "stale definitions cannot authorize the sandwich",
        );

        let signed_source = IntegerType::new(IntegerSign::Signed, 16).expect("i16 source");
        let signed_root_id = ValueId::new(1343).expect("signed root");
        let signed_root = ScalarTerm::value(signed_root_id, ScalarType::Integer(signed_source));
        let signed_right = ScalarTerm::value(
            ValueId::new(1344).expect("signed right"),
            ScalarType::Integer(signed_source),
        );
        let signed_left = ScalarTerm::value(
            ValueId::new(1345).expect("signed left"),
            ScalarType::Integer(signed_source),
        );
        let signed_cast = ScalarTerm::value(
            ValueId::new(1346).expect("signed cast"),
            ScalarType::Integer(target_type),
        );
        let signed_target_right = ScalarTerm::value(
            ValueId::new(1347).expect("signed target right"),
            ScalarType::Integer(target_type),
        );
        let signed_definitions = vec![
            Proposition::Equal(
                signed_right.clone(),
                ScalarTerm::exact_integer_shift_right(
                    signed_source,
                    u16_count,
                    signed_root.clone(),
                    ScalarTerm::integer(u16_count, IntegerValue::Unsigned(1)).expect("1u16"),
                )
                .expect("signed root >> 1"),
            ),
            Proposition::Equal(
                signed_left.clone(),
                ScalarTerm::exact_integer_shift_left(
                    signed_source,
                    i8_count,
                    signed_right,
                    ScalarTerm::integer(i8_count, IntegerValue::Signed(2)).expect("2i8"),
                )
                .expect("(signed root >> 1) << 2"),
            ),
            Proposition::Equal(
                signed_cast.clone(),
                ScalarTerm::integer_exact_cast(signed_source, target_type, signed_left)
                    .expect("i16 to u8 cast"),
            ),
            Proposition::Equal(
                signed_target_right.clone(),
                ScalarTerm::exact_integer_shift_right(
                    target_type,
                    i32_count,
                    signed_cast,
                    ScalarTerm::integer(i32_count, IntegerValue::Signed(1)).expect("1i32"),
                )
                .expect("signed cast >> 1"),
            ),
        ];
        let minimum = ScalarTerm::integer(signed_source, IntegerValue::Signed(0)).expect("0i16");
        let maximum = ScalarTerm::integer(signed_source, IntegerValue::Signed(63)).expect("63i16");
        assert_eq!(
            exact_integer_shift_cast_shift_obligation(
                target_type,
                signed_target_right,
                2,
                &signed_definitions,
                signed_definitions.len(),
                &BTreeSet::from([signed_root_id]),
            ),
            Some(canonical_conjunction(vec![
                Proposition::LessOrEqual(minimum, signed_root.clone()),
                Proposition::LessOrEqual(signed_root, maximum),
            ])),
        );
    }

    #[test]
    fn exact_divide_remainder_cross_cast_reconstructs_carrier_total_target_prefixes() {
        let source_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 source");
        let target_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 target");
        let i8_count = IntegerType::new(IntegerSign::Signed, 8).expect("i8 count");
        let root_id = ValueId::new(1501).expect("divide root");
        let root = ScalarTerm::value(root_id, ScalarType::Integer(source_type));
        let divided = ScalarTerm::value(
            ValueId::new(1502).expect("divided"),
            ScalarType::Integer(source_type),
        );
        let remainder = ScalarTerm::value(
            ValueId::new(1503).expect("remainder"),
            ScalarType::Integer(source_type),
        );
        let cast = ScalarTerm::value(
            ValueId::new(1504).expect("cast"),
            ScalarType::Integer(target_type),
        );
        let target_add = ScalarTerm::value(
            ValueId::new(1505).expect("target add"),
            ScalarType::Integer(target_type),
        );
        let target_right = ScalarTerm::value(
            ValueId::new(1506).expect("target right"),
            ScalarType::Integer(target_type),
        );
        let definitions = vec![
            Proposition::Equal(
                divided.clone(),
                ScalarTerm::exact_integer_divide(
                    source_type,
                    root.clone(),
                    ScalarTerm::integer(source_type, IntegerValue::Unsigned(2)).expect("2u16"),
                )
                .expect("root / 2"),
            ),
            Proposition::Equal(
                remainder.clone(),
                ScalarTerm::exact_integer_remainder(
                    source_type,
                    divided,
                    ScalarTerm::integer(source_type, IntegerValue::Unsigned(64)).expect("64u16"),
                )
                .expect("divided % 64"),
            ),
            Proposition::Equal(
                cast.clone(),
                ScalarTerm::integer_exact_cast(source_type, target_type, remainder)
                    .expect("carrier-total u16 to u8 cast"),
            ),
            Proposition::Equal(
                target_add.clone(),
                ScalarTerm::exact_integer_add(
                    target_type,
                    cast.clone(),
                    ScalarTerm::integer(target_type, IntegerValue::Unsigned(1)).expect("1u8"),
                )
                .expect("cast + 1"),
            ),
            Proposition::Equal(
                target_right.clone(),
                ScalarTerm::exact_integer_shift_right(
                    target_type,
                    i8_count,
                    cast.clone(),
                    ScalarTerm::integer(i8_count, IntegerValue::Signed(1)).expect("1i8"),
                )
                .expect("cast >> 1"),
            ),
        ];
        let parameters = BTreeSet::from([root_id]);
        assert_eq!(
            exact_integer_divide_remainder_cast_affine_obligation(
                target_type,
                target_add,
                IntegerValue::Unsigned(2),
                ExactIntegerAffineOperation::Multiply,
                &definitions,
                4,
                &parameters,
            ),
            Some(Proposition::Truth),
            "the full [0,63] remainder hull fits the checked (value + 1) * 2 prefix",
        );
        assert_eq!(
            exact_integer_divide_remainder_cast_affine_obligation(
                target_type,
                cast.clone(),
                IntegerValue::Unsigned(64),
                ExactIntegerAffineOperation::Subtract,
                &definitions,
                3,
                &parameters,
            ),
            Some(Proposition::Falsehood),
            "a hull disjoint from the current safe interval is canonical falsehood",
        );
        assert_eq!(
            exact_integer_divide_remainder_cast_affine_obligation(
                target_type,
                cast.clone(),
                IntegerValue::Unsigned(200),
                ExactIntegerAffineOperation::Add,
                &definitions,
                3,
                &parameters,
            ),
            None,
            "partial overlap needs a guard-sensitive preimage and remains outside the family",
        );
        assert_eq!(
            exact_integer_divide_remainder_cast_affine_obligation(
                target_type,
                cast,
                IntegerValue::Unsigned(0),
                ExactIntegerAffineOperation::Multiply,
                &definitions,
                3,
                &parameters,
            ),
            Some(Proposition::Truth),
            "zero decides only the current target prefix after the complete source walk",
        );
        assert_eq!(
            exact_integer_divide_remainder_cast_shift_obligation(
                target_type,
                target_right,
                2,
                &definitions,
                definitions.len(),
                &parameters,
            ),
            Some(Proposition::Truth),
            "the target-right preimage admits the whole hull before the target-left prefix",
        );
        assert_eq!(
            exact_integer_divide_remainder_cast_shift_obligation(
                target_type,
                ScalarTerm::value(
                    ValueId::new(1507).expect("stale target"),
                    ScalarType::Integer(target_type),
                ),
                2,
                &definitions,
                definitions.len(),
                &parameters,
            ),
            None,
            "stale target definitions cannot authorize carrier-total replay",
        );
    }

    #[test]
    fn exact_divide_remainder_cast_sandwich_keeps_each_obligation_independent() {
        let source_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 source");
        let target_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 target");
        let root_id = ValueId::new(1508).expect("sandwich root");
        let root = ScalarTerm::value(root_id, ScalarType::Integer(source_type));
        let remainder = ScalarTerm::value(
            ValueId::new(1509).expect("source remainder"),
            ScalarType::Integer(source_type),
        );
        let cast = ScalarTerm::value(
            ValueId::new(1510).expect("sandwich cast"),
            ScalarType::Integer(target_type),
        );
        let definitions = vec![
            Proposition::Equal(
                remainder.clone(),
                ScalarTerm::exact_integer_remainder(
                    source_type,
                    root,
                    ScalarTerm::integer(source_type, IntegerValue::Unsigned(64)).expect("64u16"),
                )
                .expect("root % 64"),
            ),
            Proposition::Equal(
                cast.clone(),
                ScalarTerm::integer_exact_cast(source_type, target_type, remainder.clone())
                    .expect("carrier-total u16 remainder to i8 cast"),
            ),
        ];
        assert_eq!(
            exact_integer_cast_obligation(
                source_type,
                target_type,
                remainder,
                &definitions[..1],
                &BTreeSet::from([root_id]),
            ),
            Proposition::Truth,
            "the cast replays only the complete source-chain hull",
        );
        let two = ScalarTerm::integer(target_type, IntegerValue::Signed(2)).expect("2i8");
        let three = ScalarTerm::integer(target_type, IntegerValue::Signed(3)).expect("3i8");
        assert_eq!(
            exact_integer_divide_obligation(target_type, cast.clone(), two, &definitions,),
            Proposition::Truth,
            "the target divide uses only its independently safe divisor",
        );
        assert_eq!(
            exact_integer_remainder_obligation(target_type, cast, three, &definitions),
            Proposition::Truth,
            "the target remainder uses only its independently safe divisor",
        );
    }

    #[test]
    fn exact_divide_remainder_cross_chain_reconstructs_carrier_total_target_prefixes() {
        let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 carrier");
        let i8_count = IntegerType::new(IntegerSign::Signed, 8).expect("i8 count");
        let root_id = ValueId::new(1511).expect("divide root");
        let root = ScalarTerm::value(root_id, ScalarType::Integer(integer_type));
        let divided = ScalarTerm::value(
            ValueId::new(1512).expect("divided"),
            ScalarType::Integer(integer_type),
        );
        let remainder = ScalarTerm::value(
            ValueId::new(1513).expect("remainder"),
            ScalarType::Integer(integer_type),
        );
        let target_add = ScalarTerm::value(
            ValueId::new(1514).expect("target add"),
            ScalarType::Integer(integer_type),
        );
        let target_right = ScalarTerm::value(
            ValueId::new(1515).expect("target right"),
            ScalarType::Integer(integer_type),
        );
        let definitions = vec![
            Proposition::Equal(
                divided.clone(),
                ScalarTerm::exact_integer_divide(
                    integer_type,
                    root.clone(),
                    ScalarTerm::integer(integer_type, IntegerValue::Unsigned(2)).expect("2u8"),
                )
                .expect("root / 2"),
            ),
            Proposition::Equal(
                remainder.clone(),
                ScalarTerm::exact_integer_remainder(
                    integer_type,
                    divided,
                    ScalarTerm::integer(integer_type, IntegerValue::Unsigned(64)).expect("64u8"),
                )
                .expect("divided % 64"),
            ),
            Proposition::Equal(
                target_add.clone(),
                ScalarTerm::exact_integer_add(
                    integer_type,
                    remainder.clone(),
                    ScalarTerm::integer(integer_type, IntegerValue::Unsigned(1)).expect("1u8"),
                )
                .expect("remainder + 1"),
            ),
            Proposition::Equal(
                target_right.clone(),
                ScalarTerm::exact_integer_shift_right(
                    integer_type,
                    i8_count,
                    remainder.clone(),
                    ScalarTerm::integer(i8_count, IntegerValue::Signed(1)).expect("1i8"),
                )
                .expect("remainder >> 1"),
            ),
        ];
        let parameters = BTreeSet::from([root_id]);
        assert_eq!(
            exact_integer_divide_remainder_then_affine_obligation(
                integer_type,
                target_add,
                IntegerValue::Unsigned(2),
                ExactIntegerAffineOperation::Multiply,
                &definitions,
                3,
                &parameters,
            ),
            Some(Proposition::Truth),
            "the complete [0,63] remainder hull fits the (value + 1) * 2 prefix",
        );
        assert_eq!(
            exact_integer_divide_remainder_then_affine_obligation(
                integer_type,
                remainder.clone(),
                IntegerValue::Unsigned(64),
                ExactIntegerAffineOperation::Subtract,
                &definitions,
                2,
                &parameters,
            ),
            Some(Proposition::Falsehood),
            "a hull disjoint from the current safe interval is canonical falsehood",
        );
        assert_eq!(
            exact_integer_divide_remainder_then_affine_obligation(
                integer_type,
                remainder.clone(),
                IntegerValue::Unsigned(200),
                ExactIntegerAffineOperation::Add,
                &definitions,
                2,
                &parameters,
            ),
            None,
            "partial overlap remains outside the carrier-total family",
        );
        assert_eq!(
            exact_integer_divide_remainder_then_affine_obligation(
                integer_type,
                remainder,
                IntegerValue::Unsigned(0),
                ExactIntegerAffineOperation::Multiply,
                &definitions,
                2,
                &parameters,
            ),
            Some(Proposition::Truth),
            "zero decides only the current target prefix after the full source walk",
        );
        assert_eq!(
            exact_integer_divide_remainder_then_shift_obligation(
                integer_type,
                target_right,
                2,
                &definitions,
                definitions.len(),
                &parameters,
            ),
            Some(Proposition::Truth),
            "the target-right preimage admits the whole hull before the target-left prefix",
        );
        assert_eq!(
            exact_integer_divide_remainder_then_shift_obligation(
                integer_type,
                ScalarTerm::value(
                    ValueId::new(1516).expect("stale target"),
                    ScalarType::Integer(integer_type),
                ),
                2,
                &definitions,
                definitions.len(),
                &parameters,
            ),
            None,
            "stale definitions cannot authorize carrier-total replay",
        );
    }

    #[test]
    fn exact_affine_shift_cast_sandwich_reconstructs_both_directions_and_zero_locally() {
        let source_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 source");
        let target_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 target");
        let i8_count = IntegerType::new(IntegerSign::Signed, 8).expect("i8 count");
        let u16_count = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 count");

        let affine_root_id = ValueId::new(1401).expect("affine root");
        let affine_root = ScalarTerm::value(affine_root_id, ScalarType::Integer(source_type));
        let source_add = ScalarTerm::value(
            ValueId::new(1402).expect("source add"),
            ScalarType::Integer(source_type),
        );
        let source_multiply = ScalarTerm::value(
            ValueId::new(1403).expect("source multiply"),
            ScalarType::Integer(source_type),
        );
        let affine_cast = ScalarTerm::value(
            ValueId::new(1404).expect("affine cast"),
            ScalarType::Integer(target_type),
        );
        let target_right = ScalarTerm::value(
            ValueId::new(1405).expect("target right"),
            ScalarType::Integer(target_type),
        );
        let affine_to_shift_definitions = vec![
            Proposition::Equal(
                source_add.clone(),
                ScalarTerm::exact_integer_add(
                    source_type,
                    affine_root.clone(),
                    ScalarTerm::integer(source_type, IntegerValue::Unsigned(1)).expect("1u16"),
                )
                .expect("root + 1"),
            ),
            Proposition::Equal(
                source_multiply.clone(),
                ScalarTerm::exact_integer_multiply(
                    source_type,
                    source_add,
                    ScalarTerm::integer(source_type, IntegerValue::Unsigned(2)).expect("2u16"),
                )
                .expect("(root + 1) * 2"),
            ),
            Proposition::Equal(
                affine_cast.clone(),
                ScalarTerm::integer_exact_cast(source_type, target_type, source_multiply)
                    .expect("u16 to u8 cast"),
            ),
            Proposition::Equal(
                target_right.clone(),
                ScalarTerm::exact_integer_shift_right(
                    target_type,
                    i8_count,
                    affine_cast.clone(),
                    ScalarTerm::integer(i8_count, IntegerValue::Signed(1)).expect("1i8"),
                )
                .expect("cast >> 1"),
            ),
        ];
        assert_eq!(
            exact_integer_affine_cast_shift_obligation(
                target_type,
                target_right.clone(),
                2,
                &affine_to_shift_definitions,
                affine_to_shift_definitions.len(),
                &BTreeSet::from([affine_root_id]),
            ),
            Some(Proposition::LessOrEqual(
                affine_root.clone(),
                ScalarTerm::integer(source_type, IntegerValue::Unsigned(62)).expect("62u16"),
            )),
        );
        assert_eq!(
            exact_integer_affine_cast_shift_obligation(
                target_type,
                target_right,
                2,
                &[Proposition::Truth],
                1,
                &BTreeSet::from([affine_root_id]),
            ),
            None,
            "stale target definitions cannot authorize the affine-to-shift direction",
        );

        let zero = ScalarTerm::value(
            ValueId::new(1406).expect("zero"),
            ScalarType::Integer(source_type),
        );
        let constant = ScalarTerm::value(
            ValueId::new(1407).expect("constant"),
            ScalarType::Integer(source_type),
        );
        let constant_cast = ScalarTerm::value(
            ValueId::new(1408).expect("constant cast"),
            ScalarType::Integer(target_type),
        );
        let constant_definitions = vec![
            Proposition::Equal(
                zero.clone(),
                ScalarTerm::exact_integer_multiply(
                    source_type,
                    affine_root.clone(),
                    ScalarTerm::integer(source_type, IntegerValue::Unsigned(0)).expect("0u16"),
                )
                .expect("root * 0"),
            ),
            Proposition::Equal(
                constant.clone(),
                ScalarTerm::exact_integer_add(
                    source_type,
                    zero,
                    ScalarTerm::integer(source_type, IntegerValue::Unsigned(255)).expect("255u16"),
                )
                .expect("zero + 255"),
            ),
            Proposition::Equal(
                constant_cast.clone(),
                ScalarTerm::integer_exact_cast(source_type, target_type, constant)
                    .expect("constant cast"),
            ),
        ];
        assert_eq!(
            exact_integer_affine_cast_shift_obligation(
                target_type,
                constant_cast,
                2,
                &constant_definitions,
                constant_definitions.len(),
                &BTreeSet::from([affine_root_id]),
            ),
            Some(Proposition::Falsehood),
            "a constant source affine value outside the target-left interval is mathematically false",
        );

        let shift_root_id = ValueId::new(1411).expect("shift root");
        let shift_root = ScalarTerm::value(shift_root_id, ScalarType::Integer(source_type));
        let source_right = ScalarTerm::value(
            ValueId::new(1412).expect("source right"),
            ScalarType::Integer(source_type),
        );
        let source_left = ScalarTerm::value(
            ValueId::new(1413).expect("source left"),
            ScalarType::Integer(source_type),
        );
        let shift_cast = ScalarTerm::value(
            ValueId::new(1414).expect("shift cast"),
            ScalarType::Integer(target_type),
        );
        let target_add = ScalarTerm::value(
            ValueId::new(1415).expect("target add"),
            ScalarType::Integer(target_type),
        );
        let shift_to_affine_definitions = vec![
            Proposition::Equal(
                source_right.clone(),
                ScalarTerm::exact_integer_shift_right(
                    source_type,
                    i8_count,
                    shift_root.clone(),
                    ScalarTerm::integer(i8_count, IntegerValue::Signed(1)).expect("1i8"),
                )
                .expect("root >> 1"),
            ),
            Proposition::Equal(
                source_left.clone(),
                ScalarTerm::exact_integer_shift_left(
                    source_type,
                    u16_count,
                    source_right,
                    ScalarTerm::integer(u16_count, IntegerValue::Unsigned(2)).expect("2u16"),
                )
                .expect("(root >> 1) << 2"),
            ),
            Proposition::Equal(
                shift_cast.clone(),
                ScalarTerm::integer_exact_cast(source_type, target_type, source_left)
                    .expect("shift cast"),
            ),
            Proposition::Equal(
                target_add.clone(),
                ScalarTerm::exact_integer_add(
                    target_type,
                    shift_cast.clone(),
                    ScalarTerm::integer(target_type, IntegerValue::Unsigned(3)).expect("3u8"),
                )
                .expect("cast + 3"),
            ),
        ];
        assert_eq!(
            exact_integer_shift_cast_affine_obligation(
                target_type,
                target_add,
                IntegerValue::Unsigned(2),
                ExactIntegerAffineOperation::Multiply,
                &shift_to_affine_definitions,
                shift_to_affine_definitions.len(),
                &BTreeSet::from([shift_root_id]),
            ),
            Some(Proposition::LessOrEqual(
                shift_root.clone(),
                ScalarTerm::integer(source_type, IntegerValue::Unsigned(63)).expect("63u16"),
            )),
        );
        assert_eq!(
            exact_integer_shift_cast_affine_obligation(
                target_type,
                shift_cast,
                IntegerValue::Unsigned(0),
                ExactIntegerAffineOperation::Multiply,
                &shift_to_affine_definitions[..3],
                3,
                &BTreeSet::from([shift_root_id]),
            ),
            Some(Proposition::Truth),
            "a target zero coefficient decides only the current prefix after the source shift walk",
        );
    }

    #[test]
    fn exact_arithmetic_then_shift_chain_reconstructs_affine_preimages_independently() {
        let value_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let i8_count = IntegerType::new(IntegerSign::Signed, 8).expect("i8 count");
        let u16_count = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 count");
        let root_id = ValueId::new(341).expect("root");
        let root = ScalarTerm::value(root_id, ScalarType::Integer(value_type));
        let add = ScalarTerm::value(
            ValueId::new(342).expect("add"),
            ScalarType::Integer(value_type),
        );
        let multiply = ScalarTerm::value(
            ValueId::new(343).expect("multiply"),
            ScalarType::Integer(value_type),
        );
        let right = ScalarTerm::value(
            ValueId::new(344).expect("right"),
            ScalarType::Integer(value_type),
        );
        let definitions = vec![
            Proposition::Equal(
                add.clone(),
                ScalarTerm::exact_integer_add(
                    value_type,
                    root.clone(),
                    ScalarTerm::integer(value_type, IntegerValue::Unsigned(3)).expect("3u8"),
                )
                .expect("root + 3"),
            ),
            Proposition::Equal(
                multiply.clone(),
                ScalarTerm::exact_integer_multiply(
                    value_type,
                    add,
                    ScalarTerm::integer(value_type, IntegerValue::Unsigned(2)).expect("2u8"),
                )
                .expect("(root + 3) * 2"),
            ),
            Proposition::Equal(
                right.clone(),
                ScalarTerm::exact_integer_shift_right(
                    value_type,
                    i8_count,
                    multiply,
                    ScalarTerm::integer(i8_count, IntegerValue::Signed(1)).expect("1i8"),
                )
                .expect("((root + 3) * 2) >> 1"),
            ),
        ];
        assert_eq!(
            exact_integer_arithmetic_then_shift_chain_obligation(
                value_type,
                right.clone(),
                2,
                &definitions,
                definitions.len(),
                &BTreeSet::from([root_id]),
            ),
            Some(Proposition::LessOrEqual(
                root.clone(),
                ScalarTerm::integer(value_type, IntegerValue::Unsigned(60)).expect("60u8"),
            )),
        );
        assert_eq!(
            exact_integer_arithmetic_then_shift_chain_obligation(
                value_type,
                right,
                2,
                &[Proposition::Truth],
                1,
                &BTreeSet::from([root_id]),
            ),
            None,
            "stale definitions cannot authorize the computed left prefix",
        );

        let signed_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let signed_root_id = ValueId::new(345).expect("signed root");
        let signed_root = ScalarTerm::value(signed_root_id, ScalarType::Integer(signed_type));
        let signed_subtract = ScalarTerm::value(
            ValueId::new(346).expect("signed subtract"),
            ScalarType::Integer(signed_type),
        );
        let signed_multiply = ScalarTerm::value(
            ValueId::new(347).expect("signed multiply"),
            ScalarType::Integer(signed_type),
        );
        let signed_right = ScalarTerm::value(
            ValueId::new(348).expect("signed right"),
            ScalarType::Integer(signed_type),
        );
        let signed_definitions = vec![
            Proposition::Equal(
                signed_subtract.clone(),
                ScalarTerm::exact_integer_subtract(
                    signed_type,
                    signed_root.clone(),
                    ScalarTerm::integer(signed_type, IntegerValue::Signed(-3)).expect("-3i8"),
                )
                .expect("root - -3"),
            ),
            Proposition::Equal(
                signed_multiply.clone(),
                ScalarTerm::exact_integer_multiply(
                    signed_type,
                    signed_subtract,
                    ScalarTerm::integer(signed_type, IntegerValue::Signed(2)).expect("2i8"),
                )
                .expect("(root - -3) * 2"),
            ),
            Proposition::Equal(
                signed_right.clone(),
                ScalarTerm::exact_integer_shift_right(
                    signed_type,
                    u16_count,
                    signed_multiply,
                    ScalarTerm::integer(u16_count, IntegerValue::Unsigned(1)).expect("1u16"),
                )
                .expect("((root - -3) * 2) >> 1"),
            ),
        ];
        assert_eq!(
            exact_integer_arithmetic_then_shift_chain_obligation(
                signed_type,
                signed_right,
                2,
                &signed_definitions,
                signed_definitions.len(),
                &BTreeSet::from([signed_root_id]),
            ),
            Some(canonical_conjunction(vec![
                Proposition::LessOrEqual(
                    ScalarTerm::integer(signed_type, IntegerValue::Signed(-35)).expect("-35i8"),
                    signed_root.clone(),
                ),
                Proposition::LessOrEqual(
                    signed_root,
                    ScalarTerm::integer(signed_type, IntegerValue::Signed(28)).expect("28i8"),
                ),
            ])),
        );
    }

    #[test]
    fn exact_arithmetic_then_shift_chain_handles_zero_and_checked_composition_failure() {
        let value_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let root_id = ValueId::new(351).expect("root");
        let root = ScalarTerm::value(root_id, ScalarType::Integer(value_type));
        let zero = ScalarTerm::value(
            ValueId::new(352).expect("zero"),
            ScalarType::Integer(value_type),
        );
        let constant = ScalarTerm::value(
            ValueId::new(353).expect("constant"),
            ScalarType::Integer(value_type),
        );
        let definitions = vec![
            Proposition::Equal(
                zero.clone(),
                ScalarTerm::exact_integer_multiply(
                    value_type,
                    root,
                    ScalarTerm::integer(value_type, IntegerValue::Unsigned(0)).expect("0u8"),
                )
                .expect("root * 0"),
            ),
            Proposition::Equal(
                constant.clone(),
                ScalarTerm::exact_integer_add(
                    value_type,
                    zero,
                    ScalarTerm::integer(value_type, IntegerValue::Unsigned(255)).expect("255u8"),
                )
                .expect("zero + 255"),
            ),
        ];
        assert_eq!(
            exact_integer_arithmetic_then_shift_chain_obligation(
                value_type,
                constant,
                1,
                &definitions,
                definitions.len(),
                &BTreeSet::from([root_id]),
            ),
            Some(Proposition::Falsehood),
            "a constant affine result outside the left-shift interval is mathematically false",
        );

        let wide_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
        let wide_root_id = ValueId::new(354).expect("wide root");
        let wide_root = ScalarTerm::value(wide_root_id, ScalarType::Integer(wide_type));
        let first = ScalarTerm::value(
            ValueId::new(355).expect("first"),
            ScalarType::Integer(wide_type),
        );
        let second = ScalarTerm::value(
            ValueId::new(356).expect("second"),
            ScalarType::Integer(wide_type),
        );
        let third = ScalarTerm::value(
            ValueId::new(357).expect("third"),
            ScalarType::Integer(wide_type),
        );
        let wide_definitions = vec![
            Proposition::Equal(
                first.clone(),
                ScalarTerm::exact_integer_multiply(
                    wide_type,
                    wide_root,
                    ScalarTerm::integer(wide_type, IntegerValue::Unsigned(u64::MAX as u128))
                        .expect("u64 max"),
                )
                .expect("root * max"),
            ),
            Proposition::Equal(
                second.clone(),
                ScalarTerm::exact_integer_multiply(
                    wide_type,
                    first,
                    ScalarTerm::integer(wide_type, IntegerValue::Unsigned(u64::MAX as u128))
                        .expect("u64 max"),
                )
                .expect("prior * max"),
            ),
            Proposition::Equal(
                third.clone(),
                ScalarTerm::exact_integer_multiply(
                    wide_type,
                    second,
                    ScalarTerm::integer(wide_type, IntegerValue::Unsigned(2)).expect("2u64"),
                )
                .expect("prior * 2"),
            ),
        ];
        assert_eq!(
            exact_integer_arithmetic_then_shift_chain_obligation(
                wide_type,
                third,
                1,
                &wide_definitions,
                wide_definitions.len(),
                &BTreeSet::from([wide_root_id]),
            ),
            None,
            "checked affine-composition failure admits no computed-shift family",
        );
    }

    #[test]
    fn exact_shift_then_arithmetic_chain_replays_shifts_before_each_affine_prefix() {
        let value_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let i8_count = IntegerType::new(IntegerSign::Signed, 8).expect("i8 count");
        let u16_count = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 count");
        let root_id = ValueId::new(361).expect("root");
        let root = ScalarTerm::value(root_id, ScalarType::Integer(value_type));
        let right = ScalarTerm::value(
            ValueId::new(362).expect("right"),
            ScalarType::Integer(value_type),
        );
        let left = ScalarTerm::value(
            ValueId::new(363).expect("left"),
            ScalarType::Integer(value_type),
        );
        let add = ScalarTerm::value(
            ValueId::new(364).expect("add"),
            ScalarType::Integer(value_type),
        );
        let definitions = vec![
            Proposition::Equal(
                right.clone(),
                ScalarTerm::exact_integer_shift_right(
                    value_type,
                    i8_count,
                    root.clone(),
                    ScalarTerm::integer(i8_count, IntegerValue::Signed(1)).expect("1i8"),
                )
                .expect("root >> 1"),
            ),
            Proposition::Equal(
                left.clone(),
                ScalarTerm::exact_integer_shift_left(
                    value_type,
                    u16_count,
                    right,
                    ScalarTerm::integer(u16_count, IntegerValue::Unsigned(2)).expect("2u16"),
                )
                .expect("(root >> 1) << 2"),
            ),
            Proposition::Equal(
                add.clone(),
                ScalarTerm::exact_integer_add(
                    value_type,
                    left,
                    ScalarTerm::integer(value_type, IntegerValue::Unsigned(3)).expect("3u8"),
                )
                .expect("shifted + 3"),
            ),
        ];
        assert_eq!(
            exact_integer_shift_then_arithmetic_chain_obligation(
                value_type,
                add.clone(),
                IntegerValue::Unsigned(2),
                ExactIntegerAffineOperation::Multiply,
                &definitions,
                definitions.len(),
                &BTreeSet::from([root_id]),
            ),
            Some(Proposition::LessOrEqual(
                root.clone(),
                ScalarTerm::integer(value_type, IntegerValue::Unsigned(63)).expect("63u8"),
            )),
        );
        assert_eq!(
            exact_integer_shift_then_arithmetic_chain_obligation(
                value_type,
                add,
                IntegerValue::Unsigned(2),
                ExactIntegerAffineOperation::Multiply,
                &[Proposition::Truth],
                1,
                &BTreeSet::from([root_id]),
            ),
            None,
            "stale definitions cannot authorize the shift-rooted affine suffix",
        );

        let shifted = ScalarTerm::value(
            ValueId::new(365).expect("shifted"),
            ScalarType::Integer(value_type),
        );
        let zero = ScalarTerm::value(
            ValueId::new(366).expect("zero"),
            ScalarType::Integer(value_type),
        );
        let zero_definitions = vec![
            Proposition::Equal(
                shifted.clone(),
                ScalarTerm::exact_integer_shift_left(
                    value_type,
                    i8_count,
                    root,
                    ScalarTerm::integer(i8_count, IntegerValue::Signed(1)).expect("1i8"),
                )
                .expect("root << 1"),
            ),
            Proposition::Equal(
                zero.clone(),
                ScalarTerm::exact_integer_multiply(
                    value_type,
                    shifted,
                    ScalarTerm::integer(value_type, IntegerValue::Unsigned(0)).expect("0u8"),
                )
                .expect("shifted * 0"),
            ),
        ];
        assert_eq!(
            exact_integer_shift_then_arithmetic_chain_obligation(
                value_type,
                zero,
                IntegerValue::Unsigned(255),
                ExactIntegerAffineOperation::Add,
                &zero_definitions,
                zero_definitions.len(),
                &BTreeSet::from([root_id]),
            ),
            Some(Proposition::Truth),
            "A=0 decides only this outer obligation after validating the complete shift root",
        );
    }

    #[test]
    fn exact_mixed_shift_preimage_distinguishes_empty_from_arithmetic_failure() {
        let value_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
        let count_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 count");
        let one = ScalarTerm::integer(count_type, IntegerValue::Unsigned(1)).expect("1u8");
        let value = ScalarTerm::integer(value_type, IntegerValue::Signed(0)).expect("0i64");
        let left = ScalarTerm::exact_integer_shift_left(
            value_type,
            count_type,
            value.clone(),
            one.clone(),
        )
        .expect("left shape");
        assert_eq!(
            exact_integer_mixed_shift_preimage(value_type, (1, 1), &left, 1),
            Ok(None),
            "a mathematically empty inverse interval is an ordinary false proposition",
        );
        let right = ScalarTerm::exact_integer_shift_right(value_type, count_type, value, one)
            .expect("right shape");
        assert_eq!(
            exact_integer_mixed_shift_preimage(value_type, (i128::MAX, i128::MAX), &right, 1),
            Err(()),
            "checked interval arithmetic failure is not admitted as a false proposition",
        );
    }

    #[test]
    fn exact_shift_left_chain_reconstructs_cumulative_parameter_bounds() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 value");
        let i8_count_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 count");
        let u16_count_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 count");
        let i32_count_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32 count");
        let root = ScalarTerm::value(ValueId::new(1).expect("root"), ScalarType::Integer(u8_type));
        let first = ScalarTerm::value(
            ValueId::new(2).expect("first"),
            ScalarType::Integer(u8_type),
        );
        let second = ScalarTerm::value(
            ValueId::new(3).expect("second"),
            ScalarType::Integer(u8_type),
        );
        let one = ScalarTerm::integer(i8_count_type, IntegerValue::Signed(1)).expect("1i8");
        let two = ScalarTerm::integer(u16_count_type, IntegerValue::Unsigned(2)).expect("2u16");
        let zero = ScalarTerm::integer(i32_count_type, IntegerValue::Signed(0)).expect("0i32");
        let axioms = vec![
            Proposition::Equal(
                first.clone(),
                ScalarTerm::exact_integer_shift_left(u8_type, i8_count_type, root.clone(), one)
                    .expect("root << 1i8"),
            ),
            Proposition::Equal(
                second.clone(),
                ScalarTerm::exact_integer_shift_left(u8_type, u16_count_type, first, two)
                    .expect("first << 2u16"),
            ),
        ];
        let thirty_one = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(31)).expect("31u8");
        assert_eq!(
            exact_integer_shift_left_obligation(
                u8_type,
                i32_count_type,
                second,
                zero,
                &axioms,
                axioms.len(),
                &BTreeSet::from([ValueId::new(1).expect("root")]),
            ),
            Proposition::LessOrEqual(root, thirty_one)
        );

        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 value");
        let signed_root = ScalarTerm::value(
            ValueId::new(4).expect("signed root"),
            ScalarType::Integer(i8_type),
        );
        let signed_first = ScalarTerm::value(
            ValueId::new(5).expect("signed first"),
            ScalarType::Integer(i8_type),
        );
        let one = ScalarTerm::integer(i8_count_type, IntegerValue::Signed(1)).expect("1i8");
        let two = ScalarTerm::integer(u16_count_type, IntegerValue::Unsigned(2)).expect("2u16");
        let signed_axioms = vec![Proposition::Equal(
            signed_first.clone(),
            ScalarTerm::exact_integer_shift_left(i8_type, i8_count_type, signed_root.clone(), one)
                .expect("signed root << 1i8"),
        )];
        let negative_sixteen =
            ScalarTerm::integer(i8_type, IntegerValue::Signed(-16)).expect("-16i8");
        let fifteen = ScalarTerm::integer(i8_type, IntegerValue::Signed(15)).expect("15i8");
        assert_eq!(
            exact_integer_shift_left_obligation(
                i8_type,
                u16_count_type,
                signed_first,
                two,
                &signed_axioms,
                signed_axioms.len(),
                &BTreeSet::from([ValueId::new(4).expect("signed root")]),
            ),
            canonical_conjunction(vec![
                Proposition::LessOrEqual(negative_sixteen, signed_root.clone()),
                Proposition::LessOrEqual(signed_root, fifteen),
            ])
        );
    }

    #[test]
    fn exact_shift_left_chain_rejects_broken_definitions_and_counts() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 value");
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 value");
        let count_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 count");
        let signed_count_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 count");
        let root_id = ValueId::new(1).expect("root");
        let root = ScalarTerm::value(root_id, ScalarType::Integer(u8_type));
        let first = ScalarTerm::value(
            ValueId::new(2).expect("first"),
            ScalarType::Integer(u8_type),
        );
        let second = ScalarTerm::value(
            ValueId::new(3).expect("second"),
            ScalarType::Integer(u8_type),
        );
        let local = ScalarTerm::value(
            ValueId::new(4).expect("local"),
            ScalarType::Integer(u8_type),
        );
        let one = ScalarTerm::integer(count_type, IntegerValue::Unsigned(1)).expect("1u8");
        let two = ScalarTerm::integer(count_type, IntegerValue::Unsigned(2)).expect("2u8");
        let first_definition = Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_shift_left(u8_type, count_type, root.clone(), one.clone())
                .expect("root << 1u8"),
        );
        let second_definition = Proposition::Equal(
            second.clone(),
            ScalarTerm::exact_integer_shift_left(u8_type, count_type, first.clone(), two.clone())
                .expect("first << 2u8"),
        );
        let parameters = BTreeSet::from([root_id]);
        let reconstruct = |axioms: &[Proposition], parameters: &BTreeSet<ValueId>| {
            exact_integer_shift_left_chain_obligation(
                u8_type,
                second.clone(),
                0,
                axioms,
                axioms.len(),
                parameters,
            )
        };
        assert_eq!(
            reconstruct(
                &[first_definition.clone(), second_definition.clone()],
                &parameters,
            ),
            Some(Proposition::LessOrEqual(
                root.clone(),
                ScalarTerm::integer(u8_type, IntegerValue::Unsigned(31)).expect("31u8"),
            ))
        );
        assert_eq!(
            reconstruct(
                &[second_definition.clone(), first_definition.clone()],
                &parameters,
            ),
            None,
            "a definition may not be recovered from outside the shrinking prefix"
        );

        let reversed_first = Proposition::Equal(
            ScalarTerm::exact_integer_shift_left(u8_type, count_type, root.clone(), one.clone())
                .expect("root << 1u8"),
            first.clone(),
        );
        assert_eq!(
            reconstruct(&[reversed_first, second_definition.clone()], &parameters),
            None,
            "a symmetric equality is not operation-definition authority"
        );

        let redirected_second = Proposition::Equal(
            second.clone(),
            ScalarTerm::exact_integer_shift_left(u8_type, count_type, local.clone(), two.clone())
                .expect("local << 2u8"),
        );
        assert_eq!(
            reconstruct(&[first_definition.clone(), redirected_second], &parameters),
            None
        );
        assert_eq!(
            reconstruct(
                &[first_definition.clone(), second_definition.clone()],
                &BTreeSet::new(),
            ),
            None,
            "a local or block parameter is not a machine-parameter root"
        );

        let cyclic_first = Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_shift_left(u8_type, count_type, second.clone(), one.clone())
                .expect("second << 1u8"),
        );
        assert_eq!(
            reconstruct(&[cyclic_first, second_definition.clone()], &parameters),
            None
        );

        let negative =
            ScalarTerm::integer(signed_count_type, IntegerValue::Signed(-1)).expect("-1i8");
        let negative_first = Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_shift_left(
                u8_type,
                signed_count_type,
                root.clone(),
                negative,
            )
            .expect("root << -1i8 remains a proposition term"),
        );
        assert_eq!(
            reconstruct(&[negative_first, second_definition.clone()], &parameters),
            None
        );

        let eight = ScalarTerm::integer(count_type, IntegerValue::Unsigned(8)).expect("8u8");
        let out_of_range_first = Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_shift_left(u8_type, count_type, root.clone(), eight)
                .expect("root << 8u8 remains a proposition term"),
        );
        assert_eq!(
            reconstruct(
                &[out_of_range_first, second_definition.clone()],
                &parameters,
            ),
            None
        );

        let computed_count = ScalarTerm::value(
            ValueId::new(5).expect("computed count"),
            ScalarType::Integer(count_type),
        );
        let computed_first = Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_shift_left(u8_type, count_type, root.clone(), computed_count)
                .expect("root << computed count"),
        );
        assert_eq!(
            reconstruct(&[computed_first, second_definition.clone()], &parameters),
            None
        );

        let signed_root = ScalarTerm::value(
            ValueId::new(6).expect("signed root"),
            ScalarType::Integer(i8_type),
        );
        let mismatched_first = Proposition::Equal(
            first,
            ScalarTerm::exact_integer_shift_left(
                i8_type,
                signed_count_type,
                signed_root,
                ScalarTerm::integer(signed_count_type, IntegerValue::Signed(1)).expect("1i8"),
            )
            .expect("signed root << 1i8"),
        );
        assert_eq!(
            reconstruct(&[mismatched_first, second_definition], &parameters),
            None
        );
    }

    #[test]
    fn exact_shift_left_chain_handles_zero_width_and_count_overflow() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 value");
        let count_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 count");
        let root = ScalarTerm::value(ValueId::new(1).expect("root"), ScalarType::Integer(u8_type));
        let first = ScalarTerm::value(
            ValueId::new(2).expect("first"),
            ScalarType::Integer(u8_type),
        );
        let four = ScalarTerm::integer(count_type, IntegerValue::Unsigned(4)).expect("4u8");
        let zero = ScalarTerm::integer(count_type, IntegerValue::Unsigned(0)).expect("0u8");
        let width_axioms = vec![Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_shift_left(u8_type, count_type, root.clone(), four.clone())
                .expect("root << 4u8"),
        )];
        let zero_value = ScalarTerm::integer(u8_type, IntegerValue::Unsigned(0)).expect("0u8");
        assert_eq!(
            exact_integer_shift_left_obligation(
                u8_type,
                count_type,
                first.clone(),
                four,
                &width_axioms,
                width_axioms.len(),
                &BTreeSet::from([ValueId::new(1).expect("root")]),
            ),
            Proposition::LessOrEqual(root.clone(), zero_value)
        );
        let identity_axioms = vec![Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_shift_left(u8_type, count_type, root, zero.clone())
                .expect("root << 0u8"),
        )];
        assert_eq!(
            exact_integer_shift_left_obligation(
                u8_type,
                count_type,
                first.clone(),
                zero,
                &identity_axioms,
                identity_axioms.len(),
                &BTreeSet::from([ValueId::new(1).expect("root")]),
            ),
            Proposition::Truth
        );
        assert_eq!(
            exact_integer_shift_left_chain_obligation(
                u8_type,
                first,
                u128::MAX,
                &width_axioms,
                width_axioms.len(),
                &BTreeSet::from([ValueId::new(1).expect("root")]),
            ),
            Some(Proposition::Falsehood)
        );
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 value");
        let signed_root = ScalarTerm::value(
            ValueId::new(3).expect("signed root"),
            ScalarType::Integer(i8_type),
        );
        let signed_zero = ScalarTerm::integer(i8_type, IntegerValue::Signed(0)).expect("0i8");
        assert_eq!(
            exact_integer_cumulative_shift_left_obligation(i8_type, signed_root.clone(), 8),
            Proposition::Conjunction(vec![
                Proposition::LessOrEqual(signed_zero.clone(), signed_root.clone()),
                Proposition::LessOrEqual(signed_root, signed_zero),
            ])
        );
    }

    #[test]
    fn exact_shift_left_chain_after_partial_cast_reconstructs_source_intersections() {
        let source_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
        let target_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let i8_count_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 count");
        let u16_count_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 count");
        let i32_count_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32 count");
        let root_id = ValueId::new(241).expect("root");
        let root = ScalarTerm::value(root_id, ScalarType::Integer(source_type));
        let cast = ScalarTerm::value(
            ValueId::new(242).expect("cast"),
            ScalarType::Integer(target_type),
        );
        let first = ScalarTerm::value(
            ValueId::new(243).expect("first shift"),
            ScalarType::Integer(target_type),
        );
        let second = ScalarTerm::value(
            ValueId::new(244).expect("second shift"),
            ScalarType::Integer(target_type),
        );
        let cast_definition = Proposition::Equal(
            cast.clone(),
            ScalarTerm::integer_exact_cast(source_type, target_type, root.clone())
                .expect("u16 to u8 exact cast"),
        );
        let one = ScalarTerm::integer(i8_count_type, IntegerValue::Signed(1)).expect("1i8");
        let two = ScalarTerm::integer(u16_count_type, IntegerValue::Unsigned(2)).expect("2u16");
        let first_definition = Proposition::Equal(
            first.clone(),
            ScalarTerm::exact_integer_shift_left(
                target_type,
                i8_count_type,
                cast.clone(),
                one.clone(),
            )
            .expect("cast << 1i8"),
        );
        let second_definition = Proposition::Equal(
            second.clone(),
            ScalarTerm::exact_integer_shift_left(
                target_type,
                u16_count_type,
                first.clone(),
                two.clone(),
            )
            .expect("(cast << 1i8) << 2u16"),
        );
        let parameters = BTreeSet::from([root_id]);
        assert_eq!(
            exact_integer_shift_left_obligation(
                target_type,
                i8_count_type,
                cast.clone(),
                one,
                std::slice::from_ref(&cast_definition),
                1,
                &parameters,
            ),
            Proposition::LessOrEqual(
                root.clone(),
                ScalarTerm::integer(source_type, IntegerValue::Unsigned(127)).expect("127u16"),
            )
        );
        assert_eq!(
            exact_integer_shift_left_obligation(
                target_type,
                u16_count_type,
                first,
                two,
                &[cast_definition.clone(), first_definition.clone()],
                2,
                &parameters,
            ),
            Proposition::LessOrEqual(
                root.clone(),
                ScalarTerm::integer(source_type, IntegerValue::Unsigned(31)).expect("31u16"),
            )
        );
        assert_eq!(
            exact_integer_shift_left_obligation(
                target_type,
                i32_count_type,
                second.clone(),
                ScalarTerm::integer(i32_count_type, IntegerValue::Signed(5)).expect("5i32"),
                &[
                    cast_definition.clone(),
                    first_definition.clone(),
                    second_definition.clone(),
                ],
                3,
                &parameters,
            ),
            Proposition::LessOrEqual(
                root.clone(),
                ScalarTerm::integer(source_type, IntegerValue::Unsigned(0)).expect("0u16"),
            ),
            "a cumulative count at the target width requires the source root to be zero"
        );
        assert_eq!(
            exact_integer_shift_left_obligation(
                target_type,
                i32_count_type,
                cast.clone(),
                ScalarTerm::integer(i32_count_type, IntegerValue::Signed(0)).expect("0i32"),
                std::slice::from_ref(&cast_definition),
                1,
                &parameters,
            ),
            Proposition::Truth
        );

        let signed_source = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
        let signed_target = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let signed_root_id = ValueId::new(251).expect("signed root");
        let signed_root = ScalarTerm::value(signed_root_id, ScalarType::Integer(signed_source));
        let signed_cast = ScalarTerm::value(
            ValueId::new(252).expect("signed cast"),
            ScalarType::Integer(signed_target),
        );
        let signed_definition = Proposition::Equal(
            signed_cast.clone(),
            ScalarTerm::integer_exact_cast(signed_source, signed_target, signed_root.clone())
                .expect("i16 to i8 exact cast"),
        );
        assert_eq!(
            exact_integer_shift_left_obligation(
                signed_target,
                u16_count_type,
                signed_cast,
                ScalarTerm::integer(u16_count_type, IntegerValue::Unsigned(3)).expect("3u16"),
                std::slice::from_ref(&signed_definition),
                1,
                &BTreeSet::from([signed_root_id]),
            ),
            canonical_conjunction(vec![
                Proposition::LessOrEqual(
                    ScalarTerm::integer(signed_source, IntegerValue::Signed(-16)).expect("-16i16"),
                    signed_root.clone(),
                ),
                Proposition::LessOrEqual(
                    signed_root,
                    ScalarTerm::integer(signed_source, IntegerValue::Signed(15)).expect("15i16"),
                ),
            ])
        );

        let cross_source = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let cross_target = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let cross_root_id = ValueId::new(261).expect("cross root");
        let cross_root = ScalarTerm::value(cross_root_id, ScalarType::Integer(cross_source));
        let cross_cast = ScalarTerm::value(
            ValueId::new(262).expect("cross cast"),
            ScalarType::Integer(cross_target),
        );
        let cross_definition = Proposition::Equal(
            cross_cast.clone(),
            ScalarTerm::integer_exact_cast(cross_source, cross_target, cross_root.clone())
                .expect("i8 to u8 exact cast"),
        );
        assert_eq!(
            exact_integer_shift_left_obligation(
                cross_target,
                i8_count_type,
                cross_cast,
                ScalarTerm::integer(i8_count_type, IntegerValue::Signed(1)).expect("1i8"),
                std::slice::from_ref(&cross_definition),
                1,
                &BTreeSet::from([cross_root_id]),
            ),
            Proposition::LessOrEqual(
                ScalarTerm::integer(cross_source, IntegerValue::Signed(0)).expect("0i8"),
                cross_root,
            )
        );

        let reversed_cast_definition = match cast_definition.clone() {
            Proposition::Equal(left, right) => Proposition::Equal(right, left),
            _ => unreachable!("cast definition is an equality"),
        };
        assert_ne!(
            exact_integer_shift_left_obligation(
                target_type,
                i8_count_type,
                cast,
                ScalarTerm::integer(i8_count_type, IntegerValue::Signed(1)).expect("1i8"),
                std::slice::from_ref(&reversed_cast_definition),
                1,
                &parameters,
            ),
            Proposition::LessOrEqual(
                root,
                ScalarTerm::integer(source_type, IntegerValue::Unsigned(127)).expect("127u16"),
            )
        );
        assert_eq!(
            exact_integer_cast_then_shift_left_chain_obligation(
                target_type,
                second,
                u128::MAX,
                &[cast_definition, first_definition, second_definition],
                3,
                &parameters,
            ),
            Some(Proposition::Falsehood),
            "cumulative-count overflow fails closed"
        );
    }

    #[test]
    fn wrapping_divide_reconstructs_known_nonzero_divisor_safety() {
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let value = ScalarTerm::value(
            ValueId::new(1).expect("value"),
            ScalarType::Integer(i8_type),
        );
        let negative_one = ScalarTerm::integer(i8_type, IntegerValue::Signed(-1)).unwrap();
        assert_eq!(
            wrapping_integer_divide_obligation(i8_type, value.clone(), negative_one, &[]),
            Proposition::Truth
        );
        let zero = ScalarTerm::integer(i8_type, IntegerValue::Signed(0)).unwrap();
        assert_eq!(
            wrapping_integer_divide_obligation(i8_type, value, zero, &[]),
            Proposition::Falsehood
        );
        let minimum = ScalarTerm::integer(i8_type, IntegerValue::Signed(-128)).unwrap();
        let negative_one = ScalarTerm::integer(i8_type, IntegerValue::Signed(-1)).unwrap();
        assert_eq!(
            wrapping_integer_divide_obligation(i8_type, minimum, negative_one, &[]),
            Proposition::Truth
        );
        let unknown = ScalarTerm::value(
            ValueId::new(2).expect("divisor"),
            ScalarType::Integer(i8_type),
        );
        let one = ScalarTerm::integer(i8_type, IntegerValue::Signed(1)).unwrap();
        assert_eq!(
            wrapping_integer_divide_obligation(i8_type, one.clone(), unknown.clone(), &[]),
            Proposition::LessOrEqual(one, unknown)
        );
    }

    #[test]
    fn wrapping_remainder_reconstructs_known_nonzero_divisor_safety() {
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let value = ScalarTerm::value(
            ValueId::new(1).expect("value"),
            ScalarType::Integer(i8_type),
        );
        let negative_one = ScalarTerm::integer(i8_type, IntegerValue::Signed(-1)).unwrap();
        assert_eq!(
            wrapping_integer_remainder_obligation(i8_type, value.clone(), negative_one, &[]),
            Proposition::Truth
        );
        let zero = ScalarTerm::integer(i8_type, IntegerValue::Signed(0)).unwrap();
        assert_eq!(
            wrapping_integer_remainder_obligation(i8_type, value, zero, &[]),
            Proposition::Falsehood
        );
        let minimum = ScalarTerm::integer(i8_type, IntegerValue::Signed(-128)).unwrap();
        let negative_one = ScalarTerm::integer(i8_type, IntegerValue::Signed(-1)).unwrap();
        assert_eq!(
            wrapping_integer_remainder_obligation(i8_type, minimum, negative_one, &[]),
            Proposition::Truth
        );
    }

    #[test]
    fn saturating_divide_reconstructs_known_nonzero_divisor_safety() {
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let value = ScalarTerm::value(
            ValueId::new(1).expect("value"),
            ScalarType::Integer(i8_type),
        );
        let negative_one = ScalarTerm::integer(i8_type, IntegerValue::Signed(-1)).unwrap();
        assert_eq!(
            saturating_integer_divide_obligation(i8_type, value.clone(), negative_one, &[]),
            Proposition::Truth
        );
        let zero = ScalarTerm::integer(i8_type, IntegerValue::Signed(0)).unwrap();
        assert_eq!(
            saturating_integer_divide_obligation(i8_type, value, zero, &[]),
            Proposition::Falsehood
        );
        let minimum = ScalarTerm::integer(i8_type, IntegerValue::Signed(-128)).unwrap();
        let negative_one = ScalarTerm::integer(i8_type, IntegerValue::Signed(-1)).unwrap();
        assert_eq!(
            saturating_integer_divide_obligation(i8_type, minimum, negative_one, &[]),
            Proposition::Truth
        );
    }

    #[test]
    fn saturating_remainder_reconstructs_known_nonzero_divisor_safety() {
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let value = ScalarTerm::value(
            ValueId::new(1).expect("value"),
            ScalarType::Integer(i8_type),
        );
        let negative_one = ScalarTerm::integer(i8_type, IntegerValue::Signed(-1)).unwrap();
        assert_eq!(
            saturating_integer_remainder_obligation(i8_type, value.clone(), negative_one, &[]),
            Proposition::Truth
        );
        let zero = ScalarTerm::integer(i8_type, IntegerValue::Signed(0)).unwrap();
        assert_eq!(
            saturating_integer_remainder_obligation(i8_type, value, zero, &[]),
            Proposition::Falsehood
        );
        let minimum = ScalarTerm::integer(i8_type, IntegerValue::Signed(-128)).unwrap();
        let negative_one = ScalarTerm::integer(i8_type, IntegerValue::Signed(-1)).unwrap();
        assert_eq!(
            saturating_integer_remainder_obligation(i8_type, minimum, negative_one, &[]),
            Proposition::Truth
        );
    }

    #[test]
    fn runtime_divisor_bounds_reconstruct_for_every_policy() {
        type Reconstruct = fn(IntegerType, ScalarTerm, ScalarTerm, &[Proposition]) -> Proposition;
        let reconstructors: [Reconstruct; 6] = [
            exact_integer_divide_obligation,
            exact_integer_remainder_obligation,
            wrapping_integer_divide_obligation,
            wrapping_integer_remainder_obligation,
            saturating_integer_divide_obligation,
            saturating_integer_remainder_obligation,
        ];
        let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let left = ScalarTerm::value(
            ValueId::new(10).expect("left"),
            ScalarType::Integer(integer_type),
        );
        let right = ScalarTerm::value(
            ValueId::new(11).expect("right"),
            ScalarType::Integer(integer_type),
        );
        let one = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(1)).expect("one");
        let expected = Proposition::LessOrEqual(one, right.clone());

        for &reconstruct in &reconstructors {
            assert_eq!(
                reconstruct(integer_type, left.clone(), right.clone(), &[],),
                expected
            );
        }

        let signed_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let left = ScalarTerm::value(
            ValueId::new(14).expect("left"),
            ScalarType::Integer(signed_type),
        );
        let right = ScalarTerm::value(
            ValueId::new(15).expect("right"),
            ScalarType::Integer(signed_type),
        );
        let negative_two =
            ScalarTerm::integer(signed_type, IntegerValue::Signed(-2)).expect("-2i8");
        let negative_bound = Proposition::LessOrEqual(right.clone(), negative_two);
        for &reconstruct in &reconstructors {
            assert_eq!(
                reconstruct(
                    signed_type,
                    left.clone(),
                    right.clone(),
                    std::slice::from_ref(&negative_bound),
                ),
                negative_bound.clone()
            );
        }

        let minimum_plus_one =
            ScalarTerm::integer(signed_type, IntegerValue::Signed(-127)).expect("-127i8");
        let negative_one =
            ScalarTerm::integer(signed_type, IntegerValue::Signed(-1)).expect("-1i8");
        let negative_one_bound = Proposition::LessOrEqual(right.clone(), negative_one);
        let dividend_bound = Proposition::LessOrEqual(minimum_plus_one, left.clone());
        let exact_expected =
            canonical_conjunction(vec![negative_one_bound.clone(), dividend_bound.clone()]);
        for (index, &reconstruct) in reconstructors.iter().enumerate() {
            let axioms = if index < 2 {
                vec![negative_one_bound.clone(), dividend_bound.clone()]
            } else {
                vec![negative_one_bound.clone()]
            };
            assert_eq!(
                reconstruct(signed_type, left.clone(), right.clone(), &axioms,),
                if index < 2 {
                    exact_expected.clone()
                } else {
                    negative_one_bound.clone()
                }
            );
        }

        let signed_one_bit = IntegerType::new(IntegerSign::Signed, 1).expect("i1");
        let left = ScalarTerm::value(
            ValueId::new(12).expect("left"),
            ScalarType::Integer(signed_one_bit),
        );
        let right = ScalarTerm::value(
            ValueId::new(13).expect("right"),
            ScalarType::Integer(signed_one_bit),
        );
        assert_eq!(
            exact_integer_divide_obligation(signed_one_bit, left, right, &[],),
            Proposition::Falsehood
        );
    }
}
