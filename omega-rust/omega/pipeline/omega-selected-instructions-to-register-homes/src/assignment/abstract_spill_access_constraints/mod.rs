//! Optimizer module role: executable entrance. Abstract spill-access constraints.
//!
//! This boundary orders compiler-private abstract accesses and records their
//! data, declared-barrier, and overlapping-slice dependencies. It grants no
//! executable operation, address, frame, fault, opcode, or publication authority.

mod compute;
mod identity;
mod model;
mod replay;
mod validate;

pub use identity::abstract_spill_access_constraint_plan_identity;
pub use model::*;
pub use validate::validate_abstract_spill_access_constraints;

pub fn constrain_abstract_spill_accesses(
    source: &crate::ValidatedAbstractSpillMemoryEffects,
    policy: AbstractSpillAccessConstraintPolicy,
    budget: omega_optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedAbstractSpillAccessConstraints, AbstractSpillAccessConstraintError> {
    let plan = compute::compute(source, policy, budget)?;
    validate_abstract_spill_access_constraints(source, plan)
}
