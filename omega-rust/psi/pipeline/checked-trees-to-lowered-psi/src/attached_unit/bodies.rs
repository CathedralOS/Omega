//! Borrowed views of complete Unit bodies; no synthetic checked plans or states.

use super::*;
use checked_trees::{
    CheckedComposedUnitControlMachinePlan, CheckedStructuralScalarParameterPlan,
    CheckedUnitEntryClaimPlan, CheckedUnitStructuralParameterPlan,
};

#[derive(Clone, Copy)]
pub(crate) enum UnitBody<'a> {
    Ordinary(&'a CheckedUnitEffectMachinePlan),
    Composed(&'a CheckedComposedUnitControlMachinePlan),
}

pub(crate) struct UnitEntry<'a> {
    pub(crate) machine: symbols::SymbolHandle,
    pub(crate) state: symbols::SymbolHandle,
    pub(crate) structural_parameters: &'a [CheckedUnitStructuralParameterPlan],
    pub(crate) scalar_parameters: &'a [CheckedStructuralScalarParameterPlan],
    pub(crate) entry_claims: &'a [CheckedUnitEntryClaimPlan],
    pub(crate) contract_report_fingerprint: u64,
    pub(crate) contract_service_reach: ServiceReachPlan,
}

impl<'a> UnitBody<'a> {
    pub(crate) fn result(self) -> checked_trees::CheckedControlResultPlan {
        match self {
            Self::Ordinary(plan) => plan.structural_result.as_ref().map_or(
                checked_trees::CheckedControlResultPlan::Unit,
                |result| {
                    checked_trees::CheckedControlResultPlan::Structural(
                        checked_trees::CheckedStructuralResultPlan {
                            type_identity: result.type_identity.clone(),
                            multiplicity: result.multiplicity,
                            qualifications: Vec::new(),
                        },
                    )
                },
            ),
            Self::Composed(plan) => plan.result.clone(),
        }
    }
    pub(crate) fn find(
        plans: &'a checked_trees::CheckedUnitEffectPlans,
        symbol: symbols::SymbolHandle,
    ) -> Result<Self, LoweringError> {
        let mut matches = plans
            .machines
            .iter()
            .filter(|plan| plan.machine == symbol)
            .map(Self::Ordinary)
            .chain(
                plans
                    .composed_machines
                    .iter()
                    .filter(|plan| plan.machine == symbol)
                    .map(Self::Composed),
            );
        let body = matches.next().ok_or(LoweringError::Unsupported(
            "attached Unit closure is missing a checked transitive machine plan",
        ))?;
        if matches.next().is_some() {
            return unsupported("attached Unit closure contains duplicate checked machine plans");
        }
        Ok(body)
    }

    pub(crate) fn entry(self) -> Result<UnitEntry<'a>, LoweringError> {
        Ok(match self {
            Self::Ordinary(plan) => UnitEntry {
                machine: plan.machine,
                state: plan.state,
                structural_parameters: &plan.structural_parameters,
                scalar_parameters: &plan.scalar_parameters,
                entry_claims: &plan.entry_claims,
                contract_report_fingerprint: plan.contract_report_fingerprint,
                contract_service_reach: plan.contract_service_reach,
            },
            Self::Composed(plan) => {
                let state = plan.states.first().ok_or(LoweringError::Unsupported(
                    "composed Unit body has no entry state",
                ))?;
                UnitEntry {
                    machine: plan.machine,
                    state: state.state,
                    structural_parameters: &state.structural_parameters,
                    scalar_parameters: &state.scalar_parameters,
                    entry_claims: &state.entry_claims,
                    contract_report_fingerprint: plan.contract_report_fingerprint,
                    contract_service_reach: plan.contract_service_reach,
                }
            }
        })
    }

    pub(crate) fn operations(self) -> impl Iterator<Item = &'a CheckedUnitEffectOperationPlan> {
        let (ordinary, composed) = match self {
            Self::Ordinary(plan) => (plan.operations.as_slice(), &[][..]),
            Self::Composed(plan) => (&[][..], plan.states.as_slice()),
        };
        ordinary
            .iter()
            .chain(composed.iter().flat_map(|state| &state.operations))
    }

    pub(crate) fn structural_parameters(
        self,
    ) -> impl Iterator<Item = &'a CheckedUnitStructuralParameterPlan> {
        let (ordinary, composed) = match self {
            Self::Ordinary(plan) => (plan.structural_parameters.as_slice(), &[][..]),
            Self::Composed(plan) => (&[][..], plan.states.as_slice()),
        };
        ordinary.iter().chain(
            composed
                .iter()
                .flat_map(|state| &state.structural_parameters),
        )
    }

    pub(crate) fn attachment(self) -> Option<&'a str> {
        match self {
            Self::Ordinary(plan) => plan.attachment_type_identity.as_deref(),
            Self::Composed(plan) => plan.attachment_type_identity.as_deref(),
        }
    }

    pub(crate) fn qualifications(self) -> &'a [SemanticDomainId] {
        match self {
            Self::Ordinary(plan) => &plan.body_qualifications,
            Self::Composed(plan) => &plan.body_qualifications,
        }
    }

    pub(crate) fn service_reach(self) -> ServiceReachSummary {
        match self {
            Self::Ordinary(plan) => plan.service_reach,
            Self::Composed(plan) => plan.service_reach,
        }
    }

    pub(crate) fn ordinary(self) -> Result<&'a CheckedUnitEffectMachinePlan, LoweringError> {
        match self {
            Self::Ordinary(plan) => Ok(plan),
            Self::Composed(_) => {
                unsupported("this Unit body consumer requires ordinary body metadata")
            }
        }
    }
}
