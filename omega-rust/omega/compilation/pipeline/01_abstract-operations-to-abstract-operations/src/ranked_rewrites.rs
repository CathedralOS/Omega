/*
Authenticated ranked-cycle rewrites (`loop_invariant_scalar_motion`): exact
unique-preheader loop-invariant scalar and structural relocation proposal,
atomic node movement and custody rebinding, and independent relocation-plan
replay validation.
*/

pub(crate) mod loop_invariant_scalar_motion {
    use optimization_core::{
        AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
        OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
        OptimizationUnitIdentity, OptimizationValidatorIdentity,
    };
    use optimization_unit::{
        EffectLink, NodeLocation, OptimizationNode, ProvenanceDisposition, ProvenanceRewrite,
        PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance, PsiRealizationSite,
        PsiTransformationLedger, PsiTransformationRecord, ValueDefinitionSite,
        recompute_psi_optimization_unit_identity,
    };
    pub use optimization_unit::{
        LoopInvariantNodeResult, LoopInvariantScalarMotionCandidate, LoopInvariantScalarNode,
        LoopInvariantScalarRelocation,
    };
    use semantic_vocabulary::{BlockId, MachineId, OperationId, PlaceId, ValueId};

    use crate::{
        CountdownInvariantConstantAnalysisError, CountdownInvariantConstantPlacementAnalysisError,
        CountedLoopAnalysisError, VerifiedPsiOptimizationSession,
    };

    pub(crate) mod apply {
        use super::{
            AppliedLoopInvariantScalarMotion, LoopInvariantScalarMotionError,
            PsiTransformationLedger, PsiTransformationRecord, ValidatedLoopInvariantScalarMotion,
            VerifiedPsiOptimizationSession, rule_identity, validator_identity,
        };
        pub(crate) mod realize {
            use super::super::{
                EffectLink, LoopInvariantScalarMotionError, LoopInvariantScalarNode, NodeLocation,
                OperationId, PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance,
                ValueDefinitionSite, recompute_psi_optimization_unit_identity,
            };
            use crate::validation::member_blocks::shared_entry_source;
            use crate::validation::relocation_rewrites as relocations;
            use abstract_operations::{
                AbstractCallTarget, AbstractOperation as O, AbstractStructuralCallResult,
            };
            use optimization_unit::{OptimizationFact, OptimizerCycleComponent};
            use semantic_vocabulary::{BlockId, PlaceId};
            use std::collections::{BTreeMap, BTreeSet};
            use terminal_psi::StructuralMultiplicity;

