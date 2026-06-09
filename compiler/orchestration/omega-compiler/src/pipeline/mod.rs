mod artifacts;
mod boundary_report;
mod checked_entry;
pub mod compile_options;
pub mod compile_report;
pub mod compiler;
pub mod frontend;
mod output;
mod project;
pub mod source;
mod stage;
mod stages;
mod timing;

pub use checked_entry::compile_to_checked;
pub use compile_options::CompileOptions;
pub use compile_report::CompileReport;
pub use compiler::compile;
