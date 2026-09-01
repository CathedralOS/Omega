use crate::selection::shared::*;

use super::{
    active_resident_exact_add_bridge_chain_return, active_resident_exact_add_chain_return,
    exact_binary_return, immediate_return, parameter_return,
};

pub(super) fn validate(
    function_index: usize,
    source: &SourceFunction,
    function: &SelectedFunction,
    keys: SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    match (&source.when_true.value, &source.when_false.value) {
        (SourceLeafValue::ActiveResidentExactAddChain(..), SourceLeafValue::Immediate { .. }) => {
            active_resident_exact_add_chain_return::validate(
                function_index,
                &function.blocks[1],
                &source.when_true,
                keys,
                catalog,
            )?;
            immediate_return::validate(
                function_index,
                &function.blocks[2],
                9,
                10,
                VirtualRegisterId(7),
                &source.when_false,
                keys,
                catalog,
            )
        }
        (
            SourceLeafValue::ActiveResidentExactAddBridgeChain(..),
            SourceLeafValue::Immediate { .. },
        ) => {
            active_resident_exact_add_bridge_chain_return::validate(
                function_index,
                &function.blocks[1],
                &source.when_true,
                keys,
                catalog,
            )?;
            immediate_return::validate(
                function_index,
                &function.blocks[2],
                10,
                11,
                VirtualRegisterId(8),
                &source.when_false,
                keys,
                catalog,
            )
        }
        (SourceLeafValue::Immediate { .. }, SourceLeafValue::Immediate { .. }) => {
            immediate_return::validate(
                function_index,
                &function.blocks[1],
                2,
                3,
                VirtualRegisterId(1),
                &source.when_true,
                keys,
                catalog,
            )?;
            immediate_return::validate(
                function_index,
                &function.blocks[2],
                4,
                5,
                VirtualRegisterId(2),
                &source.when_false,
                keys,
                catalog,
            )
        }
        (SourceLeafValue::EntryParameter { .. }, SourceLeafValue::EntryParameter { .. }) => {
            parameter_return::validate(
                function_index,
                &function.blocks[1],
                2,
                VirtualRegisterId(1),
                &source.when_true,
                keys,
                catalog,
            )?;
            parameter_return::validate(
                function_index,
                &function.blocks[2],
                3,
                VirtualRegisterId(1),
                &source.when_false,
                keys,
                catalog,
            )
        }
        (SourceLeafValue::ExactAdd { .. }, SourceLeafValue::ExactAdd { .. }) => {
            validate_exact_binary_pair(function_index, source, function, keys, catalog)
        }
        (SourceLeafValue::WidenedExactAdd { .. }, SourceLeafValue::WidenedExactAdd { .. }) => {
            validate_exact_binary_pair(function_index, source, function, keys, catalog)
        }
        (
            SourceLeafValue::WidenedExactSubtract { .. },
            SourceLeafValue::WidenedExactSubtract { .. },
        ) => validate_exact_binary_pair(function_index, source, function, keys, catalog),
        (SourceLeafValue::ExactSubtract { .. }, SourceLeafValue::ExactSubtract { .. }) => {
            validate_exact_binary_pair(function_index, source, function, keys, catalog)
        }
        _ => Err(SelectedInstructionError::UnsupportedSourceShape {
            function: function_index,
        }),
    }
}

fn validate_exact_binary_pair(
    function_index: usize,
    source: &SourceFunction,
    function: &SelectedFunction,
    keys: SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    exact_binary_return::validate(
        function_index,
        &function.blocks[1],
        [2, 3, 4, 5],
        [
            VirtualRegisterId(1),
            VirtualRegisterId(2),
            VirtualRegisterId(3),
        ],
        &source.when_true,
        keys,
        catalog,
    )?;
    exact_binary_return::validate(
        function_index,
        &function.blocks[2],
        [6, 7, 8, 9],
        [
            VirtualRegisterId(4),
            VirtualRegisterId(5),
            VirtualRegisterId(6),
        ],
        &source.when_false,
        keys,
        catalog,
    )
}
