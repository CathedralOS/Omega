//! Checked-fact retention installed on the assembled module.
//!
//! After the selected machines exist, machine lowering installs the checked
//! custody that must survive discarding source companions: reborrow root
//! handoffs and restored call uses, foreign borrow custody, suspension call
//! plans, placed-view inputs, operation-level crash contracts for selected
//! operator invocations, and closed reach or conformance applications.

use checked_trees::CheckedTrees;
use checked_trees::types::PrimitiveType;
use language_semantics::Multiplicity;
use lowered_psi::LoweredSourceCallOccurrence;
use semantic_vocabulary::{
    BoundaryMachineId, ContentPlaceVersion, DomainSemanticId, OperationId, ValueId,
};
use terminal_psi::{
    BoundaryContentGuarantee, BoundaryMachineDeclaration, BoundaryMachineResult, OperationKind,
    StructuralAccess, StructuralMultiplicity, TerminalModule, ValueDeclaration,
};

use crate::emission::scalar_types::terminal_scalar_type;
use crate::lowering_error::{LoweringError, unsupported};
use crate::proofs::{content_conservation, evidence_lowering};
use crate::unit::attached_unit::checked_unit_boundary_identity;

pub(crate) mod closed_reach_applications;
pub(crate) mod conformance_applications;
pub(crate) mod operation_crash_contracts;
pub(crate) mod placed_view_inputs;
pub(crate) mod reborrow_restored_call_use;
pub(crate) mod reborrow_root_handoff;
pub(crate) mod retained_borrow_custody;
pub(crate) mod suspension_call_plan;
