#![forbid(unsafe_code)]

//! Structural validation and proof checking for terminal-Psi modules.
//!
//! The verifier reconstructs semantic axioms from executable operations and
//! edges, then requires evidence for every bodyful contract clause. Proof
//! bundles cannot choose which obligations exist.
//!
//! Start at `validation.rs`: `validate_module` checks a module's structure
//! in a fixed pass order and yields a `ValidatedTerminalModule`. Then
//! `verification.rs`: `verify_module` reconstructs every obligation of a
//! validated module and discharges each against the proof bundle, yielding
//! a `VerifiedTerminalModule` (or the interpretable, optimizable and
//! fixed-fuel variants). `trusted_surface` inventories what verification
//! trusts. The remaining root modules answer questions over an already
//! validated module: `control_graph` and `control_cycles` (control
//! topology and natural ranks), `proof_recursion` (recursive-component
//! questions), `optimization` (checks of target-neutral rewrites),
//! and `terminal_trace_v1` (the bounded observation profile).

mod control_cycles;
use terminal_semantics::control_graph;
pub mod trusted_surface;
pub use control_cycles::{
    AcceptedControlCycle, ReconstructedControlCycleObligation, control_cycle_components,
    reconstruct_control_cycle_obligations,
};
mod optimization;
mod proof_recursion;
mod terminal_trace_v1;
mod validation;
mod verification;

pub use optimization::{
    CopyPropagationRewriteError, DeadScalarRewriteError, ProofCheckElisionRewriteError,
    transport_copy_propagation_evidence,
    transport_dead_scalar_elimination_evidence, validate_dead_scalar_elimination,
    validate_proof_check_elision,
};
pub use proof_recursion::{
    proof_recursive_component_identity, proof_recursive_edge_obligation_id,
    proof_recursive_well_foundedness_obligation_id,
    reconstruct_proof_recursive_component_obligations,
};
pub use terminal_trace_v1::{
    TerminalTraceV1ReconstructionError, reconstruct_terminal_observation_profile_rows,
    reconstruct_terminal_trace_v1_rows,
};
pub use validation::trapping_integer::reconstruct_trapping_integer_normal_return_equation;
pub use validation::validate_module_retaining_frontiers;
pub use validation::{
    AtomicAccessRefusal, ContractClauseKind, ControlCycleIneligibility,
    ModuleError, ServiceCeilingOwner, StructuralSignatureOwner, SuspensionCallPlanError,
    ValidatedInterpretableTerminalModule, ValidatedOptimizableTerminalModule,
    ValidatedTerminalModule, VerifiedLiveClaim, VerifiedMachineStructuralFrontiers,
    VerifiedOwnedStructuralPlace, VerifiedPartialStructuralCustody,
    VerifiedStructuralOwnershipFrontier, VerifiedTerminalStructuralFrontiers,
    maximum_registered_obligation_id,
    scalar_block_invariant_scope, substitute_crash_routes,
    validate_module, validate_module_for_interpretation, validate_module_for_optimization,
    validate_module_representation,
};
pub use verification::{
    ControlCycleEvidence, CrashCertificate, CrashObligationEvidence, CrashObligationOwner,
    CrashObligationQuestion, EvidenceProducerProvenance, EvidenceProducerRealization,
    EvidenceProducerRowSource, FloatMeaningProjectionVerificationError, ObligationEvidence,
    ProofBundle, ReconstructedCrashObligation, ReconstructedFloatMeaningProjection,
    ReconstructedOperationObligation, ReconstructedTerminalObligation,
    ReconstructedTerminalObligationOwner, ReconstructedTerminalObligationSet,
    RecursiveComponentEvidence, VerificationError, VerifiedFixedFuelTerminalModule,
    VerifiedInterpretableTerminalModule, VerifiedOptimizableTerminalModule, VerifiedTerminalModule,
    crash_obligation_discharged, reconstruct_crash_obligations,
    reconstruct_execution_crash_obligations, reconstruct_execution_terminal_obligations,
    reconstruct_float_meaning_projection, reconstruct_interpretable_crash_obligations,
    reconstruct_interpretable_operation_obligations,
    reconstruct_interpretable_terminal_obligations, reconstruct_operation_obligations,
    reconstruct_optimizable_crash_obligations, reconstruct_optimizable_terminal_obligations,
    reconstruct_terminal_obligations, verify_module, verify_module_for_fixed_fuel,
    verify_module_for_interpretation, verify_module_for_optimization,
    verify_reconstructed_obligation,
};
