#![forbid(unsafe_code)]

//! Independent structural validation for [`PsiOptimizationUnit`].
//!
//! Pass implementations do not participate in this validator. Publication
//! must call it after applying a candidate and before committing the candidate
//! to the durable transformation ledger.

use std::collections::{BTreeMap, BTreeSet};

use omega_optimization_core::{
    AnalysisKind, OptimizationCandidateIdentity, OptimizationSafetyClass,
    OptimizationValidatorIdentity,
};
use omega_optimization_unit::{
    IntegerConstantRewrite, OptimizationEdge, OptimizationFact, OwnershipEvent,
    PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance, PsiRewriteCandidate,
    PsiRewritePatch, ValueDefinition, ValueDefinitionSite, ValueUse,
};
use psi_core::{BlockId, ClaimId, EdgeId, MachineId, PlaceId, ScalarType, ValueId};
use psi_terminal_fuel::TerminalFuelSchedule;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizationUnitValidationError {
    WrongFuelSchedule,
    MissingEntryMachine(MachineId),
    DuplicateMachine(MachineId),
    MissingEntryBlock {
        machine: MachineId,
        block: BlockId,
    },
    EntryBlockHasParameters {
        machine: MachineId,
        block: BlockId,
    },
    DuplicateBlock {
        machine: MachineId,
        block: BlockId,
    },
    EmptyBlock {
        machine: MachineId,
        block: BlockId,
    },
    TerminatorNotLast {
        machine: MachineId,
        block: BlockId,
    },
    MissingTerminator {
        machine: MachineId,
        block: BlockId,
    },
    UnknownSuccessor {
        machine: MachineId,
        block: BlockId,
        target: BlockId,
    },
    UnreachableBlock {
        machine: MachineId,
        block: BlockId,
    },
    ControlCycle {
        machine: MachineId,
        block: BlockId,
    },
    ParameterMetadataMismatch {
        machine: MachineId,
        block: Option<BlockId>,
    },
    DuplicateEdge(EdgeId),
    DuplicateProvenance(PsiProvenance),
    IncompleteProvenance {
        machine: MachineId,
        block: BlockId,
        node: u32,
    },
    FuelDoesNotMatchProvenance {
        machine: MachineId,
        block: BlockId,
        node: u32,
    },
    DuplicateFuelSettlement(PsiProvenance),
    OperationMetadataMismatch {
        machine: MachineId,
        block: BlockId,
        node: u32,
    },
    FactIndexMismatch(MachineId),
    BrokenEffectChain {
        machine: MachineId,
        expected: u64,
        actual: u64,
    },
    DuplicateValue(ValueId),
    UndefinedValue {
        machine: MachineId,
        block: BlockId,
        value: ValueId,
    },
    NondominatingValue {
        machine: MachineId,
        block: BlockId,
        value: ValueId,
    },
    UseBeforeDefinition {
        machine: MachineId,
        block: BlockId,
        value: ValueId,
    },
    BindingArityMismatch {
        machine: MachineId,
        edge: EdgeId,
    },
    BindingTypeMismatch {
        machine: MachineId,
        edge: EdgeId,
        value: ValueId,
    },
    UnknownPlace {
        machine: MachineId,
        place: PlaceId,
    },
    UnknownClaim {
        machine: MachineId,
        claim: ClaimId,
    },
    TerminalIdentityMismatch,
    ProofFingerprintMismatch,
    AcceptedObligationMismatch(psi_core::ObligationId),
    OperationObligationOwnerMismatch {
        machine: MachineId,
        operation: psi_core::OperationId,
        obligation: psi_core::ObligationId,
    },
    MissingStructuralFrontierMachine(MachineId),
    MissingStructuralOperationFrontier {
        machine: MachineId,
        operation: psi_core::OperationId,
    },
    MissingStructuralEdgeFrontier {
        machine: MachineId,
        edge: EdgeId,
    },
    ContextIdentity(psi_terminal_codec::CodecError),
    ContextProofFingerprint(psi_terminal_codec::ProofCodecError),
    CandidateInputMismatch,
    CandidateAnalysisContractMismatch,
    CandidateSafetyClassMismatch,
    CandidateLocationMissing,
    CandidatePatchMismatch,
    CandidateProvenanceMismatch,
    CandidateFuelMismatch,
    CandidateOperandFactMismatch,
    CandidateEvaluationMismatch,
    CandidateFactReplacementMissing,
}

impl std::fmt::Display for OptimizationUnitValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid Psi optimization unit: {self:?}")
    }
}

impl std::error::Error for OptimizationUnitValidationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPsiRewrite {
    unit: PsiOptimizationUnit,
    candidate: OptimizationCandidateIdentity,
    validator: OptimizationValidatorIdentity,
}

impl ValidatedPsiRewrite {
    pub const fn unit(&self) -> &PsiOptimizationUnit {
        &self.unit
    }

    pub const fn candidate(&self) -> OptimizationCandidateIdentity {
        self.candidate
    }

    pub const fn validator(&self) -> OptimizationValidatorIdentity {
        self.validator
    }

    pub fn into_unit(self) -> PsiOptimizationUnit {
        self.unit
    }
}

pub fn validate_psi_optimization_unit(
    unit: &PsiOptimizationUnit,
) -> Result<(), OptimizationUnitValidationError> {
    if unit.fuel_schedule != TerminalFuelSchedule::CURRENT.identity() {
        return Err(OptimizationUnitValidationError::WrongFuelSchedule);
    }
    let mut machines = BTreeSet::new();
    for function in &unit.functions {
        if !machines.insert(function.machine) {
            return Err(OptimizationUnitValidationError::DuplicateMachine(
                function.machine,
            ));
        }
        validate_function(function)?;
    }
    if !machines.contains(&unit.entry) {
        return Err(OptimizationUnitValidationError::MissingEntryMachine(
            unit.entry,
        ));
    }
    Ok(())
}

