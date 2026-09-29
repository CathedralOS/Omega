use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedInstructionId, ValidatedMachineEffectCatalog};

use super::{CopiedCallOperandError, ValidatedCopiedCallOperand, admission};
use crate::ValidatedSelectedAnalysis;

/// Rebind every `Use` operand of `call` that reads `producer`'s `CopyI64`
/// destination to the copy's source register. The destination already
/// observes the source's value — the copy forwarded it and nothing between
/// the two instructions redefined the source — so the rewritten call
/// performs the identical invocation at the identical ABI placements while
/// its complete activation surface — barrier, call effect, control, trap,
/// implicit-unit and caller-saved clobber rosters, and the target's stack
/// lifecycle — rides the same constraint row verbatim. The producer stays:
/// its defined register remains published for every other reader the
/// function still holds. Every other function, block, instruction,
/// register, call, settlement, and access is retained, and replay
/// independently confirms that.
pub fn fold_selected_copied_call_operand(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    producer: SelectedInstructionId,
    call: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    effect_catalog: &ValidatedMachineEffectCatalog,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedCopiedCallOperand, CopiedCallOperandError> {
    let admitted = admission::admit(
        source,
        function_index,
        producer,
        call,
        environment,
        effect_catalog,
        budget,
    )?;
    let mut transformed = source.selected_plan().clone();
    transformed.functions[function_index].blocks[admitted.block_index].instructions
        [admitted.position] = admission::rewritten(&admitted);
    super::validate_copied_call_operand_fold(
        source,
        function_index,
        producer,
        call,
        environment,
        effect_catalog,
        budget,
        transformed,
    )
}
