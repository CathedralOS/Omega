//! Independent checks of target-neutral rewrites before Terminal publication.

use crate::{
    ModuleError, reconstruct_optimizable_terminal_obligations, validate_module_for_optimization,
};
use semantic_vocabulary::{
    BlockId, EdgeId, MachineId, ObligationId, OperationId, Proposition, ValueId,
};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{ProofBundle, TerminalModule, Terminator};

/// Block-local identities that module-level evidence carriers name by
/// identity rather than through a direct executable use. A control-flow
/// rewrite may not drop a row one of these carriers retains: the referent
/// stays authoritative even where no ordinary operand reads it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BlockLocalEvidence {
    /// Blocks named as invariant headers or exact projection owners.
    pub blocks: BTreeSet<BlockId>,
    /// Edges carrying qualification coercions or invariant arrivals.
    pub edges: BTreeSet<EdgeId>,
    /// Operations carrying suspension plans, proof-output joins, dynamic
    /// dispatches, reborrow restorations, closed reach calls, or partition
    /// theorem producers.
    pub operations: BTreeSet<OperationId>,
    /// Values named inside propositions, coercion endpoints, retained
    /// suspension frontiers, or float-meaning projection sources.
    pub values: BTreeSet<ValueId>,
}

/// Inventory the block-local rows semantic evidence retains across rewrites.
///
/// Every Terminal identity is module-unique, so the carrier that names a row
/// pins it wherever the row lives. The scan covers only carriers stored in the
/// module itself; the lowering stage separately protects rows its unsealed
/// sidecars name.
pub fn block_local_evidence(module: &TerminalModule) -> BlockLocalEvidence {
    let mut evidence = BlockLocalEvidence::default();
    fn retain_proposition(
        proposition: &semantic_vocabulary::Proposition,
        evidence: &mut BlockLocalEvidence,
    ) {
        proposition.visit_value_ids(|value| {
            evidence.values.insert(value);
        });
    }
    for coercion in &module.scalar_qualifications.coercions {
        evidence.edges.insert(coercion.edge);
        evidence.values.insert(coercion.source);
        evidence.values.insert(coercion.destination);
    }
    for invariant in &module.scalar_block_invariants {
        evidence.blocks.insert(invariant.header);
        retain_proposition(&invariant.predicate, &mut evidence);
        for arrival in &invariant.arrivals {
            evidence.edges.insert(arrival.edge);
        }
    }
    for site in &module.suspension_call_sites {
        evidence.operations.insert(site.operation);
    }
    for plan in &module.suspension_call_plans {
        evidence.operations.insert(plan.operation);
        for live in &plan.live_values {
            if let terminal_psi::TerminalSuspensionPlace::Scalar(value) = live.place {
                evidence.values.insert(value);
            }
        }
    }
    for call in &module.proof_output_calls {
        if let Some(runtime) = &call.runtime_call {
            evidence.operations.insert(runtime.operation);
        }
    }
    let dispatch = &module.dynamic_dispatch;
    for operation in dispatch
        .arguments
        .iter()
        .map(|row| row.operation)
        .chain(dispatch.direct_dispatches.iter().map(|row| row.operation))
        .chain(dispatch.indirect_dispatches.iter().map(|row| row.operation))
        .chain(dispatch.stored_dispatches.iter().map(|row| row.operation))
        .chain(
            dispatch
                .parameter_dispatches
                .iter()
                .map(|row| row.operation),
        )
    {
        evidence.operations.insert(operation);
    }
    for restoration in &module.reborrow_restored_call_uses {
        evidence.operations.insert(restoration.operation);
    }
    for projection in &module.float_meaning_projections {
        match &projection.source {
            terminal_psi::FloatMeaningSource::DirectMachineParameter(parameter) => {
                evidence.values.insert(parameter.parameter);
            }
            terminal_psi::FloatMeaningSource::DirectMachineResult(result) => {
                evidence.values.insert(result.result);
            }
            terminal_psi::FloatMeaningSource::DirectBlockParameter(parameter) => {
                evidence.blocks.insert(parameter.block);
                evidence.values.insert(parameter.parameter);
            }
            terminal_psi::FloatMeaningSource::DirectOperationResult(result) => {
                evidence.operations.insert(result.producer);
                evidence.values.insert(result.result);
            }
            terminal_psi::FloatMeaningSource::DirectCallResult(result) => {
                evidence.operations.insert(result.producer);
                evidence.values.insert(result.result);
            }
            terminal_psi::FloatMeaningSource::TransitionalInput(_)
            | terminal_psi::FloatMeaningSource::DirectStructuralLeaf(_)
            | terminal_psi::FloatMeaningSource::ExactBinary32Literal(_)
            | terminal_psi::FloatMeaningSource::ExactBinary64Literal(_) => {}
        }
    }
    for machine in &module.machines {
        if let Some(application) = &machine.closed_reach_application {
            for call in &application.calls {
                evidence.operations.insert(call.operation);
            }
        }
        for composition in &machine.content_partition_compositions {
            evidence.operations.insert(composition.producer_operation);
        }
        for proposition in &machine.contract.requires {
            retain_proposition(proposition, &mut evidence);
        }
        for clause in &machine.contract.ensures {
            retain_proposition(&clause.proposition, &mut evidence);
        }
        for clause in &machine.contract.outcome_specific_ensures {
            retain_proposition(&clause.proposition, &mut evidence);
        }
        for bucket in &machine.contract.crash_routes {
            for alternative in &bucket.alternatives {
                if let terminal_psi::CrashRouteGuard::Predicate(term) = alternative {
                    retain_proposition(term.proposition(), &mut evidence);
                }
            }
        }
    }
    evidence
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeadScalarRewriteError {
    InvalidModule(ModuleError),
    ChangedProgramStructure,
    ChangedSurvivingOperation(OperationId),
    RemovedNonTotalOperation(OperationId),
    ChangedBlockParameters(BlockId),
    ChangedEdgeArguments(EdgeId),
    ChangedProofQuestion,
}

/// Check an operation-removal subsequence, not the producer's liveness result.
/// Output validation independently rejects surviving uses of a removed value.
/// Exact reconstruction preserves assumptions and their numbering, so every
/// unchanged proof bundle is asked exactly the same questions after the rewrite.
pub fn validate_dead_scalar_elimination(
    before: &TerminalModule,
    after: &TerminalModule,
) -> Result<(), DeadScalarRewriteError> {
    let before_valid =
        validate_module_for_optimization(before).map_err(DeadScalarRewriteError::InvalidModule)?;
    let after_valid =
        validate_module_for_optimization(after).map_err(DeadScalarRewriteError::InvalidModule)?;
    if before.machines.len() != after.machines.len() {
        return Err(DeadScalarRewriteError::ChangedProgramStructure);
    }
    let mut restored = after.clone();
    let mut removed_parameters: BTreeMap<MachineId, BTreeMap<BlockId, Vec<usize>>> =
        BTreeMap::new();
    for ((old, new), restored_machine) in before
        .machines
        .iter()
        .zip(&after.machines)
        .zip(&mut restored.machines)
    {
        if old.blocks.len() != new.blocks.len() {
            return Err(DeadScalarRewriteError::ChangedProgramStructure);
        }
        let mut machine_removed = BTreeMap::new();
        for ((old_block, new_block), restored_block) in old
            .blocks
            .iter()
            .zip(&new.blocks)
            .zip(&mut restored_machine.blocks)
        {
            let mut retained = new_block.operations.iter().peekable();
            for operation in &old_block.operations {
                if retained.peek().is_some_and(|next| next.id == operation.id) {
                    if retained.next() != Some(operation) {
                        return Err(DeadScalarRewriteError::ChangedSurvivingOperation(
                            operation.id,
                        ));
                    }
                } else if operation.result.scalar().is_none()
                    || !terminal_semantics::is_unconditionally_total_scalar(&operation.kind)
                {
                    return Err(DeadScalarRewriteError::RemovedNonTotalOperation(
                        operation.id,
                    ));
                }
            }
            if retained.next().is_some() {
                return Err(DeadScalarRewriteError::ChangedProgramStructure);
            }
            let mut retained_parameters = new_block.parameters.iter().peekable();
            let mut removed = Vec::new();
            for (position, parameter) in old_block.parameters.iter().enumerate() {
                if retained_parameters
                    .peek()
                    .is_some_and(|next| next.id == parameter.id)
                {
                    if retained_parameters.next() != Some(parameter) {
                        return Err(DeadScalarRewriteError::ChangedBlockParameters(old_block.id));
                    }
                } else {
                    removed.push(position);
                }
            }
            if retained_parameters.next().is_some() {
                return Err(DeadScalarRewriteError::ChangedBlockParameters(old_block.id));
            }
            if !removed.is_empty() {
                machine_removed.insert(old_block.id, removed);
            }
            restored_block.operations.clone_from(&old_block.operations);
            restored_block.parameters.clone_from(&old_block.parameters);
        }
        if !machine_removed.is_empty() {
            removed_parameters.insert(old.id, machine_removed);
        }
    }
    for ((old, new), restored_machine) in before
        .machines
        .iter()
        .zip(&after.machines)
        .zip(&mut restored.machines)
    {
        let machine_removed = removed_parameters.get(&old.id);
        for ((old_block, new_block), restored_block) in old
            .blocks
            .iter()
            .zip(&new.blocks)
            .zip(&mut restored_machine.blocks)
        {
            let mut expected = old_block.terminator.clone();
            match &mut expected {
                Terminator::Jump {
                    target, arguments, ..
                } => drop_removed(
                    arguments,
                    machine_removed.and_then(|removed| removed.get(target)),
                ),
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    for edge in [when_true, when_false] {
                        drop_removed(
                            &mut edge.arguments,
                            machine_removed.and_then(|removed| removed.get(&edge.target)),
                        );
                    }
                }
                _ => {}
            }
            if new_block.terminator != expected {
                let edge = new_block
                    .terminator
                    .edges()
                    .next()
                    .ok_or(DeadScalarRewriteError::ChangedProgramStructure)?;
                return Err(DeadScalarRewriteError::ChangedEdgeArguments(edge));
            }
            restored_block.terminator.clone_from(&old_block.terminator);
        }
    }
    if &restored != before {
        return Err(DeadScalarRewriteError::ChangedProgramStructure);
    }
    let old_question = reconstruct_optimizable_terminal_obligations(before_valid)
        .map_err(DeadScalarRewriteError::InvalidModule)?;
    let new_question = reconstruct_optimizable_terminal_obligations(after_valid)
        .map_err(DeadScalarRewriteError::InvalidModule)?;
    if old_question != new_question {
        return Err(DeadScalarRewriteError::ChangedProofQuestion);
    }
    Ok(())
}

