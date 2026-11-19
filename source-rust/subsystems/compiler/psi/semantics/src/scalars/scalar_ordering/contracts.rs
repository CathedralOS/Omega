use super::{Relation, Term};
use crate::exactly_one::exactly_one;
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm, ScalarType,
};

fn index(term: &ScalarTerm) -> Option<Term> {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).ok()?;
    match term {
        ScalarTerm::Value { id, scalar_type }
            if *scalar_type == ScalarType::Integer(integer) =>
        {
            Some(Term::Value(*id))
        }
        ScalarTerm::Integer {
            scalar_type,
            value: IntegerValue::Unsigned(value),
        } if *scalar_type == integer => u64::try_from(*value).ok().map(Term::Unsigned),
        _ => None,
    }
}

pub(crate) fn relations(propositions: &[Proposition]) -> Vec<Relation> {
    let mut pending = propositions.iter().collect::<Vec<_>>();
    let mut result = Vec::new();
    while let Some(proposition) = pending.pop() {
        let (left, right, strict, equality) = match proposition {
            Proposition::Conjunction(children) => {
                pending.extend(children);
                continue;
            }
            Proposition::Equal(left, right) => (left, right, false, true),
            Proposition::LessThan(left, right) => (left, right, true, false),
            Proposition::LessOrEqual(left, right) => (left, right, false, false),
            _ => continue,
        };
        let (Some(left), Some(right)) = (index(left), index(right)) else {
            continue;
        };
        result.push(Relation {
            left,
            right,
            strict,
        });
        if equality {
            result.push(Relation {
                left: right,
                right: left,
                strict: false,
            });
        }
    }
    result
}

pub fn instantiate_contract_ordering(
    proposition: &Proposition,
    bindings: &[(semantic_vocabulary::ValueId, ScalarTerm)],
) -> Vec<Proposition> {
    let integer =
        IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is a valid integer type");
    let instantiate = |term| -> Option<ScalarTerm> {
        Some(match term {
            Term::Unsigned(value) => ScalarTerm::Integer {
                scalar_type: integer,
                value: IntegerValue::Unsigned(u128::from(value)),
            },
            Term::Value(formal) => {
                let (_, actual) =
                    exactly_one(bindings.iter(), |(value, _)| *value == formal)?;
                index(actual)?;
                actual.clone()
            }
        })
    };
    relations(std::slice::from_ref(proposition))
        .into_iter()
        .filter_map(|relation| {
            let left = instantiate(relation.left)?;
            let right = instantiate(relation.right)?;
            Some(if relation.strict {
                Proposition::LessThan(left, right)
            } else {
                Proposition::LessOrEqual(left, right)
            })
        })
        .collect()
}
