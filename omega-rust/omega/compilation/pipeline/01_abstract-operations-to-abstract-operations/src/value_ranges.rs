//! Constant-backed ranges, independently reconstructed by their consumers.

pub(crate) mod constant_facts {
    use optimization_unit::{
        PsiOptimizationUnit, ScalarConstantValue, ValueDefinition, ValueRangeFact,
        ValueRangeRegion, ValueRangeSupport,
    };
    use semantic_vocabulary::ScalarType;

    use crate::semantic_analyses::{
        scalar_value_definition, sparse_conditional_constants::scalar_constants,
    };

    pub(crate) fn collect(unit: &PsiOptimizationUnit) -> Vec<ValueRangeFact> {
        let mut facts = Vec::new();
        for constant in scalar_constants(unit).facts {
            let ScalarConstantValue::Integer(value) = constant.constant else {
                continue;
            };
            let Some(identity) = constant.identity else {
                continue;
            };
            let Some(function) = unit.function(constant.valid_in.machine) else {
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
                },
            ));
        }
        facts
    }
}

pub(crate) mod facts {
    use optimization_unit::{
        ValueRangeFact, ValueRangeRegion, ValueRangeSupport, value_range_fact_identity,
    };
    use semantic_vocabulary::{IntegerType, IntegerValue, ValueId};

    pub(crate) fn new(
        value: ValueId,
        scalar_type: IntegerType,
        minimum: IntegerValue,
        maximum: IntegerValue,
        support: ValueRangeSupport,
        valid_in: ValueRangeRegion,
    ) -> ValueRangeFact {
        let identity =
            value_range_fact_identity(value, scalar_type, minimum, maximum, &support, &valid_in)
                .expect("internally derived value range has a canonical identity");
        ValueRangeFact {
            identity,
            value,
            scalar_type,
            minimum,
            maximum,
            support,
            valid_in,
        }
    }
}

use optimization_unit::{PsiOptimizationUnit, ValueRangeFact};
use optimization_unit_semantics::validate_current_value_range_fact_at;
use semantic_vocabulary::{BlockId, MachineId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValueRangeAnalysis {
    pub facts: Vec<ValueRangeFact>,
}

impl ValueRangeAnalysis {
    pub(crate) fn fact_applies_at(
        &self,
        fact: &ValueRangeFact,
        unit: &PsiOptimizationUnit,
        machine: MachineId,
        block: BlockId,
        node: u32,
    ) -> bool {
        self.facts.iter().any(|candidate| candidate == fact)
            && validate_current_value_range_fact_at(unit, fact, machine, block, node).is_ok()
    }
}

pub(crate) fn value_ranges(unit: &PsiOptimizationUnit) -> ValueRangeAnalysis {
    let mut facts = constant_facts::collect(unit);
    facts.sort_by_key(|fact| (fact.valid_in.machine, fact.value, fact.identity));
    ValueRangeAnalysis { facts }
}
