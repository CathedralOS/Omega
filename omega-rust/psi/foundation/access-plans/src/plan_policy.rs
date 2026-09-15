//! What a plan permits: the operations a field can be asked for, plan
//! validation, per-descriptor authorization, boundary reach and the
//! normalized identities every validated plan carries.

pub(crate) mod access_operations;
pub(crate) mod access_plan_validation;
pub(crate) mod authorization;
pub(crate) mod boundary_reach;
pub(crate) mod normalized_identities;
