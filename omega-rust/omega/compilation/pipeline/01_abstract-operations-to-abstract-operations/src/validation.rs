/*
Abstract-optimization context admission, loop-invariant scalar-motion boundary
verifiers (`invariant_calls`, `invariant_operations`, `member_blocks`,
`place_observations`, `relocation_rewrites`, `scalar_case_frontiers`),
optimized plan projection sealing (`projection`), and pre-physical optimization
manifest projection (`prephysical_manifest`).
*/

pub(crate) use abstract_operations::AbstractOperation as O;
pub(crate) use std::collections::{BTreeMap, BTreeSet};

pub(crate) mod context {
    use optimization_unit::PsiOptimizationUnit;
    use optimization_unit_semantics::OptimizationUnitValidationError;

    pub(crate) use crate::ranked_cycles_validation as ranked_cycles;

    pub(crate) fn validate_verified_psi_context(
        verified: &crate::VerifiedPsiOptimizationUnit,
    ) -> Result<ranked_cycles::ValidatedOptimizerCycleComponents, OptimizationUnitValidationError>
    {
        derive_context(verified.input(), verified.unit())
    }

    pub(crate) fn validate_transformed_psi_context(
        input: &verified_optimization_input::VerifiedPsiOptimizationInput,
        unit: &PsiOptimizationUnit,
    ) -> Result<ranked_cycles::ValidatedOptimizerCycleComponents, OptimizationUnitValidationError>
    {
        derive_context(input, unit)
    }

    fn derive_context(
        input: &verified_optimization_input::VerifiedPsiOptimizationInput,
        unit: &PsiOptimizationUnit,
    ) -> Result<ranked_cycles::ValidatedOptimizerCycleComponents, OptimizationUnitValidationError>
    {
        optimization_unit::validate_runtime_evidence_context(
            unit,
            input.plan().psi,
            input.context().module(),
        )
        .map_err(|reason| OptimizationUnitValidationError::InvalidRuntimeEvidence { reason })?;
        let cycle_admission = ranked_cycles::validate_exact_ranked_cycles(input, unit)?;
        Ok(ranked_cycles::ValidatedOptimizerCycleComponents::new(
            cycle_admission.snapshot,
            ranked_cycles::ValidatedOptimizerRankingCertificates::new(cycle_admission.rankings),
        ))
    }

    pub(crate) use ranked_cycles::{
        ValidatedOptimizerCycleComponents, ValidatedOptimizerRankingCertificates,
    };
}
pub(crate) mod invariant_calls {
    use super::member_blocks::{member_scalar_operand_substitution, shared_entry_source};
    use super::place_observations::{
        component_preserves_place_observations_tolerating, consumed_owned_argument_root,
        copyable_owned_argument_root, invariant_member_place_parameters,
        member_root_producer_count, place_observation_root_visible,
    };
    use super::relocation_rewrites::scalar_case_result_contained;
    use crate::validation::relocation_rewrites::call_crash_continuations;
    use crate::validation::relocation_rewrites::structural_call_crash_continuations;
    use abstract_operations::{AbstractOperation as O, AbstractStructuralCallResult};
    use optimization_unit::{
        OptimizationBlock, OptimizationNode, OptimizerCycleComponent, OwnershipEvent,
        PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance,
    };
    use semantic_vocabulary::{MachineId, PlaceId, ValueId};
    pub(crate) use std::collections::{BTreeMap, BTreeSet};
    use terminal_psi::{
        ClaimTransfer, CrashRouteBucket, StructuralAccess, StructuralMultiplicity,
        StructuralOperationResult,
    };

    fn call_shape_spine(
        node: &OptimizationNode,
        psi_operation: semantic_vocabulary::OperationId,
        arguments: &[ValueId],
    ) -> bool {
        node.provenance.first() == Some(&PsiProvenance::Operation(psi_operation))
            && node.uses.len() == arguments.len()
            && node
                .uses
                .iter()
                .zip(arguments.iter())
                .all(|(value_use, argument)| value_use.value == *argument)
            && node.successors.is_empty()
    }

    fn empty_claim_transfer_shape(
        node: &OptimizationNode,
        claim_transfers: &[ClaimTransfer],
    ) -> bool {
        claim_transfers.is_empty()
            && node.ownership.as_slice() == [OwnershipEvent::ClaimTransfer(Vec::new())]
    }

    fn scalar_call_shape(
        node: &OptimizationNode,
    ) -> Option<(MachineId, &[ValueId], &[CrashRouteBucket])> {
        let O::Call {
            psi_operation,
            result,
            scalar_type,
            callee,
            arguments,
            crash_continuations,
            ..
        } = &node.operation
        else {
            return None;
        };
        (call_shape_spine(node, *psi_operation, arguments)
            && node.definitions.len() == 1
            && node.definitions[0].value == *result
            && node.definitions[0].scalar_type == *scalar_type
            && node.ownership.is_empty())
        .then_some((
            *callee,
            arguments.as_slice(),
            crash_continuations.as_slice(),
        ))
    }

    pub(crate) fn admissible_invariant_scalar_call(node: &OptimizationNode) -> Option<MachineId> {
        let (callee, _, crash_continuations) = scalar_call_shape(node)?;
        crash_continuations.is_empty().then_some(callee)
    }

    pub(crate) fn admissible_invariant_crash_continuation_call(
        node: &OptimizationNode,
    ) -> Option<MachineId> {
        let (callee, _, crash_continuations) = scalar_call_shape(node)?;
        (!crash_continuations.is_empty()).then_some(callee)
    }

    fn unit_call_shape(
        node: &OptimizationNode,
    ) -> Option<(MachineId, &[ValueId], &[CrashRouteBucket])> {
        let O::CallStructural {
            psi_operation,
            callee: abstract_operations::AbstractCallTarget::Machine(callee),
            arguments,
            claim_transfers,
            crash_continuations,
            result: AbstractStructuralCallResult::Unit,
            ..
        } = &node.operation
        else {
            return None;
        };
        (call_shape_spine(node, *psi_operation, arguments)
            && node.definitions.is_empty()
            && empty_claim_transfer_shape(node, claim_transfers))
        .then_some((
            *callee,
            arguments.as_slice(),
            crash_continuations.as_slice(),
        ))
    }

    pub(crate) fn admissible_invariant_unit_call(node: &OptimizationNode) -> Option<MachineId> {
        let (callee, _, crash_continuations) = unit_call_shape(node)?;
        crash_continuations.is_empty().then_some(callee)
    }

    pub(crate) fn admissible_invariant_crash_continuation_unit_call(
        node: &OptimizationNode,
    ) -> Option<MachineId> {
        let (callee, _, crash_continuations) = unit_call_shape(node)?;
        (!crash_continuations.is_empty()).then_some(callee)
    }

    fn structural_scalar_call_shape(
        node: &OptimizationNode,
    ) -> Option<(MachineId, &[ValueId], &[CrashRouteBucket])> {
        let O::CallStructural {
            psi_operation,
            result: AbstractStructuralCallResult::Scalar(result),
            callee: abstract_operations::AbstractCallTarget::Machine(callee),
            arguments,
            claim_transfers,
            crash_continuations,
            ..
        } = &node.operation
        else {
            return None;
        };
        (call_shape_spine(node, *psi_operation, arguments)
            && node.definitions.len() == 1
            && node.definitions[0].value == result.value
            && node.definitions[0].scalar_type == result.scalar_type
            && empty_claim_transfer_shape(node, claim_transfers))
        .then_some((
            *callee,
            arguments.as_slice(),
            crash_continuations.as_slice(),
        ))
    }

    pub(crate) fn admissible_invariant_structural_scalar_call(
        node: &OptimizationNode,
    ) -> Option<MachineId> {
        let (callee, _, crash_continuations) = structural_scalar_call_shape(node)?;
        crash_continuations.is_empty().then_some(callee)
    }

    pub(crate) fn admissible_invariant_crash_continuation_structural_scalar_call(
        node: &OptimizationNode,
    ) -> Option<MachineId> {
        let (callee, _, crash_continuations) = structural_scalar_call_shape(node)?;
        (!crash_continuations.is_empty()).then_some(callee)
    }

    fn structural_call_shape(
        node: &OptimizationNode,
    ) -> Option<(MachineId, StructuralOperationResult, &[CrashRouteBucket])> {
        let O::CallStructural {
            psi_operation,
            result:
                AbstractStructuralCallResult::Structural {
                    result,
                    selected_evidence,
                },
            callee: abstract_operations::AbstractCallTarget::Machine(callee),
            arguments,
            claim_transfers,
            crash_continuations,
            ..
        } = &node.operation
        else {
            return None;
        };
        (call_shape_spine(node, *psi_operation, arguments)
            && node.definitions.is_empty()
            && matches!(
                result.multiplicity,
                StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
            )
            && result.qualifications.is_empty()
            && result.projected_qualifications.is_empty()
            && result.claims.is_empty()
            && selected_evidence.is_empty()
            && empty_claim_transfer_shape(node, claim_transfers))
        .then(|| (*callee, result.clone(), crash_continuations.as_slice()))
    }

    pub(crate) fn admissible_invariant_structural_call(
        node: &OptimizationNode,
    ) -> Option<(MachineId, StructuralOperationResult)> {
        let (callee, result, crash_continuations) = structural_call_shape(node)?;
        crash_continuations.is_empty().then_some((callee, result))
    }

    pub(crate) fn admissible_invariant_crash_continuation_structural_call(
        node: &OptimizationNode,
    ) -> Option<(MachineId, StructuralOperationResult)> {
        let (callee, result, crash_continuations) = structural_call_shape(node)?;
        (!crash_continuations.is_empty()).then_some((callee, result))
    }

    pub(crate) type CallAdmission = Option<(BTreeMap<ValueId, ValueId>, Vec<(PlaceId, PlaceId)>)>;

