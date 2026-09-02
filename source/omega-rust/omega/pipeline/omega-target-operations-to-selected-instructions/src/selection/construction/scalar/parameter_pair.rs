//! Two entry-parameter-return arms.

use crate::selection::constraints::fixed_input_constraint;
use crate::selection::shared::*;

use super::blocks;
use super::model::{ConstructedScalarBody, ScalarConstructionContext};
use super::registers;

pub(super) fn is_candidate(source: &SourceFunction) -> bool {
    matches!(
        source.when_true.value,
        SourceLeafValue::EntryParameter { .. }
    ) && matches!(
        source.when_false.value,
        SourceLeafValue::EntryParameter { .. }
    )
}

pub(super) fn build(
    context: &ScalarConstructionContext<'_>,
) -> Result<ConstructedScalarBody, SelectedInstructionError> {
    let SourceLeafValue::EntryParameter {
        parameter_index,
        register,
        definition_site,
    } = &context.source.when_true.value
    else {
        unreachable!("catalog selected the parameter-pair family")
    };
    let fixed = fixed_input_constraint(
        context.source.machine,
        context.source.when_true.source_value,
        *parameter_index,
        *register,
        &context.constraints.fixed_inputs,
    )
    .ok_or(SelectedInstructionError::MissingInputRegisterView {
        function: context.function,
    })?;
    let result_class = context
        .physical
        .model()
        .views
        .iter()
        .find(|view| view.id == fixed.fixed_view)
        .ok_or(SelectedInstructionError::MissingInputRegisterView {
            function: context.function,
        })?
        .class;
    let result = registers::entry_parameter(
        context,
        1,
        context.source.when_true.source_value,
        *parameter_index,
        *definition_site,
        result_class,
        fixed.fixed_view,
    );
    Ok(ConstructedScalarBody {
        virtual_registers: vec![registers::condition_input(context, 0, 0), result],
        blocks: vec![
            blocks::condition(context)?,
            blocks::parameter_return(
                context,
                SelectedBlockId(1),
                context.source.true_block,
                2,
                VirtualRegisterId(1),
                &context.source.when_true,
            )?,
            blocks::parameter_return(
                context,
                SelectedBlockId(2),
                context.source.false_block,
                3,
                VirtualRegisterId(1),
                &context.source.when_false,
            )?,
        ],
    })
}
