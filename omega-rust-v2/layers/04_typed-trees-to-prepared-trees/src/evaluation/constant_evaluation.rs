use std::cell::{OnceCell, RefCell};
use std::collections::{HashMap, HashSet, hash_map::Entry};
use std::sync::Arc;

use omega_resolved::DeclarationId;
use omega_typed::TypeError;
use omega_typed::{MachineEffects, TypedConstant, TypedTrees};

use crate::evaluation::integer_arithmetic::IntegerRange;
use crate::evaluation::owned_values::NominalAdmission;
use crate::evaluation::refusal;
use crate::invocation::EvaluationError;
use crate::invocation::admission::{InvocationProofs, ProviderAdmission};
use crate::invocation::static_callbacks::CallableRefinements;
use crate::invocation::termination::{SignatureIndex, TerminationAdmission};
use crate::reasons::{PreparationReason, PreparationRefusal};
use crate::{PreparedValue, TargetObservation, TargetSemantics};

#[derive(Debug)]
pub(crate) enum StaticDemandError {
    Refused(PreparationRefusal),
    OpenTarget(PreparationRefusal),
}

impl StaticDemandError {
    fn into_refusal(self) -> PreparationRefusal {
        match self {
            Self::Refused(refusal) | Self::OpenTarget(refusal) => refusal,
        }
    }
}

impl From<PreparationRefusal> for StaticDemandError {
    fn from(refusal: PreparationRefusal) -> Self {
        Self::Refused(refusal)
    }
}

/// How a checked judgment's target observations replay under this pass's target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ObservationReplay {
    Replays,
    NeedsTarget,
    Changed,
}

/// Values belong to this typing pass because it owns the selected calls.
pub(crate) struct ConstantEvaluation<'trees, 'text> {
    pub(crate) trees: &'trees TypedTrees<'text>,
    pub(crate) definitions: HashMap<&'trees DeclarationId, &'trees TypedConstant>,
    repeated: HashMap<&'trees DeclarationId, &'trees TypedConstant>,
    values: HashMap<DeclarationId, PreparedValue>,
    pub(crate) open_target_constants: HashMap<DeclarationId, Arc<TypeError>>,
    pub(crate) boundary_slots: &'trees [omega_typed::BoundarySlot],
    pub(crate) target: Option<&'trees TargetSemantics>,
    pub(crate) observations: RefCell<Vec<TargetObservation>>,
    pub(crate) dependencies: RefCell<crate::PreparationDependencies>,
    origin: Option<crate::PreparedValueOrigin>,
    signatures: OnceCell<SignatureIndex<'trees>>,
    effects: OnceCell<Vec<Option<&'trees MachineEffects>>>,
    pub(crate) termination: TerminationAdmission<'trees, 'text>,
    pub(crate) invocation_proofs: InvocationProofs<'trees, 'text>,
    pub(crate) providers: ProviderAdmission<'trees, 'text>,
    pub(crate) nominal: NominalAdmission<'trees, 'text>,
    pub(crate) callable_refinements: CallableRefinements<'trees, 'text>,
}

impl<'trees, 'text> ConstantEvaluation<'trees, 'text> {
    pub(crate) fn new(
        trees: &'trees TypedTrees<'text>,
        boundary_slots: &'trees [omega_typed::BoundarySlot],
    ) -> Self {
        // A pass may only consume some keys. Retain the first record and its
        // first conflict; neither traversal nor hash order selects a winner.
        let mut definitions = HashMap::with_capacity(trees.constants.len());
        let mut repeated = HashMap::new();
        for constant in &trees.constants {
            match definitions.entry(&constant.declaration) {
                Entry::Vacant(entry) => { entry.insert(constant); }
                Entry::Occupied(_) => { repeated.entry(&constant.declaration).or_insert(constant); }
            }
        }
        Self {
            trees,
            boundary_slots,
            definitions,
            repeated,
            values: HashMap::new(),
            open_target_constants: HashMap::new(),
            target: None,
            origin: None,
            dependencies: RefCell::default(),
            observations: RefCell::default(),
            signatures: OnceCell::new(),
            effects: OnceCell::new(),
            termination: TerminationAdmission::default(),
            invocation_proofs: InvocationProofs::default(),
            providers: ProviderAdmission::default(),
            nominal: NominalAdmission::default(),
            callable_refinements: CallableRefinements::default(),
        }
    }

    pub(crate) fn record_observation(
        &self,
        observation: TargetObservation,
    ) {
        if let Some(origin) = &self.origin {
            self.dependencies
                .borrow_mut()
                .record_observation(origin, &observation);
        }
        let mut observations = self.observations.borrow_mut();
        if !observations.contains(&observation) {
            observations.push(observation);
        }
    }

    /// Judge a checked proof's recorded target observations in this pass. A
    /// proof that records none replays everywhere. Without a selected target the
    /// proof is neither valid nor stale; its demand waits for target selection.
    pub(crate) fn replay_observations(
        &self,
        observations: &[TargetObservation],
    ) -> ObservationReplay {
        match self.target {
            _ if observations.is_empty() => ObservationReplay::Replays,
            None => ObservationReplay::NeedsTarget,
            Some(target) if observations.iter().all(|used| target.matches_observation(used)) => ObservationReplay::Replays,
            Some(_) => ObservationReplay::Changed,
        }
    }

