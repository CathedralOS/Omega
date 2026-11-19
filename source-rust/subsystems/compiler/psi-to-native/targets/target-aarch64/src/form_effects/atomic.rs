use register_model::RegisterConstraintKey;
use selected_instructions::AtomicForm;
use selected_instructions::AtomicOperation;
use selected_instructions::AtomicWidth;
use selected_instructions::MachineAlternativeFamily;
use selected_instructions::MachineBarrier;
use selected_instructions::MachineEffectDeclaration;
use selected_instructions::MachineEncodedControlEffect;
use selected_instructions::MachineEncodedEffects;
use selected_instructions::MachineEncodedMemoryEffect;
use selected_instructions::MachineEncodedTrapBehavior;
use selected_instructions::MachineMemoryEffect;
use selected_instructions::MachineSemanticKind;
use selected_instructions::MachineSizeKnowledge;
use selected_instructions::MachineTrapBehavior;

pub fn effects(form: AtomicForm) -> MachineEncodedEffects {
    let retry = form
        .operation
        .retries_until_commit(target::Architecture::Aarch64);
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
    if retry {
        effects.control = MachineEncodedControlEffect::AtomicRetryUntilCommitV1;
    }
    let compares = matches!(
        form.operation,
        AtomicOperation::CompareExchange | AtomicOperation::CompareExchangeOnce
    );
    effects.implicit_unit_clobbers = ["x16", "x17"]
        .into_iter()
        .filter(|_| retry)
        .chain(compares.then_some("nzcv"))
        .flat_map(|name| {
            crate::register_model::aarch64_physical_register_model_ref()
                .view_named(name)
                .into_iter()
                .flat_map(|view| view.units.iter().copied())
        })
        .collect();
    effects.implicit_unit_clobbers.sort_unstable();
    effects
}

pub fn declaration(
    form: AtomicForm,
    constraint: RegisterConstraintKey,
) -> MachineEffectDeclaration {
    let byte_count = match form.operation {
        AtomicOperation::Load | AtomicOperation::Store => 4,
        AtomicOperation::CompareExchangeOnce => compare_exchange_once_byte_count(form.width),
        AtomicOperation::CompareExchange => compare_exchange_byte_count(form.width),
        _ => RETRY_UPDATE_BYTES,
    };
    MachineEffectDeclaration::single(
        MachineSemanticKind::AtomicAccess(form),
        constraint,
        MachineMemoryEffect::AtomicPointerV1,
        MachineTrapBehavior::MayArchitecturalFaultV1,
        MachineBarrier::ExternalEffect,
        MachineAlternativeFamily::AtomicAccess(form),
        MachineSizeKnowledge::ExactBytes(byte_count),
        effects(form),
    )
}

pub const fn compare_exchange_byte_count(width: AtomicWidth) -> u16 {
    if matches!(width, AtomicWidth::Bits8 | AtomicWidth::Bits16) {
        40
    } else {
        36
    }
}

pub const fn compare_exchange_once_byte_count(width: AtomicWidth) -> u16 {
    if matches!(width, AtomicWidth::Bits8 | AtomicWidth::Bits16) {
        44
    } else {
        40
    }
}

pub const RETRY_UPDATE_BYTES: u16 = 24;
