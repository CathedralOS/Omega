mod driver;
mod ir;
mod native;
mod proof;
mod semantic;

pub(crate) use omega_ast as ast;
pub(crate) use omega_core::{diagnostics, source};
pub(crate) use omega_lexer as lexer;
pub(crate) use omega_parser as parser;

pub use driver::{CheckOutput, CompileOptions, CompileOutput, PhaseTiming, check, compile};

#[cfg(test)]
mod tests;
