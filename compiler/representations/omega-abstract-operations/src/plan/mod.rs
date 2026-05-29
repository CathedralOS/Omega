mod capacity;
mod code;

pub use code::{AbstractOperationCode, AbstractOperationPlan};

impl Default for AbstractOperationPlan {
    fn default() -> Self {
        Self::with_capacity(0, 0, 0, 0)
    }
}
