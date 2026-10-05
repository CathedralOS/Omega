//! Storage observation helpers used during checked-plan construction.

use super::{CheckFacts, TypedTrees};

mod observations;
pub(in crate::execution::terminal_unit) use observations::uses_receiver_storage;