            pub(crate) fn realize(
                unit: &PsiOptimizationUnit,
                component: &OptimizerCycleComponent,
                nodes: &[LoopInvariantScalarNode],
                certificate_tail: usize,
            ) -> Result<PsiOptimizationUnit, LoopInvariantScalarMotionError> {
                let mut output = unit.clone();
                let machine = component.id.machine;
                let function = output
                    .function_mut(machine)
                    .ok_or(LoopInvariantScalarMotionError::UnknownComponent)?;
                let mut requests = nodes
                    .iter()
                    .map(|node| (node.location(), node.psi_operation()))
                    .collect::<Vec<_>>();
                requests.sort_by_key(|(location, _)| {
                    (location.block, std::cmp::Reverse(location.node))
                });
                let mut removed = BTreeMap::new();
                for (location, operation) in requests {
                    if location.machine != machine {
                        return Err(LoopInvariantScalarMotionError::MissingNode {
                            machine: location.machine,
                            block: location.block,
                            node: location.node,
                        });
                    }
                    let block = function.block_mut(location.block).ok_or(
                        LoopInvariantScalarMotionError::MissingNode {
                            machine,
                            block: location.block,
                            node: location.node,
                        },
                    )?;
                    let index = usize::try_from(location.node)
                        .map_err(|_| LoopInvariantScalarMotionError::CoordinateOverflow)?;
                    let node = block.nodes.get(index).filter(|node| {
                        node.provenance.first() == Some(&PsiProvenance::Operation(operation))
                    });
                    if node.is_none() {
                        return Err(LoopInvariantScalarMotionError::MissingNode {
                            machine,
                            block: location.block,
                            node: location.node,
                        });
                    }
                    if removed
                        .insert(operation, block.nodes.remove(index))
                        .is_some()
                    {
                        return Err(LoopInvariantScalarMotionError::CandidateMismatch);
                    }
                }
                let nodes = nodes
                    .iter()
                    .map(|planned| {
                        let mut node = removed
                            .remove(&planned.psi_operation())
                            .ok_or(LoopInvariantScalarMotionError::CandidateMismatch)?;
                        let substitution = planned
                            .operand_rewrites()
                            .iter()
                            .copied()
                            .collect::<BTreeMap<_, _>>();
                        relocations::substitute_invariant_scalar_operands(
                            &mut node.operation,
                            &substitution,
                        );
                        for value_use in &mut node.uses {
                            if let Some(representative) = substitution.get(&value_use.value) {
                                value_use.value = *representative;
                            }
                        }
                        let structural_call = matches!(node.operation, O::CallStructural { .. });
                        let crash_custody = match &node.operation {
                            O::Call {
                                callee,
                                crash_continuations,
                                ..
                            }
                            | O::CallStructural {
                                callee: AbstractCallTarget::Machine(callee),
                                crash_continuations,
                                ..
                            } if !crash_continuations.is_empty() => Some(*callee),
                            _ => None,
                        };
                        if let Some(callee) = crash_custody {
                            let callee_function = unit
                                .function(callee)
                                .ok_or(LoopInvariantScalarMotionError::CandidateMismatch)?;
                            let (arguments, crash_continuations) = match &mut node.operation {
                                abstract_operations::call_or_call_structural_pattern!(
                                    arguments,
                                    crash_continuations,
                                    ..
                                ) => (arguments, crash_continuations),
                                _ => return Err(LoopInvariantScalarMotionError::CandidateMismatch),
                            };
                            *crash_continuations = if structural_call {
                                relocations::structural_call_crash_continuations(
                                    callee_function,
                                    arguments,
                                )
                            } else {
                                relocations::call_crash_continuations(callee_function, arguments)
                            }
                            .ok_or(LoopInvariantScalarMotionError::CandidateMismatch)?;
                        }
                        if let Some((parameter, representative)) = planned.root_rewrite()
                            && !relocations::substitute_invariant_place_root(
                                &mut node.operation,
                                parameter,
                                representative,
                            )
                        {
                            return Err(LoopInvariantScalarMotionError::CandidateMismatch);
                        }
                        if !planned.argument_rewrites().is_empty() {
                            let rewrites = planned
                                .argument_rewrites()
                                .iter()
                                .copied()
                                .collect::<BTreeMap<_, _>>();
                            if !relocations::substitute_invariant_call_roots(
                                &mut node.operation,
                                &rewrites,
                            ) {
                                return Err(LoopInvariantScalarMotionError::CandidateMismatch);
                            }
                        }
                        Ok(node)
                    })
                    .collect::<Result<Vec<_>, LoopInvariantScalarMotionError>>()?;
                let Some(preheader_source) = shared_entry_source(component) else {
                    return Err(LoopInvariantScalarMotionError::CandidateMismatch);
                };
                let preheader = function.block_mut(preheader_source).ok_or(
                    LoopInvariantScalarMotionError::MissingNode {
                        machine,
                        block: preheader_source,
                        node: 0,
                    },
                )?;
                let insertion = preheader
                    .nodes
                    .len()
                    .checked_sub(1)
                    .and_then(|jump| jump.checked_sub(certificate_tail))
                    .ok_or(LoopInvariantScalarMotionError::CandidateMismatch)?;
                let relocated_affine: BTreeSet<PlaceId> = nodes
                    .iter()
                    .filter_map(|node| match &node.operation {
                        abstract_operations::establish_case_or_record_result_pattern!(result)
                        | O::CallStructural {
                            result: AbstractStructuralCallResult::Structural { result, .. },
                            ..
                        } if result.multiplicity == StructuralMultiplicity::Affine => {
                            Some(result.place)
                        }
                        O::EstablishTrivialAffineLocal { place, .. } => Some(place.id),
                        _ => None,
                    })
                    .collect();
                let consumed: BTreeSet<PlaceId> = {
                    let seed_function = unit
                        .function(machine)
                        .ok_or(LoopInvariantScalarMotionError::UnknownComponent)?;
                    relocated_affine
                        .iter()
                        .copied()
                        .filter(|place| {
                            relocations::scalar_case_result_consumed(
                                seed_function,
                                component,
                                *place,
                            )
                        })
                        .collect()
                };
                let case_results: BTreeSet<PlaceId> =
                    relocated_affine.difference(&consumed).copied().collect();
                for (offset, node) in nodes.into_iter().enumerate() {
                    preheader.nodes.insert(insertion + offset, node);
                }
                if !case_results.is_empty() {
                    let members: BTreeSet<BlockId> = component.members.iter().copied().collect();
                    let structural_places = function.structural_places.clone();
                    for member in &component.members {
                        let block = function.block_mut(*member).ok_or(
                            LoopInvariantScalarMotionError::MissingNode {
                                machine,
                                block: *member,
                                node: 0,
                            },
                        )?;
                        for node in &mut block.nodes {
                            relocations::rewrite_scalar_case_custody(
                                &structural_places,
                                &members,
                                &case_results,
                                node,
                            );
                        }
                    }
                }
                refresh_coordinates_effects_and_facts(function)?;
                if !relocated_affine.is_empty() {
                    let relocated = BTreeMap::from([(machine, relocated_affine)]);
                    let mut facts = std::mem::take(&mut output.ownership_frontier_facts);
                    crate::validation::rewrite_relocated_case_result_frontiers(
                        &mut facts, &output, &relocated,
                    );
                    output.ownership_frontier_facts = facts;
                }
                output.identity = recompute_psi_optimization_unit_identity(&output);
                Ok(output)
            }

            pub(crate) fn operation_location(
                unit: &PsiOptimizationUnit,
                operation: OperationId,
            ) -> Option<NodeLocation> {
                let mut matches = unit.functions.iter().flat_map(|function| {
                    function.blocks.iter().flat_map(move |block| {
                        block
                            .nodes
                            .iter()
                            .enumerate()
                            .filter_map(move |(node, value)| {
                                (value.provenance.first()
                                    == Some(&PsiProvenance::Operation(operation)))
                                .then_some(NodeLocation {
                                    machine: function.machine,
                                    block: block.id,
                                    node: u32::try_from(node).ok()?,
                                })
                            })
                    })
                });
                let location = matches.next()?;
                matches.next().is_none().then_some(location)
            }

