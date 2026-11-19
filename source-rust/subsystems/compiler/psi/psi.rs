//! The portable Psi program shared by source publication, interpretation and native lowering.
//! The wire format, artifact envelope and receiving authorization have separate owners.

#![forbid(unsafe_code)]

/// A concrete program, its declared boundaries, and the evidence carried in its canonical encoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Psi {
    /// The instruction and type vocabulary used to interpret these records.
    pub vocabulary_marker: VocabularyMarker,
    pub identities: TerminalIdentityTable,
    /// The default executable entry, when the publication selects one.
    pub entry: Option<TargetMachineCallable>,
    /// Executable bodies, each containing its blocks, operations and terminators.
    pub machines: Vec<TerminalMachine>,
    pub types: TypeDeclarations,
    pub scalars: ScalarDeclarations,
    /// Declared services, calling conventions, providers and boundary demands.
    pub interfaces: ProgramInterfaces,
    pub access: AccessDeclarations,
    /// Callable declarations, dispatch, suspension and task activation.
    pub execution: ExecutionDeclarations,
    /// Propositions, evidence, proof outputs and checked operation obligations.
    pub proofs: ProgramProofs,
    /// Declared layout applications and their retained target descriptions.
    pub layouts: LayoutDeclarations,
}

#[path = "types/declarations.rs"]
mod type_declarations;
pub use type_declarations::TypeDeclarations;
#[path = "types/scalars.rs"]
mod scalar_declarations;
pub use scalar_declarations::ScalarDeclarations;
#[path = "boundary/interfaces.rs"]
mod program_interfaces;
pub use program_interfaces::ProgramInterfaces;
#[path = "ownership/declarations.rs"]
mod access_declarations;
pub use access_declarations::AccessDeclarations;
#[path = "machines/execution.rs"]
mod machine_declarations;
pub use machine_declarations::ExecutionDeclarations;
#[path = "evidence/program_proofs.rs"]
mod program_proofs;
pub use program_proofs::ProgramProofs;
#[path = "types/layouts/declarations.rs"]
mod layout_declarations;
pub use layout_declarations::LayoutDeclarations;

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

#[path = "types/opaque_storage.rs"]
mod opaque_storage;
pub use opaque_storage::OpaqueStorageShapes;
#[path = "boundary/queries.rs"]
mod boundary_queries;
#[path = "machines/instructions.rs"]
mod instruction_queries;
#[path = "machines/contracts.rs"]
mod machine_contracts;
#[path = "types/queries.rs"]
mod type_queries;

mod exports;
pub use exports::*;

impl Psi {
    pub fn for_entry(entry: semantic_vocabulary::MachineId) -> Self {
        Self {
            entry: Some(TargetMachineCallable::Machine(entry)),
            ..Self::declarations()
        }
    }

    pub fn declarations() -> Self {
        Self {
            vocabulary_marker: VocabularyMarker::CURRENT,
            identities: Default::default(),
            entry: None,
            machines: Vec::new(),
            types: Default::default(),
            scalars: Default::default(),
            interfaces: Default::default(),
            access: Default::default(),
            execution: Default::default(),
            proofs: Default::default(),
            layouts: Default::default(),
        }
    }
}
