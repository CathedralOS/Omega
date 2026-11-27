//! Resolve a signature's own lifetime telescope under the exact supplying
//! declaration and preceding applications. This is source formation metadata.

use crate::source_state::type_identity::LifetimeParameterIdentity;

#[derive(Clone, Copy)]
pub struct CallableLifetimeBinding {
    pub parameter: LifetimeParameterIdentity,
    pub argument: LifetimeParameterIdentity,
}

#[derive(Clone, Default)]
pub struct CallableLifetimeFrame {
    pub required: Vec<CallableLifetimeBinding>,
    pub actual: Vec<CallableLifetimeBinding>,
}

#[derive(Clone, Copy)]
pub enum CallableLifetimeApplication<'scope> {
    Enclosing {
        bindings: &'scope [CallableLifetimeBinding],
    },
    Invocation {
        bindings: &'scope [CallableLifetimeBinding],
    },
}
