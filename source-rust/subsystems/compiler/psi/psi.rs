//! The portable Psi program shared by source publication, interpretation and native lowering.
//! The wire format, artifact envelope and receiving authorization have separate owners.

#![forbid(unsafe_code)]

/// A concrete program, its declared boundaries, and the evidence carried in its canonical encoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Psi {
    pub vocabulary_marker: VocabularyMarker,
    pub identities: TerminalIdentityTable,
    pub entry: Option<TargetMachineCallable>,
    pub machines: Vec<TerminalMachine>,
    pub types: TypeDeclarations,
    pub scalars: ScalarDeclarations,
    pub interfaces: ProgramInterfaces,
    pub access: AccessDeclarations,
    pub execution: ExecutionDeclarations,
    pub proofs: ProgramProofs,
    pub layouts: LayoutDeclarations,
}

#[path = "types/declarations.rs"]
mod types_declarations;
pub use types_declarations::TypeDeclarations;
#[path = "types/scalars.rs"]
mod scalars_declarations;
pub use scalars_declarations::ScalarDeclarations;
#[path = "boundary/interfaces.rs"]
mod interfaces_declarations;
pub use interfaces_declarations::ProgramInterfaces;
#[path = "ownership/declarations.rs"]
mod access_declarations;
pub use access_declarations::AccessDeclarations;
#[path = "machines/execution.rs"]
mod execution_declarations;
pub use execution_declarations::ExecutionDeclarations;
#[path = "evidence/program_proofs.rs"]
mod proofs_declarations;
pub use proofs_declarations::ProgramProofs;
#[path = "types/layouts/declarations.rs"]
mod layouts_declarations;
pub use layouts_declarations::LayoutDeclarations;

#[path = "machines/published_entry.rs"]
mod published_entry;
#[path = "evidence/artifacts/mod.rs"]
pub mod artifacts;
#[path = "boundary/mod.rs"]
pub mod boundary;
#[path = "machines/call_contexts.rs"]
mod call_contexts;
#[path = "machines/callable_values.rs"]
pub mod callable_values;
#[path = "machines/generic_calls.rs"]
pub mod generic_calls;
#[macro_use]
#[path = "machines/control_flow.rs"]
pub mod control_flow;
#[path = "identity/mod.rs"]
pub mod identity;
#[macro_use]
#[path = "ownership/mod.rs"]
pub mod ownership;
#[path = "evidence/proof/mod.rs"]
pub mod proof;
#[path = "boundary/service_schemas.rs"]
pub mod service_schemas;
#[path = "machines/static_reach.rs"]
pub mod static_reach;
#[path = "types/structural_path_encoding.rs"]
pub mod structural_path_encoding;
#[path = "vocabulary.rs"]
pub mod terminal_module;
#[path = "types/mod.rs"]
pub mod types_and_values;
pub(crate) use ::wire::digest_encoder;
#[path = "machines/roots.rs"]
pub mod prepared_roots;
#[path = "boundary/callback_application.rs"]
pub mod callback_application;
#[path = "boundary/installed_interrupt.rs"]
pub mod installed_interrupt;
#[path = "boundary/calling_policies.rs"]
pub mod calling_policies;
#[path = "types/layouts/native_layouts.rs"]
pub mod native_layouts;
#[path = "machines/unoptimized_behavior.rs"]
pub mod unoptimized_behavior;
#[path = "boundary/callback_declarations.rs"]
pub mod callback_declarations;
#[path = "types/layouts/target_layouts.rs"]
pub mod target_layouts;
#[path = "machines/direct_calls.rs"]
pub mod direct_calls;

mod exports;
pub use exports::*;
