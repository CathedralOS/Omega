use std::cell::OnceCell;
use std::collections::{HashMap, HashSet};

use num_bigint::BigInt;
use omega_resolver::DeclarationId;
use omega_typer::{StaticIntegerRequest, TypeError, TypedConstant, TypedSignature, TypedTrees, ValueType};

use crate::integer_invocation::EvaluationError;
use crate::{PreparedValue, refusal};

/// Values belong to this typing pass because it owns the selected calls.
pub(super) struct ConstantEvaluation<'trees, 'text> {
    pub(super) trees: &'trees TypedTrees<'text>,
    pub(super) definitions: HashMap<&'trees DeclarationId, &'trees TypedConstant>,
    pub(super) values: HashMap<DeclarationId, PreparedValue>,
    signatures: OnceCell<HashMap<&'trees DeclarationId, &'trees TypedSignature>>,
}

impl<'trees, 'text> ConstantEvaluation<'trees, 'text> {
    pub(super) fn new(trees: &'trees TypedTrees<'text>) -> Self {
        Self {
            trees,
            definitions: trees
                .constants
                .iter()
                .map(|constant| (&constant.declaration, constant))
                .collect(),
            values: HashMap::new(),
            signatures: OnceCell::new(),
        }
    }

    /// A tail chain selects a declaration at each hop. Index the signatures
    /// once for this typing pass, when evaluation first encounters a call.
    pub(super) fn signature(
        &self,
        machine: &DeclarationId,
    ) -> Option<&'trees TypedSignature> {
        self.signatures
            .get_or_init(|| {
                let mut by_machine = HashMap::with_capacity(self.trees.signatures.len());
                for signature in &self.trees.signatures {
                    by_machine
                        .entry(&signature.machine)
                        .or_insert(signature.as_ref());
                }
                by_machine
            })
            .get(machine)
            .copied()
    }

    pub(super) fn finish(mut self) -> Result<HashMap<DeclarationId, PreparedValue>, TypeError> {
        for constant in &self.trees.constants {
            self.materialize(constant)?;
        }
        Ok(self.values)
    }

    pub(super) fn evaluate(
        &mut self,
        request: &StaticIntegerRequest,
        arguments: Option<&[ValueType]>,
    ) -> Result<BigInt, TypeError> {
        loop {
            match self.evaluate_request(request, arguments) {
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
        request: &StaticIntegerRequest,
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
            match self.owned_value(request) {
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
