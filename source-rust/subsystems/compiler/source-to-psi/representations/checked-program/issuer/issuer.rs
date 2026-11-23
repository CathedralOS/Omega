//! Stage 06 holds this capability while it records the program it accepted.
//! Consumers depend on the checked program, which never returns the capability.
#![forbid(unsafe_code)]

pub struct CheckerIssuer(());
pub const CHECKER: CheckerIssuer = CheckerIssuer(());
