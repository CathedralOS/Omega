//! Canonical virtual-register constructors shared by scalar families.

use legalized_operations::LegalizedTemporaryId;

use crate::selection::shared::*;

use super::model::ScalarConstructionContext;

pub(super) fn condition_input(
    context: &ScalarConstructionContext<'_>,
    id: u32,
    input: usize,
) -> VirtualRegister {
    let input = context.condition_inputs[input];
    VirtualRegister {
        id: VirtualRegisterId(id),
        scalar_type: input.scalar_type,
        class: input.class,
        origin: VirtualRegisterOrigin::EntryParameter {
            source_value: input.source_value,
            parameter_index: input.parameter_index,
        },
        definition_site: input.definition_site,
        entry_fixed_view: Some(input.view),
    }
}

pub(super) fn instruction_result(
    context: &ScalarConstructionContext<'_>,
    id: u32,
    instruction: u32,
    source_value: semantic_vocabulary::ValueId,
    definition_site: ValueDefinitionSite,
    class: RegisterClassId,
) -> VirtualRegister {
    VirtualRegister {
        id: VirtualRegisterId(id),
        scalar_type: context.u64_type,
        class,
        origin: VirtualRegisterOrigin::InstructionResult {
            instruction: SelectedInstructionId(instruction),
            source_value,
        },
        definition_site,
        entry_fixed_view: None,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn entry_parameter(
    context: &ScalarConstructionContext<'_>,
    id: u32,
    source_value: semantic_vocabulary::ValueId,
    parameter_index: usize,
    definition_site: ValueDefinitionSite,
    class: RegisterClassId,
    fixed_view: RegisterViewId,
) -> VirtualRegister {
    VirtualRegister {
        id: VirtualRegisterId(id),
        scalar_type: context.u64_type,
        class,
        origin: VirtualRegisterOrigin::EntryParameter {
            source_value,
            parameter_index,
        },
        definition_site,
        entry_fixed_view: Some(fixed_view),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn legalization_temporary(
    context: &ScalarConstructionContext<'_>,
    id: u32,
    instruction: u32,
    temporary: LegalizedTemporaryId,
    source_value: semantic_vocabulary::ValueId,
    definition_site: ValueDefinitionSite,
    class: RegisterClassId,
) -> VirtualRegister {
    VirtualRegister {
        id: VirtualRegisterId(id),
        scalar_type: context.u64_type,
        class,
        origin: VirtualRegisterOrigin::LegalizationTemporary {
            instruction: SelectedInstructionId(instruction),
            temporary,
            source_value,
        },
        definition_site,
        entry_fixed_view: None,
    }
}
