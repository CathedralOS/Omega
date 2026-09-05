//! Optimizer module role: stage group. Abstract publication and target-lowering tests.
//!
//! Fixtures are grouped by the source shape they construct. Test leaves own
//! selection/custody, control-flow, scalar, GVN, proof-elision, manifest, and
//! independent-corruption behavior.

use std::collections::BTreeSet;

use omega_abstract_operations::AbstractOperation;
use omega_abstract_operations_optimizer::{
    built_in_psi_registry, replay_psi_pipeline, run_psi_pipeline,
};
use omega_optimization_core::{Optimization, OptimizationSelections, OptimizationWorkBudget};
use omega_optimization_validation::{
    OptimizationUnitValidationError, OptimizedAbstractPlanProjectionError,
    PhysicalOptimizationDataStatus, PrePhysicalOptimizationManifest,
    PrePhysicalOptimizationManifestDecodeError, PrePhysicalOptimizationManifestError,
    validate_optimized_abstract_plan_projection, validate_pre_physical_optimization_manifest,
};
use omega_psi_to_abstract_operations::VerifiedPsiOptimizationUnit;
use omega_target::NativeTarget;
use psi_core::{
    BlockId, ContractId, EdgeId, EvidenceIdentity, IntegerSign, IntegerType, IntegerValue,
    MachineId, ObligationId, OperationId, Proposition, ScalarTerm, ScalarType, ValueId,
};
use psi_proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, PrimitiveJudgment, ProofNode, ProofRule,
    ProofSystemMarker,
};
use psi_terminal::{
    Block, MachineContract, Operation, OperationKind, OperationResult, SuccessorEdge,
    TerminalMachine, TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
    VocabularyMarker,
};
use psi_terminal_verifier::{ObligationEvidence, ProofBundle};

use super::work_usage;
use super::*;

mod fixtures_boundary_qualifications;
mod fixtures_common;
mod fixtures_control_flow;
mod fixtures_copy_propagation;
mod fixtures_gvn;
mod fixtures_proof_elision;
mod fixtures_scalar;

use fixtures_boundary_qualifications::*;
use fixtures_common::*;
use fixtures_control_flow::*;
use fixtures_copy_propagation::*;
use fixtures_gvn::*;
use fixtures_proof_elision::*;
use fixtures_scalar::*;

mod boundary_qualifications;
mod control_flow;
mod copy_propagation;
mod corruption;
mod dead_scalar_elimination;
mod decision_custody;
mod global_value_numbering;
mod manifests;
mod proof_check_elision;
mod selection_and_external_decisions;
mod sparse_conditional_constants;
