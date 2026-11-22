//! Stage 05 holds this capability while it binds a lowered program to the source it lowered.
//! Consumers depend on the lowered source publication, which never returns the capability.
#![forbid(unsafe_code)]

pub struct LoweringIssuer(());
pub const LOWERING: LoweringIssuer = LoweringIssuer(());
