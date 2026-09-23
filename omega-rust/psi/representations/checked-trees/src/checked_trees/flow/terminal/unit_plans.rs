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
            Self::StructuralControl(plan) => Some(&plan.attachment_type_identity),
        }
    }

    /// Whether `signature` is the selection this plan lowers under: an
    /// attached signature when the plan retains its owner, either free Unit
    /// signature otherwise.
    pub fn admits_signature(self, signature: CheckedTerminalSignatureEligibility) -> bool {
        matches!(
            (signature, self.attachment_type_identity().is_some()),
            (CheckedTerminalSignatureEligibility::Attached, true)
                | (CheckedTerminalSignatureEligibility::Eligible, false)
                | (CheckedTerminalSignatureEligibility::FreeUnitEffect, false)
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

#[cfg(test)]
mod tests {
    use symbols::SymbolHandle;

    use super::CheckedUnitPlan;
    use crate::checked_trees::flow::FlowFacts;
    use crate::checked_trees::flow::terminal::{
        CheckedStructuralUnitControlMachinePlan, CheckedTerminalSignatureEligibility,
    };

    fn structural_control(machine: SymbolHandle) -> CheckedStructuralUnitControlMachinePlan {
        CheckedStructuralUnitControlMachinePlan {
            machine,
            attachment_type_identity: "Owner".to_owned(),
            states: Vec::new(),
            ranked_scc: None,
        }
    }

    #[test]
    fn an_attached_control_plan_admits_only_the_attached_signature() {
        let machine = SymbolHandle::from_arena_index(1);
        let mut flow = FlowFacts::default();
        flow.terminal_structural_unit_controls
            .machines
            .push(structural_control(machine));

        let plan = CheckedUnitPlan::for_machine(&flow, machine).expect("plan");
        assert!(matches!(plan, CheckedUnitPlan::StructuralControl(_)));
        assert_eq!(plan.machine(), machine);
        assert_eq!(plan.attachment_type_identity(), Some("Owner"));
        assert!(plan.admits_signature(CheckedTerminalSignatureEligibility::Attached));
        assert!(!plan.admits_signature(CheckedTerminalSignatureEligibility::Eligible));
        assert!(!plan.admits_signature(CheckedTerminalSignatureEligibility::FreeUnitEffect));
        assert!(!plan.is_duplicated_in(&flow));
        assert_eq!(
            CheckedUnitPlan::for_machine(&flow, SymbolHandle::from_arena_index(2)),
            None
        );
    }

    #[test]
    fn a_machine_listed_twice_in_its_roster_is_duplicated() {
        let machine = SymbolHandle::from_arena_index(1);
        let mut flow = FlowFacts::default();
        flow.terminal_structural_unit_controls
            .machines
            .push(structural_control(machine));
        flow.terminal_structural_unit_controls
            .machines
            .push(structural_control(machine));

        let plan = CheckedUnitPlan::for_machine(&flow, machine).expect("plan");
        assert!(plan.is_duplicated_in(&flow));
    }
}
