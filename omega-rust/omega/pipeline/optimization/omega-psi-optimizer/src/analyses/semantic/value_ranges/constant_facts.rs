//! Entire-value ranges derived from exact scalar-constant facts.

use omega_optimization_unit::{
    PsiOptimizationUnit, ValueDefinition, ValueRangeFact, ValueRangeRegion, ValueRangeScope,
    ValueRangeSupport,
};
use psi_core::ScalarType;

use super::super::{
    shared::scalar_value_definition,
    sparse_conditional_constants::{ScalarConstant, scalar_constants},
};

pub(super) fn collect(unit: &PsiOptimizationUnit) -> Vec<ValueRangeFact> {
    let mut facts = Vec::new();
    for constant in scalar_constants(unit).facts {
        let ScalarConstant::Integer(value) = constant.constant else {
            continue;
        };
        let Some(identity) = constant.identity else {
            continue;
        };
        let Some(function) = unit
            .functions
            .iter()
            .find(|function| function.machine == constant.valid_in.machine)
        else {
            continue;
        };
        let Some(ValueDefinition {
            scalar_type: ScalarType::Integer(scalar_type),
            ..
        }) = scalar_value_definition(function, constant.value)
        else {
            continue;
        };
        facts.push(super::facts::new(
            constant.value,
            scalar_type,
            value,
            value,
            ValueRangeSupport::ScalarConstant(identity),
            ValueRangeRegion {
                revision: unit.identity,
                machine: function.machine,
                value: constant.value,
                scope: ValueRangeScope::EntireValue,
                dominated_blocks: Vec::new(),
            },
        ));
    }
    facts
}
