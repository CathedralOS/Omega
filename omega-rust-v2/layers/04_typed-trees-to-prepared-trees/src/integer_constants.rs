use std::collections::{HashMap, HashSet};

use num_bigint::BigInt;
use omega_resolver::DeclarationId;
use omega_typer::{StaticIntegerRequest, TypeError, TypedConstant, TypedTrees, ValueType};

use crate::integer_invocation::{self, EvaluationError};
use crate::refusal;

/// Values belong to this typing pass because it owns the selected calls.
pub(super) struct IntegerEvaluation<'trees, 'text> {
    trees: &'trees TypedTrees<'text>,
    definitions: HashMap<&'trees DeclarationId, &'trees TypedConstant>,
    values: HashMap<DeclarationId, BigInt>,
}

impl<'trees, 'text> IntegerEvaluation<'trees, 'text> {
    pub(super) fn new(trees: &'trees TypedTrees<'text>) -> Self {
        Self {
            trees,
            definitions: trees
                .constants
                .iter()
                .map(|constant| (&constant.declaration, constant))
                .collect(),
            values: HashMap::new(),
        }
    }

    pub(super) fn finish(mut self) -> Result<HashMap<DeclarationId, BigInt>, TypeError> {
        for constant in &self.trees.constants {
            if matches!(
                constant.initializer.carrier.carrier(),
                ValueType::Integer { .. } | ValueType::IndexedInteger { .. }
            ) {
                self.materialize(constant)?;
            }
        }
        Ok(self.values)
    }

    pub(super) fn evaluate(
        &mut self,
        request: &StaticIntegerRequest,
        arguments: Option<&[ValueType]>,
    ) -> Result<BigInt, TypeError> {
        loop {
            match integer_invocation::evaluate(self.trees, request, arguments, &self.definitions, &self.values) {
                Ok(value) => return Ok(value),
                Err(EvaluationError::Refused(reason)) => return Err(refusal(request, reason)),
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
            .ok_or_else(|| refusal(request, "the selected constant has no local typed initializer"))
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
            match integer_invocation::evaluate(self.trees, request, None, &self.definitions, &self.values) {
                Ok(value) => {
                    self.values.insert(constant.declaration.clone(), value);
                    active.remove(&constant.declaration);
                    stack.pop();
                }
                Err(EvaluationError::Refused(reason)) => return Err(refusal(request, reason)),
                Err(EvaluationError::NeedsConstant(id)) => {
                    let dependency = self.definition(request, &id)?;
                    if !active.insert(&dependency.declaration) {
                        return Err(refusal(request, "cyclic integer constant initializer"));
                    }
                    stack.push(dependency);
                }
            }
        }
        Ok(())
    }
}
