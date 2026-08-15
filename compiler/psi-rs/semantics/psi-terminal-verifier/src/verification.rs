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
                    let mut available_bounds = axioms.clone();
                    available_bounds.extend(machine.contract.requires.iter().cloned());
                    operation_obligations.push(ReconstructedOperationObligation {
                        obligation: Obligation {
                            id: obligation,
                            proposition: exact_integer_multiply_obligation(
                                integer_type,
                                value_term(left),
                                value_term(right),
                                &available_bounds,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IntegerOffset {
    Nonnegative(u128),
    Negative(u128),
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

fn exact_integer_multiply_obligation(
    integer_type: psi_core::IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
    semantic_axioms: &[Proposition],
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
    let (variable, constant) = match (known_left, known_right) {
        (Some(constant), None) => (right, constant),
        (None, Some(constant)) => (left, constant),
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
