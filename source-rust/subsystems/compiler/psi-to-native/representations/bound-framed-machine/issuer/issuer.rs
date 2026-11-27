//! Stage 09's installation binding holds this capability while it binds a body or entry.
//! Consumers depend on the bound representation, which never returns the capability.
#![forbid(unsafe_code)]

pub struct BindingIssuer(());
pub const BINDER: BindingIssuer = BindingIssuer(());