            fn refresh_coordinates_effects_and_facts(
                function: &mut PsiOptimizationFunction,
            ) -> Result<(), LoopInvariantScalarMotionError> {
                let mut effect = 0u64;
                for block in &mut function.blocks {
                    for (node_index, node) in block.nodes.iter_mut().enumerate() {
                        let node_index = u32::try_from(node_index)
                            .map_err(|_| LoopInvariantScalarMotionError::CoordinateOverflow)?;
                        for definition in &mut node.definitions {
                            definition.site = ValueDefinitionSite::Node {
                                block: block.id,
                                node: node_index,
                            };
                        }
                        for value_use in &mut node.uses {
                            value_use.block = block.id;
                            value_use.node = node_index;
                        }
                        node.effect = EffectLink {
                            input: effect,
                            output: effect
                                .checked_add(1)
                                .ok_or(LoopInvariantScalarMotionError::CoordinateOverflow)?,
                        };
                        effect = node.effect.output;
                    }
                }
                let operation_order = function
                    .block_nodes()
                    .enumerate()
                    .filter_map(|(position, node)| match node.provenance.first() {
                        Some(PsiProvenance::Operation(operation)) => Some((*operation, position)),
                        _ => None,
                    })
                    .collect::<BTreeMap<_, _>>();
                function.facts.sort_by_key(|fact| {
                    let support = match fact {
                        OptimizationFact::OperationObligationReference { support, .. }
                        | OptimizationFact::BooleanConstant { support, .. }
                        | OptimizationFact::IntegerConstant { support, .. } => support,
                    };
                    operation_order.get(support).copied()
                });
                Ok(())
            }
        }

        pub(crate) use realize::{operation_location, realize};

        pub(crate) fn validated(
            session: VerifiedPsiOptimizationSession,
            validated: ValidatedLoopInvariantScalarMotion,
        ) -> Result<AppliedLoopInvariantScalarMotion, LoopInvariantScalarMotionError> {
            if validated.candidate.input() != session.unit().identity {
                return Err(LoopInvariantScalarMotionError::StaleCandidateRevision {
                    candidate: validated.candidate.input(),
                    current: session.unit().identity,
                });
            }
            if validated.output.identity != validated.candidate.output() {
                return Err(LoopInvariantScalarMotionError::OutputIdentityMismatch {
                    candidate: validated.candidate.output(),
                    reconstructed: validated.output.identity,
                });
            }
            let input = session.unit().identity;
            let terminal = session.unit().psi;
            let work_schedule = session.unit().work_schedule;
            let (verified_input, _) = session.into_parts();
            let next =
                VerifiedPsiOptimizationSession::from_transformed(verified_input, validated.output)
                    .map_err(LoopInvariantScalarMotionError::TransformedValidation)?;
            reconstruct_custody(&next)?;
            let record = PsiTransformationRecord {
                rule: rule_identity(),
                candidate: validated.candidate.identity(),
                validator: validator_identity(),
                input,
                output: next.unit().identity,
                pruned_machines: Vec::new(),
                provenance: validated.provenance,
            };
            let ledger = PsiTransformationLedger::new(
                terminal,
                work_schedule,
                input,
                next.unit().identity,
                vec![record],
            )
            .map_err(LoopInvariantScalarMotionError::InvalidLedger)?;
            Ok(AppliedLoopInvariantScalarMotion {
                session: next,
                candidate: validated.candidate,
                ledger,
            })
        }

        pub(crate) fn reconstruct_custody(
            session: &VerifiedPsiOptimizationSession,
        ) -> Result<(), LoopInvariantScalarMotionError> {
            if !session.ranking_certificates().certificates().is_empty() {
                session
                    .counted_loop_analysis()
                    .map_err(LoopInvariantScalarMotionError::CountedLoop)?;
                session
                    .countdown_invariant_constant_analysis()
                    .map_err(LoopInvariantScalarMotionError::InvariantConstant)?;
                session
                    .countdown_invariant_constant_placement_analysis()
                    .map_err(LoopInvariantScalarMotionError::ReconstructedPlacement)?;
            }
            Ok(())
        }
    }
    pub(crate) mod propose {
        use super::{
            BlockId, LoopInvariantNodeResult, LoopInvariantScalarMotionCandidate,
            LoopInvariantScalarMotionError, LoopInvariantScalarNode, LoopInvariantScalarRelocation,
            NodeLocation, OperationId, OptimizationNode, PlaceId, PsiProvenance,
            ValueDefinitionSite, ValueId, VerifiedPsiOptimizationSession, apply, contract,
        };
        use crate::validation::{
            invariant_calls as calls, invariant_operations as ops, member_blocks as blocks,
            place_observations as places, relocation_rewrites as relocations,
        };
        use abstract_operations::{AbstractOperation, AbstractStructuralCallResult};
        pub(crate) fn all(
            session: &VerifiedPsiOptimizationSession,
            candidate_limit: u64,
        ) -> Result<Vec<LoopInvariantScalarMotionCandidate>, LoopInvariantScalarMotionError>
        {
            let mut candidates = Vec::new();
            let effects = calls::unit_effect_summaries(session.unit())
                .map_err(LoopInvariantScalarMotionError::CallableProvenance)?;
            for component in session.cycle_components().components() {
                if let Some(candidate) = component_candidate(session, component, &effects)? {
                    candidates.push(candidate);
                }
            }
            let required = u64::try_from(candidates.len())
                .map_err(|_| LoopInvariantScalarMotionError::CoordinateOverflow)?;
            if required > candidate_limit {
                return Err(LoopInvariantScalarMotionError::CandidateBudgetExhausted {
                    required,
                    limit: candidate_limit,
                });
            }
            Ok(candidates)
        }

        fn component_candidate(
            session: &VerifiedPsiOptimizationSession,
            component: &optimization_unit::OptimizerCycleComponent,
            effects: &crate::EffectSummaryAnalysis,
        ) -> Result<Option<LoopInvariantScalarMotionCandidate>, LoopInvariantScalarMotionError>
        {
            let Some(plan) = component_plan(session, component, effects)? else {
                return Ok(None);
            };
            let output = apply::realize(
                session.unit(),
                component,
                &plan.nodes,
                plan.certificate_tail,
            )?;
            let relocations = plan
                .nodes
                .iter()
                .map(|node| {
                    Ok(LoopInvariantScalarRelocation::new(
                        node.clone(),
                        apply::operation_location(&output, node.psi_operation())
                            .ok_or(LoopInvariantScalarMotionError::CandidateMismatch)?,
                    ))
                })
                .collect::<Result<Vec<_>, LoopInvariantScalarMotionError>>()?;
            Ok(Some(LoopInvariantScalarMotionCandidate::new(
                contract(),
                session.unit().identity,
                output.identity,
                component.id.clone(),
                relocations,
            )))
        }

