//! Executable scopes: the eras that share one process, the scopes admitted
//! as isolated, each scope's trusted computing base and the allowance it is
//! judged against, selected-composition behavior exclusions, and process-wide
//! service registrations.

pub(crate) mod executable_tcb_manifest;

pub(crate) mod behavior_exclusions;
