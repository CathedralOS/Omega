use std::collections::{BTreeMap, BTreeSet};

use psi_core::{
    EvidenceIdentity, EvidenceTermId, ObligationId, Proposition, ScalarTerm, ScalarType, ValueId,
};
#[cfg(test)]
use psi_core::{IntegerSign, IntegerValue};
use psi_proof_kernel::{
    AcceptedFact, AdmissionProfile, EvidenceError, EvidenceRoute, Obligation, ObligationClass,
    verify_obligation,
};
use psi_terminal::{OperationKind, TerminalMachine, TerminalModule, Terminator};
use psi_terminal_semantics::goal_free_scalar_leaf_equation;

use crate::{ModuleError, ValidatedTerminalModule, validate_module};

mod affine_joins;
mod evidence_provenance;
mod integer_add_subtract;
mod integer_affine;
mod integer_conversion;
mod integer_divide_remainder;
mod integer_foundation;
mod integer_multiply;
mod integer_shift;
mod substitution;

use evidence_provenance::validate_evidence_producer_provenance;
use integer_foundation::*;
use substitution::{substitute_proposition_places, substitute_scalar_term_values};
pub(crate) use substitution::{
    substitute_proposition_structural_places, substitute_proposition_values,
};

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
        let semantics =
            reconstruct_machine_semantics(module, machine).map_err(VerificationError::Module)?;
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
fn reconstruct_machine_semantics(
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
                OperationKind::BooleanStructuralField { source, field } => {
                    axioms.push(Proposition::Equal(
                        value_term(operation.result.expect_scalar().id),
                        ScalarTerm::boolean_field(source, field),
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
