//! Stages 19-22: serialize the file envelope, apply the procedure-linkage fixups, assemble the file and admit it as an executable.
//!
//! Each stage takes the previous stage's `ValidatedElf*` carrier as its only
//! input; the modules are listed in chain order.

pub(crate) mod assembled_dynamic_file;
pub(crate) mod dynamic_file_envelope;
pub(crate) mod resolved_procedure_linkage;