/// Independently check and construct one exact integer-evaluation rewrite.
/// The proposing rule never receives a mutable unit and cannot construct the
/// accepted output itself.
pub fn validate_integer_evaluation_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    if !candidate
        .required_analyses()
        .contains(AnalysisKind::ScalarConstants)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate.substitutions().is_empty()
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    if candidate.safety_class() != OptimizationSafetyClass::ProofCertified {
        return Err(OptimizationUnitValidationError::CandidateSafetyClassMismatch);
    }
    let PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) = candidate.patch();
    if candidate.decision_point() != patch.location {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = input
        .functions
        .iter()
        .find(|function| function.machine == patch.location.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == patch.location.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let node = block
        .nodes
        .get(usize::try_from(patch.location.node).expect("u32 fits usize"))
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let [provenance] = candidate.provenance() else {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    };
    if provenance.output != patch.location || provenance.sources != node.provenance {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }
    if provenance.fuel != node.fuel {
        return Err(OptimizationUnitValidationError::CandidateFuelMismatch);
    }

    let (source_operation, result, scalar_type, left, right, evaluated) =
        evaluate_exact_binary(function, node, candidate)?;
    if patch
        != (IntegerConstantRewrite {
            location: patch.location,
            source_operation,
            result,
            scalar_type,
            constant: evaluated,
        })
    {
        return Err(OptimizationUnitValidationError::CandidateEvaluationMismatch);
    }
    let _ = (left, right);

    let mut output = input.clone();
    let function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.location.machine)
        .expect("candidate source function exists");
    let block = function
        .blocks
        .iter_mut()
        .find(|block| block.id == patch.location.block)
        .expect("candidate source block exists");
    let node = &mut block.nodes[usize::try_from(patch.location.node).expect("u32 fits usize")];
    node.operation =
        omega_terminal_abstract_operations::TerminalAbstractOperation::IntegerConstant {
            psi_operation: patch.source_operation,
            result: patch.result,
            scalar_type: ScalarType::Integer(patch.scalar_type),
            value: patch.constant,
        };
    node.definitions = vec![ValueDefinition {
        value: patch.result,
        scalar_type: ScalarType::Integer(patch.scalar_type),
        site: ValueDefinitionSite::Node {
            block: patch.location.block,
            node: patch.location.node,
        },
    }];
    node.uses.clear();
    node.successors.clear();
    node.ownership.clear();
    let Some(fact) = function.facts.iter_mut().find(|fact| {
        matches!(
            fact,
            OptimizationFact::OperationObligationReference { support, .. }
                if *support == patch.source_operation
        )
    }) else {
        return Err(OptimizationUnitValidationError::CandidateFactReplacementMissing);
    };
    *fact = OptimizationFact::IntegerConstant {
        value: patch.result,
        constant: patch.constant,
        support: patch.source_operation,
    };
    output.identity = candidate.output();
    validate_psi_optimization_unit(&output)?;
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.exact-integer-evaluation.v1",
        ),
    })
}

fn evaluate_exact_binary(
    function: &PsiOptimizationFunction,
    node: &omega_optimization_unit::OptimizationNode,
    candidate: &PsiRewriteCandidate,
) -> Result<
    (
        psi_core::OperationId,
        ValueId,
        psi_core::IntegerType,
        ValueId,
        ValueId,
        psi_core::IntegerValue,
    ),
    OptimizationUnitValidationError,
> {
    use omega_terminal_abstract_operations::TerminalAbstractOperation as O;
    enum ExactOperation {
        Add,
        Subtract,
        Multiply,
    }
    let (kind, source, result, scalar_type, left, right) = match &node.operation {
        O::ExactIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            ExactOperation::Add,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            ExactOperation::Subtract,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            ExactOperation::Multiply,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        _ => return Err(OptimizationUnitValidationError::CandidatePatchMismatch),
    };
    let witness = candidate.witness();
    let left_value = literal_integer_fact(function, left, witness.left_support)
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    let right_value = literal_integer_fact(function, right, witness.right_support)
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    let evaluated = match kind {
        ExactOperation::Add => scalar_type.exact_add(left_value, right_value),
        ExactOperation::Subtract => scalar_type.exact_sub(left_value, right_value),
        ExactOperation::Multiply => scalar_type.exact_mul(left_value, right_value),
    }
    .ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;
    Ok((source, result, scalar_type, left, right, evaluated))
}

fn literal_integer_fact(
    function: &PsiOptimizationFunction,
    value: ValueId,
    support: psi_core::OperationId,
) -> Option<psi_core::IntegerValue> {
    function.facts.iter().find_map(|fact| match fact {
        OptimizationFact::IntegerConstant {
            value: fact_value,
            constant,
            support: fact_support,
        } if *fact_value == value && *fact_support == support => Some(*constant),
        _ => None,
    })
}

