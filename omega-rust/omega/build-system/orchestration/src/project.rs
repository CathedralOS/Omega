// Project build modules: `build_directory` guards output directories, `products` maps project demands to compiler requests, `project_compilation` executes admitted package batches, and `publication` writes final artifacts to disk.

pub(crate) mod build_directory;
pub(crate) mod products;
pub(crate) mod project_compilation;
pub mod publication;
