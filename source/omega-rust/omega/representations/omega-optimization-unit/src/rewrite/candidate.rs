//! Candidate construction, canonical accessors, and validation.

use super::codec::encode_candidate;
use super::model::*;
use super::*;

impl PsiRewriteCandidate {
    #[allow(clippy::too_many_arguments)]
    pub fn new_integer_evaluation(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        substitutions: Vec<ScalarSubstitution>,
        provenance: Vec<ProvenanceRewrite>,
        witness: ScalarEvaluationWitness,
        predicted_cost_delta: i64,
        patch: IntegerConstantRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            substitutions,
            provenance,
            PsiRewriteWitness::ScalarEvaluation(witness),
            predicted_cost_delta,
            PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch),
        )
    }

    /// Replace one proof-certified integer operation with an independently
    /// equivalent typed constant while preserving its result and source
    /// occurrence in place. Unlike scalar constant evaluation, the witness is
    /// the exact accepted obligation alone; the rule-specific validator must
    /// reconstruct the symbolic law that determines `patch.constant`.
    #[allow(clippy::too_many_arguments)]
    pub fn new_proof_certified_integer_constant_replacement(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        provenance: Vec<ProvenanceRewrite>,
        obligation_fact: AcceptedObligationFactIdentity,
        predicted_cost_delta: i64,
        patch: IntegerConstantRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            Vec::new(),
            provenance,
            PsiRewriteWitness::AcceptedObligation(obligation_fact),
            predicted_cost_delta,
            PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch),
        )
    }

    /// Replace one proof-certified integer operation with an independently
    /// equivalent typed constant when the symbolic law also depends on one
    /// direct scalar-literal fact. The operation stays at its authored site,
    /// so no scalar substitution is introduced.
    #[allow(clippy::too_many_arguments)]
    pub fn new_literal_proof_certified_integer_constant_replacement(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        provenance: Vec<ProvenanceRewrite>,
        constant_fact: ScalarConstantFactIdentity,
        obligation_fact: AcceptedObligationFactIdentity,
        predicted_cost_delta: i64,
        patch: IntegerConstantRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            Vec::new(),
            provenance,
            PsiRewriteWitness::ProofCertifiedScalarIdentity {
                constant_fact,
                obligation_fact,
            },
            predicted_cost_delta,
            PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_boolean_evaluation(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        substitutions: Vec<ScalarSubstitution>,
        provenance: Vec<ProvenanceRewrite>,
        witness: ScalarEvaluationWitness,
        predicted_cost_delta: i64,
        patch: BooleanConstantRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            substitutions,
            provenance,
            PsiRewriteWitness::ScalarEvaluation(witness),
            predicted_cost_delta,
            PsiRewritePatch::ReplaceBooleanOperationWithConstant(patch),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_redundant_block_parameter(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        provenance: Vec<ProvenanceRewrite>,
        witness: RedundantBlockParameterWitness,
        predicted_cost_delta: i64,
        patch: RedundantBlockParameterRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        if witness.incoming.is_empty() {
            return Err(PsiRewriteCandidateError::EmptyIncomingBindings);
        }
        if witness
            .incoming
            .windows(2)
            .any(|pair| (pair[0].edge, pair[0].source) >= (pair[1].edge, pair[1].source))
        {
            return Err(PsiRewriteCandidateError::NonCanonicalIncomingBindings);
        }
        let substitutions = vec![ScalarSubstitution {
            from: patch.parameter,
            to: patch.replacement,
            scalar_type: patch.scalar_type,
        }];
        Self::new(
            input,
            contract,
            affected_blocks,
            substitutions,
            provenance,
            PsiRewriteWitness::RedundantBlockParameter(witness),
            predicted_cost_delta,
            PsiRewritePatch::RemoveRedundantBlockParameter(patch),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_constant_conditional(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        provenance: Vec<ProvenanceRewrite>,
        condition_fact: ScalarConstantFactIdentity,
        predicted_cost_delta: i64,
        patch: ConstantConditionalRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            Vec::new(),
            provenance,
            PsiRewriteWitness::ScalarEvaluation(ScalarEvaluationWitness::Unary {
                operand_fact: condition_fact,
            }),
            predicted_cost_delta,
            PsiRewritePatch::FoldConstantConditional(patch),
        )
    }

    pub fn new_linear_empty_block(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        provenance: Vec<ProvenanceRewrite>,
        predicted_cost_delta: i64,
        patch: LinearEmptyBlockRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            Vec::new(),
            provenance,
            PsiRewriteWitness::StructuralIdentity,
            predicted_cost_delta,
            PsiRewritePatch::ThreadLinearEmptyBlock(patch),
        )
    }

    pub fn new_path_qualified_empty_block(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        provenance: Vec<ProvenanceRewrite>,
        predicted_cost_delta: i64,
        patch: PathQualifiedEmptyBlockRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            Vec::new(),
            provenance,
            PsiRewriteWitness::StructuralIdentity,
            predicted_cost_delta,
            PsiRewritePatch::ThreadPathQualifiedEmptyBlock(patch),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_adjacent_block_merge(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        substitutions: Vec<ScalarSubstitution>,
        provenance: Vec<ProvenanceRewrite>,
        ownership_witness: OwnershipFrontierWitness,
        predicted_cost_delta: i64,
        patch: AdjacentBlockMergeRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        if ownership_witness
            .rows
            .windows(2)
            .any(|pair| pair[0].site >= pair[1].site)
        {
            return Err(PsiRewriteCandidateError::NonCanonicalOwnershipFrontierWitness);
        }
        Self::new(
            input,
            contract,
            affected_blocks,
            substitutions,
            provenance,
            PsiRewriteWitness::OwnershipFrontiers(ownership_witness),
            predicted_cost_delta,
            PsiRewritePatch::MergeAdjacentBlock(patch),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_non_adjacent_block_merge(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        substitutions: Vec<ScalarSubstitution>,
        provenance: Vec<ProvenanceRewrite>,
        predicted_cost_delta: i64,
        patch: NonAdjacentBlockMergeRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            substitutions,
            provenance,
            PsiRewriteWitness::StructuralIdentity,
            predicted_cost_delta,
            PsiRewritePatch::MergeNonAdjacentBlock(patch),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_shared_jump_fusion(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        substitutions: Vec<ScalarSubstitution>,
        provenance: Vec<ProvenanceRewrite>,
        predicted_cost_delta: i64,
        patch: SharedJumpFusionRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            substitutions,
            provenance,
            PsiRewriteWitness::StructuralIdentity,
            predicted_cost_delta,
            PsiRewritePatch::FuseSharedTerminalJump(patch),
        )
    }

    pub fn new_dead_scalar_node(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        provenance: Vec<ProvenanceRewrite>,
        predicted_cost_delta: i64,
        patch: DeadScalarNodeRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            Vec::new(),
            provenance,
            PsiRewriteWitness::StructuralIdentity,
            predicted_cost_delta,
            PsiRewritePatch::RemoveDeadScalarNode(patch),
        )
    }

    pub fn new_proof_certified_dead_scalar_node(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        provenance: Vec<ProvenanceRewrite>,
        obligation_fact: AcceptedObligationFactIdentity,
        predicted_cost_delta: i64,
        patch: DeadScalarNodeRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            Vec::new(),
            provenance,
            PsiRewriteWitness::AcceptedObligation(obligation_fact),
            predicted_cost_delta,
            PsiRewritePatch::RemoveDeadScalarNode(patch),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_proof_certified_scalar_identity(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        provenance: Vec<ProvenanceRewrite>,
        constant_fact: ScalarConstantFactIdentity,
        obligation_fact: AcceptedObligationFactIdentity,
        predicted_cost_delta: i64,
        patch: ProofCertifiedScalarIdentityRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            vec![ScalarSubstitution {
                from: patch.result,
                to: patch.replacement,
                scalar_type: ScalarType::Integer(patch.scalar_type),
            }],
            provenance,
            PsiRewriteWitness::ProofCertifiedScalarIdentity {
                constant_fact,
                obligation_fact,
            },
            predicted_cost_delta,
            PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch),
        )
    }

    pub fn new_local_scalar_common_subexpression(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        provenance: Vec<ProvenanceRewrite>,
        predicted_cost_delta: i64,
        patch: LocalScalarCommonSubexpressionRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            vec![ScalarSubstitution {
                from: patch.redundant_result,
                to: patch.leader_result,
                scalar_type: patch.scalar_type,
            }],
            provenance,
            PsiRewriteWitness::StructuralIdentity,
            predicted_cost_delta,
            PsiRewritePatch::EliminateLocalScalarCommonSubexpression(patch),
        )
    }

    pub fn new_proof_certified_local_scalar_common_subexpression(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        provenance: Vec<ProvenanceRewrite>,
        obligation_fact: AcceptedObligationFactIdentity,
        predicted_cost_delta: i64,
        patch: LocalScalarCommonSubexpressionRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            vec![ScalarSubstitution {
                from: patch.redundant_result,
                to: patch.leader_result,
                scalar_type: patch.scalar_type,
            }],
            provenance,
            PsiRewriteWitness::AcceptedObligation(obligation_fact),
            predicted_cost_delta,
            PsiRewritePatch::EliminateLocalScalarCommonSubexpression(patch),
        )
    }

    pub fn new_dominating_scalar_common_subexpression(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        provenance: Vec<ProvenanceRewrite>,
        predicted_cost_delta: i64,
        patch: DominatingScalarCommonSubexpressionRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            vec![ScalarSubstitution {
                from: patch.redundant_result,
                to: patch.leader_result,
                scalar_type: patch.scalar_type,
            }],
            provenance,
            PsiRewriteWitness::StructuralIdentity,
            predicted_cost_delta,
            PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(patch),
        )
    }

    pub fn new_proof_certified_dominating_scalar_common_subexpression(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        provenance: Vec<ProvenanceRewrite>,
        obligation_fact: AcceptedObligationFactIdentity,
        predicted_cost_delta: i64,
        patch: DominatingScalarCommonSubexpressionRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            vec![ScalarSubstitution {
                from: patch.redundant_result,
                to: patch.leader_result,
                scalar_type: patch.scalar_type,
            }],
            provenance,
            PsiRewriteWitness::AcceptedObligation(obligation_fact),
            predicted_cost_delta,
            PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(patch),
        )
    }

    pub fn new_phi_translated_scalar_common_subexpression(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        provenance: Vec<ProvenanceRewrite>,
        predicted_cost_delta: i64,
        patch: PhiTranslatedScalarGvnRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            Vec::new(),
            provenance,
            PsiRewriteWitness::StructuralIdentity,
            predicted_cost_delta,
            PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(patch),
        )
    }

    pub fn new_proof_certified_phi_translated_scalar_common_subexpression(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        provenance: Vec<ProvenanceRewrite>,
        obligation_fact: AcceptedObligationFactIdentity,
        predicted_cost_delta: i64,
        patch: PhiTranslatedScalarGvnRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            affected_blocks,
            Vec::new(),
            provenance,
            PsiRewriteWitness::AcceptedObligation(obligation_fact),
            predicted_cost_delta,
            PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(patch),
        )
    }

    pub fn new_unreachable_private_machines(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        provenance: Vec<ProvenanceRewrite>,
        predicted_cost_delta: i64,
        patch: UnreachablePrivateMachinesRewrite,
    ) -> Result<Self, PsiRewriteCandidateError> {
        Self::new(
            input,
            contract,
            Vec::new(),
            Vec::new(),
            provenance,
            PsiRewriteWitness::StructuralIdentity,
            predicted_cost_delta,
            PsiRewritePatch::PruneUnreachablePrivateMachines(patch),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new(
        input: OptimizationUnitIdentity,
        contract: OptimizationRuleContract,
        affected_blocks: Vec<BlockId>,
        substitutions: Vec<ScalarSubstitution>,
        provenance: Vec<ProvenanceRewrite>,
        witness: PsiRewriteWitness,
        predicted_cost_delta: i64,
        patch: PsiRewritePatch,
    ) -> Result<Self, PsiRewriteCandidateError> {
        let decision_point = match &patch {
            PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) => {
                PsiRewriteDecisionPoint::Node(patch.location)
            }
            PsiRewritePatch::ReplaceBooleanOperationWithConstant(patch) => {
                PsiRewriteDecisionPoint::Node(patch.location)
            }
            PsiRewritePatch::RemoveRedundantBlockParameter(patch) => {
                PsiRewriteDecisionPoint::Node(NodeLocation {
                    machine: patch.machine,
                    block: patch.block,
                    node: 0,
                })
            }
            PsiRewritePatch::FoldConstantConditional(patch) => {
                PsiRewriteDecisionPoint::Node(patch.location)
            }
            PsiRewritePatch::ThreadLinearEmptyBlock(patch) => {
                PsiRewriteDecisionPoint::Node(patch.predecessor)
            }
            PsiRewritePatch::ThreadPathQualifiedEmptyBlock(patch) => {
                PsiRewriteDecisionPoint::Node(patch.empty)
            }
            PsiRewritePatch::MergeAdjacentBlock(patch) => {
                PsiRewriteDecisionPoint::Node(patch.predecessor)
            }
            PsiRewritePatch::MergeNonAdjacentBlock(patch) => {
                PsiRewriteDecisionPoint::Node(patch.predecessor)
            }
            PsiRewritePatch::FuseSharedTerminalJump(patch) => {
                PsiRewriteDecisionPoint::Node(patch.predecessor)
            }
            PsiRewritePatch::RemoveDeadScalarNode(patch) => {
                PsiRewriteDecisionPoint::Node(patch.location)
            }
            PsiRewritePatch::EliminateLocalScalarCommonSubexpression(patch) => {
                PsiRewriteDecisionPoint::Node(patch.redundant)
            }
            PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(patch) => {
                PsiRewriteDecisionPoint::Node(patch.redundant)
            }
            PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(patch) => {
                PsiRewriteDecisionPoint::Node(patch.redundant)
            }
            PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) => {
                PsiRewriteDecisionPoint::Node(patch.location)
            }
            PsiRewritePatch::PruneUnreachablePrivateMachines(patch) => {
                if patch.machines.is_empty()
                    || patch.machines.windows(2).any(|pair| pair[0] >= pair[1])
                {
                    return Err(PsiRewriteCandidateError::NonCanonicalAffectedRegion);
                }
                let machines = patch
                    .machines
                    .iter()
                    .map(|row| row.machine)
                    .collect::<Vec<_>>();
                if machines.windows(2).any(|pair| pair[0] >= pair[1]) {
                    return Err(PsiRewriteCandidateError::NonCanonicalAffectedRegion);
                }
                PsiRewriteDecisionPoint::MachineSet(machines)
            }
        };
        let location = match &decision_point {
            PsiRewriteDecisionPoint::Node(location) => Some(*location),
            PsiRewriteDecisionPoint::MachineSet(_) => None,
        };
        if affected_blocks.is_empty() && location.is_some() {
            return Err(PsiRewriteCandidateError::EmptyAffectedRegion);
        }
        if affected_blocks.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(PsiRewriteCandidateError::NonCanonicalAffectedRegion);
        }
        if location.is_some_and(|location| !affected_blocks.contains(&location.block)) {
            return Err(PsiRewriteCandidateError::DecisionPointOutsideRegion);
        }
        if substitutions.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(PsiRewriteCandidateError::NonCanonicalSubstitutions);
        }
        if provenance.is_empty() || provenance.iter().any(|row| row.sources.is_empty()) {
            return Err(PsiRewriteCandidateError::EmptyProvenanceSource);
        }
        if provenance.windows(2).any(|pair| {
            let left = (
                pair[0].input,
                pair[0].disposition.canonical_tag(),
                pair[0].disposition.site(),
            );
            let right = (
                pair[1].input,
                pair[1].disposition.canonical_tag(),
                pair[1].disposition.site(),
            );
            left >= right
        }) || provenance.iter().any(|row| {
            row.sources.iter().copied().collect::<BTreeSet<_>>().len() != row.sources.len()
        }) {
            return Err(PsiRewriteCandidateError::NonCanonicalProvenance);
        }
        for group in provenance.chunk_by(|left, right| left.input == right.input) {
            if group.len() > 1
                && (group.iter().any(|row| !row.disposition.is_realized())
                    || group
                        .iter()
                        .skip(1)
                        .any(|row| row.sources != group[0].sources || row.fuel != group[0].fuel))
            {
                return Err(PsiRewriteCandidateError::NonCanonicalProvenance);
            }
        }
        for row in &provenance {
            let sources = row.sources.iter().copied().collect::<BTreeSet<_>>();
            if row.input.machine() != row.disposition.site().machine() {
                return Err(PsiRewriteCandidateError::NonCanonicalProvenance);
            }
            let fuel = row
                .fuel
                .iter()
                .map(|settlement| settlement.site)
                .collect::<BTreeSet<_>>();
            if fuel.len() != row.fuel.len()
                || fuel != sources
                || row.fuel.iter().any(|settlement| settlement.units == 0)
            {
                return Err(PsiRewriteCandidateError::FuelProvenanceMismatch);
            }
        }
        if matches!(
            contract.safety_class(),
            OptimizationSafetyClass::ProofCertified
        ) != matches!(
            witness,
            PsiRewriteWitness::ScalarEvaluation(
                ScalarEvaluationWitness::ProofCertifiedUnary { .. }
                    | ScalarEvaluationWitness::ProofCertifiedBinary { .. }
                    | ScalarEvaluationWitness::RangeAgainstConstant { .. }
                    | ScalarEvaluationWitness::RangeAgainstRange { .. }
            ) | PsiRewriteWitness::AcceptedObligation(_)
                | PsiRewriteWitness::ProofCertifiedScalarIdentity { .. }
        ) {
            return Err(PsiRewriteCandidateError::ProofWitnessSafetyMismatch);
        }
        match &patch {
            PsiRewritePatch::ReplaceIntegerOperationWithConstant(_)
            | PsiRewritePatch::ReplaceBooleanOperationWithConstant(_) => {
                if provenance.iter().any(|row| {
                    row.disposition
                        != ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(
                            location.unwrap(),
                        ))
                        || row.input != PsiRealizationSite::Node(location.unwrap())
                }) {
                    return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
                }
            }
            PsiRewritePatch::RemoveRedundantBlockParameter(patch) => {
                if provenance.is_empty()
                    || provenance.iter().any(|row| {
                        let ProvenanceDisposition::RealizedAt(site) = row.disposition else {
                            return true;
                        };
                        site.machine() != patch.machine
                            || site
                                .node()
                                .is_some_and(|location| !affected_blocks.contains(&location.block))
                    })
                {
                    return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
                }
                if substitutions.as_slice()
                    != [ScalarSubstitution {
                        from: patch.parameter,
                        to: patch.replacement,
                        scalar_type: patch.scalar_type,
                    }]
                {
                    return Err(PsiRewriteCandidateError::BlockParameterSubstitutionMismatch);
                }
            }
            PsiRewritePatch::FoldConstantConditional(patch) => {
                let selected = PsiRealizationSite::Edge {
                    machine: location.unwrap().machine,
                    edge: patch.selected_edge,
                };
                let rejected = PsiRealizationSite::Edge {
                    machine: location.unwrap().machine,
                    edge: patch.rejected_edge,
                };
                if provenance
                    .iter()
                    .filter(|row| {
                        row.input == selected
                            && row.disposition == ProvenanceDisposition::RealizedAt(selected)
                    })
                    .count()
                    != 1
                    || provenance
                        .iter()
                        .filter(|row| {
                            row.input == rejected
                                && row.disposition
                                    == ProvenanceDisposition::ProvenUnreachableAt(rejected)
                        })
                        .count()
                        != 1
                    || !provenance.iter().any(|row| {
                        matches!(
                            row.disposition,
                            ProvenanceDisposition::ProvenUnreachableAt(_)
                        )
                    })
                    || !substitutions.is_empty()
                {
                    return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
                }
            }
            PsiRewritePatch::ThreadLinearEmptyBlock(patch) => {
                let incoming = PsiRealizationSite::Edge {
                    machine: patch.predecessor.machine,
                    edge: patch.incoming_edge,
                };
                let outgoing = PsiRealizationSite::Edge {
                    machine: patch.predecessor.machine,
                    edge: patch.outgoing_edge,
                };
                if patch.empty.node != 0
                    || patch.empty.machine != patch.predecessor.machine
                    || !affected_blocks.contains(&patch.empty.block)
                    || provenance.iter().any(|row| {
                        let ProvenanceDisposition::RealizedAt(site) = row.disposition else {
                            return true;
                        };
                        site.machine() != patch.predecessor.machine
                            || site
                                .node()
                                .is_some_and(|location| !affected_blocks.contains(&location.block))
                    })
                    || !provenance.iter().any(|row| {
                        row.input == incoming
                            && row.disposition == ProvenanceDisposition::RealizedAt(incoming)
                    })
                    || !provenance.iter().any(|row| {
                        row.input == outgoing
                            && row.disposition == ProvenanceDisposition::RealizedAt(incoming)
                    })
                    || !substitutions.is_empty()
                    || !matches!(witness, PsiRewriteWitness::StructuralIdentity)
                {
                    return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
                }
            }
            PsiRewritePatch::ThreadPathQualifiedEmptyBlock(patch) => {
                let outgoing = PsiRealizationSite::Edge {
                    machine: patch.empty.machine,
                    edge: patch.outgoing_edge,
                };
                let fanout = provenance
                    .iter()
                    .filter(|row| row.input == outgoing && row.disposition.is_realized())
                    .count();
                if patch.empty.node != 0
                    || !affected_blocks.contains(&patch.empty.block)
                    || fanout == 0
                    || provenance.iter().any(|row| {
                        let ProvenanceDisposition::RealizedAt(site) = row.disposition else {
                            return true;
                        };
                        site.machine() != patch.empty.machine
                            || site
                                .node()
                                .is_some_and(|location| !affected_blocks.contains(&location.block))
                    })
                    || !substitutions.is_empty()
                    || !matches!(witness, PsiRewriteWitness::StructuralIdentity)
                {
                    return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
                }
            }
            PsiRewritePatch::MergeAdjacentBlock(patch) => {
                let incoming = PsiRealizationSite::Edge {
                    machine: patch.predecessor.machine,
                    edge: patch.incoming_edge,
                };
                if !affected_blocks.contains(&patch.target)
                    || !provenance.iter().any(|row| row.input == incoming)
                    || provenance.iter().any(|row| {
                        let ProvenanceDisposition::RealizedAt(site) = row.disposition else {
                            return true;
                        };
                        site.machine() != patch.predecessor.machine
                            || site
                                .node()
                                .is_some_and(|location| !affected_blocks.contains(&location.block))
                    })
                    || !matches!(witness, PsiRewriteWitness::OwnershipFrontiers(_))
                {
                    return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
                }
            }
            PsiRewritePatch::MergeNonAdjacentBlock(patch) => {
                let incoming = PsiRealizationSite::Edge {
                    machine: patch.predecessor.machine,
                    edge: patch.incoming_edge,
                };
                if !affected_blocks.contains(&patch.target)
                    || !provenance.iter().any(|row| row.input == incoming)
                    || provenance.iter().any(|row| {
                        let ProvenanceDisposition::RealizedAt(site) = row.disposition else {
                            return true;
                        };
                        site.machine() != patch.predecessor.machine
                            || site
                                .node()
                                .is_some_and(|location| !affected_blocks.contains(&location.block))
                    })
                    || !matches!(witness, PsiRewriteWitness::StructuralIdentity)
                {
                    return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
                }
            }
            PsiRewritePatch::FuseSharedTerminalJump(patch) => {
                let incoming = PsiRealizationSite::Edge {
                    machine: patch.predecessor.machine,
                    edge: patch.incoming_edge,
                };
                if !affected_blocks.contains(&patch.target)
                    || !provenance.iter().any(|row| row.input == incoming)
                    || provenance.iter().any(|row| {
                        let ProvenanceDisposition::RealizedAt(site) = row.disposition else {
                            return true;
                        };
                        site.machine() != patch.predecessor.machine
                            || site
                                .node()
                                .is_some_and(|location| !affected_blocks.contains(&location.block))
                    })
                    || !matches!(witness, PsiRewriteWitness::StructuralIdentity)
                {
                    return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
                }
            }
            PsiRewritePatch::RemoveDeadScalarNode(patch) => {
                let input = PsiRealizationSite::Node(patch.location);
                if !substitutions.is_empty()
                    || !provenance.iter().any(|row| row.input == input)
                    || provenance.iter().any(|row| {
                        let ProvenanceDisposition::RealizedAt(site) = row.disposition else {
                            return true;
                        };
                        site.machine() != patch.location.machine
                            || site
                                .node()
                                .is_some_and(|location| !affected_blocks.contains(&location.block))
                    })
                    || !matches!(
                        witness,
                        PsiRewriteWitness::StructuralIdentity
                            | PsiRewriteWitness::AcceptedObligation(_)
                    )
                {
                    return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
                }
            }
            PsiRewritePatch::EliminateLocalScalarCommonSubexpression(patch) => {
                let redundant_input = PsiRealizationSite::Node(patch.redundant);
                if patch.leader.machine != patch.redundant.machine
                    || patch.leader.block != patch.redundant.block
                    || patch.leader.node >= patch.redundant.node
                    || patch.leader_operation == patch.redundant_operation
                    || patch.leader_result == patch.redundant_result
                    || substitutions.as_slice()
                        != [ScalarSubstitution {
                            from: patch.redundant_result,
                            to: patch.leader_result,
                            scalar_type: patch.scalar_type,
                        }]
                    || !provenance.iter().any(|row| row.input == redundant_input)
                    || provenance.iter().any(|row| {
                        let ProvenanceDisposition::RealizedAt(site) = row.disposition else {
                            return true;
                        };
                        site.machine() != patch.redundant.machine
                            || site
                                .node()
                                .is_some_and(|location| !affected_blocks.contains(&location.block))
                    })
                    || !matches!(
                        witness,
                        PsiRewriteWitness::StructuralIdentity
                            | PsiRewriteWitness::AcceptedObligation(_)
                    )
                {
                    return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
                }
            }
            PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(patch) => {
                let redundant_input = PsiRealizationSite::Node(patch.redundant);
                if patch.leader.machine != patch.redundant.machine
                    || patch.leader.block == patch.redundant.block
                    || patch.leader_operation == patch.redundant_operation
                    || patch.leader_result == patch.redundant_result
                    || substitutions.as_slice()
                        != [ScalarSubstitution {
                            from: patch.redundant_result,
                            to: patch.leader_result,
                            scalar_type: patch.scalar_type,
                        }]
                    || !provenance.iter().any(|row| row.input == redundant_input)
                    || provenance.iter().any(|row| {
                        let ProvenanceDisposition::RealizedAt(site) = row.disposition else {
                            return true;
                        };
                        site.machine() != patch.redundant.machine
                            || site
                                .node()
                                .is_some_and(|location| !affected_blocks.contains(&location.block))
                    })
                    || !matches!(
                        witness,
                        PsiRewriteWitness::StructuralIdentity
                            | PsiRewriteWitness::AcceptedObligation(_)
                    )
                {
                    return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
                }
            }
            PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(patch) => {
                let redundant_input = PsiRealizationSite::Node(patch.redundant);
                if !substitutions.is_empty()
                    || patch.incoming.len() < 2
                    || patch.incoming.windows(2).any(|pair| {
                        (pair[0].edge, pair[0].source) >= (pair[1].edge, pair[1].source)
                    })
                    || patch.incoming.iter().any(|incoming| {
                        incoming.leader.machine != patch.redundant.machine
                            || incoming.leader_operation == patch.redundant_operation
                            || incoming.leader_result == patch.redundant_result
                            || !affected_blocks.contains(&incoming.source)
                    })
                    || !provenance.iter().any(|row| row.input == redundant_input)
                    || provenance.iter().any(|row| {
                        let ProvenanceDisposition::RealizedAt(site) = row.disposition else {
                            return true;
                        };
                        site.machine() != patch.redundant.machine
                            || site
                                .node()
                                .is_some_and(|location| !affected_blocks.contains(&location.block))
                    })
                    || !matches!(
                        witness,
                        PsiRewriteWitness::StructuralIdentity
                            | PsiRewriteWitness::AcceptedObligation(_)
                    )
                {
                    return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
                }
            }
            PsiRewritePatch::EliminateProofCertifiedScalarIdentity(patch) => {
                let input = PsiRealizationSite::Node(patch.location);
                if substitutions.as_slice()
                    != [ScalarSubstitution {
                        from: patch.result,
                        to: patch.replacement,
                        scalar_type: ScalarType::Integer(patch.scalar_type),
                    }]
                    || patch.result == patch.replacement
                    || !provenance.iter().any(|row| row.input == input)
                    || provenance.iter().any(|row| {
                        let ProvenanceDisposition::RealizedAt(site) = row.disposition else {
                            return true;
                        };
                        site.machine() != patch.location.machine
                            || site
                                .node()
                                .is_some_and(|location| !affected_blocks.contains(&location.block))
                    })
                    || !matches!(
                        witness,
                        PsiRewriteWitness::ProofCertifiedScalarIdentity { .. }
                    )
                {
                    return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
                }
            }
            PsiRewritePatch::PruneUnreachablePrivateMachines(patch) => {
                let pruned = patch
                    .machines
                    .iter()
                    .map(|row| row.machine)
                    .collect::<BTreeSet<_>>();
                if !affected_blocks.is_empty()
                    || !substitutions.is_empty()
                    || !matches!(witness, PsiRewriteWitness::StructuralIdentity)
                    || provenance.iter().any(|row| {
                        !pruned.contains(&row.input.machine())
                            || !matches!(row.disposition, ProvenanceDisposition::ProvenUnreachableAt(site) if site == row.input)
                    })
                {
                    return Err(PsiRewriteCandidateError::PatchDecisionPointMismatch);
                }
            }
        }
        let canonical = encode_candidate(
            input,
            contract,
            &decision_point,
            &affected_blocks,
            &substitutions,
            &provenance,
            &witness,
            predicted_cost_delta,
            &patch,
        );
        let identity = OptimizationCandidateIdentity::from_canonical_bytes(&canonical);
        Ok(Self {
            identity,
            input,
            rule: contract.identity(),
            decision_point,
            affected_blocks,
            required_analyses: contract.required_analyses(),
            invalidated_analyses: contract.invalidated_analyses(),
            safety_class: contract.safety_class(),
            substitutions,
            provenance,
            witness,
            predicted_cost_delta,
            patch,
        })
    }

    pub const fn identity(&self) -> OptimizationCandidateIdentity {
        self.identity
    }

    pub const fn input(&self) -> OptimizationUnitIdentity {
        self.input
    }

    pub const fn rule(&self) -> OptimizationRuleIdentity {
        self.rule
    }

    pub const fn decision_point(&self) -> &PsiRewriteDecisionPoint {
        &self.decision_point
    }

    pub const fn node_decision_point(&self) -> Option<NodeLocation> {
        match &self.decision_point {
            PsiRewriteDecisionPoint::Node(location) => Some(*location),
            PsiRewriteDecisionPoint::MachineSet(_) => None,
        }
    }

    pub fn affected_machines(&self) -> &[MachineId] {
        match &self.decision_point {
            PsiRewriteDecisionPoint::Node(_) => &[],
            PsiRewriteDecisionPoint::MachineSet(machines) => machines,
        }
    }

    pub fn affected_blocks(&self) -> &[BlockId] {
        &self.affected_blocks
    }

    pub const fn required_analyses(&self) -> AnalysisSet {
        self.required_analyses
    }

    pub const fn invalidated_analyses(&self) -> AnalysisInvalidationSet {
        self.invalidated_analyses
    }

    pub const fn safety_class(&self) -> OptimizationSafetyClass {
        self.safety_class
    }

    pub fn substitutions(&self) -> &[ScalarSubstitution] {
        &self.substitutions
    }

    pub fn provenance(&self) -> &[ProvenanceRewrite] {
        &self.provenance
    }

    pub const fn scalar_evaluation_witness(&self) -> Option<ScalarEvaluationWitness> {
        match &self.witness {
            PsiRewriteWitness::ScalarEvaluation(witness) => Some(*witness),
            PsiRewriteWitness::RedundantBlockParameter(_)
            | PsiRewriteWitness::AcceptedObligation(_)
            | PsiRewriteWitness::ProofCertifiedScalarIdentity { .. }
            | PsiRewriteWitness::OwnershipFrontiers(_)
            | PsiRewriteWitness::StructuralIdentity => None,
        }
    }

    pub fn redundant_block_parameter_witness(&self) -> Option<&RedundantBlockParameterWitness> {
        match &self.witness {
            PsiRewriteWitness::ScalarEvaluation(_) => None,
            PsiRewriteWitness::RedundantBlockParameter(witness) => Some(witness),
            PsiRewriteWitness::AcceptedObligation(_) => None,
            PsiRewriteWitness::ProofCertifiedScalarIdentity { .. } => None,
            PsiRewriteWitness::OwnershipFrontiers(_) => None,
            PsiRewriteWitness::StructuralIdentity => None,
        }
    }

    pub const fn accepted_obligation_witness(&self) -> Option<AcceptedObligationFactIdentity> {
        match &self.witness {
            PsiRewriteWitness::AcceptedObligation(identity) => Some(*identity),
            PsiRewriteWitness::ProofCertifiedScalarIdentity {
                obligation_fact, ..
            } => Some(*obligation_fact),
            PsiRewriteWitness::ScalarEvaluation(_)
            | PsiRewriteWitness::RedundantBlockParameter(_)
            | PsiRewriteWitness::OwnershipFrontiers(_)
            | PsiRewriteWitness::StructuralIdentity => None,
        }
    }

    pub const fn proof_certified_scalar_identity_witness(
        &self,
    ) -> Option<(ScalarConstantFactIdentity, AcceptedObligationFactIdentity)> {
        match &self.witness {
            PsiRewriteWitness::ProofCertifiedScalarIdentity {
                constant_fact,
                obligation_fact,
            } => Some((*constant_fact, *obligation_fact)),
            PsiRewriteWitness::ScalarEvaluation(_)
            | PsiRewriteWitness::RedundantBlockParameter(_)
            | PsiRewriteWitness::AcceptedObligation(_)
            | PsiRewriteWitness::OwnershipFrontiers(_)
            | PsiRewriteWitness::StructuralIdentity => None,
        }
    }

    pub fn ownership_frontier_witness(&self) -> Option<&OwnershipFrontierWitness> {
        match &self.witness {
            PsiRewriteWitness::OwnershipFrontiers(witness) => Some(witness),
            _ => None,
        }
    }

    pub fn consumed_facts(&self) -> Vec<OptimizationFactReference> {
        let mut facts = match &self.witness {
            PsiRewriteWitness::ScalarEvaluation(ScalarEvaluationWitness::Unary {
                operand_fact,
            }) => {
                vec![OptimizationFactReference::ScalarConstant(*operand_fact)]
            }
            PsiRewriteWitness::ScalarEvaluation(ScalarEvaluationWitness::Binary {
                left_fact,
                right_fact,
            }) => vec![
                OptimizationFactReference::ScalarConstant(*left_fact),
                OptimizationFactReference::ScalarConstant(*right_fact),
            ],
            PsiRewriteWitness::ScalarEvaluation(ScalarEvaluationWitness::ProofCertifiedUnary {
                operand_fact,
                obligation_fact,
            }) => vec![
                OptimizationFactReference::ScalarConstant(*operand_fact),
                OptimizationFactReference::AcceptedObligation(*obligation_fact),
            ],
            PsiRewriteWitness::ScalarEvaluation(
                ScalarEvaluationWitness::ProofCertifiedBinary {
                    left_fact,
                    right_fact,
                    obligation_fact,
                },
            ) => vec![
                OptimizationFactReference::ScalarConstant(*left_fact),
                OptimizationFactReference::ScalarConstant(*right_fact),
                OptimizationFactReference::AcceptedObligation(*obligation_fact),
            ],
            PsiRewriteWitness::ScalarEvaluation(
                ScalarEvaluationWitness::RangeAgainstConstant {
                    range_fact,
                    constant_fact,
                },
            ) => vec![
                OptimizationFactReference::ValueRange(*range_fact),
                OptimizationFactReference::ScalarConstant(*constant_fact),
            ],
            PsiRewriteWitness::ScalarEvaluation(ScalarEvaluationWitness::RangeAgainstRange {
                left_range_fact,
                right_range_fact,
            }) => vec![
                OptimizationFactReference::ValueRange(*left_range_fact),
                OptimizationFactReference::ValueRange(*right_range_fact),
            ],
            PsiRewriteWitness::AcceptedObligation(identity) => {
                vec![OptimizationFactReference::AcceptedObligation(*identity)]
            }
            PsiRewriteWitness::ProofCertifiedScalarIdentity {
                constant_fact,
                obligation_fact,
            } => vec![
                OptimizationFactReference::ScalarConstant(*constant_fact),
                OptimizationFactReference::AcceptedObligation(*obligation_fact),
            ],
            PsiRewriteWitness::OwnershipFrontiers(witness) => witness
                .rows
                .iter()
                .map(|row| OptimizationFactReference::OwnershipFrontier(row.fact))
                .collect(),
            PsiRewriteWitness::RedundantBlockParameter(_)
            | PsiRewriteWitness::StructuralIdentity => Vec::new(),
        };
        facts.sort_unstable();
        facts.dedup();
        facts
    }

    pub const fn predicted_cost_delta(&self) -> i64 {
        self.predicted_cost_delta
    }

    pub fn patch(&self) -> PsiRewritePatch {
        self.patch.clone()
    }

    pub const fn patch_ref(&self) -> &PsiRewritePatch {
        &self.patch
    }
}
