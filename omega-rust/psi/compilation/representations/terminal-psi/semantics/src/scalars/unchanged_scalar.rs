//! Identity-preserving scalar value flow, without arithmetic denotation.
//! Every predecessor must supply the same typed identity. Unknowns and cycles
//! withhold evidence; one invocation memoizes the immutable graph it observes.

use semantic_vocabulary::ValueId;

pub enum UnchangedScalarOrigin<Identity> {
    Known(Identity),
    Incoming(Vec<ValueId>),
}

pub fn unchanged_scalar_identity<Identity: Clone + Eq>(
    value: ValueId,
    mut origin: impl FnMut(ValueId) -> Option<UnchangedScalarOrigin<Identity>>,
) -> Option<Identity> {
    fn resolve<Identity: Clone + Eq>(
        value: ValueId,
        origin: &mut impl FnMut(ValueId) -> Option<UnchangedScalarOrigin<Identity>>,
        active: &mut Vec<ValueId>,
        completed: &mut Vec<(ValueId, Option<Identity>)>,
    ) -> Option<Identity> {
        if let Some((_, result)) = completed.iter().find(|(known, _)| *known == value) {
            return result.clone();
        }
        if active.contains(&value) {
            return None;
        }
        active.push(value);
        let result = (|| match origin(value)? {
            UnchangedScalarOrigin::Known(identity) => Some(identity),
            UnchangedScalarOrigin::Incoming(values) => {
                let mut values = values.into_iter();
                let first = resolve(values.next()?, origin, active, completed)?;
                for value in values {
                    if resolve(value, origin, active, completed)? != first {
                        return None;
                    }
                }
                Some(first)
            }
        })();
        active.pop();
        completed.push((value, result.clone()));
        result
    }
    resolve(value, &mut origin, &mut Vec::new(), &mut Vec::new())
}
