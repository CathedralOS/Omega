mod builder;
mod clauses;
mod model;
mod normalize;
mod operands;

pub use builder::{build_state_guard_plan, classify_transition_guard_expression};
pub use clauses::{StateGuardClause, StateGuardClauses};
pub use model::{
    StateGuard, StateGuardKind, StateGuardOperand, StateGuardOperandKind, StateGuardOperandStorage,
    StateGuardPlan,
};
mod conjunction;
pub use conjunction::lower_guard_conjunction;
pub use omega_target_operations::{StateGuardLowering, StateGuardOperator};
