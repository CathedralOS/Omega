//! Select edge-owned calls with ordinary borrowed-pointer ABI transport.
use super::{
    Builder, SelectedInstructionId, SelectedInstructionKind, SelectedInstructionProvenance,
};
use crate::SelectedInstructionError;
use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use legalized_operations::{LegalizedScalarArgument, LegalizedScalarCall};
use optimization_unit::EffectLink;
use selected_instructions::SelectedCallContract;
use target_operations::{CallSiteOwner, NativeCallOrigin};
use terminal_psi::StructuralAccess;

pub(super) fn emit(
    owner: CallSiteOwner,
    call: &LegalizedScalarCall,
    effect: EffectLink,
    builder: &mut Builder<'_>,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<(), SelectedInstructionError> {
    let invalid = SelectedInstructionError::custody;
    let edge = owner.edge().ok_or_else(invalid)?;
    if call.source != NativeCallOrigin::Authored
        || call.result_placement.is_some()
        || call.structural_result.is_some()
        || !call.claim_transfers.is_empty()
        || !call.requirement_obligations.is_empty()
        || !call.crash_continuations.is_empty()
        || call.arguments.len() > 1
    {
        return Err(invalid());
    }
    let mut shapes = Vec::new();
    for argument in &call.arguments {
        let LegalizedScalarArgument::Structural {
            semantic,
            target,
            indices,
        } = argument
        else {
            return Err(invalid());
        };
        if !matches!(
            semantic.access,
            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
        ) || target.access != semantic.access
            || !semantic.path.is_empty()
            || !target.path.is_empty()
            || !indices.is_empty()
            || target.source_byte_offset != 0
            || target.shape != ValueShape::borrowed_reference(8, 8)
            || target.destination.shape != target.shape
        {
            return Err(invalid());
        }
        shapes.push(target.shape);
    }
    let expected = evaluate_call_plan(
        CallingPolicy::native_for_target(environment.target()),
        &CallSignature {
            parameters: shapes,
            result: None,
        },
    )
    .map_err(|_| invalid())?;
    if expected != call.call_plan
        || call
            .arguments
            .iter()
            .zip(&expected.parameters)
            .any(|(argument, placement)| argument.placement() != placement)
    {
        return Err(invalid());
    }
    let key = crate::selection::scalar_call_abi::unit_key(call, environment).ok_or_else(invalid)?;
    let provenance = SelectedInstructionProvenance {
        edges: vec![edge],
        ..Default::default()
    };
    let mut operands = Vec::new();
    for argument in &call.arguments {
        let LegalizedScalarArgument::Structural { semantic, .. } = argument else {
            return Err(invalid());
        };
        operands.push(super::structural::place_pointer_at_site(
            builder,
            provenance.clone(),
            semantic.place,
            0,
        )?);
    }
    builder.transport.calls.push(SelectedCallContract {
        instruction: SelectedInstructionId(
            builder
                .instructions
                .len()
                .try_into()
                .map_err(|_| invalid())?,
        ),
        owner,
        call: call.clone(),
        effect,
        ownership: Vec::new(),
    });
    builder.emit(
        SelectedInstructionKind::CallUnit {
            callee: call.callee,
        },
        key,
        &operands,
        provenance,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use calling_conventions::{CallSignature, CallingPolicy, evaluate_call_plan};
    use legalized_operations::{
        LegalizedScalarBlock, LegalizedScalarCall, LegalizedScalarFunction, LegalizedScalarReturn,
        LegalizedScalarReturnValue, LegalizedScalarTerminator,
    };
    use optimization_unit::{EffectLink, FuelSettlement, PsiProvenance};
    use selected_instructions::{SelectedInstructionKind, SelectedSelectionConstraints};
    use semantic_vocabulary::{BlockId, EdgeId, MachineId, OperationId};
    use target_operations::{CallSiteOwner, NativeCallOrigin, TerminalPsiProvenance};

    #[test]
    fn repeated_cleanup_calls_keep_action_owners_and_settle_the_return_edge_once() {
        for target in [
            target::NativeTarget::linux_x64(),
            target::NativeTarget::linux_arm64(),
            target::NativeTarget::macos_arm64(),
            target::NativeTarget::windows_x64(),
        ] {
            let edge = EdgeId::new(1).unwrap();
            let block = BlockId::new(1).unwrap();
            let call_plan = evaluate_call_plan(
                CallingPolicy::native_for_target(target),
                &CallSignature {
                    parameters: Vec::new(),
                    result: None,
                },
            )
            .unwrap();
            let call = LegalizedScalarCall {
                source: NativeCallOrigin::Authored,
                callee: MachineId::new(2).unwrap(),
                call_plan: call_plan.clone(),
                arguments: Vec::new(),
                result_placement: None,
                structural_result: None,
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            };
            // Raw selection vocabulary only: proof admission and exact source
            // cleanup custody are tested at their independent earlier boundary.
            let source = LegalizedScalarFunction {
                machine: MachineId::new(1).unwrap(),
                attachment: None,
                provenance: TerminalPsiProvenance {
                    operations: Vec::new(),
                    edges: vec![edge],
                },
                call_plan,
                parameters: Vec::new(),
                structural: None,
                entry_block: block,
                ownership_frontier_facts: Vec::new(),
                blocks: vec![LegalizedScalarBlock {
                    id: block,
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    instructions: Vec::new(),
                    terminator: LegalizedScalarTerminator::ReturnCleanup {
                        returned: LegalizedScalarReturn {
                            edge,
                            value: LegalizedScalarReturnValue::Unit,
                            fuel: vec![FuelSettlement {
                                site: PsiProvenance::Edge(edge),
                                units: 1,
                            }],
                            effect: EffectLink {
                                input: 0,
                                output: 0,
                            },
                            ownership: Vec::new(),
                        },
                        calls: vec![
                            (
                                CallSiteOwner::CleanupAction {
                                    edge,
                                    action_ordinal: 0,
                                },
                                call.clone(),
                            ),
                            (
                                CallSiteOwner::CleanupAction {
                                    edge,
                                    action_ordinal: 2,
                                },
                                call,
                            ),
                        ],
                    },
                }],
            };
            let environment =
                register_environment::baseline_target_register_environment(target).unwrap();
            let constraints = SelectedSelectionConstraints {
                keys: environment.selected_keys(),
                fixed_inputs: Vec::new(),
            };
            let selected = super::super::build(
                0,
                &source,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            let validate = |selected: &selected_instructions::SelectedFunction| {
                crate::selection::validation::scalar_graph::validate_with_environment(
                    0,
                    &source,
                    selected,
                    &constraints,
                    &environment,
                )
            };
            validate(&selected).unwrap();
            assert_eq!(selected.calls.len(), 2);
            assert_eq!(
                selected.blocks[0]
                    .instructions
                    .iter()
                    .filter(|row| matches!(row.kind, SelectedInstructionKind::CallUnit { .. }))
                    .count(),
                2
            );
            assert!(
                selected.blocks[0].instructions.iter().all(|row| row
                    .provenance
                    .operations
                    .is_empty()
                    && row.provenance.fuel.is_empty())
            );
            for mutation in 0..3 {
                let mut hostile = selected.clone();
                match mutation {
                    0 => {
                        hostile.calls[1].owner = CallSiteOwner::CleanupAction {
                            edge,
                            action_ordinal: 1,
                        }
                    }
                    1 => {
                        hostile.blocks[0].instructions.remove(0);
                    }
                    _ => hostile.blocks[0].instructions[0]
                        .provenance
                        .operations
                        .push(OperationId::new(1).unwrap()),
                }
                assert!(validate(&hostile).is_err());
            }
        }
    }
}