    pub(crate) struct StructuralCallRun<'a> {
        pub(crate) functions: &'a [PsiOptimizationFunction],
        pub(crate) function: &'a PsiOptimizationFunction,
        pub(crate) component: &'a OptimizerCycleComponent,
        pub(crate) node: &'a OptimizationNode,
        pub(crate) relocating: &'a BTreeSet<ValueId>,
        pub(crate) relocating_roots: &'a BTreeSet<PlaceId>,
        pub(crate) effects: &'a crate::EffectSummaryAnalysis,
    }

    pub(crate) fn invariant_unit_call_admission(run: &StructuralCallRun<'_>) -> CallAdmission {
        borrow_call_admission(
            run,
            admissible_invariant_unit_call(run.node)?,
            run.relocating_roots,
            |result| matches!(result, AbstractStructuralCallResult::Unit),
        )
    }

    pub(crate) fn invariant_structural_scalar_call_admission(
        run: &StructuralCallRun<'_>,
    ) -> CallAdmission {
        borrow_call_admission(
            run,
            admissible_invariant_structural_scalar_call(run.node)?,
            run.relocating_roots,
            |result| matches!(result, AbstractStructuralCallResult::Scalar(_)),
        )
    }

    pub(crate) fn invariant_structural_call_admission(
        run: &StructuralCallRun<'_>,
    ) -> CallAdmission {
        let (callee, result) = admissible_invariant_structural_call(run.node)?;
        let tolerated = structural_call_result_tolerated(run, &result)?;
        borrow_call_admission(run, callee, &tolerated, |result| {
            matches!(result, AbstractStructuralCallResult::Structural { .. })
        })
    }

    fn structural_call_result_tolerated(
        run: &StructuralCallRun<'_>,
        result: &StructuralOperationResult,
    ) -> Option<BTreeSet<PlaceId>> {
        let mut tolerated = run.relocating_roots.clone();
        if result.multiplicity == StructuralMultiplicity::Affine {
            if !scalar_case_result_contained(run.function, run.component, result.place) {
                return None;
            }
            tolerated.insert(result.place);
        }
        Some(tolerated)
    }

    pub(crate) fn invariant_crash_continuation_unit_call_admission(
        run: &StructuralCallRun<'_>,
    ) -> CallAdmission {
        crash_continuation_borrow_call_admission(
            run,
            admissible_invariant_crash_continuation_unit_call(run.node)?,
            run.relocating_roots,
            |result| matches!(result, AbstractStructuralCallResult::Unit),
        )
    }

    pub(crate) fn invariant_crash_continuation_structural_scalar_call_admission(
        run: &StructuralCallRun<'_>,
    ) -> CallAdmission {
        crash_continuation_borrow_call_admission(
            run,
            admissible_invariant_crash_continuation_structural_scalar_call(run.node)?,
            run.relocating_roots,
            |result| matches!(result, AbstractStructuralCallResult::Scalar(_)),
        )
    }

    pub(crate) fn invariant_crash_continuation_structural_call_admission(
        run: &StructuralCallRun<'_>,
    ) -> CallAdmission {
        let (callee, result) = admissible_invariant_crash_continuation_structural_call(run.node)?;
        let tolerated = structural_call_result_tolerated(run, &result)?;
        crash_continuation_borrow_call_admission(run, callee, &tolerated, |result| {
            matches!(result, AbstractStructuralCallResult::Structural { .. })
        })
    }

    fn crash_continuation_borrow_call_admission(
        run: &StructuralCallRun<'_>,
        callee: MachineId,
        tolerated: &BTreeSet<PlaceId>,
        expected: impl FnOnce(&AbstractStructuralCallResult) -> bool,
    ) -> CallAdmission {
        let O::CallStructural {
            arguments,
            structural_arguments,
            crash_continuations,
            result,
            ..
        } = &run.node.operation
        else {
            return None;
        };
        if !expected(result) {
            return None;
        }
        let callee_function = run
            .functions
            .iter()
            .find(|candidate| candidate.machine == callee)?;
        if structural_call_crash_continuations(callee_function, arguments)? != *crash_continuations
        {
            return None;
        }
        let (substitution, rewrites) =
            borrow_call_evidence(run, callee, structural_arguments, tolerated)?;
        let moved_arguments = arguments
            .iter()
            .map(|argument| substitution.get(argument).copied().unwrap_or(*argument))
            .collect::<Vec<_>>();
        let continuations = structural_call_crash_continuations(callee_function, &moved_arguments)?;
        let caller_contract = run.function.verified_contract.as_ref()?;
        crash_routes_cover(&caller_contract.crash_routes, &continuations)
            .then_some((substitution, rewrites))
    }

    fn borrow_call_admission(
        run: &StructuralCallRun<'_>,
        callee: MachineId,
        tolerated: &BTreeSet<PlaceId>,
        expected: impl FnOnce(&AbstractStructuralCallResult) -> bool,
    ) -> CallAdmission {
        let O::CallStructural {
            structural_arguments,
            result,
            ..
        } = &run.node.operation
        else {
            return None;
        };
        if !expected(result) {
            return None;
        }
        borrow_call_evidence(run, callee, structural_arguments, tolerated)
    }

    fn borrow_call_evidence(
        run: &StructuralCallRun<'_>,
        callee: MachineId,
        structural_arguments: &[terminal_psi::StructuralArgument],
        tolerated: &BTreeSet<PlaceId>,
    ) -> CallAdmission {
        if !scalar_call_callee_pure(run.effects, callee) {
            return None;
        }
        if !component_members_unobservable(run.function, run.component, run.effects) {
            return None;
        }
        if !component_preserves_place_observations_tolerating(
            run.function,
            run.component,
            tolerated,
        ) {
            return None;
        }
        let substitution = member_scalar_operand_substitution(
            run.function,
            run.component,
            run.node,
            run.relocating,
        )?;
        let preheader_source = shared_entry_source(run.component)?;
        let preheader = run.function.block(preheader_source)?;
        let representatives =
            invariant_member_place_parameters(run.function, run.component, run.relocating_roots);
        let mut rewrites = Vec::new();
        for argument in structural_arguments {
            match argument.access {
                StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow => {
                    if !(run.relocating_roots.contains(&argument.place)
                        && member_root_producer_count(run.function, run.component, argument.place)
                            == 1)
                    {
                        return None;
                    }
                }
                StructuralAccess::SharedBorrow => {
                    let resolved = call_argument_root_landing(
                        run.function,
                        run.component,
                        preheader,
                        &representatives,
                        run.relocating_roots,
                        argument.place,
                    )?;
                    if resolved != argument.place {
                        rewrites.push((argument.place, resolved));
                    }
                }
                StructuralAccess::Owned => {
                    if !argument.path.is_empty() {
                        return None;
                    }
                    let resolved = call_argument_root_landing(
                        run.function,
                        run.component,
                        preheader,
                        &representatives,
                        run.relocating_roots,
                        argument.place,
                    )?;
                    if copyable_owned_argument_root(run.function, resolved) {
                        if resolved != argument.place {
                            rewrites.push((argument.place, resolved));
                        }
                    } else if !(resolved == argument.place
                        && consumed_owned_argument_root(run.function, run.component, resolved))
                    {
                        return None;
                    }
                }
            }
        }
        Some((substitution, rewrites))
    }

    fn call_argument_root_landing(
        function: &PsiOptimizationFunction,
        component: &OptimizerCycleComponent,
        preheader: &OptimizationBlock,
        representatives: &BTreeMap<PlaceId, PlaceId>,
        relocating_roots: &BTreeSet<PlaceId>,
        place: PlaceId,
    ) -> Option<PlaceId> {
        if place_observation_root_visible(function, preheader, place)
            || (relocating_roots.contains(&place)
                && member_root_producer_count(function, component, place) == 1)
        {
            return Some(place);
        }
        let representative = *representatives.get(&place)?;
        (place_observation_root_visible(function, preheader, representative)
            || (relocating_roots.contains(&representative)
                && member_root_producer_count(function, component, representative) == 1))
            .then_some(representative)
    }

    pub(crate) fn invariant_scalar_call_admission(
        function: &PsiOptimizationFunction,
        component: &OptimizerCycleComponent,
        node: &OptimizationNode,
        relocating: &BTreeSet<ValueId>,
        effects: &crate::EffectSummaryAnalysis,
    ) -> Option<BTreeMap<ValueId, ValueId>> {
        let callee = admissible_invariant_scalar_call(node)?;
        if !scalar_call_callee_pure(effects, callee) {
            return None;
        }
        if !component_members_unobservable(function, component, effects) {
            return None;
        }
        member_scalar_operand_substitution(function, component, node, relocating)
    }

    pub(crate) fn invariant_crash_continuation_call_admission(
        functions: &[PsiOptimizationFunction],
        function: &PsiOptimizationFunction,
        component: &OptimizerCycleComponent,
        node: &OptimizationNode,
        relocating: &BTreeSet<ValueId>,
        effects: &crate::EffectSummaryAnalysis,
    ) -> Option<BTreeMap<ValueId, ValueId>> {
        let callee = admissible_invariant_crash_continuation_call(node)?;
        if !scalar_call_callee_pure(effects, callee) {
            return None;
        }
        if !component_members_unobservable(function, component, effects) {
            return None;
        }
        let O::Call {
            arguments,
            crash_continuations,
            ..
        } = &node.operation
        else {
            return None;
        };
        let callee_function = functions
            .iter()
            .find(|candidate| candidate.machine == callee)?;
        if call_crash_continuations(callee_function, arguments)? != *crash_continuations {
            return None;
        }
        let substitution =
            member_scalar_operand_substitution(function, component, node, relocating)?;
        let moved_arguments = arguments
            .iter()
            .map(|argument| substitution.get(argument).copied().unwrap_or(*argument))
            .collect::<Vec<_>>();
        let continuations = call_crash_continuations(callee_function, &moved_arguments)?;
        let caller_contract = function.verified_contract.as_ref()?;
        crash_routes_cover(&caller_contract.crash_routes, &continuations).then_some(substitution)
    }

    fn crash_routes_cover(
        published: &[CrashRouteBucket],
        continuations: &[CrashRouteBucket],
    ) -> bool {
        continuations.iter().all(|continuation| {
            published.iter().any(|route| {
                route.cause == continuation.cause
                    && (route.alternatives == [terminal_psi::CrashRouteGuard::Truth]
                        || continuation
                            .alternatives
                            .iter()
                            .all(|alternative| route.alternatives.contains(alternative)))
            })
        })
    }

    pub(crate) fn unit_effect_summaries(
        unit: &PsiOptimizationUnit,
    ) -> crate::EffectSummaryAnalysis {
        match crate::compute_analysis(unit, optimization_core::AnalysisKind::EffectSummaries) {
            Some(crate::AnalysisProduct::EffectSummaries(analysis)) => analysis,
            _ => crate::EffectSummaryAnalysis {
                nodes: Vec::new(),
                functions: Vec::new(),
            },
        }
    }

    fn scalar_call_callee_pure(effects: &crate::EffectSummaryAnalysis, callee: MachineId) -> bool {
        effects
            .functions
            .iter()
            .find(|summary| summary.machine == callee)
            .is_some_and(|summary| {
                summary.observable == crate::EffectKnowledge::No
                    && summary.crash == crate::EffectKnowledge::No
                    && summary.suspension == crate::EffectKnowledge::No
            })
    }

    fn member_call_target(operation: &O) -> Option<MachineId> {
        match operation {
            O::Call { callee, .. } => Some(*callee),
            O::CallStructural { callee, .. } => callee.machine(),
            _ => None,
        }
    }

    pub(crate) fn component_members_unobservable(
        function: &PsiOptimizationFunction,
        component: &OptimizerCycleComponent,
        effects: &crate::EffectSummaryAnalysis,
    ) -> bool {
        for member in &component.members {
            let Some(block) = function.block(*member) else {
                return false;
            };
            for (index, node) in block.nodes.iter().enumerate() {
                let index = match u32::try_from(index) {
                    Ok(index) => index,
                    Err(_) => return false,
                };
                match member_call_target(&node.operation) {
                    Some(callee) => {
                        if !scalar_call_callee_pure(effects, callee) {
                            return false;
                        }
                    }
                    None => {
                        let observable = effects
                            .nodes
                            .iter()
                            .find(|summary| {
                                summary.machine == function.machine
                                    && summary.block == *member
                                    && summary.node == index
                            })
                            .map(|summary| summary.observable);
                        if observable != Some(crate::EffectKnowledge::No) {
                            return false;
                        }
                    }
                }
            }
        }
        true
    }
}
pub(crate) mod invariant_operations {
    use super::member_blocks::{member_scalar_operand_substitution, shared_entry_source};
    use super::place_observations::{
        component_preserves_place_observations, invariant_member_place_parameters,
        member_root_producer_count, place_observation_root_visible,
    };
    use super::relocation_rewrites::scalar_case_result_contained;
    pub(crate) use abstract_operations::AbstractOperation as O;
    use optimization_unit::{
        OptimizationNode, OptimizerCycleComponent, PsiOptimizationFunction, PsiProvenance,
    };
    use semantic_vocabulary::{OperationId, PlaceId, ValueId};
    pub(crate) use std::collections::{BTreeMap, BTreeSet};
    use terminal_psi::RecordFieldValue;

    fn operation_shape_spine(node: &OptimizationNode, psi_operation: OperationId) -> bool {
        node.provenance.first() == Some(&PsiProvenance::Operation(psi_operation))
            && node.successors.is_empty()
            && node.ownership.is_empty()
    }

    fn declaration_spine(
        node: &OptimizationNode,
        psi_operation: OperationId,
        result: Option<&terminal_psi::StructuralOperationResult>,
    ) -> bool {
        operation_shape_spine(node, psi_operation)
            && node.definitions.is_empty()
            && result.is_none_or(|result| {
                result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && result.claims.is_empty()
            })
    }

    fn uses_index_length(node: &OptimizationNode, index: ValueId, length: ValueId) -> bool {
        node.uses.len() == 2 && node.uses[0].value == index && node.uses[1].value == length
    }

    pub(crate) fn admissible_scalar_leaf_relocation(node: &OptimizationNode) -> bool {
        let psi_operation = match &node.operation {
            abstract_operations::constant_value_triple_pattern!(psi_operation, ..) => {
                *psi_operation
            }
            _ => return false,
        };
        operation_shape_spine(node, psi_operation) && node.uses.is_empty()
    }

    pub(crate) fn admissible_invariant_scalar_computation(node: &OptimizationNode) -> bool {
        let psi_operation = match &node.operation {
            abstract_operations::boolean_not_or_equal_pair_pattern!(psi_operation, ..)
            | abstract_operations::integer_equal_less_pair_pattern!(psi_operation, ..)
            | O::IntegerLessOrEqual { psi_operation, .. }
            | abstract_operations::bitwise_not_and_pair_pattern!(psi_operation, ..)
            | abstract_operations::bitwise_or_xor_pair_pattern!(psi_operation, ..)
            | abstract_operations::integer_widen_or_exact_cast_pattern!(psi_operation, ..)
            | O::ScalarRepresentationView { psi_operation, .. }
            | O::IntegerShift { psi_operation, .. }
            | O::IntegerArithmetic { psi_operation, .. }
            | O::IeeeFloatCompare { psi_operation, .. }
            | O::NearestIeeeFloatFusedMultiplyAdd { psi_operation, .. } => *psi_operation,
            _ => return false,
        };
        operation_shape_spine(node, psi_operation)
            && !node.uses.is_empty()
            && node.definitions.len() == 1
    }

    pub(crate) fn admissible_invariant_place_read(node: &OptimizationNode) -> Option<PlaceId> {
        let (psi_operation, source) = match &node.operation {
            O::PrimitiveScalarRead {
                psi_operation,
                source,
                ..
            }
            | O::StructuralCaseMembership {
                psi_operation,
                source,
                ..
            }
            | O::StructuralByteSequenceFieldLength {
                psi_operation,
                source,
                ..
            }
            | O::StructuralScalarFieldRead {
                psi_operation,
                source,
                ..
            } => (*psi_operation, *source),
            O::ByteSequenceLength {
                psi_operation,
                source,
                ..
            }
            | O::ElementViewLength {
                psi_operation,
                source,
                ..
            } => (*psi_operation, source.place),
            _ => return None,
        };
        (operation_shape_spine(node, psi_operation)
            && node.definitions.len() == 1
            && node.uses.is_empty())
        .then_some(source)
    }

