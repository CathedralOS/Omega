use std::cell::OnceCell;
use std::collections::{HashMap, HashSet};

use omega_resolver::DeclarationId;
use omega_typer::{StaticValueRequest, TypeError, TypedConstant, TypedTrees, ValueType};

use crate::invocation::EvaluationError;
use crate::invocation::termination::{SignatureIndex, TerminationAdmission};
use crate::{PreparedValue, refusal};

/// Values belong to this typing pass because it owns the selected calls.
pub(super) struct ConstantEvaluation<'trees, 'text> {
    pub(super) trees: &'trees TypedTrees<'text>,
    pub(super) definitions: HashMap<&'trees DeclarationId, &'trees TypedConstant>,
    pub(super) values: HashMap<DeclarationId, PreparedValue>,
    pub(super) boundary_slots: &'trees [omega_typer::BoundarySlot],
    signatures: OnceCell<SignatureIndex<'trees>>,
    pub(super) termination: TerminationAdmission<'trees, 'text>,
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
            signatures: OnceCell::new(),
            termination: TerminationAdmission::default(),
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

    pub(super) fn finish(mut self) -> Result<HashMap<DeclarationId, PreparedValue>, TypeError> {
        for constant in &self.trees.constants {
            self.materialize(constant)?;
        }
        Ok(self.values)
    }

    pub(super) fn evaluate(
        &mut self,
        request: &StaticValueRequest,
        arguments: Option<&[ValueType]>,
    ) -> Result<PreparedValue, TypeError> {
        loop {
            match self.evaluate_value_request(request, arguments) {
                Ok(value) => return Ok(value),
                Err(EvaluationError::AtCall(error)) => return Err(error),
                Err(EvaluationError::Refused(reason)) => return Err(refusal(&request.expression, reason)),
                Err(EvaluationError::NeedsConstant(id)) => {
                    let definition = self.definition(request, &id)?;
                    self.materialize(definition)?;
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
    ) -> Result<(), TypeError> {
        if self.values.contains_key(&root.declaration) {
            return Ok(());
        }
        // Follow the dependencies the evaluator actually reads. Keep the path
        // on the heap so a long chain of forward declarations can be prepared.
        let mut stack = vec![root];
        let mut active = HashSet::from([&root.declaration]);
        while let Some(&constant) = stack.last() {
            let request = &constant.initializer;
            match self.evaluate_value_request(request, None) {
                Ok(value) => {
                    self.values.insert(constant.declaration.clone(), value);
                    active.remove(&constant.declaration);
                    stack.pop();
                }
                Err(EvaluationError::AtCall(error)) => return Err(error),
                Err(EvaluationError::Refused(reason)) => return Err(refusal(&request.expression, reason)),
                Err(EvaluationError::NeedsConstant(id)) => {
                    let dependency = self.definition(request, &id)?;
                    if !active.insert(&dependency.declaration) {
                        return Err(refusal(&request.expression, "cyclic constant initializer"));
                    }
                    stack.push(dependency);
                }
            }
        }
        Ok(())
    }
}
