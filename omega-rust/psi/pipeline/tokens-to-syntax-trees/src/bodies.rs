//! Machine and trait-default bodies: `parse_body` reads one body as a
//! statement sequence (`sequence`) of states (`states`), rewriting terminal
//! tail self-calls (`tail_calls`); `trait_default` reads a trait's default
//! machine body. `statements` owns the statement grammar and `transitions`
//! the transition blocks, their guards and targets.

pub(crate) mod parse_body;
pub(crate) mod sequence;
pub(crate) mod statements;
pub(crate) mod states;
pub(crate) mod tail_calls;
pub(crate) mod trait_default;
pub(crate) mod transitions;