    pub(crate) fn admissible_invariant_byte_read(
        node: &OptimizationNode,
    ) -> Option<(PlaceId, ValueId, ValueId)> {
        let O::ByteSequenceRead {
            psi_operation,
            source,
            index,
            length,
            ..
        } = &node.operation
        else {
            return None;
        };
        (operation_shape_spine(node, *psi_operation)
            && node.definitions.len() == 1
            && uses_index_length(node, *index, *length))
        .then_some((*source, *index, *length))
    }

    pub(crate) fn admissible_invariant_field_byte_read(
        node: &OptimizationNode,
    ) -> Option<(PlaceId, ValueId, ValueId)> {
        let O::StructuralByteSequenceFieldRead {
            psi_operation,
            source,
            index,
            length,
            ..
        } = &node.operation
        else {
            return None;
        };
        (operation_shape_spine(node, *psi_operation)
            && node.definitions.len() == 1
            && uses_index_length(node, *index, *length))
        .then_some((*source, *index, *length))
    }

    pub(crate) fn admissible_invariant_subslice(
        node: &OptimizationNode,
    ) -> Option<(PlaceId, ValueId, ValueId, ValueId)> {
        let (psi_operation, result, source, start, end, length) = match &node.operation {
            O::ByteSequenceSubslice {
                psi_operation,
                result,
                source,
                start,
                end,
                length,
                ..
            }
            | O::ElementViewSubslice {
                psi_operation,
                result,
                source,
                start,
                end,
                length,
                ..
            } => (psi_operation, result, source, start, end, length),
            _ => return None,
        };
        (declaration_spine(node, *psi_operation, Some(result))
            && node.uses.len() == 3
            && node.uses[0].value == *start
            && node.uses[1].value == *end
            && node.uses[2].value == *length)
            .then_some((source.place, *start, *end, *length))
    }

    pub(crate) fn admissible_invariant_byte_literal(node: &OptimizationNode) -> bool {
        let O::EstablishByteSequenceLiteral { psi_operation, .. } = &node.operation else {
            return false;
        };
        declaration_spine(node, *psi_operation, None) && node.uses.is_empty()
    }

    pub(crate) fn admissible_invariant_primitive_local(
        node: &OptimizationNode,
    ) -> Option<terminal_psi::StructuralOperationResult> {
        let O::EstablishPrimitiveLocal {
            psi_operation,
            result,
            value,
            ..
        } = &node.operation
        else {
            return None;
        };
        (declaration_spine(node, *psi_operation, Some(result))
            && node.uses.len() == 1
            && node.uses[0].value == value.value
            && result.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted)
            .then(|| result.clone())
    }

    pub(crate) fn invariant_primitive_local_admission(
        function: &PsiOptimizationFunction,
        component: &OptimizerCycleComponent,
        node: &OptimizationNode,
        relocating: &BTreeSet<ValueId>,
    ) -> Option<BTreeMap<ValueId, ValueId>> {
        admissible_invariant_primitive_local(node)?;
        if !component_preserves_place_observations(function, component) {
            return None;
        }
        member_scalar_operand_substitution(function, component, node, relocating)
    }

    pub(crate) fn admissible_invariant_record(
        node: &OptimizationNode,
    ) -> Option<terminal_psi::StructuralOperationResult> {
        let O::EstablishRecord {
            psi_operation,
            result,
            fields,
        } = &node.operation
        else {
            return None;
        };
        (declaration_spine(node, *psi_operation, Some(result))
            && node.uses.len()
                == fields
                    .iter()
                    .filter(|field| matches!(field.value, RecordFieldValue::Scalar { .. }))
                    .count()
            && node
                .uses
                .iter()
                .zip(fields.iter().filter_map(|field| match &field.value {
                    RecordFieldValue::Scalar { value, .. } => Some(value),
                    RecordFieldValue::Structural(_) => None,
                }))
                .all(|(value_use, value)| value_use.value == *value)
            && fields.iter().all(|field| match &field.value {
                RecordFieldValue::Scalar { .. } => true,
                RecordFieldValue::Structural(argument) => {
                    argument.access == terminal_psi::StructuralAccess::Owned
                        && argument.path.is_empty()
                }
            })
            && (result.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                || (result.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                    && fields
                        .iter()
                        .all(|field| matches!(field.value, RecordFieldValue::Scalar { .. })))))
        .then(|| result.clone())
    }

    pub(crate) fn invariant_record_admission(
        function: &PsiOptimizationFunction,
        component: &OptimizerCycleComponent,
        node: &OptimizationNode,
        relocating: &BTreeSet<ValueId>,
        relocating_roots: &BTreeSet<PlaceId>,
    ) -> Option<(BTreeMap<ValueId, ValueId>, Vec<(PlaceId, PlaceId)>)> {
        let result = admissible_invariant_record(node)?;
        if result.multiplicity == terminal_psi::StructuralMultiplicity::Affine {
            if !scalar_case_result_contained(function, component, result.place) {
                return None;
            }
        } else if !component_preserves_place_observations(function, component) {
            return None;
        }
        let substitution =
            member_scalar_operand_substitution(function, component, node, relocating)?;
        let O::EstablishRecord { fields, .. } = &node.operation else {
            return None;
        };
        let preheader_source = shared_entry_source(component)?;
        let preheader = function.block(preheader_source)?;
        let representatives =
            invariant_member_place_parameters(function, component, relocating_roots);
        let mut rewrites = Vec::new();
        for field in fields {
            let RecordFieldValue::Structural(argument) = &field.value else {
                continue;
            };
            let resolved = if place_observation_root_visible(function, preheader, argument.place)
                || (relocating_roots.contains(&argument.place)
                    && member_root_producer_count(function, component, argument.place) == 1)
            {
                argument.place
            } else {
                let representative = *representatives.get(&argument.place)?;
                (place_observation_root_visible(function, preheader, representative)
                    || (relocating_roots.contains(&representative)
                        && member_root_producer_count(function, component, representative) == 1))
                    .then_some(representative)?
            };
            if resolved != argument.place {
                rewrites.push((argument.place, resolved));
            }
        }
        Some((substitution, rewrites))
    }

    pub(crate) fn admissible_invariant_scalar_array(
        node: &OptimizationNode,
    ) -> Option<terminal_psi::StructuralOperationResult> {
        let O::EstablishScalarArray {
            psi_operation,
            result,
            elements,
        } = &node.operation
        else {
            return None;
        };
        (declaration_spine(node, *psi_operation, Some(result))
            && node.uses.len() == elements.len()
            && node
                .uses
                .iter()
                .zip(elements.iter())
                .all(|(value_use, element)| value_use.value == *element)
            && result.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted)
            .then(|| result.clone())
    }

    pub(crate) fn invariant_scalar_array_admission(
        function: &PsiOptimizationFunction,
        component: &OptimizerCycleComponent,
        node: &OptimizationNode,
        relocating: &BTreeSet<ValueId>,
    ) -> Option<BTreeMap<ValueId, ValueId>> {
        admissible_invariant_scalar_array(node)?;
        if !component_preserves_place_observations(function, component) {
            return None;
        }
        member_scalar_operand_substitution(function, component, node, relocating)
    }

    pub(crate) fn admissible_invariant_scalar_case(
        node: &OptimizationNode,
    ) -> Option<terminal_psi::StructuralOperationResult> {
        let O::EstablishCase {
            psi_operation,
            result,
            fields,
            ..
        } = &node.operation
        else {
            return None;
        };
        (declaration_spine(node, *psi_operation, Some(result))
            && node.uses.len() == fields.len()
            && node
                .uses
                .iter()
                .zip(fields.iter())
                .all(|(value_use, field)| matches!(field.value, RecordFieldValue::Scalar { value, .. } if value_use.value == value))
            && matches!(
                result.multiplicity,
                terminal_psi::StructuralMultiplicity::Unrestricted
                    | terminal_psi::StructuralMultiplicity::Affine
            ))
        .then(|| result.clone())
    }

    pub(crate) fn invariant_scalar_case_admission(
        function: &PsiOptimizationFunction,
        component: &OptimizerCycleComponent,
        node: &OptimizationNode,
        relocating: &BTreeSet<ValueId>,
    ) -> Option<BTreeMap<ValueId, ValueId>> {
        let result = admissible_invariant_scalar_case(node)?;
        if result.multiplicity == terminal_psi::StructuralMultiplicity::Affine {
            if !scalar_case_result_contained(function, component, result.place) {
                return None;
            }
        } else if !component_preserves_place_observations(function, component) {
            return None;
        }
        member_scalar_operand_substitution(function, component, node, relocating)
    }

    pub(crate) fn admissible_invariant_trivial_affine_local(
        node: &OptimizationNode,
    ) -> Option<terminal_psi::StructuralPlaceDeclaration> {
        let O::EstablishTrivialAffineLocal {
            psi_operation,
            place,
            ..
        } = &node.operation
        else {
            return None;
        };
        (declaration_spine(node, *psi_operation, None) && node.uses.is_empty()).then_some(*place)
    }

    pub(crate) fn invariant_trivial_affine_local_admission(
        function: &PsiOptimizationFunction,
        component: &OptimizerCycleComponent,
        node: &OptimizationNode,
    ) -> Option<PlaceId> {
        let place = admissible_invariant_trivial_affine_local(node)?;
        scalar_case_result_contained(function, component, place.id).then_some(place.id)
    }
}
pub(crate) mod member_blocks {
    use super::invariant_operations::admissible_invariant_scalar_computation;
    use optimization_unit::{
        OptimizationEdge, OptimizationNode, OptimizerCycleComponent, PsiOptimizationFunction,
        ValueDefinitionSite,
    };
    use semantic_vocabulary::{BlockId, ValueId};
    pub(crate) use std::collections::{BTreeMap, BTreeSet};

    pub(crate) fn value_definition_sites(
        function: &PsiOptimizationFunction,
    ) -> BTreeMap<ValueId, ValueDefinitionSite> {
        function
            .parameters
            .iter()
            .chain(function.blocks.iter().flat_map(|block| {
                block
                    .parameters
                    .iter()
                    .chain(block.nodes.iter().flat_map(|node| node.definitions.iter()))
            }))
            .map(|definition| (definition.value, definition.site))
            .collect()
    }

    pub(crate) fn shared_entry_source(component: &OptimizerCycleComponent) -> Option<BlockId> {
        let [first, rest @ ..] = component.entries.as_slice() else {
            return None;
        };
        (rest.iter().all(|entry| entry.source == first.source)
            && !component.members.contains(&first.source))
        .then_some(first.source)
    }

