//! Resolve a signature's own lifetime telescope under the exact supplying
//! declaration and preceding applications. This is source formation metadata.

use crate::source_state::type_identity::LifetimeParameterIdentity;

#[derive(Clone, Copy)]
pub struct CallableLifetimeBinding {
    pub parameter: LifetimeParameterIdentity,
    pub argument: LifetimeParameterIdentity,
}