    /// A refusal whose rule is missing target input leaves the request open,
    /// wherever execution located it. Layout and floating operations already
    /// return `OpenTarget` directly; proofs report their reason like any refusal.
    fn open_when_target_missing(
        &self,
        request: &omega_typed::StaticValueRequest,
        error: EvaluationError,
    ) -> EvaluationError {
        match error {
            EvaluationError::Refused(cause, node) if cause.awaits_target() => EvaluationError::OpenTarget(self.request_refusal(request, node, cause)),
            EvaluationError::Located(refusal) if refusal.cause.awaits_target() => EvaluationError::OpenTarget(refusal),
            error => error,
        }
    }

    pub(crate) fn record_constant_read(
        &self,
        constant: &DeclarationId,
    ) {
        if let Some(origin) = &self.origin {
            self.dependencies.borrow_mut().record_constant(origin, constant);
        }
    }

    /// A tail chain selects a declaration at each hop. Index the signatures
    /// once for this typing pass, when evaluation first encounters a call.
    pub(crate) fn signatures(&self) -> &SignatureIndex<'trees> {
        self.signatures.get_or_init(|| {
            let mut by_machine = HashMap::with_capacity(self.trees.signatures.len());
            for (index, signature) in self.trees.signatures.iter().enumerate() {
                by_machine
                    .entry(&signature.machine)
                    .or_insert((index, signature.as_ref()));
            }
            by_machine
        })
    }

    pub(crate) fn machine_effects(
        &self,
        signature: usize,
    ) -> Option<&MachineEffects> {
        self.effects.get_or_init(|| {
            let mut effects = vec![None; self.trees.signatures.len()];
            let signatures = self.signatures();
            for row in &self.trees.machine_effects {
                if let Some(&(index, _)) = signatures.get(&row.machine) {
                    effects[index] = Some(row.as_ref());
                }
            }
            effects
        })[signature]
    }

    pub(crate) fn frozen_value(&self, id: &DeclarationId) -> Option<&PreparedValue> {
        self.values.get(id)
    }

    pub(crate) fn finish(&mut self) -> Result<HashMap<DeclarationId, PreparedValue>, PreparationRefusal> {
        for constant in &self.trees.constants {
            self.materialize(constant, true)
                .map_err(StaticDemandError::into_refusal)?;
        }
        Ok(std::mem::take(&mut self.values))
    }

    #[cfg(test)]
    pub(crate) fn evaluate(
        &mut self,
        request: &omega_typed::StaticValueRequest,
        arguments: Option<&[omega_typed::ValueType]>,
    ) -> Result<omega_typed::StaticValue, TypeError> {
        let demand = match arguments {
            Some(arguments) => omega_typed::StaticValueDemand::Specialized {
                request: request.clone(),
                arguments: arguments.to_vec(),
            },
            None => omega_typed::StaticValueDemand::Closed(request.clone()),
        };
        self.evaluate_demand(&demand)
            .map_err(|error| error.into_refusal().diagnostic)
    }

    pub(crate) fn evaluate_demand(
        &mut self,
        demand: &omega_typed::StaticValueDemand,
    ) -> Result<omega_typed::StaticValue, StaticDemandError> {
        use omega_typed::StaticValueDemand;
        let request = demand.request();
        let arguments = match demand {
            StaticValueDemand::Closed(_) => None,
            StaticValueDemand::Specialized { arguments, .. } => Some(arguments.as_slice()),
        };
        loop {
            self.origin = Some(crate::PreparedValueOrigin::StaticIndex(demand.clone()));
            match self
                .evaluate_value_request(request, arguments)
                .map_err(|error| self.open_when_target_missing(request, error))
            {
                Ok(value) => {
                    return self.canonical_identity(&value, demand).map_err(|reason| {
                        let refusal = refusal(&request.expression, reason);
                        if reason.awaits_target() {
                            StaticDemandError::OpenTarget(refusal)
                        } else {
                            StaticDemandError::Refused(refusal)
                        }
                    });
                }
                Err(EvaluationError::OpenTarget(error)) => return Err(StaticDemandError::OpenTarget(error)),
                Err(EvaluationError::Located(error)) => return Err(error.into()),
                Err(EvaluationError::Refused(reason, node)) => return Err(self.request_refusal(request, node, reason).into()),
                Err(EvaluationError::NeedsConstant { declaration, reference }) => {
                    let definition = self.definition(&declaration, reference)?;
                    self.materialize(definition, false)?;
                }
            }
        }
    }

    fn request_refusal(
        &self,
        request: &omega_typed::StaticValueRequest,
        node: Option<omega_resolved::TreeNodeId>,
        reason: crate::RefusalCause,
    ) -> PreparationRefusal {
        let site = node.map(|node| self.visited_site(node));
        refusal(site.as_ref().unwrap_or(&request.expression), reason)
    }

    fn definition(
        &self,
        id: &DeclarationId,
        reference: omega_resolved::TreeNodeId,
    ) -> Result<&'trees TypedConstant, PreparationRefusal> {
        self.definitions
            .get(id)
            .copied()
            .ok_or_else(|| refusal(&self.visited_site(reference), PreparationReason::ConstantWithoutInitializer))
    }

    /// Admit a key before it enters the evaluation stack. Consequently neither
    /// a frozen value nor an open recipe can exist for a conflicted key.
    fn admit_initializer(&self, first: &TypedConstant) -> Result<(), PreparationRefusal> {
        let repeated = self.repeated.get(&first.declaration);
        let check = |constant: &TypedConstant| {
            let request = &constant.initializer;
            let at_rhs = |reason| refusal(&request.expression, reason);
            let tree = self.trees.resolved().trees().nth(request.node.file)
                .ok_or_else(|| at_rhs(PreparationReason::RequestWithoutTypedFile))?;
            let rhs = request.node.node;
            let node = tree.file.nodes.get(rhs).ok_or_else(|| at_rhs(PreparationReason::RequestWithoutTypedNode))?;
            let owner = node.parent.filter(|&p| tree.file.constant_initializer(p) == Some(rhs)).and_then(|p| tree.declaration_at(p));
            // Ordinary requests check their site in evaluate_value_request.
            // Identity refusals must also check it before citing the request.
            if (owner != Some(&constant.declaration) || repeated.is_some()) && omega_typed::site_at(tree.file, rhs) != request.expression {
                return Err(at_rhs(PreparationReason::RequestSiteMismatch));
            }
            if owner != Some(&constant.declaration) {
                return Err(at_rhs(PreparationReason::ConstantInitializerDeclarationMismatch));
            }
            Ok(())
        };
        check(first)?;
        if let Some(second) = repeated {
            check(second)?;
            return Err(refusal(&second.initializer.expression, PreparationReason::DuplicateConstantInitializer));
        }
        Ok(())
    }

    /// Guard the initializer's integer range at the only cache write, even for
    /// constants never read. A rejected value must not become visible to readers.
    fn publish_constant(&mut self, constant: &TypedConstant, value: PreparedValue) -> Result<(), PreparationRefusal> {
        let request = &constant.initializer;
        if let PreparedValue::Integer(integer) = &value {
            let range = IntegerRange::for_type(&request.carrier)
                .ok_or_else(|| refusal(&request.expression, PreparationReason::ConstantCarrierMismatch))?;
            if !range.fits(integer) {
                return Err(refusal(&request.expression, PreparationReason::FrozenConstantOutsideCarrier));
            }
        }
        self.values.insert(constant.declaration.clone(), value);
        Ok(())
    }

    fn materialize(
        &mut self,
        root: &'trees TypedConstant,
        retain_open_recipes: bool,
    ) -> Result<(), StaticDemandError> {
        if self.values.contains_key(&root.declaration) || self.open_target_constants.contains_key(&root.declaration) {
            return Ok(());
        }
        // Follow the dependencies the evaluator actually reads. Keep the path
        // on the heap so a long chain of forward declarations can be prepared.
        self.admit_initializer(root)?;
        let mut stack = vec![root];
        let mut active = HashSet::from([&root.declaration]);
        let reason = loop {
            let Some(&constant) = stack.last() else { return Ok(()) };
            let request = &constant.initializer;
            self.origin = Some(crate::PreparedValueOrigin::Constant(constant.declaration.clone()));
            match self
                .evaluate_value_request(request, None)
                .map_err(|error| self.open_when_target_missing(request, error))
            {
                Ok(value) => {
                    self.publish_constant(constant, value)?;
                    active.remove(&constant.declaration);
                    stack.pop();
                }
                Err(EvaluationError::OpenTarget(error)) if retain_open_recipes => break Arc::new(error.diagnostic),
                Err(EvaluationError::OpenTarget(error)) => return Err(StaticDemandError::OpenTarget(error)),
                Err(EvaluationError::Located(error)) => return Err(error.into()),
                Err(EvaluationError::Refused(reason, node)) => return Err(self.request_refusal(request, node, reason).into()),
                Err(EvaluationError::NeedsConstant { declaration, reference }) => {
                    let dependency = self.definition(&declaration, reference)?;
                    if let Some(reason) = self.open_target_constants.get(&dependency.declaration) {
                        break reason.clone();
                    }
                    if !active.insert(&dependency.declaration) {
                        return Err(refusal(&self.visited_site(reference), PreparationReason::CyclicConstantInitializer).into());
                    }
                    self.admit_initializer(dependency)?;
                    stack.push(dependency);
                }
            }
        };
        // Every initializer on this path demanded the open target result.
        // Its existing typed initializer remains the recipe for a later target.
        self.open_target_constants.extend(
            stack
                .into_iter()
                .map(|constant| (constant.declaration.clone(), reason.clone())),
        );
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests;
