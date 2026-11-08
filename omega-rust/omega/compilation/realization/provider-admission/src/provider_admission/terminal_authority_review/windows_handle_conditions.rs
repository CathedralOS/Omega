//! Necessary scalar guards from every actual incoming control-flow path.
//!
//! A query never inserts its goal. Only an actual selected edge establishes a
//! Boolean condition. Joins intersect conditions; new scalar instances retire
//! old ones, and simultaneous edge bindings transport only captured values.
//! These mathematical conditions do not establish a native handle's origin.

use std::collections::BTreeMap;

use semantic_vocabulary::{IdTable, ScalarTerm, ScalarType, ValueId};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct ScalarConditions {
    facts: BTreeMap<ScalarTerm, bool>,
}

impl ScalarConditions {
    fn require(&mut self, mut term: ScalarTerm, mut polarity: bool) -> bool {
        loop {
            match term {
                ScalarTerm::Boolean(value) => return value == polarity,
                ScalarTerm::BooleanNot { operand } => {
                    term = *operand;
                    polarity = !polarity;
                }
                ScalarTerm::BooleanEqual { left, right } => match (&*left, &*right) {
                    (ScalarTerm::Boolean(value), _) => {
                        polarity = polarity == *value;
                        term = *right;
                    }
                    (_, ScalarTerm::Boolean(value)) => {
                        polarity = polarity == *value;
                        term = *left;
                    }
                    _ => {
                        term = ScalarTerm::BooleanEqual { left, right };
                        break;
                    }
                },
                _ => break,
            }
        }
        // Live structural observations need their own version proof. The
        // ordinary guard path uses immutable read SSA binders instead.
        if has_storage_observation(&term) {
            return true;
        }
        match self.facts.get(&term) {
            Some(previous) => *previous == polarity,
            None => {
                self.facts.insert(term, polarity);
                true
            }
        }
    }

    pub(super) fn proves(&self, term: &ScalarTerm, polarity: bool) -> bool {
        let mut opposite = self.clone();
        !opposite.require(term.clone(), !polarity)
    }

    fn retire(&mut self, definitions: &[ValueId]) {
        let mut definitions = definitions.to_vec();
        definitions.sort_unstable();
        definitions.dedup();
        self.facts
            .retain(|term, _| !mentions_any(term, &definitions));
    }

    fn intersect(&mut self, incoming: &Self) -> bool {
        let before = self.facts.len();
        self.facts
            .retain(|term, polarity| incoming.facts.get(term) == Some(polarity));
        self.facts.len() != before
    }

    fn arrival(
        &self,
        parameters: &[ValueId],
        bindings: &[(ValueId, ScalarTerm)],
    ) -> Result<Self, String> {
        let mut result = self.clone();
        result.retire(parameters);
        let mut renamed = Vec::new();
        for (parameter, actual) in bindings {
            if let ScalarTerm::Value { id, scalar_type } = actual {
                renamed.push((
                    *id,
                    Some(ScalarTerm::Value {
                        id: *parameter,
                        scalar_type: *scalar_type,
                    }),
                ));
            }
        }
        // A repeated actual keeps the first target, as the previous ordered
        // map did. Other copies remain conservative rather than invented.
        renamed.sort_by_key(|(actual, _)| *actual);
        renamed.dedup_by_key(|(actual, _)| *actual);
        let renamed = IdTable::from_rows_with_gaps(renamed, 65_536)
            .map_err(|reason| format!("guard arrival namespace is invalid: {reason:?}"))?;
        let mut overwritten = parameters
            .iter()
            .filter(|parameter| renamed.get(**parameter).is_none_or(Option::is_none))
            .copied()
            .collect::<Vec<_>>();
        overwritten.sort_unstable();
        overwritten.dedup();
        for (term, &polarity) in &self.facts {
            // Capture all right-hand sides before installing any formal. Old
            // target instances with no exact actual transport are forgotten.
            if mentions_any(term, &overwritten) {
                continue;
            }
            let mut transported = term.clone();
            rename_values(&mut transported, &renamed);
            result.require(transported, polarity);
        }
        Ok(result)
    }
}

fn mentions_any(term: &ScalarTerm, values: &[ValueId]) -> bool {
    matches!(term, ScalarTerm::Value { id, .. } if values.binary_search(id).is_ok())
        || term
            .operands()
            .into_iter()
            .any(|operand| mentions_any(operand, values))
}

fn has_storage_observation(term: &ScalarTerm) -> bool {
    matches!(
        term,
        ScalarTerm::StructuralRead { .. }
            | ScalarTerm::BooleanField { .. }
            | ScalarTerm::IntegerField { .. }
            | ScalarTerm::IeeeFloatField { .. }
            | ScalarTerm::ViewExtent { .. }
            | ScalarTerm::ByteSequencePredicate { .. }
    ) || term.operands().into_iter().any(has_storage_observation)
}

fn rename_values(term: &mut ScalarTerm, values: &IdTable<ValueId, Option<ScalarTerm>>) {
    if let ScalarTerm::Value { id, .. } = term {
        if let Some(Some(value)) = values.get(*id) {
            *term = value.clone();
        }
    } else {
        for operand in term.operands_mut() {
            rename_values(operand, values);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};

    fn value(id: u64) -> ScalarTerm {
        ScalarTerm::value(
            ValueId::new(id).unwrap(),
            ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap()),
        )
    }

    fn zero_test(id: u64) -> ScalarTerm {
        let scalar_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
        ScalarTerm::IntegerEqual {
            scalar_type,
            left: Box::new(value(id)),
            right: Box::new(ScalarTerm::Integer {
                scalar_type,
                value: IntegerValue::Signed(0),
            }),
        }
    }

    #[test]
    fn guards_follow_simultaneous_actuals_and_expire_on_renewal_or_join() {
        let first = ValueId::new(1).unwrap();
        let second = ValueId::new(2).unwrap();
        let mut old = ScalarConditions::default();
        assert!(old.require(zero_test(1), false));
        assert!(old.proves(&zero_test(1), false));
        let mut swapped = old
            .arrival(&[first, second], &[(first, value(2)), (second, value(1))])
            .unwrap();
        assert!(swapped.proves(&zero_test(2), false));
        assert!(!swapped.proves(&zero_test(1), false));
        swapped.retire(&[second]);
        assert!(!swapped.proves(&zero_test(2), false));
        assert!(old.intersect(&ScalarConditions::default()));
        assert!(!old.proves(&zero_test(1), false));
        assert!(old.proves(&ScalarTerm::Boolean(true), true));
        assert!(!old.proves(&ScalarTerm::Boolean(false), true));
    }
}
