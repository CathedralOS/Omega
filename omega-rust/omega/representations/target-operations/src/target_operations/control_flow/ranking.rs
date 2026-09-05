//! Declared countdown control and retained ranked authority.

use crate::TargetStructuralParameter;
use abstract_operations::RankedU32CountdownCustody;
use calling_conventions::CallPlan;
use terminal_psi::{StructuralTypeDeclaration, TerminalAffineCleanupAction};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetRankedU32Countdown {
    pub custody: RankedU32CountdownCustody,
    pub call_plan: CallPlan,
    pub structural_types: Vec<StructuralTypeDeclaration>,
    pub structural_parameters: Vec<TargetStructuralParameter>,
    pub cleanup_actions: Vec<TerminalAffineCleanupAction>,
}
