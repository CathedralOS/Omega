//! Contract expressions, one file per expression form -- names, members and
//! their aliases, calls, casts, constructors, operators, atomic loads, case
//! membership and static arguments -- with `projection` the shared value,
//! operator, call and member projections and `evidence` the call evidence.

pub(in crate::package_evidence::capture) mod atomic_loads;
pub(in crate::package_evidence::capture) mod calls;
mod case_membership;
pub(in crate::package_evidence::capture) mod casts;
pub(in crate::package_evidence::capture) mod constructors;
pub(in crate::package_evidence::capture) mod evidence;
pub(in crate::package_evidence::capture) mod members;
pub(in crate::package_evidence::capture) mod names;
pub(in crate::package_evidence::capture) mod operators;
pub(in crate::package_evidence::capture) mod projection;
pub(in crate::package_evidence::capture) mod static_arguments;
