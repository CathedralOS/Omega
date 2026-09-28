//! Producer-local affine-definition candidate indexing.

mod candidates;
pub(super) mod recording;

pub(crate) use recording::{CheckedWord, DefinitionIndex};
