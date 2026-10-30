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

    pub(crate) fn structural_domains(self) -> &'a [checked_trees::CheckedUnitStructuralDomainPlan] {
        &self.checked.structural_domains
    }

    pub(crate) fn structural_types(
        self,
    ) -> impl Iterator<Item = &'a checked_trees::CheckedUnitStructuralTypePlan> {
        self.checked.structural_types.iter()
    }

    /// Exact declaration owners whose retained computations these emitted
    /// bodies read. A fused graph keeps each state's owner, independently of
    /// the dispatch head that receives its callable identity.
    pub(crate) fn source_machines(
        self,
        checked: &checked_trees::CheckedTrees,
        bodies: &[symbols::SymbolHandle],
    ) -> Result<Vec<symbols::SymbolHandle>, LoweringError> {
        let mut sources = Vec::new();
        for symbol in bodies {
            match UnitBody::find(self, *symbol)? {
                UnitBody::Ordinary(plan) => {
                    if !sources.contains(&plan.machine) {
                        sources.push(plan.machine);
                    }
                }
                UnitBody::Composed(plan) => {
                    for state in &plan.states {
                        let (owner, _) =
                            crate::declarations::state_declaration(checked, state.state)?;
                        if !sources.contains(&owner.symbol) {
                            sources.push(owner.symbol);
                        }
                    }
                }
            }
        }
        Ok(sources)
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

    /// The declared primitive result, whether or not any path completes it.
    /// Receiving homes describe completed values, never a callable's ABI.
    pub(crate) fn scalar_result_type(self) -> Option<checked_trees::types::PrimitiveType> {
        let result = match self {
            Self::Ordinary(plan) => &plan.result,
            Self::Composed(plan) => &plan.result,
        };
        match result {
            checked_trees::CheckedControlResultPlan::Scalar { primitive_type } => {
                Some(*primitive_type)
            }
            _ => None,
        }
    }

    pub(crate) fn result(self) -> Result<checked_trees::CheckedControlResultPlan, LoweringError> {
        Ok(match self {
            Self::Ordinary(plan) => plan.result.clone(),
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
        self.operations_with_states()
            .map(|(_, operation)| operation)
    }

    /// Source states remain attached to the same complete operation inventory,
    /// so call-local dependencies cannot borrow another state's facts.
    pub(crate) fn operations_with_states(
        self,
    ) -> impl Iterator<Item = (symbols::SymbolHandle, &'a CheckedUnitEffectOperationPlan)> {
        let (ordinary, composed) = match self {
            Self::Ordinary(plan) => (Some(plan), &[][..]),
            Self::Composed(plan) => (None, plan.states.as_slice()),
        };
        ordinary
            .into_iter()
            .flat_map(|plan| {
                plan.operations
                    .iter()
                    .map(move |operation| (plan.state, operation))
            })
            .chain(composed.iter().flat_map(|state| {
                state
                    .operation_dependencies()
                    .map(move |operation| (state.state, operation))
            }))
            .flat_map(|(state, operation)| {
                operation
                    .with_value_calls()
                    .map(move |operation| (state, operation))
            })
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

    /// Structural source homes admitted by this state's retained bindings,
    /// including primitive storage established before lending an address.
    pub(crate) fn structural_home_roots(
        self,
        checked: &checked_trees::CheckedTrees,
        state: symbols::SymbolHandle,
    ) -> Result<Vec<facts::PlaceRoot>, LoweringError> {
        let (_, declaration) = crate::declarations::state_declaration(checked, state)?;
        let (parameters, scalar_parameters, storage) = match self {
            Self::Ordinary(plan) if plan.state == state => (
                plan.structural_parameters.as_slice(),
                plan.scalar_parameters.as_slice(),
                plan.parameter_storage.as_slice(),
            ),
            Self::Ordinary(_) => (&[][..], &[][..], &[][..]),
            Self::Composed(plan) => plan.states.iter().find(|plan| plan.state == state).map_or(
                (&[][..], &[][..], &[][..]),
                |plan| {
                    (
                        plan.structural_parameters.as_slice(),
                        plan.scalar_parameters.as_slice(),
                        plan.parameter_storage.as_slice(),
                    )
                },
            ),
        };
        let mut roots = Vec::new();
        for parameter in parameters {
            let formal = checked
                .state_parameters(declaration)
                .get(parameter.position as usize)
                .ok_or(LoweringError::Unsupported(
                    "call membership home lost its structural formal",
                ))?;
            roots.push(facts::PlaceRoot::Symbol(formal.symbol));
        }
        for storage in storage {
            roots.push(facts::PlaceRoot::Symbol(storage.symbol));
        }
        for (_, operation) in self
            .operations_with_states()
            .filter(|(owner, _)| *owner == state)
        {
            if let Some(result) = operation.structural_result_binding()
                && result.local.is_named()
            {
                let root = facts::PlaceRoot::Symbol(result.local.symbol);
                if !roots.contains(&root) {
                    roots.push(root);
                }
            }
            if let checked_trees::CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::Structural { symbol, .. }
                | checked_trees::CheckedUnitLocalBindingPlan::Primitive { symbol, .. },
            ) = operation
            {
                let root = facts::PlaceRoot::Symbol(*symbol);
                if !roots.contains(&root) {
                    roots.push(root);
                }
            }
            if let Some(call) = operation.call() {
                for argument in &call.structural_arguments {
                    use checked_trees::CheckedUnitStructuralArgumentSourcePlan as Source;
                    let root = match argument.source {
                        Source::PrimitiveLocal { symbol }
                        | Source::StructuralLocal { symbol }
                        | Source::StructuralLocalBorrow { symbol, .. } => {
                            Some(facts::PlaceRoot::Symbol(symbol))
                        }
                        Source::StructuralExpression { expression } => {
                            Some(facts::PlaceRoot::Expression(expression))
                        }
                        Source::ScalarParameter { parameter_index } => scalar_parameters
                            .get(parameter_index as usize)
                            .and_then(|parameter| {
                                checked
                                    .state_parameters(declaration)
                                    .get(parameter.source_position as usize)
                            })
                            .map(|parameter| facts::PlaceRoot::Symbol(parameter.symbol)),
                        _ => None,
                    };
                    if let Some(root) = root
                        && !roots.contains(&root)
                    {
                        roots.push(root);
                    }
                }
            }
            if let Some(call) = operation.call()
                && call.structural_result().is_some()
                && let Some(checked_trees::NominalMachineUseSite::Expression(expression)) =
                    call.source_site()
            {
                let root = facts::PlaceRoot::Expression(expression);
                if !roots.contains(&root) {
                    roots.push(root);
                }
            }
        }
        Ok(roots)
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
