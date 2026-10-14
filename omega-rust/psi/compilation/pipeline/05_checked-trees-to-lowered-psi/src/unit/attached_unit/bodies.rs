//! Borrowed views of ordinary statement and graph bodies. Structural result
//! calls traverse either complete body through the same closure; result shape
//! does not select a different producer. No synthetic checked plans or states.
use super::{
    CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan, LoweringError, SemanticDomainId,
    ServiceReachSummary, unsupported,
};
use checked_trees::{CheckedComposedUnitControlMachinePlan, CheckedUnitStructuralParameterPlan};

/// The Unit plan roster one closure assembly reads: the checked
/// `terminal_unit_effects` lane as published, joined with the partial-affine
/// lane (`terminal_partial_affine_unit_cleanups`). Each partial row is an
/// ordinary body whose return edge owes a residual complement, published
/// beside those rows and callable like any other body; the closure installs
/// that complement on every emitted member. A nominal cleanup dispatcher is
/// the ordinary roster's own row for its machine. Every shape any lane names
/// lives in the ordinary roster's one structural type namespace, where stage
/// 04 registered or adopted it. A machine two lanes both publish is a
/// duplicate, exactly as two published rows would be.
#[derive(Clone, Copy)]
pub(crate) struct UnitPlans<'a> {
    checked: &'a checked_trees::CheckedUnitEffectPlans,
    partial_affine: &'a [checked_trees::CheckedPartialAffineUnitCleanupMachinePlan],
}

impl<'a> UnitPlans<'a> {
    /// The published roster: the ordinary lane and the partial-affine lane.
    pub(crate) fn published(flow: &'a checked_trees::FlowFacts) -> Self {
        Self {
            checked: &flow.terminal_unit_effects,
            partial_affine: &flow.terminal_partial_affine_unit_cleanups.machines,
        }
    }

    pub(crate) fn machines(self) -> impl Iterator<Item = &'a CheckedUnitEffectMachinePlan> {
        self.checked
            .machines
            .iter()
            .chain(self.partial_affine.iter().map(|plan| &plan.machine))
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

    /// Operand preparation consumes a callable's signature, not its body.
    /// Dispatch remains the checked call's decision; a boundary must never
    /// fall through to a module body or acquire a selected provider here.
    pub(crate) fn call_entry(
        self,
        call: &checked_trees::CheckedUnitCallPlan,
    ) -> Result<UnitEntry<'a>, LoweringError> {
        match call.route() {
            checked_trees::CheckedUnitCallRoute::Module { .. } => {
                UnitBody::find(self, call.target.machine)?.entry()
            }
            checked_trees::CheckedUnitCallRoute::Boundary => Ok(
                super::call_closure::unique_unit_boundary(self, call.target.machine)?
                    .callable_entry(),
            ),
            checked_trees::CheckedUnitCallRoute::Dynamic { .. } => {
                unsupported("structural expression calls require selector dispatch lowering")
            }
        }
    }

    pub(crate) fn structural_domains(self) -> &'a [checked_trees::CheckedUnitStructuralDomainPlan] {
        &self.checked.structural_domains
    }

    pub(crate) fn structural_types(
        self,
    ) -> impl Iterator<Item = &'a checked_trees::CheckedUnitStructuralTypePlan> {
        self.checked.structural_types.iter()
    }
}

#[derive(Clone, Copy)]
pub(crate) enum UnitBody<'a> {
    Ordinary(&'a CheckedUnitEffectMachinePlan),
    Composed(&'a CheckedComposedUnitControlMachinePlan),
}

pub(crate) type UnitEntry<'a> = checked_trees::CheckedUnitCallableEntry<'a>;

impl<'a> UnitBody<'a> {
    /// Body membership routes closure traversal, not validation. find still
    /// rejects duplicate or missing owners before a body is consumed.
    pub(crate) fn contains(plans: UnitPlans<'_>, symbol: symbols::SymbolHandle) -> bool {
        plans.for_machine(symbol).is_some() || plans.composed_for_machine(symbol).is_some()
    }

    /// The primitive scalar this body completes with, when it owns one: an
    /// ordinary body's scalar result binding or scalar control, or a state
    /// graph's scalar result. Such a body is called through the scalar call
    /// lane, not as a Unit or structural call.
    pub(crate) fn scalar_result_type(self) -> Option<checked_trees::types::PrimitiveType> {
        match self {
            Self::Ordinary(plan) => plan
                .scalar_result
                .as_ref()
                .map(|result| result.primitive_type)
                .or_else(|| {
                    plan.scalar_control
                        .as_ref()
                        .map(|control| control.primitive_type)
                }),
            Self::Composed(plan) => match plan.result {
                checked_trees::CheckedControlResultPlan::Scalar { primitive_type } => {
                    Some(primitive_type)
                }
                _ => None,
            },
        }
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
            Self::Ordinary(plan) => plan.callable_entry(),
            Self::Composed(plan) => plan.callable_entry().ok_or(LoweringError::Unsupported(
                "composed Unit body has no entry state",
            ))?,
        })
    }

    /// Complete catalog dependencies, including structural producers carried by
    /// selected return terminators. This is not the runtime operation order.
    pub(crate) fn operations(self) -> impl Iterator<Item = &'a CheckedUnitEffectOperationPlan> {
        let (ordinary, composed) = match self {
            Self::Ordinary(plan) => (plan.operations.as_slice(), &[][..]),
            Self::Composed(plan) => (&[][..], plan.states.as_slice()),
        };
        ordinary
            .iter()
            .chain(
                composed
                    .iter()
                    .flat_map(|state| state.operation_dependencies()),
            )
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

    /// Entry storage for every `mut` scalar formal the body's states take.
    pub(crate) fn parameter_storage(
        self,
    ) -> impl Iterator<Item = &'a checked_trees::CheckedUnitParameterStoragePlan> {
        let (ordinary, composed) = match self {
            Self::Ordinary(plan) => (plan.parameter_storage.as_slice(), &[][..]),
            Self::Composed(plan) => (&[][..], plan.states.as_slice()),
        };
        ordinary
            .iter()
            .chain(composed.iter().flat_map(|state| &state.parameter_storage))
    }

    pub(crate) fn attachment(self) -> Option<typed_trees::type_identity::TypeIdentityId> {
        match self {
            Self::Ordinary(plan) => plan.attachment_type_identity,
            Self::Composed(plan) => plan.attachment_type_identity,
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

    /// The erased-proof roster of one state: an ordinary body's own, or the
    /// matching state row's inside a state graph.
    pub(crate) fn state_erased_proof_parameters(
        self,
        state: symbols::SymbolHandle,
    ) -> Result<&'a [checked_trees::CheckedErasedProofParameterPlan], LoweringError> {
        match self {
            Self::Ordinary(plan) if plan.state == state => Ok(&plan.erased_proof_parameters),
            Self::Composed(plan) => plan
                .state(state)
                .map(|state| state.erased_proof_parameters.as_slice())
                .ok_or(LoweringError::Unsupported(
                    "Unit body has no published state",
                )),
            Self::Ordinary(_) => unsupported("Unit body has no published state"),
        }
    }
}