        pub(crate) struct ComponentPlan {
            pub(crate) nodes: Vec<LoopInvariantScalarNode>,
            pub(crate) certificate_tail: usize,
        }

        struct MemberAdmission {
            operand_rewrites: Vec<(ValueId, ValueId)>,
            root_rewrite: Option<(PlaceId, PlaceId)>,
            argument_rewrites: Vec<(PlaceId, PlaceId)>,
        }

        struct PlanEvidence<'a> {
            functions: &'a [optimization_unit::PsiOptimizationFunction],
            function: &'a optimization_unit::PsiOptimizationFunction,
            component: &'a optimization_unit::OptimizerCycleComponent,
            preheader_source: BlockId,
            insertion: usize,
            guaranteed_entry: bool,
            guaranteed: std::collections::BTreeSet<BlockId>,
            sites: std::collections::BTreeMap<ValueId, ValueDefinitionSite>,
            call_effects: &'a crate::EffectSummaryAnalysis,
        }

        impl PlanEvidence<'_> {
            fn representable(
                &self,
                substitution: &std::collections::BTreeMap<ValueId, ValueId>,
                relocating: &std::collections::BTreeSet<ValueId>,
            ) -> bool {
                substitution.values().all(|representative| {
                    relocating.contains(representative)
                        || match self.sites.get(representative) {
                            Some(ValueDefinitionSite::FunctionParameter(_)) => true,
                            Some(ValueDefinitionSite::BlockParameter { block, .. })
                                if *block == self.preheader_source =>
                            {
                                true
                            }
                            Some(ValueDefinitionSite::Node {
                                block,
                                node: defined,
                            }) if *block == self.preheader_source => usize::try_from(*defined)
                                .is_ok_and(|defined| defined < self.insertion),
                            _ => false,
                        }
                })
            }
        }

        fn admit_substitution(
            evidence: &PlanEvidence<'_>,
            relocating: &std::collections::BTreeSet<ValueId>,
            substitution: std::collections::BTreeMap<ValueId, ValueId>,
        ) -> Option<Vec<(ValueId, ValueId)>> {
            if !evidence.representable(&substitution, relocating) {
                return None;
            }
            Some(substitution.into_iter().collect())
        }

        fn admit_member_node(
            evidence: &PlanEvidence<'_>,
            member: BlockId,
            node: &OptimizationNode,
            relocating: &std::collections::BTreeSet<ValueId>,
            relocating_roots: &std::collections::BTreeSet<PlaceId>,
        ) -> Option<MemberAdmission> {
            let function = evidence.function;
            let component = evidence.component;
            if !(evidence.guaranteed_entry && evidence.guaranteed.contains(&member)) {
                return None;
            }
            let effects = evidence.call_effects;
            let run = calls::StructuralCallRun {
                functions: evidence.functions,
                function,
                component,
                node,
                relocating,
                relocating_roots,
                effects,
            };
            let produces_storage = ops::admissible_invariant_byte_literal(node)
                || ops::admissible_invariant_primitive_local(node).is_some()
                || ops::admissible_invariant_record(node).is_some()
                || ops::admissible_invariant_scalar_array(node).is_some()
                || ops::admissible_invariant_scalar_case(node).is_some()
                || ops::admissible_invariant_trivial_affine_local(node).is_some();
            if produces_storage
                && !calls::component_members_unobservable(
                    function,
                    component,
                    evidence.call_effects,
                )
            {
                return None;
            }
            let mut root_rewrite = None;
            let mut argument_rewrites = Vec::new();
            let mut admit_call = |admission: Option<(
                std::collections::BTreeMap<ValueId, ValueId>,
                Vec<(PlaceId, PlaceId)>,
            )>|
             -> Option<Vec<(ValueId, ValueId)>> {
                let (substitution, rewrites) = admission?;
                argument_rewrites = rewrites;
                admit_substitution(evidence, relocating, substitution)
            };
            let operand_rewrites = if ops::admissible_scalar_leaf_relocation(node) {
                Vec::new()
            } else if let Some(source) = ops::admissible_invariant_place_read(node) {
                let root = places::invariant_place_observation_admission(
                    function,
                    component,
                    node,
                    relocating_roots,
                )?;
                root_rewrite = (root != source).then_some((source, root));
                Vec::new()
            } else if let Some((source, _, _)) = ops::admissible_invariant_byte_read(node) {
                let (root, substitution) = places::invariant_byte_read_admission(
                    function,
                    component,
                    node,
                    relocating,
                    relocating_roots,
                )?;
                if !evidence.representable(&substitution, relocating) {
                    return None;
                }
                root_rewrite = (root != source).then_some((source, root));
                substitution.into_iter().collect()
            } else if let Some((source, _, _)) = ops::admissible_invariant_field_byte_read(node) {
                let (root, substitution) = places::invariant_field_byte_read_admission(
                    function,
                    component,
                    node,
                    relocating,
                    relocating_roots,
                )?;
                if !evidence.representable(&substitution, relocating) {
                    return None;
                }
                root_rewrite = (root != source).then_some((source, root));
                substitution.into_iter().collect()
            } else if let Some((source, _, _, _)) = ops::admissible_invariant_subslice(node) {
                let (root, substitution) = places::invariant_subslice_admission(
                    function,
                    component,
                    node,
                    relocating,
                    relocating_roots,
                )?;
                if !evidence.representable(&substitution, relocating) {
                    return None;
                }
                root_rewrite = (root != source).then_some((source, root));
                substitution.into_iter().collect()
            } else if ops::admissible_invariant_byte_literal(node) {
                Vec::new()
            } else if ops::admissible_invariant_primitive_local(node).is_some() {
                let substitution = ops::invariant_primitive_local_admission(
                    function, component, node, relocating,
                )?;
                admit_substitution(evidence, relocating, substitution)?
            } else if ops::admissible_invariant_record(node).is_some() {
                admit_call(ops::invariant_record_admission(
                    function,
                    component,
                    node,
                    relocating,
                    relocating_roots,
                ))?
            } else if ops::admissible_invariant_scalar_array(node).is_some() {
                let substitution =
                    ops::invariant_scalar_array_admission(function, component, node, relocating)?;
                admit_substitution(evidence, relocating, substitution)?
            } else if ops::admissible_invariant_scalar_case(node).is_some() {
                let substitution =
                    ops::invariant_scalar_case_admission(function, component, node, relocating)?;
                admit_substitution(evidence, relocating, substitution)?
            } else if ops::admissible_invariant_trivial_affine_local(node).is_some() {
                ops::invariant_trivial_affine_local_admission(function, component, node)?;
                Vec::new()
            } else if calls::admissible_invariant_scalar_call(node).is_some() {
                let substitution = calls::invariant_scalar_call_admission(
                    function, component, node, relocating, effects,
                )?;
                admit_substitution(evidence, relocating, substitution)?
            } else if calls::admissible_invariant_crash_continuation_call(node).is_some() {
                let substitution = calls::invariant_crash_continuation_call_admission(
                    evidence.functions,
                    function,
                    component,
                    node,
                    relocating,
                    effects,
                )?;
                admit_substitution(evidence, relocating, substitution)?
            } else if calls::admissible_invariant_unit_call(node).is_some() {
                admit_call(calls::invariant_unit_call_admission(&run))?
            } else if calls::admissible_invariant_crash_continuation_unit_call(node).is_some() {
                admit_call(calls::invariant_crash_continuation_unit_call_admission(
                    &run,
                ))?
            } else if calls::admissible_invariant_structural_scalar_call(node).is_some() {
                admit_call(calls::invariant_structural_scalar_call_admission(&run))?
            } else if calls::admissible_invariant_crash_continuation_structural_scalar_call(node)
                .is_some()
            {
                admit_call(
                    calls::invariant_crash_continuation_structural_scalar_call_admission(&run),
                )?
            } else if calls::admissible_invariant_structural_call(node).is_some() {
                admit_call(calls::invariant_structural_call_admission(&run))?
            } else if calls::admissible_invariant_crash_continuation_structural_call(node).is_some()
            {
                admit_call(calls::invariant_crash_continuation_structural_call_admission(&run))?
            } else {
                let substitution = blocks::invariant_scalar_operand_substitution(
                    function, component, node, relocating,
                )?;
                admit_substitution(evidence, relocating, substitution)?
            };
            Some(MemberAdmission {
                operand_rewrites,
                root_rewrite,
                argument_rewrites,
            })
        }

        fn roots_relocate(
            evidence: &PlanEvidence<'_>,
            root: PlaceId,
            relocating: &std::collections::BTreeSet<ValueId>,
            relocating_roots: &std::collections::BTreeSet<PlaceId>,
            observed_roots: impl Fn(&AbstractOperation) -> std::collections::BTreeSet<PlaceId>,
        ) -> bool {
            let mut extended = relocating_roots.clone();
            extended.insert(root);
            for block in &evidence.function.blocks {
                for node in &block.nodes {
                    if !observed_roots(&node.operation).contains(&root) {
                        continue;
                    }
                    if !evidence.component.members.contains(&block.id)
                        || admit_member_node(evidence, block.id, node, relocating, &extended)
                            .is_none()
                    {
                        return false;
                    }
                }
            }
            true
        }

        fn mutable_borrowers_relocate(
            evidence: &PlanEvidence<'_>,
            root: PlaceId,
            relocating: &std::collections::BTreeSet<ValueId>,
            relocating_roots: &std::collections::BTreeSet<PlaceId>,
        ) -> bool {
            roots_relocate(
                evidence,
                root,
                relocating,
                relocating_roots,
                places::mutable_borrow_roots,
            )
        }

        fn owned_consumers_relocate(
            evidence: &PlanEvidence<'_>,
            root: PlaceId,
            relocating: &std::collections::BTreeSet<ValueId>,
            relocating_roots: &std::collections::BTreeSet<PlaceId>,
        ) -> bool {
            roots_relocate(
                evidence,
                root,
                relocating,
                relocating_roots,
                places::owned_argument_roots,
            )
        }

        pub(crate) fn component_plan(
            session: &VerifiedPsiOptimizationSession,
            component: &optimization_unit::OptimizerCycleComponent,
            effects: &crate::EffectSummaryAnalysis,
        ) -> Result<Option<ComponentPlan>, LoopInvariantScalarMotionError> {
            let machine = component.id.machine;
            let function = session
                .unit()
                .function(machine)
                .ok_or(LoopInvariantScalarMotionError::UnknownComponent)?;
            let Some(preheader_source) = blocks::shared_entry_source(component) else {
                return Ok(None);
            };
            let preheader = function
                .block(preheader_source)
                .ok_or(LoopInvariantScalarMotionError::UnknownComponent)?;
            let Some(terminator_index) = preheader.nodes.len().checked_sub(1) else {
                return Ok(None);
            };
            let members: std::collections::BTreeSet<_> =
                component.members.iter().copied().collect();
            let guaranteed_entry = !preheader.nodes[terminator_index].successors.is_empty()
                && preheader.nodes[terminator_index]
                    .successors
                    .iter()
                    .all(|edge| members.contains(&edge.target));
            let certificate_operations = certificate_operations(session, component);
            let certificate_tail = preheader.nodes[..terminator_index]
                .iter()
                .rev()
                .take_while(|node| {
                    matches!(
                        node.provenance.first(),
                        Some(PsiProvenance::Operation(operation))
                            if certificate_operations.contains(operation)
                    )
                })
                .count();
            let insertion = terminator_index - certificate_tail;
            let sites = blocks::value_definition_sites(function);
            let guaranteed = blocks::guaranteed_executed_member_blocks(component);
            let mut nodes = Vec::new();
            let mut relocating = std::collections::BTreeSet::new();
            let mut relocating_roots = std::collections::BTreeSet::new();
            let mut admitted = std::collections::BTreeSet::new();
            let evidence = PlanEvidence {
                functions: &session.unit().functions,
                function,
                component,
                preheader_source,
                insertion,
                guaranteed_entry,
                guaranteed,
                sites,
                call_effects: effects,
            };
            loop {
                let mut progressed = false;
                for member in &component.members {
                    let block = function
                        .block(*member)
                        .ok_or(LoopInvariantScalarMotionError::CandidateMismatch)?;
                    for (index, node) in block.nodes.iter().enumerate() {
                        let psi_operation = match node.provenance.first() {
                            Some(&PsiProvenance::Operation(operation)) => operation,
                            _ => continue,
                        };
                        if admitted.contains(&psi_operation)
                            || certificate_operations.contains(&psi_operation)
                        {
                            continue;
                        }
                        let Some(MemberAdmission {
                            operand_rewrites,
                            root_rewrite,
                            argument_rewrites,
                        }) = admit_member_node(
                            &evidence,
                            *member,
                            node,
                            &relocating,
                            &relocating_roots,
                        )
                        else {
                            continue;
                        };
                        if let Some(root) = places::produced_place_root(&node.operation)
                            && !(mutable_borrowers_relocate(
                                &evidence,
                                root,
                                &relocating,
                                &relocating_roots,
                            ) && (!relocations::restricted_produced_root(&node.operation)
                                || owned_consumers_relocate(
                                    &evidence,
                                    root,
                                    &relocating,
                                    &relocating_roots,
                                )))
                        {
                            continue;
                        }
                        let result = match node.definitions.as_slice() {
                            [definition] => {
                                relocating.insert(definition.value);
                                LoopInvariantNodeResult::Scalar {
                                    value: definition.value,
                                    scalar_type: definition.scalar_type,
                                }
                            }
                            [] => match &node.operation {
                                AbstractOperation::ByteSequenceSubslice { result, .. }
                                | AbstractOperation::ElementViewSubslice { result, .. }
                                    if ops::admissible_invariant_subslice(node).is_some() =>
                                {
                                    LoopInvariantNodeResult::Structural(result.clone())
                                }
                                AbstractOperation::EstablishByteSequenceLiteral { place, .. }
                                    if ops::admissible_invariant_byte_literal(node) =>
                                {
                                    LoopInvariantNodeResult::LiteralPlace(*place)
                                }
                                AbstractOperation::EstablishPrimitiveLocal { result, .. }
                                    if ops::admissible_invariant_primitive_local(node).is_some() =>
                                {
                                    LoopInvariantNodeResult::Structural(result.clone())
                                }
                                AbstractOperation::EstablishRecord { result, .. }
                                    if ops::admissible_invariant_record(node).is_some() =>
                                {
                                    LoopInvariantNodeResult::Structural(result.clone())
                                }
                                AbstractOperation::EstablishScalarArray { result, .. }
                                    if ops::admissible_invariant_scalar_array(node).is_some() =>
                                {
                                    LoopInvariantNodeResult::Structural(result.clone())
                                }
                                AbstractOperation::EstablishCase { result, .. }
                                    if ops::admissible_invariant_scalar_case(node).is_some() =>
                                {
                                    LoopInvariantNodeResult::Structural(result.clone())
                                }
                                AbstractOperation::EstablishTrivialAffineLocal { place, .. }
                                    if ops::admissible_invariant_trivial_affine_local(node).is_some() =>
                                {
                                    LoopInvariantNodeResult::TrivialAffineLocal(*place)
                                }
                                AbstractOperation::CallStructural {
                                    result: AbstractStructuralCallResult::Unit,
                                    ..
                                } if calls::admissible_invariant_unit_call(node).is_some()
                                    || calls::admissible_invariant_crash_continuation_unit_call(node)
                                        .is_some() =>
                                {
                                    LoopInvariantNodeResult::Unit
                                }
                                AbstractOperation::CallStructural {
                                    result: AbstractStructuralCallResult::Structural { result, .. },
                                    ..
                                } if calls::admissible_invariant_structural_call(node).is_some()
                                    || calls::admissible_invariant_crash_continuation_structural_call(
                                        node,
                                    )
                                    .is_some() =>
                                {
                                    LoopInvariantNodeResult::Structural(result.clone())
                                }
                                _ => return Err(LoopInvariantScalarMotionError::CandidateMismatch),
                            },
                            _ => return Err(LoopInvariantScalarMotionError::CandidateMismatch),
                        };
                        admitted.insert(psi_operation);
                        if let Some(root) = places::produced_place_root(&node.operation) {
                            relocating_roots.insert(root);
                        }
                        nodes.push(LoopInvariantScalarNode::new(
                            psi_operation,
                            result,
                            NodeLocation {
                                machine,
                                block: *member,
                                node: u32::try_from(index).map_err(|_| {
                                    LoopInvariantScalarMotionError::CoordinateOverflow
                                })?,
                            },
                            operand_rewrites,
                            root_rewrite,
                            argument_rewrites,
                            node.provenance.clone(),
                            node.work.clone(),
                        ));
                        progressed = true;
                    }
                }
                if !progressed {
                    break;
                }
            }
            if nodes.is_empty() {
                return Ok(None);
            }
            Ok(Some(ComponentPlan {
                nodes,
                certificate_tail,
            }))
        }

        fn certificate_operations(
            session: &VerifiedPsiOptimizationSession,
            component: &optimization_unit::OptimizerCycleComponent,
        ) -> std::collections::BTreeSet<OperationId> {
            session
                .ranking_certificates()
                .certificates()
                .iter()
                .filter(|certificate| certificate.component == component.id)
                .flat_map(|certificate| {
                    [
                        certificate.guard.zero_operation,
                        certificate.descent.one_operation,
                    ]
                })
                .collect()
        }
    }
    pub(crate) mod validate {
        use super::{
            LoopInvariantScalarMotionCandidate, LoopInvariantScalarMotionError,
            LoopInvariantScalarRelocation, MachineId, NodeLocation, OptimizationNode,
            ProvenanceDisposition, ProvenanceRewrite, PsiOptimizationUnit, PsiProvenance,
            PsiRealizationSite, ValidatedLoopInvariantScalarMotion, VerifiedPsiOptimizationSession,
            apply, contract, propose,
        };
        pub(crate) fn candidate(
            session: &VerifiedPsiOptimizationSession,
            candidate: &LoopInvariantScalarMotionCandidate,
        ) -> Result<ValidatedLoopInvariantScalarMotion, LoopInvariantScalarMotionError> {
            if candidate.input() != session.unit().identity {
                return Err(LoopInvariantScalarMotionError::StaleCandidateRevision {
                    candidate: candidate.input(),
                    current: session.unit().identity,
                });
            }
            let component = session
                .cycle_components()
                .components()
                .iter()
                .find(|component| &component.id == candidate.component())
                .ok_or(LoopInvariantScalarMotionError::UnknownComponent)?;
            let effects = crate::validation::invariant_calls::unit_effect_summaries(session.unit())
                .map_err(LoopInvariantScalarMotionError::CallableProvenance)?;
            let Some(plan) = propose::component_plan(session, component, &effects)? else {
                return Err(LoopInvariantScalarMotionError::AlreadyRelocated);
            };

            let output = apply::realize(
                session.unit(),
                component,
                &plan.nodes,
                plan.certificate_tail,
            )?;
            let relocations = plan
                .nodes
                .iter()
                .map(|node| {
                    Ok(LoopInvariantScalarRelocation::new(
                        node.clone(),
                        apply::operation_location(&output, node.psi_operation())
                            .ok_or(LoopInvariantScalarMotionError::CandidateMismatch)?,
                    ))
                })
                .collect::<Result<Vec<_>, LoopInvariantScalarMotionError>>()?;
            let expected = LoopInvariantScalarMotionCandidate::new(
                contract(),
                session.unit().identity,
                output.identity,
                component.id.clone(),
                relocations,
            );
            if *candidate != expected {
                return Err(LoopInvariantScalarMotionError::CandidateMismatch);
            }

            let reconstructed = VerifiedPsiOptimizationSession::from_transformed(
                session.input().clone(),
                output.clone(),
            )
            .map_err(LoopInvariantScalarMotionError::TransformedValidation)?;
            apply::reconstruct_custody(&reconstructed)?;
            let provenance = reconstruct_provenance(session.unit(), &output, candidate)?;
            if provenance.is_empty() {
                return Err(LoopInvariantScalarMotionError::AlreadyRelocated);
            }
            Ok(ValidatedLoopInvariantScalarMotion {
                candidate: candidate.clone(),
                output,
                provenance,
            })
        }

        fn reconstruct_provenance(
            input: &PsiOptimizationUnit,
            output: &PsiOptimizationUnit,
            candidate: &LoopInvariantScalarMotionCandidate,
        ) -> Result<Vec<ProvenanceRewrite>, LoopInvariantScalarMotionError> {
            let machine = candidate.component().machine;
            let function = input
                .function(machine)
                .ok_or(LoopInvariantScalarMotionError::UnknownComponent)?;
            let mut rows = Vec::new();
            for block in &function.blocks {
                for (node_index, node) in block.nodes.iter().enumerate() {
                    let Some(source) = node.provenance.first().copied() else {
                        continue;
                    };
                    let input_location = NodeLocation {
                        machine,
                        block: block.id,
                        node: u32::try_from(node_index)
                            .map_err(|_| LoopInvariantScalarMotionError::CoordinateOverflow)?,
                    };
                    let (output_location, output_node) = unique_source(output, machine, source)
                        .ok_or(LoopInvariantScalarMotionError::CandidateMismatch)?;
                    if output_node.provenance != node.provenance || output_node.work != node.work {
                        return Err(LoopInvariantScalarMotionError::CandidateMismatch);
                    }
                    if input_location == output_location
                        && node.effect == output_node.effect
                        && node.definitions == output_node.definitions
                        && node.uses == output_node.uses
                    {
                        continue;
                    }
                    let (sources, work) = candidate
                        .relocations()
                        .iter()
                        .find(|relocation| relocation.node().location() == input_location)
                        .map(|relocation| {
                            (
                                relocation.node().provenance().to_vec(),
                                relocation.node().work().to_vec(),
                            )
                        })
                        .unwrap_or_else(|| (node.provenance.clone(), node.work.clone()));
                    if sources != node.provenance || work != node.work {
                        return Err(LoopInvariantScalarMotionError::CandidateMismatch);
                    }
                    rows.push(ProvenanceRewrite {
                        input: PsiRealizationSite::Node(input_location),
                        disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(
                            output_location,
                        )),
                        sources,
                        work,
                    });
                }
            }
            rows.sort_by_key(ProvenanceRewrite::canonical_sort_key);
            Ok(rows)
        }

        fn unique_source(
            unit: &PsiOptimizationUnit,
            machine: MachineId,
            source: PsiProvenance,
        ) -> Option<(NodeLocation, &OptimizationNode)> {
            let function = unit.function(machine)?;
            let mut matches = function.blocks.iter().flat_map(|block| {
                block
                    .nodes
                    .iter()
                    .enumerate()
                    .filter_map(move |(node, value)| {
                        (value.provenance.first() == Some(&source)).then_some((
                            NodeLocation {
                                machine,
                                block: block.id,
                                node: u32::try_from(node).ok()?,
                            },
                            value,
                        ))
                    })
            });
            let row = matches.next()?;
            matches.next().is_none().then_some(row)
        }
    }

    pub(crate) fn propose_loop_invariant_scalar_motion(
        session: &VerifiedPsiOptimizationSession,
        candidate_limit: u64,
    ) -> Result<Vec<LoopInvariantScalarMotionCandidate>, LoopInvariantScalarMotionError> {
        propose::all(session, candidate_limit)
    }

    pub(crate) fn validate_loop_invariant_scalar_motion(
        session: &VerifiedPsiOptimizationSession,
        candidate: &LoopInvariantScalarMotionCandidate,
    ) -> Result<ValidatedLoopInvariantScalarMotion, LoopInvariantScalarMotionError> {
        validate::candidate(session, candidate)
    }

    pub(crate) fn apply_loop_invariant_scalar_motion(
        session: VerifiedPsiOptimizationSession,
        validated: ValidatedLoopInvariantScalarMotion,
    ) -> Result<AppliedLoopInvariantScalarMotion, LoopInvariantScalarMotionError> {
        apply::validated(session, validated)
    }

    pub(crate) const LOOP_INVARIANT_SCALAR_MOTION_PASS_NAME: &[u8] =
        b"omega.psi-pass.loop-invariant-scalar-motion.v1";

    pub(crate) fn rule_identity() -> OptimizationRuleIdentity {
        OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.loop-invariant-scalar-motion.v1",
        )
    }

    pub(crate) fn validator_identity() -> OptimizationValidatorIdentity {
        OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.psi-validator.loop-invariant-scalar-motion.v1",
        )
    }

    pub(crate) fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            rule_identity(),
            OptimizationPassIdentity::from_canonical_bytes(LOOP_INVARIANT_SCALAR_MOTION_PASS_NAME),
            1,
            AnalysisSet::new([AnalysisKind::EffectSummaries]),
            AnalysisInvalidationSet::new(AnalysisKind::ALL),
            OptimizationSafetyClass::StructuralIdentity,
        )
        .expect("built-in pass contract has nonzero version")
    }

    #[derive(Debug)]
    pub(crate) struct ValidatedLoopInvariantScalarMotion {
        candidate: LoopInvariantScalarMotionCandidate,
        output: PsiOptimizationUnit,
        provenance: Vec<ProvenanceRewrite>,
    }

    impl ValidatedLoopInvariantScalarMotion {
        pub const fn candidate(&self) -> &LoopInvariantScalarMotionCandidate {
            &self.candidate
        }

        pub(crate) fn validator(&self) -> OptimizationValidatorIdentity {
            validator_identity()
        }

        pub fn provenance(&self) -> &[ProvenanceRewrite] {
            &self.provenance
        }
    }

    #[derive(Debug)]
    pub(crate) struct AppliedLoopInvariantScalarMotion {
        session: VerifiedPsiOptimizationSession,
        candidate: LoopInvariantScalarMotionCandidate,
        ledger: PsiTransformationLedger,
    }

    impl AppliedLoopInvariantScalarMotion {
        pub fn into_parts(
            self,
        ) -> (
            VerifiedPsiOptimizationSession,
            LoopInvariantScalarMotionCandidate,
            PsiTransformationLedger,
        ) {
            (self.session, self.candidate, self.ledger)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum LoopInvariantScalarMotionError {
        CallableProvenance(&'static str),
        CandidateBudgetExhausted {
            required: u64,
            limit: u64,
        },
        StaleCandidateRevision {
            candidate: OptimizationUnitIdentity,
            current: OptimizationUnitIdentity,
        },
        UnknownComponent,
        AlreadyRelocated,
        CandidateMismatch,
        MissingNode {
            machine: MachineId,
            block: BlockId,
            node: u32,
        },
        CoordinateOverflow,
        OutputIdentityMismatch {
            candidate: OptimizationUnitIdentity,
            reconstructed: OptimizationUnitIdentity,
        },
        TransformedValidation(optimization_unit_semantics::OptimizationUnitValidationError),
        CountedLoop(CountedLoopAnalysisError),
        InvariantConstant(CountdownInvariantConstantAnalysisError),
        ReconstructedPlacement(CountdownInvariantConstantPlacementAnalysisError),
        InvalidLedger(optimization_unit::InvalidPsiTransformationLedger),
    }

    failed_display!(
        LoopInvariantScalarMotionError,
        "loop-invariant scalar motion failure"
    );
}

pub(crate) use loop_invariant_scalar_motion::LoopInvariantScalarMotionError;
