//! Borrowed views of ordinary statement and graph bodies. Structural result
//! calls traverse either complete body through the same closure; result shape
//! does not select a different producer. No synthetic checked plans or states.
use super::super::ServiceReachPlan;
use super::{
    CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan, LoweringError, SemanticDomainId,
    ServiceReachSummary, unsupported,
};
use checked_trees::{
    CheckedComposedUnitControlMachinePlan, CheckedErasedProofParameterPlan,
    CheckedStructuralScalarParameterPlan, CheckedUnitEntryClaimPlan,
    CheckedUnitStructuralParameterPlan,
};

/// The Unit plan roster one closure assembly reads. Ordinary closures read
/// the checked `terminal_unit_effects` lane as published. A cleanup closure
/// begins at a dispatcher the checked trees publish in its own lane
/// (`terminal_nominal_affine_unit_cleanups` or
/// `terminal_partial_affine_unit_cleanups`), beside the ordinary roster
/// rather than inside it, together with the structural shapes that lane
/// owns. This view joins that one staged plan and those shapes onto the
/// ordinary roster so the emitter resolves them like any other body, without
/// the cleanup lowering cloning the checked trees to push them in. A staged
/// machine that the ordinary roster also publishes is a duplicate, exactly
/// as two published rows would be; a staged shape whose identity the roster
/// already carries yields to the published shape.
#[derive(Clone, Copy)]
pub(crate) struct UnitPlans<'a> {
    checked: &'a checked_trees::CheckedUnitEffectPlans,
    staged_machine: Option<&'a CheckedUnitEffectMachinePlan>,
    staged_structural_types: &'a [checked_trees::CheckedUnitStructuralTypePlan],
}

impl<'a> UnitPlans<'a> {
    /// The ordinary roster alone.
    pub(crate) fn published(checked: &'a checked_trees::CheckedUnitEffectPlans) -> Self {
        Self {
            checked,
            staged_machine: None,
            staged_structural_types: &[],
        }
    }

    /// The ordinary roster joined with one cleanup lane's dispatcher plan and
    /// the shapes that lane owns.
    pub(crate) fn with_staged(
        checked: &'a checked_trees::CheckedUnitEffectPlans,
        machine: &'a CheckedUnitEffectMachinePlan,
        structural_types: &'a [checked_trees::CheckedUnitStructuralTypePlan],
    ) -> Self {
        Self {
            checked,
            staged_machine: Some(machine),
            staged_structural_types: structural_types,
        }
    }

    /// The ordinary roster joined with a cleanup lane's shapes but not its
    /// dispatcher: the roster already publishes that machine.
    pub(crate) fn with_staged_structural_types(
        checked: &'a checked_trees::CheckedUnitEffectPlans,
        structural_types: &'a [checked_trees::CheckedUnitStructuralTypePlan],
    ) -> Self {
        Self {
            checked,
            staged_machine: None,
            staged_structural_types: structural_types,
        }
    }

    pub(crate) fn machines(self) -> impl Iterator<Item = &'a CheckedUnitEffectMachinePlan> {
        self.checked.machines.iter().chain(self.staged_machine)
    }

    pub(crate) fn for_machine(
        self,
        machine: symbols::SymbolHandle,
    ) -> Option<&'a CheckedUnitEffectMachinePlan> {
        self.machines().find(|plan| plan.machine == machine)
    }

    pub(crate) fn composed_machines(self) -> &'a [CheckedComposedUnitControlMachinePlan] {
        &self.checked.composed_machines
    }

    pub(crate) fn composed_for_machine(
        self,
        machine: symbols::SymbolHandle,
    ) -> Option<&'a CheckedComposedUnitControlMachinePlan> {
        self.checked.composed_for_machine(machine)
    }

    pub(crate) fn boundary_machines(self) -> &'a [checked_trees::CheckedBoundaryMachinePlan] {
        &self.checked.boundary_machines
    }

    pub(crate) fn structural_domains(self) -> &'a [checked_trees::CheckedUnitStructuralDomainPlan] {
        &self.checked.structural_domains
    }

    pub(crate) fn structural_types(
        self,
    ) -> impl Iterator<Item = &'a checked_trees::CheckedUnitStructuralTypePlan> {
        let published = &self.checked.structural_types;
        published
            .iter()
            .chain(self.staged_structural_types.iter().filter(move |staged| {
                !published
                    .iter()
                    .any(|shape| shape.identity == staged.identity)
            }))
    }

    pub(crate) fn structural_type(
        self,
        identity: &str,
    ) -> Option<&'a checked_trees::CheckedUnitStructuralTypePlan> {
        self.structural_types()
            .find(|shape| shape.identity == identity)
    }
}

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
    /// Proof-only erased scalar formals in authored order.
    pub(crate) erased_scalar_parameters: &'a [CheckedStructuralScalarParameterPlan],
    pub(crate) erased_proof_parameters: &'a [CheckedErasedProofParameterPlan],
    pub(crate) entry_claims: &'a [CheckedUnitEntryClaimPlan],
    pub(crate) contract_report_fingerprint: u64,
    pub(crate) contract_service_reach: ServiceReachPlan,
}

impl<'a> UnitBody<'a> {
    /// Body membership routes closure traversal, not validation. find still
    /// rejects duplicate or missing owners before a body is consumed.
    pub(crate) fn contains(plans: UnitPlans<'_>, symbol: symbols::SymbolHandle) -> bool {
        plans.for_machine(symbol).is_some() || plans.composed_for_machine(symbol).is_some()
    }

    pub(crate) fn result(self) -> Result<checked_trees::CheckedControlResultPlan, LoweringError> {
        if matches!(self, Self::Ordinary(plan) if plan.scalar_result.is_some() || plan.scalar_control.is_some())
        {
            return unsupported("scalar operation-body result requires a scalar call catalog");
        }
        Ok(match self {
            Self::Ordinary(plan) => plan.structural_result.as_ref().map_or(
                checked_trees::CheckedControlResultPlan::Unit,
                |result| {
                    checked_trees::CheckedControlResultPlan::Structural(
                        checked_trees::CheckedStructuralResultPlan {
                            type_identity: result.type_identity.clone(),
                            multiplicity: result.multiplicity,
                            qualifications: Vec::new(),
                            projected_qualifications: Vec::new(),
                        },
                    )
                },
            ),
            Self::Composed(plan) => plan.result.clone(),
        })
    }
    pub(crate) fn find(
        plans: UnitPlans<'a>,
        symbol: symbols::SymbolHandle,
    ) -> Result<Self, LoweringError> {
        let mut matches = plans
            .machines()
            .filter(|plan| plan.machine == symbol)
            .map(Self::Ordinary)
            .chain(
                plans
                    .composed_machines()
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
                erased_scalar_parameters: &plan.erased_scalar_parameters,
                erased_proof_parameters: &plan.erased_proof_parameters,
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
                    erased_scalar_parameters: &state.erased_scalar_parameters,
                    erased_proof_parameters: &state.erased_proof_parameters,
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
            .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
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
