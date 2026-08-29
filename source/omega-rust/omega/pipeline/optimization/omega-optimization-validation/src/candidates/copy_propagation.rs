//! Redundant block-parameter validation.

use super::*;

pub fn validate_redundant_block_parameter_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    if !candidate
        .required_analyses()
        .contains(AnalysisKind::ControlFlowGraph)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::Dominators)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || candidate.safety_class() != OptimizationSafetyClass::StructuralIdentity
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let PsiRewritePatch::RemoveRedundantBlockParameter(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    let witness = candidate
        .redundant_block_parameter_witness()
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    let function = input
        .functions
        .iter()
        .find(|function| function.machine == patch.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if function.entry == patch.block || patch.parameter == patch.replacement {
        return Err(OptimizationUnitValidationError::CandidateBlockParameterMismatch);
    }
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == patch.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
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
        .chain(function.blocks.iter().flat_map(|block| {
            block
                .parameters
                .iter()
                .chain(block.nodes.iter().flat_map(|node| &node.definitions))
        }))
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
            let location = NodeLocation {
                machine: patch.machine,
                block: source.id,
                node: u32::try_from(node_index).expect("unit node index fits u32"),
            };
            let changes_use = node
                .uses
                .iter()
                .any(|use_site| use_site.value == patch.parameter);
            for edge in &node.successors {
                if edge.target != patch.block {
                    continue;
                }
                let Some(binding) = edge.bindings.get(position) else {
                    return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
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
                expected_provenance.push(omega_optimization_unit::ProvenanceRewrite {
                    input: site,
                    disposition: ProvenanceDisposition::RealizedAt(site),
                    sources: edge.provenance.clone(),
                    fuel: edge.fuel.clone(),
                });
            }
            if changes_use {
                affected_blocks.insert(source.id);
                if !node.provenance.is_empty() {
                    let site = PsiRealizationSite::Node(location);
                    expected_provenance.push(omega_optimization_unit::ProvenanceRewrite {
                        input: site,
                        disposition: ProvenanceDisposition::RealizedAt(site),
                        sources: node.provenance.clone(),
                        fuel: node.fuel.clone(),
                    });
                }
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
    expected_provenance.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    if incoming != witness.incoming
        || incoming
            .iter()
            .any(|binding| binding.argument != patch.replacement)
    {
        return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
    }
    if candidate.substitutions()
        != [omega_optimization_unit::ScalarSubstitution {
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

    let normalized_input =
        normalize_redundant_parameter_observation_input(input, patch, candidate.affected_blocks())?;
    let input_region = reconstruct_psi_closed_region_observation(
        &normalized_input,
        patch.machine,
        candidate.affected_blocks(),
    )
    .ok_or(OptimizationUnitValidationError::CandidateRegionObservationUnavailable)?;

    let mut output = input.clone();
    let function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.machine)
        .expect("candidate function exists");
    let block = function
        .blocks
        .iter_mut()
        .find(|block| block.id == patch.block)
        .expect("candidate block exists");
    block.parameters.remove(position);
    for (new_position, parameter) in block.parameters.iter_mut().enumerate().skip(position) {
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
    function.facts = reconstruct_fact_index(function);
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;
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

/// Construct the validator's normalized pre-rewrite question independently of
/// the output constructor below. Only the exact scalar substitution and the
/// one proved incoming binding slot may change.
pub(crate) fn normalize_redundant_parameter_observation_input(
    input: &PsiOptimizationUnit,
    patch: RedundantBlockParameterRewrite,
    affected_blocks: &[BlockId],
) -> Result<PsiOptimizationUnit, OptimizationUnitValidationError> {
    let affected = affected_blocks.iter().copied().collect::<BTreeSet<_>>();
    let mut normalized = input.clone();
    let function = normalized
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let target = function
        .blocks
        .iter_mut()
        .find(|block| block.id == patch.block)
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
    for (new_position, parameter) in target.parameters.iter_mut().enumerate().skip(position) {
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
            node.operation =
                normalize_redundant_parameter_observation_operation(&node.operation, patch)?;
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
    operation: &omega_abstract_operations::AbstractOperation,
    patch: RedundantBlockParameterRewrite,
) -> Result<omega_abstract_operations::AbstractOperation, OptimizationUnitValidationError> {
    use omega_abstract_operations::AbstractOperation as O;

    let mut normalized = operation.clone();
    let replace = |value: &mut ValueId| {
        if *value == patch.parameter {
            *value = patch.replacement;
        }
    };
    let normalize_bindings = |target: BlockId,
                              bindings: &mut Vec<omega_abstract_operations::ValueBinding>|
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
                return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
            }
            bindings.remove(position);
        }
        Ok(())
    };

    match &mut normalized {
        O::Call { arguments, .. } | O::BoundaryCall { arguments, .. } => {
            for argument in arguments {
                replace(argument);
            }
        }
        O::BooleanNot { operand, .. }
        | O::IntegerBitwiseNot { operand, .. }
        | O::IntegerWiden { operand, .. }
        | O::IntegerExactCast { operand, .. } => replace(operand),
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
        | O::SaturatingIntegerMultiply { left, right, .. } => {
            replace(left);
            replace(right);
        }
        O::WrappingIntegerShiftLeft { value, count, .. }
        | O::WrappingIntegerShiftRight { value, count, .. }
        | O::ExactIntegerShiftLeft { value, count, .. }
        | O::ExactIntegerShiftRight { value, count, .. } => {
            replace(value);
            replace(count);
        }
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
        O::Return { value, .. } => replace(value),
        O::EstablishPayloadlessCase { .. }
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::CallUnit { .. }
        | O::CallStructuralScalar { .. }
        | O::CallStructural { .. }
        | O::PortWrite { .. }
        | O::IntegerConstant { .. }
        | O::BooleanConstant { .. }
        | O::BooleanStructuralField { .. }
        | O::ReturnUnit { .. }
        | O::ReturnStructural { .. }
        | O::Crash { .. } => {}
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
    let Some(expected_function) = expected
        .functions
        .iter_mut()
        .find(|function| function.machine == machine)
    else {
        return false;
    };
    let Some(output_function) = output
        .functions
        .iter()
        .find(|function| function.machine == machine)
    else {
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
        let Some(output_block) = output_function
            .blocks
            .iter()
            .find(|block| block.id == *block_id)
        else {
            return false;
        };
        *expected_block = output_block.clone();
    }
    expected.identity = output.identity;
    expected == *output
}

pub(crate) fn rewrite_block_parameter_operation(
    operation: &mut omega_abstract_operations::AbstractOperation,
    patch: RedundantBlockParameterRewrite,
) {
    use omega_abstract_operations::AbstractOperation as O;

    let replace = |value: &mut ValueId| {
        if *value == patch.parameter {
            *value = patch.replacement;
        }
    };
    let rewrite_bindings = |bindings: &mut Vec<omega_abstract_operations::ValueBinding>| {
        for binding in bindings.iter_mut() {
            if binding.argument == patch.parameter {
                binding.argument = patch.replacement;
            }
        }
    };
    match operation {
        O::Call { arguments, .. } | O::BoundaryCall { arguments, .. } => {
            for argument in arguments {
                replace(argument);
            }
        }
        O::BooleanNot { operand, .. }
        | O::IntegerBitwiseNot { operand, .. }
        | O::IntegerWiden { operand, .. }
        | O::IntegerExactCast { operand, .. } => replace(operand),
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
        | O::SaturatingIntegerMultiply { left, right, .. } => {
            replace(left);
            replace(right);
        }
        O::WrappingIntegerShiftLeft { value, count, .. }
        | O::WrappingIntegerShiftRight { value, count, .. }
        | O::ExactIntegerShiftLeft { value, count, .. }
        | O::ExactIntegerShiftRight { value, count, .. } => {
            replace(value);
            replace(count);
        }
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
        O::Return { value, .. } => replace(value),
        O::EstablishPayloadlessCase { .. }
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::CallUnit { .. }
        | O::CallStructuralScalar { .. }
        | O::CallStructural { .. }
        | O::PortWrite { .. }
        | O::IntegerConstant { .. }
        | O::BooleanConstant { .. }
        | O::BooleanStructuralField { .. }
        | O::ReturnUnit { .. }
        | O::ReturnStructural { .. }
        | O::Crash { .. } => {}
    }
}
