#![forbid(unsafe_code)]

/*
We lower checked source trees (`CheckedTrees`) and their frozen type, loan, and permission facts
into unsealed, target-neutral Lowered Psi (`LoweredPsi`), retaining source custody, proof
obligations, and optional debug companions for downstream optimization (`Psi 07`) and canonical
Terminal Psi publication (`Psi 08`).
*/

/*
Why resolve every expression place/result type, storage carrier, and loan transfer directly from
Psi 05's frozen fact tables rather than re-reading authored syntax or re-running type/borrow
inference during lowering? Separating semantic decision-making (Psi 05) from structural CFG and
operation construction (Psi 06) guarantees that lowering cannot silently diverge from checked
judgments or invent unverified fallback behavior; any construct without an explicit checked plan
fails closed immediately (`LoweringError::Unsupported`).
*/

/*
Entered through `lower_machine` and `select_terminal_machine` (alongside companion projections
`project_external_provider_declarations`, `lower_task_activation_demands`, and
`lowered_module_provenance`), which dispatch selected machines through `unit`, `scalar_lowering`,
`expression_preparation`, `emission`, `retention`, and `proofs` before handing the resulting
`LoweredPsi` module to `07_lowered-psi-to-lowered-psi` and `08_lowered-psi-to-terminal-psi`.
*/

pub(crate) use optimization::digest_encoder;
mod attached_unit;
mod emission;
mod emission_cleanup;
mod expression_bindings;
mod expression_preparation;
mod machine_lowering;
mod operation_emission;
mod projections;
mod proofs;
mod retention;
mod scalar_computations;
mod scalar_lowering;
mod shared;
mod unit;
mod unit_arguments;
mod unit_composed_control;
mod unit_operation_frame;
mod unit_structural_values;

pub use machine_lowering::lower_machine;
pub use machine_lowering::machine_dispatch::{TerminalMachineSelection, select_terminal_machine};
pub use projections::external_provider_declarations::{
    ExternalProviderDeclarations, PreparedExternalProviderDeclarations,
    project_external_provider_declarations,
};
pub use projections::lowered_provenance::lowered_module_provenance;
pub use projections::program_roots::TerminalProgramRoots;
pub use projections::task_activation::lower_task_activation_demands;
pub use proofs::content_conservation::{
    LoweredContentConservation, LoweredContentIdentityReshuffles,
};
pub use proofs::float_meaning_projection::FloatMeaningProjectionLoweringError;
pub use shared::lowering_error::LoweringError;
pub use terminal_psi::{PreparedTerminalRoots, RetainedTerminalRoot};
