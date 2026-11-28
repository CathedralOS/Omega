#[path = "../support/project.rs"]
mod project;
#[path = "../support/mod.rs"]
mod support;

mod build_deferred_imports;
mod build_dependency_alias;
mod build_dependency_preparation;
mod build_dependency_without_build_file;
mod build_generated_source;
mod build_source_inventory;
mod dependency_refusal_location;
mod direct_file_imports;
