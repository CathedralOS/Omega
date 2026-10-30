use selected_instructions::LiteralFoldIdentity;

use crate::LiteralFoldPlan;

pub(crate) fn literal_fold_identity(plan: &LiteralFoldPlan) -> LiteralFoldIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-literal-fold.v8\0");
    bytes.extend_from_slice(&encode_terminal_literal_fold_content(plan));
    LiteralFoldIdentity::from_canonical_bytes(&bytes)
}

pub(crate) fn encode_terminal_literal_fold_content(plan: &LiteralFoldPlan) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&plan.source_selected.bytes());
    bytes.extend_from_slice(&plan.spill_choices.bytes());
    bytes.extend_from_slice(&plan.recovery_classifications.bytes());
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.legality.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.extend_from_slice(&plan.machine_effect_catalog.bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    crate::codec_bytes::push_encoded(&mut bytes, plan.fuel_schedule.marker().to_le_bytes());
    crate::codec_bytes::push_encoded(&mut bytes, plan.policy.canonical_bits().to_le_bytes());
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    length(&mut bytes, plan.functions.len());
    for function in &plan.functions {
        crate::codec_bytes::push_encoded(&mut bytes, function.machine.get().to_le_bytes());
        length(&mut bytes, function.actions.len());
        for action in &function.actions {
            crate::codec_bytes::push_encoded(&mut bytes, action.block.0.to_le_bytes());
            crate::codec_bytes::push_encoded(
                &mut bytes,
                action.literal_instruction.0.to_le_bytes(),
            );
            crate::codec_bytes::push_encoded(&mut bytes, action.victim.0.to_le_bytes());
            crate::codec_bytes::push_encoded(
                &mut bytes,
                action.consumer_instruction.0.to_le_bytes(),
            );
            crate::codec_bytes::push_encoded(&mut bytes, action.surviving.0.to_le_bytes());
            match action.result {
                None => bytes.push(0),
                Some(result) => {
                    bytes.push(1);
                    crate::codec_bytes::push_encoded(&mut bytes, result.0.to_le_bytes());
                }
            }
            crate::codec_bytes::push_encoded(&mut bytes, action.immediate.to_le_bytes());
            register_model::encode_constraint_key(&mut bytes, action.immediate_constraint);
        }
    }
    bytes.extend_from_slice(&plan.transformed_selected.bytes());
    bytes
}

fn length(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("literal-fold identity length fits u64")
            .to_le_bytes(),
    );
}
