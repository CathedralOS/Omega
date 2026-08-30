//! Optimizer module role: stage group. Independent CFG rewrite validation by exact graph transformation.

use super::*;

mod block_merging;
mod constant_conditionals;
mod empty_block_threading;
mod shared_jump_fusion;
mod unreachable_private_machines;

pub use block_merging::*;
pub use constant_conditionals::*;
pub use empty_block_threading::*;
pub use shared_jump_fusion::*;
pub use unreachable_private_machines::*;