    pub(crate) fn guaranteed_executed_member_blocks(
        component: &OptimizerCycleComponent,
    ) -> BTreeSet<BlockId> {
        let members: BTreeSet<BlockId> = component.members.iter().copied().collect();
        let entry_targets: BTreeSet<BlockId> = component
            .entries
            .iter()
            .map(|entry| entry.target)
            .filter(|target| members.contains(target))
            .collect();
        if entry_targets.is_empty() {
            return BTreeSet::new();
        }
        let boundary: BTreeSet<BlockId> = component
            .exits
            .iter()
            .map(|exit| exit.source)
            .filter(|source| members.contains(source))
            .collect();
        let mut predecessors: BTreeMap<BlockId, BTreeSet<BlockId>> = members
            .iter()
            .map(|member| (*member, BTreeSet::new()))
            .collect();
        for edge in &component.id.internal_edges {
            if members.contains(&edge.source) && members.contains(&edge.target) {
                predecessors
                    .get_mut(&edge.target)
                    .expect("member target has a predecessor row")
                    .insert(edge.source);
            }
        }
        let mut dominators: BTreeMap<BlockId, BTreeSet<BlockId>> = members
            .iter()
            .map(|member| {
                (
                    *member,
                    if entry_targets.contains(member) {
                        BTreeSet::from([*member])
                    } else {
                        members.clone()
                    },
                )
            })
            .collect();
        loop {
            let mut changed = false;
            for member in members
                .iter()
                .copied()
                .filter(|member| !entry_targets.contains(member))
            {
                let mut incoming = predecessors[&member]
                    .iter()
                    .filter_map(|predecessor| dominators.get(predecessor));
                let mut next = incoming.next().cloned().unwrap_or_default();
                for set in incoming {
                    next = next.intersection(set).copied().collect();
                }
                next.insert(member);
                if dominators[&member] != next {
                    dominators.insert(member, next);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        members
            .iter()
            .copied()
            .filter(|member| {
                boundary
                    .iter()
                    .all(|source| dominators[source].contains(member))
            })
            .collect()
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(crate) enum MemberParameterResolution<K> {
        Unresolved,
        Representative(K),
        LoopCarried,
    }

    pub(crate) fn resolve_member_parameters<K: Copy + Ord>(
        function: &PsiOptimizationFunction,
        parameters: &[(K, BlockId)],
        contribution: impl Fn(
            &OptimizationEdge,
            K,
            &BTreeMap<K, MemberParameterResolution<K>>,
        ) -> MemberParameterResolution<K>,
    ) -> BTreeMap<K, K> {
        let mut resolutions = parameters
            .iter()
            .map(|(parameter, _)| (*parameter, MemberParameterResolution::Unresolved))
            .collect::<BTreeMap<_, _>>();
        loop {
            let mut progressed = false;
            for (parameter, block) in parameters {
                if resolutions[parameter] == MemberParameterResolution::LoopCarried {
                    continue;
                }
                let mut anchor = None;
                let mut loop_carried = false;
                for edge in function.block_edges().filter(|edge| edge.target == *block) {
                    match contribution(edge, *parameter, &resolutions) {
                        MemberParameterResolution::Unresolved => {}
                        MemberParameterResolution::Representative(value) => match anchor {
                            None => anchor = Some(value),
                            Some(anchor) if anchor == value => {}
                            Some(_) => {
                                loop_carried = true;
                                break;
                            }
                        },
                        MemberParameterResolution::LoopCarried => {
                            loop_carried = true;
                            break;
                        }
                    }
                }
                let next = if loop_carried {
                    MemberParameterResolution::LoopCarried
                } else {
                    match anchor {
                        Some(value) => MemberParameterResolution::Representative(value),
                        None => MemberParameterResolution::Unresolved,
                    }
                };
                if resolutions[parameter] != next {
                    resolutions.insert(*parameter, next);
                    progressed = true;
                }
            }
            if !progressed {
                break;
            }
        }
        resolutions
            .into_iter()
            .filter_map(|(parameter, resolution)| match resolution {
                MemberParameterResolution::Representative(representative) => {
                    Some((parameter, representative))
                }
                MemberParameterResolution::Unresolved | MemberParameterResolution::LoopCarried => {
                    None
                }
            })
            .collect()
    }

    pub(crate) fn invariant_member_parameters(
        function: &PsiOptimizationFunction,
        component: &OptimizerCycleComponent,
    ) -> BTreeMap<ValueId, ValueId> {
        let members: BTreeSet<BlockId> = component.members.iter().copied().collect();
        let sites = value_definition_sites(function);
        let mut parameters = Vec::new();
        for member in &component.members {
            if let Some(block) = function.block(*member) {
                parameters.extend(
                    block
                        .parameters
                        .iter()
                        .map(|parameter| (parameter.value, block.id)),
                );
            }
        }
        resolve_member_parameters(function, &parameters, |edge, parameter, resolutions| {
            let Some(binding) = edge.binding(parameter) else {
                return MemberParameterResolution::LoopCarried;
            };
            if binding.argument == parameter {
                return MemberParameterResolution::Unresolved;
            }
            match sites.get(&binding.argument) {
                Some(ValueDefinitionSite::FunctionParameter(_)) => {
                    MemberParameterResolution::Representative(binding.argument)
                }
                Some(ValueDefinitionSite::BlockParameter { block: site, .. })
                    if members.contains(site) =>
                {
                    resolutions
                        .get(&binding.argument)
                        .copied()
                        .unwrap_or(MemberParameterResolution::LoopCarried)
                }
                Some(ValueDefinitionSite::BlockParameter { block: site, .. })
                | Some(ValueDefinitionSite::Node { block: site, .. })
                    if !members.contains(site) =>
                {
                    MemberParameterResolution::Representative(binding.argument)
                }
                _ => MemberParameterResolution::LoopCarried,
            }
        })
    }

    pub(crate) fn invariant_scalar_operand_substitution(
        function: &PsiOptimizationFunction,
        component: &OptimizerCycleComponent,
        node: &OptimizationNode,
        relocating: &BTreeSet<ValueId>,
    ) -> Option<BTreeMap<ValueId, ValueId>> {
        if !admissible_invariant_scalar_computation(node) {
            return None;
        }
        member_scalar_operand_substitution(function, component, node, relocating)
    }

    pub(crate) fn member_scalar_operand_substitution(
        function: &PsiOptimizationFunction,
        component: &OptimizerCycleComponent,
        node: &OptimizationNode,
        relocating: &BTreeSet<ValueId>,
    ) -> Option<BTreeMap<ValueId, ValueId>> {
        let members: BTreeSet<BlockId> = component.members.iter().copied().collect();
        let sites = value_definition_sites(function);
        let representatives = invariant_member_parameters(function, component);
        let mut substitution = BTreeMap::new();
        for value_use in &node.uses {
            let site = sites.get(&value_use.value)?;
            let inside = match site {
                ValueDefinitionSite::FunctionParameter(_) => false,
                ValueDefinitionSite::BlockParameter { block, .. }
                | ValueDefinitionSite::Node { block, .. } => members.contains(block),
            };
            if !inside {
                continue;
            }
            match site {
                ValueDefinitionSite::BlockParameter { block, .. } => {
                    if let Some(representative) = representatives.get(&value_use.value) {
                        substitution.insert(value_use.value, *representative);
                    } else {
                        let result = member_parameter_run_result(
                            function,
                            &members,
                            &sites,
                            value_use.value,
                            *block,
                            relocating,
                        )?;
                        substitution.insert(value_use.value, result);
                    }
                }
                ValueDefinitionSite::Node { .. } if relocating.contains(&value_use.value) => {}
                _ => return None,
            }
        }
        Some(substitution)
    }

    fn member_parameter_run_result(
        function: &PsiOptimizationFunction,
        members: &BTreeSet<BlockId>,
        sites: &BTreeMap<ValueId, ValueDefinitionSite>,
        parameter: ValueId,
        parameter_block: BlockId,
        relocating: &BTreeSet<ValueId>,
    ) -> Option<ValueId> {
        let mut result = None;
        let mut bound = false;
        for edge in function
            .block_edges()
            .filter(|edge| edge.target == parameter_block)
        {
            let binding = edge.binding(parameter)?;
            if binding.argument == parameter {
                continue;
            }
            let Some(ValueDefinitionSite::Node { block, .. }) = sites.get(&binding.argument) else {
                return None;
            };
            if !members.contains(block) || !relocating.contains(&binding.argument) {
                return None;
            }
            bound = true;
            match result {
                None => result = Some(binding.argument),
                Some(result) if result == binding.argument => {}
                Some(_) => return None,
            }
        }
        if bound { result } else { None }
    }
}
pub(crate) mod place_observations {
    use super::invariant_operations::{
        admissible_invariant_byte_read, admissible_invariant_field_byte_read,
        admissible_invariant_place_read, admissible_invariant_subslice,
    };
    use super::member_blocks::{
        MemberParameterResolution, member_scalar_operand_substitution, resolve_member_parameters,
        shared_entry_source, value_definition_sites,
    };
    use crate::validation::relocation_rewrites::scalar_case_result_consumed;
    use crate::validation::relocation_rewrites::scalar_case_result_contained;
    pub(crate) use abstract_operations::AbstractOperation as O;
    use abstract_operations::AbstractStructuralCallResult;
    use optimization_unit::{
        OptimizationBlock, OptimizationNode, OptimizerCycleComponent, OwnershipEvent,
        PsiOptimizationFunction, PsiProvenance, ValueDefinitionSite,
    };
    use semantic_vocabulary::{BlockId, EdgeId, PlaceId, StructuralPlaceKind, ValueId};
    pub(crate) use std::collections::{BTreeMap, BTreeSet};
    use terminal_psi::StructuralAccess;

    pub(crate) fn component_preserves_place_observations(
        function: &PsiOptimizationFunction,
        component: &OptimizerCycleComponent,
    ) -> bool {
        component_preserves_place_observations_tolerating(function, component, &BTreeSet::new())
    }

    pub(crate) fn component_preserves_place_observations_tolerating(
        function: &PsiOptimizationFunction,
        component: &OptimizerCycleComponent,
        tolerated: &BTreeSet<PlaceId>,
    ) -> bool {
        for member in &component.members {
            let Some(block) = function.block(*member) else {
                return false;
            };
            for node in &block.nodes {
                let custody_quiet = node.ownership.iter().all(
                    |event| matches!(event, OwnershipEvent::ClaimTransfer(claims) if claims.is_empty()),
                );
                if !custody_quiet
                    || !(node_preserves_place_observations(&node.operation)
                        || exclusive_borrow_call_preserves_place_observations(
                            function, component, node,
                        ))
                {
                    return false;
                }
            }
        }
        let members: BTreeSet<BlockId> = component.members.iter().copied().collect();
        let adjacent: BTreeSet<EdgeId> = component
            .id
            .internal_edges
            .iter()
            .chain(component.entries.iter())
            .chain(component.exits.iter())
            .map(|edge| edge.edge)
            .collect();
        function
            .blocks
            .iter()
            .flat_map(|block| {
                let member_departing = members.contains(&block.id);
                block.nodes.iter().flat_map(move |node| {
                    node.successors
                        .iter()
                        .map(move |edge| (member_departing, edge))
                })
            })
            .filter(|(_, edge)| adjacent.contains(&edge.psi_edge))
            .all(|(member_departing, edge)| {
                edge.cleanup_actions.iter().all(|action| {
                    matches!(action,
                    terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place)
                        if member_departing && tolerated.contains(place))
                })
            })
    }

    fn node_preserves_place_observations(operation: &O) -> bool {
        match operation {
            abstract_operations::constant_value_triple_pattern!(..)
            | abstract_operations::boolean_not_or_equal_pair_pattern!(..)
            | abstract_operations::integer_equal_less_pair_pattern!(..)
            | abstract_operations::bitwise_not_or_less_pair_pattern!(..)
            | abstract_operations::integer_widen_or_exact_cast_pattern!(..)
            | abstract_operations::bitwise_and_or_view_pair_pattern!(..)
            | abstract_operations::bitwise_or_xor_pair_pattern!(..)
            | O::IntegerShift { .. }
            | O::IntegerArithmetic { .. }
            | O::IeeeFloatCompare { .. }
            | O::NearestIeeeFloatFusedMultiplyAdd { .. }
            | O::PrimitiveScalarRead { .. }
            | O::StructuralCaseMembership { .. }
            | O::ByteSequenceRead { .. }
            | O::ByteSequenceSubslice { .. }
            | O::ElementViewSubslice { .. }
            | O::ByteSequenceLength { .. }
            | O::ElementViewLength { .. }
            | abstract_operations::byte_len_or_field_read_pattern!(..)
            | O::StructuralScalarFieldRead { .. }
            | abstract_operations::jump_or_conditional_pattern!()
            | O::StructuralCase { .. }
            | O::PortWrite { .. }
            | O::EstablishByteSequenceLiteral { .. }
            | O::EstablishPrimitiveLocal { .. }
            | O::EstablishScalarArray { .. }
            | O::EstablishStructuralArray { .. }
            | abstract_operations::establish_record_or_case_pattern!(..)
            | O::Call { .. } => true,
            O::CallStructural {
                structural_arguments,
                claim_transfers,
                result: AbstractStructuralCallResult::Unit | AbstractStructuralCallResult::Scalar(_),
                ..
            } => {
                claim_transfers.is_empty()
                    && structural_arguments
                        .iter()
                        .all(|argument| argument.access == StructuralAccess::SharedBorrow)
            }
            O::CallStructural {
                structural_arguments,
                claim_transfers,
                result: AbstractStructuralCallResult::Structural { result, .. },
                ..
            } => {
                claim_transfers.is_empty()
                    && result.claims.is_empty()
                    && structural_arguments
                        .iter()
                        .all(|argument| argument.access == StructuralAccess::SharedBorrow)
            }
            _ => false,
        }
    }

    fn exclusive_borrow_call_preserves_place_observations(
        function: &PsiOptimizationFunction,
        component: &OptimizerCycleComponent,
        node: &OptimizationNode,
    ) -> bool {
        let (structural_arguments, claim_transfers) = match &node.operation {
            O::CallStructural {
                structural_arguments,
                claim_transfers,
                result: AbstractStructuralCallResult::Unit | AbstractStructuralCallResult::Scalar(_),
                ..
            } => (structural_arguments, claim_transfers),
            O::CallStructural {
                structural_arguments,
                claim_transfers,
                result: AbstractStructuralCallResult::Structural { result, .. },
                ..
            } if result.claims.is_empty() => (structural_arguments, claim_transfers),
            _ => return false,
        };
        if !claim_transfers.is_empty() {
            return false;
        }
        let mut borrowed = BTreeSet::new();
        for argument in structural_arguments {
            match argument.access {
                StructuralAccess::SharedBorrow => {}
                StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow => {
                    borrowed.insert(argument.place);
                }
                StructuralAccess::Owned => {
                    if !argument.path.is_empty()
                        || !(copyable_owned_argument_root(function, argument.place)
                            || consumed_owned_argument_root(function, component, argument.place))
                    {
                        return false;
                    }
                }
            }
        }
        if structural_arguments
            .iter()
            .filter(|argument| borrowed.contains(&argument.place))
            .count()
            != borrowed.len()
        {
            return false;
        }
        borrowed_roots_exclusive(function, component, node, &borrowed)
    }

    fn borrowed_roots_exclusive(
        function: &PsiOptimizationFunction,
        component: &OptimizerCycleComponent,
        borrower: &OptimizationNode,
        roots: &BTreeSet<PlaceId>,
    ) -> bool {
        if roots.is_empty() {
            return true;
        }
        let Some(PsiProvenance::Operation(borrower_operation)) =
            borrower.provenance.first().copied()
        else {
            return false;
        };
        let members: BTreeSet<BlockId> = component.members.iter().copied().collect();
        let mut tainted: BTreeSet<PlaceId> = roots.clone();
        loop {
            let mut progressed = false;
            for edge in function
                .block_edges()
                .filter(|edge| members.contains(&edge.target))
            {
                for binding in &edge.structural_bindings {
                    if tainted.contains(&binding.argument.place)
                        && tainted.insert(binding.parameter)
                    {
                        progressed = true;
                    }
                }
            }
            if !progressed {
                break;
            }
        }
        for member in &component.members {
            let Some(block) = function.block(*member) else {
                return false;
            };
            for node in &block.nodes {
                if node.provenance.first() == Some(&PsiProvenance::Operation(borrower_operation)) {
                    continue;
                }
                let mut references = BTreeSet::new();
                if !member_place_references(&node.operation, &mut references) {
                    return false;
                }
                if let Some(produced) = produced_place_root(&node.operation) {
                    references.remove(&produced);
                }
                if references.iter().any(|place| tainted.contains(place)) {
                    return false;
                }
                for edge in &node.successors {
                    if edge
                        .cleanup_actions
                        .iter()
                        .any(|action| tainted.contains(&action.place()))
                    {
                        return false;
                    }
                }
            }
        }
        true
    }

    pub(crate) fn member_place_references(
        operation: &O,
        references: &mut BTreeSet<PlaceId>,
    ) -> bool {
        match operation {
            abstract_operations::primitive_storage_observation_pattern!(source, ..)
            | O::ByteSequenceRead { source, .. }
            | abstract_operations::byte_len_or_field_read_pattern!(source, ..)
            | O::StructuralScalarFieldRead { source, .. }
            | O::StructuralCase { source, .. } => {
                references.insert(*source);
            }
            O::ByteSequenceLength { source, .. }
            | O::ElementViewLength { source, .. }
            | O::ByteSequenceSubslice { source, .. }
            | O::ElementViewSubslice { source, .. } => {
                references.insert(source.place);
            }
            O::PrimitiveLocalStore { destination, .. }
            | abstract_operations::byte_sequence_write_pair_pattern!(destination, ..) => {
                references.insert(*destination);
            }
            abstract_operations::byte_sequence_store_pair_pattern!(destination, source, ..) => {
                references.insert(*destination);
                references.insert(*source);
            }
            O::WriteOnlyPrimitiveStore { destination, .. }
            | O::StructuralScalarFieldStore { destination, .. } => {
                references.insert(destination.place());
            }
            O::CallStructural {
                structural_arguments,
                ..
            } => {
                for argument in structural_arguments {
                    references.insert(argument.place);
                }
            }
            O::EstablishStructuralArray {
                result, elements, ..
            } => {
                references.insert(result.place);
                references.extend(elements.iter().map(|argument| argument.place));
            }
            O::EstablishScalarArray { result, .. } | O::EstablishPrimitiveLocal { result, .. } => {
                references.insert(result.place);
            }
            abstract_operations::establish_record_or_case_pattern!(result, fields, ..) => {
                references.insert(result.place);
                for field in fields {
                    if let terminal_psi::RecordFieldValue::Structural(argument) = &field.value {
                        references.insert(argument.place);
                    }
                }
            }
            abstract_operations::byte_literal_establish_pair_pattern!(place, ..) => {
                references.insert(place.id);
            }
            abstract_operations::constant_value_triple_pattern!(..)
            | abstract_operations::boolean_not_or_equal_pair_pattern!(..)
            | abstract_operations::integer_equal_less_pair_pattern!(..)
            | O::IntegerLessOrEqual { .. }
            | abstract_operations::bitwise_not_and_pair_pattern!(..)
            | abstract_operations::bitwise_or_xor_pair_pattern!(..)
            | abstract_operations::integer_widen_or_exact_cast_pattern!(..)
            | O::ScalarRepresentationView { .. }
            | O::IntegerShift { .. }
            | O::IntegerArithmetic { .. }
            | O::IeeeFloatCompare { .. }
            | O::NearestIeeeFloatFusedMultiplyAdd { .. }
            | O::Call { .. }
            | O::PortWrite { .. }
            | abstract_operations::jump_or_conditional_pattern!()
            | abstract_operations::return_or_return_unit_pattern!()
            | O::Crash { .. } => {}
            _ => return false,
        }
        true
    }

    pub(crate) fn place_observation_root_visible(
        function: &PsiOptimizationFunction,
        preheader: &OptimizationBlock,
        root: PlaceId,
    ) -> bool {
        function.structural_parameter(root).is_some()
            || function
                .result
                .structural()
                .is_some_and(|result| result.place == root)
            || function.structural_places.iter().any(|declaration| {
                declaration.id == root
                    && matches!(
                        declaration.kind,
                        StructuralPlaceKind::ProviderAttachment { .. }
                    )
            })
            || preheader.structural_parameter(root).is_some()
            || preheader
                .nodes
                .iter()
                .take(preheader.nodes.len().saturating_sub(1))
                .any(|node| produces_place_root(&node.operation, root))
    }

    fn produces_place_root(operation: &O, root: PlaceId) -> bool {
        produced_place_root(operation) == Some(root)
    }

    pub(crate) fn produced_place_root(operation: &O) -> Option<PlaceId> {
        match operation {
            O::EstablishPrimitiveLocal { result, .. }
            | O::ByteSequenceSubslice { result, .. }
            | O::ElementViewSubslice { result, .. }
            | O::EstablishScalarArray { result, .. }
            | O::EstablishStructuralArray { result, .. }
            | abstract_operations::establish_case_or_record_result_pattern!(result)
            | O::CallStructural {
                result: AbstractStructuralCallResult::Structural { result, .. },
                ..
            } => Some(result.place),
            O::BoundaryCall {
                result: abstract_operations::AbstractBoundaryResult::Structural(result),
                ..
            } => Some(result.place),
            abstract_operations::byte_literal_establish_pair_pattern!(place, ..) => Some(place.id),
            O::AtomicEvent {
                event: abstract_operations::AbstractAtomicEvent::CompareExchangeOnce { outcome, .. },
                ..
            } => Some(outcome.place),
            _ => None,
        }
    }

    pub(crate) fn mutable_borrow_roots(operation: &O) -> BTreeSet<PlaceId> {
        structural_argument_roots(operation, StructuralAccess::MutableBorrow)
    }

    pub(crate) fn owned_argument_roots(operation: &O) -> BTreeSet<PlaceId> {
        structural_argument_roots(operation, StructuralAccess::Owned)
    }

    pub(crate) fn member_root_producer_count(
        function: &PsiOptimizationFunction,
        component: &OptimizerCycleComponent,
        root: PlaceId,
    ) -> usize {
        component
            .members
            .iter()
            .filter_map(|member| function.block(*member))
            .flat_map(|block| block.nodes.iter())
            .filter(|node| produced_place_root(&node.operation) == Some(root))
            .count()
    }

    pub(crate) fn copyable_owned_argument_root(
        function: &PsiOptimizationFunction,
        root: PlaceId,
    ) -> bool {
        function
            .structural_parameters
            .iter()
            .chain(function.block_structural_parameters())
            .any(|parameter| {
                parameter.place == root
                    && parameter.access == StructuralAccess::Owned
                    && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                    && parameter.qualifications.is_empty()
                    && parameter.projected_qualifications.is_empty()
            })
            || function.block_nodes().any(|node| {
                let result = match &node.operation {
                    abstract_operations::establish_record_or_case_pattern!(result, ..)
                    | O::EstablishScalarArray { result, .. }
                    | O::EstablishStructuralArray { result, .. }
                    | O::CallStructural {
                        result: AbstractStructuralCallResult::Structural { result, .. },
                        ..
                    } => result,
                    _ => return false,
                };
                result.place == root
                    && result.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && result.claims.is_empty()
            })
    }

    pub(crate) fn consumed_owned_argument_root(
        function: &PsiOptimizationFunction,
        component: &OptimizerCycleComponent,
        root: PlaceId,
    ) -> bool {
        member_root_producer_count(function, component, root) == 1
            && crate::validation::relocation_rewrites::restricted_member_produced_root(
                function, component, root,
            )
            && scalar_case_result_consumed(function, component, root)
            && scalar_case_result_contained(function, component, root)
    }

    pub(crate) fn invariant_place_observation_admission(
        function: &PsiOptimizationFunction,
        component: &OptimizerCycleComponent,
        node: &OptimizationNode,
        relocating_roots: &BTreeSet<PlaceId>,
    ) -> Option<PlaceId> {
        let source = admissible_invariant_place_read(node)?;
        invariant_observation_root(function, component, source, relocating_roots)
    }

    pub(crate) fn invariant_observation_root(
        function: &PsiOptimizationFunction,
        component: &OptimizerCycleComponent,
        source: PlaceId,
        relocating_roots: &BTreeSet<PlaceId>,
    ) -> Option<PlaceId> {
        let preheader_source = shared_entry_source(component)?;
        if !component_preserves_place_observations(function, component) {
            return None;
        }
        let preheader = function.block(preheader_source)?;
        if place_observation_root_visible(function, preheader, source)
            || (relocating_roots.contains(&source)
                && member_root_producer_count(function, component, source) == 1)
        {
            return Some(source);
        }
        let representatives =
            invariant_member_place_parameters(function, component, relocating_roots);
        let representative = representatives.get(&source)?;
        (place_observation_root_visible(function, preheader, *representative)
            || (relocating_roots.contains(representative)
                && member_root_producer_count(function, component, *representative) == 1))
            .then_some(*representative)
    }

    pub(crate) fn invariant_byte_read_admission(
        function: &PsiOptimizationFunction,
        component: &OptimizerCycleComponent,
        node: &OptimizationNode,
        relocating: &BTreeSet<ValueId>,
        relocating_roots: &BTreeSet<PlaceId>,
    ) -> Option<(PlaceId, BTreeMap<ValueId, ValueId>)> {
        let (source, _, length) = admissible_invariant_byte_read(node)?;
        let root = invariant_observation_root(function, component, source, relocating_roots)?;
        let substitution =
            member_scalar_operand_substitution(function, component, node, relocating)?;
        let rebound_length = substitution.get(&length).copied().unwrap_or(length);
        byte_length_operand_measures_root(
            function,
            component,
            root,
            rebound_length,
            relocating_roots,
        )
        .then_some((root, substitution))
    }

    pub(crate) fn invariant_field_byte_read_admission(
        function: &PsiOptimizationFunction,
        component: &OptimizerCycleComponent,
        node: &OptimizationNode,
        relocating: &BTreeSet<ValueId>,
        relocating_roots: &BTreeSet<PlaceId>,
    ) -> Option<(PlaceId, BTreeMap<ValueId, ValueId>)> {
        let (source, _, length) = admissible_invariant_field_byte_read(node)?;
        let O::StructuralByteSequenceFieldRead { path, field, .. } = &node.operation else {
            return None;
        };
        let root = invariant_observation_root(function, component, source, relocating_roots)?;
        member_scalar_operand_substitution(function, component, node, relocating)
            .filter(|substitution| substitution.is_empty())?;
        field_byte_length_operand_measures_root(
            function,
            component,
            root,
            length,
            relocating_roots,
            path,
            *field,
        )
        .then_some((root, BTreeMap::new()))
    }

    pub(crate) fn invariant_subslice_admission(
        function: &PsiOptimizationFunction,
        component: &OptimizerCycleComponent,
        node: &OptimizationNode,
        relocating: &BTreeSet<ValueId>,
        relocating_roots: &BTreeSet<PlaceId>,
    ) -> Option<(PlaceId, BTreeMap<ValueId, ValueId>)> {
        let (source, _, _, length) = admissible_invariant_subslice(node)?;
        let root = invariant_observation_root(function, component, source, relocating_roots)?;
        let substitution =
            member_scalar_operand_substitution(function, component, node, relocating)?;
        let rebound_length = substitution.get(&length).copied().unwrap_or(length);
        descriptor_length_operand_matches(
            function,
            component,
            node,
            root,
            rebound_length,
            relocating_roots,
        )
        .then_some((root, substitution))
    }

    fn defining_node(
        function: &PsiOptimizationFunction,
        value: ValueId,
    ) -> Option<(BlockId, &OptimizationNode)> {
        let sites = value_definition_sites(function);
        let ValueDefinitionSite::Node { block, node } = sites.get(&value)? else {
            return None;
        };
        let block = function.block(*block)?;
        let defining = block.nodes.get(usize::try_from(*node).ok()?)?;
        Some((block.id, defining))
    }

    fn descriptor_length_operand_matches(
        function: &PsiOptimizationFunction,
        component: &OptimizerCycleComponent,
        node: &OptimizationNode,
        root: PlaceId,
        length: ValueId,
        relocating_roots: &BTreeSet<PlaceId>,
    ) -> bool {
        let (source, element_count) = match &node.operation {
            O::ByteSequenceSubslice { source, .. } => (source, false),
            O::ElementViewSubslice { source, .. } => (source, true),
            _ => return false,
        };
        let Some((block, producing)) = defining_node(function, length) else {
            return false;
        };
        let measured = match &producing.operation {
            O::ByteSequenceLength { source, .. } if !element_count => source,
            O::ElementViewLength { source, .. } if element_count => source,
            _ => return false,
        };
        let mut rebound = source.clone();
        rebound.place = root;
        let mut measured = measured.clone();
        if component.members.contains(&block) {
            let Some(place) =
                invariant_observation_root(function, component, measured.place, relocating_roots)
            else {
                return false;
            };
            measured.place = place;
        }
        terminal_semantics::same_descriptor_coordinate(&rebound, &measured)
    }

    fn byte_length_operand_measures_root(
        function: &PsiOptimizationFunction,
        component: &OptimizerCycleComponent,
        root: PlaceId,
        rebound_length: ValueId,
        relocating_roots: &BTreeSet<PlaceId>,
    ) -> bool {
        let members: BTreeSet<BlockId> = component.members.iter().copied().collect();
        let Some((block, producing)) = defining_node(function, rebound_length) else {
            return false;
        };
        match &producing.operation {
            O::ByteSequenceLength {
                source: measured, ..
            } if members.contains(&block) => {
                measured.path.is_empty()
                    && invariant_observation_root(
                        function,
                        component,
                        measured.place,
                        relocating_roots,
                    ) == Some(root)
            }
            O::ByteSequenceLength {
                source: measured, ..
            } => measured.path.is_empty() && measured.place == root,
            _ => false,
        }
    }

    fn field_byte_length_operand_measures_root(
        function: &PsiOptimizationFunction,
        component: &OptimizerCycleComponent,
        root: PlaceId,
        length: ValueId,
        relocating_roots: &BTreeSet<PlaceId>,
        path: &[terminal_psi::StructuralPathSegment],
        field: semantic_vocabulary::StructuralFieldId,
    ) -> bool {
        let members: BTreeSet<BlockId> = component.members.iter().copied().collect();
        let Some((block, producing)) = defining_node(function, length) else {
            return false;
        };
        match &producing.operation {
            O::StructuralByteSequenceFieldLength {
                source: measured,
                path: measured_path,
                field: measured_field,
                ..
            } if members.contains(&block) => {
                measured_path == path
                    && *measured_field == field
                    && invariant_observation_root(function, component, *measured, relocating_roots)
                        == Some(root)
            }
            O::StructuralByteSequenceFieldLength {
                source: measured,
                path: measured_path,
                field: measured_field,
                ..
            } => *measured == root && measured_path == path && *measured_field == field,
            _ => false,
        }
    }

    pub(crate) fn invariant_member_place_parameters(
        function: &PsiOptimizationFunction,
        component: &OptimizerCycleComponent,
        relocating_roots: &BTreeSet<PlaceId>,
    ) -> BTreeMap<PlaceId, PlaceId> {
        let member_blocks: Vec<&OptimizationBlock> = component
            .members
            .iter()
            .filter_map(|member| function.block(*member))
            .collect();
        let member_parameters: BTreeSet<PlaceId> = member_blocks
            .iter()
            .flat_map(|block| {
                block
                    .structural_parameters
                    .iter()
                    .map(|parameter| parameter.place)
            })
            .collect();
        let member_produced = |place: PlaceId| {
            member_blocks.iter().any(|block| {
                block
                    .nodes
                    .iter()
                    .any(|node| produces_place_root(&node.operation, place))
            })
        };
        let parameters: Vec<(PlaceId, BlockId)> = member_blocks
            .iter()
            .flat_map(|block| {
                block
                    .structural_parameters
                    .iter()
                    .map(|parameter| (parameter.place, block.id))
            })
            .collect();
        resolve_member_parameters(function, &parameters, |edge, parameter, resolutions| {
            let Some(binding) = edge.structural_binding(parameter) else {
                return MemberParameterResolution::LoopCarried;
            };
            let argument = &binding.argument;
            if argument.place == parameter && argument.path.is_empty() {
                return MemberParameterResolution::Unresolved;
            }
            if !argument.path.is_empty() {
                return MemberParameterResolution::LoopCarried;
            }
            if member_parameters.contains(&argument.place) {
                return resolutions
                    .get(&argument.place)
                    .copied()
                    .unwrap_or(MemberParameterResolution::LoopCarried);
            }
            if member_produced(argument.place) && !relocating_roots.contains(&argument.place) {
                return MemberParameterResolution::LoopCarried;
            }
            MemberParameterResolution::Representative(argument.place)
        })
    }

    fn structural_argument_roots(operation: &O, access: StructuralAccess) -> BTreeSet<PlaceId> {
        let structural_arguments = match operation {
            abstract_operations::call_structural_or_boundary_call_pattern!(
                structural_arguments
            ) => structural_arguments,
            _ => return BTreeSet::new(),
        };
        structural_arguments
            .iter()
            .filter(|argument| argument.access == access)
            .map(|argument| argument.place)
            .collect()
    }
}
pub(crate) mod prephysical_manifest {
    pub(crate) mod projection {
        use super::{
            BaselineDecisionLog, OptimizationIdentityBundle, OptimizationManifestStage,
            OptimizationPassManifestRecord, OptimizationSelections,
            OptimizationStructuralStatistics, OptimizationWorkBudget, OptimizationWorkUsage,
            PhysicalOptimizationDataStatus, PrePhysicalOptimizationManifest,
            PrePhysicalOptimizationManifestError, PrePhysicalOptimizationManifestIdentity,
            PsiOptimizationUnit, PsiTransformationLedger, ValidatedOptimizedAbstractPlanProjection,
            VerifiedPsiOptimizationInput,
        };
        #[allow(clippy::too_many_arguments)]
        pub(crate) fn expected_record(
            input: &VerifiedPsiOptimizationInput,
            final_unit: &PsiOptimizationUnit,
            selections: &OptimizationSelections,
            psi_selections: &OptimizationSelections,
            budget_per_pass: OptimizationWorkBudget,
            usage: OptimizationWorkUsage,
            decisions: &BaselineDecisionLog,
            pass_manifests: &[OptimizationPassManifestRecord],
            ledger: &PsiTransformationLedger,
            bundle: OptimizationIdentityBundle,
            projection: ValidatedOptimizedAbstractPlanProjection,
        ) -> Result<PrePhysicalOptimizationManifest, PrePhysicalOptimizationManifestError> {
            let initial = crate::build_verified_psi_optimization_unit(
                input.clone(),
                final_unit.work_schedule,
            )
            .map_err(|_| PrePhysicalOptimizationManifestError::InitialUnitProjection)?;
            Ok(PrePhysicalOptimizationManifest {
                identity: PrePhysicalOptimizationManifestIdentity::from_canonical_bytes(b"pending"),
                stage: OptimizationManifestStage::PrePhysicalAbstractPlan,
                physical_data: PhysicalOptimizationDataStatus::UnavailableBeforePhysicalRealization,
                psi: final_unit.psi,
                work_schedule: final_unit.work_schedule,
                initial_unit: initial.unit().identity,
                final_unit: final_unit.identity,
                projection: projection.identity(),
                selections: selections.clone(),
                psi_selections: psi_selections.clone(),
                budget_per_pass,
                usage,
                decision_log: decisions.clone(),
                pass_manifests: pass_manifests.to_vec(),
                transformation_ledger: ledger.clone(),
                identity_bundle: bundle,
                source_statistics: structural_statistics(initial.unit())?,
                optimized_statistics: structural_statistics(final_unit)?,
            })
        }

        fn structural_statistics(
            unit: &PsiOptimizationUnit,
        ) -> Result<OptimizationStructuralStatistics, PrePhysicalOptimizationManifestError>
        {
            let count = |value: usize| {
                u64::try_from(value)
                    .map_err(|_| PrePhysicalOptimizationManifestError::StructuralStatisticsOverflow)
            };
            Ok(OptimizationStructuralStatistics {
                functions: count(unit.functions.len())?,
                blocks: count(
                    unit.functions
                        .iter()
                        .map(|function| function.blocks.len())
                        .sum(),
                )?,
                nodes: count(unit.blocks().map(|block| block.nodes.len()).sum())?,
                scalar_definitions: count(
                    unit.functions
                        .iter()
                        .map(|function| {
                            function.parameters.len()
                                + function
                                    .blocks
                                    .iter()
                                    .map(|block| {
                                        block.parameters.len()
                                            + block
                                                .nodes
                                                .iter()
                                                .map(|node| node.definitions.len())
                                                .sum::<usize>()
                                    })
                                    .sum::<usize>()
                        })
                        .sum(),
                )?,

                scalar_uses: count(unit.block_nodes().map(|node| node.uses.len()).sum())?,
                optimization_facts: count(
                    unit.functions
                        .iter()
                        .map(|function| function.facts.len())
                        .sum(),
                )?,
                ownership_frontier_facts: count(unit.ownership_frontier_facts.len())?,
            })
        }
    }

    use optimization_core::BaselineDecisionLog;
    use optimization_core::{
        OptimizationIdentityBundle, OptimizationPassManifestRecord, OptimizationSelections,
        OptimizationWorkBudget, OptimizationWorkUsage, PrePhysicalOptimizationManifestIdentity,
    };
    use optimization_unit::{
        OptimizationManifestStage, OptimizationStructuralStatistics,
        PhysicalOptimizationDataStatus, PrePhysicalOptimizationManifest, PsiOptimizationUnit,
        PsiTransformationLedger,
    };
    use verified_optimization_input::VerifiedPsiOptimizationInput;

    use crate::validation::ValidatedOptimizedAbstractPlanProjection;

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn project_pre_physical_optimization_manifest(
        input: &VerifiedPsiOptimizationInput,
        final_unit: &PsiOptimizationUnit,
        selections: &OptimizationSelections,
        psi_selections: &OptimizationSelections,
        budget_per_pass: OptimizationWorkBudget,
        usage: OptimizationWorkUsage,
        decisions: &BaselineDecisionLog,
        pass_manifests: &[OptimizationPassManifestRecord],
        ledger: &PsiTransformationLedger,
        bundle: OptimizationIdentityBundle,
        projection: ValidatedOptimizedAbstractPlanProjection,
    ) -> Result<ValidatedPrePhysicalOptimizationManifest, PrePhysicalOptimizationManifestError>
    {
        let mut record = projection::expected_record(
            input,
            final_unit,
            selections,
            psi_selections,
            budget_per_pass,
            usage,
            decisions,
            pass_manifests,
            ledger,
            bundle,
            projection,
        )?;
        record.identity = record.recomputed_identity();
        Ok(ValidatedPrePhysicalOptimizationManifest { record })
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ValidatedPrePhysicalOptimizationManifest {
        record: PrePhysicalOptimizationManifest,
    }

    impl ValidatedPrePhysicalOptimizationManifest {
        pub const fn record(&self) -> &PrePhysicalOptimizationManifest {
            &self.record
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum PrePhysicalOptimizationManifestError {
        InitialUnitProjection,
        StructuralStatisticsOverflow,
    }

    failed_display!(
        PrePhysicalOptimizationManifestError,
        "invalid pre-physical optimization manifest"
    );
}
pub(crate) mod projection {
    use optimization_core::{
        OptimizationIdentityBundle, OptimizationSelectionIdentity, OptimizationSelections,
        OptimizationUnitIdentity, OptimizationValidatorIdentity,
        OptimizedAbstractPlanProjectionIdentity, TransformationLedgerIdentity,
    };
    use optimization_unit::{PsiOptimizationUnit, PsiTransformationLedger};
    use semantic_vocabulary::WorkScheduleIdentity;
    use terminal_psi::TerminalPsiIdentity;

    pub(crate) fn seal_optimized_abstract_plan_projection(
        final_unit: &PsiOptimizationUnit,
        selections: &OptimizationSelections,
        psi_selections: &OptimizationSelections,
        ledger: &PsiTransformationLedger,
        bundle: OptimizationIdentityBundle,
    ) -> ValidatedOptimizedAbstractPlanProjection {
        ValidatedOptimizedAbstractPlanProjection {
            psi: final_unit.psi,
            work_schedule: final_unit.work_schedule,
            initial_unit: ledger.input(),
            final_unit: final_unit.identity,
            selections: selections.identity(),
            psi_selections: psi_selections.identity(),
            ledger: ledger.identity(),
            bundle: bundle.identity(),
            validator: OptimizationValidatorIdentity::from_canonical_bytes(
                b"omega.validator.optimized-abstract-plan-projection.v33",
            ),
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ValidatedOptimizedAbstractPlanProjection {
        psi: TerminalPsiIdentity,
        work_schedule: WorkScheduleIdentity,
        initial_unit: OptimizationUnitIdentity,
        final_unit: OptimizationUnitIdentity,
        selections: OptimizationSelectionIdentity,
        psi_selections: OptimizationSelectionIdentity,
        ledger: TransformationLedgerIdentity,
        bundle: optimization_core::OptimizationIdentityBundleIdentity,
        validator: OptimizationValidatorIdentity,
    }

    impl ValidatedOptimizedAbstractPlanProjection {
        pub const fn psi(self) -> TerminalPsiIdentity {
            self.psi
        }

        pub const fn work_schedule(self) -> WorkScheduleIdentity {
            self.work_schedule
        }

        pub const fn initial_unit(self) -> OptimizationUnitIdentity {
            self.initial_unit
        }

        pub const fn final_unit(self) -> OptimizationUnitIdentity {
            self.final_unit
        }

        pub const fn selections(self) -> OptimizationSelectionIdentity {
            self.selections
        }

        pub const fn psi_selections(self) -> OptimizationSelectionIdentity {
            self.psi_selections
        }

        pub const fn ledger(self) -> TransformationLedgerIdentity {
            self.ledger
        }

        pub const fn bundle(self) -> optimization_core::OptimizationIdentityBundleIdentity {
            self.bundle
        }

        pub const fn validator(self) -> OptimizationValidatorIdentity {
            self.validator
        }

        pub fn identity(self) -> OptimizedAbstractPlanProjectionIdentity {
            let mut canonical = Vec::with_capacity(272);
            crate::codec_bytes::push_encoded(
                &mut canonical,
                self.psi.vocabulary_marker.get().to_le_bytes(),
            );
            canonical.extend_from_slice(self.psi.program_fingerprint.as_bytes());
            crate::codec_bytes::push_encoded(
                &mut canonical,
                self.work_schedule.marker().to_le_bytes(),
            );
            canonical.extend_from_slice(&self.initial_unit.bytes());
            canonical.extend_from_slice(&self.final_unit.bytes());
            canonical.extend_from_slice(&self.selections.bytes());
            canonical.extend_from_slice(&self.psi_selections.bytes());
            canonical.extend_from_slice(&self.ledger.bytes());
            canonical.extend_from_slice(&self.bundle.bytes());
            canonical.extend_from_slice(&self.validator.bytes());
            OptimizedAbstractPlanProjectionIdentity::from_canonical_bytes(&canonical)
        }
    }
}
pub(crate) mod relocation_rewrites {
    use super::place_observations::{member_place_references, produced_place_root};
    pub(crate) use abstract_operations::AbstractOperation as O;
    use abstract_operations::AbstractStructuralCallResult;
    use optimization_unit::{
        OptimizationNode, OptimizerCycleComponent, OwnershipEvent, PsiOptimizationFunction,
    };
    use semantic_vocabulary::{BlockId, PlaceId, ScalarTerm, StructuralPlaceKind, ValueId};
    pub(crate) use std::collections::{BTreeMap, BTreeSet};
    use terminal_psi::{CrashRouteGuard, TerminalAffineCleanupAction};

    pub(crate) fn restricted_produced_root(operation: &O) -> bool {
        match operation {
            abstract_operations::establish_case_or_record_result_pattern!(result)
            | O::CallStructural {
                result: AbstractStructuralCallResult::Structural { result, .. },
                ..
            } => result.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted,
            O::EstablishTrivialAffineLocal { .. } => true,
            _ => false,
        }
    }

    pub(crate) fn scalar_case_result_consumed(
        function: &PsiOptimizationFunction,
        component: &OptimizerCycleComponent,
        picked: PlaceId,
    ) -> bool {
        let members: BTreeSet<BlockId> = component.members.iter().copied().collect();
        function
            .blocks
            .iter()
            .filter(|block| members.contains(&block.id))
            .flat_map(|block| block.nodes.iter())
            .any(|node| {
                matches!(
                    &node.operation,
                    abstract_operations::call_structural_or_boundary_call_pattern!(structural_arguments) if structural_arguments.iter().any(|argument| {
                        argument.place == picked
                            && argument.access == terminal_psi::StructuralAccess::Owned
                            && argument.path.is_empty()
                    })
                )
            })
    }

    pub(crate) fn restricted_member_produced_root(
        function: &PsiOptimizationFunction,
        component: &OptimizerCycleComponent,
        root: PlaceId,
    ) -> bool {
        component
            .members
            .iter()
            .filter_map(|member| function.block(*member))
            .flat_map(|block| block.nodes.iter())
            .filter(|node| produced_place_root(&node.operation) == Some(root))
            .all(|node| match &node.operation {
                abstract_operations::establish_case_or_record_result_pattern!(result)
                | O::CallStructural {
                    result: AbstractStructuralCallResult::Structural { result, .. },
                    ..
                } => {
                    result.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                        && result.qualifications.is_empty()
                        && result.projected_qualifications.is_empty()
                        && result.claims.is_empty()
                }
                O::EstablishTrivialAffineLocal { place, .. } => place.id == root,
                _ => false,
            })
    }

    pub(crate) fn scalar_case_result_contained(
        function: &PsiOptimizationFunction,
        component: &OptimizerCycleComponent,
        picked: PlaceId,
    ) -> bool {
        let members: BTreeSet<BlockId> = component.members.iter().copied().collect();
        let consumed = scalar_case_result_consumed(function, component, picked);
        for block in &function.blocks {
            for node in &block.nodes {
                let member = members.contains(&block.id);
                let confined = match &node.operation {
                    abstract_operations::establish_case_or_record_result_pattern!(result)
                    | O::CallStructural {
                        result: AbstractStructuralCallResult::Structural { result, .. },
                        ..
                    } if result.place == picked => member,
                    O::EstablishTrivialAffineLocal { place, .. } if place.id == picked => member,
                    O::StructuralCase { source, .. }
                    | abstract_operations::scalar_storage_observation_pattern!(source, ..)
                    | O::ByteSequenceRead { source, .. }
                    | O::StructuralByteSequenceFieldLength { source, .. }
                        if *source == picked =>
                    {
                        member && !consumed
                    }
                    O::ByteSequenceLength { source, .. }
                    | O::ElementViewLength { source, .. }
                    | O::ByteSequenceSubslice { source, .. }
                    | O::ElementViewSubslice { source, .. }
                        if source.place == picked =>
                    {
                        member && !consumed
                    }
                    O::ReturnStructural { source, .. } => {
                        *source != picked || (member && !consumed)
                    }
                    O::CallStructural {
                        structural_arguments,
                        claim_transfers,
                        result,
                        ..
                    } if structural_arguments
                        .iter()
                        .any(|argument| argument.place == picked) =>
                    {
                        let claims_quiet = claim_transfers.is_empty()
                            && !matches!(
                                result,
                                AbstractStructuralCallResult::Structural { result, .. }
                                    if !result.claims.is_empty()
                            );
                        member
                            && claims_quiet
                            && structural_arguments
                                .iter()
                                .filter(|argument| argument.place == picked)
                                .all(|argument| {
                                    if consumed {
                                        argument.access == terminal_psi::StructuralAccess::Owned
                                            && argument.path.is_empty()
                                    } else {
                                        argument.access
                                            == terminal_psi::StructuralAccess::SharedBorrow
                                    }
                                })
                            && (consumed || matches!(result, AbstractStructuralCallResult::Unit))
                    }
                    operation => {
                        let mut references = BTreeSet::new();
                        if member_place_references(operation, &mut references) {
                            !references.contains(&picked)
                        } else {
                            !member
                        }
                    }
                };
                if !confined {
                    return false;
                }
                if !member {
                    continue;
                }
                if let abstract_operations::return_or_return_unit_pattern!(cleanup_actions, ..) =
                    &node.operation
                    && cleanup_actions.iter().any(|action| {
                        matches!(action, TerminalAffineCleanupAction::InvokeNominal(_))
                    })
                {
                    return false;
                }
                for edge in &node.successors {
                    if edge.cleanup_actions.iter().any(|action| {
                        action.place() == picked
                            && !matches!(action, TerminalAffineCleanupAction::DiscardRoot(_))
                    }) || edge.structural_bindings.iter().any(|binding| {
                        binding.parameter == picked || binding.argument.place == picked
                    }) || (consumed
                        && edge
                            .cleanup_actions
                            .iter()
                            .any(|action| action.place() == picked))
                    {
                        return false;
                    }
                }
            }
        }
        function
            .blocks
            .iter()
            .filter(|block| !members.contains(&block.id))
            .flat_map(|block| block.successors())
            .all(|edge| {
                !edge
                    .cleanup_actions
                    .iter()
                    .any(|action| action.place() == picked)
            })
    }

    pub(crate) fn rewrite_scalar_case_custody(
        structural_places: &[terminal_psi::StructuralPlaceDeclaration],
        members: &BTreeSet<BlockId>,
        case_results: &BTreeSet<PlaceId>,
        node: &mut OptimizationNode,
    ) {
        if case_results.is_empty() {
            return;
        }
        match &mut node.operation {
            O::Jump {
                target,
                cleanup_actions,
                ..
            } => {
                rewrite_case_edge_discards(
                    structural_places,
                    members,
                    *target,
                    case_results,
                    cleanup_actions,
                );
            }
            O::Conditional {
                when_true,
                when_false,
                ..
            } => {
                for successor in [when_true, when_false] {
                    rewrite_case_edge_discards(
                        structural_places,
                        members,
                        successor.target,
                        case_results,
                        &mut successor.cleanup_actions,
                    );
                }
            }
            O::StructuralCase { cases, .. } => {
                for case in cases {
                    rewrite_case_edge_discards(
                        structural_places,
                        members,
                        case.target,
                        case_results,
                        &mut case.cleanup_actions,
                    );
                }
            }
            abstract_operations::return_or_return_unit_pattern!(cleanup_actions, ..) => {
                for place in case_results {
                    insert_case_cleanup_discard(structural_places, cleanup_actions, *place);
                }
            }
            O::ReturnStructural {
                source,
                cleanup_actions,
                ..
            } => {
                for place in case_results {
                    if *source != *place {
                        insert_case_cleanup_discard(structural_places, cleanup_actions, *place);
                    }
                }
            }
            _ => {}
        }
        for edge in &mut node.successors {
            rewrite_case_edge_discards(
                structural_places,
                members,
                edge.target,
                case_results,
                &mut edge.cleanup_actions,
            );
        }
        for event in &mut node.ownership {
            if let OwnershipEvent::Cleanup(actions) = event {
                for place in case_results {
                    insert_case_cleanup_discard(structural_places, actions, *place);
                }
            }
        }
    }

    fn rewrite_case_edge_discards(
        structural_places: &[terminal_psi::StructuralPlaceDeclaration],
        members: &BTreeSet<BlockId>,
        target: BlockId,
        case_results: &BTreeSet<PlaceId>,
        discards: &mut Vec<TerminalAffineCleanupAction>,
    ) {
        if members.contains(&target) {
            discards.retain(|action| {
                !matches!(action,
                TerminalAffineCleanupAction::DiscardRoot(place) if case_results.contains(place))
            });
        } else {
            for place in case_results {
                insert_case_cleanup_discard(structural_places, discards, *place);
            }
        }
    }

    fn insert_case_cleanup_discard(
        structural_places: &[terminal_psi::StructuralPlaceDeclaration],
        actions: &mut Vec<TerminalAffineCleanupAction>,
        place: PlaceId,
    ) {
        if actions.iter().any(|action| {
            matches!(
                action,
                TerminalAffineCleanupAction::DiscardRoot(existing)
                    if *existing == place
            )
        }) {
            return;
        }
        let Some(key) = case_result_schedule_key(structural_places, place) else {
            return;
        };
        let position = actions
            .iter()
            .position(|action| {
                case_result_schedule_key(structural_places, action.place())
                    .is_none_or(|existing_key| existing_key >= key)
            })
            .unwrap_or(actions.len());
        actions.insert(position, TerminalAffineCleanupAction::DiscardRoot(place));
    }

    fn case_result_schedule_key(
        structural_places: &[terminal_psi::StructuralPlaceDeclaration],
        place: PlaceId,
    ) -> Option<(u8, std::cmp::Reverse<u64>, usize)> {
        structural_places
            .iter()
            .enumerate()
            .find_map(|(index, declaration)| {
                (declaration.id == place).then_some(match declaration.kind {
                    StructuralPlaceKind::OperationResult { producer, .. } => {
                        (0, std::cmp::Reverse(producer.get()), index)
                    }
                    StructuralPlaceKind::TrivialAffineLocal {
                        declaration_ordinal,
                        ..
                    } => (1, std::cmp::Reverse(u64::from(declaration_ordinal)), index),
                    _ => return None,
                })
            })
    }

    pub(crate) fn substitute_invariant_scalar_operands(
        operation: &mut O,
        substitution: &BTreeMap<ValueId, ValueId>,
    ) {
        fn substitute(field: &mut ValueId, substitution: &BTreeMap<ValueId, ValueId>) {
            if let Some(representative) = substitution.get(field) {
                *field = *representative;
            }
        }
        match operation {
            O::BooleanNot { operand, .. }
            | O::IntegerBitwiseNot { operand, .. }
            | abstract_operations::integer_widen_or_exact_cast_pattern!(operand, ..)
            | O::ScalarRepresentationView { operand, .. } => substitute(operand, substitution),
            O::BooleanEqual { left, right, .. }
            | abstract_operations::integer_equal_less_pair_pattern!(left, right, ..)
            | O::IntegerLessOrEqual { left, right, .. }
            | O::IntegerBitwiseAnd { left, right, .. }
            | abstract_operations::bitwise_or_xor_pair_pattern!(left, right, ..)
            | O::IntegerArithmetic { left, right, .. }
            | O::IeeeFloatCompare { left, right, .. } => {
                substitute(left, substitution);
                substitute(right, substitution);
            }
            O::IntegerShift { value, count, .. } => {
                substitute(value, substitution);
                substitute(count, substitution);
            }
            O::ByteSequenceRead { index, length, .. } => {
                substitute(index, substitution);
                substitute(length, substitution);
            }
            O::ByteSequenceSubslice {
                start, end, length, ..
            }
            | O::ElementViewSubslice {
                start, end, length, ..
            } => {
                substitute(start, substitution);
                substitute(end, substitution);
                substitute(length, substitution);
            }
            O::NearestIeeeFloatFusedMultiplyAdd {
                left,
                right,
                addend,
                ..
            } => {
                substitute(left, substitution);
                substitute(right, substitution);
                substitute(addend, substitution);
            }
            abstract_operations::call_or_call_structural_pattern!(arguments, ..) => {
                for argument in arguments {
                    substitute(argument, substitution);
                }
            }
            O::EstablishPrimitiveLocal { value, .. } => {
                substitute(&mut value.value, substitution);
            }
            O::EstablishScalarArray { elements, .. } => {
                for element in elements {
                    substitute(element, substitution);
                }
            }
            abstract_operations::establish_case_or_record_fields_pattern!(fields) => {
                for field in fields {
                    if let terminal_psi::RecordFieldValue::Scalar { value, .. } = &mut field.value {
                        substitute(value, substitution);
                    }
                }
            }
            _ => {}
        }
    }

    pub(crate) fn substitute_invariant_place_root(
        operation: &mut O,
        parameter: PlaceId,
        representative: PlaceId,
    ) -> bool {
        let source = match operation {
            abstract_operations::primitive_storage_observation_pattern!(source, ..)
            | O::ByteSequenceRead { source, .. }
            | abstract_operations::byte_len_or_field_read_pattern!(source, ..)
            | O::StructuralScalarFieldRead { source, .. } => source,
            O::ByteSequenceLength { source, .. }
            | O::ElementViewLength { source, .. }
            | O::ByteSequenceSubslice { source, .. }
            | O::ElementViewSubslice { source, .. } => &mut source.place,
            _ => return false,
        };
        if *source != parameter {
            return false;
        }
        *source = representative;
        true
    }

    pub(crate) fn substitute_invariant_call_roots(
        operation: &mut O,
        rewrites: &BTreeMap<PlaceId, PlaceId>,
    ) -> bool {
        let mut applied = BTreeSet::new();
        match operation {
            O::EstablishStructuralArray {
                elements: structural_arguments,
                ..
            }
            | O::CallStructural {
                structural_arguments,
                ..
            } => {
                for argument in structural_arguments.iter_mut() {
                    if let Some(root) = rewrites.get(&argument.place) {
                        applied.insert(argument.place);
                        argument.place = *root;
                    }
                }
            }
            abstract_operations::establish_case_or_record_fields_pattern!(fields) => {
                for field in fields.iter_mut() {
                    if let terminal_psi::RecordFieldValue::Structural(argument) = &mut field.value
                        && let Some(root) = rewrites.get(&argument.place)
                    {
                        applied.insert(argument.place);
                        argument.place = *root;
                    }
                }
            }
            _ => {}
        }
        rewrites.keys().all(|from| applied.contains(from))
    }

    pub(crate) fn call_crash_continuations(
        callee: &PsiOptimizationFunction,
        arguments: &[ValueId],
    ) -> Option<Vec<terminal_psi::CrashRouteBucket>> {
        let contract = callee.verified_contract.as_ref()?;
        if !(contract.erased_scalar_formals.is_empty()
            && contract.erased_proof_formals.is_empty()
            && callee.parameters.len() == arguments.len())
        {
            return None;
        }
        let substitutions = callee
            .parameters
            .iter()
            .zip(arguments)
            .map(|(parameter, argument)| {
                (
                    parameter.value,
                    ScalarTerm::value(*argument, parameter.scalar_type),
                )
            })
            .collect();
        Some(terminal_semantics::substitution::substitute_crash_routes(
            &contract.crash_routes,
            &substitutions,
        ))
    }

    pub(crate) fn structural_call_crash_continuations(
        callee: &PsiOptimizationFunction,
        arguments: &[ValueId],
    ) -> Option<Vec<terminal_psi::CrashRouteBucket>> {
        let contract = callee.verified_contract.as_ref()?;
        let scalar_only = contract
            .crash_routes
            .iter()
            .flat_map(|bucket| &bucket.alternatives)
            .all(|guard| match guard {
                CrashRouteGuard::Truth => true,
                CrashRouteGuard::Predicate(term) => term.proposition().visit_value_ids(|_| ()),
            });
        if !scalar_only {
            return None;
        }
        call_crash_continuations(callee, arguments)
    }
}
pub(crate) mod scalar_case_frontiers {
    pub(crate) use abstract_operations::AbstractOperation as O;
    use optimization_unit::{
        OwnershipFrontierFact, OwnershipFrontierOwnedPlace, OwnershipFrontierSite,
        PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance,
    };
    use semantic_vocabulary::{BlockId, EdgeId, MachineId, OperationId, PlaceId};
    pub(crate) use std::collections::{BTreeMap, BTreeSet};

    pub(crate) fn rewrite_relocated_case_result_frontiers(
        facts: &mut [OwnershipFrontierFact],
        unit: &PsiOptimizationUnit,
        relocated: &BTreeMap<MachineId, BTreeSet<PlaceId>>,
    ) {
        for (machine, places) in relocated {
            let Some(function) = unit.function(*machine) else {
                continue;
            };
            for place in places {
                let owned = case_result_owned_sites(function, *place);
                for fact in facts.iter_mut().filter(|fact| fact.machine == *machine) {
                    let owns = owned.contains(&fact.site);
                    let has = fact
                        .snapshot
                        .owned_places
                        .iter()
                        .any(|owned_place| owned_place.place == *place);
                    if owns == has {
                        continue;
                    }
                    let mut snapshot = fact.snapshot.clone();
                    if owns {
                        snapshot.owned_places.push(OwnershipFrontierOwnedPlace {
                            place: *place,
                            multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                        });
                        snapshot
                            .owned_places
                            .sort_by_key(|owned_place| owned_place.place);
                    } else {
                        snapshot
                            .owned_places
                            .retain(|owned_place| owned_place.place != *place);
                    }
                    *fact = OwnershipFrontierFact::new(fact.psi, *machine, fact.site, snapshot);
                }
            }
        }
    }

    fn case_result_owned_sites(
        function: &PsiOptimizationFunction,
        place: PlaceId,
    ) -> BTreeSet<OwnershipFrontierSite> {
        struct BlockPlan {
            operations: Vec<(OperationId, bool, bool)>,
        }

        let mut plans: BTreeMap<BlockId, BlockPlan> = BTreeMap::new();
        let mut edge_rows: BTreeMap<EdgeId, (BlockId, EdgeId, bool)> = BTreeMap::new();
        let mut edge_disposes: BTreeSet<EdgeId> = BTreeSet::new();
        let mut in_edges: BTreeMap<BlockId, Vec<EdgeId>> = BTreeMap::new();
        let mut sites: Vec<OwnershipFrontierSite> = Vec::new();
        for block in &function.blocks {
            let mut plan = BlockPlan {
                operations: Vec::new(),
            };
            sites.push(OwnershipFrontierSite::BlockEntry(block.id));
            for node in &block.nodes {
                if let Some(PsiProvenance::Operation(operation)) = node.provenance.first() {
                    plan.operations.push((
                        *operation,
                        crate::validation::place_observations::produced_place_root(&node.operation)
                            == Some(place),
                        operation_consumes_place(&node.operation, place),
                    ));
                    sites.push(OwnershipFrontierSite::OperationEntry(*operation));
                    sites.push(OwnershipFrontierSite::OperationExit(*operation));
                }
                for edge in &node.successors {
                    in_edges.entry(edge.target).or_default().push(edge.psi_edge);
                    if edge
                        .cleanup_actions
                        .iter()
                        .any(|action| action.place() == place)
                        || edge.structural_bindings.iter().any(|binding| {
                            binding.argument.place == place
                                && binding.argument.path.is_empty()
                                && binding.argument.access == terminal_psi::StructuralAccess::Owned
                        })
                    {
                        edge_disposes.insert(edge.psi_edge);
                    }
                    edge_rows.insert(edge.psi_edge, (block.id, edge.psi_edge, true));
                    sites.push(OwnershipFrontierSite::EdgeEntry(edge.psi_edge));
                    sites.push(OwnershipFrontierSite::EdgeExit(edge.psi_edge));
                    for source in edge.provenance.iter().skip(1) {
                        let PsiProvenance::Edge(source_edge) = *source else {
                            continue;
                        };
                        edge_rows.insert(source_edge, (block.id, edge.psi_edge, false));
                        sites.push(OwnershipFrontierSite::EdgeEntry(source_edge));
                        sites.push(OwnershipFrontierSite::EdgeExit(source_edge));
                    }
                }
                match &node.operation {
                    abstract_operations::return_or_return_unit_pattern!(psi_edge, ..)
                    | O::ReturnStructural { psi_edge, .. }
                    | O::Crash { psi_edge, .. } => {
                        edge_rows.insert(*psi_edge, (block.id, *psi_edge, false));
                        sites.push(OwnershipFrontierSite::EdgeEntry(*psi_edge));
                    }
                    _ => {}
                }
            }
            plans.insert(block.id, plan);
        }

        let mut operation_position: BTreeMap<OperationId, (BlockId, usize)> = BTreeMap::new();
        for (block, plan) in &plans {
            for (index, (operation, _, _)) in plan.operations.iter().enumerate() {
                operation_position.insert(*operation, (*block, index));
            }
        }
        let block_tail = |block: &BlockId| -> OwnershipFrontierSite {
            plans
                .get(block)
                .and_then(|plan| plan.operations.last())
                .map(|(operation, _, _)| OwnershipFrontierSite::OperationExit(*operation))
                .unwrap_or(OwnershipFrontierSite::BlockEntry(*block))
        };

        let mut owned: BTreeMap<OwnershipFrontierSite, bool> =
            sites.iter().map(|site| (*site, true)).collect();
        for _ in 0..=sites.len() {
            let mut changed = false;
            for site in &sites {
                let next = match *site {
                    OwnershipFrontierSite::BlockEntry(block) => {
                        block != function.entry
                            && in_edges.get(&block).is_some_and(|entries| {
                                entries
                                    .iter()
                                    .all(|edge| owned[&OwnershipFrontierSite::EdgeExit(*edge)])
                            })
                    }
                    OwnershipFrontierSite::OperationEntry(operation) => operation_position
                        .get(&operation)
                        .is_some_and(|(block, index)| {
                            if *index == 0 {
                                owned[&OwnershipFrontierSite::BlockEntry(*block)]
                            } else {
                                let previous = plans[block].operations[*index - 1].0;
                                owned[&OwnershipFrontierSite::OperationExit(previous)]
                            }
                        }),
                    OwnershipFrontierSite::OperationExit(operation) => operation_position
                        .get(&operation)
                        .is_some_and(|(block, index)| {
                            let (_, produces, consumes) = plans[block].operations[*index];
                            produces
                                || (owned[&OwnershipFrontierSite::OperationEntry(operation)]
                                    && !consumes)
                        }),
                    OwnershipFrontierSite::EdgeEntry(edge) => edge_rows
                        .get(&edge)
                        .is_some_and(|(source, _, _)| owned[&block_tail(source)]),
                    OwnershipFrontierSite::EdgeExit(edge) => {
                        edge_rows
                            .get(&edge)
                            .is_some_and(|(source, canonical, row_zero)| {
                                owned[&block_tail(source)]
                                    && !(*row_zero && edge_disposes.contains(canonical))
                            })
                    }
                };
                if owned[site] != next {
                    owned.insert(*site, next);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        owned
            .into_iter()
            .filter_map(|(site, owned)| owned.then_some(site))
            .collect()
    }

    fn operation_consumes_place(operation: &O, place: PlaceId) -> bool {
        match operation {
            abstract_operations::call_structural_or_boundary_call_pattern!(
                structural_arguments
            ) => structural_arguments.iter().any(|argument| {
                argument.place == place
                    && argument.access == terminal_psi::StructuralAccess::Owned
                    && argument.path.is_empty()
            }),
            abstract_operations::establish_record_or_case_pattern!(fields, ..) => {
                fields.iter().any(|field| match &field.value {
                    terminal_psi::RecordFieldValue::Structural(argument) => {
                        argument.place == place
                            && argument.access == terminal_psi::StructuralAccess::Owned
                            && argument.path.is_empty()
                    }
                    terminal_psi::RecordFieldValue::Scalar { .. } => false,
                })
            }
            _ => false,
        }
    }
}

pub(crate) use scalar_case_frontiers::rewrite_relocated_case_result_frontiers;

pub(crate) use context::{
    ValidatedOptimizerCycleComponents, ValidatedOptimizerRankingCertificates,
    validate_transformed_psi_context, validate_verified_psi_context,
};
pub use prephysical_manifest::ValidatedPrePhysicalOptimizationManifest;
pub(crate) use prephysical_manifest::{
    PrePhysicalOptimizationManifestError, project_pre_physical_optimization_manifest,
};
pub(crate) use projection::{
    ValidatedOptimizedAbstractPlanProjection, seal_optimized_abstract_plan_projection,
};
