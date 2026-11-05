//! Admit the closure's bodies before allocating shared identities: collect
//! every boundary their calls name into the closure roster, and admit each
//! composed body's callable graph.
//!
//! Psi 05 planned each call against its exact target, contract and reach,
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

pub(super) struct AdmittedClosure<'a> {
    pub(super) bodies: Vec<AdmittedBody<'a>>,
    pub(super) boundary_sources: Vec<(SymbolHandle, SymbolHandle)>,
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
) -> Result<AdmittedClosure<'a>, LoweringError> {
    let trace = std::env::var_os("OMEGA_STRUCT_TRACE").is_some();
    let mut bodies = Vec::with_capacity(closure.len());
    let mut boundary_sources = Vec::new();
    for source in closure {
        let admitted = match UnitBody::find(plans, *source)? {
            UnitBody::Composed(plan) => {
                let body = composed_control::admit_callable(checked, plan)?;
                for (boundary, _) in body.boundaries() {
                    if trace {
                        boundary_sources.push((*source, boundary.machine));
                    }
                    retain_unit_boundary(checked, plans, boundaries, boundary.machine)?;
                }
                AdmittedBody::Composed { source: plan, body }
            }
            UnitBody::Ordinary(plan) => {
                retain_call_boundaries(checked, plans, &plan.operations, boundaries, |target| {
                    if trace {
                        boundary_sources.push((*source, target));
                    }
                })?;
                AdmittedBody::Ordinary(plan)
            }
        };
        bodies.push(admitted);
    }
    Ok(AdmittedClosure {
        bodies,
        boundary_sources,
    })
}

/// Add every boundary the calls of `operations` name, member and operand
/// calls of structural values included, to the closure roster.
pub(super) fn retain_call_boundaries<'a, 'o>(
    checked: &CheckedTrees,
    plans: UnitPlans<'a>,
    operations: impl IntoIterator<Item = &'o CheckedUnitEffectOperationPlan>,
    boundaries: &mut Vec<(&'a CheckedBoundaryMachinePlan, BoundaryIdentity)>,
    mut observe: impl FnMut(SymbolHandle),
) -> Result<(), LoweringError> {
    for operation in operations {
        for call in operation
            .with_value_calls()
            .filter_map(CheckedUnitEffectOperationPlan::call)
            .filter(|call| call.is_boundary())
        {
            observe(call.target.machine);
            retain_unit_boundary(checked, plans, boundaries, call.target.machine)?;
        }
    }
    Ok(())
}
