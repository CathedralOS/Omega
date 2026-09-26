//! Optimizer module role: executable entrance. Canonical constant and accepted-proof value-range analysis.
//!
//! This entrance joins the two exact support families, orders their facts
//! canonically, and exposes only the validated current-fact query. Constant
//! facts cover an entire value; proof facts descend through exact goal,
//! interval, reachability, and dominance reconstruction.

mod constant_facts;
mod control_flow;
mod facts;
mod intervals;
mod proof_facts;
mod proof_goals;

use optimization_unit::{PsiOptimizationUnit, ValueRangeFact};
use optimization_unit_semantics::validate_current_value_range_fact_at;
use semantic_vocabulary::{BlockId, MachineId};

use crate::analyses::control_flow::dominators;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueRangeAnalysis {
    pub facts: Vec<ValueRangeFact>,
}

impl ValueRangeAnalysis {
    pub fn fact_applies_at(
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

pub(in crate::analyses) fn value_ranges(unit: &PsiOptimizationUnit) -> ValueRangeAnalysis {
    let mut facts = constant_facts::collect(unit);
    proof_facts::extend(unit, &dominators(unit, false), &mut facts);
    facts.sort_by_key(|fact| {
        (
            fact.valid_in.machine,
            fact.value,
            fact.valid_in.scope,
            fact.identity,
        )
    });
    ValueRangeAnalysis { facts }
}
