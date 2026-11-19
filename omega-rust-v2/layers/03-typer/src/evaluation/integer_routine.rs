use super::integer_arithmetic;
use crate::{IntegerIndex, ValueType};
use omega_resolver::DeclarationId;
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::Arc,
};

pub(crate) type EvaluationCache = HashMap<DeclarationId, HashMap<Vec<num_bigint::BigInt>, IntegerIndex>>;
pub struct IntegerRoutine {
    pub declaration: DeclarationId,
    pub parameters: Vec<ValueType>,
    pub result: ValueType,
    pub(super) body: Option<IntegerIndex>,
    pub(super) values: HashMap<Vec<num_bigint::BigInt>, IntegerIndex>,
    pub(super) intermediate: Vec<(usize, IntegerIndex, ValueType)>,
}
impl std::fmt::Debug for IntegerRoutine {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        self.declaration.fmt(f)
    }
}
// Open application identity is the exact selected declaration and its inputs.
// Its private implementation participates only when the inputs close.
impl PartialEq for IntegerRoutine {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.declaration == other.declaration
    }
}
impl Eq for IntegerRoutine {}
impl Hash for IntegerRoutine {
    fn hash<H: Hasher>(
        &self,
        state: &mut H,
    ) {
        self.declaration.hash(state)
    }
}

impl IntegerRoutine {
    pub(crate) fn apply(
        self: &Arc<Self>,
        arguments: Vec<IntegerIndex>,
    ) -> IntegerIndex {
        self.evaluate(arguments, &mut EvaluationCache::new())
    }
    pub(crate) fn evaluate(
        self: &Arc<Self>,
        arguments: Vec<IntegerIndex>,
        cache: &mut EvaluationCache,
    ) -> IntegerIndex {
        if let Some(IntegerIndex::Invalid(reason)) = arguments.iter().find(|a| matches!(a, IntegerIndex::Invalid(_))) {
            return IntegerIndex::Invalid(reason.clone());
        }
        if !arguments.iter().all(|a| matches!(a, IntegerIndex::Value(_))) {
            return IntegerIndex::Application {
                routine: self.clone(),
                arguments,
            };
        }
        let key: Vec<_> = arguments
            .iter()
            .map(|arg| match arg {
                IntegerIndex::Value(v) => v.clone(),
                _ => unreachable!(),
            })
            .collect();
        if let Some(value) = self
            .values
            .get(&key)
            .or_else(|| cache.get(&self.declaration)?.get(&key))
            .cloned()
        {
            return value;
        }
        let Some(body) = &self.body else {
            return IntegerIndex::Invalid(format!(
                "static call {}({}) needs an admitted evaluation result",
                self.declaration.path,
                key.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        };
        let args: Vec<_> = arguments
            .into_iter()
            .zip(&self.parameters)
            .map(|(term, ty)| term.typed(ty.clone()))
            .collect();
        for (_, term, carrier) in &self.intermediate {
            let result = term.evaluate(&self.declaration, &args, cache);
            if let IntegerIndex::Invalid(reason) = result {
                return IntegerIndex::Invalid(reason);
            }
            if let IntegerIndex::Value(value) = result
                && !integer_arithmetic::fits(&value, carrier)
            {
                return IntegerIndex::Invalid(format!("static index {value} exceeds {carrier}"));
            }
        }
        let value = body.evaluate(&self.declaration, &args, cache);
        cache
            .entry(self.declaration.clone())
            .or_default()
            .insert(key, value.clone());
        value
    }
}
