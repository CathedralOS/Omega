//! Control-flow cleanup rule coverage.
//!
//! The entrance supplies the shared pass-test vocabulary; leaves follow the
//! exact reachability, branch, merge, jump-fusion, and threading rule families.

use super::*;

mod block_merges;
mod constant_conditionals;
mod empty_blocks;
mod shared_jump_fusion;
mod unreachable_machines;
