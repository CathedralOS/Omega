//! An exclusive loan formed on one branch still suspends its backing at a merge.

use super::address_joins::validate_address_join_ownership;
use crate::tests::structural_cases::source_machine_unit;
use crate::tests::support::refresh_node_derivatives;
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use abstract_operations::AbstractOperation;
use semantic_vocabulary::{OperationId, ValueId};
use terminal_psi::StructuralAccess;

#[test]
fn an_exclusive_join_on_one_diamond_arm_pins_its_owner_at_the_merge() {
    let source = "data Score { value: u64; }
        machine run(borrow: bool) -> u64 {
            let mut score: Score;
            score.value = 70;
            transition borrow {
                true -> inspect(&mut score)
                false -> finish(70)
            }
            state inspect(score: &mut Score) -> u64 {
                let code: u64 = score.value;
                score.value = 71;
                transition { _ -> finish(code) }
            }
            state finish(code: u64) -> u64 {
                transition { _ -> (code) }
            }
        }";
    let mut candidate = source_machine_unit(source, "run", true);
    validate_psi_optimization_unit(&candidate).expect("the source loan ends with its cleanup");
    let function = &candidate.functions[0];
    let (loan_block, loan) = function
        .blocks
        .iter()
        .find_map(|block| {
            block
                .structural_parameters
                .iter()
                .find(|parameter| parameter.access == StructuralAccess::MutableBorrow)
                .map(|parameter| (block.id, parameter.place))
        })
        .expect("the true arm receives one exclusive record loan");
    let owner = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
        .flat_map(|edge| &edge.structural_bindings)
        .find(|binding| binding.parameter == loan)
        .unwrap()
        .argument
        .place;
    let merge = function
        .blocks
        .iter()
        .position(|block| {
            block
                .nodes
                .iter()
                .any(|node| matches!(node.operation, AbstractOperation::Return { .. }))
        })
        .expect("both arms return through finish");
    let merge_id = function.blocks[merge].id;
    let dominators =
        crate::candidates::global_value_numbering::independent_reachable_dominators(function);
    assert!(
        !dominators[&merge_id].contains(&loan_block),
        "the exclusive arm cannot dominate the merge"
    );
    let mut stale_read = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find(|node| {
            matches!(
                node.operation,
                AbstractOperation::StructuralScalarFieldRead { .. }
            )
        })
        .expect("the source observes its only record field through the current loan spelling")
        .clone();
    if let AbstractOperation::StructuralScalarFieldRead {
        psi_operation,
        result,
        source,
        ..
    } = &mut stale_read.operation
    {
        *source = owner;
        *psi_operation = OperationId::new(9_000_001).unwrap();
        result.value = ValueId::new(9_000_001).unwrap();
    }

    // Forge retention of the owned home on both branches instead of ending its
    // storage on the authored exits. The new read then names a live home, not a
    // missing place: only exclusive loan replay can reject its observation.
    for block in &mut candidate.functions[0].blocks {
        for node in &mut block.nodes {
            match &mut node.operation {
                AbstractOperation::Jump {
                    trivial_affine_discards,
                    ..
                } => {
                    trivial_affine_discards.retain(|place| *place != owner);
                }
                AbstractOperation::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    when_true
                        .trivial_affine_discards
                        .retain(|place| *place != owner);
                    when_false
                        .trivial_affine_discards
                        .retain(|place| *place != owner);
                }
                _ => {}
            }
        }
    }
    candidate.functions[0].blocks[merge]
        .nodes
        .insert(0, stale_read);
    for block in 0..candidate.functions[0].blocks.len() {
        for node in 0..candidate.functions[0].blocks[block].nodes.len() {
            refresh_node_derivatives(&mut candidate, 0, block, node);
        }
    }
    let result = validate_address_join_ownership(&candidate);
    assert!(
        matches!(
            result,
            Err(OptimizationUnitValidationError::CurrentSharedJoinOriginDisturbed { block, place, .. })
                if block == merge_id && place == owner
        ),
        "the possible exclusive loan must pin its backing beyond dominance: {result:?}"
    );

    // Isolate ownership replay from operation-specific non-observation checks:
    // attenuating this exclusive loan cannot unpin the forged backing read.
    // Full validation independently rejects observing through the loan itself.
    for block in &mut candidate.functions[0].blocks {
        for parameter in &mut block.structural_parameters {
            if parameter.place == loan {
                parameter.access = StructuralAccess::WriteOnlyBorrow;
            }
        }
        for node in &mut block.nodes {
            match &mut node.operation {
                AbstractOperation::Jump {
                    structural_bindings,
                    ..
                } => {
                    for binding in structural_bindings {
                        if binding.parameter == loan {
                            binding.argument.access = StructuralAccess::WriteOnlyBorrow;
                        }
                    }
                }
                AbstractOperation::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    for edge in [when_true, when_false] {
                        for binding in &mut edge.structural_bindings {
                            if binding.parameter == loan {
                                binding.argument.access = StructuralAccess::WriteOnlyBorrow;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    for block in 0..candidate.functions[0].blocks.len() {
        for node in 0..candidate.functions[0].blocks[block].nodes.len() {
            refresh_node_derivatives(&mut candidate, 0, block, node);
        }
    }
    let result = validate_address_join_ownership(&candidate);
    assert!(
        matches!(result,
        Err(OptimizationUnitValidationError::CurrentSharedJoinOriginDisturbed { block, place, .. })
            if block == merge_id && place == owner),
        "write-only attenuation must not release exclusive backing: {result:?}"
    );
}
