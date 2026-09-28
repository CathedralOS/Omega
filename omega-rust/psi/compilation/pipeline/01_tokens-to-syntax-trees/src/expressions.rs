//! Expression grammar: `parse_expression` is the precedence entry over the
//! `primary` forms and `parse_postfix` continuations, `membership` reads
//! membership expressions, and `context` names which forms a position
//! admits (struct literals, membership).

pub(crate) mod context;
mod membership;
pub(crate) mod parse_expression;
pub(crate) mod parse_postfix;
mod primary;
#[cfg(test)]
mod static_targets_tests;
