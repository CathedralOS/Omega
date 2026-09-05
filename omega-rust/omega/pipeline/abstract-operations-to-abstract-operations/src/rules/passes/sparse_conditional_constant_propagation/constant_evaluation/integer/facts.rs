use optimization_core::ScalarConstantFactIdentity;
use semantic_vocabulary::{IntegerValue, MachineId, ValueId};

use crate::{ScalarConstant, ScalarConstantAnalysis};

pub(in crate::rules::passes::sparse_conditional_constant_propagation) fn integer_constant(
    constants: &ScalarConstantAnalysis,
    machine: MachineId,
    value: ValueId,
) -> Option<(IntegerValue, ScalarConstantFactIdentity)> {
    constants.facts.iter().find_map(|fact| {
        (fact.valid_in.machine == machine && fact.value == value)
            .then_some(fact)
            .and_then(|fact| match fact.constant {
                ScalarConstant::Integer(value) => fact.identity.map(|identity| (value, identity)),
                ScalarConstant::Boolean(_) => None,
            })
    })
}

pub(in crate::rules::passes::sparse_conditional_constant_propagation) fn integer_value_type(
    function: &optimization_unit::PsiOptimizationFunction,
    value: ValueId,
) -> Option<semantic_vocabulary::IntegerType> {
    function
        .parameters
        .iter()
        .chain(function.blocks.iter().flat_map(|block| &block.parameters))
        .chain(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .flat_map(|node| &node.definitions),
        )
        .find_map(|definition| {
            (definition.value == value)
                .then_some(definition.scalar_type)
                .and_then(|scalar_type| match scalar_type {
                    semantic_vocabulary::ScalarType::Integer(integer) => Some(integer),
                    semantic_vocabulary::ScalarType::Boolean
                    | semantic_vocabulary::ScalarType::IeeeFloat(_) => None,
                })
        })
}
