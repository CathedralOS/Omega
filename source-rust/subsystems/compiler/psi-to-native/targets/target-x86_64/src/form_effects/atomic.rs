use crate::register_model::units;
use selected_instructions::AtomicForm;
use selected_instructions::AtomicOperation;
use selected_instructions::MachineEncodedControlEffect;
use selected_instructions::MachineEncodedEffects;
use selected_instructions::MachineEncodedMemoryEffect;
use selected_instructions::MachineEncodedTrapBehavior;

pub const fn writes_rflags(operation: AtomicOperation) -> bool {
    !matches!(operation, AtomicOperation::Load | AtomicOperation::Store)
}

pub fn effects(form: AtomicForm) -> MachineEncodedEffects {
    let mut effects = MachineEncodedEffects {
        memory: MachineEncodedMemoryEffect::AtomicPointerV1 {
            pointer_operand: 0,
            form,
        },
        trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ..MachineEncodedEffects::fallthrough_v1(
            (0..form.operation.input_count()).collect(),
            form.output_operands().collect(),
        )
    };
    if writes_rflags(form.operation) {
        effects
            .implicit_unit_clobbers
            .extend(units("condition_flags"));
    }
    if matches!(
        form.operation,
        AtomicOperation::CompareExchange | AtomicOperation::CompareExchangeOnce
    ) || form.operation.is_bitwise_fetch()
    {
        effects.implicit_unit_clobbers.extend(units("rax"));
    }
    if form.operation.is_bitwise_fetch() {
        effects.control = MachineEncodedControlEffect::AtomicRetryUntilCommitV1;
    }
    effects.implicit_unit_clobbers.sort_unstable();
    effects
}
