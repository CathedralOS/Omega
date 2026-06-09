mod pipeline;

pub(crate) use omega_core::source;
pub(crate) use omega_source_files_to_tokens as lexer;
pub(crate) use omega_tokens_to_syntax_trees as parser;

pub use pipeline::{CompileOptions, CompileReport, compile, compile_to_checked};
