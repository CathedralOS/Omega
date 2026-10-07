//! The one Unit-bodied plan a checked machine carries.
//!
//! The affine cleanup rosters and the two control rosters remain separate
//! tables because each retains the structural-type roster its lowering emits
//! and the cleanup kinds must never be reread as a trivial discard by another
//! producer. This view names the family once, so a consumer asks which Unit
//! plan a machine lowers instead of walking four rosters and repeating the
//! same attachment admission beside each.

use symbols::SymbolHandle;

use crate::checked_trees::flow::FlowFacts;
use crate::checked_trees::flow::terminal::{
    CheckedComposedUnitControlMachinePlan, CheckedNominalAffineUnitCleanupMachinePlan,
    CheckedPartialAffineUnitCleanupMachinePlan, CheckedStructuralUnitControlMachinePlan,
    CheckedTerminalSignatureEligibility,
};

/// The Unit-bodied plan one machine lowers. Each variant borrows the row of
/// the roster that admitted it. The rosters are filled by independent
/// recognizers over the same machines, so a machine admitted by more than one
/// resolves in this declaration order, which is the order terminal lowering
/// has always dispatched in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedUnitPlan<'a> {
    /// One return edge disposing whole affine parameters through their exact
    /// nominal cleanup machines.
    NominalAffineCleanup(&'a CheckedNominalAffineUnitCleanupMachinePlan),
    /// One return edge disposing the residual subtrees of partially consumed
    /// affine roots.
    PartialAffineCleanup(&'a CheckedPartialAffineUnitCleanupMachinePlan),
    /// A composed multi-state control graph with a Unit or structural result.
    ComposedControl(&'a CheckedComposedUnitControlMachinePlan),
    /// An attached structural multi-state control graph.
    StructuralControl(&'a CheckedStructuralUnitControlMachinePlan),
}

impl<'a> CheckedUnitPlan<'a> {
    /// The Unit plan `machine` lowers, if any roster admitted it.
    pub fn for_machine(flow: &'a FlowFacts, machine: SymbolHandle) -> Option<Self> {
        flow.terminal_nominal_affine_unit_cleanups
            .for_machine(machine)
            .map(Self::NominalAffineCleanup)
            .or_else(|| {
                flow.terminal_partial_affine_unit_cleanups
                    .for_machine(machine)
                    .map(Self::PartialAffineCleanup)
            })
            .or_else(|| {
                flow.terminal_unit_effects
                    .composed_for_machine(machine)
                    .map(Self::ComposedControl)
            })
            .or_else(|| {
                flow.terminal_structural_unit_controls
                    .for_machine(machine)
                    .map(Self::StructuralControl)
            })
    }

    /// The selected machine this plan lowers.
    pub fn machine(self) -> SymbolHandle {
        match self {
            Self::NominalAffineCleanup(plan) => plan.machine.machine,
            Self::PartialAffineCleanup(plan) => plan.machine.machine,
            Self::ComposedControl(plan) => plan.machine,
            Self::StructuralControl(plan) => plan.machine,
        }
    }

    /// The exact attached data carrier, or `None` for a free machine.
    pub fn attachment_type_identity(self) -> Option<&'a str> {
        match self {
            Self::NominalAffineCleanup(plan) => plan.machine.attachment_type_identity.as_deref(),
            Self::PartialAffineCleanup(plan) => plan.machine.attachment_type_identity.as_deref(),
            Self::ComposedControl(plan) => plan.attachment_type_identity.as_deref(),
            Self::StructuralControl(plan) => plan.attachment_type_identity.as_deref(),
        }
    }

    /// Whether `signature` is the selection this plan lowers under: either
    /// free Unit signature, and never a plan that retains an owner.
    pub fn admits_signature(self, signature: CheckedTerminalSignatureEligibility) -> bool {
        self.attachment_type_identity().is_none()
            && matches!(
                signature,
                CheckedTerminalSignatureEligibility::Eligible
                    | CheckedTerminalSignatureEligibility::FreeUnitEffect
            )
    }

    /// Whether the roster that admitted this plan carries its machine more
    /// than once, which no lowering may resolve by picking one row.
    pub fn is_duplicated_in(self, flow: &FlowFacts) -> bool {
        let machine = self.machine();
        let rows = match self {
            Self::NominalAffineCleanup(_) => flow
                .terminal_nominal_affine_unit_cleanups
                .machines
                .iter()
                .filter(|plan| plan.machine.machine == machine)
                .count(),
            Self::PartialAffineCleanup(_) => flow
                .terminal_partial_affine_unit_cleanups
                .machines
                .iter()
                .filter(|plan| plan.machine.machine == machine)
                .count(),
            Self::ComposedControl(_) => flow
                .terminal_unit_effects
                .composed_machines
                .iter()
                .filter(|plan| plan.machine == machine)
                .count(),
            Self::StructuralControl(_) => flow
                .terminal_structural_unit_controls
                .machines
                .iter()
                .filter(|plan| plan.machine == machine)
                .count(),
        };
        rows > 1
    }
}
