//! Stage 04 holds this capability while it freezes the trees its passes completed.
//! Consumers depend on the prepared source, which never returns the capability.
#![forbid(unsafe_code)]

pub struct PreparationIssuer(());
pub const PREPARATION: PreparationIssuer = PreparationIssuer(());
