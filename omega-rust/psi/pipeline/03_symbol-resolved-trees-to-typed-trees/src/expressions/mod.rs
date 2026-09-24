//! Typing of expressions and statements: the expression table, statements,
//! declared call results, propositions, equality and Equatable synthesis,
//! exhaustiveness, fixed byte-array literals and qualification casts.

pub(crate) mod call_results;
pub(crate) mod equality;
pub(crate) mod equatable;
pub(crate) mod exhaustiveness;
pub(crate) mod expression;
pub(crate) mod fixed_byte_array_literals;
pub(crate) mod proposition;
pub(crate) mod qualification_casts;
pub(crate) mod statement;
