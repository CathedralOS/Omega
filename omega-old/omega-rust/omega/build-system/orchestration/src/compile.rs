// Compiler orchestration modules: `source_compilation` starts a compilation, `build_execution` runs `build.omg` activations and dependency frontiers, and `native_completion` finishes post-Terminal native products.

pub(crate) mod build_execution;
pub(crate) mod native_completion;
pub(crate) mod source_compilation;
pub(crate) mod test_execution;
