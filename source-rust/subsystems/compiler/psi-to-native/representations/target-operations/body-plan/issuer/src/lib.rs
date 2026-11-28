//! Target lowering holds this capability when it publishes a complete body.
#![forbid(unsafe_code)]

pub struct LoweringIssuer(());
pub const TARGET_LOWERING: LoweringIssuer = LoweringIssuer(());
