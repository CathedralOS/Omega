//! Admit the closure's bodies before allocating shared identities: collect
//! every boundary their calls name into the closure roster, and admit each
//! composed body's callable graph.
//!
//! Stage 04 planned each call against its exact target, contract and reach,
//! so admission does not re-check them; it only gathers what emission needs.
//! The result retains each checked source alongside its admitted body, which
//! emission consumes in order.

use super::super::{
    CheckedTrees, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan, LoweringError,
};
use crate::unit::attached_unit::call_closure::BoundaryIdentity;

use super::bodies::{UnitBody, UnitPlans};
use super::composed_control::{self, callable::CallableBody};
use super::retain_unit_boundary;
use checked_trees::{CheckedBoundaryMachinePlan, CheckedComposedUnitControlMachinePlan};
use symbols::SymbolHandle;

pub(super) enum AdmittedBody<'a> {
    Ordinary(&'a CheckedUnitEffectMachinePlan),
    Composed {
        source: &'a CheckedComposedUnitControlMachinePlan,
        body: CallableBody<'a>,
    },
}

impl<'a> AdmittedBody<'a> {
    pub(super) fn source(&self) -> UnitBody<'a> {
        match self {
            Self::Ordinary(plan) => UnitBody::Ordinary(plan),
            Self::Composed { source, .. } => UnitBody::Composed(source),
        }
    }
}

pub(super) fn admit<'a>(
    checked: &'a CheckedTrees,
    plans: UnitPlans<'a>,
    closure: &[SymbolHandle],
    boundaries: &mut Vec<(&'a CheckedBoundaryMachinePlan, BoundaryIdentity)>,
) -> Result<Vec<AdmittedBody<'a>>, LoweringError> {
    let mut bodies = Vec::with_capacity(closure.len());
    for source in closure {
        let admitted = match UnitBody::find(plans, *source)? {
            UnitBody::Composed(plan) => {
                let body = composed_control::admit_callable(checked, plan)?;
                for (boundary, _) in body.boundaries() {
                    retain_unit_boundary(checked, plans, boundaries, boundary.machine)?;
                }
                AdmittedBody::Composed { source: plan, body }
            }
            UnitBody::Ordinary(plan) => {
                retain_call_boundaries(checked, plans, &plan.operations, boundaries)?;
                AdmittedBody::Ordinary(plan)
            }
        };
        bodies.push(admitted);
    }
    Ok(bodies)
}

/// Add every boundary the calls of `operations` name, member and operand
/// calls of structural values included, to the closure roster.
pub(super) fn retain_call_boundaries<'a, 'o>(
    checked: &CheckedTrees,
    plans: UnitPlans<'a>,
    operations: impl IntoIterator<Item = &'o CheckedUnitEffectOperationPlan>,
    boundaries: &mut Vec<(&'a CheckedBoundaryMachinePlan, BoundaryIdentity)>,
) -> Result<(), LoweringError> {
    for operation in operations {
        for call in operation
            .with_value_calls()
            .filter_map(CheckedUnitEffectOperationPlan::call)
            .filter(|call| call.is_boundary())
        {
            retain_unit_boundary(checked, plans, boundaries, call.target.machine)?;
        }
    }
    Ok(())
}
