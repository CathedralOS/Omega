//! Contract expressions, one file per expression form -- names, members and
//! their aliases, calls, casts, constructors, operators, atomic loads, case
//! membership and static arguments -- with `projection` the shared value,
//! operator, call and member projections and `evidence` the call evidence.

pub(in crate::capture) mod atomic_loads;
pub(in crate::capture) mod calls;
mod case_membership;
pub(in crate::capture) mod casts;
pub(in crate::capture) mod constructors;
pub(in crate::capture) mod evidence;
pub(in crate::capture) mod members;
pub(in crate::capture) mod names;
pub(in crate::capture) mod operators;
pub(in crate::capture) mod projection;
pub(in crate::capture) mod static_arguments;
