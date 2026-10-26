//! The one Unit-bodied plan a checked machine carries.
//!
//! Partial residual bodies and nominal caller metadata retain their semantic
//! distinctions. Nominal bodies and selected actions use the ordinary roster;
//! every shape any plan names lives in the ordinary
//! `CheckedUnitEffectPlans` type namespace, which stage 04 fills once with a
//! conflict check. This view names the family once, so a consumer asks which Unit
//! plan a machine lowers instead of walking three rosters and repeating the
//! same attachment admission beside each. Every multi-state Unit control graph
//! is a composed plan; no separate structural control roster competes for it.

use symbols::SymbolHandle;
use typed_trees::type_identity::TypeIdentityId;

use crate::checked_trees::flow::FlowFacts;
use crate::checked_trees::flow::terminal::{
    CheckedComposedUnitControlMachinePlan, CheckedNominalAffineUnitCleanupMachinePlan,
    CheckedPartialAffineUnitCleanupMachinePlan, CheckedTerminalSignatureEligibility,
};

/// The selected entry family one machine lowers. Nominal metadata roots its
/// already-published body; partial cleanup retains a residual body and composed
/// control retains its graph. The declaration order preserves entry dispatch.
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
    }

    /// The selected machine this plan lowers.
    pub fn machine(self) -> SymbolHandle {
        match self {
            Self::NominalAffineCleanup(plan) => plan.machine,
            Self::PartialAffineCleanup(plan) => plan.machine.machine,
            Self::ComposedControl(plan) => plan.machine,
        }
    }

    /// The exact attached data carrier, or `None` for a free machine.
    pub fn attachment_type_identity(self) -> Option<TypeIdentityId> {
        match self {
            Self::NominalAffineCleanup(plan) => plan.attachment_type_identity,
            Self::PartialAffineCleanup(plan) => plan.machine.attachment_type_identity,
            Self::ComposedControl(plan) => plan.attachment_type_identity,
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
                .filter(|plan| plan.machine == machine)
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
        };
        rows > 1
    }
}
