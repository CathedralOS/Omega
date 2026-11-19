use std::cell::{OnceCell, RefCell};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use omega_resolver::DeclarationId;
use omega_typer::{MachineEffects, StaticValueRequest, TypeError, TypedConstant, TypedTrees};

use crate::invocation::EvaluationError;
use crate::invocation::admission::{InvocationProofs, ProviderAdmission};
use crate::invocation::static_callbacks::CallableRefinements;
use crate::invocation::termination::{SignatureIndex, TerminationAdmission};
use crate::owned_values::NominalAdmission;
use crate::{PreparedValue, TargetObservation, TargetSemantics, refusal};

#[derive(Debug)]
pub(super) enum StaticDemandError {
    Refused(TypeError),
    OpenTarget(TypeError),
}

impl StaticDemandError {
    fn into_diagnostic(self) -> TypeError {
        match self {
            Self::Refused(error) | Self::OpenTarget(error) => error,
        }
    }
}

impl From<TypeError> for StaticDemandError {
    fn from(error: TypeError) -> Self {
        Self::Refused(error)
    }
}

/// Values belong to this typing pass because it owns the selected calls.
pub(super) struct ConstantEvaluation<'trees, 'text> {
    pub(super) trees: &'trees TypedTrees<'text>,
    pub(super) definitions: HashMap<&'trees DeclarationId, &'trees TypedConstant>,
    pub(super) values: HashMap<DeclarationId, PreparedValue>,
    pub(super) open_target_constants: HashMap<DeclarationId, Arc<TypeError>>,
    pub(super) boundary_slots: &'trees [omega_typer::BoundarySlot],
    pub(super) target: Option<&'trees TargetSemantics>,
    pub(super) observations: RefCell<Vec<TargetObservation>>,
    pub(crate) dependencies: RefCell<crate::PreparationDependencies>,
    origin: Option<crate::PreparedValueOrigin>,
    signatures: OnceCell<SignatureIndex<'trees>>,
    effects: OnceCell<Vec<Option<&'trees MachineEffects>>>,
    pub(super) termination: TerminationAdmission<'trees, 'text>,
    pub(super) invocation_proofs: InvocationProofs<'trees, 'text>,
    pub(super) providers: ProviderAdmission<'trees, 'text>,
    pub(super) nominal: NominalAdmission<'trees, 'text>,
    pub(super) callable_refinements: CallableRefinements<'trees, 'text>,
}

impl<'trees, 'text> ConstantEvaluation<'trees, 'text> {
    pub(super) fn new(
        trees: &'trees TypedTrees<'text>,
        boundary_slots: &'trees [omega_typer::BoundarySlot],
    ) -> Self {
        Self {
            trees,
            boundary_slots,
            definitions: trees
                .constants
                .iter()
                .map(|constant| (&constant.declaration, constant))
                .collect(),
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

    pub(super) fn record_observation(
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

    pub(super) fn record_constant_read(
        &self,
        constant: &DeclarationId,
    ) {
        if let Some(origin) = &self.origin {
            self.dependencies.borrow_mut().record_constant(origin, constant);
        }
    }

    /// A tail chain selects a declaration at each hop. Index the signatures
    /// once for this typing pass, when evaluation first encounters a call.
    pub(super) fn signatures(&self) -> &SignatureIndex<'trees> {
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

    pub(super) fn machine_effects(
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

    pub(super) fn finish(&mut self) -> Result<HashMap<DeclarationId, PreparedValue>, TypeError> {
        for constant in &self.trees.constants {
            self.materialize(constant, true)
                .map_err(StaticDemandError::into_diagnostic)?;
        }
        Ok(std::mem::take(&mut self.values))
    }

    #[cfg(test)]
    pub(super) fn evaluate(
        &mut self,
        request: &StaticValueRequest,
        arguments: Option<&[omega_typer::ValueType]>,
    ) -> Result<omega_typer::StaticValue, TypeError> {
        let demand = match arguments {
            Some(arguments) => omega_typer::StaticValueDemand::Specialized {
                request: request.clone(),
                arguments: arguments.to_vec(),
            },
            None => omega_typer::StaticValueDemand::Closed(request.clone()),
        };
        self.evaluate_demand(&demand)
            .map_err(StaticDemandError::into_diagnostic)
    }

    pub(super) fn evaluate_demand(
        &mut self,
        demand: &omega_typer::StaticValueDemand,
    ) -> Result<omega_typer::StaticValue, StaticDemandError> {
        use omega_typer::StaticValueDemand;
        let request = demand.request();
        let arguments = match demand {
            StaticValueDemand::Closed(_) => None,
            StaticValueDemand::Specialized { arguments, .. } => Some(arguments.as_slice()),
        };
        loop {
            self.origin = Some(crate::PreparedValueOrigin::StaticIndex(demand.clone()));
            match self.evaluate_value_request(request, arguments) {
                Ok(value) => {
                    return self
                        .canonical_identity(&value, demand)
                        .ok_or_else(|| refusal(&request.expression, "the prepared value has no canonical static index identity").into());
                }
                Err(EvaluationError::OpenTarget(error)) => return Err(StaticDemandError::OpenTarget(error)),
                Err(EvaluationError::AtCall(error)) => return Err(error.into()),
                Err(EvaluationError::Refused(reason)) => return Err(refusal(&request.expression, reason).into()),
                Err(EvaluationError::NeedsConstant(id)) => {
                    let definition = self.definition(request, &id)?;
                    self.materialize(definition, false)?;
                }
            }
        }
    }

    fn definition(
        &self,
        request: &StaticValueRequest,
        id: &DeclarationId,
    ) -> Result<&'trees TypedConstant, TypeError> {
        self.definitions
            .get(id)
            .copied()
            .ok_or_else(|| refusal(&request.expression, "the selected constant has no local typed initializer"))
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
        let mut stack = vec![root];
        let mut active = HashSet::from([&root.declaration]);
        let reason = loop {
            let Some(&constant) = stack.last() else { return Ok(()) };
            let request = &constant.initializer;
            self.origin = Some(crate::PreparedValueOrigin::Constant(constant.declaration.clone()));
            match self.evaluate_value_request(request, None) {
                Ok(value) => {
                    self.values.insert(constant.declaration.clone(), value);
                    active.remove(&constant.declaration);
                    stack.pop();
                }
                Err(EvaluationError::OpenTarget(error)) if retain_open_recipes => break Arc::new(error),
                Err(EvaluationError::OpenTarget(error)) => return Err(StaticDemandError::OpenTarget(error)),
                Err(EvaluationError::AtCall(error)) => return Err(error.into()),
                Err(EvaluationError::Refused(reason)) => return Err(refusal(&request.expression, reason).into()),
                Err(EvaluationError::NeedsConstant(id)) => {
                    let dependency = self.definition(request, &id)?;
                    if let Some(reason) = self.open_target_constants.get(&dependency.declaration) {
                        break reason.clone();
                    }
                    if !active.insert(&dependency.declaration) {
                        return Err(refusal(&request.expression, "cyclic constant initializer").into());
                    }
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
