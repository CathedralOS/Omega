//! One project's products: `products` turns project demands into compiler
//! requests, `project_compilation` executes an admitted package batch,
//! `build_directory` owns the output directory, and `publication` writes the
//! compiler's outputs into it.

pub(crate) mod build_directory;
pub(crate) mod products;
pub(crate) mod project_compilation;
pub mod publication;
