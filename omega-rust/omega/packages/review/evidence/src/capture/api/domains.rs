//! Public domains: `projection` projects a domain declaration, `facts` its
//! predicate and definition-contract facts, and `aliases` the alias
//! expansion and establishment route it names.

pub(in crate::capture) mod aliases;
pub(in crate::capture) mod facts;
pub(in crate::capture) mod projection;
