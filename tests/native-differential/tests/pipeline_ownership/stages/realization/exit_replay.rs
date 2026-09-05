//! Reauthenticated exit-record contradictions must fail direct replay.

use super::current_allocation::allocation;
use crate::tests::*;

type Mutation = (&'static str, fn(&mut WholeFunctionExitContract));

#[test]
fn exit_replay_checks_rosters_and_return_fields_after_reauthentication() {
    let mutations: &[Mutation] = &[
        ("function omitted", |record| record.functions.clear()),
        ("function duplicated", |record| {
            record.functions.push(record.functions[0].clone())
        }),
        ("return omitted", |record| {
            record.functions[0].returns.pop();
        }),
        ("return duplicated", |record| {
            let returned = record.functions[0].returns[0].clone();
            record.functions[0].returns.push(returned);
        }),
        ("return order", |record| {
            record.functions[0].returns.reverse()
        }),
        ("stack delta", |record| {
            record.functions[0].body_stack_delta = 1
        }),
        ("callee save", |record| {
            record.functions[0]
                .modified_callee_saved_units
                .push(register_model::RegisterUnitId(0))
        }),
        ("entry block", |record| {
            record.functions[0].entry_block = selected_instructions::SelectedBlockId(u32::MAX)
        }),
        ("return byte", |record| {
            record.functions[0].returns[0].bytes[0] ^= 1
        }),
        ("return coordinate", |record| {
            record.functions[0].returns[0].offset += 1
        }),
        ("return edge", |record| {
            record.functions[0].returns[0].psi_return_edge =
                semantic_vocabulary::EdgeId::new(u64::MAX).unwrap()
        }),
        ("return value category", |record| {
            record.functions[0].returns[0].value = WholeFunctionReturnValueEvidence::UnitV1
        }),
        ("return mechanism", |record| {
            match &mut record.functions[0].returns[0].mechanism {
                WholeFunctionReturnMechanism::X86ActivationStackReturnV1 { pop_bytes, .. } => {
                    *pop_bytes = 7
                }
                WholeFunctionReturnMechanism::Aarch64LinkRegisterReturnV1 {
                    link_register, ..
                } => link_register.0 ^= 1,
            }
        }),
        ("ABI alignment", |record| record.stack_alignment += 1),
        ("layout role", |record| {
            record.layout_custody =
                WholeFunctionExitLayoutCustody::PostAllocationMachineOptimizationV1 {
                    optimization: Optimization::CopyPropagation,
                    artifact_identity: [7; 32],
                }
        }),
    ];
    for (target, relaxation) in [
        (NativeTarget::linux_x64(), false),
        (NativeTarget::linux_x64(), true),
        (NativeTarget::linux_arm64(), false),
    ] {
        let mut realization = stage_selected_lowering_function_relative_realization(allocation(
            target, true, relaxation,
        ))
        .unwrap();
        let original = realization.exit_contract().shared_contract();
        assert!(original.functions[0].returns.len() > 1);
        for (name, mutate) in mutations {
            let record = realization.exit_contract_mut().contract_mut();
            mutate(record);
            record.identity = record.recomputed_identity();
            assert_ne!(
                record.identity, original.identity,
                "{name} must change the record"
            );
            assert_eq!(
                validate_selected_lowering_function_relative_realization_custody(&realization),
                Err(FunctionRelativeOptimizationRealizationError::ExitContract(
                    WholeFunctionExitContractError::ArtifactMismatch
                )),
                "{name}: {target:?}, relaxation={relaxation}"
            );
            *realization.exit_contract_mut().contract_mut() = (*original).clone();
            validate_selected_lowering_function_relative_realization_custody(&realization).unwrap();
        }
    }
}
