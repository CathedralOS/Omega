//! A callable read through a formal belongs to that projection of the
//! caller's value. Keep field names in source coordinates until the
//! executable record declarations intern them.
use super::{E, Expression, IdTable, Invocation, RecordType, StorageShape, ValueType};
use crate::publication::reach::InvocationStep;
use semantic_vocabulary::{CallableContractId, ScalarType, StructuralTypeId};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Formal {
    pub position: u32,
    pub path: Vec<InvocationStep>,
}

impl Formal {
    pub(super) fn invocation(&self, suffix: &[InvocationStep]) -> Invocation {
        if suffix.is_empty() && self.path.is_empty() {
            return Invocation::Parameter(self.position);
        }
        let mut path = self.path.clone();
        path.extend_from_slice(suffix);
        Invocation::ParameterPath {
            parameter: self.position,
            path,
        }
    }
}

/// Edges describe which formal a private call forwards into another
/// private formal in the same recursive component. Appending a proper
/// subplace on a cycle requires infinitely many different finite paths.
#[derive(Default)]
pub(super) struct Forwarding {
    edges: std::collections::HashMap<Coordinate, Vec<Coordinate>>,
}
type Coordinate = (semantic_vocabulary::MachineId, u32);

impl Forwarding {
    pub(super) fn of(
        calls: &IdTable<semantic_vocabulary::MachineId, Vec<super::Use<'_>>>,
        policies: &IdTable<semantic_vocabulary::MachineId, super::Policy>,
        signatures: &IdTable<semantic_vocabulary::MachineId, super::Signature>,
        components: &crate::components::Components,
    ) -> Self {
        let mut flow = Self::default();
        for (caller, uses) in calls.iter() {
            if policies
                .get(caller)
                .is_none_or(|policy| policy.invokes.is_some())
            {
                continue;
            }
            for call in uses {
                let super::CallTarget::Machine(callee) = call.target else {
                    continue;
                };
                if call.required_contract.is_some()
                    || call.static_binding.is_some()
                    || !components.same_component(
                        crate::components::index(caller),
                        crate::components::index(*callee),
                    )
                {
                    continue;
                }
                let Some(signature) = signatures.get(*callee) else {
                    continue;
                };
                for (source_position, formal) in &call.parameter_arguments {
                    if let Some(position) = signature
                        .runtime_parameter_position(*source_position)
                        .and_then(|n| u32::try_from(n).ok())
                    {
                        flow.edges
                            .entry((*callee, position))
                            .or_default()
                            .push((caller, formal.position));
                    }
                }
            }
        }
        flow
    }

    pub(super) fn grows(
        &self,
        call: &super::Use<'_>,
        caller: semantic_vocabulary::MachineId,
        parameter: u32,
        formal: &Formal,
        suffix: &[InvocationStep],
    ) -> bool {
        let super::CallTarget::Machine(callee) = call.target else {
            return false;
        };
        if suffix.is_empty()
            || formal.path.is_empty()
            || call.required_contract.is_some()
            || call.static_binding.is_some()
        {
            return false;
        }
        let from = (*callee, parameter);
        let to = (caller, formal.position);
        self.edges.get(&from).is_some_and(|next| next.contains(&to)) && self.returns_to(to, from)
    }

    fn returns_to(&self, from: Coordinate, to: Coordinate) -> bool {
        let mut pending = vec![from];
        let mut visited = std::collections::HashSet::new();
        while let Some(at) = pending.pop() {
            if at == to {
                return true;
            }
            if visited.insert(at)
                && let Some(next) = self.edges.get(&at)
            {
                pending.extend_from_slice(next);
            }
        }
        false
    }
}

fn record(
    records: &IdTable<StructuralTypeId, RecordType>,
    ty: Option<ValueType>,
) -> Option<&RecordType> {
    let ValueType::Record(id, _, _) = ty? else {
        return None;
    };
    records.get(id)
}

pub(super) fn projection<'a>(
    records: &IdTable<StructuralTypeId, RecordType>,
    expression: &'a Expression,
) -> Option<(&'a Expression, Vec<InvocationStep>)> {
    match &expression.kind {
        E::Field { base, field } => Some((
            base,
            vec![InvocationStep::Field(
                record(records, base.result)?
                    .fields()
                    .get(*field)?
                    .identity
                    .clone(),
            )],
        )),
        E::Payload { base, case, field } => {
            let case = record(records, base.result)?.cases().get(*case)?;
            Some((
                base,
                vec![
                    InvocationStep::Case(case.id),
                    InvocationStep::Field(case.fields.get(*field)?.identity.clone()),
                ],
            ))
        }
        E::Index { base, index } => {
            let E::Integer(value) = &index.kind else {
                return None;
            };
            let index = value.to_u64()?;
            let StorageShape::FixedArray { length, .. } = record(records, base.result)?.shape
            else {
                return None;
            };
            (index < length.literal()?).then_some((base, vec![InvocationStep::FixedIndex(index)]))
        }
        E::Referent(base) => Some((base, vec![InvocationStep::Referent])),
        _ => None,
    }
}

/// When a callee invokes a field of a local actual, use that field's
/// published contract. An implementation selected for one package does not
/// narrow the callable contract of a different package of the same type.
pub(super) fn callable_at(
    records: &IdTable<StructuralTypeId, RecordType>,
    actual: Option<ValueType>,
    path: &[InvocationStep],
) -> Option<CallableContractId> {
    let mut ty = actual?;
    let mut path = path.iter();
    while let Some(step) = path.next() {
        let owner = record(records, Some(ty))?;
        ty = match step {
            InvocationStep::Field(name) => {
                owner
                    .fields()
                    .iter()
                    .find(|field| field.identity == *name)?
                    .value_type
            }
            InvocationStep::Case(id) => {
                let case = owner.cases().iter().find(|case| case.id == *id)?;
                let InvocationStep::Field(name) = path.next()? else {
                    return None;
                };
                case.fields
                    .iter()
                    .find(|field| field.identity == *name)?
                    .value_type
            }
            InvocationStep::FixedIndex(index) => {
                let StorageShape::FixedArray {
                    element, length, ..
                } = owner.shape
                else {
                    return None;
                };
                if *index >= length.literal()? {
                    return None;
                }
                ValueType::record(element)
            }
            InvocationStep::Referent => {
                let StorageShape::Reference { referent, .. } = owner.shape else {
                    return None;
                };
                ValueType::record(referent)
            }
        };
    }
    match ty {
        ValueType::Scalar(ScalarType::Callable(id), _) => Some(id),
        ValueType::Record(id, _, _) => match records.get(id)?.shape {
            StorageShape::PrimitiveScalar {
                scalar_type: ScalarType::Callable(id),
                ..
            } => Some(id),
            _ => None,
        },
        _ => None,
    }
}
