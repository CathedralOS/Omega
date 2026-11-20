//! Source IR shared with Build and tooling.
pub mod publication;
mod published_target_program;
pub use published_target_program::{PublishedTargetProgram, RuntimePermissionDeclarations};

pub use source_declarations::declaration_selection;
pub use source_symbol_table::{source_inputs, source_symbols};
pub use typed_trees::source_state;
