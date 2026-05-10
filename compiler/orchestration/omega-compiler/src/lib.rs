mod pipeline;

pub(crate) use omega_abstract_syntax_tree as ast;
pub(crate) use omega_core::source;
pub(crate) use omega_lexer as lexer;
pub(crate) use omega_parser as parser;

pub use pipeline::{CompileOptions, CompileReport, compile};

#[cfg(test)]
mod tests;