/// Independently validate both the reconstructible unit and the required
/// verifier context retained by the optimizer-facing constructor.
pub fn validate_verified_psi_optimization_unit(
    verified: &omega_terminal_psi_to_abstract_operations::VerifiedPsiOptimizationUnit,
) -> Result<(), OptimizationUnitValidationError> {
    let unit = verified.unit();
    validate_psi_optimization_unit(unit)?;
    let input = verified.input();
    let context = input.context();
    let terminal_identity = psi_terminal_codec::terminal_psi_identity(context.terminal_module())
        .map_err(OptimizationUnitValidationError::ContextIdentity)?;
    if input.plan().terminal_psi != terminal_identity || unit.terminal_psi != terminal_identity {
        return Err(OptimizationUnitValidationError::TerminalIdentityMismatch);
    }
    let proof_fingerprint = psi_terminal_codec::proof_bundle_fingerprint(context.proof_bundle())
        .map_err(OptimizationUnitValidationError::ContextProofFingerprint)?;
    if proof_fingerprint != context.proof_bundle_fingerprint() {
        return Err(OptimizationUnitValidationError::ProofFingerprintMismatch);
    }

    let reconstructed = context
        .reconstructed_obligations()
        .obligations()
        .iter()
        .map(|row| (row.obligation.id, row))
        .collect::<BTreeMap<_, _>>();
    let accepted = context
        .accepted_facts()
        .iter()
        .map(|fact| (fact.obligation, fact))
        .collect::<BTreeMap<_, _>>();
    if reconstructed.len() != accepted.len() {
        let obligation = reconstructed
            .keys()
            .find(|id| !accepted.contains_key(id))
            .or_else(|| accepted.keys().find(|id| !reconstructed.contains_key(id)))
            .copied()
            .expect("different finite obligation maps have a differing key");
        return Err(OptimizationUnitValidationError::AcceptedObligationMismatch(
            obligation,
        ));
    }
    for (obligation, row) in &reconstructed {
        if accepted
            .get(obligation)
            .is_none_or(|fact| fact.proposition != row.obligation.proposition)
        {
            return Err(OptimizationUnitValidationError::AcceptedObligationMismatch(
                *obligation,
            ));
        }
    }

    for function in &unit.functions {
        let Some(frontiers) = context.structural_frontiers().machine(function.machine) else {
            return Err(
                OptimizationUnitValidationError::MissingStructuralFrontierMachine(function.machine),
            );
        };
        for fact in &function.facts {
            let OptimizationFact::OperationObligationReference {
                obligation,
                support,
            } = fact
            else {
                continue;
            };
            let owner_matches = reconstructed.get(obligation).is_some_and(|row| {
                row.owner
                    == psi_terminal_verifier::ReconstructedTerminalObligationOwner::Operation {
                        machine: function.machine,
                        operation: *support,
                    }
            });
            if !owner_matches || !accepted.contains_key(obligation) {
                return Err(
                    OptimizationUnitValidationError::OperationObligationOwnerMismatch {
                        machine: function.machine,
                        operation: *support,
                        obligation: *obligation,
                    },
                );
            }
        }
        for site in function.blocks.iter().flat_map(|block| {
            block
                .nodes
                .iter()
                .flat_map(|node| node.provenance.iter().copied())
        }) {
            match site {
                PsiProvenance::Operation(operation)
                    if frontiers.operation_entry(operation).is_none()
                        || frontiers.operation_exit(operation).is_none() =>
                {
                    return Err(
                        OptimizationUnitValidationError::MissingStructuralOperationFrontier {
                            machine: function.machine,
                            operation,
                        },
                    );
                }
                PsiProvenance::Edge(edge) if frontiers.edge_entry(edge).is_none() => {
                    return Err(
                        OptimizationUnitValidationError::MissingStructuralEdgeFrontier {
                            machine: function.machine,
                            edge,
                        },
                    );
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn validate_function(
    function: &PsiOptimizationFunction,
) -> Result<(), OptimizationUnitValidationError> {
    let mut blocks = BTreeMap::new();
    for block in &function.blocks {
        if blocks.insert(block.id, block).is_some() {
            return Err(OptimizationUnitValidationError::DuplicateBlock {
                machine: function.machine,
                block: block.id,
            });
        }
    }
    if !blocks.contains_key(&function.entry) {
        return Err(OptimizationUnitValidationError::MissingEntryBlock {
            machine: function.machine,
            block: function.entry,
        });
    }
    if !blocks[&function.entry].parameters.is_empty() {
        return Err(OptimizationUnitValidationError::EntryBlockHasParameters {
            machine: function.machine,
            block: function.entry,
        });
    }
    validate_parameter_metadata(function)?;

    let mut edge_ids = BTreeSet::new();
    let mut predecessor = function
        .blocks
        .iter()
        .map(|block| (block.id, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    let mut successors = function
        .blocks
        .iter()
        .map(|block| (block.id, Vec::new()))
        .collect::<BTreeMap<_, _>>();
    for block in &function.blocks {
        if block.nodes.is_empty() {
            return Err(OptimizationUnitValidationError::EmptyBlock {
                machine: function.machine,
                block: block.id,
            });
        }
        for (index, node) in block.nodes.iter().enumerate() {
            let node_index = u32::try_from(index).expect("unit node index was built as u32");
            if node.provenance != expected_provenance(&node.operation)
                || node.definitions != expected_definitions(&node.operation, block.id, node_index)
                || node.uses != expected_uses(&node.operation, block.id, node_index)
                || node.successors != expected_edges(&node.operation)
                || node.ownership != expected_ownership(&node.operation)
            {
                return Err(OptimizationUnitValidationError::OperationMetadataMismatch {
                    machine: function.machine,
                    block: block.id,
                    node: node_index,
                });
            }
            let terminal = is_terminator(&node.operation);
            if terminal && index + 1 != block.nodes.len() {
                return Err(OptimizationUnitValidationError::TerminatorNotLast {
                    machine: function.machine,
                    block: block.id,
                });
            }
            for edge in &node.successors {
                if !blocks.contains_key(&edge.target) {
                    return Err(OptimizationUnitValidationError::UnknownSuccessor {
                        machine: function.machine,
                        block: block.id,
                        target: edge.target,
                    });
                }
                if !edge_ids.insert(edge.psi_edge) {
                    return Err(OptimizationUnitValidationError::DuplicateEdge(
                        edge.psi_edge,
                    ));
                }
                predecessor
                    .get_mut(&edge.target)
                    .expect("known target")
                    .insert(block.id);
                successors
                    .get_mut(&block.id)
                    .expect("every block has a successor row")
                    .push(edge.target);
            }
        }
        if !is_terminator(&block.nodes.last().expect("nonempty").operation) {
            return Err(OptimizationUnitValidationError::MissingTerminator {
                machine: function.machine,
                block: block.id,
            });
        }
    }

    validate_total_cfg(function, &blocks, &successors)?;

    validate_provenance_fuel_effects(function)?;
    validate_fact_index(function)?;
    validate_values_and_bindings(function, &blocks, &predecessor)?;
    validate_places_and_claims(function)?;
    Ok(())
}

fn validate_parameter_metadata(
    function: &PsiOptimizationFunction,
) -> Result<(), OptimizationUnitValidationError> {
    for (position, parameter) in function.parameters.iter().enumerate() {
        let Ok(position) = u32::try_from(position) else {
            return Err(OptimizationUnitValidationError::ParameterMetadataMismatch {
                machine: function.machine,
                block: None,
            });
        };
        if parameter.site != ValueDefinitionSite::FunctionParameter(position) {
            return Err(OptimizationUnitValidationError::ParameterMetadataMismatch {
                machine: function.machine,
                block: None,
            });
        }
    }
    for block in &function.blocks {
        for (position, parameter) in block.parameters.iter().enumerate() {
            let Ok(position) = u32::try_from(position) else {
                return Err(OptimizationUnitValidationError::ParameterMetadataMismatch {
                    machine: function.machine,
                    block: Some(block.id),
                });
            };
            if parameter.site
                != (ValueDefinitionSite::BlockParameter {
                    block: block.id,
                    position,
                })
            {
                return Err(OptimizationUnitValidationError::ParameterMetadataMismatch {
                    machine: function.machine,
                    block: Some(block.id),
                });
            }
        }
    }
    Ok(())
}

fn validate_total_cfg(
    function: &PsiOptimizationFunction,
    blocks: &BTreeMap<BlockId, &omega_optimization_unit::OptimizationBlock>,
    successors: &BTreeMap<BlockId, Vec<BlockId>>,
) -> Result<(), OptimizationUnitValidationError> {
    let mut reachable = BTreeSet::new();
    let mut pending = vec![function.entry];
    while let Some(block) = pending.pop() {
        if reachable.insert(block) {
            pending.extend(successors[&block].iter().copied());
        }
    }
    if reachable.len() != blocks.len() {
        let block = blocks
            .keys()
            .find(|block| !reachable.contains(block))
            .copied()
            .expect("different block counts have an unreachable block");
        return Err(OptimizationUnitValidationError::UnreachableBlock {
            machine: function.machine,
            block,
        });
    }

    let mut indegree = blocks
        .keys()
        .copied()
        .map(|block| (block, 0usize))
        .collect::<BTreeMap<_, _>>();
    for target in successors.values().flatten() {
        *indegree.get_mut(target).expect("successor was validated") += 1;
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(block, count)| (*count == 0).then_some(*block))
        .collect::<BTreeSet<_>>();
    let mut visited = 0usize;
    while let Some(block) = ready.pop_first() {
        visited += 1;
        for target in &successors[&block] {
            let count = indegree.get_mut(target).expect("successor was validated");
            *count -= 1;
            if *count == 0 {
                ready.insert(*target);
            }
        }
    }
    if visited != blocks.len() {
        let block = indegree
            .iter()
            .find_map(|(block, count)| (*count != 0).then_some(*block))
            .expect("a cyclic graph leaves positive indegree");
        return Err(OptimizationUnitValidationError::ControlCycle {
            machine: function.machine,
            block,
        });
    }
    Ok(())
}

fn validate_fact_index(
    function: &PsiOptimizationFunction,
) -> Result<(), OptimizationUnitValidationError> {
    use omega_terminal_abstract_operations::TerminalAbstractOperation as O;

    let mut expected = Vec::new();
    for operation in function
        .blocks
        .iter()
        .flat_map(|block| block.nodes.iter().map(|node| &node.operation))
    {
        match operation {
            O::IntegerExactCast {
                psi_operation,
                obligation,
                ..
            }
            | O::ExactIntegerShiftLeft {
                psi_operation,
                obligation,
                ..
            }
            | O::ExactIntegerShiftRight {
                psi_operation,
                obligation,
                ..
            }
            | O::ExactIntegerAdd {
                psi_operation,
                obligation,
                ..
            }
            | O::ExactIntegerSubtract {
                psi_operation,
                obligation,
                ..
            }
            | O::ExactIntegerMultiply {
                psi_operation,
                obligation,
                ..
            }
            | O::ExactIntegerDivide {
                psi_operation,
                obligation,
                ..
            }
            | O::ExactIntegerRemainder {
                psi_operation,
                obligation,
                ..
            }
            | O::WrappingIntegerDivide {
                psi_operation,
                obligation,
                ..
            }
            | O::WrappingIntegerRemainder {
                psi_operation,
                obligation,
                ..
            }
            | O::SaturatingIntegerDivide {
                psi_operation,
                obligation,
                ..
            }
            | O::SaturatingIntegerRemainder {
                psi_operation,
                obligation,
                ..
            } => expected.push(OptimizationFact::OperationObligationReference {
                obligation: *obligation,
                support: *psi_operation,
            }),
            _ => {}
        }
        match operation {
            O::BooleanConstant {
                psi_operation,
                result,
                value,
            } => expected.push(OptimizationFact::BooleanConstant {
                value: *result,
                constant: *value,
                support: *psi_operation,
            }),
            O::IntegerConstant {
                psi_operation,
                result,
                value,
                ..
            } => expected.push(OptimizationFact::IntegerConstant {
                value: *result,
                constant: *value,
                support: *psi_operation,
            }),
            _ => {}
        }
    }
    if expected != function.facts {
        return Err(OptimizationUnitValidationError::FactIndexMismatch(
            function.machine,
        ));
    }
    Ok(())
}

fn validate_provenance_fuel_effects(
    function: &PsiOptimizationFunction,
) -> Result<(), OptimizationUnitValidationError> {
    let mut provenance = BTreeSet::new();
    let mut fuel = BTreeSet::new();
    let mut expected_effect = 0u64;
    for block in &function.blocks {
        for (index, node) in block.nodes.iter().enumerate() {
            let index = u32::try_from(index).expect("unit node index was built as u32");
            if node.provenance.is_empty() {
                return Err(OptimizationUnitValidationError::IncompleteProvenance {
                    machine: function.machine,
                    block: block.id,
                    node: index,
                });
            }
            for site in &node.provenance {
                if !provenance.insert(*site) {
                    return Err(OptimizationUnitValidationError::DuplicateProvenance(*site));
                }
            }
            let source_sites = node.provenance.iter().copied().collect::<BTreeSet<_>>();
            let settled_sites = node
                .fuel
                .iter()
                .map(|settlement| settlement.site)
                .collect::<BTreeSet<_>>();
            if source_sites != settled_sites
                || node.fuel.iter().any(|settlement| settlement.units != 1)
            {
                return Err(
                    OptimizationUnitValidationError::FuelDoesNotMatchProvenance {
                        machine: function.machine,
                        block: block.id,
                        node: index,
                    },
                );
            }
            for settlement in &node.fuel {
                if !fuel.insert(settlement.site) {
                    return Err(OptimizationUnitValidationError::DuplicateFuelSettlement(
                        settlement.site,
                    ));
                }
            }
            if node.effect.input != expected_effect || node.effect.output != expected_effect + 1 {
                return Err(OptimizationUnitValidationError::BrokenEffectChain {
                    machine: function.machine,
                    expected: expected_effect,
                    actual: node.effect.input,
                });
            }
            expected_effect += 1;
        }
    }
    Ok(())
}

fn validate_values_and_bindings(
    function: &PsiOptimizationFunction,
    blocks: &BTreeMap<BlockId, &omega_optimization_unit::OptimizationBlock>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> Result<(), OptimizationUnitValidationError> {
    let mut definitions = BTreeMap::new();
    for definition in function
        .parameters
        .iter()
        .chain(function.blocks.iter().flat_map(|block| {
            block
                .parameters
                .iter()
                .chain(block.nodes.iter().flat_map(|node| &node.definitions))
        }))
    {
        if definitions.insert(definition.value, *definition).is_some() {
            return Err(OptimizationUnitValidationError::DuplicateValue(
                definition.value,
            ));
        }
    }

    let dominators = dominators(function.entry, blocks.keys().copied(), predecessors);
    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            for use_site in &node.uses {
                let Some(definition) = definitions.get(&use_site.value) else {
                    return Err(OptimizationUnitValidationError::UndefinedValue {
                        machine: function.machine,
                        block: block.id,
                        value: use_site.value,
                    });
                };
                match definition.site {
                    ValueDefinitionSite::FunctionParameter(_) => {}
                    ValueDefinitionSite::BlockParameter {
                        block: defining, ..
                    } => {
                        if !dominators
                            .get(&block.id)
                            .is_some_and(|set| set.contains(&defining))
                        {
                            return Err(OptimizationUnitValidationError::NondominatingValue {
                                machine: function.machine,
                                block: block.id,
                                value: use_site.value,
                            });
                        }
                    }
                    ValueDefinitionSite::Node {
                        block: defining,
                        node,
                    } if defining == block.id => {
                        if usize::try_from(node).expect("u32 fits usize") >= node_index {
                            return Err(OptimizationUnitValidationError::UseBeforeDefinition {
                                machine: function.machine,
                                block: block.id,
                                value: use_site.value,
                            });
                        }
                    }
                    ValueDefinitionSite::Node {
                        block: defining, ..
                    } => {
                        if !dominators
                            .get(&block.id)
                            .is_some_and(|set| set.contains(&defining))
                        {
                            return Err(OptimizationUnitValidationError::NondominatingValue {
                                machine: function.machine,
                                block: block.id,
                                value: use_site.value,
                            });
                        }
                    }
                }
            }
            for edge in &node.successors {
                let target = blocks.get(&edge.target).expect("successor validated");
                if edge.bindings.len() != target.parameters.len() {
                    return Err(OptimizationUnitValidationError::BindingArityMismatch {
                        machine: function.machine,
                        edge: edge.psi_edge,
                    });
                }
                for (binding, parameter) in edge.bindings.iter().zip(&target.parameters) {
                    let source_type = definitions
                        .get(&binding.argument)
                        .map(|row| row.scalar_type);
                    if binding.parameter != parameter.value
                        || binding.scalar_type != parameter.scalar_type
                        || source_type != Some(parameter.scalar_type)
                    {
                        return Err(OptimizationUnitValidationError::BindingTypeMismatch {
                            machine: function.machine,
                            edge: edge.psi_edge,
                            value: binding.argument,
                        });
                    }
                }
            }
        }
    }
    Ok(())
}

fn dominators(
    entry: BlockId,
    block_ids: impl Iterator<Item = BlockId>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> BTreeMap<BlockId, BTreeSet<BlockId>> {
    let all = block_ids.collect::<BTreeSet<_>>();
    let mut result = all
        .iter()
        .copied()
        .map(|block| {
            let initial = if block == entry {
                [entry].into_iter().collect()
            } else {
                all.clone()
            };
            (block, initial)
        })
        .collect::<BTreeMap<_, _>>();
    loop {
        let mut changed = false;
        for block in all.iter().copied().filter(|block| *block != entry) {
            let incoming = predecessors.get(&block).expect("all blocks indexed");
            let mut next = if let Some(first) = incoming.first() {
                result[first].clone()
            } else {
                BTreeSet::new()
            };
            for predecessor in incoming.iter().skip(1) {
                next = next.intersection(&result[predecessor]).copied().collect();
            }
            next.insert(block);
            if result[&block] != next {
                result.insert(block, next);
                changed = true;
            }
        }
        if !changed {
            return result;
        }
    }
}

fn validate_places_and_claims(
    function: &PsiOptimizationFunction,
) -> Result<(), OptimizationUnitValidationError> {
    let mut known_places = function
        .structural_parameters
        .iter()
        .map(|parameter| parameter.place)
        .collect::<BTreeSet<_>>();
    for parameter in &function.structural_parameters {
        if !function.declared_places.contains(&parameter.place) {
            return Err(OptimizationUnitValidationError::UnknownPlace {
                machine: function.machine,
                place: parameter.place,
            });
        }
    }
    for block in &function.blocks {
        for node in &block.nodes {
            validate_operation_places(function.machine, &node.operation, &mut known_places)?;
            for event in &node.ownership {
                let claims: &[ClaimId] = match event {
                    omega_optimization_unit::OwnershipEvent::ClaimTransfer(claims)
                    | omega_optimization_unit::OwnershipEvent::ClaimCompletion(claims)
                    | omega_optimization_unit::OwnershipEvent::StructuralReturn(claims)
                    | omega_optimization_unit::OwnershipEvent::CrashFrontier(claims) => claims,
                    omega_optimization_unit::OwnershipEvent::Cleanup(_) => continue,
                };
                for claim in claims {
                    if !function.entry_claims.contains(claim) {
                        return Err(OptimizationUnitValidationError::UnknownClaim {
                            machine: function.machine,
                            claim: *claim,
                        });
                    }
                }
            }
        }
    }
    if known_places != function.declared_places {
        let place = known_places
            .symmetric_difference(&function.declared_places)
            .next()
            .copied()
            .expect("different place sets have a difference");
        return Err(OptimizationUnitValidationError::UnknownPlace {
            machine: function.machine,
            place,
        });
    }
    Ok(())
}

fn validate_operation_places(
    machine: MachineId,
    operation: &omega_terminal_abstract_operations::TerminalAbstractOperation,
    known: &mut BTreeSet<PlaceId>,
) -> Result<(), OptimizationUnitValidationError> {
    use omega_terminal_abstract_operations::TerminalAbstractOperation as O;
    let require = |place: PlaceId, known: &BTreeSet<PlaceId>| {
        if known.contains(&place) {
            Ok(())
        } else {
            Err(OptimizationUnitValidationError::UnknownPlace { machine, place })
        }
    };
    match operation {
        O::EstablishByteSequenceLiteral { place, .. }
        | O::EstablishTrivialAffineLocal { place, .. } => {
            known.insert(place.id);
        }
        O::CallUnit {
            structural_arguments,
            ..
        }
        | O::CallStructuralScalar {
            structural_arguments,
            ..
        }
        | O::CallStructural {
            structural_arguments,
            ..
        }
        | O::BoundaryCall {
            structural_arguments,
            ..
        } => {
            for argument in structural_arguments {
                require(argument.place, known)?;
            }
            if let O::CallStructural { result, .. } = operation {
                known.insert(result.place);
            }
        }
        O::BooleanStructuralField { source, .. } | O::ReturnStructural { source, .. } => {
            require(*source, known)?;
        }
        O::Return {
            cleanup_actions, ..
        }
        | O::ReturnUnit {
            cleanup_actions, ..
        } => {
            for cleanup in cleanup_actions {
                let place = match cleanup {
                    psi_terminal::TerminalAffineCleanupAction::DiscardRoot(place) => *place,
                    psi_terminal::TerminalAffineCleanupAction::DiscardResidual(discard) => {
                        discard.place
                    }
                    psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                        cleanup.place
                    }
                };
                require(place, known)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn expected_definitions(
    operation: &omega_terminal_abstract_operations::TerminalAbstractOperation,
    block: BlockId,
    node: u32,
) -> Vec<ValueDefinition> {
    use omega_terminal_abstract_operations::TerminalAbstractOperation as O;
    let definition = match operation {
        O::Call {
            result,
            scalar_type,
            ..
        }
        | O::IntegerConstant {
            result,
            scalar_type,
            ..
        } => Some((*result, *scalar_type)),
        O::CallStructuralScalar { result, .. } => Some((result.value, result.scalar_type)),
        O::BoundaryCall {
            result: Some(result),
            ..
        } => Some((result.value, result.scalar_type)),
        O::BooleanConstant { result, .. }
        | O::BooleanStructuralField { result, .. }
        | O::BooleanNot { result, .. }
        | O::BooleanEqual { result, .. }
        | O::IntegerEqual { result, .. }
        | O::IntegerLessThan { result, .. }
        | O::IntegerLessOrEqual { result, .. } => Some((*result, ScalarType::Boolean)),
        O::IntegerBitwiseNot {
            result,
            scalar_type,
            ..
        }
        | O::IntegerBitwiseAnd {
            result,
            scalar_type,
            ..
        }
        | O::IntegerBitwiseOr {
            result,
            scalar_type,
            ..
        }
        | O::IntegerBitwiseXor {
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerAdd {
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerAdd {
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerAdd {
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerSubtract {
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerSubtract {
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerSubtract {
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerMultiply {
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerMultiply {
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerDivide {
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerRemainder {
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerDivide {
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerRemainder {
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerDivide {
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerRemainder {
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerMultiply {
            result,
            scalar_type,
            ..
        } => Some((*result, ScalarType::Integer(*scalar_type))),
        O::IntegerWiden {
            result,
            target_type,
            ..
        }
        | O::IntegerExactCast {
            result,
            target_type,
            ..
        } => Some((*result, ScalarType::Integer(*target_type))),
        O::WrappingIntegerShiftLeft {
            result, value_type, ..
        }
        | O::WrappingIntegerShiftRight {
            result, value_type, ..
        }
        | O::ExactIntegerShiftLeft {
            result, value_type, ..
        }
        | O::ExactIntegerShiftRight {
            result, value_type, ..
        } => Some((*result, ScalarType::Integer(*value_type))),
        _ => None,
    };
    definition
        .into_iter()
        .map(|(value, scalar_type)| ValueDefinition {
            value,
            scalar_type,
            site: ValueDefinitionSite::Node { block, node },
        })
        .collect()
}

fn expected_uses(
    operation: &omega_terminal_abstract_operations::TerminalAbstractOperation,
    block: BlockId,
    node: u32,
) -> Vec<ValueUse> {
    use omega_terminal_abstract_operations::TerminalAbstractOperation as O;
    let values = match operation {
        O::Call { arguments, .. } | O::BoundaryCall { arguments, .. } => arguments.clone(),
        O::BooleanNot { operand, .. }
        | O::IntegerBitwiseNot { operand, .. }
        | O::IntegerWiden { operand, .. }
        | O::IntegerExactCast { operand, .. } => vec![*operand],
        O::BooleanEqual { left, right, .. }
        | O::IntegerEqual { left, right, .. }
        | O::IntegerLessThan { left, right, .. }
        | O::IntegerLessOrEqual { left, right, .. }
        | O::IntegerBitwiseAnd { left, right, .. }
        | O::IntegerBitwiseOr { left, right, .. }
        | O::IntegerBitwiseXor { left, right, .. }
        | O::WrappingIntegerAdd { left, right, .. }
        | O::ExactIntegerAdd { left, right, .. }
        | O::SaturatingIntegerAdd { left, right, .. }
        | O::WrappingIntegerSubtract { left, right, .. }
        | O::ExactIntegerSubtract { left, right, .. }
        | O::SaturatingIntegerSubtract { left, right, .. }
        | O::WrappingIntegerMultiply { left, right, .. }
        | O::ExactIntegerMultiply { left, right, .. }
        | O::ExactIntegerDivide { left, right, .. }
        | O::ExactIntegerRemainder { left, right, .. }
        | O::WrappingIntegerDivide { left, right, .. }
        | O::WrappingIntegerRemainder { left, right, .. }
        | O::SaturatingIntegerDivide { left, right, .. }
        | O::SaturatingIntegerRemainder { left, right, .. }
        | O::SaturatingIntegerMultiply { left, right, .. } => vec![*left, *right],
        O::WrappingIntegerShiftLeft { value, count, .. }
        | O::WrappingIntegerShiftRight { value, count, .. }
        | O::ExactIntegerShiftLeft { value, count, .. }
        | O::ExactIntegerShiftRight { value, count, .. } => vec![*value, *count],
        O::Jump { bindings, .. } => bindings.iter().map(|binding| binding.argument).collect(),
        O::Conditional {
            condition,
            when_true,
            when_false,
        } => std::iter::once(*condition)
            .chain(when_true.bindings.iter().map(|binding| binding.argument))
            .chain(when_false.bindings.iter().map(|binding| binding.argument))
            .collect(),
        O::Return { value, .. } => vec![*value],
        _ => Vec::new(),
    };
    values
        .into_iter()
        .map(|value| ValueUse { value, block, node })
        .collect()
}

fn expected_provenance(
    operation: &omega_terminal_abstract_operations::TerminalAbstractOperation,
) -> Vec<PsiProvenance> {
    use omega_terminal_abstract_operations::TerminalAbstractOperation as O;
    match operation {
        O::Jump { psi_edge, .. }
        | O::Return { psi_edge, .. }
        | O::ReturnUnit { psi_edge, .. }
        | O::ReturnStructural { psi_edge, .. }
        | O::Crash { psi_edge, .. } => vec![PsiProvenance::Edge(*psi_edge)],
        O::Conditional {
            when_true,
            when_false,
            ..
        } => vec![
            PsiProvenance::Edge(when_true.psi_edge),
            PsiProvenance::Edge(when_false.psi_edge),
        ],
        O::EstablishByteSequenceLiteral { psi_operation, .. }
        | O::EstablishTrivialAffineLocal { psi_operation, .. }
        | O::CallUnit { psi_operation, .. }
        | O::CallStructuralScalar { psi_operation, .. }
        | O::CallStructural { psi_operation, .. }
        | O::BoundaryCall { psi_operation, .. }
        | O::PortWrite { psi_operation, .. }
        | O::Call { psi_operation, .. }
        | O::IntegerConstant { psi_operation, .. }
        | O::BooleanConstant { psi_operation, .. }
        | O::BooleanStructuralField { psi_operation, .. }
        | O::BooleanNot { psi_operation, .. }
        | O::BooleanEqual { psi_operation, .. }
        | O::IntegerEqual { psi_operation, .. }
        | O::IntegerLessThan { psi_operation, .. }
        | O::IntegerLessOrEqual { psi_operation, .. }
        | O::IntegerBitwiseNot { psi_operation, .. }
        | O::IntegerWiden { psi_operation, .. }
        | O::IntegerExactCast { psi_operation, .. }
        | O::IntegerBitwiseAnd { psi_operation, .. }
        | O::IntegerBitwiseOr { psi_operation, .. }
        | O::IntegerBitwiseXor { psi_operation, .. }
        | O::WrappingIntegerShiftLeft { psi_operation, .. }
        | O::WrappingIntegerShiftRight { psi_operation, .. }
        | O::ExactIntegerShiftLeft { psi_operation, .. }
        | O::ExactIntegerShiftRight { psi_operation, .. }
        | O::WrappingIntegerAdd { psi_operation, .. }
        | O::ExactIntegerAdd { psi_operation, .. }
        | O::SaturatingIntegerAdd { psi_operation, .. }
        | O::WrappingIntegerSubtract { psi_operation, .. }
        | O::ExactIntegerSubtract { psi_operation, .. }
        | O::SaturatingIntegerSubtract { psi_operation, .. }
        | O::WrappingIntegerMultiply { psi_operation, .. }
        | O::ExactIntegerMultiply { psi_operation, .. }
        | O::ExactIntegerDivide { psi_operation, .. }
        | O::ExactIntegerRemainder { psi_operation, .. }
        | O::WrappingIntegerDivide { psi_operation, .. }
        | O::WrappingIntegerRemainder { psi_operation, .. }
        | O::SaturatingIntegerDivide { psi_operation, .. }
        | O::SaturatingIntegerRemainder { psi_operation, .. }
        | O::SaturatingIntegerMultiply { psi_operation, .. } => {
            vec![PsiProvenance::Operation(*psi_operation)]
        }
    }
}

fn expected_edges(
    operation: &omega_terminal_abstract_operations::TerminalAbstractOperation,
) -> Vec<OptimizationEdge> {
    use omega_terminal_abstract_operations::TerminalAbstractOperation as O;
    match operation {
        O::Jump {
            psi_edge,
            target,
            bindings,
        } => vec![OptimizationEdge {
            psi_edge: *psi_edge,
            target: *target,
            bindings: bindings.clone(),
        }],
        O::Conditional {
            when_true,
            when_false,
            ..
        } => [when_true, when_false]
            .into_iter()
            .map(|edge| OptimizationEdge {
                psi_edge: edge.psi_edge,
                target: edge.target,
                bindings: edge.bindings.clone(),
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn expected_ownership(
    operation: &omega_terminal_abstract_operations::TerminalAbstractOperation,
) -> Vec<OwnershipEvent> {
    use omega_terminal_abstract_operations::TerminalAbstractOperation as O;
    match operation {
        O::CallUnit {
            claim_transfers, ..
        }
        | O::CallStructuralScalar {
            claim_transfers, ..
        }
        | O::CallStructural {
            claim_transfers, ..
        } => vec![OwnershipEvent::ClaimTransfer(
            claim_transfers
                .iter()
                .map(|transfer| transfer.claim)
                .collect(),
        )],
        O::BoundaryCall {
            completion_receipts,
            ..
        } => vec![OwnershipEvent::ClaimCompletion(
            completion_receipts
                .iter()
                .map(|receipt| receipt.claim)
                .collect(),
        )],
        O::Return {
            cleanup_actions, ..
        }
        | O::ReturnUnit {
            cleanup_actions, ..
        } => vec![OwnershipEvent::Cleanup(cleanup_actions.clone())],
        O::ReturnStructural {
            returned_claims, ..
        } => vec![OwnershipEvent::StructuralReturn(returned_claims.clone())],
        O::Crash {
            frontier_lower_bound,
            ..
        } => vec![OwnershipEvent::CrashFrontier(frontier_lower_bound.clone())],
        _ => Vec::new(),
    }
}

fn is_terminator(
    operation: &omega_terminal_abstract_operations::TerminalAbstractOperation,
) -> bool {
    use omega_terminal_abstract_operations::TerminalAbstractOperation as O;
    matches!(
        operation,
        O::Jump { .. }
            | O::Conditional { .. }
            | O::Return { .. }
            | O::ReturnUnit { .. }
            | O::ReturnStructural { .. }
            | O::Crash { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_optimization_core::{
        AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
        OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
    };
    use omega_optimization_unit::{
        IntegerConstantRewrite, IntegerEvaluationWitness, NodeLocation, ProvenanceRewrite,
        PsiRewriteCandidate, ValueUse, reconstruct_psi_optimization_unit_seed,
    };
    use omega_terminal_abstract_operations::{
        TerminalAbstractBlockEntry, TerminalAbstractFunction, TerminalAbstractFunctionResult,
        TerminalAbstractOperation, TerminalAbstractOperationPlan, TerminalAbstractResult,
    };
    use psi_core::{
        FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, OperationId, ScalarType,
        ValueId,
    };
    use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    fn verified_unit() -> omega_terminal_psi_to_abstract_operations::VerifiedPsiOptimizationUnit {
        use psi_terminal::{
            Block, MachineContract, TerminalMachine, TerminalMachineResult, TerminalModule,
            Terminator,
        };

        let module = TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: id(101, MachineId::new),
            structural_types: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            proof_output_calls: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            closed_conformance_applications: Vec::new(),
            machines: vec![TerminalMachine {
                id: id(101, MachineId::new),
                attachment: None,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: TerminalMachineResult::Unit,
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: id(102, BlockId::new),
                blocks: vec![Block {
                    id: id(102, BlockId::new),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: id(103, EdgeId::new),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: MachineContract {
                    id: id(104, psi_core::ContractId::new),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                },
            }],
        };
        let proof = psi_terminal_verifier::ProofBundle::default();
        let semantic = psi_terminal_codec::encode_module(&module).expect("encode unit module");
        let proof = psi_terminal_codec::encode_proof_bundle(&proof).expect("encode empty proof");
        let input =
            omega_terminal_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
                &semantic,
                &proof,
                &psi_proof_admission::AdmissionProfile::default(),
            )
            .expect("verified optimizer input");
        omega_terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
            input,
            TerminalFuelSchedule::CURRENT.identity(),
        )
        .expect("verified optimizer unit")
    }

    fn id<T>(raw: u64, constructor: impl FnOnce(u64) -> Option<T>) -> T {
        constructor(raw).expect("nonzero test identity")
    }

    fn unit() -> PsiOptimizationUnit {
        let machine = id(1, MachineId::new);
        let block = id(2, BlockId::new);
        let result = id(3, ValueId::new);
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("valid width");
        let plan = TerminalAbstractOperationPlan {
            terminal_psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([11; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![TerminalAbstractFunction {
                machine,
                attachment: None,
                entry: block,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                    value: result,
                    scalar_type: ScalarType::Integer(integer),
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![TerminalAbstractBlockEntry {
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    TerminalAbstractOperation::IntegerConstant {
                        psi_operation: id(4, OperationId::new),
                        result,
                        scalar_type: ScalarType::Integer(integer),
                        value: IntegerValue::Unsigned(7),
                    },
                    TerminalAbstractOperation::Return {
                        psi_edge: id(5, EdgeId::new),
                        result,
                        value: result,
                        scalar_type: ScalarType::Integer(integer),
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        };
        reconstruct_psi_optimization_unit_seed(
            &plan,
            FuelScheduleIdentity::new(1).expect("nonzero schedule"),
        )
        .expect("valid unit")
    }

    fn exact_add_unit() -> PsiOptimizationUnit {
        let machine = id(201, MachineId::new);
        let block = id(202, BlockId::new);
        let left = id(203, ValueId::new);
        let right = id(204, ValueId::new);
        let result = id(205, ValueId::new);
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let plan = TerminalAbstractOperationPlan {
            terminal_psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([12; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![TerminalAbstractFunction {
                machine,
                attachment: None,
                entry: block,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                    value: result,
                    scalar_type: ScalarType::Integer(integer),
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![TerminalAbstractBlockEntry {
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    TerminalAbstractOperation::IntegerConstant {
                        psi_operation: id(206, OperationId::new),
                        result: left,
                        scalar_type: ScalarType::Integer(integer),
                        value: IntegerValue::Unsigned(7),
                    },
                    TerminalAbstractOperation::IntegerConstant {
                        psi_operation: id(207, OperationId::new),
                        result: right,
                        scalar_type: ScalarType::Integer(integer),
                        value: IntegerValue::Unsigned(8),
                    },
                    TerminalAbstractOperation::ExactIntegerAdd {
                        psi_operation: id(208, OperationId::new),
                        obligation: id(209, psi_core::ObligationId::new),
                        result,
                        scalar_type: integer,
                        left,
                        right,
                    },
                    TerminalAbstractOperation::Return {
                        psi_edge: id(210, EdgeId::new),
                        result,
                        value: result,
                        scalar_type: ScalarType::Integer(integer),
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        };
        reconstruct_psi_optimization_unit_seed(&plan, FuelScheduleIdentity::new(1).unwrap())
            .unwrap()
    }

    fn integer_candidate(
        unit: &PsiOptimizationUnit,
        constant: IntegerValue,
    ) -> PsiRewriteCandidate {
        let function = &unit.functions[0];
        let block = &function.blocks[0];
        let node = &block.nodes[2];
        let TerminalAbstractOperation::ExactIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            ..
        } = node.operation
        else {
            panic!("fixture contains exact add")
        };
        let location = NodeLocation {
            machine: function.machine,
            block: block.id,
            node: 2,
        };
        let contract = OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(b"fold-exact-add"),
            OptimizationPassIdentity::from_canonical_bytes(b"constant-evaluation"),
            1,
            AnalysisSet::new([AnalysisKind::ScalarConstants]),
            AnalysisInvalidationSet::new([AnalysisKind::UseDefinition]),
            OptimizationSafetyClass::ProofCertified,
        )
        .unwrap();
        PsiRewriteCandidate::new_integer_evaluation(
            unit.identity,
            contract,
            vec![block.id],
            Vec::new(),
            vec![ProvenanceRewrite {
                output: location,
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            }],
            IntegerEvaluationWitness {
                left_support: id(206, OperationId::new),
                right_support: id(207, OperationId::new),
            },
            -1,
            IntegerConstantRewrite {
                location,
                source_operation: psi_operation,
                result,
                scalar_type,
                constant,
            },
        )
        .unwrap()
    }

    #[test]
    fn independently_accepts_builder_output() {
        validate_psi_optimization_unit(&unit()).unwrap();
    }

    #[test]
    fn independently_accepts_verified_context_and_frontier_coverage() {
        validate_verified_psi_optimization_unit(&verified_unit()).unwrap();
    }

    #[test]
    fn independent_integer_rewrite_constructor_accepts_only_exact_evaluation() {
        let input = exact_add_unit();
        let candidate = integer_candidate(&input, IntegerValue::Unsigned(15));
        let replay = integer_candidate(&input, IntegerValue::Unsigned(15));
        assert_eq!(candidate.identity(), replay.identity());
        let accepted = validate_integer_evaluation_candidate(&input, &candidate).unwrap();
        assert_eq!(accepted.candidate(), candidate.identity());
        assert_ne!(accepted.unit().identity, input.identity);
        assert_eq!(accepted.unit().identity, candidate.output());
        assert_eq!(
            accepted.unit().functions[0].blocks[0].nodes[2].provenance,
            input.functions[0].blocks[0].nodes[2].provenance
        );
        assert_eq!(
            accepted.unit().functions[0].blocks[0].nodes[2].fuel,
            input.functions[0].blocks[0].nodes[2].fuel
        );
        assert!(matches!(
            accepted.unit().functions[0].blocks[0].nodes[2].operation,
            TerminalAbstractOperation::IntegerConstant {
                value: IntegerValue::Unsigned(15),
                ..
            }
        ));
        assert!(matches!(
            accepted.unit().functions[0].facts[2],
            OptimizationFact::IntegerConstant {
                constant: IntegerValue::Unsigned(15),
                ..
            }
        ));
        assert!(matches!(
            input.functions[0].blocks[0].nodes[2].operation,
            TerminalAbstractOperation::ExactIntegerAdd { .. }
        ));

        let wrong = integer_candidate(&input, IntegerValue::Unsigned(14));
        assert!(matches!(
            validate_integer_evaluation_candidate(&input, &wrong),
            Err(OptimizationUnitValidationError::CandidateEvaluationMismatch)
        ));
    }

    #[test]
    fn corruption_classes_fail_independently() {
        let mut provenance = unit();
        provenance.functions[0].blocks[0].nodes[0]
            .provenance
            .clear();
        assert!(matches!(
            validate_psi_optimization_unit(&provenance),
            Err(OptimizationUnitValidationError::OperationMetadataMismatch { .. })
        ));

        let mut fuel = unit();
        fuel.functions[0].blocks[0].nodes[0].fuel.clear();
        assert!(matches!(
            validate_psi_optimization_unit(&fuel),
            Err(OptimizationUnitValidationError::FuelDoesNotMatchProvenance { .. })
        ));

        let mut effects = unit();
        effects.functions[0].blocks[0].nodes[1].effect.input = 99;
        assert!(matches!(
            validate_psi_optimization_unit(&effects),
            Err(OptimizationUnitValidationError::BrokenEffectChain { .. })
        ));

        let mut facts = unit();
        facts.functions[0].facts.clear();
        assert!(matches!(
            validate_psi_optimization_unit(&facts),
            Err(OptimizationUnitValidationError::FactIndexMismatch(_))
        ));

        let mut forged_uses = unit();
        let block = forged_uses.functions[0].blocks[0].id;
        forged_uses.functions[0].blocks[0].nodes[1]
            .uses
            .push(ValueUse {
                value: id(99, ValueId::new),
                block,
                node: 1,
            });
        assert!(matches!(
            validate_psi_optimization_unit(&forged_uses),
            Err(OptimizationUnitValidationError::OperationMetadataMismatch { .. })
        ));

        let mut forged_definitions = unit();
        forged_definitions.functions[0].blocks[0].nodes[0]
            .definitions
            .clear();
        assert!(matches!(
            validate_psi_optimization_unit(&forged_definitions),
            Err(OptimizationUnitValidationError::OperationMetadataMismatch { .. })
        ));

        let mut undefined = unit();
        let unknown = id(99, ValueId::new);
        let TerminalAbstractOperation::Return { value, .. } =
            &mut undefined.functions[0].blocks[0].nodes[1].operation
        else {
            panic!("unit ends in return")
        };
        *value = unknown;
        undefined.functions[0].blocks[0].nodes[1].uses = vec![ValueUse {
            value: unknown,
            block,
            node: 1,
        }];
        assert!(matches!(
            validate_psi_optimization_unit(&undefined),
            Err(OptimizationUnitValidationError::UndefinedValue { .. })
        ));

        let mut place = unit();
        place.functions[0]
            .declared_places
            .insert(id(88, PlaceId::new));
        assert!(matches!(
            validate_psi_optimization_unit(&place),
            Err(OptimizationUnitValidationError::UnknownPlace { .. })
        ));

        let mut cleanup = unit();
        cleanup.functions[0].blocks[0].nodes[1].ownership.clear();
        assert!(matches!(
            validate_psi_optimization_unit(&cleanup),
            Err(OptimizationUnitValidationError::OperationMetadataMismatch { .. })
        ));

        let mut cfg = unit();
        cfg.functions[0].blocks[0].nodes[1].operation = TerminalAbstractOperation::Jump {
            psi_edge: id(5, EdgeId::new),
            target: id(77, BlockId::new),
            bindings: Vec::new(),
        };
        cfg.functions[0].blocks[0].nodes[1].successors =
            expected_edges(&cfg.functions[0].blocks[0].nodes[1].operation);
        cfg.functions[0].blocks[0].nodes[1].uses.clear();
        cfg.functions[0].blocks[0].nodes[1].ownership.clear();
        assert!(matches!(
            validate_psi_optimization_unit(&cfg),
            Err(OptimizationUnitValidationError::UnknownSuccessor { .. })
        ));

        let mut entry_parameters = unit();
        let block = entry_parameters.functions[0].entry;
        entry_parameters.functions[0].blocks[0]
            .parameters
            .push(ValueDefinition {
                value: id(76, ValueId::new),
                scalar_type: ScalarType::Boolean,
                site: ValueDefinitionSite::BlockParameter { block, position: 0 },
            });
        assert!(matches!(
            validate_psi_optimization_unit(&entry_parameters),
            Err(OptimizationUnitValidationError::EntryBlockHasParameters { .. })
        ));

        let mut unreachable = unit();
        let block = id(75, BlockId::new);
        let mut detached = unreachable.functions[0].blocks[0].clone();
        detached.id = block;
        for (node_index, node) in detached.nodes.iter_mut().enumerate() {
            let node_index = u32::try_from(node_index).unwrap();
            node.definitions = expected_definitions(&node.operation, block, node_index);
            node.uses = expected_uses(&node.operation, block, node_index);
        }
        unreachable.functions[0].blocks.push(detached);
        assert!(matches!(
            validate_psi_optimization_unit(&unreachable),
            Err(OptimizationUnitValidationError::UnreachableBlock { .. })
        ));

        let mut cycle = unit();
        let block = cycle.functions[0].entry;
        let operation = TerminalAbstractOperation::Jump {
            psi_edge: id(5, EdgeId::new),
            target: block,
            bindings: Vec::new(),
        };
        let node = &mut cycle.functions[0].blocks[0].nodes[1];
        node.operation = operation;
        node.provenance = expected_provenance(&node.operation);
        node.uses = expected_uses(&node.operation, block, 1);
        node.successors = expected_edges(&node.operation);
        node.ownership = expected_ownership(&node.operation);
        assert!(matches!(
            validate_psi_optimization_unit(&cycle),
            Err(OptimizationUnitValidationError::ControlCycle { .. })
        ));
    }

    #[test]
    fn unknown_claim_frontier_is_rejected() {
        let mut unit = unit();
        let claim = id(71, ClaimId::new);
        let edge = id(5, EdgeId::new);
        let operation = TerminalAbstractOperation::Crash {
            psi_edge: edge,
            cause: psi_terminal::CrashCause::Trap,
            site_guard: Vec::new(),
            frontier_lower_bound: vec![claim],
        };
        let node = &mut unit.functions[0].blocks[0].nodes[1];
        node.operation = operation;
        node.provenance = expected_provenance(&node.operation);
        node.fuel[0].site = PsiProvenance::Edge(edge);
        node.uses.clear();
        node.successors.clear();
        node.ownership = expected_ownership(&node.operation);
        assert!(matches!(
            validate_psi_optimization_unit(&unit),
            Err(OptimizationUnitValidationError::UnknownClaim { .. })
        ));
    }
}
