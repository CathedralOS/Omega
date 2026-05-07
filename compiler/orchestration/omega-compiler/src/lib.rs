mod pipeline;
mod proof;
mod semantic;

pub(crate) use omega_abstract_syntax_tree as ast;
pub(crate) use omega_core::{diagnostics, source};
pub(crate) use omega_lexer as lexer;
pub(crate) use omega_parser as parser;

pub use pipeline::{CheckOutput, CompileOptions, CompileOutput, PhaseTiming, check, compile};

#[cfg(test)]
mod tests;
