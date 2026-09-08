//! Input-only block layout. Backedges are layout edges, not ranking evidence.
use super::SelectedInstructionError;
use legalized_operations::{LegalizedScalarFunction, LegalizedScalarTerminator};

pub(super) fn derive(
    source: &LegalizedScalarFunction,
) -> Result<Vec<usize>, SelectedInstructionError> {
    let invalid = SelectedInstructionError::SourceCustodyMismatch;
    let entry = source
        .blocks
        .iter()
        .position(|block| block.id == source.entry_block)
        .ok_or(invalid.clone())?;
    let mut outgoing = Vec::new();
    for block in &source.blocks {
        let targets = match &block.terminator {
            LegalizedScalarTerminator::Return(_) => Vec::new(),
            LegalizedScalarTerminator::Jump { successor, .. } => vec![successor.target],
            LegalizedScalarTerminator::Conditional {
                when_true,
                when_false,
                ..
            } => vec![when_true.target, when_false.target],
            LegalizedScalarTerminator::StructuralCase { cases, .. } => {
                cases.iter().map(|case| case.target).collect()
            }
        };
        outgoing.push(
            targets
                .into_iter()
                .map(|target| {
                    source
                        .blocks
                        .iter()
                        .position(|candidate| candidate.id == target)
                        .ok_or(invalid.clone())
                })
                .collect::<Result<Vec<_>, _>>()?,
        );
    }
    // Iterative reverse postorder puts every dominating definition before its
    // uses, including when a predecessor is a backedge. No execution fuel is set.
    let mut visited = vec![false; source.blocks.len()];
    let mut pending = vec![(entry, false)];
    let mut postorder = Vec::new();
    while let Some((block, expanded)) = pending.pop() {
        if expanded {
            postorder.push(block);
            continue;
        }
        if visited[block] {
            continue;
        }
        visited[block] = true;
        pending.push((block, true));
        pending.extend(outgoing[block].iter().rev().map(|target| (*target, false)));
    }
    if postorder.len() != source.blocks.len() {
        return Err(invalid);
    }
    postorder.reverse();
    let mut order = vec![entry];
    if source.ranked.is_some() {
        order.extend((0..source.blocks.len()).filter(|index| *index != entry));
        return Ok(order);
    }
    while order.len() < source.blocks.len() {
        // Keep the established layout for acyclic inputs. At a cycle, use the
        // next reverse-postorder block instead of waiting for its own backedge.
        let next = (0..source.blocks.len())
            .find(|index| {
                !order.contains(index)
                    && outgoing.iter().enumerate().all(|(predecessor, targets)| {
                        !targets.contains(index) || order.contains(&predecessor)
                    })
            })
            .or_else(|| {
                postorder
                    .iter()
                    .copied()
                    .find(|index| !order.contains(index))
            })
            .ok_or(invalid.clone())?;
        order.push(next);
    }
    Ok(order)
}

#[cfg(test)]
mod tests {
    use super::*;
    use legalized_operations::{
        LegalizedScalarInstructionKind, LegalizedScalarSuccessor, LegalizedStructuralCaseSuccessor,
    };
    use optimization_unit::EffectLink;
    use semantic_vocabulary::{BlockId, EdgeId, StructuralCaseId};

    #[test]
    fn case_targets_precede_their_join_in_a_permuted_roster() {
        let (abstracted, native, unit) =
            crate::tests::legalization::byte_input::fixture(target::NativeTarget::linux_x64());
        let legal = crate::legalize_target_operations(&native, &abstracted, &unit).unwrap();
        // Exercise layout on raw graph data, not a forged admission receipt.
        let mut source = legal.plan().scalar_functions[0].clone();
        let operation = source.blocks[0].instructions[0].operation;
        let LegalizedScalarInstructionKind::HostedReadByte { result, layout, .. } =
            source.blocks[0].instructions[0].kind.clone()
        else {
            unreachable!()
        };
        let mut join = source.blocks[0].clone();
        join.id = BlockId::new(4).unwrap();
        join.instructions.clear();
        let arm = |identity, edge| {
            let mut block = join.clone();
            block.id = BlockId::new(identity).unwrap();
            block.terminator = LegalizedScalarTerminator::Jump {
                successor: LegalizedScalarSuccessor {
                    edge: EdgeId::new(edge).unwrap(),
                    target: join.id,
                    bindings: Vec::new(),
                    structural_bindings: Vec::new(),
                    fuel: Vec::new(),
                },
                effect: EffectLink {
                    input: 0,
                    output: 0,
                },
                ownership: Vec::new(),
            };
            block
        };
        let empty = arm(2, 12);
        let present = arm(3, 13);
        source.blocks[0].terminator = LegalizedScalarTerminator::StructuralCase {
            defining_operation: operation,
            result,
            layout,
            cases: [empty.id, present.id]
                .into_iter()
                .enumerate()
                .map(|(position, target)| LegalizedStructuralCaseSuccessor {
                    edge: EdgeId::new(10 + position as u64).unwrap(),
                    target,
                    case: StructuralCaseId::new(1 + position as u64).unwrap(),
                    case_tag: position as i32,
                    payloads: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                    fuel: Vec::new(),
                })
                .collect(),
            effect: EffectLink {
                input: 0,
                output: 0,
            },
            ownership: Vec::new(),
        };
        source.blocks.extend([empty, join, present]);
        assert_eq!(derive(&source).unwrap(), [0, 1, 3, 2]);

        let LegalizedScalarTerminator::StructuralCase { cases, .. } =
            &mut source.blocks[0].terminator
        else {
            unreachable!()
        };
        cases[1].target = BlockId::new(99).unwrap();
        assert!(derive(&source).is_err());
    }
}
