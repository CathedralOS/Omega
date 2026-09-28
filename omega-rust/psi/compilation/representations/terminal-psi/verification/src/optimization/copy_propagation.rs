//! Validation of copy propagation rewrites and copy source resolution.

use crate::optimization::dead_scalar_elimination::drop_removed;
use crate::{
    ModuleError, reconstruct_optimizable_crash_obligations,
    reconstruct_optimizable_terminal_obligations, validate_module_for_optimization,
};
use semantic_vocabulary::{BlockId, MachineId, ValueId};
use std::collections::BTreeMap;
use terminal_psi::{TerminalModule, Terminator};

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
    // The crash-obligation roster is a second reconstructed question: a
    // rewrite that leaves every terminal obligation intact may still retarget
    // a crash site's reconstructed paths or a continuation's coverage goal,
    // and the supplied certificates answer only the questions they were
    // produced against.
    let old_crash = reconstruct_optimizable_crash_obligations(before_valid)
        .map_err(CopyPropagationRewriteError::InvalidModule)?;
    let new_crash = reconstruct_optimizable_crash_obligations(after_valid)
        .map_err(CopyPropagationRewriteError::InvalidModule)?;
    if old_crash != new_crash {
        return Err(CopyPropagationRewriteError::ChangedProofQuestion);
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
