//! The calls around the compiler: `source_compilation` begins a compilation,
//! runs Build through `build_execution`, completes it, and hands native work
//! to `native_completion`. The compiler never runs Build itself.

pub(crate) mod build_execution;
pub(crate) mod native_completion;
pub(crate) mod source_compilation;
