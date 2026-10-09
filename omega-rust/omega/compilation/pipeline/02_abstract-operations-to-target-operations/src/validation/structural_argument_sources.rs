//! Bind each retained structural call argument to its referent's exact home.
//!
//! `TargetStructuralArgument` retains the semantic referent (`place`, `path`,
//! `access`) beside the caller-side storage custody that produced the borrowed
//! pointer. A physically equivalent placement or a staged local is not the
//! referent, and shared physical shape cannot authorize access substitution:
//! this pass reconstructs the canonical home for every claimed argument place
//! from the source function — a caller parameter's checked placement, a
//! non-entry block parameter, or the exact establishing operation — and
//! rejects any argument whose `source` names different storage or a different
//! producer. Independent replay then owns the same identities downstream.

use std::collections::BTreeMap;

use abstract_operations::{
    AbstractBoundaryResult, AbstractFunction, AbstractOperation, AbstractStructuralCallResult,
};
use calling_conventions::ValuePlacement;
use semantic_vocabulary::{BlockId, OperationId, PlaceId};
use target_operations::{
    TargetFunction, TargetStructuralArgument, TargetStructuralArgumentSource,
    TargetStructuralHomeRequirement, TargetUnitOperation,
};
use terminal_psi::StructuralOperationResult;


/// A retained structural-result `Call` producer: its semantic result, retained
/// home requirement, and published result placement.
type ResultCall<'a> = (
    &'a StructuralOperationResult,
    Option<&'a TargetStructuralHomeRequirement>,
    Option<&'a ValuePlacement>,
);