/// Drop the listed old positions from one edge's scalar arguments.
fn drop_removed(arguments: &mut Vec<ValueId>, removed: Option<&Vec<usize>>) {
    let Some(removed) = removed else { return };
    let mut position = 0;
    arguments.retain(|_| {
        let retained = !removed.contains(&position);
        position += 1;
        retained
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyPropagationRewriteError {
    InvalidModule(ModuleError),
    ChangedProgramStructure,
    ChangedBlockParameters(BlockId),
    RemovedNonCopyParameter(BlockId),
    ChangedMachine(MachineId),
    ChangedProofQuestion,
}

/// Check the exact scalar-copy relation, not the producer's resolution result.
///
/// A removed block parameter is justified only when every inventoried incoming
/// `Jump`/`Conditional` edge binds the same resolved value at its position.
/// The check re-derives the resolution lattice from `before`, substitutes only
/// removed parameters at scalar use sites, drops their edge-argument
/// positions, and compares the reconstructed module to `after`. Propositions
/// and proof evidence are not use sites: value identities they carry are
/// preserved by valid module validation rather than rewritten here.
pub fn validate_copy_propagation(
    before: &TerminalModule,
    after: &TerminalModule,
) -> Result<(), CopyPropagationRewriteError> {
    let before_valid = validate_module_for_optimization(before)
        .map_err(CopyPropagationRewriteError::InvalidModule)?;
    let after_valid = validate_module_for_optimization(after)
        .map_err(CopyPropagationRewriteError::InvalidModule)?;
    if before.machines.len() != after.machines.len() {
        return Err(CopyPropagationRewriteError::ChangedProgramStructure);
    }
    let mut expected = before.clone();
    for ((old, new), expected_machine) in before
        .machines
        .iter()
        .zip(&after.machines)
        .zip(&mut expected.machines)
    {
        if old.blocks.len() != new.blocks.len() {
            return Err(CopyPropagationRewriteError::ChangedProgramStructure);
        }
        let mut removed_positions: BTreeMap<BlockId, Vec<usize>> = BTreeMap::new();
        let mut removed_ids = std::collections::BTreeSet::new();
        for (old_block, new_block) in old.blocks.iter().zip(&new.blocks) {
            let mut retained = new_block.parameters.iter().peekable();
            let mut removed = Vec::new();
            for (position, parameter) in old_block.parameters.iter().enumerate() {
                if retained.peek().is_some_and(|next| next.id == parameter.id) {
                    if retained.next() != Some(parameter) {
                        return Err(CopyPropagationRewriteError::ChangedBlockParameters(
                            old_block.id,
                        ));
                    }
                } else {
                    removed.push(position);
                    removed_ids.insert(parameter.id);
                }
            }
            if retained.next().is_some() {
                return Err(CopyPropagationRewriteError::ChangedBlockParameters(
                    old_block.id,
                ));
            }
            if !removed.is_empty() {
                removed_positions.insert(old_block.id, removed);
            }
        }
        // Inventory the scalar arguments each inventoried edge binds at a
        // target block, in `before`. Structural-case successors bind payloads
        // positionally without listed arguments, so their targets can never
        // justify a copy removal here.
        let mut case_targets = std::collections::BTreeSet::new();
        let mut incoming: BTreeMap<BlockId, Vec<Vec<ValueId>>> = BTreeMap::new();
        for block in &old.blocks {
            match &block.terminator {
                Terminator::Jump {
                    target, arguments, ..
                } => {
                    incoming.entry(*target).or_default().push(arguments.clone());
                }
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    for edge in [when_true, when_false] {
                        incoming
                            .entry(edge.target)
                            .or_default()
                            .push(edge.arguments.clone());
                    }
                }
                Terminator::StructuralCase { cases, .. } => {
                    case_targets.extend(cases.iter().map(|case| case.target));
                }
                _ => {}
            }
        }
        let mut raw: BTreeMap<ValueId, Vec<ValueId>> = BTreeMap::new();
        let mut declarations: BTreeMap<ValueId, terminal_psi::ValueDeclaration> = BTreeMap::new();
        for parameter in &old.parameters {
            declarations.insert(parameter.id, *parameter);
        }
        if let Some(result) = old.result.scalar() {
            declarations.insert(result.id, result);
        }
        for block in &old.blocks {
            for parameter in &block.parameters {
                declarations.insert(parameter.id, *parameter);
            }
            for operation in &block.operations {
                if let Some(result) = operation.result.scalar() {
                    declarations.insert(result.id, result);
                }
            }
            if case_targets.contains(&block.id) {
                continue;
            }
            if let Some(edges) = incoming.get(&block.id) {
                for (position, parameter) in block.parameters.iter().enumerate() {
                    raw.insert(
                        parameter.id,
                        edges.iter().map(|arguments| arguments[position]).collect(),
                    );
                }
            }
        }
        let mut memo: BTreeMap<ValueId, ValueId> = BTreeMap::new();
        let mut substitution: BTreeMap<ValueId, ValueId> = BTreeMap::new();
        for (block_id, positions) in &removed_positions {
            let old_block = old
                .blocks
                .iter()
                .find(|block| block.id == *block_id)
                .ok_or(CopyPropagationRewriteError::ChangedProgramStructure)?;
            for &position in positions {
                let parameter = &old_block.parameters[position];
                if !raw.contains_key(&parameter.id) {
                    return Err(CopyPropagationRewriteError::RemovedNonCopyParameter(
                        *block_id,
                    ));
                }
                let source = resolve_copy_source(parameter.id, &raw, &mut memo);
                if source == parameter.id
                    || declarations.get(&source).is_none_or(|declaration| {
                        declaration.scalar_type != parameter.scalar_type
                            || declaration.qualifications != parameter.qualifications
                    })
                {
                    return Err(CopyPropagationRewriteError::RemovedNonCopyParameter(
                        *block_id,
                    ));
                }
                substitution.insert(parameter.id, source);
            }
        }
        for expected_block in &mut expected_machine.blocks {
            expected_block
                .parameters
                .retain(|parameter| !removed_ids.contains(&parameter.id));
            for operation in &mut expected_block.operations {
                operation.kind.map_scalar_uses(&mut |value| {
                    substitution.get(&value).copied().unwrap_or(value)
                });
            }
            expected_block
                .terminator
                .map_scalar_uses(&mut |value| substitution.get(&value).copied().unwrap_or(value));
            match &mut expected_block.terminator {
                Terminator::Jump {
                    target, arguments, ..
                } => drop_removed(arguments, removed_positions.get(target)),
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    for edge in [when_true, when_false] {
                        drop_removed(&mut edge.arguments, removed_positions.get(&edge.target));
                    }
                }
                _ => {}
            }
        }
        if expected_machine != new {
            return Err(CopyPropagationRewriteError::ChangedMachine(old.id));
        }
    }
    if &expected != after {
        return Err(CopyPropagationRewriteError::ChangedProgramStructure);
    }
    let old_question = reconstruct_optimizable_terminal_obligations(before_valid)
        .map_err(CopyPropagationRewriteError::InvalidModule)?;
    let new_question = reconstruct_optimizable_terminal_obligations(after_valid)
        .map_err(CopyPropagationRewriteError::InvalidModule)?;
    if old_question != new_question {
        return Err(CopyPropagationRewriteError::ChangedProofQuestion);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GlobalValueNumberingRewriteError {
    InvalidModule(ModuleError),
    ChangedProgramStructure,
    RemovedIneligibleOperation(OperationId),
    MissingDominatingEquivalent(OperationId),
    ChangedMachine(MachineId),
    ChangedProofQuestion,
}

/// Check the exact duplicate-removal relation, not the producer's numbering.
///
/// A removed operation is justified only when a surviving
/// unconditionally-total scalar operation with the same kind and an equal
/// result declaration dominates it: earlier in its own block or anywhere in a
/// dominating block. Operand identities compare after resolving the results
/// of already-removed duplicates to their canonical survivors, so the check
/// re-derives which surviving result each removed operation must map to,
/// applies exactly that substitution at every direct scalar use, and compares
/// the reconstructed module to `after`. The canonical survivor is the first
/// matching operation in reverse postorder, so the producer cannot pick a
/// different representative and still verify.
pub fn validate_global_value_numbering(
    before: &TerminalModule,
    after: &TerminalModule,
) -> Result<(), GlobalValueNumberingRewriteError> {
    let before_valid = validate_module_for_optimization(before)
        .map_err(GlobalValueNumberingRewriteError::InvalidModule)?;
    let after_valid = validate_module_for_optimization(after)
        .map_err(GlobalValueNumberingRewriteError::InvalidModule)?;
    if before.machines.len() != after.machines.len() {
        return Err(GlobalValueNumberingRewriteError::ChangedProgramStructure);
    }
    let mut expected = before.clone();
    for ((old, new), expected_machine) in before
        .machines
        .iter()
        .zip(&after.machines)
        .zip(&mut expected.machines)
    {
        if old.blocks.len() != new.blocks.len() {
            return Err(GlobalValueNumberingRewriteError::ChangedProgramStructure);
        }
        let mut surviving = std::collections::BTreeSet::new();
        for (old_block, new_block) in old.blocks.iter().zip(&new.blocks) {
            if old_block.id != new_block.id
                || old_block.parameters != new_block.parameters
                || old_block.structural_parameters != new_block.structural_parameters
            {
                return Err(GlobalValueNumberingRewriteError::ChangedProgramStructure);
            }
            // Surviving operations keep their identity and relative order;
            // substituted operand content is checked against `expected` below.
            let mut retained = old_block.operations.iter();
            for operation in &new_block.operations {
                if !retained
                    .by_ref()
                    .any(|old_operation| old_operation.id == operation.id)
                {
                    return Err(GlobalValueNumberingRewriteError::ChangedProgramStructure);
                }
                surviving.insert(operation.id);
            }
        }
        let dominators = crate::control_graph::dominators(old);
        let mut representative: BTreeMap<ValueId, ValueId> = BTreeMap::new();
        let mut leaders: Vec<(
            terminal_psi::OperationKind,
            BlockId,
            terminal_psi::ValueDeclaration,
        )> = Vec::new();
        for block_id in crate::control_graph::reverse_postorder(old) {
            let old_block = old
                .blocks
                .iter()
                .find(|block| block.id == block_id)
                .ok_or(GlobalValueNumberingRewriteError::ChangedProgramStructure)?;
            for operation in &old_block.operations {
                let eligible = operation.static_reach_binding.is_none()
                    && operation.result.scalar().is_some()
                    && terminal_semantics::is_unconditionally_total_scalar(&operation.kind);
                if surviving.contains(&operation.id) {
                    if eligible {
                        let mut kind = operation.kind.clone();
                        kind.map_scalar_uses(&mut |value| {
                            representative.get(&value).copied().unwrap_or(value)
                        });
                        leaders.push((kind, block_id, operation.result.expect_scalar()));
                    }
                    continue;
                }
                if !eligible {
                    return Err(
                        GlobalValueNumberingRewriteError::RemovedIneligibleOperation(operation.id),
                    );
                }
                let mut kind = operation.kind.clone();
                kind.map_scalar_uses(&mut |value| {
                    representative.get(&value).copied().unwrap_or(value)
                });
                let result = operation.result.expect_scalar();
                let Some((_, _, survivor)) = leaders.iter().find(|(leader_kind, block, leader)| {
                    *leader_kind == kind
                        && dominators.dominates(*block, block_id)
                        && leader.scalar_type == result.scalar_type
                        && leader.qualifications == result.qualifications
                }) else {
                    return Err(
                        GlobalValueNumberingRewriteError::MissingDominatingEquivalent(operation.id),
                    );
                };
                representative.insert(result.id, survivor.id);
            }
        }
        for expected_block in &mut expected_machine.blocks {
            expected_block
                .operations
                .retain(|operation| surviving.contains(&operation.id));
            for operation in &mut expected_block.operations {
                operation.kind.map_scalar_uses(&mut |value| {
                    representative.get(&value).copied().unwrap_or(value)
                });
            }
            expected_block
                .terminator
                .map_scalar_uses(&mut |value| representative.get(&value).copied().unwrap_or(value));
        }
        if expected_machine != new {
            return Err(GlobalValueNumberingRewriteError::ChangedMachine(old.id));
        }
    }
    if &expected != after {
        return Err(GlobalValueNumberingRewriteError::ChangedProgramStructure);
    }
    let old_question = reconstruct_optimizable_terminal_obligations(before_valid)
        .map_err(GlobalValueNumberingRewriteError::InvalidModule)?;
    let new_question = reconstruct_optimizable_terminal_obligations(after_valid)
        .map_err(GlobalValueNumberingRewriteError::InvalidModule)?;
    if old_question != new_question {
        return Err(GlobalValueNumberingRewriteError::ChangedProofQuestion);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SparseConditionalConstantPropagationRewriteError {
    InvalidModule(ModuleError),
    ChangedProgramStructure,
    ChangedMachine(MachineId),
    ChangedProofQuestion,
}

/// Check the exact constant-fold relation, not the producer's literal scan.
///
/// The fold is reconstructed from `before` alone: scanning reachable blocks in
/// reverse postorder meets every dominating definition first, so a literal
/// binding is recorded before each read. A goal-free scalar leaf whose
/// operands all resolve to literals — literal operation results and results
/// of leaves already folded under this same rule — rewrites in place to the
/// literal operation computing its denotation. The folded operation keeps its
/// identity, result declaration, and position; blocks, edges, parameters, and
/// every value identity are unchanged. Ranked machines carry ranking evidence
/// over execution positions and pass through unchanged.
///
/// A `before` module carrying reconstructed proof obligations admits only a
/// rewrite the question carries verbatim: the unchanged-question check above
/// is the refusal boundary, so a fold that leaves the question intact is
/// checked by the same fold relation while a fold that perturbs it fails
/// `ChangedProofQuestion` until proof-context transport is implemented.
pub fn validate_sparse_conditional_constant_propagation(
    before: &TerminalModule,
    after: &TerminalModule,
) -> Result<(), SparseConditionalConstantPropagationRewriteError> {
    use SparseConditionalConstantPropagationRewriteError as RewriteError;
    let before_valid =
        validate_module_for_optimization(before).map_err(RewriteError::InvalidModule)?;
    let after_valid =
        validate_module_for_optimization(after).map_err(RewriteError::InvalidModule)?;
    if before.machines.len() != after.machines.len() {
        return Err(RewriteError::ChangedProgramStructure);
    }
    let old_question = reconstruct_optimizable_terminal_obligations(before_valid)
        .map_err(RewriteError::InvalidModule)?;
    let new_question = reconstruct_optimizable_terminal_obligations(after_valid)
        .map_err(RewriteError::InvalidModule)?;
    if old_question != new_question {
        return Err(RewriteError::ChangedProofQuestion);
    }
    let mut expected = before.clone();
    for ((old, new), expected_machine) in before
        .machines
        .iter()
        .zip(&after.machines)
        .zip(&mut expected.machines)
    {
        if old.id != new.id || old.blocks.len() != new.blocks.len() {
            return Err(RewriteError::ChangedProgramStructure);
        }
        if old.ranked_scc.is_some() {
            if new != old {
                return Err(RewriteError::ChangedMachine(old.id));
            }
            continue;
        }
        let mut value_types = BTreeMap::new();
        for parameter in &old.parameters {
            value_types.insert(parameter.id, parameter.scalar_type);
        }
        if let Some(result) = old.result.scalar() {
            value_types.insert(result.id, result.scalar_type);
        }
        let mut block_positions = BTreeMap::new();
        for (position, block) in old.blocks.iter().enumerate() {
            block_positions.insert(block.id, position);
            for parameter in &block.parameters {
                value_types.insert(parameter.id, parameter.scalar_type);
            }
            for operation in &block.operations {
                if let Some(result) = operation.result.scalar() {
                    value_types.insert(result.id, result.scalar_type);
                }
            }
        }
        let mut literals: BTreeMap<ValueId, terminal_semantics::ScalarLeafLiteral> =
            BTreeMap::new();
        for block_id in crate::control_graph::reverse_postorder(old) {
            let position = block_positions
                .get(&block_id)
                .copied()
                .ok_or(RewriteError::ChangedProgramStructure)?;
            for (operation_position, operation) in
                old.blocks[position].operations.iter().enumerate()
            {
                let Some(result) = operation.result.scalar() else {
                    continue;
                };
                match &operation.kind {
                    terminal_psi::OperationKind::IntegerConstant { value } => {
                        literals.insert(
                            result.id,
                            terminal_semantics::ScalarLeafLiteral::Integer(*value),
                        );
                        continue;
                    }
                    terminal_psi::OperationKind::BooleanConstant { value } => {
                        literals.insert(
                            result.id,
                            terminal_semantics::ScalarLeafLiteral::Boolean(*value),
                        );
                        continue;
                    }
                    _ => {}
                }
                // A static reach binder position is semantic call evidence
                // carried by the operation row, never a fold candidate.
                if operation.static_reach_binding.is_some() {
                    continue;
                }
                let Some(literal) = terminal_semantics::constant_goal_free_scalar_leaf(
                    operation,
                    &literals,
                    &value_types,
                ) else {
                    continue;
                };
                literals.insert(result.id, literal);
                expected_machine.blocks[position].operations[operation_position].kind =
                    match literal {
                        terminal_semantics::ScalarLeafLiteral::Integer(value) => {
                            terminal_psi::OperationKind::IntegerConstant { value }
                        }
                        terminal_semantics::ScalarLeafLiteral::Boolean(value) => {
                            terminal_psi::OperationKind::BooleanConstant { value }
                        }
                    };
            }
        }
        if expected_machine != new {
            return Err(RewriteError::ChangedMachine(old.id));
        }
    }
    if &expected != after {
        return Err(RewriteError::ChangedProgramStructure);
    }
    Ok(())
}

/// The identity `value` keeps after all justified copies collapse: the unique
/// resolved source when every incoming edge binds the same resolved value, or
/// `value` itself when it is not a copy parameter or its bindings disagree.
/// A value still mid-resolution on the current cycle stands for itself.
fn resolve_copy_source(
    value: ValueId,
    raw: &BTreeMap<ValueId, Vec<ValueId>>,
    memo: &mut BTreeMap<ValueId, ValueId>,
) -> ValueId {
    resolve_copy_source_in(value, raw, memo, &mut std::collections::BTreeSet::new())
}

fn resolve_copy_source_in(
    value: ValueId,
    raw: &BTreeMap<ValueId, Vec<ValueId>>,
    memo: &mut BTreeMap<ValueId, ValueId>,
    visiting: &mut std::collections::BTreeSet<ValueId>,
) -> ValueId {
    let Some(arguments) = raw.get(&value) else {
        return value;
    };
    if let Some(resolved) = memo.get(&value) {
        return *resolved;
    }
    if !visiting.insert(value) {
        return value;
    }
    let mut resolved = arguments
        .iter()
        .map(|argument| resolve_copy_source_in(*argument, raw, memo, visiting));
    let first = resolved
        .next()
        .expect("an inventoried edge argument list is never empty");
    let result = if resolved.all(|other| other == first) {
        first
    } else {
        value
    };
    visiting.remove(&value);
    memo.insert(value, result);
    result
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlFlowCleanupRewriteError {
    InvalidModule(ModuleError),
    ChangedProgramStructure,
    ChangedMachine(MachineId),
    ChangedSurvivingBlock(BlockId),
    UnjustifiedFold(BlockId),
    RemovedReachableBlock(BlockId),
    RemovedEvidenceBlock(BlockId),
    RemovedReachableMachine(MachineId),
    RemovedEvidenceMachine(MachineId),
    ChangedProofQuestion,
}

/// Check the exact branch-cleanup relation, not the producer's folding order.
///
/// A rewritten terminator is justified only when the `before` conditional's
/// condition is the result of a `BooleanConstant` operation in `before`: the
/// selected successor edge — identity, scalar and structural bindings, and
/// edge-scoped cleanup rows — is carried verbatim onto an unconditional
/// `Jump` with an empty residual cleanup list, and the untaken successor
/// disappears with the conditional. Every other surviving block row is
/// identical; no block is added, reordered, or re-entered.
///
/// A removed block is justified only when it is unreachable under the
/// performed folds and carries nothing evidence retains: no static reach
/// binding, no structural result or parameter row whose machine-level place
/// declaration would orphan, and no identity the module's contract,
/// qualification, invariant, suspension, dispatch, call-evidence, or
/// projection carriers name. Removing any such row would leave evidence
/// pointing at structure the module no longer contains.
///
/// A `before` module carrying reconstructed proof obligations admits only a
/// rewrite the question carries verbatim: the unchanged-question check above
/// is the refusal boundary, so a fold that leaves the question intact is
/// checked by the same cleanup relation while a fold that perturbs it fails
/// `ChangedProofQuestion` until proof-context transport is implemented.
/// Ranked machines carry ranking evidence over exact control positions and
/// pass through unchanged.
///
/// The rewrite may additionally drop whole machines: `after` keeps
/// `before`'s machine order with a subsequence of its rows. A removed machine
/// is justified only when no retained transition reaches it — the entry
/// machine, every provider candidate, every attached machine, and every
/// machine a surviving module-level custody or evidence row names are roots,
/// and each retained machine keeps the machines its surviving blocks call,
/// dispatch, or clean up — and when no surviving evidence row names a block,
/// edge, operation, or value inside it. The check re-derives retention from
/// `after`: a machine reachable only through removed machines leaves, while
/// one named by a surviving row or a surviving machine's transition does not.
pub fn validate_control_flow_cleanup(
    before: &TerminalModule,
    after: &TerminalModule,
) -> Result<(), ControlFlowCleanupRewriteError> {
    use ControlFlowCleanupRewriteError as RewriteError;
    let before_valid =
        validate_module_for_optimization(before).map_err(RewriteError::InvalidModule)?;
    let after_valid =
        validate_module_for_optimization(after).map_err(RewriteError::InvalidModule)?;
    // Machines leave only by removal: `after` carries an ordered subsequence
    // of `before`'s rows, so a cursor match separates surviving pairs from
    // removed machines and rejects added or reordered rows at once.
    let mut surviving_pairs = Vec::new();
    let mut removed_machines = Vec::new();
    let mut incoming = after.machines.iter().peekable();
    for old in &before.machines {
        if incoming.peek().is_some_and(|new| new.id == old.id) {
            surviving_pairs.push((old, incoming.next().expect("peeked row")));
        } else {
            removed_machines.push(old);
        }
    }
    if incoming.next().is_some() {
        return Err(RewriteError::ChangedProgramStructure);
    }
    let old_question = reconstruct_optimizable_terminal_obligations(before_valid)
        .map_err(RewriteError::InvalidModule)?;
    let new_question = reconstruct_optimizable_terminal_obligations(after_valid)
        .map_err(RewriteError::InvalidModule)?;
    if old_question != new_question {
        return Err(RewriteError::ChangedProofQuestion);
    }
    let evidence = block_local_evidence(before);
    for (old, new) in surviving_pairs {
        if old.id != new.id {
            return Err(RewriteError::ChangedProgramStructure);
        }
        let mut non_blocks = new.clone();
        non_blocks.blocks.clone_from(&old.blocks);
        if &non_blocks != old {
            return Err(RewriteError::ChangedMachine(old.id));
        }
        if old.ranked_scc.is_some() {
            if new != old {
                return Err(RewriteError::ChangedMachine(old.id));
            }
            continue;
        }
        // `after` is already validated, so every surviving block's condition
        // still resolves: a literal producer inside a removed block can never
        // feed a surviving conditional.
        let mut literals = BTreeMap::new();
        for block in &old.blocks {
            for operation in &block.operations {
                if let terminal_psi::OperationKind::BooleanConstant { value } = &operation.kind
                    && let Some(result) = operation.result.scalar()
                {
                    literals.insert(result.id, *value);
                }
            }
        }
        let mut folded = BTreeMap::new();
        let mut old_blocks = old.blocks.iter();
        for new_block in &new.blocks {
            let Some(old_block) = old_blocks.find(|block| block.id == new_block.id) else {
                return Err(RewriteError::ChangedProgramStructure);
            };
            if new_block == old_block {
                continue;
            }
            if new_block.parameters != old_block.parameters
                || new_block.structural_parameters != old_block.structural_parameters
                || new_block.operations != old_block.operations
            {
                return Err(RewriteError::ChangedSurvivingBlock(old_block.id));
            }
            let Terminator::Conditional {
                condition,
                when_true,
                when_false,
            } = &old_block.terminator
            else {
                return Err(RewriteError::UnjustifiedFold(old_block.id));
            };
            let taken = match literals.get(condition) {
                Some(true) => when_true,
                Some(false) => when_false,
                None => return Err(RewriteError::UnjustifiedFold(old_block.id)),
            };
            let expected = Terminator::Jump {
                edge: taken.edge,
                target: taken.target,
                arguments: taken.arguments.clone(),
                structural_arguments: taken.structural_arguments.clone(),
                trivial_affine_discards: taken.trivial_affine_discards.clone(),
                residual_affine_discards: Vec::new(),
            };
            if new_block.terminator != expected {
                return Err(RewriteError::UnjustifiedFold(old_block.id));
            }
            folded.insert(old_block.id, new_block.terminator.clone());
        }
        let mut simulated = old.clone();
        for block in &mut simulated.blocks {
            if let Some(terminator) = folded.get(&block.id) {
                block.terminator = terminator.clone();
            }
        }
        let outgoing = crate::control_graph::successors(&simulated);
        let mut reachable = BTreeSet::from([simulated.entry]);
        let mut pending = vec![simulated.entry];
        while let Some(block) = pending.pop() {
            for (_, target) in &outgoing[&block] {
                if reachable.insert(*target) {
                    pending.push(*target);
                }
            }
        }
        let surviving: BTreeSet<BlockId> = new.blocks.iter().map(|block| block.id).collect();
        for old_block in &old.blocks {
            if surviving.contains(&old_block.id) {
                continue;
            }
            if reachable.contains(&old_block.id) {
                return Err(RewriteError::RemovedReachableBlock(old_block.id));
            }
            if evidence_bound_block(old_block, &evidence) {
                return Err(RewriteError::RemovedEvidenceBlock(old_block.id));
            }
        }
    }
    if !removed_machines.is_empty() {
        // Retention is re-derived over `after`: only surviving rows and
        // surviving machines can keep a machine alive, and their own
        // transitions then close the set. A machine a removed sibling still
        // named is dead with it; a machine any surviving row or reachable
        // call, dispatch, or cleanup transition names is not.
        let retained = retained_machines(after);
        let surviving_evidence = block_local_evidence(after);
        for machine in &removed_machines {
            if retained.contains(&machine.id) {
                return Err(RewriteError::RemovedReachableMachine(machine.id));
            }
            if machine_evidence_bound(machine, &surviving_evidence) {
                return Err(RewriteError::RemovedEvidenceMachine(machine.id));
            }
        }
    }
    let mut non_machines = after.clone();
    non_machines.machines.clone_from(&before.machines);
    if &non_machines != before {
        return Err(RewriteError::ChangedProgramStructure);
    }
    Ok(())
}

/// Whether dropping `block` would erase a row module-level evidence still
/// names, or orphan a machine-level structural place declaration rooted at
/// this block's parameters or operation results.
fn evidence_bound_block(block: &terminal_psi::Block, evidence: &BlockLocalEvidence) -> bool {
    if evidence.blocks.contains(&block.id) || !block.structural_parameters.is_empty() {
        return true;
    }
    for operation in &block.operations {
        if operation.static_reach_binding.is_some()
            || evidence.operations.contains(&operation.id)
            || matches!(
                operation.result,
                terminal_psi::OperationResult::Structural(_)
            )
        {
            return true;
        }
        if operation
            .result
            .scalar()
            .is_some_and(|result| evidence.values.contains(&result.id))
        {
            return true;
        }
    }
    if block
        .parameters
        .iter()
        .any(|parameter| evidence.values.contains(&parameter.id))
    {
        return true;
    }
    block
        .terminator
        .edges()
        .any(|edge| evidence.edges.contains(&edge))
}

/// Machines surviving rows keep alive independent of intra-machine
/// reachability: the module entry, every attached or ranked machine, every
/// provider candidate, and every machine a module-level custody or evidence
/// row names are roots, and each retained machine then keeps the machines
/// its call, selected-evidence, cleanup, and closed-reach transitions name.
pub fn retained_machines(module: &TerminalModule) -> BTreeSet<MachineId> {
    let mut retained = machine_retention_roots(module);
    let by_id: BTreeMap<MachineId, &terminal_psi::TerminalMachine> = module
        .machines
        .iter()
        .map(|machine| (machine.id, machine))
        .collect();
    let mut frontier: Vec<MachineId> = retained.iter().copied().collect();
    while let Some(machine) = frontier.pop() {
        let Some(body) = by_id.get(&machine) else {
            // A row may name a machine outside the module's own machine
            // table; that root still forbids removal but closes over nothing.
            continue;
        };
        for target in machine_transitions(body) {
            if retained.insert(target) {
                frontier.push(target);
            }
        }
    }
    retained
}

/// Every machine identity a module-level row names is a retention root:
/// coercions, invariants, suspensions, proof outputs, conformance
/// applications, dynamic-dispatch custody, reborrow publications, placed
/// views, float projections, evidence lanes, providers, and attached
/// machines each keep their named machine regardless of call reachability.
fn machine_retention_roots(module: &TerminalModule) -> BTreeSet<MachineId> {
    let mut roots = BTreeSet::new();
    roots.insert(module.entry);
    for coercion in &module.scalar_qualifications.coercions {
        roots.insert(coercion.machine);
    }
    for invariant in &module.scalar_block_invariants {
        roots.insert(invariant.machine);
    }
    for site in &module.suspension_call_sites {
        if let terminal_psi::TerminalSuspensionCallTarget::Machine(machine) = site.target {
            roots.insert(machine);
        }
    }
    for plan in &module.suspension_call_plans {
        if let terminal_psi::TerminalSuspensionCallTarget::Machine(machine) = plan.target {
            roots.insert(machine);
        }
    }
    for call in &module.proof_output_calls {
        roots.insert(call.caller);
        if let Some(runtime) = &call.runtime_call {
            roots.insert(runtime.callee);
        }
        if let Some(dispatch) = &call.static_requirement_dispatch {
            roots.insert(dispatch.realization);
        }
    }
    for application in &module.closed_conformance_applications {
        roots.insert(application.owner);
        for callable in &application.realization_callables {
            roots.insert(callable.machine);
        }
    }
    let dispatch = &module.dynamic_dispatch;
    for owner in dispatch
        .parameters
        .iter()
        .map(|row| row.owner)
        .chain(dispatch.arguments.iter().map(|row| row.owner))
        .chain(dispatch.selections.iter().map(|row| row.owner))
        .chain(dispatch.rebound_descriptors.iter().map(|row| row.owner))
        .chain(dispatch.stored_descriptors.iter().map(|row| row.owner))
        .chain(dispatch.direct_dispatches.iter().map(|row| row.owner))
        .chain(dispatch.indirect_dispatches.iter().map(|row| row.owner))
        .chain(dispatch.stored_dispatches.iter().map(|row| row.owner))
        .chain(dispatch.parameter_dispatches.iter().map(|row| row.owner))
    {
        roots.insert(owner);
    }
    for realization in dispatch
        .direct_dispatches
        .iter()
        .map(|row| row.realization)
        .chain(
            dispatch
                .indirect_dispatches
                .iter()
                .map(|row| row.realization),
        )
        .chain(dispatch.stored_dispatches.iter().map(|row| row.realization))
    {
        roots.insert(realization);
    }
    for handoff in &module.reborrow_root_handoffs {
        roots.insert(handoff.machine);
    }
    for use_row in &module.reborrow_restored_call_uses {
        roots.insert(use_row.machine);
        roots.insert(use_row.call_target_machine);
    }
    for input in &module.placed_view_inputs {
        roots.insert(input.machine);
    }
    for lane in &module.evidence_contract_lanes {
        roots.insert(lane.machine);
    }
    for candidate in &module.provider_candidates {
        roots.insert(candidate.candidate);
    }
    for projection in &module.float_meaning_projections {
        match &projection.source {
            terminal_psi::FloatMeaningSource::DirectMachineParameter(row) => {
                roots.insert(row.owner);
            }
            terminal_psi::FloatMeaningSource::DirectMachineResult(row) => {
                roots.insert(row.owner);
            }
            terminal_psi::FloatMeaningSource::DirectBlockParameter(row) => {
                roots.insert(row.owner);
            }
            terminal_psi::FloatMeaningSource::DirectOperationResult(row) => {
                roots.insert(row.owner);
            }
            terminal_psi::FloatMeaningSource::DirectCallResult(row) => {
                roots.insert(row.owner);
            }
            terminal_psi::FloatMeaningSource::DirectStructuralLeaf(row) => {
                roots.insert(row.owner);
            }
            terminal_psi::FloatMeaningSource::TransitionalInput(_)
            | terminal_psi::FloatMeaningSource::ExactBinary32Literal(_)
            | terminal_psi::FloatMeaningSource::ExactBinary64Literal(_) => {}
        }
    }
    for machine in &module.machines {
        // Attached machines are reachable through nominal-type custody rather
        // than call edges, and ranked machines carry execution-position
        // evidence that must survive: both are unconditional roots.
        if machine.attachment.is_some() || machine.ranked_scc.is_some() {
            roots.insert(machine.id);
        }
    }
    roots
}

/// Machines a retained machine keeps alive through its surviving contents:
/// every direct call callee, each selected-evidence use target, each nominal
/// cleanup machine, and each callee the closed reach application selects.
/// Blocks already removed by the rewrite no longer carry their callees.
fn machine_transitions(machine: &terminal_psi::TerminalMachine) -> Vec<MachineId> {
    let mut targets = Vec::new();
    for block in &machine.blocks {
        for operation in &block.operations {
            match &operation.kind {
                terminal_psi::OperationKind::Call { callee, .. }
                | terminal_psi::OperationKind::CallUnit { callee, .. }
                | terminal_psi::OperationKind::CallStructuralScalar { callee, .. }
                | terminal_psi::OperationKind::CallStructuralWithScalarArguments {
                    callee, ..
                } => targets.push(*callee),
                terminal_psi::OperationKind::CallStructural {
                    callee,
                    selected_evidence,
                    ..
                } => {
                    targets.push(*callee);
                    for evidence in selected_evidence {
                        for evidence_use in &evidence.uses {
                            targets.push(evidence_use.target);
                        }
                    }
                }
                _ => {}
            }
        }
        match &block.terminator {
            terminal_psi::Terminator::Return {
                cleanup_actions, ..
            } => {
                for action in cleanup_actions {
                    if let terminal_psi::TerminalAffineCleanupAction::InvokeNominal(cleanup) =
                        action
                    {
                        targets.push(cleanup.cleanup_machine);
                    }
                }
            }
            terminal_psi::Terminator::ReturnUnitNominalAffine { cleanups, .. } => {
                for cleanup in cleanups {
                    targets.push(cleanup.cleanup_machine);
                }
            }
            _ => {}
        }
    }
    if let Some(application) = &machine.closed_reach_application {
        for parameter in &application.telescope {
            if let terminal_psi::ClosedReachParameter::Machine(binding) = parameter
                && let Some(callee) = binding.callee
            {
                targets.push(callee);
            }
        }
        for call in &application.calls {
            if let Some(application) = &call.application {
                targets.push(application.callee);
            }
        }
    }
    targets
}

/// Whether dropping `machine` would erase a block, edge, operation, or value
/// surviving evidence still names, including the machine's own scalar
/// parameters and declared scalar result. Machine-local declarations such as
/// structural places and reach bindings die with the machine and do not pin
/// it; only identities a surviving row can still name matter here.
pub fn machine_evidence_bound(
    machine: &terminal_psi::TerminalMachine,
    evidence: &BlockLocalEvidence,
) -> bool {
    if machine
        .parameters
        .iter()
        .any(|parameter| evidence.values.contains(&parameter.id))
        || machine
            .result
            .scalar_ref()
            .is_some_and(|result| evidence.values.contains(&result.id))
    {
        return true;
    }
    for block in &machine.blocks {
        if evidence.blocks.contains(&block.id)
            || block
                .parameters
                .iter()
                .any(|parameter| evidence.values.contains(&parameter.id))
            || block
                .terminator
                .edges()
                .any(|edge| evidence.edges.contains(&edge))
        {
            return true;
        }
        for operation in &block.operations {
            if evidence.operations.contains(&operation.id)
                || operation
                    .result
                    .scalar_ref()
                    .is_some_and(|result| evidence.values.contains(&result.id))
            {
                return true;
            }
        }
    }
    false
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofCheckElisionRewriteError {
    InvalidModule(ModuleError),
    ChangedProgramStructure,
    ChangedMachine(MachineId),
    ChangedProofQuestion,
    ChangedProofEvidence,
}

/// Check the exact discharged-goal rewrite, not the producer's literal scan.
///
/// The elision set is re-derived from `before` alone: scanning reachable
/// blocks in reverse postorder meets every dominating literal definition
/// first, so each proof-bearing scalar leaf's canonical goal is decided under
/// the literal bindings its predecessors actually provide. A discharged leaf
/// rewrites in place to the goal-free leaf the shared semantic judgment
/// selects, keeping its identity, result declaration, and position; a decided
/// leaf with no goal-free form stays checked. Ranked machines carry ranking
/// evidence over exact execution positions and pass through unchanged, and a
/// leaf whose obligation a recursive or control-cycle certificate names keeps
/// its question.
///
/// The reconstructed proof question changes exactly at the consumed rows: the
/// discharged operation's obligation disappears and every surviving row's
/// semantic axioms substitute the original result equation with the
/// replacement's — the same element in the same position. The proof bundle's
/// evidence section loses the consumed obligations' rows; recursive
/// certificates, control-cycle certificates, and producer provenance are
/// unchanged.
pub fn validate_proof_check_elision(
    before: &TerminalModule,
    after: &TerminalModule,
    before_bundle: &ProofBundle,
    after_bundle: &ProofBundle,
) -> Result<(), ProofCheckElisionRewriteError> {
    use ProofCheckElisionRewriteError as RewriteError;
    let before_valid =
        validate_module_for_optimization(before).map_err(RewriteError::InvalidModule)?;
    let after_valid =
        validate_module_for_optimization(after).map_err(RewriteError::InvalidModule)?;
    if before.machines.len() != after.machines.len() {
        return Err(RewriteError::ChangedProgramStructure);
    }
    // Obligation identities a recursive or control-cycle certificate names are
    // consumed by that certificate: they cannot disappear with an operation.
    let mut reserved = BTreeSet::new();
    for certificate in before_bundle
        .recursive_components
        .iter()
        .map(|row| &row.certificate)
        .chain(
            before_bundle
                .control_cycles
                .iter()
                .map(|row| &row.certificate),
        )
    {
        for edge in &certificate.edges {
            reserved.insert(edge.obligation);
        }
    }
    let mut expected = before.clone();
    // Each elided leaf's original result equation maps to the replacement's.
    let mut substitutions: BTreeMap<Proposition, Proposition> = BTreeMap::new();
    let mut elided: BTreeMap<(MachineId, OperationId), ObligationId> = BTreeMap::new();
    let mut elided_obligations = BTreeSet::new();
    for ((old, new), expected_machine) in before
        .machines
        .iter()
        .zip(&after.machines)
        .zip(&mut expected.machines)
    {
        if old.id != new.id || old.blocks.len() != new.blocks.len() {
            return Err(RewriteError::ChangedProgramStructure);
        }
        if old.ranked_scc.is_some() {
            if new != old {
                return Err(RewriteError::ChangedMachine(old.id));
            }
            continue;
        }
        let mut value_types = BTreeMap::new();
        for parameter in &old.parameters {
            value_types.insert(parameter.id, parameter.scalar_type);
        }
        if let Some(result) = old.result.scalar() {
            value_types.insert(result.id, result.scalar_type);
        }
        let mut block_positions = BTreeMap::new();
        for (position, block) in old.blocks.iter().enumerate() {
            block_positions.insert(block.id, position);
            for parameter in &block.parameters {
                value_types.insert(parameter.id, parameter.scalar_type);
            }
            for operation in &block.operations {
                if let Some(result) = operation.result.scalar() {
                    value_types.insert(result.id, result.scalar_type);
                }
            }
        }
        let mut literals: BTreeMap<ValueId, terminal_semantics::ScalarLeafLiteral> =
            BTreeMap::new();
        for block_id in crate::control_graph::reverse_postorder(old) {
            let position = block_positions
                .get(&block_id)
                .copied()
                .ok_or(RewriteError::ChangedProgramStructure)?;
            for (operation_position, operation) in
                old.blocks[position].operations.iter().enumerate()
            {
                let Some(result) = operation.result.scalar() else {
                    continue;
                };
                match &operation.kind {
                    terminal_psi::OperationKind::IntegerConstant { value } => {
                        literals.insert(
                            result.id,
                            terminal_semantics::ScalarLeafLiteral::Integer(*value),
                        );
                        continue;
                    }
                    terminal_psi::OperationKind::BooleanConstant { value } => {
                        literals.insert(
                            result.id,
                            terminal_semantics::ScalarLeafLiteral::Boolean(*value),
                        );
                        continue;
                    }
                    _ => {}
                }
                // A static reach binder position is semantic call evidence
                // carried by the operation row, never an elision candidate.
                if operation.static_reach_binding.is_some() {
                    continue;
                }
                let Some(elision) = terminal_semantics::elidable_proof_bearing_scalar_leaf(
                    operation,
                    &literals,
                    &value_types,
                )
                .map_err(|error| {
                    RewriteError::InvalidModule(ModuleError::OperationSemanticSchema(error))
                })?
                else {
                    continue;
                };
                if reserved.contains(&elision.obligation()) {
                    continue;
                }
                let expected_operation =
                    &mut expected_machine.blocks[position].operations[operation_position];
                expected_operation.kind = elision.replacement().clone();
                let replacement_equation = terminal_semantics::goal_free_scalar_leaf_semantics(
                    expected_operation,
                    &value_types,
                )
                .map_err(|error| {
                    RewriteError::InvalidModule(ModuleError::OperationSemanticSchema(error))
                })?
                .ok_or(RewriteError::ChangedMachine(old.id))?
                .result_equation()
                .clone();
                substitutions.insert(
                    elision.semantics().result_equation().clone(),
                    replacement_equation,
                );
                elided.insert((old.id, operation.id), elision.obligation());
                elided_obligations.insert(elision.obligation());
                if let Some(literal) = elision.result_literal() {
                    literals.insert(
                        result.id,
                        terminal_semantics::ScalarLeafLiteral::Integer(literal),
                    );
                }
            }
        }
        if expected_machine != new {
            return Err(RewriteError::ChangedMachine(old.id));
        }
    }
    if &expected != after {
        return Err(RewriteError::ChangedProgramStructure);
    }
    // The complete reconstructed question changes only at consumed rows: an
    // elided leaf's obligation disappears and its result equation is
    // substituted inside every surviving row's axiom snapshot at the exact
    // position reconstruction places it.
    let old_question = reconstruct_optimizable_terminal_obligations(before_valid)
        .map_err(RewriteError::InvalidModule)?;
    let new_question = reconstruct_optimizable_terminal_obligations(after_valid)
        .map_err(RewriteError::InvalidModule)?;
    let mut remaining = new_question.obligations().iter();
    for old_row in old_question.obligations() {
        if let crate::ReconstructedTerminalObligationOwner::Operation { machine, operation } =
            old_row.owner
            && let Some(obligation) = elided.get(&(machine, operation))
        {
            if old_row.obligation.id != *obligation {
                return Err(RewriteError::ChangedProofQuestion);
            }
            continue;
        }
        let Some(new_row) = remaining.next() else {
            return Err(RewriteError::ChangedProofQuestion);
        };
        if new_row.owner != old_row.owner
            || new_row.obligation != old_row.obligation
            || new_row.requirements != old_row.requirements
            || new_row.canonical_certificate != old_row.canonical_certificate
            || new_row.semantic_axioms.len() != old_row.semantic_axioms.len()
        {
            return Err(RewriteError::ChangedProofQuestion);
        }
        for (old_axiom, new_axiom) in old_row
            .semantic_axioms
            .iter()
            .zip(new_row.semantic_axioms.iter())
        {
            if new_axiom == old_axiom {
                continue;
            }
            if substitutions.get(old_axiom) != Some(new_axiom) {
                return Err(RewriteError::ChangedProofQuestion);
            }
        }
    }
    if remaining.next().is_some() {
        return Err(RewriteError::ChangedProofQuestion);
    }
    // The consumed obligations' evidence rows leave with them; every other
    // bundle section is byte-identical.
    let mut expected_bundle = before_bundle.clone();
    expected_bundle
        .evidence
        .retain(|row| !elided_obligations.contains(&row.obligation));
    if after_bundle != &expected_bundle {
        return Err(RewriteError::ChangedProofEvidence);
    }
    Ok(())
}
