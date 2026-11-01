//! One callable comparison's positional static binders and enclosing region
//! frames. Contravariant descent reverses both together; sibling refinement
//! borrows the same parent frames without consuming their captures.

use super::BinderBinding;
use crate::borrow_contracts::CallableLifetimeFrame;

#[derive(Clone, Default)]
pub(super) struct CallableBinderScope {
    pub(super) static_bindings: Vec<BinderBinding>,
    pub(super) lifetime_frames: Vec<CallableLifetimeFrame>,
}

impl CallableBinderScope {
    pub(super) fn reversed(&self) -> Self {
        Self {
            static_bindings: self
                .static_bindings
                .iter()
                .map(|binding| BinderBinding {
                    required: binding.actual,
                    actual: binding.required,
                })
                .collect(),
            lifetime_frames: self
                .lifetime_frames
                .iter()
                .map(|frame| CallableLifetimeFrame {
                    required: frame.actual.clone(),
                    actual: frame.required.clone(),
                })
                .collect(),
        }
    }
}
