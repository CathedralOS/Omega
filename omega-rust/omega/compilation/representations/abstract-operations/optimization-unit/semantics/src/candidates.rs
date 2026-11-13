/*
We validate proposed `PsiRewriteCandidate` patches by routing each candidate to
the independent replay checker of the pass family that proposed it. Each family
validator checks its analysis contract, replays the transformation on a clone
of the admitted input unit, verifies provenance disposition and work-settlement
conservation, rebuilds derived function metadata and unit identity, re-runs
whole-unit validation, and confirms that all blocks and functions outside the
declared rewrite region remain untouched before issuing a `ValidatedPsiRewrite`.
*/

use optimization_core::{OptimizationCandidateIdentity, OptimizationValidatorIdentity};
use optimization_unit::{
    ProvenanceRewrite, PsiOptimizationFunction, PsiOptimizationUnit, PsiRewriteCandidate,
    PsiRewritePatch,
};

use crate::OptimizationUnitValidationError;

macro_rules! candidate_patch {
    ($patch:ident, $variant:ident, $subject:ident) => {
        let PsiRewritePatch::$variant($patch) = $subject.patch() else {
            return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
        };
    };
}

pub(crate) mod case_membership_specialization {
    use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
    use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

    mod replay {
        use crate::candidates::candidate_block;
        use crate::candidates::candidate_function;
        use crate::candidates::candidate_node;
        use crate::candidates::realized_at_own_site;
        use crate::candidates::state_specialization::cyclic_machines;
        use crate::candidates::{
            accept_region_rewrite, canonicalize_provenance, land_folded_nodes,
        };
        use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
        use abstract_operations::AbstractOperation as O;
        use optimization_unit::{
            FoldedCaseMembershipRow, OptimizationNode, ProvenanceRewrite, PsiOptimizationFunction,
            PsiOptimizationUnit, PsiRealizationSite, PsiRewriteCandidate, PsiRewritePatch,
            declared_structural_type, resolved_sole_case, sole_case_at_path,
        };
        use semantic_vocabulary::{
            BlockId, MachineId, OperationId, PlaceId, ScalarType, StructuralCaseId,
            StructuralTypeId,
        };

        use optimization_unit::established_case_at;
        use terminal_psi::StructuralPlaceDeclaration;

        struct MembershipEvidence<'a> {
            declaration: &'a StructuralPlaceDeclaration,
            root_type: Option<StructuralTypeId>,
            root_basis: Option<(StructuralCaseId, Option<OperationId>)>,
            function: &'a PsiOptimizationFunction,
        }

        fn membership_evidence<'a>(
            unit: &PsiOptimizationUnit,
            function: &'a PsiOptimizationFunction,
            place: PlaceId,
        ) -> Option<MembershipEvidence<'a>> {
            let declaration = function.structural_place(place)?;
            Some(MembershipEvidence {
                declaration,
                root_type: declared_structural_type(function, declaration),
                root_basis: optimization_unit::proof_basis(unit, function, declaration),
                function,
            })
        }

        fn admit_membership_node(
            unit: &PsiOptimizationUnit,
            evidence: &MembershipEvidence<'_>,
            machine: MachineId,
            block: BlockId,
            node_index: usize,
            node: &OptimizationNode,
        ) -> Option<FoldedCaseMembershipRow> {
            let O::StructuralCaseMembership {
                psi_operation,
                result,
                source,
                path,
                case,
            } = &node.operation
            else {
                return None;
            };
            if *source != evidence.declaration.id || result.scalar_type != ScalarType::Boolean {
                return None;
            }
            let (proven_case, producer) =
                established_case_at(unit, evidence.function, evidence.declaration, path)
                    .map(|(operation, case)| (case, Some(operation)))
                    .or_else(|| {
                        resolved_sole_case(unit, evidence.root_type?, path).map(|case| (case, None))
                    })?;
            Some(FoldedCaseMembershipRow {
                site: optimization_unit::NodeLocation {
                    machine,
                    block,
                    node: u32::try_from(node_index).ok()?,
                },
                psi_operation: *psi_operation,
                result: result.value,
                source: evidence.declaration.id,
                producer,
                observed_case: *case,
                proven_case,
                outcome: proven_case == *case,
            })
        }

        fn plan_memberships(
            unit: &PsiOptimizationUnit,
            evidence: &MembershipEvidence<'_>,
            function: &PsiOptimizationFunction,
        ) -> Vec<FoldedCaseMembershipRow> {
            let mut memberships = Vec::new();
            for block in &function.blocks {
                for (node_index, node) in block.nodes.iter().enumerate() {
                    let Some(row) = admit_membership_node(
                        unit,
                        evidence,
                        function.machine,
                        block.id,
                        node_index,
                        node,
                    ) else {
                        continue;
                    };
                    memberships.push(row);
                }
            }
            memberships.sort_by_key(|row| row.site);
            memberships
        }

        fn folded_node(
            row: &FoldedCaseMembershipRow,
            unit: &PsiOptimizationUnit,
            function: &PsiOptimizationFunction,
        ) -> Result<OptimizationNode, OptimizationUnitValidationError> {
            let node = candidate_node(candidate_block(function, row.site.block)?, row.site.node)?;
            let O::StructuralCaseMembership {
                psi_operation,
                result,
                source,
                path,
                case,
            } = &node.operation
            else {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            };
            if row.site.machine != function.machine
                || *psi_operation != row.psi_operation
                || result.value != row.result
                || result.scalar_type != ScalarType::Boolean
                || *source != row.source
                || *case != row.observed_case
                || row.outcome != (row.proven_case == row.observed_case)
            {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            match row.producer {
                Some(producer) => {
                    let resolved = function
                        .structural_place(row.source)
                        .and_then(|declaration| {
                            established_case_at(unit, function, declaration, path)
                        });
                    if resolved != Some((producer, row.proven_case)) {
                        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                    }
                }
                None => {
                    let declaration = function.structural_place(row.source);
                    if declaration.and_then(|declaration| {
                        sole_case_at_path(unit, function, declaration, path)
                    }) != Some(row.proven_case)
                    {
                        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                    }
                }
            }
            let operation = O::BooleanConstant {
                psi_operation: *psi_operation,
                result: result.value,
                value: row.outcome,
            };
            Ok(OptimizationNode {
                operation,
                provenance: node.provenance.clone(),
                work: node.work.clone(),
                effect: node.effect,
                definitions: node.definitions.clone(),
                uses: node.uses.clone(),
                successors: node.successors.clone(),
                ownership: node.ownership.clone(),
            })
        }

        fn accepted_provenance(
            function: &PsiOptimizationFunction,
            rows: &[FoldedCaseMembershipRow],
        ) -> Result<Vec<ProvenanceRewrite>, OptimizationUnitValidationError> {
            let mut provenance = Vec::new();
            for row in rows {
                let node =
                    candidate_node(candidate_block(function, row.site.block)?, row.site.node)?;
                let site = PsiRealizationSite::Node(row.site);
                provenance.push(realized_at_own_site(site, &node.provenance, &node.work));
            }
            canonicalize_provenance(&mut provenance);
            Ok(provenance)
        }

        pub(super) fn validate(
            input: &PsiOptimizationUnit,
            candidate: &PsiRewriteCandidate,
        ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
            candidate_patch!(patch, SpecializeCaseMembership, candidate);
            if candidate.node_decision_point() != patch.memberships.first().map(|row| row.site) {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            if patch.memberships.is_empty()
                || candidate.predicted_cost_delta()
                    != -i64::try_from(patch.memberships.len())
                        .map_err(|_| OptimizationUnitValidationError::CandidatePatchMismatch)?
            {
                return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
            }
            if cyclic_machines(input).contains(&patch.machine) {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            let function = candidate_function(input, patch.machine)?;
            let Some(evidence) = membership_evidence(input, function, patch.place) else {
                return Err(OptimizationUnitValidationError::CandidateLocationMissing);
            };
            if patch.producer != evidence.root_basis.and_then(|(_, producer)| producer) {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            let replayed = plan_memberships(input, &evidence, function);
            if replayed.is_empty() || patch.memberships != replayed {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            let mut expected_blocks = patch
                .memberships
                .iter()
                .map(|row| row.site.block)
                .collect::<Vec<_>>();
            expected_blocks.sort_unstable();
            expected_blocks.dedup();
            let expected_provenance = accepted_provenance(function, &patch.memberships)?;

            let folded = patch
                .memberships
                .iter()
                .map(|row| folded_node(row, input, function).map(|node| (row.site, node)))
                .collect::<Result<Vec<_>, _>>()?;
            accept_region_rewrite(
                input,
                candidate,
                patch.machine,
                &expected_blocks,
                b"omega.validator.case-membership-specialization.v1",
                expected_provenance,
                |output| land_folded_nodes(output, patch.machine, folded),
            )
        }
    }
    pub fn validate_case_membership_specialization_candidate(
        input: &PsiOptimizationUnit,
        candidate: &PsiRewriteCandidate,
    ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
        super::contract::admit(
            input,
            candidate,
            &super::contract::CASE_MEMBERSHIP_SPECIALIZATION,
        )?;
        if !candidate.substitutions().is_empty() {
            return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
        }
        replay::validate(input, candidate)
    }
}
pub(crate) mod constant_installation {
    use crate::OptimizationUnitValidationError;
    use crate::candidates::observation::{
        ClosedScalarObservationBoundary, reconstruct_closed_scalar_node_boundary,
    };
    use crate::candidates::sparse_conditional_constant_propagation::{
        observation_at, same_closed_scalar_observation,
    };
    use optimization_unit::{
        NodeLocation, PsiNodeObservation, PsiOptimizationUnit, ValueDefinition, ValueDefinitionSite,
    };
    use semantic_vocabulary::{ScalarType, ValueId};

    pub(crate) fn install_evaluated_constant(
        input: &PsiOptimizationUnit,
        location: NodeLocation,
        operation: abstract_operations::AbstractOperation,
        result: ValueId,
        scalar_type: ScalarType,
        input_observation: &PsiNodeObservation,
        input_live: &ClosedScalarObservationBoundary,
    ) -> Result<PsiOptimizationUnit, OptimizationUnitValidationError> {
        let mut output = input.clone();
        let function = output
            .function_mut(location.machine)
            .expect("candidate source function exists");
        let block = function
            .block_mut(location.block)
            .expect("candidate source block exists");
        let node = &mut block.nodes[usize::try_from(location.node).expect("u32 fits usize")];
        node.operation = operation;
        node.definitions = vec![ValueDefinition {
            value: result,
            scalar_type,
            site: ValueDefinitionSite::Node {
                block: location.block,
                node: location.node,
            },
        }];
        node.uses.clear();
        node.successors.clear();
        node.ownership.clear();
        crate::unit_validation::revalidate_rewrite_output(&mut output, location.machine)?;
        let output_observation = observation_at(&output, location)
            .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
        if !same_closed_scalar_observation(input_observation, &output_observation) {
            return Err(OptimizationUnitValidationError::CandidateObservationMismatch);
        }
        let output_live = reconstruct_closed_scalar_node_boundary(&output, location)
            .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
        if input_live.live_out != output_live.live_out
            || output_live
                .live_in
                .iter()
                .any(|value| !input_live.live_in.contains(value))
        {
            return Err(OptimizationUnitValidationError::CandidateLiveBoundaryMismatch);
        }
        Ok(output)
    }
}
pub(crate) mod contract {
    use optimization_core::AnalysisKind::{
        CallGraph, ControlFlowGraph, Dominators, EffectSummaries, ExecutableEdges, LoopForest,
        OwnershipFrontiers, PostDominators, ScalarConstants, StronglyConnectedComponents,
        UseDefinition, ValueLiveness, ValueRanges,
    };
    use optimization_core::OptimizationSafetyClass::{ExactOperationSemantics, StructuralIdentity};
    use optimization_core::{
        AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationRuleIdentity,
        OptimizationSafetyClass,
    };
    use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

    use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};

    pub(crate) struct RuleContract {
        pub(crate) rule_domain: &'static [u8],
        pub(crate) required_analyses: &'static [AnalysisKind],
        pub(crate) invalidated_analyses: &'static [AnalysisKind],
        pub(crate) safety_class: OptimizationSafetyClass,
        pub(crate) predicted_cost_delta: Option<i64>,
    }

    const CONTROL_FLOW_INVALIDATIONS: &[AnalysisKind] =
        &[ControlFlowGraph, UseDefinition, EffectSummaries];

    const CONTROL_FLOW_AND_CALL_GRAPH_INVALIDATIONS: &[AnalysisKind] =
        &[ControlFlowGraph, CallGraph, UseDefinition, EffectSummaries];

    pub(crate) const USE_DEFINITION_AND_EFFECT_SUMMARIES: &[AnalysisKind] =
        &[UseDefinition, EffectSummaries];

    const SPECIALIZATION_INVALIDATIONS: &[AnalysisKind] =
        &[UseDefinition, ScalarConstants, ValueRanges, EffectSummaries];

    #[rustfmt::skip]
macro_rules! rule_rows {
    ( $( ($name:ident, $domain:literal, $required:expr, $invalidated:expr, $safety:expr, $delta:expr) ),+ $(,)? ) => {
        $(
            pub(crate) const $name: RuleContract = RuleContract {
                rule_domain: $domain,
                required_analyses: $required,
                invalidated_analyses: $invalidated,
                safety_class: $safety,
                predicted_cost_delta: $delta,
            };
        )+
    };
}

    #[rustfmt::skip]
rule_rows![
    (ADJACENT_BLOCK_MERGE, b"omega.psi-rule.adjacent-single-predecessor-block-merge.v5", &[ControlFlowGraph, Dominators, UseDefinition, OwnershipFrontiers], CONTROL_FLOW_INVALIDATIONS, StructuralIdentity, Some(-2)),
    (NON_ADJACENT_BLOCK_MERGE, b"omega.psi-rule.non-adjacent-unique-predecessor-block-merge.v1", &[ControlFlowGraph, Dominators, UseDefinition, OwnershipFrontiers], CONTROL_FLOW_INVALIDATIONS, StructuralIdentity, Some(-2)),
    (LINEAR_EMPTY_BLOCK_THREAD, b"omega.psi-rule.linear-empty-block-thread.v2", &[ControlFlowGraph, UseDefinition, OwnershipFrontiers], CONTROL_FLOW_INVALIDATIONS, StructuralIdentity, Some(-3)),
    (PATH_QUALIFIED_EMPTY_BLOCK_THREAD, b"omega.psi-rule.path-qualified-empty-block-thread.v1", &[ControlFlowGraph, UseDefinition, OwnershipFrontiers], CONTROL_FLOW_INVALIDATIONS, StructuralIdentity, Some(-3)),
    (SHARED_TERMINAL_JUMP_FUSION, b"omega.psi-rule.shared-terminal-jump-fusion.v2", &[ControlFlowGraph, OwnershipFrontiers, PostDominators], CONTROL_FLOW_INVALIDATIONS, StructuralIdentity, Some(-1)),
    (CONSTANT_CONDITIONAL_FOLD, b"omega.psi-rule.constant-conditional-fold.v5", &[ControlFlowGraph, ScalarConstants], CONTROL_FLOW_AND_CALL_GRAPH_INVALIDATIONS, ExactOperationSemantics, Some(-1)),
    (UNREACHABLE_PRIVATE_MACHINE_PRUNING, b"omega.psi-rule.unreachable-private-machine-pruning.v1", &[CallGraph], CONTROL_FLOW_AND_CALL_GRAPH_INVALIDATIONS, StructuralIdentity, None),
    (REDUNDANT_BLOCK_PARAMETER, b"omega.psi-rule.redundant-block-parameter.v1", &[ControlFlowGraph, Dominators, UseDefinition], USE_DEFINITION_AND_EFFECT_SUMMARIES, StructuralIdentity, Some(-1)),
    (CASE_MEMBERSHIP_SPECIALIZATION, b"omega.psi-rule.case-membership-specialization.v1", &[StronglyConnectedComponents], SPECIALIZATION_INVALIDATIONS, StructuralIdentity, None),
    (FIELD_VALUE_SPECIALIZATION, b"omega.psi-rule.field-value-specialization.v1", &[StronglyConnectedComponents], SPECIALIZATION_INVALIDATIONS, StructuralIdentity, None),
    (STATE_ARGUMENT_SPECIALIZATION, b"omega.psi-rule.state-argument-specialization.v1", &[ScalarConstants, StronglyConnectedComponents], &[ControlFlowGraph, Dominators, PostDominators, LoopForest, StronglyConnectedComponents, UseDefinition, ExecutableEdges, ScalarConstants, ValueRanges, EffectSummaries, ValueLiveness], StructuralIdentity, None),
];

    pub(crate) fn admit_origin(
        input: &PsiOptimizationUnit,
        candidate: &PsiRewriteCandidate,
    ) -> Result<(), OptimizationUnitValidationError> {
        validate_psi_optimization_unit(input)?;
        if candidate.input() != input.identity {
            return Err(OptimizationUnitValidationError::CandidateInputMismatch);
        }
        Ok(())
    }

    pub(crate) fn admit_roster(
        candidate: &PsiRewriteCandidate,
        required_analyses: &[AnalysisKind],
        invalidated_analyses: &[AnalysisKind],
        safety_class: OptimizationSafetyClass,
        predicted_cost_delta: Option<i64>,
    ) -> Result<(), OptimizationUnitValidationError> {
        if candidate.required_analyses() != AnalysisSet::new(required_analyses.iter().copied())
            || candidate.invalidated_analyses()
                != AnalysisInvalidationSet::new(invalidated_analyses.iter().copied())
            || candidate.safety_class() != safety_class
            || predicted_cost_delta.is_some_and(|delta| candidate.predicted_cost_delta() != delta)
        {
            return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
        }
        Ok(())
    }

    pub(crate) fn admit(
        input: &PsiOptimizationUnit,
        candidate: &PsiRewriteCandidate,
        contract: &RuleContract,
    ) -> Result<(), OptimizationUnitValidationError> {
        admit_origin(input, candidate)?;
        if candidate.rule() != OptimizationRuleIdentity::from_canonical_bytes(contract.rule_domain)
        {
            return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
        }
        admit_roster(
            candidate,
            contract.required_analyses,
            contract.invalidated_analyses,
            contract.safety_class,
            contract.predicted_cost_delta,
        )
    }
}
pub(crate) mod control_flow_cleanup {
    use crate::OptimizationUnitValidationError;
    use abstract_operations::{AbstractOperation as O, ValueBinding};
    use optimization_unit::{
        OptimizationNode, PsiOptimizationFunction, PsiRewriteCandidate, ScalarSubstitution,
        ValueDefinition,
    };
    use semantic_vocabulary::{BlockId, EdgeId};

    mod block_merging {
        mod adjacent {
            use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
            use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

            mod replay {
                use crate::candidates::control_flow_cleanup::{
                    checked_scalar_substitutions, patched_jump_bindings, require_scalar_bindings,
                };
                use crate::candidates::global_value_numbering::{
                    independent_reachable_dominators, independently_replacement_dominates_uses,
                };
                use crate::candidates::rewrite_accounting::adjacent_merge::{
                    reconstruct_adjacent_merge_accounting,
                    reconstruct_adjacent_merge_ownership_witness,
                };
                use crate::candidates::rewrite_accounting::substitutions::rewrite_scalar_value_uses;
                use crate::candidates::{
                    accept_region_rewrite, candidate_function, sweep_output_nodes,
                };
                use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
                use abstract_operations::AbstractOperation as O;
                use optimization_unit::{
                    PsiOptimizationUnit, PsiProvenance, PsiRewriteCandidate, PsiRewritePatch,
                };

                pub(super) fn validate(
                    input: &PsiOptimizationUnit,
                    candidate: &PsiRewriteCandidate,
                ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
                    candidate_patch!(patch, MergeAdjacentBlock, candidate);
                    if candidate.node_decision_point() != Some(patch.predecessor) {
                        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                    }
                    let function = candidate_function(input, patch.predecessor.machine)?;
                    require_scalar_bindings(function)?;

                    let predecessor_position = function
                        .block_index(patch.predecessor.block)
                        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
                    let target_position = function
                        .block_index(patch.target)
                        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
                    if target_position != predecessor_position + 1 || function.entry == patch.target
                    {
                        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
                    }
                    let predecessor = &function.blocks[predecessor_position];
                    let target = &function.blocks[target_position];
                    let predecessor_index = usize::try_from(patch.predecessor.node)
                        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
                    let eligible_first = target.nodes.first().is_some_and(|node| {
                        (node.successors.is_empty()
                            && (matches!(
                                node.provenance.first(),
                                Some(PsiProvenance::Operation(_))
                            ) || (matches!(
                                node.provenance.first(),
                                Some(PsiProvenance::Edge(_))
                            ) && matches!(
                                node.operation,
                                abstract_operations::return_or_return_unit_pattern!()
                                    | abstract_operations::return_structural_or_crash_pattern!()
                            ))))
                            || (matches!(node.operation, O::Conditional { .. })
                                && node.successors.len() == 2
                                && node.provenance.is_empty())
                    });
                    if predecessor_index + 1 != predecessor.nodes.len() || !eligible_first {
                        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                    }
                    let predecessor_node = &predecessor.nodes[predecessor_index];
                    let bindings =
                        patched_jump_bindings(predecessor_node, patch.incoming_edge, patch.target)?;
                    let incoming = function
                        .block_edges()
                        .filter(|edge| edge.target == patch.target)
                        .collect::<Vec<_>>();
                    if incoming.len() != 1 || incoming[0].psi_edge != patch.incoming_edge {
                        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
                    }
                    let dominators = independent_reachable_dominators(function);
                    let substitutions = checked_scalar_substitutions(
                        candidate,
                        &target.parameters,
                        bindings,
                        |parameter, binding| {
                            independently_replacement_dominates_uses(
                                function,
                                &dominators,
                                binding.argument,
                                parameter.value,
                                parameter.scalar_type,
                            )
                        },
                    )?;
                    let ownership_witness = reconstruct_adjacent_merge_ownership_witness(
                        input,
                        function,
                        patch.incoming_edge,
                        patch.target,
                    )
                    .ok_or(OptimizationUnitValidationError::CandidateObservationMismatch)?;
                    if candidate.ownership_frontier_witness() != Some(&ownership_witness) {
                        return Err(OptimizationUnitValidationError::CandidateObservationMismatch);
                    }
                    let (expected_blocks, accepted_provenance) =
                        reconstruct_adjacent_merge_accounting(function, patch, &substitutions)
                            .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;

                    accept_region_rewrite(
                        input,
                        candidate,
                        patch.predecessor.machine,
                        &expected_blocks,
                        b"omega.validator.adjacent-single-predecessor-block-merge.v5",
                        accepted_provenance,
                        |output| {
                            let output_function = output
                                .function_mut(patch.predecessor.machine)
                                .expect("candidate function exists");
                            let mut moved = output_function.blocks.remove(target_position);
                            let output_predecessor =
                                &mut output_function.blocks[predecessor_position];
                            let removed = output_predecessor
                                .nodes
                                .pop()
                                .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
                            let removed_edge = removed
                                .edge(patch.incoming_edge)
                                .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
                            let first = moved
                                .nodes
                                .first_mut()
                                .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
                            if first.successors.is_empty() {
                                first.provenance.extend_from_slice(&removed_edge.provenance);
                                first.work.extend_from_slice(&removed_edge.work);
                            } else {
                                for successor in &mut first.successors {
                                    successor
                                        .provenance
                                        .extend_from_slice(&removed_edge.provenance);
                                    successor.work.extend_from_slice(&removed_edge.work);
                                }
                            }
                            output_predecessor.nodes.append(&mut moved.nodes);
                            sweep_output_nodes(output_function, |node| {
                                for substitution in &substitutions {
                                    rewrite_scalar_value_uses(
                                        &mut node.operation,
                                        substitution.from,
                                        substitution.to,
                                    );
                                }
                                Ok(())
                            })?;
                            Ok(())
                        },
                    )
                }
            }
            pub(crate) fn validate_adjacent_block_merge_candidate(
                input: &PsiOptimizationUnit,
                candidate: &PsiRewriteCandidate,
            ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
                super::super::super::contract::admit(
                    input,
                    candidate,
                    &super::super::super::contract::ADJACENT_BLOCK_MERGE,
                )?;
                replay::validate(input, candidate)
            }
        }
        mod non_adjacent {
            use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
            use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

            mod replay {
                use crate::candidates::control_flow_cleanup::{
                    checked_scalar_substitutions, patched_jump_bindings, require_scalar_bindings,
                };
                use crate::candidates::global_value_numbering::{
                    independent_reachable_dominators, independently_replacement_dominates_uses,
                };
                use crate::candidates::rewrite_accounting::adjacent_merge::reconstruct_adjacent_merge_ownership_is_identity;
                use crate::candidates::rewrite_accounting::non_adjacent_merge::reconstruct_non_adjacent_merge_accounting;
                use crate::candidates::rewrite_accounting::substitutions::rewrite_scalar_value_uses;
                use crate::candidates::{
                    accept_region_rewrite, candidate_function, sweep_output_nodes,
                };
                use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
                use abstract_operations::AbstractOperation as O;
                use optimization_unit::{
                    PsiOptimizationUnit, PsiRewriteCandidate, PsiRewritePatch,
                };

                pub(super) fn validate(
                    input: &PsiOptimizationUnit,
                    candidate: &PsiRewriteCandidate,
                ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
                    candidate_patch!(patch, MergeNonAdjacentBlock, candidate);
                    if candidate.node_decision_point() != Some(patch.predecessor) {
                        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                    }
                    let function = candidate_function(input, patch.predecessor.machine)?;
                    require_scalar_bindings(function)?;

                    let predecessor_position = function
                        .block_index(patch.predecessor.block)
                        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
                    let target_position = function
                        .block_index(patch.target)
                        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
                    if function.entry == patch.target
                        || target_position == predecessor_position.saturating_add(1)
                        || function.blocks[target_position].nodes.is_empty()
                        || matches!(function.blocks[target_position].nodes.as_slice(), [node] if matches!(node.operation, O::Jump { .. }))
                    {
                        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
                    }
                    let predecessor = &function.blocks[predecessor_position];
                    let target = &function.blocks[target_position];
                    let predecessor_index = usize::try_from(patch.predecessor.node)
                        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
                    if predecessor_index + 1 != predecessor.nodes.len() {
                        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                    }
                    let predecessor_node = &predecessor.nodes[predecessor_index];
                    let bindings =
                        patched_jump_bindings(predecessor_node, patch.incoming_edge, patch.target)?;
                    let incoming = function
                        .block_edges()
                        .filter(|edge| edge.target == patch.target)
                        .collect::<Vec<_>>();
                    if incoming.len() != 1 || incoming[0].psi_edge != patch.incoming_edge {
                        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
                    }
                    let dominators = independent_reachable_dominators(function);
                    if !dominators
                        .get(&patch.target)
                        .is_some_and(|rows| rows.contains(&patch.predecessor.block))
                    {
                        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
                    }
                    let substitutions = checked_scalar_substitutions(
                        candidate,
                        &target.parameters,
                        bindings,
                        |parameter, binding| {
                            independently_replacement_dominates_uses(
                                function,
                                &dominators,
                                binding.argument,
                                parameter.value,
                                parameter.scalar_type,
                            )
                        },
                    )?;
                    if !reconstruct_adjacent_merge_ownership_is_identity(
                        input,
                        function,
                        patch.incoming_edge,
                        patch.target,
                    ) {
                        return Err(OptimizationUnitValidationError::CandidateObservationMismatch);
                    }
                    let (expected_blocks, accepted_provenance) =
                        reconstruct_non_adjacent_merge_accounting(function, patch, &substitutions)
                            .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;

                    accept_region_rewrite(
                        input,
                        candidate,
                        patch.predecessor.machine,
                        &expected_blocks,
                        b"omega.validator.non-adjacent-unique-predecessor-block-merge.v1",
                        accepted_provenance,
                        |output| {
                            let output_function = output
                                .function_mut(patch.predecessor.machine)
                                .expect("candidate function exists");
                            let output_target_position = output_function
                                .block_index(patch.target)
                                .expect("candidate target exists");
                            let mut moved = output_function.blocks.remove(output_target_position);
                            let output_predecessor_position = output_function
                                .block_index(patch.predecessor.block)
                                .expect("candidate predecessor exists");
                            let removed = output_function.blocks[output_predecessor_position]
                                .nodes
                                .pop()
                                .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
                            let removed_edge = removed
                                .edge(patch.incoming_edge)
                                .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
                            let first = moved
                                .nodes
                                .first_mut()
                                .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
                            if !first.provenance.is_empty() {
                                first.provenance.extend_from_slice(&removed_edge.provenance);
                                first.work.extend_from_slice(&removed_edge.work);
                            } else if !first.successors.is_empty() {
                                for successor in &mut first.successors {
                                    successor
                                        .provenance
                                        .extend_from_slice(&removed_edge.provenance);
                                    successor.work.extend_from_slice(&removed_edge.work);
                                }
                            } else {
                                return Err(
                                    OptimizationUnitValidationError::CandidatePatchMismatch,
                                );
                            }
                            output_function.blocks[output_predecessor_position]
                                .nodes
                                .append(&mut moved.nodes);
                            sweep_output_nodes(output_function, |node| {
                                for substitution in &substitutions {
                                    rewrite_scalar_value_uses(
                                        &mut node.operation,
                                        substitution.from,
                                        substitution.to,
                                    );
                                }
                                Ok(())
                            })?;
                            Ok(())
                        },
                    )
                }
            }
            pub(crate) fn validate_non_adjacent_block_merge_candidate(
                input: &PsiOptimizationUnit,
                candidate: &PsiRewriteCandidate,
            ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
                super::super::super::contract::admit(
                    input,
                    candidate,
                    &super::super::super::contract::NON_ADJACENT_BLOCK_MERGE,
                )?;
                replay::validate(input, candidate)
            }
        }
        pub(crate) use adjacent::validate_adjacent_block_merge_candidate;
        pub(crate) use non_adjacent::validate_non_adjacent_block_merge_candidate;
    }
    mod constant_conditionals {
        use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
        use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

        mod replay {
            use crate::candidates::sparse_conditional_constant_propagation::literal_boolean_fact;
            use crate::candidates::{
                accept_region_rewrite, candidate_block, candidate_function,
                canonicalize_provenance, require_realized_rows, sweep_output_nodes,
            };
            use crate::candidates::{
                candidate_node, proven_unreachable_at_own_site, realized_at_own_site,
            };
            use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
            use abstract_operations::AbstractOperation as O;
            use optimization_unit::{
                ConstantConditionalRewrite, IntegerEvaluationWitness, NodeLocation,
                OptimizationEdge, ProvenanceDisposition, PsiOptimizationFunction,
                PsiOptimizationUnit, PsiProvenance, PsiRealizationSite, PsiRewriteCandidate,
                PsiRewritePatch, ValueUse, reachable_blocks_after_conditional_fold,
            };
            use semantic_vocabulary::{BlockId, EdgeId};
            use std::collections::BTreeSet;

            pub(super) fn validate(
                input: &PsiOptimizationUnit,
                candidate: &PsiRewriteCandidate,
            ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
                candidate_patch!(patch, FoldConstantConditional, candidate);
                if candidate.node_decision_point() != Some(patch.location) {
                    return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                }
                let function = candidate_function(input, patch.location.machine)?;
                let block = candidate_block(function, patch.location.block)?;
                let node = candidate_node(block, patch.location.node)?;
                let O::Conditional {
                    condition,
                    when_true,
                    when_false,
                } = &node.operation
                else {
                    return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                };
                let condition_fact = candidate
                    .scalar_evaluation_witness()
                    .and_then(IntegerEvaluationWitness::unary_operand)
                    .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
                let constant =
                    literal_boolean_fact(function, input.identity, *condition, condition_fact)
                        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
                let (selected, rejected) = if constant {
                    (when_true, when_false)
                } else {
                    (when_false, when_true)
                };
                if patch
                    != (ConstantConditionalRewrite {
                        location: patch.location,
                        condition: *condition,
                        constant,
                        selected_edge: selected.psi_edge,
                        rejected_edge: rejected.psi_edge,
                    })
                {
                    return Err(OptimizationUnitValidationError::CandidateEvaluationMismatch);
                }
                let reachable = reachable_blocks_after_conditional_fold(
                    function,
                    patch.location.block,
                    selected.psi_edge,
                )
                .ok_or(OptimizationUnitValidationError::CandidateReachabilityMismatch)?;
                if function.blocks.iter().any(|block| {
                    !reachable.contains(&block.id) && !block.structural_parameters.is_empty()
                }) {
                    return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
                }
                let (expected_blocks, accepted_provenance) =
                    reconstruct_conditional_fold_accounting(
                        function,
                        patch.location,
                        selected.psi_edge,
                        rejected.psi_edge,
                        &reachable,
                    )
                    .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
                if candidate.affected_blocks() != expected_blocks {
                    return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
                }
                require_realized_rows(candidate, &accepted_provenance)?;
                let selected_site = PsiRealizationSite::Edge {
                    machine: patch.location.machine,
                    edge: selected.psi_edge,
                };
                let selected_work = accepted_provenance
                    .iter()
                    .find(|row| row.disposition == ProvenanceDisposition::RealizedAt(selected_site))
                    .expect("independent accounting includes the selected edge")
                    .work
                    .clone();

                accept_region_rewrite(
                    input,
                    candidate,
                    patch.location.machine,
                    &expected_blocks,
                    b"omega.validator.constant-conditional-fold.v4",
                    accepted_provenance,
                    |output| {
                        let output_function = output
                            .function_mut(patch.location.machine)
                            .expect("candidate function exists");
                        let output_block = output_function
                            .block_mut(patch.location.block)
                            .expect("candidate block exists");
                        let output_node = &mut output_block.nodes
                            [usize::try_from(patch.location.node).expect("u32 fits usize")];
                        output_node.operation = O::Jump {
                            structural_bindings: selected.structural_bindings.clone(),
                            psi_edge: selected.psi_edge,
                            target: selected.target,
                            bindings: selected.bindings.clone(),
                            cleanup_actions: selected.cleanup_actions.clone(),
                        };
                        output_node.definitions.clear();
                        output_node.uses = selected
                            .bindings
                            .iter()
                            .map(|binding| ValueUse {
                                value: binding.argument,
                                block: patch.location.block,
                                node: patch.location.node,
                            })
                            .collect();
                        output_node.successors = vec![OptimizationEdge {
                            structural_bindings: selected.structural_bindings.clone(),
                            psi_edge: selected.psi_edge,
                            target: selected.target,
                            bindings: selected.bindings.clone(),
                            cleanup_actions: selected.cleanup_actions.clone(),
                            provenance: vec![PsiProvenance::Edge(selected.psi_edge)],
                            work: selected_work,
                        }];
                        output_node.ownership.clear();
                        output_node.provenance.clear();
                        output_node.work.clear();
                        output_function
                            .blocks
                            .retain(|block| reachable.contains(&block.id));
                        sweep_output_nodes(output_function, |_| Ok(()))?;
                        Ok(())
                    },
                )
            }

            pub(crate) fn reconstruct_conditional_fold_accounting(
                function: &PsiOptimizationFunction,
                decision: NodeLocation,
                selected_edge: EdgeId,
                rejected_edge: EdgeId,
                reachable: &BTreeSet<BlockId>,
            ) -> Option<(Vec<BlockId>, Vec<optimization_unit::ProvenanceRewrite>)> {
                let decision_node = function
                    .block(decision.block)?
                    .nodes
                    .get(usize::try_from(decision.node).ok()?)?;
                let selected = decision_node.edge(selected_edge)?;
                let rejected = decision_node.edge(rejected_edge)?;
                let selected_site = PsiRealizationSite::Edge {
                    machine: function.machine,
                    edge: selected_edge,
                };
                let rejected_site = PsiRealizationSite::Edge {
                    machine: function.machine,
                    edge: rejected_edge,
                };
                let removed = function
                    .blocks
                    .iter()
                    .map(|block| block.id)
                    .filter(|block| !reachable.contains(block))
                    .collect::<BTreeSet<_>>();
                let mut affected = BTreeSet::from([decision.block]);
                affected.extend(removed.iter().copied());
                let mut realized = vec![realized_at_own_site(
                    selected_site,
                    &selected.provenance,
                    &selected.work,
                )];
                let mut unreachable = vec![proven_unreachable_at_own_site(
                    rejected_site,
                    &rejected.provenance,
                    &rejected.work,
                )];
                let mut expected_effect = 0u64;
                for block in &function.blocks {
                    if removed.contains(&block.id) {
                        for (node_index, node) in block.nodes.iter().enumerate() {
                            let location = NodeLocation {
                                machine: function.machine,
                                block: block.id,
                                node: u32::try_from(node_index).ok()?,
                            };
                            if !node.provenance.is_empty() {
                                let site = PsiRealizationSite::Node(location);
                                unreachable.push(proven_unreachable_at_own_site(
                                    site,
                                    &node.provenance,
                                    &node.work,
                                ));
                            }
                            for edge in &node.successors {
                                let site = PsiRealizationSite::Edge {
                                    machine: function.machine,
                                    edge: edge.psi_edge,
                                };
                                unreachable.push(proven_unreachable_at_own_site(
                                    site,
                                    &edge.provenance,
                                    &edge.work,
                                ));
                            }
                        }
                        continue;
                    }
                    for (node_index, node) in block.nodes.iter().enumerate() {
                        let location = NodeLocation {
                            machine: function.machine,
                            block: block.id,
                            node: u32::try_from(node_index).ok()?,
                        };
                        let effect_changes = node.effect.input != expected_effect
                            || node.effect.output != expected_effect.checked_add(1)?;
                        if effect_changes && location != decision {
                            affected.insert(block.id);
                            if !node.provenance.is_empty() {
                                let site = PsiRealizationSite::Node(location);
                                realized.push(realized_at_own_site(
                                    site,
                                    &node.provenance,
                                    &node.work,
                                ));
                            }
                        }
                        expected_effect = expected_effect.checked_add(1)?;
                    }
                }
                realized.extend(unreachable);
                canonicalize_provenance(&mut realized);
                Some((affected.into_iter().collect(), realized))
            }
        }
        pub(crate) fn validate_constant_conditional_candidate(
            input: &PsiOptimizationUnit,
            candidate: &PsiRewriteCandidate,
        ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
            super::super::contract::admit(
                input,
                candidate,
                &super::super::contract::CONSTANT_CONDITIONAL_FOLD,
            )?;
            if !candidate.substitutions().is_empty() {
                return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
            }
            replay::validate(input, candidate)
        }
    }
    mod empty_block_threading {
        mod linear {
            use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
            use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

            mod replay {
                use crate::candidates::control_flow_cleanup::{
                    patched_jump_bindings, require_scalar_bindings,
                };
                use crate::candidates::rewrite_accounting::threading::{
                    reconstruct_linear_thread_accounting, reconstruct_linear_thread_bindings,
                    reconstruct_linear_thread_ownership_is_identity,
                };
                use crate::candidates::sweep_output_nodes;
                use crate::candidates::{
                    accept_region_rewrite, candidate_block, candidate_function,
                    require_realized_rows,
                };
                use crate::unit_validation::derived_metadata::{
                    expected_definitions, expected_edges, expected_ownership, expected_uses,
                };
                use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
                use abstract_operations::AbstractOperation as O;
                use optimization_unit::{
                    NodeLocation, PsiOptimizationUnit, PsiRewriteCandidate, PsiRewritePatch,
                };

                pub(super) fn validate(
                    input: &PsiOptimizationUnit,
                    candidate: &PsiRewriteCandidate,
                ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
                    candidate_patch!(patch, ThreadLinearEmptyBlock, candidate);
                    if candidate.node_decision_point() != Some(patch.predecessor)
                        || patch.empty.node != 0
                        || patch.empty.machine != patch.predecessor.machine
                    {
                        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                    }
                    let function = candidate_function(input, patch.predecessor.machine)?;
                    require_scalar_bindings(function)?;

                    if function.entry == patch.empty.block {
                        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
                    }
                    let empty_block = candidate_block(function, patch.empty.block)?;
                    let [empty_node] = empty_block.nodes.as_slice() else {
                        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                    };
                    let outgoing_bindings =
                        patched_jump_bindings(empty_node, patch.outgoing_edge, patch.target)?;
                    if empty_block.parameters.iter().any(|parameter| {
                        function.blocks.iter().any(|block| {
                            block.nodes.iter().any(|node| {
                                node.uses.iter().any(|use_site| {
                                    use_site.value == parameter.value
                                        && (use_site.block != empty_block.id || use_site.node != 0)
                                })
                            })
                        })
                    }) {
                        return Err(
                            OptimizationUnitValidationError::CandidateIncomingBindingMismatch,
                        );
                    }

                    let incoming =
                        function
                            .blocks
                            .iter()
                            .flat_map(|block| {
                                block.nodes.iter().enumerate().filter_map(
                                    move |(node_index, node)| {
                                        node.successors
                                            .iter()
                                            .any(|edge| edge.target == patch.empty.block)
                                            .then_some((block, node_index, node))
                                    },
                                )
                            })
                            .collect::<Vec<_>>();
                    let [(predecessor_block, predecessor_index, predecessor_node)] =
                        incoming.as_slice()
                    else {
                        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
                    };
                    let predecessor_location = NodeLocation {
                        machine: function.machine,
                        block: predecessor_block.id,
                        node: u32::try_from(*predecessor_index).map_err(|_| {
                            OptimizationUnitValidationError::CandidateLocationMissing
                        })?,
                    };
                    let incoming_bindings = patched_jump_bindings(
                        predecessor_node,
                        patch.incoming_edge,
                        patch.empty.block,
                    )?;
                    if predecessor_location != patch.predecessor {
                        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                    }
                    let composed_bindings = reconstruct_linear_thread_bindings(
                        &empty_block.parameters,
                        incoming_bindings,
                        outgoing_bindings,
                    )
                    .ok_or(OptimizationUnitValidationError::CandidateIncomingBindingMismatch)?;
                    if !reconstruct_linear_thread_ownership_is_identity(
                        input,
                        function,
                        patch.incoming_edge,
                        patch.empty.block,
                        patch.outgoing_edge,
                        patch.target,
                    ) {
                        return Err(OptimizationUnitValidationError::CandidateObservationMismatch);
                    }
                    let (expected_blocks, accepted_provenance) =
                        reconstruct_linear_thread_accounting(
                            function,
                            patch.predecessor,
                            patch.empty,
                        )
                        .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
                    if candidate.affected_blocks() != expected_blocks {
                        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
                    }
                    require_realized_rows(candidate, &accepted_provenance)?;

                    accept_region_rewrite(
                        input,
                        candidate,
                        patch.predecessor.machine,
                        &expected_blocks,
                        b"omega.validator.linear-empty-block-thread.v2",
                        accepted_provenance,
                        |output| {
                            let output_function = output
                                .function_mut(patch.predecessor.machine)
                                .expect("candidate function exists");
                            let output_predecessor = output_function
                                .block_mut(patch.predecessor.block)
                                .and_then(|block| {
                                    block
                                        .nodes
                                        .get_mut(usize::try_from(patch.predecessor.node).ok()?)
                                })
                                .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
                            let predecessor_edge = predecessor_node
                                .edge(patch.incoming_edge)
                                .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
                            let empty_edge = empty_node
                                .edge(patch.outgoing_edge)
                                .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
                            let mut combined_sources = predecessor_edge.provenance.clone();
                            combined_sources.extend_from_slice(&empty_edge.provenance);
                            let mut combined_work = predecessor_edge.work.clone();
                            combined_work.extend_from_slice(&empty_edge.work);
                            output_predecessor.operation = O::Jump {
                                structural_bindings: Vec::new(),
                                psi_edge: patch.incoming_edge,
                                target: patch.target,
                                bindings: composed_bindings,
                                cleanup_actions: Vec::new(),
                            };
                            output_predecessor.definitions = expected_definitions(
                                &output_predecessor.operation,
                                patch.predecessor.block,
                                patch.predecessor.node,
                            );
                            output_predecessor.uses = expected_uses(
                                &output_predecessor.operation,
                                patch.predecessor.block,
                                patch.predecessor.node,
                            );
                            output_predecessor.successors =
                                expected_edges(&output_predecessor.operation);
                            output_predecessor.successors[0].provenance = combined_sources;
                            output_predecessor.successors[0].work = combined_work;
                            output_predecessor.ownership =
                                expected_ownership(&output_predecessor.operation);
                            output_predecessor.provenance.clear();
                            output_predecessor.work.clear();
                            output_function
                                .blocks
                                .retain(|block| block.id != patch.empty.block);
                            sweep_output_nodes(output_function, |_| Ok(()))?;
                            Ok(())
                        },
                    )
                }
            }
            pub(crate) fn validate_linear_empty_block_candidate(
                input: &PsiOptimizationUnit,
                candidate: &PsiRewriteCandidate,
            ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
                super::super::super::contract::admit(
                    input,
                    candidate,
                    &super::super::super::contract::LINEAR_EMPTY_BLOCK_THREAD,
                )?;
                if !candidate.substitutions().is_empty() {
                    return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
                }
                replay::validate(input, candidate)
            }
        }
        mod path_qualified {
            use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
            use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

            mod replay {
                use crate::candidates::accept_region_rewrite;
                use crate::candidates::candidate_block;
                use crate::candidates::candidate_function;
                use crate::candidates::control_flow_cleanup::{
                    patched_jump_bindings, require_scalar_bindings,
                };
                use crate::candidates::rewrite_accounting::substitutions::rewrite_successor_operation;
                use crate::candidates::rewrite_accounting::threading::{
                    reconstruct_linear_thread_bindings,
                    reconstruct_linear_thread_ownership_is_identity,
                    reconstruct_path_thread_accounting,
                };
                use crate::candidates::sweep_output_nodes;
                use crate::unit_validation::derived_metadata::{
                    expected_definitions, expected_ownership, expected_uses,
                };
                use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
                use abstract_operations::AbstractOperation as O;
                use optimization_unit::{
                    NodeLocation, PsiOptimizationUnit, PsiRewriteCandidate, PsiRewritePatch,
                };

                pub(super) fn validate(
                    input: &PsiOptimizationUnit,
                    candidate: &PsiRewriteCandidate,
                ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
                    candidate_patch!(patch, ThreadPathQualifiedEmptyBlock, candidate);
                    if candidate.node_decision_point() != Some(patch.empty) || patch.empty.node != 0
                    {
                        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                    }
                    let function = candidate_function(input, patch.empty.machine)?;
                    require_scalar_bindings(function)?;

                    if function.entry == patch.empty.block {
                        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
                    }
                    let empty_block = candidate_block(function, patch.empty.block)?;
                    let [empty_node] = empty_block.nodes.as_slice() else {
                        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                    };
                    let outgoing_bindings =
                        patched_jump_bindings(empty_node, patch.outgoing_edge, patch.target)?;
                    if empty_block.parameters.iter().any(|parameter| {
                        function.blocks.iter().any(|block| {
                            block.nodes.iter().any(|node| {
                                node.uses.iter().any(|use_site| {
                                    use_site.value == parameter.value
                                        && (use_site.block != empty_block.id || use_site.node != 0)
                                })
                            })
                        })
                    }) {
                        return Err(
                            OptimizationUnitValidationError::CandidateIncomingBindingMismatch,
                        );
                    }
                    let mut incoming = Vec::new();
                    for block in &function.blocks {
                        for (node_index, node) in block.nodes.iter().enumerate() {
                            for edge in node
                                .successors
                                .iter()
                                .filter(|edge| edge.target == patch.empty.block)
                            {
                                if !edge.cleanup_actions.is_empty() {
                                    return Err(
                                        OptimizationUnitValidationError::CandidatePatchMismatch,
                                    );
                                }
                                let composed = reconstruct_linear_thread_bindings(
                    &empty_block.parameters,
                    &edge.bindings,
                    outgoing_bindings,
                )
                .ok_or(OptimizationUnitValidationError::CandidateIncomingBindingMismatch)?;
                                if !reconstruct_linear_thread_ownership_is_identity(
                                    input,
                                    function,
                                    edge.psi_edge,
                                    patch.empty.block,
                                    patch.outgoing_edge,
                                    patch.target,
                                ) {
                                    return Err(OptimizationUnitValidationError::CandidateObservationMismatch);
                                }
                                incoming.push((
                    NodeLocation {
                        machine: function.machine,
                        block: block.id,
                        node: u32::try_from(node_index).map_err(|_| {
                            OptimizationUnitValidationError::CandidateLocationMissing
                        })?,
                    },
                    edge.psi_edge,
                    composed,
                ));
                            }
                        }
                    }
                    if incoming.is_empty()
                        || (incoming.len() == 1
                            && matches!(
                                function
                                    .block(incoming[0].0.block)
                                    .and_then(|block| block
                                        .nodes
                                        .get(usize::try_from(incoming[0].0.node).ok()?))
                                    .map(|node| &node.operation),
                                Some(O::Jump { .. })
                            ))
                    {
                        return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
                    }
                    let incoming_edges = incoming
                        .iter()
                        .map(|(_, edge, _)| *edge)
                        .collect::<Vec<_>>();
                    let (expected_blocks, accepted_provenance) =
                        reconstruct_path_thread_accounting(function, patch.empty, &incoming_edges)
                            .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;

                    let outgoing_edge = empty_node
                        .edge(patch.outgoing_edge)
                        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
                    accept_region_rewrite(
                        input,
                        candidate,
                        patch.empty.machine,
                        &expected_blocks,
                        b"omega.validator.path-qualified-empty-block-thread.v1",
                        accepted_provenance,
                        |output| {
                            let output_function = output
                                .function_mut(patch.empty.machine)
                                .expect("candidate function exists");
                            for (location, incoming_edge, composed) in &incoming {
                                let node = output_function
                                    .block_mut(location.block)
                                    .and_then(|block| {
                                        block.nodes.get_mut(usize::try_from(location.node).ok()?)
                                    })
                                    .ok_or(
                                        OptimizationUnitValidationError::CandidateLocationMissing,
                                    )?;
                                if !rewrite_successor_operation(
                                    &mut node.operation,
                                    *incoming_edge,
                                    patch.target,
                                    composed,
                                ) {
                                    return Err(
                                        OptimizationUnitValidationError::CandidatePatchMismatch,
                                    );
                                }
                                let edge = node
                                    .successors
                                    .iter_mut()
                                    .find(|edge| edge.psi_edge == *incoming_edge)
                                    .ok_or(
                                        OptimizationUnitValidationError::CandidatePatchMismatch,
                                    )?;
                                edge.target = patch.target;
                                edge.bindings = composed.clone();
                                edge.provenance.extend_from_slice(&outgoing_edge.provenance);
                                edge.work.extend_from_slice(&outgoing_edge.work);
                                node.definitions = expected_definitions(
                                    &node.operation,
                                    location.block,
                                    location.node,
                                );
                                node.uses =
                                    expected_uses(&node.operation, location.block, location.node);
                                node.ownership = expected_ownership(&node.operation);
                            }
                            output_function
                                .blocks
                                .retain(|block| block.id != patch.empty.block);
                            sweep_output_nodes(output_function, |_| Ok(()))?;
                            Ok(())
                        },
                    )
                }
            }
            pub(crate) fn validate_path_qualified_empty_block_candidate(
                input: &PsiOptimizationUnit,
                candidate: &PsiRewriteCandidate,
            ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
                super::super::super::contract::admit(
                    input,
                    candidate,
                    &super::super::super::contract::PATH_QUALIFIED_EMPTY_BLOCK_THREAD,
                )?;
                if !candidate.substitutions().is_empty() {
                    return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
                }
                replay::validate(input, candidate)
            }
        }
        pub(crate) use linear::validate_linear_empty_block_candidate;
        pub(crate) use path_qualified::validate_path_qualified_empty_block_candidate;
    }
    mod shared_jump_fusion {
        use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
        use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

        mod replay {
            use crate::candidates::accept_region_rewrite;
            use crate::candidates::candidate_block;
            use crate::candidates::candidate_function;
            use crate::candidates::control_flow_cleanup::{
                checked_scalar_substitutions, patched_jump_bindings, require_scalar_bindings,
            };
            use crate::candidates::rewrite_accounting::adjacent_merge::reconstruct_adjacent_merge_ownership_is_identity;
            use crate::candidates::rewrite_accounting::rewrite_scalar_substitutions;
            use crate::candidates::rewrite_accounting::terminal_fusion::reconstruct_shared_terminal_fusion_accounting;
            use crate::unit_validation::derived_metadata::{
                expected_definitions, expected_edges, expected_ownership, expected_uses,
            };
            use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
            use optimization_unit::{
                PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance, PsiRewriteCandidate,
                PsiRewritePatch,
            };
            use semantic_vocabulary::BlockId;
            use std::collections::{BTreeMap, BTreeSet};

            pub(super) fn validate(
                input: &PsiOptimizationUnit,
                candidate: &PsiRewriteCandidate,
            ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
                candidate_patch!(patch, FuseSharedTerminalJump, candidate);
                if candidate.node_decision_point() != Some(patch.predecessor) {
                    return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                }
                let function = candidate_function(input, patch.predecessor.machine)?;
                require_scalar_bindings(function)?;

                let predecessor = candidate_block(function, patch.predecessor.block)?;
                let predecessor_index = usize::try_from(patch.predecessor.node)
                    .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
                if predecessor_index + 1 != predecessor.nodes.len() {
                    return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                }
                let predecessor_node =
                    crate::candidates::candidate_node(predecessor, patch.predecessor.node)?;
                let bindings =
                    patched_jump_bindings(predecessor_node, patch.incoming_edge, patch.target)?;
                let target = candidate_block(function, patch.target)?;
                let [terminal] = target.nodes.as_slice() else {
                    return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                };
                if target.id == function.entry
                    || predecessor.id == target.id
                    || !terminal.successors.is_empty()
                    || !matches!(terminal.provenance.first(), Some(PsiProvenance::Edge(_)))
                    || !matches!(
                        terminal.operation,
                        abstract_operations::return_or_return_unit_pattern!()
                            | abstract_operations::return_structural_or_crash_pattern!()
                    )
                {
                    return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                }
                let incoming = function
                    .block_edges()
                    .filter(|edge| edge.target == patch.target)
                    .collect::<Vec<_>>();
                if incoming.len() < 2
                    || incoming
                        .iter()
                        .filter(|edge| edge.psi_edge == patch.incoming_edge)
                        .count()
                        != 1
                {
                    return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
                }
                if !independently_post_dominates(function, patch.predecessor.block, patch.target) {
                    return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
                }
                let substitutions = checked_scalar_substitutions(
                    candidate,
                    &target.parameters,
                    bindings,
                    |_, _| true,
                )?;
                if !reconstruct_adjacent_merge_ownership_is_identity(
                    input,
                    function,
                    patch.incoming_edge,
                    patch.target,
                ) {
                    return Err(OptimizationUnitValidationError::CandidateObservationMismatch);
                }
                let (expected_blocks, accepted_provenance) =
                    reconstruct_shared_terminal_fusion_accounting(function, patch)
                        .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;

                let incoming_edge = predecessor_node
                    .edge(patch.incoming_edge)
                    .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?
                    .clone();
                let removed_effect = predecessor_node.effect;
                let mut clone = terminal.clone();
                rewrite_scalar_substitutions(
                    &mut clone.operation,
                    &substitutions,
                    patch.predecessor.machine,
                    patch.target,
                );
                clone
                    .provenance
                    .extend_from_slice(&incoming_edge.provenance);
                clone.work.extend_from_slice(&incoming_edge.work);
                clone.effect = removed_effect;
                clone.definitions = expected_definitions(
                    &clone.operation,
                    patch.predecessor.block,
                    patch.predecessor.node,
                );
                clone.uses = expected_uses(
                    &clone.operation,
                    patch.predecessor.block,
                    patch.predecessor.node,
                );
                clone.successors = expected_edges(&clone.operation);
                clone.ownership = expected_ownership(&clone.operation);

                accept_region_rewrite(
                    input,
                    candidate,
                    patch.predecessor.machine,
                    &expected_blocks,
                    b"omega.validator.shared-terminal-jump-fusion.v2",
                    accepted_provenance,
                    |output| {
                        let output_function = output
                            .function_mut(patch.predecessor.machine)
                            .expect("candidate function exists");
                        let output_predecessor = output_function
                            .block_mut(patch.predecessor.block)
                            .expect("candidate predecessor exists");
                        output_predecessor.nodes[predecessor_index] = clone;
                        Ok(())
                    },
                )
            }

            fn independently_post_dominates(
                function: &PsiOptimizationFunction,
                block: BlockId,
                candidate: BlockId,
            ) -> bool {
                let successors = function
                    .blocks
                    .iter()
                    .map(|block| {
                        let mut targets = block
                            .nodes
                            .last()
                            .into_iter()
                            .flat_map(|node| expected_edges(&node.operation))
                            .map(|edge| edge.target)
                            .collect::<Vec<_>>();
                        targets.sort_unstable();
                        targets.dedup();
                        (block.id, targets)
                    })
                    .collect::<BTreeMap<_, _>>();
                let all = successors.keys().copied().collect::<BTreeSet<_>>();
                let exits = successors
                    .iter()
                    .filter_map(|(block, targets)| targets.is_empty().then_some(*block))
                    .collect::<BTreeSet<_>>();
                if exits.is_empty() {
                    return false;
                }
                let mut post_dominators = all
                    .iter()
                    .copied()
                    .map(|block| {
                        let initial = if exits.contains(&block) {
                            BTreeSet::from([block])
                        } else {
                            all.clone()
                        };
                        (block, initial)
                    })
                    .collect::<BTreeMap<_, _>>();
                loop {
                    let mut changed = false;
                    for current in all
                        .iter()
                        .copied()
                        .filter(|current| !exits.contains(current))
                    {
                        let mut adjacent = successors[&current]
                            .iter()
                            .filter_map(|successor| post_dominators.get(successor));
                        let Some(mut next) = adjacent.next().cloned() else {
                            return false;
                        };
                        for set in adjacent {
                            next = next.intersection(set).copied().collect();
                        }
                        next.insert(current);
                        if post_dominators[&current] != next {
                            post_dominators.insert(current, next);
                            changed = true;
                        }
                    }
                    if !changed {
                        return post_dominators
                            .get(&block)
                            .is_some_and(|blocks| blocks.contains(&candidate));
                    }
                }
            }
        }
        pub(crate) fn validate_shared_jump_fusion_candidate(
            input: &PsiOptimizationUnit,
            candidate: &PsiRewriteCandidate,
        ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
            super::super::contract::admit(
                input,
                candidate,
                &super::super::contract::SHARED_TERMINAL_JUMP_FUSION,
            )?;
            replay::validate(input, candidate)
        }
    }
    mod unreachable_private_machines {
        use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
        use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

        mod replay {
            use crate::candidates::{canonicalize_provenance, proven_unreachable_at_own_site};
            use crate::{
                OptimizationUnitValidationError, ValidatedPsiRewrite,
                validate_psi_optimization_unit,
            };
            use optimization_core::OptimizationValidatorIdentity;
            use optimization_unit::{
                NodeLocation, PsiOptimizationFunction, PsiOptimizationUnit, PsiRealizationSite,
                PsiRewriteCandidate, PsiRewritePatch, recompute_psi_optimization_unit_identity,
            };
            use semantic_vocabulary::MachineId;
            use std::collections::{BTreeMap, BTreeSet};

            pub(super) fn validate(
                input: &PsiOptimizationUnit,
                candidate: &PsiRewriteCandidate,
            ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
                let optimization_unit::PsiRewriteDecisionPoint::MachineSet(decision_machines) =
                    candidate.decision_point()
                else {
                    return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                };
                candidate_patch!(patch, PruneUnreachablePrivateMachines, candidate);
                let expected_machines = unreachable_private_machine_complement(input);
                if candidate.predicted_cost_delta()
                    != -i64::try_from(expected_machines.len()).unwrap_or(i64::MAX)
                {
                    return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
                }
                let patch_machines = patch
                    .machines
                    .iter()
                    .map(|row| row.machine)
                    .collect::<Vec<_>>();
                if expected_machines.is_empty()
                    || *decision_machines != expected_machines
                    || patch_machines != expected_machines
                    || candidate.affected_machines() != expected_machines
                {
                    return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
                }
                let source_ordinals = validator_active_source_ordinals(input);
                let expected_custody = expected_machines
                    .iter()
                    .map(|machine| optimization_unit::PrunedMachineCustody {
                        machine: *machine,
                        source_ordinal: source_ordinals[machine],
                    })
                    .collect::<Vec<_>>();
                if patch.machines != expected_custody {
                    return Err(OptimizationUnitValidationError::CandidateReachabilityMismatch);
                }
                let expected_provenance = pruned_machine_provenance(input, &expected_machines)
                    .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
                if candidate.provenance() != expected_provenance {
                    return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
                }
                let removed = expected_machines.iter().copied().collect::<BTreeSet<_>>();
                let mut output = input.clone();
                output
                    .functions
                    .retain(|function| !removed.contains(&function.machine));
                output.pruned_machines.extend(expected_custody);
                output.pruned_machines.sort_unstable();
                output.identity = recompute_psi_optimization_unit_identity(&output);
                validate_psi_optimization_unit(&output)?;
                Ok(ValidatedPsiRewrite {
                    unit: output,
                    candidate: candidate.identity(),
                    validator: OptimizationValidatorIdentity::from_canonical_bytes(
                        b"omega.validator.unreachable-private-machine-pruning.v1",
                    ),
                    provenance: expected_provenance,
                })
            }

            pub(crate) fn validator_active_source_ordinals(
                unit: &PsiOptimizationUnit,
            ) -> BTreeMap<MachineId, u32> {
                let pruned = unit
                    .pruned_machines
                    .iter()
                    .map(|row| (row.source_ordinal, row.machine))
                    .collect::<BTreeMap<_, _>>();
                let mut active = unit.functions.iter();
                let mut result = BTreeMap::new();
                for ordinal in 0..(unit.functions.len() + unit.pruned_machines.len()) {
                    let ordinal = u32::try_from(ordinal).expect("function ordinal fits u32");
                    if !pruned.contains_key(&ordinal)
                        && let Some(function) = active.next()
                    {
                        result.insert(function.machine, ordinal);
                    }
                }
                result
            }

            pub(crate) fn unreachable_private_machine_complement(
                unit: &PsiOptimizationUnit,
            ) -> Vec<MachineId> {
                let active = unit
                    .functions
                    .iter()
                    .map(|function| function.machine)
                    .collect::<BTreeSet<_>>();
                let mut reachable = BTreeSet::from([unit.entry]);
                reachable.extend(
                    unit.dynamic_dispatch
                        .tables
                        .iter()
                        .flat_map(|table| table.rows.iter().copied().flatten()),
                );
                reachable.extend(
                    unit.provider_candidates
                        .iter()
                        .map(|candidate| candidate.candidate),
                );
                reachable.extend(
                    unit.functions
                        .iter()
                        .filter(|function| function.attachment.is_some())
                        .map(|function| function.machine),
                );
                let references = unit
                    .functions
                    .iter()
                    .map(|function| {
                        (
                            function.machine,
                            validator_machine_references(unit, function),
                        )
                    })
                    .collect::<BTreeMap<_, _>>();
                let mut work = reachable.iter().copied().collect::<Vec<_>>();
                while let Some(machine) = work.pop() {
                    for callee in references.get(&machine).into_iter().flatten().copied() {
                        if active.contains(&callee) && reachable.insert(callee) {
                            work.push(callee);
                        }
                    }
                }
                active.difference(&reachable).copied().collect()
            }

            pub(crate) fn validator_machine_references(
                unit: &PsiOptimizationUnit,
                function: &PsiOptimizationFunction,
            ) -> BTreeSet<MachineId> {
                let mut references = BTreeSet::new();
                for operation in function.block_nodes().map(|node| &node.operation) {
                    references.extend(
                        operation
                            .cleanup_rosters()
                            .flat_map(|(_, actions)| actions)
                            .filter_map(|action| match action {
                                terminal_psi::TerminalAffineCleanupAction::InvokeNominal(
                                    cleanup,
                                ) => Some(cleanup.cleanup_machine),
                                _ => None,
                            }),
                    );
                    references.extend(unit.operation_machine_references(operation));
                }
                references
            }

            pub(crate) fn pruned_machine_provenance(
                unit: &PsiOptimizationUnit,
                machines: &[MachineId],
            ) -> Option<Vec<optimization_unit::ProvenanceRewrite>> {
                let machines = machines.iter().copied().collect::<BTreeSet<_>>();
                let mut rows = Vec::new();
                for function in unit
                    .functions
                    .iter()
                    .filter(|function| machines.contains(&function.machine))
                {
                    for block in &function.blocks {
                        for (node_index, node) in block.nodes.iter().enumerate() {
                            let input = PsiRealizationSite::Node(NodeLocation {
                                machine: function.machine,
                                block: block.id,
                                node: u32::try_from(node_index).ok()?,
                            });
                            if !node.provenance.is_empty() {
                                rows.push(proven_unreachable_at_own_site(
                                    input,
                                    &node.provenance,
                                    &node.work,
                                ));
                            }
                            for edge in &node.successors {
                                let input = PsiRealizationSite::Edge {
                                    machine: function.machine,
                                    edge: edge.psi_edge,
                                };
                                if !edge.provenance.is_empty() {
                                    rows.push(proven_unreachable_at_own_site(
                                        input,
                                        &edge.provenance,
                                        &edge.work,
                                    ));
                                }
                            }
                        }
                    }
                }
                canonicalize_provenance(&mut rows);
                Some(rows)
            }
        }
        pub(crate) fn validate_unreachable_private_machines_candidate(
            input: &PsiOptimizationUnit,
            candidate: &PsiRewriteCandidate,
        ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
            super::super::contract::admit(
                input,
                candidate,
                &super::super::contract::UNREACHABLE_PRIVATE_MACHINE_PRUNING,
            )?;
            if !candidate.affected_blocks().is_empty() || !candidate.substitutions().is_empty() {
                return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
            }
            replay::validate(input, candidate)
        }
    }
    pub(crate) use block_merging::{
        validate_adjacent_block_merge_candidate, validate_non_adjacent_block_merge_candidate,
    };
    pub(crate) use constant_conditionals::validate_constant_conditional_candidate;
    pub(crate) use empty_block_threading::{
        validate_linear_empty_block_candidate, validate_path_qualified_empty_block_candidate,
    };
    pub(crate) use shared_jump_fusion::validate_shared_jump_fusion_candidate;
    pub(crate) use unreachable_private_machines::validate_unreachable_private_machines_candidate;

    pub(super) fn patched_jump_bindings(
        node: &OptimizationNode,
        edge: EdgeId,
        target: BlockId,
    ) -> Result<&[ValueBinding], OptimizationUnitValidationError> {
        let O::Jump {
            psi_edge,
            target: jump_target,
            bindings,
            cleanup_actions,
            ..
        } = &node.operation
        else {
            return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
        };
        if !cleanup_actions.is_empty() || *psi_edge != edge || *jump_target != target {
            return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
        }
        Ok(bindings)
    }

    pub(super) fn require_scalar_bindings(
        function: &PsiOptimizationFunction,
    ) -> Result<(), OptimizationUnitValidationError> {
        if function
            .blocks
            .iter()
            .any(|block| !block.structural_parameters.is_empty())
        {
            return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
        }
        Ok(())
    }

    pub(super) fn checked_scalar_substitutions<'bindings>(
        candidate: &PsiRewriteCandidate,
        parameters: &[ValueDefinition],
        bindings: &'bindings [ValueBinding],
        admits: impl Fn(&ValueDefinition, &'bindings ValueBinding) -> bool,
    ) -> Result<Vec<ScalarSubstitution>, OptimizationUnitValidationError> {
        if parameters.len() != bindings.len() {
            return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
        }
        let mut substitutions = parameters
            .iter()
            .zip(bindings)
            .map(|(parameter, binding)| {
                (binding.parameter == parameter.value
                    && binding.scalar_type == parameter.scalar_type
                    && admits(parameter, binding))
                .then_some(ScalarSubstitution {
                    from: parameter.value,
                    to: binding.argument,
                    scalar_type: parameter.scalar_type,
                })
            })
            .collect::<Option<Vec<_>>>()
            .ok_or(OptimizationUnitValidationError::CandidateIncomingBindingMismatch)?;
        substitutions.sort();
        if candidate.substitutions() != substitutions {
            return Err(OptimizationUnitValidationError::CandidateSubstitutionMismatch);
        }
        Ok(substitutions)
    }
}
pub(crate) mod copy_propagation {
    use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
    use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

    mod observation {
        use crate::OptimizationUnitValidationError;
        use crate::unit_validation::derived_metadata::{
            expected_definitions, expected_edges, expected_ownership, expected_uses,
        };
        use optimization_unit::{
            PsiOptimizationUnit, RedundantBlockParameterRewrite, ValueDefinitionSite,
            recompute_psi_optimization_unit_identity,
        };
        use semantic_vocabulary::{BlockId, MachineId, ValueId};
        use std::collections::BTreeSet;

        pub(crate) fn normalize_redundant_parameter_observation_input(
            input: &PsiOptimizationUnit,
            patch: RedundantBlockParameterRewrite,
            affected_blocks: &[BlockId],
        ) -> Result<PsiOptimizationUnit, OptimizationUnitValidationError> {
            let affected = affected_blocks.iter().copied().collect::<BTreeSet<_>>();
            let mut normalized = input.clone();
            let function = normalized
                .function_mut(patch.machine)
                .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
            let target = function
                .block_mut(patch.block)
                .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
            let position = usize::try_from(patch.position).expect("u32 fits usize");
            let removed = target
                .parameters
                .get(position)
                .copied()
                .ok_or(OptimizationUnitValidationError::CandidateBlockParameterMismatch)?;
            if removed.value != patch.parameter
                || removed.scalar_type != patch.scalar_type
                || removed.site
                    != (ValueDefinitionSite::BlockParameter {
                        block: patch.block,
                        position: patch.position,
                    })
            {
                return Err(OptimizationUnitValidationError::CandidateBlockParameterMismatch);
            }
            target.parameters.remove(position);
            for (new_position, parameter) in target.parameters.iter_mut().enumerate().skip(position)
            {
                parameter.site = ValueDefinitionSite::BlockParameter {
                    block: patch.block,
                    position: u32::try_from(new_position).expect("parameter index fits u32"),
                };
            }

            for block in function
                .blocks
                .iter_mut()
                .filter(|block| affected.contains(&block.id))
            {
                for (node_index, node) in block.nodes.iter_mut().enumerate() {
                    node.operation = normalize_redundant_parameter_observation_operation(
                        &node.operation,
                        patch,
                    )?;
                    let node_index = u32::try_from(node_index).expect("unit node index fits u32");
                    node.definitions = expected_definitions(&node.operation, block.id, node_index);
                    node.uses = expected_uses(&node.operation, block.id, node_index);
                    node.successors = expected_edges(&node.operation);
                    node.ownership = expected_ownership(&node.operation);
                }
            }
            normalized.identity = recompute_psi_optimization_unit_identity(&normalized);
            Ok(normalized)
        }

        pub(crate) fn normalize_redundant_parameter_observation_operation(
            operation: &abstract_operations::AbstractOperation,
            patch: RedundantBlockParameterRewrite,
        ) -> Result<abstract_operations::AbstractOperation, OptimizationUnitValidationError>
        {
            use abstract_operations::AbstractOperation as O;

            let mut normalized = operation.clone();
            let mut replace = |value: &mut ValueId| {
                if *value == patch.parameter {
                    *value = patch.replacement;
                }
            };
            let normalize_bindings = |target: BlockId,
                                      bindings: &mut Vec<abstract_operations::ValueBinding>|
             -> Result<(), OptimizationUnitValidationError> {
                for binding in bindings.iter_mut() {
                    replace(&mut binding.argument);
                }
                if target == patch.block {
                    let position = usize::try_from(patch.position).expect("u32 fits usize");
                    let binding = bindings
                        .get(position)
                        .ok_or(OptimizationUnitValidationError::CandidateIncomingBindingMismatch)?;
                    if binding.parameter != patch.parameter
                        || binding.argument != patch.replacement
                        || binding.scalar_type != patch.scalar_type
                    {
                        return Err(
                            OptimizationUnitValidationError::CandidateIncomingBindingMismatch,
                        );
                    }
                    bindings.remove(position);
                }
                Ok(())
            };

            match &mut normalized {
                O::Jump {
                    target, bindings, ..
                } => normalize_bindings(*target, bindings)?,
                O::Conditional {
                    condition,
                    when_true,
                    when_false,
                } => {
                    replace(condition);
                    normalize_bindings(when_true.target, &mut when_true.bindings)?;
                    normalize_bindings(when_false.target, &mut when_false.bindings)?;
                }
                O::StructuralCase { cases, .. } => {
                    for successor in cases {
                        if successor.target == patch.block {
                            let position = usize::try_from(patch.position).expect("u32 fits usize");
                            let payload = successor.payloads.get(position).ok_or(
                                OptimizationUnitValidationError::CandidateIncomingBindingMismatch,
                            )?;
                            if payload.parameter != patch.parameter
                                || payload.scalar_type != patch.scalar_type
                            {
                                return Err(
                            OptimizationUnitValidationError::CandidateIncomingBindingMismatch,
                        );
                            }
                            successor.payloads.remove(position);
                        }
                    }
                }
                _ => crate::candidates::value_operand_rewrites::rewrite_value_operands(
                    &mut normalized,
                    &mut replace,
                ),
            }
            Ok(normalized)
        }

        pub(crate) fn unchanged_outside_redundant_parameter_region(
            input: &PsiOptimizationUnit,
            output: &PsiOptimizationUnit,
            machine: MachineId,
            affected_blocks: &[BlockId],
        ) -> bool {
            let mut expected = input.clone();
            let Some(expected_function) = expected.function_mut(machine) else {
                return false;
            };
            let Some(output_function) = output.function(machine) else {
                return false;
            };
            for block_id in affected_blocks {
                let Some(expected_block) = expected_function
                    .blocks
                    .iter_mut()
                    .find(|block| block.id == *block_id)
                else {
                    return false;
                };
                let Some(output_block) = output_function.block(*block_id) else {
                    return false;
                };
                *expected_block = output_block.clone();
            }
            expected.identity = output.identity;
            expected == *output
        }
    }
    mod operation_rewrite {
        use optimization_unit::RedundantBlockParameterRewrite;
        use semantic_vocabulary::ValueId;

        pub(crate) fn rewrite_block_parameter_operation(
            operation: &mut abstract_operations::AbstractOperation,
            patch: RedundantBlockParameterRewrite,
        ) {
            use abstract_operations::AbstractOperation as O;

            let mut replace = |value: &mut ValueId| {
                if *value == patch.parameter {
                    *value = patch.replacement;
                }
            };
            let rewrite_bindings = |bindings: &mut Vec<abstract_operations::ValueBinding>| {
                for binding in bindings.iter_mut() {
                    if binding.argument == patch.parameter {
                        binding.argument = patch.replacement;
                    }
                }
            };
            match operation {
                O::Jump {
                    target, bindings, ..
                } => {
                    rewrite_bindings(bindings);
                    if *target == patch.block {
                        bindings.remove(usize::try_from(patch.position).expect("u32 fits usize"));
                    }
                }
                O::Conditional {
                    condition,
                    when_true,
                    when_false,
                } => {
                    replace(condition);
                    for successor in [when_true, when_false] {
                        rewrite_bindings(&mut successor.bindings);
                        if successor.target == patch.block {
                            successor
                                .bindings
                                .remove(usize::try_from(patch.position).expect("u32 fits usize"));
                        }
                    }
                }
                O::StructuralCase { cases, .. } => {
                    for successor in cases {
                        if successor.target == patch.block {
                            successor
                                .payloads
                                .remove(usize::try_from(patch.position).expect("u32 fits usize"));
                        }
                    }
                }
                O::WriteOnlyPrimitiveStore {
                    range_obligation: Some(_),
                    ..
                } => {}
                _ => crate::candidates::value_operand_rewrites::rewrite_value_operands(
                    operation,
                    &mut replace,
                ),
            }
        }
    }
    mod validation {
        use crate::candidates::{
            canonicalize_provenance, realize_node_provenance, realized_at_own_site,
        };
        use crate::unit_validation::derived_metadata::{
            expected_definitions, expected_edges, expected_ownership, expected_uses,
        };
        use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
        use optimization_core::OptimizationValidatorIdentity;
        use optimization_unit::{
            BlockParameterIncomingBinding, PsiOptimizationUnit, PsiRealizationSite,
            PsiRewriteCandidate, PsiRewritePatch, ValueDefinitionSite,
            reconstruct_psi_closed_region_observation,
        };
        use std::collections::BTreeSet;

        use super::observation::{
            normalize_redundant_parameter_observation_input,
            unchanged_outside_redundant_parameter_region,
        };
        use super::operation_rewrite::rewrite_block_parameter_operation;
        use crate::candidates::{candidate_block, candidate_function};

        pub(super) fn validate_redundant_block_parameter_candidate(
            input: &PsiOptimizationUnit,
            candidate: &PsiRewriteCandidate,
        ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
            candidate_patch!(patch, RemoveRedundantBlockParameter, candidate);
            let witness = candidate
                .redundant_block_parameter_witness()
                .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
            let function = candidate_function(input, patch.machine)?;
            if function.entry == patch.block || patch.parameter == patch.replacement {
                return Err(OptimizationUnitValidationError::CandidateBlockParameterMismatch);
            }
            let block = candidate_block(function, patch.block)?;
            let position = usize::try_from(patch.position).expect("u32 fits usize");
            let Some(parameter) = block.parameters.get(position) else {
                return Err(OptimizationUnitValidationError::CandidateBlockParameterMismatch);
            };
            if parameter.value != patch.parameter
                || parameter.scalar_type != patch.scalar_type
                || parameter.site
                    != (ValueDefinitionSite::BlockParameter {
                        block: patch.block,
                        position: patch.position,
                    })
            {
                return Err(OptimizationUnitValidationError::CandidateBlockParameterMismatch);
            }
            let replacement_type = function
                .parameters
                .iter()
                .chain(
                    function
                        .blocks
                        .iter()
                        .flat_map(|block| block.parameters.iter().chain(block.definitions())),
                )
                .find(|definition| definition.value == patch.replacement)
                .map(|definition| definition.scalar_type);
            if replacement_type != Some(patch.scalar_type) {
                return Err(OptimizationUnitValidationError::CandidateSubstitutionMismatch);
            }

            let mut incoming = Vec::new();
            let mut expected_provenance = Vec::new();
            let mut affected_blocks = BTreeSet::from([patch.block]);
            for source in &function.blocks {
                for (node_index, node) in source.nodes.iter().enumerate() {
                    let changes_use = node
                        .uses
                        .iter()
                        .any(|use_site| use_site.value == patch.parameter);
                    for edge in &node.successors {
                        if edge.target != patch.block {
                            continue;
                        }
                        let Some(binding) = edge.bindings.get(position) else {
                            return Err(
                                OptimizationUnitValidationError::CandidateIncomingBindingMismatch,
                            );
                        };
                        incoming.push(BlockParameterIncomingBinding {
                            source: source.id,
                            edge: edge.psi_edge,
                            argument: binding.argument,
                        });
                        let site = PsiRealizationSite::Edge {
                            machine: patch.machine,
                            edge: edge.psi_edge,
                        };
                        expected_provenance.push(realized_at_own_site(
                            site,
                            &edge.provenance,
                            &edge.work,
                        ));
                    }
                    if changes_use {
                        affected_blocks.insert(source.id);
                        realize_node_provenance(
                            patch.machine,
                            source.id,
                            std::iter::once((node_index, node)),
                            &mut expected_provenance,
                        )
                        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
                    }
                    if node
                        .successors
                        .iter()
                        .any(|edge| edge.target == patch.block)
                    {
                        affected_blocks.insert(source.id);
                    }
                }
            }
            incoming.sort_by_key(|row| (row.edge, row.source));
            canonicalize_provenance(&mut expected_provenance);
            if incoming != witness.incoming
                || incoming
                    .iter()
                    .any(|binding| binding.argument != patch.replacement)
            {
                return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
            }
            if candidate.substitutions()
                != [optimization_unit::ScalarSubstitution {
                    from: patch.parameter,
                    to: patch.replacement,
                    scalar_type: patch.scalar_type,
                }]
            {
                return Err(OptimizationUnitValidationError::CandidateSubstitutionMismatch);
            }
            if candidate.affected_blocks() != affected_blocks.into_iter().collect::<Vec<_>>()
                || candidate.provenance() != expected_provenance
            {
                return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
            }

            let normalized_input = normalize_redundant_parameter_observation_input(
                input,
                patch,
                candidate.affected_blocks(),
            )?;
            let input_region = reconstruct_psi_closed_region_observation(
                &normalized_input,
                patch.machine,
                candidate.affected_blocks(),
            )
            .ok_or(OptimizationUnitValidationError::CandidateRegionObservationUnavailable)?;

            let mut output = input.clone();
            let function = output
                .function_mut(patch.machine)
                .expect("candidate function exists");
            let block = function
                .block_mut(patch.block)
                .expect("candidate block exists");
            block.parameters.remove(position);
            for (new_position, parameter) in block.parameters.iter_mut().enumerate().skip(position)
            {
                parameter.site = ValueDefinitionSite::BlockParameter {
                    block: patch.block,
                    position: u32::try_from(new_position).expect("parameter index fits u32"),
                };
            }
            for block in &mut function.blocks {
                for (node_index, node) in block.nodes.iter_mut().enumerate() {
                    rewrite_block_parameter_operation(&mut node.operation, patch);
                    let node_index = u32::try_from(node_index).expect("unit node index fits u32");
                    node.definitions = expected_definitions(&node.operation, block.id, node_index);
                    node.uses = expected_uses(&node.operation, block.id, node_index);
                    node.successors = expected_edges(&node.operation);
                    node.ownership = expected_ownership(&node.operation);
                }
            }
            crate::unit_validation::revalidate_rewrite_output(&mut output, patch.machine)?;
            if !unchanged_outside_redundant_parameter_region(
                input,
                &output,
                patch.machine,
                candidate.affected_blocks(),
            ) {
                return Err(OptimizationUnitValidationError::CandidateOutsideRegionMismatch);
            }
            let output_region = reconstruct_psi_closed_region_observation(
                &output,
                patch.machine,
                candidate.affected_blocks(),
            )
            .ok_or(OptimizationUnitValidationError::CandidateRegionObservationUnavailable)?;
            if input_region.semantics != output_region.semantics {
                return Err(OptimizationUnitValidationError::CandidateRegionObservationMismatch);
            }
            Ok(ValidatedPsiRewrite {
                unit: output,
                candidate: candidate.identity(),
                validator: OptimizationValidatorIdentity::from_canonical_bytes(
                    b"omega.validator.redundant-block-parameter.v2",
                ),
                provenance: expected_provenance,
            })
        }
    }
    pub(crate) use operation_rewrite::rewrite_block_parameter_operation;

    pub(crate) fn validate_redundant_block_parameter_candidate(
        input: &PsiOptimizationUnit,
        candidate: &PsiRewriteCandidate,
    ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
        super::contract::admit(
            input,
            candidate,
            &super::contract::REDUNDANT_BLOCK_PARAMETER,
        )?;
        validation::validate_redundant_block_parameter_candidate(input, candidate)
    }
}
pub(crate) mod dead_scalar_elimination {
    use super::contract;
    use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
    use optimization_core::AnalysisKind;
    use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

    mod operation_partition {
        use abstract_operations::AbstractOperation as O;
        use optimization_core::OptimizationRuleIdentity;
        use semantic_vocabulary::{ScalarType, ValueId};

        use super::rule_catalog::{DeadScalarFamily, dead_scalar_family};
        use abstract_operations::bitwise_or_xor_pair_pattern;
        use abstract_operations::boolean_not_or_equal_pair_pattern;
        use abstract_operations::integer_equal_less_pair_pattern;
        use semantic_vocabulary::ObligationId;

        fn independently_validated_dead_scalar_operation_family(
            operation: &O,
        ) -> Option<DeadScalarFamily> {
            match operation {
                O::EstablishNativeCodeLayout { .. }
                | O::EstablishActivationClaim { .. }
                | O::SplitActivationClaim { .. }
                | O::MergeActivationClaim { .. }
                | O::ReleaseActivationClaim { .. } => None,
                O::IntegerConstant { .. } | O::BooleanConstant { .. } => {
                    Some(DeadScalarFamily::Literal)
                }
                boolean_not_or_equal_pair_pattern!(..)
                | integer_equal_less_pair_pattern!(..)
                | abstract_operations::bitwise_not_or_less_pair_pattern!(..)
                | O::IntegerWiden { .. }
                | O::IntegerBitwiseAnd { .. }
                | bitwise_or_xor_pair_pattern!(..) => Some(DeadScalarFamily::UnconditionallyTotal),
                O::IntegerExactCast { .. } => Some(DeadScalarFamily::ProofCertified),
                O::IntegerShift { policy, .. } => Some(obligation_family(policy.obligation())),
                O::IntegerArithmetic { arithmetic, .. } => {
                    Some(obligation_family(arithmetic.obligation()))
                }
                O::StorageAddress { .. }
                | O::SelectImplementation { .. }
                | O::SelectMachine { .. }
                | O::EstablishPrimitiveLocal { .. }
                | O::PrimitiveLocalStore { .. }
                | abstract_operations::primitive_storage_observation_pattern!(..)
                | O::WriteOnlyPrimitiveStore { .. }
                | O::StructuralScalarFieldStore { .. }
                | O::AtomicEvent { .. }
                | O::EstablishScalarArray { .. }
                | abstract_operations::structural_array_or_case_pattern!(..)
                | abstract_operations::establish_byte_literal_triple_pattern!(..)
                | abstract_operations::structural_establish_triple_pattern!(..)
                | abstract_operations::boundary_or_structural_call_pair_pattern!(..)
                | O::ActivateTaskRegister { .. }
                | O::ReadTaskRegister { .. }
                | O::ReloadSegmentDescriptors { .. }
                | O::ReadSegmentSelector { .. }
                | O::GlobalDescriptorTable { .. }
                | O::PortWrite { .. }
                | O::RegisterAccess { .. }
                | O::Call { .. }
                | O::IeeeFloatConstant { .. }
                | O::NearestIeeeFloatFusedMultiplyAdd { .. }
                | O::IeeeFloatCompare { .. }
                | O::ScalarRepresentationView { .. }
                | O::FloatResultPolicy { .. }
                | abstract_operations::byte_read_predicate_equal_pattern!(..)
                | O::ByteSequenceWrite { .. }
                | abstract_operations::byte_sequence_store_pair_pattern!(..)
                | abstract_operations::byte_view_triple_pattern!(..)
                | abstract_operations::byte_view_length_triple_pattern!(..)
                | abstract_operations::element_view_read_write_pair_pattern!(..)
                | O::ElementViewSubslice { .. }
                | abstract_operations::byte_len_or_field_read_pattern!(..)
                | O::StructuralScalarFieldRead { .. }
                | O::EstablishReference { .. }
                | O::EstablishAccessReference { .. }
                | O::ReleaseReference { .. }
                | abstract_operations::move_or_store_structural_field_pattern!()
                | O::StructuralLeafCopy { .. }
                | O::MoveStructuralValue { .. }
                | O::TransformContent { .. }
                | O::TrappingInteger { .. }
                | abstract_operations::jump_or_conditional_pattern!()
                | O::StructuralCase { .. }
                | abstract_operations::return_or_return_unit_pattern!()
                | abstract_operations::return_structural_or_crash_pattern!() => None,
                O::ExternalCompletion { .. } | O::InvokeNoNormal { .. } => None,
            }
        }

        fn obligation_family(obligation: Option<ObligationId>) -> DeadScalarFamily {
            match obligation {
                None => DeadScalarFamily::UnconditionallyTotal,
                Some(_) => DeadScalarFamily::ProofCertified,
            }
        }

        pub(crate) fn independently_validated_dead_scalar_shape(
            rule: OptimizationRuleIdentity,
            operation: &O,
        ) -> Option<(
            semantic_vocabulary::OperationId,
            ValueId,
            ScalarType,
            Option<ObligationId>,
        )> {
            let rule_family = dead_scalar_family(rule)?;
            if independently_validated_dead_scalar_operation_family(operation) != Some(rule_family)
            {
                return None;
            }
            match (rule_family, operation) {
                (
                    DeadScalarFamily::Literal,
                    O::IntegerConstant {
                        psi_operation,
                        result,
                        scalar_type,
                        ..
                    },
                ) => Some((*psi_operation, *result, *scalar_type, None)),
                (
                    DeadScalarFamily::Literal,
                    O::BooleanConstant {
                        psi_operation,
                        result,
                        ..
                    },
                ) => Some((*psi_operation, *result, ScalarType::Boolean, None)),
                (
                    DeadScalarFamily::UnconditionallyTotal,
                    boolean_not_or_equal_pair_pattern!(psi_operation, result, ..)
                    | integer_equal_less_pair_pattern!(psi_operation, result, ..)
                    | O::IntegerLessOrEqual {
                        psi_operation,
                        result,
                        ..
                    },
                ) => Some((*psi_operation, *result, ScalarType::Boolean, None)),
                (
                    DeadScalarFamily::UnconditionallyTotal,
                    abstract_operations::bitwise_not_and_pair_pattern!(
                        psi_operation,
                        result,
                        scalar_type,
                        ..
                    )
                    | bitwise_or_xor_pair_pattern!(psi_operation, result, scalar_type, ..),
                ) => Some((
                    *psi_operation,
                    *result,
                    ScalarType::Integer(*scalar_type),
                    None,
                )),
                (
                    DeadScalarFamily::UnconditionallyTotal,
                    O::IntegerWiden {
                        psi_operation,
                        result,
                        target_type,
                        ..
                    },
                ) => Some((
                    *psi_operation,
                    *result,
                    ScalarType::Integer(*target_type),
                    None,
                )),
                (
                    DeadScalarFamily::ProofCertified,
                    O::IntegerExactCast {
                        psi_operation,
                        obligation,
                        result,
                        target_type,
                        ..
                    },
                ) => Some((
                    *psi_operation,
                    *result,
                    ScalarType::Integer(*target_type),
                    Some(*obligation),
                )),
                (
                    DeadScalarFamily::UnconditionallyTotal | DeadScalarFamily::ProofCertified,
                    O::IntegerShift {
                        psi_operation,
                        result,
                        value_type,
                        policy,
                        ..
                    },
                ) => Some((
                    *psi_operation,
                    *result,
                    ScalarType::Integer(*value_type),
                    policy.obligation(),
                )),
                (
                    DeadScalarFamily::UnconditionallyTotal | DeadScalarFamily::ProofCertified,
                    O::IntegerArithmetic {
                        psi_operation,
                        result,
                        scalar_type,
                        arithmetic,
                        ..
                    },
                ) => Some((
                    *psi_operation,
                    *result,
                    ScalarType::Integer(*scalar_type),
                    arithmetic.obligation(),
                )),
                _ => None,
            }
        }
    }
    mod rule_catalog {
        use optimization_core::{OptimizationRuleIdentity, OptimizationSafetyClass};

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(super) enum DeadScalarFamily {
            Literal,
            UnconditionallyTotal,
            ProofCertified,
        }

        pub(super) fn dead_scalar_family(
            rule: OptimizationRuleIdentity,
        ) -> Option<DeadScalarFamily> {
            if rule
                == OptimizationRuleIdentity::from_canonical_bytes(
                    b"omega.psi-rule.dead-unused-scalar-literal-elimination.v1",
                )
            {
                Some(DeadScalarFamily::Literal)
            } else if rule
                == OptimizationRuleIdentity::from_canonical_bytes(
                    b"omega.psi-rule.dead-unused-unconditionally-total-scalar-elimination.v1",
                )
            {
                Some(DeadScalarFamily::UnconditionallyTotal)
            } else if rule
                == OptimizationRuleIdentity::from_canonical_bytes(
                    b"omega.psi-rule.dead-unused-proof-certified-scalar-elimination.v1",
                )
            {
                Some(DeadScalarFamily::ProofCertified)
            } else {
                None
            }
        }

        pub(super) fn expected_safety(
            rule: OptimizationRuleIdentity,
        ) -> Option<OptimizationSafetyClass> {
            dead_scalar_family(rule).map(|family| match family {
                DeadScalarFamily::Literal | DeadScalarFamily::UnconditionallyTotal => {
                    OptimizationSafetyClass::ExactOperationSemantics
                }
                DeadScalarFamily::ProofCertified => OptimizationSafetyClass::ProofCertified,
            })
        }

        pub(super) fn validator_bytes(rule: OptimizationRuleIdentity) -> Option<&'static [u8]> {
            dead_scalar_family(rule).map(|family| match family {
                DeadScalarFamily::Literal => {
                    b"omega.validator.dead-unused-scalar-literal.v1".as_slice()
                }
                DeadScalarFamily::UnconditionallyTotal => {
                    b"omega.validator.dead-unused-unconditionally-total-scalar.v1".as_slice()
                }
                DeadScalarFamily::ProofCertified => {
                    b"omega.validator.dead-unused-proof-certified-scalar-node.v1".as_slice()
                }
            })
        }
    }
    mod validation {
        use crate::candidates::rewrite_accounting::dead_scalar::reconstruct_dead_scalar_node_accounting;
        use crate::{
            OptimizationUnitValidationError, ValidatedPsiRewrite,
            reconstruct_closed_scalar_node_boundary,
        };
        use optimization_unit::{
            OptimizationFact, PsiOptimizationUnit, PsiRewriteCandidate, PsiRewritePatch,
        };

        use super::operation_partition::independently_validated_dead_scalar_shape;
        use crate::candidates::{
            accept_region_rewrite, candidate_block, candidate_function, retire_node_custody,
            sweep_output_nodes,
        };
        use optimization_unit::{receiver_custody_distinct, retirable_scalar_node};

        pub(super) fn validate_dead_scalar_node_candidate(
            input: &PsiOptimizationUnit,
            candidate: &PsiRewriteCandidate,
        ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
            candidate_patch!(patch, RemoveDeadScalarNode, candidate);
            if candidate.node_decision_point() != Some(patch.location)
                || !candidate.substitutions().is_empty()
            {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            let function = candidate_function(input, patch.location.machine)?;
            let block = candidate_block(function, patch.location.block)?;
            let node_index = usize::try_from(patch.location.node)
                .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
            let node = crate::candidates::candidate_node(block, patch.location.node)?;
            let (source_operation, result, scalar_type, obligation) =
                independently_validated_dead_scalar_shape(candidate.rule(), &node.operation)
                    .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
            if source_operation != patch.source_operation
                || result != patch.result
                || scalar_type != patch.scalar_type
                || !retirable_scalar_node(block, node_index, node, result, scalar_type)
            {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            let live = reconstruct_closed_scalar_node_boundary(input, patch.location)
                .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
            if live.live_out.contains(&result)
                || function
                    .node_uses()
                    .any(|use_site| use_site.value == result)
            {
                return Err(OptimizationUnitValidationError::CandidateLiveBoundaryMismatch);
            }
            match (obligation, candidate.accepted_obligation_witness()) {
                (Some(obligation), Some(identity)) => {
                    if !function.facts.iter().any(|fact| {
                        matches!(
                            fact,
                            OptimizationFact::OperationObligationReference {
                                obligation: reference,
                                support,
                            } if *support == source_operation && *reference == obligation
                        )
                    }) || input
                        .accepted_obligation_fact(
                            function.machine,
                            optimization_unit::PsiProvenance::Operation(source_operation),
                            Some(obligation),
                        )
                        .is_none_or(|fact| fact.identity != identity)
                    {
                        return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
                    }
                }
                (None, None) => {
                    if function.facts.iter().any(|fact| {
                        matches!(
                            fact,
                            OptimizationFact::OperationObligationReference { support, .. }
                                if *support == source_operation
                        )
                    }) {
                        return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
                    }
                }
                _ => {
                    return Err(
                        OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                    );
                }
            }
            if !receiver_custody_distinct(block, node_index, node) {
                return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
            }
            let (expected_blocks, accepted_provenance) =
                reconstruct_dead_scalar_node_accounting(function, patch)
                    .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;

            accept_region_rewrite(
                input,
                candidate,
                patch.location.machine,
                &expected_blocks,
                super::rule_catalog::validator_bytes(candidate.rule())
                    .expect("the entrance rejects unknown dead-scalar rules"),
                accepted_provenance,
                |output| {
                    let output_function = output
                        .function_mut(patch.location.machine)
                        .expect("candidate function exists");
                    let output_block = output_function
                        .block_mut(patch.location.block)
                        .expect("candidate block exists");
                    retire_node_custody(output_block, node_index)?;
                    sweep_output_nodes(output_function, |_| Ok(()))?;
                    Ok(())
                },
            )
        }
    }
    pub(crate) fn validate_dead_scalar_node_candidate(
        input: &PsiOptimizationUnit,
        candidate: &PsiRewriteCandidate,
    ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
        contract::admit_origin(input, candidate)?;
        let safety_class = rule_catalog::expected_safety(candidate.rule())
            .ok_or(OptimizationUnitValidationError::CandidateAnalysisContractMismatch)?;
        contract::admit_roster(
            candidate,
            &[AnalysisKind::ValueLiveness, AnalysisKind::EffectSummaries],
            contract::USE_DEFINITION_AND_EFFECT_SUMMARIES,
            safety_class,
            Some(-1),
        )?;
        validation::validate_dead_scalar_node_candidate(input, candidate)
    }
}
pub(crate) mod field_value_specialization {
    use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
    use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

    mod replay {
        use crate::candidates::rewrite_accounting::substitutions::rewrite_scalar_value_uses;
        use crate::candidates::state_specialization::cyclic_machines;
        use crate::candidates::{
            accept_region_rewrite, candidate_block, candidate_function, candidate_node,
            land_folded_nodes, retire_node_custody, sweep_output_nodes,
        };
        use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
        use abstract_operations::AbstractOperation as O;
        use optimization_unit::{
            FieldValueResolution, FieldValueRow, FoldedFieldValue, OptimizationNode,
            PsiOptimizationFunction, PsiOptimizationUnit, PsiRewriteCandidate, PsiRewritePatch,
            ScalarSubstitution, field_evidence, field_value_reads,
        };
        use semantic_vocabulary::{BlockId, ScalarType};
        use std::collections::{BTreeMap, BTreeSet};

        fn folded_node(
            row: &FieldValueRow,
            function: &PsiOptimizationFunction,
        ) -> Result<OptimizationNode, OptimizationUnitValidationError> {
            let node = candidate_node(candidate_block(function, row.site.block)?, row.site.node)?;
            let O::StructuralScalarFieldRead {
                psi_operation,
                result: read_result,
                source,
                path,
                field,
            } = &node.operation
            else {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            };
            let result = read_result.value;
            if row.site.machine != function.machine
                || *psi_operation != row.psi_operation
                || result != row.result
                || *source != row.source
                || path.as_slice() != row.path.as_slice()
                || *field != row.field
            {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            let operation = match &row.resolution {
                FieldValueResolution::Constant(FoldedFieldValue::Boolean(value)) => {
                    O::BooleanConstant {
                        psi_operation: *psi_operation,
                        result,
                        value: *value,
                    }
                }
                FieldValueResolution::Constant(FoldedFieldValue::Integer(value)) => {
                    O::IntegerConstant {
                        psi_operation: *psi_operation,
                        result,
                        scalar_type: match read_result.scalar_type {
                            ScalarType::Boolean => {
                                return Err(
                                    OptimizationUnitValidationError::CandidatePatchMismatch,
                                );
                            }
                            scalar_type => scalar_type,
                        },
                        value: *value,
                    }
                }
                FieldValueResolution::Forward(_) => {
                    return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                }
            };
            Ok(OptimizationNode {
                operation,
                provenance: node.provenance.clone(),
                work: node.work.clone(),
                effect: node.effect,
                definitions: node.definitions.clone(),
                uses: node.uses.clone(),
                successors: node.successors.clone(),
                ownership: node.ownership.clone(),
            })
        }

        fn expected_substitutions(rows: &[FieldValueRow]) -> Vec<ScalarSubstitution> {
            let mut substitutions = rows
                .iter()
                .filter_map(|row| match &row.resolution {
                    FieldValueResolution::Forward(forwarded) => Some(ScalarSubstitution {
                        from: row.result,
                        to: forwarded.initializer,
                        scalar_type: forwarded.scalar_type,
                    }),
                    FieldValueResolution::Constant(_) => None,
                })
                .collect::<Vec<_>>();
            substitutions.sort();
            substitutions
        }

        pub(super) fn validate(
            input: &PsiOptimizationUnit,
            candidate: &PsiRewriteCandidate,
        ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
            candidate_patch!(patch, SpecializeFieldValue, candidate);
            if candidate.node_decision_point() != patch.reads.first().map(|row| row.site) {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            if patch.reads.is_empty()
                || candidate.predicted_cost_delta()
                    != -i64::try_from(patch.reads.len())
                        .map_err(|_| OptimizationUnitValidationError::CandidatePatchMismatch)?
            {
                return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
            }
            if cyclic_machines(input).contains(&patch.machine) {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            let function = candidate_function(input, patch.machine)?;
            let Some(evidence) = field_evidence(function, patch.place) else {
                return Err(OptimizationUnitValidationError::CandidateLocationMissing);
            };
            if patch.producer != evidence.root_producer.operation() {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            let replayed = field_value_reads(input, &evidence, function);
            if replayed.is_empty() || patch.reads != replayed {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            if candidate.substitutions() != expected_substitutions(&patch.reads) {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            let (expected_blocks, expected_provenance) =
                optimization_unit::field_value_accounting(function, &patch.reads)
                    .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;

            let folded = patch
                .reads
                .iter()
                .filter(|row| matches!(row.resolution, FieldValueResolution::Constant(_)))
                .map(|row| folded_node(row, function).map(|node| (row.site, node)))
                .collect::<Result<Vec<_>, _>>()?;
            accept_region_rewrite(
                input,
                candidate,
                patch.machine,
                &expected_blocks,
                b"omega.validator.field-value-specialization.v1",
                expected_provenance,
                |output| {
                    land_folded_nodes(output, patch.machine, folded)?;
                    let output_function = output
                        .function_mut(patch.machine)
                        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
                    for row in &patch.reads {
                        let FieldValueResolution::Forward(forwarded) = &row.resolution else {
                            continue;
                        };
                        for node in output_function
                            .blocks
                            .iter_mut()
                            .flat_map(|block| &mut block.nodes)
                        {
                            rewrite_scalar_value_uses(
                                &mut node.operation,
                                row.result,
                                forwarded.initializer,
                            );
                        }
                    }
                    let mut removals: BTreeMap<BlockId, BTreeSet<u32>> = BTreeMap::new();
                    for row in &patch.reads {
                        if matches!(row.resolution, FieldValueResolution::Forward(_)) {
                            removals
                                .entry(row.site.block)
                                .or_default()
                                .insert(row.site.node);
                        }
                    }
                    for (block_id, sites) in removals {
                        let block = output_function
                            .block_mut(block_id)
                            .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
                        for site in sites.iter().rev() {
                            let index = usize::try_from(*site).map_err(|_| {
                                OptimizationUnitValidationError::CandidateLocationMissing
                            })?;
                            retire_node_custody(block, index)?;
                        }
                    }
                    sweep_output_nodes(output_function, |_| Ok(()))?;
                    Ok(())
                },
            )
        }
    }
    pub fn validate_field_value_specialization_candidate(
        input: &PsiOptimizationUnit,
        candidate: &PsiRewriteCandidate,
    ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
        super::contract::admit(
            input,
            candidate,
            &super::contract::FIELD_VALUE_SPECIALIZATION,
        )?;
        replay::validate(input, candidate)
    }
}
pub(crate) mod global_value_numbering {
    use crate::candidates::contract;
    use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
    use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit, PsiRewriteCandidate};
    use semantic_vocabulary::{ScalarType, ValueId};
    use std::collections::BTreeMap;

    mod admission {
        use crate::candidates::global_value_numbering::ScalarCseProofClass;
        use abstract_operations::AbstractOperation as O;
        use optimization_unit::{OptimizationFact, PsiOptimizationFunction, PsiOptimizationUnit};
        use semantic_vocabulary::{OperationId, ScalarType, ValueId};
        use std::collections::BTreeMap;

        use super::expression_keys::{
            IndependentScalarExpressionKey, independent_proof_scalar_expression,
            independent_total_scalar_expression,
        };

        pub(crate) fn independent_cse_expression(
            operation: &O,
            value_types: &BTreeMap<ValueId, ScalarType>,
            proof_class: ScalarCseProofClass,
        ) -> Option<(
            IndependentScalarExpressionKey,
            OperationId,
            ValueId,
            ScalarType,
            Option<semantic_vocabulary::ObligationId>,
        )> {
            match proof_class {
                ScalarCseProofClass::ObligationFree => {
                    let (key, operation, result, scalar_type) =
                        independent_total_scalar_expression(operation, value_types)?;
                    Some((
                        IndependentScalarExpressionKey::ObligationFree(key),
                        operation,
                        result,
                        scalar_type,
                        None,
                    ))
                }
                ScalarCseProofClass::ProofCertified => {
                    let (key, operation, result, scalar_type, obligation) =
                        independent_proof_scalar_expression(operation)?;
                    Some((
                        IndependentScalarExpressionKey::ProofCertified(key),
                        operation,
                        result,
                        scalar_type,
                        Some(obligation),
                    ))
                }
                ScalarCseProofClass::CompatiblePolicy => None,
            }
        }

        pub(crate) fn independently_accepted_operation_fact(
            input: &PsiOptimizationUnit,
            function: &PsiOptimizationFunction,
            operation: OperationId,
            obligation: semantic_vocabulary::ObligationId,
        ) -> Option<optimization_core::AcceptedObligationFactIdentity> {
            function
                .facts
                .iter()
                .any(|fact| {
                    matches!(
                        fact,
                        OptimizationFact::OperationObligationReference {
                            obligation: reference,
                            support,
                        } if *support == operation && *reference == obligation
                    )
                })
                .then(|| {
                    input
                        .accepted_obligation_fact(
                            function.machine,
                            optimization_unit::PsiProvenance::Operation(operation),
                            Some(obligation),
                        )
                        .map(|fact| fact.identity)
                })
                .flatten()
        }
    }
    mod dominance_reconstruction {
        use optimization_unit::{PsiOptimizationFunction, ValueDefinitionSite};
        use semantic_vocabulary::{BlockId, ScalarType, ValueId};
        use std::collections::{BTreeMap, BTreeSet};

        pub(crate) fn independent_reachable_dominators(
            function: &PsiOptimizationFunction,
        ) -> BTreeMap<BlockId, BTreeSet<BlockId>> {
            let successors = function
                .blocks
                .iter()
                .map(|block| {
                    (
                        block.id,
                        block
                            .nodes
                            .last()
                            .map(|node| node.successors.iter().map(|edge| edge.target).collect())
                            .unwrap_or_default(),
                    )
                })
                .collect::<BTreeMap<BlockId, Vec<BlockId>>>();
            let mut reachable = BTreeSet::from([function.entry]);
            let mut frontier = vec![function.entry];
            while let Some(block) = frontier.pop() {
                for successor in successors.get(&block).into_iter().flatten() {
                    if reachable.insert(*successor) {
                        frontier.push(*successor);
                    }
                }
            }
            let mut predecessors = reachable
                .iter()
                .copied()
                .map(|block| (block, BTreeSet::new()))
                .collect::<BTreeMap<_, _>>();
            for (source, targets) in &successors {
                if !reachable.contains(source) {
                    continue;
                }
                for target in targets.iter().filter(|target| reachable.contains(target)) {
                    predecessors.get_mut(target).unwrap().insert(*source);
                }
            }
            let mut result = reachable
                .iter()
                .copied()
                .map(|block| {
                    (
                        block,
                        if block == function.entry {
                            BTreeSet::from([block])
                        } else {
                            reachable.clone()
                        },
                    )
                })
                .collect::<BTreeMap<_, _>>();
            loop {
                let mut changed = false;
                for block in reachable
                    .iter()
                    .copied()
                    .filter(|block| *block != function.entry)
                {
                    let mut incoming = predecessors[&block].iter();
                    let mut next = incoming
                        .next()
                        .map(|predecessor| result[predecessor].clone())
                        .unwrap_or_default();
                    for predecessor in incoming {
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

        pub(crate) fn independently_replacement_dominates_uses(
            function: &PsiOptimizationFunction,
            dominators: &BTreeMap<BlockId, BTreeSet<BlockId>>,
            replacement: ValueId,
            parameter: ValueId,
            scalar_type: ScalarType,
        ) -> bool {
            if replacement == parameter {
                return false;
            }
            let Some(definition) = function
                .parameters
                .iter()
                .chain(
                    function
                        .blocks
                        .iter()
                        .flat_map(|block| block.parameters.iter().chain(block.definitions())),
                )
                .find(|definition| definition.value == replacement)
            else {
                return false;
            };
            if definition.scalar_type != scalar_type {
                return false;
            }
            function
                .node_uses()
                .filter(|use_site| use_site.value == parameter)
                .all(|use_site| match definition.site {
                    ValueDefinitionSite::FunctionParameter(_) => true,
                    ValueDefinitionSite::BlockParameter {
                        block: defining, ..
                    } => dominators
                        .get(&use_site.block)
                        .is_some_and(|rows| rows.contains(&defining)),
                    ValueDefinitionSite::Node {
                        block: defining,
                        node,
                    } if defining == use_site.block => node < use_site.node,
                    ValueDefinitionSite::Node {
                        block: defining, ..
                    } => dominators
                        .get(&use_site.block)
                        .is_some_and(|rows| rows.contains(&defining)),
                })
        }
    }
    mod expression_keys {
        use semantic_vocabulary::ValueId;

        mod compatible_policy {
            use crate::candidates::global_value_numbering::expression_keys::{
                IndependentCompatiblePolicyScalarExpressionKey, IndependentScalarExpressionKey,
                independent_pair,
            };
            use abstract_operations::{
                AbstractOperation as O, IntegerArithmeticOperator, IntegerOverflowPolicy,
                IntegerShiftDirection, IntegerShiftPolicy,
            };
            use semantic_vocabulary::{IntegerType, OperationId, ScalarType, ValueId};

            pub(crate) fn independent_compatible_policy_scalar_leader(
                operation: &O,
            ) -> Option<(
                IndependentScalarExpressionKey,
                OperationId,
                ValueId,
                ScalarType,
                Option<semantic_vocabulary::ObligationId>,
            )> {
                let (key, psi_operation, result, scalar_type, obligation) =
                    compatible_policy_scalar_row(operation)?;
                obligation.is_none().then_some((
                    IndependentScalarExpressionKey::CompatiblePolicy(key),
                    psi_operation,
                    result,
                    scalar_type,
                    None,
                ))
            }

            pub(crate) fn independent_compatible_policy_scalar_redundant(
                operation: &O,
            ) -> Option<(
                IndependentScalarExpressionKey,
                OperationId,
                ValueId,
                ScalarType,
                Option<semantic_vocabulary::ObligationId>,
            )> {
                let (key, psi_operation, result, scalar_type, obligation) =
                    compatible_policy_scalar_row(operation)?;
                obligation.map(|obligation| {
                    (
                        IndependentScalarExpressionKey::CompatiblePolicy(key),
                        psi_operation,
                        result,
                        scalar_type,
                        Some(obligation),
                    )
                })
            }

            fn compatible_policy_scalar_row(
                operation: &O,
            ) -> Option<(
                IndependentCompatiblePolicyScalarExpressionKey,
                OperationId,
                ValueId,
                ScalarType,
                Option<semantic_vocabulary::ObligationId>,
            )> {
                match operation {
                    O::IntegerShift {
                        psi_operation,
                        result,
                        value_type,
                        count_type,
                        policy,
                        direction,
                        value,
                        count,
                    } => {
                        let obligation = match policy {
                            IntegerShiftPolicy::Wrapping => None,
                            IntegerShiftPolicy::Exact { obligation } => Some(*obligation),
                        };
                        Some((
                            shift_key(*direction, *value_type, *count_type, *value, *count),
                            *psi_operation,
                            *result,
                            ScalarType::Integer(*value_type),
                            obligation,
                        ))
                    }
                    O::IntegerArithmetic {
                        psi_operation,
                        result,
                        scalar_type,
                        arithmetic,
                        left,
                        right,
                    } => {
                        let obligation = match arithmetic.policy() {
                            IntegerOverflowPolicy::Wrapping | IntegerOverflowPolicy::Saturating => {
                                None
                            }
                            IntegerOverflowPolicy::Exact => Some(arithmetic.obligation()?),
                        };
                        Some((
                            arithmetic_key(arithmetic.operator(), *scalar_type, *left, *right)?,
                            *psi_operation,
                            *result,
                            ScalarType::Integer(*scalar_type),
                            obligation,
                        ))
                    }
                    _ => None,
                }
            }

            fn shift_key(
                direction: IntegerShiftDirection,
                value_type: IntegerType,
                count_type: IntegerType,
                value: ValueId,
                count: ValueId,
            ) -> IndependentCompatiblePolicyScalarExpressionKey {
                match direction {
                    IntegerShiftDirection::Left => {
                        IndependentCompatiblePolicyScalarExpressionKey::ShiftLeft(
                            value_type, count_type, value, count,
                        )
                    }
                    IntegerShiftDirection::Right => {
                        IndependentCompatiblePolicyScalarExpressionKey::ShiftRight(
                            value_type, count_type, value, count,
                        )
                    }
                }
            }

            fn arithmetic_key(
                operator: IntegerArithmeticOperator,
                scalar_type: IntegerType,
                left: ValueId,
                right: ValueId,
            ) -> Option<IndependentCompatiblePolicyScalarExpressionKey> {
                let (low, high) = independent_pair(left, right);
                match operator {
                    IntegerArithmeticOperator::Add => Some(
                        IndependentCompatiblePolicyScalarExpressionKey::Add(scalar_type, low, high),
                    ),
                    IntegerArithmeticOperator::Subtract => {
                        Some(IndependentCompatiblePolicyScalarExpressionKey::Subtract(
                            scalar_type,
                            left,
                            right,
                        ))
                    }
                    IntegerArithmeticOperator::Multiply => {
                        Some(IndependentCompatiblePolicyScalarExpressionKey::Multiply(
                            scalar_type,
                            low,
                            high,
                        ))
                    }
                    IntegerArithmeticOperator::Divide | IntegerArithmeticOperator::Remainder => {
                        None
                    }
                }
            }
        }
        mod proof_certified {
            use crate::candidates::global_value_numbering::expression_keys::{
                IndependentProofScalarExpressionKey, independent_pair,
            };
            use abstract_operations::{
                AbstractOperation as O, IntegerArithmeticOperator, IntegerOverflowPolicy,
                IntegerShiftDirection, IntegerShiftPolicy,
            };
            use semantic_vocabulary::{OperationId, ScalarType, ValueId};

            pub(crate) fn independent_proof_scalar_expression(
                operation: &O,
            ) -> Option<(
                IndependentProofScalarExpressionKey,
                OperationId,
                ValueId,
                ScalarType,
                semantic_vocabulary::ObligationId,
            )> {
                Some(match operation {
                    O::IntegerExactCast {
                        psi_operation,
                        obligation,
                        result,
                        source_type,
                        target_type,
                        operand,
                    } => (
                        IndependentProofScalarExpressionKey::ExactCast(
                            *source_type,
                            *target_type,
                            *operand,
                        ),
                        *psi_operation,
                        *result,
                        ScalarType::Integer(*target_type),
                        *obligation,
                    ),
                    O::IntegerShift {
                        psi_operation,
                        result,
                        value_type,
                        count_type,
                        policy: IntegerShiftPolicy::Exact { obligation },
                        direction,
                        value,
                        count,
                    } => (
                        match direction {
                            IntegerShiftDirection::Left => {
                                IndependentProofScalarExpressionKey::ExactShiftLeft(
                                    *value_type,
                                    *count_type,
                                    *value,
                                    *count,
                                )
                            }
                            IntegerShiftDirection::Right => {
                                IndependentProofScalarExpressionKey::ExactShiftRight(
                                    *value_type,
                                    *count_type,
                                    *value,
                                    *count,
                                )
                            }
                        },
                        *psi_operation,
                        *result,
                        ScalarType::Integer(*value_type),
                        *obligation,
                    ),
                    O::IntegerArithmetic {
                        psi_operation,
                        result,
                        scalar_type,
                        arithmetic,
                        left,
                        right,
                    } => {
                        use IndependentProofScalarExpressionKey as Key;
                        use IntegerArithmeticOperator as Operator;
                        use IntegerOverflowPolicy as Policy;
                        let obligation = arithmetic.obligation()?;
                        let (low, high) = independent_pair(*left, *right);
                        let key = match (arithmetic.policy(), arithmetic.operator()) {
                            (Policy::Exact, Operator::Add) => {
                                Key::ExactAdd(*scalar_type, low, high)
                            }
                            (Policy::Exact, Operator::Subtract) => {
                                Key::ExactSubtract(*scalar_type, *left, *right)
                            }
                            (Policy::Exact, Operator::Multiply) => {
                                Key::ExactMultiply(*scalar_type, low, high)
                            }
                            (Policy::Exact, Operator::Divide) => {
                                Key::ExactDivide(*scalar_type, *left, *right)
                            }
                            (Policy::Exact, Operator::Remainder) => {
                                Key::ExactRemainder(*scalar_type, *left, *right)
                            }
                            (Policy::Wrapping, Operator::Divide) => {
                                Key::WrappingDivide(*scalar_type, *left, *right)
                            }
                            (Policy::Wrapping, Operator::Remainder) => {
                                Key::WrappingRemainder(*scalar_type, *left, *right)
                            }
                            (Policy::Saturating, Operator::Divide) => {
                                Key::SaturatingDivide(*scalar_type, *left, *right)
                            }
                            (Policy::Saturating, Operator::Remainder) => {
                                Key::SaturatingRemainder(*scalar_type, *left, *right)
                            }
                            (
                                Policy::Wrapping | Policy::Saturating,
                                Operator::Add | Operator::Subtract | Operator::Multiply,
                            ) => return None,
                        };
                        (
                            key,
                            *psi_operation,
                            *result,
                            ScalarType::Integer(*scalar_type),
                            obligation,
                        )
                    }
                    _ => return None,
                })
            }
        }
        mod total {
            use crate::candidates::global_value_numbering::expression_keys::{
                IndependentTotalScalarExpressionKey, independent_pair,
            };
            use abstract_operations::{
                AbstractOperation as O, IntegerArithmeticOperator, IntegerOverflowPolicy,
                IntegerShiftDirection, IntegerShiftPolicy,
            };
            use semantic_vocabulary::{OperationId, ScalarType, ValueId};
            use std::collections::BTreeMap;

            pub(crate) fn independent_total_scalar_expression(
                operation: &O,
                value_types: &BTreeMap<ValueId, ScalarType>,
            ) -> Option<(
                IndependentTotalScalarExpressionKey,
                OperationId,
                ValueId,
                ScalarType,
            )> {
                let operand_integer = |value: ValueId| match value_types.get(&value) {
                    Some(ScalarType::Integer(row)) => Some(*row),
                    _ => None,
                };
                Some(match operation {
                    O::BooleanConstant {
                        psi_operation,
                        result,
                        value,
                    } => (
                        IndependentTotalScalarExpressionKey::BooleanConstant(*value),
                        *psi_operation,
                        *result,
                        ScalarType::Boolean,
                    ),
                    O::IntegerConstant {
                        psi_operation,
                        result,
                        scalar_type,
                        value,
                    } => (
                        IndependentTotalScalarExpressionKey::IntegerConstant(*scalar_type, *value),
                        *psi_operation,
                        *result,
                        *scalar_type,
                    ),
                    O::BooleanNot {
                        psi_operation,
                        result,
                        operand,
                    } => (
                        IndependentTotalScalarExpressionKey::BooleanNot(*operand),
                        *psi_operation,
                        *result,
                        ScalarType::Boolean,
                    ),
                    O::BooleanEqual {
                        psi_operation,
                        result,
                        left,
                        right,
                    } => {
                        let (left, right) = independent_pair(*left, *right);
                        (
                            IndependentTotalScalarExpressionKey::BooleanEqual(left, right),
                            *psi_operation,
                            *result,
                            ScalarType::Boolean,
                        )
                    }
                    O::IntegerEqual {
                        psi_operation,
                        result,
                        left,
                        right,
                    } => {
                        let scalar_type = operand_integer(*left)?;
                        if operand_integer(*right)? != scalar_type {
                            return None;
                        }
                        let (left, right) = independent_pair(*left, *right);
                        (
                            IndependentTotalScalarExpressionKey::IntegerEqual(
                                scalar_type,
                                left,
                                right,
                            ),
                            *psi_operation,
                            *result,
                            ScalarType::Boolean,
                        )
                    }
                    O::IntegerLessThan {
                        psi_operation,
                        result,
                        left,
                        right,
                    } => {
                        let scalar_type = operand_integer(*left)?;
                        if operand_integer(*right)? != scalar_type {
                            return None;
                        }
                        (
                            IndependentTotalScalarExpressionKey::IntegerLessThan(
                                scalar_type,
                                *left,
                                *right,
                            ),
                            *psi_operation,
                            *result,
                            ScalarType::Boolean,
                        )
                    }
                    O::IntegerLessOrEqual {
                        psi_operation,
                        result,
                        left,
                        right,
                    } => {
                        let scalar_type = operand_integer(*left)?;
                        if operand_integer(*right)? != scalar_type {
                            return None;
                        }
                        (
                            IndependentTotalScalarExpressionKey::IntegerLessOrEqual(
                                scalar_type,
                                *left,
                                *right,
                            ),
                            *psi_operation,
                            *result,
                            ScalarType::Boolean,
                        )
                    }
                    O::IntegerBitwiseNot {
                        psi_operation,
                        result,
                        scalar_type,
                        operand,
                    } => (
                        IndependentTotalScalarExpressionKey::IntegerBitwiseNot(
                            *scalar_type,
                            *operand,
                        ),
                        *psi_operation,
                        *result,
                        ScalarType::Integer(*scalar_type),
                    ),
                    O::IntegerWiden {
                        psi_operation,
                        result,
                        source_type,
                        target_type,
                        operand,
                    } => (
                        IndependentTotalScalarExpressionKey::IntegerWiden(
                            *source_type,
                            *target_type,
                            *operand,
                        ),
                        *psi_operation,
                        *result,
                        ScalarType::Integer(*target_type),
                    ),
                    O::IntegerBitwiseAnd {
                        psi_operation,
                        result,
                        scalar_type,
                        left,
                        right,
                    } => {
                        let (left, right) = independent_pair(*left, *right);
                        (
                            IndependentTotalScalarExpressionKey::IntegerBitwiseAnd(
                                *scalar_type,
                                left,
                                right,
                            ),
                            *psi_operation,
                            *result,
                            ScalarType::Integer(*scalar_type),
                        )
                    }
                    O::IntegerBitwiseOr {
                        psi_operation,
                        result,
                        scalar_type,
                        left,
                        right,
                    } => {
                        let (left, right) = independent_pair(*left, *right);
                        (
                            IndependentTotalScalarExpressionKey::IntegerBitwiseOr(
                                *scalar_type,
                                left,
                                right,
                            ),
                            *psi_operation,
                            *result,
                            ScalarType::Integer(*scalar_type),
                        )
                    }
                    O::IntegerBitwiseXor {
                        psi_operation,
                        result,
                        scalar_type,
                        left,
                        right,
                    } => {
                        let (left, right) = independent_pair(*left, *right);
                        (
                            IndependentTotalScalarExpressionKey::IntegerBitwiseXor(
                                *scalar_type,
                                left,
                                right,
                            ),
                            *psi_operation,
                            *result,
                            ScalarType::Integer(*scalar_type),
                        )
                    }
                    O::IntegerShift {
                        psi_operation,
                        result,
                        value_type,
                        count_type,
                        policy: IntegerShiftPolicy::Wrapping,
                        direction,
                        value,
                        count,
                    } => (
                        match direction {
                            IntegerShiftDirection::Left => {
                                IndependentTotalScalarExpressionKey::WrappingShiftLeft(
                                    *value_type,
                                    *count_type,
                                    *value,
                                    *count,
                                )
                            }
                            IntegerShiftDirection::Right => {
                                IndependentTotalScalarExpressionKey::WrappingShiftRight(
                                    *value_type,
                                    *count_type,
                                    *value,
                                    *count,
                                )
                            }
                        },
                        *psi_operation,
                        *result,
                        ScalarType::Integer(*value_type),
                    ),
                    O::IntegerArithmetic {
                        psi_operation,
                        result,
                        scalar_type,
                        arithmetic,
                        left,
                        right,
                    } => {
                        use IndependentTotalScalarExpressionKey as Key;
                        use IntegerArithmeticOperator as Operator;
                        use IntegerOverflowPolicy as Policy;
                        let (low, high) = independent_pair(*left, *right);
                        let key = match (arithmetic.policy(), arithmetic.operator()) {
                            (Policy::Wrapping, Operator::Add) => {
                                Key::WrappingAdd(*scalar_type, low, high)
                            }
                            (Policy::Wrapping, Operator::Subtract) => {
                                Key::WrappingSubtract(*scalar_type, *left, *right)
                            }
                            (Policy::Wrapping, Operator::Multiply) => {
                                Key::WrappingMultiply(*scalar_type, low, high)
                            }
                            (Policy::Saturating, Operator::Add) => {
                                Key::SaturatingAdd(*scalar_type, low, high)
                            }
                            (Policy::Saturating, Operator::Subtract) => {
                                Key::SaturatingSubtract(*scalar_type, *left, *right)
                            }
                            (Policy::Saturating, Operator::Multiply) => {
                                Key::SaturatingMultiply(*scalar_type, low, high)
                            }
                            _ => return None,
                        };
                        (
                            key,
                            *psi_operation,
                            *result,
                            ScalarType::Integer(*scalar_type),
                        )
                    }
                    _ => return None,
                })
            }
        }
        pub(crate) use compatible_policy::{
            independent_compatible_policy_scalar_leader,
            independent_compatible_policy_scalar_redundant,
        };
        pub(crate) use proof_certified::independent_proof_scalar_expression;
        use semantic_vocabulary::{IntegerType, ScalarType};
        pub(crate) use total::independent_total_scalar_expression;

        pub(crate) fn independent_pair(left: ValueId, right: ValueId) -> (ValueId, ValueId) {
            if left <= right {
                (left, right)
            } else {
                (right, left)
            }
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        pub(crate) enum IndependentTotalScalarExpressionKey {
            BooleanConstant(bool),
            IntegerConstant(ScalarType, semantic_vocabulary::IntegerValue),
            BooleanNot(ValueId),
            BooleanEqual(ValueId, ValueId),
            IntegerEqual(IntegerType, ValueId, ValueId),
            IntegerLessThan(IntegerType, ValueId, ValueId),
            IntegerLessOrEqual(IntegerType, ValueId, ValueId),
            IntegerBitwiseNot(IntegerType, ValueId),
            IntegerWiden(IntegerType, IntegerType, ValueId),
            IntegerBitwiseAnd(IntegerType, ValueId, ValueId),
            IntegerBitwiseOr(IntegerType, ValueId, ValueId),
            IntegerBitwiseXor(IntegerType, ValueId, ValueId),
            WrappingShiftLeft(IntegerType, IntegerType, ValueId, ValueId),
            WrappingShiftRight(IntegerType, IntegerType, ValueId, ValueId),
            WrappingAdd(IntegerType, ValueId, ValueId),
            WrappingSubtract(IntegerType, ValueId, ValueId),
            WrappingMultiply(IntegerType, ValueId, ValueId),
            SaturatingAdd(IntegerType, ValueId, ValueId),
            SaturatingSubtract(IntegerType, ValueId, ValueId),
            SaturatingMultiply(IntegerType, ValueId, ValueId),
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        pub(crate) enum IndependentProofScalarExpressionKey {
            ExactCast(IntegerType, IntegerType, ValueId),
            ExactShiftLeft(IntegerType, IntegerType, ValueId, ValueId),
            ExactShiftRight(IntegerType, IntegerType, ValueId, ValueId),
            ExactAdd(IntegerType, ValueId, ValueId),
            ExactSubtract(IntegerType, ValueId, ValueId),
            ExactMultiply(IntegerType, ValueId, ValueId),
            ExactDivide(IntegerType, ValueId, ValueId),
            ExactRemainder(IntegerType, ValueId, ValueId),
            WrappingDivide(IntegerType, ValueId, ValueId),
            WrappingRemainder(IntegerType, ValueId, ValueId),
            SaturatingDivide(IntegerType, ValueId, ValueId),
            SaturatingRemainder(IntegerType, ValueId, ValueId),
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        pub(crate) enum IndependentCompatiblePolicyScalarExpressionKey {
            ShiftLeft(IntegerType, IntegerType, ValueId, ValueId),
            ShiftRight(IntegerType, IntegerType, ValueId, ValueId),
            Add(IntegerType, ValueId, ValueId),
            Subtract(IntegerType, ValueId, ValueId),
            Multiply(IntegerType, ValueId, ValueId),
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        pub(crate) enum IndependentScalarExpressionKey {
            ObligationFree(IndependentTotalScalarExpressionKey),
            ProofCertified(IndependentProofScalarExpressionKey),
            CompatiblePolicy(IndependentCompatiblePolicyScalarExpressionKey),
        }
    }
    mod local_and_dominating {
        use crate::candidates::accept_region_rewrite;
        use crate::candidates::candidate_block;
        use crate::candidates::candidate_function;
        use crate::candidates::contract;
        use crate::candidates::global_value_numbering::{ScalarCseProofClass, ScalarCseScope};
        use crate::candidates::retire_node_custody;
        use crate::candidates::rewrite_accounting::common_subexpression::reconstruct_local_cse_accounting;
        use crate::candidates::rewrite_accounting::substitutions::rewrite_scalar_value_uses;
        use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
        use abstract_operations::AbstractOperation as O;
        use optimization_unit::{
            LocalScalarCommonSubexpressionRewrite, NodeLocation, OptimizationFact,
            PsiOptimizationUnit, PsiRewriteCandidate, PsiRewritePatch, ScalarSubstitution,
            ValueDefinition, ValueDefinitionSite,
        };
        use std::collections::BTreeSet;

        use super::admission::{independent_cse_expression, independently_accepted_operation_fact};
        use super::dominance_reconstruction::independent_reachable_dominators;
        use super::expression_keys::{
            independent_compatible_policy_scalar_leader,
            independent_compatible_policy_scalar_redundant,
        };
        use super::rule_catalog;
        use crate::candidates::sweep_output_nodes;

        pub(super) fn validate_scalar_common_subexpression_candidate(
            input: &PsiOptimizationUnit,
            candidate: &PsiRewriteCandidate,
            scope: ScalarCseScope,
            proof_class: ScalarCseProofClass,
        ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
            let required = match scope {
                ScalarCseScope::SameBlock => rule_catalog::SAME_BLOCK_ANALYSES,
                ScalarCseScope::Dominating => rule_catalog::DOMINATING_ANALYSES,
            };
            contract::admit_roster(
                candidate,
                required,
                contract::USE_DEFINITION_AND_EFFECT_SUMMARIES,
                rule_catalog::safety_class(proof_class),
                Some(-1),
            )?;
            let patch = match (scope, candidate.patch()) {
                (
                    ScalarCseScope::SameBlock,
                    PsiRewritePatch::EliminateLocalScalarCommonSubexpression(patch),
                ) => patch,
                (
                    ScalarCseScope::Dominating,
                    PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(patch),
                ) => LocalScalarCommonSubexpressionRewrite {
                    leader: patch.leader,
                    redundant: patch.redundant,
                    leader_operation: patch.leader_operation,
                    redundant_operation: patch.redundant_operation,
                    leader_result: patch.leader_result,
                    redundant_result: patch.redundant_result,
                    scalar_type: patch.scalar_type,
                },
                _ => return Err(OptimizationUnitValidationError::CandidatePatchMismatch),
            };
            if candidate.node_decision_point() != Some(patch.redundant) {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            let expected_substitution = [ScalarSubstitution {
                from: patch.redundant_result,
                to: patch.leader_result,
                scalar_type: patch.scalar_type,
            }];
            if candidate.substitutions() != expected_substitution {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            if patch.leader.machine != patch.redundant.machine {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            let function = candidate_function(input, patch.leader.machine)?;
            let leader_block = candidate_block(function, patch.leader.block)?;
            let redundant_block = candidate_block(function, patch.redundant.block)?;
            let leader = leader_block
                .nodes
                .get(
                    usize::try_from(patch.leader.node)
                        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?,
                )
                .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
            let redundant_index = usize::try_from(patch.redundant.node)
                .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
            let redundant =
                crate::candidates::candidate_node(redundant_block, patch.redundant.node)?;
            if redundant_block.nodes.get(redundant_index + 1).is_none() {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            match scope {
                ScalarCseScope::SameBlock
                    if patch.leader.block != patch.redundant.block
                        || patch.leader.node >= patch.redundant.node =>
                {
                    return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                }
                ScalarCseScope::Dominating if patch.leader.block == patch.redundant.block => {
                    return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                }
                _ => {}
            }
            let value_types = super::scalar_value_types(function);
            let admitted_expression = |operation: &O| {
                let row = match proof_class {
                    ScalarCseProofClass::CompatiblePolicy => {
                        independent_compatible_policy_scalar_leader(operation)?
                    }
                    _ => independent_cse_expression(operation, &value_types, proof_class)?,
                };
                match (proof_class, row.4) {
            (ScalarCseProofClass::ObligationFree, None) => Some(row),
            (ScalarCseProofClass::ProofCertified, Some(obligation))
                if independently_accepted_operation_fact(input, function, row.1, obligation)
                    .is_some() =>
            {
                Some(row)
            }
            (ScalarCseProofClass::CompatiblePolicy, None)
                if !function.facts.iter().any(|fact| {
                    matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                        if *support == row.1)
                }) =>
            {
                Some(row)
            }
            _ => None,
        }
            };
            let (leader_key, leader_operation, leader_result, leader_type, leader_obligation) =
                match proof_class {
                    ScalarCseProofClass::CompatiblePolicy => {
                        independent_compatible_policy_scalar_leader(&leader.operation)
                    }
                    _ => independent_cse_expression(&leader.operation, &value_types, proof_class),
                }
                .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
            let (
                redundant_key,
                redundant_operation,
                redundant_result,
                redundant_type,
                redundant_obligation,
            ) = match proof_class {
                ScalarCseProofClass::CompatiblePolicy => {
                    independent_compatible_policy_scalar_redundant(&redundant.operation)
                }
                _ => independent_cse_expression(&redundant.operation, &value_types, proof_class),
            }
            .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
            if leader_key != redundant_key
                || leader_operation != patch.leader_operation
                || redundant_operation != patch.redundant_operation
                || leader_result != patch.leader_result
                || redundant_result != patch.redundant_result
                || leader_type != patch.scalar_type
                || redundant_type != patch.scalar_type
                || leader_result == redundant_result
                || leader_operation == redundant_operation
            {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            let _proof_facts = match (proof_class, leader_obligation, redundant_obligation) {
                (ScalarCseProofClass::ObligationFree, None, None) => {
                    if candidate.accepted_obligation_witness().is_some()
                || function.facts.iter().any(|fact| {
                    matches!(
                        fact,
                        OptimizationFact::OperationObligationReference { support, .. }
                            if *support == leader_operation || *support == redundant_operation
                    )
                })
            {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            }
                    None
                }
                (ScalarCseProofClass::ProofCertified, Some(leader), Some(redundant)) => {
                    let leader_fact = independently_accepted_operation_fact(
                        input,
                        function,
                        leader_operation,
                        leader,
                    )
                    .ok_or(
                        OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                    )?;
                    let redundant_fact = independently_accepted_operation_fact(
                        input,
                        function,
                        redundant_operation,
                        redundant,
                    )
                    .ok_or(
                        OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                    )?;
                    if candidate.accepted_obligation_witness() != Some(redundant_fact) {
                        return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
                    }
                    Some((leader_fact, redundant_fact))
                }
                (ScalarCseProofClass::CompatiblePolicy, None, Some(redundant)) => {
                    if function.facts.iter().any(|fact| {
                matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                    if *support == leader_operation)
            }) {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            }
                    let redundant_fact = independently_accepted_operation_fact(
                        input,
                        function,
                        redundant_operation,
                        redundant,
                    )
                    .ok_or(
                        OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                    )?;
                    if candidate.accepted_obligation_witness() != Some(redundant_fact) {
                        return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
                    }
                    None
                }
                _ => {
                    return Err(
                        OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                    );
                }
            };
            if scope == ScalarCseScope::SameBlock {
                let canonical_leader = leader_block
                    .nodes
                    .iter()
                    .take(redundant_index)
                    .enumerate()
                    .filter_map(|(node, candidate)| {
                        let (key, _, _, scalar_type, _) =
                            admitted_expression(&candidate.operation)?;
                        (key == redundant_key && scalar_type == patch.scalar_type).then_some(
                            NodeLocation {
                                machine: function.machine,
                                block: leader_block.id,
                                node: u32::try_from(node).ok()?,
                            },
                        )
                    })
                    .next();
                if canonical_leader != Some(patch.leader) {
                    return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                }
            }
            if scope == ScalarCseScope::Dominating {
                let dominators = independent_reachable_dominators(function);
                if !dominators
                    .get(&patch.redundant.block)
                    .is_some_and(|rows| rows.contains(&patch.leader.block))
                {
                    return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                }
                let canonical_leader = function
                    .blocks
                    .iter()
                    .filter(|block| block.id != patch.redundant.block)
                    .filter(|block| {
                        dominators
                            .get(&patch.redundant.block)
                            .is_some_and(|rows| rows.contains(&block.id))
                    })
                    .flat_map(|block| {
                        block
                            .nodes
                            .iter()
                            .enumerate()
                            .filter_map(|(node, candidate)| {
                                let (key, _, _, scalar_type, _) =
                                    admitted_expression(&candidate.operation)?;
                                (key == redundant_key && scalar_type == patch.scalar_type)
                                    .then_some(NodeLocation {
                                        machine: function.machine,
                                        block: block.id,
                                        node: u32::try_from(node).ok()?,
                                    })
                            })
                    })
                    .min_by_key(|location| {
                        (
                            dominators
                                .get(&location.block)
                                .map_or(usize::MAX, BTreeSet::len),
                            *location,
                        )
                    });
                if canonical_leader != Some(patch.leader)
                    || function
                        .node_uses()
                        .filter(|use_site| use_site.value == redundant_result)
                        .any(|use_site| {
                            if use_site.block == patch.leader.block {
                                patch.leader.node >= use_site.node
                            } else {
                                !dominators
                                    .get(&use_site.block)
                                    .is_some_and(|rows| rows.contains(&patch.leader.block))
                            }
                        })
                {
                    return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                }
            }
            if leader.definitions
                != [ValueDefinition {
                    value: leader_result,
                    scalar_type: leader_type,
                    site: ValueDefinitionSite::Node {
                        block: leader_block.id,
                        node: patch.leader.node,
                    },
                }]
                || redundant.definitions
                    != [ValueDefinition {
                        value: redundant_result,
                        scalar_type: redundant_type,
                        site: ValueDefinitionSite::Node {
                            block: redundant_block.id,
                            node: patch.redundant.node,
                        },
                    }]
                || !leader.successors.is_empty()
                || !redundant.successors.is_empty()
                || !leader.ownership.is_empty()
                || !redundant.ownership.is_empty()
                || !function
                    .blocks
                    .iter()
                    .flat_map(|row| &row.nodes)
                    .flat_map(|row| &row.uses)
                    .any(|row| row.value == redundant_result)
            {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            let (expected_blocks, accepted_provenance) =
                reconstruct_local_cse_accounting(function, patch)
                    .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
            accept_region_rewrite(
        input,
        candidate,
        patch.leader.machine,
        &expected_blocks,
        match (scope, proof_class) {
            (ScalarCseScope::SameBlock, ScalarCseProofClass::ObligationFree) => {
                b"omega.validator.same-block-obligation-free-total-scalar-cse.v1".as_slice()
            }
            (ScalarCseScope::Dominating, ScalarCseProofClass::ObligationFree) => {
                b"omega.validator.dominator-total-scalar-cse.v1".as_slice()
            }
            (ScalarCseScope::SameBlock, ScalarCseProofClass::ProofCertified) => {
                b"omega.validator.same-block-proof-certified-total-scalar-cse.v1".as_slice()
            }
            (ScalarCseScope::Dominating, ScalarCseProofClass::ProofCertified) => {
                b"omega.validator.dominator-proof-certified-total-scalar-gvn.v1".as_slice()
            }
            (ScalarCseScope::SameBlock, ScalarCseProofClass::CompatiblePolicy) => {
                b"omega.validator.same-block-proof-certified-compatible-policy-scalar-cse.v1"
                    .as_slice()
            }
            (ScalarCseScope::Dominating, ScalarCseProofClass::CompatiblePolicy) => {
                b"omega.validator.dominator-proof-certified-compatible-policy-scalar-gvn.v1"
                    .as_slice()
            }
        },
        accepted_provenance,
        |output| {
            let output_function = output
                .function_mut(patch.leader.machine)
                .expect("candidate function exists");
            let output_block = output_function
                .blocks
                .iter_mut()
                .find(|row| row.id == patch.redundant.block)
                .expect("candidate block exists");
            retire_node_custody(output_block, redundant_index)?;
            sweep_output_nodes(output_function, |node| {
                rewrite_scalar_value_uses(&mut node.operation, redundant_result, leader_result);
                Ok(())
            })?;
            Ok(())
        },
    )
        }
    }
    mod phi_translated {
        use crate::candidates::accept_region_rewrite;
        use crate::candidates::candidate_block;
        use crate::candidates::candidate_function;
        use crate::candidates::contract;
        use crate::candidates::global_value_numbering::ScalarCseProofClass;
        use crate::candidates::retire_node_custody;
        use crate::candidates::rewrite_accounting::common_subexpression::reconstruct_phi_translated_cse_accounting;
        use crate::candidates::rewrite_accounting::substitutions::{
            rewrite_scalar_value_uses, rewrite_successor_operation,
        };
        use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
        use optimization_unit::{
            NodeLocation, OptimizationFact, PhiTranslatedScalarIncoming, PsiOptimizationUnit,
            PsiRewriteCandidate, PsiRewritePatch, ValueDefinition, ValueDefinitionSite,
        };
        use std::collections::BTreeSet;

        use super::admission::{independent_cse_expression, independently_accepted_operation_fact};
        use super::dominance_reconstruction::independent_reachable_dominators;
        use super::expression_keys::{
            independent_compatible_policy_scalar_leader,
            independent_compatible_policy_scalar_redundant,
        };
        use super::rule_catalog;
        use crate::candidates::sweep_output_nodes;

        pub(super) fn validate_phi_translated_scalar_common_subexpression_candidate(
            input: &PsiOptimizationUnit,
            candidate: &PsiRewriteCandidate,
            proof_class: ScalarCseProofClass,
        ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
            contract::admit_roster(
                candidate,
                rule_catalog::DOMINATING_ANALYSES,
                contract::USE_DEFINITION_AND_EFFECT_SUMMARIES,
                rule_catalog::safety_class(proof_class),
                Some(-1),
            )?;
            if !candidate.substitutions().is_empty() {
                return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
            }
            candidate_patch!(
                patch,
                EliminatePhiTranslatedScalarCommonSubexpression,
                candidate
            );
            if candidate.node_decision_point() != Some(patch.redundant) {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            let function = candidate_function(input, patch.redundant.machine)?;
            let join = candidate_block(function, patch.redundant.block)?;
            let redundant_index = usize::try_from(patch.redundant.node)
                .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
            let redundant = crate::candidates::candidate_node(join, patch.redundant.node)?;
            if function.entry == join.id
                || join.nodes.get(redundant_index + 1).is_none()
                || usize::try_from(patch.parameter_position).ok() != Some(join.parameters.len())
                || join
                    .parameters
                    .iter()
                    .any(|row| row.value == patch.redundant_result)
            {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            let value_types = super::scalar_value_types(function);
            let (_, redundant_operation, redundant_result, redundant_type, redundant_obligation) =
                match proof_class {
                    ScalarCseProofClass::CompatiblePolicy => {
                        independent_compatible_policy_scalar_redundant(&redundant.operation)
                    }
                    _ => {
                        independent_cse_expression(&redundant.operation, &value_types, proof_class)
                    }
                }
                .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
            if redundant_operation != patch.redundant_operation
                || redundant_result != patch.redundant_result
                || redundant_type != patch.scalar_type
                || redundant.definitions
                    != [ValueDefinition {
                        value: redundant_result,
                        scalar_type: redundant_type,
                        site: ValueDefinitionSite::Node {
                            block: join.id,
                            node: patch.redundant.node,
                        },
                    }]
                || !redundant.successors.is_empty()
                || !redundant.ownership.is_empty()
                || !function
                    .node_uses()
                    .any(|use_site| use_site.value == redundant_result)
            {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            let _redundant_fact = match (proof_class, redundant_obligation) {
                (ScalarCseProofClass::ObligationFree, None) => {
                    if candidate.accepted_obligation_witness().is_some()
                || function.facts.iter().any(|fact| {
                    matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                        if *support == redundant_operation)
                })
            {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            }
                    None
                }
                (
                    ScalarCseProofClass::ProofCertified | ScalarCseProofClass::CompatiblePolicy,
                    Some(obligation),
                ) => {
                    let fact = independently_accepted_operation_fact(
                        input,
                        function,
                        redundant_operation,
                        obligation,
                    )
                    .ok_or(
                        OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                    )?;
                    if candidate.accepted_obligation_witness() != Some(fact) {
                        return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
                    }
                    Some(fact)
                }
                _ => {
                    return Err(
                        OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                    );
                }
            };

            let dominators = independent_reachable_dominators(function);
            let mut expected_incoming = Vec::new();
            for source in &function.blocks {
                for edge in source.successors().filter(|edge| edge.target == join.id) {
                    if edge.bindings.len() != join.parameters.len() {
                        return Err(
                            OptimizationUnitValidationError::CandidateIncomingBindingMismatch,
                        );
                    }
                    let mut translated = redundant.operation.clone();
                    for (parameter, binding) in join.parameters.iter().zip(&edge.bindings) {
                        if binding.parameter != parameter.value
                            || binding.scalar_type != parameter.scalar_type
                            || value_types.get(&binding.argument) != Some(&binding.scalar_type)
                        {
                            return Err(
                                OptimizationUnitValidationError::CandidateIncomingBindingMismatch,
                            );
                        }
                        rewrite_scalar_value_uses(
                            &mut translated,
                            parameter.value,
                            binding.argument,
                        );
                    }
                    let (translated_key, _, _, translated_type, _) = match proof_class {
                        ScalarCseProofClass::CompatiblePolicy => {
                            independent_compatible_policy_scalar_redundant(&translated)
                        }
                        _ => independent_cse_expression(&translated, &value_types, proof_class),
                    }
                    .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
                    let mut available_leaders = Vec::new();
                    let mut missing_leader_evidence = false;
                    for leader_block in &function.blocks {
                        for (node_index, node) in leader_block.nodes.iter().enumerate() {
                            let available = if leader_block.id == source.id {
                                node_index + 1 < source.nodes.len()
                            } else {
                                dominators
                                    .get(&source.id)
                                    .is_some_and(|rows| rows.contains(&leader_block.id))
                            };
                            if !available {
                                continue;
                            }
                            let Some((key, operation, result, scalar_type, obligation)) =
                                (match proof_class {
                                    ScalarCseProofClass::CompatiblePolicy => {
                                        independent_compatible_policy_scalar_leader(&node.operation)
                                    }
                                    _ => independent_cse_expression(
                                        &node.operation,
                                        &value_types,
                                        proof_class,
                                    ),
                                })
                            else {
                                continue;
                            };
                            let admitted = match (proof_class, obligation) {
                        (ScalarCseProofClass::ObligationFree, None) => !function
                            .facts
                            .iter()
                            .any(|fact| matches!(fact, OptimizationFact::OperationObligationReference { support, .. } if *support == operation)),
                        (ScalarCseProofClass::ProofCertified, Some(obligation)) => {
                            independently_accepted_operation_fact(
                                input,
                                function,
                                operation,
                                obligation,
                            )
                            .is_some()
                        }
                        (ScalarCseProofClass::CompatiblePolicy, None) => !function
                            .facts
                            .iter()
                            .any(|fact| matches!(fact, OptimizationFact::OperationObligationReference { support, .. } if *support == operation)),
                        _ => false,
                    };
                            if !admitted
                                && ((proof_class == ScalarCseProofClass::ProofCertified
                                    && obligation.is_some())
                                    || proof_class == ScalarCseProofClass::CompatiblePolicy)
                                && key == translated_key
                                && scalar_type == translated_type
                            {
                                missing_leader_evidence = true;
                            }
                            if admitted && key == translated_key && scalar_type == translated_type {
                                available_leaders.push((
                            NodeLocation {
                                machine: function.machine,
                                block: leader_block.id,
                                node: u32::try_from(node_index).map_err(|_| {
                                    OptimizationUnitValidationError::CandidateLocationMissing
                                })?,
                            },
                            operation,
                            result,
                            obligation,
                        ));
                            }
                        }
                    }
                    let canonical = available_leaders
                        .into_iter()
                        .min_by_key(|(location, _, _, _)| {
                            (
                                dominators
                                    .get(&location.block)
                                    .map_or(usize::MAX, BTreeSet::len),
                                *location,
                            )
                        })
                        .ok_or(if missing_leader_evidence {
                            OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch
                        } else {
                            OptimizationUnitValidationError::CandidatePatchMismatch
                        })?;
                    expected_incoming.push(PhiTranslatedScalarIncoming {
                        source: source.id,
                        edge: edge.psi_edge,
                        leader: canonical.0,
                        leader_operation: canonical.1,
                        leader_result: canonical.2,
                    });
                }
            }
            expected_incoming.sort_by_key(|row| (row.edge, row.source));
            if expected_incoming.len() < 2 || patch.incoming != expected_incoming {
                return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
            }
            let (expected_blocks, accepted_provenance) =
                reconstruct_phi_translated_cse_accounting(function, &patch)
                    .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;

            accept_region_rewrite(
        input,
        candidate,
        patch.redundant.machine,
        &expected_blocks,
        match proof_class {
            ScalarCseProofClass::ObligationFree => {
                b"omega.validator.phi-translated-obligation-free-total-scalar-gvn.v1".as_slice()
            }
            ScalarCseProofClass::ProofCertified => {
                b"omega.validator.phi-translated-proof-certified-total-scalar-gvn.v1".as_slice()
            }
            ScalarCseProofClass::CompatiblePolicy => {
                b"omega.validator.phi-translated-proof-certified-compatible-policy-scalar-gvn.v1"
                    .as_slice()
            }
        },
        accepted_provenance,
        |output| {
            let output_function = output
                .function_mut(patch.redundant.machine)
                .expect("candidate function exists");
            let output_join = output_function
                .blocks
                .iter_mut()
                .find(|row| row.id == patch.redundant.block)
                .expect("candidate join exists");
            output_join.parameters.push(ValueDefinition {
                value: patch.redundant_result,
                scalar_type: patch.scalar_type,
                site: ValueDefinitionSite::BlockParameter {
                    block: patch.redundant.block,
                    position: patch.parameter_position,
                },
            });
            retire_node_custody(output_join, redundant_index)?;
            for incoming in &patch.incoming {
                let source = output_function
                    .blocks
                    .iter_mut()
                    .find(|row| row.id == incoming.source)
                    .expect("incoming source exists");
                let node = source
                    .nodes
                    .iter_mut()
                    .find(|node| {
                        node.successors
                            .iter()
                            .any(|edge| edge.psi_edge == incoming.edge)
                    })
                    .expect("incoming edge exists");
                let edge = node.edge(incoming.edge).expect("incoming edge exists");
                let mut bindings = edge.bindings.clone();
                bindings.push(abstract_operations::ValueBinding {
                    parameter: patch.redundant_result,
                    argument: incoming.leader_result,
                    scalar_type: patch.scalar_type,
                });
                if !rewrite_successor_operation(
                    &mut node.operation,
                    incoming.edge,
                    patch.redundant.block,
                    &bindings,
                ) {
                    return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
                }
            }
            sweep_output_nodes(output_function, |_| Ok(()))?;
            Ok(())
        },
    )
        }
    }
    mod rule_catalog {
        use crate::OptimizationUnitValidationError;
        use optimization_core::{AnalysisKind, OptimizationRuleIdentity, OptimizationSafetyClass};

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(super) enum ScalarCseScope {
            SameBlock,
            Dominating,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(super) enum ScalarCseProofClass {
            ObligationFree,
            ProofCertified,
            CompatiblePolicy,
        }

        pub(super) const SAME_BLOCK_ANALYSES: &[AnalysisKind] =
            &[AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries];
        pub(super) const DOMINATING_ANALYSES: &[AnalysisKind] = &[
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::Dominators,
            AnalysisKind::UseDefinition,
            AnalysisKind::EffectSummaries,
        ];

        pub(super) fn safety_class(proof_class: ScalarCseProofClass) -> OptimizationSafetyClass {
            match proof_class {
                ScalarCseProofClass::ObligationFree => {
                    OptimizationSafetyClass::ExactOperationSemantics
                }
                ScalarCseProofClass::ProofCertified | ScalarCseProofClass::CompatiblePolicy => {
                    OptimizationSafetyClass::ProofCertified
                }
            }
        }

        pub(super) fn scoped_proof_class(
            scope: ScalarCseScope,
            rule: OptimizationRuleIdentity,
        ) -> Result<ScalarCseProofClass, OptimizationUnitValidationError> {
            let rows = match scope {
        ScalarCseScope::SameBlock => [
            (
                b"omega.psi-rule.same-block-obligation-free-total-scalar-cse.v1".as_slice(),
                ScalarCseProofClass::ObligationFree,
            ),
            (
                b"omega.psi-rule.same-block-proof-certified-total-scalar-cse.v1".as_slice(),
                ScalarCseProofClass::ProofCertified,
            ),
            (
                b"omega.psi-rule.same-block-proof-certified-compatible-policy-scalar-cse.v1"
                    .as_slice(),
                ScalarCseProofClass::CompatiblePolicy,
            ),
        ],
        ScalarCseScope::Dominating => [
            (
                b"omega.psi-rule.dominator-obligation-free-total-scalar-gvn.v1".as_slice(),
                ScalarCseProofClass::ObligationFree,
            ),
            (
                b"omega.psi-rule.dominator-proof-certified-total-scalar-gvn.v1".as_slice(),
                ScalarCseProofClass::ProofCertified,
            ),
            (
                b"omega.psi-rule.dominator-proof-certified-compatible-policy-scalar-gvn.v1"
                    .as_slice(),
                ScalarCseProofClass::CompatiblePolicy,
            ),
        ],
    };
            proof_class_from_rows(rule, &rows)
        }

        pub(super) fn phi_translated_proof_class(
            rule: OptimizationRuleIdentity,
        ) -> Result<ScalarCseProofClass, OptimizationUnitValidationError> {
            proof_class_from_rows(
        rule,
        &[
            (
                b"omega.psi-rule.phi-translated-obligation-free-total-scalar-gvn.v1".as_slice(),
                ScalarCseProofClass::ObligationFree,
            ),
            (
                b"omega.psi-rule.phi-translated-proof-certified-total-scalar-gvn.v1".as_slice(),
                ScalarCseProofClass::ProofCertified,
            ),
            (
                b"omega.psi-rule.phi-translated-proof-certified-compatible-policy-scalar-gvn.v1"
                    .as_slice(),
                ScalarCseProofClass::CompatiblePolicy,
            ),
        ],
    )
        }

        fn proof_class_from_rows(
            rule: OptimizationRuleIdentity,
            rows: &[(&[u8], ScalarCseProofClass)],
        ) -> Result<ScalarCseProofClass, OptimizationUnitValidationError> {
            rows.iter()
                .find_map(|(identity, proof_class)| {
                    (rule == OptimizationRuleIdentity::from_canonical_bytes(identity))
                        .then_some(*proof_class)
                })
                .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)
        }
    }
    mod total_scalar_identity {
        mod application {
            use crate::candidates::accept_region_rewrite;
            use crate::candidates::retire_node_custody;
            use crate::candidates::rewrite_accounting::substitutions::rewrite_scalar_value_uses;
            use crate::candidates::sweep_output_nodes;
            use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
            use optimization_unit::{
                ProvenanceRewrite, PsiOptimizationUnit, PsiRewriteCandidate,
                TotalScalarIdentityRewrite,
            };
            use semantic_vocabulary::BlockId;

            pub(super) fn independently_apply_total_scalar_identity(
                input: &PsiOptimizationUnit,
                candidate: &PsiRewriteCandidate,
                patch: TotalScalarIdentityRewrite,
                node_index: usize,
                affected_blocks: &[BlockId],
                provenance: Vec<ProvenanceRewrite>,
                validator: &[u8],
            ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
                accept_region_rewrite(
                    input,
                    candidate,
                    patch.location.machine,
                    affected_blocks,
                    validator,
                    provenance,
                    |output| {
                        if output.accepted_obligation_facts != input.accepted_obligation_facts {
                            return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
                        }
                        let output_function = output
                            .function_mut(patch.location.machine)
                            .expect("validated candidate function exists");
                        let output_block = output_function
                            .block_mut(patch.location.block)
                            .expect("validated candidate block exists");
                        retire_node_custody(output_block, node_index)?;

                        sweep_output_nodes(output_function, |node| {
                            rewrite_scalar_value_uses(
                                &mut node.operation,
                                patch.result,
                                patch.replacement,
                            );
                            Ok(())
                        })?;
                        Ok(())
                    },
                )
            }
        }
        mod classification {
            use abstract_operations::AbstractOperation as O;
            use optimization_unit::TotalScalarIdentityKind;
            use semantic_vocabulary::{
                IntegerSign, IntegerType, IntegerValue, OperationId, ValueId,
            };

            mod bitwise_absorbing {
                use abstract_operations::AbstractOperation as O;
                use optimization_unit::TotalScalarIdentityKind;

                use super::{
                    BitwiseFamily, IndependentTotalScalarIdentity, all_ones, bitwise_operands, row,
                    typed_integer,
                };

                pub(super) fn classify(
                    operation: &O,
                    identity: TotalScalarIdentityKind,
                ) -> Option<IndependentTotalScalarIdentity> {
                    use TotalScalarIdentityKind as Kind;
                    let operands = bitwise_operands(operation)?;
                    let zero = typed_integer(operands.scalar_type, 0);
                    let ones = all_ones(operands.scalar_type);
                    let (operand, law_constant) = match (operands.family, identity) {
                        (BitwiseFamily::And, Kind::IntegerBitwiseAndZeroLeft) => {
                            (operands.left, zero)
                        }
                        (BitwiseFamily::And, Kind::IntegerBitwiseAndZeroRight) => {
                            (operands.right, zero)
                        }
                        (BitwiseFamily::Or, Kind::IntegerBitwiseOrAllOnesLeft) => {
                            (operands.left, ones)
                        }
                        (BitwiseFamily::Or, Kind::IntegerBitwiseOrAllOnesRight) => {
                            (operands.right, ones)
                        }
                        _ => return None,
                    };
                    Some(row(
                        operands.source_operation,
                        operands.result,
                        operand,
                        operand,
                        operands.scalar_type,
                        operands.scalar_type,
                        law_constant,
                    ))
                }
            }
            mod bitwise_neutral {
                use abstract_operations::AbstractOperation as O;
                use optimization_unit::TotalScalarIdentityKind;

                use super::{
                    BitwiseFamily, BitwiseSide, IndependentTotalScalarIdentity, all_ones,
                    bitwise_law_row, bitwise_operands, typed_integer,
                };

                pub(super) fn classify(
                    operation: &O,
                    identity: TotalScalarIdentityKind,
                ) -> Option<IndependentTotalScalarIdentity> {
                    use TotalScalarIdentityKind as Kind;
                    let operands = bitwise_operands(operation)?;
                    let zero = typed_integer(operands.scalar_type, 0);
                    let ones = all_ones(operands.scalar_type);
                    let (law_side, law_constant) = match (operands.family, identity) {
                        (BitwiseFamily::And, Kind::IntegerBitwiseAndAllOnesLeft) => {
                            (BitwiseSide::Left, ones)
                        }
                        (BitwiseFamily::And, Kind::IntegerBitwiseAndAllOnesRight) => {
                            (BitwiseSide::Right, ones)
                        }
                        (BitwiseFamily::Or, Kind::IntegerBitwiseOrZeroLeft) => {
                            (BitwiseSide::Left, zero)
                        }
                        (BitwiseFamily::Or, Kind::IntegerBitwiseOrZeroRight) => {
                            (BitwiseSide::Right, zero)
                        }
                        (BitwiseFamily::Xor, Kind::IntegerBitwiseXorZeroLeft) => {
                            (BitwiseSide::Left, zero)
                        }
                        (BitwiseFamily::Xor, Kind::IntegerBitwiseXorZeroRight) => {
                            (BitwiseSide::Right, zero)
                        }
                        _ => return None,
                    };
                    Some(bitwise_law_row(operands, law_side, law_constant))
                }
            }
            mod saturating {
                use abstract_operations::{
                    AbstractOperation as O, IntegerArithmeticOperator, IntegerOverflowPolicy,
                };
                use optimization_unit::TotalScalarIdentityKind;

                use super::{
                    IndependentTotalScalarIdentity, left_law_row, right_law_row, row, typed_integer,
                };

                pub(super) fn classify(
                    operation: &O,
                    identity: TotalScalarIdentityKind,
                ) -> Option<IndependentTotalScalarIdentity> {
                    use IntegerArithmeticOperator as Operator;
                    use TotalScalarIdentityKind as Kind;
                    let operands = operation.integer_arithmetic()?;
                    if operands.arithmetic.policy() != IntegerOverflowPolicy::Saturating {
                        return None;
                    }
                    let (psi_operation, result, scalar_type, left, right) = (
                        operands.psi_operation,
                        operands.result,
                        operands.scalar_type,
                        operands.left,
                        operands.right,
                    );
                    match (operands.arithmetic.operator(), identity) {
                        (Operator::Add, Kind::SaturatingIntegerAddZeroLeft) => Some(left_law_row(
                            psi_operation,
                            result,
                            scalar_type,
                            left,
                            right,
                            typed_integer(scalar_type, 0),
                        )),
                        (Operator::Add, Kind::SaturatingIntegerAddZeroRight)
                        | (Operator::Subtract, Kind::SaturatingIntegerSubtractZeroRight) => {
                            Some(right_law_row(
                                psi_operation,
                                result,
                                scalar_type,
                                left,
                                right,
                                typed_integer(scalar_type, 0),
                            ))
                        }
                        (Operator::Multiply, Kind::SaturatingIntegerMultiplyOneLeft) => {
                            Some(left_law_row(
                                psi_operation,
                                result,
                                scalar_type,
                                left,
                                right,
                                typed_integer(scalar_type, 1),
                            ))
                        }
                        (Operator::Multiply, Kind::SaturatingIntegerMultiplyOneRight) => {
                            Some(right_law_row(
                                psi_operation,
                                result,
                                scalar_type,
                                left,
                                right,
                                typed_integer(scalar_type, 1),
                            ))
                        }
                        (Operator::Multiply, Kind::SaturatingIntegerMultiplyZeroLeft) => Some(row(
                            psi_operation,
                            result,
                            left,
                            left,
                            scalar_type,
                            scalar_type,
                            typed_integer(scalar_type, 0),
                        )),
                        (Operator::Multiply, Kind::SaturatingIntegerMultiplyZeroRight) => {
                            Some(row(
                                psi_operation,
                                result,
                                right,
                                right,
                                scalar_type,
                                scalar_type,
                                typed_integer(scalar_type, 0),
                            ))
                        }
                        _ => None,
                    }
                }
            }
            mod wrapping {
                use abstract_operations::{
                    AbstractOperation as O, IntegerArithmeticOperator, IntegerOverflowPolicy,
                    IntegerShiftDirection, IntegerShiftPolicy,
                };
                use optimization_unit::TotalScalarIdentityKind;

                use super::{
                    IndependentTotalScalarIdentity, left_law_row, right_law_row, row, typed_integer,
                };

                pub(super) fn classify(
                    operation: &O,
                    identity: TotalScalarIdentityKind,
                ) -> Option<IndependentTotalScalarIdentity> {
                    use IntegerArithmeticOperator as Operator;
                    use TotalScalarIdentityKind as Kind;
                    match operation {
                        O::IntegerArithmetic {
                            psi_operation,
                            result,
                            scalar_type,
                            arithmetic,
                            left,
                            right,
                        } if arithmetic.policy() == IntegerOverflowPolicy::Wrapping => {
                            let (psi_operation, result, scalar_type, left, right) =
                                (*psi_operation, *result, *scalar_type, *left, *right);
                            match (arithmetic.operator(), identity) {
                                (Operator::Add, Kind::WrappingIntegerAddZeroLeft) => {
                                    Some(left_law_row(
                                        psi_operation,
                                        result,
                                        scalar_type,
                                        left,
                                        right,
                                        typed_integer(scalar_type, 0),
                                    ))
                                }
                                (Operator::Add, Kind::WrappingIntegerAddZeroRight)
                                | (Operator::Subtract, Kind::WrappingIntegerSubtractZeroRight) => {
                                    Some(right_law_row(
                                        psi_operation,
                                        result,
                                        scalar_type,
                                        left,
                                        right,
                                        typed_integer(scalar_type, 0),
                                    ))
                                }
                                (Operator::Multiply, Kind::WrappingIntegerMultiplyOneLeft) => {
                                    Some(left_law_row(
                                        psi_operation,
                                        result,
                                        scalar_type,
                                        left,
                                        right,
                                        typed_integer(scalar_type, 1),
                                    ))
                                }
                                (Operator::Multiply, Kind::WrappingIntegerMultiplyOneRight) => {
                                    Some(right_law_row(
                                        psi_operation,
                                        result,
                                        scalar_type,
                                        left,
                                        right,
                                        typed_integer(scalar_type, 1),
                                    ))
                                }
                                (Operator::Multiply, Kind::WrappingIntegerMultiplyZeroLeft) => {
                                    Some(row(
                                        psi_operation,
                                        result,
                                        left,
                                        left,
                                        scalar_type,
                                        scalar_type,
                                        typed_integer(scalar_type, 0),
                                    ))
                                }
                                (Operator::Multiply, Kind::WrappingIntegerMultiplyZeroRight) => {
                                    Some(row(
                                        psi_operation,
                                        result,
                                        right,
                                        right,
                                        scalar_type,
                                        scalar_type,
                                        typed_integer(scalar_type, 0),
                                    ))
                                }
                                _ => None,
                            }
                        }
                        O::IntegerShift {
                            psi_operation,
                            result,
                            value_type,
                            count_type,
                            policy: IntegerShiftPolicy::Wrapping,
                            direction,
                            value,
                            count,
                        } => match (direction, identity) {
                            (
                                IntegerShiftDirection::Left,
                                Kind::WrappingIntegerShiftLeftZeroCount,
                            )
                            | (
                                IntegerShiftDirection::Right,
                                Kind::WrappingIntegerShiftRightZeroCount,
                            ) => Some(row(
                                *psi_operation,
                                *result,
                                *value,
                                *count,
                                *value_type,
                                *count_type,
                                typed_integer(*count_type, 0),
                            )),
                            _ => None,
                        },
                        _ => None,
                    }
                }
            }
            pub(super) fn independently_classify_total_scalar_identity(
                operation: &O,
                identity: TotalScalarIdentityKind,
            ) -> Option<IndependentTotalScalarIdentity> {
                match identity {
                    TotalScalarIdentityKind::WrappingIntegerAddZeroLeft
                    | TotalScalarIdentityKind::WrappingIntegerAddZeroRight
                    | TotalScalarIdentityKind::WrappingIntegerSubtractZeroRight
                    | TotalScalarIdentityKind::WrappingIntegerMultiplyOneLeft
                    | TotalScalarIdentityKind::WrappingIntegerMultiplyOneRight
                    | TotalScalarIdentityKind::WrappingIntegerShiftLeftZeroCount
                    | TotalScalarIdentityKind::WrappingIntegerShiftRightZeroCount
                    | TotalScalarIdentityKind::WrappingIntegerMultiplyZeroLeft
                    | TotalScalarIdentityKind::WrappingIntegerMultiplyZeroRight => {
                        wrapping::classify(operation, identity)
                    }
                    TotalScalarIdentityKind::SaturatingIntegerAddZeroLeft
                    | TotalScalarIdentityKind::SaturatingIntegerAddZeroRight
                    | TotalScalarIdentityKind::SaturatingIntegerSubtractZeroRight
                    | TotalScalarIdentityKind::SaturatingIntegerMultiplyOneLeft
                    | TotalScalarIdentityKind::SaturatingIntegerMultiplyOneRight
                    | TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroLeft
                    | TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroRight => {
                        saturating::classify(operation, identity)
                    }
                    TotalScalarIdentityKind::IntegerBitwiseAndAllOnesLeft
                    | TotalScalarIdentityKind::IntegerBitwiseAndAllOnesRight
                    | TotalScalarIdentityKind::IntegerBitwiseOrZeroLeft
                    | TotalScalarIdentityKind::IntegerBitwiseOrZeroRight
                    | TotalScalarIdentityKind::IntegerBitwiseXorZeroLeft
                    | TotalScalarIdentityKind::IntegerBitwiseXorZeroRight => {
                        bitwise_neutral::classify(operation, identity)
                    }
                    TotalScalarIdentityKind::IntegerBitwiseAndZeroLeft
                    | TotalScalarIdentityKind::IntegerBitwiseAndZeroRight
                    | TotalScalarIdentityKind::IntegerBitwiseOrAllOnesLeft
                    | TotalScalarIdentityKind::IntegerBitwiseOrAllOnesRight => {
                        bitwise_absorbing::classify(operation, identity)
                    }
                }
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub(crate) struct IndependentTotalScalarIdentity {
                pub source_operation: OperationId,
                pub result: ValueId,
                pub replacement: ValueId,
                pub law_operand: ValueId,
                pub scalar_type: IntegerType,
                pub law_operand_type: IntegerType,
                pub law_constant: IntegerValue,
            }

            fn row(
                source_operation: OperationId,
                result: ValueId,
                replacement: ValueId,
                law_operand: ValueId,
                scalar_type: IntegerType,
                law_operand_type: IntegerType,
                law_constant: IntegerValue,
            ) -> IndependentTotalScalarIdentity {
                IndependentTotalScalarIdentity {
                    source_operation,
                    result,
                    replacement,
                    law_operand,
                    scalar_type,
                    law_operand_type,
                    law_constant,
                }
            }

            fn left_law_row(
                source_operation: OperationId,
                result: ValueId,
                scalar_type: IntegerType,
                left: ValueId,
                right: ValueId,
                law_constant: IntegerValue,
            ) -> IndependentTotalScalarIdentity {
                row(
                    source_operation,
                    result,
                    right,
                    left,
                    scalar_type,
                    scalar_type,
                    law_constant,
                )
            }

            fn right_law_row(
                source_operation: OperationId,
                result: ValueId,
                scalar_type: IntegerType,
                left: ValueId,
                right: ValueId,
                law_constant: IntegerValue,
            ) -> IndependentTotalScalarIdentity {
                row(
                    source_operation,
                    result,
                    left,
                    right,
                    scalar_type,
                    scalar_type,
                    law_constant,
                )
            }

            struct BitwiseOperands {
                family: BitwiseFamily,
                source_operation: OperationId,
                result: ValueId,
                scalar_type: IntegerType,
                left: ValueId,
                right: ValueId,
            }

            #[derive(Clone, Copy)]
            enum BitwiseFamily {
                And,
                Or,
                Xor,
            }

            #[derive(Clone, Copy)]
            enum BitwiseSide {
                Left,
                Right,
            }

            fn bitwise_operands(operation: &O) -> Option<BitwiseOperands> {
                let family = match operation {
                    O::IntegerBitwiseAnd { .. } => BitwiseFamily::And,
                    O::IntegerBitwiseOr { .. } => BitwiseFamily::Or,
                    O::IntegerBitwiseXor { .. } => BitwiseFamily::Xor,
                    _ => return None,
                };
                let (O::IntegerBitwiseAnd {
                    psi_operation,
                    result,
                    scalar_type,
                    left,
                    right,
                }
                | O::IntegerBitwiseOr {
                    psi_operation,
                    result,
                    scalar_type,
                    left,
                    right,
                }
                | O::IntegerBitwiseXor {
                    psi_operation,
                    result,
                    scalar_type,
                    left,
                    right,
                }) = operation
                else {
                    return None;
                };
                Some(BitwiseOperands {
                    family,
                    source_operation: *psi_operation,
                    result: *result,
                    scalar_type: *scalar_type,
                    left: *left,
                    right: *right,
                })
            }

            fn bitwise_law_row(
                operands: BitwiseOperands,
                law_side: BitwiseSide,
                law_constant: IntegerValue,
            ) -> IndependentTotalScalarIdentity {
                let (replacement, law_operand) = match law_side {
                    BitwiseSide::Left => (operands.right, operands.left),
                    BitwiseSide::Right => (operands.left, operands.right),
                };
                row(
                    operands.source_operation,
                    operands.result,
                    replacement,
                    law_operand,
                    operands.scalar_type,
                    operands.scalar_type,
                    law_constant,
                )
            }

            fn typed_integer(scalar_type: IntegerType, value: u128) -> IntegerValue {
                match scalar_type.sign() {
                    IntegerSign::Signed => IntegerValue::Signed(value as i128),
                    IntegerSign::Unsigned => IntegerValue::Unsigned(value),
                }
            }

            fn all_ones(scalar_type: IntegerType) -> IntegerValue {
                match scalar_type.sign() {
                    IntegerSign::Signed => IntegerValue::Signed(-1),
                    IntegerSign::Unsigned => scalar_type.maximum_value(),
                }
            }
        }
        mod evidence {
            use crate::OptimizationUnitValidationError;
            use crate::candidates::proof_check_elision::materialized_integer_constant;
            use crate::candidates::sparse_conditional_constant_propagation::scalar_value_definition;
            use optimization_unit::{
                OptimizationFact, PsiOptimizationFunction, PsiOptimizationUnit,
                PsiRewriteCandidate, ScalarConstantValue, literal_scalar_constant_fact_identity,
            };
            use semantic_vocabulary::ScalarType;

            use super::classification::IndependentTotalScalarIdentity;

            pub(super) fn independently_validate_law_literal(
                input: &PsiOptimizationUnit,
                function: &PsiOptimizationFunction,
                shape: IndependentTotalScalarIdentity,
                candidate: &PsiRewriteCandidate,
            ) -> Result<(), OptimizationUnitValidationError> {
                let definition = scalar_value_definition(function, shape.law_operand)
                    .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
                if definition.scalar_type != ScalarType::Integer(shape.law_operand_type) {
                    return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
                }
                let literal = materialized_integer_constant(function, definition)?;
                if literal.result != shape.law_operand
                    || literal.scalar_type != ScalarType::Integer(shape.law_operand_type)
                    || literal.value != shape.law_constant
                    || !function.facts.iter().any(|fact| {
                        matches!(fact, OptimizationFact::IntegerConstant {
                value: fact_value,
                constant,
                support: fact_support,
            } if *fact_value == shape.law_operand
                && *constant == shape.law_constant
                && *fact_support == literal.operation)
                    })
                {
                    return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
                }
                let expected = literal_scalar_constant_fact_identity(
                    input.identity,
                    function.machine,
                    definition,
                    ScalarConstantValue::Integer(shape.law_constant),
                    literal.operation,
                )
                .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
                if candidate.total_scalar_identity_witness() != Some(expected)
                    || candidate.accepted_obligation_witness().is_some()
                {
                    return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
                }
                Ok(())
            }
        }
        mod validation {
            use crate::candidates::contract;
            use crate::candidates::rewrite_accounting::scalar_identity::reconstruct_total_scalar_identity_accounting;
            use crate::candidates::sparse_conditional_constant_propagation::scalar_value_definition;
            use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
            use optimization_core::{
                AnalysisKind, OptimizationRuleIdentity, OptimizationSafetyClass,
            };
            use optimization_unit::{
                OptimizationFact, PsiOptimizationUnit, PsiRewriteCandidate, PsiRewritePatch,
                ScalarSubstitution, TotalScalarIdentityKind,
            };
            use semantic_vocabulary::ScalarType;

            use super::application::independently_apply_total_scalar_identity;
            use super::classification::independently_classify_total_scalar_identity;
            use super::evidence::independently_validate_law_literal;
            use crate::candidates::{candidate_block, candidate_function};
            use optimization_unit::{receiver_custody_distinct, retirable_scalar_node};

            const NEUTRAL_ARITHMETIC_RULE_DOMAIN: &[u8] =
    b"omega.psi-rule.live-obligation-free-wrapping-integer-neutral-arithmetic-identity-elimination.v1";
            const SHIFT_ZERO_COUNT_RULE_DOMAIN: &[u8] =
    b"omega.psi-rule.live-obligation-free-wrapping-integer-shift-zero-count-elimination.v1";
            const MULTIPLY_ZERO_ANNIHILATION_RULE_DOMAIN: &[u8] =
    b"omega.psi-rule.live-obligation-free-wrapping-integer-multiply-zero-annihilation.v1";
            const SATURATING_NEUTRAL_ARITHMETIC_RULE_DOMAIN: &[u8] =
    b"omega.psi-rule.live-obligation-free-saturating-integer-neutral-arithmetic-identity-elimination.v1";
            const SATURATING_MULTIPLY_ZERO_ANNIHILATION_RULE_DOMAIN: &[u8] =
    b"omega.psi-rule.live-obligation-free-saturating-integer-multiply-zero-annihilation.v1";
            const BITWISE_NEUTRAL_LITERAL_RULE_DOMAIN: &[u8] =
    b"omega.psi-rule.live-obligation-free-integer-bitwise-neutral-literal-elimination.v1";
            const BITWISE_ABSORBING_LITERAL_RULE_DOMAIN: &[u8] =
    b"omega.psi-rule.live-obligation-free-integer-bitwise-absorbing-literal-elimination.v1";

            pub(crate) fn validate_total_scalar_identity_candidate(
                input: &PsiOptimizationUnit,
                candidate: &PsiRewriteCandidate,
            ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
                contract::admit_origin(input, candidate)?;
                contract::admit_roster(
                    candidate,
                    &[
                        AnalysisKind::ScalarConstants,
                        AnalysisKind::UseDefinition,
                        AnalysisKind::EffectSummaries,
                    ],
                    contract::USE_DEFINITION_AND_EFFECT_SUMMARIES,
                    OptimizationSafetyClass::ExactOperationSemantics,
                    Some(-1),
                )?;
                candidate_patch!(patch, EliminateTotalScalarIdentity, candidate);
                let validator = exact_rule_validator_bytes(candidate.rule(), patch.identity)
                    .ok_or(OptimizationUnitValidationError::CandidateAnalysisContractMismatch)?;
                if candidate.node_decision_point() != Some(patch.location)
                    || candidate.substitutions()
                        != [ScalarSubstitution {
                            from: patch.result,
                            to: patch.replacement,
                            scalar_type: ScalarType::Integer(patch.scalar_type),
                        }]
                {
                    return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                }
                let function = candidate_function(input, patch.location.machine)?;
                let block = candidate_block(function, patch.location.block)?;
                let node_index = usize::try_from(patch.location.node)
                    .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
                let node = crate::candidates::candidate_node(block, patch.location.node)?;
                let scalar_type = ScalarType::Integer(patch.scalar_type);
                let shape =
                    independently_classify_total_scalar_identity(&node.operation, patch.identity)
                        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
                if shape.source_operation != patch.source_operation
                    || shape.result != patch.result
                    || shape.replacement != patch.replacement
                    || shape.scalar_type != patch.scalar_type
                    || !retirable_scalar_node(block, node_index, node, patch.result, scalar_type)
                    || !function
                        .node_uses()
                        .any(|use_site| use_site.value == patch.result)
                    || scalar_value_definition(function, shape.replacement).is_none_or(
                        |definition| {
                            definition.scalar_type != ScalarType::Integer(shape.scalar_type)
                        },
                    ) || function.facts.iter().any(|fact| {
                    matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                if *support == shape.source_operation)
                }) {
                    return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                }
                independently_validate_law_literal(input, function, shape, candidate)?;
                if !receiver_custody_distinct(block, node_index, node) {
                    return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
                }
                let (affected_blocks, provenance) =
                    reconstruct_total_scalar_identity_accounting(function, patch)
                        .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
                independently_apply_total_scalar_identity(
                    input,
                    candidate,
                    patch,
                    node_index,
                    &affected_blocks,
                    provenance,
                    validator,
                )
            }

            fn exact_rule_validator_bytes(
                rule: OptimizationRuleIdentity,
                identity: TotalScalarIdentityKind,
            ) -> Option<&'static [u8]> {
                let (rule_domain, validator_domain) = match identity {
        TotalScalarIdentityKind::WrappingIntegerAddZeroLeft
        | TotalScalarIdentityKind::WrappingIntegerAddZeroRight
        | TotalScalarIdentityKind::WrappingIntegerSubtractZeroRight
        | TotalScalarIdentityKind::WrappingIntegerMultiplyOneLeft
        | TotalScalarIdentityKind::WrappingIntegerMultiplyOneRight => (
            NEUTRAL_ARITHMETIC_RULE_DOMAIN,
            b"omega.validator.live-obligation-free-wrapping-integer-neutral-arithmetic-identity-elimination.v1"
                .as_slice(),
        ),
        TotalScalarIdentityKind::WrappingIntegerShiftLeftZeroCount
        | TotalScalarIdentityKind::WrappingIntegerShiftRightZeroCount => (
            SHIFT_ZERO_COUNT_RULE_DOMAIN,
            b"omega.validator.live-obligation-free-wrapping-integer-shift-zero-count-elimination.v1"
                .as_slice(),
        ),
        TotalScalarIdentityKind::WrappingIntegerMultiplyZeroLeft
        | TotalScalarIdentityKind::WrappingIntegerMultiplyZeroRight => (
            MULTIPLY_ZERO_ANNIHILATION_RULE_DOMAIN,
            b"omega.validator.live-obligation-free-wrapping-integer-multiply-zero-annihilation.v1"
                .as_slice(),
        ),
        TotalScalarIdentityKind::SaturatingIntegerAddZeroLeft
        | TotalScalarIdentityKind::SaturatingIntegerAddZeroRight
        | TotalScalarIdentityKind::SaturatingIntegerSubtractZeroRight
        | TotalScalarIdentityKind::SaturatingIntegerMultiplyOneLeft
        | TotalScalarIdentityKind::SaturatingIntegerMultiplyOneRight => (
            SATURATING_NEUTRAL_ARITHMETIC_RULE_DOMAIN,
            b"omega.validator.live-obligation-free-saturating-integer-neutral-arithmetic-identity-elimination.v1"
                .as_slice(),
        ),
        TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroLeft
        | TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroRight => (
            SATURATING_MULTIPLY_ZERO_ANNIHILATION_RULE_DOMAIN,
            b"omega.validator.live-obligation-free-saturating-integer-multiply-zero-annihilation.v1"
                .as_slice(),
        ),
        TotalScalarIdentityKind::IntegerBitwiseAndAllOnesLeft
        | TotalScalarIdentityKind::IntegerBitwiseAndAllOnesRight
        | TotalScalarIdentityKind::IntegerBitwiseOrZeroLeft
        | TotalScalarIdentityKind::IntegerBitwiseOrZeroRight
        | TotalScalarIdentityKind::IntegerBitwiseXorZeroLeft
        | TotalScalarIdentityKind::IntegerBitwiseXorZeroRight => (
            BITWISE_NEUTRAL_LITERAL_RULE_DOMAIN,
            b"omega.validator.live-obligation-free-integer-bitwise-neutral-literal-elimination.v1"
                .as_slice(),
        ),
        TotalScalarIdentityKind::IntegerBitwiseAndZeroLeft
        | TotalScalarIdentityKind::IntegerBitwiseAndZeroRight
        | TotalScalarIdentityKind::IntegerBitwiseOrAllOnesLeft
        | TotalScalarIdentityKind::IntegerBitwiseOrAllOnesRight => (
            BITWISE_ABSORBING_LITERAL_RULE_DOMAIN,
            b"omega.validator.live-obligation-free-integer-bitwise-absorbing-literal-elimination.v1"
                .as_slice(),
        ),
    };
                (rule == OptimizationRuleIdentity::from_canonical_bytes(rule_domain))
                    .then_some(validator_domain)
            }
        }
        pub(crate) use validation::validate_total_scalar_identity_candidate;
    }
    pub(crate) use total_scalar_identity::validate_total_scalar_identity_candidate;

    use rule_catalog::{
        ScalarCseProofClass, ScalarCseScope, phi_translated_proof_class, scoped_proof_class,
    };

    pub(crate) use admission::independently_accepted_operation_fact;
    pub(crate) use dominance_reconstruction::{
        independent_reachable_dominators, independently_replacement_dominates_uses,
    };

    pub(super) fn scalar_value_types(
        function: &PsiOptimizationFunction,
    ) -> BTreeMap<ValueId, ScalarType> {
        function
            .parameters
            .iter()
            .map(|row| (row.value, row.scalar_type))
            .chain(function.blocks.iter().flat_map(|block| {
                block
                    .parameters
                    .iter()
                    .map(|row| (row.value, row.scalar_type))
            }))
            .chain(function.blocks.iter().flat_map(|block| {
                block.nodes.iter().flat_map(|node| {
                    node.definitions
                        .iter()
                        .map(|row| (row.value, row.scalar_type))
                })
            }))
            .collect()
    }

    pub(crate) fn validate_local_scalar_common_subexpression_candidate(
        input: &PsiOptimizationUnit,
        candidate: &PsiRewriteCandidate,
    ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
        validate_candidate_origin(input, candidate)?;
        let scope = ScalarCseScope::SameBlock;
        let proof_class = scoped_proof_class(scope, candidate.rule())?;
        local_and_dominating::validate_scalar_common_subexpression_candidate(
            input,
            candidate,
            scope,
            proof_class,
        )
    }

    pub(crate) fn validate_dominating_scalar_common_subexpression_candidate(
        input: &PsiOptimizationUnit,
        candidate: &PsiRewriteCandidate,
    ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
        validate_candidate_origin(input, candidate)?;
        let scope = ScalarCseScope::Dominating;
        let proof_class = scoped_proof_class(scope, candidate.rule())?;
        local_and_dominating::validate_scalar_common_subexpression_candidate(
            input,
            candidate,
            scope,
            proof_class,
        )
    }

    pub(crate) fn validate_phi_translated_scalar_common_subexpression_candidate(
        input: &PsiOptimizationUnit,
        candidate: &PsiRewriteCandidate,
    ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
        validate_candidate_origin(input, candidate)?;
        let proof_class = phi_translated_proof_class(candidate.rule())?;
        phi_translated::validate_phi_translated_scalar_common_subexpression_candidate(
            input,
            candidate,
            proof_class,
        )
    }

    fn validate_candidate_origin(
        input: &PsiOptimizationUnit,
        candidate: &PsiRewriteCandidate,
    ) -> Result<(), OptimizationUnitValidationError> {
        contract::admit_origin(input, candidate)
    }
}
pub(crate) mod observation {
    use optimization_unit::{NodeLocation, PsiOptimizationUnit};
    use semantic_vocabulary::ValueId;
    use std::collections::{BTreeMap, BTreeSet};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ClosedScalarObservationBoundary {
        pub location: NodeLocation,
        pub live_in: Vec<ValueId>,
        pub live_out: Vec<ValueId>,
    }

    pub(crate) fn reconstruct_closed_scalar_node_boundary(
        unit: &PsiOptimizationUnit,
        location: NodeLocation,
    ) -> Option<ClosedScalarObservationBoundary> {
        let function = unit.function(location.machine)?;
        let mut live_entry = function
            .blocks
            .iter()
            .map(|block| (block.id, BTreeSet::new()))
            .collect::<BTreeMap<_, _>>();
        let mut live_exit = live_entry.clone();
        loop {
            let mut changed = false;
            for block in function.blocks.iter().rev() {
                let next_exit = block
                    .nodes
                    .last()
                    .into_iter()
                    .flat_map(|node| &node.successors)
                    .filter_map(|edge| live_entry.get(&edge.target))
                    .flat_map(|values| values.iter().copied())
                    .collect::<BTreeSet<_>>();
                let mut next_entry = next_exit.clone();
                for node in block.nodes.iter().rev() {
                    for definition in &node.definitions {
                        next_entry.remove(&definition.value);
                    }
                    next_entry.extend(node.uses.iter().map(|use_site| use_site.value));
                }
                for parameter in &block.parameters {
                    next_entry.remove(&parameter.value);
                }
                if live_exit[&block.id] != next_exit {
                    live_exit.insert(block.id, next_exit);
                    changed = true;
                }
                if live_entry[&block.id] != next_entry {
                    live_entry.insert(block.id, next_entry);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }

        let block = function.block(location.block)?;
        let target = usize::try_from(location.node).ok()?;
        if target >= block.nodes.len() {
            return None;
        }
        let mut live = live_exit[&block.id].clone();
        for (node_index, node) in block.nodes.iter().enumerate().rev() {
            let live_out = live.clone();
            for definition in &node.definitions {
                live.remove(&definition.value);
            }
            live.extend(node.uses.iter().map(|use_site| use_site.value));
            if node_index == target {
                return Some(ClosedScalarObservationBoundary {
                    location,
                    live_in: live.iter().copied().collect(),
                    live_out: live_out.iter().copied().collect(),
                });
            }
        }
        None
    }
}
mod outside_region {
    use optimization_unit::{OptimizationBlock, PsiOptimizationFunction};
    use semantic_vocabulary::BlockId;

    use crate::OptimizationUnitValidationError;

    pub(super) fn validate_outside_region<'input>(
        input_blocks: impl IntoIterator<Item = &'input OptimizationBlock>,
        output_function: &PsiOptimizationFunction,
        expected_blocks: &[BlockId],
    ) -> Result<(), OptimizationUnitValidationError> {
        for input_block in input_blocks {
            if !expected_blocks.contains(&input_block.id)
                && output_function.block(input_block.id) != Some(input_block)
            {
                return Err(OptimizationUnitValidationError::CandidateOutsideRegionMismatch);
            }
        }
        Ok(())
    }
}
pub(crate) mod proof_check_elision {
    use crate::{
        OptimizationUnitValidationError, ValidatedPsiRewrite, validate_dead_scalar_node_candidate,
    };
    use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

    mod candidate_validation {
        use crate::candidates::rewrite_accounting::scalar_identity::reconstruct_proof_certified_scalar_identity_accounting;
        use crate::candidates::rewrite_accounting::substitutions::rewrite_scalar_value_uses;
        use crate::candidates::sparse_conditional_constant_propagation::scalar_value_definition;
        use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
        use optimization_core::{AnalysisKind, OptimizationSafetyClass};
        use optimization_unit::{
            OptimizationFact, PsiOptimizationUnit, PsiRewriteCandidate, PsiRewritePatch,
            ScalarConstantValue, ScalarSubstitution, literal_scalar_constant_fact_identity,
        };
        use semantic_vocabulary::ScalarType;

        use super::identity_classification::independent_proof_certified_scalar_identity;
        use super::literal_operand::materialized_integer_constant;
        use super::rule_catalog::{is_operand_substitution_rule, operand_substitution_row};
        use crate::candidates::{
            accept_region_rewrite, candidate_block, candidate_function, contract,
            retire_node_custody, sweep_output_nodes,
        };
        use optimization_unit::{receiver_custody_distinct, retirable_scalar_node};

        pub(crate) fn validate_proof_certified_scalar_identity_candidate(
            input: &PsiOptimizationUnit,
            candidate: &PsiRewriteCandidate,
        ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
            contract::admit_origin(input, candidate)?;
            if !is_operand_substitution_rule(candidate.rule()) {
                return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
            }
            contract::admit_roster(
                candidate,
                &[
                    AnalysisKind::ScalarConstants,
                    AnalysisKind::UseDefinition,
                    AnalysisKind::EffectSummaries,
                ],
                contract::USE_DEFINITION_AND_EFFECT_SUMMARIES,
                OptimizationSafetyClass::ProofCertified,
                Some(-1),
            )?;
            candidate_patch!(patch, EliminateProofCertifiedScalarIdentity, candidate);
            let validator = operand_substitution_row(candidate.rule(), patch.identity)
                .map(|row| row.validator_domain)
                .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
            if candidate.node_decision_point() != Some(patch.location)
                || candidate.substitutions()
                    != [ScalarSubstitution {
                        from: patch.result,
                        to: patch.replacement,
                        scalar_type: ScalarType::Integer(patch.scalar_type),
                    }]
            {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            let function = candidate_function(input, patch.location.machine)?;
            let block = candidate_block(function, patch.location.block)?;
            let node_index = usize::try_from(patch.location.node)
                .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
            let node = crate::candidates::candidate_node(block, patch.location.node)?;
            let scalar_type = ScalarType::Integer(patch.scalar_type);
            let shape =
                independent_proof_certified_scalar_identity(&node.operation, patch.identity)
                    .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
            if shape.source_operation != patch.source_operation
                || shape.result != patch.result
                || shape.replacement != patch.replacement
                || shape.result_type != patch.scalar_type
                || !retirable_scalar_node(block, node_index, node, patch.result, scalar_type)
                || !function
                    .node_uses()
                    .any(|use_site| use_site.value == patch.result)
            {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            if scalar_value_definition(function, shape.replacement).is_none_or(|definition| {
                definition.scalar_type != ScalarType::Integer(shape.result_type)
            }) {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            let identity_definition = scalar_value_definition(function, shape.identity_operand)
                .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
            if identity_definition.scalar_type != ScalarType::Integer(shape.identity_type) {
                return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
            }
            let literal = materialized_integer_constant(function, identity_definition)?;
            if literal.result != shape.identity_operand
                || literal.scalar_type != ScalarType::Integer(shape.identity_type)
                || literal.value != shape.identity_constant
                || !function.facts.iter().any(|fact| {
                    matches!(fact, OptimizationFact::IntegerConstant { value, constant, support }
                if *value == shape.identity_operand
                    && *constant == shape.identity_constant
                    && *support == literal.operation)
                })
            {
                return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
            }
            let expected_constant_fact = literal_scalar_constant_fact_identity(
                input.identity,
                function.machine,
                identity_definition,
                ScalarConstantValue::Integer(shape.identity_constant),
                literal.operation,
            )
            .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
            let Some((constant_fact, obligation_fact)) =
                candidate.proof_certified_scalar_identity_witness()
            else {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            };
            if constant_fact != expected_constant_fact {
                return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
            }
            if !function.facts.iter().any(|fact| {
        matches!(fact, OptimizationFact::OperationObligationReference { obligation, support }
            if *obligation == shape.obligation && *support == shape.source_operation)
    }) || input
        .accepted_obligation_fact(
            function.machine,
            optimization_unit::PsiProvenance::Operation(shape.source_operation),
            Some(shape.obligation),
        )
        .is_none_or(|fact| fact.identity != obligation_fact)
    {
        return Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch);
    }
            if !receiver_custody_distinct(block, node_index, node) {
                return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
            }
            let (expected_blocks, accepted_provenance) =
                reconstruct_proof_certified_scalar_identity_accounting(function, patch)
                    .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;

            accept_region_rewrite(
                input,
                candidate,
                patch.location.machine,
                &expected_blocks,
                validator,
                accepted_provenance,
                |output| {
                    let output_function = output
                        .function_mut(patch.location.machine)
                        .expect("candidate function exists");
                    let output_block = output_function
                        .block_mut(patch.location.block)
                        .expect("candidate block exists");
                    retire_node_custody(output_block, node_index)?;
                    sweep_output_nodes(output_function, |node| {
                        rewrite_scalar_value_uses(
                            &mut node.operation,
                            patch.result,
                            patch.replacement,
                        );
                        Ok(())
                    })?;
                    if output.accepted_obligation_facts != input.accepted_obligation_facts {
                        return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
                    }
                    Ok(())
                },
            )
        }
    }
    mod identity_classification {
        use abstract_operations::{
            AbstractOperation as O, IntegerArithmeticOperator, IntegerOverflowPolicy,
            IntegerShiftDirection, IntegerShiftPolicy,
        };
        use optimization_unit::ProofCertifiedScalarIdentityKind;
        use semantic_vocabulary::{
            IntegerCarrier, IntegerSign, IntegerType, IntegerValue, OperationId, ValueId,
        };

        #[derive(Debug, Clone, Copy)]
        pub(crate) struct IndependentProofCertifiedScalarIdentity {
            pub(super) source_operation: OperationId,
            pub(super) obligation: semantic_vocabulary::ObligationId,
            pub(super) result: ValueId,
            pub(super) replacement: ValueId,
            pub(super) identity_operand: ValueId,
            pub(super) result_type: IntegerType,
            pub(super) identity_type: IntegerType,
            pub(super) identity_constant: IntegerValue,
        }

        pub(crate) fn independent_proof_certified_scalar_identity(
            operation: &O,
            identity: ProofCertifiedScalarIdentityKind,
        ) -> Option<IndependentProofCertifiedScalarIdentity> {
            use IntegerArithmeticOperator as Operator;
            use IntegerOverflowPolicy as Policy;
            use ProofCertifiedScalarIdentityKind as Kind;
            let row = match operation {
                O::IntegerArithmetic {
                    psi_operation,
                    result,
                    scalar_type,
                    arithmetic,
                    left,
                    right,
                } => {
                    let obligation = arithmetic.obligation()?;
                    let zero = scalar_type.zero();
                    let one = scalar_type.one();
                    let (replacement, identity_operand, identity_constant) =
                        match (arithmetic.policy(), arithmetic.operator(), identity) {
                            (Policy::Exact, Operator::Add, Kind::ExactIntegerAddZeroLeft) => {
                                (*right, *left, zero)
                            }
                            (Policy::Exact, Operator::Add, Kind::ExactIntegerAddZeroRight)
                            | (
                                Policy::Exact,
                                Operator::Subtract,
                                Kind::ExactIntegerSubtractZeroRight,
                            ) => (*left, *right, zero),
                            (
                                Policy::Exact,
                                Operator::Multiply,
                                Kind::ExactIntegerMultiplyOneLeft,
                            ) => (*right, *left, one),
                            (
                                Policy::Exact,
                                Operator::Multiply,
                                Kind::ExactIntegerMultiplyOneRight,
                            )
                            | (Policy::Exact, Operator::Divide, Kind::ExactIntegerDivideOneRight)
                            | (
                                Policy::Wrapping,
                                Operator::Divide,
                                Kind::WrappingIntegerDivideOneRight,
                            )
                            | (
                                Policy::Saturating,
                                Operator::Divide,
                                Kind::SaturatingIntegerDivideOneRight,
                            ) => (*left, *right, one),
                            (
                                Policy::Exact,
                                Operator::Multiply,
                                Kind::ExactIntegerMultiplyZeroLeft,
                            )
                            | (Policy::Exact, Operator::Divide, Kind::ExactIntegerDivideZeroLeft)
                            | (
                                Policy::Wrapping,
                                Operator::Divide,
                                Kind::WrappingIntegerDivideZeroLeft,
                            )
                            | (
                                Policy::Saturating,
                                Operator::Divide,
                                Kind::SaturatingIntegerDivideZeroLeft,
                            )
                            | (
                                Policy::Exact,
                                Operator::Remainder,
                                Kind::ExactIntegerRemainderZeroLeft,
                            )
                            | (
                                Policy::Wrapping,
                                Operator::Remainder,
                                Kind::WrappingIntegerRemainderZeroLeft,
                            )
                            | (
                                Policy::Saturating,
                                Operator::Remainder,
                                Kind::SaturatingIntegerRemainderZeroLeft,
                            ) => (*left, *left, zero),
                            (
                                Policy::Exact,
                                Operator::Multiply,
                                Kind::ExactIntegerMultiplyZeroRight,
                            ) => (*right, *right, zero),
                            _ => return None,
                        };
                    (
                        *psi_operation,
                        obligation,
                        *result,
                        replacement,
                        identity_operand,
                        *scalar_type,
                        *scalar_type,
                        identity_constant,
                    )
                }
                O::IntegerShift {
                    psi_operation,
                    result,
                    value_type,
                    count_type,
                    policy: IntegerShiftPolicy::Exact { obligation },
                    direction,
                    value,
                    count,
                } => match (direction, identity) {
                    (IntegerShiftDirection::Left, Kind::ExactIntegerShiftLeftZeroCount)
                    | (IntegerShiftDirection::Right, Kind::ExactIntegerShiftRightZeroCount) => (
                        *psi_operation,
                        *obligation,
                        *result,
                        *value,
                        *count,
                        *value_type,
                        *count_type,
                        count_type.zero(),
                    ),
                    (IntegerShiftDirection::Left, Kind::ExactIntegerShiftLeftZeroValue)
                    | (IntegerShiftDirection::Right, Kind::ExactIntegerShiftRightZeroValue) => (
                        *psi_operation,
                        *obligation,
                        *result,
                        *value,
                        *value,
                        *value_type,
                        *value_type,
                        value_type.zero(),
                    ),
                    (
                        IntegerShiftDirection::Right,
                        Kind::ExactIntegerShiftRightNegativeOneValue,
                    ) if value_type.carrier() == IntegerCarrier::Fixed
                        && value_type.sign() == IntegerSign::Signed =>
                    {
                        (
                            *psi_operation,
                            *obligation,
                            *result,
                            *value,
                            *value,
                            *value_type,
                            *value_type,
                            IntegerValue::Signed(-1),
                        )
                    }
                    _ => return None,
                },
                _ => return None,
            };
            Some(IndependentProofCertifiedScalarIdentity {
                source_operation: row.0,
                obligation: row.1,
                result: row.2,
                replacement: row.3,
                identity_operand: row.4,
                result_type: row.5,
                identity_type: row.6,
                identity_constant: row.7,
            })
        }
    }
    mod integer_constant_law {
        use crate::{
            OptimizationUnitValidationError, ValidatedPsiRewrite,
            reconstruct_closed_scalar_node_boundary,
        };
        use abstract_operations::{AbstractOperation, operations::IntegerArithmeticOperands};
        use optimization_core::{
            AnalysisKind, OptimizationSafetyClass, OptimizationValidatorIdentity,
        };
        use optimization_unit::{
            ObservationKnowledge, PsiOptimizationFunction, PsiOptimizationUnit, PsiRealizationSite,
            PsiRewriteCandidate, PsiRewritePatch, ValueDefinition, ValueDefinitionSite,
        };
        use semantic_vocabulary::{
            IntegerType, IntegerValue, ObligationId, OperationId, ScalarType, ValueId,
        };

        #[derive(Debug, Clone, Copy)]
        pub(super) struct IndependentIntegerConstantLawShape {
            pub(super) psi_operation: OperationId,
            pub(super) obligation: ObligationId,
            pub(super) result: ValueId,
            pub(super) scalar_type: IntegerType,
            pub(super) left: ValueId,
            pub(super) right: ValueId,
            pub(super) replacement: IntegerValue,
            pub(super) divisor_literal: Option<IntegerValue>,
        }

        pub(super) type LawShapeRecogniser =
            fn(&AbstractOperation) -> Option<IndependentIntegerConstantLawShape>;

        pub(super) type LawEvidenceCheck = fn(
            &PsiOptimizationUnit,
            &PsiOptimizationFunction,
            &IndependentIntegerConstantLawShape,
            &PsiRewriteCandidate,
        )
            -> Result<(), OptimizationUnitValidationError>;

        #[derive(Debug, Clone, Copy)]
        pub(super) struct IndependentIntegerConstantLaw {
            pub(super) rule_domain: &'static [u8],
            pub(super) validator_domain: &'static [u8],
            pub(super) required_analyses: &'static [AnalysisKind],
            pub(super) recognise: LawShapeRecogniser,
            pub(super) verify_evidence: LawEvidenceCheck,
        }

        impl IndependentIntegerConstantLaw {
            fn contract(&self) -> crate::candidates::contract::RuleContract {
                crate::candidates::contract::RuleContract {
                    rule_domain: self.rule_domain,
                    required_analyses: self.required_analyses,
                    invalidated_analyses:
                        crate::candidates::contract::USE_DEFINITION_AND_EFFECT_SUMMARIES,
                    safety_class: OptimizationSafetyClass::ProofCertified,
                    predicted_cost_delta: Some(-1),
                }
            }
        }

        pub(super) fn accepted_law_obligation(
            input: &PsiOptimizationUnit,
            function: &PsiOptimizationFunction,
            shape: &IndependentIntegerConstantLawShape,
        ) -> Result<
            optimization_core::AcceptedObligationFactIdentity,
            OptimizationUnitValidationError,
        > {
            crate::candidates::global_value_numbering::independently_accepted_operation_fact(
                input,
                function,
                shape.psi_operation,
                shape.obligation,
            )
            .ok_or(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
        }

        pub(super) fn validate_integer_constant_law_candidate(
            input: &PsiOptimizationUnit,
            candidate: &PsiRewriteCandidate,
            law: &IndependentIntegerConstantLaw,
        ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
            crate::candidates::contract::admit(input, candidate, &law.contract())?;
            if !candidate.substitutions().is_empty() {
                return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
            }
            candidate_patch!(patch, ReplaceIntegerOperationWithConstant, candidate);
            if candidate.node_decision_point() != Some(patch.location)
                || candidate.affected_blocks() != [patch.location.block]
            {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            let function = crate::candidates::candidate_function(input, patch.location.machine)?;
            let block = crate::candidates::candidate_block(function, patch.location.block)?;
            let node = crate::candidates::candidate_node(block, patch.location.node)?;
            let shape = (law.recognise)(&node.operation)
                .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
            if patch.source_operation != shape.psi_operation
                || patch.result != shape.result
                || patch.scalar_type != shape.scalar_type
                || patch.constant != shape.replacement
                || node.definitions
                    != [ValueDefinition {
                        value: shape.result,
                        scalar_type: ScalarType::Integer(shape.scalar_type),
                        site: ValueDefinitionSite::Node {
                            block: patch.location.block,
                            node: patch.location.node,
                        },
                    }]
            {
                return Err(OptimizationUnitValidationError::CandidateEvaluationMismatch);
            }
            (law.verify_evidence)(input, function, &shape, candidate)?;
            let input_observation =
                crate::candidates::sparse_conditional_constant_propagation::observation_at(
                    input,
                    patch.location,
                )
                .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
            if !input_observation.events.is_empty()
                || !input_observation.successors.is_empty()
                || !input_observation.ownership.is_empty()
                || input_observation.crash != ObservationKnowledge::No
                || input_observation.suspension != ObservationKnowledge::No
            {
                return Err(OptimizationUnitValidationError::CandidateObservationMismatch);
            }
            let input_live = reconstruct_closed_scalar_node_boundary(input, patch.location)
                .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
            if !input_live.live_in.contains(&shape.left)
                || !input_live.live_in.contains(&shape.right)
                || !input_live.live_out.contains(&shape.result)
            {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            let site = PsiRealizationSite::Node(patch.location);
            let expected_provenance = [crate::candidates::realized_at_own_site(
                site,
                &node.provenance,
                &node.work,
            )];
            if candidate.provenance() != expected_provenance {
                return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
            }

            let output = crate::candidates::constant_installation::install_evaluated_constant(
                input,
                patch.location,
                AbstractOperation::IntegerConstant {
                    psi_operation: shape.psi_operation,
                    result: shape.result,
                    scalar_type: ScalarType::Integer(shape.scalar_type),
                    value: patch.constant,
                },
                shape.result,
                ScalarType::Integer(shape.scalar_type),
                &input_observation,
                &input_live,
            )?;
            if output.accepted_obligation_facts != input.accepted_obligation_facts {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            }
            Ok(ValidatedPsiRewrite {
                unit: output,
                candidate: candidate.identity(),
                validator: OptimizationValidatorIdentity::from_canonical_bytes(
                    law.validator_domain,
                ),
                provenance: expected_provenance.into(),
            })
        }

        pub(super) fn constant_law_shape(
            operands: &IntegerArithmeticOperands,
            replacement: IntegerValue,
            divisor_literal: Option<IntegerValue>,
        ) -> Option<IndependentIntegerConstantLawShape> {
            Some(IndependentIntegerConstantLawShape {
                psi_operation: operands.psi_operation,
                obligation: operands.arithmetic.obligation()?,
                result: operands.result,
                scalar_type: operands.scalar_type,
                left: operands.left,
                right: operands.right,
                replacement,
                divisor_literal,
            })
        }
    }
    mod literal_operand {
        use abstract_operations::AbstractOperation as O;
        use optimization_unit::{PsiOptimizationFunction, ValueDefinition, ValueDefinitionSite};
        use semantic_vocabulary::{IntegerValue, OperationId, ScalarType, ValueId};

        use crate::OptimizationUnitValidationError;

        pub(crate) struct MaterializedIntegerConstant {
            pub(crate) operation: OperationId,
            pub(crate) result: ValueId,
            pub(crate) scalar_type: ScalarType,
            pub(crate) value: IntegerValue,
        }

        pub(crate) fn materialized_integer_constant(
            function: &PsiOptimizationFunction,
            definition: ValueDefinition,
        ) -> Result<MaterializedIntegerConstant, OptimizationUnitValidationError> {
            let ValueDefinitionSite::Node { block, node } = definition.site else {
                return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
            };
            let literal = function
                .block(block)
                .and_then(|block| {
                    usize::try_from(node)
                        .ok()
                        .and_then(|node| block.nodes.get(node))
                })
                .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
            let O::IntegerConstant {
                psi_operation,
                result,
                scalar_type,
                value,
            } = literal.operation
            else {
                return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
            };
            Ok(MaterializedIntegerConstant {
                operation: psi_operation,
                result,
                scalar_type,
                value,
            })
        }
    }
    mod rule_catalog {
        use optimization_core::{AnalysisKind, OptimizationRuleIdentity};
        use optimization_unit::ProofCertifiedScalarIdentityKind;

        use super::integer_constant_law::IndependentIntegerConstantLaw;
        use super::same_operand::{
            accepted_operation_obligation_evidence, exact_integer_self_subtract_shape,
            integer_self_divide_shape, integer_self_remainder_shape,
        };
        use super::unit_divisor::{
            integer_remainder_by_one_shape, materialized_divisor_evidence,
            signed_integer_remainder_by_negative_one_shape,
        };

        #[derive(Debug, Clone, Copy)]
        pub(super) enum ProofCheckValidationRoute {
            DeadScalar,
            OperandSubstitution,
            IntegerConstantLaw(IndependentIntegerConstantLaw),
        }

        pub(super) struct OperandSubstitutionRule {
            pub(super) rule_domain: &'static [u8],
            pub(super) validator_domain: &'static [u8],
            pub(super) identities: &'static [ProofCertifiedScalarIdentityKind],
        }

        #[rustfmt::skip]
const OPERAND_SUBSTITUTION_RULES: &[OperandSubstitutionRule] = &[
    OperandSubstitutionRule { rule_domain: b"omega.psi-rule.live-proof-certified-integer-identity-elimination.v1", validator_domain: b"omega.validator.live-proof-certified-integer-identity-elimination.v1", identities: &[ProofCertifiedScalarIdentityKind::ExactIntegerAddZeroLeft, ProofCertifiedScalarIdentityKind::ExactIntegerAddZeroRight, ProofCertifiedScalarIdentityKind::ExactIntegerSubtractZeroRight, ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyOneLeft, ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyOneRight, ProofCertifiedScalarIdentityKind::ExactIntegerShiftLeftZeroCount, ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightZeroCount] },
    OperandSubstitutionRule { rule_domain: b"omega.psi-rule.live-proof-certified-integer-divide-by-one-elimination.v1", validator_domain: b"omega.validator.live-proof-certified-integer-divide-by-one-elimination.v1", identities: &[ProofCertifiedScalarIdentityKind::ExactIntegerDivideOneRight, ProofCertifiedScalarIdentityKind::WrappingIntegerDivideOneRight, ProofCertifiedScalarIdentityKind::SaturatingIntegerDivideOneRight] },
    OperandSubstitutionRule { rule_domain: b"omega.psi-rule.live-proof-certified-exact-integer-multiply-by-zero-elimination.v1", validator_domain: b"omega.validator.live-proof-certified-exact-integer-multiply-by-zero-elimination.v1", identities: &[ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroLeft, ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroRight] },
    OperandSubstitutionRule { rule_domain: b"omega.psi-rule.live-proof-certified-integer-zero-dividend-elimination.v1", validator_domain: b"omega.validator.live-proof-certified-integer-zero-dividend-elimination.v1", identities: &[ProofCertifiedScalarIdentityKind::ExactIntegerDivideZeroLeft, ProofCertifiedScalarIdentityKind::WrappingIntegerDivideZeroLeft, ProofCertifiedScalarIdentityKind::SaturatingIntegerDivideZeroLeft, ProofCertifiedScalarIdentityKind::ExactIntegerRemainderZeroLeft, ProofCertifiedScalarIdentityKind::WrappingIntegerRemainderZeroLeft, ProofCertifiedScalarIdentityKind::SaturatingIntegerRemainderZeroLeft] },
    OperandSubstitutionRule { rule_domain: b"omega.psi-rule.live-proof-certified-exact-integer-zero-value-shift-elimination.v1", validator_domain: b"omega.validator.live-proof-certified-exact-integer-zero-value-shift-elimination.v1", identities: &[ProofCertifiedScalarIdentityKind::ExactIntegerShiftLeftZeroValue, ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightZeroValue] },
    OperandSubstitutionRule { rule_domain: b"omega.psi-rule.live-proof-certified-exact-signed-integer-negative-one-value-shift-right-elimination.v1", validator_domain: b"omega.validator.live-proof-certified-exact-signed-integer-negative-one-value-shift-right-elimination.v1", identities: &[ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightNegativeOneValue] },
];

        pub(super) fn is_operand_substitution_rule(rule: OptimizationRuleIdentity) -> bool {
            OPERAND_SUBSTITUTION_RULES
                .iter()
                .any(|row| rule == identity(row.rule_domain))
        }

        pub(super) fn operand_substitution_row(
            rule: OptimizationRuleIdentity,
            identity_kind: ProofCertifiedScalarIdentityKind,
        ) -> Option<&'static OperandSubstitutionRule> {
            OPERAND_SUBSTITUTION_RULES.iter().find(|row| {
                rule == identity(row.rule_domain) && row.identities.contains(&identity_kind)
            })
        }

        impl ProofCheckValidationRoute {
            pub(super) fn for_rule(rule: OptimizationRuleIdentity) -> Option<Self> {
                let route = if rule
                    == identity(b"omega.psi-rule.dead-unused-proof-certified-scalar-elimination.v1")
                {
                    Self::DeadScalar
                } else if is_operand_substitution_rule(rule) {
                    Self::OperandSubstitution
                } else {
                    return integer_constant_laws()
                        .into_iter()
                        .find(|law| rule == identity(law.rule_domain))
                        .map(Self::IntegerConstantLaw);
                };
                Some(route)
            }
        }

        pub(crate) fn is_proof_check_elision_rule(rule: OptimizationRuleIdentity) -> bool {
            ProofCheckValidationRoute::for_rule(rule).is_some()
        }

        fn integer_constant_laws() -> [IndependentIntegerConstantLaw; 5] {
            const SAME_OPERAND_ANALYSES: &[AnalysisKind] =
                &[AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries];
            const MATERIALIZED_DIVISOR_ANALYSES: &[AnalysisKind] = &[
                AnalysisKind::ScalarConstants,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ];
            [
        IndependentIntegerConstantLaw {
            rule_domain:
                b"omega.psi-rule.live-proof-certified-exact-integer-self-subtract-elimination.v1",
            validator_domain:
                b"omega.validator.live-proof-certified-exact-integer-self-subtract-elimination.v1",
            required_analyses: SAME_OPERAND_ANALYSES,
            recognise: exact_integer_self_subtract_shape,
            verify_evidence: accepted_operation_obligation_evidence,
        },
        IndependentIntegerConstantLaw {
            rule_domain: b"omega.psi-rule.live-proof-certified-integer-self-remainder-elimination.v1",
            validator_domain: b"omega.validator.live-proof-certified-integer-self-remainder-elimination.v1",
            required_analyses: SAME_OPERAND_ANALYSES,
            recognise: integer_self_remainder_shape,
            verify_evidence: accepted_operation_obligation_evidence,
        },
        IndependentIntegerConstantLaw {
            rule_domain: b"omega.psi-rule.live-proof-certified-integer-self-divide-elimination.v1",
            validator_domain: b"omega.validator.live-proof-certified-integer-self-divide-elimination.v1",
            required_analyses: SAME_OPERAND_ANALYSES,
            recognise: integer_self_divide_shape,
            verify_evidence: accepted_operation_obligation_evidence,
        },
        IndependentIntegerConstantLaw {
            rule_domain:
                b"omega.psi-rule.live-proof-certified-integer-remainder-by-one-elimination.v1",
            validator_domain:
                b"omega.validator.live-proof-certified-integer-remainder-by-one-elimination.v1",
            required_analyses: MATERIALIZED_DIVISOR_ANALYSES,
            recognise: integer_remainder_by_one_shape,
            verify_evidence: materialized_divisor_evidence,
        },
        IndependentIntegerConstantLaw {
            rule_domain: b"omega.psi-rule.live-proof-certified-signed-integer-remainder-by-negative-one-elimination.v1",
            validator_domain: b"omega.validator.live-proof-certified-signed-integer-remainder-by-negative-one-elimination.v1",
            required_analyses: MATERIALIZED_DIVISOR_ANALYSES,
            recognise: signed_integer_remainder_by_negative_one_shape,
            verify_evidence: materialized_divisor_evidence,
        },
    ]
        }

        fn identity(domain: &[u8]) -> OptimizationRuleIdentity {
            OptimizationRuleIdentity::from_canonical_bytes(domain)
        }
    }
    mod same_operand {
        use super::integer_constant_law::{
            IndependentIntegerConstantLawShape, accepted_law_obligation, constant_law_shape,
        };
        use abstract_operations::{
            AbstractOperation as O, IntegerArithmeticOperator, IntegerOverflowPolicy,
        };
        use optimization_unit::{
            PsiOptimizationFunction, PsiOptimizationUnit, PsiRewriteCandidate,
        };
        use semantic_vocabulary::{IntegerCarrier, IntegerSign};

        use crate::OptimizationUnitValidationError;

        pub(super) fn exact_integer_self_subtract_shape(
            operation: &O,
        ) -> Option<IndependentIntegerConstantLawShape> {
            let operands =
                operation.integer_arithmetic_operands(IntegerArithmeticOperator::Subtract)?;
            if operands.arithmetic.policy() != IntegerOverflowPolicy::Exact
                || operands.left != operands.right
            {
                return None;
            }
            constant_law_shape(&operands, operands.scalar_type.zero(), None)
        }

        pub(super) fn integer_self_remainder_shape(
            operation: &O,
        ) -> Option<IndependentIntegerConstantLawShape> {
            let operands =
                operation.integer_arithmetic_operands(IntegerArithmeticOperator::Remainder)?;
            if operands.left != operands.right {
                return None;
            }
            constant_law_shape(&operands, operands.scalar_type.zero(), None)
        }

        pub(super) fn integer_self_divide_shape(
            operation: &O,
        ) -> Option<IndependentIntegerConstantLawShape> {
            let operands =
                operation.integer_arithmetic_operands(IntegerArithmeticOperator::Divide)?;
            let positive_one_is_representable = operands.scalar_type.carrier()
                == IntegerCarrier::Fixed
                && !(operands.scalar_type.sign() == IntegerSign::Signed
                    && operands.scalar_type.bits() == 1);
            if !positive_one_is_representable || operands.left != operands.right {
                return None;
            }
            constant_law_shape(&operands, operands.scalar_type.one(), None)
        }

        pub(super) fn accepted_operation_obligation_evidence(
            input: &PsiOptimizationUnit,
            function: &PsiOptimizationFunction,
            shape: &IndependentIntegerConstantLawShape,
            candidate: &PsiRewriteCandidate,
        ) -> Result<(), OptimizationUnitValidationError> {
            let expected_fact = accepted_law_obligation(input, function, shape)?;
            if candidate.accepted_obligation_witness() != Some(expected_fact) {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            }
            Ok(())
        }
    }
    mod unit_divisor {
        use super::integer_constant_law::{
            IndependentIntegerConstantLawShape, accepted_law_obligation, constant_law_shape,
        };
        use super::literal_operand::materialized_integer_constant;
        use abstract_operations::{AbstractOperation as O, IntegerArithmeticOperator};
        use optimization_unit::{
            OptimizationFact, PsiOptimizationFunction, PsiOptimizationUnit, PsiRewriteCandidate,
            ScalarConstantValue, literal_scalar_constant_fact_identity,
        };
        use semantic_vocabulary::{
            IntegerCarrier, IntegerSign, IntegerType, IntegerValue, ScalarType,
        };

        use crate::OptimizationUnitValidationError;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(super) enum IndependentRemainderUnitDivisor {
            PositiveOne,
            SignedNegativeOne,
        }

        impl IndependentRemainderUnitDivisor {
            fn value(self, scalar_type: IntegerType) -> Option<IntegerValue> {
                match self {
                    Self::PositiveOne => Some(scalar_type.one()),
                    Self::SignedNegativeOne if scalar_type.sign() == IntegerSign::Signed => {
                        Some(IntegerValue::Signed(-1))
                    }
                    Self::SignedNegativeOne => None,
                }
            }
        }

        pub(super) fn integer_remainder_by_one_shape(
            operation: &O,
        ) -> Option<IndependentIntegerConstantLawShape> {
            literal_divisor_shape(operation, IndependentRemainderUnitDivisor::PositiveOne)
        }

        pub(super) fn signed_integer_remainder_by_negative_one_shape(
            operation: &O,
        ) -> Option<IndependentIntegerConstantLawShape> {
            literal_divisor_shape(
                operation,
                IndependentRemainderUnitDivisor::SignedNegativeOne,
            )
        }

        fn literal_divisor_shape(
            operation: &O,
            divisor: IndependentRemainderUnitDivisor,
        ) -> Option<IndependentIntegerConstantLawShape> {
            let operands =
                operation.integer_arithmetic_operands(IntegerArithmeticOperator::Remainder)?;
            if operands.scalar_type.carrier() != IntegerCarrier::Fixed
                || operands.left == operands.right
            {
                return None;
            }
            let divisor_literal = divisor.value(operands.scalar_type)?;
            constant_law_shape(
                &operands,
                operands.scalar_type.zero(),
                Some(divisor_literal),
            )
        }

        pub(super) fn materialized_divisor_evidence(
            input: &PsiOptimizationUnit,
            function: &PsiOptimizationFunction,
            shape: &IndependentIntegerConstantLawShape,
            candidate: &PsiRewriteCandidate,
        ) -> Result<(), OptimizationUnitValidationError> {
            let expected_divisor = shape
                .divisor_literal
                .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
            let right_definition =
        crate::candidates::sparse_conditional_constant_propagation::scalar_value_definition(
            function,
            shape.right,
        )
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
            if right_definition.scalar_type != ScalarType::Integer(shape.scalar_type) {
                return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
            }
            let literal = materialized_integer_constant(function, right_definition)?;
            if literal.result != shape.right
                || literal.scalar_type != ScalarType::Integer(shape.scalar_type)
                || literal.value != expected_divisor
                || !shape.scalar_type.admits(expected_divisor)
                || !function.facts.iter().any(|fact| {
                    matches!(fact, OptimizationFact::IntegerConstant { value, constant, support }
                if *value == shape.right
                    && *constant == expected_divisor
                    && *support == literal.operation)
                })
            {
                return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
            }
            let expected_constant_fact = literal_scalar_constant_fact_identity(
                input.identity,
                function.machine,
                right_definition,
                ScalarConstantValue::Integer(expected_divisor),
                literal.operation,
            )
            .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
            let expected_obligation_fact = accepted_law_obligation(input, function, shape)?;
            let Some((constant_fact, obligation_fact)) =
                candidate.proof_certified_scalar_identity_witness()
            else {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            };
            if constant_fact != expected_constant_fact {
                return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
            }
            if obligation_fact != expected_obligation_fact {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            }
            Ok(())
        }
    }
    pub(crate) use candidate_validation::validate_proof_certified_scalar_identity_candidate;
    pub(crate) use literal_operand::materialized_integer_constant;

    use integer_constant_law::validate_integer_constant_law_candidate;
    use rule_catalog::ProofCheckValidationRoute;
    pub(crate) use rule_catalog::is_proof_check_elision_rule;

    pub(super) fn validate_proof_check_elision_candidate(
        input: &PsiOptimizationUnit,
        candidate: &PsiRewriteCandidate,
    ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
        match ProofCheckValidationRoute::for_rule(candidate.rule()) {
            Some(ProofCheckValidationRoute::DeadScalar) => {
                validate_dead_scalar_node_candidate(input, candidate)
            }
            Some(ProofCheckValidationRoute::OperandSubstitution) => {
                validate_proof_certified_scalar_identity_candidate(input, candidate)
            }
            Some(ProofCheckValidationRoute::IntegerConstantLaw(law)) => {
                validate_integer_constant_law_candidate(input, candidate, &law)
            }
            None => Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch),
        }
    }
}
pub(crate) mod rewrite_accounting {
    use crate::candidates::copy_propagation::rewrite_block_parameter_operation;
    use crate::candidates::provenance_realized_at;
    use crate::unit_validation::derived_metadata::expected_edges;
    use abstract_operations::AbstractOperation as O;
    use optimization_unit::{
        NodeLocation, OptimizationBlock, OptimizationEdge, PsiOptimizationFunction,
        PsiRealizationSite, RedundantBlockParameterRewrite, ScalarSubstitution,
    };
    use semantic_vocabulary::{BlockId, MachineId};

    pub(super) mod adjacent_merge {
        use crate::candidates::rewrite_accounting::realize_merged_target_provenance;
        use crate::candidates::{
            canonicalize_provenance, provenance_realized_at, realize_node_provenance,
        };
        use optimization_unit::{
            AdjacentBlockMergeRewrite, NodeLocation, OwnershipFrontierSite,
            OwnershipFrontierWitness, OwnershipFrontierWitnessRow, PsiOptimizationFunction,
            PsiOptimizationUnit, PsiRealizationSite, ScalarSubstitution,
        };
        use semantic_vocabulary::{BlockId, EdgeId};
        use std::collections::BTreeSet;

        pub(crate) fn reconstruct_adjacent_merge_ownership_is_identity(
            unit: &PsiOptimizationUnit,
            function: &PsiOptimizationFunction,
            incoming: EdgeId,
            target: BlockId,
        ) -> bool {
            reconstruct_adjacent_merge_ownership_witness(unit, function, incoming, target).is_some()
        }

        pub(crate) fn reconstruct_adjacent_merge_ownership_witness(
            unit: &PsiOptimizationUnit,
            function: &PsiOptimizationFunction,
            incoming: EdgeId,
            target: BlockId,
        ) -> Option<OwnershipFrontierWitness> {
            let sites = [
                OwnershipFrontierSite::EdgeEntry(incoming),
                OwnershipFrontierSite::EdgeExit(incoming),
                OwnershipFrontierSite::BlockEntry(target),
            ];
            let facts = sites.map(|site| {
                unit.ownership_frontier_facts
                    .iter()
                    .find(|fact| fact.machine == function.machine && fact.site == site)
            });
            if facts.iter().all(Option::is_none) {
                return (function.structural_parameters.is_empty()
                    && function.entry_claim_declarations.is_empty()
                    && function.declared_places.is_empty())
                .then_some(OwnershipFrontierWitness { rows: Vec::new() });
            }
            if !facts.iter().all(Option::is_some)
                || !facts
                    .windows(2)
                    .all(|pair| pair[0].unwrap().snapshot == pair[1].unwrap().snapshot)
            {
                return None;
            }
            let mut rows = facts
                .into_iter()
                .map(|fact| {
                    let fact = fact.expect("complete ownership frontier fact set");
                    OwnershipFrontierWitnessRow {
                        site: fact.site,
                        fact: fact.identity,
                    }
                })
                .collect::<Vec<_>>();
            rows.sort_unstable_by_key(|row| row.site);
            Some(OwnershipFrontierWitness { rows })
        }

        pub(crate) fn reconstruct_adjacent_merge_accounting(
            function: &PsiOptimizationFunction,
            patch: AdjacentBlockMergeRewrite,
            substitutions: &[ScalarSubstitution],
        ) -> Option<(Vec<BlockId>, Vec<optimization_unit::ProvenanceRewrite>)> {
            let predecessor_position = function.block_index(patch.predecessor.block)?;
            let target_position = function.block_index(patch.target)?;
            if target_position != predecessor_position.checked_add(1)? {
                return None;
            }
            let predecessor = &function.blocks[predecessor_position];
            let predecessor_node = predecessor
                .nodes
                .get(usize::try_from(patch.predecessor.node).ok()?)?;
            let incoming = predecessor_node.edge(patch.incoming_edge)?;
            let target = &function.blocks[target_position];
            let incoming_site = PsiRealizationSite::Edge {
                machine: function.machine,
                edge: patch.incoming_edge,
            };
            let mut affected = BTreeSet::from([predecessor.id, target.id]);
            let first = target.nodes.first()?;
            let mut realized = if first.successors.is_empty() {
                vec![provenance_realized_at(
                    incoming_site,
                    PsiRealizationSite::Node(NodeLocation {
                        machine: function.machine,
                        block: predecessor.id,
                        node: patch.predecessor.node,
                    }),
                    &incoming.provenance,
                    &incoming.work,
                )]
            } else {
                first
                    .successors
                    .iter()
                    .map(|successor| {
                        provenance_realized_at(
                            incoming_site,
                            PsiRealizationSite::Edge {
                                machine: function.machine,
                                edge: successor.psi_edge,
                            },
                            &incoming.provenance,
                            &incoming.work,
                        )
                    })
                    .collect()
            };
            realize_merged_target_provenance(function, patch.predecessor, target, &mut realized)?;
            for block in function.blocks.iter().skip(target_position + 1) {
                affected.insert(block.id);
                realize_node_provenance(
                    function.machine,
                    block.id,
                    block.nodes.iter().enumerate(),
                    &mut realized,
                )?;
            }
            let substituted_values = substitutions
                .iter()
                .map(|row| row.from)
                .collect::<BTreeSet<_>>();
            for block in &function.blocks {
                if affected.contains(&block.id) {
                    continue;
                }
                let changed_nodes = block
                    .nodes
                    .iter()
                    .enumerate()
                    .filter(|(_, node)| {
                        node.uses
                            .iter()
                            .any(|row| substituted_values.contains(&row.value))
                    })
                    .collect::<Vec<_>>();
                if changed_nodes.is_empty() {
                    continue;
                }
                affected.insert(block.id);
                realize_node_provenance(
                    function.machine,
                    block.id,
                    changed_nodes.into_iter(),
                    &mut realized,
                )?;
            }
            canonicalize_provenance(&mut realized);
            Some((affected.into_iter().collect(), realized))
        }
    }
    pub(super) mod common_subexpression {
        use crate::candidates::canonicalize_provenance;
        use crate::candidates::realized_at_own_site;
        use crate::candidates::rewrite_accounting::dead_scalar::{
            reconstruct_dead_scalar_node_accounting, reconstruct_removed_result_use_accounting,
        };
        use optimization_unit::{
            DeadScalarNodeRewrite, LocalScalarCommonSubexpressionRewrite,
            PhiTranslatedScalarGvnRewrite, PsiOptimizationFunction, PsiRealizationSite,
        };
        use semantic_vocabulary::BlockId;

        pub(crate) fn reconstruct_local_cse_accounting(
            function: &PsiOptimizationFunction,
            patch: LocalScalarCommonSubexpressionRewrite,
        ) -> Option<(Vec<BlockId>, Vec<optimization_unit::ProvenanceRewrite>)> {
            reconstruct_removed_result_use_accounting(
                function,
                patch.redundant,
                patch.redundant_operation,
                patch.redundant_result,
                patch.scalar_type,
            )
        }

        pub(crate) fn reconstruct_phi_translated_cse_accounting(
            function: &PsiOptimizationFunction,
            patch: &PhiTranslatedScalarGvnRewrite,
        ) -> Option<(Vec<BlockId>, Vec<optimization_unit::ProvenanceRewrite>)> {
            let dead = DeadScalarNodeRewrite {
                location: patch.redundant,
                source_operation: patch.redundant_operation,
                result: patch.redundant_result,
                scalar_type: patch.scalar_type,
            };
            let (mut blocks, mut provenance) =
                reconstruct_dead_scalar_node_accounting(function, dead)?;
            for incoming in &patch.incoming {
                let edge = function.block(incoming.source)?.successors().find(|edge| {
                    edge.psi_edge == incoming.edge && edge.target == patch.redundant.block
                })?;
                blocks.push(incoming.source);
                let site = PsiRealizationSite::Edge {
                    machine: function.machine,
                    edge: incoming.edge,
                };
                provenance.push(realized_at_own_site(site, &edge.provenance, &edge.work));
            }
            blocks.sort();
            blocks.dedup();
            canonicalize_provenance(&mut provenance);
            Some((blocks, provenance))
        }
    }
    pub(super) mod dead_scalar {
        use crate::candidates::{
            canonicalize_provenance, provenance_realized_at, realize_node_provenance,
            realized_at_own_site,
        };
        use optimization_unit::{
            DeadScalarNodeRewrite, NodeLocation, PsiOptimizationFunction, PsiRealizationSite,
        };
        use semantic_vocabulary::BlockId;

        pub(crate) fn reconstruct_dead_scalar_node_accounting(
            function: &PsiOptimizationFunction,
            patch: DeadScalarNodeRewrite,
        ) -> Option<(Vec<BlockId>, Vec<optimization_unit::ProvenanceRewrite>)> {
            let block_position = function.block_index(patch.location.block)?;
            let node_position = usize::try_from(patch.location.node).ok()?;
            let block = &function.blocks[block_position];
            let removed = block.nodes.get(node_position)?;
            block.nodes.get(node_position.checked_add(1)?)?;
            let mut provenance = vec![realized_at_own_site(
                PsiRealizationSite::Node(patch.location),
                &removed.provenance,
                &removed.work,
            )];
            for (index, node) in block.nodes.iter().enumerate().skip(node_position + 1) {
                if node.provenance.is_empty() {
                    continue;
                }
                let old = NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: u32::try_from(index).ok()?,
                };
                let new = NodeLocation {
                    node: old.node.checked_sub(1)?,
                    ..old
                };
                provenance.push(provenance_realized_at(
                    PsiRealizationSite::Node(old),
                    PsiRealizationSite::Node(new),
                    &node.provenance,
                    &node.work,
                ));
            }
            let mut blocks = vec![block.id];
            for later in function.blocks.iter().skip(block_position + 1) {
                blocks.push(later.id);
                realize_node_provenance(
                    function.machine,
                    later.id,
                    later.nodes.iter().enumerate(),
                    &mut provenance,
                )?;
            }
            blocks.sort();
            canonicalize_provenance(&mut provenance);
            Some((blocks, provenance))
        }

        pub(crate) fn reconstruct_removed_result_use_accounting(
            function: &PsiOptimizationFunction,
            location: NodeLocation,
            source_operation: semantic_vocabulary::OperationId,
            result: semantic_vocabulary::ValueId,
            scalar_type: semantic_vocabulary::ScalarType,
        ) -> Option<(Vec<BlockId>, Vec<optimization_unit::ProvenanceRewrite>)> {
            let dead = DeadScalarNodeRewrite {
                location,
                source_operation,
                result,
                scalar_type,
            };
            let (mut blocks, mut provenance) =
                reconstruct_dead_scalar_node_accounting(function, dead)?;
            for use_block in &function.blocks {
                if blocks.contains(&use_block.id)
                    || !use_block.uses().any(|row| row.value == result)
                {
                    continue;
                }
                blocks.push(use_block.id);
                realize_node_provenance(
                    function.machine,
                    use_block.id,
                    use_block.nodes.iter().enumerate(),
                    &mut provenance,
                )?;
            }
            blocks.sort();
            blocks.dedup();
            canonicalize_provenance(&mut provenance);
            Some((blocks, provenance))
        }
    }
    pub(super) mod non_adjacent_merge {
        use crate::candidates::rewrite_accounting::realize_merged_target_provenance;
        use crate::candidates::{
            canonicalize_provenance, provenance_realized_at, realize_node_provenance,
        };
        use optimization_unit::{
            NonAdjacentBlockMergeRewrite, PsiOptimizationFunction, PsiRealizationSite,
            ScalarSubstitution,
        };
        use semantic_vocabulary::BlockId;
        use std::collections::{BTreeMap, BTreeSet};

        pub(crate) fn reconstruct_non_adjacent_merge_accounting(
            function: &PsiOptimizationFunction,
            patch: NonAdjacentBlockMergeRewrite,
            substitutions: &[ScalarSubstitution],
        ) -> Option<(Vec<BlockId>, Vec<optimization_unit::ProvenanceRewrite>)> {
            let predecessor_position = function.block_index(patch.predecessor.block)?;
            let target_position = function.block_index(patch.target)?;
            if target_position == predecessor_position.checked_add(1)? {
                return None;
            }
            let predecessor = &function.blocks[predecessor_position];
            let predecessor_node = predecessor
                .nodes
                .get(usize::try_from(patch.predecessor.node).ok()?)?;
            let incoming = predecessor_node.edge(patch.incoming_edge)?;
            let target = &function.blocks[target_position];
            let first = target.nodes.first()?;
            let incoming_site = PsiRealizationSite::Edge {
                machine: function.machine,
                edge: patch.incoming_edge,
            };
            let mut realized = if !first.provenance.is_empty() {
                vec![provenance_realized_at(
                    incoming_site,
                    PsiRealizationSite::Node(patch.predecessor),
                    &incoming.provenance,
                    &incoming.work,
                )]
            } else if !first.successors.is_empty() {
                first
                    .successors
                    .iter()
                    .map(|successor| {
                        provenance_realized_at(
                            incoming_site,
                            PsiRealizationSite::Edge {
                                machine: function.machine,
                                edge: successor.psi_edge,
                            },
                            &incoming.provenance,
                            &incoming.work,
                        )
                    })
                    .collect()
            } else {
                return None;
            };

            realize_merged_target_provenance(function, patch.predecessor, target, &mut realized)?;

            let mut input_effect = 0u64;
            let mut input_starts = BTreeMap::new();
            for block in &function.blocks {
                input_starts.insert(block.id, input_effect);
                input_effect = input_effect.checked_add(u64::try_from(block.nodes.len()).ok()?)?;
            }
            let mut output_effect = 0u64;
            let mut effect_shifted = BTreeSet::new();
            for block in &function.blocks {
                if block.id == patch.target {
                    continue;
                }
                if input_starts.get(&block.id).copied()? != output_effect {
                    effect_shifted.insert(block.id);
                }
                let output_nodes = if block.id == patch.predecessor.block {
                    block
                        .nodes
                        .len()
                        .checked_sub(1)?
                        .checked_add(target.nodes.len())?
                } else {
                    block.nodes.len()
                };
                output_effect = output_effect.checked_add(u64::try_from(output_nodes).ok()?)?;
            }

            let substituted_values = substitutions
                .iter()
                .map(|row| row.from)
                .collect::<BTreeSet<_>>();
            let mut affected = BTreeSet::from([patch.predecessor.block, patch.target]);
            affected.extend(effect_shifted.iter().copied());
            for block in &function.blocks {
                if block.id == patch.target {
                    continue;
                }
                let mut changed_uses = BTreeSet::new();
                for (node_index, node) in block.nodes.iter().enumerate() {
                    if node
                        .uses
                        .iter()
                        .any(|row| substituted_values.contains(&row.value))
                    {
                        changed_uses.insert(node_index);
                        affected.insert(block.id);
                    }
                }
                realize_node_provenance(
                    function.machine,
                    block.id,
                    block.nodes.iter().enumerate().filter(|(node_index, _)| {
                        effect_shifted.contains(&block.id) || changed_uses.contains(node_index)
                    }),
                    &mut realized,
                )?;
            }
            canonicalize_provenance(&mut realized);
            Some((affected.into_iter().collect(), realized))
        }
    }
    pub(super) mod scalar_identity {
        use crate::candidates::rewrite_accounting::dead_scalar::reconstruct_removed_result_use_accounting;
        use optimization_unit::{
            ProofCertifiedScalarIdentityRewrite, PsiOptimizationFunction,
            TotalScalarIdentityRewrite,
        };
        use semantic_vocabulary::{BlockId, ScalarType};

        macro_rules! scalar_identity_reconstruction {
            ($name:ident, $patch:ty) => {
                pub(crate) fn $name(
                    function: &PsiOptimizationFunction,
                    patch: $patch,
                ) -> Option<(Vec<BlockId>, Vec<optimization_unit::ProvenanceRewrite>)> {
                    reconstruct_removed_result_use_accounting(
                        function,
                        patch.location,
                        patch.source_operation,
                        patch.result,
                        ScalarType::Integer(patch.scalar_type),
                    )
                }
            };
        }

        scalar_identity_reconstruction!(
            reconstruct_proof_certified_scalar_identity_accounting,
            ProofCertifiedScalarIdentityRewrite
        );

        scalar_identity_reconstruction!(
            reconstruct_total_scalar_identity_accounting,
            TotalScalarIdentityRewrite
        );
    }
    pub(crate) mod substitutions {
        use abstract_operations::AbstractOperation as O;
        use semantic_vocabulary::{BlockId, EdgeId, ValueId};

        pub(crate) fn rewrite_scalar_value_uses(operation: &mut O, from: ValueId, to: ValueId) {
            let mut replace = |value: &mut ValueId| {
                if *value == from {
                    *value = to;
                }
            };
            let rewrite_bindings = |bindings: &mut Vec<abstract_operations::ValueBinding>| {
                for binding in bindings {
                    replace(&mut binding.argument);
                }
            };
            match operation {
                O::Jump { bindings, .. } => rewrite_bindings(bindings),
                O::Conditional {
                    condition,
                    when_true,
                    when_false,
                } => {
                    replace(condition);
                    rewrite_bindings(&mut when_true.bindings);
                    rewrite_bindings(&mut when_false.bindings);
                }
                O::StructuralCase { .. } => {}
                _ => crate::candidates::value_operand_rewrites::rewrite_value_operands(
                    operation,
                    &mut replace,
                ),
            }
        }

        pub(crate) fn rewrite_successor_operation(
            operation: &mut O,
            edge: EdgeId,
            target: BlockId,
            bindings: &[abstract_operations::ValueBinding],
        ) -> bool {
            match operation {
                O::Jump {
                    psi_edge,
                    target: operation_target,
                    bindings: operation_bindings,
                    ..
                } if *psi_edge == edge => {
                    *operation_target = target;
                    *operation_bindings = bindings.to_vec();
                    true
                }
                O::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    let successor = if when_true.psi_edge == edge {
                        when_true
                    } else if when_false.psi_edge == edge {
                        when_false
                    } else {
                        return false;
                    };
                    successor.target = target;
                    successor.bindings = bindings.to_vec();
                    true
                }
                _ => false,
            }
        }
    }
    pub(super) mod terminal_fusion {
        use crate::candidates::{
            canonicalize_provenance, provenance_realized_at, realized_at_own_site,
        };
        use optimization_unit::{
            NodeLocation, PsiOptimizationFunction, PsiRealizationSite, SharedJumpFusionRewrite,
        };
        use semantic_vocabulary::BlockId;

        pub(crate) fn reconstruct_shared_terminal_fusion_accounting(
            function: &PsiOptimizationFunction,
            patch: SharedJumpFusionRewrite,
        ) -> Option<(Vec<BlockId>, Vec<optimization_unit::ProvenanceRewrite>)> {
            let predecessor = function.block(patch.predecessor.block)?;
            let predecessor_node = predecessor
                .nodes
                .get(usize::try_from(patch.predecessor.node).ok()?)?;
            let incoming = predecessor_node.edge(patch.incoming_edge)?;
            let target = function.block(patch.target)?;
            let [terminal] = target.nodes.as_slice() else {
                return None;
            };
            let input_edge = PsiRealizationSite::Edge {
                machine: function.machine,
                edge: patch.incoming_edge,
            };
            let input_terminal = PsiRealizationSite::Node(NodeLocation {
                machine: function.machine,
                block: patch.target,
                node: 0,
            });
            let output_clone = PsiRealizationSite::Node(patch.predecessor);
            let mut provenance = vec![
                provenance_realized_at(
                    input_edge,
                    output_clone,
                    &incoming.provenance,
                    &incoming.work,
                ),
                provenance_realized_at(
                    input_terminal,
                    output_clone,
                    &terminal.provenance,
                    &terminal.work,
                ),
                realized_at_own_site(input_terminal, &terminal.provenance, &terminal.work),
            ];
            canonicalize_provenance(&mut provenance);
            let mut blocks = vec![patch.predecessor.block, patch.target];
            blocks.sort();
            blocks.dedup();
            Some((blocks, provenance))
        }
    }
    pub(crate) mod threading {
        use crate::candidates::{
            canonicalize_provenance, provenance_realized_at, realize_node_provenance,
            realized_at_own_site,
        };
        use abstract_operations::ValueBinding;
        use optimization_unit::{
            NodeLocation, OwnershipFrontierSite, PsiOptimizationFunction, PsiOptimizationUnit,
            PsiRealizationSite, ValueDefinition,
        };
        use semantic_vocabulary::{BlockId, EdgeId};
        use std::collections::{BTreeMap, BTreeSet};

        pub fn reconstruct_linear_thread_bindings(
            parameters: &[ValueDefinition],
            incoming: &[ValueBinding],
            outgoing: &[ValueBinding],
        ) -> Option<Vec<ValueBinding>> {
            if parameters.len() != incoming.len() {
                return None;
            }
            let replacements = parameters
                .iter()
                .zip(incoming)
                .map(|(parameter, binding)| {
                    (binding.parameter == parameter.value
                        && binding.scalar_type == parameter.scalar_type)
                        .then_some((parameter.value, (binding.argument, binding.scalar_type)))
                })
                .collect::<Option<BTreeMap<_, _>>>()?;
            Some(
                outgoing
                    .iter()
                    .map(|binding| {
                        replacements.get(&binding.argument).map_or(
                            *binding,
                            |(argument, scalar_type)| ValueBinding {
                                parameter: binding.parameter,
                                argument: *argument,
                                scalar_type: *scalar_type,
                            },
                        )
                    })
                    .collect(),
            )
        }

        pub fn reconstruct_linear_thread_ownership_is_identity(
            unit: &PsiOptimizationUnit,
            function: &PsiOptimizationFunction,
            incoming: EdgeId,
            empty: BlockId,
            outgoing: EdgeId,
            target: BlockId,
        ) -> bool {
            let sites = [
                OwnershipFrontierSite::EdgeEntry(incoming),
                OwnershipFrontierSite::EdgeExit(incoming),
                OwnershipFrontierSite::BlockEntry(empty),
                OwnershipFrontierSite::EdgeEntry(outgoing),
                OwnershipFrontierSite::EdgeExit(outgoing),
                OwnershipFrontierSite::BlockEntry(target),
            ];
            let facts = sites.map(|site| {
                unit.ownership_frontier_facts
                    .iter()
                    .find(|fact| fact.machine == function.machine && fact.site == site)
            });
            if facts.iter().all(Option::is_none) {
                return function.structural_parameters.is_empty()
                    && function.entry_claim_declarations.is_empty()
                    && function.declared_places.is_empty();
            }
            facts.iter().all(Option::is_some)
                && facts
                    .windows(2)
                    .all(|pair| pair[0].unwrap().snapshot == pair[1].unwrap().snapshot)
        }

        pub(crate) fn reconstruct_linear_thread_accounting(
            function: &PsiOptimizationFunction,
            predecessor: NodeLocation,
            empty: NodeLocation,
        ) -> Option<(Vec<BlockId>, Vec<optimization_unit::ProvenanceRewrite>)> {
            let predecessor_node = function
                .block(predecessor.block)?
                .nodes
                .get(usize::try_from(predecessor.node).ok()?)?;
            let empty_node = function
                .block(empty.block)?
                .nodes
                .get(usize::try_from(empty.node).ok()?)?;
            let predecessor_edge = predecessor_node.successors.first()?;
            let empty_edge = empty_node.successors.first()?;
            let output_site = PsiRealizationSite::Edge {
                machine: function.machine,
                edge: predecessor_edge.psi_edge,
            };
            let predecessor_site = output_site;
            let empty_site = PsiRealizationSite::Edge {
                machine: function.machine,
                edge: empty_edge.psi_edge,
            };
            let mut affected = BTreeSet::from([predecessor.block, empty.block]);
            let mut realized = vec![
                provenance_realized_at(
                    predecessor_site,
                    output_site,
                    &predecessor_edge.provenance,
                    &predecessor_edge.work,
                ),
                provenance_realized_at(
                    empty_site,
                    output_site,
                    &empty_edge.provenance,
                    &empty_edge.work,
                ),
            ];
            effect_audit(
                function,
                empty,
                Some(predecessor),
                &mut affected,
                &mut realized,
            )?;
            Some((affected.into_iter().collect(), realized))
        }

        pub(crate) fn reconstruct_path_thread_accounting(
            function: &PsiOptimizationFunction,
            empty: NodeLocation,
            incoming_edges: &[EdgeId],
        ) -> Option<(Vec<BlockId>, Vec<optimization_unit::ProvenanceRewrite>)> {
            let empty_node = function
                .block(empty.block)?
                .nodes
                .get(usize::try_from(empty.node).ok()?)?;
            let outgoing = empty_node.successors.first()?;
            let outgoing_site = PsiRealizationSite::Edge {
                machine: function.machine,
                edge: outgoing.psi_edge,
            };
            let incoming_set = incoming_edges.iter().copied().collect::<BTreeSet<_>>();
            if incoming_set.len() != incoming_edges.len() || incoming_set.is_empty() {
                return None;
            }
            let mut affected = BTreeSet::from([empty.block]);
            let mut realized = Vec::new();
            for block in &function.blocks {
                for node in &block.nodes {
                    for edge in &node.successors {
                        if !incoming_set.contains(&edge.psi_edge) || edge.target != empty.block {
                            continue;
                        }
                        affected.insert(block.id);
                        let site = PsiRealizationSite::Edge {
                            machine: function.machine,
                            edge: edge.psi_edge,
                        };
                        realized.push(realized_at_own_site(site, &edge.provenance, &edge.work));
                        realized.push(provenance_realized_at(
                            outgoing_site,
                            site,
                            &outgoing.provenance,
                            &outgoing.work,
                        ));
                    }
                }
            }
            if realized.len() != incoming_edges.len().checked_mul(2)? {
                return None;
            }
            effect_audit(function, empty, None, &mut affected, &mut realized)?;
            Some((affected.into_iter().collect(), realized))
        }

        fn effect_audit(
            function: &PsiOptimizationFunction,
            empty: NodeLocation,
            except: Option<NodeLocation>,
            affected: &mut BTreeSet<BlockId>,
            realized: &mut Vec<optimization_unit::ProvenanceRewrite>,
        ) -> Option<()> {
            let mut expected_effect = 0u64;
            for block in &function.blocks {
                if block.id == empty.block {
                    continue;
                }
                for (node_index, node) in block.nodes.iter().enumerate() {
                    let location = NodeLocation {
                        machine: function.machine,
                        block: block.id,
                        node: u32::try_from(node_index).ok()?,
                    };
                    let effect_changes = node.effect.input != expected_effect
                        || node.effect.output != expected_effect.checked_add(1)?;
                    if effect_changes && except != Some(location) {
                        affected.insert(block.id);
                        realize_node_provenance(
                            function.machine,
                            block.id,
                            std::iter::once((node_index, node)),
                            realized,
                        )?;
                    }
                    expected_effect = expected_effect.checked_add(1)?;
                }
            }
            canonicalize_provenance(realized);
            Some(())
        }
    }
    pub(crate) fn realize_merged_target_provenance(
        function: &PsiOptimizationFunction,
        predecessor: NodeLocation,
        target: &OptimizationBlock,
        realized: &mut Vec<optimization_unit::ProvenanceRewrite>,
    ) -> Option<()> {
        for (node_index, node) in target.nodes.iter().enumerate() {
            if node.provenance.is_empty() {
                continue;
            }
            let node_index = u32::try_from(node_index).ok()?;
            let site = |block, node| {
                PsiRealizationSite::Node(NodeLocation {
                    machine: function.machine,
                    block,
                    node,
                })
            };
            realized.push(provenance_realized_at(
                site(target.id, node_index),
                site(predecessor.block, predecessor.node.checked_add(node_index)?),
                &node.provenance,
                &node.work,
            ));
        }
        Some(())
    }

    pub(crate) fn preserve_edge_custody(
        node: &optimization_unit::OptimizationNode,
    ) -> Vec<OptimizationEdge> {
        let expected = expected_edges(&node.operation);
        expected
            .into_iter()
            .map(|mut edge| {
                if let Some(existing) = node.edge(edge.psi_edge) {
                    edge.provenance = existing.provenance.clone();
                    edge.work = existing.work.clone();
                }
                edge
            })
            .collect()
    }

    pub(crate) fn rewrite_scalar_substitutions(
        operation: &mut O,
        substitutions: &[ScalarSubstitution],
        machine: MachineId,
        removed_block: BlockId,
    ) {
        for substitution in substitutions {
            rewrite_block_parameter_operation(
                operation,
                RedundantBlockParameterRewrite {
                    machine,
                    block: removed_block,
                    position: 0,
                    parameter: substitution.from,
                    replacement: substitution.to,
                    scalar_type: substitution.scalar_type,
                },
            );
        }
    }
}
pub(crate) mod sparse_conditional_constant_propagation {
    use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
    use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate, PsiRewritePatch};

    mod boolean_candidate {
        use crate::candidates::realized_at_own_site;
        use crate::candidates::sparse_conditional_constant_propagation::{
            boolean_evaluation, observation_at,
        };
        use crate::candidates::{
            candidate_block, candidate_function, candidate_node, require_realized_rows,
        };
        use crate::{
            OptimizationUnitValidationError, ValidatedPsiRewrite,
            reconstruct_closed_scalar_node_boundary,
        };
        use optimization_core::{
            AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationValidatorIdentity,
        };
        use optimization_unit::{
            BooleanConstantRewrite, IntegerEvaluationWitness, PsiOptimizationUnit,
            PsiRealizationSite, PsiRewriteCandidate, PsiRewritePatch,
        };
        use semantic_vocabulary::ScalarType;

        pub(crate) fn validate_boolean_evaluation_candidate(
            input: &PsiOptimizationUnit,
            candidate: &PsiRewriteCandidate,
        ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
            crate::candidates::contract::admit_origin(input, candidate)?;
            let expected_analyses = match candidate.scalar_evaluation_witness() {
                Some(IntegerEvaluationWitness::RangeAgainstRange { .. }) => {
                    AnalysisSet::new([AnalysisKind::ValueRanges])
                }
                Some(IntegerEvaluationWitness::RangeAgainstConstant { .. }) => {
                    AnalysisSet::new([AnalysisKind::ScalarConstants, AnalysisKind::ValueRanges])
                }
                Some(_) => AnalysisSet::new([AnalysisKind::ScalarConstants]),
                None => {
                    return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
                }
            };
            if candidate.required_analyses() != expected_analyses
                || candidate.invalidated_analyses()
                    != AnalysisInvalidationSet::new([AnalysisKind::UseDefinition])
                || !candidate.substitutions().is_empty()
            {
                return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
            }
            candidate_patch!(patch, ReplaceBooleanOperationWithConstant, candidate);
            if candidate.node_decision_point() != Some(patch.location) {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            let function = candidate_function(input, patch.location.machine)?;
            let block = candidate_block(function, patch.location.block)?;
            let node = candidate_node(block, patch.location.node)?;
            let input_observation = observation_at(input, patch.location)
                .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
            let input_live = reconstruct_closed_scalar_node_boundary(input, patch.location)
                .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
            let site = PsiRealizationSite::Node(patch.location);
            let accepted_provenance =
                vec![realized_at_own_site(site, &node.provenance, &node.work)];
            require_realized_rows(candidate, &accepted_provenance)?;
            let (source_operation, result, evaluated, safety_class) =
                boolean_evaluation::evaluate(input, function, node, candidate, patch.location)?;
            if candidate.safety_class() != safety_class {
                return Err(OptimizationUnitValidationError::CandidateSafetyClassMismatch);
            }
            if patch
                != (BooleanConstantRewrite {
                    location: patch.location,
                    source_operation,
                    result,
                    constant: evaluated,
                })
            {
                return Err(OptimizationUnitValidationError::CandidateEvaluationMismatch);
            }
            let output = crate::candidates::constant_installation::install_evaluated_constant(
                input,
                patch.location,
                abstract_operations::AbstractOperation::BooleanConstant {
                    psi_operation: patch.source_operation,
                    result: patch.result,
                    value: patch.constant,
                },
                patch.result,
                ScalarType::Boolean,
                &input_observation,
                &input_live,
            )?;
            Ok(ValidatedPsiRewrite {
                unit: output,
                candidate: candidate.identity(),
                validator: OptimizationValidatorIdentity::from_canonical_bytes(
                    b"omega.validator.boolean-evaluation.v4",
                ),
                provenance: accepted_provenance,
            })
        }
    }
    mod boolean_evaluation {
        mod boolean_literals {
            use abstract_operations::AbstractOperation as O;
            use optimization_core::OptimizationSafetyClass;
            use optimization_unit::{
                IntegerEvaluationWitness, OptimizationNode, PsiOptimizationFunction,
                PsiRewriteCandidate,
            };

            use crate::OptimizationUnitValidationError;

            use super::super::integer_evaluation::literal_boolean_fact;
            use super::{BooleanEvaluation, rule_identity};

            pub(super) fn evaluate(
                function: &PsiOptimizationFunction,
                node: &OptimizationNode,
                candidate: &PsiRewriteCandidate,
            ) -> Result<BooleanEvaluation, OptimizationUnitValidationError> {
                rule_identity::validate(&node.operation, candidate.rule())?;
                match node.operation {
                    O::BooleanNot {
                        psi_operation,
                        result,
                        operand,
                    } => {
                        let operand_fact = candidate
                            .scalar_evaluation_witness()
                            .and_then(IntegerEvaluationWitness::unary_operand)
                            .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
                        let operand = literal_boolean_fact(
                            function,
                            candidate.input(),
                            operand,
                            operand_fact,
                        )
                        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
                        Ok((
                            psi_operation,
                            result,
                            !operand,
                            OptimizationSafetyClass::ExactOperationSemantics,
                        ))
                    }
                    O::BooleanEqual {
                        psi_operation,
                        result,
                        left,
                        right,
                    } => {
                        let (left_fact, right_fact) = candidate
                            .scalar_evaluation_witness()
                            .and_then(IntegerEvaluationWitness::binary_operands)
                            .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
                        let left =
                            literal_boolean_fact(function, candidate.input(), left, left_fact)
                                .ok_or(
                                    OptimizationUnitValidationError::CandidateOperandFactMismatch,
                                )?;
                        let right =
                            literal_boolean_fact(function, candidate.input(), right, right_fact)
                                .ok_or(
                                    OptimizationUnitValidationError::CandidateOperandFactMismatch,
                                )?;
                        Ok((
                            psi_operation,
                            result,
                            left == right,
                            OptimizationSafetyClass::ExactOperationSemantics,
                        ))
                    }
                    _ => Err(OptimizationUnitValidationError::CandidatePatchMismatch),
                }
            }
        }
        mod integer_comparisons {
            mod constant {
                use abstract_operations::AbstractOperation as O;
                use optimization_core::OptimizationSafetyClass;
                use optimization_unit::{
                    IntegerEvaluationWitness, OptimizationNode, PsiOptimizationFunction,
                    PsiRewriteCandidate,
                };

                use crate::OptimizationUnitValidationError;

                use super::super::{BooleanEvaluation, rule_identity};
use super::IntegerComparisonShape;
use crate::candidates::sparse_conditional_constant_propagation::integer_evaluation::literal_integer_fact;
use crate::candidates::sparse_conditional_constant_propagation::snapshot_reconstruction::validator_integer_value_type;

                pub(super) fn evaluate(
                    function: &PsiOptimizationFunction,
                    node: &OptimizationNode,
                    candidate: &PsiRewriteCandidate,
                    shape: IntegerComparisonShape,
                ) -> Result<BooleanEvaluation, OptimizationUnitValidationError> {
                    rule_identity::validate(&node.operation, candidate.rule())?;
                    let (left_fact, right_fact) = candidate
                        .scalar_evaluation_witness()
                        .and_then(IntegerEvaluationWitness::binary_operands)
                        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
                    let left_value =
                        literal_integer_fact(function, candidate.input(), shape.left, left_fact)
                            .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
                    let right_value =
                        literal_integer_fact(function, candidate.input(), shape.right, right_fact)
                            .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
                    let left_type = validator_integer_value_type(function, shape.left)
                        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
                    if validator_integer_value_type(function, shape.right) != Some(left_type) {
                        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
                    }
                    let ordering = left_type
                        .compare(left_value, right_value)
                        .ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;
                    let constant = match node.operation {
                        O::IntegerEqual { .. } => ordering.is_eq(),
                        O::IntegerLessThan { .. } => ordering.is_lt(),
                        O::IntegerLessOrEqual { .. } => !ordering.is_gt(),
                        _ => unreachable!(),
                    };
                    Ok((
                        shape.psi_operation,
                        shape.result,
                        constant,
                        OptimizationSafetyClass::ExactOperationSemantics,
                    ))
                }
            }
            mod range_against_constant {
                mod semantics {
                    use abstract_operations::AbstractOperation as O;
                    use optimization_core::OptimizationRuleIdentity;
                    use semantic_vocabulary::{IntegerType, IntegerValue};

                    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
                    pub(crate) enum ValidatedIntegerRangeComparisonKind {
                        RangeEqualConstant,
                        ConstantEqualRange,
                        RangeLessThanConstant,
                        ConstantLessThanRange,
                        RangeLessOrEqualConstant,
                        ConstantLessOrEqualRange,
                    }

                    pub(crate) fn classify(
                        rule: OptimizationRuleIdentity,
                        operation: &O,
                    ) -> Option<ValidatedIntegerRangeComparisonKind> {
                        let kind = if rule
                            == OptimizationRuleIdentity::from_canonical_bytes(
                                b"omega.psi-rule.integer-equal-range-constant.v1",
                            ) {
                            ValidatedIntegerRangeComparisonKind::RangeEqualConstant
                        } else if rule
                            == OptimizationRuleIdentity::from_canonical_bytes(
                                b"omega.psi-rule.integer-equal-constant-range.v1",
                            )
                        {
                            ValidatedIntegerRangeComparisonKind::ConstantEqualRange
                        } else if rule
                            == OptimizationRuleIdentity::from_canonical_bytes(
                                b"omega.psi-rule.integer-less-than-range-constant.v1",
                            )
                        {
                            ValidatedIntegerRangeComparisonKind::RangeLessThanConstant
                        } else if rule
                            == OptimizationRuleIdentity::from_canonical_bytes(
                                b"omega.psi-rule.integer-less-than-constant-range.v1",
                            )
                        {
                            ValidatedIntegerRangeComparisonKind::ConstantLessThanRange
                        } else if rule
                            == OptimizationRuleIdentity::from_canonical_bytes(
                                b"omega.psi-rule.integer-less-or-equal-range-constant.v1",
                            )
                        {
                            ValidatedIntegerRangeComparisonKind::RangeLessOrEqualConstant
                        } else if rule
                            == OptimizationRuleIdentity::from_canonical_bytes(
                                b"omega.psi-rule.integer-less-or-equal-constant-range.v1",
                            )
                        {
                            ValidatedIntegerRangeComparisonKind::ConstantLessOrEqualRange
                        } else {
                            return None;
                        };
                        match (kind, operation) {
                            (
                                ValidatedIntegerRangeComparisonKind::RangeEqualConstant
                                | ValidatedIntegerRangeComparisonKind::ConstantEqualRange,
                                O::IntegerEqual { .. },
                            )
                            | (
                                ValidatedIntegerRangeComparisonKind::RangeLessThanConstant
                                | ValidatedIntegerRangeComparisonKind::ConstantLessThanRange,
                                O::IntegerLessThan { .. },
                            )
                            | (
                                ValidatedIntegerRangeComparisonKind::RangeLessOrEqualConstant
                                | ValidatedIntegerRangeComparisonKind::ConstantLessOrEqualRange,
                                O::IntegerLessOrEqual { .. },
                            ) => Some(kind),
                            _ => None,
                        }
                    }

                    pub(crate) fn evaluate(
                        kind: ValidatedIntegerRangeComparisonKind,
                        scalar_type: IntegerType,
                        minimum: IntegerValue,
                        maximum: IntegerValue,
                        constant: IntegerValue,
                    ) -> Option<bool> {
                        let minimum_to_constant = scalar_type.compare(minimum, constant)?;
                        let maximum_to_constant = scalar_type.compare(maximum, constant)?;
                        match kind {
                            ValidatedIntegerRangeComparisonKind::RangeEqualConstant
                            | ValidatedIntegerRangeComparisonKind::ConstantEqualRange => {
                                if minimum_to_constant.is_eq() && maximum_to_constant.is_eq() {
                                    Some(true)
                                } else if minimum_to_constant.is_gt() || maximum_to_constant.is_lt()
                                {
                                    Some(false)
                                } else {
                                    None
                                }
                            }
                            ValidatedIntegerRangeComparisonKind::RangeLessThanConstant => {
                                maximum_to_constant
                                    .is_lt()
                                    .then_some(true)
                                    .or_else(|| (!minimum_to_constant.is_lt()).then_some(false))
                            }
                            ValidatedIntegerRangeComparisonKind::ConstantLessThanRange => {
                                minimum_to_constant
                                    .is_gt()
                                    .then_some(true)
                                    .or_else(|| (!maximum_to_constant.is_gt()).then_some(false))
                            }
                            ValidatedIntegerRangeComparisonKind::RangeLessOrEqualConstant => {
                                (!maximum_to_constant.is_gt())
                                    .then_some(true)
                                    .or_else(|| minimum_to_constant.is_gt().then_some(false))
                            }
                            ValidatedIntegerRangeComparisonKind::ConstantLessOrEqualRange => {
                                (!minimum_to_constant.is_lt())
                                    .then_some(true)
                                    .or_else(|| maximum_to_constant.is_lt().then_some(false))
                            }
                        }
                    }
                }
                pub(crate) use semantics::ValidatedIntegerRangeComparisonKind;

                use optimization_core::{
                    AnalysisKind, OptimizationSafetyClass, ScalarConstantFactIdentity,
                    ValueRangeFactIdentity,
                };
                use optimization_unit::{
                    NodeLocation, OptimizationNode, PsiOptimizationFunction, PsiOptimizationUnit,
                    PsiRewriteCandidate,
                };

                use crate::{OptimizationUnitValidationError, current_value_ranges};

                use super::super::BooleanEvaluation;
use super::IntegerComparisonShape;
use crate::candidates::sparse_conditional_constant_propagation::integer_evaluation::direct_literal_integer_fact;
use crate::candidates::sparse_conditional_constant_propagation::snapshot_reconstruction::validator_integer_value_type;
use semantics::{classify, evaluate as evaluate_semantics};

                #[allow(clippy::too_many_arguments)]
                pub(super) fn evaluate(
                    input: &PsiOptimizationUnit,
                    function: &PsiOptimizationFunction,
                    node: &OptimizationNode,
                    candidate: &PsiRewriteCandidate,
                    location: NodeLocation,
                    shape: IntegerComparisonShape,
                    range_fact: ValueRangeFactIdentity,
                    constant_fact: ScalarConstantFactIdentity,
                ) -> Result<BooleanEvaluation, OptimizationUnitValidationError> {
                    if !candidate
                        .required_analyses()
                        .contains(AnalysisKind::ValueRanges)
                    {
                        return Err(
                            OptimizationUnitValidationError::CandidateAnalysisContractMismatch,
                        );
                    }
                    let kind = classify(candidate.rule(), &node.operation)
                        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
                    let (range_operand, constant_operand) = match kind {
                        ValidatedIntegerRangeComparisonKind::RangeEqualConstant
                        | ValidatedIntegerRangeComparisonKind::RangeLessThanConstant
                        | ValidatedIntegerRangeComparisonKind::RangeLessOrEqualConstant => {
                            (shape.left, shape.right)
                        }
                        ValidatedIntegerRangeComparisonKind::ConstantEqualRange
                        | ValidatedIntegerRangeComparisonKind::ConstantLessThanRange
                        | ValidatedIntegerRangeComparisonKind::ConstantLessOrEqualRange => {
                            (shape.right, shape.left)
                        }
                    };
                    let constant_value = direct_literal_integer_fact(
                        function,
                        candidate.input(),
                        constant_operand,
                        constant_fact,
                    )
                    .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
                    let range =
                        current_value_ranges::independently_reconstruct_value_range_fact_at(
                            input,
                            range_fact,
                            function.machine,
                            range_operand,
                            location.block,
                            location.node,
                        )
                        .ok_or(OptimizationUnitValidationError::CurrentValueRangeFactMismatch)?;
                    if validator_integer_value_type(function, constant_operand)
                        != Some(range.scalar_type)
                    {
                        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
                    }
                    let constant = evaluate_semantics(
                        kind,
                        range.scalar_type,
                        range.minimum,
                        range.maximum,
                        constant_value,
                    )
                    .ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;
                    Ok((
                        shape.psi_operation,
                        shape.result,
                        constant,
                        OptimizationSafetyClass::ProofCertified,
                    ))
                }
            }
            mod range_against_range {
                mod semantics {
                    use abstract_operations::AbstractOperation as O;
                    use optimization_core::OptimizationRuleIdentity;
                    use semantic_vocabulary::{IntegerType, IntegerValue};

                    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
                    pub(crate) enum ValidatedIntegerRangePairComparisonKind {
                        Equal,
                        LessThan,
                        LessOrEqual,
                    }

                    pub(crate) fn classify(
                        rule: OptimizationRuleIdentity,
                        operation: &O,
                    ) -> Option<ValidatedIntegerRangePairComparisonKind> {
                        let kind = if rule
                            == OptimizationRuleIdentity::from_canonical_bytes(
                                b"omega.psi-rule.integer-equal-range-range.v1",
                            ) {
                            ValidatedIntegerRangePairComparisonKind::Equal
                        } else if rule
                            == OptimizationRuleIdentity::from_canonical_bytes(
                                b"omega.psi-rule.integer-less-than-range-range.v1",
                            )
                        {
                            ValidatedIntegerRangePairComparisonKind::LessThan
                        } else if rule
                            == OptimizationRuleIdentity::from_canonical_bytes(
                                b"omega.psi-rule.integer-less-or-equal-range-range.v1",
                            )
                        {
                            ValidatedIntegerRangePairComparisonKind::LessOrEqual
                        } else {
                            return None;
                        };
                        match (kind, operation) {
                            (
                                ValidatedIntegerRangePairComparisonKind::Equal,
                                O::IntegerEqual { .. },
                            )
                            | (
                                ValidatedIntegerRangePairComparisonKind::LessThan,
                                O::IntegerLessThan { .. },
                            )
                            | (
                                ValidatedIntegerRangePairComparisonKind::LessOrEqual,
                                O::IntegerLessOrEqual { .. },
                            ) => Some(kind),
                            _ => None,
                        }
                    }

                    pub(crate) fn evaluate(
                        kind: ValidatedIntegerRangePairComparisonKind,
                        scalar_type: IntegerType,
                        same_value: bool,
                        left_minimum: IntegerValue,
                        left_maximum: IntegerValue,
                        right_minimum: IntegerValue,
                        right_maximum: IntegerValue,
                    ) -> Option<bool> {
                        if same_value {
                            return Some(!matches!(
                                kind,
                                ValidatedIntegerRangePairComparisonKind::LessThan
                            ));
                        }
                        let left_maximum_to_right_minimum =
                            scalar_type.compare(left_maximum, right_minimum)?;
                        let left_minimum_to_right_maximum =
                            scalar_type.compare(left_minimum, right_maximum)?;
                        match kind {
                            ValidatedIntegerRangePairComparisonKind::Equal => {
                                let both_equal_singletons = scalar_type
                                    .compare(left_minimum, left_maximum)?
                                    .is_eq()
                                    && scalar_type.compare(right_minimum, right_maximum)?.is_eq()
                                    && scalar_type.compare(left_minimum, right_minimum)?.is_eq();
                                both_equal_singletons.then_some(true).or_else(|| {
                                    (left_maximum_to_right_minimum.is_lt()
                                        || left_minimum_to_right_maximum.is_gt())
                                    .then_some(false)
                                })
                            }
                            ValidatedIntegerRangePairComparisonKind::LessThan => {
                                left_maximum_to_right_minimum
                                    .is_lt()
                                    .then_some(true)
                                    .or_else(|| {
                                        (!left_minimum_to_right_maximum.is_lt()).then_some(false)
                                    })
                            }
                            ValidatedIntegerRangePairComparisonKind::LessOrEqual => {
                                (!left_maximum_to_right_minimum.is_gt())
                                    .then_some(true)
                                    .or_else(|| {
                                        left_minimum_to_right_maximum.is_gt().then_some(false)
                                    })
                            }
                        }
                    }
                }
                use optimization_core::{
                    AnalysisKind, OptimizationSafetyClass, ValueRangeFactIdentity,
                };
                use optimization_unit::{
                    NodeLocation, OptimizationNode, PsiOptimizationFunction, PsiOptimizationUnit,
                    PsiRewriteCandidate,
                };

                use crate::{OptimizationUnitValidationError, current_value_ranges};

                use super::super::BooleanEvaluation;
use super::IntegerComparisonShape;
use crate::candidates::sparse_conditional_constant_propagation::snapshot_reconstruction::validator_integer_value_type;
use semantics::{classify, evaluate as evaluate_semantics};

                #[allow(clippy::too_many_arguments)]
                pub(super) fn evaluate(
                    input: &PsiOptimizationUnit,
                    function: &PsiOptimizationFunction,
                    node: &OptimizationNode,
                    candidate: &PsiRewriteCandidate,
                    location: NodeLocation,
                    shape: IntegerComparisonShape,
                    left_range_fact: ValueRangeFactIdentity,
                    right_range_fact: ValueRangeFactIdentity,
                ) -> Result<BooleanEvaluation, OptimizationUnitValidationError> {
                    if !candidate
                        .required_analyses()
                        .contains(AnalysisKind::ValueRanges)
                    {
                        return Err(
                            OptimizationUnitValidationError::CandidateAnalysisContractMismatch,
                        );
                    }
                    let kind = classify(candidate.rule(), &node.operation)
                        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
                    let left_range =
                        current_value_ranges::independently_reconstruct_value_range_fact_at(
                            input,
                            left_range_fact,
                            function.machine,
                            shape.left,
                            location.block,
                            location.node,
                        )
                        .ok_or(OptimizationUnitValidationError::CurrentValueRangeFactMismatch)?;
                    let right_range =
                        current_value_ranges::independently_reconstruct_value_range_fact_at(
                            input,
                            right_range_fact,
                            function.machine,
                            shape.right,
                            location.block,
                            location.node,
                        )
                        .ok_or(OptimizationUnitValidationError::CurrentValueRangeFactMismatch)?;
                    if left_range.scalar_type != right_range.scalar_type
                        || validator_integer_value_type(function, shape.left)
                            != Some(left_range.scalar_type)
                        || validator_integer_value_type(function, shape.right)
                            != Some(right_range.scalar_type)
                    {
                        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
                    }
                    let constant = evaluate_semantics(
                        kind,
                        left_range.scalar_type,
                        shape.left == shape.right,
                        left_range.minimum,
                        left_range.maximum,
                        right_range.minimum,
                        right_range.maximum,
                    )
                    .ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;
                    Ok((
                        shape.psi_operation,
                        shape.result,
                        constant,
                        OptimizationSafetyClass::ProofCertified,
                    ))
                }
            }
            use abstract_operations::AbstractOperation as O;
            use optimization_unit::{
                IntegerEvaluationWitness, NodeLocation, OptimizationNode, PsiOptimizationFunction,
                PsiOptimizationUnit, PsiRewriteCandidate,
            };
            use semantic_vocabulary::{OperationId, ValueId};

            use crate::OptimizationUnitValidationError;

            use super::BooleanEvaluation;

            #[derive(Clone, Copy)]
            pub(super) struct IntegerComparisonShape {
                pub psi_operation: OperationId,
                pub result: ValueId,
                pub left: ValueId,
                pub right: ValueId,
            }

            pub(super) fn evaluate(
                input: &PsiOptimizationUnit,
                function: &PsiOptimizationFunction,
                node: &OptimizationNode,
                candidate: &PsiRewriteCandidate,
                location: NodeLocation,
            ) -> Result<BooleanEvaluation, OptimizationUnitValidationError> {
                let shape = match node.operation {
                    abstract_operations::integer_equal_less_pair_pattern!(
                        psi_operation,
                        result,
                        left,
                        right,
                        ..
                    )
                    | O::IntegerLessOrEqual {
                        psi_operation,
                        result,
                        left,
                        right,
                    } => IntegerComparisonShape {
                        psi_operation,
                        result,
                        left,
                        right,
                    },
                    _ => return Err(OptimizationUnitValidationError::CandidatePatchMismatch),
                };
                if let Some((left_range_fact, right_range_fact)) = candidate
                    .scalar_evaluation_witness()
                    .and_then(IntegerEvaluationWitness::range_against_range)
                {
                    return range_against_range::evaluate(
                        input,
                        function,
                        node,
                        candidate,
                        location,
                        shape,
                        left_range_fact,
                        right_range_fact,
                    );
                }
                if let Some((range_fact, constant_fact)) = candidate
                    .scalar_evaluation_witness()
                    .and_then(IntegerEvaluationWitness::range_against_constant)
                {
                    return range_against_constant::evaluate(
                        input,
                        function,
                        node,
                        candidate,
                        location,
                        shape,
                        range_fact,
                        constant_fact,
                    );
                }
                constant::evaluate(function, node, candidate, shape)
            }
        }
        mod rule_identity {
            use abstract_operations::AbstractOperation as O;
            use optimization_core::OptimizationRuleIdentity;

            use crate::OptimizationUnitValidationError;

            pub(super) fn validate(
                operation: &O,
                actual: OptimizationRuleIdentity,
            ) -> Result<(), OptimizationUnitValidationError> {
                let canonical_name: &[u8] = match operation {
                    O::BooleanNot { .. } => b"omega.psi-rule.boolean-not-constants.v1",
                    O::BooleanEqual { .. } => b"omega.psi-rule.boolean-equal-constants.v1",
                    O::IntegerEqual { .. } => b"omega.psi-rule.integer-equal-constants.v1",
                    O::IntegerLessThan { .. } => b"omega.psi-rule.integer-less-than-constants.v1",
                    O::IntegerLessOrEqual { .. } => {
                        b"omega.psi-rule.integer-less-or-equal-constants.v1"
                    }
                    _ => return Err(OptimizationUnitValidationError::CandidatePatchMismatch),
                };
                let expected = OptimizationRuleIdentity::from_canonical_bytes(canonical_name);
                if actual != expected {
                    return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
                }
                Ok(())
            }
        }
        use abstract_operations::AbstractOperation as O;
        use optimization_core::OptimizationSafetyClass;
        use optimization_unit::{
            NodeLocation, OptimizationNode, PsiOptimizationFunction, PsiOptimizationUnit,
            PsiRewriteCandidate,
        };
        use semantic_vocabulary::{OperationId, ValueId};

        use crate::OptimizationUnitValidationError;

        pub(super) type BooleanEvaluation = (OperationId, ValueId, bool, OptimizationSafetyClass);

        pub(super) fn evaluate(
            input: &PsiOptimizationUnit,
            function: &PsiOptimizationFunction,
            node: &OptimizationNode,
            candidate: &PsiRewriteCandidate,
            location: NodeLocation,
        ) -> Result<BooleanEvaluation, OptimizationUnitValidationError> {
            match node.operation {
                abstract_operations::boolean_not_or_equal_pair_pattern!(..) => {
                    boolean_literals::evaluate(function, node, candidate)
                }
                abstract_operations::integer_equal_less_pair_pattern!(..)
                | O::IntegerLessOrEqual { .. } => {
                    integer_comparisons::evaluate(input, function, node, candidate, location)
                }
                _ => Err(OptimizationUnitValidationError::CandidatePatchMismatch),
            }
        }
    }
    mod integer_candidate {
        use crate::candidates::realized_at_own_site;
        use crate::candidates::sparse_conditional_constant_propagation::observation_at;
        use crate::{
            OptimizationUnitValidationError, ValidatedPsiRewrite,
            reconstruct_closed_scalar_node_boundary,
        };
        use optimization_core::{
            AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationSafetyClass,
            OptimizationValidatorIdentity,
        };
        use optimization_unit::{
            IntegerConstantRewrite, IntegerEvaluationWitness, OptimizationFact,
            PsiOptimizationUnit, PsiRealizationSite, PsiRewriteCandidate, PsiRewritePatch,
        };
        use semantic_vocabulary::ScalarType;

        use super::integer_evaluation::evaluate_integer_operation;
        use crate::candidates::{
            candidate_block, candidate_function, candidate_node, require_realized_rows,
        };

        pub(crate) fn validate_integer_evaluation_candidate(
            input: &PsiOptimizationUnit,
            candidate: &PsiRewriteCandidate,
        ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
            crate::candidates::contract::admit_origin(input, candidate)?;
            if candidate.required_analyses() != AnalysisSet::new([AnalysisKind::ScalarConstants])
                || candidate.invalidated_analyses()
                    != AnalysisInvalidationSet::new([AnalysisKind::UseDefinition])
                || !candidate.substitutions().is_empty()
            {
                return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
            }
            candidate_patch!(patch, ReplaceIntegerOperationWithConstant, candidate);
            if candidate.node_decision_point() != Some(patch.location) {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            let function = candidate_function(input, patch.location.machine)?;
            let block = candidate_block(function, patch.location.block)?;
            let node = candidate_node(block, patch.location.node)?;
            let input_observation = observation_at(input, patch.location)
                .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
            let input_live = reconstruct_closed_scalar_node_boundary(input, patch.location)
                .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
            let site = PsiRealizationSite::Node(patch.location);
            let accepted_provenance =
                vec![realized_at_own_site(site, &node.provenance, &node.work)];
            require_realized_rows(candidate, &accepted_provenance)?;

            let (source_operation, result, scalar_type, evaluated, safety_class) =
                evaluate_integer_operation(function, node, candidate)?;
            if candidate.safety_class() != safety_class {
                return Err(OptimizationUnitValidationError::CandidateSafetyClassMismatch);
            }
            match (
                safety_class,
                candidate
                    .scalar_evaluation_witness()
                    .and_then(IntegerEvaluationWitness::obligation_fact),
            ) {
                (OptimizationSafetyClass::ProofCertified, Some(identity)) => {
                    let fact = input
                .accepted_obligation_facts
                .iter()
                .find(|fact| {
                    fact.identity == identity
                        && fact.machine == function.machine
                        && fact.owner
                            == optimization_unit::PsiProvenance::Operation(source_operation)
                })
                .ok_or(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)?;
                    if !function.facts.iter().any(|reference| {
                        matches!(
                            reference,
                            OptimizationFact::OperationObligationReference { obligation, support }
                                if *support == source_operation && *obligation == fact.obligation
                        )
                    }) {
                        return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
                    }
                }
                (OptimizationSafetyClass::ProofCertified, None) | (_, Some(_)) => {
                    return Err(
                        OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                    );
                }
                (_, None) => {}
            }
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
            let output = crate::candidates::constant_installation::install_evaluated_constant(
                input,
                patch.location,
                abstract_operations::AbstractOperation::IntegerConstant {
                    psi_operation: patch.source_operation,
                    result: patch.result,
                    scalar_type: ScalarType::Integer(patch.scalar_type),
                    value: patch.constant,
                },
                patch.result,
                ScalarType::Integer(patch.scalar_type),
                &input_observation,
                &input_live,
            )?;
            Ok(ValidatedPsiRewrite {
                unit: output,
                candidate: candidate.identity(),
                validator: OptimizationValidatorIdentity::from_canonical_bytes(
                    b"omega.validator.exact-integer-evaluation.v2",
                ),
                provenance: accepted_provenance,
            })
        }
    }
    mod integer_evaluation {
        mod binary_operation_shape {
            use abstract_operations::AbstractOperation as O;

            use crate::OptimizationUnitValidationError;

            use super::{BinaryIntegerOperation as K, BinaryOperationShape};

            pub(super) fn recognize(
                operation: &O,
            ) -> Result<BinaryOperationShape, OptimizationUnitValidationError> {
                let shape = match operation {
                    O::IntegerArithmetic {
                        psi_operation,
                        result,
                        scalar_type,
                        arithmetic,
                        left,
                        right,
                    } => BinaryOperationShape {
                        kind: K::Arithmetic(arithmetic.policy(), arithmetic.operator()),
                        source: *psi_operation,
                        result: *result,
                        scalar_type: *scalar_type,
                        left: *left,
                        right: *right,
                    },
                    O::IntegerShift {
                        psi_operation,
                        result,
                        value_type,
                        count_type,
                        policy,
                        direction,
                        value,
                        count,
                    } => BinaryOperationShape {
                        kind: K::Shift(*policy, *direction, *count_type),
                        source: *psi_operation,
                        result: *result,
                        scalar_type: *value_type,
                        left: *value,
                        right: *count,
                    },
                    O::IntegerBitwiseAnd {
                        psi_operation,
                        result,
                        scalar_type,
                        left,
                        right,
                    } => BinaryOperationShape {
                        kind: K::BitwiseAnd,
                        source: *psi_operation,
                        result: *result,
                        scalar_type: *scalar_type,
                        left: *left,
                        right: *right,
                    },
                    O::IntegerBitwiseOr {
                        psi_operation,
                        result,
                        scalar_type,
                        left,
                        right,
                    } => BinaryOperationShape {
                        kind: K::BitwiseOr,
                        source: *psi_operation,
                        result: *result,
                        scalar_type: *scalar_type,
                        left: *left,
                        right: *right,
                    },
                    O::IntegerBitwiseXor {
                        psi_operation,
                        result,
                        scalar_type,
                        left,
                        right,
                    } => BinaryOperationShape {
                        kind: K::BitwiseXor,
                        source: *psi_operation,
                        result: *result,
                        scalar_type: *scalar_type,
                        left: *left,
                        right: *right,
                    },
                    _ => return Err(OptimizationUnitValidationError::CandidatePatchMismatch),
                };
                Ok(shape)
            }
        }
        mod binary_semantics {
            use abstract_operations::{
                IntegerArithmeticOperator, IntegerOverflowPolicy, IntegerShiftDirection,
                IntegerShiftPolicy,
            };
            use optimization_core::OptimizationSafetyClass;
            use semantic_vocabulary::{IntegerType, IntegerValue};

            use super::BinaryIntegerOperation;

            pub(super) fn evaluate(
                kind: BinaryIntegerOperation,
                scalar_type: IntegerType,
                left: IntegerValue,
                right: IntegerValue,
            ) -> (Option<IntegerValue>, OptimizationSafetyClass) {
                use IntegerArithmeticOperator as Operator;
                use IntegerOverflowPolicy as Policy;
                use OptimizationSafetyClass::{ExactOperationSemantics, ProofCertified};
                match kind {
                    BinaryIntegerOperation::Arithmetic(policy, operator) => {
                        match (policy, operator) {
                            (Policy::Exact, Operator::Add) => {
                                (scalar_type.exact_add(left, right), ProofCertified)
                            }
                            (Policy::Exact, Operator::Subtract) => {
                                (scalar_type.exact_sub(left, right), ProofCertified)
                            }
                            (Policy::Exact, Operator::Multiply) => {
                                (scalar_type.exact_mul(left, right), ProofCertified)
                            }
                            (Policy::Wrapping, Operator::Add) => (
                                scalar_type.wrapping_add(left, right),
                                ExactOperationSemantics,
                            ),
                            (Policy::Wrapping, Operator::Subtract) => (
                                scalar_type.wrapping_sub(left, right),
                                ExactOperationSemantics,
                            ),
                            (Policy::Wrapping, Operator::Multiply) => (
                                scalar_type.wrapping_mul(left, right),
                                ExactOperationSemantics,
                            ),
                            (Policy::Saturating, Operator::Add) => (
                                scalar_type.saturating_add(left, right),
                                ExactOperationSemantics,
                            ),
                            (Policy::Saturating, Operator::Subtract) => (
                                scalar_type.saturating_sub(left, right),
                                ExactOperationSemantics,
                            ),
                            (Policy::Saturating, Operator::Multiply) => (
                                scalar_type.saturating_mul(left, right),
                                ExactOperationSemantics,
                            ),
                            (Policy::Exact, Operator::Divide) => {
                                (scalar_type.exact_div(left, right), ProofCertified)
                            }
                            (Policy::Exact, Operator::Remainder) => {
                                (scalar_type.exact_rem(left, right), ProofCertified)
                            }
                            (Policy::Wrapping, Operator::Divide) => {
                                (scalar_type.wrapping_div(left, right), ProofCertified)
                            }
                            (Policy::Wrapping, Operator::Remainder) => {
                                (scalar_type.wrapping_rem(left, right), ProofCertified)
                            }
                            (Policy::Saturating, Operator::Divide) => {
                                (scalar_type.saturating_div(left, right), ProofCertified)
                            }
                            (Policy::Saturating, Operator::Remainder) => {
                                (scalar_type.saturating_rem(left, right), ProofCertified)
                            }
                        }
                    }
                    BinaryIntegerOperation::Shift(policy, direction, count_type) => {
                        match (policy, direction) {
                            (IntegerShiftPolicy::Exact { .. }, IntegerShiftDirection::Left) => (
                                scalar_type.exact_shift_left(left, count_type, right),
                                ProofCertified,
                            ),
                            (IntegerShiftPolicy::Exact { .. }, IntegerShiftDirection::Right) => (
                                scalar_type.exact_shift_right(left, count_type, right),
                                ProofCertified,
                            ),
                            (IntegerShiftPolicy::Wrapping, IntegerShiftDirection::Left) => (
                                scalar_type.wrapping_shift_left(left, count_type, right),
                                ExactOperationSemantics,
                            ),
                            (IntegerShiftPolicy::Wrapping, IntegerShiftDirection::Right) => (
                                scalar_type.wrapping_shift_right(left, count_type, right),
                                ExactOperationSemantics,
                            ),
                        }
                    }
                    BinaryIntegerOperation::BitwiseAnd => (
                        scalar_type.bitwise_and(left, right),
                        ExactOperationSemantics,
                    ),
                    BinaryIntegerOperation::BitwiseOr => {
                        (scalar_type.bitwise_or(left, right), ExactOperationSemantics)
                    }
                    BinaryIntegerOperation::BitwiseXor => (
                        scalar_type.bitwise_xor(left, right),
                        ExactOperationSemantics,
                    ),
                }
            }
        }
        mod literal_facts {
            use abstract_operations::AbstractOperation as O;
            use optimization_core::{OptimizationUnitIdentity, ScalarConstantFactIdentity};
            use optimization_unit::{
                IntegerEvaluationWitness, PsiOptimizationFunction, PsiRewriteCandidate,
                ScalarConstantValue, ValueDefinitionSite, literal_scalar_constant_fact_identity,
            };
            use semantic_vocabulary::{IntegerValue, ValueId};

            use crate::OptimizationUnitValidationError;

            use super::super::{scalar_value_definition, validator_scalar_constant_facts};

            pub(super) fn binary_integer_operands(
                function: &PsiOptimizationFunction,
                candidate: &PsiRewriteCandidate,
                left: ValueId,
                right: ValueId,
            ) -> Result<(IntegerValue, IntegerValue), OptimizationUnitValidationError> {
                let Some((left_fact, right_fact)) = candidate
                    .scalar_evaluation_witness()
                    .and_then(IntegerEvaluationWitness::binary_operands)
                else {
                    return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
                };
                let left = literal_integer_fact(function, candidate.input(), left, left_fact)
                    .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
                let right = literal_integer_fact(function, candidate.input(), right, right_fact)
                    .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
                Ok((left, right))
            }

            pub(crate) fn unary_integer_operand(
                function: &PsiOptimizationFunction,
                candidate: &PsiRewriteCandidate,
                operand: ValueId,
            ) -> Result<IntegerValue, OptimizationUnitValidationError> {
                let Some(operand_fact) = candidate
                    .scalar_evaluation_witness()
                    .and_then(IntegerEvaluationWitness::unary_operand)
                else {
                    return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
                };
                literal_integer_fact(function, candidate.input(), operand, operand_fact)
                    .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)
            }

            pub(crate) fn literal_integer_fact(
                function: &PsiOptimizationFunction,
                input: OptimizationUnitIdentity,
                value: ValueId,
                identity: ScalarConstantFactIdentity,
            ) -> Option<IntegerValue> {
                validator_scalar_constant_facts(input, function)
                    .into_iter()
                    .find_map(|(fact_value, constant, fact_identity)| {
                        (fact_value == value && fact_identity == identity)
                            .then_some(constant)
                            .and_then(|constant| match constant {
                                ScalarConstantValue::Integer(value) => Some(value),
                                ScalarConstantValue::Boolean(_) => None,
                            })
                    })
            }

            pub(crate) fn direct_literal_integer_fact(
                function: &PsiOptimizationFunction,
                input: OptimizationUnitIdentity,
                value: ValueId,
                identity: ScalarConstantFactIdentity,
            ) -> Option<IntegerValue> {
                let definition = scalar_value_definition(function, value)?;
                let ValueDefinitionSite::Node { block, node } = definition.site else {
                    return None;
                };
                let operation = &function
                    .block(block)?
                    .nodes
                    .get(usize::try_from(node).ok()?)?
                    .operation;
                let O::IntegerConstant {
                    psi_operation,
                    result,
                    scalar_type,
                    value: constant,
                } = operation
                else {
                    return None;
                };
                if *result != value || *scalar_type != definition.scalar_type {
                    return None;
                }
                let expected = literal_scalar_constant_fact_identity(
                    input,
                    function.machine,
                    definition,
                    ScalarConstantValue::Integer(*constant),
                    *psi_operation,
                )?;
                (identity == expected).then_some(*constant)
            }

            pub(crate) fn literal_boolean_fact(
                function: &PsiOptimizationFunction,
                input: OptimizationUnitIdentity,
                value: ValueId,
                identity: ScalarConstantFactIdentity,
            ) -> Option<bool> {
                validator_scalar_constant_facts(input, function)
                    .into_iter()
                    .find_map(|(fact_value, constant, fact_identity)| {
                        (fact_value == value && fact_identity == identity)
                            .then_some(constant)
                            .and_then(|constant| match constant {
                                ScalarConstantValue::Boolean(value) => Some(value),
                                ScalarConstantValue::Integer(_) => None,
                            })
                    })
            }
        }
        mod rule_identity {
            use abstract_operations::{
                AbstractOperation as O, IntegerArithmetic, IntegerArithmeticOperator,
                IntegerOverflowPolicy, IntegerShiftDirection, IntegerShiftPolicy,
            };
            use optimization_core::OptimizationRuleIdentity;

            use crate::OptimizationUnitValidationError;

            pub(super) fn validate(
                operation: &O,
                actual: OptimizationRuleIdentity,
            ) -> Result<(), OptimizationUnitValidationError> {
                let canonical_name: &[u8] = match operation {
                    O::IntegerArithmetic { arithmetic, .. } => arithmetic_rule_name(*arithmetic),
                    O::IntegerShift {
                        policy, direction, ..
                    } => shift_rule_name(*policy, *direction),
                    O::IntegerExactCast { .. } => b"omega.psi-rule.exact-integer-cast-constants.v1",
                    O::IntegerWiden { .. } => b"omega.psi-rule.integer-widen-constants.v1",
                    O::IntegerBitwiseNot { .. } => {
                        b"omega.psi-rule.integer-bitwise-not-constants.v1"
                    }
                    O::IntegerBitwiseAnd { .. } => {
                        b"omega.psi-rule.integer-bitwise-and-constants.v1"
                    }
                    O::IntegerBitwiseOr { .. } => b"omega.psi-rule.integer-bitwise-or-constants.v1",
                    O::IntegerBitwiseXor { .. } => {
                        b"omega.psi-rule.integer-bitwise-xor-constants.v1"
                    }
                    _ => return Err(OptimizationUnitValidationError::CandidatePatchMismatch),
                };
                let expected = OptimizationRuleIdentity::from_canonical_bytes(canonical_name);
                if actual != expected {
                    return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
                }
                Ok(())
            }

            fn arithmetic_rule_name(arithmetic: IntegerArithmetic) -> &'static [u8] {
                use IntegerArithmeticOperator as Operator;
                use IntegerOverflowPolicy as Policy;
                match (arithmetic.policy(), arithmetic.operator()) {
                    (Policy::Exact, Operator::Add) => {
                        b"omega.psi-rule.exact-integer-add-constants.v1"
                    }
                    (Policy::Exact, Operator::Subtract) => {
                        b"omega.psi-rule.exact-integer-subtract-constants.v1"
                    }
                    (Policy::Exact, Operator::Multiply) => {
                        b"omega.psi-rule.exact-integer-multiply-constants.v1"
                    }
                    (Policy::Exact, Operator::Divide) => {
                        b"omega.psi-rule.exact-integer-divide-constants.v1"
                    }
                    (Policy::Exact, Operator::Remainder) => {
                        b"omega.psi-rule.exact-integer-remainder-constants.v1"
                    }
                    (Policy::Wrapping, Operator::Add) => {
                        b"omega.psi-rule.wrapping-integer-add-constants.v1"
                    }
                    (Policy::Wrapping, Operator::Subtract) => {
                        b"omega.psi-rule.wrapping-integer-subtract-constants.v1"
                    }
                    (Policy::Wrapping, Operator::Multiply) => {
                        b"omega.psi-rule.wrapping-integer-multiply-constants.v1"
                    }
                    (Policy::Wrapping, Operator::Divide) => {
                        b"omega.psi-rule.wrapping-integer-divide-constants.v1"
                    }
                    (Policy::Wrapping, Operator::Remainder) => {
                        b"omega.psi-rule.wrapping-integer-remainder-constants.v1"
                    }
                    (Policy::Saturating, Operator::Add) => {
                        b"omega.psi-rule.saturating-integer-add-constants.v1"
                    }
                    (Policy::Saturating, Operator::Subtract) => {
                        b"omega.psi-rule.saturating-integer-subtract-constants.v1"
                    }
                    (Policy::Saturating, Operator::Multiply) => {
                        b"omega.psi-rule.saturating-integer-multiply-constants.v1"
                    }
                    (Policy::Saturating, Operator::Divide) => {
                        b"omega.psi-rule.saturating-integer-divide-constants.v1"
                    }
                    (Policy::Saturating, Operator::Remainder) => {
                        b"omega.psi-rule.saturating-integer-remainder-constants.v1"
                    }
                }
            }

            fn shift_rule_name(
                policy: IntegerShiftPolicy,
                direction: IntegerShiftDirection,
            ) -> &'static [u8] {
                match (policy, direction) {
                    (IntegerShiftPolicy::Exact { .. }, IntegerShiftDirection::Left) => {
                        b"omega.psi-rule.exact-integer-shift-left-constants.v1"
                    }
                    (IntegerShiftPolicy::Exact { .. }, IntegerShiftDirection::Right) => {
                        b"omega.psi-rule.exact-integer-shift-right-constants.v1"
                    }
                    (IntegerShiftPolicy::Wrapping, IntegerShiftDirection::Left) => {
                        b"omega.psi-rule.wrapping-integer-shift-left-constants.v1"
                    }
                    (IntegerShiftPolicy::Wrapping, IntegerShiftDirection::Right) => {
                        b"omega.psi-rule.wrapping-integer-shift-right-constants.v1"
                    }
                }
            }
        }
        mod unary_operations {
            use abstract_operations::AbstractOperation as O;
            use optimization_core::OptimizationSafetyClass;
            use optimization_unit::{
                OptimizationNode, PsiOptimizationFunction, PsiRewriteCandidate,
            };

            use crate::OptimizationUnitValidationError;

            use super::{IntegerEvaluation, unary_integer_operand};

            pub(super) fn evaluate(
                function: &PsiOptimizationFunction,
                node: &OptimizationNode,
                candidate: &PsiRewriteCandidate,
            ) -> Option<Result<IntegerEvaluation, OptimizationUnitValidationError>> {
                match node.operation {
                    O::IntegerExactCast {
                        psi_operation,
                        result,
                        source_type,
                        target_type,
                        operand,
                        ..
                    } => Some((|| {
                        let operand = unary_integer_operand(function, candidate, operand)?;
                        let evaluated = source_type
                            .exact_cast_value_to(target_type, operand)
                            .ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;
                        Ok((
                            psi_operation,
                            result,
                            target_type,
                            evaluated,
                            OptimizationSafetyClass::ProofCertified,
                        ))
                    })()),
                    O::IntegerWiden {
                        psi_operation,
                        result,
                        source_type,
                        target_type,
                        operand,
                    } => Some((|| {
                        let operand = unary_integer_operand(function, candidate, operand)?;
                        let evaluated = source_type
                            .widen_value_to(target_type, operand)
                            .ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;
                        Ok((
                            psi_operation,
                            result,
                            target_type,
                            evaluated,
                            OptimizationSafetyClass::ExactOperationSemantics,
                        ))
                    })()),
                    O::IntegerBitwiseNot {
                        psi_operation,
                        result,
                        scalar_type,
                        operand,
                    } => Some((|| {
                        let operand = unary_integer_operand(function, candidate, operand)?;
                        let evaluated = scalar_type
                            .bitwise_not(operand)
                            .ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;
                        Ok((
                            psi_operation,
                            result,
                            scalar_type,
                            evaluated,
                            OptimizationSafetyClass::ExactOperationSemantics,
                        ))
                    })()),
                    _ => None,
                }
            }
        }
        use optimization_unit::{OptimizationNode, PsiOptimizationFunction, PsiRewriteCandidate};

        use crate::OptimizationUnitValidationError;

        use abstract_operations::{
            IntegerArithmeticOperator, IntegerOverflowPolicy, IntegerShiftDirection,
            IntegerShiftPolicy,
        };
        pub(crate) use literal_facts::{
            direct_literal_integer_fact, literal_boolean_fact, literal_integer_fact,
            unary_integer_operand,
        };
        use optimization_core::OptimizationSafetyClass;
        use semantic_vocabulary::{IntegerType, IntegerValue, OperationId, ValueId};

        pub(crate) fn evaluate_integer_operation(
            function: &PsiOptimizationFunction,
            node: &OptimizationNode,
            candidate: &PsiRewriteCandidate,
        ) -> Result<IntegerEvaluation, OptimizationUnitValidationError> {
            rule_identity::validate(&node.operation, candidate.rule())?;
            if let Some(evaluation) = unary_operations::evaluate(function, node, candidate) {
                return evaluation;
            }

            let operation = binary_operation_shape::recognize(&node.operation)?;
            let (left, right) = literal_facts::binary_integer_operands(
                function,
                candidate,
                operation.left,
                operation.right,
            )?;
            let (evaluated, safety_class) =
                binary_semantics::evaluate(operation.kind, operation.scalar_type, left, right);
            let evaluated =
                evaluated.ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;

            Ok((
                operation.source,
                operation.result,
                operation.scalar_type,
                evaluated,
                safety_class,
            ))
        }

        type IntegerEvaluation = (
            OperationId,
            ValueId,
            IntegerType,
            IntegerValue,
            OptimizationSafetyClass,
        );

        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        enum BinaryIntegerOperation {
            Arithmetic(IntegerOverflowPolicy, IntegerArithmeticOperator),
            Shift(IntegerShiftPolicy, IntegerShiftDirection, IntegerType),
            BitwiseAnd,
            BitwiseOr,
            BitwiseXor,
        }

        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        struct BinaryOperationShape {
            kind: BinaryIntegerOperation,
            source: OperationId,
            result: ValueId,
            scalar_type: IntegerType,
            left: ValueId,
            right: ValueId,
        }
    }
    mod observation {
        use optimization_unit::{
            PsiNodeObservation, PsiOptimizationUnit, reconstruct_psi_observation_model,
        };

        pub(crate) fn observation_at(
            unit: &PsiOptimizationUnit,
            location: optimization_unit::NodeLocation,
        ) -> Option<PsiNodeObservation> {
            reconstruct_psi_observation_model(unit)
                .nodes
                .into_iter()
                .find(|row| {
                    row.machine == location.machine
                        && row.block == location.block
                        && row.node == location.node
                })
        }

        pub(crate) fn same_closed_scalar_observation(
            input: &PsiNodeObservation,
            output: &PsiNodeObservation,
        ) -> bool {
            input.machine == output.machine
                && input.block == output.block
                && input.node == output.node
                && input.definitions == output.definitions
                && input.effect == output.effect
                && input.ownership == output.ownership
                && input.provenance == output.provenance
                && input.work == output.work
                && input.crash == output.crash
                && input.suspension == output.suspension
                && input.events == output.events
        }
    }
    mod snapshot_reconstruction {
        use crate::unit_validation::derived_metadata::expected_edges;
        use optimization_unit::{
            OptimizationFact, PsiOptimizationFunction, PsiProvenance, ScalarConstantValue,
            SccpBlockRow, SccpEdgeRow, SccpEdgeState, SccpMachineSnapshot, SccpValueRow,
            SccpValueState, derived_sccp_scalar_constant_fact_identity,
            literal_scalar_constant_fact_identity, scalar_value_definition,
        };
        use semantic_vocabulary::{BlockId, EdgeId, ScalarType, ValueId};
        use std::collections::{BTreeMap, BTreeSet};

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum ValidatorSccpValue {
            Unknown,
            Constant(ScalarConstantValue),
            Overdefined,
        }

        pub(crate) fn validator_scalar_constant_facts(
            input: optimization_core::OptimizationUnitIdentity,
            function: &PsiOptimizationFunction,
        ) -> Vec<(
            ValueId,
            ScalarConstantValue,
            optimization_core::ScalarConstantFactIdentity,
        )> {
            fn merge(target: &mut ValidatorSccpValue, incoming: ValidatorSccpValue) -> bool {
                let next = match (*target, incoming) {
                    (ValidatorSccpValue::Unknown, incoming) => incoming,
                    (_, ValidatorSccpValue::Unknown) | (ValidatorSccpValue::Overdefined, _) => {
                        return false;
                    }
                    (_, ValidatorSccpValue::Overdefined) => ValidatorSccpValue::Overdefined,
                    (
                        ValidatorSccpValue::Constant(current),
                        ValidatorSccpValue::Constant(incoming),
                    ) if current == incoming => {
                        return false;
                    }
                    (ValidatorSccpValue::Constant(_), ValidatorSccpValue::Constant(_)) => {
                        ValidatorSccpValue::Overdefined
                    }
                };
                if *target == next {
                    false
                } else {
                    *target = next;
                    true
                }
            }

            let mut values = BTreeMap::<ValueId, ValidatorSccpValue>::new();
            for parameter in &function.parameters {
                values.insert(parameter.value, ValidatorSccpValue::Overdefined);
            }
            for block in &function.blocks {
                for parameter in &block.parameters {
                    values.insert(parameter.value, ValidatorSccpValue::Unknown);
                }
                for definition in block.definitions() {
                    values.insert(definition.value, ValidatorSccpValue::Overdefined);
                }
            }
            let support_blocks = function
                .blocks
                .iter()
                .flat_map(|block| {
                    block.nodes.iter().flat_map(move |node| {
                        node.provenance
                            .iter()
                            .filter_map(move |source| match source {
                                PsiProvenance::Operation(operation) => Some((*operation, block.id)),
                                PsiProvenance::Edge(_) => None,
                            })
                    })
                })
                .collect::<BTreeMap<_, _>>();
            let mut literal_rows = Vec::new();
            let mut literal_support = BTreeMap::new();
            for fact in &function.facts {
                let (value, constant, support) = match fact {
                    OptimizationFact::BooleanConstant {
                        value,
                        constant,
                        support,
                    } => (*value, ScalarConstantValue::Boolean(*constant), *support),
                    OptimizationFact::IntegerConstant {
                        value,
                        constant,
                        support,
                    } => (*value, ScalarConstantValue::Integer(*constant), *support),
                    OptimizationFact::OperationObligationReference { .. } => continue,
                };
                let block = support_blocks.get(&support).copied();
                literal_rows.push((value, constant, block));
                literal_support.insert(value, support);
                values.insert(
                    value,
                    if block.is_some() {
                        ValidatorSccpValue::Unknown
                    } else {
                        ValidatorSccpValue::Constant(constant)
                    },
                );
            }

            let mut reachable = BTreeSet::from([function.entry]);
            let mut feasible_edges = BTreeSet::<EdgeId>::new();
            loop {
                let mut changed = false;
                for block in &function.blocks {
                    if !reachable.contains(&block.id) {
                        continue;
                    }
                    for (value, constant, site) in &literal_rows {
                        if *site == Some(block.id)
                            && matches!(values.get(value), Some(ValidatorSccpValue::Unknown))
                        {
                            values.insert(*value, ValidatorSccpValue::Constant(*constant));
                            changed = true;
                        }
                    }
                    let Some(node) = block.nodes.last() else {
                        continue;
                    };
                    let operation_successors = expected_edges(&node.operation);
                    let successors = match &node.operation {
                        abstract_operations::AbstractOperation::Jump { .. }
                        | abstract_operations::AbstractOperation::StructuralCase { .. } => {
                            operation_successors.iter().collect::<Vec<_>>()
                        }
                        abstract_operations::AbstractOperation::Conditional {
                            condition,
                            when_true,
                            when_false,
                        } => match values.get(condition) {
                            Some(ValidatorSccpValue::Constant(ScalarConstantValue::Boolean(
                                value,
                            ))) => {
                                let selected = if *value {
                                    when_true.psi_edge
                                } else {
                                    when_false.psi_edge
                                };
                                operation_successors
                                    .iter()
                                    .filter(|successor| successor.psi_edge == selected)
                                    .collect()
                            }
                            Some(ValidatorSccpValue::Overdefined) => {
                                operation_successors.iter().collect::<Vec<_>>()
                            }
                            _ => Vec::new(),
                        },
                        _ => Vec::new(),
                    };
                    for successor in successors {
                        changed |= feasible_edges.insert(successor.psi_edge);
                        changed |= reachable.insert(successor.target);
                        if let abstract_operations::AbstractOperation::StructuralCase {
                            cases,
                            ..
                        } = &node.operation
                        {
                            for payload in cases
                                .iter()
                                .filter(|case| case.psi_edge == successor.psi_edge)
                                .flat_map(|case| &case.payloads)
                            {
                                let target = values
                                    .entry(payload.parameter)
                                    .or_insert(ValidatorSccpValue::Unknown);
                                changed |= merge(target, ValidatorSccpValue::Overdefined);
                            }
                        }
                        for binding in &successor.bindings {
                            let incoming = values
                                .get(&binding.argument)
                                .copied()
                                .unwrap_or(ValidatorSccpValue::Overdefined);
                            let target = values
                                .entry(binding.parameter)
                                .or_insert(ValidatorSccpValue::Unknown);
                            changed |= merge(target, incoming);
                        }
                    }
                }
                if !changed {
                    break;
                }
            }

            let snapshot = validator_sccp_snapshot(function, &values, &reachable, &feasible_edges);
            values
                .into_iter()
                .filter_map(|(value, state)| {
                    let ValidatorSccpValue::Constant(constant) = state else {
                        return None;
                    };
                    let definition = scalar_value_definition(function, value)?;
                    let identity = literal_support
                        .get(&value)
                        .and_then(|support| {
                            literal_scalar_constant_fact_identity(
                                input,
                                function.machine,
                                definition,
                                constant,
                                *support,
                            )
                        })
                        .or_else(|| {
                            derived_sccp_scalar_constant_fact_identity(
                                input,
                                function.machine,
                                definition,
                                constant,
                                &snapshot,
                            )
                        })?;
                    Some((value, constant, identity))
                })
                .collect()
        }

        pub(crate) fn validator_sccp_snapshot(
            function: &PsiOptimizationFunction,
            values: &BTreeMap<ValueId, ValidatorSccpValue>,
            reachable: &BTreeSet<BlockId>,
            feasible_edges: &BTreeSet<EdgeId>,
        ) -> SccpMachineSnapshot {
            use abstract_operations::AbstractOperation as O;
            let mut blocks = function
                .blocks
                .iter()
                .map(|block| SccpBlockRow {
                    block: block.id,
                    executable: reachable.contains(&block.id),
                })
                .collect::<Vec<_>>();
            blocks.sort_by_key(|row| row.block);
            let mut edges = function
                .blocks
                .iter()
                .flat_map(|block| {
                    let reachable_source = reachable.contains(&block.id);
                    block.nodes.last().into_iter().flat_map(move |node| {
                        expected_edges(&node.operation)
                            .into_iter()
                            .map(move |successor| {
                                let state = if feasible_edges.contains(&successor.psi_edge) {
                                    SccpEdgeState::Executable
                                } else if !reachable_source {
                                    SccpEdgeState::Inexecutable
                                } else if let O::Conditional { condition, .. } = &node.operation {
                                    match values.get(condition) {
                                        Some(ValidatorSccpValue::Constant(
                                            ScalarConstantValue::Boolean(_),
                                        )) => SccpEdgeState::Inexecutable,
                                        _ => SccpEdgeState::Unknown,
                                    }
                                } else {
                                    SccpEdgeState::Inexecutable
                                };
                                SccpEdgeRow {
                                    source: block.id,
                                    edge: successor.psi_edge,
                                    target: successor.target,
                                    state,
                                }
                            })
                    })
                })
                .collect::<Vec<_>>();
            edges.sort_by_key(|row| (row.source, row.edge));
            let mut snapshot_values = values
                .iter()
                .filter_map(|(value, state)| {
                    let definition = scalar_value_definition(function, *value)?;
                    Some(SccpValueRow {
                        definition,
                        state: match state {
                            ValidatorSccpValue::Unknown => SccpValueState::Unknown,
                            ValidatorSccpValue::Constant(ScalarConstantValue::Boolean(value)) => {
                                SccpValueState::Boolean(*value)
                            }
                            ValidatorSccpValue::Constant(ScalarConstantValue::Integer(value)) => {
                                SccpValueState::Integer(*value)
                            }
                            ValidatorSccpValue::Overdefined => SccpValueState::Overdefined,
                        },
                    })
                })
                .collect::<Vec<_>>();
            snapshot_values.sort_by_key(|row| row.definition.value);
            SccpMachineSnapshot {
                blocks,
                edges,
                values: snapshot_values,
            }
        }

        pub(crate) fn validator_integer_value_type(
            function: &PsiOptimizationFunction,
            value: ValueId,
        ) -> Option<semantic_vocabulary::IntegerType> {
            scalar_value_definition(function, value).and_then(|definition| {
                match definition.scalar_type {
                    ScalarType::Integer(integer) => Some(integer),
                    ScalarType::Boolean | ScalarType::IeeeFloat(_) | ScalarType::Callable(_) => {
                        None
                    }
                }
            })
        }
    }
    pub(crate) use boolean_candidate::validate_boolean_evaluation_candidate;
    pub(crate) use integer_candidate::validate_integer_evaluation_candidate;

    pub(crate) use integer_evaluation::literal_boolean_fact;
    pub(crate) use observation::{observation_at, same_closed_scalar_observation};
    pub(crate) use optimization_unit::scalar_value_definition;
    pub(crate) use snapshot_reconstruction::validator_scalar_constant_facts;

    pub(super) fn validate_scalar_evaluation_candidate(
        input: &PsiOptimizationUnit,
        candidate: &PsiRewriteCandidate,
    ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
        match candidate.patch() {
            PsiRewritePatch::ReplaceIntegerOperationWithConstant(_) => {
                validate_integer_evaluation_candidate(input, candidate)
            }
            PsiRewritePatch::ReplaceBooleanOperationWithConstant(_) => {
                validate_boolean_evaluation_candidate(input, candidate)
            }
            PsiRewritePatch::RemoveRedundantBlockParameter(_)
            | PsiRewritePatch::FoldConstantConditional(_)
            | PsiRewritePatch::ThreadLinearEmptyBlock(_)
            | PsiRewritePatch::ThreadPathQualifiedEmptyBlock(_)
            | PsiRewritePatch::MergeAdjacentBlock(_)
            | PsiRewritePatch::MergeNonAdjacentBlock(_)
            | PsiRewritePatch::FuseSharedTerminalJump(_)
            | PsiRewritePatch::RemoveDeadScalarNode(_)
            | PsiRewritePatch::EliminateLocalScalarCommonSubexpression(_)
            | PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(_)
            | PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(_)
            | PsiRewritePatch::EliminateProofCertifiedScalarIdentity(_)
            | PsiRewritePatch::EliminateTotalScalarIdentity(_)
            | PsiRewritePatch::PruneUnreachablePrivateMachines(_)
            | PsiRewritePatch::SpecializeStateArgument(_)
            | PsiRewritePatch::SpecializeCaseMembership(_)
            | PsiRewritePatch::SpecializeFieldValue(_) => {
                Err(OptimizationUnitValidationError::CandidatePatchMismatch)
            }
        }
    }
}
pub(crate) mod state_specialization {
    use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
    use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

    mod replay {
        use crate::candidates::provenance_realized_at;
        use crate::candidates::realized_at_own_site;
        use crate::candidates::sparse_conditional_constant_propagation::validator_scalar_constant_facts;
        use crate::candidates::{
            accept_region_rewrite, candidate_block, candidate_function, canonicalize_provenance,
            land_folded_nodes,
        };
        use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
        use abstract_operations::AbstractOperation as O;
        use abstract_operations::binding_arguments;
        use optimization_unit::{
            DispatchCondition, NodeLocation, OptimizationEdge, OptimizationNode, ProvenanceRewrite,
            PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance, PsiRealizationSite,
            PsiRewriteCandidate, PsiRewritePatch, ScalarConstantValue, SpecializedStateEdgeRow,
            ValueUse, integer_comparison_condition,
        };
        use semantic_vocabulary::{BlockId, EdgeId, MachineId, ValueId};
        use std::collections::{BTreeMap, BTreeSet};

        pub(crate) fn cyclic_machines(unit: &PsiOptimizationUnit) -> BTreeSet<MachineId> {
            unit.functions
                .iter()
                .filter_map(|function| {
                    let graph = function
                        .blocks
                        .iter()
                        .map(|block| {
                            (
                                block.id,
                                block
                                    .successors()
                                    .map(|edge| edge.target)
                                    .collect::<Vec<_>>(),
                            )
                        })
                        .collect::<BTreeMap<_, _>>();
                    has_cycle(&graph).then_some(function.machine)
                })
                .collect()
        }

        fn has_cycle(graph: &BTreeMap<BlockId, Vec<BlockId>>) -> bool {
            #[derive(Clone, Copy, PartialEq, Eq)]
            enum Mark {
                Visiting,
                Done,
            }
            let mut marks = BTreeMap::<BlockId, Mark>::new();
            for &root in graph.keys() {
                if marks.contains_key(&root) {
                    continue;
                }
                let mut stack = vec![(root, 0usize)];
                marks.insert(root, Mark::Visiting);
                while let Some(&(block, index)) = stack.last() {
                    let successors = graph.get(&block).cloned().unwrap_or_default();
                    if index >= successors.len() {
                        marks.insert(block, Mark::Done);
                        stack.pop();
                        continue;
                    }
                    let successor = successors[index];
                    if let Some(top) = stack.last_mut() {
                        top.1 = index + 1;
                    }
                    match marks.get(&successor) {
                        Some(Mark::Visiting) => return true,
                        Some(Mark::Done) => {}
                        None => {
                            marks.insert(successor, Mark::Visiting);
                            stack.push((successor, 0));
                        }
                    }
                }
            }
            false
        }

        fn plan_dispatch(
            input: &PsiOptimizationUnit,
            function: &PsiOptimizationFunction,
            dispatch: BlockId,
            constants: &BTreeMap<ValueId, ScalarConstantValue>,
        ) -> Option<Vec<SpecializedStateEdgeRow>> {
            let machine = function.machine;
            let block = function.block(dispatch)?;
            if block.id == function.entry || !block.structural_parameters.is_empty() {
                return None;
            }
            let (node, prefix) = block.nodes.split_last()?;
            let O::Conditional {
                condition,
                when_true,
                when_false,
            } = &node.operation
            else {
                return None;
            };
            let (parameter, condition_kind) = if prefix.is_empty() {
                let parameter = block
                    .parameters
                    .iter()
                    .find(|parameter| parameter.value == *condition)?;
                (parameter.value, DispatchCondition::DirectParameter)
            } else {
                integer_comparison_condition(block, prefix, *condition)?
            };
            if constants.contains_key(&parameter) {
                return None;
            }
            let arm_edge = |edge: EdgeId| node.edge(edge);
            let when_true_edge = arm_edge(when_true.psi_edge)?;
            let when_false_edge = arm_edge(when_false.psi_edge)?;

            let mut incoming = Vec::new();
            for owner_block in &function.blocks {
                for (node_index, owner_node) in owner_block.nodes.iter().enumerate() {
                    for edge in &owner_node.successors {
                        if edge.target == dispatch {
                            incoming.push((owner_block.id, node_index, owner_node, edge));
                        }
                    }
                }
            }
            if incoming.is_empty() {
                return None;
            }

            let mut edges = Vec::new();
            for (owner_block, node_index, owner_node, edge) in &incoming {
                let predecessor = NodeLocation {
                    machine,
                    block: *owner_block,
                    node: u32::try_from(*node_index).ok()?,
                };
                match &owner_node.operation {
                    O::Jump {
                        psi_edge,
                        target,
                        structural_bindings,
                        cleanup_actions,
                        ..
                    } => {
                        if *psi_edge != edge.psi_edge
                            || *target != dispatch
                            || !structural_bindings.is_empty()
                            || !cleanup_actions.is_empty()
                        {
                            continue;
                        }
                    }
                    O::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => {
                        let Some(arm) = [when_true, when_false]
                            .into_iter()
                            .find(|arm| arm.psi_edge == edge.psi_edge)
                        else {
                            continue;
                        };
                        if arm.target != dispatch
                            || !arm.structural_bindings.is_empty()
                            || !arm.cleanup_actions.is_empty()
                        {
                            continue;
                        }
                    }
                    _ => continue,
                }
                if !edge.structural_bindings.is_empty() || !edge.cleanup_actions.is_empty() {
                    continue;
                }
                let Some(binding) = edge.binding(parameter) else {
                    continue;
                };
                let constant = constants.get(&binding.argument).copied().or_else(|| {
                    let delivered = delivered_argument(function, *owner_block, binding.argument);
                    call_result_constant(input, function, delivered)
                });
                let Some(constant) =
                    constant.and_then(|constant| condition_kind.resolve(&constant))
                else {
                    continue;
                };
                let (resolved, rejected) = if constant {
                    (when_true_edge, when_false_edge)
                } else {
                    (when_false_edge, when_true_edge)
                };
                if !resolved.structural_bindings.is_empty() || !resolved.cleanup_actions.is_empty()
                {
                    continue;
                }
                let Some(resolved_block) = function.block(resolved.target) else {
                    continue;
                };
                if !resolved_block.structural_parameters.is_empty() {
                    continue;
                }
                edges.push(SpecializedStateEdgeRow {
                    incoming_edge: edge.psi_edge,
                    predecessor,
                    parameter,
                    argument: binding.argument,
                    constant,
                    taken_edge: resolved.psi_edge,
                    rejected_edge: rejected.psi_edge,
                    resolved_target: resolved.target,
                });
            }
            edges.sort_by_key(|row| row.incoming_edge);
            if edges.len() == incoming.len() {
                edges.clear();
            }
            Some(edges)
        }

        fn delivered_argument(
            function: &PsiOptimizationFunction,
            owner_block: BlockId,
            argument: ValueId,
        ) -> ValueId {
            let mut owner_block = owner_block;
            let mut argument = argument;
            for _ in 0..function.blocks.len() {
                let Some(block) = function.block(owner_block) else {
                    break;
                };
                if !block
                    .parameters
                    .iter()
                    .any(|parameter| parameter.value == argument)
                {
                    break;
                }
                let mut incoming = function.blocks.iter().flat_map(|candidate| {
                    candidate.nodes.iter().flat_map(move |node| {
                        node.successors
                            .iter()
                            .filter(move |successor| successor.target == owner_block)
                            .map(move |successor| (candidate.id, successor))
                    })
                });
                let (Some((predecessor_block, predecessor_edge)), None) =
                    (incoming.next(), incoming.next())
                else {
                    break;
                };
                let Some(binding) = predecessor_edge.binding(argument) else {
                    break;
                };
                argument = binding.argument;
                owner_block = predecessor_block;
            }
            argument
        }

        fn call_result_constant(
            input: &PsiOptimizationUnit,
            function: &PsiOptimizationFunction,
            argument: ValueId,
        ) -> Option<ScalarConstantValue> {
            let callee = function
                .block_nodes()
                .find_map(|node| match &node.operation {
                    O::Call { result, callee, .. } if *result == argument => Some(*callee),
                    _ => None,
                })?;
            let callee_function = input.function(callee)?;
            let mut returned =
                callee_function
                    .block_nodes()
                    .filter_map(|node| match &node.operation {
                        O::Return { value, .. } => Some(*value),
                        _ => None,
                    });
            let value = returned.next()?;
            if returned.next().is_some() {
                return None;
            }
            validator_scalar_constant_facts(input.identity, callee_function)
                .into_iter()
                .find(|(fact_value, _, _)| *fact_value == value)
                .map(|(_, constant, _)| constant)
        }

        fn fused_node(
            site_rows: &[&SpecializedStateEdgeRow],
            dispatch: BlockId,
            function: &PsiOptimizationFunction,
        ) -> Result<OptimizationNode, OptimizationUnitValidationError> {
            let Some(first) = site_rows.first() else {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            };
            if site_rows
                .iter()
                .any(|row| row.predecessor != first.predecessor)
            {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            let block = candidate_block(function, first.predecessor.block)?;
            let predecessor = block
                .nodes
                .get(
                    usize::try_from(first.predecessor.node)
                        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?,
                )
                .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
            let dispatch_block = function
                .block(dispatch)
                .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
            let dispatch_node = dispatch_block
                .nodes
                .last()
                .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
            match &predecessor.operation {
                O::Jump {
                    psi_edge,
                    target,
                    structural_bindings,
                    cleanup_actions,
                    ..
                } => {
                    if site_rows.len() != 1 {
                        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                    }
                    if first.predecessor.machine != function.machine
                        || *psi_edge != first.incoming_edge
                        || *target != dispatch
                        || !structural_bindings.is_empty()
                        || !cleanup_actions.is_empty()
                    {
                        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                    }
                    let incoming = predecessor
                        .edge(first.incoming_edge)
                        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
                    if incoming.target != dispatch
                        || !incoming.structural_bindings.is_empty()
                        || !incoming.cleanup_actions.is_empty()
                    {
                        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                    }
                    let fused_edge = fused_edge(first, incoming, dispatch_node)?;
                    let operation = O::Jump {
                        psi_edge: incoming.psi_edge,
                        target: fused_edge.target,
                        bindings: fused_edge.bindings.clone(),
                        structural_bindings: structural_bindings.clone(),
                        cleanup_actions: cleanup_actions.clone(),
                    };
                    Ok(OptimizationNode {
                        uses: node_uses(&operation, first.predecessor),
                        operation,
                        provenance: predecessor.provenance.clone(),
                        work: predecessor.work.clone(),
                        effect: predecessor.effect,
                        definitions: predecessor.definitions.clone(),
                        successors: vec![fused_edge],
                        ownership: predecessor.ownership.clone(),
                    })
                }
                O::Conditional {
                    condition,
                    when_true,
                    when_false,
                } => {
                    if first.predecessor.machine != function.machine {
                        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                    }
                    let mut fused_when_true = when_true.clone();
                    let mut fused_when_false = when_false.clone();
                    let mut successors = predecessor.successors.clone();
                    for row in site_rows {
                        let arm = if fused_when_true.psi_edge == row.incoming_edge {
                            &mut fused_when_true
                        } else if fused_when_false.psi_edge == row.incoming_edge {
                            &mut fused_when_false
                        } else {
                            return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                        };
                        if arm.target != dispatch
                            || !arm.structural_bindings.is_empty()
                            || !arm.cleanup_actions.is_empty()
                        {
                            return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                        }
                        let Some(index) = successors
                            .iter()
                            .position(|edge| edge.psi_edge == row.incoming_edge)
                        else {
                            return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                        };
                        let incoming = &successors[index];
                        if incoming.target != dispatch
                            || !incoming.structural_bindings.is_empty()
                            || !incoming.cleanup_actions.is_empty()
                        {
                            return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
                        }
                        let fused = fused_edge(row, incoming, dispatch_node)?;
                        *arm = abstract_operations::AbstractSuccessor {
                            psi_edge: fused.psi_edge,
                            target: fused.target,
                            bindings: fused.bindings.clone(),
                            structural_bindings: fused.structural_bindings.clone(),
                            cleanup_actions: fused.cleanup_actions.clone(),
                        };
                        successors[index] = fused;
                    }
                    let operation = O::Conditional {
                        condition: *condition,
                        when_true: fused_when_true,
                        when_false: fused_when_false,
                    };
                    Ok(OptimizationNode {
                        uses: node_uses(&operation, first.predecessor),
                        operation,
                        provenance: predecessor.provenance.clone(),
                        work: predecessor.work.clone(),
                        effect: predecessor.effect,
                        definitions: predecessor.definitions.clone(),
                        successors,
                        ownership: predecessor.ownership.clone(),
                    })
                }
                _ => Err(OptimizationUnitValidationError::CandidatePatchMismatch),
            }
        }

        fn fused_edge(
            row: &SpecializedStateEdgeRow,
            incoming: &OptimizationEdge,
            dispatch_node: &OptimizationNode,
        ) -> Result<OptimizationEdge, OptimizationUnitValidationError> {
            let resolved = dispatch_node
                .edge(row.taken_edge)
                .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
            if resolved.target != row.resolved_target
                || row.rejected_edge == row.taken_edge
                || !dispatch_node
                    .successors
                    .iter()
                    .any(|edge| edge.psi_edge == row.rejected_edge)
            {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            let bindings = resolved
                .bindings
                .iter()
                .map(|binding| {
                    let argument = incoming
                        .binding(binding.argument)
                        .map(|incoming_binding| incoming_binding.argument)
                        .unwrap_or(binding.argument);
                    abstract_operations::ValueBinding {
                        parameter: binding.parameter,
                        argument,
                        scalar_type: binding.scalar_type,
                    }
                })
                .collect::<Vec<_>>();
            let mut provenance = incoming.provenance.clone();
            provenance.extend_from_slice(&resolved.provenance);
            if provenance.first() != Some(&PsiProvenance::Edge(incoming.psi_edge)) {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            let mut work = incoming.work.clone();
            work.extend_from_slice(&resolved.work);
            Ok(OptimizationEdge {
                psi_edge: incoming.psi_edge,
                target: resolved.target,
                bindings,
                structural_bindings: Vec::new(),
                cleanup_actions: Vec::new(),
                provenance,
                work,
            })
        }

        fn node_uses(
            operation: &abstract_operations::AbstractOperation,
            location: NodeLocation,
        ) -> Vec<ValueUse> {
            let arguments: Vec<ValueId> = match operation {
                O::Jump { bindings, .. } => binding_arguments(bindings).collect(),
                O::Conditional {
                    condition,
                    when_true,
                    when_false,
                } => std::iter::once(*condition)
                    .chain(binding_arguments(&when_true.bindings))
                    .chain(binding_arguments(&when_false.bindings))
                    .collect(),
                _ => Vec::new(),
            };
            arguments
                .into_iter()
                .map(|value| ValueUse {
                    value,
                    block: location.block,
                    node: location.node,
                })
                .collect()
        }

        fn accepted_provenance(
            function: &PsiOptimizationFunction,
            machine: MachineId,
            rows: &[SpecializedStateEdgeRow],
        ) -> Result<Vec<ProvenanceRewrite>, OptimizationUnitValidationError> {
            let find_edge = |edge: EdgeId| {
                function
                    .block_edges()
                    .find(|candidate| candidate.psi_edge == edge)
                    .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)
            };
            let mut provenance = Vec::new();
            let mut resolved_edges = BTreeSet::new();
            for row in rows {
                let incoming = find_edge(row.incoming_edge)?;
                let resolved = find_edge(row.taken_edge)?;
                let incoming_site = PsiRealizationSite::Edge {
                    machine,
                    edge: row.incoming_edge,
                };
                let resolved_site = PsiRealizationSite::Edge {
                    machine,
                    edge: row.taken_edge,
                };
                provenance.push(realized_at_own_site(
                    incoming_site,
                    &incoming.provenance,
                    &incoming.work,
                ));
                provenance.push(provenance_realized_at(
                    resolved_site,
                    incoming_site,
                    &resolved.provenance,
                    &resolved.work,
                ));
                resolved_edges.insert(row.taken_edge);
            }
            for taken_edge in resolved_edges {
                let resolved = find_edge(taken_edge)?;
                let resolved_site = PsiRealizationSite::Edge {
                    machine,
                    edge: taken_edge,
                };
                provenance.push(realized_at_own_site(
                    resolved_site,
                    &resolved.provenance,
                    &resolved.work,
                ));
            }
            canonicalize_provenance(&mut provenance);
            Ok(provenance)
        }

        pub(super) fn validate(
            input: &PsiOptimizationUnit,
            candidate: &PsiRewriteCandidate,
        ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
            candidate_patch!(patch, SpecializeStateArgument, candidate);
            if candidate.node_decision_point()
                != Some(NodeLocation {
                    machine: patch.machine,
                    block: patch.dispatch,
                    node: 0,
                })
            {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            if patch.edges.is_empty()
                || candidate.predicted_cost_delta()
                    != -i64::try_from(patch.edges.len())
                        .map_err(|_| OptimizationUnitValidationError::CandidatePatchMismatch)?
            {
                return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
            }
            if cyclic_machines(input).contains(&patch.machine) {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            let function = candidate_function(input, patch.machine)?;
            let constants = validator_scalar_constant_facts(input.identity, function)
                .into_iter()
                .map(|(value, constant, _)| (value, constant))
                .collect::<BTreeMap<ValueId, ScalarConstantValue>>();
            let replayed = plan_dispatch(input, function, patch.dispatch, &constants)
                .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
            if replayed.is_empty() {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            if patch.edges != replayed {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
            let mut expected_blocks = vec![patch.dispatch];
            expected_blocks.extend(patch.edges.iter().map(|row| row.predecessor.block));
            expected_blocks.sort_unstable();
            expected_blocks.dedup();
            let expected_provenance = accepted_provenance(function, patch.machine, &patch.edges)?;

            let mut sites = BTreeMap::<NodeLocation, Vec<&SpecializedStateEdgeRow>>::new();
            for row in &patch.edges {
                sites.entry(row.predecessor).or_default().push(row);
            }
            let fused = sites
                .into_iter()
                .map(|(location, rows)| {
                    fused_node(&rows, patch.dispatch, function).map(|node| (location, node))
                })
                .collect::<Result<Vec<_>, _>>()?;
            accept_region_rewrite(
                input,
                candidate,
                patch.machine,
                &expected_blocks,
                b"omega.validator.state-argument-specialization.v1",
                expected_provenance,
                |output| land_folded_nodes(output, patch.machine, fused),
            )
        }
    }
    pub(crate) use replay::cyclic_machines;

    pub fn validate_state_argument_specialization_candidate(
        input: &PsiOptimizationUnit,
        candidate: &PsiRewriteCandidate,
    ) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
        super::contract::admit(
            input,
            candidate,
            &super::contract::STATE_ARGUMENT_SPECIALIZATION,
        )?;
        if !candidate.substitutions().is_empty() {
            return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
        }
        replay::validate(input, candidate)
    }
}
mod value_operand_rewrites {
    use abstract_operations::AbstractOperation as O;
    use semantic_vocabulary::ValueId;

    pub(crate) fn rewrite_value_operands(operation: &mut O, mut replace: impl FnMut(&mut ValueId)) {
        match operation {
            O::EstablishActivationClaim { committed, .. } => replace(committed),
            O::SplitActivationClaim {
                committed_at: committed,
                ..
            } => replace(committed),
            O::ReleaseActivationClaim { .. }
            | O::MergeActivationClaim { .. }
            | O::EstablishNativeCodeLayout { .. } => {}
            O::RegisterAccess { event, .. } => {
                event.map_operands(|mut value| {
                    replace(&mut value);
                    value
                });
            }
            O::EstablishStructuralArray { .. } | O::EstablishAccessReference { .. } => {}
            O::EstablishReference { storage, .. } => {
                if let Some(view) = storage {
                    replace(&mut view.byte_offset);
                }
            }
            O::EstablishScalarArray { elements, .. } => {
                for element in elements {
                    replace(element);
                }
            }
            abstract_operations::establish_case_or_record_fields_pattern!(fields) => {
                for initializer in fields {
                    if let terminal_psi::RecordFieldValue::Scalar { value, .. } =
                        &mut initializer.value
                    {
                        replace(value);
                    }
                }
            }
            O::EstablishZeroedStructural { .. } | O::EstablishStructuralLocal { .. } => {}
            abstract_operations::primitive_local_triple_pattern!(value, ..)
            | O::StructuralScalarFieldStore { value, .. } => replace(&mut value.value),
            O::CallStructural {
                callee, arguments, ..
            } => {
                if let abstract_operations::AbstractCallTarget::Dynamic { selector, .. } = callee {
                    replace(selector);
                }
                for argument in arguments {
                    replace(argument);
                }
            }
            O::Call { arguments, .. }
            | O::InvokeNoNormal { arguments, .. }
            | O::ExternalCompletion { arguments, .. }
            | O::BoundaryCall { arguments, .. } => {
                for argument in arguments {
                    replace(argument);
                }
            }
            O::BooleanNot { operand, .. }
            | O::IntegerBitwiseNot { operand, .. }
            | abstract_operations::integer_widen_or_exact_cast_pattern!(operand, ..)
            | O::ScalarRepresentationView { operand, .. } => replace(operand),
            O::BooleanEqual { left, right, .. }
            | abstract_operations::integer_equal_less_pair_pattern!(left, right, ..)
            | O::IntegerLessOrEqual { left, right, .. }
            | O::IntegerBitwiseAnd { left, right, .. }
            | abstract_operations::bitwise_or_xor_pair_pattern!(left, right, ..)
            | O::IntegerArithmetic { left, right, .. } => {
                replace(left);
                replace(right);
            }
            O::TrappingInteger { operation, .. } => operation.map_operands(&mut |mut value| {
                replace(&mut value);
                value
            }),
            O::FloatResultPolicy { policy, .. } => policy.map_operands(&mut |mut value| {
                replace(&mut value);
                value
            }),
            O::IeeeFloatCompare { left, right, .. } => {
                replace(left);
                replace(right);
            }
            O::NearestIeeeFloatFusedMultiplyAdd {
                left,
                right,
                addend,
                ..
            } => {
                replace(left);
                replace(right);
                replace(addend);
            }
            O::IntegerShift { value, count, .. } => {
                replace(value);
                replace(count);
            }
            O::Jump { .. } | O::Conditional { .. } | O::StructuralCase { .. } => {}
            O::Return { value, .. } => replace(value),
            O::ActivateTaskRegister { selector, .. }
            | O::ReloadSegmentDescriptors { selector, .. } => replace(selector),
            O::ReadTaskRegister { .. } | O::ReadSegmentSelector { .. } => {}
            O::AtomicEvent { event, .. } => {
                event.for_each_input_mut(&mut replace);
            }
            abstract_operations::establish_byte_literal_triple_pattern!(..)
            | O::GlobalDescriptorTable { .. }
            | O::PortWrite { .. }
            | O::SelectImplementation { .. }
            | O::SelectMachine { .. }
            | abstract_operations::constant_value_triple_pattern!(..)
            | O::StorageAddress { .. }
            | abstract_operations::primitive_storage_observation_pattern!(..)
            | abstract_operations::byte_read_predicate_equal_pattern!(..)
            | O::ByteSequenceWrite { .. }
            | abstract_operations::byte_sequence_store_pair_pattern!(..)
            | abstract_operations::byte_view_triple_pattern!(..)
            | abstract_operations::byte_view_length_triple_pattern!(..)
            | abstract_operations::element_view_read_write_pair_pattern!(..)
            | O::ElementViewSubslice { .. }
            | abstract_operations::byte_len_or_field_read_pattern!(..)
            | O::StructuralScalarFieldRead { .. }
            | O::ReleaseReference { .. }
            | abstract_operations::move_or_store_structural_field_pattern!()
            | O::StructuralLeafCopy { .. }
            | O::MoveStructuralValue { .. }
            | O::TransformContent { .. }
            | O::ReturnUnit { .. }
            | abstract_operations::return_structural_or_crash_pattern!() => {}
        }
    }
}
use case_membership_specialization::validate_case_membership_specialization_candidate;
use control_flow_cleanup::{
    validate_adjacent_block_merge_candidate, validate_constant_conditional_candidate,
    validate_linear_empty_block_candidate, validate_non_adjacent_block_merge_candidate,
    validate_path_qualified_empty_block_candidate, validate_shared_jump_fusion_candidate,
    validate_unreachable_private_machines_candidate,
};
use copy_propagation::validate_redundant_block_parameter_candidate;
use dead_scalar_elimination::validate_dead_scalar_node_candidate;
use field_value_specialization::validate_field_value_specialization_candidate;
use global_value_numbering::{
    validate_dominating_scalar_common_subexpression_candidate,
    validate_local_scalar_common_subexpression_candidate,
    validate_phi_translated_scalar_common_subexpression_candidate,
    validate_total_scalar_identity_candidate,
};
use optimization_unit::OptimizationNode;
use proof_check_elision::{is_proof_check_elision_rule, validate_proof_check_elision_candidate};
use sparse_conditional_constant_propagation::validate_scalar_evaluation_candidate;
use state_specialization::validate_state_argument_specialization_candidate;

pub fn validate_psi_rewrite_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    if is_proof_check_elision_rule(candidate.rule()) {
        return validate_proof_check_elision_candidate(input, candidate);
    }
    match candidate.patch() {
        PsiRewritePatch::ReplaceIntegerOperationWithConstant(_)
        | PsiRewritePatch::ReplaceBooleanOperationWithConstant(_) => {
            validate_scalar_evaluation_candidate(input, candidate)
        }
        PsiRewritePatch::RemoveRedundantBlockParameter(_) => {
            validate_redundant_block_parameter_candidate(input, candidate)
        }
        PsiRewritePatch::FoldConstantConditional(_) => {
            validate_constant_conditional_candidate(input, candidate)
        }
        PsiRewritePatch::ThreadLinearEmptyBlock(_) => {
            validate_linear_empty_block_candidate(input, candidate)
        }
        PsiRewritePatch::ThreadPathQualifiedEmptyBlock(_) => {
            validate_path_qualified_empty_block_candidate(input, candidate)
        }
        PsiRewritePatch::MergeAdjacentBlock(_) => {
            validate_adjacent_block_merge_candidate(input, candidate)
        }
        PsiRewritePatch::MergeNonAdjacentBlock(_) => {
            validate_non_adjacent_block_merge_candidate(input, candidate)
        }
        PsiRewritePatch::FuseSharedTerminalJump(_) => {
            validate_shared_jump_fusion_candidate(input, candidate)
        }
        PsiRewritePatch::RemoveDeadScalarNode(_) => {
            validate_dead_scalar_node_candidate(input, candidate)
        }
        PsiRewritePatch::EliminateLocalScalarCommonSubexpression(_) => {
            validate_local_scalar_common_subexpression_candidate(input, candidate)
        }
        PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(_) => {
            validate_dominating_scalar_common_subexpression_candidate(input, candidate)
        }
        PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(_) => {
            validate_phi_translated_scalar_common_subexpression_candidate(input, candidate)
        }
        PsiRewritePatch::EliminateProofCertifiedScalarIdentity(_) => {
            validate_proof_check_elision_candidate(input, candidate)
        }
        PsiRewritePatch::EliminateTotalScalarIdentity(_) => {
            validate_total_scalar_identity_candidate(input, candidate)
        }
        PsiRewritePatch::PruneUnreachablePrivateMachines(_) => {
            validate_unreachable_private_machines_candidate(input, candidate)
        }
        PsiRewritePatch::SpecializeStateArgument(_) => {
            validate_state_argument_specialization_candidate(input, candidate)
        }
        PsiRewritePatch::SpecializeCaseMembership(_) => {
            validate_case_membership_specialization_candidate(input, candidate)
        }
        PsiRewritePatch::SpecializeFieldValue(_) => {
            validate_field_value_specialization_candidate(input, candidate)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPsiRewrite {
    pub(in crate::candidates) unit: PsiOptimizationUnit,
    pub(in crate::candidates) candidate: OptimizationCandidateIdentity,
    pub(in crate::candidates) validator: OptimizationValidatorIdentity,
    pub(in crate::candidates) provenance: Vec<ProvenanceRewrite>,
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

    pub fn provenance(&self) -> &[ProvenanceRewrite] {
        &self.provenance
    }

    pub fn into_unit(self) -> PsiOptimizationUnit {
        self.unit
    }
}

pub(crate) fn provenance_realized_at(
    input: optimization_unit::PsiRealizationSite,
    realized_at: optimization_unit::PsiRealizationSite,
    sources: &[optimization_unit::PsiProvenance],
    work: &[optimization_unit::WorkSettlement],
) -> ProvenanceRewrite {
    ProvenanceRewrite {
        input,
        disposition: optimization_unit::ProvenanceDisposition::RealizedAt(realized_at),
        sources: sources.to_vec(),
        work: work.to_vec(),
    }
}

pub(crate) fn realized_at_own_site(
    site: optimization_unit::PsiRealizationSite,
    sources: &[optimization_unit::PsiProvenance],
    work: &[optimization_unit::WorkSettlement],
) -> ProvenanceRewrite {
    provenance_realized_at(site, site, sources, work)
}

pub(crate) fn proven_unreachable_at_own_site(
    site: optimization_unit::PsiRealizationSite,
    sources: &[optimization_unit::PsiProvenance],
    work: &[optimization_unit::WorkSettlement],
) -> ProvenanceRewrite {
    provenance_proven_unreachable_at(site, site, sources, work)
}

pub(crate) fn provenance_proven_unreachable_at(
    input: optimization_unit::PsiRealizationSite,
    unreachable_at: optimization_unit::PsiRealizationSite,
    sources: &[optimization_unit::PsiProvenance],
    work: &[optimization_unit::WorkSettlement],
) -> ProvenanceRewrite {
    ProvenanceRewrite {
        input,
        disposition: optimization_unit::ProvenanceDisposition::ProvenUnreachableAt(unreachable_at),
        sources: sources.to_vec(),
        work: work.to_vec(),
    }
}

pub(crate) fn candidate_function(
    input: &PsiOptimizationUnit,
    machine: semantic_vocabulary::MachineId,
) -> Result<&PsiOptimizationFunction, OptimizationUnitValidationError> {
    input
        .function(machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)
}

pub(crate) fn rebuild_node_metadata(
    node: &mut OptimizationNode,
    block: semantic_vocabulary::BlockId,
    node_index: usize,
) -> Result<(), OptimizationUnitValidationError> {
    let node_index = u32::try_from(node_index)
        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
    node.definitions = crate::unit_validation::derived_metadata::expected_definitions(
        &node.operation,
        block,
        node_index,
    );
    node.uses =
        crate::unit_validation::derived_metadata::expected_uses(&node.operation, block, node_index);
    node.successors = rewrite_accounting::preserve_edge_custody(node);
    node.ownership = crate::unit_validation::derived_metadata::expected_ownership(&node.operation);
    Ok(())
}

pub(crate) fn set_node_effect_link(
    node: &mut OptimizationNode,
    effect: u64,
) -> Result<u64, OptimizationUnitValidationError> {
    let output = effect
        .checked_add(1)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    node.effect = optimization_unit::EffectLink {
        input: effect,
        output,
    };
    Ok(output)
}

pub(crate) fn land_folded_nodes(
    output: &mut PsiOptimizationUnit,
    machine: semantic_vocabulary::MachineId,
    folded: Vec<(optimization_unit::NodeLocation, OptimizationNode)>,
) -> Result<(), OptimizationUnitValidationError> {
    let function = output
        .function_mut(machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    for (location, node) in folded {
        let index = usize::try_from(location.node)
            .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
        let Some(slot) = function
            .block_mut(location.block)
            .and_then(|block| block.nodes.get_mut(index))
        else {
            return Err(OptimizationUnitValidationError::CandidateLocationMissing);
        };
        *slot = node;
    }
    Ok(())
}

pub(crate) fn sweep_output_nodes(
    function: &mut PsiOptimizationFunction,
    mut per_node: impl FnMut(&mut OptimizationNode) -> Result<(), OptimizationUnitValidationError>,
) -> Result<(), OptimizationUnitValidationError> {
    let mut effect = 0u64;
    for block in &mut function.blocks {
        let block_id = block.id;
        for (node_index, node) in block.nodes.iter_mut().enumerate() {
            per_node(node)?;
            rebuild_node_metadata(node, block_id, node_index)?;
            effect = set_node_effect_link(node, effect)?;
        }
    }
    Ok(())
}

pub(crate) fn require_realized_rows(
    candidate: &PsiRewriteCandidate,
    expected: &[ProvenanceRewrite],
) -> Result<(), OptimizationUnitValidationError> {
    let actual = candidate.provenance();
    let mut paired = actual.iter().zip(expected);
    if actual.len() != expected.len()
        || paired.clone().any(|(actual, expected)| {
            actual.input != expected.input
                || actual.disposition != expected.disposition
                || actual.sources != expected.sources
        })
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }
    if paired.any(|(actual, expected)| actual.work != expected.work) {
        return Err(OptimizationUnitValidationError::CandidateWorkMismatch);
    }
    Ok(())
}

pub(crate) fn candidate_node(
    block: &optimization_unit::OptimizationBlock,
    node: u32,
) -> Result<&OptimizationNode, OptimizationUnitValidationError> {
    let index = usize::try_from(node)
        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
    block
        .nodes
        .get(index)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)
}

pub(crate) fn candidate_block(
    function: &PsiOptimizationFunction,
    block: semantic_vocabulary::BlockId,
) -> Result<&optimization_unit::OptimizationBlock, OptimizationUnitValidationError> {
    function
        .block(block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)
}

pub(crate) fn realize_node_provenance<'node>(
    machine: semantic_vocabulary::MachineId,
    block: semantic_vocabulary::BlockId,
    nodes: impl Iterator<Item = (usize, &'node OptimizationNode)>,
    realized: &mut Vec<ProvenanceRewrite>,
) -> Option<()> {
    for (node_index, node) in nodes {
        if node.provenance.is_empty() {
            continue;
        }
        let site = optimization_unit::PsiRealizationSite::Node(optimization_unit::NodeLocation {
            machine,
            block,
            node: u32::try_from(node_index).ok()?,
        });
        realized.push(realized_at_own_site(site, &node.provenance, &node.work));
    }
    Some(())
}

pub(crate) fn retire_node_custody(
    block: &mut optimization_unit::OptimizationBlock,
    node_index: usize,
) -> Result<(), OptimizationUnitValidationError> {
    let retired = block.nodes.remove(node_index);
    let receiver = block
        .nodes
        .get_mut(node_index)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    receiver.provenance.extend_from_slice(&retired.provenance);
    receiver.work.extend_from_slice(&retired.work);
    Ok(())
}

pub(crate) fn canonicalize_provenance(rows: &mut [ProvenanceRewrite]) {
    rows.sort_by_key(ProvenanceRewrite::canonical_sort_key);
}

pub(crate) fn accept_region_rewrite(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
    machine: semantic_vocabulary::MachineId,
    expected_blocks: &[semantic_vocabulary::BlockId],
    validator: &[u8],
    provenance: Vec<ProvenanceRewrite>,
    rewrite: impl FnOnce(&mut PsiOptimizationUnit) -> Result<(), OptimizationUnitValidationError>,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    if candidate.affected_blocks() != expected_blocks || candidate.provenance() != provenance {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }
    let input_function = candidate_function(input, machine)?;
    let mut output = input.clone();
    rewrite(&mut output)?;
    output
        .function(machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let output_function = crate::unit_validation::revalidate_rewrite_output(&mut output, machine)?;
    outside_region::validate_outside_region(
        input_function.blocks.iter(),
        output_function,
        expected_blocks,
    )?;
    for input_function_other in &input.functions {
        if input_function_other.machine != machine
            && !output.functions.contains(input_function_other)
        {
            return Err(OptimizationUnitValidationError::CandidateOutsideRegionMismatch);
        }
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(validator),
        provenance,
    })
}
