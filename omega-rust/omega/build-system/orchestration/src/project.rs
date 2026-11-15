// Project build modules: `build_directory` guards output directories, `products` maps project demands to compiler requests, `project_compilation` executes admitted package batches, `publication` writes final artifacts to disk, and `stage_validators` runs explicitly requested independent stage validators.

pub(crate) mod build_directory;
pub(crate) mod products;
pub(crate) mod project_compilation;
pub mod publication;
pub mod stage_validators;
