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


use calling_conventions::ValuePlacement;
use target_operations::TargetStructuralHomeRequirement;
use terminal_psi::StructuralOperationResult;





